//! The dressage tablet file against Rosson 3.3.15's own (golden/corpus/<show>/timing/*-tablet*,
//! made by golden/src/timing-fixtures.mjs): for every dressage class of every corpus show,
//!
//! - our export has the same cells (and cell types) as Rosson's;
//! - importing the filled-in file (text and Excel) gives, per rider, the status, and per judge the
//!   marks and penalties Rosson reads from it — except where Rosson is wrong by design
//!   (penalties ticked by value, see `timing::tablet`), which is reported as a note.

use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::Path;

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use rust_decimal::Decimal;
use serde_json::Value;
use show::master::{self, PenaltyValue};
use show::model::JudgeSheet;
use show::oplog::Change;
use show::State;
use timing::decimal;
use timing::tablet::{import_tablet, read_tablet, tablet_sheet, tablet_xlsx};

/// Sheet names and cells of an Excel file; empty text counts as an empty cell.
fn grid(bytes: &[u8]) -> Vec<(String, Vec<Vec<Data>>)> {
    let mut book = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec())).unwrap();
    book.sheet_names()
        .into_iter()
        .map(|name| {
            let range = book.worksheet_range(&name).unwrap();
            let rows = range
                .rows()
                .map(|r| {
                    let mut cells: Vec<Data> = r.iter().map(|d| if matches!(d, Data::String(s) if s.is_empty()) { Data::Empty } else { d.clone() }).collect();
                    while cells.last() == Some(&Data::Empty) {
                        cells.pop();
                    }
                    cells
                })
                .collect();
            (name, rows)
        })
        .collect()
}

/// Per judge: marks by movement, and the ticked penalties' values (sorted).
type Judged = BTreeMap<String, (BTreeMap<String, Decimal>, Vec<Decimal>)>;
/// What Rosson read for a rider: status, judges, and per judge the ticked penalties' positions.
type Expected = (Option<u32>, Judged, BTreeMap<String, Vec<String>>);

fn ours(sheet: &JudgeSheet) -> (BTreeMap<String, Decimal>, Vec<Decimal>) {
    let mut penalties: Vec<Decimal> = sheet.penalty_points.iter().chain(&sheet.penalty_percent).map(|d| d.normalize()).collect();
    penalties.sort();
    (sheet.marks.iter().map(|(k, v)| (k.clone(), v.normalize())).collect(), penalties)
}

#[test]
fn tablet_files_match_rosson() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let mut problems: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let (mut classes, mut riders, mut sheets) = (0, 0, 0);

    let mut shows: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("timing").is_dir()).collect();
    shows.sort();
    for show in shows {
        let snapshot: Value = serde_json::from_str(&fs::read_to_string(show.join("snapshot.json")).unwrap()).unwrap();
        let (state, _) = State::from_log(rosson_import::ops::to_operations(&snapshot).unwrap());
        let label = show.file_name().unwrap().to_string_lossy().to_string();

        let mut files: Vec<_> = fs::read_dir(show.join("timing")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.to_string_lossy().ends_with("-tablet.xlsx")).collect();
        files.sort();
        for file in files {
            let stem = file.file_name().unwrap().to_string_lossy().trim_end_matches("-tablet.xlsx").to_string();
            let Some(class) = state.classes.values().find(|c| c.number.replace(|ch: char| !(ch.is_alphanumeric() || ch == '.' || ch == '-' || ch == '_'), "_") == stem) else {
                problems.push(format!("{label} {stem}: class not found"));
                continue;
            };
            classes += 1;
            let at = |what: &str| format!("{label} P{} {what}", class.number);

            // 1. Export, cell by cell.
            let theirs = grid(&fs::read(&file).unwrap());
            let sheet = tablet_sheet(&state, &class.id).unwrap();
            let ours_grid = grid(&tablet_xlsx(&sheet, &class.number).unwrap());
            if theirs.iter().map(|(n, _)| n).ne(ours_grid.iter().map(|(n, _)| n)) {
                problems.push(format!("{}: sheet names: Rosson {:?}, ours {:?}", at("export"), theirs.iter().map(|(n, _)| n).collect::<Vec<_>>(), ours_grid.iter().map(|(n, _)| n).collect::<Vec<_>>()));
            }
            let (a, b) = (&theirs[0].1, &ours_grid[0].1);
            if a != b {
                let i = a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
                problems.push(format!("{}: row {} of {}/{}\n      Rosson: {:?}\n      ours:   {:?}", at("export"), i + 1, a.len(), b.len(), a.get(i), b.get(i)));
            }

            // 2. Import, text and Excel.
            let rosson: Value = serde_json::from_str(&fs::read_to_string(show.join("timing").join(format!("{stem}-tablet-imported.json"))).unwrap()).unwrap();
            for format in ["tsv", "xlsx"] {
                let bytes = fs::read(show.join("timing").join(format!("{stem}-tablet-results.{format}"))).unwrap();
                let result = import_tablet(&state, &class.id, &read_tablet(&bytes).unwrap()).unwrap();
                for n in &result.notes {
                    notes.push(format!("{}: {n}", at(&format!("import ({format})"))));
                }
                let mut read: BTreeMap<String, (Option<u32>, Judged)> = BTreeMap::new();
                for change in &result.changes {
                    match change {
                        Change::ResultEntered { entry, out, .. } => {
                            read.entry(entry.0.to_string()).or_default().0 = *out;
                        }
                        Change::JudgeSheetEntered { entry, judge, sheet, .. } => {
                            read.entry(entry.0.to_string()).or_default().1.insert(judge.0.clone(), ours(sheet));
                            sheets += 1;
                        }
                        other => problems.push(format!("{}: unexpected change {other:?}", at("import"))),
                    }
                }

                // What Rosson read: riders with a status or anything entered.
                let mut expected: BTreeMap<String, Expected> = BTreeMap::new();
                for (entry, v) in rosson["imported"][format].as_object().unwrap() {
                    let out = v["out"].as_u64().map(|o| o as u32);
                    let mut judged = Judged::new();
                    let mut positions = BTreeMap::new();
                    for (judge, j) in v["judges"].as_object().unwrap() {
                        let marks: BTreeMap<String, Decimal> = j["marks"].as_object().unwrap().iter().filter_map(|(k, m)| Some((k.clone(), decimal(m.as_str()?)?.normalize()))).collect();
                        let mut penalties: Vec<Decimal> = j["penalties"].as_object().unwrap().values().filter_map(|p| Some(decimal(p.as_str()?)?.normalize())).collect();
                        penalties.sort();
                        positions.insert(judge.clone(), j["penalties"].as_object().unwrap().keys().cloned().collect());
                        if !marks.is_empty() || !penalties.is_empty() {
                            judged.insert(judge.clone(), (marks, penalties));
                        }
                    }
                    if out.is_some() || !judged.is_empty() {
                        expected.insert(entry.clone(), (out, judged, positions));
                    }
                }

                for entry in expected.keys().filter(|e| !read.contains_key(*e)) {
                    problems.push(format!("{}: entry {entry}: Rosson read it, we did not", at(&format!("import ({format})"))));
                }
                for (entry, (out, judged)) in &read {
                    riders += 1;
                    let Some((their_out, their_judged, positions)) = expected.get(entry) else {
                        problems.push(format!("{}: entry {entry}: we read it, Rosson did not", at(&format!("import ({format})"))));
                        continue;
                    };
                    if out != their_out {
                        problems.push(format!("{}: entry {entry}: out Rosson {their_out:?}, ours {out:?}", at(&format!("import ({format})"))));
                    }
                    for judge in their_judged.keys().chain(judged.keys()).collect::<std::collections::BTreeSet<_>>() {
                        let (theirs, ours) = (their_judged.get(judge), judged.get(judge));
                        if theirs == ours {
                            continue;
                        }
                        // Rosson ticks every penalty with a given value; explained when the marks agree
                        // and Rosson's ticked penalties are exactly those whose value we read.
                        // One set of penalties for every judge (docs/deviations.md S40): ours are what any
                        // of Rosson's judges ticked, each value as often as the judge who ticked it most.
                        let one_set = {
                            let mut values: Vec<Decimal> = their_judged.values().flat_map(|(_, p)| p.iter().copied()).collect();
                            values.sort();
                            values.dedup();
                            let mut all: Vec<Decimal> = values.iter().flat_map(|v| std::iter::repeat_n(*v, their_judged.values().map(|(_, p)| p.iter().filter(|x| *x == v).count()).max().unwrap_or(0))).collect();
                            all.sort();
                            all
                        };
                        if let (Some((tm, _)), Some((om, op))) = (theirs, ours) {
                            if tm == om && *op == one_set {
                                notes.push(format!("{}: entry {entry}, judge {judge}: penalties {op:?}, one set for every judge (S40); Rosson: {:?}", at(&format!("import ({format})")), theirs.unwrap().1));
                                continue;
                            }
                        }
                        let explained = match (theirs, ours) {
                            (Some((tm, _)), Some((om, op))) if tm == om => {
                                let def = state.entries.get(&show::model::EntryId(entry.parse().unwrap())).and_then(|e| {
                                    let class_def = class.programme.and_then(|p| state.programmes.get(&p))?;
                                    if master::is_individual(class_def) { e.programme.and_then(|p| state.programmes.get(&p)) } else { Some(class_def) }
                                });
                                let tables = def.map(|d| master::penalty_tables(d).unwrap()).unwrap_or_default();
                                let by_value: Vec<String> = tables
                                    .iter()
                                    .enumerate()
                                    .flat_map(|(t, fields)| fields.iter().enumerate().map(move |(i, f)| (t, i, f)))
                                    .filter(|(_, _, f)| matches!(f.value, Some(PenaltyValue::Points(v) | PenaltyValue::Percent(v)) if op.contains(&v.normalize())))
                                    .map(|(t, i, _)| format!("{}_{}", t + 1, i + 1))
                                    .collect();
                                positions.get(judge.as_str()).is_some_and(|p: &Vec<String>| *p == by_value)
                            }
                            _ => false,
                        };
                        if explained {
                            notes.push(format!("{}: entry {entry}, judge {judge}: penalties {:?}; Rosson ticked every penalty with that value: {:?}", at(&format!("import ({format})")), ours.unwrap().1, theirs.unwrap().1));
                        } else {
                            problems.push(format!("{}: entry {entry}, judge {judge}\n      Rosson: {theirs:?}\n      ours:   {ours:?}", at(&format!("import ({format})"))));
                        }
                    }
                }
            }
        }
    }

    println!("\n{classes} classes, {riders} riders and {sheets} judge sheets imported, {} problems", problems.len());
    for p in problems.iter().take(40) {
        println!("  {p}");
    }
    println!("Notes ({}):", notes.len());
    for n in notes.iter().take(60) {
        println!("  {n}");
    }
    assert!(classes >= 20, "expected the corpus fixtures (golden/src/timing-fixtures.mjs)");
    assert!(problems.is_empty(), "{} differences with Rosson's tablet files", problems.len());
}
