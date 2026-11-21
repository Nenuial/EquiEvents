//! What two computers changed apart, for the office to look at after a sync (rules R2, R4, R7,
//! R11, docs/architecture/reconciliation.md): which change was kept and which it replaced, by
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
    /// `class`, `order`, `numbers`, `duplicates`, `result`, `late_final`, `class_gone`.
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
    };
    let restorable = open
        && match c.kind {
            ConflictKind::ClassEdited | ConflictKind::ConcurrentEdit => true,
            ConflictKind::StartOrder | ConflictKind::StartNumbers => over.is_some_and(|o| matches!(&o.change, Change::StartsSet { seen: Some(_), .. })),
            _ => false,
        };
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

/// The ids in a show file's `conflicts_seen`.
pub fn seen_ids(meta: Option<String>) -> BTreeSet<String> {
    meta.and_then(|m| serde_json::from_str::<Vec<String>>(&m).ok()).unwrap_or_default().into_iter().collect()
}
