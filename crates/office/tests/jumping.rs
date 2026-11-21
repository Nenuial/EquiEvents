//! Show-jumping results in the office: by hand, from the timekeeper's file; a final bringing
//! the qualifier's penalties (U9); a ranking over several classes (U8). On the rehearsal of
//! 2026.8352: classes 1 and 2 are both "A au chrono" over one round, with 18 riders and horses
//! in common.

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

    /// Set up a published class by its number; its class id.
    fn set_up(&self, number: &str) -> String {
        let show = self.call("show", json!({})).unwrap();
        let published = show["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == number).unwrap()["id"].as_u64().unwrap();
        self.call("set_up_class", json!({ "published": published })).unwrap()["class"].as_str().unwrap().to_string()
    }

    fn results(&self, class: &str) -> Value {
        self.call("results", json!({ "class": class })).unwrap()
    }

    fn line(&self, class: &str, entry: u64) -> Value {
        self.results(class)["lines"].as_array().unwrap().iter().find(|l| l["entry"] == entry).unwrap().clone()
    }

    fn enter(&self, class: &str, entry: u64, faults: &str, time: &str) {
        self.call("edit_result", json!({ "class": class, "entry": entry, "values": { "strafpunkte_initial_1": faults, "zeit_initial_1": time } })).unwrap();
    }

    fn eliminate(&self, class: &str, entry: u64) {
        let reasons = self.results(class)["reasons"].clone();
        let code = reasons.as_array().unwrap().iter().find(|r| r["name"] == "ausgeschieden").unwrap()["code"].clone();
        self.call("edit_result", json!({ "class": class, "entry": entry, "out": code })).unwrap();
    }

    /// Entries of the same rider and horse in two classes: (in `a`, in `b`).
    fn common(&self, a: &str, b: &str) -> Vec<(u64, u64)> {
        let key = |l: &Value| (l["rider"].as_str().unwrap().to_string(), l["horse"].as_str().unwrap().to_string());
        // Riders who start in both (the draw is random; some entries do not start).
        let starts = |l: &&Value| l["not_starting"] == false;
        let lines_b: Vec<Value> = self.results(b)["lines"].as_array().unwrap().iter().filter(starts).cloned().collect();
        self.results(a)["lines"].as_array().unwrap().iter().filter(starts).filter_map(|la| lines_b.iter().find(|lb| key(lb) == key(la)).map(|lb| (la["entry"].as_u64().unwrap(), lb["entry"].as_u64().unwrap()))).collect()
    }
}

#[test]
fn results_by_hand_and_undo() {
    let show = Show::rehearsal();
    let class = show.set_up("1");
    let view = show.results(&class);
    assert_eq!(view["fields"], json!(["strafpunkte_initial_1", "zeit_initial_1"]));
    assert!(view["reasons"].as_array().unwrap().iter().any(|r| r["name"] == "ausgeschieden"));
    let entries: Vec<u64> = view["lines"].as_array().unwrap().iter().filter(|l| l["not_starting"] == false).take(4).map(|l| l["entry"].as_u64().unwrap()).collect();

    show.enter(&class, entries[0], "0", "62,5");
    show.enter(&class, entries[1], "4", "60.10");
    show.enter(&class, entries[2], "0", "65");
    show.eliminate(&class, entries[3]);

    let rank = |e: u64| show.line(&class, e)["rank"].clone();
    assert_eq!((rank(entries[0]), rank(entries[2]), rank(entries[1]), rank(entries[3])), (json!(1), json!(2), json!(3), Value::Null));
    assert_eq!(show.line(&class, entries[0])["time"], "62.5");
    assert_eq!(show.line(&class, entries[3])["has_result"], true);

    // The same again changes nothing, and adds nothing to undo.
    let answer = show.call("edit_result", json!({ "class": class, "entry": entries[0], "values": { "zeit_initial_1": "62.50" } })).unwrap();
    assert_eq!(answer["unchanged"], true);

    // Everything cleared: no result any more; undo brings it back.
    show.call("edit_result", json!({ "class": class, "entry": entries[2], "values": { "strafpunkte_initial_1": null, "zeit_initial_1": "" } })).unwrap();
    assert_eq!(show.line(&class, entries[2])["has_result"], false);
    assert_eq!(show.call("undo", json!({})).unwrap()["undone"], "edit_result");
    assert_eq!(rank(entries[2]), json!(2));

    // Back in the class after an elimination.
    show.call("edit_result", json!({ "class": class, "entry": entries[3], "out": null, "values": { "strafpunkte_initial_1": "8", "zeit_initial_1": "70" } })).unwrap();
    assert_eq!(rank(entries[3]), json!(4));

    assert!(show.call("edit_result", json!({ "class": class, "entry": entries[0], "values": { "punkte": "3" } })).unwrap_err().contains("no field"));
    assert!(show.call("edit_result", json!({ "class": class, "entry": entries[0], "values": { "zeit_initial_1": "-3" } })).is_err());
    assert!(show.call("edit_result", json!({ "class": class, "entry": entries[0], "values": { "zeit_initial_1": "abc" } })).is_err());
}

#[test]
fn timekeeper_files_out_and_in() {
    let show = Show::rehearsal();
    let class = show.set_up("1");
    let dir = tempfile::tempdir().unwrap();

    // The start list for the timekeeper, as Excel and as text.
    let xlsx = dir.path().join("class 1.xlsx");
    show.call("timekeeper_file", json!({ "class": class, "path": xlsx })).unwrap();
    assert!(std::fs::read(&xlsx).unwrap().starts_with(b"PK"));
    let txt = dir.path().join("class 1.txt");
    show.call("timekeeper_file", json!({ "class": class, "path": txt })).unwrap();
    let text = std::fs::read(&txt).unwrap();

    // The timekeeper fills in three riders: penalties, time, "OK" (Windows-1252, tab-separated).
    let mut lines: Vec<Vec<Vec<u8>>> = text.split(|b| *b == b'\n').map(|l| l.split(|b| *b == b'\t').map(<[u8]>::to_vec).collect()).collect();
    assert_eq!(lines[0][11], b"Aufgabe");
    for (i, (faults, time)) in [("0", "61.20"), ("4", "58.00"), ("0", "59.75")].iter().enumerate() {
        let row = &mut lines[i + 1];
        row[9] = faults.as_bytes().to_vec();
        row[10] = time.as_bytes().to_vec();
        row[11] = b"OK".to_vec();
    }
    let filled: Vec<u8> = lines.iter().map(|l| l.join(&b'\t')).collect::<Vec<_>>().join(&b'\n');
    let back = dir.path().join("results 1.txt");
    std::fs::write(&back, filled).unwrap();

    let answer = show.call("import_results", json!({ "class": class, "path": back })).unwrap();
    assert_eq!(answer["results"], 3);
    assert_eq!(answer["notes"], json!([]));
    let ranked: Vec<Value> = show.results(&class)["lines"].as_array().unwrap().iter().filter(|l| l["rank"].is_number()).cloned().collect();
    assert_eq!(ranked.len(), 3);
    assert!(ranked.iter().any(|l| l["rank"] == 1 && l["time"] == "59.75"));

    // The same file again: nothing new.
    assert_eq!(show.call("import_results", json!({ "class": class, "path": back })).unwrap()["unchanged"], true);
    // Undo takes the whole file back.
    show.call("undo", json!({})).unwrap();
    assert_eq!(show.results(&class)["lines"].as_array().unwrap().iter().filter(|l| l["has_result"] == true).count(), 0);
}

#[test]
fn a_final_brings_the_qualifier_penalties() {
    let show = Show::rehearsal();
    let (qualifier, final_) = (show.set_up("1"), show.set_up("2"));
    let common = show.common(&qualifier, &final_);
    assert!(common.len() >= 3);
    let [(qa, fa), (qb, fb), (qc, fc)] = [common[0], common[1], common[2]];
    show.enter(&qualifier, qa, "0", "70");
    show.enter(&qualifier, qb, "4", "60");
    show.eliminate(&qualifier, qc);

    show.call("edit_class", json!({ "class": final_, "qualifier": qualifier })).unwrap();
    let list = show.call("start_list", json!({ "class": final_ })).unwrap();
    assert_eq!(list["qualifier"], "1");
    let starter = |e: u64| list["lines"].as_array().unwrap().iter().find(|l| l["entry"] == e).unwrap().clone();
    assert_eq!(starter(fa)["carried"], "0");
    assert_eq!(starter(fb)["carried"], "4");
    assert_eq!((starter(fc)["carried"].clone(), starter(fc)["carried_out"].clone()), (Value::Null, json!(true)));
    assert_eq!(show.results(&final_)["qualifier"], "1");

    // Reverse order of the qualifier: its winner last; riders without a result there first.
    show.call("reverse_order", json!({ "class": final_ })).unwrap();
    let order: Vec<u64> = show.results(&final_)["lines"].as_array().unwrap().iter().map(|l| l["entry"].as_u64().unwrap()).collect();
    assert_eq!(&order[order.len() - 3..], &[fc, fb, fa]);

    // A class cannot be its own qualifier; the qualifier can be taken away.
    assert!(show.call("edit_class", json!({ "class": final_, "qualifier": final_ })).is_err());
    show.call("edit_class", json!({ "class": final_, "qualifier": null })).unwrap();
    let list = show.call("start_list", json!({ "class": final_ })).unwrap();
    assert!(list["lines"].as_array().unwrap().iter().all(|l| l["carried"].is_null()));
}

#[test]
fn ranking_over_two_classes() {
    let show = Show::rehearsal();
    let (one, two) = (show.set_up("1"), show.set_up("2"));
    let common = show.common(&one, &two);
    let [a, b, c, d] = [common[0], common[1], common[2], common[3]];
    // Class 1: A 0/70, B 0/72, C 4/60, D eliminated.
    show.enter(&one, a.0, "0", "70");
    show.enter(&one, b.0, "0", "72");
    show.enter(&one, c.0, "4", "60");
    show.eliminate(&one, d.0);
    // Class 2: D 0/60, C 0/65, B 4/75, A 4/80.
    show.enter(&two, d.1, "0", "60");
    show.enter(&two, c.1, "0", "65");
    show.enter(&two, b.1, "4", "75");
    show.enter(&two, a.1, "4", "80");
    let name = |entry: u64, class: &str| show.line(class, entry)["rider"].as_str().unwrap().to_string();
    let (na, nb, nc, nd) = (name(a.1, &two), name(b.1, &two), name(c.1, &two), name(d.1, &two));

    // Penalties added: 4 each, ex aequo (nothing breaks the tie unless chosen). D, out in class
    // 1, last without rank, said why (eliminated, not « out »).
    let id = show.call("set_combined", json!({ "name": "Championnat", "classes": [one, two], "method": "penalties" })).unwrap()["id"].as_str().unwrap().to_string();
    let view = show.call("combined", json!({ "id": id })).unwrap();
    assert_eq!(view["ranking"]["tie"], "none");
    // Numbered by the show, never typed: the highest class's number with ".1"; another ranking
    // whose highest class is the same gets ".2"; a ranking keeps its number while its classes stay.
    let highest = show.call("start_list", json!({ "class": two })).unwrap()["class"]["number"].as_str().unwrap().to_string();
    assert_eq!(view["ranking"]["number"], format!("{highest}.1"));
    assert_eq!(show.call("combined_number", json!({ "classes": [one, two] })).unwrap(), format!("{highest}.2"));
    assert_eq!(show.call("combined_number", json!({ "id": id, "classes": [one, two] })).unwrap(), format!("{highest}.1"));
    let other = show.call("set_combined", json!({ "name": "Coupe", "classes": [two, one], "method": "penalties", "number": "99" })).unwrap()["id"].as_str().unwrap().to_string();
    assert_eq!(show.call("combined", json!({ "id": other })).unwrap()["ranking"]["number"], format!("{highest}.2"));
    show.call("set_combined", json!({ "id": id, "name": "Championnat B", "classes": [one, two], "method": "penalties" })).unwrap();
    assert_eq!(show.call("combined", json!({ "id": id })).unwrap()["ranking"]["number"], format!("{highest}.1"));
    show.call("remove_combined", json!({ "id": other })).unwrap();
    let lines = view["lines"].as_array().unwrap();
    let mut got: Vec<(String, Value, Value)> = lines.iter().map(|l| (l["rider"].as_str().unwrap().to_string(), l["rank"].clone(), l["total"].clone())).collect();
    got.sort_by(|x, y| x.0.cmp(&y.0));
    let mut tied = vec![(na.clone(), json!(1), json!("4")), (nb.clone(), json!(1), json!("4")), (nc.clone(), json!(1), json!("4")), (nd.clone(), Value::Null, Value::Null)];
    tied.sort_by(|x, y| x.0.cmp(&y.0));
    assert_eq!(got, tied);
    assert_eq!((lines[3]["rider"].as_str(), lines[3]["cells"][0]["out"].as_str()), (Some(nd.as_str()), Some("ausgeschieden")));

    // Chosen tie-breaks: class 1's penalties (A and B still tied); class 1's result, penalties
    // then time (A 0/70 before B 0/72).
    let ranks = |tie: &str| -> Vec<(String, Value)> {
        show.call("set_combined", json!({ "id": id, "name": "Championnat", "classes": [one, two], "method": "penalties", "tie": tie, "tie_break": one })).unwrap();
        let view = show.call("combined", json!({ "id": id })).unwrap();
        assert_eq!(view["ranking"]["tie"], tie);
        view["lines"].as_array().unwrap().iter().map(|l| (l["rider"].as_str().unwrap().to_string(), l["rank"].clone())).collect()
    };
    let faults = ranks("faults");
    assert_eq!((&faults[0].1, &faults[1].1, &faults[2]), (&json!(1), &json!(1), &(nc.clone(), json!(3))));
    assert_eq!(ranks("result"), vec![(na.clone(), json!(1)), (nb.clone(), json!(2)), (nc.clone(), json!(3)), (nd.clone(), Value::Null)]);
    assert!(show.call("set_combined", json!({ "id": id, "name": "Championnat", "classes": [one, two], "method": "penalties", "tie": "result", "tie_break": "nope" })).is_err());
    assert!(show.call("set_combined", json!({ "id": id, "name": "Championnat", "classes": [one, two], "method": "penalties", "tie": "last_time" })).is_err());
    show.call("set_combined", json!({ "id": id, "name": "Championnat", "classes": [one, two], "method": "penalties" })).unwrap();

    // Ranks added: 1+4, 2+3, 3+2 = 5 each; class 1 breaks the tie.
    show.call("set_combined", json!({ "id": id, "name": "Championnat", "classes": [one, two], "method": "ranks", "tie_break": one })).unwrap();
    let lines = show.call("combined", json!({ "id": id })).unwrap()["lines"].as_array().unwrap().clone();
    let got: Vec<(String, Value)> = lines.iter().map(|l| (l["rider"].as_str().unwrap().to_string(), l["rank"].clone())).collect();
    assert_eq!(got, vec![(na.clone(), json!(1)), (nb.clone(), json!(2)), (nc.clone(), json!(3)), (nd.clone(), Value::Null)]);
    assert_eq!(lines[0]["total"], "5");

    // Listed with the show; undo, then removed.
    assert_eq!(show.call("show", json!({})).unwrap()["combined"][0]["method"], "ranks");
    show.call("undo", json!({})).unwrap();
    assert_eq!(show.call("show", json!({})).unwrap()["combined"][0]["method"], "penalties");
    show.call("remove_combined", json!({ "id": id })).unwrap();
    assert!(show.call("show", json!({})).unwrap()["combined"].as_array().unwrap().is_empty());

    assert!(show.call("set_combined", json!({ "name": "One", "classes": [one], "method": "penalties" })).is_err());
    assert!(show.call("set_combined", json!({ "name": "X", "classes": [one, two], "method": "ranks", "tie_break": "nope" })).is_err());
}

impl Show {
    /// A class's starters in start order: (entry, rider, horse).
    fn order(&self, class: &str) -> Vec<(u64, String, String)> {
        let list = self.call("start_list", json!({ "class": class })).unwrap();
        list["lines"].as_array().unwrap().iter().filter(|l| l["kind"] == "starter").map(|l| (l["entry"].as_u64().unwrap(), l["rider"].as_str().unwrap().to_string(), l["horse"].as_str().unwrap().to_string())).collect()
    }
}

#[test]
fn start_orders_made_again() {
    let show = Show::rehearsal();
    let (one, two) = (show.set_up("1"), show.set_up("2"));
    let pairs = |class: &str| show.order(class).into_iter().map(|(_, r, h)| (r, h)).collect::<Vec<_>>();

    // By name, then the other way round: riders with one horse in that order (those with
    // several are spread out, U11).
    let singles = |list: Vec<(String, String)>| {
        let count = |r: &String| list.iter().filter(|(x, _)| x == r).count();
        list.iter().filter(|(r, _)| count(r) == 1).cloned().collect::<Vec<_>>()
    };
    show.call("reorder", json!({ "class": two, "by": "name" })).unwrap();
    // (The names are printed first name first; ordered by the last name.)
    let riders: Vec<String> = singles(pairs(&two)).into_iter().map(|(r, _)| r.rsplit(' ').next().unwrap().to_string()).collect();
    assert!(riders.windows(2).all(|w| scoring::ranking::compare_names(&w[0], &w[1]).is_le()), "{riders:?}");
    show.call("reorder", json!({ "class": two, "by": "horse", "descending": true })).unwrap();
    let horses: Vec<String> = singles(pairs(&two)).into_iter().map(|(_, h)| h).collect();
    assert!(horses.windows(2).all(|w| scoring::ranking::compare_names(&w[0], &w[1]).is_ge()), "{horses:?}");

    // As in class 1, the same or the reverse: riders with one horse in class 1's order (or the
    // reverse), those not there after; a rider's horses spread out as always, the first here the
    // one that started first there (or, reversed, last).
    let theirs = pairs(&one);
    let at = |p: &(String, String)| theirs.iter().position(|t| t == p);
    let firsts = |mine: &[(String, String)]| {
        // Riders with several horses here, two at least ridden in class 1: (first horse's place
        // there, the places there of all of them).
        let riders: Vec<&String> = mine.iter().map(|(r, _)| r).filter(|r| mine.iter().filter(|(x, _)| x == *r).count() > 1).collect();
        let mut out: Vec<(usize, Vec<usize>)> = Vec::new();
        for r in riders {
            let places: Vec<usize> = mine.iter().filter(|(x, _)| x == r).filter_map(at).collect();
            let first = mine.iter().find(|(x, _)| x == r).and_then(at);
            if let (true, Some(first)) = (places.len() > 1, first) {
                if !out.iter().any(|(f, _)| *f == first) {
                    out.push((first, places));
                }
            }
        }
        out
    };
    show.call("reorder", json!({ "class": two, "by": "follow", "other": one })).unwrap();
    let mine = pairs(&two);
    let places: Vec<usize> = singles(mine.clone()).iter().filter_map(at).collect();
    assert!(places.len() >= 10 && places.windows(2).all(|w| w[0] < w[1]), "{places:?}");
    let newcomers = mine.iter().rposition(|p| at(p).is_some() && singles(mine.clone()).contains(p)).unwrap();
    // (A rider there on another horse takes that horse's place; who was not there at all, after.)
    assert!(singles(mine.clone()).iter().filter(|p| !theirs.iter().any(|(r, _)| r == &p.0)).all(|p| mine.iter().position(|m| m == p).unwrap() > newcomers));
    let several = firsts(&mine);
    assert!(several.iter().all(|(first, places)| first == places.iter().min().unwrap()), "{several:?}");
    show.call("reorder", json!({ "class": two, "by": "follow", "other": one, "descending": true })).unwrap();
    let mine = pairs(&two);
    let places: Vec<usize> = singles(mine.clone()).iter().filter_map(at).collect();
    assert!(places.len() >= 10 && places.windows(2).all(|w| w[0] > w[1]), "{places:?}");
    let reversed = firsts(&mine);
    assert!(!reversed.is_empty() && reversed.iter().all(|(first, places)| first == places.iter().max().unwrap()), "{reversed:?}");

    // Rotated: from the third starter; the first two go to the end.
    let before = show.order(&two);
    show.call("reorder", json!({ "class": two, "by": "from", "entry": before[2].0 })).unwrap();
    let after = show.order(&two);
    assert_eq!(after[0], before[2]);
    assert_eq!(&after[after.len() - 2..], &before[..2]);

    // After class 1's ranking: the worst first, the winner last, then who has no result there.
    let common = show.common(&one, &two);
    let (a, b, c) = (common[0], common[1], common[2]);
    show.enter(&one, a.0, "0", "60");
    show.enter(&one, b.0, "4", "61");
    show.enter(&one, c.0, "0", "65");
    show.call("reorder", json!({ "class": two, "by": "ranking", "other": one })).unwrap();
    let firsts: Vec<u64> = show.order(&two).into_iter().take(3).map(|(e, _, _)| e).collect();
    assert_eq!(firsts, vec![b.1, c.1, a.1]);
    // Undone: the rotated order again.
    show.call("undo", json!({})).unwrap();
    assert_eq!(show.order(&two), after);

    assert!(show.call("reorder", json!({ "class": two, "by": "colour" })).is_err());
    assert!(show.call("reorder", json!({ "class": two, "by": "from", "entry": 1 })).is_err());
}

#[test]
fn a_follow_up_class_after_a_split_one() {
    let show = Show::rehearsal();
    let one = show.set_up("1");
    show.call("split", json!({ "class": one, "parts": 2 })).unwrap();
    let published = |number: &str| show.call("show", json!({})).unwrap()["not_set_up"].as_array().unwrap().iter().find(|p| p["number"] == number).unwrap()["id"].as_u64().unwrap();
    let model: u64 = one.parse().unwrap();
    let two = published("2");
    show.call("set_up_following", json!({ "published": two, "model": model })).unwrap();

    // Two sections, each with class 1's section's pairs in its order; the others at the end of the last.
    for (letter, last) in [("a", false), ("b", true)] {
        let theirs: Vec<(String, String)> = show.order(&format!("{one}{letter}")).into_iter().map(|(_, r, h)| (r, h)).collect();
        let mine: Vec<(String, String)> = show.order(&format!("{two}{letter}")).into_iter().map(|(_, r, h)| (r, h)).collect();
        let shared = mine.iter().take_while(|p| theirs.contains(p)).count();
        assert!(shared > 0);
        let places: Vec<usize> = mine[..shared].iter().map(|p| theirs.iter().position(|t| t == p).unwrap()).collect();
        assert!(places.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(mine.len() > shared, last, "section {letter}");
    }
    let numbers: Vec<String> = show.call("show", json!({})).unwrap()["days"][0]["classes"].as_array().unwrap().iter().map(|c| c["number"].as_str().unwrap().to_string()).collect();
    assert!(numbers.contains(&"2a".to_string()) && numbers.contains(&"2b".to_string()), "{numbers:?}");

    // Already set up; nothing to follow; not itself.
    assert!(show.call("set_up_following", json!({ "published": two, "model": model })).is_err());
    let three = published("3");
    assert!(show.call("set_up_following", json!({ "published": three, "model": published("4") })).is_err());
    assert!(show.call("set_up_following", json!({ "published": three, "model": three })).is_err());
}

#[test]
fn results_deleted_and_start_lists_removed() {
    let show = Show::rehearsal();
    let (one, two) = (show.set_up("1"), show.set_up("2"));
    let entries: Vec<u64> = show.results(&one)["lines"].as_array().unwrap().iter().filter(|l| l["not_starting"] == false).take(3).map(|l| l["entry"].as_u64().unwrap()).collect();
    for (i, e) in entries.iter().enumerate() {
        show.enter(&one, *e, "0", &format!("{}", 60 + i));
    }
    let people: Vec<Value> = show.call("find_officials", json!({ "query": "a" })).unwrap().as_array().unwrap().iter().take(1).cloned().collect();
    show.call("set_role", json!({ "class": one, "person": people[0]["person"], "role": "P" })).unwrap();
    let with_results = |class: &str| show.results(class)["lines"].as_array().unwrap().iter().filter(|l| l["has_result"] == true).count();

    // Every result taken away; undo brings them back.
    show.call("delete_results", json!({ "class": one })).unwrap();
    assert_eq!(with_results(&one), 0);
    show.call("undo", json!({})).unwrap();
    assert_eq!(with_results(&one), 3);

    // The start lists: not while class 2 takes class 1 as its qualifier.
    show.call("edit_class", json!({ "class": two, "qualifier": one })).unwrap();
    let published: u64 = one.parse().unwrap();
    assert!(show.call("remove_start_lists", json!({ "published": published })).unwrap_err().contains("qualifier"));
    show.call("edit_class", json!({ "class": two, "qualifier": null })).unwrap();
    let order = show.order(&one);
    show.call("remove_start_lists", json!({ "published": published })).unwrap();
    let view = show.call("show", json!({})).unwrap();
    assert!(view["not_set_up"].as_array().unwrap().iter().any(|p| p["id"] == published));
    assert!(show.call("start_list", json!({ "class": one })).is_err());
    assert!(show.call("officials", json!({})).unwrap().to_string().matches(&format!("\"{one}\":{{")).count() == 0);
    // Undone: the class, its order, results and officials again.
    show.call("undo", json!({})).unwrap();
    assert_eq!(show.order(&one), order);
    assert_eq!(with_results(&one), 3);
    assert_eq!(show.call("officials", json!({})).unwrap().to_string().matches(&format!("\"{one}\":{{")).count(), 1);
    // A final class keeps them.
    show.call("finalise", json!({ "class": one })).unwrap();
    assert!(show.call("delete_results", json!({ "class": one })).is_err());
    assert!(show.call("remove_start_lists", json!({ "published": published })).is_err());
}

/// The ideal-time table (S20): the office sets the ideal time; equal penalties rank by the gap
/// to it, over or under the same; no ranking until it is set.
#[test]
fn ideal_time_table() {
    let show = Show::rehearsal();
    let class = show.set_up("1");
    let entries: Vec<u64> = show.results(&class)["lines"].as_array().unwrap().iter().filter(|l| l["not_starting"] == false).take(4).map(|l| l["entry"].as_u64().unwrap()).collect();
    show.call("set_scoring_table", json!({ "class": class, "table": 35 })).unwrap();
    let details = show.call("class_details", json!({ "class": class })).unwrap();
    assert_eq!((details["ideal_table"].clone(), details["ideal_time"].clone()), (json!(true), Value::Null));

    show.enter(&class, entries[0], "0", "76.20");
    show.enter(&class, entries[1], "0", "74.50");
    show.enter(&class, entries[2], "4", "75");
    show.enter(&class, entries[3], "0", "75.50");
    // No ideal time yet: said so, not ranked on the fastest time.
    assert!(show.results(&class)["ranking_error"].as_str().unwrap().contains("Idealzeit"));
    assert!(show.call("set_class_details", json!({ "class": class, "ideal_time": "1:75" })).is_err());
    show.call("set_class_details", json!({ "class": class, "ideal_time": "1:15" })).unwrap();
    assert_eq!(show.call("class_details", json!({ "class": class })).unwrap()["ideal_time"], "1:15");

    let rank = |e: u64| show.line(&class, e)["rank"].clone();
    // 74.50 and 75.50: half a second off, tied; 76.20 next; 4 penalties last.
    assert_eq!((rank(entries[1]), rank(entries[3]), rank(entries[0]), rank(entries[2])), (json!(1), json!(1), json!(3), json!(4)));
    // The start list says it.
    let list = show.call("print", json!({ "what": "start_list", "class": class, "language": "fr" })).unwrap();
    assert!(list["pages"].as_u64().unwrap() >= 1);
    // A class with results keeps its table.
    assert!(show.call("set_scoring_table", json!({ "class": class, "table": 14 })).unwrap_err().contains("results"));
    // In the class's details too.
    assert!(show.call("set_class_details", json!({ "class": class, "scoring_table": 14 })).unwrap_err().contains("results"));
}
