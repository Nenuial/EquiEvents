//! Show jumping: entered faults and times → ranking, according to the class's scoring table.
//!
//! # The rule
//!
//! 1. **Deciding block.** Each rider is ranked in the first block of the scoring table
//!    ([`table`]) for which all required fields are entered. Faults and times of that block are
//!    added up.
//! 2. **Order of blocks.** Everyone ranked in a more decisive block is ahead of everyone in a
//!    less decisive one — e.g. riders who rode both rounds of a two-round class ahead of riders
//!    who rode round 1 only.
//! 3. **Within a block.** Fewer faults first (more points first in points-based blocks, S1),
//!    then the faster time. A missing or zero time counts as slowest.
//! 4. **Ties.** Same block, same fault (or point) total and, if the block is timed, the same
//!    time total: same rank. 4+0 and 0+4 faults with the same time are a tie (S11). Tied riders
//!    are listed by horse name (S4).
//! 5. **Jump-offs** (S13). Riders who rode the rounds before the jump-off are grouped by their
//!    score there (faults, or points; in a table-C round, the time) — time does not count for
//!    this. A group of two or more riders **equal first** is entitled to a jump-off; in
//!    championships with medal jump-offs (option [`Options::medal_jump_offs`]) so is every such
//!    group that includes a medal place (S14). Within such a group: the jump-off decides — faults,
//!    then a further jump-off if there is one, then the time. A rider of the group withdrawn,
//!    retired or eliminated in a jump-off ranks **equal last of that jump-off** — keeping the
//!    jump-offs completed before it (Springreglement 2.3.8; S8, S9, S29) — ahead of the next
//!    group. If every rider of a jump-off is out of it, nobody wins it: they are all equal last
//!    of it, e.g. five riders equal 5th (2.3.9; S30). A rider of the group with no result in it
//!    and not entered as out (not ridden yet, or cancelled) comes after those who rode it and
//!    ahead of those out of it; a jump-off nobody rode: all equal first of it. Other groups are
//!    ranked by time (rule 3).
//! 6. **Winning rounds** (Siegerrunde, S17) work like a jump-off, except who is entitled: the
//!    placed riders of the rounds before it (30 %, or the number the organiser set). Whether
//!    points are carried into the winning round is part of the scoring table (table 29 adds the
//!    round-1 faults, tables 30 and 31 start from zero). Withdrawn, retired or eliminated in the
//!    winning round: equal last of the winning round.
//! 7. **Second rounds** (S16) — round 2 of a two-round class, phase 2 of a normal two-phase
//!    class (S2): eliminated or retired there ranks **equal last of round 2**, ahead of the riders
//!    who only rode round 1. Withdrawn before round 2: ranked by round 1, like a rider who did not
//!    qualify (unlike a jump-off). In a normal two-phase class only riders clear in phase 1 can
//!    be in phase 2, and phase 2 follows without a break: withdrawing is not possible there, a
//!    rider who leaves the arena has retired — "withdrawn" entered for phase 2 counts as retired.
//! 8. **Special two-phase classes** (S2), everyone rides both phases: eliminated, retired or
//!    withdrawn in either phase is out of the class.
//! 9. Out otherwise — without any completed round, or after one but not in a jump-off or a
//!    second round — is **out**: equal last of the whole class (S31; Rosson ranked the second by
//!    its round).
//! 10. Out riders are listed after all ranked riders: by reason code, then by horse name (S4).
//! 11. **Missing time** in a points-based part (Zeitspringen, Punktespringen…): ranked last of
//!     those with the same points (Springreglement 4.3.3, 4.4.4; S32).
//! 12. **Times** count to the hundredth: finer digits are cut, not rounded (7.9; S33).
//! 13. **Placed:** in a class with a reduced second round, only riders who completed both rounds
//!     (3.4.2.1; S34); if nobody rode round 2 (cancelled), round 1 places.
//! 14. **Knock-out** (table 55): duels; each round's field holds the duel's result (Wertung C:
//!     the time with 3 s per fault). The rider who goes furthest ranks first; in the furthest
//!     round the faster result wins; the losers of the same round are equal, eliminated or not —
//!     1st, 2nd, two 3rd, four 5th: eight placed (4.5; S35).
//!
//! Source in Rosson: `RanglisteService.updateRanglisteSpringen`, `ResultatService.sortierenSpringen`.
//! Golden tests: every jumping class in `golden/corpus/`. Teams: [`team`].
//!
//! # Why a rider stands where they do
//!
//! [`rank_explained`] gives each rider a [`Why`]: the values the ranking compared, in the order
//! it compares them, each the rider's own ("round 1: 0 faults; jump-off: 4 faults, 33.00 s").
//! Whenever a class is ranked in a debug build (every test, the golden corpus included), the
//! order and the ties these values give are checked against the ranking itself: an explanation
//! that does not match the ranking fails the tests.
//!
//! Not covered yet: ideal-time and maximum-time classes, manual ranks.

pub mod table;
pub mod team;

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::number::Decimal;
use crate::ranking::{compare_names, competition_ranks, default_placed_count, order_ties_by_horse, Placement};
use table::{is_jump_off_field, Block, Phases, ScoringTable};

/// One rider (horse and rider combination) in a jumping class.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: String,
    /// Used to list tied riders (S4).
    pub horse: String,
    /// What was entered, by field (`strafpunkte_initial_1`, `zeit_stechen_1`, ...). Fields not
    /// ridden or not entered yet are missing.
    pub values: BTreeMap<String, Decimal>,
    /// Eliminated, retired, withdrawn, ...; `None` if the rider finished.
    pub out: Option<Out>,
}

/// Why a rider is out, as entered (Aufgabegrund).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Out {
    /// Swiss Equestrian reason code; out riders are listed by it.
    pub code: u32,
    pub kind: OutKind,
}

/// The kinds of reason the rules distinguish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OutKind {
    /// Ausgeschieden (also disqualified).
    Eliminated,
    /// Aufgegeben.
    Retired,
    /// Verzichtet: did not start the round.
    Withdrawn,
    Other,
}

/// Class options that change the ranking.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// Championships: every group of riders equal on score that includes a medal place (1–3)
    /// has its own jump-off, e.g. one for gold and silver and one for bronze (S14).
    pub medal_jump_offs: bool,
    /// Placed riders, if the organiser set the number; otherwise 30 % of the entries (S10).
    /// Decides who rides a winning round (rule 6).
    pub placed_count: Option<usize>,
    /// The class's ideal time, in seconds, for an ideal-time block (table AZI, S20).
    pub ideal_time: Option<Decimal>,
}

/// One rider's place in the ranking.
#[derive(Debug, Clone, PartialEq)]
pub struct Ranked {
    pub id: String,
    pub placement: Placement,
    /// Index of the deciding block, if any.
    pub block: Option<usize>,
    /// Faults (or points, in points-based blocks) of the deciding block.
    pub faults: Option<Decimal>,
    /// Time of the deciding block; zero when the block is not timed.
    pub time: Option<Decimal>,
    /// May be placed (klassiert) if ranked high enough: not in a class with a reduced second
    /// round without having completed both rounds (rule 13).
    pub placeable: bool,
}

/// Rank a class with default options.
pub fn rank(table: &ScoringTable, entries: &[Entry]) -> Vec<Ranked> {
    rank_with(table, entries, Options::default())
}

/// Rank a class. Returns the riders in listing order: ranked riders, then riders who are out.
/// Riders with nothing entered yet are not listed.
pub fn rank_with(table: &ScoringTable, entries: &[Entry], options: Options) -> Vec<Ranked> {
    rank_explained(table, entries, options).into_iter().map(|(r, _)| r).collect()
}

/// Rank a class, each rider with why they stand where they do ([`Why`]).
pub fn rank_explained(table: &ScoringTable, entries: &[Entry], options: Options) -> Vec<(Ranked, Why)> {
    if table.knock_out {
        return rank_knock_out(table, entries);
    }
    let jump_offs = table.blocks.iter().take_while(|b| is_jump_off(b)).count();
    let mut out: Vec<&Entry> = Vec::new();
    let mut riders: Vec<Rider> = Vec::new();

    for entry in entries {
        match evaluate(table, entry, jump_offs) {
            Some(rider) => riders.push(rider),
            None if entry.out.is_some() => out.push(entry),
            None => {}
        }
    }
    riders.retain(|r| {
        let keep = !(r.entry.out.is_some() && table.phases == Phases::Special);
        if !keep {
            out.push(r.entry);
        }
        keep
    });

    // Rule 5: which score groups are entitled to a jump-off.
    let with_jump_offs = jump_offs > 0 && !table.winning_round;
    if with_jump_offs {
        mark_jump_off_groups(table, &mut riders, jump_offs, options);
    }
    // Rule 6: the placed riders are entitled to the winning round.
    if jump_offs > 0 && table.winning_round {
        mark_winning_round(table, &mut riders, options.placed_count.unwrap_or_else(|| default_placed_count(entries.len())));
    }
    // Rule 5: score groups that rode a jump-off, entitled or not (a championship's medal
    // jump-off ridden without the option set): a rider of such a group out after the round was
    // in the jump-off.
    let rode_jump_off: Vec<Decimal> = riders.iter().filter(|r| with_jump_offs && r.round_block == jump_offs && r.jump_offs.iter().any(Option::is_some)).map(|r| r.round_score).collect();
    // Rules 5–9 for riders out after a completed round.
    for r in riders.iter_mut().filter(|r| r.entry.out.is_some()) {
        // Withdrawing from phase 2 of a normal two-phase class is not possible: it means retired.
        let withdrawn = r.entry.out.is_some_and(|o| o.kind == OutKind::Withdrawn) && table.phases != Phases::Normal;
        let second_round = r.round_block.checked_sub(1).filter(|&n| n >= jump_offs && is_second_round(table, n));
        r.kind = if r.jump_off_group || (r.round_block == jump_offs && rode_jump_off.contains(&r.round_score)) {
            Kind::EqualLastOfJumpOff
        } else if second_round.is_some() {
            if withdrawn {
                Kind::Finished
            } else if table.phases == Phases::Normal && !r.round_score.is_zero() {
                Kind::Out
            } else {
                Kind::EqualLastOfRound
            }
        } else {
            // Rule 9: out of the class, equal last of it (S31).
            Kind::Out
        };
        // Rule 5: the jump-off they are equal last of: the first they did not complete.
        if r.kind == Kind::EqualLastOfJumpOff {
            r.out_level = r.jump_offs.iter().position(Option::is_none);
        }
    }
    out.extend(riders.iter().filter(|r| r.kind == Kind::Out).map(|r| r.entry));
    riders.retain(|r| r.kind != Kind::Out);

    // Rules 2–5: order.
    riders.sort_by(|a, b| compare(table, a, b, options.ideal_time));
    let ranks = competition_ranks(&riders, |a, b| compare(table, a, b, options.ideal_time) == Ordering::Equal);
    // Why each rider stands there: what the ranking compared. In debug builds (every test),
    // checked against the ranking: the same order, the same ties.
    let whys: Vec<Why> = riders.iter().map(|r| why(table, r, options.ideal_time)).collect();
    #[cfg(debug_assertions)]
    for i in 1..riders.len() {
        let by_rule = compare(table, &riders[i - 1], &riders[i], options.ideal_time);
        assert_eq!(whys[i - 1].cmp_steps(&whys[i]), by_rule, "table {}: the why of {} ({:?}) and {} ({:?}) does not match the ranking", table.code, riders[i - 1].entry.id, whys[i - 1].steps, riders[i].entry.id, whys[i].steps);
    }
    let mut ranks = ranks;
    nobody_wins_a_jump_off_all_left(&riders, &whys, &mut ranks);
    let mut listed: Vec<(u32, Rider, Why)> = ranks.into_iter().zip(riders).zip(whys).map(|((rank, r), w)| (rank, r, w)).collect();
    order_ties_by_horse(&mut listed, |e| e.0, |e| e.1.entry.horse.as_str());

    // Rule 10.
    out.sort_by(|a, b| a.out.map(|o| o.code).cmp(&b.out.map(|o| o.code)).then_with(|| compare_names(&a.horse, &b.horse)));

    // Rule 13: a reduced second round places only riders who completed both rounds — once
    // round 2 was ridden: nobody completed it (cancelled), round 1 places.
    let completed_both = |r: &Rider| r.kind != Kind::EqualLastOfRound && table.blocks[r.round_block].required.iter().any(|f| f.ends_with("initial_2"));
    let round_2_ridden = listed.iter().any(|(_, r, _)| completed_both(r));
    listed
        .into_iter()
        .map(|(rank, r, why)| {
            let has_result = r.kind == Kind::Finished;
            let (faults, time) = r.deciding_totals;
            let ranked = Ranked {
                id: r.entry.id.clone(),
                placement: Placement::Rank(rank),
                block: Some(r.deciding_block),
                faults: has_result.then_some(faults),
                time: has_result.then_some(time),
                placeable: !table.reduced_second_round || !round_2_ridden || completed_both(&r),
            };
            (ranked, why)
        })
        .chain(out.into_iter().map(|e| {
            let ranked = Ranked { id: e.id.clone(), placement: Placement::Out, block: None, faults: None, time: None, placeable: false };
            (ranked, Why { steps: Vec::new(), standing: Standing::Out, out: e.out })
        }))
        .collect()
}

/// Why a rider stands where they do: the values the ranking compared, in the order it compares
/// them (rules 2–7), each the rider's own. Two riders are tied exactly when their steps are equal,
/// and one is listed before the other exactly when the first step that differs is better for it
/// (checked against the ranking in debug builds).
#[derive(Debug, Clone, PartialEq)]
pub struct Why {
    pub steps: Vec<Step>,
    pub standing: Standing,
    /// Out of the class: the reason.
    pub out: Option<Out>,
}

/// Where a rider stands, beyond the numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Ranked on their results.
    Finished,
    /// Rules 5 and 6: entitled to the jump-off or winning round, and ranked in it.
    InJumpOff,
    /// Rules 5 and 6: entitled to the jump-off or winning round, did not complete it.
    EqualLastOfJumpOff,
    /// Rule 7: eliminated or retired in round 2 (phase 2).
    EqualLastOfRound,
    /// Rules 8–10: out of the class.
    Out,
}

/// One thing the ranking compared, with the rider's value.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Rule 2: the block of the table the rider is ranked in (0 is the most decisive).
    Stands { block: usize },
    /// Rule 7: equal last of round 2 (phase 2), after everyone who completed it.
    LastOfRound(bool),
    /// Faults (fewer first), or points (more first), of a block.
    Score { block: usize, value: Decimal, points: bool },
    /// Rules 5 and 6: a jump-off or the winning round: its faults (points); or not ridden (not
    /// yet, or cancelled); or out of it (`out`): equal last of it.
    JumpOff { block: usize, result: Option<Decimal>, points: bool, out: bool },
    /// Knock-out: the duel round reached (rule 14), the furthest first.
    Reached { block: usize },
    /// The time of a block: faster first; none or zero counts as slowest.
    Time { block: usize, value: Decimal },
    /// S20: how far from the class's ideal time, over or under the same; closer first.
    IdealGap { block: usize, time: Decimal, gap: Decimal },
}

impl Step {
    /// The step as one value to compare: smaller is better.
    fn order(&self) -> (u8, Decimal) {
        let signed = |v: Decimal, points: bool| if points { -v } else { v };
        let flag = |b: bool| if b { Decimal::ONE } else { Decimal::ZERO };
        match self {
            Step::Stands { block } => (0, Decimal::from(*block as u64)),
            Step::LastOfRound(b) => (1, flag(*b)),
            Step::Reached { block } => (2, Decimal::from(*block as u64)),
            Step::Score { value, points, .. } => (4, signed(*value, *points)),
            // Completed, then not ridden, then out of it.
            Step::JumpOff { result: Some(v), points, .. } => (5, signed(*v, *points)),
            Step::JumpOff { result: None, out: false, .. } => (5, Decimal::MAX - Decimal::ONE),
            Step::JumpOff { result: None, out: true, .. } => (5, Decimal::MAX),
            Step::Time { value, .. } => (6, slowest_if_zero(*value)),
            Step::IdealGap { gap, .. } => (7, *gap),
        }
    }
}

impl Why {
    /// Ahead (Less), tied (Equal) or behind another rider, by these steps.
    pub fn cmp_steps(&self, other: &Why) -> Ordering {
        self.steps.iter().zip(&other.steps).map(|(a, b)| a.order().cmp(&b.order())).find(|o| *o != Ordering::Equal).unwrap_or(Ordering::Equal)
    }

    /// In words: "round 1: 0 faults; jump-off: 4 faults, 33.00 s".
    pub fn text(&self, table: &ScoringTable) -> String {
        if let Some(out) = self.out.filter(|_| self.standing == Standing::Out) {
            return format!("out: {} (reason {})", out_word(out.kind), out.code);
        }
        let label = |block: usize| table.block_label(block);
        // Phrases per block, in the order compared; the same score twice is said once.
        let mut parts: Vec<(String, Vec<String>)> = Vec::new();
        let mut say = |block: usize, phrase: String| match parts.last_mut() {
            Some((l, phrases)) if *l == label(block) => {
                if !phrases.contains(&phrase) {
                    phrases.push(phrase)
                }
            }
            _ => parts.push((label(block), vec![phrase])),
        };
        let mut stands = 0;
        let entitled = matches!(self.standing, Standing::InJumpOff | Standing::EqualLastOfJumpOff) && !table.winning_round && !table.knock_out;
        let the = |l: String| if l.ends_with(char::is_numeric) { l } else { format!("the {l}") };
        // In the jump-off or winning round: a jump-off not ridden is worth saying until one was.
        let in_contest = matches!(self.standing, Standing::InJumpOff | Standing::EqualLastOfJumpOff);
        let mut rode_one = false;
        // A part scored on time only (table C): its score is a time.
        let timed_score = |block: usize| table.blocks[block].faults.is_empty();
        let score = |block: usize, value: &Decimal, points: bool| match (timed_score(block), points) {
            (true, _) => format!("{value} s"),
            (false, true) => format!("{value} points"),
            (false, false) => format!("{value} faults"),
        };
        // Knock-out: the riders of the furthest round have its time; the others lost a duel.
        let in_final = self.steps.iter().any(|s| matches!(s, Step::Time { .. }));
        // The first jump-off ridden: the last jump-off block, just before the rounds.
        let first_jump_off = table.blocks.iter().rposition(|b| b.required.iter().any(|f| is_jump_off_field(f))).map(label).unwrap_or_default();
        let jump_off_word = if first_jump_off.ends_with(char::is_numeric) { first_jump_off } else { format!("the {first_jump_off}") };
        for step in &self.steps {
            match step {
                Step::Stands { block } => stands = *block,
                // Entitled to the jump-off: the score that made it so (time does not count).
                Step::Score { block, value, points } if entitled && *block == stands => say(*block, format!("{}, entitled to {jump_off_word}", score(*block, value, *points))),
                Step::JumpOff { block, result: None, out: true, .. } => say(*block, format!("{} — equal last of {}", self.out.map_or("out", |o| out_word(o.kind)), the(label(*block)))),
                // Not ridden: said only of a rider who was to ride it and rode none before.
                Step::JumpOff { result: None, .. } if !in_contest || rode_one => {}
                Step::LastOfRound(true) => say(stands, format!("{} — equal last of {}", self.out.map_or("out", |o| out_word(o.kind)), label(stands))),
                Step::LastOfRound(false) => {}
                Step::Reached { block } => match (self.out, in_final) {
                    (Some(o), _) => say(*block, out_word(o.kind).into()),
                    (None, true) => {}
                    (None, false) => say(*block, "lost the duel".into()),
                },
                // A part with no faults to add (table C): its time says it.
                Step::Score { block, value, .. } if timed_score(*block) && value.is_zero() => {}
                Step::Score { block, value, points } => say(*block, score(*block, value, *points)),
                Step::JumpOff { block, result: Some(v), points, .. } => {
                    rode_one = true;
                    if !(timed_score(*block) && v.is_zero()) {
                        say(*block, score(*block, v, *points))
                    }
                }
                Step::JumpOff { block, result: None, .. } => say(*block, "not ridden".into()),
                Step::Time { value, .. } if value.is_zero() && table.knock_out && self.out.is_some() => {}
                Step::Time { block, value } if value.is_zero() => say(*block, "no time (slowest)".into()),
                Step::Time { block, value } => say(*block, format!("{value} s")),
                Step::IdealGap { block, time, gap } => say(*block, format!("{time} s, {gap} s from the ideal time")),
            }
        }
        parts.into_iter().map(|(l, p)| format!("{l}: {}", p.join(", "))).collect::<Vec<_>>().join("; ")
    }
}

fn out_word(kind: OutKind) -> &'static str {
    match kind {
        OutKind::Eliminated => "eliminated",
        OutKind::Retired => "retired",
        OutKind::Withdrawn => "withdrawn",
        OutKind::Other => "out",
    }
}

/// The steps of a rider's ranking: [`compare`], step by step.
fn why(table: &ScoringTable, r: &Rider, ideal: Option<Decimal>) -> Why {
    let rode = r.jump_offs.iter().any(Option::is_some);
    let standing = match r.kind {
        Kind::EqualLastOfRound => Standing::EqualLastOfRound,
        Kind::EqualLastOfJumpOff => Standing::EqualLastOfJumpOff,
        _ if r.jump_off_group && rode => Standing::InJumpOff,
        _ if r.jump_off_group => Standing::EqualLastOfJumpOff,
        _ => Standing::Finished,
    };
    let mut why = Why { steps: Vec::new(), standing, out: r.entry.out };
    let steps = &mut why.steps;
    // Rule 2.
    let block = standing_block(table, r);
    steps.push(Step::Stands { block });
    // Rule 7.
    let last = r.kind == Kind::EqualLastOfRound;
    steps.push(Step::LastOfRound(last));
    if last {
        return why;
    }
    // Rule 6.
    if table.winning_round && block == 0 {
        jump_off_steps(table, r, steps);
        return why;
    }
    // Rule 5.
    let jump_offs = r.jump_offs.len();
    if jump_offs > 0 && !table.winning_round && block == jump_offs {
        steps.push(Step::Score { block, value: r.round_score, points: table.blocks[block].points });
        jump_off_steps(table, r, steps);
        if r.jump_off_group || rode || r.kind == Kind::EqualLastOfJumpOff {
            return why;
        }
    }
    // Rule 3.
    let (score, time) = totals_in(table, r, block);
    steps.push(Step::Score { block, value: score, points: table.blocks[block].points });
    match ideal.filter(|_| table.blocks[block].ideal_time) {
        Some(ideal) => steps.push(Step::IdealGap { block, time, gap: (time - ideal).abs() }),
        None if !table.blocks[block].time.is_empty() => steps.push(Step::Time { block, value: time }),
        None => {}
    }
    why
}

/// [`compare_jump_offs`], step by step: each jump-off until one not completed (out of it: equal
/// last of it, nothing more), then the time of the last one completed.
fn jump_off_steps(table: &ScoringTable, r: &Rider, steps: &mut Vec<Step>) {
    let n = r.jump_offs.len();
    let mut deepest: Option<(usize, Decimal)> = None;
    for level in 0..n {
        let block = n - 1 - level;
        let result = r.jump_offs[level];
        let out = result.is_none() && r.out_level == Some(level);
        steps.push(Step::JumpOff { block, result: result.map(|(f, _)| f), points: table.blocks[block].points, out });
        match result {
            Some((_, t)) => deepest = Some((block, t)),
            None if out => return,
            None => break,
        }
    }
    if let Some((block, t)) = deepest.filter(|(b, _)| !table.blocks[*b].time.is_empty()) {
        steps.push(Step::Time { block, value: t });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Finished,
    /// Rules 5 and 6: entitled to the jump-off / winning round, did not complete it.
    EqualLastOfJumpOff,
    /// Rule 7: eliminated or retired in round 2 (phase 2).
    EqualLastOfRound,
    /// Removed from the ranking.
    Out,
}

#[derive(Debug, Clone)]
struct Rider<'a> {
    entry: &'a Entry,
    /// Rule 1: first complete block, and its totals (what Rosson stored as faults / time).
    deciding_block: usize,
    deciding_totals: (Decimal, Decimal),
    /// The first complete block that is not a jump-off: the rider's standing in the rounds.
    round_block: usize,
    round_score: Decimal,
    round_time: Decimal,
    /// Jump-off results, first jump-off first (`None` = not completed).
    jump_offs: Vec<Option<(Decimal, Decimal)>>,
    /// Rules 5 and 6: entitled to the jump-off or winning round.
    jump_off_group: bool,
    kind: Kind,
    /// Rule 5: out in this jump-off (level, the first jump-off 0): equal last of it.
    out_level: Option<usize>,
}

/// Rule 1, plus what rule 5 needs.
fn evaluate<'a>(table: &ScoringTable, entry: &'a Entry, jump_offs: usize) -> Option<Rider<'a>> {
    // Rule 11: a points-based part does not need the time (missing: slowest).
    let complete = |b: &Block| b.required.iter().all(|f| entry.values.contains_key(f) || (b.points && b.time.contains(f)));
    let deciding_block = table.blocks.iter().position(complete)?;
    let round_block = (jump_offs..table.blocks.len()).find(|&i| complete(&table.blocks[i])).unwrap_or(jump_offs.min(table.blocks.len() - 1));
    let round = &table.blocks[round_block];
    let (round_faults, round_time) = block_totals(round, entry);
    // In a round without faults (table C) the time is the score.
    let round_score = if round.faults.is_empty() { round_time } else { round_faults };
    let jump_off_results = (0..jump_offs)
        .map(|level| {
            let block = &table.blocks[jump_offs - 1 - level];
            complete(block).then(|| block_totals(block, entry))
        })
        .collect();
    Some(Rider {
        entry,
        deciding_block,
        deciding_totals: block_totals(&table.blocks[deciding_block], entry),
        round_block,
        round_score,
        round_time,
        jump_offs: jump_off_results,
        jump_off_group: false,
        kind: Kind::Finished,
        out_level: None,
    })
}

/// Rule 5: mark riders in score groups entitled to a jump-off.
fn mark_jump_off_groups(table: &ScoringTable, riders: &mut [Rider], jump_offs: usize, options: Options) {
    let points = table.blocks[jump_offs].points;
    let mut scores: Vec<Decimal> = riders.iter().filter(|r| r.round_block == jump_offs).map(|r| r.round_score).collect();
    scores.sort_by(|a, b| if points { b.cmp(a) } else { a.cmp(b) });
    let mut position = 1;
    let mut i = 0;
    while i < scores.len() {
        let score = scores[i];
        let size = scores[i..].iter().take_while(|s| **s == score).count();
        let entitled = size >= 2 && (position == 1 || (options.medal_jump_offs && position <= 3));
        if entitled {
            for r in riders.iter_mut().filter(|r| r.round_block == jump_offs && r.round_score == score) {
                r.jump_off_group = true;
            }
        }
        position += size;
        i += size;
    }
}

/// Rule 6: mark the placed riders of the rounds before the winning round.
fn mark_winning_round(table: &ScoringTable, riders: &mut [Rider], placed: usize) {
    let mut order: Vec<usize> = (0..riders.len()).collect();
    order.sort_by(|&i, &j| compare_rounds(table, &riders[i], &riders[j]));
    let ranks = competition_ranks(&order, |&i, &j| compare_rounds(table, &riders[i], &riders[j]) == Ordering::Equal);
    for (position, &i) in order.iter().enumerate() {
        if ranks[position] as usize <= placed {
            riders[i].jump_off_group = true;
        }
    }
}

/// Ranking in the rounds only (rules 2 and 3), ignoring any jump-off or winning round.
fn compare_rounds(table: &ScoringTable, a: &Rider, b: &Rider) -> Ordering {
    let block = a.round_block;
    if block != b.round_block {
        return block.cmp(&b.round_block);
    }
    let by_score = if table.blocks[block].points { b.round_score.cmp(&a.round_score) } else { a.round_score.cmp(&b.round_score) };
    let timed = !table.blocks[block].time.is_empty();
    by_score.then_with(|| if timed { slowest_if_zero(a.round_time).cmp(&slowest_if_zero(b.round_time)) } else { Ordering::Equal })
}

/// Rule 7: block `n` is a second round (round 2 of a two-round class, phase 2 of a normal
/// two-phase class) and the block after it is the round before.
fn is_second_round(table: &ScoringTable, n: usize) -> bool {
    if table.phases == Phases::Normal {
        return n == 0;
    }
    let second = |b: &Block| b.required.iter().any(|f| f.ends_with("initial_2"));
    n + 1 < table.blocks.len() && second(&table.blocks[n]) && !second(&table.blocks[n + 1])
}

fn is_jump_off(block: &Block) -> bool {
    block.required.iter().any(|f| is_jump_off_field(f))
}

/// Faults (or points) and time of one block, added up.
pub(crate) fn block_totals(block: &Block, entry: &Entry) -> (Decimal, Decimal) {
    let sum = |fields: &[String]| fields.iter().filter_map(|f| entry.values.get(f)).copied().sum::<Decimal>();
    // Rule 12: times to the hundredth, finer digits cut.
    let times = block.time.iter().filter_map(|f| entry.values.get(f)).map(|t| hundredths(*t)).sum::<Decimal>();
    (sum(&block.faults), times)
}

/// A time to the hundredth, finer digits cut (not rounded): 61.239 → 61.23.
pub fn hundredths(time: Decimal) -> Decimal {
    time.round_dp_with_strategy(2, rust_decimal::RoundingStrategy::ToZero)
}

/// The whole ranking order (rules 2–7). Equal means tied.
fn compare(table: &ScoringTable, a: &Rider, b: &Rider, ideal: Option<Decimal>) -> Ordering {
    let by_score = |block: usize, x: Decimal, y: Decimal| if table.blocks[block].points { y.cmp(&x) } else { x.cmp(&y) };

    // Rule 2: the block a rider stands in.
    let (sa, sb) = (standing_block(table, a), standing_block(table, b));
    if sa != sb {
        return sa.cmp(&sb);
    }
    let block = sa;

    // Rule 7: equal last of round 2 after everyone who completed round 2.
    let last_of_round = |r: &Rider| r.kind == Kind::EqualLastOfRound;
    if last_of_round(a) || last_of_round(b) {
        return last_of_round(a).cmp(&last_of_round(b));
    }

    // Rule 6: among the riders entitled to the winning round, the winning round decides.
    if table.winning_round && block == 0 {
        return compare_jump_offs(table, a, b);
    }

    // Rule 5: in the rounds before a jump-off, by score group; within a group, the jump-off.
    let jump_offs = a.jump_offs.len();
    if jump_offs > 0 && !table.winning_round && block == jump_offs {
        let o = by_score(block, a.round_score, b.round_score);
        if o != Ordering::Equal {
            return o;
        }
        let rode = |r: &Rider| r.jump_offs.iter().any(Option::is_some);
        let in_it = |r: &Rider| r.jump_off_group || rode(r) || r.kind == Kind::EqualLastOfJumpOff;
        if in_it(a) || in_it(b) {
            return compare_jump_offs(table, a, b);
        }
    }

    // Rule 3; an ideal-time block: the gap to the ideal time, over or under the same (S20).
    let (fa, ta) = totals_in(table, a, block);
    let (fb, tb) = totals_in(table, b, block);
    let timed = !table.blocks[block].time.is_empty();
    by_score(block, fa, fb).then_with(|| match ideal.filter(|_| table.blocks[block].ideal_time) {
        Some(ideal) => (ta - ideal).abs().cmp(&(tb - ideal).abs()),
        None if timed => slowest_if_zero(ta).cmp(&slowest_if_zero(tb)),
        None => Ordering::Equal,
    })
}

/// Rule 2: the block a rider stands in. With jump-offs (rule 5) that is the rounds before the
/// jump-off; riders entitled to a winning round stand in it (rule 6); equal last of round 2
/// stands in round 2 (rule 7).
fn standing_block(table: &ScoringTable, r: &Rider) -> usize {
    if r.kind == Kind::EqualLastOfRound {
        r.round_block - 1
    } else if table.winning_round && r.jump_off_group {
        0
    } else {
        r.round_block
    }
}

/// Faults (or points) and time of a rider in the block they stand in.
fn totals_in(table: &ScoringTable, r: &Rider, block: usize) -> (Decimal, Decimal) {
    if block == r.round_block {
        let faults = if table.blocks[block].faults.is_empty() { Decimal::ZERO } else { r.round_score };
        (faults, r.round_time)
    } else {
        r.deciding_totals
    }
}

/// Rule 5 within a score group: jump-off by jump-off, faults first; whoever completed a jump-off
/// the other did not is ahead; out of a jump-off is equal last of it, after those who did not
/// ride it yet; the time of the last jump-off both completed decides; riders who rode none (a
/// jump-off cancelled) are tied.
fn compare_jump_offs(table: &ScoringTable, a: &Rider, b: &Rider) -> Ordering {
    let jump_offs = a.jump_offs.len();
    let mut last_common: Option<(Decimal, Decimal, usize)> = None;
    for level in 0..jump_offs {
        let block = jump_offs - 1 - level;
        match (a.jump_offs[level], b.jump_offs[level]) {
            (Some((fa, ta)), Some((fb, tb))) => {
                let o = if table.blocks[block].points { fb.cmp(&fa) } else { fa.cmp(&fb) };
                if o != Ordering::Equal {
                    return o;
                }
                last_common = Some((ta, tb, block));
            }
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            (None, None) => match (a.out_level == Some(level), b.out_level == Some(level)) {
                (true, true) => return Ordering::Equal,
                (true, false) => return Ordering::Greater,
                (false, true) => return Ordering::Less,
                (false, false) => break,
            },
        }
    }
    match last_common {
        Some((ta, tb, block)) if !table.blocks[block].time.is_empty() => slowest_if_zero(ta).cmp(&slowest_if_zero(tb)),
        _ => Ordering::Equal,
    }
}

/// Rule 5: a jump-off everyone of whom is out of it has no winner — they are all equal *last* of
/// it: the rank of its last place (five riders out: all 5th; Springreglement 2.3.9; S30).
fn nobody_wins_a_jump_off_all_left(riders: &[Rider], whys: &[Why], ranks: &mut [u32]) {
    // The steps before a rider's "out of this jump-off": who rode that jump-off with them.
    let contest = |w: &Why| -> Option<usize> { w.steps.iter().position(|s| matches!(s, Step::JumpOff { result: None, out: true, .. })) };
    let same = |a: &Why, b: &Why, upto: usize| b.steps.len() > upto && a.steps[..upto].iter().zip(&b.steps[..upto]).all(|(x, y)| x.order() == y.order());
    let mut i = 0;
    while i < riders.len() {
        let Some(at) = contest(&whys[i]) else {
            i += 1;
            continue;
        };
        // The tied riders out of the same jump-off.
        let end = (i..riders.len()).take_while(|&k| ranks[k] == ranks[i]).last().map_or(i + 1, |k| k + 1);
        // Nobody of that jump-off ahead of them: nobody completed it.
        let first = i == 0 || !same(&whys[i], &whys[i - 1], at);
        if first {
            let last = ranks[i] + (end - i) as u32 - 1;
            for r in ranks.iter_mut().take(end).skip(i) {
                *r = last;
            }
        }
        i = end;
    }
}

/// Knock-out (table 55, rule 14): duels, round after round. The rider who reached the furthest
/// round ranks first; in the final the better result (Wertung C: the time with 3 s per fault)
/// wins, a rider out of it loses it; the losers of the same round are equal. Out in a round:
/// lost that round's duel.
fn rank_knock_out(table: &ScoringTable, entries: &[Entry]) -> Vec<(Ranked, Why)> {
    let n = table.blocks.len();
    // Rounds in riding order: the last block is round 1.
    let value = |e: &Entry, round: usize| table.blocks[n - 1 - round].faults.iter().chain(&table.blocks[n - 1 - round].time).find_map(|f| e.values.get(f)).copied();
    let reached = |e: &Entry| {
        let done = (0..n).take_while(|&r| value(e, r).is_some()).count();
        (done + usize::from(e.out.is_some())).min(n)
    };
    let riders: Vec<(&Entry, usize)> = entries.iter().map(|e| (e, reached(e))).filter(|(_, r)| *r > 0).collect();
    let furthest = riders.iter().map(|(_, r)| *r).max().unwrap_or(0);
    let why = |e: &Entry, r: usize| {
        let block = n - r;
        let mut steps = vec![Step::Reached { block }];
        if r == furthest {
            steps.push(Step::Time { block, value: value(e, r - 1).map(hundredths).unwrap_or_default() });
        }
        Why { steps, standing: Standing::Finished, out: e.out }
    };
    let whys: Vec<Why> = riders.iter().map(|(e, r)| why(e, *r)).collect();
    let mut order: Vec<usize> = (0..riders.len()).collect();
    order.sort_by(|&a, &b| whys[a].cmp_steps(&whys[b]).then_with(|| compare_names(&riders[a].0.horse, &riders[b].0.horse)));
    let ranks = competition_ranks(&order, |&a, &b| whys[a].cmp_steps(&whys[b]) == Ordering::Equal);
    let listed: Vec<(Ranked, Why)> = order
        .iter()
        .zip(ranks)
        .map(|(&i, rank)| {
            let (e, r) = riders[i];
            let ranked = Ranked { id: e.id.clone(), placement: Placement::Rank(rank), block: Some(n - r), faults: None, time: value(e, r - 1), placeable: true };
            (ranked, whys[i].clone())
        })
        .collect();
    listed
}

fn slowest_if_zero(time: Decimal) -> Decimal {
    if time.is_zero() { Decimal::MAX } else { time }
}
