//! Eventing (Concours Complet): the cross-country phase and the overall ranking of a class made
//! of phases (dressage, show jumping, cross-country), as Rosson ranks them
//! (`pruefungsdurchfuehrung.service.js` `calcCrossResultat` 536–557, `rangliste.service.js`
//! `updateRanglisteCross` 709–727, `prepareGesamtResultate` and `updateRanglisteGesamt` 751–958).
//!
//! Cross: the obstacle penalties as entered, plus, when the class counts them, 0.4 per second
//! started over the ideal time (none for being early). Ranked on those points, then the gap to
//! the ideal time, then the time, lowest first.
//!
//! Overall: each phase gives a value (dressage 100 − %, jumping its penalties, cross its points;
//! or each phase's rank, "rank points"), added up; lowest first. A rider out in any phase is out
//! of the overall ranking; a phase not ridden adds nothing while it runs, and is out once it is
//! closed (eventing, E10).
//!
//! Equal totals, eventing (Eventing-Reglement 5.9; decided 2026-11-17):
//! - CCN (dressage, jumping, cross): the cross result (obstacles, time and other penalties), then
//!   the time closer to the cross's ideal time, then dressage, then the faster jumping time;
//!   still equal, the rank is shared.
//! - Combined dressage + jumping: ranked by rank points, always; then the better dressage result.
//! - Combined jumping + cross: penalties; then the better cross result.
//! - Combined dressage + cross: penalties; then the better cross result, then dressage.
//!
//! Equal totals, driving (Fahrreglement 13.10 §7; decided 2026-11-17):
//! - Full event (Vollprüfung: dressage, marathon, cones): the marathon, then dressage. Eliminated
//!   or not finished in a test: not out of the event — that test counts the highest penalties
//!   there + 25 %, and the driver ranks after everyone who finished every test (§7.1.2).
//! - Derby events (dressage + derby, with cones too): the derby, then dressage (§7.2).
//! - Any other (short events): dressage, then the driven dressage movements (without the
//!   collective marks), the most first; still equal, shared (§7.3).
//!
//! Other classes made of phases: the phases in their order decide (lower value, then smaller gap
//! to the ideal time).

use crate::number::{round, Decimal};
use crate::ranking::{competition_ranks, order_ties_by_horse, Placement};

/// Penalty points per second started over the ideal time.
pub const PER_SECOND: Decimal = Decimal::from_parts(4, 0, 0, false, 1);

/// One rider's cross-country result.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossEntry {
    pub id: String,
    /// Used to list tied riders (S4).
    pub horse: String,
    /// Obstacle penalties as entered.
    pub points: Decimal,
    /// Time in seconds; `None`: not timed yet.
    pub time: Option<Decimal>,
    /// Eliminated, retired or withdrawn.
    pub out: bool,
}

/// A cross-country result as ranked.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossScore {
    pub id: String,
    pub placement: Placement,
    /// Obstacle penalties plus time points.
    pub total: Option<Decimal>,
    pub time_points: Option<Decimal>,
    /// The gap to the ideal time, in seconds.
    pub deviation: Option<Decimal>,
    pub time: Option<Decimal>,
}

/// Why a rider stands where they do in the cross-country: what [`rank_cross`] compares, in
/// order — penalties (obstacles and time), then the gap to the ideal time, then the time.
pub fn cross_reasons(s: &CrossScore) -> Vec<crate::ranking::Reason> {
    use crate::ranking::Reason;
    [("penalties", s.total), ("gap to the ideal time", s.deviation), ("time", s.time)].into_iter().filter_map(|(what, v)| v.map(|value| Reason { what, value, higher_first: false })).collect()
}

/// Why a rider stands where they do overall: what [`rank_overall`] compares, in order — the
/// total, then each phase's value and gap to the ideal time, in the phases' order.
pub fn overall_reasons(e: &OverallEntry) -> Vec<crate::ranking::Reason> {
    use crate::ranking::Reason;
    const PHASE: [&str; 4] = ["phase 1", "phase 2", "phase 3", "phase 4"];
    const GAP: [&str; 4] = ["phase 1 gap to the ideal time", "phase 2 gap to the ideal time", "phase 3 gap to the ideal time", "phase 4 gap to the ideal time"];
    let total = round(e.phases.iter().flatten().map(|p| p.value).sum(), 2);
    let mut out = vec![Reason { what: "total", value: total, higher_first: false }];
    if let Some(ties) = &e.ties {
        out.extend(ties.iter().map(|(what, value)| Reason { what, value: *value, higher_first: false }));
        return out;
    }
    for (i, p) in e.phases.iter().enumerate().take(4) {
        let (value, gap) = p.map_or((Decimal::ZERO, Decimal::ZERO), |p| (p.value, p.deviation.unwrap_or_default()));
        out.push(Reason { what: PHASE[i], value, higher_first: false });
        out.push(Reason { what: GAP[i], value: gap, higher_first: false });
    }
    out
}

/// The time points of a cross-country time: 0.4 per second started over the ideal time.
pub fn time_points(time: Decimal, ideal: Decimal) -> Decimal {
    if time > ideal { (time - ideal).ceil() * PER_SECOND } else { Decimal::ZERO }
}

/// Rank a cross-country phase. `time_penalties`: the class counts time points (Rosson's
/// `idealzeit_berechnen`). Riders without a time are not ranked (not ridden yet), except when out.
pub fn rank_cross(entries: &[CrossEntry], ideal: Decimal, time_penalties: bool) -> Vec<CrossScore> {
    let score = |e: &CrossEntry| -> CrossScore {
        let (total, points, deviation) = match e.time {
            Some(t) => {
                let points = if time_penalties { time_points(t, ideal) } else { Decimal::ZERO };
                (Some(round(e.points + points, 2)), Some(points), Some((t - ideal).abs()))
            }
            None => (None, None, None),
        };
        CrossScore { id: e.id.clone(), placement: Placement::Out, total, time_points: points, deviation, time: e.time }
    };
    let key = |s: &CrossScore| (s.total, s.deviation, s.time);
    let mut ranked: Vec<(CrossScore, &CrossEntry)> = entries.iter().filter(|e| !e.out && e.time.is_some()).map(|e| (score(e), e)).collect();
    ranked.sort_by_key(|(s, _)| key(s));
    let ranks = competition_ranks(&ranked, |a, b| key(&a.0) == key(&b.0));
    let mut listed: Vec<(u32, (CrossScore, &CrossEntry))> = ranks.into_iter().zip(ranked).collect();
    order_ties_by_horse(&mut listed, |e| e.0, |e| e.1 .1.horse.as_str());
    let mut out: Vec<CrossScore> = listed.into_iter().map(|(r, (s, _))| CrossScore { placement: Placement::Rank(r), ..s }).collect();
    let mut gone: Vec<&CrossEntry> = entries.iter().filter(|e| e.out).collect();
    gone.sort_by(|a, b| a.horse.cmp(&b.horse));
    out.extend(gone.into_iter().map(score));
    out
}

/// A rider's result in one phase, for the overall ranking.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseValue {
    /// Penalties (dressage 100 − %, jumping penalties, cross points), or the phase rank.
    pub value: Decimal,
    /// The gap to the ideal time (cross; jumping on an ideal-time table).
    pub deviation: Option<Decimal>,
    pub out: bool,
}

/// A rider of the overall ranking: one value per phase, in the phases' order; `None`: not ridden
/// (yet).
#[derive(Debug, Clone, PartialEq)]
pub struct OverallEntry {
    pub id: String,
    pub horse: String,
    pub phases: Vec<Option<PhaseValue>>,
    /// What decides between equal totals, in order, lower first (eventing: the cross, its gap to
    /// the ideal time, dressage, the jumping time; driving: the marathon or the derby, dressage —
    /// as the class's kind says). `None`: the phases in their order, value then gap.
    pub ties: Option<Vec<(&'static str, Decimal)>>,
    /// Ranked after everyone who is not (driving, a full event: eliminated in a test, which
    /// counts the highest penalties + 25 %; Fahrreglement 13.10 §7.1.2).
    pub behind: bool,
}

/// What an overall tie-break compares when a value is missing (a phase not ridden yet): last.
pub const MISSING: Decimal = Decimal::MAX;

#[derive(Debug, Clone, PartialEq)]
pub struct OverallScore {
    pub id: String,
    pub placement: Placement,
    pub total: Decimal,
}

/// The overall ranking: lowest total first; ties by the phases in order; riders out of any phase
/// after the others. Riders without any phase are left out.
pub fn rank_overall(entries: &[OverallEntry]) -> Vec<OverallScore> {
    let total = |e: &OverallEntry| round(e.phases.iter().flatten().map(|p| p.value).sum(), 2);
    let out = |e: &OverallEntry| e.phases.iter().flatten().any(|p| p.out);
    // The tie-break: as the class's kind says (eventing); or each phase's value then gap, in order
    // (a phase not ridden: as zero).
    let key = |e: &OverallEntry| -> (bool, Decimal, Vec<Decimal>) {
        let ties = match &e.ties {
            Some(ties) => ties.iter().map(|(_, v)| *v).collect(),
            None => e.phases.iter().flat_map(|p| p.map_or([Decimal::ZERO, Decimal::ZERO], |p| [p.value, p.deviation.unwrap_or_default()])).collect(),
        };
        (e.behind, total(e), ties)
    };
    let mut ranked: Vec<&OverallEntry> = entries.iter().filter(|e| !out(e) && e.phases.iter().any(Option::is_some)).collect();
    ranked.sort_by_key(|e| key(e));
    let ranks = competition_ranks(&ranked, |a, b| key(a) == key(b));
    let mut listed: Vec<(u32, &OverallEntry)> = ranks.into_iter().zip(ranked).collect();
    order_ties_by_horse(&mut listed, |e| e.0, |e| e.1.horse.as_str());
    let mut scores: Vec<OverallScore> = listed.into_iter().map(|(r, e)| OverallScore { id: e.id.clone(), placement: Placement::Rank(r), total: total(e) }).collect();
    let mut gone: Vec<&OverallEntry> = entries.iter().filter(|e| out(e)).collect();
    gone.sort_by(|a, b| a.horse.cmp(&b.horse));
    scores.extend(gone.into_iter().map(|e| OverallScore { id: e.id.clone(), placement: Placement::Out, total: total(e) }));
    scores
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    fn cross(id: &str, points: &str, time: Option<&str>) -> CrossEntry {
        CrossEntry { id: id.into(), horse: id.to_uppercase(), points: d(points), time: time.map(d), out: false }
    }

    #[test]
    fn cross_time_points_per_second_started_over_the_ideal_time() {
        // Ideal 5:00. 5:12.3 is 12.3 s over: 13 seconds started, 5.2 points; early: nothing.
        assert_eq!(time_points(d("312.3"), d("300")), d("5.2"));
        assert_eq!(time_points(d("290"), d("300")), Decimal::ZERO);
        let ranked = rank_cross(&[cross("a", "20", Some("312")), cross("b", "0", Some("320")), cross("c", "0", Some("290")), cross("d", "0", None)], d("300"), true);
        let line = |id: &str| ranked.iter().find(|s| s.id == id).unwrap().clone();
        // c: 0 points, 10 s early; b: 0 + 8 = 8; a: 20 + 4.8 = 24.8; d: not timed, not ranked.
        assert_eq!((line("c").placement, line("c").total), (Placement::Rank(1), Some(d("0.00"))));
        assert_eq!((line("b").placement, line("b").total), (Placement::Rank(2), Some(d("8.00"))));
        assert_eq!((line("a").placement, line("a").total), (Placement::Rank(3), Some(d("24.80"))));
        assert!(ranked.iter().all(|s| s.id != "d"));
        // Without time points: equal points, the smaller gap to the ideal time first.
        let ranked = rank_cross(&[cross("a", "0", Some("312")), cross("b", "0", Some("295"))], d("300"), false);
        assert_eq!(ranked[0].id, "b");
    }

    #[test]
    fn overall_lowest_total_ties_by_phases_in_order() {
        let p = |v: &str, dev: Option<&str>| Some(PhaseValue { value: d(v), deviation: dev.map(d), out: false });
        let entries = vec![
            // 30 + 4 + 0 = 34
            OverallEntry { id: "a".into(), horse: "A".into(), phases: vec![p("30", None), p("4", None), p("0", Some("3"))], ties: None, behind: false },
            // 28 + 0 + 6 = 34: better dressage, first.
            OverallEntry { id: "b".into(), horse: "B".into(), phases: vec![p("28", None), p("0", None), p("6", Some("1"))], ties: None, behind: false },
            // 25 + 0 + 0 = 25
            OverallEntry { id: "c".into(), horse: "C".into(), phases: vec![p("25", None), p("0", None), p("0", Some("2"))], ties: None, behind: false },
            // Out in the cross.
            OverallEntry { id: "e".into(), horse: "E".into(), phases: vec![p("20", None), p("0", None), Some(PhaseValue { value: Decimal::ZERO, deviation: None, out: true })], ties: None, behind: false },
            // Dressage only so far: ranked on it.
            OverallEntry { id: "f".into(), horse: "F".into(), phases: vec![p("40", None), None, None], ties: None, behind: false },
        ];
        let ranked: Vec<(String, Placement, Decimal)> = rank_overall(&entries).into_iter().map(|s| (s.id, s.placement, s.total)).collect();
        assert_eq!(
            ranked,
            vec![
                ("c".to_string(), Placement::Rank(1), d("25.00")),
                ("b".to_string(), Placement::Rank(2), d("34.00")),
                ("a".to_string(), Placement::Rank(3), d("34.00")),
                ("f".to_string(), Placement::Rank(4), d("40.00")),
                ("e".to_string(), Placement::Out, d("20.00")),
            ]
        );
    }

    #[test]
    fn overall_equal_in_every_phase_share_the_rank() {
        let p = |v: &str| Some(PhaseValue { value: d(v), deviation: None, out: false });
        let entries = vec![
            OverallEntry { id: "z".into(), horse: "Zorro".into(), phases: vec![p("30"), p("4")], ties: None, behind: false },
            OverallEntry { id: "y".into(), horse: "Atlas".into(), phases: vec![p("30"), p("4")], ties: None, behind: false },
        ];
        let ranked = rank_overall(&entries);
        assert_eq!(ranked.iter().map(|s| (s.id.as_str(), s.placement)).collect::<Vec<_>>(), vec![("y", Placement::Rank(1)), ("z", Placement::Rank(1))]);
    }

    /// Eventing-Reglement 5.9.2: equal totals go to the cross, then the gap to its ideal time,
    /// then dressage, then the faster jumping time; still equal, the rank is shared.
    #[test]
    fn ccn_ties_by_the_cross_then_its_ideal_time_then_dressage_then_the_jumping_time() {
        let p = |v: &str| Some(PhaseValue { value: d(v), deviation: None, out: false });
        let entry = |id: &str, phases: [&str; 3], cross: &str, gap: &str, dressage: &str, time: &str| OverallEntry {
            id: id.into(),
            horse: id.to_uppercase(),
            phases: phases.iter().map(|v| p(v)).collect(),
            ties: Some(vec![("cross", d(cross)), ("cross gap to the ideal time", d(gap)), ("dressage", d(dressage)), ("jumping time", d(time))]),
            behind: false,
        };
        let entries = vec![
            // All 40 in total.
            entry("a", ["30", "4", "6"], "6", "2", "30", "70"),
            entry("b", ["34", "0", "6"], "6", "1", "34", "75"),
            entry("c", ["36", "4", "0"], "0", "9", "36", "80"),
            entry("d", ["30", "4", "6"], "6", "2", "30", "68"),
            entry("e", ["30", "4", "6"], "6", "2", "30", "68"),
        ];
        let ranked: Vec<(String, Placement)> = rank_overall(&entries).into_iter().map(|s| (s.id, s.placement)).collect();
        // c: best cross; b: cross 6 but closest to the ideal time; d and e: faster jumping than a,
        // and equal in everything: shared.
        assert_eq!(ranked, vec![("c".into(), Placement::Rank(1)), ("b".into(), Placement::Rank(2)), ("d".into(), Placement::Rank(3)), ("e".into(), Placement::Rank(3)), ("a".into(), Placement::Rank(5))]);
    }
}
