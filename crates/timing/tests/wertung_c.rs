//! Wertung C (J10, docs/regulations-check.md): a round with a time and no faults field. The time
//! the timekeeper sends includes the penalty seconds; a faults column is not read, as Rosson
//! (`importSpringen` reads the table's visible fields only), and the import says so when it is
//! filled in.

use std::fs;
use std::path::Path;

use rust_decimal::Decimal;
use serde_json::Value;
use show::oplog::Change;
use show::State;
use timing::{import, read_results};

#[test]
fn a_faults_column_in_a_wertung_c_round_is_not_read_but_said() {
    // 2025.8118 P5: table 40, Wertung C.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus/2025.8118");
    let snapshot: Value = serde_json::from_str(&fs::read_to_string(dir.join("snapshot.json")).unwrap()).unwrap();
    let (state, _) = State::from_log(rosson_import::ops::to_operations(&snapshot).unwrap());
    let class = state.classes.values().find(|c| c.number == "5").unwrap().id.clone();
    let mut rows: Vec<timing::Row> = read_results(&fs::read(dir.join("timing/5-results.tsv")).unwrap()).unwrap().into_iter().filter(|r| r.cells.get("Aufgabe").is_some_and(|a| a == "OK")).take(2).collect();
    let time: Decimal = rows[0].cells["Zeit NP"].parse().unwrap();
    rows[0].cells.insert("Strafpunkte NP".into(), "4".into());

    let imported = import(&state, &class, &rows).unwrap();
    let values: Vec<_> = imported.changes.iter().filter_map(|c| if let Change::ResultEntered { values, .. } = c { Some(values.clone()) } else { None }).collect();
    assert_eq!(values[0].keys().collect::<Vec<_>>(), ["zeit_initial_1"], "only the time is read");
    assert_eq!(values[0]["zeit_initial_1"], time, "the time as sent, nothing added");
    assert_eq!(imported.notes.len(), 1, "{:?}", imported.notes);
    assert!(imported.notes[0].contains("Strafpunkte NP 4: not read"), "{:?}", imported.notes);
}
