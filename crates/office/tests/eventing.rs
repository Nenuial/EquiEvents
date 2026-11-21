//! Eventing (Concours Complet), on a corpus class made an eventing class: its phases set up,
//! each entered as its own class, the overall ranking from them.

use office::Office;
use serde_json::{json, Value};

mod common;

/// Show 2026.8352 with its class 108211 made an eventing class (dressage, jumping, cross).
fn eventing_show(dir: &std::path::Path) -> (Office, String) {
    eventing_show_with(dir, &["dressur", "springen", "cross"])
}

/// … made an eventing class of these phases.
fn eventing_show_with(dir: &std::path::Path, phases: &[&str]) -> (Office, String) {
    let file = common::rosson_file_edited(dir, "2026.8352", |data| {
        for a in data["ausschreibung_pruefungen"].as_array_mut().unwrap() {
            if a["id"] == 108211 {
                a["disziplin_id"] = json!(4);
                a["kombipruefung_disziplinen"] = json!(phases);
            }
        }
    });
    let office = Office::new(&dir.join("library"), "test").unwrap();
    let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
    let show = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    (office, show)
}

#[test]
fn eventing_phases_entered_and_ranked_overall() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = eventing_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let (cd, cross, cs) = (format!("{parent}_cd"), format!("{parent}_cross"), format!("{parent}_cs"));
    // The phases, in Rosson's order, each a class of its own with every entry, in the same order.
    let classes: Vec<Value> = call("show", json!({})).unwrap()["days"].as_array().unwrap().iter().flat_map(|d| d["classes"].as_array().unwrap().clone()).collect();
    let discipline = |id: &str| classes.iter().find(|c| c["id"] == id).unwrap()["discipline"].clone();
    assert_eq!((discipline(&parent), discipline(&cd), discipline(&cross), discipline(&cs)), (json!("CC"), json!("CD"), json!("CROSS"), json!("CS")));
    let starters = |class: &str| -> Vec<u64> { call("start_list", json!({ "class": class })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| l["entry"].as_u64().unwrap()).collect() };
    assert_eq!(starters(&cd), starters(&cs));
    assert_eq!(starters(&cd), starters(&cross));
    let riders = starters(&cd);
    let (a, b, c) = (riders[0], riders[1], riders[2]);

    // Dressage: two judges, every movement the same mark: 70 %, 65 %, 60 % (30, 35, 40).
    let ga06 = call("programmes", json!({ "class": cd })).unwrap().as_array().unwrap().iter().find(|p| p["name"] == "GA06/60").unwrap()["id"].clone();
    call("edit_class", json!({ "class": cd, "programme": ga06 })).unwrap();
    let judges: Vec<u64> = call("find_officials", json!({ "query": "" })).unwrap().as_array().unwrap().iter().filter(|o| !o["titles"].as_array().unwrap().is_empty()).take(2).map(|o| o["person"].as_u64().unwrap()).collect();
    for (judge, letter) in judges.iter().zip(["C", "M"]) {
        call("set_role", json!({ "class": cd, "person": judge, "role": "DRI", "letter": letter })).unwrap();
    }
    let sheets = call("sheets", json!({ "class": cd })).unwrap();
    let movements: Vec<String> = sheets["programmes"].as_object().unwrap().values().next().unwrap()["sections"].as_array().unwrap().iter().flat_map(|s| s["movements"].as_array().unwrap().clone()).map(|m| m["number"].as_str().unwrap().to_string()).collect();
    for (entry, mark) in [(a, "7"), (b, "6.5"), (c, "6")] {
        for j in sheets["judges"].as_array().unwrap() {
            let marks: serde_json::Map<String, Value> = movements.iter().map(|m| (m.clone(), json!(mark))).collect();
            call("edit_sheet", json!({ "class": cd, "entry": entry, "judge": j["assignment"], "marks": marks })).unwrap();
        }
    }

    // Cross: ideal time 5:00, time over it counts. a 5:12 → 4.8; b 20 penalties, early → 20; c → 0.
    assert!(call("ranking", json!({ "class": cross })).is_err(), "no ideal time yet");
    call("set_class_details", json!({ "class": cross, "ideal_time": "5:00", "ideal_time_penalties": true })).unwrap();
    for (entry, points, time) in [(a, "0", "5:12"), (b, "20", "4:55"), (c, "0", "5:00")] {
        call("edit_result", json!({ "class": cross, "entry": entry, "values": { "strafpunkte_initial_1": points, "zeit_initial_1": time } })).unwrap();
    }
    let ranking = call("ranking", json!({ "class": cross })).unwrap();
    let faults = |r: &Value, e: u64| r["lines"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some(e.to_string().as_str())).unwrap()["faults"].as_str().unwrap().to_string();
    assert_eq!((faults(&ranking, a), faults(&ranking, b), faults(&ranking, c)), ("4.8".into(), "20".into(), "0".into()));

    // Jumping: maximum time 1:20, counted. a 4 + 5 s over (2.0) → 6; b clear; c 8.
    call("set_class_details", json!({ "class": cs, "max_time": "1:20", "max_time_penalties": true })).unwrap();
    for (entry, points, time) in [(a, "4", "85"), (b, "0", "70"), (c, "8", "75")] {
        call("edit_result", json!({ "class": cs, "entry": entry, "values": { "strafpunkte_initial_1": points, "zeit_initial_1": time } })).unwrap();
    }

    // Overall: a 30 + 4.8 + 6 = 40.8; c 40 + 0 + 8 = 48; b 35 + 20 + 0 = 55.
    let overall = call("ranking", json!({ "class": parent })).unwrap();
    let lines: Vec<(String, Value, Value)> = overall["lines"].as_array().unwrap().iter().map(|l| (l["id"].as_str().unwrap().to_string(), l["rank"].clone(), l["faults"].clone())).collect();
    assert_eq!(lines[..3], [(a.to_string(), json!(1), json!("40.80")), (c.to_string(), json!(2), json!("48.00")), (b.to_string(), json!(3), json!("55.00"))]);

    // The jury's 25 penalties for a in dressage (a yellow card): 30 + 25 = 55 there, 65.8 in all;
    // taken back, as before. Not for the cross: it carries them in its own points (S41).
    call("set_jury_penalty", json!({ "class": cd, "entry": a, "points": "25" })).unwrap();
    let overall = call("ranking", json!({ "class": parent })).unwrap();
    let order: Vec<(String, Value)> = overall["lines"].as_array().unwrap().iter().take(3).map(|l| (l["id"].as_str().unwrap().to_string(), l["faults"].clone())).collect();
    assert_eq!(order, [(c.to_string(), json!("48.00")), (b.to_string(), json!("55.00")), (a.to_string(), json!("65.80"))]);
    let cell = call("overall", json!({ "class": parent })).unwrap()["lines"].as_array().unwrap().iter().find(|l| l["entry"] == a).unwrap()["cells"][0].clone();
    assert_eq!((cell["value"].as_str(), cell["jury"].as_str()), (Some("55.00"), Some("25")));
    assert!(call("set_jury_penalty", json!({ "class": cross, "entry": a, "points": "25" })).unwrap_err().contains("not an eventing dressage or jumping phase"));
    call("set_jury_penalty", json!({ "class": cd, "entry": a, "points": null })).unwrap();
    assert_eq!(call("ranking", json!({ "class": parent })).unwrap()["lines"][0]["id"], a.to_string());

    // The overall view: a column per phase, in order (dressage, cross, jumping).
    let view = call("overall", json!({ "class": parent })).unwrap();
    let kinds: Vec<&str> = view["phases"].as_array().unwrap().iter().map(|p| p["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["dressage", "cross", "jumping"]);
    let first = &view["lines"][0];
    assert_eq!((first["entry"].as_u64(), first["cells"][0]["text"].as_str(), first["cells"][1]["text"].as_str(), first["cells"][1]["value"].as_str()), (Some(a), Some("70.00 %"), Some("5:12"), Some("4.8")));

    // One number per rider in every phase, given in the order of the one numbered.
    call("number", json!({ "class": cross, "first": 1 })).unwrap();
    let number_of = |class: &str, e: u64| call("start_list", json!({ "class": class })).unwrap()["lines"].as_array().unwrap().iter().find(|l| l["entry"] == e).unwrap()["start_number"].clone();
    assert_eq!(number_of(&cd, b), number_of(&cs, b));
    assert!(number_of(&cd, b).is_number());

    // The phases' order breaks ties: moved by drag and drop.
    call("move_phase", json!({ "phase": cs, "to": 0 })).unwrap();
    let kinds: Vec<String> = call("overall", json!({ "class": parent })).unwrap()["phases"].as_array().unwrap().iter().map(|p| p["kind"].as_str().unwrap().to_string()).collect();
    assert_eq!(kinds, ["jumping", "dressage", "cross"]);
    call("undo", json!({})).unwrap();

    // Sent to Swiss Equestrian: a start list per phase, the overall results, each phase's.
    call("set_connection", json!({ "access_key": "KEY" })).unwrap();
    call("send", json!({ "what": "start_lists", "published": 108211, "language": "de" })).unwrap();
    let posted = |path: &str| -> Value {
        let log = call("simulation_log", json!({})).unwrap();
        log.as_array().unwrap().iter().find(|r| r["path"] == path).unwrap()["body"].clone()
    };
    let lists = posted("putStartlisten")["startlisten"].clone();
    let numbers: Vec<&str> = lists.as_array().unwrap().iter().map(|l| l["nummer"].as_str().unwrap()).collect();
    assert_eq!(numbers.len(), 3, "{numbers:?}");
    assert!(numbers.contains(&"12 Dressur") && numbers.contains(&"12 Cross") && numbers.contains(&"12 Springen"), "{numbers:?}");
    call("send", json!({ "what": "results", "class": parent })).unwrap();
    let results = posted("putResultate");
    let row = results["resultate"].as_array().unwrap().iter().find(|r| r["rang"] == 1).unwrap().clone();
    assert_eq!((row["punkte_total"].as_f64(), row["punkte_dressur"].as_f64(), row["strafpunkte_cross_hindernis"].as_f64(), row["strafpunkte_springen_hindernis"].as_f64()), (Some(40.8), Some(30.0), Some(4.8), Some(6.0)));
    assert_eq!(results["pruefungen"][0]["anzahl_gemeldete_reiter"], 10);
    call("send", json!({ "what": "results", "class": cross })).unwrap();
    let partial = posted("putZwischenresultate");
    assert_eq!((partial["kopf"]["code"].as_str(), partial["pruefung"]["nummer"].as_str()), (Some("cross"), Some("12")));
    let first = partial["zeilen"].as_array().unwrap().iter().find(|z| z["rang"] == 1).unwrap().clone();
    assert_eq!(first["resultat_layout"].as_str(), Some("P: 0.00, T: 5:00"));

    // To Rosson's format and back: the phases, their settings, results and judges come back.
    let path = dir.path().join("back.txt");
    call("export_rosson", json!({ "path": path, "private": false })).unwrap();
    let events = office.call("rosson_events", &json!({ "path": path })).unwrap();
    let back = office.call("import_rosson", &json!({ "path": path, "event": events[0]["id"], "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    let summary = |show: &str| -> Vec<(u64, Value, Value)> {
        let v = office.call("overall", &json!({ "show": show, "class": parent })).unwrap();
        v["lines"].as_array().unwrap().iter().map(|l| (l["entry"].as_u64().unwrap(), l["rank"].clone(), l["total"].clone())).collect()
    };
    assert_eq!(summary(&back), summary(&show));
    let judges = office.call("sheets", &json!({ "show": back, "class": cd })).unwrap()["judges"].as_array().unwrap().len();
    assert_eq!(judges, 2);
    let details = office.call("class_details", &json!({ "show": back, "class": cs })).unwrap();
    assert_eq!((details["max_time"].as_str(), details["max_time_penalties"].clone()), (Some("1:20"), json!(true)));

    // A CCN ranks on penalties, by the rules, whatever the option says (Eventing-Reglement 5.9.2):
    // still a, c, b — the office has no choice (not offered for eventing).
    assert_eq!(call("class_details", json!({ "class": parent })).unwrap()["eventing"], true);
    call("set_class_details", json!({ "class": parent, "rank_points": true })).unwrap();
    let overall = call("ranking", json!({ "class": parent })).unwrap();
    let order: Vec<String> = overall["lines"].as_array().unwrap().iter().take(3).map(|l| l["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(order, [a.to_string(), c.to_string(), b.to_string()]);
    call("set_class_details", json!({ "class": parent, "rank_points": false })).unwrap();

    // Equal totals: the cross decides first (5.9.2). b's jumping 0 → 15 faults: b 35 + 20 + 15 =
    // 70; c's dressage stays 40, cross 0, jumping 8 → 30: c 40 + 0 + 30 = 70. Equal: c, better
    // in the cross (0 against 20), ahead.
    call("edit_result", json!({ "class": cs, "entry": b, "values": { "strafpunkte_initial_1": "15", "zeit_initial_1": "70" } })).unwrap();
    call("edit_result", json!({ "class": cs, "entry": c, "values": { "strafpunkte_initial_1": "30", "zeit_initial_1": "75" } })).unwrap();
    let overall = call("ranking", json!({ "class": parent })).unwrap();
    let lines: Vec<(String, Value, Value)> = overall["lines"].as_array().unwrap().iter().take(3).map(|l| (l["id"].as_str().unwrap().to_string(), l["rank"].clone(), l["faults"].clone())).collect();
    assert_eq!(lines, [(a.to_string(), json!(1), json!("40.80")), (c.to_string(), json!(2), json!("70.00")), (b.to_string(), json!(3), json!("70.00"))]);

    // The cross over its maximum time, twice the ideal time: eliminated (8.3.5).
    let d = riders[3];
    call("edit_result", json!({ "class": cross, "entry": d, "values": { "strafpunkte_initial_1": "0", "zeit_initial_1": "10:01" } })).unwrap();
    let ranking = call("ranking", json!({ "class": cross })).unwrap();
    let line = ranking["lines"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some(d.to_string().as_str())).unwrap().clone();
    assert_eq!(line["rank"], Value::Null, "{line}");

    // A closed phase without a result: out of the overall ranking (E10). e has a cross result
    // only; while the jumping runs, it is ranked; the jumping closed, out.
    let e = riders[4];
    call("edit_result", json!({ "class": cross, "entry": e, "values": { "strafpunkte_initial_1": "0", "zeit_initial_1": "5:00" } })).unwrap();
    let rank_of = |who: u64| call("ranking", json!({ "class": parent })).unwrap()["lines"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some(who.to_string().as_str())).map(|l| l["rank"].clone());
    assert!(rank_of(e).is_some_and(|r| r.is_number()));
    call("finalise", json!({ "class": cs })).unwrap();
    assert_eq!(rank_of(e), Some(Value::Null));
    call("reopen", json!({ "class": cs, "reason": "test" })).unwrap();

    // Out of the cross: out of the overall ranking.
    let reason = call("results", json!({ "class": cross })).unwrap()["reasons"][0]["code"].clone();
    call("edit_result", json!({ "class": cross, "entry": a, "values": {}, "out": reason })).unwrap();
    let overall = call("ranking", json!({ "class": parent })).unwrap();
    let line = overall["lines"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some(a.to_string().as_str())).unwrap().clone();
    assert_eq!(line["rank"], Value::Null);
    let place = |who: u64| overall["lines"].as_array().unwrap().iter().position(|l| l["id"].as_str() == Some(who.to_string().as_str())).unwrap();
    assert!(place(c) < place(b));
}

#[test]
fn eventing_overall_printed_in_three_languages() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = eventing_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let cross = format!("{parent}_cross");
    call("set_class_details", json!({ "class": cross, "ideal_time": "5:00", "ideal_time_penalties": true })).unwrap();
    let entry = call("start_list", json!({ "class": cross })).unwrap()["lines"].as_array().unwrap().iter().find(|l| l["kind"] == "starter").unwrap()["entry"].clone();
    call("edit_result", json!({ "class": cross, "entry": entry, "values": { "strafpunkte_initial_1": "0", "zeit_initial_1": "5:12" } })).unwrap();
    let state = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap().state().clone();
    for (lang, words) in [(office::prints::Language::Fr, ["Dressage", "Cross", "Saut"]), (office::prints::Language::De, ["Dressur", "Gelände", "Springen"]), (office::prints::Language::It, ["Dressage", "Cross", "Salto"])] {
        let print = office::prints::rankings(&state, &[show::model::ClassId(parent.clone())], lang, "now").unwrap();
        let text = typeset::text(print.template, &print.data).unwrap().join("\n");
        for w in words {
            assert!(text.to_uppercase().contains(&w.to_uppercase()), "{w} not in {text}");
        }
        assert!(text.contains("5:12") && text.contains("→") && text.contains("4.80"), "{text}");
    }
    assert!(office::italian::missing().is_empty(), "{:?}", office::italian::missing());
}

/// A jumping phase on the ideal-time table (35) is scored as that table in show jumping (S20):
/// penalties, then the gap to the ideal time, over or under the same, never time penalties.
/// In the overall ranking that gap breaks a tie in the phase, as the cross's does (S24).
#[test]
fn eventing_jumping_phase_on_the_ideal_time_table() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = eventing_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let (cross, cs) = (format!("{parent}_cross"), format!("{parent}_cs"));
    let riders: Vec<u64> = call("start_list", json!({ "class": cs })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| l["entry"].as_u64().unwrap()).collect();
    let (a, b, c) = (riders[0], riders[1], riders[2]);
    call("set_scoring_table", json!({ "class": cs, "table": 35 })).unwrap();
    call("set_class_details", json!({ "class": cs, "ideal_time": "1:15" })).unwrap();
    // a 1:20 (5 s over), b 1:12 (3 s under), c 1:15 with 4 faults.
    for (entry, time) in [(a, "80"), (b, "72")] {
        call("edit_result", json!({ "class": cs, "entry": entry, "values": { "strafpunkte_initial_1": "0", "zeit_initial_1": time } })).unwrap();
    }
    call("edit_result", json!({ "class": cs, "entry": c, "values": { "strafpunkte_initial_1": "4", "zeit_initial_1": "75" } })).unwrap();
    let ranking = call("ranking", json!({ "class": cs })).unwrap();
    let line = |r: &Value, e: u64| r["lines"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some(e.to_string().as_str())).unwrap().clone();
    assert_eq!((line(&ranking, b)["rank"].clone(), line(&ranking, a)["rank"].clone(), line(&ranking, c)["rank"].clone()), (json!(1), json!(2), json!(3)));
    assert_eq!(line(&ranking, a)["faults"], json!("0"), "no time penalties on the ideal-time table");

    // The cross alike for both, the jumping phase's gap decides the overall tie: b, then a.
    call("set_class_details", json!({ "class": cross, "ideal_time": "5:00" })).unwrap();
    for entry in [a, b, c] {
        call("edit_result", json!({ "class": cross, "entry": entry, "values": { "strafpunkte_initial_1": "0", "zeit_initial_1": "5:00" } })).unwrap();
    }
    let overall = call("ranking", json!({ "class": parent })).unwrap();
    let order: Vec<String> = overall["lines"].as_array().unwrap().iter().take(3).map(|l| l["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(order, [b.to_string(), a.to_string(), c.to_string()]);
    assert_eq!((line(&overall, b)["rank"].clone(), line(&overall, a)["rank"].clone()), (json!(1), json!(2)));
}

/// A combined class, dressage and jumping (Eventing-Reglement 5.9.1): ranked by rank points,
/// always; equal, the better dressage.
#[test]
fn combined_dressage_and_jumping_by_rank_points_then_dressage() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = eventing_show_with(dir.path(), &["dressur", "springen"]);
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let parent = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let (cd, cs) = (format!("{parent}_cd"), format!("{parent}_cs"));
    let riders: Vec<u64> = call("start_list", json!({ "class": cd })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| l["entry"].as_u64().unwrap()).collect();
    let (a, b, c) = (riders[0], riders[1], riders[2]);
    // Dressage: 70 %, 65 %, 60 % — ranks 1, 2, 3.
    let ga06 = call("programmes", json!({ "class": cd })).unwrap().as_array().unwrap().iter().find(|p| p["name"] == "GA06/60").unwrap()["id"].clone();
    call("edit_class", json!({ "class": cd, "programme": ga06 })).unwrap();
    let judge = call("find_officials", json!({ "query": "" })).unwrap().as_array().unwrap().iter().find(|o| !o["titles"].as_array().unwrap().is_empty()).unwrap()["person"].clone();
    call("set_role", json!({ "class": cd, "person": judge, "role": "DRI", "letter": "C" })).unwrap();
    let sheets = call("sheets", json!({ "class": cd })).unwrap();
    let movements: Vec<String> = sheets["programmes"].as_object().unwrap().values().next().unwrap()["sections"].as_array().unwrap().iter().flat_map(|s| s["movements"].as_array().unwrap().clone()).map(|m| m["number"].as_str().unwrap().to_string()).collect();
    for (entry, mark) in [(a, "7"), (b, "6.5"), (c, "6")] {
        let marks: serde_json::Map<String, Value> = movements.iter().map(|m| (m.clone(), json!(mark))).collect();
        call("edit_sheet", json!({ "class": cd, "entry": entry, "judge": sheets["judges"][0]["assignment"], "marks": marks })).unwrap();
    }
    // Jumping: a 12, b 4, c 0 — ranks 3, 2, 1. On penalties b (39) would be first; on rank
    // points all have 4: the dressage decides, a, b, c.
    for (entry, faults) in [(a, "12"), (b, "4"), (c, "0")] {
        call("edit_result", json!({ "class": cs, "entry": entry, "values": { "strafpunkte_initial_1": faults, "zeit_initial_1": "70" } })).unwrap();
    }
    let overall = call("ranking", json!({ "class": parent })).unwrap();
    let lines: Vec<(String, Value)> = overall["lines"].as_array().unwrap().iter().take(3).map(|l| (l["id"].as_str().unwrap().to_string(), l["rank"].clone())).collect();
    assert_eq!(lines, [(a.to_string(), json!(1)), (b.to_string(), json!(2)), (c.to_string(), json!(3))]);
}
