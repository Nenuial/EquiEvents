//! A show imported from Rosson, exported to an EquiEvents show file and imported again: the same
//! show, with or without its private data; and Rosson's own export files read like the corpus.

use std::fs;
use std::path::Path;

use serde_json::Value;
use show::export::{self, Privacy};
use show::State;

fn shows() -> Vec<(String, Value)> {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let mut dirs: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("snapshot.json").is_file()).collect();
    dirs.sort();
    dirs.into_iter().map(|d| (d.file_name().unwrap().to_string_lossy().to_string(), serde_json::from_str(&fs::read_to_string(d.join("snapshot.json")).unwrap()).unwrap())).collect()
}

#[test]
fn a_show_survives_export_and_import() {
    let dir = tempfile::tempdir().unwrap();
    for (show, snapshot) in shows() {
        let ops = rosson_import::ops::to_operations(&snapshot).unwrap();
        let (state, _) = State::from_log(ops.clone());
        let bytes = export::write(&ops, Privacy::All, "test", "2026-11-13T12:00:00Z").unwrap();
        let (manifest, back) = export::read(&bytes).unwrap();
        assert_eq!(back, ops, "{show}");
        assert_eq!(manifest.event.as_ref().map(|e| e.number.as_str()), Some(show.as_str()));

        // Into a show file and out again.
        let path = dir.path().join(format!("{show}.sqlite"));
        let mut store = show::Store::open(&path).unwrap();
        export::import_into(&mut store, &bytes).unwrap();
        drop(store);
        let store = show::Store::open(&path).unwrap();
        assert_eq!(store.state(), &state, "{show}");
        assert!(store.ignored().is_empty());
        assert_eq!(export::write(&store.log().unwrap(), Privacy::All, "test", "2026-11-13T12:00:00Z").unwrap(), bytes, "{show}: not the same file again");

        // Without private data: the same show, without contacts and notes.
        let public = export::write(&ops, Privacy::Public, "test", "2026-11-13T12:00:00Z").unwrap();
        let (_, public_ops) = export::read(&public).unwrap();
        let (public_state, _) = State::from_log(public_ops);
        assert!(public_state.contacts.is_empty() && public_state.entry_notes.is_empty());
        assert!(!state.contacts.is_empty(), "{show}: no contacts to leave out");
        let mut expected = state.clone();
        expected.contacts.clear();
        expected.entry_notes.clear();
        assert_eq!(public_state, expected, "{show}");
        println!("{show}: {} operations, {} KB ({} KB without private data)", ops.len(), bytes.len() / 1024, public.len() / 1024);
    }
}

/// JavaScript's `encodeURIComponent`, as Rosson writes its exports.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[test]
fn rosson_export_files_are_read_like_the_corpus() {
    for (show, snapshot) in shows() {
        let file = format!("Rosson: 3.3.15\n{}", encode(&snapshot.to_string()));
        let rosson = rosson_import::export::read(file.as_bytes()).unwrap();
        let events = rosson.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].number, show);
        let event = rosson.event(&events[0].id).unwrap();
        let (from_file, _) = State::from_log(rosson_import::ops::to_operations(&event).unwrap());
        let (from_corpus, _) = State::from_log(rosson_import::ops::to_operations(&snapshot).unwrap());
        assert_eq!(from_file, from_corpus, "{show}");
    }
}

/// The real exports, where they are (not in the repository: they hold personal data).
/// `cargo test -p rosson-import --test show_file -- --ignored --nocapture`
#[test]
#[ignore]
fn every_real_export_imports() {
    let root = Path::new(&std::env::var("HOME").unwrap()).join("Library/CloudStorage/Dropbox/Concours");
    let mut files = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if p.is_dir() && !name.starts_with('.') && name != "node_modules" {
                stack.push(p);
            } else if name.ends_with(".txt") && name.contains("_export_") && !name.contains("master") && !name.contains("client") {
                files.push(p);
            }
        }
    }
    files.sort();
    let (mut events, mut refused) = (0, 0);
    for f in &files {
        match rosson_import::export::read(&fs::read(f).unwrap()) {
            Err(e) => {
                refused += 1;
                println!("{}: {}", f.display(), e.0);
            }
            Ok(rosson) => {
                for e in rosson.events() {
                    let ops = rosson_import::ops::to_operations(&rosson.event(&e.id).unwrap()).unwrap_or_else(|err| panic!("{} {}: {}", f.display(), e.number, err.0));
                    let (state, ignored) = State::from_log(ops);
                    assert!(ignored.is_empty(), "{} {}: {ignored:?}", f.display(), e.number);
                    for class in state.classes.keys() {
                        state.ranking(class).unwrap_or_else(|err| panic!("{} {} {class}: {err}", f.display(), e.number));
                    }
                    events += 1;
                }
            }
        }
    }
    println!("{} files, {events} events imported and ranked, {refused} files refused", files.len());
}
