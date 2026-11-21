//! Changes made apart, put together (R7, R11): start lists rider by rider, classes field by
//! field; what both changed is a conflict, the later change kept.

use show::merge::{self, sequence};
use show::model::*;
use show::oplog::{Change, Operation, Stamp};
use show::state::ConflictKind;
use show::State;

fn seq(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn text(v: &[char]) -> String {
    v.iter().collect()
}

#[test]
fn nothing_meanwhile_gives_the_authors_order() {
    for (seen, mine) in [("abcde", "cabde"), ("abcde", "edcba"), ("abcde", "abdx"), ("abc", "abc"), ("", "ba"), ("abcde", "bcdea")] {
        let m = sequence(&seq(seen), &seq(seen), &seq(mine));
        assert_eq!(text(&m.items), mine, "{seen} → {mine}");
        assert!(m.both.is_empty());
    }
}

#[test]
fn moves_of_different_riders_both_stay() {
    // The author puts e first; meanwhile a went to third place and z came in at the end.
    let m = sequence(&seq("abcde"), &seq("bcadez"), &seq("eabcd"));
    assert_eq!(text(&m.items), "ebcadz");
    assert!(m.both.is_empty());
    // The author put a rider last: after the one added meanwhile, still last.
    assert_eq!(text(&sequence(&seq("abcd"), &seq("dabcz"), &seq("bcda")).items), "dbcza");
    // In the middle: after the rider before it, unless that one moved meanwhile, then before the
    // one after it. (Meanwhile a went to the end.)
    assert_eq!(text(&sequence(&seq("abcdef"), &seq("bcdefa"), &seq("abecdf")).items), "becdfa");
    assert_eq!(text(&sequence(&seq("abcdef"), &seq("bcdefa"), &seq("aebcdf")).items), "ebcdfa");
}

#[test]
fn the_same_rider_moved_on_both_is_a_conflict() {
    // The author puts a last; meanwhile a went after c: the author's place.
    let m = sequence(&seq("abcde"), &seq("bcade"), &seq("bcdea"));
    assert_eq!(text(&m.items), "bcdea");
    assert_eq!(m.both, vec!['a']);
}

#[test]
fn taken_out_on_either_side_stays_out() {
    // The author takes c out; meanwhile d is taken out and z comes in.
    let m = sequence(&seq("abcde"), &seq("abcez"), &seq("abde"));
    assert_eq!(text(&m.items), "abez");
    // The author moves d, which was taken out meanwhile (gone to another section): it stays out.
    let m = sequence(&seq("abcde"), &seq("abce"), &seq("dabce"));
    assert_eq!(text(&m.items), "abce");
}

fn start(entry: u64, order: u32, number: Option<u32>) -> Start {
    Start { entry: EntryId(entry), order, start_number: number, team_order: None }
}

fn list(entries: &[u64]) -> Vec<Start> {
    entries.iter().enumerate().map(|(i, e)| start(*e, i as u32 + 1, None)).collect()
}

fn numbered(entries: &[u64], first: u32) -> Vec<Start> {
    entries.iter().enumerate().map(|(i, e)| start(*e, i as u32 + 1, Some(first + i as u32))).collect()
}

fn entries(l: &[Start]) -> Vec<(u64, Option<u32>)> {
    let mut l = l.to_vec();
    l.sort_by_key(|s| s.order);
    l.iter().map(|s| (s.entry.0, s.start_number)).collect()
}

#[test]
fn numbers_rider_by_rider() {
    let seen = list(&[1, 2, 3]);
    // Numbers given here; meanwhile rider 3 moved first there: both kept.
    let m = merge::starts(&seen, &list(&[3, 1, 2]), &numbered(&[1, 2, 3], 1), false);
    assert_eq!(entries(&m.starts), vec![(3, Some(3)), (1, Some(1)), (2, Some(2))]);
    assert!(m.numbers_both.is_empty() && m.moved_both.is_empty());
    // Numbers given on both: the author's kept, the others' listed.
    let m = merge::starts(&seen, &numbered(&[1, 2, 3], 1), &numbered(&[1, 2, 3], 101), false);
    assert_eq!(entries(&m.starts), vec![(1, Some(101)), (2, Some(102)), (3, Some(103))]);
    assert_eq!(m.numbers_both, vec![(EntryId(1), Some(1)), (EntryId(2), Some(2)), (EntryId(3), Some(3))]);
}

fn class(name: &str) -> Class {
    Class {
        id: ClassId("1".into()),
        published: PublishedClassId(1),
        number: "1".into(),
        name: name.into(),
        date: "2026-11-20".into(),
        scoring_table: Some(3),
        programme: None,
        options: ClassOptions::default(),
        status: ClassStatus::Planned,
        extra_fields: None,
        position: Some(1),
        time_text: "08h00".into(),
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

#[test]
fn a_class_field_by_field() {
    let seen = class("Libre");
    let current = Class { time_text: "10h00".into(), options: ClassOptions { placed_count: Some(5), ..ClassOptions::default() }, ..seen.clone() };
    let mine = Class { name: "Libre R/N".into(), options: ClassOptions { ideal_time: Some(60.into()), ..ClassOptions::default() }, ..seen.clone() };
    let m = merge::class(&seen, &current, &mine);
    assert_eq!((m.class.name.as_str(), m.class.time_text.as_str(), m.class.options.placed_count, m.class.options.ideal_time), ("Libre R/N", "10h00", Some(5), Some(60.into())));
    assert!(m.both.is_empty());
    // The same field: the author's kept, the other value listed.
    let m = merge::class(&seen, &Class { name: "Libre du PC".into(), ..seen.clone() }, &mine);
    assert_eq!(m.class.name, "Libre R/N");
    assert_eq!(m.both, vec![("name".to_string(), serde_json::json!("Libre du PC"))]);
}

fn op(id: &str, millis: u64, device: &str, change: Change) -> Operation {
    Operation { id: id.into(), stamp: Stamp { millis, counter: 0, device: device.into() }, author: device.into(), change }
}

fn set_class(c: Class, seen: Option<Class>) -> Change {
    Change::ClassSet { class: c, seen }
}

fn set_starts(starts: Vec<Start>, seen: Vec<Start>) -> Change {
    Change::StartsSet { class: ClassId("1".into()), starts, seen: Some(seen), next_numbers: Vec::new(), numbered: false }
}

/// Every device, whatever order the operations came in, ends with the same state and conflicts.
fn fold(ops: &[Operation]) -> State {
    let (state, _) = State::from_log(ops.to_vec());
    let (back, _) = State::from_log(ops.iter().rev().cloned().collect());
    assert_eq!(state.classes, back.classes);
    assert_eq!(state.starts, back.starts);
    assert_eq!(state.conflicts, back.conflicts);
    state
}

#[test]
fn renamed_on_two_devices_the_later_name_and_a_conflict() {
    let base = class("Libre");
    let ops = vec![
        op("set", 1, "pc", set_class(base.clone(), None)),
        op("pc:name", 10, "pc", set_class(Class { name: "Nom du PC".into(), ..base.clone() }, Some(base.clone()))),
        op("mac:time", 15, "mac", set_class(Class { time_text: "13h00".into(), ..base.clone() }, Some(base.clone()))),
        op("mac:name", 20, "mac", set_class(Class { name: "Nom du Mac".into(), time_text: "13h00".into(), ..base.clone() }, Some(Class { time_text: "13h00".into(), ..base.clone() }))),
    ];
    let state = fold(&ops);
    let c = &state.classes[&ClassId("1".into())];
    assert_eq!((c.name.as_str(), c.time_text.as_str()), ("Nom du Mac", "13h00"));
    assert_eq!(state.conflicts.len(), 1);
    let k = &state.conflicts[0];
    assert_eq!((k.op.as_str(), k.kind, k.over.as_deref(), k.what.clone()), ("mac:name", ConflictKind::ClassEdited, Some("pc:name"), vec!["name".to_string()]));
    assert_eq!(k.lost, serde_json::json!({ "name": "Nom du PC" }));
}

#[test]
fn a_late_entry_and_a_numbering_made_apart() {
    let base = class("Libre");
    let seen = list(&[1, 2, 3]);
    let mut late = numbered(&[1, 2, 3], 1);
    late.push(start(9, 4, Some(4)));
    let ops = vec![
        op("set", 1, "pc", set_class(base.clone(), None)),
        op("starts", 2, "pc", Change::StartsSet { class: ClassId("1".into()), starts: seen.clone(), seen: None, next_numbers: Vec::new(), numbered: false }),
        op("pc:numbers", 10, "pc", Change::StartsSet { class: ClassId("1".into()), starts: numbered(&[1, 2, 3], 1), seen: Some(seen.clone()), next_numbers: Vec::new(), numbered: true }),
        op("pc:late", 11, "pc", Change::StartsSet { class: ClassId("1".into()), starts: late, seen: Some(numbered(&[1, 2, 3], 1)), next_numbers: vec![EntryId(9)], numbered: false }),
        op("mac:move", 15, "mac", set_starts(list(&[3, 1, 2]), seen.clone())),
        op("mac:numbers", 20, "mac", Change::StartsSet { class: ClassId("1".into()), starts: numbered(&[3, 1, 2], 101), seen: Some(list(&[3, 1, 2])), next_numbers: Vec::new(), numbered: true }),
    ];
    let mut ops = ops;
    // Numbers are given in the class (as a numbering's class change says).
    ops.insert(2, op("given", 3, "pc", set_class(Class { start_numbers_assigned: true, ..base.clone() }, Some(base.clone()))));
    let state = fold(&ops);
    // The Mac's order and numbers; the late entry kept, at the end, with the next number.
    assert_eq!(entries(&state.starts[&ClassId("1".into())]), vec![(3, Some(101)), (1, Some(102)), (2, Some(103)), (9, Some(104))]);
    let kinds: Vec<_> = state.conflicts.iter().map(|c| (c.op.as_str(), c.kind, c.over.as_deref())).collect();
    assert_eq!(kinds, vec![("mac:numbers", ConflictKind::StartNumbers, Some("pc:numbers"))]);
}
