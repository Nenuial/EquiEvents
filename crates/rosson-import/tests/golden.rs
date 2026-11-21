//! Golden tests: the engine against every result of the real shows in `golden/corpus/`.
//!
//! The expected value is what Rosson stored at the show, except for the classes in
//! [`EXPECT_RECOMPUTED`], where docs/deviations.md decided the stored result was wrong; there the
//! expectation is Rosson 3.3.15's recomputation (`golden/corpus/<show>/oracle.json`).
//!
//! Run with `cargo test -p rosson-import --test golden -- --nocapture` to see the report.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use rosson_import::{read_recomputed, read_show, read_show_via_store, ClassKind, Stored};
use rust_decimal::Decimal;
use scoring::ranking::Placement;
use show::ranking::Settings;
use serde_json::Value;

/// (show, class number, ledger entry): expected values come from Rosson 3.3.15's recomputation.
const EXPECT_RECOMPUTED: &[(&str, &str, &str)] = &[
    ("2026.8352", "6", "S1 points-based class ranked lowest first at the show"),
    ("2025.8161", "7", "S1 points-based class ranked lowest first at the show"),
    ("2026.8352", "5", "S2 retired in a special two-phase class is out"),
    ("2025.8161", "5", "S2 retired in a special two-phase class is out"),
    ("2025.8161", "6", "S2 retired in a special two-phase class is out"),
];

/// Single results where the stored value is wrong by a decided rule, and neither Rosson at the
/// show nor Rosson 3.3.15 gets it right: (show, class, result id, field, correct value, why).
const CORRECTIONS: &[(&str, &str, &str, &str, &str, &str)] = &[
    ("2025.8158", "1", "57856_1758267054977_9r6fuw3siwp", "rank", "14", "S11 4+0 and 0+4 faults with the same time are a tie (Rosson: 15)"),
    // Rosson ranks before recounting the placed riders when a result is saved
    // (pruefungsdurchfuehrung.controller.js:147–150): the flags use the count before the last
    // result (10 results: 3; the 11th makes it 4). By its own rule (rangliste.service.js:813) the 4th is placed.
    ("2025.8126", "4_hi", "57559_1751812605913_f1e5972ay2f", "placed", "true", "Rosson's placed flags lag one result behind its placed count (4th of 4 placed)"),
    ("2026.8299", "2_hi", "59193_1783247509845_v1wffulmph", "placed", "true", "Rosson's placed flags lag one result behind its placed count (4th of 4 placed)"),
    // Driving, a short event (dressage + cones): equal in dressage and cones, the driven dressage
    // movements decide (Fahrreglement 13.10 §7.3; S42); Rosson ranked both equal.
    ("2025.8126", "2", "57559_1751795524840_zhjilne243", "rank", "11", "S42 short driving event: equal dressage, the driven movements decide (Rosson: equal 10th)"),
    ("2025.8126", "4", "57559_1751809659520_lw6ibei14e", "rank", "3", "S42 short driving event: equal dressage, the driven movements decide (Rosson: equal 2nd)"),
];

/// Teams Rosson ranked although none of their riders started (all withdrawn from the entry):
/// (show, class, team id, why). They are not part of the expected ranking, and every stored rank
/// below them is one too low per ghost (docs/deviations.md S15).
const GHOST_TEAMS: &[(&str, &str, &str, &str)] = &[
    ("2025.8161", "8", "13103", "S15 all riders withdrawn (team re-registered as 13105), ranked 19th by Rosson"),
];

/// Classes where Rosson ranked riders eliminated or retired in round 2 by their round-1 result.
/// By the decided rule (S16) they are equal last of round 2: the expected ranking is the stored
/// one with those riders moved there (see `round_2_rule`).
const ROUND_2_RULE: &[(&str, &str)] = &[("2025.8118", "7"), ("2025.8161", "9b"), ("2025.8161", "9c"), ("2026.8352", "8"), ("2026.8352", "11")];

/// Teams out by the decided team rule (S7: 2 riders out of 4, 1 out of 3 or 2) that Rosson
/// still ranked: (show, class, team id, why). Expected: out, and the teams below move up.
const TEAMS_OUT: &[(&str, &str, &str, &str)] = &[
    ("2026.8352", "7", "14061", "S7 team of 3, all riders eliminated, ranked 6th by Rosson"),
];

/// Differences that are expected while a ledger entry is open: (show, class, result id, field, why).
/// Each one must go away (or become a correction) once the entry is decided.
const OPEN_DIFFERENCES: &[(&str, &str, &str, &str, &str)] = &[];

/// Rosson's rank number for riders who are out.
const ROSSON_OUT: u32 = 99;

#[derive(Debug, Default)]
struct Computed {
    placement: Option<Placement>,
    placed: bool,
    faults: Option<Decimal>,
    time: Option<Decimal>,
    percentage: Option<Decimal>,
    judge_totals: BTreeMap<String, Decimal>,
}

#[derive(Debug)]
struct Diff {
    class: String,
    result: String,
    field: &'static str,
    expected: String,
    computed: String,
}

#[test]
fn engine_reproduces_the_corpus() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus");
    let mut shows: Vec<PathBuf> = fs::read_dir(&corpus).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("snapshot.json").exists()).collect();
    shows.sort();
    assert!(!shows.is_empty(), "no shows in {}", corpus.display());

    let mut diffs: Vec<Diff> = Vec::new();
    let (mut compared, mut identical, mut skipped) = (0usize, 0usize, Vec::new());
    println!("\n{:<10} {:<5} {:<10} {:<40} {:>9}  notes", "show", "class", "via", "scoring", "identical");

    for dir in shows {
        let snapshot: Value = serde_json::from_str(&fs::read_to_string(dir.join("snapshot.json")).unwrap()).unwrap();
        let oracle: Value = fs::read_to_string(dir.join("oracle.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
        let recomputed = read_recomputed(&oracle);
        let file = std::env::temp_dir().join(format!("equievents-golden-{}-{}.show", std::process::id(), dir.file_name().unwrap().to_string_lossy()));
        let _ = fs::remove_file(&file);
        let paths = [
            ("export", read_show(&snapshot).unwrap()),
            ("show file", read_show_via_store(&snapshot, &file).unwrap_or_else(|e| panic!("{}: {}", dir.display(), e.0))),
        ];
        let _ = fs::remove_file(&file);
        for (path, show) in &paths {

        for class in &show.classes {
            let label = format!("{} P{} [{path}]", show.nr, class.number);
            let scoring_label = match &class.kind {
                ClassKind::NotCovered(reason) => {
                    skipped.push(format!("{label}: {reason}"));
                    continue;
                }
                ClassKind::Jumping { table, .. } => format!("{} {}", table.id, table.code),
                ClassKind::JumpingTeams { table, .. } => format!("{} {} (teams)", table.id, table.code),
                ClassKind::Dressage { entries, .. } => format!("dressage {}", entries.first().map_or("", |(p, _)| p.name.as_str())),
                ClassKind::DressageTeams { teams, .. } => format!("team dressage {}", teams.first().map_or("", |(p, _)| p.name.as_str())),
                ClassKind::Cross { .. } => "cross".into(),
                ClassKind::Overall { .. } => "overall".into(),
                ClassKind::Driving { .. } => "driving".into(),
                ClassKind::ConesTeams { .. } => "team cones".into(),
            };
            // The app's own path from results to ranking (show::ranking).
            let settings = Settings { placed_count: class.placed_count_override, results_counted: class.results_counted, ..Default::default() };
            let ranking = show::ranking::rank(&class.kind, &settings).unwrap_or_else(|e| panic!("{label}: {e}"));
            let computed: HashMap<String, Computed> = ranking
                .lines
                .iter()
                .map(|l| (l.id.clone(), Computed { placement: Some(l.placement), placed: l.placed, faults: l.faults, time: l.time, percentage: l.percentage, judge_totals: l.judge_totals.clone() }))
                .collect();

            let override_note = EXPECT_RECOMPUTED.iter().find(|(s, c, _)| *s == show.nr && *c == class.number).map(|(_, _, why)| *why);
            let placed_count = ranking.placed_count;
            let mut class_identical = 0;
            let before = diffs.len();

            if class.placed_count_override.is_none() && class.stored_placed_count.is_some_and(|n| n != placed_count) {
                diffs.push(Diff { class: label.clone(), result: "-".into(), field: "placed count", expected: format!("{:?}", class.stored_placed_count), computed: placed_count.to_string() });
            }

            let adjusted = expected_by_rule(&show.nr, class, &snapshot);
            let ghosts: Vec<&str> = GHOST_TEAMS.iter().filter(|(s, c, _, _)| *s == show.nr && *c == class.number).map(|(_, _, t, _)| *t).collect();
            let ghost_ranks: Vec<Decimal> = class.stored_teams.iter().filter(|t| ghosts.contains(&t.id.as_str())).filter_map(|t| t.rank).collect();
            for stored in class.stored.iter().chain(&class.stored_teams) {
                if ghosts.contains(&stored.id.as_str()) {
                    continue;
                }
                let mut expected: Stored = if override_note.is_some() { recomputed.get(&stored.record).unwrap_or(stored).clone() } else { stored.clone() };
                if let Some(r) = expected.rank {
                    expected.rank = Some(r - Decimal::from(ghost_ranks.iter().filter(|g| **g < r).count()));
                }
                if let Some(rank) = adjusted.get(&stored.id) {
                    expected.rank = *rank;
                    expected.placed = Some(rank.is_some_and(|r| r != Decimal::from(ROSSON_OUT) && r <= Decimal::from(placed_count)));
                }
                let expected = &expected;
                let got = computed.get(&stored.id);
                let mut ok = true;
                let mut check = |field: &'static str, e: String, c: String| {
                    let open = OPEN_DIFFERENCES.iter().any(|(sh, cl, id, f, _)| *sh == show.nr && *cl == class.number && *id == stored.record && *f == field);
                    let corrected = CORRECTIONS.iter().find(|(sh, cl, id, f, _, _)| *sh == show.nr && *cl == class.number && *id == stored.record && *f == field);
                    let e = corrected.map_or(e, |c| c.4.to_string());
                    if e != c && !open {
                        ok = false;
                        diffs.push(Diff { class: label.clone(), result: stored.id.clone(), field, expected: e, computed: c });
                    }
                };

                let rank = got.and_then(|g| g.placement).map(|p| match p {
                    Placement::Rank(r) => r,
                    Placement::Out => ROSSON_OUT,
                });
                check("rank", num(&expected.rank), num(&rank.map(Decimal::from)));
                let placed = got.is_some_and(|g| g.placed);
                check("placed", fmt(&expected.placed.or(Some(false))), fmt(&Some(placed)));

                if let Some(g) = got.filter(|g| matches!(g.placement, Some(Placement::Rank(_)))) {
                    if g.faults.is_some() {
                        check("faults", num(&expected.faults), num(&g.faults));
                        check("time", num(&expected.time), num(&g.time));
                    }
                    if g.percentage.is_some() {
                        check("percentage", num(&expected.percentage), num(&g.percentage));
                    }
                    if g.percentage.is_some() || !g.judge_totals.is_empty() {
                        check("judge totals", nums(&expected.judge_totals), nums(&g.judge_totals));
                    }
                }
                compared += 1;
                if ok {
                    identical += 1;
                    class_identical += 1;
                }
            }

            let notes = [override_note.map(str::to_string), (diffs.len() > before).then(|| format!("{} differences", diffs.len() - before))]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("; ");
            println!("{:<10} {:<5} {:<10} {:<40} {:>4}/{:<4}  {}", show.nr, class.number, path, truncate(&scoring_label, 40), class_identical, class.stored.len() + class.stored_teams.len(), notes);
        }
        }
    }

    println!("\nCorrections of stored results ({}):", CORRECTIONS.len());
    for (sh, cl, _, f, v, why) in CORRECTIONS {
        println!("  {sh} P{cl} {f} = {v}: {why}");
    }
    println!("\nGhost teams removed from stored rankings ({}):", GHOST_TEAMS.len());
    for (sh, cl, t, why) in GHOST_TEAMS {
        println!("  {sh} P{cl} team {t}: {why}");
    }
    println!("\nRanking changed by decided rules: round 2 (S16) in {} classes; teams out (S7): {}", ROUND_2_RULE.len(), TEAMS_OUT.iter().map(|t| format!("{} P{} team {}", t.0, t.1, t.2)).collect::<Vec<_>>().join(", "));
    println!("\nOpen differences, waiting for a decision ({}):", OPEN_DIFFERENCES.len());
    for (sh, cl, _, f, why) in OPEN_DIFFERENCES {
        println!("  {sh} P{cl} {f}: {why}");
    }
    println!("\nNot covered yet ({}):", skipped.len());
    for s in &skipped {
        println!("  {s}");
    }
    println!("\n{identical}/{compared} results identical, {} differences", diffs.len());
    for d in diffs.iter().take(80) {
        println!("  {:<15} {:<14} {:<12} expected {:<36} computed {}", d.class, &d.result[d.result.len().saturating_sub(12)..], d.field, truncate(&d.expected, 36), d.computed);
    }
    assert!(diffs.is_empty(), "{} differences between the engine and the golden corpus (see report above)", diffs.len());
}

/// Expected ranks where a decided rule changes the stored ranking (ROUND_2_RULE, TEAMS_OUT).
/// Derived from the stored ranking alone, independently of the engine.
fn expected_by_rule(show: &str, class: &rosson_import::Class, snapshot: &Value) -> HashMap<String, Option<Decimal>> {
    let mut adjusted = HashMap::new();
    let out = Decimal::from(ROSSON_OUT);

    if ROUND_2_RULE.contains(&(show, class.number.as_str())) {
        let details: HashMap<String, &Value> = snapshot["resultate"].as_array().unwrap().iter().map(|r| (rosson_import::id(&r["id"]), &r["details"])).collect();
        let filled = |d: &Value, f: &str| d.get(f).is_some_and(|v| !v.is_null() && v != "");
        // 0: rode round 2 (and maybe the jump-off) · 1: eliminated or retired in round 2 · 2: round 1 only
        let mut rows: Vec<(&Stored, u8)> = class
            .stored
            .iter()
            .filter(|s| s.rank.is_some_and(|r| r != out))
            .map(|s| {
                let d = details[&s.record];
                let code = d.get("aufgabe").and_then(rosson_import::decimal).unwrap_or_default();
                let group = if filled(d, "strafpunkte_initial_2") && filled(d, "zeit_initial_2") {
                    0
                } else if code == Decimal::from(21) || code == Decimal::from(22) {
                    1
                } else {
                    2
                };
                (s, group)
            })
            .collect();
        rows.sort_by_key(|(s, g)| (*g, if *g == 1 { Decimal::ZERO } else { s.rank.unwrap() }));
        let ranks = scoring::ranking::competition_ranks(&rows, |a, b| a.1 == b.1 && (a.1 == 1 || a.0.rank == b.0.rank));
        for ((s, _), r) in rows.iter().zip(ranks) {
            adjusted.insert(s.id.clone(), Some(Decimal::from(r)));
        }
    }

    let teams_out: Vec<&str> = TEAMS_OUT.iter().filter(|(sh, cl, _, _)| *sh == show && *cl == class.number).map(|(_, _, t, _)| *t).collect();
    if let (false, ClassKind::JumpingTeams { team_of, .. }) = (teams_out.is_empty(), &class.kind) {
        let out_ranks: Vec<Decimal> = class.stored_teams.iter().filter(|t| teams_out.contains(&t.id.as_str())).filter_map(|t| t.rank).collect();
        for s in class.stored.iter().chain(&class.stored_teams) {
            let team = team_of.get(&s.id).map(String::as_str).unwrap_or(s.id.as_str());
            let rank = if teams_out.contains(&team) {
                Some(out)
            } else {
                s.rank.map(|r| if r == out { r } else { r - Decimal::from(out_ranks.iter().filter(|o| **o < r).count()) })
            };
            adjusted.insert(s.id.clone(), rank);
        }
    }
    adjusted
}

fn fmt<T: std::fmt::Display>(v: &Option<T>) -> String {
    v.as_ref().map_or_else(|| "-".to_string(), |x| x.to_string())
}

/// A number for comparison: Rosson's stored values carry float noise (136.52999999999997 for
/// 136.53, docs/deviations.md E5), so both sides are rounded to 6 decimals, far below any
/// timing or marking precision, and printed without trailing zeros.
fn num(v: &Option<Decimal>) -> String {
    v.map_or_else(|| "-".to_string(), |d| scoring::number::round(d, 6).normalize().to_string())
}

fn nums(m: &BTreeMap<String, Decimal>) -> String {
    m.iter().map(|(k, v)| format!("{}={}", &k[k.len().saturating_sub(6)..], num(&Some(*v)))).collect::<Vec<_>>().join(" ")
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n - 1).collect::<String>() + "…" }
}
