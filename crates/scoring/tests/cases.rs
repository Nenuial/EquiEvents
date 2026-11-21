//! Runs the hand-written cases in `tests/cases/*.md` (format: `tests/cases/README.md`).
//!
//! Each case is a table whose rows are the expected ranking, in printed order. A failing case
//! reports the file, the case and what differs, in the words of the table.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use scoring::dressage::{self, JudgeSheet, Movement, Penalty, Programme, ProgrammeKind};
use scoring::jumping::{self, table::ScoringTable, team};
use scoring::number::{self, Decimal};
use scoring::ranking::{self, Placement};
use serde_json::Value;

struct Case {
    file: String,
    name: String,
    settings: BTreeMap<String, String>,
    header: Vec<String>,
    rows: Vec<BTreeMap<String, String>>,
}

#[test]
fn hand_written_cases() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let tables = load_tables();
    let mut files: Vec<_> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "md") && !p.ends_with("README.md")).collect();
    files.sort();

    let mut failures = Vec::new();
    let mut count = 0;
    for file in files {
        for case in parse(&file) {
            count += 1;
            let result = if case.settings.contains_key("table") {
                run_jumping(&case, &tables)
            } else if case.settings.contains_key("programme") {
                run_dressage(&case)
            } else {
                Err("needs `table:` (jumping) or `programme:` (dressage)".into())
            };
            match result {
                Ok(()) => println!("ok    {} — {}", case.file, case.name),
                Err(e) => {
                    println!("FAIL  {} — {}\n      {}", case.file, case.name, e.replace('\n', "\n      "));
                    failures.push(format!("{} — {}", case.file, case.name));
                }
            }
        }
    }
    assert!(count > 0, "no cases found in {}", dir.display());
    assert!(failures.is_empty(), "{} of {count} cases failed:\n{}", failures.len(), failures.join("\n"));
}

fn load_tables() -> BTreeMap<u32, ScoringTable> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scoring-tables.json");
    let json: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    json["tables"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| {
            let id = t["id"].as_u64()? as u32;
            ScoringTable::from_definition(id, t["code"].as_str()?, t["name"].as_str()?, t["definition"].as_str().unwrap_or("")).ok().map(|table| (id, table))
        })
        .collect()
}

fn parse(path: &Path) -> Vec<Case> {
    let file = path.file_name().unwrap().to_string_lossy().to_string();
    let text = fs::read_to_string(path).unwrap();
    let mut cases: Vec<Case> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("## ") {
            cases.push(Case { file: file.clone(), name: name.to_string(), settings: BTreeMap::new(), header: Vec::new(), rows: Vec::new() });
            continue;
        }
        let Some(case) = cases.last_mut() else { continue };
        if line.starts_with('|') {
            let cells: Vec<String> = line.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect();
            if case.header.is_empty() {
                case.header = cells;
            } else if !cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':')) {
                case.rows.push(case.header.iter().cloned().zip(cells).collect());
            }
        } else if let Some((key, value)) = line.split_once(':') {
            if case.header.is_empty() && !key.contains(' ') && ["table", "programme", "judges", "options", "ideal", "final"].contains(&key) {
                case.settings.insert(key.to_string(), value.trim().to_string());
            }
        }
    }
    cases
}

/// `out` column: `eliminated`, `retired`, `withdrawn` (Swiss Equestrian jumping codes 21, 22, 23), or a code.
fn parse_out(v: &str) -> jumping::Out {
    use jumping::OutKind::*;
    let (code, kind) = match v {
        "eliminated" => (21, Eliminated),
        "retired" => (22, Retired),
        "withdrawn" => (23, Withdrawn),
        code => {
            let code: u32 = code.parse().expect("out: eliminated, retired, withdrawn or a reason code");
            (code, match code { 21 => Eliminated, 22 => Retired, 23 => Withdrawn, _ => Other })
        }
    };
    jumping::Out { code, kind }
}

fn dec(s: &str) -> Decimal {
    number::parse(s).unwrap().unwrap_or_else(|| panic!("empty number"))
}

/// Ranks set by hand in the `manual` column (by `key`: horse, or team in team cases).
fn manual_ranks(case: &Case, key: &str) -> BTreeMap<String, u32> {
    case.rows.iter().filter_map(|r| Some((r[key].clone(), r.get("manual").filter(|v| !v.is_empty())?.parse().expect("manual: a rank")))).collect()
}

/// Compare the computed listing with the table: same order, same ranks.
fn check_listing(case: &Case, listing: &[(String, Placement)]) -> Result<(), String> {
    let listing = ranking::with_manual_ranks(listing, &manual_ranks(case, "horse"));
    let expected: Vec<(String, String)> = case.rows.iter().map(|r| (r["horse"].clone(), r["rank"].clone())).collect();
    let computed: Vec<(String, String)> = listing
        .iter()
        .map(|(h, p)| (h.clone(), match p {
            Placement::Rank(r) => r.to_string(),
            Placement::Out => "out".to_string(),
        }))
        .collect();
    if expected != computed {
        let show = |v: &[(String, String)]| v.iter().map(|(h, r)| format!("{r:>3}  {h}")).collect::<Vec<_>>().join("\n");
        return Err(format!("expected:\n{}\ncomputed:\n{}", show(&expected), show(&computed)));
    }
    Ok(())
}

fn run_jumping(case: &Case, tables: &BTreeMap<u32, ScoringTable>) -> Result<(), String> {
    let id: u32 = case.settings["table"].parse().map_err(|_| "table: must be a number".to_string())?;
    let table = tables.get(&id).ok_or_else(|| format!("unknown or unusable scoring table {id}"))?;
    let special = ["horse", "out", "rank", "faults", "time", "team", "team faults", "team time", "manual", "why", "placeable"];
    let options = jumping::Options {
        medal_jump_offs: case.settings.get("options").is_some_and(|o| o.contains("medal jump-offs")),
        placed_count: case.settings.get("options").and_then(|o| o.split("placed ").nth(1)).and_then(|n| n.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()),
        ideal_time: case.settings.get("ideal").map(|t| dec(t)),
    };
    let entries: Vec<jumping::Entry> = case
        .rows
        .iter()
        .map(|r| jumping::Entry {
            id: r["horse"].clone(),
            horse: r["horse"].clone(),
            values: r.iter().filter(|(k, v)| !special.contains(&k.as_str()) && !v.is_empty()).map(|(k, v)| (k.clone(), dec(v))).collect(),
            out: r.get("out").filter(|v| !v.is_empty()).map(|v| parse_out(v)),
        })
        .collect();

    if case.header.iter().any(|h| h == "team") {
        return run_teams(case, table, entries);
    }

    let explained = jumping::rank_explained(table, &entries, options);
    let ranked: Vec<jumping::Ranked> = explained.iter().map(|(r, _)| r.clone()).collect();
    check_listing(case, &ranked.iter().map(|r| (r.id.clone(), r.placement)).collect::<Vec<_>>())?;
    // `why`: why the rider stands there, as the engine says it.
    for (r, why) in &explained {
        let said = why.text(table);
        if std::env::var("EQUIEVENTS_PRINT_WHY").is_ok() {
            eprintln!("{} · {} | {} | {said}", case.name, table.code, r.id);
        }
        if let Some(expected) = case.rows.iter().find(|row| row["horse"] == r.id).and_then(|row| row.get("why")).filter(|v| !v.is_empty()) {
            if *expected != said {
                return Err(format!("{}: why expected {expected:?}, the engine says {said:?}", r.id));
            }
        }
    }
    for row in &case.rows {
        let r = ranked.iter().find(|r| r.id == row["horse"]).unwrap();
        if let Some(expected) = row.get("placeable").filter(|v| !v.is_empty()) {
            if (expected == "yes") != r.placeable {
                return Err(format!("{}: placeable expected {expected}, computed {}", row["horse"], r.placeable));
            }
        }
        for (column, value) in [("faults", r.faults), ("time", r.time)] {
            if let Some(expected) = row.get(column).filter(|v| !v.is_empty()) {
                if value != Some(dec(expected)) {
                    return Err(format!("{}: {column} expected {expected}, computed {value:?}", row["horse"]));
                }
            }
        }
    }
    Ok(())
}

/// Team cases: rows are riders with a `team` column, teams listed in expected ranking order,
/// `rank` is the team's rank (on every row of the team), `team faults` / `team time` the team's
/// expected totals (on any row of the team).
fn run_teams(case: &Case, table: &ScoringTable, entries: Vec<jumping::Entry>) -> Result<(), String> {
    let mut teams: Vec<team::Team> = Vec::new();
    let mut expected: Vec<(String, String)> = Vec::new();
    for (row, entry) in case.rows.iter().zip(entries) {
        let name = &row["team"];
        if teams.last().is_none_or(|t| &t.id != name) {
            teams.push(team::Team { id: name.clone(), name: name.clone(), riders: Vec::new() });
            expected.push((name.clone(), row["rank"].clone()));
        }
        teams.last_mut().unwrap().riders.push(entry);
    }
    let count_all = case.settings.get("options").is_some_and(|o| o.contains("every result counts"));
    let ranked = team::rank(table, &teams, count_all);
    let listing = ranking::with_manual_ranks(&ranked.iter().map(|r| (r.id.clone(), r.placement)).collect::<Vec<_>>(), &manual_ranks(case, "team"));
    let computed: Vec<(String, String)> = listing.iter().map(|(id, p)| (id.clone(), match p {
        Placement::Rank(n) => n.to_string(),
        Placement::Out => "out".to_string(),
    })).collect();
    if expected != computed {
        let show = |v: &[(String, String)]| v.iter().map(|(h, r)| format!("{r:>3}  {h}")).collect::<Vec<_>>().join("\n");
        return Err(format!("expected:\n{}\ncomputed:\n{}", show(&expected), show(&computed)));
    }
    for row in &case.rows {
        let r = ranked.iter().find(|r| r.id == row["team"]).unwrap();
        for (column, value) in [("team faults", r.faults), ("team time", r.time)] {
            if let Some(e) = row.get(column).filter(|v| !v.is_empty()) {
                if value != Some(dec(e)) {
                    return Err(format!("team {}: {column} expected {e}, computed {value:?}", row["team"]));
                }
            }
        }
    }
    Ok(())
}

/// `programme: max 30; 1x1, 2x2`
fn parse_programme(spec: &str) -> Result<Programme, String> {
    let (max, movements) = spec.split_once(';').ok_or("programme: max <points>; <movement>x<coefficient>, ...")?;
    let max_points = dec(max.trim().strip_prefix("max").ok_or("programme: starts with `max`")?.trim());
    let movements = movements
        .split(',')
        .map(|m| {
            let (number, coefficient) = m.trim().split_once('x').ok_or("movement as <number>x<coefficient>")?;
            Ok(Movement { number: number.trim().to_string(), coefficient: dec(coefficient.trim()), part: None })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Programme { name: "test".into(), kind: ProgrammeKind::Standard, movements, max_points, final_ties: None })
}

fn run_dressage(case: &Case) -> Result<(), String> {
    let mut programme = parse_programme(&case.settings["programme"])?;
    // `final: 2, 3`: a Tag der Jugend final, ties in ranks 1–3 by judges C + H on these movements.
    if let Some(movements) = case.settings.get("final") {
        programme.final_ties = Some(scoring::dressage::FinalTies { movements: movements.split(',').map(|m| m.trim().to_string()).collect(), judges: vec!["C".into(), "H".into()] });
    }
    let judge_columns: Vec<&String> = case.header.iter().filter(|h| h.contains('.')).collect();
    let mut judges: Vec<String> = judge_columns.iter().map(|h| h.split('.').next().unwrap().to_string()).collect();
    judges.dedup();
    let judge_count = case.settings.get("judges").map(|j| j.parse().unwrap()).unwrap_or(judges.len());

    let mut scores = Vec::new();
    for row in &case.rows {
        let mut sheets: BTreeMap<String, JudgeSheet> = BTreeMap::new();
        for column in &judge_columns {
            let value = &row[*column];
            if value.is_empty() {
                continue;
            }
            let (judge, what) = column.split_once('.').unwrap();
            let sheet = sheets.entry(judge.to_string()).or_default();
            if what == "penalty" {
                sheet.penalties.push(Penalty::Points(dec(value)));
            } else {
                sheet.marks.insert(what.to_string(), dec(value));
            }
        }
        let entry = dressage::Entry { id: row["horse"].clone(), horse: row["horse"].clone(), out: row.get("out").is_some_and(|v| !v.is_empty()), sheets };
        scores.push(dressage::score(&programme, judge_count, &entry).map_err(|e| format!("{}: {e:?}", row["horse"]))?);
    }

    check_listing(case, &dressage::rank(&scores))?;
    for row in &case.rows {
        // `why`: what the ranking compared, as the engine says it.
        let said = scoring::ranking::reasons_text(&dressage::reasons(scores.iter().find(|s| s.id == row["horse"]).unwrap()));
        if std::env::var("EQUIEVENTS_PRINT_WHY").is_ok() {
            eprintln!("{} · dressage | {} | {said}", case.name, row["horse"]);
        }
        if let Some(expected) = row.get("why").filter(|v| !v.is_empty()) {
            if *expected != said {
                return Err(format!("{}: why expected {expected:?}, the engine says {said:?}", row["horse"]));
            }
        }
        if let Some(expected) = row.get("percentage").filter(|v| !v.is_empty()) {
            let s = scores.iter().find(|s| s.id == row["horse"]).unwrap();
            if s.percentage != Some(Decimal::from_str(expected).unwrap()) {
                return Err(format!("{}: percentage expected {expected}, computed {:?}", row["horse"], s.percentage));
            }
        }
    }
    Ok(())
}
