//! Dressage: marks per judge → total per judge → percentage → ranking.
//!
//! # The rule
//!
//! For each judge:
//! 1. every movement's mark (including collective marks) times its coefficient, added up;
//! 2. minus percentage penalties: that percentage of the points, rounded to 0.1;
//! 3. minus point penalties (e.g. 2 points for the first error of course);
//! 4. rounded to 2 decimals: the judge's total.
//!
//! The entry's percentage is the judges' totals added up, divided by the maximum (the
//! programme's maximum points times the number of judges of the class), times 100, rounded to
//! 2 decimals. A judge who has not marked the entry yet counts as 0.
//!
//! **Tag der Jugend finals** (regional and Swiss, a class option): no ex aequo in ranks 1–3. Tied
//! entries there are separated by the sum of judges C and H's marks for the programme's given
//! movements, in their order (Führzügelklasse 2, 3; Einfacher Reitwettbewerb 6, 3;
//! Einsteigerdressur 15, 17; Dressuraufgabe 21, 24), then by drawing lots: the office sets those
//! ranks by hand (RL Tag der Jugend 10.3; S38).
//!
//! Ranking: on the points, as the regulation ranks (RL Prüfungen 5; S3, decided 2026-11-17):
//! compared as the percentage to 4 decimals — within one programme the same order as the points,
//! and comparable when riders ride different programmes. The percentage is shown to 2 decimals;
//! two entries can show the same and rank apart. Equal entries rank equal and are listed
//! alphabetically by horse name (S4). Eliminated or retired entries are out.
//!
//! Other programmes and ways of marking, as Rosson computes them (no corpus show has them: from
//! its code, `calcDressurResultat`, `getMaxPunkteDressur`):
//! - **Freestyle (Kür)**: technical (A) and artistic (B) part marks; a judge's total is half
//!   the points (after penalties); the part percentages are each part's points over its
//!   maximum (10 × coefficients) times the judges.
//! - **Children**: one sheet for the entry (judge C marks part A, the two E judges part B,
//!   together one result); the percentage is its total over the programme's maximum (not
//!   times the judges); parts A and B each over their maximum.
//! - **Totals entered per judge** (a class option): the judge's total as entered, less the
//!   ticked penalties; no part marks.
//!
//! Source in Rosson: `PruefungsdurchfuehrungService.calcDressurResultat` and
//! `RanglisteService.updateRanglisteDressur`. Golden tests: every dressage class in
//! `golden/corpus/`. Not covered here: team dressage and driving dressage (their own rules).

use std::collections::BTreeMap;

use crate::number::{round, Decimal};
use crate::ranking::{competition_ranks, order_ties_by_horse, Placement, Reason};

/// A dressage test (Programm) as published: what is marked, with which coefficient.
#[derive(Debug, Clone, PartialEq)]
pub struct Programme {
    pub name: String,
    pub kind: ProgrammeKind,
    /// Scored movements and collective marks, by their number on the score sheet.
    pub movements: Vec<Movement>,
    /// Maximum points one judge can give, as published with the programme.
    pub max_points: Decimal,
    /// A Tag der Jugend final: how ties in ranks 1–3 are broken (S38).
    pub final_ties: Option<FinalTies>,
}

/// Tag der Jugend finals (RL Tag der Jugend 10.3): ties in ranks 1–3 are broken by the sum of
/// these judges' marks (C and H) for these movements, in order; then lots are drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct FinalTies {
    /// Movement numbers, the first decisive first.
    pub movements: Vec<String>,
    /// The judges whose marks are added (judges C and H of the class).
    pub judges: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgrammeKind {
    Standard,
    /// Kür: technical (A) and artistic (B) part marks; a judge's total is half the points.
    Freestyle,
    /// Children: one sheet, part A by judge C, part B by the E judges.
    Children,
}

/// The part a mark counts for (freestyle, Children).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    A,
    B,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Movement {
    pub number: String,
    pub coefficient: Decimal,
    pub part: Option<Part>,
}

/// A penalty a judge ticked on the score sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Penalty {
    /// Points taken off the judge's total.
    Points(Decimal),
    /// A percentage of the judge's points taken off.
    Percent(Decimal),
}

/// One judge's score sheet for one entry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JudgeSheet {
    /// Marks by movement number.
    pub marks: BTreeMap<String, Decimal>,
    pub penalties: Vec<Penalty>,
    /// The judge's total as entered (classes with totals per judge): the marks are not used.
    pub total: Option<Decimal>,
}

/// One entry (horse and rider) in a dressage class.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: String,
    /// Used to list tied entries (S4).
    pub horse: String,
    /// Eliminated or retired.
    pub out: bool,
    /// Score sheets by judge id; judges who have not marked the entry yet are missing.
    pub sheets: BTreeMap<String, JudgeSheet>,
}

/// The computed score of one entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub id: String,
    pub horse: String,
    pub out: bool,
    /// Total per judge (step 4 of the rule), for judges who marked the entry.
    pub judge_totals: BTreeMap<String, Decimal>,
    /// Each judge's total in percent of one sheet's maximum, to 2 decimals (the ranking with
    /// each judge's details, Rosson's `getRanglisteProRichter`).
    pub judge_percentages: BTreeMap<String, Decimal>,
    /// `None` when no judge has marked the entry yet, or when it is out.
    pub percentage: Option<Decimal>,
    /// The percentage to 4 decimals: what the ranking compares (the points, S3).
    pub exact: Option<Decimal>,
    /// The maximum the percentage is computed from: programme maximum × judges (Children: the
    /// programme's maximum).
    pub max_points: Decimal,
    /// Freestyle and Children: the technical (A) and artistic (B) parts, in percent.
    pub part_a: Option<Decimal>,
    pub part_b: Option<Decimal>,
    /// Freestyle and Children: each judge's parts (judges who marked the movements).
    pub judge_parts: BTreeMap<String, JudgeParts>,
    /// Equal percentages: the higher this, the better. Freestyle, the artistic part (Swiss
    /// Equestrian's Kür tests: "le participant ayant obtenu le meilleur résultat artistique est
    /// vainqueur"); OKV team dressage, judge E's total.
    pub tie_break: Option<Decimal>,
    /// A Tag der Jugend final: per tie movement, judges C and H's marks added (S38).
    pub final_marks: Vec<Decimal>,
}

/// One judge's parts A and B: the points (freestyle: halved; both rounded to 0.1, as Rosson) and
/// their percentages of the part's maximum (freestyle: halved too).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JudgeParts {
    pub a: Decimal,
    pub b: Decimal,
    pub percent_a: Option<Decimal>,
    pub percent_b: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The programme kind needs rules that are not implemented yet.
    NotCovered(String),
    /// A mark was given for a movement the programme does not have.
    UnknownMovement { judge: String, movement: String },
    /// The class has no judges, so no percentage can be computed.
    NoJudges,
}

/// A sheet's points before penalties, and its parts A and B (from the marks; an entered total
/// has no parts).
fn points(programme: &Programme, judge: &str, sheet: &JudgeSheet) -> Result<(Decimal, Decimal, Decimal), Error> {
    if let Some(total) = sheet.total {
        return Ok((total, Decimal::ZERO, Decimal::ZERO));
    }
    let movements: BTreeMap<&str, &Movement> = programme.movements.iter().map(|m| (m.number.as_str(), m)).collect();
    let (mut points, mut a, mut b) = (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
    for (movement, mark) in &sheet.marks {
        let m = movements.get(movement.as_str()).ok_or_else(|| Error::UnknownMovement { judge: judge.to_string(), movement: movement.clone() })?;
        let value = mark * m.coefficient;
        points += value;
        match m.part {
            Some(Part::A) => a += value,
            Some(Part::B) => b += value,
            None => {}
        }
    }
    Ok((points, a, b))
}

/// The points less the penalties: the percentage ones first, rounded to 0.1, then the points.
fn penalised(points: Decimal, penalties: &[Penalty]) -> Decimal {
    let percent: Decimal = penalties.iter().filter_map(|p| if let Penalty::Percent(x) = p { Some(*x) } else { None }).sum();
    let penalty_points: Decimal = penalties.iter().filter_map(|p| if let Penalty::Points(x) = p { Some(*x) } else { None }).sum();
    let mut points = points;
    if !percent.is_zero() {
        points -= round(points * percent / Decimal::ONE_HUNDRED, 1);
    }
    points - penalty_points
}

/// One judge's total for one entry (steps 1–4 of the rule; freestyle: half of it, unless the
/// judge entered the total).
pub fn judge_total(programme: &Programme, judge: &str, sheet: &JudgeSheet) -> Result<Decimal, Error> {
    let (points, _, _) = points(programme, judge, sheet)?;
    let total = round(penalised(points, &sheet.penalties), 2);
    Ok(if programme.kind == ProgrammeKind::Freestyle && sheet.total.is_none() { total / Decimal::TWO } else { total })
}

/// A part's maximum: 10 × the coefficients of its movements.
fn part_max(programme: &Programme, part: Part) -> Decimal {
    programme.movements.iter().filter(|m| m.part == Some(part)).map(|m| m.coefficient * Decimal::TEN).sum()
}

fn percent(points: Decimal, max: Decimal) -> Option<Decimal> {
    (!max.is_zero()).then(|| round(points / max * Decimal::ONE_HUNDRED, 2))
}

/// The percentage to 4 decimals: totals have 2 decimals, so up to a maximum of 10 000 points
/// two different totals never give the same value (S3).
fn exact_percent(points: Decimal, max: Decimal) -> Option<Decimal> {
    (!max.is_zero()).then(|| tidy(round(points / max * Decimal::ONE_HUNDRED, 4)))
}

/// Without trailing zeros beyond the hundredths: 75.00, 73.3333 (the same value, said plainly).
fn tidy(value: Decimal) -> Decimal {
    let mut v = value.normalize();
    if v.scale() < 2 {
        v.rescale(2);
    }
    v
}

/// Score one entry: every judge's total and the percentage.
pub fn score(programme: &Programme, judges: usize, entry: &Entry) -> Result<Score, Error> {
    if judges == 0 {
        return Err(Error::NoJudges);
    }
    let children = programme.kind == ProgrammeKind::Children;
    // Children: one result for the three judges, over the programme's maximum.
    let max_points = if children { programme.max_points } else { programme.max_points * Decimal::from(judges as u64) };

    let freestyle = programme.kind == ProgrammeKind::Freestyle;
    // A part's maximum on one judge's sheet (freestyle: halved, like the points).
    let (max_a, max_b) = (part_max(programme, Part::A), part_max(programme, Part::B));
    let (sheet_max_a, sheet_max_b) = if freestyle { (max_a / Decimal::TWO, max_b / Decimal::TWO) } else { (max_a, max_b) };

    let mut judge_totals = BTreeMap::new();
    let mut judge_parts = BTreeMap::new();
    for (judge, sheet) in &entry.sheets {
        judge_totals.insert(judge.clone(), judge_total(programme, judge, sheet)?);
        if programme.kind != ProgrammeKind::Standard && sheet.total.is_none() {
            let (_, pa, pb) = points(programme, judge, sheet)?;
            let (a, b) = if freestyle { (round(pa / Decimal::TWO, 1), round(pb / Decimal::TWO, 1)) } else { (round(pa, 1), round(pb, 1)) };
            judge_parts.insert(judge.clone(), JudgeParts { a, b, percent_a: percent(a, sheet_max_a), percent_b: percent(b, sheet_max_b) });
        }
    }
    let (percentage, exact, part_a, part_b) = if entry.out || judge_totals.is_empty() {
        (None, None, None, None)
    } else if children {
        // The first sheet is the entry's (Rosson copies it to the other judges).
        let judge = entry.sheets.keys().next().expect("a sheet").clone();
        let total = judge_totals[&judge];
        for t in judge_totals.values_mut() {
            *t = total;
        }
        let first = judge_parts.get(&judge).copied();
        for p in judge_parts.values_mut() {
            *p = first.expect("the first sheet's parts");
        }
        (percent(total, max_points), exact_percent(total, max_points), first.and_then(|p| p.percent_a), first.and_then(|p| p.percent_b))
    } else {
        let sum: Decimal = judge_totals.values().sum();
        // Freestyle: the judges' halved parts over the parts' maxima × judges / 2.
        let parts = freestyle && judge_parts.len() == entry.sheets.len();
        let n = Decimal::from(judges as u64);
        let (a, b): (Decimal, Decimal) = (judge_parts.values().map(|p| p.a).sum(), judge_parts.values().map(|p| p.b).sum());
        (percent(sum, max_points), exact_percent(sum, max_points), parts.then(|| percent(a, sheet_max_a * n)).flatten(), parts.then(|| percent(b, sheet_max_b * n)).flatten())
    };

    let judge_percentages = judge_totals.iter().filter_map(|(j, t)| Some((j.clone(), percent(*t, programme.max_points)?))).collect();
    let tie_break = if freestyle { part_b } else { None };
    // A Tag der Jugend final: judges C and H's marks for each tie movement, added.
    let final_marks = programme.final_ties.as_ref().map_or_else(Vec::new, |f| {
        f.movements.iter().map(|m| f.judges.iter().filter_map(|j| entry.sheets.get(j)?.marks.get(m)).sum::<Decimal>().normalize()).collect()
    });
    Ok(Score { id: entry.id.clone(), horse: entry.horse.clone(), out: entry.out, judge_totals, judge_percentages, percentage, exact, max_points, part_a, part_b, judge_parts, tie_break, final_marks })
}

/// Score one entry of an eventing dressage phase: the percentage is the average of each judge's
/// percentage, each to 2 decimals (a judge who has not marked: 0), as the Eventing-Reglement
/// computes it (8.1.2; E11). Standard programmes; others as [`score`].
pub fn score_by_judges_mean(programme: &Programme, judges: usize, entry: &Entry) -> Result<Score, Error> {
    let mut score = score(programme, judges, entry)?;
    if programme.kind != ProgrammeKind::Standard || score.percentage.is_none() || programme.max_points.is_zero() {
        return Ok(score);
    }
    let each: Decimal = score.judge_totals.values().map(|t| round(*t / programme.max_points * Decimal::ONE_HUNDRED, 2)).sum();
    let mean = each / Decimal::from(judges as u64);
    score.percentage = Some(round(mean, 2));
    score.exact = Some(tidy(round(mean, 4)));
    Ok(score)
}

/// Why an entry stands where it does: what [`rank`] compares, in order — the percentage, then
/// the tie-break if the class has one (freestyle: the artistic part; team dressage: the side
/// judges, E and B).
pub fn reasons(score: &Score) -> Vec<Reason> {
    let mut out: Vec<Reason> = score.exact.map(|p| Reason { what: "percentage", value: p, higher_first: true }).into_iter().collect();
    if let Some(t) = score.tie_break {
        let what = if score.part_b == Some(t) { "artistic (B) percentage" } else { "side judges' (E, B) percentages" };
        out.push(Reason { what, value: t, higher_first: true });
    }
    // Only between entries tied in ranks 1–3 (S38).
    for (i, m) in score.final_marks.iter().enumerate() {
        out.push(Reason { what: if i == 0 { "judges C + H, first tie movement (ranks 1–3)" } else { "judges C + H, second tie movement (ranks 1–3)" }, value: *m, higher_first: true });
    }
    out
}

/// Rank scored entries. Returns (entry id, placement) in listing order: ranked entries by
/// percentage (ties by horse name), then entries that are out. Entries without a percentage
/// that are not out (not marked yet) are left out of the ranking.
pub fn rank(scores: &[Score]) -> Vec<(String, Placement)> {
    // What [`reasons`] says, in the same order.
    let key = |s: &Score| (s.exact, s.tie_break);
    let mut ranked: Vec<&Score> = scores.iter().filter(|s| !s.out && s.percentage.is_some()).collect();
    ranked.sort_by_key(|s| std::cmp::Reverse(key(s)));
    let mut ranks = competition_ranks(&ranked, |a, b| key(a) == key(b));
    // A Tag der Jugend final: entries tied in ranks 1–3 are separated by judges C and H's marks
    // for the tie movements, in order (S38); still tied, lots are drawn (ranks set by hand).
    let mut i = 0;
    while i < ranked.len() {
        let end = i + ranks[i..].iter().take_while(|r| **r == ranks[i]).count();
        if ranks[i] <= 3 && end - i > 1 && ranked[i..end].iter().any(|s| !s.final_marks.is_empty()) {
            let first = ranks[i];
            ranked[i..end].sort_by(|a, b| b.final_marks.cmp(&a.final_marks));
            let within = competition_ranks(&ranked[i..end], |a, b| a.final_marks == b.final_marks);
            for (r, w) in ranks[i..end].iter_mut().zip(within) {
                *r = first + w - 1;
            }
        }
        i = end;
    }

    let mut listed: Vec<(u32, &Score)> = ranks.into_iter().zip(ranked).collect();
    order_ties_by_horse(&mut listed, |e| e.0, |e| e.1.horse.as_str());

    let mut out: Vec<(String, Placement)> = listed.into_iter().map(|(r, s)| (s.id.clone(), Placement::Rank(r))).collect();
    out.extend(scores.iter().filter(|s| s.out).map(|s| (s.id.clone(), Placement::Out)));
    out
}

/// How a team class's percentage is made (team dressage, Rosson's `calcDressurResultat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamMethod {
    /// One sheet per team per judge, scored as an entry's.
    Plain,
    /// Five judges, 3/2 (Rosson's `mannschaft`): H, C and M each judge one rider, E and B the
    /// team; Σ % (H, C, M) / 6 + Σ % (E, B) / 4.
    ThreeTwo,
    /// OKV team dressage (programme `okv_mannschaftsdressur`): the mean of H, C, M's
    /// percentages and the mean of E, B's, averaged. Ties (3/2 too): the side judges' marks.
    Okv,
}

/// A judge's maximum for team dressage's per-judge percentages: 10 × the movements'
/// coefficients (freestyle: the programme's maximum), as Rosson's `maxPunkte`.
fn sheet_max(programme: &Programme) -> Decimal {
    if programme.kind == ProgrammeKind::Freestyle {
        programme.max_points
    } else {
        programme.movements.iter().map(|m| m.coefficient * Decimal::TEN).sum()
    }
}

/// Score a team: `judges` are the class's judges (assignment id, position letter).
pub fn team_score(programme: &Programme, judges: &[(String, String)], method: TeamMethod, entry: &Entry) -> Result<Score, Error> {
    let mut score = score(programme, judges.len(), entry)?;
    if method == TeamMethod::Plain || score.percentage.is_none() {
        return Ok(score);
    }
    let max = sheet_max(programme);
    let percent_of = |judge: &str| -> Decimal {
        let total = score.judge_totals.get(judge).copied().unwrap_or_default();
        if max.is_zero() { Decimal::ZERO } else { round(total / max * Decimal::ONE_HUNDRED, 2) }
    };
    let group = |letters: &[&str]| -> Vec<&String> { judges.iter().filter(|(_, l)| letters.contains(&l.as_str())).map(|(id, _)| id).collect() };
    let (single, whole) = (group(&["H", "C", "M"]), group(&["E", "B"]));
    let sum = |js: &[&String]| -> Decimal { js.iter().map(|j| percent_of(j)).sum() };
    let percentage = match method {
        TeamMethod::ThreeTwo => sum(&single) / Decimal::from(6) + sum(&whole) / Decimal::from(4),
        _ => {
            let mean = |js: &[&String]| sum(js) / Decimal::from(js.len().max(1) as u64);
            (mean(&single) + mean(&whole)) / Decimal::TWO
        }
    };
    score.percentage = Some(round(percentage, 2));
    score.exact = Some(tidy(round(percentage, 4)));
    score.judge_percentages = judges.iter().filter(|(id, _)| score.judge_totals.contains_key(id)).map(|(id, _)| (id.clone(), percent_of(id))).collect();
    // Ties: "die Equipe mit der höheren Note der beiden Seitenrichter" (OKV, Reglement
    // Mannschaftsdressur 5); Rosson took judge E alone, and nothing in 3/2.
    score.tie_break = Some(sum(&whole));
    Ok(score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn programme() -> Programme {
        Programme {
            name: "Test".into(),
            kind: ProgrammeKind::Standard,
            final_ties: None,
            movements: vec![
                Movement { number: "1".into(), coefficient: d("1"), part: None },
                Movement { number: "2".into(), coefficient: d("2"), part: None },
            ],
            max_points: d("30"),
        }
    }

    fn sheet(marks: &[(&str, &str)], penalties: Vec<Penalty>) -> JudgeSheet {
        JudgeSheet { marks: marks.iter().map(|(m, v)| (m.to_string(), d(v))).collect(), penalties, total: None }
    }

    /// Eventing-Reglement 8.1.2's example: 171, 181 and 161 of 250 — 68.40, 72.40 and 64.40 %,
    /// on average 68.40 %.
    #[test]
    fn eventing_averages_each_judges_percentage() {
        let programme = Programme { name: "E".into(), kind: ProgrammeKind::Standard, movements: vec![Movement { number: "1".into(), coefficient: d("1"), part: None }], max_points: d("250"), final_ties: None };
        let entered = |t: &str| JudgeSheet { marks: BTreeMap::new(), penalties: vec![], total: Some(d(t)) };
        let entry = Entry { id: "a".into(), horse: "A".into(), out: false, sheets: [("H", "171"), ("C", "181"), ("M", "161")].iter().map(|(j, t)| (j.to_string(), entered(t))).collect() };
        assert_eq!(score_by_judges_mean(&programme, 3, &entry).unwrap().percentage, Some(d("68.40")));
        // 170.01 of 250 is 68.004 %, written 68.00: the average of the judges' written
        // percentages, not of the points.
        let entry = Entry { id: "b".into(), horse: "B".into(), out: false, sheets: [("H", "170.01"), ("C", "170.01"), ("M", "170.03")].iter().map(|(j, t)| (j.to_string(), entered(t))).collect() };
        assert_eq!(score_by_judges_mean(&programme, 3, &entry).unwrap().percentage, Some(d("68.00")));
        assert_eq!(score(&programme, 3, &entry).unwrap().percentage, Some(d("68.01")));
    }

    #[test]
    fn marks_times_coefficients_minus_penalties() {
        let s = sheet(&[("1", "7"), ("2", "6.5")], vec![Penalty::Points(d("2"))]);
        // 7×1 + 6.5×2 = 20, minus 2 = 18
        assert_eq!(judge_total(&programme(), "C", &s).unwrap(), d("18"));
    }

    #[test]
    fn percentage_penalty_is_rounded_to_a_tenth_before_point_penalties() {
        let s = sheet(&[("1", "7"), ("2", "6.5")], vec![Penalty::Percent(d("2")), Penalty::Points(d("2"))]);
        // 20 − round(20 × 2 %, 0.1) = 20 − 0.4 = 19.6, minus 2 = 17.6
        assert_eq!(judge_total(&programme(), "C", &s).unwrap(), d("17.6"));
    }

    #[test]
    fn percentage_over_all_judges_of_the_class() {
        let entry = Entry {
            id: "a".into(),
            horse: "Amigo".into(),
            out: false,
            sheets: [("C".to_string(), sheet(&[("1", "7"), ("2", "6.5")], vec![])), ("M".to_string(), sheet(&[("1", "8"), ("2", "7")], vec![]))].into(),
        };
        let s = score(&programme(), 2, &entry).unwrap();
        // (20 + 22) / (30 × 2) × 100 = 70.00
        assert_eq!(s.percentage, Some(d("70.00")));
    }

    fn parts(kind: ProgrammeKind, max: &str) -> Programme {
        Programme {
            name: "Parts".into(),
            kind,
            final_ties: None,
            movements: vec![
                Movement { number: "1".into(), coefficient: d("2"), part: Some(Part::A) },
                Movement { number: "2".into(), coefficient: d("3"), part: Some(Part::B) },
            ],
            max_points: d(max),
        }
    }

    #[test]
    fn freestyle_halves_a_judges_points_and_gives_the_parts() {
        let entry = Entry { id: "a".into(), horse: "A".into(), out: false, sheets: [("C".to_string(), sheet(&[("1", "7"), ("2", "8")], vec![]))].into() };
        let s = score(&parts(ProgrammeKind::Freestyle, "25"), 1, &entry).unwrap();
        // 7×2 + 8×3 = 38, halved: 19; 19 / 25 = 76 %; A 14 / 20, B 24 / 30.
        assert_eq!((s.judge_totals["C"], s.percentage, s.part_a, s.part_b), (d("19"), Some(d("76.00")), Some(d("70.00")), Some(d("80.00"))));
        assert_eq!(s.judge_parts["C"], JudgeParts { a: d("7"), b: d("12"), percent_a: Some(d("70.00")), percent_b: Some(d("80.00")) });
    }

    #[test]
    fn freestyle_ties_go_to_the_artistic_part() {
        // 7 × 2 + 8 × 3 = 38 and 8.5 × 2 + 7 × 3 = 38: equal; artistic 24 against 21.
        let p = parts(ProgrammeKind::Freestyle, "25");
        let a = Entry { id: "a".into(), horse: "Zorro".into(), out: false, sheets: [("C".to_string(), sheet(&[("1", "7"), ("2", "8")], vec![]))].into() };
        let b = Entry { id: "b".into(), horse: "Atlas".into(), out: false, sheets: [("C".to_string(), sheet(&[("1", "8.5"), ("2", "7")], vec![]))].into() };
        let scores = vec![score(&p, 1, &a).unwrap(), score(&p, 1, &b).unwrap()];
        assert_eq!(scores[0].percentage, scores[1].percentage);
        // "a" has the better artistic part (24 against 21): first, though its horse sorts last.
        assert_eq!(rank(&scores), vec![("a".to_string(), Placement::Rank(1)), ("b".to_string(), Placement::Rank(2))]);
    }

    #[test]
    fn freestyle_parts_halved_to_a_tenth_per_judge_as_rosson() {
        let mut p = parts(ProgrammeKind::Freestyle, "25");
        p.movements[0].coefficient = d("1");
        // A: 7.5 halved is 3.75, written 3.8 by each judge; the class's part from the written ones.
        let entry = Entry {
            id: "a".into(),
            horse: "A".into(),
            out: false,
            sheets: [("C".to_string(), sheet(&[("1", "7.5"), ("2", "8")], vec![])), ("M".to_string(), sheet(&[("1", "7.5"), ("2", "8")], vec![]))].into(),
        };
        let s = score(&p, 2, &entry).unwrap();
        assert_eq!(s.judge_parts["M"].a, d("3.8"));
        // 7.6 / (10 × 2 / 2) = 76 % (not 75 %).
        assert_eq!(s.part_a, Some(d("76.00")));
    }

    #[test]
    fn children_one_sheet_for_the_judges_over_the_programmes_maximum() {
        let one = sheet(&[("1", "6"), ("2", "7")], vec![]);
        let entry = Entry { id: "a".into(), horse: "A".into(), out: false, sheets: [("C".to_string(), one.clone()), ("E1".to_string(), one.clone()), ("E2".to_string(), one)].into() };
        let s = score(&parts(ProgrammeKind::Children, "50"), 3, &entry).unwrap();
        // 6×2 + 7×3 = 33 over 50 (not × 3 judges): 66 %; A 12 / 20, B 21 / 30.
        assert_eq!((s.percentage, s.part_a, s.part_b, s.max_points), (Some(d("66.00")), Some(d("60.00")), Some(d("70.00")), d("50")));
        assert!(s.judge_totals.values().all(|t| *t == d("33")));
    }

    #[test]
    fn totals_entered_per_judge_less_the_penalties() {
        let entered = |t: &str| JudgeSheet { marks: BTreeMap::new(), penalties: vec![Penalty::Points(d("2"))], total: Some(d(t)) };
        let entry = Entry { id: "a".into(), horse: "A".into(), out: false, sheets: [("C".to_string(), entered("22")), ("M".to_string(), entered("20"))].into() };
        let s = score(&programme(), 2, &entry).unwrap();
        // (20 + 18) / 60 = 63.33 %; no parts.
        assert_eq!((s.judge_totals["C"], s.percentage, s.part_a), (d("20"), Some(d("63.33")), None));
    }

    fn five(marks: [(&str, &str); 5]) -> (Vec<(String, String)>, Entry) {
        let judges: Vec<(String, String)> = ["H", "C", "M", "E", "B"].iter().map(|l| (format!("j{l}"), l.to_string())).collect();
        let sheets = judges.iter().zip(marks).map(|((id, _), (m1, m2))| (id.clone(), sheet(&[("1", m1), ("2", m2)], vec![]))).collect();
        (judges, Entry { id: "t".into(), horse: "Team".into(), out: false, sheets })
    }

    #[test]
    fn team_three_two_and_okv() {
        // Programme max 30 (10 × 1 + 10 × 2). H 21 = 70 %, C 18 = 60 %, M 24 = 80 %, E 27 = 90 %, B 15 = 50 %.
        let (judges, entry) = five([("7", "7"), ("6", "6"), ("8", "8"), ("9", "9"), ("5", "5")]);
        let p = programme();
        let three_two = team_score(&p, &judges, TeamMethod::ThreeTwo, &entry).unwrap();
        // (70 + 60 + 80) / 6 + (90 + 50) / 4 = 35 + 35 = 70.
        assert_eq!(three_two.percentage, Some(d("70.00")));
        let okv = team_score(&p, &judges, TeamMethod::Okv, &entry).unwrap();
        // (70 + 70) / 2 = 70.
        assert_eq!(okv.percentage, Some(d("70.00")));
        let plain = team_score(&p, &judges, TeamMethod::Plain, &entry).unwrap();
        // 105 of 150.
        assert_eq!(plain.percentage, Some(d("70.00")));
        // A judge missing counts 0 in 3/2 (the divisors stay 6 and 4), not in OKV's means.
        let mut partial = entry.clone();
        partial.sheets.remove("jB");
        assert_eq!(team_score(&p, &judges, TeamMethod::ThreeTwo, &partial).unwrap().percentage, Some(d("57.50")));
        assert_eq!(team_score(&p, &judges, TeamMethod::Okv, &partial).unwrap().percentage, Some(d("57.50")));
    }

    #[test]
    fn team_ties_go_to_the_side_judges() {
        // Equal (70 %), but a's side judges E and B gave 90 % + 50 %, b's 80 % + 70 % (OKV rule:
        // the higher mark of the two side judges); E alone (Rosson's) would put a first.
        let (judges, a) = five([("7", "7"), ("6", "6"), ("8", "8"), ("9", "9"), ("5", "5")]);
        let (_, mut b) = five([("7", "7"), ("6.5", "6.5"), ("6", "6"), ("8", "8"), ("7", "7")]);
        b.id = "u".into();
        b.horse = "Another".into();
        let p = programme();
        for method in [TeamMethod::Okv, TeamMethod::ThreeTwo] {
            let scores: Vec<Score> = [&a, &b].iter().map(|e| team_score(&p, &judges, method, e).unwrap()).collect();
            assert_eq!(scores[0].percentage, Some(d("70.00")));
            assert_eq!(scores[1].percentage, Some(d("70.00")), "{method:?}");
            assert_eq!(rank(&scores), vec![("u".to_string(), Placement::Rank(1)), ("t".to_string(), Placement::Rank(2))], "{method:?}");
        }
        // Equal side judges too: the same rank, listed by name.
        let (_, mut c) = five([("6", "6"), ("7", "7"), ("8", "8"), ("9", "9"), ("5", "5")]);
        c.id = "v".into();
        c.horse = "Zorro".into();
        let scores: Vec<Score> = [&a, &c].iter().map(|e| team_score(&p, &judges, TeamMethod::ThreeTwo, e).unwrap()).collect();
        assert_eq!(rank(&scores), vec![("t".to_string(), Placement::Rank(1)), ("v".to_string(), Placement::Rank(1))]);
    }
}
