//! A class's start list laid out by the office: start times with breaks, blocks, a line after
//! the n-th starter; who does not start gives up the time or not.

use office::Office;
use serde_json::{json, Value};

mod common;

fn rehearsal() -> (Office, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = common::rosson_file(dir.path(), "2026.8352");
    let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
    let id = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    (office, id, dir)
}

fn lines(office: &Office, show: &str, class: &str) -> Vec<Value> {
    office.call("start_list", &json!({ "show": show, "class": class })).unwrap()["lines"].as_array().unwrap().clone()
}

#[test]
fn start_times_breaks_blocks_and_a_line() {
    let (office, show, _dir) = rehearsal();
    let published = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == "2").unwrap()["id"].as_u64().unwrap();
    let class = office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string();
    office.call("number", &json!({ "show": show, "class": class })).unwrap();

    // Every 6 minutes from 08:00, a break 08:10–08:30; those who do not start keep their time.
    let setup = json!({ "layout": { "StartTimes": { "first": 480, "minutes": 6, "breaks": [[490, 510]], "move_up": false } }, "line_after": [4, "Platzpflege"] });
    office.call("set_layout", &json!({ "show": show, "class": class, "setup": setup })).unwrap();
    let list = office.call("start_list", &json!({ "show": show, "class": class })).unwrap();
    assert_eq!(list["setup"], setup);
    let starters: Vec<&Value> = list["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").collect();
    let times: Vec<&str> = starters.iter().take(4).map(|l| l["time"].as_str().unwrap()).collect();
    // 08:06 would run into the break (its 6 minutes end at 08:12): after it, at 08:30.
    assert_eq!(times, ["08:00", "08:30", "08:36", "08:42"]);
    let texts: Vec<&Value> = list["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "text").collect();
    assert!(texts.iter().any(|t| t["pause"] == json!(["08:10", "08:30"])), "{texts:?}");
    assert!(texts.iter().any(|t| t["text"] == "Platzpflege"));

    // Blocks of 10, headed.
    let blocks = json!({ "layout": { "Blocks": { "size": 10, "headings": ["08h00", "à la suite"], "move_up": true } }, "line_after": null });
    office.call("set_layout", &json!({ "show": show, "class": class, "setup": blocks })).unwrap();
    let headings: Vec<String> = lines(&office, &show, &class).iter().filter(|l| l["kind"] == "text").map(|l| l["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(&headings[..2], ["08h00", "à la suite"]);

    // Refused: no minutes between starts, a break ending before it starts, an empty line.
    for bad in [
        json!({ "layout": { "StartTimes": { "first": 480, "minutes": 0, "breaks": [], "move_up": false } }, "line_after": null }),
        json!({ "layout": { "StartTimes": { "first": 480, "minutes": 5, "breaks": [[600, 590]], "move_up": false } }, "line_after": null }),
        json!({ "layout": "Plain", "line_after": [3, " "] }),
    ] {
        assert!(office.call("set_layout", &json!({ "show": show, "class": class, "setup": bad })).is_err(), "{bad}");
    }
    // Undo: the blocks again.
    office.call("set_layout", &json!({ "show": show, "class": class, "setup": { "layout": "Plain", "line_after": null } })).unwrap();
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(office.call("start_list", &json!({ "show": show, "class": class })).unwrap()["setup"], blocks);
}

#[test]
fn a_class_made_final_and_reopened() {
    let (office, show, _dir) = rehearsal();
    let published = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == "1").unwrap()["id"].as_u64().unwrap();
    let class = office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string();
    let status = || office.call("start_list", &json!({ "show": show, "class": class })).unwrap()["class"]["status"].clone();
    office.call("finalise", &json!({ "show": show, "class": class })).unwrap();
    assert_eq!(status(), "final");
    // Final: nothing changes it.
    assert!(office.call("draw", &json!({ "show": show, "class": class })).unwrap_err().contains("final"));
    // Reopened only with a reason; undo makes it final again.
    assert!(office.call("reopen", &json!({ "show": show, "class": class, "reason": " " })).is_err());
    office.call("reopen", &json!({ "show": show, "class": class, "reason": "jury decision" })).unwrap();
    assert_eq!(status(), "running");
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(status(), "final");
}

#[test]
fn the_history_of_a_show() {
    let (office, show, _dir) = rehearsal();
    let published = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == "1").unwrap()["id"].as_u64().unwrap();
    let class = office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string();
    office.call("finalise", &json!({ "show": show, "class": class })).unwrap();
    let lines = office.call("history", &json!({ "show": show, "language": "fr" })).unwrap();
    let lines = lines.as_array().unwrap();
    // Latest first, said in French; the import's officials on one line.
    assert_eq!(lines[0]["text"], "épreuve 1 clôturée");
    assert_eq!(lines[1]["text"], "Liste de départ : épreuve 1");
    assert!(lines.iter().any(|l| l["text"].as_str().unwrap().starts_with("Officiels attribués (×")), "{lines:?}");
    assert_eq!(lines.last().unwrap()["text"], "Concours importé");
    assert!(lines.iter().all(|l| l["text"].as_str().unwrap().contains(' ')), "a kind left unsaid: {lines:?}");
    // Searched.
    let found = office.call("history", &json!({ "show": show, "language": "de", "query": "abgeschlossen" })).unwrap();
    assert_eq!(found.as_array().unwrap().len(), 1);
}

#[test]
fn prizes_as_rosson_computes_them() {
    use rust_decimal::Decimal;
    let d = |v: i64| Decimal::from(v);
    // 100, 80, 64 → 65, 52 → 55, 44 → 45, 36 → 40.
    assert_eq!(office::results::prize_scale(d(100), d(0), 6), [100, 80, 65, 55, 45, 40].map(d));
    // Never below the minimum.
    assert_eq!(office::results::prize_scale(d(100), d(50), 6), [100, 80, 65, 55, 50, 50].map(d));

    let (office, show, _dir) = rehearsal();
    let published = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == "1").unwrap()["id"].as_u64().unwrap();
    // The fee of class 1 is 55: the minimum starts at (55 − 18) × 2, 18 CHF being the tax.
    let start = office.call("prizes", &json!({ "show": show, "published": published })).unwrap();
    assert_eq!((start["fee"].clone(), start["prizes"]["minimum"].clone(), start["set"].clone(), start["scale"].clone()), (json!("55"), json!("74"), json!(false), json!(true)));
    // The scale and its minimum are show jumping's: a dressage class's prizes are typed rank by rank.
    let dressage = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == "14").unwrap()["id"].as_u64().unwrap();
    let start = office.call("prizes", &json!({ "show": show, "published": dressage })).unwrap();
    assert_eq!((start["prizes"]["minimum"].clone(), start["scale"].clone()), (json!("0"), json!(false)));
    let scale = office.call("prize_scale", &json!({ "show": show, "first": "200", "minimum": "90", "ranks": 4 })).unwrap();
    assert_eq!(scale, json!(["200", "160", "130", "105"]));

    let prizes = json!({ "minimum": "90", "amounts": ["200", "160", "130", "105"], "natural": ["Couverture"] });
    office.call("set_prizes", &json!({ "show": show, "published": published, "prizes": prizes })).unwrap();
    assert_eq!(office.call("prizes", &json!({ "show": show, "published": published })).unwrap()["set"], true);

    // Never on the ranking (only on the prize labels).
    let class = office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string();
    let entry = office.call("results", &json!({ "show": show, "class": class })).unwrap()["lines"].as_array().unwrap().iter().find(|l| l["not_starting"] == false).unwrap()["entry"].clone();
    office.call("edit_result", &json!({ "show": show, "class": class, "entry": entry, "values": { "strafpunkte_initial_1": "0", "zeit_initial_1": "60" } })).unwrap();
    let store = show::Store::open(&_dir.path().join("library").join(format!("{show}.show"))).unwrap();
    let print = office::prints::rankings(store.state(), &[show::model::ClassId(class.clone())], office::prints::Language::Fr, "now").unwrap();
    let c = &print.data["classes"][0];
    assert!(!c["columns"].as_array().unwrap().iter().any(|h| h == "Prix"), "{c}");
    assert!(!c.to_string().contains("Couverture"), "{c}");

    // Taken away; undo brings them back.
    office.call("set_prizes", &json!({ "show": show, "published": published, "prizes": null })).unwrap();
    assert_eq!(office.call("prizes", &json!({ "show": show, "published": published })).unwrap()["set"], false);
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(office.call("prizes", &json!({ "show": show, "published": published })).unwrap()["prizes"]["natural"], json!(["Couverture"]));
}

#[test]
fn start_numbers_fixed_for_the_whole_show() {
    let (office, show, _dir) = rehearsal();
    let set_up = |n: &str| {
        let published = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == n).unwrap()["id"].as_u64().unwrap();
        office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string()
    };
    let (one, two) = (set_up("1"), set_up("2"));
    office.call("split", &json!({ "show": show, "class": two, "parts": 2 })).unwrap();
    assert!(office.call("number_show", &json!({ "show": show })).unwrap_err().contains("fix them"));
    office.call("set_fixed_numbers", &json!({ "show": show, "fixed": true })).unwrap();
    office.call("number_show", &json!({ "show": show, "order": "name" })).unwrap();
    assert_eq!(office.call("show", &json!({ "show": show })).unwrap()["event"]["numbers_assigned"], true);

    // One number per entry, unique in the show, from 1.
    let numbers = |class: &str| -> Vec<(u64, u64)> { lines(&office, &show, class).iter().filter(|l| l["kind"] == "starter").map(|l| (l["entry"].as_u64().unwrap(), l["start_number"].as_u64().unwrap())).collect() };
    let all: Vec<(u64, u64)> = [one.clone(), format!("{two}a"), format!("{two}b")].iter().flat_map(|c| numbers(c)).collect();
    let mut seen: Vec<u64> = all.iter().map(|(_, n)| *n).collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), all.len());
    assert_eq!((seen[0], *seen.last().unwrap()), (1, all.len() as u64));

    // By winning points: the fewest first.
    office.call("number_show", &json!({ "show": show, "order": "winning_points" })).unwrap();
    let store = show::Store::open(&_dir.path().join("library").join(format!("{show}.show"))).unwrap();
    let points = |e: u64| store.state().entries[&show::model::EntryId(e)].winning_points.pair.unwrap_or_default();
    let mut by_number = numbers(&one);
    by_number.sort_by_key(|(_, n)| *n);
    assert!(by_number.windows(2).all(|w| points(w[0].0) <= points(w[1].0)));
    office.call("undo", &json!({ "show": show })).unwrap();

    // Off: every number cleared; undo brings them back.
    office.call("set_fixed_numbers", &json!({ "show": show, "fixed": false })).unwrap();
    assert!(lines(&office, &show, &one).iter().all(|l| l["start_number"].is_null()));
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(numbers(&one), all.iter().filter(|(e, _)| numbers(&one).iter().any(|(x, _)| x == e)).cloned().collect::<Vec<_>>());
}

#[test]
fn classes_reordered_in_their_day() {
    let (office, show, _dir) = rehearsal();
    let set_up = |n: &str| {
        let published = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == n).unwrap()["id"].as_u64().unwrap();
        office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string()
    };
    let (_one, two, _three) = (set_up("1"), set_up("2"), set_up("3"));
    let day = || -> Vec<String> { office.call("show", &json!({ "show": show })).unwrap()["days"][0]["classes"].as_array().unwrap().iter().map(|c| c["number"].as_str().unwrap().to_string()).collect() };
    assert_eq!(day(), ["1", "2", "3"]);
    office.call("move_class", &json!({ "show": show, "class": two, "step": -1 })).unwrap();
    assert_eq!(day(), ["2", "1", "3"]);
    // At the start already: nothing happens.
    assert_eq!(office.call("move_class", &json!({ "show": show, "class": two, "step": -1 })).unwrap()["unchanged"], true);
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(day(), ["1", "2", "3"]);
    // Dragged to a place: the last first, the others in their order.
    office.call("move_class", &json!({ "show": show, "class": _three, "to": 0 })).unwrap();
    assert_eq!(day(), ["3", "1", "2"]);
    office.call("move_class", &json!({ "show": show, "class": _three, "to": 2 })).unwrap();
    assert_eq!(day(), ["1", "2", "3"]);
    assert_eq!(office.call("move_class", &json!({ "show": show, "class": _three, "to": 7 })).unwrap()["unchanged"], true);

    // To another day, at a place there: the class takes that day; both days keep their order.
    let _saturday = set_up("7");
    let days = || -> Vec<(String, Vec<String>)> {
        office.call("show", &json!({ "show": show })).unwrap()["days"].as_array().unwrap().iter().map(|d| (d["date"].as_str().unwrap().to_string(), d["classes"].as_array().unwrap().iter().map(|c| c["number"].as_str().unwrap().to_string()).collect())).collect()
    };
    let date = days()[1].0.clone();
    assert_eq!(days()[1].1, ["7"]);
    office.call("move_class", &json!({ "show": show, "class": two, "date": date, "to": 0 })).unwrap();
    assert_eq!(days(), [(days()[0].0.clone(), vec!["1".to_string(), "3".into()]), (date.clone(), vec!["2".to_string(), "7".into()])]);
    // Past the end: the last.
    office.call("move_class", &json!({ "show": show, "class": _one, "date": date, "to": 9 })).unwrap();
    assert_eq!(days()[1].1, ["2", "7", "1"]);
    assert_eq!(office.call("class_details", &json!({ "show": show, "class": _one })).unwrap()["date"], json!(date));
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(days()[0].1, ["1", "3"]);
}

#[test]
fn a_class_details() {
    let (office, show, dir) = rehearsal();
    let not_set_up = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].clone();
    let published = |n: &str| not_set_up.as_array().unwrap().iter().find(|p| p["number"] == n).unwrap()["id"].as_u64().unwrap();
    let class = office.call("set_up_class", &json!({ "show": show, "published": published("1") })).unwrap()["class"].as_str().unwrap().to_string();
    office.call("set_up_class", &json!({ "show": show, "published": published("2") })).unwrap();
    let details = |office: &Office| office.call("class_details", &json!({ "show": show, "class": class })).unwrap();
    assert_eq!(details(&office)["days"], json!(["2026-09-25", "2026-09-26", "2026-09-27"]));

    // A 1×1 PNG.
    let logo = "data:image/octet-stream;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";
    // (The number is the app's: given, it changes nothing. The day is set with the name.)
    let change = json!({
        "show": show, "class": class, "number": " 1bis ", "sponsor": "Garage du Lac",
        "placed_count": 5, "extra_fields": [{ "key": "Plaques", "value": "offertes par la Commune" }, { "key": " ", "value": "" }], "logo": logo
    });
    office.call("set_class_details", &change).unwrap();
    office.call("edit_class", &json!({ "show": show, "class": class, "date": "2026-09-26" })).unwrap();
    let d = details(&office);
    assert_eq!((d["number"].as_str(), d["date"].as_str(), d["sponsor"].as_str(), d["placed_count"].as_u64()), (Some("1"), Some("2026-09-26"), Some("Garage du Lac"), Some(5)));
    assert_eq!(d["extra_fields"], json!([{ "key": "Plaques", "value": "offertes par la Commune" }]));
    assert_eq!(d["logo"], logo);
    // The same again: nothing changes.
    assert_eq!(office.call("set_class_details", &change).unwrap()["unchanged"], true);

    // Printed: the extra line on the start list, the logo with it; the sponsor on the prize labels.
    let store = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap();
    let state = store.state();
    let print = office::prints::start_lists(state, &[show::model::ClassId(class.clone())], office::prints::Language::Fr, false, "now").unwrap();
    let text = typeset::text(print.template, &print.data).unwrap().join("\n");
    assert!(text.contains("offertes par la Commune"), "{text}");
    assert!(String::from_utf8_lossy(&typeset::render(print.template, &print.data).unwrap().bytes).contains("/Image"));
    office.call("set_prizes", &json!({ "show": show, "published": published("1"), "prizes": { "minimum": "50", "amounts": ["100"], "natural": [] } })).unwrap();
    let store = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap();
    let labels = office::prints::labels(store.state(), &office::prints::Labels::Prizes(show::model::ClassId(class.clone())), office::prints::Sheet::of("3475"), 0, office::prints::Language::Fr, "now").unwrap();
    assert_eq!(labels.data["labels"][0]["lines"][1]["text"], "Garage du Lac");

    // Refused: something not an image, none placed; a day not a date (any real day is fine).
    assert!(office.call("edit_class", &json!({ "show": show, "class": class, "date": "26.09.2026" })).is_err());
    for bad in [json!({ "logo": "hello" }), json!({ "placed_count": 0 })] {
        let mut args = bad.clone();
        args["show"] = json!(show);
        args["class"] = json!(class);
        assert!(office.call("set_class_details", &args).is_err(), "{bad}");
    }
    // 30 % again, no logo; undo brings them back.
    office.call("set_class_details", &json!({ "show": show, "class": class, "placed_count": null, "logo": null })).unwrap();
    assert_eq!((details(&office)["placed_count"].clone(), details(&office)["logo"].clone()), (Value::Null, Value::Null));
    office.call("undo", &json!({ "show": show })).unwrap();
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(details(&office)["logo"], logo);
}

/// A class's own name (Rosson's « Nom de l'épreuve », a prize's) is apart from its category and
/// scoring table: none when the class is set up, lists then say its category and table; typed,
/// it is the class's title, printed and sent as is. The table is changed with it, before results.
#[test]
fn a_class_name_apart_from_its_table() {
    let (office, show, dir) = rehearsal();
    let not_set_up = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].clone();
    let published = not_set_up.as_array().unwrap().iter().find(|p| p["number"] == "1").unwrap()["id"].as_u64().unwrap();
    let class = office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string();
    let row = |office: &Office| {
        let s = office.call("show", &json!({ "show": show })).unwrap();
        s["days"].as_array().unwrap().iter().flat_map(|d| d["classes"].as_array().unwrap().clone()).find(|c| c["id"] == class.as_str()).unwrap()
    };
    let r = row(&office);
    assert_eq!((r["name"].as_str(), r["title"].as_str()), (Some(""), Some(format!("{} · {}", r["category"].as_str().unwrap(), r["scoring"].as_str().unwrap()).as_str())));

    // Named, another table: one change; the title is the name, the scoring stays apart.
    let tables = office.call("class_details", &json!({ "show": show, "class": class })).unwrap()["scoring_tables"].clone();
    let other = tables.as_array().unwrap().iter().find(|t| t["code"] == "AZ2RUZ2").unwrap()["id"].as_u64().unwrap();
    office.call("edit_class", &json!({ "show": show, "class": class, "name": " Prix « Emil Frey » ", "scoring_table": other })).unwrap();
    let r = row(&office);
    assert_eq!((r["name"].as_str(), r["title"].as_str()), (Some("Prix « Emil Frey »"), Some("Prix « Emil Frey »")));
    assert_eq!(office.call("class_details", &json!({ "show": show, "class": class })).unwrap()["scoring_table"], other);
    let store = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap();
    let print = office::prints::start_lists(store.state(), &[show::model::ClassId(class.clone())], office::prints::Language::Fr, false, "now").unwrap();
    // The quotes are held to the name by no-break spaces.
    let text = typeset::text(print.template, &print.data).unwrap().join("\n").replace('\u{a0}', " ");
    assert!(text.contains("Prix « Emil Frey »") && text.contains(r["scoring"].as_str().unwrap()), "{text}");
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(row(&office)["name"], "");
}

/// The published class, as Rosson's info box: what Swiss Equestrian published, the entries by
/// status, and what the office changes there — the category as printed (empty: the code again),
/// the levels (fewer: the entries above lose theirs), and the scoring table before results. One
/// change with the class's details; undone as one.
#[test]
fn the_published_class_and_its_scoring_table() {
    let (office, show, _dir) = rehearsal();
    let not_set_up = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].clone();
    let published = not_set_up.as_array().unwrap().iter().find(|p| p["number"] == "1").unwrap()["id"].as_u64().unwrap();
    let class = office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string();
    let details = || office.call("class_details", &json!({ "show": show, "class": class })).unwrap();
    let d = details();
    let p = &d["published"];
    assert_eq!((p["number"].as_str(), p["discipline"].as_str(), p["has_levels"].as_bool()), (Some("1"), Some("CS"), Some(true)));
    assert!(p["counts"]["total"].as_u64().unwrap() > 30 && p["counts"]["not_starting"].as_u64().is_some());
    let code = p["category"].as_str().unwrap().to_string();
    let tables = d["scoring_tables"].as_array().unwrap();
    assert!(tables.len() > 20 && d["scoring_table"].is_u64() && d["has_results"] == false);

    // Two levels, a rider at level 2; the category as the office prints it; another table.
    let other = tables.iter().find(|t| t["code"] == "AZ2RUZ2").unwrap()["id"].as_u64().unwrap();
    office.call("set_class_details", &json!({ "show": show, "class": class, "category_text": "R/N 120 – Prix du Manège", "levels": 2, "scoring_table": other })).unwrap();
    let d = details();
    assert_eq!((d["published"]["category_text"].as_str(), d["published"]["levels"].as_u64(), d["scoring_table"].as_u64()), (Some("R/N 120 – Prix du Manège"), Some(2), Some(other)));
    let entry = office.call("results", &json!({ "show": show, "class": class })).unwrap()["lines"][0]["entry"].as_u64().unwrap();
    office.call("edit_entry_details", &json!({ "show": show, "entry": entry, "stage": 2 })).unwrap();
    let stage = || office.call("participants", &json!({ "show": show })).unwrap().as_array().unwrap().iter().find(|e| e["id"] == entry).unwrap()["stage"].clone();
    assert_eq!(stage(), json!(2));
    // One level again: the rider's level goes; the category's code again.
    office.call("set_class_details", &json!({ "show": show, "class": class, "category_text": " ", "levels": 1 })).unwrap();
    assert_eq!((details()["published"]["category_text"].as_str(), stage()), (Some(code.as_str()), Value::Null));
    let history = office.call("history", &json!({ "show": show })).unwrap().to_string();
    assert!(history.contains("Épreuve publiée 1"), "{history}");
    // Undone as one: two levels, the rider's level, the office's category.
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!((details()["published"]["category_text"].as_str(), details()["published"]["levels"].as_u64(), stage()), (Some("R/N 120 – Prix du Manège"), Some(2), json!(2)));
    // Not for dressage; not more than 9.
    assert!(office.call("set_class_details", &json!({ "show": show, "class": class, "levels": 12 })).is_err());
}

/// A class with more than 90 entries is divided before it runs (docs/deviations.md S19): it
/// says so. Team classes count teams (132 riders in 44 teams: not flagged), and are divided by
/// whole teams.
#[test]
fn a_large_class_divided_before_it_runs() {
    let (office, show, _dir) = rehearsal();
    let rows = |office: &Office| -> Vec<Value> { office.call("show", &json!({ "show": show })).unwrap()["days"].as_array().unwrap().iter().flat_map(|d| d["classes"].as_array().unwrap().clone()).collect() };
    let published = |n: &str| office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == n).unwrap()["id"].as_u64().unwrap();

    // Class 1 with 50 late entries: 93.
    let one = published("1");
    let class = office.call("set_up_class", &json!({ "show": show, "published": one })).unwrap()["class"].as_str().unwrap().to_string();
    common::registry(&_dir.path().join("library"), (0..50).map(|i| common::registry_rider(990_000 + i, "Late", &format!("Rider {i:02}"), "Bulle", "Gast")).collect(), vec![]);
    for i in 0..50 {
        office.call("add_entry", &json!({ "show": show, "published": one, "rider": { "person": 990_000 + i }, "horse": { "guest": format!("Guest {i:02}") } })).unwrap();
    }
    let row = rows(&office).into_iter().find(|c| c["id"] == class).unwrap();
    assert_eq!((row["starters"].as_u64(), row["divide_into"].as_u64()), (Some(93), Some(2)));
    office.call("split", &json!({ "show": show, "class": class, "parts": 2 })).unwrap();
    let sections: Vec<Value> = rows(&office).into_iter().filter(|c| c["published"] == one).collect();
    assert!(sections.len() == 2 && sections.iter().all(|c| c["divide_into"].is_null()));

    // The team class: 132 riders, but under 90 teams.
    let seven = published("7");
    let class = office.call("set_up_class", &json!({ "show": show, "published": seven })).unwrap()["class"].as_str().unwrap().to_string();
    let row = rows(&office).into_iter().find(|c| c["id"] == class).unwrap();
    assert!(row["starters"].as_u64().unwrap() > 90 && row["divide_into"].is_null());
    // Divided anyway, by whole teams.
    office.call("split", &json!({ "show": show, "class": class, "parts": 2 })).unwrap();
    let sections: Vec<Value> = rows(&office).into_iter().filter(|c| c["published"] == seven).collect();
    assert_eq!(sections.iter().map(|c| c["number"].as_str().unwrap()).collect::<Vec<_>>(), ["7a", "7b"]);
    let teams = |id: &str| -> Vec<String> {
        office.call("start_list", &json!({ "show": show, "class": id })).unwrap()["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "team").map(|l| l["team"].as_str().unwrap().to_string()).collect()
    };
    let (a, b) = (teams(sections[0]["id"].as_str().unwrap()), teams(sections[1]["id"].as_str().unwrap()));
    assert!(!a.is_empty() && !b.is_empty() && a.iter().all(|t| !b.contains(t)));
    // Each rider under their team: the first of every team has place 1.
    let list = office.call("start_list", &json!({ "show": show, "class": sections[1]["id"] })).unwrap();
    let lines = list["lines"].as_array().unwrap();
    for (i, l) in lines.iter().enumerate() {
        if l["kind"] == "team" {
            assert_eq!(lines[i + 1]["position"], 1);
        }
    }
    // More sections than teams: refused.
    assert!(office.call("split", &json!({ "show": show, "class": sections[0]["id"], "parts": 26 })).is_err());
}

/// Riders with several horses start as far apart as possible (U11), drawn or sorted.
#[test]
fn a_riders_horses_spread_out() {
    let (office, show, _dir) = rehearsal();
    let published = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == "1").unwrap()["id"].as_u64().unwrap();
    let class = office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string();
    let check = |office: &Office| {
        let lines = lines(office, &show, &class);
        let riders: Vec<String> = lines.iter().filter(|l| l["kind"] == "starter").map(|l| l["rider"].as_str().unwrap().to_string()).collect();
        let n = riders.len();
        let mut places: std::collections::BTreeMap<&str, Vec<usize>> = Default::default();
        for (i, r) in riders.iter().enumerate() {
            places.entry(r.as_str()).or_default().push(i);
        }
        let twos: Vec<&Vec<usize>> = places.values().filter(|p| p.len() == 2).collect();
        assert!(twos.len() >= 2, "{places:?}");
        let ones = places.values().filter(|p| p.len() == 1).count();
        for p in &twos {
            // One at the start, one at the end: everyone with one horse in between.
            assert!(p[0] < twos.len() && p[1] >= n - twos.len(), "{p:?} of {n}");
            assert!(p[1] - p[0] > ones, "{p:?}");
        }
    };
    check(&office);
    office.call("draw", &json!({ "show": show, "class": class })).unwrap();
    check(&office);
    office.call("reorder", &json!({ "show": show, "class": class, "by": "name" })).unwrap();
    check(&office);
}

/// A rider's horses swapped among their places (U12): numbers stay with the horses, or follow
/// the places when asked; nobody else moves.
#[test]
fn a_riders_horses_rearranged() {
    let (office, show, _dir) = rehearsal();
    let published = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == "1").unwrap()["id"].as_u64().unwrap();
    let class = office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string();
    office.call("number", &json!({ "show": show, "class": class, "first": 1 })).unwrap();
    let starters = |office: &Office| -> Vec<(u64, u64, u64)> {
        lines(office, &show, &class).iter().filter(|l| l["kind"] == "starter").map(|l| (l["entry"].as_u64().unwrap(), l["person"].as_u64().unwrap(), l["start_number"].as_u64().unwrap())).collect()
    };
    let before = starters(&office);
    let person = before.iter().map(|s| s.1).find(|p| before.iter().filter(|s| s.1 == *p).count() == 2).unwrap();
    let mine: Vec<(usize, (u64, u64, u64))> = before.iter().cloned().enumerate().filter(|(_, s)| s.1 == person).collect();
    let (first, second) = (mine[0], mine[1]);

    // Swapped; each keeps its number.
    office.call("rearrange_rider", &json!({ "show": show, "class": class, "entries": [second.1 .0, first.1 .0] })).unwrap();
    let after = starters(&office);
    assert_eq!(after[first.0], second.1);
    assert_eq!(after[second.0], first.1);
    let others = |list: &[(u64, u64, u64)]| list.iter().filter(|s| s.1 != person).cloned().collect::<Vec<_>>();
    assert_eq!(others(&after), others(&before));

    // With the numbers following: the earlier start has the lower number, whichever horse it is.
    office.call("rearrange_rider", &json!({ "show": show, "class": class, "entries": [second.1 .0, first.1 .0], "numbers_follow": true })).unwrap();
    let after = starters(&office);
    let low = first.1 .2.min(second.1 .2);
    let high = first.1 .2.max(second.1 .2);
    assert_eq!(after[first.0], (second.1 .0, person, low));
    assert_eq!(after[second.0], (first.1 .0, person, high));
    office.call("rearrange_rider", &json!({ "show": show, "class": class, "entries": [first.1 .0, second.1 .0], "numbers_follow": true })).unwrap();
    assert_eq!(starters(&office), before);

    // Undo: back as it was; not someone else's horse, not one twice.
    office.call("undo", &json!({ "show": show })).unwrap();
    office.call("undo", &json!({ "show": show })).unwrap();
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(starters(&office), before);
    let stranger = before.iter().find(|s| s.1 != person).unwrap().0;
    assert!(office.call("rearrange_rider", &json!({ "show": show, "class": class, "entries": [first.1 .0, stranger] })).is_err());
    assert!(office.call("rearrange_rider", &json!({ "show": show, "class": class, "entries": [first.1 .0, first.1 .0] })).is_err());
}

/// A pony class's start order is grouped by pony size (A, B, C, D, then without), as Rosson's:
/// the course is set for each size.
#[test]
fn ponies_start_by_size() {
    let (office, show, _dir) = rehearsal();
    let published = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == "10").unwrap()["id"].as_u64().unwrap();
    office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap();
    let plan = office.call("sections", &json!({ "show": show, "published": published })).unwrap();
    let size_of: std::collections::BTreeMap<u64, String> = plan["riders"].as_array().unwrap().iter().flat_map(|r| r["horses"].as_array().unwrap().clone()).map(|h| (h["entry"].as_u64().unwrap(), h["pony"].as_str().unwrap_or("~").to_string())).collect();
    let order: Vec<String> = lines(&office, &show, &published.to_string()).iter().filter(|l| l["kind"] == "starter").map(|l| size_of[&l["entry"].as_u64().unwrap()].clone()).collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted);
    assert!(order.iter().collect::<std::collections::BTreeSet<_>>().len() >= 3, "{order:?}");
}

/// A maximum of starters: the entries made last beyond it leave the start list and wait on none,
/// to move up; with results, the starters stay.
#[test]
fn a_maximum_of_starters() {
    let (office, show, _dir) = rehearsal();
    let published = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == "1").unwrap()["id"].as_u64().unwrap();
    let class = office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string();
    let list = |office: &Office| office.call("start_list", &json!({ "show": show, "class": class })).unwrap();
    let all = list(&office)["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").count();
    assert!(all > 30);

    office.call("set_class_details", &json!({ "show": show, "class": class, "max_starters": 30 })).unwrap();
    let view = list(&office);
    assert_eq!(view["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").count(), 30);
    assert_eq!(view["unplaced"].as_array().unwrap().len(), all - 30);
    assert_eq!(view["class"]["max_starters"], 30);
    let waiting: Vec<u64> = view["unplaced"].as_array().unwrap().iter().map(|u| u["entry"].as_u64().unwrap()).collect();
    // One moves up.
    office.call("place_entry", &json!({ "show": show, "entry": waiting[0], "class": class })).unwrap();
    assert_eq!(list(&office)["unplaced"].as_array().unwrap().len(), all - 31);
    // Undo twice: everyone again.
    office.call("undo", &json!({ "show": show })).unwrap();
    office.call("undo", &json!({ "show": show })).unwrap();
    assert_eq!(list(&office)["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").count(), all);
    assert!(office.call("set_class_details", &json!({ "show": show, "class": class, "max_starters": 0 })).is_err());
}

/// Printed before start numbers: by name (last, first, then the horse), no place column; the
/// office still works in the drawn order.
#[test]
fn printed_by_name_before_numbers() {
    let (office, show, dir) = rehearsal();
    let not_set_up = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].clone();
    let published = not_set_up.as_array().unwrap().iter().find(|p| p["number"] == "1").unwrap()["id"].as_u64().unwrap();
    let class = office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap().to_string();
    let store = show::Store::open(&dir.path().join("library").join(format!("{show}.show"))).unwrap();
    let state = store.state();
    let id = show::model::ClassId(class.clone());
    let key = |e: &show::model::Entry| {
        let p = &state.persons[&e.rider];
        (p.last_name.clone(), p.first_name.clone(), e.horses_on_entry[0]["pferdename"].as_str().unwrap_or("").to_string())
    };
    let order = |view: office::views::StartListView| {
        view.lines.iter().filter_map(|l| if let office::views::StartLine::Starter { entry, .. } = l { Some(key(&state.entries[&show::model::EntryId(*entry)])) } else { None }).collect::<Vec<_>>()
    };
    let printed = order(office::views::published_start_list(state, &id).unwrap());
    let names = scoring::ranking::compare_names;
    assert!(printed.windows(2).all(|w| names(&w[0].0, &w[1].0).then_with(|| names(&w[0].1, &w[1].1)).then_with(|| names(&w[0].2, &w[1].2)).is_le()), "{printed:?}");
    // The office's view: the drawn order, not by name.
    assert_ne!(order(office::views::start_list(state, &id).unwrap()), printed);
    let print = office::prints::start_lists(state, &[id], office::prints::Language::Fr, false, "now").unwrap();
    let text = typeset::text(print.template, &print.data).unwrap().join("\n");
    assert!(!text.lines().any(|l| l.trim() == "#"), "{text}");
}

/// Dressage (Dressurreglement 5.2 (1)): more than 35 starters, two series with two juries are
/// the rule (the technical delegate can allow up to 40); more than 40, the class has to be
/// divided. Hors concours rides do not count. Dividing stays the office's decision.
#[test]
fn a_large_dressage_class_is_told_about_series() {
    let dir = tempfile::tempdir().unwrap();
    let office = Office::new(&dir.path().join("library"), "test").unwrap();
    let file = common::rosson_file(dir.path(), "2026.8298");
    let event = office.call("rosson_events", &json!({ "path": file })).unwrap()[0]["id"].as_str().unwrap().to_string();
    let show = office.call("import_rosson", &json!({ "path": file, "event": event, "mode": "delivered" })).unwrap()["show"].as_str().unwrap().to_string();
    let rows = |office: &Office| -> Vec<Value> { office.call("show", &json!({ "show": show })).unwrap()["days"].as_array().unwrap().iter().flat_map(|d| d["classes"].as_array().unwrap().clone()).collect() };
    let not_set_up = office.call("show", &json!({ "show": show })).unwrap()["not_set_up"].as_array().unwrap().clone();
    let published = not_set_up.iter().find(|p| p["discipline"] == "CD" && p["entries"].as_u64().unwrap() < 30).expect("a dressage class")["id"].as_u64().unwrap();
    let class = json!(office.call("set_up_class", &json!({ "show": show, "published": published })).unwrap()["class"].as_str().unwrap());
    let row = rows(&office).into_iter().find(|c| c["id"] == class).unwrap();
    let starters = |office: &Office| rows(office).into_iter().find(|c| c["id"] == class).unwrap();
    let have = row["starters"].as_u64().unwrap();
    common::registry(&dir.path().join("library"), (0..45).map(|i| common::registry_rider(990_000 + i, "Late", &format!("Rider {i:02}"), "Bulle", "Gast")).collect(), vec![]);
    let mut added = 0;
    let mut add = |office: &Office, n: u64| {
        for _ in 0..n {
            office.call("add_entry", &json!({ "show": show, "published": published, "rider": { "person": 990_000 + added }, "horse": { "guest": format!("Guest {added:02}") } })).unwrap();
            added += 1;
        }
    };
    add(&office, 35 - have);
    assert!(starters(&office)["divide_into"].is_null(), "35: nothing to say");
    add(&office, 1);
    let r = starters(&office);
    assert_eq!((r["divide_into"].as_u64(), r["over"].as_u64(), r["must_divide"].as_bool()), (Some(2), Some(35), Some(false)));
    add(&office, 5);
    let r = starters(&office);
    assert_eq!((r["divide_into"].as_u64(), r["over"].as_u64(), r["must_divide"].as_bool()), (Some(2), Some(40), Some(true)));
    // One of the 41 rides hors concours: 40 count.
    let hc = office.call("participants", &json!({ "show": show })).unwrap().as_array().unwrap().iter().find(|e| e["published"].as_u64() == Some(published)).expect("an entry of the class")["id"].clone();
    office.call("edit_entry", &json!({ "show": show, "entry": hc, "hors_concours": true })).unwrap();
    let r = starters(&office);
    assert_eq!((r["over"].as_u64(), r["must_divide"].as_bool()), (Some(35), Some(false)));
}
