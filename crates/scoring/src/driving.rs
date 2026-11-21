//! Driving (Fahren, attelage): the phases of a driving class, as Rosson scores them
//! (`pruefungsdurchfuehrung.service.js`; `docs/rosson/driving.md`), with Rosson's bugs left out
//! (`docs/deviations.md` S25).
//!
//! - Dressage: each judge's total `160 − (points − penalties) × 160 / (movements × 10)` — the
//!   programme errors taken off the marks before the conversion to 160 (Fahrreglement 13.10 §3;
//!   S43; Rosson added them after it); the class's the judges' mean; lower is better.
//! - Marathon: phases A and B against their allowed and minimum times, B's obstacles' time × the
//!   factor and their penalties; lower is better.
//! - Cones: per round, time faults (A: half a point per second started over the allowed time, or
//!   half the overtime to the hundredth "internationally"; B: the time itself; JC: half a point
//!   per second started outside ±5 s around the allowed time) plus obstacle and other faults;
//!   lower is better, then (JC) the gap to the allowed time, else the time.
//! - Derby: the driven time and penalty seconds, each × the time factor; lower is better.
//! - Open: by time, points, penalty points, or points then time.
//! - Multitest: each judge's marks × coefficients less penalties A, B, C; the judges' mean;
//!   higher is better.

use crate::number::{round, Decimal};
use crate::ranking::{competition_ranks, order_ties_by_horse, Placement, Reason};

const TWO: Decimal = Decimal::TWO;

/// Driving dressage: one judge's total from the sum of their marks (coefficients are 1 in
/// driving), the number of marked movements and the penalty points, taken off the marks before
/// the conversion to 160 (13.10 §3; S43).
pub fn dressage_judge_total(points: Decimal, movements: usize, penalties: Decimal) -> Option<Decimal> {
    let max = Decimal::from(movements as u64) * Decimal::TEN;
    (!max.is_zero()).then(|| round(Decimal::from(160) - (points - penalties) * Decimal::from(160) / max, 2))
}

/// Driving dressage: penalty points on a judge's sheet in points of the 160 basis (a judge who
/// gave the total: the penalties converted like the marks, S43).
pub fn dressage_penalties_on_160(penalties: Decimal, movements: usize) -> Decimal {
    let max = Decimal::from(movements as u64) * Decimal::TEN;
    if max.is_zero() { penalties } else { penalties * Decimal::from(160) / max }
}

/// The judges' mean, to the hundredth.
pub fn mean(totals: &[Decimal]) -> Option<Decimal> {
    (!totals.is_empty()).then(|| round(totals.iter().copied().sum::<Decimal>() / Decimal::from(totals.len() as u64), 2))
}

/// A marathon phase's settings (seconds).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MarathonTimes {
    pub allowed: Option<Decimal>,
    pub minimum: Option<Decimal>,
}

/// One marathon phase as driven: the time between start and finish, its penalties and the
/// neutralised seconds.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MarathonRun {
    pub time: Option<Decimal>,
    pub penalties: Decimal,
    pub neutralised: Decimal,
}

/// An obstacle of phase B: its time, penalties and neutralised seconds.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MarathonObstacle {
    pub time: Decimal,
    pub penalties: Decimal,
    pub neutralised: Decimal,
}

/// The marathon's penalties, broken down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarathonScore {
    pub phase_a: Decimal,
    pub phase_b: Decimal,
    pub obstacles: Decimal,
    pub total: Decimal,
}

/// Time penalties of a marathon phase: `factor` per second started under the minimum or over
/// the allowed time.
fn marathon_time(t: Decimal, times: MarathonTimes, factor: Decimal) -> Decimal {
    let under = times.minimum.filter(|m| t < *m).map(|m| (m - t).ceil() * factor).unwrap_or_default();
    let over = times.allowed.filter(|a| t > *a).map(|a| (t - a).ceil() * factor).unwrap_or_default();
    under + over
}

/// Score a marathon (`calcGelaendeResultat`; times over an hour counted right).
pub fn marathon(a: MarathonRun, b: MarathonRun, obstacles: &[MarathonObstacle], times_a: MarathonTimes, times_b: MarathonTimes, factor: Decimal) -> MarathonScore {
    let obstacle_time: Decimal = obstacles.iter().map(|o| o.time).sum();
    let obstacle_points = round(obstacle_time * factor, 2) + obstacles.iter().map(|o| o.penalties).sum::<Decimal>();
    let neutralised_b: Decimal = b.neutralised + obstacles.iter().map(|o| o.neutralised).sum::<Decimal>();
    let phase = |run: MarathonRun, neutralised: Decimal, times: MarathonTimes| run.penalties + run.time.map(|t| marathon_time(t - neutralised, times, factor)).unwrap_or_default();
    let phase_a = phase(a, a.neutralised, times_a);
    let phase_b = phase(b, neutralised_b, times_b);
    MarathonScore { phase_a, phase_b, obstacles: obstacle_points, total: round(phase_a + phase_b + obstacle_points, 2) }
}

/// Seconds a driver may spend in a marathon obstacle; over them, eliminated (rules).
pub const OBSTACLE_LIMIT_SECONDS: u32 = 300;

/// The marathon's time limits (the rules: phase A the allowed time + 20 %, phase B twice the
/// allowed time, five minutes in an obstacle): over any of them, eliminated. A phase's time is
/// counted as for its penalties, without the neutralised seconds. Rosson does not apply them.
pub fn marathon_over_limit(a: MarathonRun, b: MarathonRun, obstacles: &[MarathonObstacle], limit_a: Option<Decimal>, limit_b: Option<Decimal>) -> bool {
    let neutralised_b: Decimal = b.neutralised + obstacles.iter().map(|o| o.neutralised).sum::<Decimal>();
    let over = |run: MarathonRun, neutralised: Decimal, limit: Option<Decimal>| run.time.zip(limit).is_some_and(|(t, l)| t - neutralised > l);
    over(a, a.neutralised, limit_a) || over(b, neutralised_b, limit_b) || obstacles.iter().any(|o| o.time > Decimal::from(OBSTACLE_LIMIT_SECONDS))
}

/// A cones scoring (Rosson's driving scoring tables: A, A2U, B, B2U, JC2U).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConesScoring {
    /// Points: time faults over the allowed time.
    A,
    /// Time: the driven time added to the faults.
    B,
    /// Youth cup: a window of ±5 s around the allowed time (an ideal time).
    JuniorCup,
}

/// The window of the youth cup, seconds each side.
pub const JC_WINDOW: u32 = 5;

/// One round of cones as driven.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ConesRound {
    pub time: Option<Decimal>,
    pub obstacle_faults: Decimal,
    pub other_faults: Decimal,
}

/// A round's time faults, total and gap to the allowed time (`calcHindernisResultat`).
pub fn cones_round(scoring: ConesScoring, round_: ConesRound, allowed: Decimal, international: bool) -> (Decimal, Decimal, Decimal) {
    let t = round_.time.unwrap_or_default();
    let (time_faults, gap) = match scoring {
        ConesScoring::A => {
            let over = t - allowed;
            let faults = if over <= Decimal::ZERO {
                Decimal::ZERO
            } else if international {
                round(round(over, 2) / TWO, 2)
            } else {
                over.ceil() / TWO
            };
            (faults, Decimal::ZERO)
        }
        ConesScoring::B => (t, Decimal::ZERO),
        ConesScoring::JuniorCup => {
            let window = Decimal::from(JC_WINDOW);
            let gap = (t - allowed).abs();
            let faults = if gap > window { (gap - window).ceil() / TWO } else { Decimal::ZERO };
            (faults, gap)
        }
    };
    (time_faults, time_faults + round_.obstacle_faults + round_.other_faults, gap)
}

/// Derby: the driven time and penalty seconds (knock-downs, others), each × the time factor.
pub fn derby(time: Decimal, knock_downs: Decimal, others: Decimal, factor: Decimal) -> Decimal {
    round(round(time * factor, 2) + round(knock_downs * factor, 2) + round(others * factor, 2), 2)
}

/// Multitest: one judge's total from the marks × coefficients, less penalties A, B and C.
pub fn multitest_judge_total(marks: &[(Decimal, Decimal)], penalties: [Decimal; 3]) -> Decimal {
    round(marks.iter().map(|(mark, coefficient)| mark * coefficient).sum(), 2) - penalties.iter().copied().sum::<Decimal>()
}

/// Rosson's multitest programmes (`programme.service.js` 116–158): coefficients of items 1… .
pub fn multitest_coefficients(kind: &str) -> Vec<Decimal> {
    let c = |v: &[i64]| v.iter().map(|x| Decimal::from(*x)).collect();
    match kind {
        "B1" => c(&[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 3, 3, 3]),
        _ => c(&[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 2, 2, 2]),
    }
}

/// Team cones (the OKV Fahrcup, "Equipe Kegelfahren"): a round of one of the team's drivers,
/// its points and time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamRound {
    pub points: Decimal,
    pub time: Decimal,
}

/// A team of team cones with every round of its drivers (credited rounds included).
#[derive(Debug, Clone, PartialEq)]
pub struct ConesTeam {
    pub id: String,
    pub name: String,
    pub rounds: Vec<TeamRound>,
    /// Every round of every driver is in (else not ranked yet).
    pub complete: bool,
    pub out: bool,
}

/// A team's result: the counted rounds' points and time, the round struck.
#[derive(Debug, Clone, PartialEq)]
pub struct ConesTeamScore {
    pub id: String,
    pub total: Decimal,
    pub time: Decimal,
    pub struck: Option<TeamRound>,
}

/// The OKV Fahrcup's rank points: 12, 10, 9 … 2 for the ten first, 1 for every other team that
/// started, eliminated ones included (Reglement Fahrcup 4.3).
pub fn team_rank_points(placement: Placement) -> u32 {
    match placement {
        Placement::Rank(1) => 12,
        Placement::Rank(r) if r <= 10 => 12 - r,
        _ => 1,
    }
}

/// A round a driver could not complete in team cones: 125 % of the last placed in the first
/// round (Reglement Fahrcup 4.2), to the hundredth.
pub fn credited_round(worst_first_round: Decimal) -> Decimal {
    round(worst_first_round * Decimal::new(125, 2), 2)
}

/// A team's score: all rounds but the worst (the most points; equal points, the slower), their
/// points and time added (Reglement Fahrcup 4.2: "Gewertet werden 5 der 6 gefahrenen Umgänge").
pub fn team_score(team: &ConesTeam) -> ConesTeamScore {
    let struck = struck_index(&team.rounds);
    let counted = team.rounds.iter().enumerate().filter(|(i, _)| Some(*i) != struck).map(|(_, r)| r);
    let (total, time) = counted.fold((Decimal::ZERO, Decimal::ZERO), |(p, t), r| (p + r.points, t + r.time));
    ConesTeamScore { id: team.id.clone(), total: round(total, 2), time: round(time, 2), struck: struck.map(|i| team.rounds[i]) }
}

/// Which of a team's rounds is struck: the most points, then the slower; equal in both, the
/// last of them. None with a single round.
pub fn struck_index(rounds: &[TeamRound]) -> Option<usize> {
    if rounds.len() < 2 {
        return None;
    }
    (0..rounds.len()).max_by(|a, b| (rounds[*a].points, rounds[*a].time).cmp(&(rounds[*b].points, rounds[*b].time)))
}

/// Rank team cones (Reglement Fahrcup 4.2): the fewest points; teams equal on points for the
/// win share it (a jump-off decides: ranks set by hand); further down, the better struck round,
/// then the counted rounds' time; equal in all, the same rank, listed by name. Teams out after
/// the others; teams not complete left out.
pub fn rank_teams(teams: &[ConesTeam]) -> Vec<(ConesTeamScore, Placement)> {
    let mut scored: Vec<(&ConesTeam, ConesTeamScore)> = teams.iter().filter(|t| t.complete && !t.out).map(|t| (t, team_score(t))).collect();
    let best = scored.iter().map(|(_, s)| s.total).min();
    let key = |s: &ConesTeamScore| -> (Decimal, Decimal, Decimal) {
        // Equal first on points: nothing further decides (the jump-off does).
        if Some(s.total) == best {
            (s.total, Decimal::ZERO, Decimal::ZERO)
        } else {
            (s.total, s.struck.map(|r| r.points).unwrap_or_default(), s.time)
        }
    };
    scored.sort_by(|a, b| key(&a.1).cmp(&key(&b.1)).then_with(|| a.0.name.cmp(&b.0.name)));
    let ranks = competition_ranks(&scored, |a, b| key(&a.1) == key(&b.1));
    let mut out: Vec<(ConesTeamScore, Placement)> = scored.into_iter().zip(ranks).map(|((_, s), r)| (s, Placement::Rank(r))).collect();
    let mut gone: Vec<&ConesTeam> = teams.iter().filter(|t| t.out).collect();
    gone.sort_by(|a, b| a.name.cmp(&b.name));
    out.extend(gone.into_iter().map(|t| (team_score(t), Placement::Out)));
    out
}

/// Which way a driving phase ranks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Lowest first (penalties, time).
    Lowest,
    /// Highest first (points).
    Highest,
}

/// Why a rider stands where they do in a driving phase: what [`rank`] compares, in order — the
/// phase's result, then its tie-breaks.
pub fn reasons(s: &Scored, order: Order) -> Vec<Reason> {
    let mut out: Vec<Reason> = s.value.map(|v| Reason { what: "result", value: v, higher_first: order == Order::Highest }).into_iter().collect();
    out.extend(s.ties.iter().map(|(what, v)| Reason { what, value: *v, higher_first: false }));
    out
}

/// A rider's score in a driving phase, ready to rank: the value, then tie-breaks in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    pub id: String,
    pub horse: String,
    pub value: Option<Decimal>,
    /// Compared after the value (lowest first), each with what it is: e.g. the gap to the
    /// allowed time, the time.
    pub ties: Vec<(&'static str, Decimal)>,
    pub out: bool,
}

/// Rank a driving phase: by value (`order`), then the tie-breaks (lowest first); equal in all,
/// the same rank, listed by horse; out after the others; no value and not out: not ranked.
pub fn rank(scores: &[Scored], order: Order) -> Vec<(String, Placement)> {
    // What [`reasons`] says, in the same order.
    let key = |s: &Scored| {
        let v = s.value.unwrap_or_default();
        (if order == Order::Highest { -v } else { v }, s.ties.iter().map(|(_, t)| *t).collect::<Vec<_>>())
    };
    let mut ranked: Vec<&Scored> = scores.iter().filter(|s| !s.out && s.value.is_some()).collect();
    ranked.sort_by_key(|s| key(s));
    let ranks = competition_ranks(&ranked, |a, b| key(a) == key(b));
    let mut listed: Vec<(u32, &Scored)> = ranks.into_iter().zip(ranked).collect();
    order_ties_by_horse(&mut listed, |e| e.0, |e| e.1.horse.as_str());
    let mut out: Vec<(String, Placement)> = listed.into_iter().map(|(r, s)| (s.id.clone(), Placement::Rank(r))).collect();
    let mut gone: Vec<&Scored> = scores.iter().filter(|s| s.out).collect();
    gone.sort_by(|a, b| a.horse.cmp(&b.horse));
    out.extend(gone.into_iter().map(|s| (s.id.clone(), Placement::Out)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    #[test]
    fn dressage_on_160() {
        // 12 movements, marks adding to 84 (of 120), 5 penalty points taken off the marks before
        // the conversion: 160 − 79 × 160 / 120 = 54.67 (Rosson: 160 − 112 + 5 = 53).
        assert_eq!(dressage_judge_total(d("84"), 12, d("5")), Some(d("54.67")));
        // On 160 points (16 movements) it makes no difference: 160 − 105 = 55.
        assert_eq!(dressage_judge_total(d("110"), 16, d("5")), Some(d("55.00")));
        assert_eq!(mean(&[d("53"), d("50")]), Some(d("51.50")));
    }

    #[test]
    fn marathon_phases_and_obstacles() {
        let times_a = MarathonTimes { allowed: Some(d("1800")), minimum: Some(d("1680")) };
        let times_b = MarathonTimes { allowed: Some(d("3000")), minimum: Some(d("2820")) };
        // A: 1810.5 s driven → 11 s started over × 0.25 = 2.75; B: 3790 s, 30 neutralised in an
        // obstacle and 3720 in all: 1 h 2 min — 720 s over → 180 (Rosson's hours × 360 would not).
        let a = MarathonRun { time: Some(d("1810.5")), penalties: Decimal::ZERO, neutralised: Decimal::ZERO };
        let b = MarathonRun { time: Some(d("3750")), penalties: d("2"), neutralised: Decimal::ZERO };
        let obstacles = [MarathonObstacle { time: d("95.3"), penalties: d("5"), neutralised: d("30") }, MarathonObstacle { time: d("88.1"), penalties: Decimal::ZERO, neutralised: Decimal::ZERO }];
        let s = marathon(a, b, &obstacles, times_a, times_b, d("0.25"));
        // Obstacles: 183.4 × 0.25 = 45.85 + 5 = 50.85; B: 3750 − 30 = 3720 → 720 × 0.25 + 2 = 182.
        assert_eq!((s.phase_a, s.obstacles, s.phase_b), (d("2.75"), d("50.85"), d("182.00")));
        assert_eq!(s.total, d("235.60"));
        // Too fast in A: 20 s under the minimum → 5.
        let fast = marathon(MarathonRun { time: Some(d("1660")), ..a }, MarathonRun::default(), &[], times_a, times_b, d("0.25"));
        assert_eq!(fast.phase_a, d("5.00"));
    }

    #[test]
    fn cones_scorings() {
        let r = |t: &str, ob: &str| ConesRound { time: Some(d(t)), obstacle_faults: d(ob), other_faults: Decimal::ZERO };
        // A: 4.2 s over → 5 s started / 2 = 2.5, + 3 = 5.5; internationally 2.1 + 3 = 5.1.
        assert_eq!(cones_round(ConesScoring::A, r("124.2", "3"), d("120"), false).1, d("5.5"));
        assert_eq!(cones_round(ConesScoring::A, r("124.2", "3"), d("120"), true).1, d("5.10"));
        // B: the time itself.
        assert_eq!(cones_round(ConesScoring::B, r("124.2", "3"), d("120"), false).1, d("127.2"));
        // JC: inside ±5 s nothing; 8.4 s early: 3.4 past the window → 4 / 2 = 2; the gap 8.4.
        assert_eq!(cones_round(ConesScoring::JuniorCup, r("116", "0"), d("120"), false), (Decimal::ZERO, Decimal::ZERO, d("4")));
        assert_eq!(cones_round(ConesScoring::JuniorCup, r("111.6", "0"), d("120"), false), (d("2"), d("2"), d("8.4")));
    }

    #[test]
    fn derby_multitest_and_ranking() {
        assert_eq!(derby(d("180.2"), d("10"), d("5"), d("0.25")), d("48.80"));
        let b4 = multitest_coefficients("B4");
        let marks: Vec<(Decimal, Decimal)> = b4.iter().map(|c| (d("7"), *c)).collect();
        // 7 × 22 = 154 less 2 + 1 = 151.
        assert_eq!(multitest_judge_total(&marks, [d("2"), d("1"), Decimal::ZERO]), d("151"));
        let s = |id: &str, v: &str, tie: &str| Scored { id: id.into(), horse: id.to_uppercase(), value: Some(d(v)), ties: vec![("time", d(tie))], out: false };
        assert_eq!(rank(&[s("a", "5", "120"), s("b", "5", "118"), s("c", "2", "130")], Order::Lowest), vec![("c".into(), Placement::Rank(1)), ("b".into(), Placement::Rank(2)), ("a".into(), Placement::Rank(3))]);
        assert_eq!(rank(&[s("a", "151", "0"), s("b", "160", "0")], Order::Highest)[0].0, "b");
    }

    fn team(id: &str, rounds: &[(&str, &str)]) -> ConesTeam {
        ConesTeam { id: id.into(), name: id.to_uppercase(), rounds: rounds.iter().map(|(p, t)| TeamRound { points: d(p), time: d(t) }).collect(), complete: true, out: false }
    }

    #[test]
    fn team_cones_five_of_six_rounds() {
        // The worst round (6 points) struck: 0 + 3 + 1.5 + 0 + 2 = 6.5, the time of the five.
        let s = team_score(&team("a", &[("0", "100"), ("3", "110"), ("1.5", "105"), ("6", "120"), ("0", "98"), ("2", "101")]));
        assert_eq!((s.total, s.time, s.struck.map(|r| r.points)), (d("6.5"), d("514"), Some(d("6"))));
        // Equal worst points: the slower struck.
        let s = team_score(&team("b", &[("3", "100"), ("3", "110"), ("0", "100")]));
        assert_eq!(s.struck, Some(TeamRound { points: d("3"), time: d("110") }));
    }

    #[test]
    fn team_cones_ties() {
        // a and b equal first (6.5): shared, a jump-off decides. c and d equal on 8: c's struck
        // round is better; e and f equal on points and struck round: the faster five.
        let teams = vec![
            team("a", &[("0", "100"), ("6.5", "100"), ("9", "100")]),
            team("b", &[("3", "100"), ("3.5", "100"), ("4", "100")]),
            team("c", &[("4", "100"), ("4", "100"), ("5", "100")]),
            team("d", &[("4", "100"), ("4", "100"), ("7", "100")]),
            team("e", &[("5", "100"), ("5", "101"), ("10", "100")]),
            team("f", &[("5", "100"), ("5", "100"), ("10", "100")]),
            team("g", &[("5", "100"), ("5", "100"), ("10", "100")]),
        ];
        let ranked: Vec<(String, Placement)> = rank_teams(&teams).into_iter().map(|(s, p)| (s.id, p)).collect();
        let r = |n| Placement::Rank(n);
        assert_eq!(ranked, vec![("a".into(), r(1)), ("b".into(), r(1)), ("c".into(), r(3)), ("d".into(), r(4)), ("f".into(), r(5)), ("g".into(), r(5)), ("e".into(), r(7))]);
        assert_eq!([1, 2, 3, 10, 11].map(|n| team_rank_points(r(n))), [12, 10, 9, 2, 1]);
        assert_eq!(team_rank_points(Placement::Out), 1);
        assert_eq!(credited_round(d("13.5")), d("16.88"));
    }
}
