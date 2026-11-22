//! Anything changed on two computers apart is said (R12): an entry's horse, its status, an
//! official's role, a judge's mark… The later change is applied, and both are told what it
//! replaced (the user, 2026-11-22: two computers offline gave a rider two different horses; the
//! later stayed, no one was told).

use std::collections::BTreeMap;

use serde_json::json;
use show::model::*;
use show::oplog::{Change, Operation, Stamp};
use show::state::ConflictKind;
use show::touched::Thing;
use show::State;

fn entry(id: u64) -> Entry {
    serde_json::from_value(json!({
        "id": id, "published": 1, "rider": 10, "horse": 20, "guest_horse": null, "status": "Entered",
        "hors_concours": false, "team": null, "programme": null
    }))
    .unwrap()
}

/// The show both computers had before they went offline.
fn start() -> Vec<Operation> {
    vec![Operation {
        id: "import".into(),
        stamp: Stamp { millis: 1, counter: 0, device: "import".into() },
        author: "import".into(),
        change: Change::EntryAdded { entry: entry(5) },
        saw: Default::default(),
    }]
}

/// A change made on `device` at `millis`, as the office makes it: with what it saw then.
fn made(seen: &State, id: &str, millis: u64, device: &str, change: Change) -> Operation {
    let saw = seen.saw(std::slice::from_ref(&change)).remove(0);
    Operation { id: id.into(), stamp: Stamp { millis, counter: 0, device: device.into() }, author: device.into(), change, saw }
}

fn horse(id: u64) -> Change {
    Change::EntryEdited { entry: EntryId(5), edit: EntryEdit { horse: Some(HorseOnEntry { horse: Some(HorseId(id)), guest_horse: None, horses_on_entry: Vec::new() }), ..Default::default() } }
}

fn status(s: EntryStatus) -> Change {
    Change::EntryEdited { entry: EntryId(5), edit: EntryEdit { status: Some(s), ..Default::default() } }
}

fn running_class() -> Class {
    Class {
        id: ClassId("1".into()),
        published: PublishedClassId(1),
        number: "1".into(),
        name: "Libre".into(),
        date: "2026-11-20".into(),
        scoring_table: None,
        programme: None,
        options: ClassOptions::default(),
        status: ClassStatus::Running,
        extra_fields: None,
        position: Some(1),
        time_text: String::new(),
        scoring_text: None,
        logo: None,
        start_numbers_assigned: false,
        teams: Vec::new(),
        start_list: StartListSetup::default(),
        sponsor: String::new(),
        phase: None,
        phases: Vec::new(),
    }
}

/// Everything, in whatever order it arrives: the same state and conflicts.
fn together(ops: Vec<Operation>) -> State {
    let mut reversed = ops.clone();
    reversed.reverse();
    let (a, _) = State::from_log(ops);
    let (b, _) = State::from_log(reversed);
    assert_eq!(a, b);
    a
}

#[test]
fn two_horses_for_one_rider_is_said() {
    let (before, _) = State::from_log(start());
    // Offline, the PC gives horse 21, then the Mac horse 22.
    let pc = made(&before, "pc", 100, "pc", horse(21));
    let mac = made(&before, "mac", 200, "mac", horse(22));
    let mut ops = start();
    ops.extend([pc, mac]);
    let state = together(ops);
    // The later one stays…
    assert_eq!(state.entries[&EntryId(5)].horse, Some(HorseId(22)));
    // … and it is a conflict: the Mac's change went over the PC's, horse 21 replaced.
    assert_eq!(state.conflicts.len(), 1, "{:#?}", state.conflicts);
    let c = &state.conflicts[0];
    assert_eq!((c.kind, c.op.as_str(), c.over.as_deref()), (ConflictKind::Changed, "mac", Some("pc")));
    assert_eq!(c.things, vec![Thing::EntryField { entry: EntryId(5), field: "horse".into() }]);
    assert_eq!(c.lost["e/5/horse"]["horse"], json!(21));
    // Putting the PC's horse back: the change made gives horse 21 again.
    let back = c.things[0].restore(&c.lost["e/5/horse"]).unwrap();
    assert_eq!(back, horse(21));
}

#[test]
fn the_same_horse_on_both_or_different_fields_is_no_clash() {
    let (before, _) = State::from_log(start());
    let mut ops = start();
    // Both give horse 21: nothing was replaced.
    ops.extend([made(&before, "pc", 100, "pc", horse(21)), made(&before, "mac", 200, "mac", horse(21))]);
    // One withdraws the rider, the other notes something: two fields, both stay.
    ops.extend([
        made(&before, "pc2", 300, "pc", status(EntryStatus::NotStarting)),
        made(&before, "mac2", 400, "mac", Change::EntryEdited { entry: EntryId(5), edit: EntryEdit { comment: Some("Arrive tard".into()), ..Default::default() } }),
    ]);
    let state = together(ops);
    assert!(state.conflicts.is_empty(), "{:#?}", state.conflicts);
    let e = &state.entries[&EntryId(5)];
    assert_eq!((e.status, e.comment.as_str()), (EntryStatus::NotStarting, "Arrive tard"));
}

#[test]
fn a_change_seen_before_is_no_clash() {
    // The Mac saw the PC's horse (synced), then changed it: a change, not a clash.
    let (before, _) = State::from_log(start());
    let pc = made(&before, "pc", 100, "pc", horse(21));
    let mut ops = start();
    ops.push(pc);
    let (synced, _) = State::from_log(ops.clone());
    ops.push(made(&synced, "mac", 200, "mac", horse(22)));
    assert!(together(ops).conflicts.is_empty());
}

#[test]
fn a_change_to_an_entry_taken_back_meanwhile_is_said() {
    let (before, _) = State::from_log(start());
    // The Mac takes the entry back; later the PC, offline, changes its horse: not applied, said.
    let mut ops = start();
    ops.extend([made(&before, "mac", 100, "mac", Change::EntryRemoved { entry: EntryId(5) }), made(&before, "pc", 200, "pc", horse(21))]);
    let state = together(ops);
    assert!(!state.entries.contains_key(&EntryId(5)));
    assert_eq!(state.conflicts.len(), 1, "{:#?}", state.conflicts);
    assert_eq!((state.conflicts[0].kind, state.conflicts[0].op.as_str(), state.conflicts[0].over.as_deref()), (ConflictKind::ChangedGone, "pc", Some("mac")));
    // The other way round: the entry taken back after the PC changed it, the change lost: said.
    let mut ops = start();
    ops.extend([made(&before, "pc", 100, "pc", horse(21)), made(&before, "mac", 200, "mac", Change::EntryRemoved { entry: EntryId(5) })]);
    let state = together(ops);
    assert_eq!(state.conflicts.len(), 1, "{:#?}", state.conflicts);
    assert_eq!((state.conflicts[0].kind, state.conflicts[0].over.as_deref()), (ConflictKind::Changed, Some("pc")));
}

#[test]
fn officials_marks_and_ranks_too() {
    let class = ClassId("1".into());
    let judge = AssignmentId("j1".into());
    let assignment = |role: &str| Assignment {
        id: judge.clone(),
        class: Some(class.clone()),
        person: PersonId(30),
        role: role.into(),
        letter: Some("C".into()),
        role_id: None,
        event: None,
        se_id: None,
        on_event: None,
        responsible: None,
        readonly: None,
        days: None,
        updated_at: None,
        looked_up: false,
    };
    let mut base = start();
    base.push(Operation {
        id: "class".into(),
        stamp: Stamp { millis: 2, counter: 0, device: "import".into() },
        author: "import".into(),
        change: Change::ClassSet { class: running_class(), seen: None },
        saw: Default::default(),
    });
    base.push(Operation { id: "judge".into(), stamp: Stamp { millis: 3, counter: 0, device: "import".into() }, author: "import".into(), change: Change::AssignmentSet { assignment: assignment("DRI") }, saw: Default::default() });
    let (before, _) = State::from_log(base.clone());
    let mark = |v: i64| Change::SheetEdited { class: class.clone(), entry: EntryId(5), judge: judge.clone(), marks: BTreeMap::from([("1".to_string(), Some(v.into()))]), penalties: None, total: None };
    let mut ops = base.clone();
    ops.extend([
        made(&before, "pc", 100, "pc", Change::AssignmentSet { assignment: assignment("DRI-P") }),
        made(&before, "mac", 200, "mac", Change::AssignmentSet { assignment: assignment("DRA") }),
        made(&before, "pc2", 300, "pc", mark(7)),
        made(&before, "mac2", 400, "mac", mark(8)),
        // Another mark of the same sheet: no clash.
        made(&before, "mac3", 500, "mac", Change::SheetEdited { class: class.clone(), entry: EntryId(5), judge: judge.clone(), marks: BTreeMap::from([("2".to_string(), Some(6.into()))]), penalties: None, total: None }),
    ]);
    let state = together(ops);
    let kinds: Vec<(&str, Vec<String>)> = state.conflicts.iter().map(|c| (c.op.as_str(), c.what.clone())).collect();
    assert_eq!(kinds, vec![("mac", vec!["a/j1".to_string()]), ("mac2", vec!["s/1/5/j1/m/1".to_string()])]);
}

#[test]
fn a_mark_for_a_class_finalised_meanwhile_is_said() {
    let class = ClassId("1".into());
    let mut base = start();
    base.push(Operation {
        id: "class".into(),
        stamp: Stamp { millis: 2, counter: 0, device: "import".into() },
        author: "import".into(),
        change: Change::ClassSet { class: running_class(), seen: None },
        saw: Default::default(),
    });
    let (before, _) = State::from_log(base.clone());
    let mut ops = base;
    // The Mac finalises the class; later, the PC (offline, not knowing) enters a mark.
    ops.push(made(&before, "final", 100, "mac", Change::ClassFinalised { class: class.clone(), based_on: Vec::new() }));
    ops.push(made(&before, "pc", 200, "pc", Change::SheetEdited { class: class.clone(), entry: EntryId(5), judge: AssignmentId("j1".into()), marks: BTreeMap::from([("1".to_string(), Some(7.into()))]), penalties: None, total: None }));
    let state = together(ops);
    assert!(state.sheets.is_empty());
    assert_eq!(state.conflicts.iter().map(|c| (c.kind, c.op.as_str())).collect::<Vec<_>>(), vec![(ConflictKind::ChangedGone, "pc")]);
}
