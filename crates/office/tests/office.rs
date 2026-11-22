//! The office as the app uses it: commands in, JSON out.

use office::Office;
use serde_json::{json, Value};

mod common;
use common::rosson_file;

#[test]
fn import_look_export_import() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8352");

    let events = office.call("rosson_events", &json!({ "path": file })).unwrap();
    let event = events[0]["id"].as_str().unwrap().to_string();
    assert_eq!(events[0]["number"], "2026.8352");

    // As it ended.
    let ended = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    let show = office.call("show", &json!({ "show": ended })).unwrap();
    let days = show["days"].as_array().unwrap();
    assert_eq!(days.len(), 3);
    let class = &days[0]["classes"][0];
    assert_eq!(class["status"], "final");
    let class_id = class["id"].as_str().unwrap();
    let list = office.call("start_list", &json!({ "show": ended, "class": class_id })).unwrap();
    assert!(list["lines"].as_array().unwrap().iter().any(|l| l["kind"] == "starter" && !l["rider"].as_str().unwrap().is_empty()));
    let ranking = office.call("ranking", &json!({ "show": ended, "class": class_id })).unwrap();
    assert_eq!(ranking["lines"][0]["rank"], 1);
    let participants = office.call("participants", &json!({ "show": ended })).unwrap();
    assert_eq!(participants.as_array().unwrap().len(), 454);
    assert!(participants.as_array().unwrap().iter().any(|p| !p["starts"].as_array().unwrap().is_empty()));

    // As delivered, to rehearse: nothing of the show itself.
    let rehearsal = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    assert_eq!(rehearsal, "2026.8352 rehearsal");
    let show = office.call("show", &json!({ "show": rehearsal })).unwrap();
    assert!(show["days"].as_array().unwrap().is_empty());
    assert_eq!(show["not_set_up"].as_array().unwrap().len(), 17);

    // Export, delete, import back.
    let exported = dir.path().join("show.equievents");
    office.call("export_show", &json!({ "show": ended, "path": exported, "private": true })).unwrap();
    office.call("delete_show", &json!({ "show": ended })).unwrap();
    let library = office.call("library", &json!({})).unwrap();
    assert_eq!(library.as_array().unwrap().len(), 1);
    let back = office.call("import_show", &json!({ "path": exported })).unwrap()["show"].as_str().unwrap().to_string();
    assert_eq!(office.call("show", &json!({ "show": back })).unwrap()["days"].as_array().unwrap().len(), 3);
    // Importing the same file again updates that show instead of adding one.
    assert_eq!(office.call("import_show", &json!({ "path": exported })).unwrap()["show"], back);
    assert_eq!(office.call("library", &json!({})).unwrap().as_array().unwrap().len(), 2);

    assert!(office.call("show", &json!({ "show": "../etc" })).is_err());
    assert!(office.call("nonsense", &json!({})).unwrap_err().contains("unknown command"));
}

fn starters(office: &Office, show: &str, class: &str) -> Vec<Value> {
    office.call("start_list", &json!({ "show": show, "class": class })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").cloned().collect()
}

#[test]
fn the_library_is_listed_without_rebuilding_shows() {
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("library");
    let file = rosson_file(dir.path(), "2026.8352");
    let show = {
        let office = Office::new(&library, "test").unwrap();
        let show = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
        let listed = office.call("library", &json!({})).unwrap();
        assert_eq!((listed[0]["classes"].as_u64(), listed[0]["rehearsal"].as_bool()), (Some(0), Some(true)));
        // A change: the summary follows.
        office.call("set_up_class", &json!({ "show": show, "published": 108196 })).unwrap();
        assert_eq!(office.call("library", &json!({})).unwrap()[0]["classes"], 1);
        show
    };
    // Another start: the kept summary, the show not opened.
    let office = Office::new(&library, "test").unwrap();
    let listed = office.call("library", &json!({})).unwrap();
    assert_eq!((listed[0]["id"].as_str(), listed[0]["classes"].as_u64(), listed[0]["entries"].as_u64(), listed[0]["rehearsal"].as_bool()), (Some(show.as_str()), Some(1), Some(452), Some(true)));
}

fn participants(office: &Office, show: &str) -> Vec<Value> {
    office.call("participants", &json!({ "show": show })).unwrap().as_array().unwrap().clone()
}

fn entry_row(office: &Office, show: &str, id: u64) -> Value {
    participants(office, show).into_iter().find(|e| e["id"] == id).unwrap()
}

#[test]
fn a_pair_changed_in_the_classes_picked() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8352");
    let show = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    let rows = participants(&office, &show);
    let same_pair = |a: &Value, b: &Value| a["rider_id"] == b["rider_id"] && a["horse_id"] == b["horse_id"];
    // A pair entered in three classes or more.
    let first = rows.iter().find(|e| rows.iter().filter(|o| same_pair(e, o)).count() >= 3).unwrap().clone();
    let pair: Vec<u64> = rows.iter().filter(|o| same_pair(&first, o)).map(|o| o["id"].as_u64().unwrap()).collect();
    let (horse, rider) = (first["horse_id"].as_u64().unwrap(), first["rider_id"].as_u64().unwrap());
    assert!(pair.iter().all(|id| !entry_row(&office, &show, *id)["locked"].as_bool().unwrap() && !entry_row(&office, &show, *id)["ridden"].as_bool().unwrap()));

    // Another horse in all its classes but the last: one change, undone at once.
    let other = rows.iter().find(|e| e["horse_id"] != first["horse_id"] && !e["guest_horse"].as_bool().unwrap()).unwrap()["horse_id"].as_u64().unwrap();
    let (changed, kept) = pair.split_at(pair.len() - 1);
    office.call("change_horse", &json!({ "show": show, "entries": changed, "horse": other })).unwrap();
    assert!(changed.iter().all(|id| entry_row(&office, &show, *id)["horse_id"] == other));
    assert_eq!(entry_row(&office, &show, kept[0])["horse_id"], horse);
    assert_eq!(office.call("undo", &json!({ "show": show })).unwrap()["undone"], "change_horse");
    assert!(pair.iter().all(|id| entry_row(&office, &show, *id)["horse_id"] == horse));

    // Another rider in one class only (RW there); the others keep theirs.
    let someone = rows.iter().find(|e| e["rider_id"] != first["rider_id"]).unwrap()["rider_id"].as_u64().unwrap();
    office.call("change_rider", &json!({ "show": show, "entries": [pair[0]], "rider": { "person": someone } })).unwrap();
    let changed = entry_row(&office, &show, pair[0]);
    assert_eq!((changed["rider_id"].as_u64(), changed["rider_changed"].as_bool()), (Some(someone), Some(true)));
    assert!(pair[1..].iter().all(|id| entry_row(&office, &show, *id)["rider_id"] == rider));
    // In all of them: those already changed stay as they are.
    office.call("change_rider", &json!({ "show": show, "entries": pair, "rider": { "person": someone } })).unwrap();
    assert!(pair.iter().all(|id| entry_row(&office, &show, *id)["rider_id"] == someone));
    assert!(office.call("change_rider", &json!({ "show": show, "entries": [], "rider": { "person": someone } })).is_err());
}

#[test]
fn a_section_moved_to_a_day_not_planned() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8352");
    let show = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    let class = office.call("set_up_class", &json!({ "show": show, "published": 108196 })).unwrap()["class"].as_str().unwrap().to_string();
    office.call("split", &json!({ "show": show, "class": class, "parts": 2 })).unwrap();
    let (a, b) = (format!("{class}a"), format!("{class}b"));
    let planned = office.call("class_details", &json!({ "show": show, "class": a })).unwrap()["planned_days"].as_array().unwrap().clone();
    assert!(!planned.contains(&json!("2026-09-28")));

    // The second section on the Monday after the show; the first stays.
    office.call("edit_class", &json!({ "show": show, "class": b, "date": "2026-09-28", "time": "08h00" })).unwrap();
    let days = office.call("show", &json!({ "show": show })).unwrap()["days"].as_array().unwrap().iter().map(|d| d["date"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(days.last().map(String::as_str), Some("2026-09-28"));
    let details = office.call("class_details", &json!({ "show": show, "class": a })).unwrap();
    assert!(details["days"].as_array().unwrap().contains(&json!("2026-09-28")) && details["date"] != "2026-09-28");
    assert!(office.call("edit_class", &json!({ "show": show, "class": b, "date": "2026-02-30" })).unwrap_err().contains("not a date"));

    // The number is the app's: details do not change it.
    office.call("set_class_details", &json!({ "show": show, "class": a, "number": "99", "sponsor": "Écurie" })).unwrap();
    assert_eq!(office.call("class_details", &json!({ "show": show, "class": a })).unwrap()["number"], details["number"]);
}

#[test]
fn prepare_a_class_in_a_rehearsal_and_undo() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8352");
    let show = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();

    // Class 1 (published 108196): set up with every entry, drawn.
    let class = office.call("set_up_class", &json!({ "show": show, "published": 108196 })).unwrap()["class"].as_str().unwrap().to_string();
    let list = starters(&office, &show, &class);
    let entries = office.call("participants", &json!({ "show": show })).unwrap().as_array().unwrap().iter().filter(|e| e["published"] == 108196).count();
    assert_eq!(list.len(), entries);
    assert!(office.call("set_up_class", &json!({ "show": show, "published": 108196 })).unwrap_err().contains("already set up"));

    // Start numbers; a starter moved to the front keeps its number.
    office.call("number", &json!({ "show": show, "class": class, "first": 1 })).unwrap();
    let list = starters(&office, &show, &class);
    assert_eq!(list.iter().map(|l| l["start_number"].as_u64().unwrap()).collect::<Vec<_>>(), (1..=list.len() as u64).collect::<Vec<_>>());
    let last = list.last().unwrap()["entry"].as_u64().unwrap();
    office.call("move_start", &json!({ "show": show, "class": class, "entry": last, "to": 1 })).unwrap();
    let list = starters(&office, &show, &class);
    assert_eq!((list[0]["entry"].as_u64(), list[0]["start_number"].as_u64()), (Some(last), Some(list.len() as u64)));

    // Does not start; another horse of the show (a guest horse: only with a guest licence).
    office.call("edit_entry", &json!({ "show": show, "entry": last, "not_starting": true })).unwrap();
    assert!(office.call("change_horse", &json!({ "show": show, "entry": last, "guest": "GASTPFERD" })).unwrap_err().contains("guest licence"));
    // (Not the horse it has: the draw may have put that pair last.)
    let own = list[0]["horse"].as_str().unwrap().to_string();
    let found = office.call("find_horses", &json!({ "show": show, "query": "to" })).unwrap()["found"].clone();
    let other = found.as_array().unwrap().iter().find(|h| h["name"].as_str() != Some(own.as_str()) && h["paid"] != false).unwrap().clone();
    office.call("change_horse", &json!({ "show": show, "entry": last, "horse": other["horse"] })).unwrap();
    let list = starters(&office, &show, &class);
    assert_eq!((list[0]["not_starting"].as_bool(), list[0]["horse"].as_str()), (Some(true), other["name"].as_str()));

    // Two sections; a rider moved from the first to the second.
    office.call("split", &json!({ "show": show, "class": class, "parts": 2 })).unwrap();
    let (a, b) = (format!("{class}a"), format!("{class}b"));
    let (na, nb) = (starters(&office, &show, &a).len(), starters(&office, &show, &b).len());
    assert_eq!(na + nb, entries);
    office.call("move_entry", &json!({ "show": show, "entry": last, "from": a, "to": b })).unwrap();
    assert_eq!(starters(&office, &show, &b).last().unwrap()["entry"].as_u64(), Some(last));

    // Undo, step by step: the move, the split, the horse.
    assert_eq!(office.call("undo", &json!({ "show": show })).unwrap()["undone"], "move_entry");
    assert_eq!(starters(&office, &show, &a).len(), na);
    assert_eq!(office.call("undo", &json!({ "show": show })).unwrap()["undone"], "split");
    assert_eq!(starters(&office, &show, &class).len(), entries);
    assert!(office.call("start_list", &json!({ "show": show, "class": a })).is_err());
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_ne!(starters(&office, &show, &class)[0]["horse"].as_str(), other["name"].as_str());

    // Redo, step by step, the other way: the horse, the split, the move; then undo again.
    let redone = office.call("redo", &json!({ "show": show })).unwrap();
    assert_eq!((redone["redone"].as_str(), redone["redo"].as_str()), (Some("change_horse"), Some("split")));
    assert_eq!(starters(&office, &show, &class)[0]["horse"].as_str(), other["name"].as_str());
    assert_eq!(office.call("redo", &json!({ "show": show })).unwrap()["redone"], "split");
    assert_eq!(starters(&office, &show, &a).len(), na);
    let redone = office.call("redo", &json!({ "show": show })).unwrap();
    assert_eq!((redone["redone"].as_str(), redone["undo"].as_str(), redone["redo"].as_str()), (Some("move_entry"), Some("move_entry"), None));
    assert_eq!(starters(&office, &show, &b).last().unwrap()["entry"].as_u64(), Some(last));
    assert!(office.call("redo", &json!({ "show": show })).is_err());
    assert_eq!(office.call("undo", &json!({ "show": show })).unwrap()["undone"], "move_entry");
    assert_eq!(starters(&office, &show, &a).len(), na);
    // A new change: what was undone cannot come back.
    office.call("edit_entry", &json!({ "show": show, "entry": last, "not_starting": false })).unwrap();
    assert!(office.call("redo", &json!({ "show": show })).unwrap_err().contains("nothing to redo"));
    office.call("undo", &json!({ "show": show })).unwrap();
    office.call("undo", &json!({ "show": show })).unwrap();
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_ne!(starters(&office, &show, &class)[0]["horse"].as_str(), other["name"].as_str());

    // A final class is not changed.
    let ended = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    assert!(office.call("draw", &json!({ "show": ended, "class": "108196" })).unwrap_err().contains("final"));
}

#[test]
fn officials_as_a_table() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8352");

    // As it ended: the jury of every class, day by day.
    let ended = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    let table = office.call("officials", &json!({ "show": ended })).unwrap();
    let friday = &table["days"][0];
    assert!(friday["rows"].as_array().unwrap().iter().any(|r| r["cells"]["108196"]["role"] == "P"));
    assert!(table["roles"].as_array().unwrap().iter().any(|r| r["code"] == "DRI"));
    let sunday = table["days"].as_array().unwrap().last().unwrap();
    assert!(sunday["rows"].as_array().unwrap().iter().any(|r| r["cells"].as_object().unwrap().values().any(|c| c["role"] == "DRI" && c["letter"] == "C")));
    // A final class is not changed.
    let someone = friday["rows"][0]["person"].as_u64().unwrap();
    assert!(office.call("set_role", &json!({ "show": ended, "class": "108196", "person": someone, "role": "A" })).unwrap_err().contains("final"));

    // A rehearsal: a dressage class set up, its judges given (the national list of officials as Rosson had it).
    let show = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    office.call("set_up_class", &json!({ "show": show, "published": 108211 })).unwrap();
    let found = office.call("find_officials", &json!({ "show": show, "query": "" })).unwrap();
    let judges: Vec<u64> = found.as_array().unwrap().iter().filter(|c| !c["titles"].as_array().unwrap().is_empty()).take(3).map(|c| c["person"].as_u64().unwrap()).collect();
    assert_eq!(judges.len(), 3);
    // Their titles in three languages, as Swiss Equestrian says them; found by a title too.
    let dressage = office.call("find_officials", &json!({ "show": show, "query": "juge de dressage" })).unwrap();
    let titles = dressage[0]["titles"].as_array().unwrap();
    let title = titles.iter().find(|t| t["fr"].as_str().unwrap().starts_with("Juge de dressage")).unwrap();
    assert!(title["de"].as_str().unwrap().starts_with("Dressurrichter") && title["it"].as_str().unwrap().starts_with("Giudice di"));
    for (judge, letter) in judges.iter().zip(["H", "C", "M"]) {
        office.call("set_role", &json!({ "show": show, "class": "108211", "person": judge, "role": "DRI", "letter": letter })).unwrap();
    }
    let table = office.call("officials", &json!({ "show": show })).unwrap();
    let day = table["days"].as_array().unwrap().iter().find(|d| d["classes"].as_array().unwrap().iter().any(|c| c["id"] == "108211")).unwrap().clone();
    assert_eq!(day["rows"].as_array().unwrap().len(), 3);
    // Another role for the same judge replaces the first; taking it away removes it; undo brings it back.
    office.call("set_role", &json!({ "show": show, "class": "108211", "person": judges[0], "role": "DRI", "letter": "E" })).unwrap();
    office.call("set_role", &json!({ "show": show, "class": "108211", "person": judges[0], "role": null })).unwrap();
    let rows = |office: &Office| office.call("officials", &json!({ "show": show })).unwrap()["days"].as_array().unwrap().iter().flat_map(|d| d["rows"].as_array().unwrap().clone()).count();
    assert_eq!(rows(&office), 2);
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(rows(&office), 3);
    let table = office.call("officials", &json!({ "show": show })).unwrap();
    let letters: Vec<String> = table["days"].as_array().unwrap().iter().flat_map(|d| d["rows"].as_array().unwrap().clone()).map(|r| r["cells"]["108211"]["letter"].as_str().unwrap().to_string()).collect();
    assert!(letters.contains(&"E".to_string()));
}

/// One official in several classes at once: one change, undone at once.
#[test]
fn one_official_in_several_classes() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8352");
    let show = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    for published in [108196, 108197, 108211] {
        office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap();
    }
    let person = office.call("find_officials", &json!({ "show": show, "query": "" })).unwrap()[0]["person"].as_u64().unwrap();
    let cells = |office: &Office| -> Vec<(String, String)> {
        let table = office.call("officials", &json!({ "show": show })).unwrap();
        let mut cells: Vec<(String, String)> = table["days"].as_array().unwrap().iter().flat_map(|d| d["rows"].as_array().unwrap().clone()).filter(|r| r["person"] == person).flat_map(|r| r["cells"].as_object().unwrap().iter().map(|(c, v)| (c.clone(), format!("{}{}", v["role"].as_str().unwrap(), v["letter"].as_str().unwrap_or("")))).collect::<Vec<_>>()).collect();
        cells.sort();
        cells
    };
    // President of both jumping classes, judge at C in the dressage class.
    let roles = json!([{ "class": "108196", "role": "P" }, { "class": "108197", "role": "P" }, { "class": "108211", "role": "DRI", "letter": "C" }]);
    assert_eq!(office.call("set_roles", &json!({ "show": show, "person": person, "roles": roles })).unwrap()["undo"], "set_roles");
    assert_eq!(cells(&office), [("108196".into(), "P".into()), ("108197".into(), "P".into()), ("108211".into(), "DRIC".into())]);
    // Taken away from two; then undone: all three again.
    office.call("set_roles", &json!({ "show": show, "person": person, "roles": [{ "class": "108196", "role": null }, { "class": "108197", "role": null }] })).unwrap();
    assert_eq!(cells(&office).len(), 1);
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(cells(&office).len(), 3);
    office.call("undo", &json!({ "show": show })).unwrap();
    assert!(cells(&office).is_empty());
    // A role unknown in one class: nothing changes.
    assert!(office.call("set_roles", &json!({ "show": show, "person": person, "roles": [{ "class": "108196", "role": "P" }, { "class": "108197", "role": "XX" }] })).is_err());
    assert!(cells(&office).is_empty());
}

/// Changes made in quick succession (the same millisecond) are all kept.
#[test]
fn quick_changes_are_all_kept() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8053");
    let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
    let show = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    let entries: Vec<u64> = office.call("participants", &json!({ "show": show })).unwrap().as_array().unwrap().iter().take(20).map(|e| e["id"].as_u64().unwrap()).collect();
    for e in &entries {
        office.call("edit_entry", &json!({ "show": show, "entry": e, "hors_concours": true })).unwrap();
    }
    let hc = office.call("participants", &json!({ "show": show })).unwrap().as_array().unwrap().iter().filter(|e| e["hors_concours"] == true).count();
    assert_eq!(hc, entries.len());
}

#[test]
fn dressage_sheets_never_lose_marks() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8352");
    let show = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    office.call("set_up_class", &json!({ "show": show, "published": 108211 })).unwrap();
    // A special class (SP/CD): the office chooses the programme, here GA06/60.
    let programmes = office.call("programmes", &json!({ "show": show })).unwrap();
    let ga06 = programmes.as_array().unwrap().iter().find(|p| p["name"] == "GA06/60").unwrap()["id"].clone();
    // For the class: the tests of its discipline only (dressage, not driving's).
    let theirs = office.call("programmes", &json!({ "show": show, "class": "108211" })).unwrap().as_array().unwrap().len();
    assert!(theirs < programmes.as_array().unwrap().len() && theirs > 50, "{theirs}");
    assert!(office.call("programmes", &json!({ "show": show, "class": "108211" })).unwrap().as_array().unwrap().iter().any(|p| p["id"] == ga06));
    office.call("edit_class", &json!({ "show": show, "class": "108211", "programme": ga06 })).unwrap();
    let judges: Vec<u64> = office.call("find_officials", &json!({ "show": show, "query": "" })).unwrap().as_array().unwrap().iter().filter(|c| !c["titles"].as_array().unwrap().is_empty()).take(3).map(|c| c["person"].as_u64().unwrap()).collect();
    for (judge, letter) in judges.iter().zip(["H", "C", "M"]) {
        office.call("set_role", &json!({ "show": show, "class": "108211", "person": judge, "role": "DRI", "letter": letter })).unwrap();
    }
    let view = office.call("sheets", &json!({ "show": show, "class": "108211" })).unwrap();
    let letters: Vec<&str> = view["judges"].as_array().unwrap().iter().map(|j| j["letter"].as_str().unwrap()).collect();
    assert_eq!(letters, ["H", "C", "M"]);
    let programme = view["programmes"].as_object().unwrap().values().next().unwrap().clone();
    let movements: Vec<String> = programme["sections"].as_array().unwrap().iter().flat_map(|s| s["movements"].as_array().unwrap().clone()).map(|m| m["number"].as_str().unwrap().to_string()).collect();
    assert!(movements.len() > 10);
    let judge = |i: usize| view["judges"][i]["assignment"].as_str().unwrap().to_string();
    let rider = |i: usize| view["riders"][i]["entry"].as_u64().unwrap();
    let sheet = |r: u64, j: &str| -> Value {
        let v = office.call("sheets", &json!({ "show": show, "class": "108211" })).unwrap();
        v["riders"].as_array().unwrap().iter().find(|x| x["entry"] == r).unwrap()["sheets"][j].clone()
    };

    // A whole sheet, mark by mark (judge C, first rider): its total and percentage follow.
    for m in &movements {
        office.call("edit_sheet", &json!({ "show": show, "class": "108211", "entry": rider(0), "judge": judge(1), "marks": { m: "6.5" } })).unwrap();
    }
    let s = sheet(rider(0), &judge(1));
    assert_eq!(s["complete"], true);
    assert!(s["total"].as_str().unwrap().parse::<f64>().unwrap() > 0.0, "{s}");
    assert!(office.call("edit_sheet", &json!({ "show": show, "class": "108211", "entry": rider(0), "judge": judge(1), "marks": { movements[0].clone(): "6.3" } })).unwrap_err().contains("halves"));

    // Judge C has marks: not removed, nor given another role (in the class or for the whole day);
    // marked so in the officials' table. Another position is fine.
    let c = judges[1];
    assert!(office.call("set_role", &json!({ "show": show, "class": "108211", "person": c })).unwrap_err().contains("marks"));
    assert!(office.call("set_role", &json!({ "show": show, "class": "108211", "person": c, "role": "A" })).unwrap_err().contains("marks"));
    assert!(office.call("set_roles", &json!({ "show": show, "person": c, "roles": [{ "class": "108211" }] })).unwrap_err().contains("marks"));
    let marked = |office: &Office| {
        let table = office.call("officials", &json!({ "show": show })).unwrap();
        table["days"].as_array().unwrap().iter().flat_map(|d| d["rows"].as_array().unwrap().clone()).find(|r| r["person"] == c).unwrap()["cells"]["108211"].clone()
    };
    assert_eq!((marked(&office)["marked"].as_bool(), marked(&office)["letter"].as_str()), (Some(true), Some("C")));
    office.call("set_role", &json!({ "show": show, "class": "108211", "person": c, "role": "DRI", "letter": "E" })).unwrap();
    office.call("set_role", &json!({ "show": show, "class": "108211", "person": c, "role": "DRI", "letter": "C" })).unwrap();
    assert_eq!(marked(&office)["letter"], "C");

    // They were the second rider's: sent there; then judges C and H swapped; then put aside and placed back.
    office.call("move_sheet", &json!({ "show": show, "class": "108211", "from": { "entry": rider(0), "judge": judge(1) }, "to": { "entry": rider(1), "judge": judge(1) } })).unwrap();
    assert!(sheet(rider(0), &judge(1)).is_null());
    office.call("move_sheet", &json!({ "show": show, "class": "108211", "from": { "entry": rider(1), "judge": judge(1) }, "to": { "entry": rider(1), "judge": judge(0) } })).unwrap();
    assert_eq!(sheet(rider(1), &judge(0))["complete"], true);
    office.call("move_sheet", &json!({ "show": show, "class": "108211", "from": { "entry": rider(1), "judge": judge(0) } })).unwrap();
    let aside = office.call("sheets", &json!({ "show": show, "class": "108211" })).unwrap()["aside"].clone();
    assert_eq!(aside[0]["marks"].as_u64(), Some(movements.len() as u64));
    office.call("move_sheet", &json!({ "show": show, "class": "108211", "from": { "aside": aside[0]["slot"] }, "to": { "entry": rider(2), "judge": judge(2) } })).unwrap();
    assert_eq!(sheet(rider(2), &judge(2))["complete"], true);

    // The programme cannot change once there are marks; the class's settings say so.
    assert!(office.call("edit_class", &json!({ "show": show, "class": "108211", "programme": null })).unwrap_err().contains("marks"));
    let details = office.call("class_details", &json!({ "show": show, "class": "108211" })).unwrap();
    assert_eq!((details["uses_programme"].as_bool(), details["programme_locked"].as_bool(), details["programme"].is_u64()), (Some(true), Some(true), true));

    // Undo the placing: back aside (cut), its first marks shown to know the paper sheet.
    office.call("undo", &json!({ "show": show })).unwrap();
    assert!(sheet(rider(2), &judge(2)).is_null());
    let aside = || office.call("sheets", &json!({ "show": show, "class": "108211" })).unwrap()["aside"].clone();
    assert_eq!(aside().as_array().unwrap().len(), 1);
    assert_eq!(aside()[0]["first"], json!(["6.5", "6.5", "6.5", "6.5", "6.5", "6.5"]));

    // A cut sheet deleted, and back with undo; named so for undo.
    let slot = aside()[0]["slot"].clone();
    assert_eq!(office.call("delete_sheet", &json!({ "show": show, "class": "108211", "aside": slot })).unwrap()["undo"], "delete_sheet");
    assert_eq!(aside(), json!([]));
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(aside()[0]["marks"].as_u64(), Some(movements.len() as u64));

    // A rider's sheet deleted (pasted first: named so), and back with undo.
    let pasted = office.call("move_sheet", &json!({ "show": show, "class": "108211", "from": { "aside": aside()[0]["slot"] }, "to": { "entry": rider(2), "judge": judge(2) } })).unwrap();
    assert_eq!(pasted["undo"], "paste_sheet");
    office.call("delete_sheet", &json!({ "show": show, "class": "108211", "entry": rider(2), "judge": judge(2) })).unwrap();
    assert!(sheet(rider(2), &judge(2)).is_null());
    assert!(office.call("delete_sheet", &json!({ "show": show, "class": "108211", "entry": rider(2), "judge": judge(2) })).unwrap_err().contains("no sheet"));
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(sheet(rider(2), &judge(2))["complete"], true);
    let cut = office.call("move_sheet", &json!({ "show": show, "class": "108211", "from": { "entry": rider(2), "judge": judge(2) } })).unwrap();
    assert_eq!(cut["undo"], "cut_sheet");
}

#[test]
fn officials_at_the_event() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8352");
    let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
    let show = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    let view = || office.call("event_officials", &json!({ "show": show, "language": "fr" })).unwrap();
    let before = view()["rows"].as_array().unwrap().len();
    let roles = view()["roles"].as_array().unwrap().clone();
    // Several roles share a code: by id, each with its own name.
    let td = roles.iter().find(|r| r[1].as_str().unwrap().contains("technique")).unwrap()[0].clone();
    let person = office.call("find_officials", &json!({ "show": show, "query": "a" })).unwrap()[0]["person"].clone();

    office.call("set_event_role", &json!({ "show": show, "person": person, "role": td, "days": 2, "responsible": true })).unwrap();
    let rows = view()["rows"].as_array().unwrap().clone();
    assert_eq!(rows.len(), before + 1);
    let td_id = td.clone();
    let td = rows.iter().find(|r| r["person"] == person && r["role_id"] == td_id).unwrap().clone();
    assert_eq!((td["days"].clone(), td["responsible"].clone(), td["readonly"].clone()), (json!(2), json!(true), json!(false)));
    // Changed, then removed; undo brings it back.
    office.call("set_event_role", &json!({ "show": show, "assignment": td["assignment"], "person": person, "role": td_id, "days": 3, "responsible": false })).unwrap();
    assert_eq!(view()["rows"].as_array().unwrap().iter().find(|r| r["assignment"] == td["assignment"]).unwrap()["days"], 3);
    office.call("remove_event_role", &json!({ "show": show, "assignment": td["assignment"] })).unwrap();
    assert_eq!(view()["rows"].as_array().unwrap().len(), before);
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(view()["rows"].as_array().unwrap().len(), before + 1);
    // A role of a class is not a role at the event.
    assert!(office.call("set_event_role", &json!({ "show": show, "person": person, "role": 99999 })).is_err());
    // Swiss Equestrian's own stay.
    if let Some(given) = view()["rows"].as_array().unwrap().iter().find(|r| r["readonly"] == true) {
        assert!(office.call("remove_event_role", &json!({ "show": show, "assignment": given["assignment"] })).is_err());
    }
}

/// A final over two dressage classes by percentages, as Rosson's *Finalprüfung* 12.1 of the
/// show (classes 12 and 15, *Prozentpunkte*): the mean of the two percentages, the highest
/// first; riders in one class only listed last, without a rank; 30 % of the others placed.
#[test]
fn a_dressage_final_by_percentages() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8352");
    let show = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    let id = office.call("set_combined", &json!({ "show": show, "name": "Finale dressage non-licenciés", "classes": ["108211", "108212"], "method": "percentages" })).unwrap()["id"].as_str().unwrap().to_string();
    // A tie: the percentage in a class picked for it (not picked, as Rosson: the first class).
    let tie = |id: &str| office.call("combined", &json!({ "show": show, "id": id })).unwrap()["ranking"].clone();
    assert_eq!((tie(&id)["tie"].as_str(), tie(&id)["tie_break"].as_str()), (Some("percentage"), Some("108211")));
    office.call("set_combined", &json!({ "show": show, "id": id, "name": "Finale dressage non-licenciés", "classes": ["108211", "108212"], "method": "percentages", "tie_break": "108212" })).unwrap();
    assert_eq!(tie(&id)["tie_break"], "108212");
    assert!(office.call("set_combined", &json!({ "show": show, "id": id, "name": "X", "classes": ["108211", "108212"], "method": "percentages", "tie_break": "108196" })).is_err());
    let view = office.call("combined", &json!({ "show": show, "id": id })).unwrap();
    let lines = view["lines"].as_array().unwrap();
    let dec = |v: &Value| v.as_str().map(|s| s.parse::<f64>().unwrap());
    let ranked: Vec<&Value> = lines.iter().filter(|l| !l["rank"].is_null()).collect();
    assert!(ranked.len() >= 5, "{view}");
    for l in &ranked {
        let (a, b) = (dec(&l["cells"][0]["percentage"]).unwrap(), dec(&l["cells"][1]["percentage"]).unwrap());
        assert!((dec(&l["total"]).unwrap() - (a + b) / 2.0).abs() < 0.001, "{l}");
    }
    assert!(ranked.windows(2).all(|w| dec(&w[0]["total"]) >= dec(&w[1]["total"])));
    assert_eq!(ranked[0]["rank"], 1);
    // In one class only: last, no rank.
    let tail: Vec<&Value> = lines.iter().skip(ranked.len()).collect();
    assert!(tail.iter().all(|l| l["rank"].is_null() && l["cells"].as_array().unwrap().iter().any(|c| c["missing"] == true)));
    // Placed: 30 % of those in both, rounded up.
    let complete = lines.iter().filter(|l| l["cells"].as_array().unwrap().iter().all(|c| c["missing"] == false)).count();
    assert_eq!(lines.iter().filter(|l| l["placed"] == true).count(), (complete * 3).div_ceil(10));
    // Printed.
    let print = office.call("print", &json!({ "show": show, "what": "combined", "id": id, "language": "fr" })).unwrap();
    assert!(print["pages"].as_u64().unwrap() >= 1);
    // Percentages over a jumping class: refused.
    assert!(office.call("set_combined", &json!({ "show": show, "name": "X", "classes": ["108211", "108196"], "method": "percentages" })).is_err());
    // A jumping and a dressage class: ranks.
    office.call("set_combined", &json!({ "show": show, "name": "Mixte", "classes": ["108196", "108211"], "method": "ranks", "tie_break": "108211" })).unwrap();
}

/// Rosson's developer mode, an administrator's only (the user, 2026-11-22): a scoring table's
/// definition changed in a show (when Swiss Equestrian's is wrong), read as the app reads tables
/// before it is kept; undone as any change. The dressage tests' are no longer changed in a show.
/// And the error log: what failed, without keys, also in the library's errors.log.
#[test]
fn reference_data_edited_and_errors_logged() {
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("library");
    let office = Office::new(&library, "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8352");
    let show = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    // Nobody signed in, or someone who is not an administrator of equievents.ch: refused.
    assert!(office.call("reference_data", &json!({ "show": show })).unwrap_err().contains("administrators"));
    signed_in(&library, false);
    assert!(office.call("reference_data", &json!({ "show": show })).unwrap_err().contains("administrators"));
    signed_in(&library, true);
    let data = office.call("reference_data", &json!({ "show": show })).unwrap();
    assert!(data.get("programmes").is_none(), "{data}");
    // The dressage tests: not shown, not changed.
    assert!(office.call("set_definition", &json!({ "show": show, "kind": "programme", "id": 1, "definition": "{}" })).unwrap_err().contains("no definitions of programme"));
    // A table no class uses: changed, then undone.
    let table = |code: &str| data["scoring_tables"].as_array().unwrap().iter().find(|t| t["code"] == code).unwrap()["id"].as_u64().unwrap();
    let definition = |office: &Office| office.call("definition", &json!({ "show": show, "kind": "scoring_table", "id": table("C") })).unwrap()["definition"].as_str().unwrap().to_string();
    let text = definition(&office);
    assert!(text.contains("\n  \"")); // indented
    let mut def: Value = serde_json::from_str(&text).unwrap();
    def["bemerkung"] = json!("corrigé");
    office.call("set_definition", &json!({ "show": show, "kind": "scoring_table", "id": table("C"), "definition": def.to_string() })).unwrap();
    assert!(definition(&office).contains("corrigé"));
    office.call("undo", &json!({ "show": show })).unwrap();
    assert!(!definition(&office).contains("corrigé"));
    // Not JSON: refused.
    assert!(office.call("set_definition", &json!({ "show": show, "kind": "scoring_table", "id": table("C"), "definition": "{ nope" })).unwrap_err().contains("JSON"));
    // A table a final class uses: refused (its results are official).
    let text = office.call("definition", &json!({ "show": show, "kind": "scoring_table", "id": table("AZ") })).unwrap()["definition"].as_str().unwrap().to_string();
    let answer = office.call("set_definition", &json!({ "show": show, "kind": "scoring_table", "id": table("AZ"), "definition": format!("{text} ") })).unwrap();
    assert_eq!(answer["unchanged"], true, "the same table: nothing to do");
    assert!(office.call("set_definition", &json!({ "show": show, "kind": "scoring_table", "id": table("AZ"), "definition": "[]" })).is_err());

    // The error log: the newest first, the key masked; in the library's file too.
    assert!(office.call("set_connection", &json!({ "show": "nope", "access_key": "SECRET" })).is_err());
    let log = office.call("error_log", &json!({})).unwrap();
    assert_eq!(log[0]["command"], "set_connection");
    assert_eq!(log[0]["args"]["access_key"], "•••");
    let file = std::fs::read_to_string(dir.path().join("library/errors.log")).unwrap();
    assert!(file.contains("set_connection") && !file.contains("SECRET"));
}

/// Someone signed in to equievents.ch on this computer (its settings and token, as a sign-in
/// leaves them), an administrator or not.
fn signed_in(library: &std::path::Path, admin: bool) {
    let server = "http://127.0.0.1:9";
    let path = library.join("settings.json");
    let mut settings: Value = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    settings["server_url"] = json!(server);
    settings["account"] = json!({ "server": server, "id": 7, "name": "Ada Admin", "email": "ada@example.ch", "language": "fr", "admin": admin });
    std::fs::write(&path, settings.to_string()).unwrap();
    let path = library.join("secrets.json");
    let mut secrets: Value = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
    secrets[format!("equievents token {server}")] = json!("token");
    std::fs::write(&path, secrets.to_string()).unwrap();
}

/// Young horses' programmes (JP…) are judged in tenths (Dressurreglement 8.2 (2)), the others in
/// halves; and jointly (8.1): the jury's one sheet, entered as the first judge's, counts for
/// every judge (docs/deviations.md S39).
#[test]
fn young_horses_are_marked_in_tenths_by_the_jury_together() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8352");
    let show = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    office.call("set_up_class", &json!({ "show": show, "published": 108211 })).unwrap();
    let programmes = office.call("programmes", &json!({ "show": show })).unwrap();
    let jp = programmes.as_array().unwrap().iter().find(|p| p["name"] == "JP4/40").unwrap()["id"].clone();
    office.call("edit_class", &json!({ "show": show, "class": "108211", "programme": jp })).unwrap();
    let people: Vec<u64> = office.call("find_officials", &json!({ "show": show, "query": "" })).unwrap().as_array().unwrap().iter().filter(|c| !c["titles"].as_array().unwrap().is_empty()).take(3).map(|c| c["person"].as_u64().unwrap()).collect();
    for (person, letter) in people.iter().zip(["H", "C", "M"]) {
        office.call("set_role", &json!({ "show": show, "class": "108211", "person": person, "role": "DRI", "letter": letter })).unwrap();
    }
    let view = office.call("sheets", &json!({ "show": show, "class": "108211" })).unwrap();
    let programme = view["programmes"].as_object().unwrap().values().next().unwrap().clone();
    assert_eq!(programme["kind"], "joint");
    let judge = view["judges"][0]["assignment"].as_str().unwrap().to_string();
    let rider = view["riders"][0]["entry"].as_u64().unwrap();
    let movements: Vec<String> = programme["sections"].as_array().unwrap().iter().flat_map(|s| s["movements"].as_array().unwrap().clone()).map(|m| m["number"].as_str().unwrap().to_string()).collect();
    let mark = |m: &str, v: &str| office.call("edit_sheet", &json!({ "show": show, "class": "108211", "entry": rider, "judge": judge, "marks": { m: v } }));
    assert!(mark(&movements[0], "7.35").unwrap_err().contains("tenths"));
    // The jury's sheet: 7.3 for each of the 5 movements, 36.5 of 50 points.
    for m in &movements {
        mark(m, "7.3").unwrap();
    }
    let view = office.call("sheets", &json!({ "show": show, "class": "108211" })).unwrap();
    let r = view["riders"].as_array().unwrap().iter().find(|r| r["entry"] == rider).unwrap();
    assert_eq!(r["percentage"], "73.00", "the jury's sheet over the programme's 50 points, not 150: {r}");
}

/// One set of penalties for the rider, on every judge's sheet (docs/deviations.md S40); the
/// exclusion box switches the rider to eliminated and back (Dressurreglement 8.3 (4)).
#[test]
fn penalties_are_one_set_and_the_exclusion_box_eliminates() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = rosson_file(dir.path(), "2026.8352");
    let show = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    office.call("set_up_class", &json!({ "show": show, "published": 108211 })).unwrap();
    let programmes = office.call("programmes", &json!({ "show": show })).unwrap();
    let lk = programmes.as_array().unwrap().iter().find(|p| p["name"] == "LK").unwrap()["id"].clone();
    office.call("edit_class", &json!({ "show": show, "class": "108211", "programme": lk })).unwrap();
    let people: Vec<u64> = office.call("find_officials", &json!({ "show": show, "query": "" })).unwrap().as_array().unwrap().iter().filter(|c| !c["titles"].as_array().unwrap().is_empty()).take(3).map(|c| c["person"].as_u64().unwrap()).collect();
    for (person, letter) in people.iter().zip(["H", "C", "M"]) {
        office.call("set_role", &json!({ "show": show, "class": "108211", "person": person, "role": "DRI", "letter": letter })).unwrap();
    }
    let view = || office.call("sheets", &json!({ "show": show, "class": "108211" })).unwrap();
    let v = view();
    let programme = v["programmes"].as_object().unwrap().values().next().unwrap().clone();
    let exclusion = programme["penalties"].as_array().unwrap().iter().position(|p| p["points"].is_null() && p["percent"].is_null()).expect("an exclusion box");
    let rider = v["riders"][0]["entry"].as_u64().unwrap();
    let judges: Vec<String> = v["judges"].as_array().unwrap().iter().map(|j| j["assignment"].as_str().unwrap().to_string()).collect();
    let movement = programme["sections"][0]["movements"][0]["number"].as_str().unwrap().to_string();
    office.call("edit_sheet", &json!({ "show": show, "class": "108211", "entry": rider, "judge": judges[1], "marks": { movement: "7" } })).unwrap();
    let seen = || {
        let v = view();
        let r = v["riders"].as_array().unwrap().iter().find(|r| r["entry"] == rider).unwrap().clone();
        let each: Vec<Value> = judges.iter().map(|j| r["sheets"][j]["penalties"].clone()).collect();
        (r["penalties"].clone(), each, r["out"].clone(), r["sheets"][&judges[0]]["total"].clone())
    };

    // The first error, ticked once: on every judge's sheet. Judge H has no marks yet: no total.
    office.call("set_penalties", &json!({ "show": show, "class": "108211", "entry": rider, "penalties": [0] })).unwrap();
    let (penalties, each, out, h_total) = seen();
    assert_eq!(penalties, json!([0]));
    assert!(each.iter().all(|p| *p == json!([0])), "{each:?}");
    assert!(out.is_null() && h_total.is_null(), "{out} {h_total}");

    // The exclusion box: eliminated; unticked, back in the class.
    office.call("set_penalties", &json!({ "show": show, "class": "108211", "entry": rider, "penalties": [0, exclusion] })).unwrap();
    let (penalties, _, out, _) = seen();
    assert_eq!(penalties, json!([0, exclusion]));
    let reasons = view()["reasons"].clone();
    let eliminated = reasons.as_array().unwrap().iter().find(|r| r["name"] == "ausgeschieden").unwrap()["code"].clone();
    assert_eq!(out, eliminated);
    office.call("set_penalties", &json!({ "show": show, "class": "108211", "entry": rider, "penalties": [0] })).unwrap();
    assert!(seen().2.is_null());
}
