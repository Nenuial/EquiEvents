//! Show-jumping results in the office, as changes to the show (each function reads the show
//! and returns the changes; it changes nothing itself):
//!
//! - a rider's result **entered or corrected by hand**, field by field (penalties, time, out);
//! - the **timekeeper's results file** read into a class (`timing`, Rosson's format);
//! - the class's start order as the **qualifier's ranking reversed** (U9);
//! - **rankings over several classes** set up, changed, removed (U8).

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;
use serde::Serialize;
use show::model::*;
use show::oplog::{Change, FieldChange};
use show::State;

/// The fields of a class's scoring table that are entered, in the order of the rounds.
/// The presentation's fields: each judge's overall mark (S44).
pub const PRESENTATION_MARKS: [&str; 3] = ["note_1", "note_2", "note_3"];

pub fn fields(state: &State, class: &Class) -> Vec<String> {
    // Driving phases: their own fields.
    if let Some(kind) = class.phase.as_ref().map(|p| p.kind) {
        let settings = class.options.driving.clone().unwrap_or_default();
        let list = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        match kind {
            PhaseKind::Marathon => {
                let mut f = list(&["a_start", "a_finish", "a_penalties", "a_neutralised", "b_start", "b_finish", "b_penalties", "b_neutralised"]);
                for i in 1..=settings.obstacles.unwrap_or(5) {
                    f.extend([format!("h{i}_time"), format!("h{i}_penalties"), format!("h{i}_neutralised")]);
                }
                return f;
            }
            PhaseKind::Cones => return (1..=settings.rounds.unwrap_or(1)).flat_map(|r| [format!("zeit_{r}"), format!("hindernis_{r}"), format!("andere_{r}")]).collect(),
            PhaseKind::Derby => return list(&["zeit", "abwuerfe", "uebrige"]),
            PhaseKind::Open => return list(&["sekunden", "punkte"]),
            PhaseKind::Multitest => return Vec::new(),
            // Each judge's overall mark (up to three judges, 13.5; S44).
            PhaseKind::Presentation => return list(&PRESENTATION_MARKS),
            _ => {}
        }
    }
    let Some(def) = class.scoring_table.and_then(|t| state.scoring_tables.get(&t)) else { return Vec::new() };
    let visible: BTreeSet<String> = scoring::jumping::table::parse_definition(scoring::jumping::table::definition_text(&def.code, &def.definition))
        .ok()
        .and_then(|d| d["sichtbare_felder"].as_array().map(|a| a.iter().filter_map(|f| f.as_str().map(str::to_string)).collect()))
        .unwrap_or_default();
    timing::FIELD_COLUMNS.iter().map(|(f, _)| f.to_string()).filter(|f| visible.contains(f)).collect()
}

/// The class's scoring table has two phases (phase 2 in the `initial_2` fields).
pub fn two_phase(state: &State, class: &Class) -> bool {
    class
        .scoring_table
        .and_then(|t| state.scoring_tables.get(&t))
        .and_then(|d| scoring::jumping::table::ScoringTable::from_definition(d.id, &d.code, &d.name, &d.definition).ok())
        .is_some_and(|t| t.phases != scoring::jumping::table::Phases::None)
}

/// The class's scoring table counts points, the most first (style, Gymkhana, Rückeprüfung).
pub fn points(state: &State, class: &Class) -> bool {
    class
        .scoring_table
        .and_then(|t| state.scoring_tables.get(&t))
        .and_then(|d| scoring::jumping::table::ScoringTable::from_definition(d.id, &d.code, &d.name, &d.definition).ok())
        .is_some_and(|t| t.blocks.iter().any(|b| b.points))
}

/// The class's scoring table is the ideal-time table (AZI): the office sets the ideal time (S20).
pub fn ideal_table(state: &State, class: &Class) -> bool {
    class
        .scoring_table
        .and_then(|t| state.scoring_tables.get(&t))
        .and_then(|d| scoring::jumping::table::ScoringTable::from_definition(d.id, &d.code, &d.name, &d.definition).ok())
        .is_some_and(|t| t.blocks.iter().any(|b| b.ideal_time))
}

/// The reasons for being out in a discipline (its own, or the general ones), as Rosson offers them.
pub fn reasons<'a>(state: &'a State, discipline: &str) -> Vec<&'a ReasonDef> {
    let mut reasons: Vec<&ReasonDef> = state.reasons.values().filter(|r| r.discipline.as_deref() == Some(discipline)).collect();
    if reasons.is_empty() {
        reasons = state.reasons.values().filter(|r| r.discipline.is_none() && !r.name.is_empty()).collect();
    }
    reasons
}

fn open_class<'a>(state: &'a State, id: &ClassId) -> Result<&'a Class, String> {
    let class = state.classes.get(id).ok_or_else(|| format!("unknown class {id}"))?;
    if state.status_of(class) == ClassStatus::Final {
        return Err(format!("class {} is final; reopen it first", class.number));
    }
    Ok(class)
}

/// Change a rider's result by hand: fields set (`Some`) or cleared (`None`); `out`: the reason
/// for being out, or back in the class (`Some(None)`). A result left empty is removed.
pub fn edit_result(state: &State, id: &ClassId, entry: &EntryId, values: BTreeMap<String, Option<Decimal>>, out: Option<Option<u32>>) -> Result<Vec<Change>, String> {
    let class = open_class(state, id)?;
    if !state.starts.get(id).is_some_and(|s| s.iter().any(|s| &s.entry == entry)) {
        return Err(format!("entry {entry} does not start in class {}", class.number));
    }
    let allowed = fields(state, class);
    if let Some(field) = values.keys().find(|f| !allowed.contains(f)) {
        return Err(format!("class {} has no field {field}", class.number));
    }
    if let Some(Some(code)) = out {
        state.reasons.get(&code).ok_or_else(|| format!("unknown reason {code}"))?;
    }
    if values.values().flatten().any(|v| v.is_sign_negative()) {
        return Err("penalties and times are not negative".into());
    }
    // The presentation: marks from 0 to 10, in halves (Fahrreglement 6.1.1).
    if class.phase.as_ref().is_some_and(|p| p.kind == PhaseKind::Presentation) {
        if let Some((field, v)) = values.iter().find_map(|(f, v)| v.filter(|v| *v > Decimal::TEN || !(v * Decimal::TWO).fract().is_zero()).map(|v| (f, v))) {
            return Err(format!("{field}: {v} is not a mark (0 to 10, in halves)"));
        }
    }

    let current = state.results.get(&(id.clone(), entry.clone()));
    let mut card = current.cloned().unwrap_or_default();
    let mut changes = Vec::new();
    for (field, to) in values {
        let from = card.values.get(&field).copied();
        if from != to {
            changes.push(FieldChange { field: field.clone(), from, to });
            match to {
                Some(v) => card.values.insert(field, v),
                None => card.values.remove(&field),
            };
        }
    }
    if let Some(out) = out {
        if card.out != out {
            changes.push(FieldChange { field: "out".into(), from: card.out.map(Decimal::from), to: out.map(Decimal::from) });
            card.out = out;
        }
    }
    if changes.is_empty() {
        return Ok(Vec::new());
    }
    if card.values.is_empty() && card.out.is_none() {
        return Ok(if current.is_some() { vec![Change::ResultRemoved { class: id.clone(), entry: entry.clone() }] } else { Vec::new() });
    }
    Ok(vec![Change::ResultEdited { class: id.clone(), entry: entry.clone(), changes }])
}

/// A rank set by hand (Rosson's `customRang`): after a jump-off decided by the jury, a jury
/// decision; `None` takes it away. The others' ranks skip it (S18).
pub fn set_rank(state: &State, id: &ClassId, who: Competitor, rank: Option<u32>, reason: &str) -> Result<Vec<Change>, String> {
    let class = open_class(state, id)?;
    match &who {
        Competitor::Entry(e) if !state.starts.get(id).is_some_and(|s| s.iter().any(|s| &s.entry == e)) => return Err(format!("entry {e} does not start in class {}", class.number)),
        Competitor::Team(t) if !state.class_teams(id).iter().any(|(x, _)| x == t) => return Err(format!("class {}: no team {t}", class.number)),
        _ => {}
    }
    if rank == Some(0) {
        return Err("a rank is 1 or more".into());
    }
    let before = state.manual_ranks.get(&(id.clone(), who.clone())).map(|m| m.rank);
    if before == rank {
        return Ok(Vec::new());
    }
    Ok(vec![Change::RankSet { class: id.clone(), competitor: who, rank, reason: reason.to_string() }])
}

/// Team dressage: why a team is out of the class (retired, eliminated…), or in again.
pub fn edit_team_out(state: &State, id: &ClassId, team: &TeamId, out: Option<u32>) -> Result<Vec<Change>, String> {
    let class = open_class(state, id)?;
    if !state.class_teams(id).iter().any(|(t, _)| t == team) {
        return Err(format!("class {}: no team {team}", class.number));
    }
    if let Some(code) = out {
        state.reasons.get(&code).ok_or_else(|| format!("unknown reason {code}"))?;
    }
    let card = state.team_results.get(&(id.clone(), team.clone())).cloned().unwrap_or_default();
    if card.out == out {
        return Ok(Vec::new());
    }
    Ok(vec![Change::TeamResultEntered { class: id.clone(), team: team.clone(), values: card.values, out }])
}

/// What reading a timekeeper's results file into a class does.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Imported {
    /// Riders whose result is new or changed.
    pub results: usize,
    /// Rows not read, and what the office should check (in English, from `timing`).
    pub notes: Vec<String>,
}

/// Read a timekeeper's results file into a class: the changes (results that are new or changed
/// only), and what to tell the office.
pub fn import_results(state: &State, id: &ClassId, bytes: &[u8], adopt_ranks: bool) -> Result<(Vec<Change>, Imported), String> {
    open_class(state, id)?;
    let rows = timing::read_results(bytes).map_err(|e| e.0)?;
    let import = timing::import_with(state, id, &rows, timing::Options { adopt_ranks }).map_err(|e| e.0)?;
    let changes: Vec<Change> = import
        .changes
        .into_iter()
        .filter(|c| match c {
            Change::ResultEntered { class, entry, values, out } => state.results.get(&(class.clone(), entry.clone())) != Some(&ScoreCard { values: values.clone(), out: *out }),
            _ => true,
        })
        .collect();
    let results = changes.iter().filter(|c| matches!(c, Change::ResultEntered { .. })).count();
    Ok((changes, Imported { results, notes: import.notes }))
}

/// The class's start order as its qualifier's ranking, reversed (U9). Start numbers stay with
/// their riders.
pub fn reverse_of_qualifier(state: &State, id: &ClassId) -> Result<Vec<Change>, String> {
    let class = open_class(state, id)?;
    if !class.teams.is_empty() {
        return Err("team classes: the order is set by teams".into());
    }
    let order = state.reverse_of_qualifier(id)?;
    let current = state.starts.get(id).cloned().unwrap_or_default();
    let starts = order
        .iter()
        .enumerate()
        .filter_map(|(i, e)| current.iter().find(|s| &s.entry == e).map(|s| Start { order: i as u32 + 1, ..s.clone() }))
        .collect();
    Ok(vec![Change::starts_set(id.clone(), starts)])
}

/// Set up or change a ranking over several classes (U8).
pub fn set_combined(state: &State, ranking: CombinedRanking) -> Result<Vec<Change>, String> {
    if ranking.name.trim().is_empty() {
        return Err("the ranking needs a name".into());
    }
    if ranking.classes.len() < 2 {
        return Err("a ranking over several classes needs at least two classes".into());
    }
    // Over team classes the teams are ranked: ranks or penalties added, as Rosson's team finals.
    let teams = ranking.classes.iter().all(|c| state.classes.get(c).and_then(|k| state.published_classes.get(&k.published)).is_some_and(|p| p.team_size.is_some()));
    if teams && matches!(ranking.method, CombinedMethod::Percentages { .. }) {
        return Err("teams are ranked by ranks or penalties added".into());
    }
    // Jumping or dressage classes, or both (Rosson's finals over a jumping and a dressage class
    // add ranks); percentages only over dressage classes, penalties only over jumping ones.
    for c in &ranking.classes {
        let class = state.classes.get(c).ok_or_else(|| format!("unknown class {c}"))?;
        let discipline = state.published_classes.get(&class.published).map(|p| p.discipline.as_str());
        if !matches!(discipline, Some("CS") | Some("CD")) {
            return Err(format!("class {} is neither show jumping nor dressage", class.number));
        }
        match (&ranking.method, discipline) {
            (CombinedMethod::Percentages { .. }, Some("CS")) => return Err(format!("class {} is not a dressage class: add ranks", class.number)),
            (CombinedMethod::Penalties { .. }, Some("CD")) => return Err(format!("class {} is a dressage class: add ranks or percentages", class.number)),
            _ => {}
        }
    }
    if ranking.classes.iter().collect::<BTreeSet<_>>().len() != ranking.classes.len() {
        return Err("a class is in the ranking twice".into());
    }
    let tie_class = match &ranking.method {
        CombinedMethod::Ranks { tie_break } => Some(tie_break),
        CombinedMethod::Penalties { tie } => tie.class(),
        CombinedMethod::Percentages { tie_break } => tie_break.as_ref(),
    };
    if tie_class.is_some_and(|c| !ranking.classes.contains(c)) {
        return Err("the class that breaks ties must be one of the ranking's".into());
    }
    Ok(vec![Change::CombinedSet { ranking: CombinedRanking { name: ranking.name.trim().to_string(), ..ranking } }])
}

pub fn remove_combined(state: &State, id: &str) -> Result<Vec<Change>, String> {
    state.combined.get(id).ok_or_else(|| format!("unknown ranking {id}"))?;
    Ok(vec![Change::CombinedRemoved { id: id.to_string() }])
}

// ─── Prizes (Rosson's "Preisgeld") ───────────────────────────────────────────

/// Rosson's prize scale (`preisgeld.controller.js`, `neuBerechnen`): from the first prize, each
/// amount rounded up to 5 CHF, never below the minimum, 80 % of it for the next rank.
/// *Better than Rosson:* in decimals, so no amount is rounded up by a floating-point remainder.
pub fn prize_scale(first: Decimal, minimum: Decimal, ranks: usize) -> Vec<Decimal> {
    let five = Decimal::from(5);
    let mut amount = first;
    (0..ranks)
        .map(|_| {
            let rounded = ((amount / five).ceil() * five).max(minimum);
            amount = rounded * Decimal::new(8, 1);
            rounded
        })
        .collect()
}

/// Swiss Equestrian's tax on an entry fee, CHF (Rosson still counts 10; docs/deviations.md E43).
pub const ENTRY_TAX: i64 = 18;

/// Whether a published class's prizes follow the jumping scale (`prize_scale` and its minimum,
/// (fee − tax) × 2): show jumping only, the other disciplines' prizes are typed rank by rank.
pub fn prize_scale_applies(state: &State, published: &PublishedClassId) -> bool {
    state.published_classes.get(published).is_some_and(|p| p.discipline == "CS")
}

/// A published class's prizes, or what they start from: in show jumping the minimum (fee − tax) × 2,
/// elsewhere none.
pub fn prizes_or_default(state: &State, published: &PublishedClassId) -> Prizes {
    state.prizes.get(published).cloned().unwrap_or_else(|| {
        let fee = state.published_classes.get(published).and_then(|p| p.fee).unwrap_or_default();
        let minimum = if prize_scale_applies(state, published) { ((fee - Decimal::from(ENTRY_TAX)) * Decimal::from(2)).max(Decimal::ZERO) } else { Decimal::ZERO };
        Prizes { minimum, amounts: Vec::new(), natural: Vec::new() }
    })
}

/// Set a published class's prizes (at most 30 ranks, as Rosson), or take them away.
pub fn set_prizes(state: &State, published: &PublishedClassId, prizes: Option<Prizes>) -> Result<Vec<Change>, String> {
    state.published_classes.get(published).ok_or_else(|| format!("unknown published class {published}"))?;
    let prizes = prizes.map(|mut p| {
        while p.amounts.last().is_some_and(|a| a.is_zero()) && p.natural.get(p.amounts.len() - 1).is_none_or(|n| n.trim().is_empty()) {
            p.amounts.pop();
        }
        p.natural = p.natural.into_iter().map(|n| n.trim().to_string()).collect();
        while p.natural.last().is_some_and(|n| n.is_empty()) {
            p.natural.pop();
        }
        p
    });
    if let Some(p) = &prizes {
        if p.amounts.len() > 30 || p.natural.len() > 30 {
            return Err("prizes for at most 30 ranks".into());
        }
        if p.minimum.is_sign_negative() || p.amounts.iter().any(|a| a.is_sign_negative()) {
            return Err("prizes are not negative".into());
        }
    }
    if state.prizes.get(published) == prizes.as_ref() {
        return Ok(Vec::new());
    }
    Ok(vec![Change::PrizesSet { published: published.clone(), prizes }])
}

/// A number from Swiss Equestrian's JSON (a number or a text).
pub fn decimal_of(v: &serde_json::Value) -> Option<Decimal> {
    match v {
        serde_json::Value::Number(n) => Decimal::from_str_exact(&n.to_string()).ok(),
        serde_json::Value::String(s) => Decimal::from_str_exact(s.trim()).ok(),
        _ => None,
    }
}

