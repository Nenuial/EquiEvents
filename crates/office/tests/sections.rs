//! Dividing a class into sections (U13): the plan, rules, applying, dividing again, whole again.

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

    fn set_up(&self, number: &str) -> u64 {
        let published = self.call("show", json!({})).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == number).unwrap()["id"].as_u64().unwrap();
        self.call("set_up_class", json!({ "published": published })).unwrap();
        published
    }

    fn plan(&self, published: u64) -> Value {
        self.call("sections", json!({ "published": published })).unwrap()
    }

    /// Each section's riders (person ids), by class number.
    fn sections(&self, published: u64) -> Vec<(String, Vec<u64>)> {
        let show = self.call("show", json!({})).unwrap();
        let mut classes: Vec<Value> = show["days"].as_array().unwrap().iter().flat_map(|d| d["classes"].as_array().unwrap().clone()).filter(|c| c["published"] == published).collect();
        classes.sort_by_key(|c| c["number"].as_str().unwrap().to_string());
        classes
            .iter()
            .map(|c| {
                let list = self.call("start_list", json!({ "class": c["id"] })).unwrap();
                let mut people: Vec<u64> = list["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| l["person"].as_u64().unwrap()).collect();
                people.dedup();
                (c["number"].as_str().unwrap().to_string(), people)
            })
            .collect()
    }
}

#[test]
fn a_class_divided_by_pony_size_then_again_then_whole() {
    let show = Show::rehearsal();
    // Class 10: the pony final (A, B, C, D ponies).
    let published = show.set_up("10");
    let plan = show.plan(published);
    assert!(plan["locked"].is_null());
    let riders = plan["riders"].as_array().unwrap().clone();
    let starters: usize = riders.iter().map(|r| r["horses"].as_array().unwrap().len()).sum();
    assert!(riders.iter().any(|r| r["horses"][0]["pony"].is_string()), "{plan}");

    // Ponies AB · C · D: three sections, each pony by its size (they are ranked separately).
    let sections = show.call("suggest_sections", json!({ "published": published, "rule": "pony", "groups": ["AB", "C", "D"] })).unwrap();
    for (r, s) in riders.iter().zip(sections.as_array().unwrap()) {
        for (h, x) in r["horses"].as_array().unwrap().iter().zip(s.as_array().unwrap()) {
            let expected = match h["pony"].as_str().unwrap_or("") { "A" | "B" => 0, "C" => 1, _ => 2 };
            assert_eq!(x.as_u64().unwrap(), expected, "{r}");
        }
    }
    let answer = show.call("apply_sections", json!({ "published": published, "parts": 3, "sections": sections })).unwrap();
    assert_eq!(answer["class"], format!("{published}a"));
    let divided = show.sections(published);
    assert_eq!(divided.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["10a", "10b", "10c"]);
    // Every rider there.
    let mut all: Vec<u64> = divided.iter().flat_map(|(_, p)| p.clone()).collect();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), riders.len());
    let plan = show.plan(published);
    assert_eq!(plan["riders"].as_array().unwrap().iter().map(|r| r["horses"].as_array().unwrap().len()).sum::<usize>(), starters);

    // Again, balanced in two: 10a and 10b, 10c gone.
    let two = show.call("suggest_sections", json!({ "published": published, "rule": "balanced", "parts": 2 })).unwrap();
    show.call("apply_sections", json!({ "published": published, "parts": 2, "sections": two })).unwrap();
    let divided = show.sections(published);
    assert_eq!(divided.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["10a", "10b"]);
    let sizes: Vec<usize> = show.plan(published)["riders"].as_array().unwrap().iter().flat_map(|r| r["horses"].as_array().unwrap().clone()).fold(vec![0, 0], |mut acc, h| {
        acc[h["section"].as_u64().unwrap() as usize] += 1;
        acc
    });
    assert!(sizes[0].abs_diff(sizes[1]) <= 3, "{sizes:?}");

    // Whole again; undo: the two sections back.
    let one: Vec<Vec<u64>> = riders.iter().map(|r| vec![0; r["horses"].as_array().unwrap().len()]).collect();
    show.call("apply_sections", json!({ "published": published, "parts": 1, "sections": one })).unwrap();
    assert_eq!(show.sections(published).iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["10"]);
    show.call("undo", json!({})).unwrap();
    assert_eq!(show.sections(published).iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["10a", "10b"]);

    // Refused: a rider missing, an empty section, too many.
    assert!(show.call("apply_sections", json!({ "published": published, "parts": 2, "sections": [[0]] })).is_err());
    assert!(show.call("apply_sections", json!({ "published": published, "parts": 3, "sections": one })).is_err());
}

#[test]
fn riders_of_a_club_stay_together_and_results_lock_it() {
    let show = Show::rehearsal();
    let published = show.set_up("1");
    let plan = show.plan(published);
    let riders = plan["riders"].as_array().unwrap().clone();
    let sections = show.call("suggest_sections", json!({ "published": published, "rule": "club", "parts": 3 })).unwrap();
    // Rider by rider: all of a rider's horses in one section; a club together.
    let of_rider: Vec<u64> = sections
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            let s: Vec<u64> = s.as_array().unwrap().iter().map(|x| x.as_u64().unwrap()).collect();
            assert!(s.windows(2).all(|w| w[0] == w[1]));
            s[0]
        })
        .collect();
    for (a, sa) in riders.iter().zip(&of_rider) {
        for (b, sb) in riders.iter().zip(&of_rider) {
            if !a["club"].as_str().unwrap().is_empty() && a["club"] == b["club"] {
                assert_eq!(sa, sb, "{} / {}", a["name"], b["name"]);
            }
        }
    }
    assert!((0..3).all(|s| of_rider.contains(&s)));
    show.call("apply_sections", json!({ "published": published, "parts": 3, "sections": sections })).unwrap();

    // A rider with two horses: both in one section, spread out there (one first, one last).
    let plan = show.plan(published);
    let two = plan["riders"].as_array().unwrap().iter().find(|r| r["horses"].as_array().unwrap().len() == 2).unwrap().clone();
    let class = format!("{published}{}", (b'a' + two["horses"][0]["section"].as_u64().unwrap() as u8) as char);
    let list = show.call("start_list", json!({ "class": class })).unwrap();
    let people: Vec<u64> = list["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| l["person"].as_u64().unwrap()).collect();
    let at: Vec<usize> = people.iter().enumerate().filter(|(_, p)| **p == two["person"].as_u64().unwrap()).map(|(i, _)| i).collect();
    assert_eq!(at.len(), 2);
    assert!(at[0] < 3 && at[1] + 3 >= people.len(), "{at:?} of {}", people.len());

    // A result in one section: no dividing again.
    let entry = list["lines"].as_array().unwrap().iter().find(|l| l["kind"] == "starter").unwrap()["entry"].clone();
    show.call("edit_result", json!({ "class": class, "entry": entry, "values": { "strafpunkte_initial_1": "0", "zeit_initial_1": "60" } })).unwrap();
    // Said by the screen in the office's language: why, and the section (its number).
    let plan = show.plan(published);
    let section = format!("{}{}", plan["number"].as_str().unwrap(), class.chars().last().unwrap());
    assert_eq!((plan["locked"]["why"].as_str(), plan["locked"]["class"].as_str()), (Some("results"), Some(section.as_str())));
    let again = show.call("suggest_sections", json!({ "published": published, "rule": "balanced", "parts": 2 })).unwrap();
    assert!(show.call("apply_sections", json!({ "published": published, "parts": 2, "sections": again })).unwrap_err().contains("results"));
}
