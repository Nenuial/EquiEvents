//! The guarantees of the operation log and the reconciliation rules
//! (docs/architecture/reconciliation.md): finalised classes are protected, sync is idempotent,
//! devices converge whatever order operations arrive in, conflicts are recorded.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use show::model::*;
use show::oplog::{too_far_ahead, Change, Clock, FieldChange, Operation, Stamp};
use show::state::ConflictKind;
use show::{State, Store};

fn op(id: &str, stamp: Stamp, change: Change) -> Operation {
    Operation { id: id.into(), stamp, author: "test".into(), change }
}

fn class() -> Class {
    Class {
        id: ClassId("1".into()),
        published: PublishedClassId(1),
        number: "1".into(),
        name: "Test".into(),
        date: "2026-11-13".into(),
        scoring_table: Some(14),
        programme: None,
        options: ClassOptions::default(),
        status: ClassStatus::Running,
        extra_fields: None,
        position: None,
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

fn result(faults: i64) -> Change {
    Change::ResultEntered {
        class: ClassId("1".into()),
        entry: EntryId(7),
        values: BTreeMap::from([("strafpunkte_initial_1".to_string(), Decimal::from(faults))]),
        out: None,
    }
}

fn faults(state: &State) -> Option<Decimal> {
    state.results.get(&(ClassId("1".into()), EntryId(7))).and_then(|r| r.values.get("strafpunkte_initial_1")).copied()
}

#[test]
fn clock_never_goes_backwards() {
    let mut c = Clock::new("office");
    let a = c.tick(1000);
    let b = c.tick(900); // wall clock went back
    let d = c.tick(1000);
    assert!(a < b && b < d);
    c.observe(&Stamp { millis: 5000, counter: 3, device: "judge".into() });
    assert!(c.tick(1200) > Stamp { millis: 5000, counter: 3, device: "judge".into() });
}

#[test]
fn a_final_class_ignores_changes_until_reopened() {
    let mut c = Clock::new("office");
    let ops = vec![
        op("a", c.tick(1), Change::class_set(class())),
        op("b", c.tick(2), result(4)),
        op("c", c.tick(3), Change::ClassFinalised { class: ClassId("1".into()), based_on: vec!["b".into()] }),
        op("d", c.tick(4), result(8)),
        op("e", c.tick(5), Change::ClassReopened { class: ClassId("1".into()), reason: "protest upheld".into() }),
        op("f", c.tick(6), result(0)),
    ];
    let (state, ignored) = State::from_log(ops[..4].to_vec());
    assert_eq!(faults(&state), Some(Decimal::from(4)));
    assert_eq!(ignored.len(), 1);
    assert_eq!(ignored[0].0, "d");

    let (state, _) = State::from_log(ops);
    assert_eq!(faults(&state), Some(Decimal::from(0)));
}

#[test]
fn devices_converge_whatever_order_operations_arrive_in() {
    let mut office = Clock::new("office");
    let mut judge = Clock::new("judge");
    let ops = vec![
        op("a", office.tick(1), Change::class_set(class())),
        op("b", judge.tick(2), result(4)),
        op("c", office.tick(3), result(8)),
    ];
    let dir = tempfile::tempdir().unwrap();

    let mut in_order = Store::open(&dir.path().join("a.show")).unwrap();
    in_order.append(&ops).unwrap();

    let mut late = Store::open(&dir.path().join("b.show")).unwrap();
    late.append(&[ops[0].clone(), ops[2].clone()]).unwrap();
    late.append(&[ops[1].clone()]).unwrap(); // made earlier offline, arrives later
    late.append(&ops).unwrap(); // and everything again: idempotent

    assert_eq!(faults(in_order.state()), Some(Decimal::from(8)));
    assert_eq!(in_order.state().results, late.state().results);
    assert_eq!(late.log().unwrap().len(), 3);

    // Reopening the file rebuilds the same state from the log.
    drop(late);
    let reopened = Store::open(&dir.path().join("b.show")).unwrap();
    assert_eq!(reopened.state().results, in_order.state().results);
}

fn edit(field: &str, from: Option<i64>, to: Option<i64>) -> Change {
    Change::ResultEdited {
        class: ClassId("1".into()),
        entry: EntryId(7),
        changes: vec![FieldChange { field: field.into(), from: from.map(Decimal::from), to: to.map(Decimal::from) }],
    }
}

fn value(state: &State, field: &str) -> Option<Decimal> {
    state.results.get(&(ClassId("1".into()), EntryId(7))).and_then(|r| r.values.get(field)).copied()
}

/// R1: the timekeeper changes the time, the office the faults, at the same time: both apply.
#[test]
fn edits_of_different_fields_both_apply() {
    let mut office = Clock::new("office");
    let mut timing = Clock::new("timing");
    let setup = op("a", office.tick(1), Change::class_set(class()));
    let faults = op("b", office.tick(10), edit("strafpunkte_initial_1", None, Some(4)));
    let time = op("c", timing.tick(10), edit("zeit_initial_1", None, Some(61)));
    for ops in [vec![setup.clone(), faults.clone(), time.clone()], vec![setup.clone(), time.clone(), faults.clone()]] {
        let (state, _) = State::from_log(ops);
        assert_eq!(value(&state, "strafpunkte_initial_1"), Some(Decimal::from(4)));
        assert_eq!(value(&state, "zeit_initial_1"), Some(Decimal::from(61)));
        assert!(state.conflicts.is_empty());
    }
}

/// R2: both change the faults from 0: the later stamp wins on every device, and it is a conflict.
#[test]
fn edits_of_the_same_field_later_wins_and_is_a_conflict() {
    let mut office = Clock::new("office");
    let mut timing = Clock::new("timing");
    let ops = vec![
        op("a", office.tick(1), Change::class_set(class())),
        op("b", office.tick(2), edit("strafpunkte_initial_1", None, Some(0))),
        op("c", timing.tick(10), edit("strafpunkte_initial_1", Some(0), Some(4))),
        op("d", office.tick(11), edit("strafpunkte_initial_1", Some(0), Some(8))),
    ];
    let mut reversed = ops.clone();
    reversed.reverse();
    for ops in [ops, reversed] {
        let (state, _) = State::from_log(ops);
        assert_eq!(value(&state, "strafpunkte_initial_1"), Some(Decimal::from(8)));
        assert_eq!(state.conflicts.len(), 1);
        assert_eq!(state.conflicts[0].kind, ConflictKind::ConcurrentEdit);
        assert_eq!(state.conflicts[0].op, "d");
        assert!(state.conflicts[0].detail.contains("author saw 0, was 4, now 8"), "{}", state.conflicts[0].detail);
    }
}

/// R4: a device offline before the class was finalised syncs later: its change is not applied
/// and is listed, on every device and whatever the arrival order.
#[test]
fn a_late_change_does_not_alter_a_final_class() {
    let mut office = Clock::new("office");
    let mut judge = Clock::new("judge");
    let setup = op("a", office.tick(1), Change::class_set(class()));
    let known = op("b", office.tick(2), result(4));
    let offline = op("c", judge.tick(3), result(8)); // made before the finalisation, synced after
    let mut office_state = State::from_log(vec![setup.clone(), known.clone()]).0;
    let finalised = op("d", office.tick(4), office_state.finalise(&ClassId("1".into())));
    office_state.apply(&finalised);

    let dir = tempfile::tempdir().unwrap();
    for (name, arrivals) in [
        ("hub.show", vec![vec![setup.clone(), known.clone(), finalised.clone()], vec![offline.clone()]]),
        ("judge.show", vec![vec![setup.clone(), offline.clone()], vec![known.clone(), finalised.clone()]]),
    ] {
        let mut store = Store::open(&dir.path().join(name)).unwrap();
        for batch in arrivals {
            store.append(&batch).unwrap();
        }
        assert_eq!(faults(store.state()), Some(Decimal::from(4)), "{name}");
        assert_eq!(store.state().conflicts.iter().map(|c| (c.op.as_str(), c.kind)).collect::<Vec<_>>(), vec![("c", ConflictKind::LateForFinalClass)], "{name}");
    }
}

/// R5: the hub refuses operations from a device whose clock is far ahead.
#[test]
fn operations_from_the_future_are_refused() {
    let now = 1_000_000_000;
    let ok = op("a", Stamp { millis: now + 60_000, counter: 0, device: "tablet".into() }, result(0));
    let bad = op("b", Stamp { millis: now + 3_600_000, counter: 0, device: "tablet".into() }, result(0));
    assert_eq!(too_far_ahead(&[ok, bad], now), vec!["b".to_string()]);
}

#[test]
fn ranks_set_by_hand_follow_the_rules_for_results() {
    let mut c = Clock::new("office");
    let set = |rank: Option<u32>| Change::RankSet { class: ClassId("1".into()), competitor: Competitor::Entry(EntryId(7)), rank, reason: "jury decision".into() };
    let manual = |state: &State| state.manual_ranks.get(&(ClassId("1".into()), Competitor::Entry(EntryId(7)))).map(|m| m.rank);
    let ops = vec![
        op("a", c.tick(1), Change::class_set(class())),
        op("b", c.tick(2), set(Some(3))),
        op("c", c.tick(3), Change::ClassFinalised { class: ClassId("1".into()), based_on: vec!["b".into()] }),
        op("d", c.tick(4), set(Some(1))),
    ];
    let (state, ignored) = State::from_log(ops.clone());
    assert_eq!(manual(&state), Some(3), "a final class keeps its ranks (R3)");
    assert_eq!(ignored.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), vec!["d"]);

    // Removed after reopening.
    let mut more = ops;
    more.push(op("e", c.tick(5), Change::ClassReopened { class: ClassId("1".into()), reason: "protest".into() }));
    more.push(op("f", c.tick(6), set(None)));
    let (state, _) = State::from_log(more);
    assert_eq!(manual(&state), None);

    // R4: a rank set before the finalisation, but unknown to it, is not applied.
    let mut late = Clock::new("tablet");
    let ops = vec![
        op("a", c.tick(10), Change::class_set(class())),
        op("x", late.tick(11), set(Some(2))),
        op("c", c.tick(12), Change::ClassFinalised { class: ClassId("1".into()), based_on: vec![] }),
    ];
    let (state, _) = State::from_log(ops);
    assert_eq!(manual(&state), None);
    assert_eq!(state.conflicts.len(), 1);
}

/// The file knows which operations the server has: the others wait, also after a restart; what
/// comes from the server is the server's; a version-1 file is brought up to date.
#[test]
fn the_file_remembers_what_went_up() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.show");
    let stamp = |millis: u64| Stamp { millis, counter: 0, device: "laptop".into() };
    {
        let mut store = Store::open(&path).unwrap();
        store.append(&[op("a", stamp(1), Change::class_set(class())), op("b", stamp(2), result(4))]).unwrap();
        assert_eq!(store.unsynced_count().unwrap(), 2);
        store.mark_synced(&["a".to_string()]).unwrap();
        // From the server: one new, one known (made here, now the server's too).
        assert_eq!(store.receive(&[op("b", stamp(2), result(4)), op("c", stamp(3), result(8))]).unwrap(), 1);
        assert_eq!(faults(store.state()), Some(Decimal::from(8)));
        store.append(&[op("d", stamp(4), result(12))]).unwrap();
        store.set_meta("remote", Some("2026.8352-abcd")).unwrap();
        store.set_meta("cursor", Some("17")).unwrap();
        store.set_meta("cursor", Some("18")).unwrap();
    }
    let mut store = Store::open(&path).unwrap();
    assert_eq!(store.unsynced(10).unwrap().iter().map(|o| o.id.as_str()).collect::<Vec<_>>(), ["d"]);
    assert_eq!((store.meta("remote").unwrap().as_deref(), store.meta("cursor").unwrap().as_deref()), (Some("2026.8352-abcd"), Some("18")));
    store.set_meta("cursor", None).unwrap();
    assert_eq!(store.meta("cursor").unwrap(), None);
    store.mark_all_unsynced().unwrap();
    assert_eq!(store.unsynced_count().unwrap(), 4);

    // A file of the first version: opened, its operations wait to go up.
    let old = dir.path().join("old.show");
    {
        let conn = rusqlite::Connection::open(&old).unwrap();
        conn.execute_batch(
            "CREATE TABLE operations (id TEXT PRIMARY KEY, millis INTEGER NOT NULL, counter INTEGER NOT NULL, device TEXT NOT NULL, author TEXT NOT NULL, change TEXT NOT NULL);
             CREATE INDEX operations_by_stamp ON operations (millis, counter, device);
             PRAGMA user_version = 1;",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO operations VALUES ('x', 1, 0, 'laptop', 'office', ?1)",
            [serde_json::to_string(&Change::class_set(class())).unwrap()],
        )
        .unwrap();
    }
    let store = Store::open(&old).unwrap();
    assert_eq!((store.unsynced_count().unwrap(), store.state().classes.len()), (1, 1));
}
