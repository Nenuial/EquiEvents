//! `putResultate` against Rosson 3.3.15's own uploads (golden/corpus/<show>/api/results-*.json):
//! for every class of every corpus show, our request has the same fields with the same values.
//!
//! - Ranks (`rang`, `klassiert`, the placed count) come from `show::ranking`, which the golden
//!   tests check; they may differ from Rosson's only in the classes of [`DECIDED_RANKINGS`].
//! - Values Rosson sends sometimes as text, sometimes as numbers (entered results, reasons)
//!   compare by value; the type differences are counted.
//! - The order of riders within the list is not compared (Rosson orders ties differently, S4),
//!   nor the order of judges within a rider (Rosson's follows the order marks were typed in).
//! - Where Rosson writes "nothing" in several ways, they are the same: a reason of 0, "" or none
//!   (not out); an entered value of "" or none (not entered).
//! - `updated_at` is a time: it differs by nature (in the fixtures, the time of the replay).
//! - The percentage per judge is exact; Rosson's is off by 0.01 where floating point puts a
//!   value just below a half (60.625 → "60.62", decided 2026-11-13: fixed). Such differences are
//!   recognised by recomputing Rosson's value ([`rosson_percent`]) and counted.
//! - The ghost team of 2025.8161 P8 (docs/deviations.md S15) is in Rosson's upload, not in ours;
//!   team 14061 of 2026.8352 P7 is out by the team rule (S7), Rosson still ranked it.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

mod common;
use common::{diff, ranking_field, Counts, DECIDED_RANKINGS};
use se_api::results::put_resultate;
use se_api::Keys;
use serde_json::Value;
use show::State;

/// A result line's identity: team, or rider and horse (and team).
fn identity(r: &Value) -> String {
    if r["team_resultat"] == true {
        format!("team {}", r["team_id"])
    } else {
        format!("rider {} horse {} team {}", r["reiter_id"], r["pferd_id"], r.get("team_id").unwrap_or(&Value::Null))
    }
}

#[test]
fn results_uploads_match_rosson() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let keys = Keys { access_key: "REDACTED".into(), rosson_key: "<rosson_key>".into(), rosson_version: "3.3.15".into() };
    let mut problems: Vec<String> = Vec::new();
    let mut by_field: BTreeMap<String, usize> = BTreeMap::new();
    let mut ranking_differences: BTreeMap<String, usize> = BTreeMap::new();
    let (mut classes, mut lines) = (0, 0);
    let mut counts = Counts::default();

    let mut shows: Vec<_> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("api").is_dir()).collect();
    shows.sort();
    for show in shows {
        let snapshot: Value = serde_json::from_str(&fs::read_to_string(show.join("snapshot.json")).unwrap()).unwrap();
        let (state, _) = State::from_log(rosson_import::ops::to_operations(&snapshot).unwrap());
        let label = show.file_name().unwrap().to_string_lossy().to_string();
        let mut files: Vec<_> = fs::read_dir(show.join("api")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("results-")).collect();
        files.sort();
        for file in files {
            let fixture: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
            let theirs = &fixture["requests"].as_array().unwrap().iter().find(|r| r["path"] == "/putResultate").unwrap()["body"];
            let number = theirs["pruefungen"][0]["nummer"].as_str().unwrap().to_string();
            let at = format!("{label} P{number}");
            let class = state.classes.values().find(|c| c.number == number).unwrap();
            classes += 1;
            let request = match put_resultate(&state, &class.id, &keys) {
                Ok(r) => r,
                Err(e) => {
                    problems.push(format!("{at}: {e}"));
                    continue;
                }
            };
            let ours = request.body.unwrap();

            let mut differences: Vec<(String, String)> = Vec::new();
            // Everything but the result lines, as a whole.
            let strip = |v: &Value| {
                let mut v = v.clone();
                v.as_object_mut().unwrap().remove("resultate");
                v
            };
            diff("", &strip(theirs), &strip(&ours), &mut differences, &mut counts);
            // Result lines, matched by who they are.
            let index = |v: &Value| -> BTreeMap<String, Value> { v["resultate"].as_array().unwrap().iter().map(|r| (identity(r), r.clone())).collect() };
            let (their_lines, our_lines) = (index(theirs), index(&ours));
            if their_lines.len() != theirs["resultate"].as_array().unwrap().len() {
                problems.push(format!("{at}: two Rosson lines for the same rider and horse"));
            }
            for (id, line) in &their_lines {
                lines += 1;
                match our_lines.get(id) {
                    Some(o) => diff(&format!(".resultate[{id}]"), line, o, &mut differences, &mut counts),
                    None => differences.push((format!(".resultate[{id}]"), "missing in ours".into())),
                }
            }
            // S15: Rosson ranks a team none of whose riders started.
            if at == "2025.8161 P8" {
                let ghost = "team 13103";
                if their_lines.contains_key(ghost) && !our_lines.contains_key(ghost) {
                    differences.retain(|(p, _)| !p.contains(ghost));
                    for count in ["anzahl_gestartete_reiter", "anzahl_vollendete_reiter"] {
                        if theirs["pruefungen"][0][count].as_u64() == ours["pruefungen"][0][count].as_u64().map(|n| n + 1) {
                            differences.retain(|(p, _)| !p.ends_with(count));
                        }
                    }
                }
            }
            // S7: a team of 3 with all riders eliminated is out; Rosson still ranked it.
            if at == "2026.8352 P7" {
                let out_team = "team 14061";
                differences.retain(|(p, _)| !(p.contains(out_team) && p.ends_with("aufgabegrund_id")));
                if theirs["pruefungen"][0]["anzahl_vollendete_reiter"].as_u64() == ours["pruefungen"][0]["anzahl_vollendete_reiter"].as_u64().map(|n| n + 1) {
                    differences.retain(|(p, _)| !p.ends_with("anzahl_vollendete_reiter"));
                }
            }
            for id in our_lines.keys().filter(|id| !their_lines.contains_key(*id)) {
                differences.push((format!(".resultate[{id}]"), "not in Rosson's".into()));
            }

            for (path, what) in differences {
                let field = path.rsplit('.').next().unwrap().split('[').next().unwrap().to_string();
                if ranking_field(&field) {
                    *ranking_differences.entry(at.clone()).or_default() += 1;
                    continue;
                }
                *by_field.entry(field.clone()).or_default() += 1;
                if by_field[&field] <= 3 {
                    problems.push(format!("{at}{path}: {what}"));
                }
            }
        }
    }

    println!("\n{classes} classes, {lines} result lines; {} values in another of Rosson's forms; {} per-judge percentages exact where Rosson's floating point was off", counts.loose_types, counts.float_fixed);
    for ((field, theirs, ours), n) in &counts.forms {
        println!("  form: {field:<20} Rosson {theirs:<10} ours {ours:<10} {n}");
    }
    println!("Ranking differences (checked by the golden tests): {ranking_differences:?}");
    println!("Differences by field: {by_field:?}");
    for p in problems.iter().take(80) {
        println!("  {p}");
    }
    let unexplained: Vec<&String> = ranking_differences.keys().filter(|c| !DECIDED_RANKINGS.iter().any(|(d, _)| d == c)).collect();
    assert!(unexplained.is_empty(), "ranking differs from Rosson's without a decided rule: {unexplained:?}");
    assert!(classes > 70);
    assert!(by_field.is_empty() && problems.is_empty(), "{} differences with Rosson's uploads", by_field.values().sum::<usize>() + problems.len());
}
