//! Entries the office makes or changes at the show (docs/rosson-inventory.md, "Participants"
//! and "Class preparation"), as changes to the show; each function reads the show and returns the
//! changes:
//!
//! - **a late entry** or an organiser's place (Rosson's "Startplatz vergeben"): a rider and a
//!   horse of the show or of Swiss Equestrian's registry ([`crate::registry`]), or a guest horse
//!   for a rider with a guest licence; put at the end of the published class's last section,
//!   with the next start number once numbers are given;
//! - **taking an office entry back** (only the office's own);
//! - **a change of rider** (Reiterwechsel, printed "RW" when the rider is not the one entered);
//! - **moving an entry to another published class**: its starts and results in the old one go
//!   (as Rosson), it is put at the end of the new one;
//! - **an entry's level and the office's note**.
//!
//! Nothing touches a final class. Riders and horses are Swiss Equestrian's: the office picks
//! them, never types them in or changes them (P. Burkhard, 2026-11-14).

use serde_json::{json, Value};
use show::model::*;
use show::oplog::Change;
use show::State;

/// The rider of an entry: one the show knows, or one of Swiss Equestrian's registry (copied
/// into the show as delivered, with its contact data).
#[derive(Debug, Clone)]
pub enum RiderPick {
    Known(PersonId),
    Registry(Box<(Person, Contact)>),
}

/// The horse of an entry: one the show knows, one of the registry (copied into the show), or a
/// guest horse (no passport; riders with a guest licence only).
#[derive(Debug, Clone)]
pub enum HorsePick {
    Known(HorseId),
    Registry(Box<Horse>),
    Guest(String),
}

/// A guest licence, whose holders may ride a guest horse (Rosson's `hasGuestLicense`:
/// `/gast|RBE(?:,|$)/i` on the licences).
pub fn guest_licence(licences: &str) -> bool {
    let l = licences.to_lowercase();
    l.contains("gast") || l.split(',').any(|t| t.ends_with("rbe"))
}

/// The show's sport year (its first day's).
pub fn show_year(state: &State) -> Option<u32> {
    state.event.as_ref().and_then(|e| e.from.get(..4)?.parse().ok())
}

/// Refused unless Swiss Equestrian lets a horse of the registry start in the show's year (its
/// fee paid; Rosson offers no other).
pub fn starts_this_year(state: &State, horse: &Horse) -> Result<(), String> {
    match show_year(state) {
        Some(year) if !crate::registry::paid_for(&horse.record, Some(year)) => Err(format!("{} is not registered with Swiss Equestrian for {year}", horse.name)),
        _ => Ok(()),
    }
}

/// Refused unless the rider may ride a guest horse.
pub fn guest_allowed(rider: &Person) -> Result<(), String> {
    if guest_licence(&rider.licences) {
        Ok(())
    } else {
        Err(format!("{} {} has no guest licence: a guest horse is only for riders with one", rider.first_name, rider.last_name))
    }
}

/// The team of a late entry in a team class: one with a free place, or a new one by name.
#[derive(Debug, Clone)]
pub enum TeamPick {
    Join(TeamId),
    New(String),
}

/// The teams of a published class, with their riders (not those turned down).
pub fn teams_of(state: &State, published: &PublishedClassId) -> Vec<(TeamId, usize)> {
    let mut teams: Vec<(TeamId, usize)> = state.teams.values().filter(|t| &t.published == published).map(|t| (t.id.clone(), 0)).collect();
    for e in state.entries.values().filter(|e| &e.published == published && !e.waiting_list) {
        if let Some(t) = teams.iter_mut().find(|(t, _)| e.team.as_ref() == Some(t)) {
            t.1 += 1;
        }
    }
    teams
}

/// Rosson's placeholder horse for guest horses.
const GUEST_HORSE: u64 = 5;

fn rider(state: &State, pick: &RiderPick, changes: &mut Vec<Change>) -> Result<(Person, RiderOnEntry), String> {
    let person = match pick {
        RiderPick::Known(id) => state.persons.get(id).cloned().ok_or_else(|| format!("unknown rider {id}"))?,
        RiderPick::Registry(picked) => {
            let (person, contact) = &**picked;
            match state.persons.get(&person.id) {
                // The show's own record (Swiss Equestrian's updates keep it current).
                Some(p) => p.clone(),
                None => {
                    changes.push(Change::PersonAdded { person: person.clone() });
                    changes.push(Change::PrivateDataDelivered { contacts: vec![contact.clone()], notes: Vec::new() });
                    person.clone()
                }
            }
        }
    };
    let on_entry = RiderOnEntry { first_name: person.first_name.clone(), last_name: person.last_name.clone(), town: person.town.clone(), club: person.club.clone() };
    Ok((person, on_entry))
}

/// A horse for an entry: its id, a guest horse's name, what is written on the entry about it.
type HorseOnNewEntry = (Option<HorseId>, Option<String>, Vec<Value>);

/// The horse of an entry, from what the office picked, for `rider`.
fn horse(state: &State, pick: &HorsePick, rider: &Person, changes: &mut Vec<Change>) -> Result<HorseOnNewEntry, String> {
    let record = |h: &Horse| json!({ "pferd_id": h.id.0, "pferdename": h.name, "pferd_signalement": h.signalement, "pferd_besitzer": h.owner, "pferd_zuechter": h.breeder, "einsaetze": [] });
    Ok(match pick {
        HorsePick::Known(id) => {
            let h = state.horses.get(id).ok_or_else(|| format!("unknown horse {id}"))?;
            (Some(h.id.clone()), None, vec![record(h)])
        }
        HorsePick::Registry(picked) => {
            if picked.id.0 == GUEST_HORSE {
                return Err("not a horse of the registry".into());
            }
            let h = match state.horses.get(&picked.id) {
                Some(h) => h.clone(),
                None => {
                    starts_this_year(state, picked)?;
                    changes.push(Change::HorseAdded { horse: (**picked).clone() });
                    (**picked).clone()
                }
            };
            (Some(h.id.clone()), None, vec![record(&h)])
        }
        HorsePick::Guest(name) => {
            guest_allowed(rider)?;
            let name = name.trim().to_uppercase();
            if name.is_empty() {
                return Err("a guest horse needs a name".into());
            }
            (Some(HorseId(GUEST_HORSE)), Some(name.clone()), vec![json!({ "pferd_id": GUEST_HORSE, "pferdename": name, "einsaetze": [] })])
        }
    })
}

/// The classes an entry starts in.
fn classes_of(state: &State, entry: &EntryId) -> Vec<ClassId> {
    state.starts.iter().filter(|(_, s)| s.iter().any(|s| &s.entry == entry)).map(|(c, _)| c.clone()).collect()
}

pub(crate) fn no_final(state: &State, entry: &EntryId) -> Result<(), String> {
    for c in classes_of(state, entry) {
        if let Some(class) = state.classes.get(&c).filter(|c| c.status == ClassStatus::Final) {
            return Err(format!("class {} is final; reopen it first", class.number));
        }
    }
    Ok(())
}

/// The entry at the end of the published class's last section, if it is set up; with the next
/// start number once numbers are given (for the whole show when they are fixed).
fn placed(state: &State, published: &PublishedClassId, entry: &EntryId, team: Option<TeamId>) -> Vec<Change> {
    // Team classes: the section where the team starts, if it does.
    let mut classes: Vec<&Class> = state.classes.values().filter(|c| &c.published == published).collect();
    classes.sort_by(|a, b| a.id.0.cmp(&b.id.0));
    let class = team.as_ref().and_then(|t| classes.iter().find(|c| c.teams.contains(t)).copied()).or(classes.last().copied());
    class.map(|c| placed_in_team(state, c, entry, team)).unwrap_or_default()
}

/// The entry at the end of a class's start list, with the next start number once numbers are
/// given (for the whole show when they are fixed). Team classes: after its team's riders, the
/// team at the end of the rotation if it is not in it yet (then the class changes too).
fn placed_in(state: &State, class: &Class, entry: &EntryId) -> Vec<Change> {
    placed_in_team(state, class, entry, state.entries.get(entry).and_then(|e| e.team.clone()))
}

fn placed_in_team(state: &State, class: &Class, entry: &EntryId, team: Option<TeamId>) -> Vec<Change> {
    let mut starts = state.starts.get(&class.id).cloned().unwrap_or_default();
    let mut changes = Vec::new();
    if let Some(team) = team.filter(|_| !class.teams.is_empty() || state.published_classes.get(&class.published).is_some_and(|p| p.team_size.is_some())) {
        let mut teams = class.teams.clone();
        if !teams.contains(&team) {
            teams.push(team.clone());
            changes.push(Change::class_set(Class { teams: teams.clone(), ..class.clone() }));
        }
        let team_order = teams.iter().position(|t| t == &team).map(|i| i as u32 + 1);
        let of_team = |s: &Start| state.entries.get(&s.entry).and_then(|e| e.team.as_ref()) == Some(&team);
        let order = starts.iter().filter(|s| of_team(s)).map(|s| s.order).max().unwrap_or(0) + 1;
        starts.push(Start { entry: entry.clone(), order, start_number: next_number(state, class), team_order });
        changes.push(with_next_number(Change::starts_set(class.id.clone(), starts), entry));
        return changes;
    }
    starts.sort_by_key(|s| s.order);
    starts.push(Start { entry: entry.clone(), order: starts.len() as u32 + 1, start_number: next_number(state, class), team_order: None });
    changes.push(with_next_number(Change::starts_set(class.id.clone(), starts), entry));
    changes
}

/// The entry takes the next number when the change is applied (R7): after numbers another
/// computer gave meanwhile, never one of theirs.
fn with_next_number(mut change: Change, entry: &EntryId) -> Change {
    if let Change::StartsSet { next_numbers, .. } = &mut change {
        next_numbers.push(entry.clone());
    }
    change
}

/// The next start number of a class once its numbers are given (the show's when fixed).
fn next_number(state: &State, class: &Class) -> Option<u32> {
    state.start_numbers_assigned(class).then(|| {
        let fixed = state.event.as_ref().is_some_and(|e| e.fixed_start_numbers);
        let highest = state.starts.iter().filter(|(c, _)| fixed || *c == &class.id).flat_map(|(_, s)| s.iter().filter_map(|s| s.start_number)).max().unwrap_or(0);
        highest + 1
    })
}

/// Entries of a published class that is set up, on none of its start lists (Rosson's "Ohne
/// Prüfung"): delivered after it was set up. Not those who do not start, nor those Swiss
/// Equestrian turned down (as Rosson).
pub fn unplaced(state: &State, published: &PublishedClassId) -> Vec<EntryId> {
    let sections: Vec<&ClassId> = state.classes.values().filter(|c| &c.published == published).map(|c| &c.id).collect();
    if sections.is_empty() {
        return Vec::new();
    }
    let on_a_list = |e: &EntryId| sections.iter().any(|c| state.starts.get(*c).is_some_and(|s| s.iter().any(|s| &s.entry == e)));
    state.entries.values().filter(|e| &e.published == published && !e.waiting_list && e.status != EntryStatus::NotStarting && !on_a_list(&e.id)).map(|e| e.id.clone()).collect()
}

/// Put an entry on no start list at the end of one of its class's (sections') start lists, with
/// the next start number once numbers are given.
pub fn place_entry(state: &State, entry: &EntryId, class: &ClassId) -> Result<Vec<Change>, String> {
    let e = state.entries.get(entry).ok_or_else(|| format!("unknown entry {entry}"))?;
    let c = state.classes.get(class).ok_or_else(|| format!("unknown class {class}"))?;
    if c.published != e.published {
        return Err("the entry is for another class".into());
    }
    if state.status_of(c) == ClassStatus::Final {
        return Err(format!("class {} is final; reopen it first", c.number));
    }
    if !unplaced(state, &e.published).contains(entry) {
        return Err("the entry is on a start list already".into());
    }
    Ok(placed_in(state, c, entry))
}

/// A late entry (an organiser's place). `id`: a free entry id.
pub fn add_entry(state: &State, published: &PublishedClassId, rider_pick: &RiderPick, horse_pick: &HorsePick, team_pick: Option<&TeamPick>, id: EntryId) -> Result<Vec<Change>, String> {
    let p = state.published_classes.get(published).ok_or_else(|| format!("unknown published class {published}"))?;
    let team = match (p.team_size, team_pick) {
        (None, _) => None,
        (Some(_), None) => return Err("a team class: the entry joins a team or makes a new one".into()),
        (Some(size), Some(TeamPick::Join(t))) => {
            let (_, riders) = teams_of(state, published).into_iter().find(|(x, _)| x == t).ok_or_else(|| format!("no team {t} in class {}", p.number))?;
            if riders >= size as usize {
                return Err(format!("the team {} is full ({size} riders)", state.teams.get(t).map_or(t.0.as_str(), |t| t.name.as_str())));
            }
            Some((t.clone(), None))
        }
        (Some(_), Some(TeamPick::New(name))) => {
            let name = name.trim();
            if name.is_empty() {
                return Err("a new team needs a name".into());
            }
            // Rosson's id for a team made at the show: the time, as the entry's.
            let team = Team { id: TeamId(id.0.to_string()), published: published.clone(), name: name.to_string(), start_number: None };
            Some((team.id.clone(), Some(team)))
        }
    };
    if state.classes.values().any(|c| &c.published == published && c.status == ClassStatus::Final) {
        return Err(format!("class {} is final; reopen it first", p.number));
    }
    let mut changes = Vec::new();
    if let Some((_, Some(new))) = &team {
        changes.push(Change::TeamSet { team: new.clone() });
    }
    let team = team.map(|(t, _)| t);
    let (person, on_entry) = rider(state, rider_pick, &mut changes)?;
    let (horse_id, guest, horses) = horse(state, horse_pick, &person, &mut changes)?;
    let rider_id = person.id;
    if state.entries.values().any(|e| &e.published == published && e.rider == rider_id && e.horse == horse_id && e.guest_horse == guest && e.status == EntryStatus::Entered) {
        return Err(format!("this rider is entered with this horse in class {} already", p.number));
    }
    let entry = Entry {
        id: id.clone(),
        published: published.clone(),
        rider: rider_id,
        horse: horse_id,
        guest_horse: guest,
        status: EntryStatus::Entered,
        hors_concours: false,
        team: team.clone(),
        programme: None,
        horses_on_entry: horses,
        stage: None,
        rider_on_entry: Some(on_entry),
        waiting_list: false,
        fee_paid: None,
        organiser_place: true,
        se_status: String::new(),
        entered_by: None,
        other_horses: Vec::new(),
        winning_points: WinningPoints::default(),
        se_changed_at: None,
        rider_changed: false,
        comment: String::new(),
        record: Value::Null,
    };
    changes.push(Change::EntryAdded { entry });
    changes.extend(placed(state, published, &id, team));
    Ok(changes)
}

/// Take an office entry back (with its starts and results).
pub fn remove_entry(state: &State, entry: &EntryId) -> Result<Vec<Change>, String> {
    let e = state.entries.get(entry).ok_or_else(|| format!("unknown entry {entry}"))?;
    if !e.organiser_place {
        return Err("only entries made by the office can be taken back; this one: does not start".into());
    }
    no_final(state, entry)?;
    let mut changes: Vec<Change> = state.results.keys().filter(|(_, e)| e == entry).map(|(c, e)| Change::ResultRemoved { class: c.clone(), entry: e.clone() }).collect();
    changes.push(Change::EntryRemoved { entry: entry.clone() });
    Ok(changes)
}

/// Another rider for entries (a pair's, in the classes the office picked), or back to the one
/// entered.
pub fn change_rider(state: &State, entries: &[EntryId], pick: &RiderPick) -> Result<Vec<Change>, String> {
    if entries.is_empty() {
        return Err("no entry to change".into());
    }
    let mut changes = Vec::new();
    let (person, on_entry) = rider(state, pick, &mut changes)?;
    let rider_id = person.id;
    let mut edited = false;
    for entry in entries {
        let e = state.entries.get(entry).ok_or_else(|| format!("unknown entry {entry}"))?;
        no_final(state, entry)?;
        if rider_id == e.rider {
            continue;
        }
        // A change only against the rider Swiss Equestrian delivered (office entries: a correction).
        let entered = state.delivered_entries.get(entry).map(|d| d.rider.clone());
        let changed = entered.is_some_and(|r| r != rider_id);
        let edit = EntryEdit { rider: Some(RiderChange { rider: rider_id.clone(), rider_on_entry: Some(on_entry.clone()), changed }), ..Default::default() };
        changes.push(Change::EntryEdited { entry: entry.clone(), edit });
        edited = true;
    }
    // The same rider everywhere: nothing changes (not even a rider copied from the registry).
    if !edited {
        return Ok(Vec::new());
    }
    Ok(changes)
}

/// Move an entry to another published class of the same discipline.
pub fn move_to_class(state: &State, entry: &EntryId, to: &PublishedClassId) -> Result<Vec<Change>, String> {
    let e = state.entries.get(entry).ok_or_else(|| format!("unknown entry {entry}"))?;
    if &e.published == to {
        return Ok(Vec::new());
    }
    let (from, target) = (state.published_classes.get(&e.published), state.published_classes.get(to).ok_or_else(|| format!("unknown published class {to}"))?);
    if from.is_some_and(|f| f.discipline != target.discipline) {
        return Err("an entry moves to a class of the same discipline".into());
    }
    if from.is_some_and(|f| f.team_size.is_some()) || target.team_size.is_some() || e.team.is_some() {
        return Err("team classes: entries move with their team (not yet in the app)".into());
    }
    no_final(state, entry)?;
    if state.classes.values().any(|c| &c.published == to && c.status == ClassStatus::Final) {
        return Err(format!("class {} is final; reopen it first", target.number));
    }
    let mut changes: Vec<Change> = state.results.keys().filter(|(_, x)| x == entry).map(|(c, x)| Change::ResultRemoved { class: c.clone(), entry: x.clone() }).collect();
    for c in classes_of(state, entry) {
        let mut starts: Vec<Start> = state.starts.get(&c).cloned().unwrap_or_default().into_iter().filter(|s| &s.entry != entry).collect();
        starts.sort_by_key(|s| s.order);
        for (i, s) in starts.iter_mut().enumerate() {
            s.order = i as u32 + 1;
        }
        changes.push(Change::starts_set(c, starts));
    }
    changes.push(Change::EntryEdited { entry: entry.clone(), edit: EntryEdit { published: Some(to.clone()), ..Default::default() } });
    changes.extend(placed(state, to, entry, None));
    Ok(changes)
}

/// An entry's level (`Some(0)`: none) and the office's note.
pub fn edit_details(state: &State, entry: &EntryId, stage: Option<i64>, comment: Option<String>) -> Result<Vec<Change>, String> {
    let e = state.entries.get(entry).ok_or_else(|| format!("unknown entry {entry}"))?;
    let stage = stage.filter(|s| e.stage.unwrap_or(0) != *s);
    let comment = comment.map(|c| c.trim().to_string()).filter(|c| c != &e.comment);
    if stage.is_none() && comment.is_none() {
        return Ok(Vec::new());
    }
    if stage.is_some() {
        no_final(state, entry)?;
    }
    Ok(vec![Change::EntryEdited { entry: entry.clone(), edit: EntryEdit { stage, comment, ..Default::default() } }])
}

/// Driving: the phases one horse of an entry starts in (Rosson's horse × phase grid,
/// `fahren-pferde-verwalten`): `horse` is its place on the entry, `phases` Swiss Equestrian's
/// names of the class's phases (`dressur`, `gelaende`, `hindernis` …).
pub fn set_horse_phases(state: &State, entry: &EntryId, horse: usize, phases: &[String]) -> Result<Vec<Change>, String> {
    let e = state.entries.get(entry).ok_or_else(|| format!("unknown entry {entry}"))?;
    let published = state.published_classes.get(&e.published).ok_or("unknown published class")?;
    if published.discipline != "CA" {
        return Err("not a driving class".into());
    }
    if horse >= e.horses_on_entry.len() {
        return Err(format!("the entry has no horse {}", horse + 1));
    }
    let known: Vec<&str> = published.driving_phases.iter().filter_map(|p| p.as_str()).collect();
    if let Some(p) = phases.iter().find(|p| !known.contains(&p.as_str())) {
        return Err(format!("{p} is not a phase of the class"));
    }
    let mut all: Vec<Vec<String>> = e.horses_on_entry.iter().map(|h| h["einsaetze"].as_array().into_iter().flatten().filter_map(|p| p.as_str().map(str::to_string)).collect()).collect();
    // In the class's order.
    let ordered: Vec<String> = known.iter().filter(|k| phases.iter().any(|p| p == *k)).map(|k| k.to_string()).collect();
    if all[horse] == ordered {
        return Ok(Vec::new());
    }
    all[horse] = ordered;
    Ok(vec![Change::EntryEdited { entry: entry.clone(), edit: EntryEdit { horse_phases: Some(all), ..Default::default() } }])
}

/// Riders the show knows, by name, licence number or town.
pub fn find_riders(state: &State, query: &str) -> Vec<Value> {
    let words: Vec<String> = crate::registry::fold(query).split_whitespace().map(str::to_string).collect();
    if words.is_empty() {
        return Vec::new();
    }
    // The show's riders only: those on its entries, not its officials, owners or organisers
    // (Swiss Equestrian's persons of the show are all of them; the user, 2026-11-15).
    let riders: std::collections::HashSet<&PersonId> = state.entries.values().map(|e| &e.rider).collect();
    let mut found: Vec<&Person> = state
        .persons
        .values()
        .filter(|p| riders.contains(&p.id))
        .filter(|p| {
            let text = crate::registry::fold(&format!("{} {} {} {} {}", p.first_name, p.last_name, p.town, p.club, p.id.0));
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .collect();
    found.sort_by(|a, b| (&a.last_name, &a.first_name).cmp(&(&b.last_name, &b.first_name)));
    found.into_iter().take(12).map(|p| rider_found(p, true)).collect()
}

/// A rider found, for the office to pick (`in_show`: the show has them already).
pub fn rider_found(p: &Person, in_show: bool) -> Value {
    json!({ "person": p.id.0, "name": format!("{} {}", p.first_name, p.last_name), "town": p.town, "club": p.club, "licences": p.licences, "guest": guest_licence(&p.licences), "in_show": in_show })
}

/// The show's horses matching `query` (name, owner, passport number).
pub fn find_horses(state: &State, query: &str, year: Option<u32>) -> Vec<Value> {
    let words: Vec<String> = crate::registry::fold(query).split_whitespace().map(str::to_string).collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<&Horse> = state
        .horses
        .values()
        .filter(|h| h.id.0 != GUEST_HORSE)
        .filter(|h| {
            let text = crate::registry::fold(&format!("{} {} {}", h.name, h.owner, h.id.0));
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found.into_iter().take(12).map(|h| horse_found(h, true, year)).collect()
}

/// A horse found, for the office to pick: `paid`: Swiss Equestrian lets it start in the show's
/// year (Rosson offers only those).
pub fn horse_found(h: &Horse, in_show: bool, year: Option<u32>) -> Value {
    let detail = [h.signalement.as_str(), h.owner.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" · ");
    json!({ "horse": h.id.0, "name": h.name, "detail": detail, "in_show": in_show, "paid": crate::registry::paid_for(&h.record, year) })
}


