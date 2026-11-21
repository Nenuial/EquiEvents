//! A class's ranking as the office prints it and Swiss Equestrian receives it: the scoring engine's result,
//! then the ranks set by hand (docs/deviations.md S18), then who is placed (S10).
//!
//! This is the one way from a class's results to its ranking; the results screen, the printed
//! lists and the upload to Swiss Equestrian all use it, and the golden tests check it against the corpus.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use scoring::ranking::{default_placed_count, is_placed, with_manual_ranks, Placement};
use scoring::{dressage, eventing, jumping, jumping::team};

use crate::input::ScoringInput;
use crate::model::*;
use crate::state::State;

/// One line of a ranking: a rider, or a team.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Entry id, or team id on team lines.
    pub id: String,
    pub team: bool,
    /// What counts: the rank set by hand, or the computed one with the numbers set by hand skipped.
    pub placement: Placement,
    /// As the rules give it, before ranks set by hand.
    pub computed: Placement,
    /// The rank was set by hand.
    pub manual: bool,
    pub placed: bool,
    /// Can be placed at all: jumping classes with a reduced second round place only riders who
    /// completed both rounds (Springreglement 3.4.2.1; S34).
    pub placeable: bool,
    /// A Tag der Jugend final: still tied in ranks 1–3 after the tie movements — lots are drawn,
    /// the office sets the ranks by hand (S38).
    pub draw: bool,
    /// Jumping: faults (or points) and time of the deciding block.
    pub faults: Option<Decimal>,
    pub time: Option<Decimal>,
    /// Cross-country: the gap to the ideal time (the overall ranking's tie-break).
    pub deviation: Option<Decimal>,
    /// Dressage: percentage and each judge's total.
    pub percentage: Option<Decimal>,
    pub judge_totals: BTreeMap<String, Decimal>,
    /// … and each judge's percentage (of one sheet's maximum).
    pub judge_percentages: BTreeMap<String, Decimal>,
    /// Dressage freestyle and Children: the parts A and B, in percent.
    pub part_a: Option<Decimal>,
    pub part_b: Option<Decimal>,
    /// … and each judge's (judge assignment id → parts).
    pub judge_parts: BTreeMap<String, dressage::JudgeParts>,
    /// Team cones (OKV Fahrcup): the team's rank points (12, 10, 9 … 2, then 1).
    pub rank_points: Option<u32>,
}

impl Line {
    fn new(id: String, team: bool, computed: Placement) -> Line {
        Line { id, team, placement: computed, computed, manual: false, placed: false, placeable: true, draw: false, faults: None, time: None, deviation: None, percentage: None, judge_totals: BTreeMap::new(), judge_percentages: BTreeMap::new(), part_a: None, part_b: None, judge_parts: BTreeMap::new(), rank_points: None }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ranking {
    /// In listing order: ranked, then out. Team classes: the teams, then every rider with their
    /// team's placement.
    pub lines: Vec<Line>,
    pub placed_count: usize,
}

/// What a ranking depends on besides the results.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    /// Set by the organiser; otherwise 30 % of the results counted (S10).
    pub placed_count: Option<usize>,
    /// Results counted for the default placed count (all but hors concours; teams in team classes).
    pub results_counted: usize,
    /// Championship: jump-offs for every medal tie (S14).
    pub medal_jump_offs: bool,
    /// The ideal time of an ideal-time class (S20), in seconds.
    pub ideal_time: Option<rust_decimal::Decimal>,
    /// Ranks set by hand, by the scoring id (entry id, or team id).
    pub manual: BTreeMap<String, u32>,
    /// A Tag der Jugend final: no ex aequo in ranks 1–3 (S38).
    pub youth_final: bool,
}

/// Rank a class from its scoring input.
pub fn rank(input: &ScoringInput, settings: &Settings) -> Result<Ranking, String> {
    // Knock-out: eight placed, 1st, 2nd, two 3rd, four 5th (Springreglement 4.5 (6); S35).
    let knock_out = matches!(input, ScoringInput::Jumping { table, .. } if table.knock_out);
    let placed_count = settings.placed_count.unwrap_or_else(|| if knock_out { 8 } else { default_placed_count(settings.results_counted) });
    let mut riders_of_teams: Vec<(String, String)> = Vec::new();
    // Team dressage 3/2: a rider's judge (H, C, M for the team's first, second, third rider).
    let mut riders_judges: BTreeMap<String, String> = BTreeMap::new();
    let lines: Vec<Line> = match input {
        ScoringInput::NotCovered(why) => return Err(why.clone()),
        ScoringInput::Jumping { table, entries } => {
            let options = jumping::Options { medal_jump_offs: settings.medal_jump_offs, placed_count: settings.placed_count, ideal_time: settings.ideal_time };
            // On the ideal-time table, the gap to the ideal time (over or under the same) breaks ties:
            // in the class (S20), and as a phase of eventing in the overall ranking (S24).
            let ideal = settings.ideal_time.filter(|_| table.blocks.iter().any(|b| b.ideal_time));
            jumping::rank_with(table, entries, options)
                .into_iter()
                .map(|r| Line { faults: r.faults, time: r.time, deviation: ideal.zip(r.time).map(|(i, t)| (t - i).abs()), placeable: r.placeable, ..Line::new(r.id, false, r.placement) })
                .collect()
        }
        ScoringInput::JumpingTeams { table, teams, team_of, count_all } => {
            riders_of_teams = team_of.iter().map(|(rider, team)| (rider.clone(), team.clone())).collect();
            riders_of_teams.sort();
            team::rank(table, teams, *count_all).into_iter().map(|r| Line { faults: r.faults, time: r.time, ..Line::new(r.id, true, r.placement) }).collect()
        }
        ScoringInput::Cross { ideal, time_penalties, entries } => eventing::rank_cross(entries, *ideal, *time_penalties)
            .into_iter()
            .map(|s| Line { faults: s.total, time: s.time, deviation: s.deviation, ..Line::new(s.id, false, s.placement) })
            .collect(),
        ScoringInput::Driving { order, lines } => {
            let scores: Vec<scoring::driving::Scored> = lines.iter().map(|l| l.scored.clone()).collect();
            scoring::driving::rank(&scores, *order)
                .into_iter()
                .map(|(id, placement)| {
                    let l = lines.iter().find(|l| l.scored.id == id).expect("same ids");
                    Line { faults: l.scored.value, time: l.time, deviation: l.deviation, judge_totals: l.judge_totals.clone(), ..Line::new(id, false, placement) }
                })
                .collect()
        }
        ScoringInput::ConesTeams { teams, team_of } => {
            riders_of_teams = team_of.clone();
            scoring::driving::rank_teams(teams).into_iter().map(|(s, placement)| Line { faults: Some(s.total), time: Some(s.time), ..Line::new(s.id, true, placement) }).collect()
        }
        ScoringInput::Overall { entries } => eventing::rank_overall(entries).into_iter().map(|s| Line { faults: Some(s.total), ..Line::new(s.id, false, s.placement) }).collect(),
        ScoringInput::DressageTeams { judges, method, teams, members } => {
            let scores = teams.iter().map(|(p, e)| dressage::team_score(p, judges, *method, e).map_err(|err| format!("team {}: {err:?}", e.id))).collect::<Result<Vec<_>, _>>()?;
            for (team, riders) in members {
                for (i, rider) in riders.iter().enumerate() {
                    riders_of_teams.push((rider.clone(), team.clone()));
                    if *method == dressage::TeamMethod::ThreeTwo {
                        if let Some((judge, _)) = ["H", "C", "M"].get(i).and_then(|l| judges.iter().find(|(_, x)| x == l)) {
                            riders_judges.insert(rider.clone(), judge.clone());
                        }
                    }
                }
            }
            dressage::rank(&scores)
                .into_iter()
                .map(|(id, placement)| {
                    let score = scores.iter().find(|s| s.id == id);
                    Line {
                        percentage: score.and_then(|s| s.percentage),
                        judge_totals: score.map(|s| s.judge_totals.clone()).unwrap_or_default(),
                        judge_percentages: score.map(|s| s.judge_percentages.clone()).unwrap_or_default(),
                        part_a: score.and_then(|s| s.part_a),
                        part_b: score.and_then(|s| s.part_b),
                        judge_parts: score.map(|s| s.judge_parts.clone()).unwrap_or_default(),
                        ..Line::new(id, true, placement)
                    }
                })
                .collect()
        }
        ScoringInput::Dressage { judges, entries, judges_mean } => {
            // Eventing: the judges' percentages averaged (Eventing-Reglement 8.1.2).
            let score = if *judges_mean { dressage::score_by_judges_mean } else { dressage::score };
            let scores = entries.iter().map(|(p, e)| score(p, judges.len(), e).map_err(|err| format!("entry {}: {err:?}", e.id))).collect::<Result<Vec<_>, _>>()?;
            dressage::rank(&scores)
                .into_iter()
                .map(|(id, placement)| {
                    let score = scores.iter().find(|s| s.id == id);
                    Line {
                        percentage: score.and_then(|s| s.percentage),
                        judge_totals: score.map(|s| s.judge_totals.clone()).unwrap_or_default(),
                        judge_percentages: score.map(|s| s.judge_percentages.clone()).unwrap_or_default(),
                        part_a: score.and_then(|s| s.part_a),
                        part_b: score.and_then(|s| s.part_b),
                        judge_parts: score.map(|s| s.judge_parts.clone()).unwrap_or_default(),
                        ..Line::new(id, false, placement)
                    }
                })
                .collect()
        }
    };

    // Ranks set by hand.
    let listing = with_manual_ranks(&lines.iter().map(|l| (l.id.clone(), l.computed)).collect::<Vec<_>>(), &settings.manual);
    let mut lines: Vec<Line> = listing
        .into_iter()
        .map(|(id, placement)| {
            let line = lines.iter().find(|l| l.id == id).expect("same ids").clone();
            Line { placement, manual: settings.manual.contains_key(&id), placed: line.placeable && is_placed(placement, placed_count), ..line }
        })
        .collect();
    // A Tag der Jugend final: still tied in ranks 1–3, lots are drawn (S38).
    if settings.youth_final && matches!(input, ScoringInput::Dressage { .. }) {
        let tied: Vec<bool> = lines.iter().map(|l| l.placement.rank().is_some_and(|r| r <= 3) && lines.iter().filter(|o| o.placement == l.placement).count() > 1).collect();
        for (l, t) in lines.iter_mut().zip(tied) {
            l.draw = t;
        }
    }
    // Team cones: rank points by the placement (after a jump-off's ranks set by hand).
    if matches!(input, ScoringInput::ConesTeams { .. }) {
        for l in lines.iter_mut().filter(|l| l.team) {
            l.rank_points = Some(scoring::driving::team_rank_points(l.placement));
        }
    }

    // Riders of team classes get their team's placement.
    for (rider, team) in riders_of_teams {
        if let Some(t) = lines.iter().find(|l| l.team && l.id == team).cloned() {
            // Team dressage 3/2: the rider's own judge's total.
            let own = |of: &BTreeMap<String, Decimal>| riders_judges.get(&rider).and_then(|j| of.get(j).map(|v| (j.clone(), *v))).into_iter().collect();
            let (judge_totals, judge_percentages) = (own(&t.judge_totals), own(&t.judge_percentages));
            lines.push(Line { placement: t.placement, computed: t.computed, manual: t.manual, placed: t.placed, judge_totals, judge_percentages, ..Line::new(rider, false, t.computed) });
        }
    }
    Ok(Ranking { lines, placed_count })
}

impl State {
    /// The ranking of a class, as it stands.
    pub fn ranking(&self, class_id: &ClassId) -> Result<Ranking, String> {
        let class = self.classes.get(class_id).ok_or_else(|| format!("unknown class {class_id}"))?;
        // A driving class of teams: its team cones' ranking, ranks set by hand included.
        if let Some(cones) = self.team_cones_phase(class) {
            return self.ranking(cones);
        }
        let manual = self.manual_ranks.range((class_id.clone(), Competitor::Entry(EntryId(0)))..).take_while(|((c, _), _)| c == class_id).map(|((_, who), m)| (who.scoring_id(), m.rank)).collect();
        let settings = Settings {
            placed_count: class.options.placed_count.map(|n| n as usize),
            results_counted: self.results_counted(class_id),
            medal_jump_offs: class.options.medal_jump_offs,
            ideal_time: class.options.ideal_time,
            manual,
            youth_final: class.options.youth_final,
        };
        rank(&self.scoring_input(class_id), &settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoring::jumping::table::ScoringTable;

    fn entry(id: &str, values: &[(&str, &str)], out: Option<u32>) -> jumping::Entry {
        jumping::Entry {
            id: id.into(),
            horse: id.into(),
            values: values.iter().map(|(f, v)| (f.to_string(), v.parse().unwrap())).collect(),
            out: out.map(|code| jumping::Out { code, kind: jumping::OutKind::Eliminated }),
        }
    }

    fn placed(ranking: &Ranking) -> Vec<&str> {
        ranking.lines.iter().filter(|l| l.placed).map(|l| l.id.as_str()).collect()
    }

    /// S34: a reduced second round places only riders who completed both rounds.
    #[test]
    fn reduced_second_round_places_riders_of_both_rounds_only() {
        let definition = r#"{"sichtbare_felder": ["zeit_initial_1", "zeit_initial_2", "strafpunkte_initial_1", "strafpunkte_initial_2"],
          "bloecke": [{"definierte_felder": ["strafpunkte_initial_1", "strafpunkte_initial_2", "zeit_initial_2"], "zeit": ["zeit_initial_2"], "strafpunkte": ["strafpunkte_initial_1", "strafpunkte_initial_2"], "resultate": []},
                      {"definierte_felder": ["strafpunkte_initial_1", "zeit_initial_1"], "zeit": ["zeit_initial_1"], "strafpunkte": ["strafpunkte_initial_1"], "resultate": []}]}"#;
        let table = ScoringTable::from_definition(21, "AZ2RUZ2", "2 Umgänge (2.red.)", definition).unwrap();
        let both = |id: &str, f2: &str| entry(id, &[("strafpunkte_initial_1", "0"), ("zeit_initial_1", "60"), ("strafpunkte_initial_2", f2), ("zeit_initial_2", "50")], None);
        let entries = vec![
            both("Amigo", "0"),
            both("Bijou", "4"),
            entry("Cassis", &[("strafpunkte_initial_1", "0"), ("zeit_initial_1", "58")], Some(21)),
            entry("Dandy", &[("strafpunkte_initial_1", "4"), ("zeit_initial_1", "55")], None),
        ];
        let ranking = rank(&ScoringInput::Jumping { table, entries }, &Settings { placed_count: Some(4), results_counted: 4, ..Settings::default() }).unwrap();
        assert_eq!(placed(&ranking), ["Amigo", "Bijou"]);
    }

    /// S35: a knock-out places eight riders, whatever 30 % of the entries is.
    #[test]
    fn knock_out_places_eight() {
        let definition = r#"{"sichtbare_felder": ["strafpunkte_initial_1", "strafpunkte_stechen_1"],
          "bloecke": [{"definierte_felder": ["strafpunkte_stechen_1"], "zeit": [], "strafpunkte": ["strafpunkte_stechen_1"], "resultate": []},
                      {"definierte_felder": ["strafpunkte_initial_1"], "zeit": [], "strafpunkte": ["strafpunkte_initial_1"], "resultate": []}]}"#;
        let table = ScoringTable::from_definition(55, "KNOCK", "Knock-out Springen", definition).unwrap();
        let entries: Vec<jumping::Entry> = (0..16)
            .map(|i| {
                let id = format!("r{i:02}");
                if i < 8 { entry(&id, &[("strafpunkte_initial_1", "40"), ("strafpunkte_stechen_1", &format!("{}", 30 + i))], None) } else { entry(&id, &[("strafpunkte_initial_1", "45")], None) }
            })
            .collect();
        let ranking = rank(&ScoringInput::Jumping { table, entries }, &Settings { results_counted: 16, ..Settings::default() }).unwrap();
        assert_eq!(ranking.placed_count, 8);
        assert_eq!(placed(&ranking).len(), 8);
    }
}
