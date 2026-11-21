//! Team classes in show jumping.
//!
//! # The rule (P. Burkhard, 2026-11-13; docs/deviations.md S7)
//!
//! A team's result is built **round by round** from its riders' results, then the team is
//! ranked like a single rider with the class's scoring table ([`super::rank`]). So the table
//! decides what counts — faults of both rounds with the time of both rounds, or with the time
//! of round 2 only — and a jump-off counts if there is one.
//!
//! 1. **Which results count, per round** (round 1, round 2, each jump-off):
//!    - team of 4 (Nations Cup formula): the team's worst result of the round is not counted —
//!      the most faults, then the slowest time; a rider without a result in that round
//!      (eliminated, retired, not started) is the worst;
//!    - in a jump-off usually one rider per team starts and that result counts; if all 4 start,
//!      the worst is dropped as well;
//!    - teams of 3: nothing is dropped; teams of 2: both results are always added;
//!    - **every result counts** when the class says so (`count_all`, a class option: the
//!      Ausschreibung defines how results count, Springreglement 4.6; S36): nothing is dropped,
//!      whatever the team's size.
//! 2. **The team's values** are the sums, field by field, of the counted results.
//! 3. **Team out:** a team of 4 with 2 riders out (eliminated, retired or withdrawn), a team of 3
//!    or 2 with 1 rider out — when every result counts, any team with 1 rider out — or a team
//!    without any result (P. Burkhard, 2026-11-13, 2026-11-17). Rosson kept
//!    ranking such teams with the results of the remaining riders.
//! 4. **Riders** get their team's rank and placement (as in Rosson's `updateEquipenResultate`).
//!
//! Rosson differs in two places (S7): it never drops a result in a jump-off, and in tables where
//! the time of both rounds counts it drops the rider worst over both rounds together instead of
//! per round.

use std::collections::{BTreeMap, BTreeSet};

use crate::number::Decimal;
use super::table::ScoringTable;
use super::{rank as rank_riders, Entry, Ranked};

/// A team and its riders' results in the class.
#[derive(Debug, Clone, PartialEq)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub riders: Vec<Entry>,
}

/// Team size from which the worst result per round is dropped.
pub const DROP_FROM_SIZE: usize = 4;

/// Rank the teams. Returns the teams in listing order; `id` is the team id, `faults` and
/// `time` are the team's totals in its deciding block.
/// `count_all`: every rider's result counts, none is dropped (rule 1).
pub fn rank(table: &ScoringTable, teams: &[Team], count_all: bool) -> Vec<Ranked> {
    let entries: Vec<Entry> = teams.iter().map(|t| team_entry(table, t, count_all)).collect();
    rank_riders(table, &entries)
}

/// Rule 2: per field, the sum of the counted results — also for a team that is out (Rosson
/// reports them to Swiss Equestrian either way).
pub fn team_values(table: &ScoringTable, team: &Team, count_all: bool) -> BTreeMap<String, Decimal> {
    let mut values: BTreeMap<String, Decimal> = BTreeMap::new();
    for round in rounds(table) {
        for rider in counted_riders(team, &round, count_all) {
            for field in [&round.faults, &round.time] {
                if let Some(v) = rider.values.get(field) {
                    *values.entry(field.clone()).or_default() += *v;
                }
            }
        }
    }
    values
}

/// The team as one virtual rider: per field, the sum of the counted results (rules 1–3).
pub fn team_entry(table: &ScoringTable, team: &Team, count_all: bool) -> Entry {
    let values = team_values(table, team, count_all);
    let nobody_has_a_result = team.riders.iter().all(|r| r.values.is_empty());
    let riders_out = team.riders.iter().filter(|r| r.out.is_some()).count();
    let allowed_out = if team.riders.len() >= DROP_FROM_SIZE && !count_all { 1 } else { 0 };
    if riders_out > allowed_out || nobody_has_a_result {
        // An eliminated team has no result.
        let out = team.riders.iter().filter_map(|r| r.out).min();
        return Entry { id: team.id.clone(), horse: team.name.clone(), values: BTreeMap::new(), out };
    }
    Entry { id: team.id.clone(), horse: team.name.clone(), values, out: None }
}

/// One round of the class: its fault (or point) field and its time field.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Round {
    faults: String,
    time: String,
    jump_off: bool,
}

/// The rounds of a table, from its field names: `strafpunkte_<round>` / `zeit_<round>`, e.g.
/// `initial_1`, `initial_2`, `stechen_1`.
fn rounds(table: &ScoringTable) -> Vec<Round> {
    let names: BTreeSet<String> = table
        .blocks
        .iter()
        .flat_map(|b| b.required.iter().chain(&b.time).chain(&b.faults))
        .filter_map(|f| f.strip_prefix("strafpunkte_").or_else(|| f.strip_prefix("zeit_")))
        .map(str::to_string)
        .collect();
    names
        .into_iter()
        .map(|name| Round { faults: format!("strafpunkte_{name}"), time: format!("zeit_{name}"), jump_off: name.starts_with("stechen") })
        .collect()
}

/// Rule 1: the riders whose result counts for the team in this round.
fn counted_riders<'a>(team: &'a Team, round: &Round, count_all: bool) -> Vec<&'a Entry> {
    let with_result: Vec<&Entry> = team.riders.iter().filter(|r| r.values.contains_key(&round.faults) || r.values.contains_key(&round.time)).collect();
    if team.riders.len() < DROP_FROM_SIZE || count_all {
        return with_result;
    }
    if round.jump_off && with_result.len() < team.riders.len() {
        return with_result; // usually one rider jumps off: that result counts
    }
    if with_result.len() < team.riders.len() {
        return with_result; // the rider without a result is the dropped one
    }
    // All riders have a result: drop the worst (most faults, then slowest; ties: the last listed).
    let worst = with_result
        .iter()
        .enumerate()
        .max_by(|(ia, a), (ib, b)| {
            let key = |e: &Entry| (e.values.get(&round.faults).copied().unwrap_or_default(), e.values.get(&round.time).copied().unwrap_or_default());
            key(a).cmp(&key(b)).then(ia.cmp(ib))
        })
        .map(|(i, _)| i);
    with_result.into_iter().enumerate().filter(|(i, _)| Some(*i) != worst).map(|(_, r)| r).collect()
}
