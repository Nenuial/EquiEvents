//! Team dressage (Rosson's Equipen-Dressur), on a corpus dressage class made a team class: a
//! sheet per team and judge, the three ways of making the percentage, the upload.

use office::Office;
use serde_json::{json, Value};

mod common;

/// Show 2026.8352 with its dressage class 108211 made a team class: three teams of three riders.
fn team_show(dir: &std::path::Path) -> (Office, String) {
    let file = common::rosson_file_edited(dir, "2026.8352", |data| {
        let published = 108211;
        for a in data["ausschreibung_pruefungen"].as_array_mut().unwrap() {
            if a["id"] == published {
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
                }
            }
        }
        data["equipen"].as_array_mut().unwrap().extend(teams);
    });
    let office = Office::new(&dir.join("library"), "test").unwrap();
    let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
    let show = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    (office, show)
}

#[test]
fn team_dressage_three_two_entered_ranked_and_sent() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = team_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let class = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let ga06 = call("programmes", json!({})).unwrap().as_array().unwrap().iter().find(|p| p["name"] == "GA06/60").unwrap()["id"].clone();
    call("edit_class", json!({ "class": class, "programme": ga06 })).unwrap();
    let judges: Vec<u64> = call("find_officials", json!({ "query": "" })).unwrap().as_array().unwrap().iter().filter(|c| !c["titles"].as_array().unwrap().is_empty()).take(5).map(|c| c["person"].as_u64().unwrap()).collect();
    for (judge, letter) in judges.iter().zip(["E", "H", "C", "M", "B"]) {
        call("set_role", json!({ "class": class, "person": judge, "role": "DRI", "letter": letter })).unwrap();
    }
    let details = call("class_details", json!({ "class": class })).unwrap();
    assert_eq!((details["team_dressage"].clone(), details["team_3_2"].clone()), (json!(true), json!(false)));
    call("set_class_details", json!({ "class": class, "team_3_2": true })).unwrap();

    // A row per team; the judges H, C, M first, each with the rider they judge.
    let view = call("sheets", json!({ "class": class })).unwrap();
    assert_eq!(view["teams"], true);
    let teams = view["riders"].as_array().unwrap().clone();
    // In team order (drawn when the class was set up).
    let mut names: Vec<&str> = teams.iter().map(|t| t["rider"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["Équipe 1", "Équipe 2", "Équipe 3"]);
    let letters: Vec<&str> = view["judges"].as_array().unwrap().iter().map(|j| j["letter"].as_str().unwrap()).collect();
    assert_eq!(letters, ["H", "C", "M", "E", "B"]);
    let members: Vec<&str> = teams[0]["horse"].as_str().unwrap().split(" · ").collect();
    let h = view["judges"][0]["assignment"].as_str().unwrap();
    assert_eq!(teams[0]["judge_riders"][h].as_str(), Some(members[0]));
    assert!(teams[0]["judge_riders"].get(view["judges"][3]["assignment"].as_str().unwrap()).is_none());

    // Every movement the same mark: a judge's percentage is the mark × 10.
    let programme = view["programmes"].as_object().unwrap().values().next().unwrap().clone();
    let movements: Vec<String> = programme["sections"].as_array().unwrap().iter().flat_map(|s| s["movements"].as_array().unwrap().clone()).map(|m| m["number"].as_str().unwrap().to_string()).collect();
    let mark = |team: &Value, judge: usize, m: &str| {
        let marks: serde_json::Map<String, Value> = movements.iter().map(|n| (n.clone(), json!(m))).collect();
        call("edit_sheet", json!({ "class": class, "team": team["team"], "judge": view["judges"][judge]["assignment"], "marks": marks })).unwrap();
    };
    // Team 1: H 7, C 6, M 8, E 9, B 5 → (70 + 60 + 80) / 6 + (90 + 50) / 4 = 70.
    for (j, m) in ["7", "6", "8", "9", "5"].iter().enumerate() {
        mark(&teams[0], j, m);
    }
    // Team 2: 6.5 from every judge → 65.
    for j in 0..5 {
        mark(&teams[1], j, "6.5");
    }
    assert!(call("edit_sheet", json!({ "class": class, "team": "nobody", "judge": view["judges"][0]["assignment"], "marks": { "1": "6" } })).is_err());
    // Not to be switched once there are sheets.
    assert!(call("set_class_details", json!({ "class": class, "team_3_2": false })).unwrap_err().contains("sheets"));

    let view = call("sheets", json!({ "class": class })).unwrap();
    assert_eq!((view["riders"][0]["percentage"].as_str(), view["riders"][0]["rank"].as_u64()), (Some("70.00"), Some(1)));
    assert_eq!((view["riders"][1]["percentage"].as_str(), view["riders"][1]["rank"].as_u64()), (Some("65.00"), Some(2)));
    assert_eq!(view["riders"][0]["sheets"][h]["complete"], true);

    // A team's sheet deleted, and back with undo.
    call("delete_sheet", json!({ "class": class, "team": teams[0]["team"], "judge": h })).unwrap();
    assert!(call("sheets", json!({ "class": class })).unwrap()["riders"][0]["sheets"][h].is_null());
    call("undo", json!({})).unwrap();
    let view = call("sheets", json!({ "class": class })).unwrap();
    assert_eq!((view["riders"][0]["percentage"].as_str(), view["riders"][0]["sheets"][h]["complete"].as_bool()), (Some("70.00"), Some(true)));

    // The ranking: the teams, then their riders, each with their judge's total.
    let ranking = call("ranking", json!({ "class": class })).unwrap();
    let lines = ranking["lines"].as_array().unwrap();
    assert_eq!(lines.iter().filter(|l| l["team"] == true).count(), 2);
    let first_rider = lines.iter().find(|l| l["team"] == false && l["rider"] == members[0]).unwrap();
    assert_eq!(first_rider["judges"].as_object().unwrap().keys().collect::<Vec<_>>(), ["H"]);

    // Undo the last mark; delete the results: the sheets go.
    call("undo", json!({})).unwrap();
    call("set_connection", json!({ "access_key": "KEY" })).unwrap();
    call("send", json!({ "what": "results", "class": class })).unwrap();
    let log = call("simulation_log", json!({})).unwrap();
    let put = log.as_array().unwrap().iter().find(|r| r["path"] == "putResultate").unwrap().clone();
    let rows = put["body"]["resultate"].as_array().unwrap();
    let team = rows.iter().find(|r| r["team_resultat"] == true && r["rang"] == 1).unwrap();
    assert_eq!((team["punkte_prozent"].as_str(), team["reiter_id"].clone()), (Some("70.00"), Value::Null));
    let team_judges: Vec<&str> = team["pro_richter"].as_array().unwrap().iter().map(|j| j["buchstabe"].as_str().unwrap()).collect();
    assert_eq!(team_judges, ["E", "B"]);
    let member = rows.iter().find(|r| r["team_mitglied"] == true && r["team_id"] == team["team_id"]).unwrap();
    assert!(member.get("rang").is_none() && member.get("punkte_prozent").is_none());
    assert_eq!(member["pro_richter"].as_array().unwrap().len(), 1);
    assert_eq!(put["body"]["pruefungen"][0]["anzahl_gestartete_reiter"], 2);

    // The ranking print: the teams with their judges, each rider under the team with their judge.
    let state = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap().state().clone();
    let print = office::prints::rankings(&state, &[show::model::ClassId(class.clone())], office::prints::Language::Fr, "now").unwrap();
    let text = typeset::text(print.template, &print.data).unwrap().join("\n");
    if let Ok(path) = std::env::var("EQUIEVENTS_SHOW_TEXT") {
        std::fs::write(path, &text).unwrap();
    }
    assert!(text.contains("70.00") && text.contains(members[0]), "{text}");

    // To Rosson's format and back: the sheets, the percentages and 3/2 come back.
    let path = dir.path().join("back.txt");
    call("export_rosson", json!({ "path": path, "private": false })).unwrap();
    let events = office.call("rosson_events", &json!({ "path": path })).unwrap();
    let back = office.call("import_rosson", &json!({ "path": path, "event": events[0]["id"], "mode": "ended" })).unwrap()["show"].as_str().unwrap().to_string();
    let percentages = |show: &str| -> Vec<(String, Value)> {
        let v = office.call("sheets", &json!({ "show": show, "class": class })).unwrap();
        v["riders"].as_array().unwrap().iter().map(|t| (t["rider"].as_str().unwrap().to_string(), t["percentage"].clone())).collect()
    };
    assert_eq!(percentages(&back), percentages(&show));
    assert_eq!(office.call("class_details", &json!({ "show": back, "class": class })).unwrap()["team_3_2"], true);

    call("delete_results", json!({ "class": class })).unwrap();
    assert!(call("sheets", json!({ "class": class })).unwrap()["riders"][0]["sheets"].as_object().unwrap().is_empty());
}

#[test]
fn team_judges_sheets_name_each_judges_rider() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = team_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let class = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let ga06 = call("programmes", json!({})).unwrap().as_array().unwrap().iter().find(|p| p["name"] == "GA06/60").unwrap()["id"].clone();
    call("edit_class", json!({ "class": class, "programme": ga06 })).unwrap();
    let judges: Vec<u64> = call("find_officials", json!({ "query": "" })).unwrap().as_array().unwrap().iter().filter(|c| !c["titles"].as_array().unwrap().is_empty()).take(5).map(|c| c["person"].as_u64().unwrap()).collect();
    for (judge, letter) in judges.iter().zip(["E", "H", "C", "M", "B"]) {
        call("set_role", json!({ "class": class, "person": judge, "role": "DRI", "letter": letter })).unwrap();
    }
    call("set_class_details", json!({ "class": class, "team_3_2": true })).unwrap();
    let view = call("sheets", json!({ "class": class })).unwrap();
    let members: Vec<String> = view["riders"][1]["horse"].as_str().unwrap().split(" · ").map(str::to_string).collect();
    // The print's data, from the show's file as the app keeps it.
    let state = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap().state().clone();
    let print = |judge: &Value| -> Vec<Value> {
        let print = office::prints::judge_sheets(&state, &show::model::ClassId(class.clone()), Some(&show::model::AssignmentId(judge.as_str().unwrap().into())), false, office::prints::Language::Fr, "now").unwrap();
        print.data["sheets"].as_array().unwrap().clone()
    };
    // Judge C: one sheet per team, for its second rider; judge E: one per team, the team's.
    let c = print(&view["judges"][1]["assignment"]);
    assert_eq!(c.len(), 3);
    assert!(c[1]["rider"].as_str().unwrap().starts_with(&members[1]), "{}", c[1]);
    assert!(c[1]["rider"].as_str().unwrap().ends_with(view["riders"][1]["rider"].as_str().unwrap()), "{}", c[1]);
    let e = print(&view["judges"][3]["assignment"]);
    assert_eq!(e[0]["rider"].as_str().unwrap(), format!("Équipe : {}", view["riders"][0]["rider"].as_str().unwrap()));

    // OKV's test (GA 02/60): the side judges E and B mark on "OKV Mannschaftsdressur GA02/60",
    // H, C, M on GA02/60, whichever of the two is the class's test.
    let id_of = |name: &str| call("programmes", json!({})).unwrap().as_array().unwrap().iter().find(|p| p["name"] == name).unwrap()["id"].clone();
    for test in ["GA02/60", "OKV Mannschaftsdressur GA02/60"] {
        call("edit_class", json!({ "class": class, "programme": id_of(test) })).unwrap();
        let state = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap().state().clone();
        let sheets = |judge: &Value| office::prints::judge_sheets(&state, &show::model::ClassId(class.clone()), Some(&show::model::AssignmentId(judge.as_str().unwrap().into())), false, office::prints::Language::Fr, "now").unwrap().data;
        let title = |d: &Value| d["tests"][d["sheets"][0]["test"].as_str().unwrap()]["title"].as_str().unwrap_or_default().to_string();
        let test_id = |d: &Value| d["sheets"][0]["test"].clone();
        let (c, e) = (sheets(&view["judges"][1]["assignment"]), sheets(&view["judges"][3]["assignment"]));
        assert_eq!((test_id(&c), test_id(&e)), (json!(id_of("GA02/60").to_string()), json!(id_of("OKV Mannschaftsdressur GA02/60").to_string())), "{test}");
        // Its French title is the riders' test's at Swiss Equestrian: printed in German.
        assert!(!title(&c).contains("OKV"), "{test}: H, C, M: {}", title(&c));
        assert_eq!(title(&e), "OKV Mannschaftsdressur GA 02/60", "{test}: E, B");
    }
}

/// A team retires (or is eliminated): entered on the team's row, out of the ranking; in again
/// when cleared.
#[test]
fn a_team_out_of_the_class() {
    let dir = tempfile::tempdir().unwrap();
    let (office, show) = team_show(dir.path());
    let call = |command: &str, mut args: Value| {
        args["show"] = json!(show);
        office.call(command, &args)
    };
    let class = call("set_up_class", json!({ "published": 108211 })).unwrap()["class"].as_str().unwrap().to_string();
    let ga06 = call("programmes", json!({})).unwrap().as_array().unwrap().iter().find(|p| p["name"] == "GA06/60").unwrap()["id"].clone();
    call("edit_class", json!({ "class": class, "programme": ga06 })).unwrap();
    let judge = call("find_officials", json!({ "query": "" })).unwrap().as_array().unwrap().iter().find(|c| !c["titles"].as_array().unwrap().is_empty()).unwrap()["person"].clone();
    call("set_role", json!({ "class": class, "person": judge, "role": "DRI", "letter": "C" })).unwrap();
    let view = call("sheets", json!({ "class": class })).unwrap();
    let team = view["riders"][0]["team"].clone();
    let reason = view["reasons"][0]["code"].clone();
    assert!(reason.is_number(), "the reasons offered: {}", view["reasons"]);
    call("edit_result", json!({ "class": class, "team": team, "values": {}, "out": reason })).unwrap();
    let view = call("sheets", json!({ "class": class })).unwrap();
    assert_eq!(view["riders"][0]["out"], reason);
    let ranking = call("ranking", json!({ "class": class })).unwrap();
    let line = ranking["lines"].as_array().unwrap().iter().find(|l| l["id"] == team && l["team"] == true).unwrap().clone();
    assert_eq!(line["rank"], Value::Null, "out: no rank");
    call("edit_result", json!({ "class": class, "team": team, "values": {}, "out": null })).unwrap();
    assert_eq!(call("sheets", json!({ "class": class })).unwrap()["riders"][0]["out"], Value::Null);
}
