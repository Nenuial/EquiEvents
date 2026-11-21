//! The tablet import's rules where Rosson got them wrong (see `timing::tablet`), on a real class:
//! 2026.8298 P9, programme "TdJ Einste" (three errors of 2 points each), judges at C and M.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use rust_decimal::Decimal;
use serde_json::Value;
use show::model::{AssignmentId, ClassId, EntryId, JudgeSheet};
use show::oplog::Change;
use show::State;
use timing::tablet::{import_tablet, read_tablet, tablet_sheet, Sheet};
use timing::Cell;

fn class() -> (State, ClassId) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus/2026.8298/snapshot.json");
    let snapshot: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let (state, _) = State::from_log(rosson_import::ops::to_operations(&snapshot).unwrap());
    let class = state.classes.values().find(|c| c.number == "9").unwrap().id.clone();
    (state, class)
}

/// The exported sheet as editable text cells.
fn cells(sheet: &Sheet) -> (Vec<String>, Vec<Vec<String>>) {
    let text = |c: &Cell| match c {
        Cell::Text(s) => s.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Empty => String::new(),
    };
    (sheet.head.clone(), sheet.rows.iter().map(|r| r.iter().map(text).collect()).collect())
}

/// Tab-separated, LF line ends (what Rosson could not read).
fn file(head: &[String], rows: &[Vec<String>]) -> Vec<u8> {
    let mut lines = vec![head.join("\t")];
    lines.extend(rows.iter().map(|r| r.join("\t")));
    lines.join("\n").into_bytes()
}

fn set(head: &[String], row: &mut [String], column: &str, value: &str) {
    let i = head.iter().position(|h| h == column).unwrap_or_else(|| panic!("no column {column}"));
    row[i] = value.to_string();
}

/// Status OK and every movement marked `mark`.
fn fill(head: &[String], row: &mut [String], mark: &str) {
    set(head, row, "status", "OK");
    for (i, h) in head.iter().enumerate() {
        if h.starts_with('V') {
            row[i] = mark.to_string();
        }
    }
}

fn imported(state: &State, class: &ClassId, bytes: &[u8]) -> (Vec<Change>, Vec<String>) {
    let result = import_tablet(state, class, &read_tablet(bytes).unwrap()).unwrap();
    (result.changes, result.notes)
}

fn sheets(changes: &[Change]) -> BTreeMap<(EntryId, AssignmentId), JudgeSheet> {
    changes.iter().filter_map(|c| if let Change::JudgeSheetEntered { entry, judge, sheet, .. } = c { Some(((entry.clone(), judge.clone()), sheet.clone())) } else { None }).collect()
}

fn outs(changes: &[Change]) -> BTreeMap<EntryId, Option<u32>> {
    changes.iter().filter_map(|c| if let Change::ResultEntered { entry, out, .. } = c { Some((entry.clone(), *out)) } else { None }).collect()
}

fn entry_of(state: &State, class: &ClassId, head: &[String], row: &[String]) -> EntryId {
    let column = |c: &str| row[head.iter().position(|h| h == c).unwrap()].parse::<u64>().unwrap();
    let (rider, horse) = (column("rider_id"), column("horse_id"));
    state.starts[class].iter().map(|s| &state.entries[&s.entry]).find(|e| e.rider.0 == rider && e.horse.as_ref().map(|h| h.0) == Some(horse)).unwrap().id.clone()
}

#[test]
fn text_files_with_lf_line_ends_are_read_completely() {
    let (state, class) = class();
    let (head, mut rows) = cells(&tablet_sheet(&state, &class).unwrap());
    for row in &mut rows {
        fill(&head, row, "7");
    }
    let (changes, notes) = imported(&state, &class, &file(&head, &rows));
    let judges = state.dressage_judges(&class).len();
    assert_eq!(judges, 2);
    assert_eq!(outs(&changes).len(), rows.len() / judges, "every rider");
    assert_eq!(sheets(&changes).len(), rows.len(), "every judge's sheet");
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn a_missing_row_does_not_shift_the_following_marks() {
    let (state, class) = class();
    let (head, mut rows) = cells(&tablet_sheet(&state, &class).unwrap());
    let judges: Vec<AssignmentId> = state.dressage_judges(&class).iter().map(|j| j.id.clone()).collect();
    // Rider 1: only the judge at C sent a row. Rider 2: C gives 6, M gives 8.
    fill(&head, &mut rows[0], "7");
    rows.remove(1);
    fill(&head, &mut rows[1], "6");
    fill(&head, &mut rows[2], "8");
    let (first, second) = (entry_of(&state, &class, &head, &rows[0]), entry_of(&state, &class, &head, &rows[1]));
    let (changes, _) = imported(&state, &class, &file(&head, &rows[..3]));
    let sheets = sheets(&changes);
    assert_eq!(sheets.len(), 3);
    assert_eq!(sheets[&(second.clone(), judges[0].clone())].marks["1"], Decimal::from(6));
    assert_eq!(sheets[&(second, judges[1].clone())].marks["1"], Decimal::from(8));
    assert!(!sheets.contains_key(&(first, judges[1].clone())), "rider 1's sheet at M is left as it was");
}

#[test]
fn the_status_belongs_to_its_rider() {
    let (state, class) = class();
    let (head, mut rows) = cells(&tablet_sheet(&state, &class).unwrap());
    // Rider 1 eliminated; rider 2 marked but without status (Rosson: eliminated as well).
    set(&head, &mut rows[0], "status", "DISQUAL");
    set(&head, &mut rows[1], "status", "DISQUAL");
    fill(&head, &mut rows[2], "7");
    fill(&head, &mut rows[3], "7");
    set(&head, &mut rows[2], "status", "");
    set(&head, &mut rows[3], "status", "");
    let (first, second) = (entry_of(&state, &class, &head, &rows[0]), entry_of(&state, &class, &head, &rows[2]));
    let (changes, notes) = imported(&state, &class, &file(&head, &rows[..4]));
    let outs = outs(&changes);
    assert!(outs[&first].is_some(), "rider 1 is out");
    assert!(!outs.contains_key(&second), "rider 2 is not read");
    assert!(notes.iter().any(|n| n.contains("marks but no status")), "{notes:?}");
}

#[test]
fn rows_that_disagree_on_the_status_make_the_rider_out_and_are_reported() {
    let (state, class) = class();
    let (head, mut rows) = cells(&tablet_sheet(&state, &class).unwrap());
    fill(&head, &mut rows[0], "7");
    fill(&head, &mut rows[1], "7");
    set(&head, &mut rows[1], "status", "RET");
    let (changes, notes) = imported(&state, &class, &file(&head, &rows[..2]));
    assert!(outs(&changes).values().all(Option::is_some));
    assert!(notes.iter().any(|n| n.contains("different statuses")), "{notes:?}");
}

#[test]
fn one_error_is_one_penalty_even_when_the_next_costs_the_same() {
    let (state, class) = class();
    let (head, mut rows) = cells(&tablet_sheet(&state, &class).unwrap());
    fill(&head, &mut rows[0], "7");
    fill(&head, &mut rows[1], "7");
    set(&head, &mut rows[0], "penalty_1", "2");
    let (changes, notes) = imported(&state, &class, &file(&head, &rows[..2]));
    let sheets: Vec<JudgeSheet> = sheets(&changes).into_values().collect();
    // One penalty, not two; and one set for every judge (docs/deviations.md S40).
    assert_eq!(sheets.iter().map(|s| s.penalty_points.clone()).collect::<Vec<_>>(), vec![vec![Decimal::from(2)], vec![Decimal::from(2)]]);
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn values_that_do_not_fit_are_reported_not_guessed() {
    let (state, class) = class();
    let (head, mut rows) = cells(&tablet_sheet(&state, &class).unwrap());
    fill(&head, &mut rows[0], "7,5");
    set(&head, &mut rows[0], "V2", "11");
    set(&head, &mut rows[0], "V3", "gut");
    set(&head, &mut rows[0], "penalty_2", "5");
    let (changes, notes) = imported(&state, &class, &file(&head, &rows[..1]));
    let sheet = sheets(&changes).into_values().next().unwrap();
    assert_eq!(sheet.marks["1"], Decimal::new(75, 1), "a comma as decimal separator");
    assert!(!sheet.marks.contains_key("2") && !sheet.marks.contains_key("3"));
    assert!(sheet.penalty_points.is_empty());
    assert!(notes.iter().any(|n| n.contains("V2") && n.contains("not a mark")), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("V3")), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("penalty_2") && n.contains("not the value")), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("no mark for movement 2, 3")), "{notes:?}");
}

#[test]
fn rows_not_judged_yet_change_nothing() {
    let (state, class) = class();
    let (head, rows) = cells(&tablet_sheet(&state, &class).unwrap());
    let (changes, notes) = imported(&state, &class, &file(&head, &rows));
    assert!(changes.is_empty() && notes.is_empty(), "{changes:?} {notes:?}");
}
