//! Dividing a class into sections (10a, 10b, …), cleanly (U13): the office sees every rider
//! with their horses — where they live, their club, the entries' comments (the coach is often
//! written there), pony sizes and levels — puts them into sections by a rule or by hand, sees
//! the result, and applies it as one change. Divided by club, town, comment or size, a rider's
//! horses stay in one section; divided by pony size or level, each horse goes where it is
//! ranked (the sections of a pony class are ranked separately, P. Burkhard 2026-11-13), so a
//! rider may be in two. In each section the horses are spread out (U11). Applying again changes
//! the division (two, three or more sections; one: the class whole again), as long as nothing
//! has results.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use show::model::*;
use show::oplog::Change;
use show::State;

/// A horse of a rider in the class.
#[derive(Debug, Clone, Serialize)]
pub struct Horse {
    pub entry: u64,
    pub horse: String,
    /// Pony size (A, B, C, D), if a pony.
    pub pony: Option<String>,
    pub level: Option<i64>,
    /// The office's note on the entry, and the rider's remark sent with it.
    pub comment: String,
    pub remarks: String,
    pub not_starting: bool,
    pub start_number: Option<u32>,
    /// The section it is in now.
    pub section: usize,
}

/// A rider of the class, with their horses (each with its section).
#[derive(Debug, Clone, Serialize)]
pub struct Rider {
    pub person: u64,
    pub name: String,
    pub town: String,
    pub club: String,
    pub horses: Vec<Horse>,
}

/// A published class as it is divided now.
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub published: u64,
    pub number: String,
    /// The sections now (one: not divided), by class id.
    pub sections: Vec<String>,
    /// In start order (the first horse's place, section by section).
    pub riders: Vec<Rider>,
    /// Not to be divided again: results, a final class, or teams.
    pub locked: Option<Lock>,
}

/// Why a class is not divided (again): `teams`, `phases` (eventing, driving), `final` or
/// `results`, with the class (section) for the last two; the screen says it in the office's
/// language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lock {
    pub why: &'static str,
    pub class: Option<String>,
}

impl Lock {
    /// As an error (the commands' English).
    fn text(&self) -> String {
        let class = self.class.as_deref().unwrap_or_default();
        match self.why {
            "teams" => "team classes are divided by teams".into(),
            "phases" => "eventing classes are not divided".into(),
            "final" => format!("class {class} is final"),
            _ => format!("class {class} has results"),
        }
    }
}

/// A way to put the riders into sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    /// As many starters in each, in the start order.
    Balanced,
    /// The same club together (then town, comment).
    Club,
    Town,
    /// The same comment together (the coach is often written there).
    Comment,
    /// By level, horse by horse, the lower first.
    Level,
    /// By pony size, horse by horse: a section per group of sizes (e.g. AB · C · D); a horse
    /// without a size goes in the last.
    Pony(Vec<Vec<String>>),
}

fn classes_of<'a>(state: &'a State, published: &PublishedClassId) -> Vec<&'a Class> {
    let mut classes: Vec<&Class> = state.classes.values().filter(|c| &c.published == published).collect();
    classes.sort_by(|a, b| (a.number.len(), &a.number).cmp(&(b.number.len(), &b.number)));
    classes
}

/// The class as divided now, with everything the office divides by.
pub fn plan(state: &State, published: &PublishedClassId) -> Result<Plan, String> {
    let p = state.published_classes.get(published).ok_or_else(|| format!("unknown published class {published}"))?;
    let classes = classes_of(state, published);
    if classes.is_empty() {
        return Err(format!("class {} is not set up", p.number));
    }
    let lock = |why: &'static str, class: Option<&Class>| Some(Lock { why, class: class.map(|c| c.number.clone()) });
    let locked = if p.team_size.is_some() {
        lock("teams", None)
    } else if classes.iter().any(|c| c.phase.is_some() || !c.phases.is_empty()) {
        lock("phases", None)
    } else if let Some(c) = classes.iter().find(|c| c.status == ClassStatus::Final) {
        lock("final", Some(c))
    } else if let Some(c) = classes.iter().find(|c| crate::prepare::has_results(state, &c.id)) {
        lock("results", Some(c))
    } else {
        None
    };
    let mut riders: Vec<Rider> = Vec::new();
    for (i, class) in classes.iter().enumerate() {
        let mut starts = state.starts.get(&class.id).cloned().unwrap_or_default();
        starts.sort_by_key(|s| s.order);
        for s in starts {
            let Some(e) = state.entries.get(&s.entry) else { continue };
            let horse = Horse {
                entry: e.id.0,
                horse: e.guest_horse.clone().or_else(|| e.horse.as_ref().and_then(|h| state.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default(),
                pony: crate::prepare::pony_size(e),
                level: e.stage,
                comment: crate::views::said(&e.comment),
                remarks: state.entry_notes.get(&e.id).map(|n| crate::views::said(&n.remarks)).unwrap_or_default(),
                not_starting: e.status == EntryStatus::NotStarting,
                start_number: s.start_number,
                section: i,
            };
            match riders.iter_mut().find(|r| r.person == e.rider.0) {
                Some(r) => r.horses.push(horse),
                None => {
                    let person = state.persons.get(&e.rider);
                    let on_entry = e.rider_on_entry.as_ref();
                    riders.push(Rider {
                        person: e.rider.0,
                        name: on_entry.map(|r| format!("{} {}", r.first_name, r.last_name)).or_else(|| person.map(|p| format!("{} {}", p.first_name, p.last_name))).unwrap_or_default(),
                        town: on_entry.map(|r| r.town.clone()).or_else(|| person.map(|p| p.town.clone())).unwrap_or_default(),
                        club: on_entry.map(|r| r.club.clone()).or_else(|| person.map(|p| p.club.clone())).unwrap_or_default(),
                        horses: vec![horse],
                    });
                }
            }
        }
    }
    Ok(Plan { published: published.0, number: p.number.clone(), sections: classes.iter().map(|c| c.id.0.clone()).collect(), riders, locked })
}

/// The sections by a rule, for `parts` sections: for each rider of the plan, the section of
/// each of their horses.
pub fn suggest(plan: &Plan, rule: &Rule, parts: usize) -> Result<Vec<Vec<usize>>, String> {
    let parts = match rule {
        Rule::Pony(groups) => groups.len(),
        _ => parts,
    };
    if parts == 0 || parts > 26 {
        return Err(format!("{parts} sections?"));
    }
    let sizes: Vec<usize> = plan.riders.iter().map(|r| r.horses.len()).collect();
    let total: usize = sizes.iter().sum();
    // Groups (riders with the same key) to the section with fewest starters, biggest first.
    let by_key = |key: &dyn Fn(&Rider) -> String| -> Vec<usize> {
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut alone = Vec::new();
        for (i, r) in plan.riders.iter().enumerate() {
            let k = key(r).trim().to_lowercase();
            if k.is_empty() {
                alone.push(vec![i]);
            } else {
                groups.entry(k).or_default().push(i);
            }
        }
        let mut groups: Vec<Vec<usize>> = groups.into_values().collect();
        groups.sort_by_key(|g| std::cmp::Reverse(g.iter().map(|i| sizes[*i]).sum::<usize>()));
        groups.extend(alone);
        let mut load = vec![0usize; parts];
        let mut out = vec![0usize; plan.riders.len()];
        for g in groups {
            let s = (0..parts).min_by_key(|s| (load[*s], *s)).unwrap_or(0);
            for i in g {
                out[i] = s;
                load[s] += sizes[i];
            }
        }
        out
    };
    let comment = |r: &Rider| r.horses.iter().map(|h| if h.comment.trim().is_empty() { h.remarks.clone() } else { h.comment.clone() }).find(|c| !c.trim().is_empty()).unwrap_or_default();
    // A rider's section for all their horses.
    let whole = |sections: Vec<usize>| -> Vec<Vec<usize>> { plan.riders.iter().zip(sections).map(|(r, s)| vec![s; r.horses.len()]).collect() };
    Ok(match rule {
        Rule::Balanced => whole({
            let mut before = 0;
            sizes
                .iter()
                .map(|k| {
                    // The section where the rider's middle falls.
                    let s = ((2 * before + k) * parts / (2 * total.max(1))).min(parts - 1);
                    before += k;
                    s
                })
                .collect()
        }),
        Rule::Club => whole(by_key(&|r| r.club.clone())),
        Rule::Town => whole(by_key(&|r| r.town.clone())),
        Rule::Comment => whole(by_key(&comment)),
        Rule::Level => {
            let levels: Vec<Option<i64>> = plan.riders.iter().flat_map(|r| r.horses.iter().map(|h| h.level)).collect::<BTreeSet<_>>().into_iter().collect();
            // Levels in order, cut into `parts` runs of about the same number of starters.
            let count = |l: &Option<i64>| plan.riders.iter().flat_map(|r| &r.horses).filter(|h| h.level == *l).count();
            let mut before = 0;
            let section: BTreeMap<Option<i64>, usize> = levels
                .iter()
                .map(|l| {
                    let k = count(l);
                    let s = ((2 * before + k) * parts / (2 * total.max(1))).min(parts - 1);
                    before += k;
                    (*l, s)
                })
                .collect();
            plan.riders.iter().map(|r| r.horses.iter().map(|h| section[&h.level]).collect()).collect()
        }
        Rule::Pony(groups) => {
            let groups: Vec<Vec<String>> = groups.iter().map(|g| g.iter().map(|s| s.trim().to_uppercase()).collect()).collect();
            plan.riders
                .iter()
                .map(|r| r.horses.iter().map(|h| h.pony.as_ref().and_then(|s| groups.iter().position(|g| g.contains(s))).unwrap_or(parts - 1)).collect())
                .collect()
        }
    })
}

/// Divide the class as given: `sections[i][j]` is the section of the plan's `i`-th rider's
/// `j`-th horse, `parts` sections in all (one: the class whole again). Sections keep their class (officials, layout,
/// details) when they exist; new ones take the first's, with its officials. In each section the
/// riders keep their order and a rider's horses are spread out (U11); start numbers stay.
pub fn apply(state: &State, published: &PublishedClassId, sections: &[Vec<usize>], parts: usize, new_id: &mut dyn FnMut() -> String) -> Result<Vec<Change>, String> {
    let plan = plan(state, published)?;
    if let Some(why) = &plan.locked {
        return Err(why.text());
    }
    if parts == 0 || parts > 26 {
        return Err(format!("{parts} sections?"));
    }
    let fits = sections.len() == plan.riders.len() && plan.riders.iter().zip(sections).all(|(r, s)| s.len() == r.horses.len() && s.iter().all(|x| *x < parts));
    if !fits {
        return Err("each horse goes in one of the sections".into());
    }
    if (0..parts).any(|s| !sections.iter().flatten().any(|x| *x == s)) {
        return Err("a section without riders".into());
    }
    let p = &state.published_classes[published];
    let old = classes_of(state, published);
    let template = old[0].clone();
    let ids: Vec<(ClassId, String)> = if parts == 1 {
        vec![(ClassId(published.0.to_string()), p.number.clone())]
    } else {
        (0..parts).map(|i| {
            let letter = (b'a' + i as u8) as char;
            (ClassId(format!("{}{letter}", published.0)), format!("{}{letter}", p.number))
        }).collect()
    };
    let mut changes = Vec::new();
    for (i, (id, number)) in ids.iter().enumerate() {
        let class = match state.classes.get(id) {
            Some(c) => Class { number: number.clone(), ..c.clone() },
            None => Class { id: id.clone(), number: number.clone(), status: ClassStatus::Planned, ..template.clone() },
        };
        // The section's riders in their order now; a rider's horses spread out.
        let entries: Vec<EntryId> = plan.riders.iter().zip(sections).flat_map(|(r, s)| r.horses.iter().zip(s).filter(|(_, x)| **x == i).map(|(h, _)| EntryId(h.entry))).collect();
        let mut current: Vec<(usize, u32, EntryId)> = Vec::new();
        for (k, c) in old.iter().enumerate() {
            for s in state.starts.get(&c.id).into_iter().flatten() {
                if entries.contains(&s.entry) {
                    current.push((k, s.order, s.entry.clone()));
                }
            }
        }
        current.sort();
        let ordered = crate::prepare::spread_entries(state, published, current.into_iter().map(|(_, _, e)| e).collect());
        let number_of = |e: &EntryId| old.iter().flat_map(|c| state.starts.get(&c.id).into_iter().flatten()).find(|s| &s.entry == e).and_then(|s| s.start_number);
        let starts = ordered.iter().enumerate().map(|(k, e)| Start { entry: e.clone(), order: k as u32 + 1, start_number: number_of(e), team_order: None }).collect();
        changes.push(Change::class_set(class));
        changes.push(Change::starts_set(id.clone(), starts));
        // A new section: the officials of the class it comes from.
        if !state.classes.contains_key(id) {
            for a in state.assignments.values().filter(|a| a.class.as_ref() == Some(&template.id)) {
                changes.push(Change::AssignmentSet { assignment: Assignment { id: AssignmentId(new_id()), class: Some(id.clone()), se_id: None, readonly: Some(false), looked_up: false, ..a.clone() } });
            }
        }
    }
    // Classes no longer used: their officials, then themselves.
    for c in old.iter().filter(|c| !ids.iter().any(|(id, _)| id == &c.id)) {
        changes.extend(state.assignments.values().filter(|a| a.class.as_ref() == Some(&c.id)).map(|a| Change::AssignmentRemoved { assignment: a.id.clone() }));
        changes.push(Change::ClassRemoved { class: c.id.clone() });
    }
    // Nothing different: nothing to do.
    let same = old.len() == parts && ids.iter().zip(&old).all(|((id, _), c)| id == &c.id) && changes.iter().all(|ch| match ch {
        Change::StartsSet { class, starts, .. } => state.starts.get(class).map(|s| {
            let mut s = s.clone();
            s.sort_by_key(|s| s.order);
            s
        }).as_ref() == Some(starts),
        Change::ClassSet { class, .. } => state.classes.get(&class.id) == Some(class),
        _ => false,
    });
    Ok(if same { Vec::new() } else { changes })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn horse(entry: u64, pony: Option<&str>, level: Option<i64>) -> Horse {
        Horse { entry, horse: format!("H{entry}"), pony: pony.map(str::to_string), level, comment: String::new(), remarks: String::new(), not_starting: false, start_number: None, section: 0 }
    }

    fn rider(person: u64, club: &str, horses: Vec<Horse>) -> Rider {
        Rider { person, name: format!("R{person}"), town: String::new(), club: club.into(), horses }
    }

    fn plan(riders: Vec<Rider>) -> Plan {
        Plan { published: 1, number: "10".into(), sections: vec!["1".into()], riders, locked: None }
    }

    #[test]
    fn ponies_go_where_they_are_ranked_even_from_one_rider() {
        // Rider 1 rides a B and a D pony: one in each section; a pony without size: the last.
        let p = plan(vec![rider(1, "x", vec![horse(11, Some("B"), None), horse(12, Some("D"), None)]), rider(2, "x", vec![horse(21, Some("C"), None)]), rider(3, "y", vec![horse(31, None, None)])]);
        let groups = vec![vec!["A".to_string(), "B".to_string()], vec!["C".to_string()], vec!["D".to_string()]];
        assert_eq!(suggest(&p, &Rule::Pony(groups), 2).unwrap(), vec![vec![0, 2], vec![1], vec![2]]);
    }

    #[test]
    fn levels_horse_by_horse_clubs_rider_by_rider() {
        let p = plan(vec![rider(1, "x", vec![horse(11, None, Some(1)), horse(12, None, Some(2))]), rider(2, "x", vec![horse(21, None, Some(2))]), rider(3, "y", vec![horse(31, None, Some(1))])]);
        assert_eq!(suggest(&p, &Rule::Level, 2).unwrap(), vec![vec![0, 1], vec![1], vec![0]]);
        // By club: rider 1's horses together, with rider 2 (same club).
        let club = suggest(&p, &Rule::Club, 2).unwrap();
        assert_eq!(club[0][0], club[0][1]);
        assert_eq!(club[0][0], club[1][0]);
        assert_ne!(club[0][0], club[2][0]);
    }
}

