//! What a change sets, thing by thing, and what each is (rule R12, docs/architecture/
//! reconciliation.md): every change made at the show carries what its author saw of the things it
//! sets. Applied where one of them is no longer what the author saw — changed meanwhile on
//! another computer, unknown to the author — the later change is applied all the same and the
//! show records a conflict, with what it replaced: everyone is told, always (the user,
//! 2026-11-22: two computers offline gave a rider two different horses; the later one stayed and
//! no one was told).
//!
//! Things are fine-grained: an entry's horse and its note are two things, two marks of a judge's
//! sheet too; changing different ones is no clash. Classes, start lists and results' fields have
//! their own rules (R11, R7, R2); a results change also notes the class's status, so a change
//! that reaches a class finalised meanwhile is said.

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::model::*;
use crate::oplog::Change;
use crate::state::State;

/// What an author saw of the things a change sets, by key (`Thing::key`).
pub type Saw = BTreeMap<String, Value>;

/// One thing a change sets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "thing", rename_all = "snake_case")]
pub enum Thing {
    /// A field of an entry: `status`, `hors_concours`, `horse`, `rider`, `published`, `stage`,
    /// `comment`, `horse_phases`.
    EntryField { entry: EntryId, field: String },
    /// A whole entry (added, taken back).
    Entry { entry: EntryId },
    /// An official's role (in a class, or at the event).
    Assignment { assignment: AssignmentId },
    /// A rider's address, as the office set it.
    Address { person: PersonId },
    Team { team: TeamId },
    /// Start numbers fixed for the event, given out.
    EventNumbers,
    Prizes { published: PublishedClassId },
    /// A published class's `category_text` or `levels`.
    PublishedField { published: PublishedClassId, field: String },
    Programme { id: u32 },
    ScoringTable { id: u32 },
    Combined { id: String },
    /// A class as a whole (taken away).
    Class { class: ClassId },
    /// A class's status: a results change made while it was running.
    ClassStatus { class: ClassId },
    /// A rank set by hand.
    Rank { class: ClassId, competitor: Competitor },
    /// Eventing: the jury's penalties for a rider.
    JuryPenalty { class: ClassId, entry: EntryId },
    /// A whole score card (entered from a file, or cleared).
    Card { class: ClassId, entry: EntryId },
    TeamCard { class: ClassId, team: TeamId },
    /// A judge's whole sheet, on a rider or put aside.
    Sheet { class: ClassId, place: SheetPlace },
    /// One mark of a judge's sheet; its penalties; its total.
    SheetMark { class: ClassId, entry: EntryId, judge: AssignmentId, mark: String },
    SheetPenalties { class: ClassId, entry: EntryId, judge: AssignmentId },
    SheetTotal { class: ClassId, entry: EntryId, judge: AssignmentId },
    TeamSheetMark { class: ClassId, team: TeamId, judge: AssignmentId, mark: String },
    TeamSheetPenalties { class: ClassId, team: TeamId, judge: AssignmentId },
}

impl Thing {
    /// Its key: what was seen of it, who last changed it (`State`'s writers; the class's fields
    /// are `c/<class>/<field>`, results' `r/<class>/<entry>/<field>`). A key is inside another
    /// when it starts with it and `/`.
    pub fn key(&self) -> String {
        match self {
            Thing::EntryField { entry, field } => format!("e/{entry}/{field}"),
            Thing::Entry { entry } => format!("e/{entry}"),
            Thing::Assignment { assignment } => format!("a/{assignment}"),
            Thing::Address { person } => format!("ad/{person}"),
            Thing::Team { team } => format!("t/{team}"),
            Thing::EventNumbers => "ev/numbers".into(),
            Thing::Prizes { published } => format!("pz/{published}"),
            Thing::PublishedField { published, field } => format!("pc/{published}/{field}"),
            Thing::Programme { id } => format!("pg/{id}"),
            Thing::ScoringTable { id } => format!("st/{id}"),
            Thing::Combined { id } => format!("cr/{id}"),
            Thing::Class { class } => format!("c/{class}"),
            Thing::ClassStatus { class } => format!("cs/{class}"),
            Thing::Rank { class, competitor } => format!("rk/{class}/{}", competitor_key(competitor)),
            Thing::JuryPenalty { class, entry } => format!("jp/{class}/{entry}"),
            Thing::Card { class, entry } => format!("r/{class}/{entry}"),
            Thing::TeamCard { class, team } => format!("tr/{class}/{team}"),
            Thing::Sheet { class, place: SheetPlace::Rider { entry, judge } } => format!("s/{class}/{entry}/{judge}"),
            Thing::Sheet { class, place: SheetPlace::Aside { slot } } => format!("sa/{class}/{slot}"),
            Thing::SheetMark { class, entry, judge, mark } => format!("s/{class}/{entry}/{judge}/m/{mark}"),
            Thing::SheetPenalties { class, entry, judge } => format!("s/{class}/{entry}/{judge}/penalties"),
            Thing::SheetTotal { class, entry, judge } => format!("s/{class}/{entry}/{judge}/total"),
            Thing::TeamSheetMark { class, team, judge, mark } => format!("ts/{class}/{team}/{judge}/m/{mark}"),
            Thing::TeamSheetPenalties { class, team, judge } => format!("ts/{class}/{team}/{judge}/penalties"),
        }
    }

    /// The class it belongs to, if any.
    pub fn class(&self) -> Option<&ClassId> {
        match self {
            Thing::Class { class }
            | Thing::ClassStatus { class }
            | Thing::Rank { class, .. }
            | Thing::JuryPenalty { class, .. }
            | Thing::Card { class, .. }
            | Thing::TeamCard { class, .. }
            | Thing::Sheet { class, .. }
            | Thing::SheetMark { class, .. }
            | Thing::SheetPenalties { class, .. }
            | Thing::SheetTotal { class, .. }
            | Thing::TeamSheetMark { class, .. }
            | Thing::TeamSheetPenalties { class, .. } => Some(class),
            _ => None,
        }
    }

    /// What it is in `state` (`null`: not there).
    pub fn value(&self, state: &State) -> Value {
        let to = |v: Option<Value>| v.unwrap_or(Value::Null);
        let sheet = |class: &ClassId, entry: &EntryId, judge: &AssignmentId| state.sheets.get(&(class.clone(), entry.clone(), judge.clone()));
        let team_sheet = |class: &ClassId, team: &TeamId, judge: &AssignmentId| state.team_sheets.get(&(class.clone(), team.clone(), judge.clone()));
        match self {
            Thing::EntryField { entry, field } => to(state.entries.get(entry).map(|e| entry_field(e, field))),
            Thing::Entry { entry } => to(state.entries.get(entry).map(|e| json!(e))),
            // Swiss Equestrian's lookups change an assignment too: not the office's.
            Thing::Assignment { assignment } => to(state.assignments.get(assignment).map(|a| json!(Assignment { looked_up: false, se_id: None, ..a.clone() }))),
            Thing::Address { person } => match state.contacts.get(person) {
                Some(contact) if state.addresses_set.contains(person) => json!({ "town": state.persons.get(person).map(|p| p.town.clone()).unwrap_or_default(), "contact": contact }),
                _ => Value::Null,
            },
            Thing::Team { team } => to(state.teams.get(team).map(|t| json!(t))),
            Thing::EventNumbers => to(state.event.as_ref().map(|e| json!({ "fixed": e.fixed_start_numbers, "assigned": e.start_numbers_assigned }))),
            Thing::Prizes { published } => to(state.prizes.get(published).map(|p| json!(p))),
            Thing::PublishedField { published, field } => to(state.published_classes.get(published).map(|p| if field == "levels" { json!(p.levels) } else { json!(p.category_text) })),
            Thing::Programme { id } => to(state.programmes.get(id).map(|p| json!(p))),
            Thing::ScoringTable { id } => to(state.scoring_tables.get(id).map(|t| json!(t))),
            Thing::Combined { id } => to(state.combined.get(id).map(|c| json!(c))),
            Thing::Class { class } => to(state.classes.get(class).map(|c| json!(c))),
            Thing::ClassStatus { class } => to(state.classes.get(class).map(|c| json!(c.status))),
            Thing::Rank { class, competitor } => to(state.manual_ranks.get(&(class.clone(), competitor.clone())).map(|r| json!(r))),
            Thing::JuryPenalty { class, entry } => to(state.jury_penalties.get(&(class.clone(), entry.clone())).map(|p| json!(p))),
            Thing::Card { class, entry } => to(state.results.get(&(class.clone(), entry.clone())).map(|c| json!(c))),
            Thing::TeamCard { class, team } => to(state.team_results.get(&(class.clone(), team.clone())).map(|c| json!(c))),
            Thing::Sheet { class, place: SheetPlace::Rider { entry, judge } } => to(sheet(class, entry, judge).map(|s| json!(s))),
            Thing::Sheet { class, place: SheetPlace::Aside { slot } } => to(state.aside.get(&(class.clone(), *slot)).map(|s| json!(s))),
            Thing::SheetMark { class, entry, judge, mark } => to(sheet(class, entry, judge).and_then(|s| s.marks.get(mark)).map(|m| json!(m))),
            Thing::SheetPenalties { class, entry, judge } => to(sheet(class, entry, judge).map(penalties)),
            Thing::SheetTotal { class, entry, judge } => to(sheet(class, entry, judge).and_then(|s| s.total).map(|t| json!(t))),
            Thing::TeamSheetMark { class, team, judge, mark } => to(team_sheet(class, team, judge).and_then(|s| s.marks.get(mark)).map(|m| json!(m))),
            Thing::TeamSheetPenalties { class, team, judge } => to(team_sheet(class, team, judge).map(penalties)),
        }
    }

    /// The change that makes it `value` again (a conflict's other version put back), when it
    /// can be made.
    pub fn restore(&self, value: &Value) -> Option<Change> {
        let mark_of = |v: &Value| -> Option<Option<Decimal>> { none_or(v) };
        Some(match self {
            Thing::EntryField { entry, field } => {
                if value.is_null() {
                    return None;
                }
                let mut edit = EntryEdit::default();
                match field.as_str() {
                    "status" => edit.status = from(value),
                    "hors_concours" => edit.hors_concours = from(value),
                    "horse" => edit.horse = from(value),
                    "rider" => edit.rider = from(value),
                    "published" => edit.published = from(value),
                    "stage" => edit.stage = Some(value.as_i64().unwrap_or(0)),
                    "comment" => edit.comment = from(value),
                    "horse_phases" => edit.horse_phases = from(value),
                    _ => return None,
                }
                if edit == EntryEdit::default() {
                    return None;
                }
                Change::EntryEdited { entry: entry.clone(), edit }
            }
            Thing::Assignment { assignment } => match value {
                Value::Null => Change::AssignmentRemoved { assignment: assignment.clone() },
                v => Change::AssignmentSet { assignment: from(v)? },
            },
            Thing::Address { person } => Change::AddressSet { person: person.clone(), town: value["town"].as_str()?.to_string(), contact: from(&value["contact"])? },
            Thing::Team { team } => match value {
                Value::Null => Change::TeamRemoved { team: team.clone() },
                v => Change::TeamSet { team: from(v)? },
            },
            Thing::EventNumbers => Change::EventNumbersSet { fixed: value["fixed"].as_bool()?, assigned: value["assigned"].as_bool()? },
            Thing::Prizes { published } => Change::PrizesSet { published: published.clone(), prizes: none_or(value)? },
            Thing::PublishedField { published, field } => match field.as_str() {
                "levels" => Change::PublishedClassEdited { published: published.clone(), category_text: None, levels: Some(from(value)?) },
                _ => Change::PublishedClassEdited { published: published.clone(), category_text: Some(from(value)?), levels: None },
            },
            Thing::Programme { .. } => Change::ProgrammeSet { programme: from(value)? },
            Thing::ScoringTable { .. } => Change::ScoringTableSet { table: from(value)? },
            Thing::Combined { id } => match value {
                Value::Null => Change::CombinedRemoved { id: id.clone() },
                v => Change::CombinedSet { ranking: from(v)? },
            },
            Thing::Rank { class, competitor } => {
                let rank: Option<ManualRank> = none_or(value)?;
                Change::RankSet { class: class.clone(), competitor: competitor.clone(), rank: rank.as_ref().map(|r| r.rank), reason: rank.map(|r| r.reason).unwrap_or_default() }
            }
            Thing::JuryPenalty { class, entry } => Change::JuryPenaltySet { class: class.clone(), entry: entry.clone(), points: none_or(value)? },
            Thing::Card { class, entry } => match value {
                Value::Null => Change::ResultRemoved { class: class.clone(), entry: entry.clone() },
                v => {
                    let card: ScoreCard = from(v)?;
                    Change::ResultEntered { class: class.clone(), entry: entry.clone(), values: card.values, out: card.out }
                }
            },
            Thing::TeamCard { class, team } => {
                let card: ScoreCard = from(value)?;
                Change::TeamResultEntered { class: class.clone(), team: team.clone(), values: card.values, out: card.out }
            }
            Thing::Sheet { class, place } => Change::SheetSet { class: class.clone(), place: place.clone(), sheet: none_or(value)? },
            Thing::SheetMark { class, entry, judge, mark } => {
                Change::SheetEdited { class: class.clone(), entry: entry.clone(), judge: judge.clone(), marks: BTreeMap::from([(mark.clone(), mark_of(value)?)]), penalties: None, total: None }
            }
            Thing::SheetPenalties { class, entry, judge } => {
                let p: SheetPenalties = if value.is_null() { SheetPenalties::default() } else { from(value)? };
                Change::SheetEdited { class: class.clone(), entry: entry.clone(), judge: judge.clone(), marks: BTreeMap::new(), penalties: Some(p), total: None }
            }
            Thing::SheetTotal { class, entry, judge } => Change::SheetEdited { class: class.clone(), entry: entry.clone(), judge: judge.clone(), marks: BTreeMap::new(), penalties: None, total: Some(mark_of(value)?) },
            Thing::TeamSheetMark { class, team, judge, mark } => Change::TeamSheetEdited { class: class.clone(), team: team.clone(), judge: judge.clone(), marks: BTreeMap::from([(mark.clone(), mark_of(value)?)]), penalties: None },
            Thing::TeamSheetPenalties { class, team, judge } => {
                let p: SheetPenalties = if value.is_null() { SheetPenalties::default() } else { from(value)? };
                Change::TeamSheetEdited { class: class.clone(), team: team.clone(), judge: judge.clone(), marks: BTreeMap::new(), penalties: Some(p) }
            }
            // A whole entry or class: not made again from here.
            Thing::Entry { .. } | Thing::Class { .. } | Thing::ClassStatus { .. } => return None,
        })
    }
}

fn from<T: serde::de::DeserializeOwned>(v: &Value) -> Option<T> {
    serde_json::from_value(v.clone()).ok()
}

/// `null` as `None`.
fn none_or<T: serde::de::DeserializeOwned>(v: &Value) -> Option<Option<T>> {
    if v.is_null() {
        Some(None)
    } else {
        from(v).map(Some)
    }
}

fn competitor_key(c: &Competitor) -> String {
    match c {
        Competitor::Entry(e) => format!("e{e}"),
        Competitor::Team(t) => format!("t{t}"),
    }
}

/// A sheet's penalties, as `SheetPenalties`.
fn penalties(s: &JudgeSheet) -> Value {
    json!(SheetPenalties { points: s.penalty_points.clone(), percent: s.penalty_percent.clone(), excluded: s.excluded })
}

/// An entry's field, as the edit that sets it gives it.
fn entry_field(e: &Entry, field: &str) -> Value {
    match field {
        "status" => json!(e.status),
        "hors_concours" => json!(e.hors_concours),
        "horse" => json!(HorseOnEntry { horse: e.horse.clone(), guest_horse: e.guest_horse.clone(), horses_on_entry: e.horses_on_entry.clone() }),
        "rider" => json!(RiderChange { rider: e.rider.clone(), rider_on_entry: e.rider_on_entry.clone(), changed: e.rider_changed }),
        "published" => json!(e.published),
        "stage" => json!(e.stage.unwrap_or(0)),
        "comment" => json!(e.comment),
        "horse_phases" => json!(e.horses_on_entry.iter().map(|h| h.get("einsaetze").cloned().unwrap_or(Value::Null)).collect::<Vec<_>>()),
        _ => Value::Null,
    }
}

/// The things a change sets (none for deliveries and imports, which are Swiss Equestrian's, and
/// for classes, start lists and results' fields, which have their own rules).
pub fn things(change: &Change) -> Vec<Thing> {
    let status = |class: &ClassId| Thing::ClassStatus { class: class.clone() };
    match change {
        Change::EntryEdited { entry, edit } => {
            let set = [
                ("status", edit.status.is_some()),
                ("hors_concours", edit.hors_concours.is_some()),
                ("horse", edit.horse.is_some()),
                ("rider", edit.rider.is_some()),
                ("published", edit.published.is_some()),
                ("stage", edit.stage.is_some()),
                ("comment", edit.comment.is_some()),
                ("horse_phases", edit.horse_phases.is_some()),
            ];
            set.into_iter().filter(|(_, s)| *s).map(|(f, _)| Thing::EntryField { entry: entry.clone(), field: f.into() }).collect()
        }
        Change::EntryAdded { entry: Entry { id, .. } } => vec![Thing::Entry { entry: id.clone() }],
        Change::EntryRemoved { entry } => vec![Thing::Entry { entry: entry.clone() }],
        Change::AssignmentSet { assignment } => vec![Thing::Assignment { assignment: assignment.id.clone() }],
        Change::AssignmentRemoved { assignment } => vec![Thing::Assignment { assignment: assignment.clone() }],
        Change::AddressSet { person, .. } => vec![Thing::Address { person: person.clone() }],
        Change::TeamSet { team } => vec![Thing::Team { team: team.id.clone() }],
        Change::TeamRemoved { team } => vec![Thing::Team { team: team.clone() }],
        Change::EventNumbersSet { .. } => vec![Thing::EventNumbers],
        Change::PrizesSet { published, .. } => vec![Thing::Prizes { published: published.clone() }],
        Change::PublishedClassEdited { published, category_text, levels } => [("category_text", category_text.is_some()), ("levels", levels.is_some())]
            .into_iter()
            .filter(|(_, s)| *s)
            .map(|(f, _)| Thing::PublishedField { published: published.clone(), field: f.into() })
            .collect(),
        Change::ProgrammeSet { programme } => vec![Thing::Programme { id: programme.id }],
        Change::ScoringTableSet { table } => vec![Thing::ScoringTable { id: table.id }],
        Change::CombinedSet { ranking } => vec![Thing::Combined { id: ranking.id.clone() }],
        Change::CombinedRemoved { id } => vec![Thing::Combined { id: id.clone() }],
        Change::ClassRemoved { class } => vec![Thing::Class { class: class.clone() }],
        Change::RankSet { class, competitor, .. } => vec![status(class), Thing::Rank { class: class.clone(), competitor: competitor.clone() }],
        Change::JuryPenaltySet { class, entry, .. } => vec![status(class), Thing::JuryPenalty { class: class.clone(), entry: entry.clone() }],
        Change::ResultEntered { class, entry, .. } | Change::ResultRemoved { class, entry } => vec![status(class), Thing::Card { class: class.clone(), entry: entry.clone() }],
        // Its fields: R2.
        Change::ResultEdited { class, .. } => vec![status(class)],
        Change::TeamResultEntered { class, team, .. } => vec![status(class), Thing::TeamCard { class: class.clone(), team: team.clone() }],
        Change::JudgeSheetEntered { class, entry, judge, .. } => vec![status(class), Thing::Sheet { class: class.clone(), place: SheetPlace::Rider { entry: entry.clone(), judge: judge.clone() } }],
        Change::SheetEdited { class, entry, judge, marks, penalties, total } => {
            let mut out = vec![status(class)];
            out.extend(marks.keys().map(|m| Thing::SheetMark { class: class.clone(), entry: entry.clone(), judge: judge.clone(), mark: m.clone() }));
            if penalties.is_some() {
                out.push(Thing::SheetPenalties { class: class.clone(), entry: entry.clone(), judge: judge.clone() });
            }
            if total.is_some() {
                out.push(Thing::SheetTotal { class: class.clone(), entry: entry.clone(), judge: judge.clone() });
            }
            out
        }
        Change::SheetMoved { class, from, to } => vec![status(class), Thing::Sheet { class: class.clone(), place: from.clone() }, Thing::Sheet { class: class.clone(), place: to.clone() }],
        Change::SheetSet { class, place, .. } => vec![status(class), Thing::Sheet { class: class.clone(), place: place.clone() }],
        Change::TeamSheetEdited { class, team, judge, marks, penalties } => {
            let mut out = vec![status(class)];
            out.extend(marks.keys().map(|m| Thing::TeamSheetMark { class: class.clone(), team: team.clone(), judge: judge.clone(), mark: m.clone() }));
            if penalties.is_some() {
                out.push(Thing::TeamSheetPenalties { class: class.clone(), team: team.clone(), judge: judge.clone() });
            }
            out
        }
        _ => Vec::new(),
    }
}

/// Whether two keys concern the same thing: equal, or one inside the other.
pub fn overlap(a: &str, b: &str) -> bool {
    a == b || a.strip_prefix(b).is_some_and(|r| r.starts_with('/')) || b.strip_prefix(a).is_some_and(|r| r.starts_with('/'))
}

impl State {
    /// What the author sees of the things these changes set (R12), change by change, before
    /// they become operations. A thing an earlier change of the same changes sets is theirs:
    /// not looked at again.
    pub fn saw(&self, changes: &[Change]) -> Vec<Saw> {
        let mut set: BTreeSet<String> = BTreeSet::new();
        changes
            .iter()
            .map(|change| {
                let things = things(change);
                let saw: Saw = things.iter().map(|t| (t.key(), t)).filter(|(k, _)| !set.iter().any(|s| overlap(s, k))).map(|(k, t)| (k, t.value(self))).collect();
                set.extend(things.iter().map(Thing::key));
                saw
            })
            .collect()
    }
}
