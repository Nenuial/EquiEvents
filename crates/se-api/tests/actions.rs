//! The office's actions against a fake Swiss Equestrian: the requests go out in Rosson's order,
//! the answers become operations, failures are reported (docs/deviations.md E42).

use std::cell::RefCell;
use std::fs;
use std::path::Path;

mod common;
use common::block_on;
use se_api::actions::{self, Outcome};
use se_api::http::{HttpRequest, HttpResponse, Limits};
use se_api::Keys;
use serde_json::{json, Map, Value};
use show::model::*;
use show::oplog::{Change, Clock, Operation};
use show::State;

fn keys() -> Keys {
    Keys { access_key: "<access_key>".into(), rosson_key: "<rosson_key>".into(), rosson_version: "3.3.15".into() }
}

fn corpus(show: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus").join(show)
}

fn read(path: std::path::PathBuf) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn apply(state: &mut State, changes: Vec<Change>, clock: &mut Clock) {
    for change in changes {
        let stamp = clock.tick(u64::MAX / 2);
        state.apply(&Operation { id: format!("op{}", stamp.counter), stamp, author: "office".into(), change, saw: Default::default() });
    }
}

/// A fake server: `answer` decides per request; the paths asked, in order, are kept.
fn perform(steps: Vec<actions::Step>, answer: impl Fn(&HttpRequest) -> Result<HttpResponse, String>) -> (Outcome, Vec<String>) {
    let asked = RefCell::new(Vec::new());
    let outcome = block_on(actions::perform(
        steps,
        "",
        Limits::default(),
        |r| {
            asked.borrow_mut().push(r.url.split('?').next().unwrap().to_string());
            std::future::ready(answer(&r))
        },
        |_| std::future::ready(()),
    ));
    (outcome, asked.into_inner())
}

fn ok(body: Value) -> Result<HttpResponse, String> {
    Ok(HttpResponse { status: 200, status_text: "OK".into(), body: body.to_string().into_bytes() })
}

fn shown() -> State {
    let snapshot = read(corpus("2026.8352/snapshot.json"));
    State::from_log(rosson_import::ops::to_operations(&snapshot).unwrap()).0
}

#[test]
fn an_upload_is_recorded_and_officials_are_looked_up_once() {
    let mut state = shown();
    let class = state.classes.values().find(|c| c.number == "1").unwrap().id.clone();
    let steps = actions::results(&state, &class, &keys()).unwrap();
    let lookups = steps.len() - 2;
    assert!(lookups > 0);
    // Swiss Equestrian knows every other assignment.
    let n = RefCell::new(0);
    let (outcome, asked) = perform(steps, |r| {
        if r.url.starts_with("/lookupZuweisung") {
            *n.borrow_mut() += 1;
            ok(if *n.borrow() % 2 == 0 { json!({ "id": 900 + *n.borrow() }) } else { json!({}) })
        } else {
            ok(json!({}))
        }
    });
    assert_eq!(asked[..2], ["/putResultate".to_string(), "/putVeranstaltung".to_string()]);
    assert!(outcome.failed.is_none() && outcome.warnings.is_empty());
    assert_eq!(outcome.changes[0], Change::Uploaded { upload: Upload::Results(class.clone()), based_on: None });
    assert_eq!(outcome.changes.len(), 1 + lookups);
    let mut clock = Clock::new("office");
    apply(&mut state, outcome.changes, &mut clock);
    assert_eq!(state.uploaded[&Upload::Results(class.clone())].author, "office");
    let answered = |id: u64| id > 900 && id <= 900 + lookups as u64;
    assert_eq!(state.assignments.values().filter(|a| a.se_id.is_some_and(answered)).count(), lookups / 2);
    // The next upload asks nobody again.
    assert_eq!(actions::results(&state, &class, &keys()).unwrap().len(), 2);
}

/// Style judges are EquiEvents' own role (S21): never in a request to Swiss Equestrian, never
/// looked up, not in the Rosson export (Rosson would send them).
#[test]
fn style_judges_are_never_sent() {
    let mut state = shown();
    let class = state.classes.values().find(|c| c.number == "1").unwrap().id.clone();
    let (person, event) = state.assignments.values().map(|a| (a.person.clone(), a.event.clone())).next().unwrap();
    let style = Assignment {
        id: AssignmentId("style-judge".into()),
        class: Some(class.clone()),
        person,
        role: STYLE_JUDGE.into(),
        letter: None,
        role_id: None,
        event,
        se_id: None,
        on_event: None,
        responsible: None,
        readonly: Some(false),
        days: None,
        updated_at: Some(1),
        looked_up: false,
    };
    assert!(!style.for_swiss_equestrian());
    let before = actions::results(&state, &class, &keys()).unwrap().len();
    apply(&mut state, vec![Change::AssignmentSet { assignment: style }], &mut Clock::new("office"));
    let steps = actions::results(&state, &class, &keys()).unwrap();
    assert_eq!(steps.len(), before, "no lookup for it");
    for step in &steps {
        let body = step.request.body.as_ref().map(Value::to_string).unwrap_or_default();
        assert!(!body.contains("style-judge") && !body.contains("eq-style"), "{}", step.request.path);
    }
    let rosson = rosson_import::to_rosson::event_stores(&state, true, None).unwrap().to_string();
    assert!(!rosson.contains("style-judge") && !rosson.contains("eq-style"));
}

#[test]
fn a_refused_upload_changes_nothing_and_says_why() {
    let state = shown();
    let class = state.classes.values().find(|c| c.number == "1").unwrap().id.clone();
    let (outcome, asked) = perform(actions::results(&state, &class, &keys()).unwrap(), |_| {
        Ok(HttpResponse { status: 422, status_text: "Unprocessable Entity".into(), body: br#"{"message":"Pruefung 1 ist nicht bekannt"}"#.to_vec() })
    });
    assert_eq!(asked, ["/putResultate"]);
    assert!(outcome.changes.is_empty());
    assert_eq!(outcome.failed.unwrap().to_string(), "Swiss Equestrian refused the request (422): Pruefung 1 ist nicht bekannt");
}

#[test]
fn a_failed_follow_up_is_reported_and_the_rest_still_sent() {
    let state = shown();
    let class = state.classes.values().find(|c| c.number == "1").unwrap().id.clone();
    let steps = actions::results(&state, &class, &keys()).unwrap();
    let total = steps.len();
    let (outcome, asked) = perform(steps, |r| if r.url == "/putVeranstaltung" { Err("connection reset".into()) } else { ok(json!({})) });
    assert_eq!(asked.len(), total, "lookups still sent");
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("not updated"));
    assert_eq!(outcome.changes[0], Change::Uploaded { upload: Upload::Results(class), based_on: None });
}

#[test]
fn withdrawn_start_lists_are_no_longer_sent() {
    let mut state = shown();
    let published = state.published_classes.keys().next().unwrap().clone();
    let mut clock = Clock::new("office");
    for steps in [actions::start_lists(&state, &published, &keys(), se_api::startlists::Language::De).unwrap(), actions::withdraw_start_lists(&published, &keys())] {
        let (outcome, _) = perform(steps, |_| ok(json!({})));
        apply(&mut state, outcome.changes, &mut clock);
    }
    assert!(!state.uploaded.contains_key(&Upload::StartLists(published.clone())));
    assert!(state.uploaded.contains_key(&Upload::StartListsWithdrawn(published)));
}

/// Swiss Equestrian answering the loading requests with the corpus responses.
fn swiss_equestrian(show: &str, event: Value) -> impl Fn(&HttpRequest) -> Result<HttpResponse, String> {
    let snapshot = read(corpus(&format!("{show}/snapshot.json")));
    let strip = |rows: &Value| Value::Array(rows.as_array().into_iter().flatten().map(|r| Value::Object(r.as_object().unwrap().iter().filter(|(k, _)| !matches!(k.as_str(), "updated_at" | "searchterms")).map(|(k, v)| (k.clone(), v.clone())).collect())).collect());
    let stammdaten: Map<String, Value> = ["aufgabegruende", "disziplinen", "pruefungsfunktionen", "pruefungskategorien", "programme", "veranstalter", "veranstaltungsfunktionen", "wertungen"].iter().map(|s| (s.to_string(), strip(&snapshot[*s]))).collect();
    let offizielle = strip(&snapshot["offizielle"]);
    move |r: &HttpRequest| {
        let path = r.url.split('?').next().unwrap();
        match path {
            "/getOffizielle" => ok(json!({ "data": { "offizielle": offizielle } })),
            "/getStammdaten" => ok(json!({ "data": { "stammdaten": stammdaten } })),
            // Preparing, the first time it is asked.
            "/getVeranstaltung" if !r.url.contains("ready") => Ok(HttpResponse { status: 202, status_text: "Accepted".into(), body: vec![] }),
            "/getVeranstaltung" => ok(event.clone()),
            other => Err(format!("unexpected {other}")),
        }
    }
}

#[test]
fn loading_and_updating_through_the_network_layer() {
    let load = read(corpus("2026.8352/api/load.json"));
    let update = read(corpus("2026.8352/api/update.json"));
    // The access key carries "ready" after the first poll, so the fake answers 202 once.
    let polls = RefCell::new(0);
    let fetch = |state: Option<&State>, event: Value| {
        let server = swiss_equestrian("2026.8352", event);
        let mut n = 0;
        block_on(actions::fetch(
            state,
            &keys(),
            1_794_571_200_000,
            &mut || {
                n += 1;
                format!("new-{n}")
            },
            "",
            Limits::default(),
            |r| {
                let mut r = r;
                if r.url.starts_with("/getVeranstaltung") && *polls.borrow() > 0 {
                    r.url.push_str("&ready");
                }
                if r.url.starts_with("/getVeranstaltung") {
                    *polls.borrow_mut() += 1;
                }
                std::future::ready(server(&r))
            },
            |_| std::future::ready(()),
        ))
        .unwrap()
    };
    let changes = fetch(None, load["response"].clone());
    assert_eq!(*polls.borrow(), 2, "asked again after 202");
    let mut state = State::default();
    let mut clock = Clock::new("office");
    apply(&mut state, changes, &mut clock);
    assert_eq!(state.entries.len(), 452);
    *polls.borrow_mut() = 0;
    let changes = fetch(Some(&state), update["response"].clone());
    apply(&mut state, changes, &mut clock);
    assert_eq!(state.event.as_ref().unwrap().se_last_change.as_deref(), Some("2026-11-13 10:00:00"));
    assert_eq!(state.entries.len(), 454, "two new entries");
}

/// A final over team classes ranks the teams (matched by name, as Rosson's team finals) and goes
/// up as a row per team, then a row per rider of each team. (No corpus show has two team
/// classes: one class's teams, ranked as its own ranking ranks them.)
#[test]
fn a_team_final_goes_up_team_by_team() {
    let mut state = shown();
    let class = state.classes.values().find(|c| c.number == "7").unwrap().id.clone();
    let ranking = CombinedRanking { id: "teams".into(), name: "Finale des sociétés".into(), classes: vec![class.clone()], method: CombinedMethod::Ranks { tie_break: class.clone() }, number: Some("7.1".into()), extra_fields: Vec::new(), record: Value::Null };
    apply(&mut state, vec![Change::CombinedSet { ranking }], &mut Clock::new("office"));
    let lines = state.combined_ranking("teams").unwrap();
    assert!(lines.iter().all(|l| matches!(l.who, show::combined::Who::Team(_))));
    // The class's own team ranking.
    let own: Vec<(String, Option<u32>)> = state.ranking(&class).unwrap().lines.iter().filter(|l| l.team).map(|l| (state.teams[&TeamId(l.id.clone())].name.trim().to_string(), l.placement.rank())).collect();
    let combined: Vec<(String, Option<u32>)> = lines.iter().map(|l| (match &l.who { show::combined::Who::Team(n) => n.clone(), _ => unreachable!() }, l.rank)).collect();
    assert_eq!(combined.iter().filter(|(_, r)| r.is_some()).count(), own.iter().filter(|(_, r)| r.is_some()).count());
    assert_eq!(combined[0], own[0]);
    // Two teams of one name, both kept.
    assert!(combined.iter().any(|(n, _)| n == "Ecurie des Fantastiques (2)"));

    let request = se_api::results::put_final(&state, "teams", &keys()).unwrap();
    let body = request.body.unwrap();
    let rows = body["resultate"].as_array().unwrap();
    let teams: Vec<&Value> = rows.iter().filter(|r| r["team_resultat"] == true).collect();
    let members: Vec<&Value> = rows.iter().filter(|r| r["team_mitglied"] == true).collect();
    assert_eq!(teams.len(), lines.len());
    assert!(members.len() >= teams.len() * 2, "riders of each team");
    // Teams first, then their riders; a rider's rank is the team's.
    assert!(rows.iter().position(|r| r["team_mitglied"] == true).unwrap() >= teams.len());
    let first_team = teams[0]["team_id"].clone();
    assert!(members.iter().filter(|m| m["team_id"] == first_team).all(|m| m["rang"] == teams[0]["rang"]));
    assert_eq!(body["pruefungen"][0]["anzahl_gestartete_reiter"].as_u64().unwrap() as usize, teams.len());
}
