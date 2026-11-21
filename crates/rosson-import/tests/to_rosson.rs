//! A show written in Rosson's format and read back is the same show: every corpus show,
//! imported from Rosson's export, written again with `to_rosson`, and imported from that file.
//! And the file has Rosson's shape: its stores and records as Rosson's own export of the event.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::fs;
use std::path::Path;

use serde_json::Value;
use show::State;

fn shows() -> Vec<(String, Value)> {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let mut dirs: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("snapshot.json").is_file()).collect();
    dirs.sort();
    dirs.into_iter().map(|d| (d.file_name().unwrap().to_string_lossy().to_string(), serde_json::from_str(&fs::read_to_string(d.join("snapshot.json")).unwrap()).unwrap())).collect()
}

fn state(snapshot: &Value) -> State {
    let (state, ignored) = State::from_log(rosson_import::ops::to_operations(snapshot).unwrap());
    assert!(ignored.is_empty(), "{ignored:?}");
    state
}

/// The show without the records kept from Rosson (they gain the show's own fields on the
/// way: compared by writing the file again).
fn without_records(mut s: State) -> State {
    for e in s.event.iter_mut() {
        e.record = Value::Null;
    }
    for p in s.published_classes.values_mut() {
        p.record = Value::Null;
    }
    for e in s.entries.values_mut().chain(s.delivered_entries.values_mut()) {
        e.record = Value::Null;
    }
    s
}

fn compare<K: Ord + Debug, V: PartialEq + Debug>(show: &str, what: &str, a: &BTreeMap<K, V>, b: &BTreeMap<K, V>, out: &mut Vec<String>) {
    for (k, v) in a {
        match b.get(k) {
            None => out.push(format!("{show} {what} {k:?}: lost")),
            Some(w) if w != v => out.push(format!("{show} {what} {k:?}:\n  before {v:?}\n  after  {w:?}")),
            _ => {}
        }
    }
    for k in b.keys().filter(|k| !a.contains_key(k)) {
        out.push(format!("{show} {what} {k:?}: new"));
    }
}

#[test]
fn a_show_written_in_rossons_format_reads_back_the_same() {
    let mut problems = Vec::new();
    for (show, snapshot) in shows() {
        let before = state(&snapshot);
        let stores = rosson_import::to_rosson::event_stores(&before, true, None).unwrap();
        let bytes = rosson_import::to_rosson::file(&stores);
        let export = rosson_import::export::read(&bytes).unwrap();
        assert_eq!(export.version, rosson_import::to_rosson::VERSION);
        let event = export.events()[0].id.clone();
        let read_back = state(&export.event(&event).unwrap());
        // Written again: the same file.
        if rosson_import::to_rosson::event_stores(&read_back, true, None).unwrap() != stores {
            problems.push(format!("{show}: written again, not the same file"));
        }
        let (before, after) = (without_records(before), without_records(read_back));

        let event_of = |s: &State| s.event.iter().map(|e| (e.id.clone(), e.clone())).collect::<BTreeMap<_, _>>();
        compare(&show, "event", &event_of(&before), &event_of(&after), &mut problems);
        compare(&show, "published class", &before.published_classes, &after.published_classes, &mut problems);
        compare(&show, "person", &before.persons, &after.persons, &mut problems);
        compare(&show, "contact", &before.contacts, &after.contacts, &mut problems);
        compare(&show, "entry note", &before.entry_notes, &after.entry_notes, &mut problems);
        compare(&show, "horse", &before.horses, &after.horses, &mut problems);
        compare(&show, "entry", &before.entries, &after.entries, &mut problems);
        compare(&show, "team", &before.teams, &after.teams, &mut problems);
        compare(&show, "class", &before.classes, &after.classes, &mut problems);
        compare(&show, "start list", &before.starts, &after.starts, &mut problems);
        compare(&show, "result", &before.results, &after.results, &mut problems);
        compare(&show, "judge sheet", &before.sheets, &after.sheets, &mut problems);
        compare(&show, "official", &before.assignments, &after.assignments, &mut problems);
        compare(&show, "rank by hand", &before.manual_ranks, &after.manual_ranks, &mut problems);
        compare(&show, "scoring table", &before.scoring_tables, &after.scoring_tables, &mut problems);
        compare(&show, "programme", &before.programmes, &after.programmes, &mut problems);
        compare(&show, "reason", &before.reasons, &after.reasons, &mut problems);
        compare(&show, "category", &before.categories, &after.categories, &mut problems);
        compare(&show, "national official", &before.officials, &after.officials, &mut problems);
        compare(&show, "prizes", &before.prizes, &after.prizes, &mut problems);
        compare(&show, "ranking over classes", &before.combined, &after.combined, &mut problems);
        // Rosson's finals written back as Rosson wrote them.
        if stores["finalpruefungen"] != snapshot["finalpruefungen"] {
            problems.push(format!("{show}: finals not written back as they were"));
        }
        compare(&show, "master data", &before.master_records, &after.master_records, &mut problems);
        if before.roles != after.roles {
            problems.push(format!("{show} roles differ"));
        }
        // Every class ranks the same.
        for class in before.classes.keys() {
            let rank = |s: &State| s.ranking(class).map(|r| r.lines.iter().map(|l| (l.id.clone(), l.placement)).collect::<Vec<_>>());
            if rank(&before) != rank(&after) {
                problems.push(format!("{show} class {class}: ranked differently"));
            }
        }
    }
    let shown: Vec<&String> = problems.iter().take(20).collect();
    assert!(problems.is_empty(), "{} differences:\n{}", problems.len(), shown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
}

/// The file has Rosson's stores, and each record the fields Rosson's own export of the event
/// has (a few that Rosson keeps for itself aside).
#[test]
fn the_file_has_rossons_shape() {
    // Rosson's working fields, not needed to open the event again.
    // Rosson's working fields and computed values, not needed to open the event again (Rosson
    // computes them); contacts of the national list of officials (not kept: not public).
    let own: BTreeSet<&str> = [
        "changed", "ist_neu", "index", "importiert_am", "newWertung", "programm_disziplinen", "updated_at", "hochgeladen_am", "startliste_hochgeladen_am", "rangliste_hochgeladen_am",
        "mitPruefung", "duplikat", "anzahlAufgaben", "anzahlResultate", "total", "zeit", "strafpunkte", "resultate", "equipe", "programmDuration", "reihenfolge", "wertung", "kommentar",
        "gesamttotal", "herleitung", "max", "neue_reihenfolge", "email", "postleitzahl", "strasse", "telefon_arbeit", "telefon_mobil", "telefon_privat",
    ]
        .into_iter()
        .collect();
    // Per store: values Rosson computes when a result is saved (a phase's gap and total time, a
    // dressage result's maximum), the programme Rosson's tablet import notes on a result, and the
    // junk keys Rosson's driving settings write into a phase (docs/rosson/driving.md, bug 2).
    let own_in: BTreeSet<(&str, &str)> = [
        ("resultate", "abweichung"), ("resultate", "total_zeit"), ("resultate", "maxPunkte"), ("resultate", "programm_id"), ("zwischenpruefungen", "0"), ("zwischenpruefungen", "1"),
    ]
        .into_iter()
        .collect();
    let mut missing: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (show, snapshot) in shows() {
        let stores = rosson_import::to_rosson::event_stores(&state(&snapshot), true, Some("key")).unwrap();
        for (store, records) in snapshot.as_object().unwrap() {
            let Some(theirs) = records.as_array() else { continue };
            // Not written yet: Rosson's rankings over several classes (Finalprüfungen).
            if store == "finalpruefungen" {
                continue;
            }
            let ours = stores[store].as_array().unwrap_or_else(|| panic!("{show}: no store {store}"));
            let keys = |rs: &[Value]| rs.iter().flat_map(|r| r.as_object().into_iter().flat_map(|o| o.keys().cloned())).collect::<BTreeSet<_>>();
            for k in keys(theirs).difference(&keys(ours)) {
                if !own.contains(k.as_str()) && !own_in.contains(&(store.as_str(), k.as_str())) && k != "access_key" {
                    missing.entry(store.clone()).or_default().insert(k.clone());
                }
            }
            if store != "zwischenpruefungen" && store != "zwischenresultate" && store != "resultate" {
                assert_eq!(ours.len(), theirs.len(), "{show} {store}");
            }
        }
    }
    assert!(missing.is_empty(), "fields Rosson has and the file not: {missing:?}");
}

/// Rosson's finals become rankings over several classes: ranks added (ties first by the first
/// dressage class, else the first), or the mean percentage of dressage classes; numbered as
/// Rosson numbered them.
#[test]
fn rossons_finals_are_rankings_over_classes() {
    let (_, snapshot) = shows().into_iter().find(|(s, _)| s == "2026.8352").unwrap();
    let state = state(&snapshot);
    assert_eq!(state.combined.len(), 5);
    let by_number = |n: &str| state.combined.values().find(|r| r.number.as_deref() == Some(n)).unwrap();
    let jumping = by_number("6.1");
    assert_eq!(jumping.classes.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["108202", "108208"]);
    assert_eq!(jumping.method, show::model::CombinedMethod::Ranks { tie_break: show::model::ClassId("108202".into()) });
    let dressage = by_number("12.1");
    assert_eq!((dressage.name.as_str(), &dressage.method), ("Finale du Championnat vaudois de dressage non-licenciés", &show::model::CombinedMethod::Percentages { tie_break: None }));
    // Each ranks its riders.
    for r in state.combined.values() {
        let lines = state.combined_ranking(&r.id).unwrap();
        assert!(lines.iter().any(|l| l.rank == Some(1)), "{}", r.name);
    }
}

/// Overall ranks where a decided rule ranks otherwise than Rosson did: (show, class, entry, rank).
const DECIDED_RANKS: &[(&str, &str, &str, f64)] = &[
    // Driving, short events: equal dressage, the driven movements decide (docs/deviations.md S42).
    ("2025.8126", "104512", "2861189", 11.0),
    ("2025.8126", "104514", "2863104", 3.0),
];

/// The overall rows of classes made of phases (`typ: 'gesamt'`) hold what Rosson stored at the
/// show: each phase's value, gap and reason for being out, the totals, the rank and the placing.
#[test]
fn overall_rows_as_rosson_stored_them() {
    let mut compared = 0;
    for (show, snapshot) in shows() {
        let stores = rosson_import::to_rosson::event_stores(&state(&snapshot), true, Some("key")).unwrap();
        // Overall rows are matched by class and entry (the starter's entry).
        let entry_of = |s: &Value| -> BTreeMap<String, String> { s["starter"].as_array().into_iter().flatten().map(|r| (r["id"].to_string(), r["nennung_id"].to_string())).collect() };
        let (theirs_entry, ours_entry) = (entry_of(&snapshot), entry_of(&stores));
        let rows = |s: &Value, entry: &BTreeMap<String, String>| -> BTreeMap<(String, String), Value> {
            s["resultate"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|r| r["typ"] == "gesamt")
                .map(|r| ((r["pruefung_id"].as_str().unwrap_or_default().to_string(), entry.get(&r["starter_id"].to_string()).cloned().unwrap_or_default()), r.clone()))
                .collect()
        };
        let (theirs, ours) = (rows(&snapshot, &theirs_entry), rows(&stores, &ours_entry));
        assert_eq!(theirs.keys().collect::<Vec<_>>(), ours.keys().collect::<Vec<_>>(), "{show}: overall rows");
        let number = |v: &Value| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0);
        let numbers = |v: &Value| v.as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), number(v))).collect::<BTreeMap<_, _>>()).unwrap_or_default();
        for (key, t) in &theirs {
            let o = &ours[key];
            let decided = DECIDED_RANKS.iter().find(|(sh, c, e, _)| *sh == show && *c == key.0 && *e == key.1).map(|d| d.3);
            for field in ["strafpunkte_total", "rangpunkte_total", "punkte_total", "rang", "klassiert"] {
                let same = match (field, decided) {
                    ("klassiert", _) => t[field] == o[field],
                    ("rang", Some(rank)) => number(&o[field]) == rank,
                    _ => number(&t[field]) == number(&o[field]),
                };
                assert!(same, "{show} {key:?} {field}: Rosson {} ours {}", t[field], o[field]);
            }
            for field in ["strafpunkte", "rangpunkte", "abweichungen"] {
                assert_eq!(numbers(&t[field]), numbers(&o[field]), "{show} {key:?} {field}");
            }
            // Out of a phase: the same reason (Rosson also notes 0 for some phases ridden).
            let out = |v: &Value| numbers(&v["aufgaben"]).into_iter().filter(|(_, r)| *r > 0.0).collect::<Vec<_>>();
            assert_eq!(out(t), out(o), "{show} {key:?} aufgaben");
            compared += 1;
        }
    }
    assert!(compared > 0, "no overall rows in the corpus");
}
