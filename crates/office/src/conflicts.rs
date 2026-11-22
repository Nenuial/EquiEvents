//! What two computers changed apart, for the office to look at after a sync (rules R2, R4, R7,
//! R11, R12, docs/architecture/reconciliation.md): which change was kept and which it replaced, by
//! whom and when; putting the other version back.
//!
//! Every computer finds the same conflicts in the show (they come from its log). Which ones the
//! office has looked at is this computer's own (the show file's `conflicts_seen`): each person
//! sees what concerns them.

use std::collections::{BTreeSet, HashMap};

use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;
use show::model::*;
use show::oplog::{Change, FieldChange, Operation};
use show::state::{Conflict, ConflictKind};
use show::touched::Thing;
use show::State;

/// The show file's list of the conflicts looked at here.
pub const META_SEEN: &str = "conflicts_seen";

/// A change, as the office knows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChangeBy {
    pub author: String,
    /// Made on this computer.
    pub here: bool,
    pub millis: u64,
}

/// One thing both changed: a field, a rider, a number.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConflictItem {
    /// A class's field (`name`, `time_text`, `options.placed_count`…), a result's field
    /// (`zeit_1`…), or empty.
    pub field: String,
    /// The rider and horse concerned (riders, results), or empty.
    pub rider: String,
    /// The value kept and the one replaced (a number, a name…), or empty.
    pub kept: String,
    pub replaced: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConflictView {
    pub id: String,
    /// `class`, `order`, `numbers`, `duplicates`, `result`, `late_final`, `class_gone`;
    /// `change` (anything else changed on two computers), `change_gone` (a change not applied:
    /// what it changed is gone, or its class was finalised meanwhile).
    pub kind: &'static str,
    pub class: String,
    /// "12 · Libre R/N" (the class's id when it is gone).
    pub class_label: String,
    /// The later change, kept (for `late_final`: the change set aside).
    pub kept: Option<ChangeBy>,
    /// The earlier one it went over.
    pub replaced: Option<ChangeBy>,
    pub items: Vec<ConflictItem>,
    /// How many riders in all (the list shows some).
    pub count: usize,
    /// The other version can be put back.
    pub restorable: bool,
    /// Looked at here already.
    pub seen: bool,
}

/// Riders listed by name, at most these.
const RIDERS_SHOWN: usize = 8;

fn by(op: Option<&&Operation>, device: &str) -> Option<ChangeBy> {
    op.map(|o| ChangeBy { author: o.author.clone(), here: o.stamp.device == device, millis: o.stamp.millis })
}

fn rider_label(state: &State, entry: &str) -> String {
    let Some(e) = entry.parse().ok().and_then(|id| state.entries.get(&EntryId(id))) else { return format!("#{entry}") };
    let horse = crate::views::horse(state, e);
    let (name, _, _) = crate::views::rider(state, e);
    if horse.is_empty() {
        name
    } else {
        format!("{name} · {horse}")
    }
}

/// A class's field as the office reads it.
fn field_text(state: &State, field: &str, value: &Value) -> String {
    match (field, value) {
        (_, Value::Null) => String::new(),
        ("scoring_table", Value::Number(n)) => n.as_u64().and_then(|t| state.scoring_tables.get(&(t as u32))).map_or_else(|| n.to_string(), |t| t.code.clone()),
        ("programme", Value::Number(n)) => n.as_u64().and_then(|p| state.programmes.get(&(p as u32))).map_or_else(|| n.to_string(), |p| p.name.clone()),
        (_, Value::String(s)) => s.clone(),
        (_, v) => v.to_string(),
    }
}

/// A value at a field path of a class (`options.placed_count`).
fn at<'a>(class: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(class, |v, k| v.get(k))
}

fn number_text(n: Option<u32>) -> String {
    n.map(|n| n.to_string()).unwrap_or_default()
}

/// The show's conflicts, the latest first.
pub fn conflicts(log: &[Operation], state: &State, device: &str, seen: &BTreeSet<String>) -> Vec<ConflictView> {
    let ops: HashMap<&str, &Operation> = log.iter().map(|o| (o.id.as_str(), o)).collect();
    let mut out: Vec<ConflictView> = state.conflicts.iter().map(|c| view(c, &ops, state, device, seen)).collect();
    out.sort_by(|a, b| b.kept.as_ref().map(|k| k.millis).cmp(&a.kept.as_ref().map(|k| k.millis)).then_with(|| b.id.cmp(&a.id)));
    out
}

fn view(c: &Conflict, ops: &HashMap<&str, &Operation>, state: &State, device: &str, seen: &BTreeSet<String>) -> ConflictView {
    let class = state.classes.get(&c.class);
    let open = class.is_some_and(|k| state.status_of(k) != ClassStatus::Final);
    let class_label = class.map_or_else(|| c.class.0.clone(), |k| if k.name.is_empty() { k.number.clone() } else { format!("{} · {}", k.number, k.name) });
    let number_of = |entry: &str| entry.parse().ok().and_then(|id| state.starts.get(&c.class).and_then(|l| l.iter().find(|s| s.entry == EntryId(id))).and_then(|s| s.start_number));
    let over = c.over.as_deref().and_then(|o| ops.get(o));
    let mut count = c.what.len();
    let items: Vec<ConflictItem> = match c.kind {
        ConflictKind::ClassEdited => {
            let now = class.and_then(|k| serde_json::to_value(k).ok()).unwrap_or(Value::Null);
            c.what
                .iter()
                .map(|f| ConflictItem {
                    field: f.clone(),
                    rider: String::new(),
                    kept: at(&now, f).map(|v| field_text(state, f, v)).unwrap_or_default(),
                    replaced: c.lost.get(f).map(|v| field_text(state, f, v)).unwrap_or_default(),
                })
                .collect()
        }
        ConflictKind::StartOrder => c.what.iter().take(RIDERS_SHOWN).map(|e| ConflictItem { field: String::new(), rider: rider_label(state, e), kept: String::new(), replaced: String::new() }).collect(),
        ConflictKind::StartNumbers => c
            .what
            .iter()
            .take(RIDERS_SHOWN)
            .map(|e| ConflictItem { field: String::new(), rider: rider_label(state, e), kept: number_text(number_of(e)), replaced: number_text(c.lost.get(e).and_then(Value::as_u64).map(|n| n as u32)) })
            .collect(),
        ConflictKind::DuplicateNumbers => {
            let list = state.starts.get(&c.class).cloned().unwrap_or_default();
            let riders: Vec<ConflictItem> = c
                .what
                .iter()
                .filter_map(|n| n.parse::<u32>().ok())
                .flat_map(|n| list.iter().filter(move |s| s.start_number == Some(n)).map(move |s| (n, s.entry.to_string())))
                .map(|(n, e)| ConflictItem { field: String::new(), rider: rider_label(state, &e), kept: n.to_string(), replaced: String::new() })
                .collect();
            count = riders.len();
            riders
        }
        ConflictKind::ConcurrentEdit => match c.what.as_slice() {
            [entry, field] => {
                let card = entry.parse().ok().and_then(|id| state.results.get(&(c.class.clone(), EntryId(id))));
                let now = card.and_then(|k| if field == "out" { k.out.map(Decimal::from) } else { k.values.get(field).copied() });
                let lost: Option<Decimal> = c.lost.get(field).cloned().and_then(|v| serde_json::from_value(v).ok()).flatten();
                vec![ConflictItem { field: field.clone(), rider: rider_label(state, entry), kept: now.map(|d| d.to_string()).unwrap_or_default(), replaced: lost.map(|d| d.to_string()).unwrap_or_default() }]
            }
            _ => Vec::new(),
        },
        ConflictKind::LateForFinalClass | ConflictKind::ClassGone => Vec::new(),
        // R12: each thing, with what it is now and what was there (the change kept), or what is
        // there now (the change not applied).
        ConflictKind::Changed | ConflictKind::ChangedGone => c
            .things
            .iter()
            .map(|t| {
                let lost = c.lost.get(t.key()).cloned().unwrap_or(Value::Null);
                ConflictItem { field: thing_field(t), rider: subject(state, t), kept: thing_text(state, t, &t.value(state)), replaced: if c.kind == ConflictKind::Changed { thing_text(state, t, &lost) } else { String::new() } }
            })
            .collect(),
    };
    let restorable = match c.kind {
        ConflictKind::ClassEdited | ConflictKind::ConcurrentEdit => open,
        ConflictKind::StartOrder | ConflictKind::StartNumbers => open && over.is_some_and(|o| matches!(&o.change, Change::StartsSet { seen: Some(_), .. })),
        // Not in a final class; every thing can be put back.
        ConflictKind::Changed => (class.is_none() || open) && !c.things.is_empty() && c.things.iter().all(|t| t.restore(c.lost.get(t.key()).unwrap_or(&Value::Null)).is_some()),
        _ => false,
    };
    // Not about a class: what it is about (a rider, an official, a team…).
    let class_label = if class.is_none() && c.class.0.is_empty() { c.things.first().map(|t| subject(state, t)).unwrap_or_default() } else { class_label };
    ConflictView {
        id: c.id.clone(),
        kind: c.kind.code(),
        class: c.class.0.clone(),
        class_label,
        kept: by(ops.get(c.op.as_str()), device),
        replaced: by(over, device),
        items,
        count,
        restorable,
        seen: seen.contains(&c.id),
    }
}

/// The changes that put the replaced version of a conflict back: the class's fields as they
/// were, the earlier change to the start list made again (its moves and numbers over what is
/// there now), the result's value as it was.
pub fn restore(log: &[Operation], state: &State, id: &str) -> Result<Vec<Change>, String> {
    let c = state.conflicts.iter().find(|c| c.id == id).ok_or_else(|| format!("no conflict {id}"))?;
    // R12: each thing as it was (not in a final class).
    if c.kind == ConflictKind::Changed {
        if let Some(class) = state.classes.get(&c.class).filter(|k| state.status_of(k) == ClassStatus::Final) {
            return Err(format!("class {} is final; reopen it first", class.number));
        }
        return c.things.iter().map(|t| t.restore(c.lost.get(t.key()).unwrap_or(&Value::Null)).ok_or_else(|| "the other version is not known".to_string())).collect();
    }
    let class = state.classes.get(&c.class).ok_or_else(|| format!("unknown class {}", c.class))?;
    if state.status_of(class) == ClassStatus::Final {
        return Err(format!("class {} is final; reopen it first", class.number));
    }
    match c.kind {
        ConflictKind::ClassEdited => {
            let mut json = serde_json::to_value(class).map_err(|e| e.to_string())?;
            for f in &c.what {
                let lost = c.lost.get(f).cloned().unwrap_or(Value::Null);
                if f == "teams" {
                    json["teams"] = lost;
                    continue;
                }
                let mut target = &mut json;
                let mut keys: Vec<&str> = f.split('.').collect();
                let last = keys.pop().unwrap_or_default();
                for k in keys {
                    target = &mut target[k];
                }
                target[last] = lost;
            }
            let restored: Class = serde_json::from_value(json).map_err(|e| e.to_string())?;
            Ok(vec![Change::class_set(restored)])
        }
        ConflictKind::StartOrder | ConflictKind::StartNumbers => {
            let over = c.over.as_deref().and_then(|o| log.iter().find(|op| op.id == o)).ok_or("the other version is not known")?;
            match &over.change {
                Change::StartsSet { seen: Some(_), .. } => Ok(vec![over.change.clone()]),
                _ => Err("the other version is not known".into()),
            }
        }
        ConflictKind::ConcurrentEdit => {
            let [entry, field] = c.what.as_slice() else { return Err("the other version is not known".into()) };
            let entry = EntryId(entry.parse().map_err(|_| "the other version is not known")?);
            let card = state.results.get(&(c.class.clone(), entry.clone()));
            let from = card.and_then(|k| if field == "out" { k.out.map(Decimal::from) } else { k.values.get(field).copied() });
            let to: Option<Decimal> = c.lost.get(field).cloned().and_then(|v| serde_json::from_value(v).ok()).flatten();
            Ok(vec![Change::ResultEdited { class: c.class.clone(), entry, changes: vec![FieldChange { field: field.clone(), from, to }] }])
        }
        _ => Err("this conflict has no other version to put back".into()),
    }
}

/// What a thing is, for the office (the app names it): an entry's field (`horse`, `rider`,
/// `status`…), or the kind of thing (`assignment`, `mark`, `prizes`…).
fn thing_field(t: &Thing) -> String {
    match t {
        Thing::EntryField { field, .. } | Thing::PublishedField { field, .. } => field.clone(),
        Thing::Entry { .. } => "entry".into(),
        Thing::Assignment { .. } => "assignment".into(),
        Thing::Address { .. } => "address".into(),
        Thing::Team { .. } => "team".into(),
        Thing::EventNumbers => "event_numbers".into(),
        Thing::Prizes { .. } => "prizes".into(),
        Thing::Programme { .. } => "programme".into(),
        Thing::ScoringTable { .. } => "scoring_table".into(),
        Thing::Combined { .. } => "combined".into(),
        Thing::Class { .. } => "class".into(),
        Thing::ClassStatus { .. } => "class_status".into(),
        Thing::Rank { .. } => "rank".into(),
        Thing::JuryPenalty { .. } => "jury_penalty".into(),
        Thing::Card { .. } | Thing::TeamCard { .. } => "card".into(),
        Thing::Sheet { .. } => "sheet".into(),
        Thing::SheetMark { mark, .. } | Thing::TeamSheetMark { mark, .. } => format!("mark:{mark}"),
        Thing::SheetPenalties { .. } | Thing::TeamSheetPenalties { .. } => "penalties".into(),
        Thing::SheetTotal { .. } => "total".into(),
    }
}

/// Who or what a thing is about: the rider and horse, the official, the team, the ranking…
fn subject(state: &State, t: &Thing) -> String {
    let rider = |e: &EntryId| rider_label(state, &e.to_string());
    match t {
        // The horse is what changed: the rider alone.
        Thing::EntryField { entry, field } if field == "horse" => state.entries.get(entry).map_or_else(|| format!("#{entry}"), |e| crate::views::rider(state, e).0),
        Thing::EntryField { entry, .. }
        | Thing::Entry { entry }
        | Thing::JuryPenalty { entry, .. }
        | Thing::Card { entry, .. }
        | Thing::Sheet { place: SheetPlace::Rider { entry, .. }, .. }
        | Thing::SheetMark { entry, .. }
        | Thing::SheetPenalties { entry, .. }
        | Thing::SheetTotal { entry, .. } => rider(entry),
        Thing::Rank { competitor: Competitor::Entry(entry), .. } => rider(entry),
        Thing::Rank { competitor: Competitor::Team(team), .. } | Thing::Team { team } | Thing::TeamCard { team, .. } | Thing::TeamSheetMark { team, .. } | Thing::TeamSheetPenalties { team, .. } => {
            state.teams.get(team).map_or_else(|| team.to_string(), |t| t.name.clone())
        }
        Thing::Assignment { assignment } => state.assignments.get(assignment).map(|a| person_name(state, &a.person)).unwrap_or_default(),
        Thing::Address { person } => person_name(state, person),
        Thing::Prizes { published } | Thing::PublishedField { published, .. } => state.published_classes.get(published).map_or_else(|| published.to_string(), |p| p.number.clone()),
        Thing::Programme { id } => state.programmes.get(id).map(|p| p.name.clone()).unwrap_or_default(),
        Thing::ScoringTable { id } => state.scoring_tables.get(id).map(|t| t.code.clone()).unwrap_or_default(),
        Thing::Combined { id } => state.combined.get(id).map_or_else(|| id.clone(), |c| c.name.clone()),
        Thing::EventNumbers | Thing::Class { .. } | Thing::ClassStatus { .. } | Thing::Sheet { .. } => String::new(),
    }
}

fn person_name(state: &State, person: &PersonId) -> String {
    state.persons.get(person).map_or_else(|| format!("#{person}"), |p| format!("{} {}", p.first_name, p.last_name).trim().to_string())
}

/// A thing's value as the office reads it (empty: not there, or nothing short to say).
fn thing_text(state: &State, t: &Thing, value: &Value) -> String {
    if value.is_null() {
        return String::new();
    }
    let text = |v: &Value| match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        v => v.to_string(),
    };
    match t {
        Thing::EntryField { entry, field } => {
            // As the lists show it: the entry with that value.
            let mut e = state.entries.get(entry).cloned();
            match field.as_str() {
                "horse" => {
                    let Ok(h) = serde_json::from_value::<HorseOnEntry>(value.clone()) else { return String::new() };
                    match e.as_mut() {
                        Some(e) => {
                            e.horse = h.horse;
                            e.guest_horse = h.guest_horse;
                            e.horses_on_entry = h.horses_on_entry;
                            crate::views::horse(state, e)
                        }
                        None => h.horse.and_then(|id| state.horses.get(&id)).map(|h| h.name.clone()).unwrap_or_default(),
                    }
                }
                "rider" => {
                    let Ok(r) = serde_json::from_value::<RiderChange>(value.clone()) else { return String::new() };
                    match e.as_mut() {
                        Some(e) => {
                            e.rider = r.rider;
                            e.rider_on_entry = r.rider_on_entry;
                            crate::views::rider(state, e).0
                        }
                        None => person_name(state, &r.rider),
                    }
                }
                "published" => serde_json::from_value::<PublishedClassId>(value.clone()).ok().and_then(|p| state.published_classes.get(&p).map(|c| c.number.clone())).unwrap_or_else(|| text(value)),
                "stage" if value.as_i64() == Some(0) => String::new(),
                "horse_phases" => String::new(),
                // `status`: `Entered`, `NotStarting`; `hors_concours`: `true`, `false` (the app says them).
                _ => text(value),
            }
        }
        Thing::Assignment { .. } => {
            let Ok(a) = serde_json::from_value::<Assignment>(value.clone()) else { return String::new() };
            let name = person_name(state, &a.person);
            match a.letter.filter(|l| !l.is_empty()) {
                Some(letter) => format!("{name} · {} ({letter})", a.role),
                None => format!("{name} · {}", a.role),
            }
        }
        Thing::Address { .. } => text(&value["town"]),
        Thing::Team { .. } => text(&value["name"]),
        Thing::PublishedField { .. } | Thing::JuryPenalty { .. } | Thing::SheetMark { .. } | Thing::SheetTotal { .. } | Thing::TeamSheetMark { .. } => text(value),
        Thing::Rank { .. } => text(&value["rank"]),
        Thing::Combined { .. } | Thing::Programme { .. } => text(&value["name"]),
        Thing::ScoringTable { .. } => text(&value["code"]),
        Thing::ClassStatus { .. } => text(value),
        Thing::SheetPenalties { .. } | Thing::TeamSheetPenalties { .. } => {
            let list = |k: &str| value[k].as_array().map(|a| a.iter().map(text).collect::<Vec<_>>()).unwrap_or_default();
            let mut parts = list("points");
            parts.extend(list("percent").into_iter().map(|p| format!("{p} %")));
            parts.join(", ")
        }
        Thing::Entry { .. } | Thing::EventNumbers | Thing::Prizes { .. } | Thing::Class { .. } | Thing::Card { .. } | Thing::TeamCard { .. } | Thing::Sheet { .. } => String::new(),
    }
}

/// The ids in a show file's `conflicts_seen`.
pub fn seen_ids(meta: Option<String>) -> BTreeSet<String> {
    meta.and_then(|m| serde_json::from_str::<Vec<String>>(&m).ok()).unwrap_or_default().into_iter().collect()
}
