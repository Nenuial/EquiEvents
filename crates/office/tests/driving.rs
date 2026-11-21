//! Driving (Fahren), on a corpus class made a driving class: dressage on 160, marathon, cones,
//! the overall ranking, the upload.

use office::Office;
use serde_json::{json, Value};

mod common;

/// Show 2026.8352 with its class 108211 made a driving class: dressage, marathon, cones; its
/// drivers hold the FB licence.
fn driving_show(dir: &std::path::Path) -> (Office, String) {
    driving_show_with(dir, &["dressur", "gelaende", "hindernis"])
}

/// The same with the phases given (Swiss Equestrian's names).
fn driving_show_with(dir: &std::path::Path, phases: &[&str]) -> (Office, String) {
    let file = common::rosson_file_edited(dir, "2026.8352", |data| {
        for a in data["ausschreibung_pruefungen"].as_array_mut().unwrap() {
            if a["id"] == 108211 {
                a["disziplin_id"] = json!(3);
                a["fahren_zwischenpruefungen"] = json!(phases);
                a["fahren_lizenzen"] = json!(["FB", "FL"]);
            }
        }
        for n in data["nennungen"].as_array_mut().unwrap() {
            if n["ausschreibung_pruefung_id"] == 108211 {
                n["lizenzen"] = json!("FB");
                n["fahren_gespann"] = json!("2");
                // Every horse in every phase, as Swiss Equestrian delivers a driving entry.
                for h in n["pferde"].as_array_mut().into_iter().flatten() {
                    h["einsaetze"] = json!(phases);
                }
            }
        }
    });
    let office = Office::new(&dir.join("library"), "test").unwrap();
    let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
    let show = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    (office, show)
}

#[test]
fn driving_dressage_marathon_cones_and_overall() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let (cd, ge, hi) = (format!("{parent}_cd"), format!("{parent}_ge"), format!("{parent}_hi"));
    let riders: Vec<u64> = call("start_list", json!({ "class": cd })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| l["entry"].as_u64().unwrap()).collect();
    let (a, b) = (riders[0], riders[1]);

    // Dressage: judges at C and R; every movement 7 (a) or 6 (b): 160 − 7 × 16 = 48, 160 − 96 = 64.
    let test = call("programmes", json!({ "class": cd })).unwrap().as_array().unwrap().iter().find(|p| p["name"] == "GA06/60").unwrap()["id"].clone();
    call("edit_class", json!({ "class": cd, "programme": test })).unwrap();
    let judges: Vec<u64> = call("find_officials", json!({ "query": "" })).unwrap().as_array().unwrap().iter().filter(|o| !o["titles"].as_array().unwrap().is_empty()).take(2).map(|o| o["person"].as_u64().unwrap()).collect();
    for (judge, letter) in judges.iter().zip(["B", "C"]) {
        call("set_role", json!({ "class": cd, "person": judge, "role": "DRI", "letter": letter })).unwrap();
    }
    let sheets = call("sheets", json!({ "class": cd })).unwrap();
    let letters: Vec<&str> = sheets["judges"].as_array().unwrap().iter().map(|j| j["letter"].as_str().unwrap()).collect();
    assert_eq!(letters, ["C", "B"]);
    let movements: Vec<String> = sheets["programmes"].as_object().unwrap().values().next().unwrap()["sections"].as_array().unwrap().iter().flat_map(|s| s["movements"].as_array().unwrap().clone()).map(|m| m["number"].as_str().unwrap().to_string()).collect();
    for (entry, mark) in [(a, "7"), (b, "6")] {
        for j in sheets["judges"].as_array().unwrap() {
            let marks: serde_json::Map<String, Value> = movements.iter().map(|m| (m.clone(), json!(mark))).collect();
            call("edit_sheet", json!({ "class": cd, "entry": entry, "judge": j["assignment"], "marks": marks })).unwrap();
        }
    }
    let ranking = call("ranking", json!({ "class": cd })).unwrap();
    let faults = |r: &Value, e: u64| r["lines"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some(e.to_string().as_str())).map(|l| l["faults"].clone()).unwrap_or(Value::Null);
    assert_eq!((faults(&ranking, a), faults(&ranking, b)), (json!("48"), json!("64")));
    assert_eq!(ranking["lines"][0]["id"], a.to_string());

    // Marathon: A allowed 30:00, B 50:00, 0.25 a second. a: A 30:10.5 → 2.75; B on time; obstacle 1
    // 95.3 s → 23.83: 26.58. b: B 50:20 → 5; obstacle 1 80 s → 20: 25.
    call("set_class_details", json!({ "class": ge, "driving": { "a_allowed": "1800", "b_allowed": "3000", "factor": "0.25", "obstacles": 2 } })).unwrap();
    let fields: Vec<String> = call("results", json!({ "class": ge })).unwrap()["fields"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect();
    assert_eq!(fields.len(), 8 + 6);
    call("edit_result", json!({ "class": ge, "entry": a, "values": { "a_start": "10:00", "a_finish": "10:30:10.5", "b_start": "11:00", "b_finish": "11:50", "h1_time": "95.3" } })).unwrap();
    call("edit_result", json!({ "class": ge, "entry": b, "values": { "a_start": "10:05", "a_finish": "10:35", "b_start": "11:05", "b_finish": "11:55:20", "h1_time": "80" } })).unwrap();
    let ranking = call("ranking", json!({ "class": ge })).unwrap();
    assert_eq!((faults(&ranking, a), faults(&ranking, b)), (json!("26.58"), json!("25.00")));

    // Cones: allowed 120 s for FB, a bonus of 2 s for pairs. a full event: Wertung A, the overtime
    // to the hundredth (Fahrreglement 8.4.4 (4)). a: 124.2 → 2.2 s over → 1.1 + 3 = 4.1.
    assert!(call("ranking", json!({ "class": hi })).is_ok());
    call("edit_result", json!({ "class": hi, "entry": a, "values": { "zeit_1": "124.2", "hindernis_1": "3" } })).unwrap();
    assert!(call("ranking", json!({ "class": hi })).unwrap_err().contains("FB"), "no allowed time yet");
    call("set_class_details", json!({ "class": hi, "driving": { "cones": "a", "rounds": 1, "allowed": { "1:FB": "120" }, "bonus": { "2": "2" } } })).unwrap();
    call("edit_result", json!({ "class": hi, "entry": b, "values": { "zeit_1": "118", "hindernis_1": "0" } })).unwrap();
    let ranking = call("ranking", json!({ "class": hi })).unwrap();
    assert_eq!((faults(&ranking, a), faults(&ranking, b)), (json!("4.1"), json!("0")));

    // Overall: a 48 + 26.58 + 4.1 = 78.68; b 64 + 25 + 0 = 89.
    let overall = call("ranking", json!({ "class": parent })).unwrap();
    assert_eq!((overall["lines"][0]["id"].clone(), overall["lines"][0]["faults"].clone()), (json!(a.to_string()), json!("78.68")));

    // To Rosson's format and back: settings, results (start and finish of day), judges.
    let path = dir.path().join("back.txt");
    call("export_rosson", json!({ "path": path, "private": false })).unwrap();
    let events = office.call("rosson_events", &json!({ "path": path })).unwrap();
    let back = office.call("import_rosson", &json!({ "path": path, "event": events[0]["id"], "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    let lines = |show: &str| -> Vec<(Value, Value)> { office.call("ranking", &json!({ "show": show, "class": parent })).unwrap()["lines"].as_array().unwrap().iter().map(|l| (l["id"].clone(), l["faults"].clone())).collect() };
    assert_eq!(lines(&back), lines(&show));
    let cones = office.call("class_details", &json!({ "show": back, "class": hi })).unwrap();
    assert_eq!(cones["driving"]["allowed"]["1:FB"], json!("120"));
    let marathon = office.call("results", &json!({ "show": back, "class": ge })).unwrap();
    let first = marathon["lines"].as_array().unwrap().iter().find(|l| l["entry"] == a).unwrap().clone();
    assert_eq!(first["values"]["a_finish"], json!("37810.5"));

    // Sent: each phase's penalties named after it, the total.
    call("set_connection", json!({ "access_key": "KEY" })).unwrap();
    call("send", json!({ "what": "results", "class": parent })).unwrap();
    let log = call("simulation_log", json!({})).unwrap();
    let put = log.as_array().unwrap().iter().find(|r| r["path"] == "putResultate").unwrap();
    let row = put["body"]["resultate"].as_array().unwrap().iter().find(|r| r["rang"] == 1).unwrap();
    assert_eq!((row["strafpunkte_dressur"].as_f64(), row["strafpunkte_gelaende"].as_f64(), row["strafpunkte_hindernis"].as_f64(), row["strafpunkte_total"].as_f64()), (Some(48.0), Some(26.58), Some(4.1), Some(78.68)));

    // A full event: eliminated in the marathon is not out of the event (Fahrreglement 13.10
    // §7.1.2). a's marathon counts the highest there, b's 25, + 25 %: 31.25; a 48 + 31.25 + 4.1 =
    // 83.35 — and a, though better than b's 89 in total, ranks after b, who finished every test.
    let reason = call("results", json!({ "class": ge })).unwrap()["reasons"][0]["code"].clone();
    call("edit_result", json!({ "class": ge, "entry": a, "values": {}, "out": reason })).unwrap();
    let overall = call("ranking", json!({ "class": parent })).unwrap();
    let lines: Vec<(String, Value, Value)> = overall["lines"].as_array().unwrap().iter().take(2).map(|l| (l["id"].as_str().unwrap().to_string(), l["rank"].clone(), l["faults"].clone())).collect();
    assert_eq!(lines, [(b.to_string(), json!(1), json!("89.00")), (a.to_string(), json!(2), json!("83.35"))]);
}

/// A full event's ties (13.10 §7.1.1): the marathon decides, then dressage — whatever the order
/// of the phases.
#[test]
fn a_full_driving_event_ties_by_the_marathon() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let (cd, ge, hi) = (format!("{parent}_cd"), format!("{parent}_ge"), format!("{parent}_hi"));
    let riders: Vec<u64> = call("start_list", json!({ "class": cd })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| l["entry"].as_u64().unwrap()).collect();
    let (a, b) = (riders[0], riders[1]);
    // Dressage given as each judge's total: a 40, b 50.
    let test = call("programmes", json!({ "class": cd })).unwrap().as_array().unwrap().iter().find(|p| p["name"] == "GA06/60").unwrap()["id"].clone();
    call("edit_class", json!({ "class": cd, "programme": test })).unwrap();
    call("set_class_details", json!({ "class": cd, "totals_per_judge": true })).unwrap();
    let judge = call("find_officials", json!({ "query": "" })).unwrap().as_array().unwrap().iter().find(|o| !o["titles"].as_array().unwrap().is_empty()).unwrap()["person"].clone();
    call("set_role", json!({ "class": cd, "person": judge, "role": "DRI", "letter": "C" })).unwrap();
    let assignment = call("sheets", json!({ "class": cd })).unwrap()["judges"][0]["assignment"].clone();
    for (entry, total) in [(a, "40"), (b, "50")] {
        call("edit_sheet", json!({ "class": cd, "entry": entry, "judge": assignment, "marks": {}, "total": total })).unwrap();
    }
    // Marathon: a 30, b 20 (penalties entered). Cones: a 0, b 0. Totals: a 70, b 70: the
    // marathon decides, b first — dressage alone would put a first.
    call("set_class_details", json!({ "class": ge, "driving": { "a_allowed": "1800", "b_allowed": "3000", "factor": "0.25", "obstacles": 1 } })).unwrap();
    for (entry, penalties) in [(a, "30"), (b, "20")] {
        call("edit_result", json!({ "class": ge, "entry": entry, "values": { "a_start": "10:00", "a_finish": "10:30", "b_start": "11:00", "b_finish": "11:50", "a_penalties": penalties } })).unwrap();
    }
    call("set_class_details", json!({ "class": hi, "driving": { "cones": "b", "rounds": 1, "allowed": { "1:FB": "200", "1:FL": "200", "1:FM": "200", "1:FS": "200" } } })).unwrap();
    for entry in [a, b] {
        call("edit_result", json!({ "class": hi, "entry": entry, "values": { "zeit_1": "150", "hindernis_1": "0" } })).unwrap();
    }
    // Cones in a full event: Wertung A whatever is set (8.1.1): 150 s within 200 → 0, not 150.
    let cones = call("ranking", json!({ "class": hi })).unwrap();
    assert!(cones["lines"].as_array().unwrap().iter().all(|l| l["faults"] == "0"), "{cones}");
    let overall = call("ranking", json!({ "class": parent })).unwrap();
    let lines: Vec<(String, Value, Value)> = overall["lines"].as_array().unwrap().iter().take(2).map(|l| (l["id"].as_str().unwrap().to_string(), l["rank"].clone(), l["faults"].clone())).collect();
    assert_eq!(lines, [(b.to_string(), json!(1), json!("70")), (a.to_string(), json!(2), json!("70"))]);
}

/// Cones: a round driven over its time limit eliminates (Rosson, reason 27; the rules: "le temps
/// limite est le double du temps accordé"). Unset, the limit is twice the allowed time.
#[test]
fn a_part_reopened_with_its_class() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let cones = format!("{parent}_hi");
    let status = |class: &str| call("start_list", json!({ "class": class })).unwrap()["class"]["status"].as_str().unwrap().to_string();
    // The whole class final: its parts are too (as Tavannes 2026 came from Rosson).
    call("finalise", json!({ "class": parent })).unwrap();
    assert_eq!((status(&parent), status(&cones)), ("final".into(), "final".into()));
    assert!(call("draw", json!({ "class": cones })).unwrap_err().contains("final"));
    // Reopening the part (its row says final) reopens the class: both open, with a reason.
    assert!(call("reopen", json!({ "class": cones, "reason": "" })).is_err());
    let answer = call("reopen", json!({ "class": cones, "reason": "cones result corrected" })).unwrap();
    assert!(answer.get("unchanged").is_none(), "{answer}");
    // The part back to its own status (no results yet: planned).
    assert_eq!((status(&parent), status(&cones)), ("running".into(), "planned".into()));
    call("draw", json!({ "class": cones })).unwrap();
    // Undone (the draw, then the reopening): final again.
    call("undo", json!({})).unwrap();
    call("undo", json!({})).unwrap();
    assert_eq!((status(&parent), status(&cones)), ("final".into(), "final".into()));
    // A part made final on its own, the class too: both reopened.
    call("reopen", json!({ "class": parent, "reason": "again" })).unwrap();
    call("finalise", json!({ "class": cones })).unwrap();
    call("finalise", json!({ "class": parent })).unwrap();
    call("reopen", json!({ "class": cones, "reason": "again" })).unwrap();
    assert_eq!((status(&parent), status(&cones)), ("running".into(), "running".into()));
}

#[test]
fn cones_over_the_time_limit_eliminated() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let hi = format!("{parent}_hi");
    let riders: Vec<u64> = call("start_list", json!({ "class": hi })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| l["entry"].as_u64().unwrap()).collect();
    let (a, b) = (riders[0], riders[1]);
    call("set_class_details", json!({ "class": hi, "driving": { "cones": "a", "rounds": 1, "allowed": { "1:FB": "120" } } })).unwrap();
    call("edit_result", json!({ "class": hi, "entry": a, "values": { "zeit_1": "241", "hindernis_1": "0" } })).unwrap();
    call("edit_result", json!({ "class": hi, "entry": b, "values": { "zeit_1": "130", "hindernis_1": "6" } })).unwrap();
    let line = |r: &Value, e: u64| r["lines"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some(e.to_string().as_str()) || l["entry"] == e).unwrap().clone();
    // 241 s over the limit of 2 × 120: out; b ranked first.
    let ranking = call("ranking", json!({ "class": hi })).unwrap();
    assert_eq!((line(&ranking, a)["rank"].clone(), line(&ranking, b)["rank"].clone()), (Value::Null, json!(1)));
    let results = call("results", json!({ "class": hi })).unwrap();
    assert_eq!((line(&results, a)["over_maximum"].clone(), line(&results, a)["out"].clone()), (json!(true), Value::Null), "the rules' elimination, not the office's");

    // A limit set by hand: 250 s, a is in again (241 → 121 s started over → 60.5).
    call("set_class_details", json!({ "class": hi, "driving": { "cones": "a", "rounds": 1, "allowed": { "1:FB": "120" }, "maximum": { "1:FB": "250" } } })).unwrap();
    let ranking = call("ranking", json!({ "class": hi })).unwrap();
    assert_eq!(line(&ranking, a)["faults"], json!("60.50"));
    assert_eq!(line(&call("results", json!({ "class": hi })).unwrap(), a)["over_maximum"], json!(false));

    // Back to 240: out again, sent as Rosson's reason 27, and kept in Rosson's format.
    call("set_class_details", json!({ "class": hi, "driving": { "cones": "a", "rounds": 1, "allowed": { "1:FB": "120" }, "maximum": { "1:FB": "240" } } })).unwrap();
    let path = dir.path().join("back.txt");
    call("export_rosson", json!({ "path": path, "private": false })).unwrap();
    let events = office.call("rosson_events", &json!({ "path": path })).unwrap();
    let back = office.call("import_rosson", &json!({ "path": path, "event": events[0]["id"], "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    let details = office.call("class_details", &json!({ "show": back, "class": hi })).unwrap();
    assert_eq!(details["driving"]["maximum"]["1:FB"], json!("240"));
    assert_eq!(line(&office.call("ranking", &json!({ "show": back, "class": hi })).unwrap(), a)["rank"], Value::Null);
    call("set_connection", json!({ "access_key": "KEY" })).unwrap();
    call("send", json!({ "what": "results", "class": parent })).unwrap();
    let log = call("simulation_log", json!({})).unwrap();
    let put = log.as_array().unwrap().iter().find(|r| r["path"] == "putResultate").unwrap();
    let row = put["body"]["resultate"].as_array().unwrap().iter().find(|r| r["aufgabegrund_hindernis_id"].is_number()).expect("a's row with the reason").clone();
    assert_eq!(row["aufgabegrund_hindernis_id"], json!(27));
}

/// Driving: which horse starts in which phase (Rosson's horse × phase grid); start lists and
/// results sent with only the phase's horses; kept in Rosson's format.
#[test]
fn horses_by_phase() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let hi = format!("{parent}_hi");
    let row = |e: u64| call("participants", json!({})).unwrap().as_array().unwrap().iter().find(|r| r["id"] == e).unwrap().clone();
    let a = call("start_list", json!({ "class": hi })).unwrap()["lines"].as_array().unwrap().iter().find(|l| l["kind"] == "starter").unwrap()["entry"].as_u64().unwrap();
    assert_eq!(row(a)["driving_phases"], json!(["dressur", "gelaende", "hindernis"]));
    assert_eq!(row(a)["driving_horses"][0]["phases"], json!(["dressur", "gelaende", "hindernis"]));
    let rider = row(a)["rider_id"].clone();

    // The horse out of the cones: in the class's order, whatever the order given.
    call("set_horse_phases", json!({ "entry": a, "horse": 0, "phases": ["gelaende", "dressur"] })).unwrap();
    assert_eq!(row(a)["driving_horses"][0]["phases"], json!(["dressur", "gelaende"]));
    assert!(call("set_horse_phases", json!({ "entry": a, "horse": 0, "phases": ["springen"] })).is_err(), "not a phase of the class");
    assert!(call("set_horse_phases", json!({ "entry": a, "horse": 9, "phases": [] })).is_err());

    // Sent: the cones start list without the horse, the dressage one with it.
    call("set_connection", json!({ "access_key": "KEY" })).unwrap();
    call("send", json!({ "what": "start_lists", "published": 108211, "language": "de" })).unwrap();
    let log = call("simulation_log", json!({})).unwrap();
    let lists = log.as_array().unwrap().iter().find(|r| r["path"] == "putStartlisten").unwrap()["body"]["startlisten"].clone();
    let horses_of = |code: &str| -> usize {
        let list = lists.as_array().unwrap().iter().find(|l| l["nummer"].as_str().is_some_and(|n| n.ends_with(code))).unwrap_or_else(|| panic!("{code}: {lists}"));
        let line = list["zeilen"].as_array().unwrap().iter().find(|z| z["reiter_id"] == rider).unwrap().clone();
        assert_eq!(line["weiteres"], json!("FB / 2 Sp"), "the licence and the turnout");
        line["pferde"].as_array().map_or(0, |p| p.len())
    };
    assert_eq!(horses_of("Dressur"), 1);
    assert_eq!(horses_of("Kegelfahren"), 0);

    // Kept in Rosson's format; undone with the rest.
    let path = dir.path().join("back.txt");
    call("export_rosson", json!({ "path": path, "private": false })).unwrap();
    let events = office.call("rosson_events", &json!({ "path": path })).unwrap();
    let back = office.call("import_rosson", &json!({ "path": path, "event": events[0]["id"], "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    let rows = office.call("participants", &json!({ "show": back })).unwrap();
    assert_eq!(rows.as_array().unwrap().iter().find(|r| r["id"] == a).unwrap()["driving_horses"][0]["phases"], json!(["dressur", "gelaende"]));
    call("undo", json!({})).unwrap();
    call("undo", json!({})).unwrap();
    assert_eq!(row(a)["driving_horses"][0]["phases"], json!(["dressur", "gelaende", "hindernis"]));
}

/// Driving's multitest, entered judge by judge as Rosson's form: Rosson's programme B1 (items
/// 1–17, coefficients 1 … 3), marks in tenths, penalties A, B, C; each judge's total, the mean,
/// the most points first.
#[test]
fn multitest_entered_and_ranked() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show_with(dir.path(), &["dressur", "multitest"]);
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let mt = format!("{parent}_mt");
    call("set_class_details", json!({ "class": mt, "driving": { "multitest": "B1" } })).unwrap();
    let judges: Vec<u64> = call("find_officials", json!({ "query": "" })).unwrap().as_array().unwrap().iter().filter(|o| !o["titles"].as_array().unwrap().is_empty()).take(2).map(|o| o["person"].as_u64().unwrap()).collect();
    for (judge, letter) in judges.iter().zip(["C", "B"]) {
        call("set_role", json!({ "class": mt, "person": judge, "role": "DRI", "letter": letter })).unwrap();
    }
    let view = call("multitest", json!({ "class": mt })).unwrap();
    assert_eq!((view["programme"].as_str(), view["items"].as_array().unwrap().len()), (Some("B1"), 17));
    assert_eq!(view["items"][16]["coefficient"], json!("3"));
    let assignments: Vec<Value> = view["judges"].as_array().unwrap().iter().map(|j| j["assignment"].clone()).collect();
    let (a, b) = (view["lines"][0]["entry"].as_u64().unwrap(), view["lines"][1]["entry"].as_u64().unwrap());
    let sheet = |entry: u64, judge: &Value, mark: &str, extra: Value| {
        let mut marks: serde_json::Map<String, Value> = (1..=17).map(|n| (n.to_string(), json!(mark))).collect();
        for (k, v) in extra.as_object().unwrap() {
            marks.insert(k.clone(), v.clone());
        }
        call("edit_sheet", json!({ "class": mt, "entry": entry, "judge": judge, "marks": marks }))
    };
    // a: judge C all 7 → 7 × 24 = 168 − 2 − 0.5 = 165.5; judge R all 6.5 → 156. Mean 160.75.
    sheet(a, &assignments[0], "7", json!({ "strafpunkte_a": "2", "strafpunkte_b": "0.5" })).unwrap();
    sheet(a, &assignments[1], "6.5", json!({})).unwrap();
    // b: both all 7.1 → 170.4.
    sheet(b, &assignments[0], "7.1", json!({})).unwrap();
    sheet(b, &assignments[1], "7.1", json!({})).unwrap();
    let view = call("multitest", json!({ "class": mt })).unwrap();
    let line = |e: u64| view["lines"].as_array().unwrap().iter().find(|l| l["entry"] == e).unwrap().clone();
    assert_eq!(line(a)["totals"][assignments[0].as_str().unwrap()], json!("165.5"));
    assert_eq!((line(a)["total"].clone(), line(a)["rank"].clone()), (json!("160.75"), json!(2)));
    assert_eq!((line(b)["total"].clone(), line(b)["rank"].clone()), (json!("170.4"), json!(1)));

    // Rosson's steps: items in tenths, B in halves, A and C whole; no item 18 in B1.
    assert!(sheet(a, &assignments[0], "7.25", json!({})).is_err());
    assert!(call("edit_sheet", json!({ "class": mt, "entry": a, "judge": assignments[0], "marks": { "strafpunkte_b": "0.25" } })).is_err());
    assert!(call("edit_sheet", json!({ "class": mt, "entry": a, "judge": assignments[0], "marks": { "strafpunkte_c": "1.5" } })).is_err());
    assert!(call("edit_sheet", json!({ "class": mt, "entry": a, "judge": assignments[0], "marks": { "18": "5" } })).is_err());
}

/// Marathon: over a phase's time limit (A: allowed + 20 %, B: twice the allowed, unless set) or
/// five minutes in an obstacle, eliminated (the rules; Rosson did not apply them).
#[test]
fn marathon_time_limits() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let ge = format!("{parent}_ge");
    let riders: Vec<u64> = call("start_list", json!({ "class": ge })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| l["entry"].as_u64().unwrap()).collect();
    let (a, b, c, d) = (riders[0], riders[1], riders[2], riders[3]);
    // A allowed 30:00 → limit 36:00; B 50:00 → 100:00.
    call("set_class_details", json!({ "class": ge, "driving": { "a_allowed": "1800", "b_allowed": "3000", "factor": "0.25", "obstacles": 2 } })).unwrap();
    let result = |e: u64, a_finish: &str, b_finish: &str, h1: &str| call("edit_result", json!({ "class": ge, "entry": e, "values": { "a_start": "10:00", "a_finish": a_finish, "b_start": "11:00", "b_finish": b_finish, "h1_time": h1 } })).unwrap();
    result(a, "10:36:01", "11:50", "60"); // A 36:01: over the limit
    result(b, "10:36", "12:40:01", "60"); // B 100:01: over
    result(c, "10:30", "11:50", "300.5"); // an obstacle over five minutes
    result(d, "10:36", "12:40", "300"); // on every limit: in
    let ranking = call("ranking", json!({ "class": ge })).unwrap();
    let rank = |e: u64| ranking["lines"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some(e.to_string().as_str())).unwrap()["rank"].clone();
    assert_eq!((rank(a), rank(b), rank(c), rank(d)), (Value::Null, Value::Null, Value::Null, json!(1)));
    let results = call("results", json!({ "class": ge })).unwrap();
    let over = |e: u64| results["lines"].as_array().unwrap().iter().find(|l| l["entry"] == e).unwrap()["over_maximum"].clone();
    assert_eq!((over(a), over(d)), (json!(true), json!(false)));

    // A limit set by hand: A 37:00, a is in again.
    call("set_class_details", json!({ "class": ge, "driving": { "a_allowed": "1800", "b_allowed": "3000", "a_maximum": "2220", "factor": "0.25", "obstacles": 2 } })).unwrap();
    let ranking = call("ranking", json!({ "class": ge })).unwrap();
    assert!(ranking["lines"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some(a.to_string().as_str())).unwrap()["rank"].is_number());

    // Kept in Rosson's format (its `max_zeit`).
    let path = dir.path().join("back.txt");
    call("export_rosson", json!({ "path": path, "private": false })).unwrap();
    let events = office.call("rosson_events", &json!({ "path": path })).unwrap();
    let back = office.call("import_rosson", &json!({ "path": path, "event": events[0]["id"], "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    let details = office.call("class_details", &json!({ "show": back, "class": ge })).unwrap();
    assert_eq!((details["driving"]["a_maximum"].clone(), details["driving"]["b_maximum"].clone()), (json!("2220"), json!("6000")));
}

/// Team cones (OKV Reglement Fahrcup): teams of three, two rounds each, 5 of 6 rounds counted;
/// equal points for the win shared (a jump-off decides); a driver out credited 125 % of the
/// worst first round; rank points 12, 10, 9 …; the class's overall is the team ranking.
#[test]
fn team_cones() {
    let dir = tempfile::tempdir().unwrap();
    let file = common::rosson_file_edited(dir.path(), "2026.8352", |data| {
        let published = 108211;
        for a in data["ausschreibung_pruefungen"].as_array_mut().unwrap() {
            if a["id"] == published {
                a["disziplin_id"] = json!(3);
                a["fahren_zwischenpruefungen"] = json!(["hindernis"]);
                a["fahren_lizenzen"] = json!(["FB"]);
                a["equipe_anzahl_nennungen"] = json!(3);
            }
        }
        let entries: Vec<u64> = data["nennungen"].as_array().unwrap().iter().filter(|n| n["ausschreibung_pruefung_id"] == published).map(|n| n["id"].as_u64().unwrap()).take(9).collect();
        let mut teams = Vec::new();
        for (t, members) in entries.chunks(3).enumerate() {
            let id = 990_001 + t as u64;
            teams.push(json!({ "id": id, "name": format!("Équipe {}", t + 1), "veranstaltung_id": 59010, "ausschreibung_pruefung_id": published, "reihenfolge": t + 1, "startnummer": t + 1 }));
            for n in data["nennungen"].as_array_mut().unwrap() {
                if members.contains(&n["id"].as_u64().unwrap()) {
                    n["equipe_id"] = json!(id);
                    n["lizenzen"] = json!("FB");
                    n["fahren_gespann"] = json!("1");
                }
            }
        }
        data["equipen"].as_array_mut().unwrap().extend(teams);
    });
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
    let show = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let hi = format!("{parent}_hi");
    call("set_class_details", json!({ "class": hi, "driving": { "cones": "a", "rounds": 2, "allowed": { "1:FB": "120", "2:FB": "115" } } })).unwrap();
    let rows = call("participants", json!({})).unwrap();
    let team = |name: &str| -> Vec<u64> { rows.as_array().unwrap().iter().filter(|r| r["team"] == name).map(|r| r["id"].as_u64().unwrap()).collect() };
    let (t1, t2, t3) = (team("Équipe 1"), team("Équipe 2"), team("Équipe 3"));
    assert_eq!((t1.len(), t2.len(), t3.len()), (3, 3, 3));
    let drive = |e: u64, r1: &str, r2: &str| call("edit_result", json!({ "class": hi, "entry": e, "values": { "zeit_1": "100", "hindernis_1": r1, "zeit_2": "100", "hindernis_2": r2 } })).unwrap();
    // Équipe 1: 0 3 | 0 0 | 5 0 → the 5 struck: 3. Équipe 2: 0 3 | 3 0 | 0 0 → a 3 struck: 3.
    for (e, r) in t1.iter().zip([("0", "3"), ("0", "0"), ("5", "0")]) {
        drive(*e, r.0, r.1);
    }
    for (e, r) in t2.iter().zip([("0", "3"), ("3", "0"), ("0", "0")]) {
        drive(*e, r.0, r.1);
    }
    // Équipe 3: 5 0 | 0 0, the third driver out before driving: 125 % of the worst first round
    // (5) twice, 6.25 + 6.25, one struck: 5 + 6.25 = 11.25.
    drive(t3[0], "5", "0");
    drive(t3[1], "0", "0");
    let ranking = call("ranking", json!({ "class": hi })).unwrap();
    let line = |r: &Value, id: &str| r["lines"].as_array().unwrap().iter().find(|l| l["team"] == true && l["id"] == id).cloned();
    assert!(line(&ranking, "990003").is_none(), "Équipe 3 still has a driver to come");
    let reason = call("results", json!({ "class": hi })).unwrap()["reasons"][0]["code"].clone();
    call("edit_result", json!({ "class": hi, "entry": t3[2], "values": {}, "out": reason })).unwrap();
    let ranking = call("ranking", json!({ "class": hi })).unwrap();
    let summary = |id: &str| { let l = line(&ranking, id).unwrap(); (l["rank"].clone(), l["faults"].clone(), l["rank_points"].clone()) };
    assert_eq!(summary("990001"), (json!(1), json!("3"), json!(12)));
    assert_eq!(summary("990002"), (json!(1), json!("3"), json!(12)), "equal for the win: shared until the jump-off");
    assert_eq!(summary("990003"), (json!(3), json!("11.25"), json!(9)));
    // The drivers listed under their team, with its rank.
    let rider = ranking["lines"].as_array().unwrap().iter().find(|l| l["team"] == false && l["id"].as_str().and_then(|i| i.parse::<u64>().ok()) == Some(t3[0])).unwrap().clone();
    assert_eq!(rider["rank"], json!(3));

    // After the jump-off, Équipe 2 first by hand: 12 and 10 points.
    call("set_rank", json!({ "class": hi, "team": "990002", "rank": 1 })).unwrap();
    call("set_rank", json!({ "class": hi, "team": "990001", "rank": 2 })).unwrap();
    let ranking = call("ranking", json!({ "class": hi })).unwrap();
    assert_eq!((line(&ranking, "990002").unwrap()["rank_points"].clone(), line(&ranking, "990001").unwrap()["rank_points"].clone()), (json!(12), json!(10)));

    // The class's overall: the team ranking.
    let overall = call("ranking", json!({ "class": parent })).unwrap();
    assert_eq!(line(&overall, "990003").unwrap()["faults"], json!("11.25"));

    // Sent as Rosson: a row per team, then its drivers, the struck round marked.
    call("set_connection", json!({ "access_key": "KEY" })).unwrap();
    call("send", json!({ "what": "results", "class": hi })).unwrap();
    let log = call("simulation_log", json!({})).unwrap();
    let rows = log.as_array().unwrap().iter().find(|r| r["path"] == "putZwischenresultate").unwrap()["body"]["zeilen"].as_array().unwrap().clone();
    assert_eq!(rows.len(), 12, "three teams of a row and three drivers");
    assert_eq!((rows[0]["team_resultat"].clone(), rows[0]["cache_reiter_name"].clone(), rows[0]["rang"].clone()), (json!(true), json!("Équipe 2"), json!(1)));
    assert_eq!((rows[1]["team_mitglied"].clone(), rows[1]["rang"].clone()), (json!(true), json!(1)));
    let team3: Vec<&Value> = rows.iter().filter(|r| r["team_id"] == 990003 && r["team_mitglied"] == true).collect();
    let mut layouts: Vec<&str> = team3.iter().map(|r| r["resultat_layout"].as_str().unwrap()).collect();
    layouts.sort();
    assert_eq!(layouts, ["P: 0.00", "P: 5.00", "P: 6.25"], "the third driver's credited rounds, one struck");
    call("send", json!({ "what": "results", "class": parent })).unwrap();
    let log = call("simulation_log", json!({})).unwrap();
    let put = log.as_array().unwrap().iter().find(|r| r["path"] == "putResultate").unwrap()["body"]["resultate"].clone();
    // The class's results: the teams as ranked after the jump-off.
    let team_rank = |id: u64| put.as_array().unwrap().iter().find(|r| r["team_id"] == id && r["team_resultat"] == true).unwrap()["rang"].clone();
    assert_eq!((team_rank(990002), team_rank(990001), team_rank(990003)), (json!(1), json!(2), json!(3)));
}

/// Driving's start lists as Rosson prints them: the carriage number, the F licences and the
/// turnout, the phase's horses with their signalement; the marathon's phase B start (A's start,
/// its time limit and the transfer, to the minute); the carriage width on the jury's list.
#[test]
fn driving_start_lists_printed() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let ge = format!("{parent}_ge");
    // A allowed 25:00 → limit 30:00, transfer 10 min: B 40 min after A.
    call("set_class_details", json!({ "class": ge, "driving": { "a_allowed": "1500", "b_allowed": "3000", "transfer": "10", "obstacles": 5 } })).unwrap();
    call("set_layout", json!({ "class": ge, "setup": { "layout": { "StartTimes": { "first": 540, "minutes": 6, "breaks": [], "move_up": false } }, "line_after": null } })).unwrap();
    call("number", json!({ "class": ge, "first": 1 })).unwrap();
    let state = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap().state().clone();
    let id = show::model::ClassId(ge.clone());
    let print = office::prints::start_lists_as(&state, &[id.clone()], office::prints::Language::Fr, office::prints::StartListKind::Full { pedigree: false }, "now").unwrap();
    let first = &print.data["classes"][0]["lines"].as_array().unwrap().iter().find(|l| l["kind"] == "starter").unwrap().clone();
    assert_eq!((first["time"].as_str(), first["time_b"].as_str(), first["licence"].as_str()), (Some("09:00"), Some("09:40 (B)"), Some("FB / 2-Attel")));
    assert!(first["horse_sub"].as_str().unwrap().contains('('), "the horse with its signalement: {}", first["horse_sub"]);
    let text = typeset::text(print.template, &print.data).unwrap().join("\n").replace('\u{2011}', "-");
    assert!(text.contains("09:40 (B)") && text.contains("FB / 2-Attel") && text.contains("Terrain – SP/CD"), "{text}");
    assert!(!text.contains("Largeur"), "the carriage width on the jury's list only");
    let jury = office::prints::start_lists_as(&state, &[id], office::prints::Language::De, office::prints::StartListKind::Judges, "now").unwrap();
    let jury_text = typeset::text(jury.template, &jury.data).unwrap().join("\n");
    assert!(jury_text.to_lowercase().contains("wagenbreite"), "{}", &jury_text[..jury_text.len().min(900)]);
}

/// Driving's dressage judges' sheet: no correction or coefficient columns, the carriage number
/// and the driver (Rosson's).
#[test]
fn driving_judge_sheets() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let cd = format!("{parent}_cd");
    let test = call("programmes", json!({ "class": cd })).unwrap().as_array().unwrap().iter().find(|p| p["name"] == "GA06/60").unwrap()["id"].clone();
    call("edit_class", json!({ "class": cd, "programme": test })).unwrap();
    let judge = call("find_officials", json!({ "query": "" })).unwrap().as_array().unwrap().iter().find(|o| !o["titles"].as_array().unwrap().is_empty()).unwrap()["person"].clone();
    call("set_role", json!({ "class": cd, "person": judge, "role": "DRI", "letter": "C" })).unwrap();
    let state = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap().state().clone();
    let print = office::prints::judge_sheets(&state, &show::model::ClassId(cd), None, false, office::prints::Language::Fr, "now").unwrap();
    let text = typeset::text(print.template, &print.data).unwrap().join("\n");
    assert!(text.contains("N° de voiture") && text.contains("Meneur/euse"), "{}", &text[..text.len().min(600)]);
    assert!(!text.contains("Corr.") && !text.contains("Coeff."), "no correction nor coefficient");
}

/// Driving's control sheets (Rosson's Kontrollblatt): cones with each round's gates and sums,
/// the driver's allowed time filled in; the marathon's obstacle sheets, one per obstacle and
/// starter with the obstacle's number; the derby's round and obstacle sheet; in Italian the
/// rules' penalties in German.
#[test]
fn driving_control_sheets() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show_with(dir.path(), &["dressur", "gelaende", "hindernis", "derby"]);
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let (ge, hi, de) = (format!("{parent}_ge"), format!("{parent}_hi"), format!("{parent}_de"));
    call("set_class_details", json!({ "class": hi, "driving": { "cones": "a", "rounds": 2, "allowed": { "1:FB": "120", "2:FB": "115" }, "bonus": { "2": "2" } } })).unwrap();
    call("set_class_details", json!({ "class": ge, "driving": { "a_allowed": "1800", "b_allowed": "3000", "obstacles": 3 } })).unwrap();
    let state = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap().state().clone();
    let print = |class: &str, lang| office::prints::control_sheets(&state, &show::model::ClassId(class.into()), lang, "now").unwrap();
    let starters = 10;

    let cones = print(&hi, office::prints::Language::Fr);
    let sheets = cones.data["sheets"].as_array().unwrap();
    assert_eq!(sheets.len(), starters);
    assert_eq!((sheets[0]["rounds"][0]["allowed"].as_str(), sheets[0]["rounds"][1]["allowed"].as_str()), (Some("122"), Some("117")), "the allowed time with the pair's 2 s");
    let text = typeset::text(cones.template, &cones.data).unwrap().join("\n");
    assert!(text.contains("Écartement des cônes") && text.contains("2e manche") && text.contains("20"), "{}", &text[..text.len().min(800)]);

    let marathon = print(&ge, office::prints::Language::De);
    let sheets = marathon.data["sheets"].as_array().unwrap();
    assert_eq!(sheets.len(), 3 * starters, "a sheet per obstacle and starter");
    assert_eq!((sheets[0]["obstacle"].clone(), sheets[starters]["obstacle"].clone()), (json!(1), json!(2)));
    let text = typeset::text(marathon.template, &marathon.data).unwrap().join("\n");
    assert!(text.contains("Fall eines abwerfbaren Elementes") && text.contains("Total Strafpunkte"), "{}", &text[..text.len().min(800)]);

    let derby = print(&de, office::prints::Language::It);
    let text = typeset::text(derby.template, &derby.data).unwrap().join("\n");
    assert!(text.contains("Abwerfen eines Balles") && text.to_lowercase().contains("derby – foglio di controllo"), "the rules in German, ours in Italian: {}", &text[..text.len().min(800)]);
    let missing: Vec<_> = office::italian::missing();
    assert!(missing.is_empty(), "French without Italian: {missing:?}");
    assert!(office::prints::control_sheets(&state, &show::model::ClassId(format!("{parent}_cd")), office::prints::Language::Fr, "now").is_err(), "not for dressage");
    // To look at them: EQUIEVENTS_PRINTS=<folder>.
    if let Ok(folder) = std::env::var("EQUIEVENTS_PRINTS") {
        for (name, p) in [("cones", &cones), ("marathon", &marathon), ("derby", &derby)] {
            std::fs::write(format!("{folder}/{name}.pdf"), typeset::render(p.template, &p.data).unwrap().bytes).unwrap();
        }
    }
}

/// Driving's result lists: each phase its own columns — dressage each judge's total with its
/// rank and the mean on 160; the marathon's phases and obstacles; the cones' rounds.
#[test]
fn driving_result_lists() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let (cd, ge, hi) = (format!("{parent}_cd"), format!("{parent}_ge"), format!("{parent}_hi"));
    let riders: Vec<u64> = call("start_list", json!({ "class": cd })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| l["entry"].as_u64().unwrap()).collect();
    let (a, b) = (riders[0], riders[1]);
    // Dressage: judges C and R; a 7 and 6 → 48 and 64, mean 56; b 6 and 6 → 64, 64.
    let test = call("programmes", json!({ "class": cd })).unwrap().as_array().unwrap().iter().find(|p| p["name"] == "GA06/60").unwrap()["id"].clone();
    call("edit_class", json!({ "class": cd, "programme": test })).unwrap();
    let judges: Vec<u64> = call("find_officials", json!({ "query": "" })).unwrap().as_array().unwrap().iter().filter(|o| !o["titles"].as_array().unwrap().is_empty()).take(2).map(|o| o["person"].as_u64().unwrap()).collect();
    for (judge, letter) in judges.iter().zip(["C", "B"]) {
        call("set_role", json!({ "class": cd, "person": judge, "role": "DRI", "letter": letter })).unwrap();
    }
    let sheets = call("sheets", json!({ "class": cd })).unwrap();
    let movements: Vec<String> = sheets["programmes"].as_object().unwrap().values().next().unwrap()["sections"].as_array().unwrap().iter().flat_map(|s| s["movements"].as_array().unwrap().clone()).map(|m| m["number"].as_str().unwrap().to_string()).collect();
    for (entry, marks) in [(a, ["7", "6"]), (b, ["6", "6"])] {
        for (j, mark) in sheets["judges"].as_array().unwrap().iter().zip(marks) {
            let all: serde_json::Map<String, Value> = movements.iter().map(|m| (m.clone(), json!(mark))).collect();
            call("edit_sheet", json!({ "class": cd, "entry": entry, "judge": j["assignment"], "marks": all })).unwrap();
        }
    }
    // Marathon: A over by 10.5 s → 2.75; obstacle 1 95.3 s → 23.83. Cones (a full event: the
    // overtime to the hundredth): 124.2 s, 4.2 s over → 2.1, and 3 faults: 5.1.
    call("set_class_details", json!({ "class": ge, "driving": { "a_allowed": "1800", "b_allowed": "3000", "factor": "0.25", "obstacles": 2 } })).unwrap();
    call("edit_result", json!({ "class": ge, "entry": a, "values": { "a_start": "10:00", "a_finish": "10:30:10.5", "b_start": "11:00", "b_finish": "11:50", "h1_time": "95.3" } })).unwrap();
    call("set_class_details", json!({ "class": hi, "driving": { "cones": "a", "rounds": 1, "allowed": { "1:FB": "120" } } })).unwrap();
    call("edit_result", json!({ "class": hi, "entry": a, "values": { "zeit_1": "124.2", "hindernis_1": "3" } })).unwrap();
    let state = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap().state().clone();
    let print = |class: &str| office::prints::rankings(&state, &[show::model::ClassId(class.into())], office::prints::Language::Fr, "now").unwrap();
    let first = |p: &office::prints::Print| (p.data["classes"][0]["columns"].clone(), p.data["classes"][0]["lines"][0]["cells"].clone());
    let (columns, cells) = first(&print(&cd));
    assert_eq!(columns, json!(["C", "B", "Total"]));
    assert_eq!(cells, json!(["48.00 (1)", "64.00 (1)", "56.00"]), "each judge's total with its rank, the mean");
    let (columns, cells) = first(&print(&ge));
    assert_eq!(columns, json!(["A", "B", "Obst. 1", "Obst. 2", "Total"]));
    assert_eq!(cells, json!(["2.75", "0.00", "23.83", "0.00", "26.58"]));
    let (_, cells) = first(&print(&hi));
    assert_eq!(cells, json!(["5.10", "124.20", "124.20", "5.10"]));
    let p = print(&ge);
    typeset::text(p.template, &p.data).unwrap();
}

/// A presentation judged standing (Fahrreglement 6.2.1, 13.10 §2; docs/deviations.md S44): a phase
/// added to the class, each judge's overall mark, 10 − their mean, in the overall ranking.
#[test]
fn a_presentation_judged_on_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    call("add_presentation", json!({ "class": parent })).unwrap();
    assert!(call("add_presentation", json!({ "class": parent })).unwrap_err().contains("has a presentation"));
    let pr = format!("{parent}_pr");
    let kinds: Vec<String> = call("overall", json!({ "class": parent })).unwrap()["phases"].as_array().unwrap().iter().map(|p| p["kind"].as_str().unwrap().to_string()).collect();
    assert_eq!(kinds[0], "presentation", "the first phase, Prüfung A I: {kinds:?}");
    let riders: Vec<u64> = call("start_list", json!({ "class": pr })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| l["entry"].as_u64().unwrap()).collect();
    assert!(!riders.is_empty());
    let a = riders[0];
    // Two judges: 8 and 7.5, mean 7.75 → 2.25 penalties.
    assert!(call("edit_result", json!({ "class": pr, "entry": a, "values": { "note_1": "10.5" } })).unwrap_err().contains("not a mark"));
    call("edit_result", json!({ "class": pr, "entry": a, "values": { "note_1": "8", "note_2": "7.5" } })).unwrap();
    let ranking = call("ranking", json!({ "class": pr })).unwrap();
    assert_eq!(ranking["lines"][0]["faults"], json!("2.25"));
    let overall = call("ranking", json!({ "class": parent })).unwrap();
    let line = overall["lines"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some(a.to_string().as_str())).unwrap().clone();
    assert_eq!(line["faults"], json!("2.25"));
    // Rosson has no presentation: its file leaves it out, the other phases stay.
    let path = dir.path().join("back.txt");
    call("export_rosson", json!({ "path": path, "private": false })).unwrap();
    let events = office.call("rosson_events", &json!({ "path": path })).unwrap();
    let back = office.call("import_rosson", &json!({ "path": path, "event": events[0]["id"], "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    let kinds: Vec<String> = office.call("overall", &json!({ "show": back, "class": parent })).unwrap()["phases"].as_array().unwrap().iter().map(|p| p["kind"].as_str().unwrap().to_string()).collect();
    assert!(!kinds.contains(&"presentation".to_string()) && !kinds.is_empty(), "{kinds:?}");
}

/// Cones in Wertung B: the faults are seconds, added to the time (Fahrreglement 8.4.6, 8.4.7):
/// the fields say so; no penalty for overtime. A full event's cones are always Wertung A.
#[test]
fn cones_by_time_take_faults_in_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show_with(dir.path(), &["dressur", "hindernis"]);
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let hi = format!("{parent}_hi");
    call("set_class_details", json!({ "class": hi, "driving": { "cones": "b", "rounds": 1, "allowed": { "1:FB": "100", "1:FL": "100" } } })).unwrap();
    assert_eq!(call("results", json!({ "class": hi })).unwrap()["faults_in_seconds"], true);
    let riders: Vec<u64> = call("start_list", json!({ "class": hi })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| l["entry"].as_u64().unwrap()).collect();
    // Two balls, 10 s, on 112.5 s (over the allowed 100, no overtime penalty): 122.5.
    call("edit_result", json!({ "class": hi, "entry": riders[0], "values": { "zeit_1": "112.5", "hindernis_1": "10" } })).unwrap();
    assert_eq!(call("ranking", json!({ "class": hi })).unwrap()["lines"][0]["faults"], json!("122.5"));

    // A full event: Wertung A whatever is set, the faults in points.
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = driving_show(dir.path());
    let parent = office.call("set_up_class", &json!({ "show": show, "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let hi = format!("{parent}_hi");
    office.call("set_class_details", &json!({ "show": show, "class": hi, "driving": { "cones": "b", "rounds": 1 } })).unwrap();
    assert_eq!(office.call("results", &json!({ "show": show, "class": hi })).unwrap()["faults_in_seconds"], false);
}
