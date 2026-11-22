//! Taking the timekeeper's ranks over (Rosson's "Rang übernehmen", docs/deviations.md S18),
//! against Rosson 3.3.15 (golden/corpus/<show>/timing/<class>-ranks*, made by
//! golden/src/timing-fixtures.mjs): in every individual jumping class the last-ranked rider is
//! sent as 1st in the timekeeper's file.
//!
//! 1. The rule: our `with_manual_ranks` applied to Rosson's ranking without the rank gives
//!    Rosson's ranking with it.
//! 2. The import: reading the file with `adopt_ranks` sets exactly that rank by hand.
//! 3. The whole way: import, then the class's ranking, equals Rosson's wherever our ranking
//!    without the rank already equals Rosson's (elsewhere a decided rule differs, see the golden
//!    tests).
//!
//! Where the rider sent as 1st was out, Rosson does not move the others (E32); that is checked
//! and reported, not compared.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use scoring::ranking::{with_manual_ranks, Placement};
use serde_json::Value;
use show::model::{Competitor, EntryId};
use show::oplog::{Change, Operation, Stamp};
use show::State;
use timing::{import_with, read_results, Options, TIMEKEEPER_RANK};

const ROSSON_OUT: u64 = 99;

/// Rosson's ranks as a listing: by rank, out last.
fn listing(ranks: &Value) -> Vec<(String, Placement)> {
    let mut l: Vec<(String, Placement)> = ranks
        .as_object()
        .unwrap()
        .iter()
        .filter_map(|(id, r)| {
            let rank = r["rank"].as_u64()?;
            Some((id.clone(), if rank == ROSSON_OUT { Placement::Out } else { Placement::Rank(rank as u32) }))
        })
        .collect();
    l.sort_by_key(|(id, p)| (*p, id.clone()));
    l
}

fn ranks(l: &[(String, Placement)]) -> BTreeMap<String, Placement> {
    l.iter().cloned().collect()
}

#[test]
fn timekeeper_ranks_taken_over_as_rosson() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let mut problems: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let (mut classes, mut whole_way, mut differing) = (0, 0, Vec::new());

    let mut shows: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("timing").is_dir()).collect();
    shows.sort();
    for show in shows {
        let snapshot: Value = serde_json::from_str(&fs::read_to_string(show.join("snapshot.json")).unwrap()).unwrap();
        let (state, _) = State::from_log(rosson_import::ops::to_operations(&snapshot).unwrap());
        let label = show.file_name().unwrap().to_string_lossy().to_string();
        let mut files: Vec<_> = fs::read_dir(show.join("timing")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.to_string_lossy().ends_with("-ranks.tsv")).collect();
        files.sort();
        for file in files {
            let stem = file.file_name().unwrap().to_string_lossy().trim_end_matches("-ranks.tsv").to_string();
            let class = state.classes.values().find(|c| c.number.replace(|ch: char| !(ch.is_alphanumeric() || ch == '.' || ch == '-' || ch == '_'), "_") == stem).unwrap();
            let at = format!("{label} P{}", class.number);
            classes += 1;
            let rosson: Value = serde_json::from_str(&fs::read_to_string(show.join("timing").join(format!("{stem}-ranks-imported.json"))).unwrap()).unwrap();
            let first = rosson["rank_1"].as_str().unwrap().to_string();
            let manual = BTreeMap::from([(first.clone(), 1)]);
            let (their_before, their_after) = (listing(&rosson["before"]), listing(&rosson["after"]));

            // 1. The rule.
            let by_rule = with_manual_ranks(&their_before, &manual);
            let was_out = ranks(&their_before).get(&first) == Some(&Placement::Out);
            if was_out {
                // Rosson collects the ranks set by hand only among riders who are ranked: one set
                // for a rider who is out is not skipped by the others (docs/deviations.md E32).
                let others_unchanged = their_before.iter().filter(|(id, _)| *id != first).all(|(id, p)| ranks(&their_after).get(id) == Some(p));
                if others_unchanged && ranks(&their_after).get(&first) == Some(&Placement::Rank(1)) {
                    notes.push(format!("{at}: rider {first} was out and is set to 1; Rosson leaves everyone else as they were, two riders are 1st (E32); ours: the others move down"));
                } else {
                    problems.push(format!("{at}: rider {first} was out; Rosson's ranking is not explained by E32"));
                }
            } else if ranks(&by_rule) != ranks(&their_after) {
                let diff: Vec<String> = by_rule.iter().filter(|(id, p)| ranks(&their_after).get(id) != Some(p)).map(|(id, p)| format!("{id}: ours {p:?}, Rosson {:?}", ranks(&their_after).get(id))).collect();
                problems.push(format!("{at}: rule: {}", diff.join("; ")));
            }

            // 2. The import.
            let rows = read_results(&fs::read(&file).unwrap()).unwrap();
            let imported = import_with(&state, &class.id, &rows, Options { adopt_ranks: true }).unwrap();
            let set: Vec<&Change> = imported.changes.iter().filter(|c| matches!(c, Change::RankSet { .. })).collect();
            let expected = Change::RankSet { class: class.id.clone(), competitor: Competitor::Entry(EntryId(first.parse().unwrap())), rank: Some(1), reason: TIMEKEEPER_RANK.into() };
            if set != vec![&expected] {
                problems.push(format!("{at}: import set {set:?}"));
            }
            if import_with(&state, &class.id, &rows, Options::default()).unwrap().changes.iter().any(|c| matches!(c, Change::RankSet { .. })) {
                problems.push(format!("{at}: ranks taken over without the option"));
            }

            // 3. The whole way: the class's results as read from the file, then its ranking.
            let mut ours = state.clone();
            ours.results.retain(|(c, _), _| c != &class.id);
            let apply = |state: &mut State, changes: &[Change]| {
                let start = state.last.as_ref().map_or(0, |s| s.counter + 1);
                for (i, change) in changes.iter().enumerate() {
                    let counter = start + i as u32;
                    let outcome = state.apply(&Operation { id: format!("t{counter}"), stamp: Stamp { millis: 1, counter, device: "test".into() }, author: "test".into(), change: change.clone(), saw: Default::default() });
                    assert_eq!(outcome, show::state::Outcome::Applied);
                }
            };
            // The corpus classes are final; the office imports into a running class.
            let mut first_changes = vec![Change::ClassReopened { class: class.id.clone(), reason: "test".into() }];
            first_changes.extend(imported.changes.iter().filter(|c| !matches!(c, Change::RankSet { .. })).cloned());
            apply(&mut ours, &first_changes);
            let as_listing = |state: &State| -> Vec<(String, Placement)> { state.ranking(&class.id).unwrap().lines.into_iter().map(|l| (l.id, l.placement)).collect() };
            let our_before = as_listing(&ours);
            apply(&mut ours, &set.into_iter().cloned().collect::<Vec<_>>());
            let our_after = as_listing(&ours);
            match ours.ranking(&class.id).unwrap().lines.into_iter().find(|l| l.id == first) {
                Some(line) if line.placement == Placement::Rank(1) && line.manual => {}
                other => problems.push(format!("{at}: rider {first}: {other:?}")),
            }
            if was_out {
                differing.push(format!("{at} (E32)"));
            } else if ranks(&our_before) == ranks(&their_before) {
                whole_way += 1;
                if ranks(&our_after) != ranks(&their_after) {
                    problems.push(format!("{at}: after taking the rank over, ours differs from Rosson's"));
                }
            } else {
                differing.push(at);
            }
        }
    }

    println!("\n{classes} classes; the whole way compared in {whole_way}; {} problems", problems.len());
    println!("Our ranking differs from Rosson's before any rank is taken over (decided rules, see the golden tests): {}", differing.join(", "));
    for p in &problems {
        println!("  {p}");
    }
    println!("Notes ({}):", notes.len());
    for n in &notes {
        println!("  {n}");
    }
    assert!(classes > 40, "expected the corpus fixtures (golden/src/timing-fixtures.mjs)");
    assert!(problems.is_empty(), "{} problems", problems.len());
}

/// 2026.8352 P1 with its timekeeper file, and the rows of that file.
fn p1() -> (State, show::model::ClassId, Vec<timing::Row>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus/2026.8352");
    let snapshot: Value = serde_json::from_str(&fs::read_to_string(dir.join("snapshot.json")).unwrap()).unwrap();
    let (state, _) = State::from_log(rosson_import::ops::to_operations(&snapshot).unwrap());
    let class = state.classes.values().find(|c| c.number == "1").unwrap().id.clone();
    // Riders with a result (an empty Aufgabe is not read).
    let rows = read_results(&fs::read(dir.join("timing/1-results.tsv")).unwrap()).unwrap().into_iter().filter(|r| r.cells.get("Aufgabe").is_some_and(|a| a == "OK")).collect();
    (state, class, rows)
}

fn rank_changes(changes: &[Change]) -> Vec<(EntryId, Option<u32>)> {
    changes.iter().filter_map(|c| if let Change::RankSet { competitor: Competitor::Entry(e), rank, .. } = c { Some((e.clone(), *rank)) } else { None }).collect()
}

#[test]
fn ranks_written_as_ordinals_are_read_and_others_reported() {
    let (state, class, mut rows) = p1();
    rows[0].cells.insert("Rang".into(), "2.".into());
    rows[1].cells.insert("Rang".into(), "zweiter".into());
    let imported = import_with(&state, &class, &rows[..2], Options { adopt_ranks: true }).unwrap();
    assert_eq!(rank_changes(&imported.changes).iter().map(|(_, r)| *r).collect::<Vec<_>>(), vec![Some(2)]);
    assert!(imported.notes.iter().any(|n| n.contains("\"zweiter\" is not a rank")), "{:?}", imported.notes);
}

#[test]
fn an_empty_rank_removes_a_timekeeper_rank_but_not_the_offices() {
    let (mut state, class, rows) = p1();
    let rows = &rows[..2];
    let entries: Vec<EntryId> = import_with(&state, &class, rows, Options::default()).unwrap().changes.iter().filter_map(|c| if let Change::ResultEntered { entry, .. } = c { Some(entry.clone()) } else { None }).collect();
    // Earlier: rider 1 ranked 3rd from a timekeeper's file, rider 2 ranked 5th by the jury.
    state.manual_ranks.insert((class.clone(), Competitor::Entry(entries[0].clone())), show::model::ManualRank { rank: 3, reason: TIMEKEEPER_RANK.into() });
    state.manual_ranks.insert((class.clone(), Competitor::Entry(entries[1].clone())), show::model::ManualRank { rank: 5, reason: "jury decision".into() });
    // Now a file without ranks.
    let imported = import_with(&state, &class, rows, Options { adopt_ranks: true }).unwrap();
    assert_eq!(rank_changes(&imported.changes), vec![(entries[0].clone(), None)]);
}
