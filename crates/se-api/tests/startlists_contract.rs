//! `putStartlisten` against Rosson 3.3.15's own uploads (golden/corpus/<show>/api/startlists-*.json):
//! for every published class of every corpus show, the same start lists, line by line. The
//! fixtures were made with Rosson's German interface, so the words are German here.

use std::fs;
use std::path::Path;

mod common;
use common::{diff, Counts};
use se_api::startlists::{put_startlisten, Language};
use se_api::Keys;
use serde_json::Value;
use show::model::PublishedClassId;
use show::State;

/// A team's heading line (it carries the team's start number and time).
fn is_team_heading(z: &Value) -> bool {
    z["typ"] == "text" && (z.get("startnummer").is_some() || z.get("startzeit").is_some())
}

/// The start lists with their lines sorted, without their running number and team headings.
fn unordered(body: &Value) -> Value {
    let mut body = body.clone();
    for list in body["startlisten"].as_array_mut().unwrap() {
        let lines = list["zeilen"].as_array_mut().unwrap();
        lines.retain(|z| !is_team_heading(z));
        for l in lines.iter_mut() {
            l.as_object_mut().unwrap().remove("reihenfolge");
        }
        lines.sort_by_key(|l| l.to_string());
    }
    body
}

#[test]
fn start_list_uploads_match_rosson() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let keys = Keys { access_key: "REDACTED".into(), rosson_key: "<rosson_key>".into(), rosson_version: "3.3.15".into() };
    let mut problems: Vec<String> = Vec::new();
    let (mut published, mut lists, mut lines) = (0, 0, 0);
    let mut counts = Counts::default();
    let mut notes: Vec<String> = Vec::new();

    let mut shows: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("api").is_dir()).collect();
    shows.sort();
    for show in shows {
        let snapshot: Value = serde_json::from_str(&fs::read_to_string(show.join("snapshot.json")).unwrap()).unwrap();
        let (state, _) = State::from_log(rosson_import::ops::to_operations(&snapshot).unwrap());
        let label = show.file_name().unwrap().to_string_lossy().to_string();
        let mut files: Vec<_> = fs::read_dir(show.join("api")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("startlists-")).collect();
        files.sort();
        for file in files {
            let fixture: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
            let theirs = &fixture["requests"].as_array().unwrap().iter().find(|r| r["path"] == "/putStartlisten").unwrap()["body"];
            let id = theirs["ausschreibung_pruefung_id"].as_u64().unwrap();
            published += 1;
            let ours = match put_startlisten(&state, &PublishedClassId(id), &keys, Language::De) {
                Ok(r) => r.body.unwrap(),
                Err(e) => {
                    problems.push(format!("{label} {id}: {e}"));
                    continue;
                }
            };
            lists += theirs["startlisten"].as_array().unwrap().len();
            lines += theirs["startlisten"].as_array().unwrap().iter().map(|l| l["zeilen"].as_array().unwrap().len()).sum::<usize>();
            // E36: in team classes with riders of a team no longer in the class, Rosson's sort
            // scrambles the order; there the lines are compared regardless of order.
            let scrambled = state.classes.values().filter(|c| c.published.0 == id && !c.teams.is_empty()).any(|c| {
                state.starts[&c.id].iter().any(|s| state.entries.get(&s.entry).and_then(|e| e.team.as_ref()).is_some_and(|t| !c.teams.contains(t)))
            });
            let (theirs, ours) = if scrambled {
                notes.push(format!("{label} {id}: team class with riders of a team no longer in it; Rosson's order is scrambled (E36), lines compared regardless of order"));
                // Team headings: Rosson's must be among ours (its scramble can lose one).
                let headings = |b: &Value| -> Vec<String> {
                    b["startlisten"].as_array().unwrap().iter().flat_map(|l| l["zeilen"].as_array().unwrap().iter()).filter(|z| is_team_heading(z)).map(|z| z["text"].to_string()).collect()
                };
                let ours_headings = headings(&ours);
                for h in headings(theirs).iter().filter(|h| !ours_headings.contains(h)) {
                    problems.push(format!("{label} {id}: Rosson's team heading {h} is not in ours"));
                }
                for h in ours_headings.iter().filter(|h| !headings(theirs).contains(h)) {
                    notes.push(format!("{label} {id}: team heading {h} only in ours (lost in Rosson's scramble)"));
                }
                (unordered(theirs), unordered(&ours))
            } else {
                (theirs.clone(), ours)
            };
            let mut differences = Vec::new();
            diff("", &theirs, &ours, &mut differences, &mut counts);
            for (path, what) in differences.iter().take(6) {
                problems.push(format!("{label} {id}{path}: {what}"));
            }
            if differences.len() > 6 {
                problems.push(format!("{label} {id}: … {} differences in all", differences.len()));
            }
        }
    }
    println!("\n{published} published classes, {lists} start lists, {lines} lines; {} values in another of Rosson's forms", counts.loose_types);
    for n in &notes {
        println!("  note: {n}");
    }
    for p in problems.iter().take(60) {
        println!("  {p}");
    }
    assert!(published > 50);
    assert!(problems.is_empty(), "{} differences with Rosson's start lists", problems.len());
}
