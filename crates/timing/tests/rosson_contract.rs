//! The timekeeping files against Rosson 3.3.15's own (golden/corpus/<show>/timing/, made by
//! golden/src/timing-fixtures.mjs): for every show-jumping class of every corpus show,
//!
//! - our start list as text is byte for byte Rosson's;
//! - our start list as Excel has the same cells as Rosson's;
//! - importing a results file (text and Excel) gives the score cards Rosson reads from it —
//!   except where the same rider and horse start more than once in the class: Rosson put the
//!   results on the first matching entry; we go by the start number and check against the
//!   results stored at the show instead.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use rust_decimal::Decimal;
use serde_json::Value;
use show::oplog::Change;
use show::State;
use timing::{decimal, import, read_results, start_list, to_text, to_xlsx};

#[test]
fn files_match_rosson() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let mut problems: Vec<String> = Vec::new();
    let mut classes = 0;
    let mut imported_cards = 0;
    let mut notes: Vec<String> = Vec::new();

    let mut shows: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("timing").is_dir()).collect();
    shows.sort();
    for show in shows {
        let snapshot: Value = serde_json::from_str(&fs::read_to_string(show.join("snapshot.json")).unwrap()).unwrap();
        let (state, _) = State::from_log(rosson_import::ops::to_operations(&snapshot).unwrap());
        let label = show.file_name().unwrap().to_string_lossy().to_string();

        let mut files: Vec<_> = fs::read_dir(show.join("timing")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "tsv") && !p.file_stem().unwrap().to_string_lossy().contains('-')).collect();
        files.sort();
        for tsv in files {
            let stem = tsv.file_stem().unwrap().to_string_lossy().to_string();
            let Some(class) = state.classes.values().find(|c| c.number.replace(|ch: char| !(ch.is_alphanumeric() || ch == '.' || ch == '-' || ch == '_'), "_") == stem) else {
                problems.push(format!("{label} {stem}: class not found"));
                continue;
            };
            classes += 1;
            let at = |what: &str| format!("{label} P{} {what}", class.number);
            let rows = start_list(&state, &class.id).unwrap();

            // 1. Text start list, byte for byte.
            let theirs = fs::read(&tsv).unwrap();
            let ours = to_text(&rows);
            if ours != theirs {
                let (a, b) = (String::from_utf8_lossy(&theirs).to_string(), String::from_utf8_lossy(&ours).to_string());
                let line = a.lines().zip(b.lines()).position(|(x, y)| x != y).unwrap_or(a.lines().count().min(b.lines().count()));
                problems.push(format!("{}: line {}\n      Rosson: {:?}\n      ours:   {:?}", at("start list (text)"), line + 1, a.lines().nth(line), b.lines().nth(line)));
            }

            // 2. Excel start list, cell by cell.
            let theirs = read_results(&fs::read(tsv.with_extension("xlsx")).unwrap()).unwrap();
            let ours = read_results(&to_xlsx(&rows, &class.number).unwrap()).unwrap();
            if ours != theirs {
                let i = theirs.iter().zip(&ours).position(|(x, y)| x != y).unwrap_or(0);
                problems.push(format!("{}: row {}\n      Rosson: {:?}\n      ours:   {:?}", at("start list (Excel)"), i + 2, theirs.get(i), ours.get(i)));
            }

            // 3. Importing the results file.
            // Entries whose rider and horse start more than once in the class, and what was stored for them.
            let starts = &state.starts[&class.id];
            let pair = |e: &show::model::EntryId| state.entries.get(e).map(|x| (x.rider.clone(), x.horse.clone()));
            let ambiguous: Vec<String> = starts.iter().filter(|s| starts.iter().filter(|o| pair(&o.entry) == pair(&s.entry)).count() > 1).map(|s| s.entry.0.to_string()).collect();
            let stored = |entry: &str| -> BTreeMap<String, Decimal> {
                let starter_ids: Vec<&Value> = snapshot["starter"].as_array().unwrap().iter().filter(|x| rosson_import::id(&x["nennung_id"]) == entry && rosson_import::id(&x["pruefung_id"]) == class.id.0).map(|x| &x["id"]).collect();
                snapshot["resultate"].as_array().unwrap().iter().filter(|r| starter_ids.contains(&&r["starter_id"])).flat_map(|r| r["details"].as_object().into_iter().flatten()).filter(|(k, _)| k.starts_with("zeit_") || k.starts_with("strafpunkte_")).filter_map(|(k, v)| Some((k.clone(), rosson_import::decimal(v)?))).collect()
            };
            let rosson: Value = serde_json::from_str(&fs::read_to_string(show.join("timing").join(format!("{stem}-imported.json"))).unwrap()).unwrap();
            for format in ["tsv", "xlsx"] {
                let file = show.join("timing").join(format!("{stem}-results.{format}"));
                let result = import(&state, &class.id, &read_results(&fs::read(&file).unwrap()).unwrap()).unwrap();
                let ours: Vec<String> = result.changes.iter().filter_map(|c| if let Change::ResultEntered { entry, .. } = c { Some(entry.0.to_string()) } else { None }).collect();
                // Rosson keeps an empty result for every starter; only those with values were read.
                let read = |v: &Value| v["values"].as_object().is_some_and(|m| !m.is_empty()) || !v["out"].is_null();
                for theirs in rosson["imported"][format].as_object().unwrap().iter().filter(|(e, v)| read(v) && !ours.contains(e) && !ambiguous.contains(e)).map(|(e, _)| e) {
                    problems.push(format!("{}: entry {theirs}: Rosson read it, we did not", at(&format!("import ({format})"))));
                }
                for change in &result.changes {
                    let Change::ResultEntered { entry, values, out, .. } = change else { continue };
                    imported_cards += 1;
                    let mut expected = &rosson["imported"][format][entry.0.to_string()];
                    if expected.is_null() && format == "tsv" && !rosson["imported"]["xlsx"][entry.0.to_string()].is_null() {
                        // Rosson does not decode Windows-1252 (docs/deviations.md E21): a guest horse
                        // whose name has e.g. a curly apostrophe is never found in a text file.
                        notes.push(format!("{}: entry {entry}: Rosson read nothing from the text file (E21, horse name not decoded); ours is what Rosson reads from the Excel file", at("import (tsv)")));
                        expected = &rosson["imported"]["xlsx"][entry.0.to_string()];
                    }
                    let mut expected_values: BTreeMap<String, Decimal> = expected["values"].as_object().into_iter().flatten().filter_map(|(k, v)| Some((k.clone(), decimal(v.as_str()?)?))).collect();
                    let expected_out = expected["out"].as_u64().map(|o| o as u32);
                    let norm = |m: &BTreeMap<String, Decimal>| m.iter().map(|(k, v)| (k.clone(), v.normalize())).collect::<BTreeMap<_, _>>();
                    if ambiguous.contains(&entry.0.to_string()) {
                        expected_values = stored(&entry.0.to_string());
                        if norm(values) != norm(&expected_values) {
                            problems.push(format!("{}: entry {entry} (rider and horse start twice): stored {:?}, ours {:?}", at(&format!("import ({format})")), norm(&expected_values), norm(values)));
                        } else {
                            notes.push(format!("{}: entry {entry}: rider and horse start twice; results put on the right entry by start number (Rosson used the first match)", at(&format!("import ({format})"))));
                        }
                        continue;
                    }
                    if norm(values) != norm(&expected_values) || *out != expected_out {
                        problems.push(format!("{}: entry {entry}\n      Rosson: {:?} out {:?}\n      ours:   {:?} out {:?}", at(&format!("import ({format})")), norm(&expected_values), expected_out, norm(values), out));
                    }
                }
                for note in &result.notes {
                    notes.push(format!("{}: {note}", at(&format!("import ({format})"))));
                }
            }
        }
    }

    println!("\n{classes} classes, {imported_cards} score cards imported, {} problems", problems.len());
    for p in problems.iter().take(40) {
        println!("  {p}");
    }
    println!("Notes for the show office ({}):", notes.len());
    for n in &notes {
        println!("  {n}");
    }
    assert!(classes > 50, "expected the corpus fixtures (golden/src/timing-fixtures.mjs)");
    assert!(problems.is_empty(), "{} differences with Rosson's timekeeping files", problems.len());
}
