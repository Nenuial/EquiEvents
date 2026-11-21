//! Rehearsing a show with a past event: a Rosson export, rebuilt into what Swiss Equestrian
//! delivered before the show (`rosson_import::delivered`), loaded through the same code as a
//! real event (`se_api::load`).

use std::fs;
use std::path::Path;

use se_api::load::{read_event, read_master_data, read_officials};
use serde_json::Value;
use show::oplog::{Clock, Operation};
use show::State;

/// From a Rosson export (one event) to a show as Swiss Equestrian delivered it.
fn rehearsal(event: &Value) -> State {
    let master = read_master_data(&rosson_import::delivered::master_data(event), 0).unwrap();
    let officials = read_officials(&rosson_import::delivered::officials(event)).unwrap();
    let mut n = 0;
    let changes = read_event(&rosson_import::delivered::event(event), &master, &officials, 0, &mut || {
        n += 1;
        format!("rehearsal-{n}")
    })
    .unwrap();
    let mut clock = Clock::new("rehearsal");
    let (state, ignored) = State::from_log(changes.into_iter().enumerate().map(|(i, change)| Operation { id: format!("op{i}"), stamp: clock.tick(0), author: "rehearsal".into(), change }).collect());
    assert!(ignored.is_empty());
    state
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[test]
fn a_past_event_as_delivered() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let mut dirs: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("snapshot.json").is_file()).collect();
    dirs.sort();
    for dir in dirs {
        let snapshot: Value = serde_json::from_str(&fs::read_to_string(dir.join("snapshot.json")).unwrap()).unwrap();
        // As a Rosson export file.
        let file = format!("Rosson: 3.3.15\n{}", encode(&snapshot.to_string()));
        let rosson = rosson_import::export::read(file.as_bytes()).unwrap();
        let event = rosson.event(&rosson.events()[0].id).unwrap();
        let state = rehearsal(&event);
        let delivered = snapshot["nennungen"].as_array().unwrap().iter().filter(|n| n["erstellt_am"].is_string()).count();
        // Everything Swiss Equestrian delivered, nothing of the show itself.
        assert_eq!(state.entries.len(), delivered);
        assert_eq!(state.published_classes.len(), snapshot["ausschreibung_pruefungen"].as_array().unwrap().len());
        assert!(state.classes.is_empty() && state.results.is_empty() && state.starts.is_empty());
        assert!(state.entries.values().all(|e| e.rider_on_entry.is_some()));
        assert!(!state.contacts.is_empty());
    }
}

/// The real exports, where they are (not in the repository).
/// `cargo test -p se-api --test rehearsal -- --ignored --nocapture`
#[test]
#[ignore]
fn every_real_export_can_be_rehearsed() {
    let root = Path::new(&std::env::var("HOME").unwrap()).join("Library/CloudStorage/Dropbox/Concours");
    let mut stack = vec![root];
    let (mut events, mut entries) = (0, 0);
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if p.is_dir() && !name.starts_with('.') && name != "node_modules" {
                stack.push(p);
            } else if name.ends_with(".txt") && name.contains("_export_") && !name.contains("master") && !name.contains("client") {
                let Ok(rosson) = rosson_import::export::read(&fs::read(&p).unwrap()) else { continue };
                for ev in rosson.events() {
                    let state = rehearsal(&rosson.event(&ev.id).unwrap());
                    assert!(state.entries.values().all(|e| e.rider_on_entry.is_some()), "{}: an entry without rider", p.display());
                    entries += state.entries.len();
                    events += 1;
                }
            }
        }
    }
    println!("{events} events rebuilt as delivered and loaded, {entries} entries");
}
