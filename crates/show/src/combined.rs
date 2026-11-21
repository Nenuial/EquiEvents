//! Results across classes, rider and horse together:
//!
//! - **Rankings over several classes** (U8, docs/ux/brief.md; Rosson's *Finalprüfungen*), e.g. a
//!   championship over two or three classes, jumping or dressage or both, one of three ways,
//!   chosen for each ranking:
//!   - *penalties added*: the penalties of every class added (obstacle and time penalties, as the
//!     class's ranking has them); a tie is not broken (ex aequo), unless chosen for the ranking:
//!     by the penalties in one class, or its result there (penalties, then time);
//!   - *ranks added* (Rosson's *Rangpunkte*): the ranks in every class added, the fewest first; a
//!     tie is broken by the rank in one class, picked for that, then in the others in order;
//!   - *percentages* (Rosson's *Prozentpunkte*, dressage classes): the mean of the percentages,
//!     the highest first; a tie is broken by the percentage in one class, picked for that, then
//!     in the others in order (Rosson: the classes in order).
//!
//!   Over team classes, every one: the teams, matched by their name (Rosson's team finals; ranks
//!   or penalties added).
//!
//!   Who is in it: a rider and the same horse, as Rosson: those in every class are ranked; out
//!   (eliminated, retired…) in one of them: after the ranked, without a rank; missing a class:
//!   last. Still tied after the tie-breaks: the same rank. Placed: 30 % of those in every class,
//!   rounded up (Rosson). Meant for classes of one round: a class's ranking gives the penalties
//!   and time of its deciding round.
//! - **Penalties brought from a qualifier** (U9): a final's start list shows each rider's
//!   penalties in the qualifier; its start order can be the qualifier's ranking reversed (the
//!   best last). Only shown: the timekeeper includes them in the final's results, and the final's
//!   own result is what Swiss Equestrian gets.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use crate::model::*;
use crate::ranking::Line;
use crate::state::State;

/// A rider and a horse: who a result across classes belongs to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pair {
    pub rider: PersonId,
    pub horse: HorseKey,
}

/// A horse of Swiss Equestrian, or a guest horse by name (they share Rosson's placeholder horse).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HorseKey {
    Horse(HorseId),
    Guest(String),
    None,
}

/// Who a line of a ranking over several classes is: a rider and a horse, or a team (team
/// classes; Rosson matches teams by their name, the same in every class).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Who {
    Pair(Pair),
    Team(String),
}

/// A pair's (or a team's) result in one class, as the class's ranking has it.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassResult {
    /// The pair's entry (a team: its first rider's).
    pub entry: EntryId,
    /// The team, in team classes.
    pub team: Option<TeamId>,
    /// `None`: out (eliminated, retired…).
    pub rank: Option<u32>,
    pub faults: Option<Decimal>,
    pub time: Option<Decimal>,
    /// Dressage.
    pub percentage: Option<Decimal>,
}

impl ClassResult {
    /// Better than another result of the same pair in a class (a pair entered twice).
    fn better_than(&self, other: &ClassResult) -> bool {
        match (self.rank, other.rank) {
            (Some(a), Some(b)) => a < b,
            (Some(_), None) => true,
            _ => false,
        }
    }
}

/// One line of a ranking over several classes.
#[derive(Debug, Clone, PartialEq)]
pub struct CombinedLine {
    pub who: Who,
    /// The pair's result in each class, in the ranking's order of classes (`None`: not in it).
    pub results: Vec<Option<ClassResult>>,
    /// Penalties or ranks added, or the mean percentage; `None`: out in a class, or missing one.
    pub total: Option<Decimal>,
    /// What breaks a tie first: the penalties in the class picked, or the rank in it.
    pub tie_break: Option<Decimal>,
    pub rank: Option<u32>,
    /// Among the placed (30 % of those in every class, rounded up).
    pub placed: bool,
}

impl CombinedLine {
    /// In every class of the ranking.
    pub fn complete(&self) -> bool {
        self.results.iter().all(Option::is_some)
    }
}

impl State {
    /// The rider and horse of an entry.
    pub fn pair(&self, entry: &Entry) -> Pair {
        let horse = match (&entry.guest_horse, &entry.horse) {
            (Some(name), _) => HorseKey::Guest(crate::model::guest_horse_name(name)),
            (None, Some(h)) => HorseKey::Horse(h.clone()),
            (None, None) => HorseKey::None,
        };
        Pair { rider: entry.rider.clone(), horse }
    }

    /// A class's results by rider and horse (riders only; team lines left out).
    fn results_by_pair(&self, class: &ClassId) -> Result<(Vec<Line>, BTreeMap<Pair, ClassResult>), String> {
        let number = self.classes.get(class).map_or_else(|| class.to_string(), |c| c.number.clone());
        let ranking = self.ranking(class).map_err(|e| format!("class {number}: {e}"))?;
        let mut by_pair: BTreeMap<Pair, ClassResult> = BTreeMap::new();
        for line in ranking.lines.iter().filter(|l| !l.team) {
            let Some(entry) = line.id.parse::<u64>().ok().and_then(|id| self.entries.get(&EntryId(id))) else { continue };
            let result = ClassResult { entry: entry.id.clone(), team: None, rank: line.placement.rank(), faults: line.faults, time: line.time, percentage: line.percentage };
            match by_pair.get(&self.pair(entry)) {
                Some(known) if !result.better_than(known) => {}
                _ => {
                    by_pair.insert(self.pair(entry), result);
                }
            }
        }
        Ok((ranking.lines, by_pair))
    }

    /// A team class's results by team name (Rosson's finals match teams by name). Two teams of
    /// one name in a class are told apart by their order ("… (2)"); Rosson merged them and left
    /// both unranked.
    fn results_by_team(&self, class: &ClassId) -> Result<(Vec<String>, BTreeMap<Who, ClassResult>), String> {
        let number = self.classes.get(class).map_or_else(|| class.to_string(), |c| c.number.clone());
        let ranking = self.ranking(class).map_err(|e| format!("class {number}: {e}"))?;
        let mut order = Vec::new();
        let mut by_team = BTreeMap::new();
        for line in ranking.lines.iter().filter(|l| l.team) {
            let team = TeamId(line.id.clone());
            let Some(mut name) = self.teams.get(&team).map(|t| t.name.trim().to_string()) else { continue };
            let base = name.clone();
            let mut n = 1;
            while by_team.contains_key(&Who::Team(name.clone())) {
                n += 1;
                name = format!("{base} ({n})");
            }
            let first = self.entries.values().find(|e| e.team.as_ref() == Some(&team)).map(|e| e.id.clone()).unwrap_or(EntryId(0));
            let result = ClassResult { entry: first, team: Some(team), rank: line.placement.rank(), faults: line.faults, time: line.time, percentage: None };
            order.push(name.clone());
            by_team.insert(Who::Team(name), result);
        }
        Ok((order, by_team))
    }

    /// A ranking over several classes, as it stands: ranked first, then the pairs out in a class.
    pub fn combined_ranking(&self, id: &str) -> Result<Vec<CombinedLine>, String> {
        let ranking = self.combined.get(id).ok_or_else(|| format!("unknown ranking {id}"))?;
        let mut order: Vec<Who> = Vec::new();
        let mut per_class: Vec<BTreeMap<Who, ClassResult>> = Vec::new();
        // Team classes, every one: the teams are ranked (Rosson's team finals).
        let teams = ranking.classes.iter().all(|c| self.classes.get(c).and_then(|k| self.published_classes.get(&k.published)).is_some_and(|p| p.team_size.is_some()));
        for class in &ranking.classes {
            if teams {
                let (names, by_team) = self.results_by_team(class)?;
                for name in names {
                    if !order.contains(&Who::Team(name.clone())) {
                        order.push(Who::Team(name));
                    }
                }
                per_class.push(by_team);
                continue;
            }
            let (lines, by_pair) = self.results_by_pair(class)?;
            let by_pair: BTreeMap<Who, ClassResult> = by_pair.into_iter().map(|(p, r)| (Who::Pair(p), r)).collect();
            // Pairs in the order the classes' rankings list them.
            for line in lines.iter().filter(|l| !l.team) {
                if let Some(entry) = line.id.parse::<u64>().ok().and_then(|id| self.entries.get(&EntryId(id))) {
                    let pair = Who::Pair(self.pair(entry));
                    if by_pair.contains_key(&pair) && !order.contains(&pair) {
                        order.push(pair);
                    }
                }
            }
            per_class.push(by_pair);
        }
        let n = Decimal::from(ranking.classes.len() as u32);
        // The tie-breaks, smaller better: the method's own, then class by class.
        let tie_class = match &ranking.method {
            CombinedMethod::Ranks { tie_break } => ranking.classes.iter().position(|c| c == tie_break),
            CombinedMethod::Penalties { tie } => tie.class().and_then(|t| ranking.classes.iter().position(|c| c == t)),
            CombinedMethod::Percentages { tie_break } => tie_break.as_ref().and_then(|t| ranking.classes.iter().position(|c| c == t)),
        };
        let mut lines: Vec<(CombinedLine, Vec<Option<Decimal>>)> = order
            .into_iter()
            .map(|who| {
                let results: Vec<Option<ClassResult>> = per_class.iter().map(|m| m.get(&who).cloned()).collect();
                let all: Option<Vec<&ClassResult>> = results.iter().map(Option::as_ref).collect();
                let ranked = all.as_ref().filter(|rs| rs.iter().all(|r| r.rank.is_some()));
                let (total, tie_break, keys) = match (&ranking.method, ranked) {
                    (CombinedMethod::Penalties { tie }, Some(rs)) if rs.iter().all(|r| r.faults.is_some()) => {
                        let picked = tie_class.map(|i| rs[i]);
                        let keys = match tie {
                            PenaltiesTie::None => Vec::new(),
                            PenaltiesTie::Faults { .. } => vec![picked.and_then(|r| r.faults)],
                            PenaltiesTie::Result { .. } => vec![picked.and_then(|r| r.faults), picked.and_then(|r| r.time)],
                        };
                        (Some(rs.iter().filter_map(|r| r.faults).sum()), keys.first().copied().flatten(), keys)
                    }
                    (CombinedMethod::Ranks { .. }, Some(rs)) => {
                        let rank = |r: &&ClassResult| r.rank.map(Decimal::from);
                        let first = tie_class.and_then(|i| rank(&rs[i]));
                        let keys = std::iter::once(first).chain(rs.iter().enumerate().filter(|(i, _)| Some(*i) != tie_class).map(|(_, r)| rank(r))).collect();
                        (Some(rs.iter().filter_map(rank).sum()), first, keys)
                    }
                    (CombinedMethod::Percentages { .. }, Some(rs)) if rs.iter().all(|r| r.percentage.is_some()) => {
                        let mean = rs.iter().filter_map(|r| r.percentage).sum::<Decimal>() / n;
                        // The class picked first, then the others in order; higher better.
                        let percent = |r: &&ClassResult| r.percentage.map(|p| -p);
                        let first = tie_class.map(|i| percent(&rs[i]));
                        let keys = first.into_iter().chain(rs.iter().enumerate().filter(|(i, _)| Some(*i) != tie_class).map(|(_, r)| percent(r))).collect();
                        (Some(mean), None, keys)
                    }
                    _ => (None, None, Vec::new()),
                };
                (CombinedLine { who, results, total, tie_break, rank: None, placed: false }, keys)
            })
            .collect();

        // Ranked first (the best total, then the tie-breaks), then out in a class, then missing one.
        let higher_better = matches!(ranking.method, CombinedMethod::Percentages { .. });
        let group = |l: &CombinedLine| if l.total.is_some() { 0 } else if l.complete() { 1 } else { 2 };
        lines.sort_by(|(a, ka), (b, kb)| {
            group(a).cmp(&group(b)).then_with(|| match (a.total, b.total) {
                (Some(x), Some(y)) => {
                    let by_total = if higher_better { y.cmp(&x) } else { x.cmp(&y) };
                    // A missing tie-break after one that is there.
                    by_total.then_with(|| ka.iter().map(|k| (k.is_none(), *k)).cmp(kb.iter().map(|k| (k.is_none(), *k))))
                }
                _ => std::cmp::Ordering::Equal,
            })
        });
        let mut previous: Option<(Decimal, Vec<Option<Decimal>>, u32)> = None;
        for (i, (line, keys)) in lines.iter_mut().enumerate() {
            let Some(total) = line.total else { break };
            let rank = match &previous {
                Some((t, k, r)) if *t == total && k == keys => *r,
                _ => i as u32 + 1,
            };
            line.rank = Some(rank);
            previous = Some((total, keys.clone(), rank));
        }
        let complete = lines.iter().filter(|(l, _)| l.complete()).count();
        let placed = (complete as u32 * 3).div_ceil(10);
        Ok(lines
            .into_iter()
            .map(|(mut l, _)| {
                l.placed = l.rank.is_some_and(|r| r <= placed);
                l
            })
            .collect())
    }

    /// What each starter of a class brings from its qualifier (U9): the result there, by entry.
    /// Empty when the class has no qualifier.
    pub fn carried(&self, class: &ClassId) -> Result<BTreeMap<EntryId, ClassResult>, String> {
        let Some(qualifier) = self.classes.get(class).and_then(|c| c.options.qualifier.clone()) else { return Ok(BTreeMap::new()) };
        let (_, by_pair) = self.results_by_pair(&qualifier)?;
        Ok(self
            .starts
            .get(class)
            .into_iter()
            .flatten()
            .filter_map(|s| self.entries.get(&s.entry))
            .filter_map(|e| by_pair.get(&self.pair(e)).map(|r| (e.id.clone(), r.clone())))
            .collect())
    }

    /// A class's starters in the reverse order of its qualifier's ranking (U9): the qualifier's
    /// winner starts last, the riders out of it first. Riders of the qualifier who are not in
    /// this class are left out; starters without a result in the qualifier start before everyone,
    /// in their present order.
    pub fn reverse_of_qualifier(&self, class: &ClassId) -> Result<Vec<EntryId>, String> {
        let qualifier = self.classes.get(class).and_then(|c| c.options.qualifier.clone()).ok_or("the class has no qualifier")?;
        let (lines, _) = self.results_by_pair(&qualifier)?;
        // Place in the qualifier's ranking, by pair (its first line if entered twice).
        let mut place: BTreeMap<Pair, usize> = BTreeMap::new();
        for (i, line) in lines.iter().filter(|l| !l.team).enumerate() {
            if let Some(entry) = line.id.parse::<u64>().ok().and_then(|id| self.entries.get(&EntryId(id))) {
                place.entry(self.pair(entry)).or_insert(i);
            }
        }
        let mut starts: Vec<&Start> = self.starts.get(class).into_iter().flatten().collect();
        starts.sort_by_key(|s| s.order);
        let mut order: Vec<(Option<usize>, EntryId)> = starts.iter().map(|s| (self.entries.get(&s.entry).and_then(|e| place.get(&self.pair(e)).copied()), s.entry.clone())).collect();
        // Stable: those without a place keep their order, ahead of the others.
        order.sort_by_key(|(p, _)| p.map_or((0, 0), |p| (1, usize::MAX - p)));
        Ok(order.into_iter().map(|(_, e)| e).collect())
    }
}

// ─── A ranking's number ──────────────────────────────────────────────────────

impl State {
    /// Every ranking's number, Swiss Equestrian's for its results (decided by the user,
    /// 2026-11-15; never typed): its highest class's number with ".1", or ".2" when another
    /// ranking has ".1"… Set when the ranking is made or its classes change, then kept (Rosson's
    /// finals keep Rosson's). A ranking without one (made before) gets the first free one, in
    /// the order of their ids.
    pub fn combined_numbers(&self) -> BTreeMap<String, String> {
        let mut numbers: BTreeMap<String, String> = self.combined.values().filter_map(|r| r.number.clone().map(|n| (r.id.clone(), n))).collect();
        for r in self.combined.values().filter(|r| r.number.is_none()) {
            let n = free_number(&self.combined_base(&r.classes), numbers.values());
            numbers.insert(r.id.clone(), n);
        }
        numbers
    }

    /// A ranking's number (`combined_numbers`).
    pub fn combined_number(&self, id: &str) -> String {
        self.combined_numbers().remove(id).unwrap_or_default()
    }

    /// The number a ranking over `classes` gets: the one it has while its classes stay the same
    /// (`id`: the ranking; `None`: a new one), else its highest class's with the first free ".n".
    pub fn next_combined_number(&self, id: Option<&str>, classes: &[ClassId]) -> String {
        if let Some(r) = id.and_then(|id| self.combined.get(id)).filter(|r| r.classes == classes) {
            return self.combined_number(&r.id);
        }
        let numbers = self.combined_numbers();
        free_number(&self.combined_base(classes), numbers.iter().filter(|(i, _)| Some(i.as_str()) != id).map(|(_, n)| n))
    }

    /// The highest of the classes' numbers, as Swiss Equestrian knows them (a series': its
    /// class's, "10" for 10a); empty without classes.
    fn combined_base(&self, classes: &[ClassId]) -> String {
        let key = |n: &String| {
            let digits: String = n.chars().take_while(char::is_ascii_digit).collect();
            (digits.parse::<u64>().unwrap_or(0), n[digits.len()..].to_string())
        };
        classes
            .iter()
            .filter_map(|c| self.classes.get(c))
            .map(|c| self.published_classes.get(&c.published).map_or_else(|| c.number.clone(), |p| p.number.clone()))
            .max_by_key(key)
            .unwrap_or_default()
    }
}

/// `base`.1, or the next one not taken.
fn free_number<'a>(base: &str, taken: impl Iterator<Item = &'a String>) -> String {
    let taken: std::collections::BTreeSet<&String> = taken.collect();
    (1..).map(|n| format!("{base}.{n}")).find(|n| !taken.contains(n)).unwrap_or_default()
}
