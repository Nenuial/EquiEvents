//! A class's start list as the office prepares, prints and publishes it.
//!
//! The starters in start order (team classes: team by team, each under its heading), with what
//! the class's [`StartListSetup`] adds:
//!
//! - **start times**: the first starter at the first time, then one every `minutes`; a start
//!   that would run into a break moves to the break's end, and a break line goes before the
//!   first start after it; with "move up", starters who do not start have no time and give
//!   theirs to the next;
//! - **blocks**: a heading ("08h00", "à la suite") before every `size` starters (teams);
//! - **the line after the n-th starter** (team classes: after the n-th team): arena maintenance,
//!   or the title of the following section;
//! - **levels** (Stufen) in show jumping in levels: a heading where the level changes.
//!
//! Blocks, the inserted line and breaks only appear once start numbers are given out; until
//! then the starters are listed by level and name.
//!
//! *Better than Rosson:* the inserted line goes after the n-th **starter**; Rosson counted the
//! block headings as starters, so with blocks the line landed too early. A starter without level
//! gets no "Stufe null" heading.

use std::collections::HashMap;

use crate::model::*;
use crate::State;

/// One line of a start list.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    /// A starter. `position`: the start position printed; `time`: with start times.
    Starter { entry: EntryId, position: u32, time: Option<TimeOfDay> },
    /// A team's heading (team classes), with the team's start time.
    Team { team: TeamId, time: Option<TimeOfDay> },
    /// A block's heading.
    Block(String),
    /// The line after the n-th starter (or team).
    Inserted(String),
    /// A new level (Stufe).
    Level(i64),
    /// A break, from–to.
    Break(TimeOfDay, TimeOfDay),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StartList {
    pub lines: Vec<Line>,
    /// Start numbers have been given out.
    pub start_numbers: bool,
    /// Start times are computed (layout with start times).
    pub start_times: bool,
}

impl State {
    /// Start numbers are given out for a class (for the whole event, when they are fixed).
    pub fn start_numbers_assigned(&self, class: &Class) -> bool {
        match &self.event {
            Some(e) if e.fixed_start_numbers => e.start_numbers_assigned,
            _ => class.start_numbers_assigned,
        }
    }

    /// The start list of a class.
    pub fn start_list(&self, class_id: &ClassId) -> Result<StartList, String> {
        let class = self.classes.get(class_id).ok_or_else(|| format!("unknown class {class_id}"))?;
        let published = self.published_classes.get(&class.published);
        let start_numbers = self.start_numbers_assigned(class);
        let mut starts: Vec<&Start> = self.starts.get(class_id).into_iter().flatten().filter(|s| self.entries.contains_key(&s.entry)).collect();
        starts.sort_by_key(|s| s.order);
        let entry = |s: &Start| &self.entries[&s.entry];
        let not_starting = |s: &Start| entry(s).status == EntryStatus::NotStarting;

        // Start times, in start order.
        let (times, team_times) = match &class.start_list.layout {
            StartListLayout::StartTimes { first, minutes, breaks, move_up } => {
                let times = start_times(starts.iter().map(|s| (s.entry.clone(), not_starting(s))), *first, *minutes, breaks, *move_up);
                let teams = class.teams.iter().map(|t| {
                    let riders: Vec<&&Start> = starts.iter().filter(|s| entry(s).team.as_ref() == Some(t)).collect();
                    (t.clone(), !riders.is_empty() && riders.iter().all(|s| not_starting(s)))
                });
                (times, start_times(teams, *first, *minutes, breaks, *move_up))
            }
            _ => (HashMap::new(), HashMap::new()),
        };

        // Order: by team, by level and name before start numbers, else as drawn.
        let team_index = |s: &Start| entry(s).team.as_ref().and_then(|t| class.teams.iter().position(|x| x == t)).unwrap_or(usize::MAX);
        let team_class = !class.teams.is_empty();
        if team_class {
            starts.sort_by_key(|s| (team_index(s), s.order));
        } else if !start_numbers {
            // By level, then the rider's last name, first name, then the horse's name (accents and
            // case do not count).
            let name_key = |s: &Start| {
                let e = entry(s);
                let (last, first) = match (&e.rider_on_entry, self.persons.get(&e.rider)) {
                    (Some(r), _) => (r.last_name.clone(), r.first_name.clone()),
                    (None, Some(p)) => (p.last_name.clone(), p.first_name.clone()),
                    _ => Default::default(),
                };
                let horse = e.guest_horse.clone().or_else(|| e.horses_on_entry.first().and_then(|h| h["pferdename"].as_str()).map(str::to_string)).or_else(|| e.horse.as_ref().and_then(|h| self.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default();
                (e.stage.unwrap_or(0), last, first, horse)
            };
            let names = scoring::ranking::compare_names;
            starts.sort_by(|a, b| {
                let (x, y) = (name_key(a), name_key(b));
                x.0.cmp(&y.0).then_with(|| names(&x.1, &y.1)).then_with(|| names(&x.2, &y.2)).then_with(|| names(&x.3, &y.3))
            });
        }

        let mut lines: Vec<Line> = Vec::new();
        let mut headed: Vec<&TeamId> = Vec::new();
        for (i, s) in starts.iter().enumerate() {
            if let Some(team) = entry(s).team.as_ref().filter(|t| class.teams.contains(t) && !headed.contains(t)) {
                headed.push(team);
                lines.push(Line::Team { team: team.clone(), time: team_times.get(team).copied().flatten() });
            }
            // Team classes: the rider's place in the team; otherwise the running number.
            let position = if team_class { s.order } else { i as u32 + 1 };
            lines.push(Line::Starter { entry: s.entry.clone(), position, time: times.get(&s.entry).copied().flatten() });
        }

        let is_team = |l: &Line| matches!(l, Line::Team { .. });
        let is_starter = |l: &Line| matches!(l, Line::Starter { .. });
        // Index of the k-th team heading (team classes) or starter line (others), k from 0.
        let nth = |lines: &[Line], k: usize, pick: &dyn Fn(&Line) -> bool| lines.iter().enumerate().filter(|(_, l)| pick(l)).nth(k).map(|(i, _)| i);

        if let (StartListLayout::Blocks { size, headings, move_up }, true) = (&class.start_list.layout, start_numbers) {
            let size = (*size).max(1) as usize;
            let units: Vec<usize> = if team_class {
                lines.iter().enumerate().filter(|(_, l)| is_team(l)).map(|(i, _)| i).collect()
            } else {
                lines
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| matches!(l, Line::Starter { entry, .. } if !(*move_up && self.entries[entry].status == EntryStatus::NotStarting)))
                    .map(|(i, _)| i)
                    .collect()
            };
            for (block, at) in units.iter().step_by(size).enumerate().collect::<Vec<_>>().into_iter().rev() {
                lines.insert(*at, Line::Block(headings.get(block).cloned().unwrap_or_default()));
            }
        }

        if let (Some((after, text)), true) = (&class.start_list.line_after, start_numbers) {
            let at = if team_class {
                nth(&lines, *after as usize, &is_team).map(|i| before_block(&lines, i))
            } else {
                nth(&lines, *after as usize - 1, &is_starter).map(|i| i + 1)
            };
            lines.insert(at.unwrap_or(lines.len()), Line::Inserted(text.clone()));
        }

        if !team_class && published.is_some_and(|p| p.levels > 1) {
            let mut previous: Option<Option<i64>> = None;
            let mut i = 0;
            while i < lines.len() {
                if let Line::Starter { entry, .. } = &lines[i] {
                    let level = self.entries[entry].stage;
                    if previous != Some(level) {
                        previous = Some(level);
                        if let Some(level) = level {
                            lines.insert(i, Line::Level(level));
                            i += 1;
                        }
                    }
                }
                i += 1;
            }
        }

        if let (StartListLayout::StartTimes { breaks, .. }, true) = (&class.start_list.layout, start_numbers) {
            let mut i = 0;
            while i < lines.len() {
                let time = match &lines[i] {
                    Line::Starter { time, .. } if !team_class => *time,
                    Line::Team { time, .. } if team_class => *time,
                    _ => None,
                };
                if let Some(b) = time.and_then(|t| breaks.iter().find(|(_, end)| *end == t)) {
                    lines.insert(i, Line::Break(b.0, b.1));
                    i += 1;
                }
                i += 1;
            }
        }

        let start_times = matches!(class.start_list.layout, StartListLayout::StartTimes { .. });
        Ok(StartList { lines, start_numbers, start_times })
    }
}

/// A line goes before a team's block heading, if the team opens a block.
fn before_block(lines: &[Line], i: usize) -> usize {
    if i > 0 && matches!(lines[i - 1], Line::Block(_)) { i - 1 } else { i }
}

/// Start times for starters (or teams) in start order; `true` = does not start.
fn start_times<K: std::hash::Hash + Eq>(
    order: impl Iterator<Item = (K, bool)>,
    first: TimeOfDay,
    minutes: u32,
    breaks: &[(TimeOfDay, TimeOfDay)],
    move_up: bool,
) -> HashMap<K, Option<TimeOfDay>> {
    let mut times = HashMap::new();
    let mut previous: Option<u32> = None;
    for (key, not_starting) in order {
        if move_up && not_starting {
            times.insert(key, None);
            continue;
        }
        let mut t = match previous {
            None => first.0 as u32,
            Some(p) => p + minutes,
        };
        for (start, end) in breaks {
            if t + minutes > start.0 as u32 && t < end.0 as u32 {
                t = end.0 as u32;
            }
        }
        previous = Some(t);
        times.insert(key, Some(TimeOfDay(t as u16)));
    }
    times
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_times_with_a_break_and_moving_up() {
        let breaks = [(TimeOfDay::new(9, 0), TimeOfDay::new(9, 30))];
        let order = [("a", false), ("b", true), ("c", false), ("d", false), ("e", false)];
        let t = start_times(order.into_iter(), TimeOfDay::new(8, 40), 8, &breaks, true);
        assert_eq!(t["a"], Some(TimeOfDay::new(8, 40)));
        assert_eq!(t["b"], None); // does not start, moves the others up
        assert_eq!(t["c"], Some(TimeOfDay::new(8, 48)));
        assert_eq!(t["d"], Some(TimeOfDay::new(9, 30))); // 8:56 would run into the break
        assert_eq!(t["e"], Some(TimeOfDay::new(9, 38)));
        let t = start_times(order.into_iter(), TimeOfDay::new(8, 40), 8, &[], false);
        assert_eq!(t["b"], Some(TimeOfDay::new(8, 48)));
    }
}
