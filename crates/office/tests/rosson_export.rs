//! A show prepared and run in EquiEvents, exported in Rosson's format and imported back from
//! that file: the same start lists, rankings, judges' sheets, officials, prizes and details.

use office::Office;
use serde_json::{json, Value};

mod common;

struct Show {
    office: Office,
    id: String,
    dir: tempfile::TempDir,
}

impl Show {
    fn call(&self, command: &str, mut args: Value) -> Result<Value, String> {
        args["show"] = json!(self.id);
        self.office.call(command, &args)
    }

    fn set_up(&self, number: &str) -> String {
        let published = self.call("show", json!({})).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == number).unwrap()["id"].as_u64().unwrap();
        self.call("set_up_class", json!({ "published": published })).unwrap()["class"].as_str().unwrap().to_string()
    }
}

/// What a class looks like to the office: its start list, ranking and details.
fn class_view(office: &Office, show: &str, class: &str) -> Value {
    let list = office.call("start_list", &json!({ "show": show, "class": class })).unwrap();
    let lines: Vec<Value> = list["lines"].as_array().unwrap().iter().map(|l| json!([l["kind"], l["entry"], l["position"], l["start_number"], l["time"], l["name"], l["not_starting"]])).collect();
    let ranking = office.call("ranking", &json!({ "show": show, "class": class })).unwrap();
    let ranked: Vec<Value> = ranking["lines"].as_array().unwrap().iter().map(|l| json!([l["id"], l["rank"], l["placed"], l["faults"], l["time"], l["percentage"]])).collect();
    let details = office.call("class_details", &json!({ "show": show, "class": class })).unwrap();
    json!({ "lines": lines, "ranked": ranked, "placed": ranking["placed_count"], "details": details, "setup": list["setup"] })
}

#[test]
fn a_show_run_here_goes_to_rosson_and_back() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = common::rosson_file(dir.path(), "2026.8352");
    let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
    let id = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    // A late rider of Swiss Equestrian's registry, with a guest licence.
    common::registry(&dir.path().join("library"), vec![common::registry_rider(990001, "Léa", "Nouvelle", "Bulle", "Gast")], vec![]);
    let show = Show { office, id, dir };

    // Jumping: numbers, start times, three results (one out), details, prizes, a president.
    let one = show.set_up("1");
    show.call("number", json!({ "class": one, "first": 1 })).unwrap();
    let setup = json!({ "layout": { "StartTimes": { "first": 480, "minutes": 3, "breaks": [[540, 570]], "move_up": false } }, "line_after": [10, "Entretien"] });
    show.call("set_layout", json!({ "class": one, "setup": setup })).unwrap();
    let starters: Vec<u64> = show.call("results", json!({ "class": one })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["not_starting"] == false).take(3).map(|l| l["entry"].as_u64().unwrap()).collect();
    show.call("edit_result", json!({ "class": one, "entry": starters[0], "values": { "strafpunkte_initial_1": "0", "zeit_initial_1": "61.5" } })).unwrap();
    show.call("edit_result", json!({ "class": one, "entry": starters[1], "values": { "strafpunkte_initial_1": "4", "zeit_initial_1": "58" } })).unwrap();
    let reasons = show.call("results", json!({ "class": one })).unwrap()["reasons"].clone();
    let out = reasons.as_array().unwrap().iter().find(|r| r["name"] == "ausgeschieden").unwrap()["code"].clone();
    show.call("edit_result", json!({ "class": one, "entry": starters[2], "out": out })).unwrap();
    let logo = "data:image/octet-stream;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";
    show.call("set_class_details", json!({ "class": one, "sponsor": "Garage du Lac", "placed_count": 2, "extra_fields": [{ "key": "Plaques", "value": "offertes" }], "logo": logo })).unwrap();
    let published_one: u64 = one.parse().unwrap();
    show.call("set_prizes", json!({ "published": published_one, "prizes": { "minimum": "74", "amounts": ["200", "160"], "natural": ["Couverture"] } })).unwrap();
    let people: Vec<Value> = show.call("find_officials", json!({ "query": "a" })).unwrap().as_array().unwrap().iter().take(3).cloned().collect();
    show.call("set_role", json!({ "class": one, "person": people[0]["person"], "role": "P" })).unwrap();

    // Dressage: two judges, a sheet each for a rider, one with a penalty.
    let dressage = show.set_up("14");
    let programme = show.call("programmes", json!({})).unwrap()[0]["id"].clone();
    show.call("edit_class", json!({ "class": dressage, "programme": programme })).unwrap();
    show.call("set_role", json!({ "class": dressage, "person": people[1]["person"], "role": "DRI", "letter": "C" })).unwrap();
    show.call("set_role", json!({ "class": dressage, "person": people[2]["person"], "role": "DRI", "letter": "H" })).unwrap();
    let sheets = show.call("sheets", json!({ "class": dressage })).unwrap();
    let rider = sheets["riders"].as_array().unwrap().iter().find(|r| r["not_starting"] == false).unwrap()["entry"].as_u64().unwrap();
    let movements = sheets["programmes"].as_object().unwrap().values().next().unwrap()["sections"].as_array().unwrap().iter().flat_map(|s| s["movements"].as_array().unwrap().clone()).map(|m| m["number"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    for (i, judge) in sheets["judges"].as_array().unwrap().iter().enumerate() {
        let marks: serde_json::Map<String, Value> = movements.iter().map(|m| (m.clone(), if i == 0 { json!(7) } else { json!(6.5) })).collect();
        let penalties = if i == 0 { json!([0]) } else { json!([]) };
        show.call("edit_sheet", json!({ "class": dressage, "entry": rider, "judge": judge["assignment"], "marks": marks, "penalties": penalties })).unwrap();
    }

    // A team class: a late entry with a new team.
    let team_class = show.set_up("7");
    let published_seven: u64 = team_class.parse().unwrap();
    show.call("add_entry", json!({ "published": published_seven, "rider": { "person": 990001 }, "horse": { "guest": "Petit Tonnerre" }, "new_team": "Les Nouveaux" })).unwrap();

    // Out in Rosson's format, with the event's access key (as Rosson's own export), back in.
    show.call("set_connection", json!({ "access_key": "ABC-123-XYZ" })).unwrap();
    let path = show.dir.path().join("rosson.txt");
    show.call("export_rosson", json!({ "path": path, "private": true })).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    if let Ok(keep) = std::env::var("EQUIEVENTS_KEEP") {
        std::fs::copy(&path, keep).unwrap();
    }
    assert!(text.starts_with("Rosson: 3.3.15\n"));
    assert!(text.contains("%22access_key%22%3A%22ABC-123-XYZ%22"));
    let events = show.office.call("rosson_events", &json!({ "path": path })).unwrap();
    let back = show.office.call("import_rosson", &json!({ "path": path, "event": events[0]["id"], "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    assert_ne!(back, show.id);
    // The key comes with the file: forgotten here, the file brings it back.
    show.office.call("set_connection", &json!({ "show": show.id, "access_key": null })).unwrap();
    assert_eq!(show.office.call("connection", &json!({ "show": back })).unwrap()["has_key"], false);
    show.office.call("import_rosson", &json!({ "path": path, "event": events[0]["id"], "mode": "ended" })).unwrap();
    assert_eq!(show.office.call("connection", &json!({ "show": back })).unwrap()["has_key"], true);

    for class in [&one, &dressage, &team_class] {
        let (before, after) = (class_view(&show.office, &show.id, class), class_view(&show.office, &back, class));
        assert_eq!(before, after, "class {class}");
    }
    // The judges' sheets: marks, the ticked penalty, the percentage.
    let rider_of = |show_id: &str| show.office.call("sheets", &json!({ "show": show_id, "class": dressage })).unwrap()["riders"].as_array().unwrap().iter().find(|r| r["entry"] == rider).cloned().unwrap();
    let (here, there) = (rider_of(&show.id), rider_of(&back));
    assert_eq!(here, there);
    assert!(here["percentage"].is_string());
    assert!(here["sheets"].as_object().unwrap().values().any(|s| s["penalties"] == json!([0])), "{here}");
    let prizes = |show_id: &str| show.office.call("prizes", &json!({ "show": show_id, "published": published_one })).unwrap()["prizes"].clone();
    assert_eq!(prizes(&back), prizes(&show.id));
    let officials = |show_id: &str| show.office.call("officials", &json!({ "show": show_id })).unwrap()["days"].clone();
    assert_eq!(officials(&back), officials(&show.id));

    // Without the private data: no addresses in the file.
    let public = show.dir.path().join("public.txt");
    show.call("export_rosson", json!({ "path": public, "private": false })).unwrap();
    let text = std::fs::read_to_string(&public).unwrap();
    assert!(!text.contains("reiter_strasse") && !text.contains("telefon_mobil"));
}
