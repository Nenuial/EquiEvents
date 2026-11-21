//! The event upload, withdrawing start lists, and the sequence of requests of each upload,
//! against Rosson 3.3.15's own requests (golden/corpus/<show>/api/).

use std::fs;
use std::path::Path;

mod common;
use common::{diff, Counts};
use se_api::event::{delete_startlisten, put_veranstaltung, upload_results, upload_start_lists};
use se_api::startlists::Language;
use se_api::{Keys, Method, Request};
use serde_json::{json, Value};
use show::model::{ClassId, PublishedClassId};
use show::State;

fn keys() -> Keys {
    Keys { access_key: "REDACTED".into(), rosson_key: "<rosson_key>".into(), rosson_version: "3.3.15".into() }
}

/// A request as the fixtures record it.
fn recorded(r: &Request) -> Value {
    let method = match r.method {
        Method::Get => "GET",
        Method::Post => "POST",
        Method::Patch => "PATCH",
        Method::Delete => "DELETE",
    };
    json!({ "method": method, "path": r.path, "query": r.query.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<serde_json::Map<_, _>>(), "body": r.body.clone().unwrap_or(Value::Null) })
}

fn shows() -> Vec<(String, State, std::path::PathBuf)> {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let mut dirs: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("api").is_dir()).collect();
    dirs.sort();
    dirs.into_iter()
        .map(|dir| {
            let snapshot: Value = serde_json::from_str(&fs::read_to_string(dir.join("snapshot.json")).unwrap()).unwrap();
            let (state, _) = State::from_log(rosson_import::ops::to_operations(&snapshot).unwrap());
            (dir.file_name().unwrap().to_string_lossy().to_string(), state, dir.join("api"))
        })
        .collect()
}

fn fixture(path: &Path) -> Vec<Value> {
    let f: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    f["requests"].as_array().unwrap().clone()
}

#[test]
fn event_upload_and_withdrawal_match_rosson() {
    let mut problems = Vec::new();
    let mut counts = Counts::default();
    for (label, state, api) in shows() {
        let theirs = &fixture(&api.join("event.json"))[0];
        let mut differences = Vec::new();
        diff("", theirs, &recorded(&put_veranstaltung(&state, &keys())), &mut differences, &mut counts);
        problems.extend(differences.into_iter().map(|(p, w)| format!("{label} putVeranstaltung{p}: {w}")));

        let theirs = &fixture(&api.join("delete-startlists.json"))[0];
        let id: u64 = theirs["path"].as_str().unwrap().rsplit('/').next().unwrap().parse().unwrap();
        let mut differences = Vec::new();
        diff("", theirs, &recorded(&delete_startlisten(&PublishedClassId(id), &keys())), &mut differences, &mut counts);
        problems.extend(differences.into_iter().map(|(p, w)| format!("{label} deleteStartlisten{p}: {w}")));
    }
    for p in &problems {
        println!("  {p}");
    }
    assert!(problems.is_empty(), "{} differences", problems.len());
}

#[test]
fn each_upload_sends_what_rosson_sends_but_looks_up_only_new_officials() {
    let mut problems = Vec::new();
    let (mut uploads, mut rosson_lookups, mut our_lookups) = (0, 0, 0);
    for (label, state, api) in shows() {
        let mut files: Vec<_> = fs::read_dir(&api).unwrap().map(|e| e.unwrap().path()).collect();
        files.sort();
        for file in files {
            let name = file.file_name().unwrap().to_string_lossy().to_string();
            let theirs = fixture(&file);
            let ours = if let Some(n) = name.strip_prefix("results-") {
                let number = theirs[0]["body"]["pruefungen"][0]["nummer"].as_str().unwrap();
                let class = state.classes.values().find(|c| c.number == number).unwrap_or_else(|| panic!("{label} {n}"));
                upload_results(&state, &ClassId(class.id.0.clone()), &keys()).unwrap()
            } else if let Some(id) = name.strip_prefix("startlists-").and_then(|n| n.trim_end_matches(".json").parse::<u64>().ok()) {
                upload_start_lists(&state, &PublishedClassId(id), &keys(), Language::De).unwrap()
            } else {
                continue;
            };
            uploads += 1;
            let path = |r: &Value| format!("{} {}", r["method"].as_str().unwrap(), r["path"].as_str().unwrap());
            let ours: Vec<Value> = ours.iter().map(recorded).collect();
            // The uploads and the event, in the same order.
            let main = |v: &[Value]| v.iter().filter(|r| r["path"] != "/lookupZuweisung").map(path).collect::<Vec<_>>();
            if main(&theirs) != main(&ours) {
                problems.push(format!("{label} {name}: Rosson {:?}, ours {:?}", main(&theirs), main(&ours)));
            }
            let mut counts = Counts::default();
            let mut differences = Vec::new();
            let event = |v: &[Value]| v.iter().find(|r| r["path"] == "/putVeranstaltung").cloned().unwrap_or(Value::Null);
            diff("", &event(&theirs), &event(&ours), &mut differences, &mut counts);
            problems.extend(differences.into_iter().map(|(p, w)| format!("{label} {name} putVeranstaltung{p}: {w}")));
            // Lookups: ours are among Rosson's, and only for assignments without id.
            let lookups = |v: &[Value]| v.iter().filter(|r| r["path"] == "/lookupZuweisung").map(|r| r["query"].to_string()).collect::<Vec<_>>();
            let (t, o) = (lookups(&theirs), lookups(&ours));
            rosson_lookups += t.len();
            our_lookups += o.len();
            for q in o.iter().filter(|q| !t.contains(q)) {
                problems.push(format!("{label} {name}: lookup {q} that Rosson does not make"));
            }
        }
    }
    println!("\n{uploads} uploads; lookups: Rosson {rosson_lookups}, ours {our_lookups} (officials not looked up yet; here the state does not record the answers)");
    for p in problems.iter().take(40) {
        println!("  {p}");
    }
    assert!(uploads > 130);
    assert!(problems.is_empty(), "{} differences", problems.len());
}

#[test]
fn an_official_is_looked_up_once() {
    let (_, mut state, _) = shows().into_iter().find(|(l, _, _)| l == "2026.8352").unwrap();
    let class = state.classes.values().find(|c| c.number == "1").unwrap().id.clone();
    let lookups = |state: &State| upload_results(state, &class, &keys()).unwrap().iter().filter(|r| r.path == "/lookupZuweisung").count();
    let before = lookups(&state);
    assert!(before > 0);
    // Swiss Equestrian answers every lookup (some without id).
    let pending: Vec<_> = state.assignments.values().filter(|a| a.event.is_some() && a.se_id.is_none()).map(|a| a.id.clone()).collect();
    for (i, id) in pending.into_iter().enumerate() {
        let change = show::oplog::Change::AssignmentLookedUp { assignment: id, se_id: (i % 2 == 0).then_some(1000 + i as u64) };
        state.apply(&show::oplog::Operation { id: format!("lookup{i}"), stamp: show::oplog::Stamp { millis: u64::MAX / 2, counter: i as u32, device: "test".into() }, author: "test".into(), change });
    }
    assert_eq!(lookups(&state), 0, "after the answers, nobody is looked up again");
}
