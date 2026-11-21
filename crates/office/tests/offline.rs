//! Two computers on the same show, offline, then synced (R7, R11): the office's question of
//! 2026-11-16. Both rename the class, move riders and give start numbers; one adds a late
//! entry. Nothing is lost without the office being told, whatever the order they worked in.

use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

use office::Office;
use serde_json::{json, Value};

mod common;

/// A show set up on the PC, copied to the Mac (as downloading it from equievents.ch would).
struct Pair {
    pc: Option<Office>,
    mac: Option<Office>,
    libraries: (PathBuf, PathBuf),
    show: String,
    class: String,
    _dir: tempfile::TempDir,
}

impl Pair {
    fn new() -> Pair {
        let dir = tempfile::tempdir().unwrap();
        let (pc, mac) = (dir.path().join("pc"), dir.path().join("mac"));
        let (show, class) = {
            let a = Office::new(&pc, "pc").unwrap();
            let file = common::rosson_file(dir.path(), "2026.8352");
            let event = a.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
            let show = a.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
            let published = a.call("published", &json!({ "show": show })).unwrap().as_array().unwrap().iter().find(|p| p["number"] == "1").unwrap()["id"].as_u64().unwrap();
            let class = a.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string();
            (show, class)
        };
        std::fs::create_dir_all(&mac).unwrap();
        std::fs::copy(pc.join(format!("{show}.show")), mac.join(format!("{show}.show"))).unwrap();
        for lib in [&pc, &mac] {
            common::registry(lib, vec![common::registry_rider(999001, "Léa", "Nouvelle", "Bulle", "R"), common::registry_rider(999002, "Noé", "Tardif", "Morat", "R")], vec![common::registry_horse(888001, "QUICK STAR", &[2025, 2026]), common::registry_horse(888002, "LATE SHOW", &[2026])]);
        }
        Pair { pc: Some(Office::new(&pc, "pc").unwrap()), mac: Some(Office::new(&mac, "mac").unwrap()), libraries: (pc, mac), show, class, _dir: dir }
    }

    fn on(&self, mac: bool) -> &Office {
        if mac { self.mac.as_ref() } else { self.pc.as_ref() }.unwrap()
    }

    fn call(&self, mac: bool, command: &str, mut args: Value) -> Value {
        args["show"] = json!(self.show);
        let answer = self.on(mac).call(command, &args).unwrap_or_else(|e| panic!("{command}: {e}"));
        // One change per millisecond at most: the order they were made in is clear.
        sleep(Duration::from_millis(3));
        answer
    }

    fn starters(&self, mac: bool) -> Vec<(u64, Option<u64>)> {
        self.call(mac, "start_list", json!({ "class": self.class }))["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| (l["entry"].as_u64().unwrap(), l["start_number"].as_u64())).collect()
    }

    fn late_entry(&self, mac: bool, person: u64, horse: u64) -> u64 {
        let published = self.call(mac, "published", json!({})).as_array().unwrap().iter().find(|p| p["number"] == "1").unwrap()["id"].as_u64().unwrap();
        self.call(mac, "add_entry", json!({ "published": published, "rider": { "person": person }, "horse": { "horse": horse } }))["entry"].as_u64().unwrap()
    }

    /// The sync: each gets the other's operations (as equievents.ch hands them on).
    fn sync(&mut self) {
        self.pc = None;
        self.mac = None;
        let open = |lib: &Path| show::Store::open(&lib.join(format!("{}.show", self.show))).unwrap();
        let (mut a, mut b) = (open(&self.libraries.0), open(&self.libraries.1));
        let (from_a, from_b) = (a.log().unwrap(), b.log().unwrap());
        a.receive(&from_b).unwrap();
        b.receive(&from_a).unwrap();
        drop((a, b));
        self.pc = Some(Office::new(&self.libraries.0, "pc").unwrap());
        self.mac = Some(Office::new(&self.libraries.1, "mac").unwrap());
    }

    fn class_row(&self, mac: bool) -> Value {
        let show = self.call(mac, "show", json!({}));
        show["days"].as_array().unwrap().iter().flat_map(|d| d["classes"].as_array().unwrap().iter()).find(|c| c["id"] == self.class.as_str()).unwrap().clone()
    }

    fn class_name(&self, mac: bool) -> String {
        self.class_row(mac)["name"].as_str().unwrap().to_string()
    }

    fn conflicts(&self, mac: bool) -> Vec<Value> {
        self.call(mac, "conflicts", json!({}))["conflicts"].as_array().unwrap().clone()
    }
}

/// The office's scenario, the PC's late entry made `late_last` (after everything) or before the
/// Mac's work. Answers the start list both see, and the late entry.
/// The pair, the start list both see (entry, number), the late entry, the riders as drawn.
type Outcome = (Pair, Vec<(u64, Option<u64>)>, u64, Vec<u64>);

fn scenario(late_last: bool) -> Outcome {
    let mut pair = Pair::new();
    let class = pair.class.clone();
    let before = pair.starters(false);
    let (first, last) = (before[0].0, before[before.len() - 1].0);
    let (pc, mac) = (false, true);
    let late;
    pair.call(pc, "edit_class", json!({ "class": class, "name": "Nom du PC" }));
    if late_last {
        pair.call(mac, "edit_class", json!({ "class": class, "name": "Nom du Mac" }));
        pair.call(pc, "move_start", json!({ "class": class, "entry": first, "to": 3 }));
        pair.call(mac, "move_start", json!({ "class": class, "entry": last, "to": 1 }));
        pair.call(pc, "number", json!({ "class": class, "first": 1 }));
        pair.call(mac, "number", json!({ "class": class, "first": 101 }));
        late = pair.late_entry(pc, 999001, 888001);
    } else {
        pair.call(pc, "move_start", json!({ "class": class, "entry": first, "to": 3 }));
        pair.call(pc, "number", json!({ "class": class, "first": 1 }));
        late = pair.late_entry(pc, 999001, 888001);
        pair.call(mac, "edit_class", json!({ "class": class, "name": "Nom du Mac" }));
        pair.call(mac, "move_start", json!({ "class": class, "entry": last, "to": 1 }));
        pair.call(mac, "number", json!({ "class": class, "first": 101 }));
    }
    pair.sync();
    let list = pair.starters(pc);
    assert_eq!(list, pair.starters(mac), "both computers see the same list");
    assert_eq!(pair.conflicts(pc).len(), pair.conflicts(mac).len());
    (pair, list, late, before.iter().map(|(e, _)| *e).collect())
}

#[test]
fn both_worked_offline_on_the_same_class() {
    let (a, list_a, late_a, drawn_a) = scenario(false);
    let (b, list_b, late_b, drawn_b) = scenario(true);
    for (pair, list, late) in [(&a, &list_a, late_a), (&b, &list_b, late_b)] {
        // The Mac renamed it last.
        assert_eq!(pair.class_name(false), "Nom du Mac");
        // Both moves stay: the Mac's last rider first, the PC's first rider third… of the
        // riders the PC knew (the Mac's moved one went before them).
        assert_eq!(list.len(), 44);
        // The Mac numbered last: its numbers, 101 to 143, and the late entry the next one, last.
        let numbers: Vec<u64> = list.iter().filter_map(|(_, n)| *n).collect();
        let mut sorted = numbers.clone();
        sorted.sort();
        assert_eq!(sorted, (101..=144).collect::<Vec<_>>());
        assert_eq!(list.last(), Some(&(late, Some(144))));
        // Told: the name, the numbers (the PC's were replaced).
        let conflicts = pair.conflicts(false);
        let kinds: Vec<&str> = conflicts.iter().map(|c| c["kind"].as_str().unwrap()).collect();
        assert_eq!(kinds, vec!["numbers", "class"], "{conflicts:#?}");
        let name = &conflicts[1];
        assert_eq!((name["items"][0]["field"].as_str(), name["items"][0]["kept"].as_str(), name["items"][0]["replaced"].as_str()), (Some("name"), Some("Nom du Mac"), Some("Nom du PC")));
        assert_eq!((name["kept"]["here"].as_bool(), name["replaced"]["here"].as_bool()), (Some(false), Some(true)), "seen from the PC");
        assert_eq!(conflicts[0]["count"], 43);
        assert!(conflicts.iter().all(|c| c["restorable"] == true && c["seen"] == false));
    }
    // The same result whatever order they worked in (each rider by their place in the draw).
    let places = |list: &[(u64, Option<u64>)], drawn: &[u64]| list.iter().map(|(e, n)| (drawn.iter().position(|d| d == e).map_or(0, |i| i + 1), *n)).collect::<Vec<_>>();
    let (a_places, b_places) = (places(&list_a, &drawn_a), places(&list_b, &drawn_b));
    assert_eq!(a_places, b_places);
    // The Mac's last rider first, the PC's first rider fourth (after the PC's second and third).
    assert_eq!(&a_places[..5], &[(43, Some(101)), (2, Some(103)), (3, Some(104)), (1, Some(102)), (4, Some(105))]);
}

#[test]
fn the_other_version_put_back_and_looked_at() {
    let (pair, _, late, _) = scenario(false);
    let conflicts = pair.conflicts(false);
    let numbers = conflicts.iter().find(|c| c["kind"] == "numbers").unwrap()["id"].as_str().unwrap().to_string();
    let name = conflicts.iter().find(|c| c["kind"] == "class").unwrap()["id"].as_str().unwrap().to_string();
    // The PC puts its numbers back: 1 to 43 as it gave them, the late entry 44.
    pair.call(false, "restore_conflict", json!({ "id": numbers }));
    let list = pair.starters(false);
    let mut sorted: Vec<u64> = list.iter().filter_map(|(_, n)| *n).collect();
    sorted.sort();
    assert_eq!(sorted, (1..=44).collect::<Vec<_>>());
    assert_eq!(list.iter().find(|(e, _)| *e == late).unwrap().1, Some(44));
    // And its name.
    pair.call(false, "restore_conflict", json!({ "id": name }));
    assert_eq!(pair.class_name(false), "Nom du PC");
    // Looked at here: the two, and those putting them back made (the Mac will see those).
    let after = pair.call(false, "conflicts", json!({}));
    assert_eq!(after["unseen"], 0, "{after:#}");
    // Undone: the Mac's numbers again.
    pair.call(false, "undo", json!({}));
    assert_eq!(pair.class_name(false), "Nom du Mac");
    pair.call(false, "acknowledge_conflicts", json!({ "ids": after["conflicts"].as_array().unwrap().iter().map(|c| c["id"].clone()).collect::<Vec<_>>() }));
    assert_eq!(pair.call(false, "conflicts", json!({}))["unseen"], 0);
}

#[test]
fn a_name_here_a_time_there_both_stay() {
    let mut pair = Pair::new();
    let class = pair.class.clone();
    pair.call(false, "edit_class", json!({ "class": class, "name": "Libre R/N" }));
    pair.call(true, "edit_class", json!({ "class": class, "time": "13h30" }));
    // The Mac also moves the class later in its day (its classes are changed one by one).
    pair.call(true, "move_class", json!({ "class": class, "to": 3 }));
    pair.sync();
    let row = pair.class_row(false);
    assert_eq!((row["name"].as_str(), row["time"].as_str()), (Some("Libre R/N"), Some("13h30")), "{row:#}");
    assert_eq!(row, pair.class_row(true));
    assert!(pair.conflicts(false).is_empty());
}

#[test]
fn late_entries_on_both_computers() {
    let mut pair = Pair::new();
    let class = pair.class.clone();
    pair.call(false, "number", json!({ "class": class, "first": 1 }));
    pair.sync();
    let pc = pair.late_entry(false, 999001, 888001);
    let mac = pair.late_entry(true, 999002, 888002);
    pair.sync();
    let list = pair.starters(false);
    assert_eq!(list.len(), 45);
    // Both at the end, each with its own number.
    assert_eq!(&list[43..], &[(pc, Some(44)), (mac, Some(45))]);
    assert!(pair.conflicts(false).is_empty());
}
