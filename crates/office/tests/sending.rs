//! Sending to Swiss Equestrian, against the simulation: the access key kept apart from the
//! show, what was sent and what changed since, the real servers out of reach before the pilot.

use office::Office;
use serde_json::{json, Value};

mod common;

struct Show {
    office: Office,
    id: String,
    dir: tempfile::TempDir,
}

impl Show {
    fn rehearsal() -> Show {
        let dir = tempfile::tempdir().unwrap();
        let office = Office::new(&dir.path().join("library"), "test").unwrap();
        let file = common::rosson_file(dir.path(), "2026.8352");
        let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
        let id = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
        Show { office, id, dir }
    }

    fn call(&self, command: &str, mut args: Value) -> Result<Value, String> {
        args["show"] = json!(self.id);
        self.office.call(command, &args)
    }

    fn set_up(&self, number: &str) -> String {
        let show = self.call("show", json!({})).unwrap();
        let published = show["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == number).unwrap()["id"].as_u64().unwrap();
        self.call("set_up_class", json!({ "published": published })).unwrap()["class"].as_str().unwrap().to_string()
    }

    fn row(&self, class: &str) -> Value {
        self.call("sending", json!({})).unwrap()["rows"].as_array().unwrap().iter().find(|r| r["class"]["id"] == class).unwrap().clone()
    }
}

#[test]
fn sending_to_the_simulation() {
    let show = Show::rehearsal();
    let class = show.set_up("1");
    let published = show.row(&class)["class"]["published"].as_u64().unwrap();
    let entries: Vec<u64> = show.call("results", json!({ "class": class })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["not_starting"] == false).take(2).map(|l| l["entry"].as_u64().unwrap()).collect();
    for (e, time) in entries.iter().zip(["61.5", "63.2"]) {
        show.call("edit_result", json!({ "class": class, "entry": e, "values": { "strafpunkte_initial_1": "0", "zeit_initial_1": time } })).unwrap();
    }

    // An official with a role: Swiss Equestrian's id for it is asked once.
    let official = show.call("find_officials", json!({ "query": "a" })).unwrap()[0]["person"].clone();
    show.call("set_role", json!({ "class": class, "person": official, "role": "P", "letter": null })).unwrap();

    // The simulation by default; no key yet.
    let connection = show.call("connection", json!({})).unwrap();
    assert_eq!((connection["server"].clone(), connection["has_key"].clone()), (json!("simulation"), json!(false)));
    assert!(show.call("send", json!({ "what": "results", "class": class })).unwrap_err().contains("access key"));

    // The key is kept apart from the show: not in its file, not in its exports.
    let connection = show.call("set_connection", json!({ "access_key": " SECRET-KEY-123 " })).unwrap();
    assert_eq!((connection["has_key"].clone(), connection["key_end"].clone()), (json!(true), json!("123")));
    let exported = show.dir.path().join("show.equievents");
    show.call("export_show", json!({ "path": exported, "private": true })).unwrap();
    assert!(!String::from_utf8_lossy(&std::fs::read(&exported).unwrap()).contains("SECRET-KEY"));
    let secrets = show.dir.path().join("library/secrets.json");
    assert!(std::fs::read_to_string(&secrets).unwrap().contains("SECRET-KEY-123"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&secrets).unwrap().permissions().mode() & 0o777, 0o600);
    }

    // Results: sent, recorded, then changed since.
    let answer = show.call("send", json!({ "what": "results", "class": class })).unwrap();
    assert_eq!(answer["warnings"], json!([]));
    let row = show.row(&class);
    assert!(row["results_sent"]["millis"].is_number());
    assert_eq!(row["results_changed"], false);
    let log = show.call("simulation_log", json!({})).unwrap();
    let put = log.as_array().unwrap().iter().find(|r| r["path"] == "putResultate").unwrap();
    assert_eq!((put["method"].clone(), put["status"].clone()), (json!("POST"), json!(200)));
    assert_eq!(put["body"]["access_key"], "•••");
    assert_eq!(put["body"]["resultate"].as_array().unwrap().len(), 2);
    assert!(log.as_array().unwrap().iter().any(|r| r["path"] == "putVeranstaltung"));
    // The event's officials looked up once, each answered.
    let lookups: Vec<&Value> = log.as_array().unwrap().iter().filter(|r| r["path"] == "lookupZuweisung").collect();
    assert!(!lookups.is_empty());
    assert!(log.as_array().unwrap().iter().all(|r| r["status"] == 200), "{log}");

    // Looked up once: the next upload asks for none.
    show.call("send", json!({ "what": "results", "class": class })).unwrap();
    let again = show.call("simulation_log", json!({})).unwrap();
    assert_eq!(again.as_array().unwrap().iter().filter(|r| r["path"] == "lookupZuweisung").count(), lookups.len());

    // Changes are compared with the upload by the millisecond: a person never edits within the
    // millisecond of sending, a test can.
    std::thread::sleep(std::time::Duration::from_millis(5));
    show.call("edit_result", json!({ "class": class, "entry": entries[0], "values": { "zeit_initial_1": "60.9" } })).unwrap();
    assert_eq!(show.row(&class)["results_changed"], true);
    show.call("send", json!({ "what": "results", "class": class })).unwrap();
    assert_eq!(show.row(&class)["results_changed"], false);

    // Start lists, sent and withdrawn; the event.
    show.call("send", json!({ "what": "start_lists", "published": published, "language": "fr" })).unwrap();
    assert!(show.row(&class)["start_lists_sent"].is_object());
    assert_eq!(show.row(&class)["lists_here"], true);
    // Swiss Equestrian has them: the office cannot take them away here.
    assert!(show.call("remove_start_lists", json!({ "published": published })).unwrap_err().contains("withdraw"));
    show.call("send", json!({ "what": "withdraw_start_lists", "published": published })).unwrap();
    assert!(show.row(&class)["start_lists_sent"].is_null());
    show.call("remove_start_lists", json!({ "published": published })).unwrap();
    show.call("undo", json!({})).unwrap();
    show.call("send", json!({ "what": "event" })).unwrap();
    assert!(show.call("sending", json!({})).unwrap()["event_sent"].is_object());

    // A wrong key is the simulation's refusal, said as Swiss Equestrian's message; nothing recorded.
    show.call("set_connection", json!({ "access_key": null })).unwrap();
    assert_eq!(show.call("connection", json!({})).unwrap()["has_key"], false);
}

/// A show only on this computer sends to the simulation here, whatever a show's settings say:
/// only equievents.ch talks to Swiss Equestrian.
#[test]
fn a_show_here_sends_to_the_simulation() {
    let show = Show::rehearsal();
    show.call("set_connection", json!({ "server": "production", "access_key": "K" })).unwrap();
    let connection = show.call("connection", json!({})).unwrap();
    assert_eq!((connection["server"].as_str(), connection["online"].as_bool(), connection["has_key"].as_bool()), (Some("simulation"), Some(false), Some(true)));
    assert_eq!(show.call("send", json!({ "what": "event" })).unwrap()["server"], "simulation");
    assert!(!show.call("simulation_log", json!({})).unwrap().as_array().unwrap().is_empty());
}

/// A final (a ranking over several classes) sent as Rosson sends its finals: one class numbered
/// as the ranking, a row per rider and horse; recorded as sent. Over jumping and dressage:
/// Swiss Equestrian's combined discipline and special category, the ranks per class.
#[test]
fn a_final_sent_as_rosson_sends_it() {
    let dir = tempfile::tempdir().unwrap();
    let office = office::Office::new(&dir.path().join("library"), "test").unwrap();
    let file = common::rosson_file(dir.path(), "2026.8352");
    let show = office.call("import_rosson", &json!({ "path": file, "event": "59010", "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    call("set_connection", json!({ "access_key": "KEY" })).unwrap();
    let finals = call("show", json!({})).unwrap()["combined"].clone();
    let id = finals.as_array().unwrap().iter().find(|r| r["number"] == "12.1").unwrap()["id"].as_str().unwrap().to_string();
    assert_eq!(call("combined", json!({ "id": id })).unwrap()["sent"], Value::Null);
    call("send", json!({ "what": "combined", "id": id })).unwrap();
    let log = call("simulation_log", json!({})).unwrap();
    let put = log.as_array().unwrap().iter().find(|r| r["path"] == "putResultate").unwrap();
    let p = &put["body"]["pruefungen"][0];
    assert_eq!((p["nummer"].as_str(), p["disziplin_id"].as_u64(), p["ausschreibung_pruefung_id"].as_u64()), (Some("12.1"), Some(2), Some(108211)));
    let rows = put["body"]["resultate"].as_array().unwrap();
    assert_eq!(rows.len() as u64, p["anzahl_gestartete_reiter"].as_u64().unwrap());
    assert_eq!(rows[0]["rang"], 1);
    assert!(rows[0]["resultat_layout"].as_str().unwrap().contains(") / 2 = <b>"), "{}", rows[0]);
    assert!(rows[0]["punkte_prozent"].as_f64().unwrap() > 50.0 && rows[0]["punkte_total"].as_f64().unwrap() > 0.0);
    assert_eq!(rows.iter().filter(|r| r["klassiert"] == true).count() as u64, p["anzahl_klassierte_reiter"].as_u64().unwrap());
    // Riders missing a class: rank 99 (none in this final: every rider rode both).
    let view = call("combined", json!({ "id": id })).unwrap();
    let missing = view["lines"].as_array().unwrap().iter().filter(|l| l["cells"].as_array().unwrap().iter().any(|c| c["missing"] == true)).count();
    assert_eq!(rows.iter().filter(|r| r["rang"] == 99).count(), missing);
    assert!(view["sent"].is_number() && view["changed"] == false);

    // Jumping and dressage: ranks added, the combined discipline, the ranks per class. Numbered
    // after its highest class (12), ".2" as Rosson's dressage final has 12.1.
    let mixed = call("set_combined", json!({ "name": "Mixte", "classes": ["108196", "108211"], "method": "ranks", "tie_break": "108211" })).unwrap()["id"].as_str().unwrap().to_string();
    call("send", json!({ "what": "combined", "id": mixed })).unwrap();
    let log = call("simulation_log", json!({})).unwrap();
    let put = log.as_array().unwrap().iter().find(|r| r["path"] == "putResultate").unwrap();
    let p = &put["body"]["pruefungen"][0];
    assert_eq!((p["nummer"].as_str(), p["disziplin_id"].as_u64(), p["kategorie_id"].as_u64()), (Some("12.2"), Some(4), Some(462)));
    let row = &put["body"]["resultate"][0];
    assert_eq!(row["pro_disziplin"].as_array().unwrap().len(), 2);
    assert_eq!(row["pro_disziplin"][0]["pruefung_nummer"], "1");
}

/// Kür (GAK) and Children tests, from their published definitions: the parts A and B per judge
/// and for the class, sent as Rosson sends them.
#[test]
fn freestyle_and_children_parts_sent_as_rosson() {
    let show = Show::rehearsal();
    show.call("set_connection", json!({ "access_key": "KEY" })).unwrap();
    let judges: Vec<u64> = show.call("find_officials", json!({ "query": "" })).unwrap().as_array().unwrap().iter().filter(|c| !c["titles"].as_array().unwrap().is_empty()).take(3).map(|c| c["person"].as_u64().unwrap()).collect();
    // GAK: A movements 1–15 (coefficients 20 in all), B 16–20 (4 each).
    let setup = |published: u64, programme: &str| -> (String, Value) {
        let class = show.call("set_up_class", json!({ "published": published })).unwrap()["class"].as_str().unwrap().to_string();
        let id = show.call("programmes", json!({})).unwrap().as_array().unwrap().iter().find(|p| p["name"] == programme).unwrap()["id"].clone();
        show.call("edit_class", json!({ "class": class, "programme": id })).unwrap();
        for (judge, letter) in judges.iter().zip(["E", "C", "B"]) {
            show.call("set_role", json!({ "class": class, "person": judge, "role": "DRI", "letter": letter })).unwrap();
        }
        let view = show.call("sheets", json!({ "class": class })).unwrap();
        (class, view)
    };
    let mark = |class: &str, view: &Value, rider: usize, judge: usize, a: &str, b: &str| {
        let programme = view["programmes"].as_object().unwrap().values().next().unwrap();
        let mut marks = serde_json::Map::new();
        for (i, section) in programme["sections"].as_array().unwrap().iter().enumerate() {
            for m in section["movements"].as_array().unwrap() {
                marks.insert(m["number"].as_str().unwrap().to_string(), json!(if i == 0 { a } else { b }));
            }
        }
        let entry = view["riders"][rider]["entry"].clone();
        let judge = view["judges"][judge]["assignment"].clone();
        show.call("edit_sheet", json!({ "class": class, "entry": entry, "judge": judge, "marks": marks })).unwrap();
    };
    let sent = |class: &str| -> Vec<Value> {
        show.call("send", json!({ "what": "results", "class": class })).unwrap();
        let log = show.call("simulation_log", json!({})).unwrap();
        let put = log.as_array().unwrap().iter().find(|r| r["path"] == "putResultate").unwrap().clone();
        put["body"]["resultate"].as_array().unwrap().iter().filter(|r| r["punkte_prozent"] != "0.00").cloned().collect()
    };

    // Kür: every judge 7.5 for the technical marks, 8 for the artistic ones. A: 150 halved 75 of
    // 100; B: 160 halved 80 of 100; the total 155 of 200 per judge: 77.50 %.
    let (kur, view) = setup(108211, "GAK");
    assert_eq!(view["programmes"].as_object().unwrap().values().next().unwrap()["kind"], "freestyle");
    for judge in 0..3 {
        mark(&kur, &view, 0, judge, "7.5", "8");
        mark(&kur, &view, 1, judge, "6", "6.5");
    }
    let rider = show.call("sheets", json!({ "class": kur })).unwrap()["riders"][0].clone();
    assert_eq!((rider["percentage"].as_str(), rider["part_a"].as_str(), rider["part_b"].as_str()), (Some("77.50"), Some("75.00"), Some("80.00")));
    let rows = sent(&kur);
    let first = rows.iter().find(|r| r["rang"] == 1).unwrap();
    assert_eq!((first["punkte_prozent"].as_str(), first["punkte_prozent_a"].as_str(), first["punkte_prozent_b"].as_str()), (Some("77.50"), Some("75.00"), Some("80.00")));
    let judge = &first["pro_richter"][0];
    assert_eq!((judge["punkte"].as_f64(), judge["prozent"].as_str(), judge["prozent_a"].as_str(), judge["prozent_b"].as_str()), (Some(155.0), Some("77.50"), Some("75.00"), Some("80.00")));
    assert_eq!(first["pro_richter"].as_array().unwrap().len(), 3);

    // Children: one sheet for the judges (the first's), over the programme's 400. A: 7 × 20 =
    // 140 of 200; B: 8 × 20 = 160 of 200; 300 of 400: 75 %. Sent as C (A) and E (B).
    let (children, view) = setup(108212, "FEI Children Preliminary A");
    assert_eq!(view["programmes"].as_object().unwrap().values().next().unwrap()["kind"], "children");
    mark(&children, &view, 0, 0, "7", "8");
    mark(&children, &view, 1, 0, "6", "6");
    let rider = show.call("sheets", json!({ "class": children })).unwrap()["riders"][0].clone();
    assert_eq!((rider["percentage"].as_str(), rider["part_a"].as_str(), rider["part_b"].as_str()), (Some("75.00"), Some("70.00"), Some("80.00")));
    let rows = sent(&children);
    let first = rows.iter().find(|r| r["rang"] == 1).unwrap();
    assert_eq!(first["punkte_prozent"].as_str(), Some("75.00"));
    assert!(first.get("punkte_prozent_a").is_none());
    let pro: Vec<(String, String, String)> = first["pro_richter"].as_array().unwrap().iter().map(|r| (r["buchstabe"].as_str().unwrap().into(), r["prozent"].as_str().unwrap().into(), r["punkte"].as_str().unwrap().into())).collect();
    assert_eq!(pro, [("C (A)".into(), "70.00".into(), "140.0".into()), ("E (B)".into(), "80.00".into(), "160.0".into())]);
    assert_eq!(first["pro_richter"][0]["person_id"], first["pro_richter"][1]["person_id"]);
}

/// Totals per judge (Rosson's manual entry): set before any sheet; each judge's total, no marks;
/// the percentage from the totals, sent as any dressage result.
#[test]
fn totals_per_judge_entered_and_sent() {
    let show = Show::rehearsal();
    show.call("set_connection", json!({ "access_key": "KEY" })).unwrap();
    let class = show.call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let ga06 = show.call("programmes", json!({})).unwrap().as_array().unwrap().iter().find(|p| p["name"] == "GA06/60").unwrap().clone();
    show.call("edit_class", json!({ "class": class, "programme": ga06["id"] })).unwrap();
    let judges: Vec<u64> = show.call("find_officials", json!({ "query": "" })).unwrap().as_array().unwrap().iter().filter(|c| !c["titles"].as_array().unwrap().is_empty()).take(2).map(|c| c["person"].as_u64().unwrap()).collect();
    for (judge, letter) in judges.iter().zip(["C", "M"]) {
        show.call("set_role", json!({ "class": class, "person": judge, "role": "DRI", "letter": letter })).unwrap();
    }
    assert_eq!(show.call("class_details", json!({ "class": class })).unwrap()["dressage"], true);
    show.call("set_class_details", json!({ "class": class, "totals_per_judge": true })).unwrap();
    let view = show.call("sheets", json!({ "class": class })).unwrap();
    assert_eq!(view["totals_per_judge"], true);
    let max: f64 = view["programmes"].as_object().unwrap().values().next().unwrap()["max_points"].as_str().unwrap().parse().unwrap();
    let (entry, c, m) = (view["riders"][0]["entry"].clone(), view["judges"][0]["assignment"].clone(), view["judges"][1]["assignment"].clone());

    // Marks refused; a total above the maximum refused.
    assert!(show.call("edit_sheet", json!({ "class": class, "entry": entry, "judge": c, "marks": { "1": "7" } })).unwrap_err().contains("total"));
    assert!(show.call("edit_sheet", json!({ "class": class, "entry": entry, "judge": c, "marks": {}, "total": format!("{}", max + 1.0) })).is_err());
    // Two totals: 70 % and 60 % of the maximum, 65 % for the rider.
    let at = |p: f64| format!("{}", max * p);
    show.call("edit_sheet", json!({ "class": class, "entry": entry, "judge": c, "marks": {}, "total": at(0.7) })).unwrap();
    let rider = show.call("sheets", json!({ "class": class })).unwrap()["riders"][0].clone();
    assert_eq!((rider["sheets"][c.as_str().unwrap()]["complete"].clone(), rider["sheets"][c.as_str().unwrap()]["entered"].as_str().map(|t| t.parse::<f64>().unwrap())), (json!(true), Some(max * 0.7)));
    show.call("edit_sheet", json!({ "class": class, "entry": entry, "judge": m, "marks": {}, "total": at(0.6) })).unwrap();
    let rider = show.call("sheets", json!({ "class": class })).unwrap()["riders"][0].clone();
    assert_eq!(rider["percentage"].as_str(), Some("65.00"));

    // Not to be switched back once there are sheets; undo takes the total away.
    assert!(show.call("set_class_details", json!({ "class": class, "totals_per_judge": false })).unwrap_err().contains("sheets"));
    show.call("undo", json!({})).unwrap();
    assert!(show.call("sheets", json!({ "class": class })).unwrap()["riders"][0]["sheets"].get(m.as_str().unwrap()).is_none());
    show.call("edit_sheet", json!({ "class": class, "entry": entry, "judge": m, "marks": {}, "total": at(0.6) })).unwrap();

    show.call("send", json!({ "what": "results", "class": class })).unwrap();
    let log = show.call("simulation_log", json!({})).unwrap();
    let put = log.as_array().unwrap().iter().find(|r| r["path"] == "putResultate").unwrap();
    let row = put["body"]["resultate"].as_array().unwrap().iter().find(|r| r["rang"] == 1).unwrap();
    assert_eq!((row["punkte_prozent"].as_str(), row["pro_richter"].as_array().unwrap().len()), (Some("65.00"), 2));
    assert_eq!(row["punkte_total"].as_f64(), Some(max * 1.3));
}
