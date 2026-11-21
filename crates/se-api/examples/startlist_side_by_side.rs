//! Debug aid: Rosson's start list and ours, line by line, for one published class.
//! `cargo run -p se-api --example startlist_side_by_side -- <show> <published id>`
use std::fs;
use std::path::Path;

use se_api::startlists::{put_startlisten, Language};
use se_api::Keys;
use serde_json::Value;

fn short(z: &Value) -> String {
    if z["typ"] == "text" {
        format!("TEXT {:?} nr {} t {}", z["text"].as_str().unwrap_or(""), z["startnummer"], z["startzeit"])
    } else {
        format!("{} nr {} pos {} {} {}", z["reiter_id"], z["startnummer"], z["start_position"], z["reiter_nachname"].as_str().unwrap_or(""), if z["kein_start"] == true { "NS" } else { "" })
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus").join(&args[1]);
    let snapshot: Value = serde_json::from_str(&fs::read_to_string(corpus.join("snapshot.json")).unwrap()).unwrap();
    let (state, _) = show::State::from_log(rosson_import::ops::to_operations(&snapshot).unwrap());
    let fixture: Value = serde_json::from_str(&fs::read_to_string(corpus.join(format!("api/startlists-{}.json", args[2]))).unwrap()).unwrap();
    let theirs = &fixture["requests"][0]["body"];
    let keys = Keys { access_key: String::new(), rosson_key: String::new(), rosson_version: String::new() };
    let ours = put_startlisten(&state, &show::model::PublishedClassId(args[2].parse().unwrap()), &keys, Language::De).unwrap().body.unwrap();
    for (a, b) in theirs["startlisten"].as_array().unwrap().iter().zip(ours["startlisten"].as_array().unwrap()) {
        println!("== {} programm_name {:?} / {:?}", a["nummer"], a["programm_name"], b["programm_name"]);
        let (za, zb) = (a["zeilen"].as_array().unwrap(), b["zeilen"].as_array().unwrap());
        for i in 0..za.len().max(zb.len()) {
            let (x, y) = (za.get(i).map(short).unwrap_or_default(), zb.get(i).map(short).unwrap_or_default());
            println!("{} {:<60} | {}", if x == y { " " } else { "*" }, x, y);
        }
    }
}
