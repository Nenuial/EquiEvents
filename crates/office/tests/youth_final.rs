//! Tag der Jugend finals (RL Tag der Jugend 10.3; docs/deviations.md S38): no ex aequo in ranks
//! 1–3 — judges C and H's marks for the programme's tie movements, then lots (ranks set by hand).

mod common;

use office::Office;
use serde_json::{json, Value};

#[test]
fn a_youth_final_breaks_top_ties_then_asks_for_lots() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = common::rosson_file(dir.path(), "2026.8298");
    let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
    let show = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    let classes: Vec<String> = office.call("show", &json!({ "show": show })).unwrap()["days"].as_array().unwrap().iter().flat_map(|d| d["classes"].as_array().unwrap().clone()).map(|c| c["id"].as_str().unwrap().to_string()).collect();
    let class = classes.into_iter().find(|c| office.call("class_details", &json!({ "show": show, "class": c })).unwrap()["youth_programme"] == true).expect("a Tag der Jugend class");
    office.call("reopen", &json!({ "show": show, "class": class, "reason": "a youth final" })).unwrap();
    office.call("set_class_details", &json!({ "show": show, "class": class, "youth_final": true })).unwrap();
    assert_eq!(office.call("class_details", &json!({ "show": show, "class": class })).unwrap()["youth_final"], true);

    // The judges: C and H for the tie movements.
    let sheets = office.call("sheets", &json!({ "show": show, "class": class })).unwrap();
    let letters: Vec<String> = sheets["judges"].as_array().unwrap().iter().map(|j| j["letter"].as_str().unwrap_or("").to_string()).collect();
    let ranking = |office: &Office| -> Vec<Value> { office.call("ranking", &json!({ "show": show, "class": class })).unwrap()["lines"].as_array().unwrap().clone() };
    let lines = ranking(&office);
    let (first, second) = (lines[0]["id"].as_str().unwrap().to_string(), lines[1]["id"].as_str().unwrap().to_string());
    let rider = |id: &str| sheets["riders"].as_array().unwrap().iter().find(|r| r["entry"].as_u64().is_some_and(|e| e.to_string() == id)).unwrap().clone();

    // The second rider gets the first's sheets: tied 1st, on the tie movements too — lots.
    let copy = rider(&first);
    for (judge, sheet) in copy["sheets"].as_object().unwrap() {
        office.call("edit_sheet", &json!({ "show": show, "class": class, "entry": second.parse::<u64>().unwrap(), "judge": judge, "marks": sheet["marks"], "penalties": sheet["penalties"] })).unwrap();
    }
    let lines = ranking(&office);
    let draw: Vec<(String, Value, Value)> = lines.iter().take(3).map(|l| (l["id"].as_str().unwrap().to_string(), l["rank"].clone(), l["draw"].clone())).collect();
    assert!(draw.iter().filter(|(id, _, _)| *id == first || *id == second).all(|(_, rank, draw)| *rank == json!(1) && *draw == json!(true)), "{draw:?} (judges {letters:?})");

    // Judge C gives the second rider half a point less on the first tie movement and half a point
    // more on a movement of the same coefficient that is not one: same points, the tie movement
    // decides — the first rider is 1st alone.
    let programme = office.call("class_details", &json!({ "show": show, "class": class })).unwrap()["programme"].clone();
    let name = office.call("programmes", &json!({ "show": show })).unwrap().as_array().unwrap().iter().find(|p| p["id"] == programme).unwrap()["name"].as_str().unwrap().to_string();
    let (tie, other) = match name.as_str() {
        "TdJ Fuerz" => ("2", "9"),
        "TdJ EinRei" => ("6", "5"),
        "TdJ Einste" => ("15", "1"),
        other => panic!("programme {other}"),
    };
    let c = sheets["judges"].as_array().unwrap().iter().find(|j| j["letter"] == "C").expect("a judge C")["assignment"].as_str().unwrap().to_string();
    let marks = &copy["sheets"][&c]["marks"];
    let shifted = |m: &str, by: f64| {
        let mark = marks[m].as_str().map(|t| t.parse::<f64>().unwrap()).or_else(|| marks[m].as_f64()).unwrap_or_else(|| panic!("movement {m} in {marks}"));
        format!("{}", mark + by)
    };
    office.call("edit_sheet", &json!({ "show": show, "class": class, "entry": second.parse::<u64>().unwrap(), "judge": c, "marks": { tie: shifted(tie, -0.5), other: shifted(other, 0.5) } })).unwrap();
    let lines = ranking(&office);
    let top: Vec<(String, Value, Value)> = lines.iter().take(2).map(|l| (l["id"].as_str().unwrap().to_string(), l["rank"].clone(), l["draw"].clone())).collect();
    assert_eq!(top, vec![(first.clone(), json!(1), json!(false)), (second.clone(), json!(2), json!(false))]);
}
