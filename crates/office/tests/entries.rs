//! Entries made and changed at the show: late entries, taking them back, a change of rider,
//! moving to another class, an entry's level and note; all undoable.

use office::Office;
use serde_json::{json, Value};

mod common;

struct Show {
    office: Office,
    id: String,
    _dir: tempfile::TempDir,
}

impl Show {
    fn rehearsal() -> Show {
        let dir = tempfile::tempdir().unwrap();
        let office = Office::new(&dir.path().join("library"), "test").unwrap();
        let file = common::rosson_file(dir.path(), "2026.8352");
        let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
        let id = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
        Show { office, id, _dir: dir }
    }

    fn call(&self, command: &str, mut args: Value) -> Result<Value, String> {
        args["show"] = json!(self.id);
        self.office.call(command, &args)
    }

    fn published(&self, number: &str) -> u64 {
        self.call("published", json!({})).unwrap().as_array().unwrap().iter().find(|p| p["number"] == number).unwrap()["id"].as_u64().unwrap()
    }

    fn set_up(&self, number: &str) -> String {
        self.call("set_up_class", json!({ "published": self.published(number) })).unwrap()["class"].as_str().unwrap().to_string()
    }

    fn entry(&self, id: u64) -> Option<Value> {
        self.call("participants", json!({})).unwrap().as_array().unwrap().iter().find(|e| e["id"] == id).cloned()
    }

    fn starters(&self, class: &str) -> Vec<Value> {
        self.call("start_list", json!({ "class": class })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").cloned().collect()
    }
}

#[test]
fn a_late_entry_and_taking_it_back() {
    let show = Show::rehearsal();
    common::registry(
        &show._dir.path().join("library"),
        vec![common::registry_rider(999001, "Léa", "Nouvelle", "Bulle", "R, Gast")],
        vec![common::registry_horse(888001, "QUICK STAR", &[2025, 2026]), common::registry_horse(888002, "QUICK SILVER", &[2025])],
    );
    let class = show.set_up("1");
    show.call("number", json!({ "class": class, "first": 1 })).unwrap();
    let before = show.starters(&class);
    let highest = before.iter().filter_map(|l| l["start_number"].as_u64()).max().unwrap();

    // A rider and a horse the show does not know, from Swiss Equestrian's registry (never typed in).
    let found = show.call("find_riders", json!({ "query": "nouvelle" })).unwrap();
    assert_eq!((found["found"][0]["person"].as_u64(), found["found"][0]["in_show"].as_bool()), (Some(999001), Some(false)));
    let horses = show.call("find_horses", json!({ "query": "quick" })).unwrap()["found"].as_array().unwrap().clone();
    let paid = |id: u64| horses.iter().find(|h| h["horse"] == id).unwrap()["paid"].as_bool();
    assert_eq!((paid(888001), paid(888002)), (Some(true), Some(false)));
    assert!(show.call("add_entry", json!({ "published": show.published("1"), "rider": { "person": 999001 }, "horse": { "horse": 888002 } })).unwrap_err().contains("not registered with Swiss Equestrian for 2026"));
    assert!(show.call("add_entry", json!({ "published": show.published("1"), "rider": { "licence": "999009", "last_name": "Typed" }, "horse": { "horse": 888001 } })).is_err());
    let answer = show.call("add_entry", json!({ "published": show.published("1"), "rider": { "person": 999001 }, "horse": { "horse": 888001 } })).unwrap();
    let id = answer["entry"].as_u64().unwrap();
    let entry = show.entry(id).unwrap();
    assert_eq!((entry["rider"].clone(), entry["horse"].clone(), entry["organiser_place"].clone()), (json!("Léa Nouvelle"), json!("QUICK STAR"), json!(true)));
    // Their contact data came with them, as Swiss Equestrian has it (read only).
    assert_eq!(show.call("address", json!({ "person": 999001 })).unwrap()["street"], "Rue du Test 1");
    assert!(show.call("set_address", json!({ "person": 999001, "street": "Elsewhere" })).unwrap_err().contains("unknown command"));
    // At the end of the start list, with the next number.
    let after = show.starters(&class);
    let last = after.last().unwrap();
    assert_eq!((last["entry"].as_u64(), last["start_number"].as_u64()), (Some(id), Some(highest + 1)));

    // The same rider and horse twice: refused. A rider with a guest licence with a guest horse in
    // a class not set up; a rider without one cannot.
    assert!(show.call("add_entry", json!({ "published": show.published("1"), "rider": { "person": 999001 }, "horse": { "horse": 888001 } })).is_err());
    let guest = show.call("add_entry", json!({ "published": show.published("3"), "rider": { "person": 999001 }, "horse": { "guest": "petit tonnerre" } })).unwrap()["entry"].as_u64().unwrap();
    assert_eq!(show.entry(guest).unwrap()["horse"], "PETIT TONNERRE");
    let delivered = before[0]["entry"].as_u64().unwrap();
    assert!(show.call("change_horse", json!({ "entry": delivered, "guest": "Ailleurs" })).unwrap_err().contains("guest licence"));
    assert_eq!(show.call("find_horses", json!({ "query": "quick", "entry": delivered })).unwrap()["guest"], false);
    assert_eq!(show.call("find_horses", json!({ "query": "quick", "entry": guest })).unwrap()["guest"], true);

    // Taken back: gone, with its start; a delivered entry cannot be.
    show.call("remove_entry", json!({ "entry": id })).unwrap();
    assert!(show.entry(id).is_none());
    assert_eq!(show.starters(&class).len(), before.len());
    let delivered = before[0]["entry"].as_u64().unwrap();
    assert!(show.call("remove_entry", json!({ "entry": delivered })).unwrap_err().contains("only entries made by the office"));
    // Undo brings it back, in its place.
    show.call("undo", json!({})).unwrap();
    assert!(show.entry(id).is_some());
    assert_eq!(show.starters(&class).last().unwrap()["entry"].as_u64(), Some(id));

    // Several classes at once: an entry in each, one change (one undo takes them all back).
    let answer = show.call("add_entry", json!({ "published": [show.published("2"), show.published("4")], "rider": { "person": 999001 }, "horse": { "horse": 888001 } })).unwrap();
    let ids: Vec<u64> = answer["entries"].as_array().unwrap().iter().map(|i| i.as_u64().unwrap()).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|i| show.entry(*i).is_some_and(|e| e["horse"] == "QUICK STAR")));
    assert_eq!(show.call("undo", json!({})).unwrap()["undone"], "add_entry");
    assert!(ids.iter().all(|i| show.entry(*i).is_none()));
    assert!(show.call("add_entry", json!({ "published": [], "rider": { "person": 999001 }, "horse": { "horse": 888001 } })).is_err());
}

#[test]
fn a_change_of_rider_moving_and_details() {
    let show = Show::rehearsal();
    let (one, two) = (show.set_up("1"), show.set_up("2"));
    let starter = show.starters(&one).into_iter().find(|l| l["not_starting"] == false).unwrap();
    let entry = starter["entry"].as_u64().unwrap();
    let original = show.entry(entry).unwrap()["rider_id"].as_u64().unwrap();

    // Only the show's riders, never its officials (the user, 2026-11-15): an official who rides
    // nothing here is not found as one of the show's.
    let riders: Vec<u64> = show.call("participants", json!({})).unwrap().as_array().unwrap().iter().filter_map(|e| e["rider_id"].as_u64()).collect();
    let officials = show.call("event_officials", json!({ "language": "fr" })).unwrap();
    let official = officials["rows"].as_array().unwrap().iter().find(|r| !riders.contains(&r["person"].as_u64().unwrap()) && !r["name"].as_str().unwrap_or_default().is_empty()).unwrap().clone();
    let last = official["name"].as_str().unwrap().split_whitespace().last().unwrap().to_string();
    let found = show.call("find_riders", json!({ "query": last })).unwrap()["found"].as_array().unwrap().clone();
    assert!(!found.iter().any(|f| f["person"] == official["person"] && f["in_show"] == true), "{official} found as a rider of the show");

    // Another rider of the show: marked as a change (RW on the start list).
    let other = show.call("find_riders", json!({ "query": "Waldemer" })).unwrap()["found"][0]["person"].as_u64().unwrap();
    show.call("change_rider", json!({ "entry": entry, "rider": { "person": other } })).unwrap();
    let e = show.entry(entry).unwrap();
    assert_eq!((e["rider_id"].as_u64(), e["rider_changed"].clone()), (Some(other), json!(true)));
    let print = office::prints::start_lists(&show::Store::open(&show._dir.path().join("library").join(format!("{}.show", show.id))).unwrap().state().clone(), &[show::model::ClassId(one.clone())], office::prints::Language::Fr, false, "now").unwrap();
    assert!(print.data["classes"][0]["lines"].as_array().unwrap().iter().any(|l| l["mark"] == "RW"));
    // Back to the rider entered: no longer a change.
    show.call("change_rider", json!({ "entry": entry, "rider": { "person": original } })).unwrap();
    assert_eq!(show.entry(entry).unwrap()["rider_changed"], false);

    // Moved to class 2, with a result in class 1: the result and the start go, it starts at the end of 2.
    show.call("edit_result", json!({ "class": one, "entry": entry, "values": { "strafpunkte_initial_1": "0", "zeit_initial_1": "60" } })).unwrap();
    show.call("move_to_class", json!({ "entry": entry, "published": show.published("2") })).unwrap();
    assert!(show.starters(&one).iter().all(|l| l["entry"].as_u64() != Some(entry)));
    assert_eq!(show.starters(&two).last().unwrap()["entry"].as_u64(), Some(entry));
    assert_eq!(show.entry(entry).unwrap()["class_number"], "2");
    // Undo: back in class 1, with its result.
    show.call("undo", json!({})).unwrap();
    assert!(show.starters(&one).iter().any(|l| l["entry"].as_u64() == Some(entry)));
    let line = show.call("results", json!({ "class": one })).unwrap()["lines"].as_array().unwrap().iter().find(|l| l["entry"].as_u64() == Some(entry)).unwrap().clone();
    assert_eq!(line["has_result"], true);
    // Not to a dressage class.
    assert!(show.call("move_to_class", json!({ "entry": entry, "published": show.published("14") })).unwrap_err().contains("same discipline"));

    // Level and the office's note.
    show.call("edit_entry_details", json!({ "entry": entry, "stage": 2, "comment": " paie sur place " })).unwrap();
    let e = show.entry(entry).unwrap();
    assert_eq!((e["stage"].clone(), e["comment"].clone()), (json!(2), json!("paie sur place")));
    show.call("edit_entry_details", json!({ "entry": entry, "stage": 0, "comment": "" })).unwrap();
    let e = show.entry(entry).unwrap();
    assert_eq!((e["stage"].clone(), e["comment"].clone()), (Value::Null, json!("")));
}

/// Team classes: a late entry joins a team with a free place, or makes a new one (at the end of
/// the rotation); undo takes the new team away with its entry.
#[test]
fn late_entries_in_a_team_class() {
    let show = Show::rehearsal();
    common::registry(&show._dir.path().join("library"), (1..=3).map(|n| common::registry_rider(990_000 + n, "Tim", &format!("Équipier {n}"), "Bulle", "Gast")).collect(), vec![]);
    let published = show.published("7");
    let class = show.set_up("7");
    let teams = show.call("teams", json!({ "published": published })).unwrap();
    let size = teams["size"].as_u64().unwrap() as usize;
    let list = teams["teams"].as_array().unwrap().clone();
    assert!(!list.is_empty());
    let lines = |show: &Show| show.call("start_list", json!({ "class": class })).unwrap()["lines"].as_array().unwrap().clone();
    let headings = |show: &Show| lines(show).iter().filter(|l| l["kind"] == "team").map(|l| l["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();

    // Without a team: refused. A full team: refused. (Riders of the registry, with guest licences.)
    let rider = |n: u64| json!({ "person": 990_000 + n });
    assert!(show.call("add_entry", json!({ "published": published, "rider": rider(1), "horse": { "guest": "Un" } })).unwrap_err().contains("team"));
    if let Some(full) = list.iter().find(|t| t["riders"].as_u64().unwrap() as usize >= size) {
        assert!(show.call("add_entry", json!({ "published": published, "rider": rider(1), "horse": { "guest": "Un" }, "team": full["id"] })).unwrap_err().contains("full"));
    }

    // A new team: at the end of the rotation, under its name.
    let entry = show.call("add_entry", json!({ "published": published, "rider": rider(2), "horse": { "guest": "Deux" }, "new_team": "Les Nouveaux" })).unwrap()["entry"].as_u64().unwrap();
    assert_eq!(headings(&show).last().unwrap(), "Les Nouveaux");
    let team = show.call("teams", json!({ "published": published })).unwrap()["teams"].as_array().unwrap().iter().find(|t| t["name"] == "Les Nouveaux").unwrap().clone();
    assert_eq!(team["riders"], 1);
    // A second rider joins it: after the first.
    let second = show.call("add_entry", json!({ "published": published, "rider": rider(3), "horse": { "guest": "Trois" }, "team": team["id"] })).unwrap()["entry"].as_u64().unwrap();
    let all = lines(&show);
    let at = |e: u64| all.iter().position(|l| l["entry"] == e).unwrap();
    assert!(at(entry) < at(second));
    assert_eq!(all[at(second)]["position"], 2);
    // Renamed.
    show.call("rename_team", json!({ "team": team["id"], "name": "Les Derniers" })).unwrap();
    assert_eq!(headings(&show).last().unwrap(), "Les Derniers");

    // Undone, all three: no new team left.
    for _ in 0..3 {
        show.call("undo", json!({})).unwrap();
    }
    assert!(!headings(&show).iter().any(|h| h.starts_with("Les ")));
    assert!(!show.call("teams", json!({ "published": published })).unwrap()["teams"].as_array().unwrap().iter().any(|t| t["name"] == "Les Nouveaux"));
}

/// Teams in their class: one earlier in the rotation, a whole team not starting.
#[test]
fn teams_moved_and_not_starting() {
    let show = Show::rehearsal();
    let class = show.set_up("7");
    let teams = |show: &Show| -> Vec<(String, bool)> {
        show.call("start_list", json!({ "class": class })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "team").map(|l| (l["team"].as_str().unwrap().to_string(), l["not_starting"].as_bool().unwrap())).collect()
    };
    let before = teams(&show);
    // Two neighbours that start (the draw is random; some teams do not start at all).
    let at = before.windows(2).position(|w| !w[0].1 && !w[1].1).unwrap();
    let (first, second) = (before[at].0.clone(), before[at + 1].0.clone());

    show.call("move_team", json!({ "class": class, "team": second, "step": -1 })).unwrap();
    let after = teams(&show);
    assert_eq!((after[at].0.as_str(), after[at + 1].0.as_str()), (second.as_str(), first.as_str()));
    // The timekeeper's rotation follows: the moved team's first rider before the other's.
    let list = show.call("start_list", json!({ "class": class })).unwrap();
    let firsts: Vec<&Value> = list["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter" && l["position"] == 1).collect();
    assert!(firsts.len() >= 2);
    // Already first: nothing changes.
    assert_eq!(show.call("move_team", json!({ "class": class, "team": after[0].0, "step": -1 })).unwrap()["unchanged"], true);
    // Dragged: the last team first, the others in their order.
    let last = after.last().unwrap().0.clone();
    show.call("move_team", json!({ "class": class, "team": last, "to": 0 })).unwrap();
    let dragged = teams(&show);
    assert_eq!(dragged[0].0, last);
    assert_eq!(dragged[1..].iter().map(|t| &t.0).collect::<Vec<_>>(), after[..after.len() - 1].iter().map(|t| &t.0).collect::<Vec<_>>());
    show.call("undo", json!({})).unwrap();

    // Start numbers as Rosson gives them: the rider's place in the team, then the team's in two
    // digits (rider 1 of the third team: 103).
    show.call("number", json!({ "class": class })).unwrap();
    let list = show.call("start_list", json!({ "class": class })).unwrap();
    let mut team_place = 0;
    for l in list["lines"].as_array().unwrap() {
        if l["kind"] == "team" {
            team_place += 1;
        } else if l["kind"] == "starter" && team_place > 0 {
            let expected = format!("{}{:02}", l["position"], team_place).parse::<u64>().unwrap();
            assert_eq!(l["start_number"].as_u64(), Some(expected), "{l}");
        }
    }
    assert!(team_place > 10);
    show.call("undo", json!({})).unwrap();

    // A whole team does not start; again; undone.
    assert!(!after.iter().any(|(t, ns)| t == &first && *ns));
    show.call("team_not_starting", json!({ "team": first, "not_starting": true })).unwrap();
    assert!(teams(&show).iter().any(|(t, ns)| t == &first && *ns));
    assert!(show.starters(&class).iter().filter(|l| l["not_starting"] == true).count() >= 2);
    show.call("undo", json!({})).unwrap();
    assert_eq!(teams(&show), after);
    show.call("undo", json!({})).unwrap();
    assert_eq!(teams(&show), before);
}

#[test]
fn the_registry_from_the_simulation() {
    let show = Show::rehearsal();
    // Another show in the library: the simulation's registry is the library's riders and horses.
    let file = common::rosson_file(show._dir.path(), "2026.8299");
    let event = show.office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
    let other = show.office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();

    // Never Swiss Equestrian in a test: the simulation's riders and horses instead.
    show.office.simulate_registry();
    assert_eq!(show.call("registry", json!({})).unwrap()["persons"], 0);
    // As Rosson: with the event's access key, none without.
    assert!(show.call("registry_download", json!({})).unwrap_err().contains("access key"));
    show.call("set_connection", json!({ "access_key": "KEY" })).unwrap();
    let about = show.call("registry_download", json!({})).unwrap();
    assert_eq!(about["simulated"], true);
    assert!(about["persons"].as_u64().unwrap() > 300 && about["horses"].as_u64().unwrap() > 300, "{about}");
    // What the simulation received: the two reads, their answers kept as sizes only.
    let received = show.call("simulation_log", json!({})).unwrap();
    let paths: Vec<&str> = received.as_array().unwrap().iter().map(|r| r["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["getPferde", "getPersonen"]);
    assert!(received.as_array().unwrap().iter().all(|r| r["answer"]["bytes"].as_u64().is_some()));

    // A driver of the other show, not of this one: found in the registry, put on an entry.
    let ours: std::collections::HashSet<u64> = show.call("participants", json!({})).unwrap().as_array().unwrap().iter().map(|e| e["rider_id"].as_u64().unwrap()).collect();
    let theirs = show.office.call("participants", &json!({ "show": other })).unwrap().as_array().unwrap().iter().find(|e| !ours.contains(&e["rider_id"].as_u64().unwrap())).unwrap().clone();
    let found = show.call("find_riders", json!({ "query": theirs["rider"] })).unwrap()["found"].as_array().unwrap().clone();
    let pick = found.iter().find(|f| f["person"] == theirs["rider_id"]).unwrap();
    assert_eq!(pick["in_show"], false);
    let class = show.set_up("1");
    let entry = show.starters(&class)[0]["entry"].as_u64().unwrap();
    show.call("change_rider", json!({ "entry": entry, "rider": { "person": pick["person"] } })).unwrap();
    assert_eq!(show.entry(entry).unwrap()["rider"], theirs["rider"]);
    // Now the show's own.
    assert_eq!(show.call("find_riders", json!({ "query": theirs["rider"] })).unwrap()["found"][0]["in_show"], true);

    // One of their horses, if Swiss Equestrian lets it start this year.
    let horses: Vec<Value> = show.office.call("horses", &json!({ "show": other })).unwrap().as_array().unwrap().clone();
    let ours_horses: std::collections::HashSet<u64> = show.call("horses", json!({})).unwrap().as_array().unwrap().iter().map(|h| h["id"].as_u64().unwrap()).collect();
    let horse = horses.iter().find(|h| !ours_horses.contains(&h["id"].as_u64().unwrap()) && h["name"].as_str().unwrap().len() >= 3).unwrap();
    let found = show.call("find_horses", json!({ "query": horse["name"] })).unwrap()["found"].as_array().unwrap().clone();
    let pick = found.iter().find(|f| f["horse"] == horse["id"]).unwrap();
    assert_eq!(pick["in_show"], false);
    let changed = show.call("change_horse", json!({ "entry": entry, "horse": horse["id"] }));
    if pick["paid"] == true {
        changed.unwrap();
        assert_eq!(show.entry(entry).unwrap()["horse"], horse["name"]);
    } else {
        assert!(changed.unwrap_err().contains("not registered"));
    }
}

/// A guest horse's name in capitals wherever it is shown (the participants, the start lists),
/// whoever typed it: a rider entering on Swiss Equestrian often does not (the user, 2026-11-21).
/// The name entered stays as it was: in the show and in the Rosson export.
#[test]
fn a_guest_horse_is_shown_in_capitals() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let mut guest = 0;
    let file = common::rosson_file_edited(dir.path(), "2026.8352", |data| {
        let n = data["nennungen"].as_array_mut().unwrap().iter_mut().find(|n| n["pferde"].as_array().is_some_and(|p| !p.is_empty()) && n["kein_start"] != true).unwrap();
        guest = n["id"].as_u64().or_else(|| n["id"].as_str().and_then(|s| s.parse().ok())).unwrap();
        n["pferde"][0]["pferd_id"] = json!(5);
        n["pferde"][0]["pferdename"] = json!("Petit Nuage d’été");
        n["pferde"][0]["gastpferd"] = json!(true);
    });
    let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
    let id = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    let show = Show { office, id, _dir: dir };

    let entry = show.entry(guest).unwrap();
    assert_eq!((entry["horse"].clone(), entry["guest_horse"].clone()), (json!("PETIT NUAGE D’ÉTÉ"), json!(true)), "{entry}");
    let class = show.set_up(entry["class_number"].as_str().unwrap());
    let line = show.starters(&class).into_iter().find(|l| l["entry"] == guest).unwrap();
    assert_eq!(line["horse"], "PETIT NUAGE D’ÉTÉ", "{line}");

    let path = show._dir.path().join("rosson.txt");
    show.call("export_rosson", json!({ "path": path, "private": true })).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("Petit%20Nuage%20d") && !text.contains("PETIT%20NUAGE"));
}
