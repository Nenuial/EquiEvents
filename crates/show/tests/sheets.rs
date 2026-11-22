//! Judges' sheets: marks entered a few at a time, sheets sent to another rider or judge,
//! swapped, put aside and placed again; nothing is ever lost.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use show::model::*;
use show::oplog::{Change, Clock, Operation};
use show::State;

fn class() -> ClassId {
    ClassId("12".into())
}

fn apply(state: &mut State, clock: &mut Clock, change: Change) {
    let stamp = clock.tick(1);
    state.apply(&Operation { id: format!("op{}", stamp.counter), stamp, author: "office".into(), change, saw: Default::default() });
}

fn marks(pairs: &[(&str, Option<i64>)]) -> BTreeMap<String, Option<Decimal>> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.map(Decimal::from))).collect()
}

fn at(entry: u64, judge: &str) -> SheetPlace {
    SheetPlace::Rider { entry: EntryId(entry), judge: AssignmentId(judge.into()) }
}

fn sheet<'a>(state: &'a State, entry: u64, judge: &str) -> Option<&'a JudgeSheet> {
    state.sheets.get(&(class(), EntryId(entry), AssignmentId(judge.into())))
}

#[test]
fn sheets_are_moved_swapped_and_put_aside() {
    let mut state = State::default();
    let mut clock = Clock::new("office");
    let edit = |entry: u64, judge: &str, m: &[(&str, Option<i64>)]| Change::SheetEdited { total: None, class: class(), entry: EntryId(entry), judge: AssignmentId(judge.into()), marks: marks(m), penalties: None };

    // Marks of rider 1, judge C, entered one by one; one corrected, one cleared.
    apply(&mut state, &mut clock, edit(1, "C", &[("1", Some(6))]));
    apply(&mut state, &mut clock, edit(1, "C", &[("2", Some(7)), ("3", Some(5))]));
    apply(&mut state, &mut clock, edit(1, "C", &[("3", None)]));
    assert_eq!(sheet(&state, 1, "C").unwrap().marks.len(), 2);

    // They were rider 2's: sent there (nothing there yet).
    apply(&mut state, &mut clock, Change::SheetMoved { class: class(), from: at(1, "C"), to: at(2, "C") });
    assert!(sheet(&state, 1, "C").is_none());
    assert_eq!(sheet(&state, 2, "C").unwrap().marks[&"1".to_string()], Decimal::from(6));

    // Judge H's sheet of rider 2 and judge C's were the wrong way round: swapped.
    apply(&mut state, &mut clock, edit(2, "H", &[("1", Some(8))]));
    apply(&mut state, &mut clock, Change::SheetMoved { class: class(), from: at(2, "C"), to: at(2, "H") });
    assert_eq!(sheet(&state, 2, "H").unwrap().marks[&"1".to_string()], Decimal::from(6));
    assert_eq!(sheet(&state, 2, "C").unwrap().marks[&"1".to_string()], Decimal::from(8));

    // Put aside, then placed on rider 3 for judge M.
    apply(&mut state, &mut clock, Change::SheetMoved { class: class(), from: at(2, "C"), to: SheetPlace::Aside { slot: 1 } });
    assert!(sheet(&state, 2, "C").is_none());
    assert_eq!(state.aside.len(), 1);
    apply(&mut state, &mut clock, Change::SheetMoved { class: class(), from: SheetPlace::Aside { slot: 1 }, to: at(3, "M") });
    assert!(state.aside.is_empty());
    assert_eq!(sheet(&state, 3, "M").unwrap().marks[&"1".to_string()], Decimal::from(8));

    // Clearing every mark removes the sheet.
    apply(&mut state, &mut clock, edit(3, "M", &[("1", None)]));
    assert!(sheet(&state, 3, "M").is_none());
}
