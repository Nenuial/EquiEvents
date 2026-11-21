//! Changes made apart, put together (docs/architecture/reconciliation.md, rules R7 and R11):
//! what the author changed, from what they saw, applied over what the show has now. Only the
//! three values count, so every device decides the same way.
//!
//! - A **sequence** (the riders of a start list, the teams of a class): the author's moves are
//!   made again on the current order; what they did not know stays where it is.
//! - A **start list**: its order as a sequence (team classes: each rider's place, as a field);
//!   start numbers rider by rider.
//! - A **class**: field by field (`options.placed_count` is a field of its own).
//!
//! Where both changed the same thing, the author's change wins (it is the later one) and the
//! merge says so: the show lists it as a conflict.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::model::{Class, EntryId, Start};

/// The items of `a` and `b` that did not move: a longest sequence of items in both, in the same
/// order (ties broken the same way on every device).
fn unmoved<T: Clone + Ord>(a: &[T], b: &[T]) -> BTreeSet<T> {
    let (n, m) = (a.len(), b.len());
    let mut len = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            len[i][j] = if a[i] == b[j] { len[i + 1][j + 1] + 1 } else { len[i + 1][j].max(len[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, BTreeSet::new());
    while i < n && j < m {
        if a[i] == b[j] {
            out.insert(a[i].clone());
            i += 1;
            j += 1;
        } else if len[i + 1][j] >= len[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// What moved (or came in) between two orders of the same things.
fn moved<T: Clone + Ord>(from: &[T], to: &[T]) -> BTreeSet<T> {
    let (in_from, in_to): (BTreeSet<&T>, BTreeSet<&T>) = (from.iter().collect(), to.iter().collect());
    let a: Vec<T> = from.iter().filter(|x| in_to.contains(x)).cloned().collect();
    let b: Vec<T> = to.iter().filter(|x| in_from.contains(x)).cloned().collect();
    let stayed = unmoved(&a, &b);
    to.iter().filter(|x| !stayed.contains(*x)).cloned().collect()
}

/// A sequence put together.
#[derive(Debug, Clone, PartialEq)]
pub struct Sequence<T> {
    pub items: Vec<T>,
    /// What the author moved or added.
    pub moved: Vec<T>,
    /// What the author moved and others moved too (the author's place kept).
    pub both: Vec<T>,
}

/// The author changed `seen` into `mine`; meanwhile it became `current`. Their moves and
/// additions are made again on `current`: what they put first goes first, what they put last
/// goes last, the others after the item before them in `mine` (one others did not move, else
/// before the item after them that nobody moved); what they took out goes; what others added
/// stays where it is; what others took out stays out.
pub fn sequence<T: Clone + Ord>(seen: &[T], current: &[T], mine: &[T]) -> Sequence<T> {
    let in_seen: BTreeSet<&T> = seen.iter().collect();
    let in_mine: BTreeSet<&T> = mine.iter().collect();
    let in_current: BTreeSet<&T> = current.iter().collect();
    let mine_moved = moved(seen, mine);
    let others_moved = moved(seen, current);
    let both: Vec<T> = mine.iter().filter(|x| mine_moved.contains(*x) && others_moved.contains(*x)).cloned().collect();
    let mut items: Vec<T> = current.iter().filter(|x| in_mine.contains(x) || !in_seen.contains(x)).filter(|x| !mine_moved.contains(*x)).cloned().collect();
    for (k, x) in mine.iter().enumerate() {
        // Taken out meanwhile: it stays out (only what the author added comes in).
        if !mine_moved.contains(x) || (in_seen.contains(x) && !in_current.contains(x)) {
            continue;
        }
        let at = |p: &T, items: &[T]| items.iter().position(|y| y == p);
        let steady = |p: &T| mine_moved.contains(p) || !others_moved.contains(p);
        let place = if k == 0 {
            0
        } else if k + 1 == mine.len() {
            items.len()
        } else if let Some(i) = mine[..k].iter().rev().filter(|p| steady(p)).find_map(|p| at(p, &items)) {
            i + 1
        } else if let Some(i) = mine[k + 1..].iter().filter(|p| !mine_moved.contains(*p) && !others_moved.contains(*p)).find_map(|p| at(p, &items)) {
            i
        } else {
            mine[..k].iter().rev().find_map(|p| at(p, &items)).map_or(0, |i| i + 1)
        };
        items.insert(place, x.clone());
    }
    Sequence { items, moved: mine.iter().filter(|x| mine_moved.contains(*x)).cloned().collect(), both }
}

/// A start list put together.
#[derive(Debug, Clone, PartialEq)]
pub struct StartsMerged {
    pub starts: Vec<Start>,
    /// Riders the author moved or added.
    pub moved: Vec<EntryId>,
    /// Riders whose number the author gave or changed.
    pub numbered: Vec<EntryId>,
    /// Riders the author moved that were moved meanwhile too.
    pub moved_both: Vec<EntryId>,
    /// Riders whose number both changed, with the number that was replaced.
    pub numbers_both: Vec<(EntryId, Option<u32>)>,
}

/// One field of a start: the author's if they changed it, else the current one. Also whether the
/// author changed it, and whether others did too (to something else).
fn field<V: Clone + PartialEq>(seen: Option<V>, current: Option<V>, mine: V) -> (V, bool, bool) {
    if seen.as_ref() == Some(&mine) {
        (current.unwrap_or(mine), false, false)
    } else {
        let both = current.as_ref().is_some_and(|c| Some(c) != seen.as_ref() && c != &mine);
        (mine, true, both)
    }
}

/// The author changed the list `seen` into `mine`; meanwhile it became `current`. `teams`: a
/// team class, where a start's order is the rider's place in the team (a field, not a position).
pub fn starts(seen: &[Start], current: &[Start], mine: &[Start], teams: bool) -> StartsMerged {
    let by_entry = |l: &[Start]| l.iter().map(|s| (s.entry.clone(), s.clone())).collect::<BTreeMap<_, _>>();
    let (s, c, m) = (by_entry(seen), by_entry(current), by_entry(mine));
    let ordered = |l: &[Start]| {
        let mut l: Vec<&Start> = l.iter().collect();
        l.sort_by_key(|s| s.order);
        l.into_iter().map(|s| s.entry.clone()).collect::<Vec<_>>()
    };
    let order = if teams {
        // Who is on the list: as now, without whom the author took out, with whom they added.
        let mut list: Vec<EntryId> = ordered(current).into_iter().filter(|e| m.contains_key(e) || !s.contains_key(e)).collect();
        list.extend(ordered(mine).into_iter().filter(|e| !s.contains_key(e) && !c.contains_key(e)));
        Sequence { items: list, moved: Vec::new(), both: Vec::new() }
    } else {
        sequence(&ordered(seen), &ordered(current), &ordered(mine))
    };
    let mut out = StartsMerged { starts: Vec::new(), moved: order.moved, numbered: Vec::new(), moved_both: order.both, numbers_both: Vec::new() };
    for (i, e) in order.items.iter().enumerate() {
        let start = match m.get(e) {
            None => c[e].clone(),
            Some(mine) => {
                let (seen, current) = (s.get(e), c.get(e));
                let (start_number, changed, both) = field(seen.map(|s| s.start_number), current.map(|s| s.start_number), mine.start_number);
                if changed && (mine.start_number.is_some() || seen.is_some()) {
                    out.numbered.push(e.clone());
                }
                if both {
                    out.numbers_both.push((e.clone(), current.and_then(|s| s.start_number)));
                }
                let (team_order, moved_team, both_team) = field(seen.map(|s| s.team_order), current.map(|s| s.team_order), mine.team_order);
                let (place, moved_place, both_place) = field(seen.map(|s| s.order), current.map(|s| s.order), mine.order);
                if teams && (moved_team || moved_place) {
                    out.moved.push(e.clone());
                    if both_team || both_place {
                        out.moved_both.push(e.clone());
                    }
                }
                Start { entry: e.clone(), order: if teams { place } else { mine.order }, start_number, team_order }
            }
        };
        out.starts.push(Start { order: if teams { start.order } else { i as u32 + 1 }, ..start });
    }
    out
}

/// A class put together.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassMerged {
    pub class: Class,
    /// The fields the author changed (`name`, `options.placed_count`, `teams`…).
    pub changed: Vec<String>,
    /// The fields both changed, with the value that was replaced.
    pub both: Vec<(String, Value)>,
}

/// The author changed the class `seen` into `mine`; meanwhile it became `current`. Its status is
/// never changed here (finalising and reopening are their own operations).
pub fn class(seen: &Class, current: &Class, mine: &Class) -> ClassMerged {
    let json = |c: &Class| match serde_json::to_value(c) {
        Ok(Value::Object(o)) => o,
        _ => Map::new(),
    };
    let (s, c, m) = (json(seen), json(current), json(mine));
    let mut out = c.clone();
    let (mut changed, mut both) = (Vec::new(), Vec::new());
    fields(&s, &c, &m, &mut out, "", &mut changed, &mut both);
    // The teams in their starting order: a sequence of their own.
    let teams = sequence(&seen.teams, &current.teams, &mine.teams);
    let mut class: Class = serde_json::from_value(Value::Object(out)).unwrap_or_else(|_| mine.clone());
    if mine.teams != seen.teams {
        changed.push("teams".into());
        if !teams.both.is_empty() {
            both.push(("teams".into(), serde_json::to_value(&current.teams).unwrap_or(Value::Null)));
        }
    }
    class.teams = teams.items;
    class.id = current.id.clone();
    class.status = current.status;
    ClassMerged { class, changed, both }
}

/// Field by field, inside the class's options and start list setup too (their fields are
/// structs; deeper values, such as a layout, count as one); `teams` and the status are not
/// fields here.
fn fields(seen: &Map<String, Value>, current: &Map<String, Value>, mine: &Map<String, Value>, out: &mut Map<String, Value>, path: &str, changed: &mut Vec<String>, both: &mut Vec<(String, Value)>) {
    let keys: BTreeSet<&String> = seen.keys().chain(mine.keys()).collect();
    for key in keys {
        if path.is_empty() && ["id", "status", "teams"].contains(&key.as_str()) {
            continue;
        }
        let (s, c, m) = (seen.get(key), current.get(key), mine.get(key));
        if s == m {
            continue;
        }
        let name = if path.is_empty() { key.clone() } else { format!("{path}.{key}") };
        let nested = path.is_empty() && ["options", "start_list"].contains(&key.as_str());
        if let (true, Some(Value::Object(s)), Some(Value::Object(c)), Some(Value::Object(m))) = (nested, s, c, m) {
            let mut inner = c.clone();
            fields(s, c, m, &mut inner, &name, changed, both);
            out.insert(key.clone(), Value::Object(inner));
            continue;
        }
        changed.push(name.clone());
        if c != s && c != m {
            both.push((name, c.cloned().unwrap_or(Value::Null)));
        }
        match m {
            Some(v) => out.insert(key.clone(), v.clone()),
            None => out.remove(key),
        };
    }
}
