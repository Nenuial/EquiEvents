//! From the show's state to the scoring engine's input, class by class.
//!
//! Which entries are ranked: those with a result in the class, except entries that do not start
//! and hors-concours entries (as Rosson, `RanglisteService.getResultate`). The default placed
//! count counts every result except hors concours (S10). A result is a score card, or in
//! dressage any judge's sheet (Rosson keeps the marks inside the result record; here a sheet can
//! be entered before anything else).

use std::collections::HashMap;

use scoring::dressage::{self, Penalty, Programme};
use scoring::driving;
use scoring::eventing::{self, CrossEntry, OverallEntry, PhaseValue};
use scoring::jumping::{self, table::ScoringTable, team::Team as ScoringTeam};

use crate::master;
use crate::model::*;
use crate::state::State;

/// A cones round as driven: its points, time, gap to the allowed time (youth cup) and whether
/// it went over the round's time limit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConesRoundResult {
    pub points: rust_decimal::Decimal,
    pub time: rust_decimal::Decimal,
    pub gap: rust_decimal::Decimal,
    pub over_limit: bool,
}

/// A round of a team cones team: whose, which, its points and time, and whether credited
/// (125 % of the worst first round, for a round not completed).
#[derive(Debug, Clone, PartialEq)]
pub struct TeamConesRound {
    pub entry: EntryId,
    pub round: usize,
    pub points: rust_decimal::Decimal,
    pub time: rust_decimal::Decimal,
    pub credited: bool,
}

/// A team of team cones: its rounds in its drivers' order, its drivers.
#[derive(Debug, Clone, PartialEq)]
pub struct TeamConesDetail {
    pub team: TeamId,
    pub rounds: Vec<TeamConesRound>,
    pub riders: Vec<EntryId>,
    pub complete: bool,
    pub out: bool,
}

/// Rosson's reason for a driver eliminated by the rules (`fahrenAufgabeAusgeschieden`).
pub const DRIVING_ELIMINATED: u32 = 27;

/// What the scoring engine needs to rank one class.
#[derive(Debug, Clone, PartialEq)]
pub enum ScoringInput {
    /// `judges_mean`: eventing, the percentage is the average of the judges' (8.1.2).
    Dressage { judges: Vec<String>, entries: Vec<(Programme, dressage::Entry)>, judges_mean: bool },
    /// Team dressage: the judges (assignment id, position), how the percentage is made, a sheet
    /// set per team (entry id = team id), and each ranked team's riders in team order.
    DressageTeams { judges: Vec<(String, String)>, method: dressage::TeamMethod, teams: Vec<(Programme, dressage::Entry)>, members: Vec<(String, Vec<String>)> },
    Jumping { table: ScoringTable, entries: Vec<jumping::Entry> },
    /// Team class: `team_of` maps a rider's entry id (as text) to the team id.
    /// `count_all`: every rider's result counts, none dropped (class option, S36).
    JumpingTeams { table: ScoringTable, teams: Vec<ScoringTeam>, team_of: HashMap<String, String>, count_all: bool },
    /// Eventing's cross-country phase: the ideal time (s), whether time over it counts.
    Cross { ideal: rust_decimal::Decimal, time_penalties: bool, entries: Vec<CrossEntry> },
    /// A class made of phases: each rider's value in each phase, in the phases' order.
    Overall { entries: Vec<OverallEntry> },
    /// Driving's team cones (the OKV Fahrcup): each team's rounds; riders → their team.
    ConesTeams { teams: Vec<driving::ConesTeam>, team_of: Vec<(String, String)> },
    /// A driving phase, scored: the value and its tie-breaks, which way it ranks.
    Driving { order: driving::Order, lines: Vec<DrivingLine> },
    /// Not scored by the engine yet, with the reason.
    NotCovered(String),
}

/// Judge positions in score-sheet order (team dressage 3/2: the riders' judges first, as
/// Rosson's `offizielle.service.js` 68–81).
const JUDGE_ORDER: &str = "EHCMB";
const JUDGE_ORDER_3_2: &str = "HCMEB";

/// The score card of an entry that has only judges' sheets.
static NO_SCORE_CARD: ScoreCard = ScoreCard { values: std::collections::BTreeMap::new(), out: None };

impl State {
    /// Every entry with a result in the class (a score card, or a judge's sheet), in entry order.
    fn with_results(&self, class: &ClassId) -> Vec<(&Entry, &ScoreCard)> {
        let mut found: std::collections::BTreeMap<&EntryId, &ScoreCard> = self.results.range((class.clone(), EntryId(0))..).take_while(|((c, _), _)| c == class).map(|((_, e), r)| (e, r)).collect();
        for (_, e, _) in self.sheets.range((class.clone(), EntryId(0), AssignmentId(String::new()))..).map(|(k, _)| k).take_while(|(c, _, _)| c == class) {
            found.entry(e).or_insert(&NO_SCORE_CARD);
        }
        found.into_iter().filter_map(|(e, r)| self.entries.get(e).map(|entry| (entry, r))).collect()
    }

    fn ranked_entries(&self, class: &ClassId) -> Vec<(&Entry, &ScoreCard)> {
        self.with_results(class).into_iter().filter(|(e, _)| e.status == EntryStatus::Entered && !e.hors_concours).collect()
    }

    /// The dressage judges of a class (riding or driving) in score-sheet order: by position (E, H,
    /// C, M, B, as the result lists show them; driving's judges sit at C, B, E too; judges
    /// without a known position last), two judges at the same position by assignment id.
    pub fn dressage_judges(&self, class: &ClassId) -> Vec<&Assignment> {
        let mut judges: Vec<&Assignment> = self.assignments.values().filter(|a| a.class.as_ref() == Some(class) && a.role == "DRI").collect();
        let order = match self.classes.get(class) {
            Some(c) if c.options.team_3_2 => JUDGE_ORDER_3_2,
            _ => JUDGE_ORDER,
        };
        let position = |a: &Assignment| a.letter.as_deref().filter(|l| l.len() == 1).and_then(|l| order.find(l)).unwrap_or(order.len());
        // Same position: as Rosson's store orders the ids (numbers first, numerically; then text).
        let id = |a: &Assignment| match a.id.0.parse::<u64>() {
            Ok(n) => (0, n, String::new()),
            Err(_) => (1, 0, a.id.0.clone()),
        };
        judges.sort_by_key(|a| (position(a), id(a)));
        judges
    }

    /// A score card's reason code with its kind (from the Swiss Equestrian reason list).
    /// Why a rider is out of a class: as the office entered it, or eliminated by a time limit
    /// (cones, marathon: Rosson's reason 27, `fahrenAufgabeAusgeschieden`; the eventing cross:
    /// eliminated).
    pub fn out_reason(&self, class_id: &ClassId, entry: &EntryId) -> Option<u32> {
        let card = self.results.get(&(class_id.clone(), entry.clone()))?;
        card.out.or_else(|| {
            let class = self.classes.get(class_id)?;
            let e = self.entries.get(entry)?;
            if !self.over_time_limit(class, e, &card.values) {
                return None;
            }
            if class.phase.as_ref().is_some_and(|p| p.kind == PhaseKind::Cross) {
                // Eventing's "ausgeschieden".
                let eliminated = |d: Option<&str>| self.reasons.values().find(|r| r.discipline.as_deref() == d && r.name == "ausgeschieden").map(|r| r.id);
                return eliminated(Some("CC")).or_else(|| eliminated(None)).or(Some(DRIVING_ELIMINATED));
            }
            Some(DRIVING_ELIMINATED)
        })
    }

    /// Driven over a time limit, eliminated by the rules: a cones round over its maximum time
    /// (per round and licence; unset: twice the allowed time; Rosson compares the time driven,
    /// without the turnout's bonus), a marathon phase over its limit or an obstacle over five
    /// minutes.
    pub fn over_time_limit(&self, class: &Class, e: &Entry, values: &std::collections::BTreeMap<String, rust_decimal::Decimal>) -> bool {
        // The eventing cross: over the maximum time, twice the ideal time, is elimination
        // (Eventing-Reglement 8.3.5; E5).
        if class.phase.as_ref().is_some_and(|p| p.kind == PhaseKind::Cross) && self.eventing(class) {
            return class.options.ideal_time.zip(values.get("zeit_initial_1")).is_some_and(|(ideal, t)| *t > ideal * rust_decimal::Decimal::TWO);
        }
        let Some(settings) = class.options.driving.as_ref() else { return false };
        match class.phase.as_ref().map(|p| p.kind) {
            Some(PhaseKind::Marathon) => {
                let (a, b, obstacles) = marathon_runs(settings, values);
                let (limit_a, limit_b) = settings.marathon_limits();
                return driving::marathon_over_limit(a, b, &obstacles, limit_a, limit_b);
            }
            Some(PhaseKind::Cones) => {}
            _ => return false,
        }
        let Some(licence) = self.driving_licence(class, e) else { return false };
        (1..=settings.rounds.unwrap_or(1)).any(|r| {
            let key = format!("{r}:{licence}");
            let maximum = settings.maximum.get(&key).copied().or_else(|| settings.allowed.get(&key).map(|a| a * rust_decimal::Decimal::TWO));
            values.get(&format!("zeit_{r}")).zip(maximum).is_some_and(|(t, m)| *t > m)
        })
    }

    pub fn out(&self, code: Option<u32>) -> Option<jumping::Out> {
        code.map(|code| jumping::Out { code, kind: self.reasons.get(&code).map_or(jumping::OutKind::Other, |r| master::reason_kind(&r.name)) })
    }

    fn horse_name(&self, entry: &Entry) -> String {
        entry.guest_horse.clone().or_else(|| entry.horse.as_ref().and_then(|h| self.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default()
    }

    /// Results counted for the default placed count: all except hors concours (team classes:
    /// the teams).
    pub fn results_counted(&self, class: &ClassId) -> usize {
        match self.scoring_input(class) {
            ScoringInput::JumpingTeams { teams, .. } => return teams.iter().filter(|t| !t.riders.is_empty()).count(),
            ScoringInput::DressageTeams { teams, .. } => return teams.len(),
            ScoringInput::Overall { entries } => return entries.len(),
            ScoringInput::Driving { lines, .. } => return lines.len(),
            ScoringInput::ConesTeams { teams, .. } => return teams.len(),
            _ => {}
        }
        self.with_results(class).into_iter().filter(|(e, _)| !e.hors_concours).count()
    }

    pub fn scoring_input(&self, class_id: &ClassId) -> ScoringInput {
        let Some(class) = self.classes.get(class_id) else { return ScoringInput::NotCovered(format!("unknown class {class_id}")) };
        let Some(published) = self.published_classes.get(&class.published) else { return ScoringInput::NotCovered("no published class".into()) };
        if let Some(what) = class.options.not_covered.first() {
            return ScoringInput::NotCovered(format!("class option {what}"));
        }
        let ranked = self.ranked_entries(class_id);
        let team_class = published.team_size.is_some_and(|n| n > 0) && ranked.iter().any(|(e, _)| e.team.is_some());

        match self.class_discipline(class) {
            // A driving class of teams ranks as its team cones (Rosson).
            "CA" if self.team_cones_phase(class).is_some() => self.scoring_input(self.team_cones_phase(class).expect("checked")),
            "CC" | "CA" if !class.phases.is_empty() => self.overall_input(class),
            "CD" if self.driving(class) => self.driving_dressage_input(class, &ranked),
            "GE" | "HI" | "DE" | "OF" | "MT" | "PR" => self.driving_input(class, &ranked),
            "CROSS" => self.cross_input(class, &ranked),
            "CS" => {
                let Some(def) = class.scoring_table.and_then(|t| self.scoring_tables.get(&t)) else { return ScoringInput::NotCovered("no scoring table".into()) };
                let table = match ScoringTable::from_definition(def.id, &def.code, &def.name, &def.definition) {
                    Ok(t) => t,
                    Err(e) => return ScoringInput::NotCovered(format!("scoring table {}: {e:?}", def.code)),
                };
                // The ideal-time table ranks on the gap to the ideal time: none set, no ranking.
                if table.blocks.iter().any(|b| b.ideal_time) && class.options.ideal_time.is_none() {
                    return ScoringInput::NotCovered("temps idéal à saisir (Détails…) · Idealzeit eingeben (Details…)".into());
                }
                let entries: Vec<(Option<TeamId>, jumping::Entry)> = ranked
                    .iter()
                    .map(|(e, r)| (e.team.clone(), jumping::Entry { id: e.id.to_string(), horse: self.horse_name(e), values: with_max_time(class, &r.values), out: self.out(r.out) }))
                    .collect();
                if class.options.team_only {
                    // The team's own score card, as its only "rider".
                    let teams = self
                        .team_results
                        .range((class_id.clone(), TeamId(String::new()))..)
                        .take_while(|((c, _), _)| c == class_id)
                        .map(|((_, t), card)| {
                            let name = self.teams.get(t).map(|t| t.name.clone()).unwrap_or_else(|| t.0.clone());
                            let rider = jumping::Entry { id: format!("team:{t}"), horse: name.clone(), values: card.values.clone(), out: self.out(card.out) };
                            ScoringTeam { id: t.0.clone(), name, riders: vec![rider] }
                        })
                        .collect();
                    return ScoringInput::JumpingTeams { table, teams, team_of: HashMap::new(), count_all: class.options.team_count_all };
                }
                if !team_class {
                    return ScoringInput::Jumping { table, entries: entries.into_iter().map(|(_, e)| e).collect() };
                }
                let mut teams: Vec<ScoringTeam> = Vec::new();
                let mut team_of = HashMap::new();
                for (team, entry) in entries {
                    let Some(team) = team else { continue };
                    team_of.insert(entry.id.clone(), team.0.clone());
                    match teams.iter_mut().find(|t| t.id == team.0) {
                        Some(t) => t.riders.push(entry),
                        None => {
                            let name = self.teams.get(&team).map(|t| t.name.clone()).unwrap_or_else(|| team.0.clone());
                            teams.push(ScoringTeam { id: team.0.clone(), name, riders: vec![entry] });
                        }
                    }
                }
                ScoringInput::JumpingTeams { table, teams, team_of, count_all: class.options.team_count_all }
            }
            "CD" if published.team_size.is_some_and(|n| n > 0) && !self.class_teams(class_id).is_empty() => self.dressage_teams_input(class),
            "CD" => self.dressage_input(class, &ranked),
            other => ScoringInput::NotCovered(format!("discipline {other}")),
        }
    }

    /// How a class is entered and scored: its phase's (`CD`, `CS`, `CROSS`), else its
    /// published class's discipline (`CC` for a class made of phases).
    pub fn class_discipline<'a>(&'a self, class: &'a Class) -> &'a str {
        match &class.phase {
            Some(p) => p.kind.discipline(),
            None => self.published_classes.get(&class.published).map(|p| p.discipline.as_str()).unwrap_or(""),
        }
    }

    /// Eventing's cross-country: the obstacle penalties and the time (`strafpunkte_initial_1`,
    /// `zeit_initial_1`) against the ideal time.
    fn cross_input(&self, class: &Class, ranked: &[(&Entry, &ScoreCard)]) -> ScoringInput {
        let Some(ideal) = class.options.ideal_time else { return ScoringInput::NotCovered("temps idéal à saisir (Détails…) · Idealzeit eingeben (Details…)".into()) };
        let entries = ranked
            .iter()
            .map(|(e, r)| CrossEntry {
                id: e.id.to_string(),
                horse: self.horse_name(e),
                points: r.values.get("strafpunkte_initial_1").copied().unwrap_or_default(),
                time: r.values.get("zeit_initial_1").copied(),
                // Out as entered, or over the maximum time (E5).
                out: r.out.is_some() || self.over_time_limit(class, e, &r.values),
            })
            .collect();
        ScoringInput::Cross { ideal, time_penalties: class.options.ideal_time_penalties, entries }
    }

    /// A class made of phases: each rider's value in each phase (dressage 100 − %, jumping its
    /// penalties, cross its points; or the phase's rank), from the phases' own rankings. Eventing:
    /// what breaks ties, as its kind says (Eventing-Reglement 5.9), and a rider without a result
    /// in a closed phase is out of it (E10).
    fn overall_input(&self, class: &Class) -> ScoringInput {
        use rust_decimal::Decimal;
        let n = class.phases.len();
        let eventing = self.eventing(class);
        let by_rank = self.overall_by_rank(class);
        let mut by_entry: std::collections::BTreeMap<String, Vec<Option<PhaseValue>>> = std::collections::BTreeMap::new();
        // Eventing: each rider's result in each kind of phase (penalties, gap to the ideal time,
        // time), for the ties.
        type PhaseResult = (Decimal, Option<Decimal>, Option<Decimal>);
        let mut results: std::collections::BTreeMap<String, std::collections::BTreeMap<PhaseKind, PhaseResult>> = std::collections::BTreeMap::new();
        let mut closed = vec![false; n];
        for (i, id) in class.phases.iter().enumerate() {
            let Some(phase) = self.classes.get(id) else { continue };
            let Some(kind) = phase.phase.as_ref().map(|p| p.kind) else { continue };
            closed[i] = self.status_of(phase) == ClassStatus::Final;
            // A phase not ranked yet (no results, no ideal time): nothing from it.
            let Ok(ranking) = self.ranking(id) else { continue };
            for l in ranking.lines.iter().filter(|l| !l.team) {
                let value = if l.placement == scoring::ranking::Placement::Out {
                    Some(PhaseValue { value: Decimal::ZERO, deviation: None, out: true })
                } else {
                    let penalties = match kind {
                        // Driving dressage counts its total (on 160), eventing's 100 − %.
                        PhaseKind::Dressage if l.percentage.is_none() => l.faults,
                        PhaseKind::Dressage => l.percentage.map(|p| scoring::number::round(Decimal::ONE_HUNDRED - p, 2)),
                        _ => l.faults,
                    };
                    // Eventing: the jury's penalties in this phase (S41).
                    let penalties = penalties.map(|p| p + self.jury_penalty(id, &l.id));
                    if let Some(p) = penalties {
                        results.entry(l.id.clone()).or_default().insert(kind, (p, l.deviation, l.time));
                    }
                    let v = if by_rank { l.placement.rank().map(Decimal::from) } else { penalties };
                    v.map(|value| PhaseValue { value, deviation: l.deviation, out: false })
                };
                by_entry.entry(l.id.clone()).or_insert_with(|| vec![None; n])[i] = value;
            }
        }
        let kinds: Vec<PhaseKind> = class.phases.iter().filter_map(|p| self.classes.get(p)?.phase.as_ref().map(|p| p.kind)).collect();
        let has = |k: PhaseKind| kinds.contains(&k);
        let event = self.driving_event(class);
        // A full driving event: eliminated or not finished in a test is not out of the event —
        // that test counts the highest penalties there + 25 %, and the driver ranks after
        // everyone who finished every test (13.10 §7.1.2).
        let mut behind: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        if event == Some(DrivingEvent::Full) {
            for i in 0..n {
                let highest = by_entry.values().filter_map(|p| p[i].filter(|v| !v.out).map(|v| v.value)).max();
                let counted = scoring::number::round(highest.unwrap_or_default() * Decimal::new(125, 2), 2);
                for (id, phases) in by_entry.iter_mut() {
                    if phases[i].is_some_and(|v| v.out) {
                        phases[i] = Some(PhaseValue { value: counted, deviation: None, out: false });
                        behind.insert(id.clone());
                    }
                }
            }
        }
        let dressage_phase = class.phases.iter().filter_map(|p| self.classes.get(p)).find(|c| c.phase.as_ref().is_some_and(|p| p.kind == PhaseKind::Dressage));
        let entries = by_entry
            .into_iter()
            .map(|(id, mut phases)| {
                let horse = id.parse().ok().and_then(|e| self.entries.get(&EntryId(e))).map(|e| self.horse_name(e)).unwrap_or_default();
                let driving_ties = event.map(|event| {
                    let mine = results.get(&id);
                    let penalties = |k: PhaseKind| mine.and_then(|r| r.get(&k)).map_or(eventing::MISSING, |r| r.0);
                    match event {
                        // 13.10 §7.1.1: the marathon (B), then dressage (A II).
                        DrivingEvent::Full => vec![("marathon penalties", penalties(PhaseKind::Marathon)), ("dressage penalties", penalties(PhaseKind::Dressage))],
                        // §7.2: the derby (D), then dressage.
                        DrivingEvent::Derby => vec![("derby penalties", penalties(PhaseKind::Derby)), ("dressage penalties", penalties(PhaseKind::Dressage))],
                        // §7.3: dressage, then its driven movements (more first).
                        DrivingEvent::Short => {
                            let movements = dressage_phase.zip(id.parse().ok()).and_then(|(p, e)| self.driven_movements(p, &EntryId(e)));
                            vec![("dressage penalties", penalties(PhaseKind::Dressage)), ("driven dressage movements, the most first", movements.map_or(eventing::MISSING, |m| -m))]
                        }
                    }
                });
                let ties = eventing.then(|| {
                    // E10: a closed phase without a result is out.
                    for (slot, closed) in phases.iter_mut().zip(&closed) {
                        if *closed && slot.is_none() {
                            *slot = Some(PhaseValue { value: Decimal::ZERO, deviation: None, out: true });
                        }
                    }
                    let mine = results.get(&id);
                    let result = |k: PhaseKind| mine.and_then(|r| r.get(&k));
                    let penalties = |k: PhaseKind| result(k).map_or(eventing::MISSING, |r| r.0);
                    let gap = |k: PhaseKind| result(k).and_then(|r| r.1).unwrap_or(eventing::MISSING);
                    let time = |k: PhaseKind| result(k).and_then(|r| r.2).filter(|t| !t.is_zero()).unwrap_or(eventing::MISSING);
                    let (dressage, jumping, cross) = (has(PhaseKind::Dressage), has(PhaseKind::Jumping), has(PhaseKind::Cross));
                    match (dressage, jumping, cross) {
                        // CCN (5.9.2).
                        (true, true, true) => vec![("cross penalties", penalties(PhaseKind::Cross)), ("cross gap to the ideal time", gap(PhaseKind::Cross)), ("dressage penalties", penalties(PhaseKind::Dressage)), ("jumping time", time(PhaseKind::Jumping))],
                        // Combined classes (5.9.1).
                        (true, true, false) => vec![("dressage penalties", penalties(PhaseKind::Dressage))],
                        (false, true, true) => vec![("cross penalties", penalties(PhaseKind::Cross))],
                        (true, false, true) => vec![("cross penalties", penalties(PhaseKind::Cross)), ("dressage penalties", penalties(PhaseKind::Dressage))],
                        _ => Vec::new(),
                    }
                });
                let behind = behind.contains(&id);
                OverallEntry { id, horse, phases, ties: ties.or(driving_ties), behind }
            })
            .collect();
        ScoringInput::Overall { entries }
    }

    /// A class made of phases ranks by its phases' ranks: as the office set, or because an open
    /// phase or a multitest can only count by rank (driving, Rosson's `rangpunkte`).
    /// Eventing: by the rules, always — rank points for combined dressage + jumping, penalties
    /// otherwise (Eventing-Reglement 5.9).
    pub fn overall_by_rank(&self, class: &Class) -> bool {
        let kinds: Vec<PhaseKind> = class.phases.iter().filter_map(|p| self.classes.get(p)?.phase.as_ref().map(|p| p.kind)).collect();
        if self.eventing(class) {
            return kinds.len() == 2 && kinds.contains(&PhaseKind::Dressage) && kinds.contains(&PhaseKind::Jumping);
        }
        // Driving: full and derby events by penalties, by the rules (13.10 §7.1, §7.2).
        if matches!(self.driving_event(class), Some(DrivingEvent::Full | DrivingEvent::Derby)) {
            return false;
        }
        class.options.rank_points || kinds.iter().any(|k| matches!(k, PhaseKind::Open | PhaseKind::Multitest))
    }

    /// Eventing: the jury's penalties for a rider (scoring id) in a phase (S41); 0 if none.
    pub fn jury_penalty(&self, phase: &ClassId, rider: &str) -> rust_decimal::Decimal {
        rider.parse().ok().and_then(|e| self.jury_penalties.get(&(phase.clone(), EntryId(e)))).copied().unwrap_or_default()
    }

    /// A driving class made of phases: which event it is (Fahrreglement 1.5 (2), 13.10 §7). A
    /// full event (Vollprüfung): dressage, marathon, cones; a derby event: dressage and derby (RVP),
    /// with cones too (DVP); any other: a short event (Kurzprüfung).
    pub fn driving_event(&self, class: &Class) -> Option<DrivingEvent> {
        if class.phases.is_empty() || self.published_classes.get(&class.published).is_none_or(|p| p.discipline != "CA") {
            return None;
        }
        let kinds: Vec<PhaseKind> = class.phases.iter().filter_map(|p| self.classes.get(p)?.phase.as_ref().map(|p| p.kind)).collect();
        let has = |k: PhaseKind| kinds.contains(&k);
        Some(if has(PhaseKind::Dressage) && has(PhaseKind::Marathon) && has(PhaseKind::Cones) && !has(PhaseKind::Derby) {
            DrivingEvent::Full
        } else if has(PhaseKind::Dressage) && has(PhaseKind::Derby) && !has(PhaseKind::Marathon) {
            DrivingEvent::Derby
        } else {
            DrivingEvent::Short
        })
    }

    /// A phase of a full driving event (Vollprüfung).
    pub fn in_full_driving_event(&self, class: &Class) -> bool {
        class.phase.as_ref().and_then(|p| self.classes.get(&p.of)).and_then(|c| self.driving_event(c)) == Some(DrivingEvent::Full)
    }

    /// How a cones phase is scored: as the office set it (A when unset), always A in a full event
    /// (8.1.1). In Wertung B the faults are seconds added to the time (8.4.6, 8.4.7); no penalty
    /// for overtime, over the maximum time eliminated (P. Burkhard, 2026-11-18).
    pub fn cones_scoring(&self, class: &Class) -> ConesKind {
        if self.in_full_driving_event(class) {
            return ConesKind::A;
        }
        class.options.driving.as_ref().and_then(|d| d.cones).unwrap_or(ConesKind::A)
    }

    /// Driving dressage: a driver's driven movements, the judges' mean of their marks without
    /// the collective marks ("Gesamteindruck"): a short event's second tie-break (13.10 §7.3).
    fn driven_movements(&self, phase: &Class, entry: &EntryId) -> Option<rust_decimal::Decimal> {
        let def = phase.programme.and_then(|p| self.programmes.get(&p))?;
        let json: serde_json::Value = serde_json::from_str(&def.definition).ok()?;
        let movements: Vec<String> = json["tabellen"]
            .as_array()?
            .iter()
            .filter(|t| !matches!(t["layout"].as_str(), Some("strafpunkte" | "lektionen_ohne_noten")) && t["titel_de"].as_str().is_none_or(|title| !title.trim().starts_with("Gesamteindruck")))
            .flat_map(|t| t["felder"].as_array().into_iter().flatten())
            .filter_map(|f| match &f["nummer"] {
                serde_json::Value::String(n) => Some(n.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .collect();
        let sums: Vec<rust_decimal::Decimal> = self
            .dressage_judges(&phase.id)
            .iter()
            .filter_map(|j| self.sheets.get(&(phase.id.clone(), entry.clone(), j.id.clone())).filter(|s| !s.marks.is_empty()))
            .map(|s| movements.iter().filter_map(|m| s.marks.get(m)).sum())
            .collect();
        driving::mean(&sums)
    }

    /// An eventing class (CC), or a phase of one.
    pub fn eventing(&self, class: &Class) -> bool {
        let of = class.phase.as_ref().and_then(|p| self.classes.get(&p.of)).unwrap_or(class);
        self.published_classes.get(&of.published).is_some_and(|p| p.discipline == "CC")
    }

    /// A phase of a driving class (CA).
    pub fn driving(&self, class: &Class) -> bool {
        class.phase.as_ref().and_then(|p| self.classes.get(&p.of)).and_then(|c| self.published_classes.get(&c.published)).is_some_and(|p| p.discipline == "CA")
    }

    /// Driving dressage: each judge's total on 160 (coefficients 1), the judges' mean; lowest first.
    fn driving_dressage_input(&self, class: &Class, ranked: &[(&Entry, &ScoreCard)]) -> ScoringInput {
        let judges: Vec<String> = self.dressage_judges(&class.id).into_iter().map(|a| a.id.0.clone()).collect();
        let Some(def) = class.programme.and_then(|p| self.programmes.get(&p)) else { return ScoringInput::NotCovered("no programme".into()) };
        let movements = match master::programme(def) {
            Ok(p) => p.movements.len(),
            Err(e) => return ScoringInput::NotCovered(e),
        };
        let lines = ranked
            .iter()
            .map(|(e, r)| {
                let judge_totals: std::collections::BTreeMap<String, rust_decimal::Decimal> = judges
                    .iter()
                    .filter_map(|j| {
                        let s = self.sheets.get(&(class.id.clone(), e.id.clone(), AssignmentId(j.clone())))?;
                        let points: rust_decimal::Decimal = s.marks.values().copied().sum();
                        let penalties: rust_decimal::Decimal = s.penalty_points.iter().copied().sum();
                        // A judge who gave the total: the total as given, plus the penalties on
                        // the 160 basis (S43).
                        let total = match s.total {
                            Some(t) => Some(scoring::number::round(t + driving::dressage_penalties_on_160(penalties, movements), 2)),
                            None => driving::dressage_judge_total(points, movements, penalties),
                        };
                        total.map(|t| (j.clone(), t))
                    })
                    .collect();
                let value = driving::mean(&judge_totals.values().copied().collect::<Vec<_>>());
                DrivingLine { scored: driving::Scored { id: e.id.to_string(), horse: self.horse_name(e), value, ties: Vec::new(), out: r.out.is_some() }, time: None, deviation: None, judge_totals }
            })
            .collect();
        ScoringInput::Driving { order: driving::Order::Lowest, lines }
    }

    /// Marathon, cones, derby, open and multitest, from the score cards (multitest: the judges'
    /// sheets).
    fn driving_input(&self, class: &Class, ranked: &[(&Entry, &ScoreCard)]) -> ScoringInput {
        let Some(kind) = class.phase.as_ref().map(|p| p.kind) else { return ScoringInput::NotCovered("not a phase".into()) };
        if kind == PhaseKind::Cones && !self.class_teams(&class.id).is_empty() {
            return self.cones_teams_input(class);
        }
        let settings = class.options.driving.clone().unwrap_or_default();
        let zero = rust_decimal::Decimal::ZERO;
        let mut order = driving::Order::Lowest;
        let mut lines = Vec::new();
        for (e, r) in ranked {
            let v = |k: &str| r.values.get(k).copied();
            let n = |k: &str| v(k).unwrap_or(zero);
            let (mut value, mut ties, mut time, mut deviation, mut out) = (None, Vec::new(), None, None, r.out.is_some());
            match kind {
                PhaseKind::Marathon => {
                    let (a, b, obstacles) = marathon_runs(&settings, &r.values);
                    let times = |a, m| driving::MarathonTimes { allowed: a, minimum: m };
                    let factor = settings.factor.unwrap_or(rust_decimal::Decimal::new(25, 2));
                    // Over a time limit: eliminated (the rules).
                    if self.over_time_limit(class, e, &r.values) {
                        out = true;
                    }
                    if a.time.is_some() || b.time.is_some() {
                        let s = driving::marathon(a, b, &obstacles, times(settings.a_allowed, settings.a_minimum), times(settings.b_allowed, settings.b_minimum), factor);
                        value = Some(s.total);
                        time = a.time.zip(b.time).map(|(x, y)| x + y);
                    }
                }
                PhaseKind::Cones => {
                    let rounds = match self.cones_rounds(class, e, &r.values) {
                        Ok(rounds) => rounds,
                        Err(why) => return ScoringInput::NotCovered(why),
                    };
                    let driven: Vec<&ConesRoundResult> = rounds.iter().flatten().collect();
                    // Over the maximum time in a round: eliminated (Rosson).
                    if driven.iter().any(|r| r.over_limit) {
                        out = true;
                    }
                    if !driven.is_empty() {
                        let (total, times, gaps) = driven.iter().fold((zero, zero, zero), |(p, t, g), r| (p + r.points, t + r.time, g + r.gap));
                        value = Some(total);
                        time = Some(times);
                        if self.cones_scoring(class) == ConesKind::JuniorCup {
                            deviation = Some(gaps);
                            ties.push(("gap to the allowed time", gaps));
                        } else {
                            ties.push(("time", times));
                        }
                    }
                }
                // The presentation: the judges' overall marks averaged (to the hundredth, half
                // up), taken from 10 (13.10 §2; S44).
                PhaseKind::Presentation => {
                    let marks: Vec<rust_decimal::Decimal> = ["note_1", "note_2", "note_3"].iter().filter_map(|k| v(k)).collect();
                    value = driving::mean(&marks).map(|m| rust_decimal::Decimal::TEN - m);
                }
                PhaseKind::Derby => {
                    if let Some(t) = v("zeit") {
                        value = Some(driving::derby(t, n("abwuerfe"), n("uebrige"), settings.time_factor.unwrap_or(rust_decimal::Decimal::new(25, 2))));
                        time = Some(t);
                    }
                }
                PhaseKind::Open => {
                    let (points, seconds) = (v("punkte"), v("sekunden"));
                    match settings.open.unwrap_or(OpenKind::Time) {
                        OpenKind::Time => value = seconds,
                        OpenKind::PenaltyPoints => value = points,
                        OpenKind::Points => {
                            order = driving::Order::Highest;
                            value = points;
                        }
                        OpenKind::PointsThenTime => {
                            order = driving::Order::Highest;
                            value = points;
                            ties.push(("time", seconds.unwrap_or(zero)));
                        }
                    }
                    time = seconds;
                }
                PhaseKind::Multitest => {
                    order = driving::Order::Highest;
                    let coefficients = driving::multitest_coefficients(settings.multitest.as_deref().unwrap_or("B4"));
                    let totals: Vec<rust_decimal::Decimal> = self
                        .dressage_judges(&class.id)
                        .into_iter()
                        .filter_map(|j| self.sheets.get(&(class.id.clone(), e.id.clone(), j.id.clone())))
                        .map(|s| {
                            let marks: Vec<(rust_decimal::Decimal, rust_decimal::Decimal)> = coefficients.iter().enumerate().map(|(i, c)| (s.marks.get(&(i + 1).to_string()).copied().unwrap_or(zero), *c)).collect();
                            let p = |k: &str| s.marks.get(k).copied().unwrap_or(zero);
                            // Penalties A, B, C as Rosson keeps them on the judge's sheet.
                            driving::multitest_judge_total(&marks, [p("strafpunkte_a"), p("strafpunkte_b"), p("strafpunkte_c")])
                        })
                        .collect();
                    value = driving::mean(&totals);
                }
                _ => return ScoringInput::NotCovered(format!("{kind:?} is not a driving phase")),
            }
            lines.push(DrivingLine { scored: driving::Scored { id: e.id.to_string(), horse: self.horse_name(e), value, ties, out }, time, deviation, judge_totals: Default::default() });
        }
        ScoringInput::Driving { order, lines }
    }

    /// Team cones (OKV Reglement Fahrcup 4.2): every round of every driver of a team; a round a
    /// driver does not complete (over its time limit, or not driven by a driver who is out or
    /// does not start) is credited 125 % of the worst first round driven in the class. A team
    /// with a round still to drive is not complete; a team none of whose drivers is left, out.
    pub fn cones_team_details(&self, class: &Class) -> Result<Vec<TeamConesDetail>, String> {
        let rounds = class.options.driving.as_ref().and_then(|d| d.rounds).unwrap_or(1) as usize;
        let mut members = Vec::new();
        for (team, riders) in self.class_teams(&class.id) {
            for e in riders {
                let card = self.results.get(&(class.id.clone(), e.id.clone()));
                let values = card.map(|c| c.values.clone()).unwrap_or_default();
                let driven = self.cones_rounds(class, e, &values)?;
                let gone = e.status != EntryStatus::Entered || card.is_some_and(|c| c.out.is_some());
                members.push((team.clone(), e, driven, gone));
            }
        }
        let worst_first = members.iter().filter_map(|(_, _, r, _)| r.first().copied().flatten()).filter(|r| !r.over_limit).map(|r| r.points).max();
        let mut details = Vec::new();
        for (team, _) in self.class_teams(&class.id) {
            let mine: Vec<_> = members.iter().filter(|m| m.0 == team).collect();
            // Not one driver of the team started: not a team of the class.
            if mine.iter().all(|(_, e, _, _)| e.status != EntryStatus::Entered) {
                continue;
            }
            let mut detail = TeamConesDetail { team: team.clone(), rounds: Vec::new(), riders: Vec::new(), complete: true, out: false };
            for (_, e, driven, gone) in &mine {
                if e.status == EntryStatus::Entered && !e.hors_concours {
                    detail.riders.push(e.id.clone());
                }
                for i in 0..rounds {
                    let round = |points, time, credited| TeamConesRound { entry: e.id.clone(), round: i + 1, points, time, credited };
                    match driven.get(i).copied().flatten() {
                        Some(r) if !r.over_limit => detail.rounds.push(round(r.points, r.time, false)),
                        r if r.is_some() || *gone => match worst_first {
                            Some(w) => detail.rounds.push(round(driving::credited_round(w), r.map(|r| r.time).unwrap_or_default(), true)),
                            None => detail.complete = false,
                        },
                        _ => detail.complete = false,
                    }
                }
            }
            detail.out = mine.iter().all(|(_, _, driven, gone)| *gone && driven.iter().flatten().all(|r| r.over_limit));
            details.push(detail);
        }
        Ok(details)
    }

    fn cones_teams_input(&self, class: &Class) -> ScoringInput {
        let details = match self.cones_team_details(class) {
            Ok(d) => d,
            Err(why) => return ScoringInput::NotCovered(why),
        };
        let team_of = details.iter().flat_map(|d| d.riders.iter().map(|e| (e.to_string(), d.team.0.clone()))).collect();
        let teams = details
            .iter()
            .map(|d| driving::ConesTeam {
                id: d.team.0.clone(),
                name: self.teams.get(&d.team).map(|t| t.name.clone()).unwrap_or_else(|| d.team.0.clone()),
                rounds: d.rounds.iter().map(|r| driving::TeamRound { points: r.points, time: r.time }).collect(),
                complete: d.complete,
                out: d.out,
            })
            .collect();
        ScoringInput::ConesTeams { teams, team_of }
    }

    /// A marathon's penalties broken down (phases A and B, each obstacle's points), for the
    /// ranking's columns; None before a phase was driven.
    pub fn marathon_breakdown(&self, class: &Class, values: &std::collections::BTreeMap<String, rust_decimal::Decimal>) -> Option<(driving::MarathonScore, Vec<rust_decimal::Decimal>)> {
        let settings = class.options.driving.clone().unwrap_or_default();
        let (a, b, obstacles) = marathon_runs(&settings, values);
        if a.time.is_none() && b.time.is_none() {
            return None;
        }
        let factor = settings.factor.unwrap_or(rust_decimal::Decimal::new(25, 2));
        let times = |a, m| driving::MarathonTimes { allowed: a, minimum: m };
        let score = driving::marathon(a, b, &obstacles, times(settings.a_allowed, settings.a_minimum), times(settings.b_allowed, settings.b_minimum), factor);
        let each = obstacles.iter().map(|o| scoring::number::round(o.time * factor, 2) + o.penalties).collect();
        Some((score, each))
    }

    /// A driving class of teams: its cones phase, whose team ranking is the class's (Rosson).
    pub fn team_cones_phase<'a>(&'a self, class: &'a Class) -> Option<&'a ClassId> {
        if class.phases.is_empty() || !self.published_classes.get(&class.published).is_some_and(|p| p.discipline == "CA" && p.team_size.is_some_and(|n| n > 0)) {
            return None;
        }
        class.phases.iter().find(|p| self.classes.get(p).and_then(|c| c.phase.as_ref()).is_some_and(|ph| ph.kind == PhaseKind::Cones)).filter(|p| !self.class_teams(p).is_empty())
    }

    /// A driver's cones rounds (`None`: not driven): the points (time faults by the scoring and
    /// the licence's allowed time with the turnout's bonus, plus the faults), the time, the gap
    /// (youth cup), and whether the round went over its time limit.
    pub fn cones_rounds(&self, class: &Class, e: &Entry, values: &std::collections::BTreeMap<String, rust_decimal::Decimal>) -> Result<Vec<Option<ConesRoundResult>>, String> {
        let mut settings = class.options.driving.clone().unwrap_or_default();
        // A full event's cones: Wertung A, the overtime to the hundredth, always (8.1.1, 8.4.4).
        if self.in_full_driving_event(class) {
            settings.international = true;
        }
        let scoring = match self.cones_scoring(class) {
            ConesKind::A => driving::ConesScoring::A,
            ConesKind::B => driving::ConesScoring::B,
            ConesKind::JuniorCup => driving::ConesScoring::JuniorCup,
        };
        let licence = self.driving_licence(class, e);
        let bonus = e.record["fahren_gespann"].as_str().map(str::to_string).or_else(|| e.record["fahren_gespann"].as_u64().map(|g| g.to_string())).and_then(|g| settings.bonus.get(&g).copied()).unwrap_or_default();
        let n = |k: &str| values.get(k).copied().unwrap_or_default();
        (1..=settings.rounds.unwrap_or(1))
            .map(|round_| {
                let Some(t) = values.get(&format!("zeit_{round_}")).copied() else { return Ok(None) };
                let key = licence.as_ref().map(|l| format!("{round_}:{l}"));
                let Some(allowed) = key.as_ref().and_then(|k| settings.allowed.get(k)).copied() else {
                    let l = licence.clone().unwrap_or_default();
                    return Err(format!("temps accordé à saisir (manche {round_}, licence {l}) · erlaubte Zeit eingeben (Umgang {round_}, Lizenz {l})"));
                };
                let cones = driving::ConesRound { time: Some(t), obstacle_faults: n(&format!("hindernis_{round_}")), other_faults: n(&format!("andere_{round_}")) };
                let (_, points, gap) = driving::cones_round(scoring, cones, allowed + bonus, settings.international);
                let maximum = key.as_ref().and_then(|k| settings.maximum.get(k)).copied().unwrap_or(allowed * rust_decimal::Decimal::TWO);
                Ok(Some(ConesRoundResult { points, time: t, gap, over_limit: t > maximum }))
            })
            .collect()
    }

    /// The licence a driver drives under in a class: the first of the class's licences (FB,
    /// FL, FM, FS) they hold (FB also as "BRF", as Rosson).
    pub fn driving_licence(&self, class: &Class, entry: &Entry) -> Option<String> {
        let parent = class.phase.as_ref().and_then(|p| self.classes.get(&p.of)).unwrap_or(class);
        let published = self.published_classes.get(&parent.published)?;
        let held = self.persons.get(&entry.rider).map(|p| p.licences.clone()).unwrap_or_default();
        published.record["fahren_lizenzen"].as_array()?.iter().filter_map(|l| l.as_str()).find(|l| if *l == "FB" { held.contains("FB") || held.contains("BRF") } else { held.contains(l) }).map(str::to_string)
    }

    /// A class's teams in team order, each with its riders in their order in the team.
    pub fn class_teams(&self, class: &ClassId) -> Vec<(TeamId, Vec<&Entry>)> {
        let mut starts: Vec<(&Start, &Entry)> = self.starts.get(class).into_iter().flatten().filter_map(|s| self.entries.get(&s.entry).map(|e| (s, e))).filter(|(_, e)| e.team.is_some()).collect();
        starts.sort_by_key(|(s, e)| (s.team_order.unwrap_or(u32::MAX), e.team.clone(), s.order));
        let mut teams: Vec<(TeamId, Vec<&Entry>)> = Vec::new();
        for (_, e) in starts {
            let team = e.team.clone().expect("a team");
            match teams.iter_mut().find(|(t, _)| *t == team) {
                Some((_, members)) => members.push(e),
                None => teams.push((team, vec![e])),
            }
        }
        teams
    }

    /// Team dressage: a sheet set per team. A team with a rider who does not start, or rides
    /// hors concours, is not ranked (Rosson's `RanglisteService.getResultate`).
    fn dressage_teams_input(&self, class: &Class) -> ScoringInput {
        let judges: Vec<(String, String)> = self.dressage_judges(&class.id).into_iter().map(|a| (a.id.0.clone(), a.letter.clone().unwrap_or_default())).collect();
        let Some(def) = class.programme.and_then(|p| self.programmes.get(&p)) else { return ScoringInput::NotCovered("no programme".into()) };
        if master::is_individual(def) {
            return ScoringInput::NotCovered("team dressage with programmes chosen by the riders".into());
        }
        let programme = match master::programme(def) {
            Ok(p) => p,
            Err(e) => return ScoringInput::NotCovered(e),
        };
        let method = if class.options.team_3_2 {
            dressage::TeamMethod::ThreeTwo
        } else if master::is_okv_team(def) {
            dressage::TeamMethod::Okv
        } else {
            dressage::TeamMethod::Plain
        };
        let (mut teams, mut members) = (Vec::new(), Vec::new());
        for (team, riders) in self.class_teams(&class.id) {
            if riders.iter().any(|e| e.status != EntryStatus::Entered || e.hors_concours) {
                continue;
            }
            let sheets: std::collections::BTreeMap<String, dressage::JudgeSheet> = judges
                .iter()
                .filter_map(|(j, _)| {
                    // Penalties ticked before the judge's marks: not a sheet yet (S40).
                    let s = self.team_sheets.get(&(class.id.clone(), team.clone(), AssignmentId(j.clone()))).filter(|s| !s.marks.is_empty())?;
                    let penalties = s.penalty_points.iter().map(|p| Penalty::Points(*p)).chain(s.penalty_percent.iter().map(|p| Penalty::Percent(*p))).collect();
                    Some((j.clone(), dressage::JudgeSheet { marks: s.marks.clone(), penalties, total: None }))
                })
                .collect();
            let card = self.team_results.get(&(class.id.clone(), team.clone()));
            if sheets.is_empty() && card.is_none() {
                continue;
            }
            let name = self.teams.get(&team).map(|t| t.name.clone()).unwrap_or_else(|| team.0.clone());
            teams.push((programme.clone(), dressage::Entry { id: team.0.clone(), horse: name, out: card.is_some_and(|c| c.out.is_some()), sheets }));
            members.push((team.0.clone(), riders.iter().map(|e| e.id.to_string()).collect()));
        }
        ScoringInput::DressageTeams { judges, method, teams, members }
    }

    fn dressage_input(&self, class: &Class, ranked: &[(&Entry, &ScoreCard)]) -> ScoringInput {
        let judges: Vec<String> = self.dressage_judges(&class.id).into_iter().map(|a| a.id.0.clone()).collect();

        let class_programme = class.programme.and_then(|p| self.programmes.get(&p));
        let individual = class_programme.is_some_and(master::is_individual);
        // A Tag der Jugend final: ties in ranks 1–3 by judges C and H (S38).
        let final_judges: Vec<String> = self.dressage_judges(&class.id).into_iter().filter(|a| matches!(a.letter.as_deref(), Some("C" | "H"))).map(|a| a.id.0.clone()).collect();

        let mut entries = Vec::new();
        for (entry, result) in ranked {
            let def = if individual { entry.programme.and_then(|p| self.programmes.get(&p)) } else { class_programme };
            let Some(def) = def else { return ScoringInput::NotCovered("no programme".into()) };
            let mut programme = match master::programme(def) {
                Ok(p) => p,
                Err(e) => return ScoringInput::NotCovered(e),
            };
            if let Some(movements) = master::youth_final_movements(def).filter(|_| class.options.youth_final) {
                programme.final_ties = Some(dressage::FinalTies { movements: movements.iter().map(|m| m.to_string()).collect(), judges: final_judges.clone() });
            }
            let mut sheets: std::collections::BTreeMap<String, dressage::JudgeSheet> = judges
                .iter()
                .filter_map(|j| {
                    // Penalties ticked before the judge's marks: not a sheet yet (S40).
                    let s = self.sheets.get(&(class.id.clone(), entry.id.clone(), AssignmentId(j.clone()))).filter(|s| !s.marks.is_empty() || s.total.is_some())?;
                    let penalties = s.penalty_points.iter().map(|p| Penalty::Points(*p)).chain(s.penalty_percent.iter().map(|p| Penalty::Percent(*p))).collect();
                    Some((j.clone(), dressage::JudgeSheet { marks: s.marks.clone(), penalties, total: s.total }))
                })
                .collect();
            // Judged jointly (young horses): the jury's one sheet, the first judge's, counts for
            // every judge (S39).
            if master::judged_jointly(def) {
                if let Some(one) = judges.iter().find_map(|j| sheets.get(j)).cloned() {
                    sheets = judges.iter().map(|j| (j.clone(), one.clone())).collect();
                }
            }
            entries.push((programme, dressage::Entry { id: entry.id.to_string(), horse: self.horse_name(entry), out: result.out.is_some(), sheets }));
        }
        // Eventing's dressage phase: the judges' percentages averaged (Eventing-Reglement 8.1.2).
        let judges_mean = class.phase.as_ref().is_some_and(|p| p.kind == PhaseKind::Dressage) && self.eventing(class);
        ScoringInput::Dressage { judges, entries, judges_mean }
    }
}

/// A jumping phase that counts its maximum time: 0.4 per second started over it, added to the
/// first round's penalties (Rosson's `max_zeit_berechnen`, eventing).
fn with_max_time(class: &Class, values: &std::collections::BTreeMap<String, rust_decimal::Decimal>) -> std::collections::BTreeMap<String, rust_decimal::Decimal> {
    let mut values = values.clone();
    if let (true, Some(max), Some(t)) = (class.options.max_time_penalties, class.options.max_time, values.get("zeit_initial_1").copied()) {
        let over = eventing::time_points(t, max);
        if !over.is_zero() {
            *values.entry("strafpunkte_initial_1".into()).or_default() += over;
        }
    }
    values
}

/// A driving phase's line, scored, with what the ranking shows besides the value.
#[derive(Debug, Clone, PartialEq)]
pub struct DrivingLine {
    pub scored: driving::Scored,
    pub time: Option<rust_decimal::Decimal>,
    /// Cones (youth cup): the gap to the allowed time.
    pub deviation: Option<rust_decimal::Decimal>,
    /// Dressage: each judge's total.
    pub judge_totals: std::collections::BTreeMap<String, rust_decimal::Decimal>,
}

/// A marathon as entered: phases A and B (start and finish as times of day, a finish after
/// midnight counted the next day), their penalties and neutralised seconds, and phase B's
/// obstacles.
fn marathon_runs(settings: &DrivingSettings, values: &std::collections::BTreeMap<String, rust_decimal::Decimal>) -> (driving::MarathonRun, driving::MarathonRun, Vec<driving::MarathonObstacle>) {
    let v = |k: &str| values.get(k).copied();
    let n = |k: &str| v(k).unwrap_or_default();
    let run = |p: &str| driving::MarathonRun { time: v(&format!("{p}_finish")).zip(v(&format!("{p}_start"))).map(|(f, s)| if f >= s { f - s } else { f + rust_decimal::Decimal::from(86_400) - s }), penalties: n(&format!("{p}_penalties")), neutralised: n(&format!("{p}_neutralised")) };
    let count = settings.obstacles.unwrap_or(8);
    let obstacles = (1..=count).map(|i| driving::MarathonObstacle { time: n(&format!("h{i}_time")), penalties: n(&format!("h{i}_penalties")), neutralised: n(&format!("h{i}_neutralised")) }).collect();
    (run("a"), run("b"), obstacles)
}
