//! Loading an event from Swiss Equestrian, against Rosson 3.3.15 given the same responses
//! (golden/corpus/<show>/api/load.json, made by golden/src/load-fixtures.mjs).
//!
//! The responses are rebuilt from the corpus (past events cannot be fetched any more):
//! `getVeranstaltung` in the fixture, `getStammdaten` and `getOffizielle` from the snapshot's
//! master data and officials. Rosson loaded them into an empty database; what it stored is read
//! with `rosson-import`, the way an export of it would be. Our reading of the same responses
//! must give the same show: event, classes, people, horses, entries (with the rider and horses
//! written on them), teams, private data, officials.
//!
//! Not compared, by nature: assignment ids (Rosson draws them at random; Swiss Equestrian keeps
//! whichever it is sent), and the time Rosson stamps on master data.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs;
use std::path::Path;

use se_api::load::{get_offizielle, get_stammdaten, get_veranstaltung, read_event, read_master_data, read_officials};
use se_api::{Keys, Method, Request};
use serde_json::{json, Map, Value};
use show::model::*;
use show::oplog::{Clock, Operation};
use show::State;

/// When Rosson stored what it loaded (the fixtures fix the page's clock).
const LOADED_AT: u64 = 1_794_571_200_000;
const MASTER_DATA: [&str; 8] = ["aufgabegruende", "disziplinen", "pruefungsfunktionen", "pruefungskategorien", "programme", "veranstalter", "veranstaltungsfunktionen", "wertungen"];

fn keys() -> Keys {
    Keys { access_key: "<access_key>".into(), rosson_key: "<rosson_key>".into(), rosson_version: "3.3.15".into() }
}

fn recorded(r: &Request) -> Value {
    assert_eq!(r.method, Method::Get);
    json!({ "method": "GET", "path": r.path, "query": r.query.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<Map<_, _>>(), "body": null })
}

fn without(rows: &Value, fields: &[&str]) -> Value {
    Value::Array(
        rows.as_array()
            .into_iter()
            .flatten()
            .map(|r| Value::Object(r.as_object().unwrap().iter().filter(|(k, _)| !fields.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect()))
            .collect(),
    )
}

fn state_of(changes: Vec<show::oplog::Change>) -> State {
    let mut clock = Clock::new("test");
    let ops = changes.into_iter().enumerate().map(|(i, change)| Operation { id: format!("op{i}"), stamp: clock.tick(0), author: "test".into(), change }).collect();
    let (mut state, ignored) = State::from_log(ops);
    assert!(ignored.is_empty(), "{ignored:?}");
    // The records kept for writing Rosson's format come from Swiss Equestrian's answers here
    // and from what Rosson stored there: compared on their own (`records_kept`), not here.
    for e in state.event.iter_mut() {
        e.record = Value::Null;
    }
    for p in state.published_classes.values_mut() {
        p.record = Value::Null;
    }
    for e in state.entries.values_mut().chain(state.delivered_entries.values_mut()) {
        e.record = Value::Null;
    }
    state.master_records.clear();
    state
}

/// Differences between two collections, by key.
fn compare<K: Ord + Debug, V: PartialEq + Debug>(show: &str, what: &str, ours: &BTreeMap<K, V>, theirs: &BTreeMap<K, V>, out: &mut Vec<String>) {
    for (k, v) in ours {
        match theirs.get(k) {
            None => out.push(format!("{show} {what} {k:?}: only ours")),
            Some(t) if t != v => out.push(format!("{show} {what} {k:?}:\n  ours   {v:?}\n  Rosson {t:?}")),
            _ => {}
        }
    }
    for k in theirs.keys().filter(|k| !ours.contains_key(k)) {
        out.push(format!("{show} {what} {k:?}: only Rosson's"));
    }
}

#[test]
fn loading_an_event_matches_rosson() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let mut dirs: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("api/load.json").is_file()).collect();
    dirs.sort();
    assert!(dirs.len() >= 6, "load fixtures missing: run `npm run load` in golden/");

    let mut problems = Vec::new();
    let (mut entries, mut persons, mut horses, mut officials) = (0, 0, 0, 0);
    for dir in dirs {
        let show = dir.file_name().unwrap().to_string_lossy().to_string();
        let snapshot: Value = serde_json::from_str(&fs::read_to_string(dir.join("snapshot.json")).unwrap()).unwrap();
        let fixture: Value = serde_json::from_str(&fs::read_to_string(dir.join("api/load.json")).unwrap()).unwrap();

        // The requests.
        let ours = vec![recorded(&get_offizielle(&keys())), recorded(&get_stammdaten(&keys())), recorded(&get_veranstaltung(&keys(), None))];
        if Value::Array(ours.clone()) != fixture["requests"] {
            problems.push(format!("{show} requests:\n  ours   {ours:?}\n  Rosson {}", fixture["requests"]));
        }

        // Swiss Equestrian's answers.
        let stammdaten = json!({ "data": { "stammdaten": MASTER_DATA.iter().map(|s| (s.to_string(), without(&snapshot[*s], &["updated_at", "searchterms"]))).collect::<Map<_, _>>() } });
        let offizielle = json!({ "data": { "offizielle": without(&snapshot["offizielle"], &["updated_at", "searchterms"]) } });

        // Ours.
        let master = read_master_data(&stammdaten, LOADED_AT).unwrap();
        let national = read_officials(&offizielle).unwrap();
        let mut n = 0;
        let changes = read_event(&fixture["response"], &master, &national, LOADED_AT, &mut || {
            n += 1;
            format!("new-{n}")
        })
        .unwrap();
        let ours = state_of(changes);

        // Rosson's: what it stored, with the master data it stored, as an export.
        let mut export = Map::new();
        for store in MASTER_DATA.iter().chain(&["offizielle"]) {
            export.insert(store.to_string(), snapshot[*store].clone());
        }
        for (store, rows) in fixture["stored"].as_object().unwrap() {
            export.insert(store.clone(), rows.clone());
        }
        let theirs = state_of(rosson_import::ops::to_operations(&Value::Object(export)).unwrap().into_iter().map(|o| o.change).collect());

        // Everything delivered.
        let event = |s: &State| s.event.iter().map(|e| (e.id.clone(), e.clone())).collect::<BTreeMap<_, _>>();
        compare(&show, "event", &event(&ours), &event(&theirs), &mut problems);
        compare(&show, "published class", &ours.published_classes, &theirs.published_classes, &mut problems);
        compare(&show, "person", &ours.persons, &theirs.persons, &mut problems);
        compare(&show, "horse", &ours.horses, &theirs.horses, &mut problems);
        compare(&show, "entry", &ours.entries, &theirs.entries, &mut problems);
        compare(&show, "team", &ours.teams, &theirs.teams, &mut problems);
        compare(&show, "contact", &ours.contacts, &theirs.contacts, &mut problems);
        compare(&show, "entry note", &ours.entry_notes, &theirs.entry_notes, &mut problems);
        compare(&show, "scoring table", &ours.scoring_tables, &theirs.scoring_tables, &mut problems);
        compare(&show, "programme", &ours.programmes, &theirs.programmes, &mut problems);
        compare(&show, "reason", &ours.reasons, &theirs.reasons, &mut problems);
        // Categories: Rosson stamps the record with the time it stored it.
        let categories = |s: &State| {
            s.categories
                .iter()
                .map(|(id, c)| {
                    let mut c = c.clone();
                    c.record.as_object_mut().map(|r| r.remove("updated_at"));
                    (*id, c)
                })
                .collect::<BTreeMap<_, _>>()
        };
        compare(&show, "category", &categories(&ours), &categories(&theirs), &mut problems);
        // Officials: by Swiss Equestrian's id; the assignment id is drawn at random.
        let assignments = |s: &State| s.assignments.values().map(|a| ((a.se_id, a.person.clone()), Assignment { id: AssignmentId(String::new()), ..a.clone() })).collect::<BTreeMap<_, _>>();
        compare(&show, "official", &assignments(&ours), &assignments(&theirs), &mut problems);

        entries += ours.entries.len();
        persons += ours.persons.len();
        horses += ours.horses.len();
        officials += ours.assignments.len();
    }
    let shown: Vec<&String> = problems.iter().take(30).collect();
    assert!(problems.is_empty(), "{} differences with Rosson's loading:\n{}", problems.len(), shown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    println!("loading: {entries} entries, {persons} people, {horses} horses, {officials} officials, as Rosson");
}

/// Rosson's stores overlaid with what the update changed (update.json holds changed records only).
fn after_update(load: &Value, update: &Value) -> Map<String, Value> {
    let mut stores = Map::new();
    for (store, rows) in load["stored"].as_object().unwrap() {
        let mut by_id: BTreeMap<String, Value> = rows.as_array().unwrap().iter().map(|r| (r["id"].to_string(), r.clone())).collect();
        for r in update["stored"][store].as_array().into_iter().flatten() {
            by_id.insert(r["id"].to_string(), r.clone());
        }
        stores.insert(store.clone(), Value::Array(by_id.into_values().collect()));
    }
    stores
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// People, contacts and horses as Rosson keeps them (its `personen` and `pferde` stores; an
/// export has none, so `rosson-import` takes them from the entries, which an update leaves stale).
fn people_and_horses(stores: &Map<String, Value>) -> (BTreeMap<PersonId, Person>, BTreeMap<PersonId, Contact>, BTreeMap<HorseId, Horse>) {
    let mut persons = BTreeMap::new();
    let mut contacts = BTreeMap::new();
    for p in stores["personen"].as_array().unwrap() {
        let id = PersonId(p["id"].as_u64().unwrap());
        persons.insert(id.clone(), Person {
            id: id.clone(),
            first_name: text(&p["vorname"]),
            last_name: text(&p["nachname"]),
            town: text(&p["ort"]),
            nationality: text(&p["land"]),
            club: text(&p["vereinsname"]),
            licences: text(&p["lizenzen"]),
            youth: text(&p["nachwuchsreiter"]),
        });
        contacts.insert(id.clone(), Contact {
            person: id,
            salutation: text(&p["anrede"]),
            street: text(&p["strasse"]),
            postcode: text(&p["postleitzahl"]),
            country: text(&p["land"]),
            email: text(&p["email"]),
            phone_private: text(&p["telefon_privat"]),
            phone_mobile: text(&p["telefon_mobil"]),
            language: text(&p["sprache"]),
            birth_date: p["geboren_am"].as_str().map(|s| s[..10].to_string()),
        });
    }
    let mut horses = BTreeMap::new();
    for h in stores["pferde"].as_array().unwrap().iter().filter(|h| h["id"] != 5) {
        let id = HorseId(h["id"].as_u64().unwrap());
        let record = Value::Object(h.as_object().unwrap().iter().filter(|(k, _)| !matches!(k.as_str(), "searchterms" | "updated_at")).map(|(k, v)| (k.clone(), v.clone())).collect());
        horses.insert(id.clone(), Horse { id, name: text(&h["name"]), signalement: text(&h["signalement_label"]), owner: text(&h["besitzer"]), breeder: text(&h["zuechter"]), record });
    }
    (persons, contacts, horses)
}

#[test]
fn updating_an_event_matches_rosson() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let mut dirs: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("api/update.json").is_file()).collect();
    dirs.sort();
    assert!(dirs.len() >= 6, "update fixtures missing: run `npm run load` in golden/");

    let mut problems = Vec::new();
    let mut decided = 0;
    let mut changes_checked = 0;
    for dir in dirs {
        let show = dir.file_name().unwrap().to_string_lossy().to_string();
        let snapshot: Value = serde_json::from_str(&fs::read_to_string(dir.join("snapshot.json")).unwrap()).unwrap();
        let load: Value = serde_json::from_str(&fs::read_to_string(dir.join("api/load.json")).unwrap()).unwrap();
        let update: Value = serde_json::from_str(&fs::read_to_string(dir.join("api/update.json")).unwrap()).unwrap();
        let stammdaten = json!({ "data": { "stammdaten": MASTER_DATA.iter().map(|s| (s.to_string(), without(&snapshot[*s], &["updated_at", "searchterms"]))).collect::<Map<_, _>>() } });
        let offizielle = json!({ "data": { "offizielle": without(&snapshot["offizielle"], &["updated_at", "searchterms"]) } });
        let master = read_master_data(&stammdaten, LOADED_AT).unwrap();
        let national = read_officials(&offizielle).unwrap();
        let mut n = 0;
        let mut ids = || {
            n += 1;
            format!("new-{n}")
        };

        // Ours: load, then update.
        let mut changes = read_event(&load["response"], &master, &national, LOADED_AT, &mut ids).unwrap();
        let loaded = state_of(changes.clone());
        let since = loaded.event.as_ref().and_then(|e| e.se_last_change.clone());
        let requests = vec![recorded(&get_offizielle(&keys())), recorded(&get_stammdaten(&keys())), recorded(&get_veranstaltung(&keys(), since.as_deref()))];
        if Value::Array(requests.clone()) != update["requests"] {
            problems.push(format!("{show} requests:\n  ours   {requests:?}\n  Rosson {}", update["requests"]));
        }
        changes.extend(se_api::load::read_update(&update["response"], &loaded, &master, &national, LOADED_AT, &mut ids).unwrap());
        let mut ours = state_of(changes);
        assert!(ours.notices.is_empty(), "{show}: {:?}", ours.notices);

        // Rosson's.
        let stores = after_update(&load, &update);
        let mut export = Map::new();
        for store in MASTER_DATA.iter().chain(&["offizielle"]) {
            export.insert(store.to_string(), snapshot[*store].clone());
        }
        export.extend(stores.clone());
        let theirs = state_of(rosson_import::ops::to_operations(&Value::Object(export)).unwrap().into_iter().map(|o| o.change).collect());

        // Decided: what Rosson does not take from an update (docs/deviations.md E39), checked
        // here and then set back to Rosson's value for the comparison.
        for change in update["changes"].as_array().unwrap() {
            let what = change["what"].as_str().unwrap();
            if let Some(id) = change["entry"].as_u64().map(EntryId) {
                let (o, t) = (ours.entries.get_mut(&id).unwrap(), &theirs.entries[&id]);
                match what {
                    "hors concours and level" => {
                        assert!(o.hors_concours && o.stage == Some(2) && !t.hors_concours && t.stage != Some(2), "{show} {id}: {o:?}");
                        (o.hors_concours, o.stage) = (t.hors_concours, t.stage);
                        decided += 1;
                    }
                    "confirmed again after a withdrawal" => {
                        assert!(o.status == EntryStatus::Entered && t.status == EntryStatus::NotStarting, "{show} {id}");
                        o.status = t.status;
                        decided += 1;
                    }
                    _ => {}
                }
                changes_checked += 1;
            }
            if let Some(id) = change["class"].as_u64().map(PublishedClassId) {
                let (o, t) = (ours.published_classes.get_mut(&id).unwrap(), &theirs.published_classes[&id]);
                match what {
                    "category text, scoring text, levels" => {
                        assert!(o.scoring_text.as_deref() == Some("Neue Wertung") && o.levels == 3 && t.levels != 3, "{show} {id}");
                        (o.scoring_text, o.levels) = (t.scoring_text.clone(), t.levels);
                        decided += 1;
                    }
                    "new class" => {
                        // Rosson stores it without the event, and never shows it.
                        assert!(!update["classes_shown"].as_array().unwrap().contains(&json!(id.0)), "{show}: Rosson shows the new class");
                        decided += 1;
                    }
                    _ => {}
                }
                changes_checked += 1;
            }
        }

        let event = |s: &State| s.event.iter().map(|e| (e.id.clone(), e.clone())).collect::<BTreeMap<_, _>>();
        compare(&show, "event", &event(&ours), &event(&theirs), &mut problems);
        compare(&show, "published class", &ours.published_classes, &theirs.published_classes, &mut problems);
        compare(&show, "entry", &ours.entries, &theirs.entries, &mut problems);
        compare(&show, "team", &ours.teams, &theirs.teams, &mut problems);
        compare(&show, "entry note", &ours.entry_notes, &theirs.entry_notes, &mut problems);
        let (persons, contacts, horses) = people_and_horses(&stores);
        compare(&show, "person", &ours.persons, &persons, &mut problems);
        compare(&show, "contact", &ours.contacts, &contacts, &mut problems);
        compare(&show, "horse", &ours.horses, &horses, &mut problems);
        // Officials keep their id through an update.
        for (id, a) in &ours.assignments {
            if let Some(before) = loaded.assignments.values().find(|b| b.se_id == a.se_id) {
                assert_eq!(&before.id, id, "{show}: official {:?} got a new id", a.se_id);
            }
        }
        let assignments = |s: &State| s.assignments.values().map(|a| ((a.se_id, a.person.clone()), Assignment { id: AssignmentId(String::new()), ..a.clone() })).collect::<BTreeMap<_, _>>();
        compare(&show, "official", &assignments(&ours), &assignments(&theirs), &mut problems);
    }
    let shown: Vec<&String> = problems.iter().take(30).collect();
    assert!(problems.is_empty(), "{} differences with Rosson's update:\n{}", problems.len(), shown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    println!("updates: {changes_checked} changes as Rosson, {decided} by decided rules (E39)");
}
