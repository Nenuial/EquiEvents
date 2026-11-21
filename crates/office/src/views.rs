//! What each screen shows, ready to display: names instead of ids, one row per thing the
//! office looks at. Built from a show's state; serialised to JSON for the UI.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::{json, Value};
use show::model::*;
use show::startlist::Line;
use show::State;

#[derive(Debug, Clone, Serialize)]
pub struct EventView {
    pub id: u64,
    pub number: String,
    pub title: Vec<String>,
    pub place: String,
    pub from: String,
    pub to: String,
    pub organiser: String,
    pub disciplines: Vec<String>,
    /// Start numbers fixed for the whole event, and given out.
    pub fixed_numbers: bool,
    pub numbers_assigned: bool,
}

/// A show in the library.
#[derive(Debug, Clone, Serialize)]
pub struct ShowSummary {
    /// The show's id in the library (its file name).
    pub id: String,
    pub event: Option<EventView>,
    pub entries: usize,
    pub classes: usize,
    pub results: usize,
    /// Imported as Swiss Equestrian delivered it, to rehearse.
    pub rehearsal: bool,
    /// A training show (the practice event, Swiss Equestrian's demo event): never synced.
    pub training: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassRow {
    pub id: String,
    pub published: u64,
    pub number: String,
    /// The class's own name (a prize's), as typed: often none.
    pub name: String,
    /// What lists call the class: its name, or else its category and scoring table.
    pub title: String,
    pub date: String,
    /// As announced ("08h00", "à la suite").
    pub time: String,
    pub discipline: String,
    pub category: String,
    pub scoring: String,
    pub starters: usize,
    pub not_starting: usize,
    pub results: usize,
    /// `planned`, `running`, `final`.
    pub status: &'static str,
    /// When the start lists / results were last sent to Swiss Equestrian (ms since 1970).
    pub start_lists_sent: Option<u64>,
    pub results_sent: Option<u64>,
    /// The class whose penalties the riders bring (U9), by id.
    pub qualifier: Option<String>,
    /// Changed since last sent to Swiss Equestrian.
    pub results_changed: bool,
    pub start_lists_changed: bool,
    /// Its riders have start numbers (renumbering asks first).
    pub numbered: bool,
    /// Entries of the published class on none of its start lists.
    pub unplaced: usize,
    /// Too many on its start list: the sections proposed (docs/deviations.md S19). Jumping:
    /// more than [`MAX_ENTRIES`] (teams in team classes). Dressage: more than
    /// [`DRESSAGE_SERIES`] starters, hors concours not counted (Dressurreglement 5.2 (1)).
    pub divide_into: Option<usize>,
    /// The number of entries it has more than: 90 (jumping), 35 or 40 (dressage).
    pub over: usize,
    /// It has to be divided (jumping over 90, dressage over 40); otherwise (dressage, 36 to 40)
    /// two series are the rule, unless the technical delegate allows up to 40.
    pub must_divide: bool,
    /// The most starters, if the office set one.
    pub max_starters: Option<u32>,
    /// Eventing: this class is a phase (`dressage`, `jumping`, `cross`) of `phase_of`.
    pub phase: Option<PhaseKind>,
    pub phase_of: Option<String>,
    /// Eventing: the class made of phases, its phases in tie-break order.
    pub phases: Vec<String>,
}

/// The most entries a class runs with (teams in team classes); a larger one is divided into
/// sections before it runs (docs/deviations.md S19).
pub const MAX_ENTRIES: usize = 90;
/// Dressage: more starters than this run in two series with two juries; the technical delegate
/// can allow up to [`DRESSAGE_MAX`] (Dressurreglement 5.2 (1)).
pub const DRESSAGE_SERIES: usize = 35;
pub const DRESSAGE_MAX: usize = 40;

/// A published class no class has been set up for yet.
#[derive(Debug, Clone, Serialize)]
pub struct PublishedRow {
    pub id: u64,
    pub number: String,
    pub date: String,
    pub discipline: String,
    pub category: String,
    pub scoring: String,
    pub entries: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Day {
    pub date: String,
    pub classes: Vec<ClassRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShowView {
    pub id: String,
    pub event: Option<EventView>,
    pub days: Vec<Day>,
    pub not_set_up: Vec<PublishedRow>,
    pub entries: usize,
    pub riders: usize,
    pub horses: usize,
    /// Rankings over several classes (U8).
    pub combined: Vec<CombinedRow>,
}

/// Where an entry starts.
#[derive(Debug, Clone, Serialize)]
pub struct StartRef {
    pub class: String,
    pub number: String,
    pub order: u32,
    pub start_number: Option<u32>,
    /// The class's day (ISO date).
    pub date: String,
    /// The start time with start times, else the class's time ("08:42"; empty: none).
    pub time: String,
    /// The position printed on the start list.
    pub position: Option<u32>,
    /// A phase of a class made of phases (eventing, driving): which.
    pub phase: Option<PhaseKind>,
}

/// An entry, with its rider and horse: a row of the participants.
#[derive(Debug, Clone, Serialize)]
pub struct EntryRow {
    pub id: u64,
    pub rider_id: u64,
    pub rider: String,
    pub town: String,
    pub club: String,
    pub nationality: String,
    pub licences: String,
    pub youth: String,
    pub horse_id: Option<u64>,
    pub horse: String,
    pub guest_horse: bool,
    pub published: u64,
    pub class_number: String,
    pub category: String,
    pub discipline: String,
    pub team: Option<String>,
    pub not_starting: bool,
    pub hors_concours: bool,
    pub waiting_list: bool,
    pub organiser_place: bool,
    pub se_status: String,
    pub fee_paid: Option<Decimal>,
    /// The rider was changed at the show (RW).
    pub rider_changed: bool,
    /// The office's note.
    pub comment: String,
    /// The rider's remark made with the entry (on devices with the show's private data).
    pub remark: String,
    /// The level (Stufe).
    pub stage: Option<i64>,
    /// How many levels its class has (more than one: the entry's level matters).
    pub levels: u32,
    pub starts: Vec<StartRef>,
    /// Driving: the class's phases (Swiss Equestrian's names, in order) and the entry's horses
    /// with the phases each starts in.
    pub driving_phases: Vec<String>,
    pub driving_horses: Vec<DrivingHorse>,
    /// A class it starts in is final: it cannot change before that class is reopened.
    pub locked: bool,
    /// It has a result or marks already (it was ridden, or is being).
    pub ridden: bool,
}

/// A horse of a driving entry and the phases it starts in.
#[derive(Debug, Clone, Serialize)]
pub struct DrivingHorse {
    pub name: String,
    pub phases: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StartLine {
    Starter {
        entry: u64,
        /// The rider (a rider's other horses in the class have the same).
        person: u64,
        position: u32,
        start_number: Option<u32>,
        time: Option<String>,
        rider: String,
        town: String,
        club: String,
        horse: String,
        not_starting: bool,
        hors_concours: bool,
        /// Penalties brought from the qualifier (U9); `carried_out`: out of the qualifier.
        carried: Option<Decimal>,
        carried_out: bool,
        /// What more the office can show in columns (only what is filled): see [`more_of`].
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        more: BTreeMap<&'static str, String>,
    },
    Team {
        team: String,
        name: String,
        time: Option<String>,
        /// Every rider of the team does not start.
        not_starting: bool,
    },
    /// A line of text; levels and breaks also as what they are (for prints, in their language).
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        level: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pause: Option<(String, String)>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct StartListView {
    pub class: ClassRow,
    /// Before start numbers are given the list is published by name; the office sees the drawn
    /// order (`lines`) and this note.
    pub published_by_name: bool,
    pub start_numbers: bool,
    pub start_times: bool,
    pub lines: Vec<StartLine>,
    /// The qualifier's number, when the riders bring its penalties (U9).
    pub qualifier: Option<String>,
    /// How it is laid out (blocks, start times and breaks, a line after the n-th starter).
    pub setup: StartListSetup,
    /// Entries of the class on none of its start lists (Rosson's "Ohne Prüfung").
    pub unplaced: Vec<Unplaced>,
    /// Dressage: how long the class's test lasts, as written on it ("4'00"), for the time between
    /// starts.
    pub programme_time: Option<String>,
    /// The test's longest duration in whole minutes (dressage): the time between starts.
    pub programme_minutes: Option<u32>,
}

/// An entry on no start list.
#[derive(Debug, Clone, Serialize)]
pub struct Unplaced {
    pub entry: u64,
    pub rider: String,
    pub horse: String,
}

fn unplaced(state: &State, published: &PublishedClassId) -> Vec<Unplaced> {
    let mut list: Vec<Unplaced> = crate::entries::unplaced(state, published)
        .iter()
        .filter_map(|id| state.entries.get(id))
        .map(|e| Unplaced {
            entry: e.id.0,
            rider: state.persons.get(&e.rider).map(|p| format!("{} {}", p.first_name, p.last_name)).unwrap_or_default(),
            horse: e.guest_horse.clone().or_else(|| e.horse.as_ref().and_then(|h| state.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default(),
        })
        .collect();
    list.sort_by(|a, b| scoring::ranking::compare_names(&a.rider, &b.rider));
    list
}

#[derive(Debug, Clone, Serialize)]
pub struct RankingLine {
    pub id: String,
    pub team: bool,
    pub rank: Option<u32>,
    pub manual: bool,
    /// A Tag der Jugend final: tied in ranks 1–3 after the tie movements, lots are drawn (S38).
    pub draw: bool,
    pub placed: bool,
    pub rider: String,
    pub horse: String,
    pub club: String,
    pub faults: Option<Decimal>,
    pub time: Option<Decimal>,
    pub percentage: Option<Decimal>,
    pub judges: BTreeMap<String, Decimal>,
    /// … and each judge's percentage (of one sheet's maximum), by position too.
    pub judge_percentages: BTreeMap<String, Decimal>,
    /// Freestyle and Children: parts A and B, in percent.
    pub part_a: Option<Decimal>,
    pub part_b: Option<Decimal>,
    /// Team cones (OKV Fahrcup): the team's rank points.
    pub rank_points: Option<u32>,
    /// Out of the class: why, as Swiss Equestrian names it (`ausgeschieden`, `aufgegeben`…).
    pub out: Option<String>,
}

/// A reason for being out, as Swiss Equestrian names it (`ausgeschieden`, `aufgegeben`…).
pub fn reason_name(state: &State, code: Option<u32>) -> Option<String> {
    code.and_then(|c| state.reasons.get(&c)).map(|r| r.name.clone()).filter(|n| !n.is_empty())
}

#[derive(Debug, Clone, Serialize)]
pub struct RankingView {
    pub class: ClassRow,
    pub placed_count: usize,
    pub lines: Vec<RankingLine>,
    /// Dressage: the judges' positions in the class's order (E H C M B; 3/2 teams H C M E B).
    pub judge_order: Vec<String>,
}

pub fn event(state: &State) -> Option<EventView> {
    state.event.as_ref().map(|e| EventView {
        id: e.id.0,
        number: e.number.clone(),
        title: e.title.clone(),
        place: e.place.clone(),
        from: e.from.clone(),
        to: e.to.clone(),
        organiser: e.organiser.clone(),
        disciplines: e.disciplines.clone(),
        fixed_numbers: e.fixed_start_numbers,
        numbers_assigned: e.start_numbers_assigned,
    })
}

/// Results in a show: a rider's result in a class once, however many judges' sheets it has
/// (Rosson's count of result records, overall rows aside).
fn results_count(state: &State) -> usize {
    let mut riders: std::collections::BTreeSet<(&ClassId, &EntryId)> = state.results.keys().map(|(c, e)| (c, e)).collect();
    riders.extend(state.sheets.keys().map(|(c, e, _)| (c, e)));
    let mut teams: std::collections::BTreeSet<(&ClassId, &TeamId)> = state.team_results.keys().map(|(c, t)| (c, t)).collect();
    teams.extend(state.team_sheets.keys().map(|(c, t, _)| (c, t)));
    riders.len() + teams.len()
}

pub fn summary(id: &str, state: &State, rehearsal: bool, training: bool) -> ShowSummary {
    ShowSummary { id: id.into(), event: event(state), entries: state.entries.len(), classes: state.classes.len(), results: results_count(state), rehearsal, training }
}

/// "First Last" of the rider as on the entry (else the person record).
pub(crate) fn rider(state: &State, entry: &Entry) -> (String, String, String) {
    match (&entry.rider_on_entry, state.persons.get(&entry.rider)) {
        (Some(r), _) => (format!("{} {}", r.first_name, r.last_name).trim().to_string(), r.town.clone(), r.club.clone()),
        (None, Some(p)) => (format!("{} {}", p.first_name, p.last_name).trim().to_string(), p.town.clone(), p.club.clone()),
        _ => (format!("#{}", entry.rider), String::new(), String::new()),
    }
}

pub(crate) fn horse(state: &State, entry: &Entry) -> String {
    if let Some(name) = &entry.guest_horse {
        return name.clone();
    }
    entry.horses_on_entry.first().and_then(|h| h["pferdename"].as_str()).map(str::to_string).or_else(|| entry.horse.as_ref().and_then(|h| state.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default()
}

fn status(s: ClassStatus) -> &'static str {
    match s {
        ClassStatus::Planned => "planned",
        ClassStatus::Running => "running",
        ClassStatus::Final => "final",
    }
}

/// What lists and prints call a class: its own name (« Prix … »), or, without one, its category
/// and scoring table (« R/N120 · A au chrono »).
pub fn class_title(state: &State, class: &Class) -> String {
    let name = class.name.trim();
    if !name.is_empty() {
        return name.to_string();
    }
    let published = state.published_classes.get(&class.published);
    let category = published.map(|p| p.category_text.trim()).filter(|c| !c.is_empty()).or_else(|| published.map(|p| p.category.as_str())).unwrap_or_default();
    let scoring = class.scoring_text.as_deref().or_else(|| published.and_then(|p| p.scoring_text.as_deref())).unwrap_or_default().trim();
    [category, scoring].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" · ")
}

pub fn class_row(state: &State, class: &Class) -> ClassRow {
    let published = state.published_classes.get(&class.published);
    let starts = state.starts.get(&class.id).map(Vec::as_slice).unwrap_or(&[]);
    let not_starting = starts.iter().filter(|s| state.entries.get(&s.entry).is_some_and(|e| e.status == EntryStatus::NotStarting)).count();
    let results = starts.iter().filter(|s| state.results.contains_key(&(class.id.clone(), s.entry.clone())) || state.sheets.range((class.id.clone(), s.entry.clone(), AssignmentId(String::new()))..).next().is_some_and(|((c, e, _), _)| c == &class.id && e == &s.entry)).count();
    // Too many entries (S19): the sections proposed, the number exceeded, whether it must be divided.
    let size: Option<(usize, usize, bool)> = if state.class_discipline(class) == "CD" && class.teams.is_empty() {
        // Dressage: hors concours rides do not count (Dressurreglement 5.2 (1)).
        match starts.iter().filter(|s| state.entries.get(&s.entry).is_some_and(|e| !e.hors_concours)).count() {
            n if n > DRESSAGE_MAX => Some((n.div_ceil(DRESSAGE_SERIES), DRESSAGE_MAX, true)),
            n if n > DRESSAGE_SERIES => Some((2, DRESSAGE_SERIES, false)),
            _ => None,
        }
    } else {
        // Team classes count teams, not riders.
        let n = if class.teams.is_empty() { starts.len() } else { class.teams.len() };
        (n > MAX_ENTRIES).then(|| (n.div_ceil(MAX_ENTRIES), MAX_ENTRIES, true))
    };
    ClassRow {
        id: class.id.0.clone(),
        published: class.published.0,
        number: class.number.clone(),
        name: class.name.clone(),
        title: class_title(state, class),
        date: class.date.clone(),
        time: class.time_text.clone(),
        discipline: state.class_discipline(class).to_string(),
        category: published.map(|p| p.category_text.clone()).unwrap_or_default(),
        scoring: class.scoring_text.clone().or_else(|| published.and_then(|p| p.scoring_text.clone())).unwrap_or_default(),
        starters: starts.len(),
        not_starting,
        results,
        status: status(state.status_of(class)),
        start_lists_sent: state.uploaded.get(&Upload::StartLists(class.published.clone())).map(|s| s.millis),
        results_sent: state.uploaded.get(&Upload::Results(class.id.clone())).map(|s| s.millis),
        qualifier: class.options.qualifier.as_ref().map(|q| q.0.clone()),
        results_changed: changed_since(state.results_changed.get(&class.id), state.uploaded.get(&Upload::Results(class.id.clone()))),
        start_lists_changed: changed_since(state.start_lists_changed.get(&class.published), state.uploaded.get(&Upload::StartLists(class.published.clone()))),
        numbered: starts.iter().any(|s| s.start_number.is_some()),
        unplaced: crate::entries::unplaced(state, &class.published).len(),
        divide_into: size.map(|(n, _, _)| n),
        over: size.map_or(MAX_ENTRIES, |(_, over, _)| over),
        must_divide: size.is_some_and(|(_, _, must)| must),
        max_starters: class.options.max_starters,
        phase: class.phase.as_ref().map(|p| p.kind),
        phase_of: class.phase.as_ref().map(|p| p.of.0.clone()),
        phases: class.phases.iter().map(|p| p.0.clone()).collect(),
    }
}

/// Sent, and changed after.
fn changed_since(changed: Option<&u64>, sent: Option<&Sent>) -> bool {
    sent.is_some_and(|s| changed.is_some_and(|m| *m > s.millis))
}

pub fn show(id: &str, state: &State) -> ShowView {
    let mut classes: Vec<&Class> = state.classes.values().collect();
    classes.sort_by_key(|c| (c.date.clone(), c.position.unwrap_or(u32::MAX), c.number.trim_start_matches(|ch: char| !ch.is_ascii_digit()).split(|ch: char| !ch.is_ascii_digit()).next().and_then(|n| n.parse::<u32>().ok()).unwrap_or(u32::MAX), c.number.clone()));
    let mut days: Vec<Day> = Vec::new();
    for c in classes {
        let row = class_row(state, c);
        match days.last_mut() {
            Some(d) if d.date == row.date => d.classes.push(row),
            _ => days.push(Day { date: row.date.clone(), classes: vec![row] }),
        }
    }
    let mut not_set_up: Vec<PublishedRow> = state
        .published_classes
        .values()
        .filter(|p| !state.classes.values().any(|c| c.published == p.id))
        .map(|p| PublishedRow {
            id: p.id.0,
            number: p.number.clone(),
            date: p.date.clone(),
            discipline: p.discipline.clone(),
            category: p.category_text.clone(),
            scoring: p.scoring_text.clone().unwrap_or_default(),
            entries: state.entries.values().filter(|e| e.published == p.id).count(),
        })
        .collect();
    not_set_up.sort_by_key(|p| (p.date.clone(), p.number.parse::<u32>().unwrap_or(u32::MAX), p.number.clone()));
    let riders: std::collections::BTreeSet<&PersonId> = state.entries.values().map(|e| &e.rider).collect();
    let horses: std::collections::BTreeSet<&HorseId> = state.entries.values().filter_map(|e| e.horse.as_ref()).collect();
    let combined = state.combined.values().map(|r| combined_row(state, r)).collect();
    ShowView { id: id.into(), event: event(state), days, not_set_up, entries: state.entries.len(), riders: riders.len(), horses: horses.len(), combined }
}

/// A rider's text (a remark, a comment) as the office reads it. The test shows come from an
/// anonymised corpus (golden) where such texts are replaced by "[41 Zeichen entfernt]": that
/// says nothing, so nothing is shown (and a rule by comment is not fooled). Real shows never
/// have it.
pub(crate) fn said(text: &str) -> String {
    let t = text.trim();
    let removed = t.strip_prefix('[').and_then(|r| r.strip_suffix(" Zeichen entfernt]")).is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    if removed { String::new() } else { text.to_string() }
}

pub fn participants(state: &State) -> Vec<EntryRow> {
    // Where each entry starts.
    let mut starts: BTreeMap<&EntryId, Vec<StartRef>> = BTreeMap::new();
    for (class_id, list) in &state.starts {
        let Some(class) = state.classes.get(class_id) else { continue };
        // Each starter's position and start time, as the start list prints them.
        let printed: BTreeMap<EntryId, (u32, Option<show::model::TimeOfDay>)> = state
            .start_list(class_id)
            .map(|l| l.lines.into_iter().filter_map(|line| if let show::startlist::Line::Starter { entry, position, time } = line { Some((entry, (position, time))) } else { None }).collect())
            .unwrap_or_default();
        for s in list {
            let (position, time) = printed.get(&s.entry).map_or((None, None), |(p, t)| (Some(*p), *t));
            starts.entry(&s.entry).or_default().push(StartRef {
                class: class_id.0.clone(),
                number: class.number.clone(),
                order: s.order,
                start_number: s.start_number,
                date: class.date.clone(),
                time: time.map_or_else(|| class.time_text.trim().to_string(), |t| t.to_string()),
                position,
                phase: class.phase.as_ref().map(|p| p.kind),
            });
        }
    }
    // Entries starting in a final class; entries with a result or marks.
    let final_entries: std::collections::BTreeSet<EntryId> = state.starts.iter().filter(|(c, _)| state.classes.get(*c).is_some_and(|c| c.status == ClassStatus::Final)).flat_map(|(_, list)| list.iter().map(|s| s.entry.clone())).collect();
    let ridden: std::collections::BTreeSet<EntryId> = state.results.keys().map(|(_, e)| e.clone()).chain(state.sheets.keys().map(|(_, e, _)| e.clone())).collect();
    // In the order they ride: by day and time, then by class.
    for list in starts.values_mut() {
        list.sort_by(|a, b| (&a.date, &a.time, &a.number).cmp(&(&b.date, &b.time, &b.number)));
    }
    let mut rows: Vec<EntryRow> = state
        .entries
        .values()
        .map(|e| {
            let (name, town, club) = rider(state, e);
            let person = state.persons.get(&e.rider);
            let published = state.published_classes.get(&e.published);
            EntryRow {
                id: e.id.0,
                rider_id: e.rider.0,
                rider: name,
                town,
                club,
                nationality: person.map(|p| p.nationality.clone()).unwrap_or_default(),
                licences: person.map(|p| p.licences.clone()).unwrap_or_default(),
                youth: person.map(|p| p.youth.clone()).unwrap_or_default(),
                horse_id: e.horse.as_ref().map(|h| h.0),
                horse: horse(state, e),
                guest_horse: e.guest_horse.is_some(),
                published: e.published.0,
                class_number: published.map(|p| p.number.clone()).unwrap_or_default(),
                category: published.map(|p| p.category_text.clone()).unwrap_or_default(),
                discipline: published.map(|p| p.discipline.clone()).unwrap_or_default(),
                team: e.team.as_ref().map(|t| state.teams.get(t).map(|t| t.name.clone()).unwrap_or_else(|| t.0.clone())),
                not_starting: e.status == EntryStatus::NotStarting,
                hors_concours: e.hors_concours,
                waiting_list: e.waiting_list,
                organiser_place: e.organiser_place,
                se_status: e.se_status.clone(),
                fee_paid: e.fee_paid,
                rider_changed: e.rider_changed,
                comment: said(&e.comment),
                remark: state.entry_notes.get(&e.id).map(|n| said(&n.remarks)).unwrap_or_default(),
                stage: e.stage,
                levels: published.map(|p| p.levels).unwrap_or(1),
                starts: starts.remove(&e.id).unwrap_or_default(),
                driving_phases: published.filter(|p| p.discipline == "CA").map(|p| p.driving_phases.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
                driving_horses: if published.is_some_and(|p| p.discipline == "CA") {
                    e.horses_on_entry
                        .iter()
                        .map(|h| DrivingHorse { name: h["pferdename"].as_str().unwrap_or_default().to_string(), phases: h["einsaetze"].as_array().into_iter().flatten().filter_map(|p| p.as_str().map(str::to_string)).collect() })
                        .collect()
                } else {
                    Vec::new()
                },
                locked: final_entries.contains(&e.id),
                ridden: ridden.contains(&e.id),
            }
        })
        .collect();
    rows.sort_by(|a, b| (a.rider.to_lowercase(), &a.class_number, a.id).cmp(&(b.rider.to_lowercase(), &b.class_number, b.id)));
    rows
}

/// A class's start list as the office works on it: in the drawn order, also while it is
/// published by name (before start numbers).
pub fn start_list(state: &State, class_id: &ClassId) -> Result<StartListView, String> {
    start_list_as(state, class_id, false)
}

/// A class's start list as published and printed: before start numbers, by name.
pub fn published_start_list(state: &State, class_id: &ClassId) -> Result<StartListView, String> {
    start_list_as(state, class_id, true)
}

fn start_list_as(state: &State, class_id: &ClassId, as_published: bool) -> Result<StartListView, String> {
    let class = state.classes.get(class_id).ok_or_else(|| format!("unknown class {class_id}"))?;
    let mut list = state.start_list(class_id)?;
    let carried = state.carried(class_id).unwrap_or_default();
    let starts = state.starts.get(class_id).map(Vec::as_slice).unwrap_or(&[]);
    // Preparing: the drawn order, not the published one (by name until numbers are given).
    let published_by_name = !list.start_numbers && class.teams.is_empty();
    if published_by_name && !as_published {
        let order = |l: &Line| match l {
            Line::Starter { entry, .. } => starts.iter().find(|s| &s.entry == entry).map_or(u32::MAX, |s| s.order),
            _ => 0,
        };
        list.lines.retain(|l| matches!(l, Line::Starter { .. }));
        list.lines.sort_by_key(order);
        for (i, l) in list.lines.iter_mut().enumerate() {
            if let Line::Starter { position, .. } = l {
                *position = i as u32 + 1;
            }
        }
    }
    let lines = list
        .lines
        .iter()
        .map(|line| match line {
            Line::Starter { entry, position, time } => {
                let e = &state.entries[entry];
                let (name, town, club) = rider(state, e);
                StartLine::Starter {
                    entry: entry.0,
                    person: e.rider.0,
                    position: *position,
                    start_number: starts.iter().find(|s| &s.entry == entry).and_then(|s| s.start_number),
                    time: time.map(|t| t.to_string()),
                    rider: name,
                    town,
                    club,
                    horse: horse(state, e),
                    not_starting: e.status == EntryStatus::NotStarting,
                    hors_concours: e.hors_concours,
                    carried: carried.get(entry).and_then(|r| r.faults),
                    carried_out: carried.get(entry).is_some_and(|r| r.rank.is_none()),
                    more: more_of(state, e),
                }
            }
            Line::Team { team, time } => {
                let mut riders = state.entries.values().filter(|e| e.team.as_ref() == Some(team) && !e.waiting_list).peekable();
                let not_starting = riders.peek().is_some() && riders.all(|e| e.status == EntryStatus::NotStarting);
                StartLine::Team { team: team.0.clone(), name: state.teams.get(team).map(|t| t.name.clone()).unwrap_or_default(), time: time.map(|t| t.to_string()), not_starting }
            }
            Line::Block(text) | Line::Inserted(text) => StartLine::Text { text: text.clone(), level: None, pause: None },
            Line::Level(level) => StartLine::Text { text: format!("Level {level}"), level: Some(level.to_string()), pause: None },
            Line::Break(from, to) => StartLine::Text { text: format!("Break {from} – {to}"), level: None, pause: Some((from.to_string(), to.to_string())) },
        })
        .collect();
    let qualifier = class.options.qualifier.as_ref().and_then(|q| state.classes.get(q)).map(|q| q.number.clone());
    Ok(StartListView { class: class_row(state, class), published_by_name, start_numbers: list.start_numbers, start_times: list.start_times, lines, qualifier, setup: class.start_list.clone(), unplaced: unplaced(state, &class.published), programme_time: programme_time(state, class), programme_minutes: programme_minutes(state, class) })
}

/// What more a start list can show of a starter, as Rosson's start list table can (the office
/// picks the columns): the rider (licence, nationality, youth, number), the horse as Swiss
/// Equestrian delivered it (number, pony size, signalement, owner, breeder, pedigree), the entry
/// (level, winning points, fee paid, status, team), the rider's remark and the office's note.
fn more_of(state: &State, e: &Entry) -> BTreeMap<&'static str, String> {
    let mut more = BTreeMap::new();
    let mut put = |key: &'static str, value: String| {
        let value = value.trim().to_string();
        if !value.is_empty() {
            more.insert(key, value);
        }
    };
    if let Some(p) = state.persons.get(&e.rider) {
        put("licence", p.licences.clone());
        put("nationality", p.nationality.clone());
        put("youth", p.youth.clone());
    }
    // To sort by the rider's last name (the name is shown first name first).
    let (first, last) = match (&e.rider_on_entry, state.persons.get(&e.rider)) {
        (Some(r), _) => (r.first_name.clone(), r.last_name.clone()),
        (None, Some(p)) => (p.first_name.clone(), p.last_name.clone()),
        _ => (String::new(), String::new()),
    };
    put("rider_sort", format!("{last} {first}"));
    put("rider_id", e.rider.0.to_string());
    if e.guest_horse.is_none() {
        put("horse_id", e.horse.as_ref().filter(|h| h.0 != se_api::load::PLACEHOLDER_HORSE).map(|h| h.0.to_string()).unwrap_or_default());
    }
    put("pony", crate::prepare::pony_size(e).unwrap_or_default());
    if let Some(h) = e.horses_on_entry.first() {
        let text = |k: &str| h[k].as_str().unwrap_or_default().to_string();
        put("signalement", text("pferd_signalement"));
        put("owner", text("pferd_besitzer"));
        put("breeder", text("pferd_zuechter"));
        put("sire", text("vater_name"));
        put("dam", text("mutter_name"));
        put("dam_sire", text("vater_der_mutter_name"));
        if h["hat_pony_startbewilligung"].as_bool() == Some(true) {
            put("pony_permit", "✓".into());
        }
    }
    // Driving: the turnout and the carriage's width.
    let number = |v: &Value| v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|n| n.to_string())).unwrap_or_default();
    put("turnout", number(&e.record["fahren_gespann"]));
    put("width", number(&e.record["wagenbreite"]));
    put("level", e.stage.map(|s| s.to_string()).unwrap_or_default());
    put("points_pair", e.winning_points.pair.map(|d| d.normalize().to_string()).unwrap_or_default());
    put("points_horse", e.winning_points.horse.map(|d| d.normalize().to_string()).unwrap_or_default());
    put("points_rider", e.winning_points.rider.map(|d| d.normalize().to_string()).unwrap_or_default());
    put("fee", e.fee_paid.map(|d| d.normalize().to_string()).unwrap_or_default());
    put("status", e.se_status.clone());
    put("team", e.team.as_ref().and_then(|t| state.teams.get(t)).map(|t| t.name.clone()).unwrap_or_default());
    // Remarks: the rider's, made with the entry (`bemerkungen`; on devices with the show's
    // private data), and the office's note; shown together, each marked.
    put("remark", state.entry_notes.get(&e.id).map(|n| said(&n.remarks)).unwrap_or_default());
    put("note", said(&e.comment));
    more
}

pub fn ranking(state: &State, class_id: &ClassId) -> Result<RankingView, String> {
    let class = state.classes.get(class_id).ok_or_else(|| format!("unknown class {class_id}"))?;
    let ranking = state.ranking(class_id)?;
    // Team classes: each team, then its riders (team dressage: the riders without a rank, as
    // Rosson's).
    let team_dressage = matches!(state.scoring_input(class_id), show::input::ScoringInput::DressageTeams { .. });
    // A judge by position (E, H, C, M, B) where known.
    let letter = |k: &String| state.assignments.get(&AssignmentId(k.clone())).and_then(|a| a.letter.clone()).unwrap_or_else(|| k.clone());
    let mut ordered: Vec<&show::ranking::Line> = Vec::new();
    for l in &ranking.lines {
        if l.team {
            ordered.push(l);
            let team = Some(TeamId(l.id.clone()));
            ordered.extend(ranking.lines.iter().filter(|r| !r.team && r.id.parse().ok().and_then(|id| state.entries.get(&EntryId(id))).is_some_and(|e| e.team == team)));
        } else if !ranking.lines.iter().any(|t| t.team) {
            ordered.push(l);
        }
    }
    let lines = ordered
        .into_iter()
        .map(|l| {
            let (rider_name, horse_name, club) = if l.team {
                (state.teams.get(&TeamId(l.id.clone())).map(|t| t.name.clone()).unwrap_or_else(|| l.id.clone()), String::new(), String::new())
            } else {
                match l.id.parse().ok().and_then(|id| state.entries.get(&EntryId(id))) {
                    Some(e) => {
                        let (name, _, club) = rider(state, e);
                        (name, horse(state, e), club)
                    }
                    None => (l.id.clone(), String::new(), String::new()),
                }
            };
            RankingLine {
                id: l.id.clone(),
                team: l.team,
                rank: if team_dressage && !l.team { None } else { l.placement.rank() },
                placed: l.placed && (l.team || !team_dressage),
                manual: l.manual,
                draw: l.draw,
                rider: rider_name,
                horse: horse_name,
                club,
                faults: l.faults,
                time: l.time,
                percentage: l.percentage,
                judges: l.judge_totals.iter().map(|(k, v)| (letter(k), *v)).collect(),
                judge_percentages: l.judge_percentages.iter().map(|(k, v)| (letter(k), *v)).collect(),
                part_a: l.part_a,
                part_b: l.part_b,
                rank_points: l.rank_points,
                out: reason_name(state, if l.team { state.team_results.get(&(class_id.clone(), TeamId(l.id.clone()))).and_then(|c| c.out) } else { l.id.parse().ok().and_then(|id| state.out_reason(class_id, &EntryId(id))) }),
            }
        })
        .collect();
    let judge_order = state.dressage_judges(class_id).into_iter().filter_map(|a| a.letter.clone()).collect();
    Ok(RankingView { class: class_row(state, class), placed_count: ranking.placed_count, lines, judge_order })
}

/// A horse the show knows, for picking another horse.
#[derive(Debug, Clone, Serialize)]
pub struct HorseRow {
    pub id: u64,
    pub name: String,
    pub signalement: String,
    pub owner: String,
}

pub fn horses(state: &State) -> Vec<HorseRow> {
    let mut rows: Vec<HorseRow> = state.horses.values().map(|h| HorseRow { id: h.id.0, name: h.name.clone(), signalement: h.signalement.clone(), owner: h.owner.clone() }).collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

/// A class as a column of the officials table.
#[derive(Debug, Clone, Serialize)]
pub struct ClassHead {
    pub id: String,
    pub number: String,
    pub discipline: String,
    /// Dressage judges' positions, as the result lists show them: E H C M B (driving's too).
    pub letters: String,
    /// A driving class (its judges at the same letters; no youth day's judge).
    pub driving: bool,
    pub category: String,
    pub time: String,
    pub scoring: String,
    pub locked: bool,
}

/// An official's role in a class.
#[derive(Debug, Clone, Serialize)]
pub struct Cell {
    pub role: String,
    pub letter: Option<String>,
    /// Marks are entered under this role (a judge's sheets): it cannot be taken away.
    pub marked: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct OfficialRow {
    pub person: u64,
    pub name: String,
    pub town: String,
    /// Class id → role.
    pub cells: BTreeMap<String, Cell>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OfficialsDay {
    pub date: String,
    pub classes: Vec<ClassHead>,
    pub rows: Vec<OfficialRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleView {
    pub code: String,
    pub de: String,
    pub fr: String,
    /// Swiss Equestrian's Italian (French when it has none).
    pub it: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OfficialsView {
    pub days: Vec<OfficialsDay>,
    /// Roles in a class.
    pub roles: Vec<RoleView>,
}

fn person_name(state: &State, id: &PersonId) -> (String, String) {
    match (state.persons.get(id), state.officials.get(id)) {
        (Some(p), _) => (format!("{} {}", p.first_name, p.last_name), p.town.clone()),
        (None, Some(o)) => (format!("{} {}", o.first_name, o.last_name), o.town.clone()),
        _ => (format!("#{id}"), String::new()),
    }
}

/// A published class as Rosson's info box shows it: what Swiss Equestrian published (category,
/// remarks, fee, scoring), what the office may change (the category as printed, the levels in
/// jumping), and its entries counted by status.
pub fn published_info(state: &State, id: &PublishedClassId) -> Value {
    let Some(p) = state.published_classes.get(id) else { return Value::Null };
    let entries: Vec<&Entry> = state.entries.values().filter(|e| e.published == *id).collect();
    let count = |f: &dyn Fn(&Entry) -> bool| entries.iter().filter(|e| f(e)).count();
    json!({
        "id": p.id.0,
        "number": p.number,
        "discipline": p.discipline,
        "category": p.category,
        "category_text": p.category_text,
        "levels": p.levels,
        // Rosson: levels in jumping and combined classes, not dressage nor driving.
        "has_levels": !matches!(p.discipline.as_str(), "CD" | "CA" | "F"),
        "description": p.description,
        "fee": p.fee,
        "scoring_text": p.scoring_text,
        "team_size": p.team_size,
        "counts": {
            "total": entries.len(),
            "hors_concours": count(&|e| e.hors_concours),
            "not_starting": count(&|e| e.status == EntryStatus::NotStarting),
            "withdrawn": count(&|e| e.se_status == "abgemeldet"),
            "wrong": count(&|e| e.se_status == "falschnennung"),
            "waiting_list": count(&|e| e.waiting_list),
            "organiser_places": count(&|e| e.organiser_place),
        },
    })
}

/// Officials and their roles, show day by show day: the officials as rows, the classes of the
/// day as columns (as JuryApp shows a jury).
pub fn officials(state: &State) -> OfficialsView {
    let shown = show("", state);
    let rank = |role: &str| ["P", "R", show::model::STYLE_JUDGE, "DRI", "DRITdj", "RW", "A"].iter().position(|r| *r == role).unwrap_or(9);
    let days = shown
        .days
        .iter()
        .map(|day| {
            let classes: Vec<ClassHead> = day
                .classes
                .iter()
                .map(|c| ClassHead { letters: "EHCMB".into(), driving: state.classes.get(&ClassId(c.id.clone())).is_some_and(|k| state.driving(k)), id: c.id.clone(), number: c.number.clone(), discipline: c.discipline.clone(), category: c.category.clone(), time: c.time.clone(), scoring: c.scoring.clone(), locked: c.status == "final" })
                .collect();
            let mut rows: BTreeMap<PersonId, OfficialRow> = BTreeMap::new();
            for a in state.assignments.values() {
                let Some(class) = a.class.as_ref().filter(|c| classes.iter().any(|h| h.id == c.0)) else { continue };
                let row = rows.entry(a.person.clone()).or_insert_with(|| {
                    let (name, town) = person_name(state, &a.person);
                    OfficialRow { person: a.person.0, name, town, cells: BTreeMap::new() }
                });
                row.cells.insert(class.0.clone(), Cell { role: a.role.clone(), letter: a.letter.clone(), marked: crate::prepare::has_marks(state, &a.id) });
            }
            let mut rows: Vec<OfficialRow> = rows.into_values().collect();
            rows.sort_by_key(|r| (r.cells.values().map(|c| rank(&c.role)).min().unwrap_or(9), r.name.clone()));
            OfficialsDay { date: day.date.clone(), classes, rows }
        })
        .collect();
    let roles = state.class_roles().iter().map(|r| RoleView { code: r.code.clone(), de: r.name_de.clone(), fr: r.name_fr.clone(), it: if r.name_it.trim().is_empty() { r.name_fr.clone() } else { r.name_it.clone() } }).collect();
    OfficialsView { days, roles }
}

/// Someone who can be given a role: from the national list of officials, or a person of the show.
#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub person: u64,
    pub name: String,
    pub town: String,
    /// What the official is (Juge national de saut…), in Swiss Equestrian's order.
    pub titles: Vec<TitleView>,
}

/// A title of an official in the three languages.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TitleView {
    pub de: String,
    pub fr: String,
    pub it: String,
}

/// An official's titles as Swiss Equestrian says them (info.swiss-equestrian.ch, officials): in
/// French and Italian as delivered, else from its list of titles, else in German; in its order
/// (dressage, jumping, eventing, driving…), each once.
pub fn titles(o: &Official) -> Vec<TitleView> {
    let tidy = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut titles: Vec<(u32, usize, TitleView)> = Vec::new();
    for (i, de) in o.roles.iter().enumerate() {
        let de = tidy(de);
        if de.is_empty() || titles.iter().any(|(_, _, t)| t.de == de) {
            continue;
        }
        let listed = se_api::titles::title(&de);
        let say = |delivered: Option<&String>, listed: Option<&str>| delivered.map(|s| tidy(s)).filter(|s| !s.is_empty()).or(listed.map(tidy)).filter(|s| !s.is_empty()).unwrap_or_else(|| de.clone());
        let view = TitleView { fr: say(o.roles_fr.get(i), listed.map(|t| t.fr)), it: say(o.roles_it.get(i), listed.map(|t| t.it)), de };
        // Swiss Equestrian's order; 0 and none: after the others.
        titles.push((listed.and_then(|t| t.order).filter(|o| *o > 0).unwrap_or(u32::MAX), i, view));
    }
    titles.sort_by_key(|(order, i, _)| (*order, *i));
    titles.into_iter().map(|(_, _, t)| t).collect()
}

/// Officials and people of the show whose name, place or title (in any language) has every word
/// of the query; officials first.
pub fn find_officials(state: &State, query: &str) -> Vec<Candidate> {
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_string).collect();
    let matches = |text: &str| words.iter().all(|w| text.to_lowercase().contains(w.as_str()));
    let mut found: Vec<Candidate> = state
        .officials
        .values()
        .filter_map(|o| {
            let titles = titles(o);
            let said: Vec<String> = titles.iter().flat_map(|t| [t.de.as_str(), t.fr.as_str(), t.it.as_str()]).map(str::to_string).collect();
            matches(&format!("{} {} {} {}", o.first_name, o.last_name, o.town, said.join(" "))).then(|| Candidate { person: o.person.0, name: format!("{} {}", o.first_name, o.last_name), town: o.town.clone(), titles })
        })
        .collect();
    for p in state.persons.values().filter(|p| !state.officials.contains_key(&p.id)) {
        if matches(&format!("{} {} {}", p.first_name, p.last_name, p.town)) {
            found.push(Candidate { person: p.id.0, name: format!("{} {}", p.first_name, p.last_name), town: p.town.clone(), titles: Vec::new() });
        }
    }
    found.sort_by(|a, b| b.titles.len().min(1).cmp(&a.titles.len().min(1)).then(a.name.cmp(&b.name)));
    found.truncate(20);
    found
}

/// A programme as the office enters it: its sections of movements, its penalties.
#[derive(Debug, Clone, Serialize)]
pub struct ProgrammeView {
    pub id: u32,
    pub name: String,
    pub title_de: String,
    pub title_fr: String,
    /// Italian where Swiss Equestrian gives it, else German (here and below).
    pub title_it: String,
    pub max_points: Decimal,
    pub sections: Vec<SectionView>,
    pub penalties: Vec<PenaltyView>,
    /// `standard`, `freestyle` (Kür: parts A and B), `children` (one sheet for the judges) or
    /// `joint` (young horses: one sheet for the jury).
    pub kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct SectionView {
    pub title_de: String,
    pub title_fr: String,
    pub title_it: String,
    pub movements: Vec<MovementView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MovementView {
    /// The mark's key on the sheet.
    pub number: String,
    pub de: String,
    pub fr: String,
    pub it: String,
    pub coefficient: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct PenaltyView {
    pub de: String,
    pub fr: String,
    pub it: String,
    pub cost_de: String,
    pub cost_fr: String,
    pub cost_it: String,
    /// Points (standard) or percent (freestyle) taken off; none: e.g. elimination.
    pub points: Option<Decimal>,
    pub percent: Option<Decimal>,
}

/// Read a programme's published definition for entering it.
pub fn programme_view(def: &ProgrammeDef) -> Option<ProgrammeView> {
    let json: serde_json::Value = serde_json::from_str(&def.definition).ok()?;
    let text = |v: &serde_json::Value| v.as_str().unwrap_or("").to_string();
    // Italian, else German (Swiss Equestrian gives German always, Italian not always).
    let italian = |v: &serde_json::Value, key: &str| [format!("{key}_it"), format!("{key}_de")].iter().filter_map(|k| v[k].as_str()).map(str::trim).find(|t| !t.is_empty()).unwrap_or("").to_string();
    let dec = |v: &serde_json::Value| match v {
        serde_json::Value::Number(n) => scoring::number::parse(&n.to_string()).ok().flatten(),
        serde_json::Value::String(s) => scoring::number::parse(s).ok().flatten(),
        _ => None,
    };
    let mut sections = Vec::new();
    let mut penalties = Vec::new();
    for t in json["tabellen"].as_array()? {
        let layout = t["layout"].as_str().unwrap_or("");
        let fields = t["felder"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        if layout == "strafpunkte" {
            penalties.extend(fields.iter().map(|f| PenaltyView {
                de: text(&f["beschreibung_de"]),
                fr: text(&f["beschreibung_fr"]),
                it: italian(f, "beschreibung"),
                cost_de: text(&f["strafe_de"]),
                cost_fr: text(&f["strafe_fr"]),
                cost_it: italian(f, "strafe"),
                points: dec(&f["strafpunkte"]).filter(|d| !d.is_zero()),
                percent: dec(&f["strafprozente"]).filter(|d| !d.is_zero()),
            }));
            continue;
        }
        if layout == "lektionen_ohne_noten" {
            continue;
        }
        let movements = fields
            .iter()
            .map(|f| MovementView {
                number: match &f["nummer"] {
                    serde_json::Value::Number(n) => n.to_string(),
                    other => text(other),
                },
                de: movement_text(&f["thema"], "de"),
                fr: movement_text(&f["thema"], "fr"),
                it: Some(movement_text(&f["thema"], "it")).filter(|t| !t.is_empty()).unwrap_or_else(|| movement_text(&f["thema"], "de")),
                coefficient: dec(&f["koeffizient"]).filter(|c| !c.is_zero()).unwrap_or(Decimal::ONE),
            })
            .collect();
        sections.push(SectionView { title_de: text(&t["titel_de"]), title_fr: text(&t["titel_fr"]), title_it: italian(t, "titel"), movements });
    }
    let kind = if json["kuer"] == true {
        "freestyle"
    } else if json["children"] == true {
        "children"
    } else if show::master::judged_jointly(def) {
        // Young horses: one sheet for the jury (S39).
        "joint"
    } else {
        "standard"
    };
    Some(ProgrammeView { id: def.id, name: def.name.clone(), title_de: text(&json["titel_de"]), title_fr: text(&json["titel_fr"]), title_it: italian(&json, "titel"), max_points: def.max_points, sections, penalties, kind })
}

/// A movement's text: one theme, or its steps, each at a letter of the arena
/// ("A Entrer au trot de travail · X Arrêt, salut").
fn movement_text(thema: &serde_json::Value, language: &str) -> String {
    let one = |t: &serde_json::Value| {
        let position = t[format!("position_{language}")].as_str().unwrap_or("").trim();
        let text = t[format!("thema_{language}")].as_str().unwrap_or("").trim();
        if position.is_empty() { text.to_string() } else { format!("{position} {text}") }
    };
    match thema {
        serde_json::Value::Array(steps) => steps.iter().map(one).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "),
        other => one(other),
    }
}

/// The penalties ticked on a sheet, as places in the programme's penalty list.
fn ticked(sheet: &JudgeSheet, penalties: &[PenaltyView]) -> Vec<usize> {
    let mut used = vec![false; penalties.len()];
    let mut out = Vec::new();
    for v in &sheet.penalty_points {
        if let Some(i) = penalties.iter().enumerate().position(|(i, p)| !used[i] && p.points == Some(*v)) {
            used[i] = true;
            out.push(i);
        }
    }
    for v in &sheet.penalty_percent {
        if let Some(i) = penalties.iter().enumerate().position(|(i, p)| !used[i] && p.percent == Some(*v)) {
            used[i] = true;
            out.push(i);
        }
    }
    // The exclusion box (no value, S40).
    if sheet.excluded {
        out.extend(penalties.iter().enumerate().filter(|(_, p)| p.points.is_none() && p.percent.is_none()).map(|(i, _)| i));
    }
    out.sort();
    out
}

/// A rider's penalties: one set for every judge (S40) — what any judge's sheet has ticked.
fn rider_penalties(sheets: &BTreeMap<String, SheetView>) -> Vec<usize> {
    let mut all: Vec<usize> = sheets.values().flat_map(|s| s.penalties.iter().copied()).collect();
    all.sort();
    all.dedup();
    all
}

#[derive(Debug, Clone, Serialize)]
pub struct SheetView {
    pub marks: BTreeMap<String, Decimal>,
    /// Places in the programme's penalty list.
    pub penalties: Vec<usize>,
    /// Every movement has its mark (classes with totals: the total is entered).
    pub complete: bool,
    pub total: Option<Decimal>,
    pub percent: Option<Decimal>,
    /// The judge's total as entered (classes where the judges give totals only).
    pub entered: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JudgeView {
    pub assignment: String,
    pub letter: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SheetRider {
    pub entry: u64,
    pub position: u32,
    pub start_number: Option<u32>,
    pub rider: String,
    pub horse: String,
    pub not_starting: bool,
    /// The programme the rider rides (classes where every rider chooses one).
    pub programme: Option<u32>,
    /// Judge (assignment id) → sheet.
    pub sheets: BTreeMap<String, SheetView>,
    /// The penalties ticked, one set for every judge (places in the programme's list; S40).
    pub penalties: Vec<usize>,
    pub percentage: Option<Decimal>,
    /// Freestyle: technical (A) and artistic (B) percentages; Children: notes A and B.
    pub part_a: Option<Decimal>,
    pub part_b: Option<Decimal>,
    /// Why out of the class (eliminated, retired…), as the office entered it.
    pub out: Option<u32>,
    pub rank: Option<u32>,
    /// Team dressage: the team (the row is the team's; `entry` is 0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    /// Team dressage 3/2: the rider each of H, C and M judges (judge assignment id → name).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub judge_riders: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AsideView {
    pub slot: u32,
    pub marks: usize,
    /// Its first marks, in the programme's order: to know the paper sheet it is.
    pub first: Vec<Decimal>,
    /// The judge's total, when the judges give a total only.
    pub total: Option<Decimal>,
}

/// A cut sheet (put aside) as the dressage screen lists it.
fn aside_view(slot: u32, sheet: &show::model::JudgeSheet) -> AsideView {
    let mut marks: Vec<(&String, &Decimal)> = sheet.marks.iter().collect();
    // Movements by number ("2" before "10"), anything else after them.
    marks.sort_by_key(|(k, _)| (k.parse::<u32>().unwrap_or(u32::MAX), (*k).clone()));
    AsideView { slot, marks: sheet.marks.len(), first: marks.iter().take(6).map(|(_, v)| **v).collect(), total: sheet.total }
}

#[derive(Debug, Clone, Serialize)]
pub struct SheetsView {
    pub class: ClassRow,
    pub programmes: BTreeMap<u32, ProgrammeView>,
    pub judges: Vec<JudgeView>,
    /// The reasons for being out the office can choose.
    pub reasons: Vec<ReasonView>,
    pub riders: Vec<SheetRider>,
    pub aside: Vec<AsideView>,
    /// The judges give a total each, not a mark per movement.
    pub totals_per_judge: bool,
    /// Team dressage: the rows are the teams.
    pub teams: bool,
    /// Driving dressage: the riders' totals on 160 (lowest first), not percentages.
    pub driving: bool,
}

/// Dressage: the judges' sheets of a class, rider by rider in start order.
/// Driving's multitest as the office enters it: Rosson's programme (B1 or B4: items with their
/// coefficients), the judges, and per starter each judge's marks and penalties A (dressage),
/// B (per obstacle), C (ground test), the judge's total and the judges' mean.
#[derive(Debug, Clone, Serialize)]
pub struct MultitestView {
    pub class: ClassRow,
    pub programme: String,
    pub items: Vec<MultitestItem>,
    pub judges: Vec<JudgeView>,
    pub reasons: Vec<ReasonView>,
    pub lines: Vec<MultitestLine>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultitestItem {
    pub number: String,
    pub coefficient: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultitestLine {
    pub entry: u64,
    pub position: u32,
    pub start_number: Option<u32>,
    pub rider: String,
    pub horse: String,
    pub not_starting: bool,
    pub out: Option<u32>,
    /// By judge (assignment id): the marks as entered (items "1"…, `strafpunkte_a|b|c`).
    pub marks: BTreeMap<String, BTreeMap<String, Decimal>>,
    /// By judge: the judge's total, when the sheet has every item.
    pub totals: BTreeMap<String, Decimal>,
    /// The judges' mean (the phase's value), the rank.
    pub total: Option<Decimal>,
    pub rank: Option<u32>,
    pub placed: bool,
}

pub fn multitest(state: &State, class_id: &ClassId) -> Result<MultitestView, String> {
    let class = state.classes.get(class_id).ok_or_else(|| format!("unknown class {class_id}"))?;
    if class.phase.as_ref().map(|p| p.kind) != Some(PhaseKind::Multitest) {
        return Err(format!("class {} is not a multitest", class.number));
    }
    let programme = class.options.driving.as_ref().and_then(|d| d.multitest.clone()).unwrap_or_else(|| "B4".into());
    let coefficients = scoring::driving::multitest_coefficients(&programme);
    let judges: Vec<JudgeView> = state.dressage_judges(class_id).into_iter().map(|a| JudgeView { assignment: a.id.0.clone(), letter: a.letter.clone().unwrap_or_default(), name: person_name(state, &a.person).0 }).collect();
    let ranking = state.ranking(class_id).ok();
    let list = start_list(state, class_id)?;
    let mut lines = Vec::new();
    for line in &list.lines {
        let StartLine::Starter { entry, position, start_number, rider, horse, not_starting, .. } = line else { continue };
        let id = EntryId(*entry);
        let mut marks = BTreeMap::new();
        let mut totals = BTreeMap::new();
        for j in &judges {
            let Some(sheet) = state.sheets.get(&(class_id.clone(), id.clone(), AssignmentId(j.assignment.clone()))) else { continue };
            let entered: BTreeMap<String, Decimal> = sheet.marks.iter().map(|(k, v)| (k.clone(), *v)).collect();
            if (1..=coefficients.len()).all(|n| entered.contains_key(&n.to_string())) {
                let items: Vec<(Decimal, Decimal)> = coefficients.iter().enumerate().map(|(n, c)| (entered[&(n + 1).to_string()], *c)).collect();
                let p = |k: &str| entered.get(k).copied().unwrap_or_default();
                totals.insert(j.assignment.clone(), scoring::driving::multitest_judge_total(&items, [p("strafpunkte_a"), p("strafpunkte_b"), p("strafpunkte_c")]));
            }
            marks.insert(j.assignment.clone(), entered);
        }
        let ranked = ranking.as_ref().and_then(|r| r.lines.iter().find(|l| !l.team && l.id == entry.to_string()));
        lines.push(MultitestLine {
            entry: *entry,
            position: *position,
            start_number: *start_number,
            rider: rider.clone(),
            horse: horse.clone(),
            not_starting: *not_starting,
            out: state.out_reason(class_id, &id),
            marks,
            totals,
            total: ranked.and_then(|l| l.faults),
            rank: ranked.and_then(|l| l.placement.rank()),
            placed: ranked.is_some_and(|l| l.placed),
        });
    }
    let discipline = state.class_discipline(class).to_string();
    Ok(MultitestView {
        class: class_row(state, class),
        programme,
        items: coefficients.iter().enumerate().map(|(n, c)| MultitestItem { number: (n + 1).to_string(), coefficient: *c }).collect(),
        judges,
        reasons: crate::results::reasons(state, &discipline).into_iter().map(|r| ReasonView { code: r.id, name: r.name.clone() }).collect(),
        lines,
    })
}

fn dressage_reasons(state: &State, class: &Class) -> Vec<ReasonView> {
    let discipline = state.class_discipline(class).to_string();
    crate::results::reasons(state, &discipline).into_iter().map(|r| ReasonView { code: r.id, name: r.name.clone() }).collect()
}

pub fn sheets(state: &State, class_id: &ClassId) -> Result<SheetsView, String> {
    let class = state.classes.get(class_id).ok_or_else(|| format!("unknown class {class_id}"))?;
    let judges: Vec<JudgeView> = state
        .dressage_judges(class_id)
        .into_iter()
        .map(|a| JudgeView { assignment: a.id.0.clone(), letter: a.letter.clone().unwrap_or_default(), name: person_name(state, &a.person).0 })
        .collect();
    let ranking = state.ranking(class_id).ok();
    let team_dressage = state.published_classes.get(&class.published).is_some_and(|p| p.discipline == "CD" && p.team_size.is_some_and(|n| n > 0)) && !state.class_teams(class_id).is_empty();
    if team_dressage {
        return team_sheets(state, class, judges, ranking.as_ref());
    }
    let list = start_list(state, class_id)?;
    let individual = class.programme.and_then(|p| state.programmes.get(&p)).is_some_and(show::master::is_individual);
    let mut programmes: BTreeMap<u32, ProgrammeView> = BTreeMap::new();
    let mut riders = Vec::new();
    for line in &list.lines {
        let StartLine::Starter { entry, position, start_number, rider, horse, not_starting, .. } = line else { continue };
        let e = &state.entries[&EntryId(*entry)];
        let programme = if individual { e.programme } else { class.programme };
        if let Some(def) = programme.and_then(|p| state.programmes.get(&p)) {
            if let std::collections::btree_map::Entry::Vacant(slot) = programmes.entry(def.id) {
                if let Some(view) = programme_view(def) {
                    slot.insert(view);
                }
            }
        }
        let view = programme.and_then(|p| programmes.get(&p));
        let line = ranking.as_ref().and_then(|r| r.lines.iter().find(|l| l.id == entry.to_string()));
        let sheets = judges
            .iter()
            .filter_map(|j| {
                let sheet = state.sheets.get(&(class_id.clone(), EntryId(*entry), AssignmentId(j.assignment.clone())))?;
                let total = line.and_then(|l| l.judge_totals.get(&j.assignment)).copied();
                let max = view.map(|v| v.max_points).filter(|m| !m.is_zero());
                Some((
                    j.assignment.clone(),
                    SheetView {
                        marks: sheet.marks.clone(),
                        penalties: view.map(|v| ticked(sheet, &v.penalties)).unwrap_or_default(),
                        complete: if class.options.totals_per_judge {
                            sheet.total.is_some()
                        } else {
                            view.is_some_and(|v| v.sections.iter().flat_map(|s| &s.movements).all(|m| sheet.marks.contains_key(&m.number)))
                        },
                        percent: total.zip(max).map(|(t, m)| scoring::number::round(t / m * Decimal::ONE_HUNDRED, 2)),
                        total,
                        entered: sheet.total,
                    },
                ))
            })
            .collect();
        riders.push(SheetRider {
            entry: *entry,
            position: *position,
            start_number: *start_number,
            rider: rider.clone(),
            horse: horse.clone(),
            not_starting: *not_starting,
            programme,
            penalties: rider_penalties(&sheets),
            sheets,
            percentage: line.and_then(|l| l.percentage),
            part_a: line.and_then(|l| l.part_a),
            part_b: line.and_then(|l| l.part_b),
            rank: line.and_then(|l| l.placement.rank()),
            out: state.results.get(&(class_id.clone(), EntryId(*entry))).and_then(|c| c.out),
            team: None,
            judge_riders: BTreeMap::new(),
        });
    }
    let aside = state.aside.iter().filter(|((c, _), _)| c == class_id).map(|((_, slot), s)| aside_view(*slot, s)).collect();
    let driving = state.driving(class);
    if driving {
        // Driving dressage: the total on 160 where dressage shows its percentage.
        for r in riders.iter_mut() {
            r.percentage = ranking.as_ref().and_then(|k| k.lines.iter().find(|l| l.id == r.entry.to_string())).and_then(|l| l.faults);
        }
    }
    Ok(SheetsView { class: class_row(state, class), programmes, judges, reasons: dressage_reasons(state, class), riders, aside, totals_per_judge: class.options.totals_per_judge, teams: false, driving })
}

/// Team dressage: a row per team in team order, its judges' sheets (one per team and judge).
fn team_sheets(state: &State, class: &Class, judges: Vec<JudgeView>, ranking: Option<&show::ranking::Ranking>) -> Result<SheetsView, String> {
    let mut programmes: BTreeMap<u32, ProgrammeView> = BTreeMap::new();
    let def = class.programme.and_then(|p| state.programmes.get(&p));
    if let Some(view) = def.and_then(programme_view) {
        programmes.insert(view.id, view);
    }
    let view = class.programme.and_then(|p| programmes.get(&p));
    let max = view.map(|v| v.max_points).filter(|m| !m.is_zero());
    let mut riders = Vec::new();
    for (i, (team, members)) in state.class_teams(&class.id).into_iter().enumerate() {
        let line = ranking.and_then(|r| r.lines.iter().find(|l| l.team && l.id == team.0));
        let sheets = judges
            .iter()
            .filter_map(|j| {
                let sheet = state.team_sheets.get(&(class.id.clone(), team.clone(), AssignmentId(j.assignment.clone())))?;
                let total = line.and_then(|l| l.judge_totals.get(&j.assignment)).copied();
                Some((
                    j.assignment.clone(),
                    SheetView {
                        marks: sheet.marks.clone(),
                        penalties: view.map(|v| ticked(sheet, &v.penalties)).unwrap_or_default(),
                        complete: view.is_some_and(|v| v.sections.iter().flat_map(|s| &s.movements).all(|m| sheet.marks.contains_key(&m.number))),
                        percent: total.zip(max).map(|(t, m)| scoring::number::round(t / m * Decimal::ONE_HUNDRED, 2)),
                        total,
                        entered: None,
                    },
                ))
            })
            .collect();
        // 3/2: H, C and M judge the team's first, second and third rider.
        let judge_riders = if class.options.team_3_2 {
            ["H", "C", "M"].iter().zip(&members).filter_map(|(letter, e)| judges.iter().find(|j| j.letter == *letter).map(|j| (j.assignment.clone(), rider(state, e).0))).collect()
        } else {
            BTreeMap::new()
        };
        let t = state.teams.get(&team);
        riders.push(SheetRider {
            entry: 0,
            position: i as u32 + 1,
            start_number: t.and_then(|t| t.start_number),
            rider: t.map(|t| t.name.clone()).unwrap_or_else(|| team.0.clone()),
            horse: members.iter().map(|e| rider(state, e).0).collect::<Vec<_>>().join(" · "),
            not_starting: members.iter().all(|e| e.status != EntryStatus::Entered),
            programme: class.programme,
            penalties: rider_penalties(&sheets),
            sheets,
            percentage: line.and_then(|l| l.percentage),
            part_a: line.and_then(|l| l.part_a),
            part_b: line.and_then(|l| l.part_b),
            rank: line.and_then(|l| l.placement.rank()),
            out: state.team_results.get(&(class.id.clone(), team.clone())).and_then(|c| c.out),
            team: Some(team.0.clone()),
            judge_riders,
        });
    }
    Ok(SheetsView { class: class_row(state, class), programmes, judges, reasons: dressage_reasons(state, class), riders, aside: Vec::new(), totals_per_judge: false, teams: true, driving: false })
}

// ─── Show-jumping results ────────────────────────────────────────────────────

/// A reason for being out (Swiss Equestrian's label, in German; the UI translates the usual ones).
#[derive(Debug, Clone, Serialize)]
pub struct ReasonView {
    pub code: u32,
    pub name: String,
}

/// A starter's result as the office enters it.
#[derive(Debug, Clone, Serialize)]
pub struct ResultLine {
    pub entry: u64,
    pub position: u32,
    pub start_number: Option<u32>,
    pub rider: String,
    pub horse: String,
    pub not_starting: bool,
    pub hors_concours: bool,
    /// The fields entered (scoring-table names: `strafpunkte_initial_1`, `zeit_initial_1`, …).
    pub values: BTreeMap<String, Decimal>,
    pub out: Option<u32>,
    /// Cones: a round over its maximum time, eliminated by the rules (not by the office).
    pub over_maximum: bool,
    pub has_result: bool,
    /// From the class's ranking.
    pub rank: Option<u32>,
    pub manual_rank: bool,
    pub placed: bool,
    pub faults: Option<Decimal>,
    pub time: Option<Decimal>,
    /// Penalties brought from the qualifier (U9).
    pub carried: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultsView {
    pub class: ClassRow,
    /// The scoring table's fields, in the order of the rounds.
    pub fields: Vec<String>,
    /// Two phases: `initial_2` is phase 2, not a second round.
    pub two_phase: bool,
    /// Scored on points, the most first: the `strafpunkte` fields hold points, not faults.
    pub points: bool,
    pub reasons: Vec<ReasonView>,
    pub qualifier: Option<String>,
    pub placed_count: usize,
    /// Why the class cannot be ranked, if it cannot.
    pub ranking_error: Option<String>,
    /// Cones in Wertung B: the faults are entered in seconds (5 s a ball; Fahrreglement 8.4.7).
    pub faults_in_seconds: bool,
    /// In start order.
    pub lines: Vec<ResultLine>,
}

pub fn results(state: &State, class_id: &ClassId) -> Result<ResultsView, String> {
    let class = state.classes.get(class_id).ok_or_else(|| format!("unknown class {class_id}"))?;
    let discipline = state.class_discipline(class).to_string();
    let ranking = state.ranking(class_id);
    let ranked: BTreeMap<String, &show::ranking::Line> = ranking.as_ref().map(|r| r.lines.iter().filter(|l| !l.team).map(|l| (l.id.clone(), l)).collect()).unwrap_or_default();
    let carried = state.carried(class_id).unwrap_or_default();
    let mut starts: Vec<&Start> = state.starts.get(class_id).into_iter().flatten().collect();
    starts.sort_by_key(|s| (s.order, s.team_order.unwrap_or(u32::MAX)));
    let lines = starts
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let e = state.entries.get(&s.entry)?;
            let card = state.results.get(&(class_id.clone(), s.entry.clone()));
            let line = ranked.get(&s.entry.to_string());
            Some(ResultLine {
                entry: s.entry.0,
                position: i as u32 + 1,
                start_number: s.start_number,
                rider: rider(state, e).0,
                horse: horse(state, e),
                not_starting: e.status == EntryStatus::NotStarting,
                hors_concours: e.hors_concours,
                values: card.map(|c| c.values.clone()).unwrap_or_default(),
                out: card.and_then(|c| c.out),
                over_maximum: card.is_some_and(|c| c.out.is_none() && state.over_time_limit(class, e, &c.values)),
                has_result: card.is_some(),
                rank: line.and_then(|l| l.placement.rank()),
                manual_rank: line.is_some_and(|l| l.manual),
                placed: line.is_some_and(|l| l.placed),
                faults: line.and_then(|l| l.faults),
                time: line.and_then(|l| l.time),
                carried: carried.get(&s.entry).and_then(|r| r.faults),
            })
        })
        .collect();
    Ok(ResultsView {
        class: class_row(state, class),
        fields: crate::results::fields(state, class),
        two_phase: crate::results::two_phase(state, class),
        points: crate::results::points(state, class),
        reasons: crate::results::reasons(state, &discipline).into_iter().map(|r| ReasonView { code: r.id, name: r.name.clone() }).collect(),
        qualifier: class.options.qualifier.as_ref().and_then(|q| state.classes.get(q)).map(|q| q.number.clone()),
        placed_count: ranking.as_ref().map_or(0, |r| r.placed_count),
        ranking_error: ranking.err(),
        faults_in_seconds: class.phase.as_ref().is_some_and(|p| p.kind == PhaseKind::Cones) && state.cones_scoring(class) == show::model::ConesKind::B,
        lines,
    })
}

// ─── Rankings over several classes (U8) ──────────────────────────────────────

/// A ranking over several classes, as listed with the classes.
#[derive(Debug, Clone, Serialize)]
pub struct CombinedRow {
    pub id: String,
    pub name: String,
    /// `penalties`, `ranks` or `percentages`.
    pub method: &'static str,
    /// What breaks ties: `none`, `faults` or `result` (penalties added), `rank`, `percentage`.
    pub tie: &'static str,
    /// The class that breaks ties (ranks, percentages; penalties by `faults` or `result`).
    pub tie_break: Option<String>,
    pub classes: Vec<CombinedClass>,
    /// Its number, Swiss Equestrian's for its results (`State::combined_numbers`).
    pub number: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CombinedClass {
    pub id: String,
    pub number: String,
    pub date: String,
}


pub fn combined_row(state: &State, r: &CombinedRanking) -> CombinedRow {
    let (method, tie, tie_break) = match &r.method {
        CombinedMethod::Penalties { tie } => {
            let by = match tie {
                PenaltiesTie::None => "none",
                PenaltiesTie::Faults { .. } => "faults",
                PenaltiesTie::Result { .. } => "result",
            };
            ("penalties", by, tie.class().map(|c| c.0.clone()))
        }
        CombinedMethod::Ranks { tie_break } => ("ranks", "rank", Some(tie_break.0.clone())),
        // Not chosen (Rosson's): the first class breaks ties.
        CombinedMethod::Percentages { tie_break } => ("percentages", "percentage", tie_break.clone().or_else(|| r.classes.first().cloned()).map(|c| c.0)),
    };
    CombinedRow {
        id: r.id.clone(),
        name: r.name.clone(),
        method,
        tie,
        tie_break,
        classes: r.classes.iter().map(|c| CombinedClass { id: c.0.clone(), number: state.classes.get(c).map_or_else(|| c.0.clone(), |k| k.number.clone()), date: state.classes.get(c).map(|k| k.date.clone()).unwrap_or_default() }).collect(),
        number: state.combined_number(&r.id),
    }
}

/// A pair's result in one class of the ranking.
#[derive(Debug, Clone, Serialize)]
pub struct CombinedCell {
    /// Not in this class.
    pub missing: bool,
    pub rank: Option<u32>,
    pub faults: Option<Decimal>,
    pub time: Option<Decimal>,
    pub percentage: Option<Decimal>,
    /// Out of this class (no rank): why, as Swiss Equestrian names it.
    pub out: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CombinedLineView {
    pub rank: Option<u32>,
    pub rider: String,
    pub club: String,
    pub horse: String,
    pub cells: Vec<CombinedCell>,
    pub total: Option<Decimal>,
    pub tie_break: Option<Decimal>,
    pub placed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CombinedView {
    pub ranking: CombinedRow,
    pub lines: Vec<CombinedLineView>,
    /// Sent to Swiss Equestrian (when), and whether a class's results changed since.
    pub sent: Option<u64>,
    pub changed: bool,
}

pub fn combined(state: &State, id: &str) -> Result<CombinedView, String> {
    let r = state.combined.get(id).ok_or_else(|| format!("unknown ranking {id}"))?;
    let lines = state
        .combined_ranking(id)?
        .into_iter()
        .map(|l| {
            let entry = l.results.iter().rev().flatten().next().and_then(|r| state.entries.get(&r.entry));
            let (name, club, horse_name) = match &l.who {
                // A team: its name, its riders.
                show::combined::Who::Team(team) => {
                    let id = l.results.iter().flatten().find_map(|r| r.team.clone());
                    let riders: Vec<String> = state.entries.values().filter(|e| id.is_some() && e.team == id).map(|e| rider(state, e).0).collect();
                    (team.clone(), String::new(), riders.join(", "))
                }
                show::combined::Who::Pair(_) => {
                    let (name, _, club) = entry.map(|e| rider(state, e)).unwrap_or_default();
                    (name, club, entry.map(|e| horse(state, e)).unwrap_or_default())
                }
            };
            CombinedLineView {
                rank: l.rank,
                rider: name,
                club,
                horse: horse_name,
                cells: l
                    .results
                    .iter()
                    .zip(&r.classes)
                    .map(|(x, class)| match x {
                        Some(x) => {
                            let out = if x.rank.is_some() {
                                None
                            } else {
                                reason_name(state, match &x.team {
                                    Some(team) => state.team_results.get(&(class.clone(), team.clone())).and_then(|c| c.out),
                                    None => state.out_reason(class, &x.entry),
                                })
                            };
                            CombinedCell { missing: false, rank: x.rank, faults: x.faults, time: x.time, percentage: x.percentage, out }
                        }
                        None => CombinedCell { missing: true, rank: None, faults: None, time: None, percentage: None, out: None },
                    })
                    .collect(),
                // Percentages to two decimals, as printed.
                total: l.total.map(|t| if matches!(r.method, CombinedMethod::Percentages { .. }) { t.round_dp(3) } else { t }),
                tie_break: l.tie_break,
                placed: l.placed,
            }
        })
        .collect();
    let sent = state.uploaded.get(&Upload::Combined(id.to_string()));
    let changed = sent.is_some_and(|s| r.classes.iter().any(|c| changed_since(state.results_changed.get(c), Some(s))));
    Ok(CombinedView { ranking: combined_row(state, r), lines, sent: sent.map(|s| s.millis), changed })
}

// ─── Sending to Swiss Equestrian ─────────────────────────────────────────────

/// When and by whom something was last sent.
#[derive(Debug, Clone, Serialize)]
pub struct SentView {
    pub millis: u64,
    pub author: String,
}

/// A class, as far as Swiss Equestrian is concerned.
#[derive(Debug, Clone, Serialize)]
pub struct SendRow {
    pub class: ClassRow,
    pub results_sent: Option<SentView>,
    /// The results changed after they were sent.
    pub results_changed: bool,
    /// The first section of its published class: start lists go per published class.
    pub lists_here: bool,
    pub start_lists_sent: Option<SentView>,
    pub start_lists_changed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SendingView {
    pub event_sent: Option<SentView>,
    /// The classes in the show's order.
    pub rows: Vec<SendRow>,
}

pub fn sending(state: &State) -> SendingView {
    let sent = |u: &Upload| state.uploaded.get(u).map(|s| SentView { millis: s.millis, author: s.author.clone() });
    let mut seen = std::collections::BTreeSet::new();
    let rows = show("", state)
        .days
        .into_iter()
        .flat_map(|d| d.classes)
        .filter_map(|row| {
            let class = state.classes.get(&ClassId(row.id.clone()))?;
            let results_sent = sent(&Upload::Results(class.id.clone()));
            let start_lists_sent = sent(&Upload::StartLists(class.published.clone()));
            Some(SendRow {
                results_changed: row.results_changed,
                lists_here: seen.insert(class.published.clone()),
                start_lists_changed: row.start_lists_changed,
                results_sent,
                start_lists_sent,
                class: row,
            })
        })
        .collect();
    SendingView { event_sent: sent(&Upload::Event), rows }
}

// ─── Officials at the event ──────────────────────────────────────────────────

/// An official's role at the event itself.
#[derive(Debug, Clone, Serialize)]
pub struct EventRoleRow {
    pub assignment: String,
    pub person: u64,
    pub name: String,
    pub town: String,
    pub role: String,
    /// Swiss Equestrian's role id (several roles share a code, e.g. each discipline's TD).
    pub role_id: Option<u32>,
    pub role_name: String,
    pub days: Option<u32>,
    pub responsible: bool,
    /// Given by Swiss Equestrian.
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventOfficials {
    pub rows: Vec<EventRoleRow>,
    /// The roles an official can have at the event: (id, name).
    pub roles: Vec<(u32, String)>,
}

pub fn event_officials(state: &State, language: &str) -> EventOfficials {
    let name_of = |r: &RoleDef| match language {
        "de" => r.name_de.clone(),
        "it" if !r.name_it.trim().is_empty() => r.name_it.clone(),
        _ => r.name_fr.clone(),
    };
    let role_name = |a: &Assignment| state.roles.iter().find(|r| !r.in_class && (Some(r.id) == a.role_id || (a.role_id.is_none() && r.code == a.role))).map(name_of).unwrap_or_else(|| a.role.clone());
    let mut rows: Vec<EventRoleRow> = state
        .assignments
        .values()
        .filter(|a| a.class.is_none())
        .map(|a| {
            let (name, town) = state
                .persons
                .get(&a.person)
                .map(|p| (format!("{} {}", p.first_name, p.last_name), p.town.clone()))
                .or_else(|| state.officials.get(&a.person).map(|o| (format!("{} {}", o.first_name, o.last_name), o.town.clone())))
                .unwrap_or_else(|| (format!("#{}", a.person), String::new()));
            EventRoleRow { assignment: a.id.0.clone(), person: a.person.0, name, town, role: a.role.clone(), role_id: a.role_id, role_name: role_name(a), days: a.days, responsible: a.responsible == Some(true), readonly: a.readonly == Some(true) }
        })
        .collect();
    rows.sort_by(|a, b| (&a.role_name, &a.name).cmp(&(&b.role_name, &b.name)));
    let mut roles: Vec<(u32, String)> = state.roles.iter().filter(|r| !r.in_class).map(|r| (r.id, name_of(r))).collect();
    roles.sort_by(|a, b| a.1.cmp(&b.1));
    EventOfficials { rows, roles }
}


/// The days from `from` to `to` (`YYYY-MM-DD`), both included; at most 31.
pub fn days_between(from: &str, to: &str) -> Vec<String> {
    let parse = |s: &str| -> Option<(i32, u32, u32)> {
        let mut p = s.get(..10)?.split('-');
        Some((p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next()?.parse().ok()?))
    };
    let (Some(mut day), Some(last)) = (parse(from), parse(to)) else { return Vec::new() };
    let length = |y: i32, m: u32| match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    let mut days = Vec::new();
    while day <= last && days.len() < 31 {
        days.push(format!("{:04}-{:02}-{:02}", day.0, day.1, day.2));
        day = if day.2 < length(day.0, day.1) { (day.0, day.1, day.2 + 1) } else if day.1 < 12 { (day.0, day.1 + 1, 1) } else { (day.0 + 1, 1, 1) };
    }
    days
}

/// A dressage class's test duration as written on the test (`zeit_de`: "Zeit 4'00" → "4'00").
/// A test's longest duration in whole minutes, rounded up (the time between starts is at least
/// that): "Zeit 4'30" → 5, "Dauer 6 Minuten 30 Sekunden" → 7, "mind. 4'30 - max. 5'00" → 5,
/// "Kür 5-6 Min." → 6. Other numbers (the arena's 20 x 60 m, a horse's age) do not count.
pub fn test_minutes(text: &str) -> Option<u32> {
    let chars: Vec<char> = text.chars().collect();
    let number = |i: &mut usize| -> Option<u32> {
        let start = *i;
        while *i < chars.len() && chars[*i].is_ascii_digit() {
            *i += 1;
        }
        chars[start..*i].iter().collect::<String>().parse().ok()
    };
    let skip_spaces = |i: &mut usize| {
        while *i < chars.len() && chars[*i] == ' ' {
            *i += 1;
        }
    };
    let word = |i: usize, w: &str| chars[i..].iter().take(w.len()).collect::<String>().to_lowercase() == w;
    let mut longest = 0u32;
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() || (i > 0 && chars[i - 1].is_ascii_digit()) {
            i += 1;
            continue;
        }
        let Some(n) = number(&mut i) else { continue };
        let mut j = i;
        skip_spaces(&mut j);
        let seconds = if j < chars.len() && matches!(chars[j], '\'' | '’' | '′') {
            // 4'30
            let mut k = j + 1;
            let s = if k < chars.len() && chars[k].is_ascii_digit() { number(&mut k).unwrap_or(0) } else { 0 };
            Some(n * 60 + s)
        } else if j < chars.len() && word(j, "min") {
            // 6 Minuten (30 Sekunden)
            let mut k = j;
            while k < chars.len() && chars[k].is_alphabetic() || (k < chars.len() && chars[k] == '.') {
                k += 1;
            }
            skip_spaces(&mut k);
            let s = if k < chars.len() && chars[k].is_ascii_digit() {
                let mut m = k;
                let s = number(&mut m).unwrap_or(0);
                skip_spaces(&mut m);
                if m < chars.len() && word(m, "s") { s } else { 0 }
            } else {
                0
            };
            Some(n * 60 + s)
        } else {
            None
        };
        if let Some(sec) = seconds {
            longest = longest.max(sec);
        }
    }
    (longest > 0).then(|| longest.div_ceil(60))
}

fn programme_minutes(state: &State, class: &Class) -> Option<u32> {
    let def = class.programme.and_then(|p| state.programmes.get(&p))?;
    let json: serde_json::Value = serde_json::from_str(&def.definition).ok()?;
    test_minutes(json["zeit_de"].as_str().or_else(|| json["zeit_fr"].as_str())?)
}

fn programme_time(state: &State, class: &Class) -> Option<String> {
    let def = class.programme.and_then(|p| state.programmes.get(&p))?;
    let json: serde_json::Value = serde_json::from_str(&def.definition).ok()?;
    let text = json["zeit_de"].as_str().or_else(|| json["zeit_fr"].as_str())?;
    let time: String = text.chars().skip_while(|c| !c.is_ascii_digit()).collect();
    Some(time.trim().to_string()).filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_tests_minutes() {
        use super::test_minutes;
        for (text, minutes) in [
            ("Zeit 4'30", Some(5)),
            ("Zeit 5'00 (nur als Information)", Some(5)),
            ("Dauer 6 Minuten 30 Sekunden", Some(7)),
            ("Dauer: 8 Minuten", Some(8)),
            (" / Viereck 20 x 60m: Zeit mind. 4'30 - max. 5'00", Some(5)),
            ("Zeit mind. 5'30 - max. 6'00 - Pferdealter mind. 8", Some(6)),
            ("Zeit Kür/Pas-de-deux 5-6 Min. - Quadrillen 7-8 Min.", Some(8)),
            ("Time 5'00 (for information only)", Some(5)),
            ("Viereck 20 x 40", None),
        ] {
            assert_eq!(test_minutes(text), minutes, "{text}");
        }
    }

    #[test]
    fn days_of_a_show() {
        assert_eq!(super::days_between("2026-09-25", "2026-09-27"), ["2026-09-25", "2026-09-26", "2026-09-27"]);
        assert_eq!(super::days_between("2028-02-28", "2028-03-01"), ["2028-02-28", "2028-02-29", "2028-03-01"]);
        assert_eq!(super::days_between("2026-12-31T00:00", "2027-01-01"), ["2026-12-31", "2027-01-01"]);
        assert!(super::days_between("x", "2027-01-01").is_empty());
    }
}

// ─── Eventing: the overall ranking ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct OverallPhase {
    pub id: String,
    pub kind: PhaseKind,
    pub date: String,
    pub results: usize,
}

/// A rider's result in one phase, as the overall ranking counts and shows it.
#[derive(Debug, Clone, Serialize)]
pub struct OverallCell {
    /// What counts: penalties (dressage 100 − %), or the phase's rank (rank points).
    pub value: Option<Decimal>,
    pub rank: Option<u32>,
    pub out: bool,
    /// Why out of the phase, as Swiss Equestrian names it.
    pub reason: Option<String>,
    /// The phase's result beside what counts: "68.25 %" (dressage), the time (jumping "72.31",
    /// cross "5:12").
    pub text: String,
    /// Eventing: the jury's penalties in this phase, included in `value` (S41).
    pub jury: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverallLine {
    pub entry: u64,
    pub rank: Option<u32>,
    pub placed: bool,
    pub start_number: Option<u32>,
    pub rider: String,
    pub horse: String,
    pub club: String,
    /// One per phase, in the phases' order.
    pub cells: Vec<OverallCell>,
    pub total: Option<Decimal>,
    pub out: bool,
    /// Why out: the first phase the rider is out of.
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverallView {
    pub class: ClassRow,
    pub phases: Vec<OverallPhase>,
    pub lines: Vec<OverallLine>,
    pub rank_points: bool,
    pub placed_count: usize,
}

/// Eventing: a class made of phases, its overall ranking with each phase's result.
pub fn overall(state: &State, class_id: &ClassId) -> Result<OverallView, String> {
    let class = state.classes.get(class_id).ok_or_else(|| format!("unknown class {class_id}"))?;
    if class.phases.is_empty() {
        return Err(format!("class {} has no phases", class.number));
    }
    let ranking = state.ranking(class_id)?;
    let phases: Vec<(&Class, PhaseKind, Option<show::ranking::Ranking>)> = class
        .phases
        .iter()
        .filter_map(|p| state.classes.get(p))
        .filter_map(|c| c.phase.as_ref().map(|p| (c, p.kind, state.ranking(&c.id).ok())))
        .collect();
    let clock = |t: Decimal| {
        let secs = t.trunc().to_string().parse::<u64>().unwrap_or(0);
        let frac = t.fract();
        if frac.is_zero() { format!("{}:{:02}", secs / 60, secs % 60) } else { format!("{}:{:02}{}", secs / 60, secs % 60, frac.normalize().to_string().trim_start_matches('0')) }
    };
    let numbers: BTreeMap<EntryId, u32> = phases.iter().flat_map(|(c, _, _)| state.starts.get(&c.id).into_iter().flatten()).filter_map(|s| s.start_number.map(|n| (s.entry.clone(), n))).collect();
    let lines = ranking
        .lines
        .iter()
        .filter(|l| !l.team)
        .filter_map(|l| {
            let entry_id = EntryId(l.id.parse().ok()?);
            let e = state.entries.get(&entry_id)?;
            let (name, _, club) = rider(state, e);
            let cells = phases
                .iter()
                .map(|(phase, kind, r)| {
                    let line = r.as_ref().and_then(|r| r.lines.iter().find(|x| !x.team && x.id == l.id));
                    let out = line.is_some_and(|x| x.placement == scoring::ranking::Placement::Out);
                    let reason = if out { reason_name(state, state.out_reason(&phase.id, &entry_id)) } else { None };
                    let text = match (kind, line) {
                        (_, None) => String::new(),
                        (_, Some(_)) if out => String::new(),
                        (PhaseKind::Dressage, Some(x)) => x.percentage.map(|p| format!("{p:.2} %")).unwrap_or_default(),
                        (PhaseKind::Jumping | PhaseKind::Cones | PhaseKind::Open | PhaseKind::Derby, Some(x)) => x.time.filter(|t| !t.is_zero()).map(|t| t.normalize().to_string()).unwrap_or_default(),
                        (PhaseKind::Cross | PhaseKind::Marathon, Some(x)) => x.time.map(clock).unwrap_or_default(),
                        (PhaseKind::Multitest | PhaseKind::Presentation, Some(_)) => String::new(),
                    };
                    // Eventing: the jury's penalties in this phase (S41).
                    let jury = state.jury_penalties.get(&(phase.id.clone(), entry_id.clone())).copied();
                    let value = match line {
                        Some(x) if !out => {
                            if state.overall_by_rank(class) {
                                x.placement.rank().map(Decimal::from)
                            } else if *kind == PhaseKind::Dressage && x.percentage.is_some() {
                                x.percentage.map(|p| scoring::number::round(Decimal::ONE_HUNDRED - p, 2) + jury.unwrap_or_default())
                            } else {
                                x.faults.map(|f| f + jury.unwrap_or_default())
                            }
                        }
                        _ => None,
                    };
                    OverallCell { value, rank: line.and_then(|x| x.placement.rank()), out, reason, text, jury }
                })
                .collect::<Vec<OverallCell>>();
            let out = l.placement == scoring::ranking::Placement::Out;
            let reason = if out { cells.iter().find_map(|c| c.reason.clone()) } else { None };
            Some(OverallLine {
                entry: entry_id.0,
                rank: l.placement.rank(),
                placed: l.placed,
                start_number: numbers.get(&entry_id).copied(),
                rider: name,
                horse: horse(state, e),
                club,
                cells,
                total: l.faults,
                out,
                reason,
            })
        })
        .collect();
    let phases = phases
        .iter()
        .map(|(c, kind, _)| OverallPhase { id: c.id.0.clone(), kind: *kind, date: c.date.clone(), results: class_row(state, c).results })
        .collect();
    Ok(OverallView { class: class_row(state, class), phases, lines, rank_points: state.overall_by_rank(class), placed_count: ranking.placed_count })
}

#[cfg(test)]
mod said_tests {
    #[test]
    fn anonymised_texts_say_nothing() {
        assert_eq!(super::said("[41 Zeichen entfernt]"), "");
        assert_eq!(super::said("Coach: M. Dupont"), "Coach: M. Dupont");
        assert_eq!(super::said("[Zeichen entfernt]"), "[Zeichen entfernt]");
    }
}
