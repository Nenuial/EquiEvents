//! Preparing classes and changing entries: what the office does before and during a show, as
//! changes to the show (operations). Each function reads the show and returns the changes; it
//! changes nothing itself.
//!
//! - **set up** a class from a published class: its entries, the start order drawn;
//! - **draw** the start order again; **reorder** a starter; give **start numbers**;
//! - **split** a class into sections (10a, 10b, …), **move** a rider to another section;
//! - **edit an entry**: does not start, hors concours, another horse;
//! - **rename** a class, set its time.
//!
//! [`inverse`] gives the changes that undo others: the office's undo.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use rust_decimal::Decimal;
use scoring::ranking::compare_names;
use show::model::*;
use show::oplog::Change;
use show::State;

fn class<'a>(state: &'a State, id: &ClassId) -> Result<&'a Class, String> {
    let class = state.classes.get(id).ok_or_else(|| format!("unknown class {id}"))?;
    if state.status_of(class) == ClassStatus::Final {
        return Err(format!("class {} is final; reopen it first", class.number));
    }
    Ok(class)
}

fn starts(state: &State, class: &ClassId) -> Vec<Start> {
    let mut starts = state.starts.get(class).cloned().unwrap_or_default();
    starts.sort_by_key(|s| s.order);
    starts
}

pub(crate) fn has_results(state: &State, class: &ClassId) -> bool {
    state.results.keys().any(|(c, _)| c == class) || state.sheets.keys().any(|(c, _, _)| c == class) || state.team_results.keys().any(|(c, _)| c == class) || state.team_sheets.keys().any(|(c, _, _)| c == class)
}

/// Fisher–Yates with a small generator; the seed comes from the caller.
fn shuffle<T>(items: &mut [T], seed: u64) {
    let mut x = seed | 1;
    for i in (1..items.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        items.swap(i, (x % (i as u64 + 1)) as usize);
    }
}

/// A start order where each rider's horses are as far apart as possible (U11): a rider with
/// `k` horses starts at the beginning, at even steps and at the end (2: B1 … B2; 3: C1 … C2 …
/// C3); where blocks meet, the riders with most horses are outermost (C1 B1 … B2 C3); the riders
/// with one horse fill the stretches between, in proportion to their length. Inside a block the
/// riders keep their order of first appearance in `items`, the same in each of their blocks, so
/// every rider of a group gets the same gap; one-horse riders keep theirs. A rider's horses
/// keep their order in `items`.
///
/// `items`: (what starts, its rider), in the order to keep (drawn, or sorted).
pub fn spread_order<T: Clone, R: PartialEq + Clone>(items: &[(T, R)]) -> Vec<T> {
    // Riders in order of first appearance, with their horses.
    let mut riders: Vec<(R, Vec<T>)> = Vec::new();
    for (item, rider) in items {
        match riders.iter_mut().find(|(r, _)| r == rider) {
            Some((_, horses)) => horses.push(item.clone()),
            None => riders.push((rider.clone(), vec![item.clone()])),
        }
    }
    let ones: Vec<T> = riders.iter().filter(|(_, h)| h.len() == 1).map(|(_, h)| h[0].clone()).collect();
    let mut groups: Vec<usize> = riders.iter().map(|(_, h)| h.len()).filter(|k| *k > 1).collect();
    groups.sort_unstable();
    groups.dedup();
    if groups.is_empty() {
        return ones;
    }
    // The blocks: (where, as a fraction num/den of the class; group k; its j-th horses).
    let mut blocks: Vec<(u64, u64, usize, usize)> = groups.iter().flat_map(|&k| (0..k).map(move |j| (j as u64, (k - 1) as u64, k, j))).collect();
    let at = |b: &(u64, u64, usize, usize)| (b.0 as f64) / (b.1 as f64);
    let rank = |b: &(u64, u64, usize, usize)| {
        // At the start the biggest groups first; at the end last; between, the biggest first.
        let end = b.0 == b.1;
        if end { b.2 as i64 } else { -(b.2 as i64) }
    };
    blocks.sort_by(|a, b| (a.0 * b.1).cmp(&(b.0 * a.1)).then(rank(a).cmp(&rank(b))));
    // Distinct places and the stretches between them, for the one-horse riders.
    let mut places: Vec<f64> = Vec::new();
    for b in &blocks {
        let p = at(b);
        if places.last().is_none_or(|last| (p - last).abs() > 1e-9) {
            places.push(p);
        }
    }
    let stretches: Vec<f64> = places.windows(2).map(|w| w[1] - w[0]).collect();
    let total: f64 = stretches.iter().sum();
    // Largest remainders: as many one-horse riders in each stretch as its share, all placed.
    let n = ones.len();
    let exact: Vec<f64> = stretches.iter().map(|s| if total > 0.0 { s / total * n as f64 } else { 0.0 }).collect();
    let mut counts: Vec<usize> = exact.iter().map(|e| e.floor() as usize).collect();
    let mut left = n - counts.iter().sum::<usize>();
    let mut by_remainder: Vec<usize> = (0..exact.len()).collect();
    by_remainder.sort_by(|a, b| (exact[*b] - exact[*b].floor()).partial_cmp(&(exact[*a] - exact[*a].floor())).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(b)));
    for i in by_remainder {
        if left == 0 {
            break;
        }
        counts[i] += 1;
        left -= 1;
    }
    let mut ones = ones.into_iter();
    let mut out = Vec::with_capacity(items.len());
    let mut place = 0;
    let mut i = 0;
    while i < blocks.len() {
        let here = at(&blocks[i]);
        while i < blocks.len() && (at(&blocks[i]) - here).abs() < 1e-9 {
            let (_, _, k, j) = blocks[i];
            out.extend(riders.iter().filter(|(_, h)| h.len() == k).map(|(_, h)| h[j].clone()));
            i += 1;
        }
        if place < counts.len() {
            out.extend(ones.by_ref().take(counts[place]));
            place += 1;
        }
    }
    out.extend(ones);
    out
}

/// A class's entries in the U11 order (see [`spread`]), for other modules.
pub(crate) fn spread_entries(state: &State, published: &PublishedClassId, entries: Vec<EntryId>) -> Vec<EntryId> {
    spread(state, published, entries)
}

/// The pony size of an entry's horse (A, B, C, D), if a pony.
pub(crate) fn pony_size(entry: &Entry) -> Option<String> {
    entry.horses_on_entry.first().and_then(|r| r["pony_groessenkategorie"].as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_uppercase)
}

/// A class's entries in the U11 order, grouped as Rosson groups a start order: by level (when the
/// class has levels; the lower first), then, in a pony class, by pony size (A, B, C, D, then
/// those without a size; the course is set for each), each group spread on its own. A class is
/// a pony class when most of its horses are ponies: a few ponies in a horse class ride the
/// same course and are not grouped (their riders' horses would end up side by side).
fn spread(state: &State, published: &PublishedClassId, entries: Vec<EntryId>) -> Vec<EntryId> {
    let levels = state.published_classes.get(published).is_some_and(|p| p.levels > 1);
    let ponies = entries.iter().filter(|e| state.entries.get(e).and_then(pony_size).is_some()).count();
    let pony_class = 2 * ponies > entries.len();
    // (level, pony size) → its entries with their riders.
    type Groups = BTreeMap<(i64, String), Vec<(EntryId, Option<PersonId>)>>;
    let mut groups: Groups = BTreeMap::new();
    for id in &entries {
        let e = state.entries.get(id);
        let level = if levels { e.and_then(|e| e.stage).unwrap_or(i64::MAX) } else { 0 };
        // "~" sorts after the sizes: ponies without one last.
        let size = if pony_class { e.and_then(pony_size).unwrap_or_else(|| "~".into()) } else { String::new() };
        groups.entry((level, size)).or_default().push((id.clone(), e.map(|e| e.rider.clone())));
    }
    groups.values().flat_map(|items| spread_order(items)).collect()
}

/// The starts of a class in a drawn order: riders (a rider's horses spread out, U11), or teams
/// and their riders in rotation.
fn drawn(state: &State, class: &Class, entries: Vec<EntryId>, seed: u64) -> (Vec<TeamId>, Vec<Start>) {
    let team_class = state.published_classes.get(&class.published).is_some_and(|p| p.team_size.is_some());
    if !team_class {
        let mut entries = entries;
        shuffle(&mut entries, seed);
        let entries = spread(state, &class.published, entries);
        let starts = entries.into_iter().enumerate().map(|(i, entry)| Start { entry, order: i as u32 + 1, start_number: None, team_order: None }).collect();
        return (Vec::new(), starts);
    }
    // Teams drawn; each team's riders in entry order, their place in the rotation.
    let mut by_team: BTreeMap<Option<TeamId>, Vec<EntryId>> = BTreeMap::new();
    for e in entries {
        by_team.entry(state.entries.get(&e).and_then(|e| e.team.clone())).or_default().push(e);
    }
    let alone = by_team.remove(&None).unwrap_or_default();
    let mut teams: Vec<TeamId> = by_team.keys().flatten().cloned().collect();
    shuffle(&mut teams, seed);
    let mut starts = Vec::new();
    for (t, team) in teams.iter().enumerate() {
        for (i, entry) in by_team[&Some(team.clone())].iter().enumerate() {
            starts.push(Start { entry: entry.clone(), order: i as u32 + 1, start_number: None, team_order: Some(t as u32 + 1) });
        }
    }
    let after = starts.iter().map(|s| s.order).max().unwrap_or(0);
    starts.extend(alone.into_iter().enumerate().map(|(i, entry)| Start { entry, order: after + i as u32 + 1, start_number: None, team_order: None }));
    (teams, starts)
}

/// Set up a class from a published class: all its entries (not the waiting list), the start
/// order drawn. The class takes the published class's id, as in Rosson.
pub fn set_up(state: &State, published: &PublishedClassId, seed: u64) -> Result<Vec<Change>, String> {
    if state.published_classes.get(published).is_some_and(|p| p.discipline == "CC" || p.discipline == "CA") {
        return set_up_phases(state, published, seed);
    }
    let mut class = new_class(state, published)?;
    let entries: Vec<EntryId> = state.entries.values().filter(|e| &e.published == published && !e.waiting_list).map(|e| e.id.clone()).collect();
    let (teams, starts) = drawn(state, &class, entries, seed);
    class.teams = teams;
    Ok(vec![Change::class_set(class.clone()), Change::starts_set(class.id, starts)])
}

/// A published class's class, not yet set up (its id is the published class's, as in Rosson).
fn new_class(state: &State, published: &PublishedClassId) -> Result<Class, String> {
    let p = state.published_classes.get(published).ok_or_else(|| format!("unknown published class {published}"))?;
    if state.classes.values().any(|c| &c.published == published) {
        return Err(format!("class {} is already set up", p.number));
    }
    let programme = p.category_id.and_then(|k| state.categories.get(&k)).and_then(|k| k.programme);
    Ok(Class {
        id: ClassId(published.0.to_string()),
        published: published.clone(),
        number: p.number.clone(),
        // The class's own name (Rosson's « Nom de l'épreuve », a prize's): typed by the office;
        // until then lists say its category and scoring table (views::class_title).
        name: String::new(),
        date: p.date.clone(),
        scoring_table: p.scoring_table,
        programme,
        options: ClassOptions::default(),
        status: ClassStatus::Planned,
        extra_fields: None,
        position: None,
        time_text: String::new(),
        scoring_text: p.scoring_text.clone(),
        logo: None,
        start_numbers_assigned: false,
        teams: Vec::new(),
        start_list: StartListSetup::default(),
        sponsor: String::new(),
        phase: None,
        phases: Vec::new(),
    })
}

/// Eventing: a phase to another place in its class's order (dragged there: `to` from 0), the
/// order that breaks ties in the overall ranking.
pub fn move_phase(state: &State, phase: &ClassId, to: usize) -> Result<Vec<Change>, String> {
    let c = class(state, phase)?;
    let parent = c.phase.as_ref().and_then(|p| state.classes.get(&p.of)).ok_or_else(|| format!("class {} is not a phase", c.number))?;
    let mut phases = parent.phases.clone();
    let from = phases.iter().position(|p| p == phase).ok_or("not one of its class's phases")?;
    if to >= phases.len() || to == from {
        return Ok(Vec::new());
    }
    let moved = phases.remove(from);
    phases.insert(to, moved);
    Ok(vec![Change::class_set(Class { phases, ..parent.clone() })])
}

/// Driving: a presentation judged on its own, standing (Prüfung A I; Fahrreglement 6.2.1, 13.10
/// §2; S44), added to a class made of phases as its first phase: `<class>_pr`, its riders in the
/// first phase's order.
pub fn add_presentation(state: &State, id: &ClassId) -> Result<Vec<Change>, String> {
    let parent = class(state, id)?;
    if parent.phases.is_empty() || state.driving_event(parent).is_none() {
        return Err(format!("class {} is not a driving class made of phases", parent.number));
    }
    let phase_id = ClassId(format!("{}_{}", parent.id.0, PhaseKind::Presentation.code()));
    if parent.phases.contains(&phase_id) || state.classes.contains_key(&phase_id) {
        return Err(format!("class {} has a presentation", parent.number));
    }
    let first = parent.phases.first().and_then(|p| state.classes.get(p)).ok_or("the class has no phases")?;
    let phase = Class {
        id: phase_id.clone(),
        phase: Some(Phase { of: parent.id.clone(), kind: PhaseKind::Presentation }),
        programme: None,
        scoring_table: None,
        phases: Vec::new(),
        options: ClassOptions { driving: None, ..first.options.clone() },
        ..first.clone()
    };
    let mut phases = parent.phases.clone();
    phases.insert(0, phase_id.clone());
    Ok(vec![Change::class_set(phase), Change::starts_set(phase_id, state.starts.get(&first.id).cloned().unwrap_or_default()), Change::class_set(Class { phases, ..parent.clone() })])
}

/// A driving phase's settings to start from, as Rosson's (`setDefaults`): marathon 5 obstacles
/// at 0.25 a second, cones on table A in one round, derby 0.25, open by time, multitest B4.
fn driving_defaults(kind: PhaseKind) -> Option<DrivingSettings> {
    let quarter = rust_decimal::Decimal::new(25, 2);
    match kind {
        PhaseKind::Marathon => Some(DrivingSettings { factor: Some(quarter), obstacles: Some(5), ..Default::default() }),
        PhaseKind::Cones => Some(DrivingSettings { cones: Some(ConesKind::A), rounds: Some(1), ..Default::default() }),
        PhaseKind::Derby => Some(DrivingSettings { time_factor: Some(quarter), ..Default::default() }),
        PhaseKind::Open => Some(DrivingSettings { open: Some(OpenKind::Time), ..Default::default() }),
        PhaseKind::Multitest => Some(DrivingSettings { multitest: Some("B4".into()), ..Default::default() }),
        _ => None,
    }
}

/// Swiss Equestrian's scoring tables a phase is scored on by default, as Rosson: jumping on A
/// with time (AZ, 14), cross-country on A with ideal time (AZI, 35).
const TABLE_AZ: u32 = 14;
const TABLE_AZI: u32 = 35;

/// Eventing (CC): the class made of phases (its overall ranking, no starters of its own) and a
/// class per phase, `<class>_cd`, `_cs`, `_cross`, as Rosson's "Startlisten erstellen": the phases
/// Swiss Equestrian lists (`kombipruefung_disziplinen`; dressage, jumping, cross when none), in
/// Rosson's order (dressage, cross, jumping); every entry in each, in the same drawn order.
fn set_up_phases(state: &State, published: &PublishedClassId, seed: u64) -> Result<Vec<Change>, String> {
    let p = state.published_classes.get(published).ok_or_else(|| format!("unknown published class {published}"))?;
    let mut parent = new_class(state, published)?;
    // Eventing's phases (`kombipruefung_disziplinen`), or driving's (`fahren_zwischenpruefungen`).
    let driving = p.discipline == "CA";
    let names: Vec<&serde_json::Value> = if driving { p.driving_phases.iter().collect() } else { p.record["kombipruefung_disziplinen"].as_array().into_iter().flatten().collect() };
    let mut kinds: Vec<PhaseKind> = names.into_iter().filter_map(|n| n.as_str().and_then(PhaseKind::from_name)).collect();
    if kinds.is_empty() {
        kinds = if driving { vec![PhaseKind::Dressage, PhaseKind::Marathon, PhaseKind::Cones] } else { vec![PhaseKind::Dressage, PhaseKind::Jumping, PhaseKind::Cross] };
    }
    kinds.sort_by_key(|k| k.default_order());
    kinds.dedup();
    let entries: Vec<EntryId> = state.entries.values().filter(|e| &e.published == published && !e.waiting_list).map(|e| e.id.clone()).collect();
    let (_, starts) = drawn(state, &parent, entries, seed);
    let table = |id: u32| state.scoring_tables.contains_key(&id).then_some(id);
    let phases: Vec<Class> = kinds
        .iter()
        .map(|&kind| Class {
            id: ClassId(format!("{}_{}", parent.id.0, kind.code())),
            phase: Some(Phase { of: parent.id.clone(), kind }),
            programme: if kind == PhaseKind::Dressage { parent.programme } else { None },
            scoring_table: match kind {
                PhaseKind::Jumping => parent.scoring_table.filter(|t| *t != TABLE_AZI).or_else(|| table(TABLE_AZ)),
                PhaseKind::Cross => table(TABLE_AZI),
                _ => None,
            },
            options: ClassOptions { driving: driving_defaults(kind), ..parent.options.clone() },
            ..parent.clone()
        })
        .collect();
    parent.phases = phases.iter().map(|c| c.id.clone()).collect();
    parent.programme = None;
    parent.scoring_table = None;
    let mut changes = vec![Change::class_set(parent)];
    for phase in phases {
        let id = phase.id.clone();
        changes.push(Change::class_set(phase));
        changes.push(Change::starts_set(id, starts.clone()));
    }
    Ok(changes)
}

/// A rider and horse (or guest horse), as start orders after another class match them.
type Pair = (PersonId, Option<HorseId>, Option<String>);

/// Set up a follow-up class (Rosson's "Folgeprüfung erstellen"): the sections of another
/// class, and its order: a rider and horse who start there start in the same section at the
/// same place; the others at the end of the last section. Start numbers stay with the riders
/// when the show's numbers are fixed.
pub fn set_up_following(state: &State, published: &PublishedClassId, model: &PublishedClassId) -> Result<Vec<Change>, String> {
    let class = new_class(state, published)?;
    if published == model {
        return Err("a class does not follow itself".into());
    }
    if state.published_classes.get(published).is_some_and(|p| p.team_size.is_some()) {
        return Err("team classes: the order is set by teams".into());
    }
    let mut sections: Vec<&Class> = state.classes.values().filter(|c| &c.published == model).collect();
    if sections.is_empty() {
        return Err("the class to follow has no start list".into());
    }
    sections.sort_by(|a, b| a.number.len().cmp(&b.number.len()).then(a.number.cmp(&b.number)));
    let pair = |e: &Entry| (e.rider.clone(), e.horse.clone(), e.guest_horse.clone());
    // Where each pair starts in the class followed: (section, place).
    let mut places: Vec<(Pair, (usize, u32))> = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        for s in starts(state, &section.id) {
            if let Some(e) = state.entries.get(&s.entry) {
                places.push((pair(e), (i, s.order)));
            }
        }
    }
    let mut lists: Vec<Vec<(u32, EntryId)>> = vec![Vec::new(); sections.len()];
    let mut others = Vec::new();
    for e in state.entries.values().filter(|e| &e.published == published && !e.waiting_list) {
        match places.iter().find(|(p, _)| *p == pair(e)) {
            Some((_, (i, order))) => lists[*i].push((*order, e.id.clone())),
            None => others.push(e.id.clone()),
        }
    }
    let last = lists.len() - 1;
    let mut changes = Vec::new();
    for (i, mut list) in lists.into_iter().enumerate() {
        list.sort();
        let mut entries: Vec<EntryId> = list.into_iter().map(|(_, e)| e).collect();
        if i == last {
            entries.append(&mut others);
        }
        let section = if sections.len() == 1 {
            class.clone()
        } else {
            let letter = (b'a' + i as u8) as char;
            Class { id: ClassId(format!("{}{letter}", class.id.0)), number: format!("{}{letter}", class.number), ..class.clone() }
        };
        let starts = entries.into_iter().enumerate().map(|(k, entry)| Start { entry, order: k as u32 + 1, start_number: None, team_order: None }).collect();
        changes.push(Change::class_set(section.clone()));
        changes.push(Change::starts_set(section.id, starts));
    }
    Ok(changes)
}

/// Draw the start order again (start numbers are taken away: they follow the order).
pub fn draw(state: &State, id: &ClassId, seed: u64) -> Result<Vec<Change>, String> {
    let class = class(state, id)?;
    let entries = starts(state, id).into_iter().map(|s| s.entry).collect();
    let (teams, starts) = drawn(state, class, entries, seed);
    let class = Class { teams, start_numbers_assigned: false, ..class.clone() };
    Ok(vec![Change::class_set(class), Change::starts_set(id.clone(), starts)])
}

/// Move a starter to a place in the start order (1 = first).
pub fn move_start(state: &State, id: &ClassId, entry: &EntryId, to: usize) -> Result<Vec<Change>, String> {
    let class = class(state, id)?;
    if !class.teams.is_empty() {
        return Err("team classes: the order is set by teams (not yet in the app)".into());
    }
    let mut starts = starts(state, id);
    let from = starts.iter().position(|s| &s.entry == entry).ok_or("not a starter of this class")?;
    let start = starts.remove(from);
    starts.insert(to.clamp(1, starts.len() + 1) - 1, start);
    for (i, s) in starts.iter_mut().enumerate() {
        s.order = i as u32 + 1;
    }
    Ok(vec![Change::starts_set(id.clone(), starts)])
}

/// Start numbers from `first`, in the drawn order (team classes: team by team).
pub fn number(state: &State, id: &ClassId, first: u32) -> Result<Vec<Change>, String> {
    let class = class(state, id)?;
    // Eventing: a rider has one number in every phase (Rosson), given in this phase's order.
    if let Some(parent) = class.phase.as_ref().and_then(|p| state.classes.get(&p.of)) {
        let mut order = starts(state, id);
        order.sort_by_key(|s| s.order);
        let numbers: BTreeMap<EntryId, u32> = order.iter().enumerate().map(|(i, s)| (s.entry.clone(), first + i as u32)).collect();
        let mut changes = Vec::new();
        for phase in parent.phases.iter().filter_map(|p| state.classes.get(p)) {
            let starts: Vec<Start> = starts(state, &phase.id).into_iter().map(|s| Start { start_number: numbers.get(&s.entry).copied(), ..s }).collect();
            changes.push(Change::class_set(Class { start_numbers_assigned: true, ..phase.clone() }));
            changes.push(Change::starts_set(phase.id.clone(), starts));
        }
        return Ok(changes);
    }
    if !class.teams.is_empty() {
        return number_teams(state, class);
    }
    let mut starts = starts(state, id);
    let team = |s: &Start| state.entries.get(&s.entry).and_then(|e| e.team.as_ref()).and_then(|t| class.teams.iter().position(|x| x == t)).unwrap_or(usize::MAX);
    starts.sort_by_key(|s| (team(s), s.order));
    for (i, s) in starts.iter_mut().enumerate() {
        s.start_number = Some(first + i as u32);
    }
    // Riders this computer does not know yet (entered on another, offline) get the next numbers.
    let numbered = Change::StartsSet { class: id.clone(), starts, seen: None, next_numbers: Vec::new(), numbered: true };
    Ok(vec![Change::class_set(Class { start_numbers_assigned: true, ..class.clone() }), numbered])
}

/// Start numbers of a team class, as Rosson gives them: a team's number is its place in the
/// order; a rider's, its place in the team then the team's in two digits (rider 1 of team 3: 103,
/// rider 2 of team 12: 212). Dressage team classes: the teams only. Riders without a team (they
/// start alone in a team class) after the highest number.
fn number_teams(state: &State, class: &Class) -> Result<Vec<Change>, String> {
    let dressage = state.class_discipline(class) == "CD";
    let place = |t: &TeamId| class.teams.iter().position(|x| x == t).map(|i| i as u32 + 1);
    let mut starts = starts(state, &class.id);
    for s in starts.iter_mut() {
        let team = state.entries.get(&s.entry).and_then(|e| e.team.as_ref()).and_then(place);
        s.start_number = match team {
            Some(_) if dressage => None,
            Some(t) => format!("{}{t:02}", s.order).parse().ok(),
            None => None,
        };
    }
    let mut next = starts.iter().filter_map(|s| s.start_number).max().unwrap_or(0) + 1;
    let mut alone: Vec<&mut Start> = starts.iter_mut().filter(|s| state.entries.get(&s.entry).is_some_and(|e| e.team.as_ref().and_then(place).is_none())).collect();
    alone.sort_by_key(|s| s.order);
    for s in alone {
        s.start_number = Some(next);
        next += 1;
    }
    let mut changes: Vec<Change> = class
        .teams
        .iter()
        .enumerate()
        .filter_map(|(i, t)| state.teams.get(t).filter(|team| team.start_number != Some(i as u32 + 1)).map(|team| Change::TeamSet { team: Team { start_number: Some(i as u32 + 1), ..team.clone() } }))
        .collect();
    changes.push(Change::class_set(Class { start_numbers_assigned: true, ..class.clone() }));
    changes.push(Change::starts_set(class.id.clone(), starts));
    Ok(changes)
}

/// Split a class into `parts` sections (10 → 10a, 10b, …), the starters in their order, in
/// blocks of (nearly) the same size; team classes by whole teams, in their rotation (riders
/// without a team in the last section). Only before it has results (docs/deviations.md S19:
/// a class with more than 90 entries is divided before it runs).
pub fn split(state: &State, id: &ClassId, parts: usize) -> Result<Vec<Change>, String> {
    let class = class(state, id)?;
    let starts = starts(state, id);
    if has_results(state, id) {
        return Err(format!("class {} has results", class.number));
    }
    if !class.teams.is_empty() {
        return split_teams(state, class, starts, parts);
    }
    if parts < 2 || parts > starts.len().max(2) || parts > 26 {
        return Err(format!("{parts} sections for {} starters?", starts.len()));
    }
    let mut changes = Vec::new();
    let size = starts.len() / parts;
    let bigger = starts.len() % parts;
    let mut rest = starts.as_slice();
    for i in 0..parts {
        let n = size + usize::from(i < bigger);
        let (mine, more) = rest.split_at(n);
        rest = more;
        let part = section(class, i);
        let starts = mine.iter().enumerate().map(|(k, s)| Start { order: k as u32 + 1, ..s.clone() }).collect();
        changes.push(Change::class_set(part.clone()));
        changes.push(Change::starts_set(part.id, starts));
    }
    changes.push(Change::ClassRemoved { class: id.clone() });
    Ok(changes)
}

/// A section of a class: `10` → `10a`, `10b`, …
fn section(class: &Class, i: usize) -> Class {
    let letter = (b'a' + i as u8) as char;
    Class { id: ClassId(format!("{}{letter}", class.id.0)), number: format!("{}{letter}", class.number), ..class.clone() }
}

/// A team class split by whole teams: (nearly) as many teams in each section, in the rotation's
/// order; each team keeps its riders' places, its team order counted in its section.
fn split_teams(state: &State, class: &Class, starts: Vec<Start>, parts: usize) -> Result<Vec<Change>, String> {
    let teams = &class.teams;
    if parts < 2 || parts > teams.len() || parts > 26 {
        return Err(format!("{parts} sections for {} teams?", teams.len()));
    }
    let team_of = |s: &Start| state.entries.get(&s.entry).and_then(|e| e.team.clone()).filter(|t| teams.contains(t));
    let (size, bigger) = (teams.len() / parts, teams.len() % parts);
    let mut changes = Vec::new();
    let mut first = 0;
    for i in 0..parts {
        let n = size + usize::from(i < bigger);
        let mine: Vec<TeamId> = teams[first..first + n].to_vec();
        first += n;
        let mut list: Vec<Start> = starts
            .iter()
            .filter_map(|s| {
                let t = team_of(s)?;
                let at = mine.iter().position(|x| x == &t)?;
                Some(Start { team_order: Some(at as u32 + 1), ..s.clone() })
            })
            .collect();
        if i == parts - 1 {
            // Riders without a team (or of a team not in the rotation): after the teams.
            let after = list.iter().map(|s| s.order).max().unwrap_or(0);
            list.extend(starts.iter().filter(|s| team_of(s).is_none()).enumerate().map(|(k, s)| Start { order: after + k as u32 + 1, team_order: None, ..s.clone() }));
        }
        let part = Class { teams: mine, ..section(class, i) };
        changes.push(Change::class_set(part.clone()));
        changes.push(Change::starts_set(part.id, list));
    }
    changes.push(Change::ClassRemoved { class: class.id.clone() });
    Ok(changes)
}

/// Move a rider to another section of the same published class, at the end of its start list.
pub fn move_entry(state: &State, entry: &EntryId, from: &ClassId, to: &ClassId) -> Result<Vec<Change>, String> {
    let (a, b) = (class(state, from)?, class(state, to)?);
    if a.published != b.published {
        return Err("only to another section of the same class".into());
    }
    let mut out = starts(state, from);
    let start = out.iter().position(|s| &s.entry == entry).map(|i| out.remove(i)).ok_or("not a starter of this class")?;
    for (i, s) in out.iter_mut().enumerate() {
        s.order = i as u32 + 1;
    }
    let mut into = starts(state, to);
    let order = into.iter().map(|s| s.order).max().unwrap_or(0) + 1;
    into.push(Start { order, ..start });
    Ok(vec![Change::starts_set(from.clone(), out), Change::starts_set(to.clone(), into)])
}

/// Does not start / starts again; hors concours or not.
pub fn edit_entry(state: &State, entry: &EntryId, not_starting: Option<bool>, hors_concours: Option<bool>) -> Result<Vec<Change>, String> {
    state.entries.get(entry).ok_or_else(|| format!("unknown entry {entry}"))?;
    let edit = EntryEdit { status: not_starting.map(|n| if n { EntryStatus::NotStarting } else { EntryStatus::Entered }), hors_concours, horse: None , ..Default::default() };
    Ok(vec![Change::EntryEdited { entry: entry.clone(), edit }])
}

/// Another horse for entries (a pair's, in the classes the office picked): one the show knows,
/// one of Swiss Equestrian's registry (copied into the show), or a guest horse by name (riders
/// with a guest licence only). Written on each entry as Rosson writes it (it goes to Swiss
/// Equestrian with the results).
pub fn change_horse(state: &State, entries: &[EntryId], pick: &crate::entries::HorsePick) -> Result<Vec<Change>, String> {
    use crate::entries::HorsePick;
    if entries.is_empty() {
        return Err("no entry to change".into());
    }
    let mut changes = Vec::new();
    let mut records: std::collections::HashMap<u64, serde_json::Value> = state.horses.values().filter(|h| !h.record.is_null()).map(|h| (h.id.0, h.record.clone())).collect();
    records.insert(se_api::load::PLACEHOLDER_HORSE, se_api::load::placeholder_horse());
    let (id, guest, nennung) = match pick {
        HorsePick::Known(h) => {
            state.horses.get(h).ok_or_else(|| format!("unknown horse {h}"))?;
            (h.clone(), None, json!({ "pferd_id": h.0 }))
        }
        HorsePick::Registry(h) => {
            if h.id.0 == se_api::load::PLACEHOLDER_HORSE {
                return Err("not a horse of the registry".into());
            }
            if !state.horses.contains_key(&h.id) {
                crate::entries::starts_this_year(state, h)?;
                changes.push(Change::HorseAdded { horse: (**h).clone() });
                records.insert(h.id.0, h.record.clone());
            }
            (h.id.clone(), None, json!({ "pferd_id": h.id.0 }))
        }
        HorsePick::Guest(name) => {
            let name = name.trim();
            if name.is_empty() {
                return Err("a guest horse needs a name".into());
            }
            (HorseId(se_api::load::PLACEHOLDER_HORSE), Some(name.to_string()), json!({ "pferd_id": se_api::load::PLACEHOLDER_HORSE, "gastpferd_name": name }))
        }
    };
    for entry in entries {
        let e = state.entries.get(entry).ok_or_else(|| format!("unknown entry {entry}"))?;
        crate::entries::no_final(state, entry)?;
        if guest.is_some() {
            crate::entries::guest_allowed(state.persons.get(&e.rider).ok_or_else(|| format!("unknown rider {}", e.rider))?)?;
        }
        let phases = state.published_classes.get(&e.published).map(|p| p.driving_phases.clone()).unwrap_or_default();
        let horses_on_entry = se_api::load::horses_on_entry(&nennung, &records, &phases);
        let edit = EntryEdit { horse: Some(HorseOnEntry { horse: Some(id.clone()), guest_horse: guest.clone(), horses_on_entry }), ..Default::default() };
        changes.push(Change::EntryEdited { entry: entry.clone(), edit });
    }
    Ok(changes)
}

/// A day as YYYY-MM-DD, a real one.
fn is_date(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    let [y, m, d] = parts.as_slice() else { return false };
    let number = |s: &str, len: usize| (s.len() == len && s.chars().all(|c| c.is_ascii_digit())).then(|| s.parse::<u32>().ok()).flatten();
    let (Some(y), Some(m), Some(d)) = (number(y, 4), number(m, 2), number(d, 2)) else { return false };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    (1..=12).contains(&m) && d >= 1 && d <= days[m as usize - 1]
}

/// Rename a class, set its day (any day: a section can go to a day the show did not plan) and
/// the time it starts ("08h00", "à la suite"), or (dressage) the programme ridden: categories
/// without a set programme leave the choice to the office.
/// What the office changes on a class with `edit_class` (`None`: unchanged).
#[derive(Default)]
pub struct ClassEdit {
    pub name: Option<String>,
    pub date: Option<String>,
    pub time: Option<String>,
    pub programme: Option<Option<u32>>,
    pub qualifier: Option<Option<ClassId>>,
    /// A jumping class's scoring table, before results.
    pub scoring_table: Option<u32>,
}

pub fn edit_class(state: &State, id: &ClassId, edit: ClassEdit) -> Result<Vec<Change>, String> {
    let class = class(state, id)?;
    if let Some(date) = &edit.date {
        if !is_date(date) {
            return Err(format!("{date} is not a date (YYYY-MM-DD)"));
        }
    }
    if let Some(Some(q)) = &edit.qualifier {
        if q == id || !state.classes.contains_key(q) {
            return Err(format!("class {q} cannot be the qualifier of class {}", class.number));
        }
    }
    if let Some(Some(p)) = edit.programme {
        state.programmes.get(&p).ok_or_else(|| format!("unknown programme {p}"))?;
    }
    if edit.programme.is_some() && edit.programme != Some(class.programme) && (has_results(state, id) || state.aside.keys().any(|(c, _)| c == id)) {
        return Err(format!("class {} has marks: the programme cannot change", class.number));
    }
    let mut class = class.clone();
    if let Some(table) = edit.scoring_table.filter(|t| class.scoring_table != Some(*t)) {
        state.scoring_tables.get(&table).ok_or_else(|| format!("unknown scoring table {table}"))?;
        if class.scoring_table.is_none() {
            return Err(format!("class {} has no scoring table", class.number));
        }
        if has_results(state, id) {
            return Err(format!("class {} has results: delete them before changing its scoring table", class.number));
        }
        class.scoring_table = Some(table);
    }
    if let Some(p) = edit.programme {
        class.programme = p;
    }
    if let Some(name) = edit.name {
        class.name = name.trim().to_string();
    }
    if let Some(date) = edit.date {
        class.date = date;
    }
    if let Some(time) = edit.time {
        class.time_text = time.trim().to_string();
    }
    if let Some(qualifier) = edit.qualifier {
        class.options.qualifier = qualifier;
    }
    Ok(vec![Change::class_set(class)])
}

/// A jumping class's scoring table (Rosson's class preparation): any of Swiss Equestrian's
/// tables, before the class has results (they are entered in the table's fields).
/// A dressage test's definition changed (Rosson's developer mode): Swiss Equestrian's JSON,
/// read as the app reads tests before it is kept.
pub fn set_programme_definition(state: &State, id: u32, definition: &str) -> Result<Vec<Change>, String> {
    let before = state.programmes.get(&id).ok_or_else(|| format!("unknown test {id}"))?;
    let json: Value = serde_json::from_str(definition).map_err(|e| format!("not JSON: {e}"))?;
    let programme = ProgrammeDef { definition: json.to_string(), ..before.clone() };
    show::master::programme(&programme).map_err(|e| format!("not a test the app can read: {e}"))?;
    if serde_json::from_str::<Value>(&before.definition).ok() == Some(json) {
        return Ok(Vec::new());
    }
    Ok(vec![Change::ProgrammeSet { programme }])
}

/// A scoring table's definition changed (Rosson's developer mode), read as the app reads
/// tables before it is kept; not while a final class uses it (its results are official).
pub fn set_scoring_table_definition(state: &State, id: u32, definition: &str) -> Result<Vec<Change>, String> {
    let before = state.scoring_tables.get(&id).ok_or_else(|| format!("unknown scoring table {id}"))?;
    let json: Value = serde_json::from_str(definition).map_err(|e| format!("not JSON: {e}"))?;
    let table = ScoringTableDef { definition: json.to_string(), ..before.clone() };
    scoring::jumping::table::ScoringTable::from_definition(table.id, &table.code, &table.name, &table.definition).map_err(|e| format!("not a scoring table the app can read: {e:?}"))?;
    if serde_json::from_str::<Value>(&before.definition).ok() == Some(json) {
        return Ok(Vec::new());
    }
    if let Some(c) = state.classes.values().find(|c| c.scoring_table == Some(id) && c.status == ClassStatus::Final) {
        return Err(format!("class {} uses it and is final: reopen it first", c.number));
    }
    Ok(vec![Change::ScoringTableSet { table }])
}

/// A published class changed at the show, as Rosson lets the office (its info box): the category
/// as printed (empty: the category's code again) and, in jumping and combined classes, how many
/// levels (Stufen) it has; entries at a level above the new count lose it. Not while one of its
/// classes is final (their entries' levels are part of the results).
pub fn edit_published(state: &State, id: &PublishedClassId, category_text: Option<String>, levels: Option<u32>) -> Result<Vec<Change>, String> {
    let p = state.published_classes.get(id).ok_or_else(|| format!("unknown published class {id}"))?;
    let category_text = category_text.map(|t| t.trim().to_string()).map(|t| if t.is_empty() { p.category.clone() } else { t }).filter(|t| *t != p.category_text);
    let levels = levels.filter(|l| *l != p.levels && !(*l <= 1 && p.levels <= 1));
    let mut changes = Vec::new();
    if let Some(levels) = levels {
        if p.discipline == "CD" {
            return Err("a dressage class has no levels".into());
        }
        if levels > 9 {
            return Err("at most 9 levels".into());
        }
        if let Some(c) = state.classes.values().find(|c| c.published == *id && c.status == ClassStatus::Final) {
            return Err(format!("class {} is final: reopen it to change the levels", c.number));
        }
        changes.extend(
            state
                .entries
                .values()
                .filter(|e| e.published == *id && e.stage.is_some_and(|s| s > levels.max(1) as i64 || levels <= 1))
                .map(|e| Change::EntryEdited { entry: e.id.clone(), edit: EntryEdit { stage: Some(0), ..Default::default() } }),
        );
    }
    if category_text.is_some() || levels.is_some() {
        changes.insert(0, Change::PublishedClassEdited { published: id.clone(), category_text, levels });
    }
    Ok(changes)
}

pub fn set_scoring_table(state: &State, id: &ClassId, table: u32) -> Result<Vec<Change>, String> {
    let class = class(state, id)?;
    state.scoring_tables.get(&table).ok_or_else(|| format!("unknown scoring table {table}"))?;
    if class.scoring_table == Some(table) {
        return Ok(Vec::new());
    }
    if has_results(state, id) {
        return Err(format!("class {} has results: delete them before changing its scoring table", class.number));
    }
    let mut class = class.clone();
    class.scoring_table = Some(table);
    Ok(vec![Change::class_set(class)])
}

/// How a class's start list is laid out: plain, in blocks, or with start times (and breaks);
/// a line after the n-th starter (ground care, a section's title).
pub fn set_layout(state: &State, id: &ClassId, setup: StartListSetup) -> Result<Vec<Change>, String> {
    let class = class(state, id)?;
    match &setup.layout {
        StartListLayout::Blocks { size, .. } if *size == 0 => return Err("a block has at least one starter".into()),
        StartListLayout::StartTimes { minutes, .. } if *minutes == 0 => return Err("starts are at least a minute apart".into()),
        StartListLayout::StartTimes { breaks, .. } if breaks.iter().any(|(from, to)| from >= to) => return Err("a break ends after it starts".into()),
        _ => {}
    }
    if let Some((after, text)) = &setup.line_after {
        if *after == 0 || text.trim().is_empty() {
            return Err("the line goes after a starter (1 or more) and says something".into());
        }
    }
    let mut setup = setup;
    if let StartListLayout::StartTimes { breaks, .. } = &mut setup.layout {
        breaks.sort();
    }
    Ok(vec![Change::class_set(Class { start_list: setup, ..class.clone() })])
}

/// Give an official a role in a class (`role`: a class role's code, with a judge's position for
/// dressage judges), or take it away (`None`). One role per official and class.
/// Marks entered under an official's role in a class (a judge's sheets, riders' or teams'): the
/// results are theirs, so the role cannot go nor become another.
pub fn has_marks(state: &State, assignment: &AssignmentId) -> bool {
    state.sheets.keys().any(|(_, _, j)| j == assignment) || state.team_sheets.keys().any(|(_, _, j)| j == assignment)
}

pub fn set_role(state: &State, id: &ClassId, person: &PersonId, role: Option<(&str, Option<&str>)>, now_millis: u64, new_id: &mut dyn FnMut() -> String) -> Result<Vec<Change>, String> {
    let c = class(state, id)?;
    let existing: Vec<&Assignment> = state.assignments.values().filter(|a| a.class.as_ref() == Some(id) && &a.person == person).collect();
    // A judge with marks entered keeps the role (a dressage judge may change position).
    let marked = |a: &Assignment| has_marks(state, &a.id);
    let refused = || {
        let name = state.persons.get(person).map(|p| format!("{} {}", p.first_name, p.last_name)).or_else(|| state.officials.get(person).map(|o| format!("{} {}", o.first_name, o.last_name))).unwrap_or_else(|| person.to_string());
        format!("class {}: {name} has marks on the judges' sheets; delete those results first", c.number)
    };
    let Some((code, letter)) = role else {
        if existing.iter().any(|a| marked(a)) {
            return Err(refused());
        }
        return Ok(existing.iter().map(|a| Change::AssignmentRemoved { assignment: a.id.clone() }).collect());
    };
    if existing.iter().skip(1).any(|a| marked(a)) || existing.first().is_some_and(|a| marked(a) && a.role != code) {
        return Err(refused());
    }
    let def = state.class_roles().into_iter().find(|r| r.code == code).ok_or_else(|| format!("unknown role {code}"))?;
    // EquiEvents' own roles have no Swiss Equestrian id.
    let role_id = (!show::model::is_own_role(&def.code)).then_some(def.id);
    let letter = letter.map(|l| l.trim().to_uppercase()).filter(|l| !l.is_empty());
    let mut changes: Vec<Change> = existing.iter().skip(1).map(|a| Change::AssignmentRemoved { assignment: a.id.clone() }).collect();
    let assignment = match existing.first() {
        Some(a) => Assignment { role: def.code.clone(), role_id, letter, updated_at: Some(now_millis), ..(*a).clone() },
        None => Assignment {
            id: AssignmentId(new_id()),
            class: Some(id.clone()),
            person: person.clone(),
            role: def.code.clone(),
            letter,
            role_id,
            event: state.event.as_ref().map(|e| e.id.clone()),
            se_id: None,
            on_event: None,
            responsible: None,
            readonly: Some(false),
            days: None,
            updated_at: Some(now_millis),
            looked_up: false,
        },
    };
    changes.push(Change::AssignmentSet { assignment });
    Ok(changes)
}

/// Marks entered on a judge's sheet (`None` clears one), and, when given, the ticked penalties
/// (places in the rider's programme's penalty list). A mark is from 0 to 10, in halves; in
/// tenths in young horses' programmes (JP…, Dressurreglement 8.2 (2)).
pub fn edit_sheet(state: &State, id: &ClassId, entry: &EntryId, judge: &AssignmentId, marks: BTreeMap<String, Option<rust_decimal::Decimal>>, penalties: Option<Vec<usize>>, total: Option<Option<rust_decimal::Decimal>>) -> Result<Vec<Change>, String> {
    let class = class(state, id)?;
    // Totals per judge: a total, at most the rider's programme's maximum; no marks.
    if let Some(Some(t)) = total {
        if !class.options.totals_per_judge {
            return Err(format!("class {}: the judges mark each movement", class.number));
        }
        let individual = class.programme.and_then(|p| state.programmes.get(&p)).is_some_and(show::master::is_individual);
        let programme = if individual { state.entries.get(entry).and_then(|e| e.programme) } else { class.programme };
        let max = programme.and_then(|p| state.programmes.get(&p)).map(|p| p.max_points).filter(|m| !m.is_zero());
        if t < rust_decimal::Decimal::ZERO || max.is_some_and(|m| t > m) {
            return Err(format!("{t} is not a total (0 to the programme's maximum)"));
        }
    }
    if class.options.totals_per_judge && marks.values().any(Option::is_some) {
        return Err(format!("class {}: the judges give a total, not marks", class.number));
    }
    if class.phase.as_ref().map(|p| p.kind) == Some(PhaseKind::Multitest) {
        let items = scoring::driving::multitest_coefficients(class.options.driving.as_ref().and_then(|d| d.multitest.as_deref()).unwrap_or("B4")).len();
        check_multitest_marks(&marks, items)?;
        return Ok(vec![Change::SheetEdited { class: id.clone(), entry: entry.clone(), judge: judge.clone(), marks, penalties: None, total: None }]);
    }
    let individual = class.programme.and_then(|p| state.programmes.get(&p)).is_some_and(show::master::is_individual);
    let programme = if individual { state.entries.get(entry).and_then(|e| e.programme) } else { class.programme };
    check_marks(&marks, programme.and_then(|p| state.programmes.get(&p)).is_some_and(show::master::in_tenths))?;
    let penalties = match penalties {
        None => None,
        Some(places) => {
            let e = state.entries.get(entry).ok_or_else(|| format!("unknown entry {entry}"))?;
            let individual = class.programme.and_then(|p| state.programmes.get(&p)).is_some_and(show::master::is_individual);
            let programme = if individual { e.programme } else { class.programme };
            let def = programme.and_then(|p| state.programmes.get(&p)).ok_or("the class has no programme")?;
            Some(sheet_penalties(def, places)?)
        }
    };
    Ok(vec![Change::SheetEdited { class: id.clone(), entry: entry.clone(), judge: judge.clone(), marks, penalties, total }])
}

/// Team dressage: marks of a judge's sheet for a team, as [`edit_sheet`].
pub fn edit_team_sheet(state: &State, id: &ClassId, team: &TeamId, judge: &AssignmentId, marks: BTreeMap<String, Option<rust_decimal::Decimal>>, penalties: Option<Vec<usize>>) -> Result<Vec<Change>, String> {
    let class = class(state, id)?;
    if !state.class_teams(id).iter().any(|(t, _)| t == team) {
        return Err(format!("class {}: no team {team}", class.number));
    }
    check_marks(&marks, class.programme.and_then(|p| state.programmes.get(&p)).is_some_and(show::master::in_tenths))?;
    let penalties = match penalties {
        None => None,
        Some(places) => Some(sheet_penalties(class.programme.and_then(|p| state.programmes.get(&p)).ok_or("the class has no programme")?, places)?),
    };
    Ok(vec![Change::TeamSheetEdited { class: id.clone(), team: team.clone(), judge: judge.clone(), marks, penalties }])
}

/// A mark is from 0 to 10, in halves.
/// Driving's multitest (Rosson's form): items 1… marked 0 to 10 in tenths; penalties A
/// (dressage) and C (ground test) in whole points, B (per obstacle) in halves.
fn check_multitest_marks(marks: &BTreeMap<String, Option<rust_decimal::Decimal>>, items: usize) -> Result<(), String> {
    let (ten, two) = (rust_decimal::Decimal::TEN, rust_decimal::Decimal::TWO);
    for (key, value) in marks {
        let Some(v) = value else { continue };
        let ok = match key.as_str() {
            "strafpunkte_a" | "strafpunkte_c" => *v >= rust_decimal::Decimal::ZERO && v.fract().is_zero(),
            "strafpunkte_b" => *v >= rust_decimal::Decimal::ZERO && (v * two).fract().is_zero(),
            n if n.parse::<usize>().is_ok_and(|n| (1..=items).contains(&n)) => *v >= rust_decimal::Decimal::ZERO && *v <= ten && (v * ten).fract().is_zero(),
            _ => return Err(format!("{key} is not an item of the multitest")),
        };
        if !ok {
            return Err(format!("{key}: {v} is not allowed (items 0 to 10 in tenths; penalties A and C whole, B in halves)"));
        }
    }
    Ok(())
}

/// A mark is from 0 to 10, in halves; in tenths (`tenths`, young horses' programmes).
fn check_marks(marks: &BTreeMap<String, Option<rust_decimal::Decimal>>, tenths: bool) -> Result<(), String> {
    let steps = if tenths { rust_decimal::Decimal::TEN } else { rust_decimal::Decimal::TWO };
    for (movement, value) in marks {
        if let Some(v) = value {
            if *v < rust_decimal::Decimal::ZERO || *v > rust_decimal::Decimal::TEN || !(v * steps).fract().is_zero() {
                return Err(if tenths { format!("movement {movement}: {v} is not a mark (0 to 10, in tenths)") } else { format!("movement {movement}: {v} is not a mark (0 to 10, in halves)") });
            }
        }
    }
    Ok(())
}

/// Eventing: the jury's penalties for a rider in its dressage or jumping phase (25 for a yellow
/// card, dangerous riding, mistreatment; Eventing-Reglement 2.2, 8.4, 8.5), added to the phase's
/// penalties overall; `None` takes them back. The cross carries them in its own points (S41).
pub fn set_jury_penalty(state: &State, id: &ClassId, entry: &EntryId, points: Option<Decimal>) -> Result<Vec<Change>, String> {
    let class = class(state, id)?;
    if !state.eventing(class) || !class.phase.as_ref().is_some_and(|p| matches!(p.kind, PhaseKind::Dressage | PhaseKind::Jumping)) {
        return Err(format!("class {} is not an eventing dressage or jumping phase", class.number));
    }
    if !state.starts.get(id).is_some_and(|s| s.iter().any(|s| &s.entry == entry)) {
        return Err(format!("entry {entry} does not start in class {}", class.number));
    }
    if points.is_some_and(|p| p <= Decimal::ZERO) {
        return Err("jury penalties are more than 0".into());
    }
    Ok(vec![Change::JuryPenaltySet { class: id.clone(), entry: entry.clone(), points }])
}

/// The penalties of a rider (or a team): one set, the same on every judge's sheet — a box ticked
/// for one judge is ticked for all (P. Burkhard, 2026-11-17; S40). The exclusion box
/// ("Ausschluss", more errors of course than the programme allows) switches the rider to
/// eliminated, and back when it is unticked (Dressurreglement 8.3 (4)).
pub fn set_penalties(state: &State, id: &ClassId, who: &Competitor, places: Vec<usize>) -> Result<Vec<Change>, String> {
    let class = class(state, id)?;
    let programme = match who {
        Competitor::Entry(entry) => {
            let e = state.entries.get(entry).ok_or_else(|| format!("unknown entry {entry}"))?;
            let individual = class.programme.and_then(|p| state.programmes.get(&p)).is_some_and(show::master::is_individual);
            if individual { e.programme } else { class.programme }
        }
        Competitor::Team(_) => class.programme,
    };
    let def = programme.and_then(|p| state.programmes.get(&p)).ok_or("the class has no programme")?;
    let penalties = sheet_penalties(def, places)?;
    let judges = state.dressage_judges(id);
    if judges.is_empty() {
        return Err(format!("class {} has no judges", class.number));
    }
    let mut changes: Vec<Change> = judges
        .iter()
        .map(|j| match who {
            Competitor::Entry(entry) => Change::SheetEdited { class: id.clone(), entry: entry.clone(), judge: j.id.clone(), marks: BTreeMap::new(), penalties: Some(penalties.clone()), total: None },
            Competitor::Team(team) => Change::TeamSheetEdited { class: id.clone(), team: team.clone(), judge: j.id.clone(), marks: BTreeMap::new(), penalties: Some(penalties.clone()) },
        })
        .collect();
    // The exclusion box: eliminated (and back), for a rider.
    if let Competitor::Entry(entry) = who {
        let before = judges.iter().any(|j| state.sheets.get(&(id.clone(), entry.clone(), j.id.clone())).is_some_and(|s| s.excluded));
        if penalties.excluded != before {
            let discipline = state.class_discipline(class).to_string();
            let eliminated = crate::results::reasons(state, &discipline).into_iter().find(|r| r.name == "ausgeschieden").map(|r| r.id).ok_or("the master data has no reason for eliminated")?;
            let out = state.results.get(&(id.clone(), entry.clone())).and_then(|c| c.out);
            if penalties.excluded && out != Some(eliminated) {
                changes.extend(crate::results::edit_result(state, id, entry, BTreeMap::new(), Some(Some(eliminated)))?);
            } else if !penalties.excluded && out == Some(eliminated) {
                changes.extend(crate::results::edit_result(state, id, entry, BTreeMap::new(), Some(None))?);
            }
        }
    }
    Ok(changes)
}

/// The penalties ticked (places in the programme's penalty list).
fn sheet_penalties(def: &ProgrammeDef, places: Vec<usize>) -> Result<SheetPenalties, String> {
    let list = crate::views::programme_view(def).map(|p| p.penalties).unwrap_or_default();
    let mut p = SheetPenalties::default();
    for i in places {
        let field = list.get(i).ok_or_else(|| format!("no penalty {i}"))?;
        match (field.points, field.percent) {
            (Some(v), _) => p.points.push(v),
            (None, Some(v)) => p.percent.push(v),
            // No value: the exclusion box ("Ausschluss", DR 8.3 (4)).
            _ => p.excluded = true,
        }
    }
    Ok(p)
}

/// Send a judge's sheet to another rider or judge, put it aside, or place it from aside; a sheet
/// already there takes its place. `to` `None`: aside, in a new place.
pub fn move_sheet(state: &State, id: &ClassId, from: SheetPlace, to: Option<SheetPlace>) -> Result<Vec<Change>, String> {
    class(state, id)?;
    let to = to.unwrap_or_else(|| SheetPlace::Aside { slot: state.aside.keys().filter(|(c, _)| c == id).map(|(_, s)| *s).max().unwrap_or(0) + 1 });
    if from == to {
        return Ok(Vec::new());
    }
    let here = match &from {
        SheetPlace::Rider { entry, judge } => state.sheets.contains_key(&(id.clone(), entry.clone(), judge.clone())),
        SheetPlace::Aside { slot } => state.aside.contains_key(&(id.clone(), *slot)),
    };
    if !here {
        return Err("there is no sheet there".into());
    }
    Ok(vec![Change::SheetMoved { class: id.clone(), from, to }])
}

/// A judge's sheet where it is: on a rider, or cut (put aside).
fn sheet_at<'a>(state: &'a State, id: &ClassId, place: &SheetPlace) -> Option<&'a show::model::JudgeSheet> {
    match place {
        SheetPlace::Rider { entry, judge } => state.sheets.get(&(id.clone(), entry.clone(), judge.clone())),
        SheetPlace::Aside { slot } => state.aside.get(&(id.clone(), *slot)),
    }
}

/// Throw a judge's sheet away, a rider's or a cut one; undo brings it back.
pub fn delete_sheet(state: &State, id: &ClassId, place: SheetPlace) -> Result<Vec<Change>, String> {
    class(state, id)?;
    if sheet_at(state, id, &place).is_none() {
        return Err("there is no sheet there".into());
    }
    Ok(vec![Change::SheetSet { class: id.clone(), place, sheet: None }])
}

/// Team dressage: a judge's sheet for a team thrown away (its marks and penalties cleared).
pub fn delete_team_sheet(state: &State, id: &ClassId, team: &TeamId, judge: &AssignmentId) -> Result<Vec<Change>, String> {
    class(state, id)?;
    let sheet = state.team_sheets.get(&(id.clone(), team.clone(), judge.clone())).ok_or("there is no sheet there")?;
    let marks = sheet.marks.keys().map(|m| (m.clone(), None)).collect();
    Ok(vec![Change::TeamSheetEdited { class: id.clone(), team: team.clone(), judge: judge.clone(), marks, penalties: Some(SheetPenalties::default()) }])
}

/// The changes that undo `changes`, made on `state` (the show before them).
pub fn inverse(state: &State, changes: &[Change]) -> Vec<Change> {
    let mut classes: BTreeSet<ClassId> = BTreeSet::new();
    let mut entries: BTreeSet<EntryId> = BTreeSet::new();
    let mut assignments: BTreeSet<AssignmentId> = BTreeSet::new();
    let mut results: BTreeSet<(ClassId, EntryId)> = BTreeSet::new();
    let mut ranks: BTreeSet<(ClassId, Competitor)> = BTreeSet::new();
    let mut combined: BTreeSet<String> = BTreeSet::new();
    let mut undo_addresses: Vec<Change> = Vec::new();
    let mut teams: BTreeSet<TeamId> = BTreeSet::new();
    let mut undo_jury: Vec<Change> = Vec::new();
    for c in changes {
        if let Change::AddressSet { person, .. } = c {
            // The address before; a rider whose address was never set keeps Swiss Equestrian's
            // from then on (as an address set).
            if let Some(p) = state.persons.get(person) {
                undo_addresses.push(Change::AddressSet { person: person.clone(), town: p.town.clone(), contact: state.contacts.get(person).cloned().unwrap_or_else(|| Contact { person: person.clone(), ..Default::default() }) });
            }
        }
        match c {
            Change::TeamSet { team } => {
                teams.insert(team.id.clone());
            }
            Change::TeamRemoved { team } => {
                teams.insert(team.clone());
            }
            Change::ResultEntered { class, entry, .. } | Change::ResultEdited { class, entry, .. } | Change::ResultRemoved { class, entry } => {
                results.insert((class.clone(), entry.clone()));
            }
            Change::RankSet { class, competitor, .. } => {
                ranks.insert((class.clone(), competitor.clone()));
            }
            Change::JuryPenaltySet { class, entry, .. } => {
                undo_jury.push(Change::JuryPenaltySet { class: class.clone(), entry: entry.clone(), points: state.jury_penalties.get(&(class.clone(), entry.clone())).copied() });
            }
            Change::CombinedSet { ranking } => {
                combined.insert(ranking.id.clone());
            }
            Change::CombinedRemoved { id } => {
                combined.insert(id.clone());
            }
            Change::EntryRemoved { entry } => {
                // Its starts come back with it.
                classes.extend(state.starts.iter().filter(|(_, s)| s.iter().any(|s| &s.entry == entry)).map(|(c, _)| c.clone()));
            }
            Change::AssignmentSet { assignment } => {
                assignments.insert(assignment.id.clone());
            }
            Change::AssignmentRemoved { assignment } => {
                assignments.insert(assignment.clone());
            }
            Change::ClassSet { class, .. } => {
                classes.insert(class.id.clone());
            }
            Change::StartsSet { class, .. } | Change::ClassRemoved { class } => {
                classes.insert(class.clone());
            }
            Change::EntryEdited { entry, .. } => {
                entries.insert(entry.clone());
            }
            _ => {}
        }
    }
    let mut undo = Vec::new();
    // Sheets, last change first: marks back to what they were; a move moved back (a swap too).
    for c in changes.iter().rev() {
        match c {
            Change::SheetEdited { class, entry, judge, marks, penalties, total } => {
                let before = state.sheets.get(&(class.clone(), entry.clone(), judge.clone()));
                let marks = marks.keys().map(|m| (m.clone(), before.and_then(|s| s.marks.get(m).copied()))).collect();
                let penalties = penalties.as_ref().map(|_| SheetPenalties { points: before.map(|s| s.penalty_points.clone()).unwrap_or_default(), percent: before.map(|s| s.penalty_percent.clone()).unwrap_or_default(), excluded: before.is_some_and(|s| s.excluded) });
                let total = total.as_ref().map(|_| before.and_then(|s| s.total));
                undo.push(Change::SheetEdited { class: class.clone(), entry: entry.clone(), judge: judge.clone(), marks, penalties, total });
            }
            Change::TeamSheetEdited { class, team, judge, marks, penalties } => {
                let before = state.team_sheets.get(&(class.clone(), team.clone(), judge.clone()));
                let marks = marks.keys().map(|m| (m.clone(), before.and_then(|s| s.marks.get(m).copied()))).collect();
                let penalties = penalties.as_ref().map(|_| SheetPenalties { points: before.map(|s| s.penalty_points.clone()).unwrap_or_default(), percent: before.map(|s| s.penalty_percent.clone()).unwrap_or_default(), excluded: before.is_some_and(|s| s.excluded) });
                undo.push(Change::TeamSheetEdited { class: class.clone(), team: team.clone(), judge: judge.clone(), marks, penalties });
            }
            Change::SheetMoved { class, from, to } => undo.push(Change::SheetMoved { class: class.clone(), from: to.clone(), to: from.clone() }),
            Change::SheetSet { class, place, .. } => undo.push(Change::SheetSet { class: class.clone(), place: place.clone(), sheet: sheet_at(state, class, place).cloned() }),
            _ => {}
        }
    }
    // A class that did not exist goes last, once its results and officials are gone.
    let mut removed = Vec::new();
    for id in &classes {
        match state.classes.get(id) {
            Some(class) => {
                undo.push(Change::class_set(class.clone()));
                undo.push(Change::starts_set(id.clone(), state.starts.get(id).cloned().unwrap_or_default()));
            }
            None => removed.push(Change::ClassRemoved { class: id.clone() }),
        }
    }
    for id in &assignments {
        undo.push(match state.assignments.get(id) {
            Some(a) => Change::AssignmentSet { assignment: a.clone() },
            None => Change::AssignmentRemoved { assignment: id.clone() },
        });
    }
    for (class, entry) in results {
        undo.push(match state.results.get(&(class.clone(), entry.clone())) {
            Some(card) => Change::ResultEntered { class, entry, values: card.values.clone(), out: card.out },
            None => Change::ResultRemoved { class, entry },
        });
    }
    for (class, competitor) in ranks {
        let before = state.manual_ranks.get(&(class.clone(), competitor.clone()));
        undo.push(Change::RankSet { class, competitor, rank: before.map(|m| m.rank), reason: before.map(|m| m.reason.clone()).unwrap_or_default() });
    }
    undo.extend(undo_jury.into_iter().rev());
    for c in changes {
        match c {
            Change::ClassFinalised { class, .. } => undo.push(Change::ClassReopened { class: class.clone(), reason: "undone".into() }),
            Change::EventNumbersSet { .. } => {
                let e = state.event.as_ref();
                undo.push(Change::EventNumbersSet { fixed: e.is_some_and(|e| e.fixed_start_numbers), assigned: e.is_some_and(|e| e.start_numbers_assigned) })
            }
            Change::PrizesSet { published, .. } => undo.push(Change::PrizesSet { published: published.clone(), prizes: state.prizes.get(published).cloned() }),
            Change::ProgrammeSet { programme } => undo.extend(state.programmes.get(&programme.id).map(|p| Change::ProgrammeSet { programme: p.clone() })),
            Change::ScoringTableSet { table } => undo.extend(state.scoring_tables.get(&table.id).map(|t| Change::ScoringTableSet { table: t.clone() })),
            Change::PublishedClassEdited { published, .. } => undo.extend(
                state.published_classes.get(published).map(|p| Change::PublishedClassEdited { published: published.clone(), category_text: Some(p.category_text.clone()), levels: Some(p.levels) }),
            ),
            Change::ClassReopened { class, .. } => undo.push(state.finalise(class)),
            Change::EntryAdded { entry } => undo.push(Change::EntryRemoved { entry: entry.id.clone() }),
            Change::EntryRemoved { entry } => undo.extend(state.entries.get(entry).map(|e| Change::EntryAdded { entry: e.clone() })),
            _ => {}
        }
    }
    for id in combined {
        undo.push(match state.combined.get(&id) {
            Some(r) => Change::CombinedSet { ranking: r.clone() },
            None => Change::CombinedRemoved { id },
        });
    }
    for id in &entries {
        if let Some(e) = state.entries.get(id) {
            let horse = HorseOnEntry { horse: e.horse.clone(), guest_horse: e.guest_horse.clone(), horses_on_entry: e.horses_on_entry.clone() };
            let rider = RiderChange { rider: e.rider.clone(), rider_on_entry: e.rider_on_entry.clone(), changed: e.rider_changed };
            let edit = EntryEdit {
                status: Some(e.status),
                hors_concours: Some(e.hors_concours),
                horse: Some(horse),
                rider: Some(rider),
                published: Some(e.published.clone()),
                stage: Some(e.stage.unwrap_or(0)),
                comment: Some(e.comment.clone()),
                // The horses above carry their phases.
                horse_phases: None,
            };
            undo.push(Change::EntryEdited { entry: id.clone(), edit });
        }
    }
    // A team made by these changes goes once its entries are gone.
    for id in teams {
        match state.teams.get(&id) {
            Some(t) => undo.push(Change::TeamSet { team: t.clone() }),
            None => removed.push(Change::TeamRemoved { team: id }),
        }
    }
    undo.extend(undo_addresses);
    undo.extend(removed);
    undo
}

/// Start numbers fixed for the whole event, or not: turning them off clears every number (as
/// Rosson); turning them on waits for [`number_show`].
pub fn set_fixed_numbers(state: &State, fixed: bool) -> Result<Vec<Change>, String> {
    let event = state.event.as_ref().ok_or("no event")?;
    if event.fixed_start_numbers == fixed {
        return Ok(Vec::new());
    }
    if let Some(c) = state.classes.values().find(|c| c.status == ClassStatus::Final && state.starts.get(&c.id).is_some_and(|s| s.iter().any(|s| s.start_number.is_some()))) {
        return Err(format!("class {} is final: its numbers cannot change; reopen it first", c.number));
    }
    let mut changes = vec![Change::EventNumbersSet { fixed, assigned: false }];
    for c in state.classes.values() {
        let starts = starts(state, &c.id);
        if starts.iter().any(|s| s.start_number.is_some()) {
            changes.push(Change::class_set(Class { start_numbers_assigned: false, ..c.clone() }));
            changes.push(Change::starts_set(c.id.clone(), starts.into_iter().map(|s| Start { start_number: None, ..s }).collect()));
        }
    }
    Ok(changes)
}

/// How the show's fixed numbers are given out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberOrder {
    Name,
    Horse,
    /// By published class, section and start order.
    StartOrder,
    Random,
    /// By the pair's winning points (Gewinnpunkte; the rider's when the pair has none), fewest
    /// first.
    WinningPoints,
}

/// Fixed start numbers 1, 2, 3… for the whole event, one per entry that starts in a class (its
/// sections share it), in the order asked (Rosson's "Startnummern vergeben").
pub fn number_show(state: &State, order: NumberOrder, descending: bool, seed: u64) -> Result<Vec<Change>, String> {
    let event = state.event.as_ref().ok_or("no event")?;
    if !event.fixed_start_numbers {
        return Err("start numbers are given class by class; fix them for the whole event first".into());
    }
    if let Some(c) = state.classes.values().find(|c| c.status == ClassStatus::Final) {
        return Err(format!("class {} is final: its numbers cannot change; reopen it first", c.number));
    }
    // Every entry that starts somewhere, with where it first starts.
    let mut first: BTreeMap<EntryId, (String, String, u32)> = BTreeMap::new();
    for c in state.classes.values() {
        let published = state.published_classes.get(&c.published).map(|p| p.number.clone()).unwrap_or_default();
        for s in state.starts.get(&c.id).into_iter().flatten() {
            let key = (format!("{published:0>6}"), c.id.0.clone(), s.order);
            first.entry(s.entry.clone()).and_modify(|k| *k = k.clone().min(key.clone())).or_insert(key);
        }
    }
    let mut entries: Vec<EntryId> = first.keys().cloned().collect();
    let name = |e: &EntryId| state.entries.get(e).and_then(|e| state.persons.get(&e.rider)).map(|p| format!("{} {}", p.last_name, p.first_name)).unwrap_or_default();
    let horse = |e: &EntryId| state.entries.get(e).map(|e| e.guest_horse_shown().or_else(|| e.horse.as_ref().and_then(|h| state.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default()).unwrap_or_default();
    match order {
        NumberOrder::Name => entries.sort_by(|a, b| compare_names(&name(a), &name(b)).then_with(|| compare_names(&horse(a), &horse(b)))),
        NumberOrder::Horse => entries.sort_by(|a, b| compare_names(&horse(a), &horse(b)).then_with(|| compare_names(&name(a), &name(b)))),
        NumberOrder::StartOrder => entries.sort_by_key(|e| first[e].clone()),
        NumberOrder::Random => shuffle(&mut entries, seed),
        NumberOrder::WinningPoints => {
            let points = |e: &EntryId| state.entries.get(e).and_then(|e| e.winning_points.pair.or(e.winning_points.rider)).unwrap_or_default();
            entries.sort_by(|a, b| points(a).cmp(&points(b)).then_with(|| compare_names(&name(a), &name(b))));
        }
    }
    if descending && order != NumberOrder::Random {
        entries.reverse();
    }
    let number: BTreeMap<EntryId, u32> = entries.iter().enumerate().map(|(i, e)| (e.clone(), i as u32 + 1)).collect();
    let mut changes = vec![Change::EventNumbersSet { fixed: true, assigned: true }];
    for c in state.classes.values() {
        let starts: Vec<Start> = starts(state, &c.id).into_iter().map(|s| Start { start_number: number.get(&s.entry).copied(), ..s }).collect();
        if !starts.is_empty() {
            changes.push(Change::starts_set(c.id.clone(), starts));
        }
    }
    Ok(changes)
}

/// A day's classes in their order: by position, then by number.
fn day_order<'a>(state: &'a State, date: &str) -> Vec<&'a Class> {
    let number = |c: &Class| c.number.trim_start_matches(|ch: char| !ch.is_ascii_digit()).split(|ch: char| !ch.is_ascii_digit()).next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(u32::MAX);
    let mut classes: Vec<&Class> = state.classes.values().filter(|c| c.date == date).collect();
    classes.sort_by_key(|c| (c.position.unwrap_or(u32::MAX), number(c), c.number.clone()));
    classes
}

/// Move a class earlier (`step` −1) or later (+1) in its day: the timetable's order (Rosson's
/// "Übersicht", position).
pub fn move_class(state: &State, id: &ClassId, step: i32) -> Result<Vec<Change>, String> {
    let class = state.classes.get(id).ok_or_else(|| format!("unknown class {id}"))?;
    let from = day_order(state, &class.date).iter().position(|c| &c.id == id).unwrap_or(0);
    let to = from as i64 + step as i64;
    if to < 0 {
        return Ok(Vec::new());
    }
    move_class_to(state, id, None, to as usize)
}

/// A class to another place of its day, or of another day (`date`; dragged there: `to` from 0,
/// its place in that day); the others follow, in both days.
pub fn move_class_to(state: &State, id: &ClassId, date: Option<&str>, to: usize) -> Result<Vec<Change>, String> {
    let class = state.classes.get(id).ok_or_else(|| format!("unknown class {id}"))?;
    let renumbered = |day: Vec<&Class>| -> Vec<Change> {
        day.into_iter().enumerate().filter(|(i, c)| c.position != Some(*i as u32 + 1)).map(|(i, c)| Change::class_set(Class { position: Some(i as u32 + 1), ..c.clone() })).collect()
    };
    match date.filter(|d| *d != class.date) {
        None => {
            let mut day = day_order(state, &class.date);
            let from = day.iter().position(|c| &c.id == id).unwrap_or(0);
            if to >= day.len() || to == from {
                return Ok(Vec::new());
            }
            let moved = day.remove(from);
            day.insert(to, moved);
            Ok(renumbered(day))
        }
        // Another day: the class takes that date, at its place there.
        Some(date) => {
            if !is_date(date) {
                return Err(format!("{date} is not a date (YYYY-MM-DD)"));
            }
            let left: Vec<&Class> = day_order(state, &class.date).into_iter().filter(|c| &c.id != id).collect();
            let mut changes = renumbered(left);
            let there = day_order(state, date);
            let at = to.min(there.len());
            let moved = Class { date: date.to_string(), position: Some(at as u32 + 1), ..class.clone() };
            for (i, c) in there.into_iter().enumerate() {
                let position = (if i < at { i } else { i + 1 }) as u32 + 1;
                if c.position != Some(position) {
                    changes.push(Change::class_set(Class { position: Some(position), ..c.clone() }));
                }
            }
            changes.push(Change::class_set(moved));
            Ok(changes)
        }
    }
}

/// An official's role at the event itself (technical delegate, course designer, vet…): new
/// (`assignment` none) or changed (role, days of work, responsible). Swiss Equestrian's own
/// assignments (read-only) keep their role and person.
#[allow(clippy::too_many_arguments)]
pub fn set_event_role(state: &State, assignment: Option<&AssignmentId>, person: &PersonId, role_id: u32, days: Option<u32>, responsible: bool, now_millis: u64, new_id: &mut dyn FnMut() -> String) -> Result<Vec<Change>, String> {
    // By id: several roles share a code (each discipline's technical delegate is "TD").
    let def = state.roles.iter().find(|r| !r.in_class && r.id == role_id).ok_or_else(|| format!("unknown role at the event {role_id}"))?;
    let assignment = match assignment {
        Some(id) => {
            let a = state.assignments.get(id).ok_or_else(|| format!("unknown assignment {id}"))?;
            if a.readonly == Some(true) && (a.role_id != Some(def.id) || &a.person != person) {
                return Err("given by Swiss Equestrian: only the days and who is responsible can change".into());
            }
            Assignment { role: def.code.clone(), role_id: Some(def.id), person: person.clone(), days, responsible: Some(responsible), updated_at: Some(now_millis), ..a.clone() }
        }
        None => Assignment {
            id: AssignmentId(new_id()),
            class: None,
            person: person.clone(),
            role: def.code.clone(),
            letter: None,
            role_id: Some(def.id),
            event: state.event.as_ref().map(|e| e.id.clone()),
            se_id: None,
            on_event: Some(true),
            responsible: Some(responsible),
            readonly: Some(false),
            days,
            updated_at: Some(now_millis),
            looked_up: false,
        },
    };
    Ok(vec![Change::AssignmentSet { assignment }])
}

/// Take an official's role at the event away (not one Swiss Equestrian gave).
pub fn remove_event_role(state: &State, assignment: &AssignmentId) -> Result<Vec<Change>, String> {
    let a = state.assignments.get(assignment).ok_or_else(|| format!("unknown assignment {assignment}"))?;
    if a.readonly == Some(true) {
        return Err("given by Swiss Equestrian: it stays".into());
    }
    Ok(vec![Change::AssignmentRemoved { assignment: assignment.clone() }])
}

/// How a class's start order is made again (Rosson's "Reihenfolge anpassen" and the orders
/// after another class).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reorder {
    /// By the rider's name (then the horse's).
    Name,
    /// By the horse's name.
    Horse,
    /// By the pair's winning points (Gewinnpunkte), most first when descending.
    WinningPoints,
    /// As in another class, the same order or (descending) the reverse: riders with the same
    /// horse at their place there, a rider's other horse at the place of the one not starting
    /// here, the others after; then grouped and spread as a drawn order.
    Follow(ClassId),
    /// By another class's ranking, the worst first, the winner last; without a result there:
    /// at the end (as Rosson).
    Ranking(ClassId),
    /// The same order, starting from this entry (the ones before it go to the end).
    From(EntryId),
}

/// A class's starters in a new order; start numbers stay with their riders.
pub fn reorder(state: &State, id: &ClassId, how: &Reorder, descending: bool) -> Result<Vec<Change>, String> {
    let c = class(state, id)?;
    if !c.teams.is_empty() {
        return Err("team classes: the order is set by teams".into());
    }
    let mut starts = starts(state, id);
    let entry = |s: &Start| state.entries.get(&s.entry);
    let rider = |s: &Start| entry(s).and_then(|e| state.persons.get(&e.rider)).map(|p| format!("{} {}", p.last_name, p.first_name)).unwrap_or_default();
    let horse = |s: &Start| entry(s).map(|e| e.guest_horse_shown().or_else(|| e.horse.as_ref().and_then(|h| state.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default()).unwrap_or_default();
    let pair = |e: &Entry| (e.rider.clone(), e.horse.clone(), e.guest_horse.clone());
    match how {
        Reorder::Name => starts.sort_by(|a, b| compare_names(&rider(a), &rider(b)).then_with(|| compare_names(&horse(a), &horse(b)))),
        Reorder::Horse => starts.sort_by(|a, b| compare_names(&horse(a), &horse(b)).then_with(|| compare_names(&rider(a), &rider(b)))),
        Reorder::WinningPoints => starts.sort_by_key(|s| entry(s).and_then(|e| e.winning_points.pair.or(e.winning_points.rider)).unwrap_or_default()),
        Reorder::Follow(other) => {
            // Each starter's place in the other class: its pair's, else (a horse that did not
            // start there) its rider's horse that does not start here. The others after, as
            // they are. Reversed: the last there first here.
            let theirs: Vec<&Entry> = self::starts(state, other).iter().filter_map(|t| state.entries.get(&t.entry)).collect();
            let mut taken = vec![false; theirs.len()];
            let mut places: BTreeMap<EntryId, usize> = BTreeMap::new();
            for by_pair in [true, false] {
                for e in starts.iter().filter_map(entry) {
                    if places.contains_key(&e.id) {
                        continue;
                    }
                    let found = (0..theirs.len()).find(|&i| !taken[i] && if by_pair { pair(theirs[i]) == pair(e) } else { theirs[i].rider == e.rider });
                    if let Some(i) = found {
                        taken[i] = true;
                        places.insert(e.id.clone(), i);
                    }
                }
            }
            starts.sort_by_key(|s| match places.get(&s.entry) {
                Some(&i) => (0, if descending { theirs.len() - i } else { i }),
                None => (1, 0),
            });
        }
        Reorder::Ranking(other) => {
            let lines = state.ranking(other)?.lines;
            let ranked: Vec<&Entry> = lines.iter().filter(|l| !l.team).filter_map(|l| l.id.parse::<u64>().ok().and_then(|id| state.entries.get(&EntryId(id)))).collect();
            // The worst first: the ranking's listing reversed; no result there: at the end.
            let place = |s: &Start| entry(s).and_then(|e| ranked.iter().position(|x| pair(x) == pair(e))).map_or(usize::MAX, |p| ranked.len() - p);
            starts.sort_by_key(place);
        }
        Reorder::From(first) => {
            let at = starts.iter().position(|s| &s.entry == first).ok_or("not a starter of this class")?;
            starts.rotate_left(at);
        }
    }
    if descending && matches!(how, Reorder::Name | Reorder::Horse | Reorder::WinningPoints) {
        starts.reverse();
    }
    // Sorted, or as another class (the same or the reverse): grouped (levels, pony sizes) and a
    // rider's horses spread out (U11), even horses that did not start in the other class. A
    // ranking's order and a rotation stay as they are.
    if matches!(how, Reorder::Name | Reorder::Horse | Reorder::WinningPoints | Reorder::Follow(_)) {
        let order = spread(state, &c.published, starts.iter().map(|s| s.entry.clone()).collect());
        starts.sort_by_key(|s| order.iter().position(|e| e == &s.entry));
    }
    for (i, s) in starts.iter_mut().enumerate() {
        s.order = i as u32 + 1;
    }
    Ok(vec![Change::starts_set(id.clone(), starts)])
}


/// Every result of a class taken away (Rosson's "Resultate löschen"): riders' results, judges'
/// sheets, ranks set by hand. Not when final. Undo brings them back.
pub fn delete_results(state: &State, id: &ClassId) -> Result<Vec<Change>, String> {
    let c = class(state, id)?;
    if state.team_results.keys().any(|(k, _)| k == id) {
        return Err(format!("class {} has team results: not yet in the app", c.number));
    }
    let mut changes = Vec::new();
    for ((_, entry), _) in state.results.range((id.clone(), EntryId(0))..).take_while(|((k, _), _)| k == id) {
        changes.push(Change::ResultRemoved { class: id.clone(), entry: entry.clone() });
    }
    for ((_, entry, judge), sheet) in state.sheets.iter().filter(|((k, _, _), _)| k == id) {
        let marks = sheet.marks.keys().map(|m| (m.clone(), None)).collect();
        changes.push(Change::SheetEdited { class: id.clone(), entry: entry.clone(), judge: judge.clone(), marks, penalties: Some(SheetPenalties::default()), total: sheet.total.map(|_| None) });
    }
    for ((_, team, judge), sheet) in state.team_sheets.iter().filter(|((k, _, _), _)| k == id) {
        let marks = sheet.marks.keys().map(|m| (m.clone(), None)).collect();
        changes.push(Change::TeamSheetEdited { class: id.clone(), team: team.clone(), judge: judge.clone(), marks, penalties: Some(SheetPenalties::default()) });
    }
    for ((_, who), _) in state.manual_ranks.iter().filter(|((k, _), _)| k == id) {
        changes.push(Change::RankSet { class: id.clone(), competitor: who.clone(), rank: None, reason: "results deleted".into() });
    }
    Ok(changes)
}

/// A published class's start lists taken away (Rosson's "Startlisten löschen"): its classes
/// (every section, a split ranking), their starts, results and officials; it is "not set up"
/// again. Not when one is final, nor while another class uses it (qualifier, combined ranking),
/// nor while Swiss Equestrian has its start lists (withdraw them first).
pub fn remove_start_lists(state: &State, published: &PublishedClassId) -> Result<Vec<Change>, String> {
    let p = state.published_classes.get(published).ok_or_else(|| format!("unknown published class {published}"))?;
    let ids: Vec<ClassId> = state.classes.values().filter(|c| &c.published == published).map(|c| c.id.clone()).collect();
    if ids.is_empty() {
        return Err(format!("class {} is not set up", p.number));
    }
    for id in &ids {
        class(state, id)?;
        if let Some(user) = state.classes.values().find(|c| !ids.contains(&c.id) && c.options.qualifier.as_ref() == Some(id)) {
            return Err(format!("class {} uses it as its qualifier", user.number));
        }
        if let Some(r) = state.combined.values().find(|r| r.classes.contains(id)) {
            return Err(format!("the ranking \"{}\" counts it", r.name));
        }
    }
    // Swiss Equestrian would keep start lists of a class the show no longer has.
    let sent = state.uploaded.get(&Upload::StartLists(published.clone())).map(|s| s.millis);
    let withdrawn = state.uploaded.get(&Upload::StartListsWithdrawn(published.clone())).map(|s| s.millis);
    if sent.is_some_and(|s| withdrawn.is_none_or(|w| w < s)) {
        return Err(format!("the start lists of class {} were sent to Swiss Equestrian: withdraw them first", p.number));
    }
    let mut changes = Vec::new();
    for id in &ids {
        changes.extend(delete_results(state, id)?);
        changes.extend(state.assignments.values().filter(|a| a.class.as_ref() == Some(id)).map(|a| Change::AssignmentRemoved { assignment: a.id.clone() }));
    }
    changes.extend(ids.into_iter().map(|class| Change::ClassRemoved { class }));
    Ok(changes)
}

/// A class's details as the office edits them (Rosson's class info box); `None`: unchanged.
#[derive(Debug, Clone, Default)]
pub struct Details {
    pub sponsor: Option<String>,
    /// Placed riders set by the organiser; `Some(None)`: 30 % again.
    pub placed_count: Option<Option<u32>>,
    pub extra_fields: Option<Vec<ExtraField>>,
    /// A PNG or JPEG as a data URL (at most 400 px, made so by the app); `Some(None)`: none.
    pub logo: Option<Option<String>>,
    /// The most starters; `Some(None)`: no limit.
    pub max_starters: Option<Option<u32>>,
    /// The ideal time of an ideal-time class (S20), as typed ("1:15", "75", "74.5"); `Some(None)`:
    /// none.
    pub ideal_time: Option<Option<String>>,
    /// Another scoring table (only before results: they depend on it).
    pub scoring_table: Option<u32>,
    /// Dressage: the judges give a total each instead of marks (only before any sheet).
    pub totals_per_judge: Option<bool>,
    /// Team dressage: five judges, 3/2 (only before any sheet).
    pub team_3_2: Option<bool>,
    /// Team jumping: every rider's result counts, none dropped (S36).
    pub team_count_all: Option<bool>,
    /// Dressage: a Tag der Jugend final, no ex aequo in ranks 1–3 (S38).
    pub youth_final: Option<bool>,
    /// Eventing: time over the ideal time counts (cross), 0.4 per second started.
    pub ideal_time_penalties: Option<bool>,
    /// Eventing's jumping phase: the maximum time, as typed; `Some(None)`: none.
    pub max_time: Option<Option<String>>,
    /// … and 0.4 per second started over it counts.
    pub max_time_penalties: Option<bool>,
    /// A class made of phases: the overall ranking adds the phases' ranks.
    pub rank_points: Option<bool>,
    /// A driving phase's settings, all of them.
    pub driving: Option<DrivingSettings>,
}

/// A time of day as the office types it ("10:12", "10:12:30"), in seconds since midnight.
pub fn parse_clock(text: &str) -> Result<rust_decimal::Decimal, String> {
    let bad = || format!("{text} is not a time of day (10:12:30)");
    let parts: Vec<&str> = text.trim().split(':').collect();
    let (h, m, s) = match parts.as_slice() {
        [h, m] => (h, m, &"0"),
        [h, m, s] => (h, m, s),
        _ => return Err(bad()),
    };
    let h: u32 = h.trim().parse().map_err(|_| bad())?;
    let m: u32 = m.trim().parse().map_err(|_| bad())?;
    let s: rust_decimal::Decimal = s.trim().replace(',', ".").parse().map_err(|_| bad())?;
    if h > 23 || m > 59 || s >= rust_decimal::Decimal::from(60) {
        return Err(bad());
    }
    Ok((rust_decimal::Decimal::from(h * 3600 + m * 60) + s).normalize())
}

/// A time as the office types it: seconds ("75", "74.5") or minutes and seconds ("1:15",
/// "1:14.5").
pub fn parse_time(text: &str) -> Result<Decimal, String> {
    let text = text.trim().replace(',', ".");
    let bad = || format!("{text} is not a time (75 or 1:15)");
    let seconds = match text.split_once(':') {
        Some((m, s)) => {
            let m: u32 = m.trim().parse().map_err(|_| bad())?;
            let s: Decimal = s.trim().parse().map_err(|_| bad())?;
            if s >= Decimal::from(60) {
                return Err(bad());
            }
            Decimal::from(m * 60) + s
        }
        None => text.parse().map_err(|_| bad())?,
    };
    if seconds <= Decimal::ZERO {
        return Err(bad());
    }
    Ok(seconds.normalize())
}

/// Seconds as the office reads a time: "1:15", "1:14.5".
pub fn time_text(seconds: Decimal) -> String {
    let whole = seconds.trunc();
    let minutes = (whole / Decimal::from(60)).trunc();
    let rest = (seconds - minutes * Decimal::from(60)).normalize();
    let rest = if rest < Decimal::from(10) { format!("0{rest}") } else { rest.to_string() };
    format!("{minutes}:{rest}")
}

/// Change a class's details: its number (unique in the show), date (a day of the show), the
/// sponsor on its prize labels, how many are placed, its extra lines, its logo.
pub fn set_details(state: &State, id: &ClassId, details: Details) -> Result<Vec<Change>, String> {
    let mut c = class(state, id)?.clone();
    if let Some(sponsor) = details.sponsor {
        c.sponsor = sponsor.trim().to_string();
    }
    if let Some(placed) = details.placed_count {
        if placed == Some(0) {
            return Err("at least one placed rider".into());
        }
        c.options.placed_count = placed;
    }
    if let Some(fields) = details.extra_fields {
        let fields: Vec<ExtraField> = fields.into_iter().map(|f| ExtraField { key: f.key.trim().to_string(), value: f.value.trim().to_string() }).filter(|f| !f.key.is_empty() || !f.value.is_empty()).collect();
        if fields.len() > 12 {
            return Err("at most 12 extra lines".into());
        }
        c.extra_fields = Some(fields);
    }
    if let Some(logo) = details.logo {
        if let Some(l) = &logo {
            if !l.starts_with("data:image/") || !l.contains(";base64,") {
                return Err("the logo is not an image".into());
            }
            if l.len() > 1_500_000 {
                return Err("the logo is too large (at most 400 px)".into());
            }
        }
        c.logo = logo.map(Value::String);
    }
    if let Some(table) = details.scoring_table.filter(|t| c.scoring_table != Some(*t)) {
        state.scoring_tables.get(&table).ok_or_else(|| format!("unknown scoring table {table}"))?;
        if has_results(state, id) {
            return Err(format!("class {} has results: delete them before changing its scoring table", c.number));
        }
        c.scoring_table = Some(table);
    }
    if let Some(totals) = details.totals_per_judge.filter(|t| *t != c.options.totals_per_judge) {
        if state.class_discipline(&c) != "CD" {
            return Err(format!("class {} is not a dressage class", c.number));
        }
        if state.sheets.keys().any(|(class, _, _)| class == id) {
            return Err(format!("class {} has judges' sheets: delete its results first", c.number));
        }
        c.options.totals_per_judge = totals;
    }
    if let Some(three_two) = details.team_3_2.filter(|t| *t != c.options.team_3_2) {
        if !state.published_classes.get(&c.published).is_some_and(|p| p.discipline == "CD" && p.team_size.is_some_and(|n| n > 0)) {
            return Err(format!("class {} is not a team dressage class", c.number));
        }
        if state.team_sheets.keys().any(|(class, _, _)| class == id) {
            return Err(format!("class {} has judges' sheets: delete its results first", c.number));
        }
        c.options.team_3_2 = three_two;
    }
    if let Some(all) = details.team_count_all.filter(|a| *a != c.options.team_count_all) {
        if !state.published_classes.get(&c.published).is_some_and(|p| p.discipline == "CS" && p.team_size.is_some_and(|n| n > 0)) {
            return Err(format!("class {} is not a team jumping class", c.number));
        }
        c.options.team_count_all = all;
    }
    if let Some(on) = details.youth_final.filter(|f| *f != c.options.youth_final) {
        if on && c.programme.and_then(|p| state.programmes.get(&p)).and_then(show::master::youth_final_movements).is_none() {
            return Err(format!("class {} has no Tag der Jugend programme", c.number));
        }
        c.options.youth_final = on;
    }
    if let Some(on) = details.ideal_time_penalties {
        c.options.ideal_time_penalties = on;
    }
    if let Some(max) = details.max_time {
        c.options.max_time = match max.filter(|t| !t.trim().is_empty()) {
            Some(t) => Some(parse_time(&t)?),
            None => None,
        };
    }
    if let Some(on) = details.max_time_penalties {
        c.options.max_time_penalties = on;
    }
    if let Some(settings) = details.driving {
        if !c.phase.as_ref().is_some_and(|p| matches!(p.kind, PhaseKind::Marathon | PhaseKind::Cones | PhaseKind::Derby | PhaseKind::Open | PhaseKind::Multitest)) {
            return Err(format!("class {} is not a driving phase", c.number));
        }
        if settings.rounds.is_some_and(|r| !(1..=2).contains(&r)) || settings.obstacles.is_some_and(|o| !(1..=12).contains(&o)) {
            return Err("one or two rounds; 1 to 12 obstacles".into());
        }
        c.options.driving = Some(settings);
    }
    if let Some(on) = details.rank_points {
        if c.phases.is_empty() {
            return Err(format!("class {} has no phases", c.number));
        }
        c.options.rank_points = on;
    }
    if let Some(ideal) = details.ideal_time {
        c.options.ideal_time = match ideal.filter(|t| !t.trim().is_empty()) {
            Some(t) => Some(parse_time(&t)?),
            None => None,
        };
    }
    // A maximum: the entries made last beyond it leave the start list (they wait on none).
    let mut cut = None;
    if let Some(max) = details.max_starters {
        if max == Some(0) {
            return Err("at least one starter".into());
        }
        c.options.max_starters = max;
        let starts = starts(state, id);
        if let Some(max) = max.filter(|m| (*m as usize) < starts.len()) {
            if has_results(state, id) {
                return Err(format!("class {} has results: its starters stay", c.number));
            }
            // Entry time: as Swiss Equestrian gives it, else the entry's id (made at the show: later).
            let made = |e: &EntryId| state.entries.get(e).map(|e| (e.record["erstellt_am"].as_str().map(str::to_string).unwrap_or_else(|| "~".into()), e.id.0)).unwrap_or_default();
            let mut by_time: Vec<&Start> = starts.iter().collect();
            by_time.sort_by_key(|s| made(&s.entry));
            let keep: BTreeSet<&EntryId> = by_time.iter().take(max as usize).map(|s| &s.entry).collect();
            let kept: Vec<Start> = starts.iter().filter(|s| keep.contains(&s.entry)).enumerate().map(|(i, s)| Start { order: i as u32 + 1, ..s.clone() }).collect();
            cut = Some(kept);
        }
    }
    let mut changes = Vec::new();
    if Some(&c) != state.classes.get(id) {
        changes.push(Change::class_set(c));
    }
    if let Some(starts) = cut {
        changes.push(Change::starts_set(id.clone(), starts));
    }
    Ok(changes)
}

/// A team one place earlier (`step` −1) or later (+1) in its class's rotation; its riders' team
/// order follows.
pub fn move_team(state: &State, id: &ClassId, team: &TeamId, step: i32) -> Result<Vec<Change>, String> {
    let c = class(state, id)?;
    let at = c.teams.iter().position(|t| t == team).ok_or_else(|| format!("team {team} does not start in class {}", c.number))?;
    let to = at as i64 + i64::from(step);
    if to < 0 {
        return Ok(Vec::new());
    }
    move_team_to(state, id, team, to as usize)
}

/// A team to another place in the class's order (dragged there: `to` from 0); its riders follow.
pub fn move_team_to(state: &State, id: &ClassId, team: &TeamId, to: usize) -> Result<Vec<Change>, String> {
    let c = class(state, id)?;
    let mut teams = c.teams.clone();
    let at = teams.iter().position(|t| t == team).ok_or_else(|| format!("team {team} does not start in class {}", c.number))?;
    if to >= teams.len() || to == at {
        return Ok(Vec::new());
    }
    let moved = teams.remove(at);
    teams.insert(to, moved);
    let order = |s: &Start| state.entries.get(&s.entry).and_then(|e| e.team.as_ref()).and_then(|t| teams.iter().position(|x| x == t)).map(|i| i as u32 + 1);
    let starts: Vec<Start> = starts(state, id).into_iter().map(|s| Start { team_order: order(&s).or(s.team_order), ..s }).collect();
    Ok(vec![Change::class_set(Class { teams: teams.clone(), ..c.clone() }), Change::starts_set(id.clone(), starts)])
}

/// A whole team does not start (or starts again): each of its riders.
pub fn team_not_starting(state: &State, team: &TeamId, not_starting: bool) -> Result<Vec<Change>, String> {
    state.teams.get(team).ok_or_else(|| format!("unknown team {team}"))?;
    let status = if not_starting { EntryStatus::NotStarting } else { EntryStatus::Entered };
    Ok(state
        .entries
        .values()
        .filter(|e| e.team.as_ref() == Some(team) && !e.waiting_list && e.status != status)
        .map(|e| Change::EntryEdited { entry: e.id.clone(), edit: EntryEdit { status: Some(status), ..Default::default() } })
        .collect())
}



/// One rider's horses in a new order among the places they hold (U12): `entries` are all their
/// starts in the class, in the new order; nobody else moves. `numbers_follow`: the rider's start
/// numbers go with the places, the lowest to the earliest start, instead of staying with the
/// horses.
pub fn rearrange_rider(state: &State, id: &ClassId, entries: &[EntryId], numbers_follow: bool) -> Result<Vec<Change>, String> {
    let c = class(state, id)?;
    if !c.teams.is_empty() {
        return Err("team classes: the order is set by teams".into());
    }
    let mut starts = starts(state, id);
    let rider = entries.first().and_then(|e| state.entries.get(e)).map(|e| e.rider.clone()).ok_or("no horse given")?;
    let theirs: Vec<usize> = starts.iter().enumerate().filter(|(_, s)| state.entries.get(&s.entry).is_some_and(|e| e.rider == rider)).map(|(i, _)| i).collect();
    let mut given: Vec<&EntryId> = entries.iter().collect();
    given.sort();
    given.dedup();
    let mut held: Vec<&EntryId> = theirs.iter().map(|i| &starts[*i].entry).collect();
    held.sort();
    if given.len() != entries.len() || given != held {
        return Err("give each of the rider's horses in the class once".into());
    }
    // The places (and their numbers) in start order; the horses take them in the new order.
    let places: Vec<u32> = theirs.iter().map(|i| starts[*i].order).collect();
    let numbers: BTreeMap<EntryId, Option<u32>> = theirs.iter().map(|i| (starts[*i].entry.clone(), starts[*i].start_number)).collect();
    // Following: the lowest number first (those without a number last).
    let mut ascending: Vec<Option<u32>> = numbers.values().copied().collect();
    ascending.sort_by_key(|n| (n.is_none(), *n));
    for (k, i) in theirs.iter().enumerate() {
        let entry = entries[k].clone();
        let number = if numbers_follow { ascending[k] } else { numbers[&entry] };
        starts[*i] = Start { entry, order: places[k], start_number: number, team_order: None };
    }
    if Some(&starts) == state.starts.get(id).map(|s| {
        let mut s = s.clone();
        s.sort_by_key(|s| s.order);
        s
    }).as_ref() {
        return Ok(Vec::new());
    }
    Ok(vec![Change::starts_set(id.clone(), starts)])
}

#[cfg(test)]
mod spread_tests {
    use super::spread_order;

    /// Riders' horses: ("a1", 'a') is rider a's first horse.
    fn order(riders: &[(char, usize)]) -> Vec<String> {
        let items: Vec<(String, char)> = riders.iter().flat_map(|&(r, k)| (1..=k).map(move |i| (format!("{r}{i}"), r))).collect();
        // Interleave as a draw would: by horse number, then rider.
        let mut items = items;
        items.sort_by_key(|(name, r)| (name[1..].parse::<usize>().unwrap(), *r));
        spread_order(&items)
    }

    #[test]
    fn two_horses_at_the_start_and_the_end() {
        // Riders b, c with two horses; x, y, z with one: B1 A B2.
        let o = order(&[('b', 2), ('c', 2), ('x', 1), ('y', 1), ('z', 1)]);
        assert_eq!(o, ["b1", "c1", "x1", "y1", "z1", "b2", "c2"]);
    }

    #[test]
    fn three_horses_around_them() {
        // C1 B1 A C2 A B2 C3, the one-horse riders cut in two.
        let o = order(&[('c', 3), ('b', 2), ('w', 1), ('x', 1), ('y', 1), ('z', 1)]);
        assert_eq!(o, ["c1", "b1", "w1", "x1", "c2", "y1", "z1", "b2", "c3"]);
    }

    #[test]
    fn four_horses_and_more_riders() {
        // D1 C1 B1 A D2 A C2 A D3 A B2 C3 D4: A cut 1/3, 1/6, 1/6, 1/3 (12 riders: 4, 2, 2, 4).
        let mut riders = vec![('d', 4), ('c', 3), ('b', 2)];
        let ones: Vec<char> = "efghijklmnop".chars().collect();
        riders.extend(ones.iter().map(|&r| (r, 1)));
        let o = order(&riders);
        let one = |s: &str| ones.contains(&s.chars().next().unwrap());
        let shape: String = o.iter().map(|s| if one(s) { 'A' } else { s.chars().next().unwrap().to_ascii_uppercase() }).collect();
        assert_eq!(shape, "DCBAAAADAACAADAAAABCD");
        assert_eq!(o.len(), 12 + 4 + 3 + 2);
    }

    #[test]
    fn each_rider_keeps_their_place_in_their_blocks() {
        // Two riders with two horses, drawn p before q: p first in both blocks; same gap.
        let items = vec![("p1", 'p'), ("x1", 'x'), ("q1", 'q'), ("p2", 'p'), ("y1", 'y'), ("q2", 'q')];
        let o = spread_order(&items);
        assert_eq!(o, ["p1", "q1", "x1", "y1", "p2", "q2"]);
        let at = |s: &str| o.iter().position(|x| *x == s).unwrap();
        assert_eq!(at("p2") - at("p1"), at("q2") - at("q1"));
    }

    #[test]
    fn nobody_lost_nobody_twice() {
        let mut items = Vec::new();
        for (r, k) in [(1, 5), (2, 1), (3, 2), (4, 3), (5, 1), (6, 2), (7, 6), (8, 1)] {
            for h in 0..k {
                items.push((r * 10 + h, r));
            }
        }
        let mut o = spread_order(&items);
        assert_eq!(o.len(), items.len());
        o.sort();
        let mut all: Vec<i32> = items.iter().map(|(i, _)| *i).collect();
        all.sort();
        assert_eq!(o, all);
        // Only one-horse riders, or nobody: as they come.
        assert_eq!(spread_order(&[(1, 'a'), (2, 'b')]), [1, 2]);
        assert!(spread_order::<i32, char>(&[]).is_empty());
    }
}

#[cfg(test)]
mod time_tests {
    use super::*;

    #[test]
    fn times_as_the_office_types_them() {
        let d = |s: &str| s.parse::<Decimal>().unwrap();
        assert_eq!(parse_time("75").unwrap(), d("75"));
        assert_eq!(parse_time("1:15").unwrap(), d("75"));
        assert_eq!(parse_time(" 1:14,5 ").unwrap(), d("74.5"));
        assert!(parse_time("1:75").is_err() && parse_time("abc").is_err() && parse_time("0").is_err());
        assert_eq!((time_text(d("75")), time_text(d("74.5")), time_text(d("125"))), ("1:15".to_string(), "1:14.5".to_string(), "2:05".to_string()));
    }
}

