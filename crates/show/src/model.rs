//! The data model of a show (docs/architecture/data-and-sync.md).
//!
//! Normalised: one fact in one place, results reference the entry instead of copying it.
//! Only *entered* values are stored (faults, times, marks, eliminations); ranks and totals are
//! always computed by the `scoring` crate.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident($inner:ty)) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub $inner);
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.0.fmt(f) }
        }
    };
}

id_type!(/// Swiss Equestrian person id (riders, officials).
    PersonId(u64));
id_type!(/// Swiss Equestrian horse id.
    HorseId(u64));
id_type!(/// Swiss Equestrian event (Veranstaltung) id.
    EventId(u64));
id_type!(/// Swiss Equestrian id of a class as published in the Ausschreibung.
    PublishedClassId(u64));
id_type!(/// A class as run at the show (a published class can be split into sections).
    ClassId(String));
id_type!(/// Swiss Equestrian entry (Nennung) id.
    EntryId(u64));
id_type!(/// Team (Equipe) id.
    TeamId(String));
id_type!(/// An official's assignment to the show or a class.
    AssignmentId(String));

/// Public data of a person, as on info.swiss-equestrian.ch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub id: PersonId,
    pub first_name: String,
    pub last_name: String,
    pub town: String,
    pub nationality: String,
    pub club: String,
    pub licences: String,
    /// Youth status as delivered by Swiss Equestrian ("J", "Y" or empty).
    pub youth: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Horse {
    pub id: HorseId,
    pub name: String,
    pub signalement: String,
    pub owner: String,
    pub breeder: String,
    /// The horse record as Swiss Equestrian delivered it (public data): what is written on an
    /// entry is built from it (`Entry::horses_on_entry`), also when an entry changes horse later.
    #[serde(default)]
    pub record: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    /// Swiss Equestrian event number, e.g. "2026.8352".
    pub number: String,
    pub title: Vec<String>,
    pub place: String,
    pub from: String,
    pub to: String,
    /// Start numbers are fixed for the whole event (a rider keeps the number in every class).
    #[serde(default)]
    pub fixed_start_numbers: bool,
    /// With fixed start numbers: they have been given out.
    #[serde(default)]
    pub start_numbers_assigned: bool,
    /// The organiser (`veranstalter`).
    #[serde(default)]
    pub organiser: String,
    /// Postcode of the show ground (`postleitzahl`), for riders' travel distances.
    #[serde(default)]
    pub postcode: String,
    /// The event's disciplines (`disziplin_code`: CS, CD, ...).
    #[serde(default)]
    pub disciplines: Vec<String>,
    /// Swiss Equestrian's time of the last change it delivered (`letzte_aenderung`); updates ask
    /// for the changes after it.
    #[serde(default)]
    pub se_last_change: Option<String>,
    /// The record as Swiss Equestrian delivered it, in Rosson's shape, without what is not
    /// public: kept to write the show in Rosson's format (`rosson_import::to_rosson`).
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub record: serde_json::Value,
}

/// A class as published by Swiss Equestrian. Read-only at the show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublishedClass {
    pub id: PublishedClassId,
    pub number: String,
    /// Discipline code: CS show jumping, CD dressage, CA driving, ...
    pub discipline: String,
    pub category: String,
    pub scoring_table: Option<u32>,
    /// Riders per team, for team classes.
    pub team_size: Option<u32>,
    /// Swiss Equestrian discipline id (`disziplin_id`), sent with results.
    #[serde(default)]
    pub discipline_id: u32,
    /// Swiss Equestrian category id (`kategorie_id`).
    #[serde(default)]
    pub category_id: Option<u32>,
    /// The category as printed, e.g. "R/N120" (`kategorie_text`).
    #[serde(default)]
    pub category_text: String,
    /// Show jumping in levels (Stufen): how many; 0 or 1 means none.
    #[serde(default)]
    pub levels: u32,
    /// Date as published, `YYYY-MM-DD` (`datum`).
    #[serde(default)]
    pub date: String,
    /// The scoring as published, e.g. "A au chrono" (`wertung_text`).
    #[serde(default)]
    pub scoring_text: Option<String>,
    /// The class's text in the schedule: who may start, conditions, prizes (`bemerkungen`).
    #[serde(default)]
    pub description: String,
    /// Driving: the phases of the class (`fahren_zwischenpruefungen`), written on every horse of
    /// an entry (`einsaetze`).
    #[serde(default)]
    pub driving_phases: Vec<serde_json::Value>,
    /// The entry fee (`nenngeld1`), CHF.
    #[serde(default)]
    pub fee: Option<Decimal>,
    /// The record as Swiss Equestrian delivered it, in Rosson's shape, without what is not
    /// public: kept to write the show in Rosson's format (`rosson_import::to_rosson`).
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub record: serde_json::Value,
}

/// A published class's prizes (Rosson's "Preisgeld"): an amount and a prize in kind per rank,
/// for the ranking and the prize labels. Local: not sent to Swiss Equestrian.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prizes {
    /// The smallest amount (Rosson's default: (fee − 10) × 2).
    pub minimum: Decimal,
    /// CHF per rank, from the first.
    pub amounts: Vec<Decimal>,
    /// A prize in kind per rank ("Couverture", …), from the first.
    pub natural: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassStatus {
    #[default]
    Planned,
    Running,
    /// Results are official; changes need a reopening.
    Final,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassOptions {
    /// Championship: jump-offs for every medal tie (docs/deviations.md S14).
    pub medal_jump_offs: bool,
    /// Placed riders set by the organiser instead of 30 % (S10).
    pub placed_count: Option<u32>,
    /// Dressage: judges enter their total, not marks per movement (Rosson's `manuelle_eingabe`).
    pub totals_per_judge: bool,
    /// Team dressage, five judges 3/2 (Rosson's `mannschaft`): H, C and M each judge one rider
    /// (the team's first, second, third), E and B the whole team.
    #[serde(default)]
    pub team_3_2: bool,
    /// Rosson class options the engine does not cover yet (eventing's ideal or maximum time
    /// counted as penalties); kept, not lost.
    pub not_covered: Vec<String>,
    /// The ideal time, in seconds, of a class on the ideal-time table (AZI, S20): set by the
    /// office; riders closest to it rank first among equal penalties.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ideal_time: Option<Decimal>,
    /// Only the team gets a result; it is entered on the team, not on its riders (S7).
    #[serde(default)]
    pub team_only: bool,
    /// Dressage: a regional or Swiss final of the Tag der Jugend — no ex aequo in ranks 1–3
    /// (RL Tag der Jugend 10.3; S38).
    #[serde(default)]
    pub youth_final: bool,
    /// Team jumping: every rider's result counts, the worst of a team of four is not dropped
    /// (the Ausschreibung's choice, Springreglement 4.6; S36).
    #[serde(default)]
    pub team_count_all: bool,
    /// The class whose penalties the riders bring into this one (a final after a qualifier,
    /// U9): shown on the start list; the start order can be the qualifier's, reversed. The
    /// timekeeper includes them in this class's results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualifier: Option<ClassId>,
    /// The most starters (Rosson's `max_startende`, the office's choice): the entries made last
    /// beyond it wait on no start list, to move up when a place frees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_starters: Option<u32>,
    /// A class of phases: the overall ranking adds each phase's rank instead of its penalties
    /// (Rosson's `wertung_nach: rangpunkte`).
    #[serde(default)]
    pub rank_points: bool,
    /// Cross-country (and jumping on an ideal-time table): time over the ideal time counts, 0.4
    /// per second started (Rosson's `idealzeit_berechnen`).
    #[serde(default)]
    pub ideal_time_penalties: bool,
    /// A phase's maximum time, in seconds; with `max_time_penalties`, 0.4 per second started over
    /// it counts (Rosson's `max_zeit`, `max_zeit_berechnen`; eventing's jumping phase).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_time: Option<Decimal>,
    #[serde(default)]
    pub max_time_penalties: bool,
    /// A driving phase's settings (marathon, cones, derby, open, multitest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driving: Option<DrivingSettings>,
}

/// What a driving class made of phases is, for its overall ranking (Fahrreglement 1.5 (2)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrivingEvent {
    /// Vollprüfung: dressage, marathon, cones.
    Full,
    /// Reduzierte Vollprüfung (dressage, derby) or Derbyvollprüfung (dressage, cones, derby).
    Derby,
    /// Kurzprüfung: any other combination.
    Short,
}

/// How a phase of cones is scored (Rosson's driving tables A, A2U, B, B2U, JC2U).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConesKind {
    A,
    B,
    JuniorCup,
}

/// How an open phase ranks (Rosson's `zeitwertung`, `punktewertung`, `strafpunktewertung`,
/// `punktefahren`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenKind {
    Time,
    Points,
    PenaltyPoints,
    PointsThenTime,
}

/// A driving phase's settings (seconds for times).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrivingSettings {
    /// Marathon: phases A and B, allowed and minimum times; the factor per second (0.25).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a_allowed: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a_minimum: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b_allowed: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b_minimum: Option<Decimal>,
    /// Marathon: the time limits of phases A and B; over them, eliminated (unset: A's allowed
    /// time + 20 %, B's twice, as the rules and Rosson's `max_zeit`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a_maximum: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b_maximum: Option<Decimal>,
    /// Marathon: the transfer from phase A to B, minutes (Rosson's `ueberfuehrungszeit`): phase B
    /// starts after A's time limit and the transfer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factor: Option<Decimal>,
    /// Marathon: obstacles in phase B (5 to 8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obstacles: Option<u32>,
    /// Cones: the scoring, one or two rounds, the allowed time per round and licence
    /// (`"1:FB"` → seconds), half the overtime to the hundredth ("international"), seconds more
    /// per turnout (Einlaufprüfung: `"2"` → seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cones: Option<ConesKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rounds: Option<u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub allowed: BTreeMap<String, Decimal>,
    /// Cones: the maximum time per round and licence (`"1:FB"` → seconds); over it, eliminated
    /// (Rosson's `max_zeit_<licence>`; unset: twice the allowed time, as Rosson computes it).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub maximum: BTreeMap<String, Decimal>,
    #[serde(default)]
    pub international: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bonus: BTreeMap<String, Decimal>,
    /// Derby: the time factor (0.25).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_factor: Option<Decimal>,
    /// Open: how it ranks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<OpenKind>,
    /// Multitest: the programme, B1 or B4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multitest: Option<String>,
}

impl DrivingSettings {
    /// The marathon's time limits of phases A and B (as set, else allowed + 20 % and × 2).
    pub fn marathon_limits(&self) -> (Option<Decimal>, Option<Decimal>) {
        (self.a_maximum.or(self.a_allowed.map(|a| a * Decimal::new(12, 1))), self.b_maximum.or(self.b_allowed.map(|b| b * Decimal::TWO)))
    }
}

/// The kind of a phase of a class made of phases (eventing: Rosson's `Zwischenprüfungen`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseKind {
    Dressage,
    Jumping,
    Cross,
    /// Driving: marathon (Gelände), cones (Hindernisfahren), derby, open, multitest.
    Marathon,
    Cones,
    Derby,
    Open,
    Multitest,
    /// Driving: the presentation (Prüfung A I) judged on its own, standing (Fahrreglement 6.2.1):
    /// each judge's overall mark, 10 − their mean (13.10 §2; S44).
    Presentation,
}

impl PhaseKind {
    /// Rosson's code (`<class>_<code>` is the phase's id).
    pub fn code(self) -> &'static str {
        match self {
            PhaseKind::Dressage => "cd",
            PhaseKind::Jumping => "cs",
            PhaseKind::Cross => "cross",
            PhaseKind::Marathon => "ge",
            PhaseKind::Cones => "hi",
            PhaseKind::Derby => "de",
            PhaseKind::Open => "of",
            PhaseKind::Multitest => "mt",
            PhaseKind::Presentation => "pr",
        }
    }

    /// How the phase is entered and scored: a dressage class, a jumping class, cross-country.
    pub fn discipline(self) -> &'static str {
        match self {
            PhaseKind::Dressage => "CD",
            PhaseKind::Jumping => "CS",
            PhaseKind::Cross => "CROSS",
            PhaseKind::Marathon => "GE",
            PhaseKind::Cones => "HI",
            PhaseKind::Derby => "DE",
            PhaseKind::Open => "OF",
            PhaseKind::Multitest => "MT",
            PhaseKind::Presentation => "PR",
        }
    }

    /// Swiss Equestrian's (and Rosson's) name of the phase: in uploads, and on a driver's horses
    /// for the phases they start in (`einsaetze`).
    pub fn se_name(self) -> &'static str {
        match self {
            PhaseKind::Dressage => "dressur",
            PhaseKind::Jumping => "springen",
            PhaseKind::Cross => "cross",
            PhaseKind::Marathon => "gelaende",
            PhaseKind::Cones => "hindernis",
            PhaseKind::Derby => "derby",
            PhaseKind::Open => "offene",
            PhaseKind::Multitest => "multitest",
            PhaseKind::Presentation => "praesentation",
        }
    }

    /// From Swiss Equestrian's name of a phase (`kombipruefung_disziplinen`) or Rosson's code.
    pub fn from_name(name: &str) -> Option<PhaseKind> {
        match name.to_ascii_lowercase().as_str() {
            "dressur" | "paradressur" | "cd" => Some(PhaseKind::Dressage),
            "springen" | "cs" => Some(PhaseKind::Jumping),
            "cross" => Some(PhaseKind::Cross),
            "gelaende" | "ge" => Some(PhaseKind::Marathon),
            "hindernis" | "hi" => Some(PhaseKind::Cones),
            "derby" | "de" => Some(PhaseKind::Derby),
            "offene" | "of" | "op" => Some(PhaseKind::Open),
            "multitest" | "mt" => Some(PhaseKind::Multitest),
            "praesentation" | "präsentation" | "pr" => Some(PhaseKind::Presentation),
            _ => None,
        }
    }

    /// The order Rosson gives new phases (its `sorter`: dressage, cross, jumping; driving:
    /// dressage, marathon, cones, derby, open, multitest).
    pub fn default_order(self) -> u32 {
        match self {
            // First: the presentation is Prüfung A I, the dressage A II.
            PhaseKind::Presentation => 0,
            PhaseKind::Dressage => 1,
            PhaseKind::Cross => 2,
            PhaseKind::Jumping => 3,
            PhaseKind::Marathon => 4,
            PhaseKind::Cones => 5,
            PhaseKind::Derby => 6,
            PhaseKind::Open => 7,
            PhaseKind::Multitest => 8,
        }
    }
}

/// A class that is a phase of another (eventing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phase {
    /// The class made of the phases (its overall ranking).
    pub of: ClassId,
    pub kind: PhaseKind,
}

/// A class as run at the show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Class {
    pub id: ClassId,
    pub published: PublishedClassId,
    pub number: String,
    pub name: String,
    /// Calendar date, `YYYY-MM-DD`.
    pub date: String,
    pub scoring_table: Option<u32>,
    pub programme: Option<u32>,
    pub options: ClassOptions,
    pub status: ClassStatus,
    /// Additional lines the office adds to the class (sponsors, prizes), printed and sent to
    /// Swiss Equestrian (`weitere_felder`). `None`: never set (not sent); `Some(vec![])`: set to none.
    #[serde(default)]
    pub extra_fields: Option<Vec<ExtraField>>,
    /// Place in the event's order of classes.
    #[serde(default)]
    pub position: Option<u32>,
    /// When the class starts, as announced: "08h00", "à la suite" (with start times: the first).
    #[serde(default)]
    pub time_text: String,
    /// The scoring table as printed for this class, in the event's language (`wertung_text`).
    #[serde(default)]
    pub scoring_text: Option<String>,
    /// The class's logo as Rosson keeps it (sent with the start list). `None`: no logo field;
    /// `Some(null)`: a field without logo (also sent).
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
    pub logo: Option<serde_json::Value>,
    /// Start numbers have been given out for the class.
    #[serde(default)]
    pub start_numbers_assigned: bool,
    /// The teams starting in the class, in their starting order (team classes).
    #[serde(default)]
    pub teams: Vec<TeamId>,
    #[serde(default)]
    pub start_list: StartListSetup,
    /// The sponsor printed on the prize labels (Rosson's `sponsor_etiketten`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sponsor: String,
    /// This class is a phase of another (eventing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<Phase>,
    /// A class made of phases: its phases, in the order that breaks ties in the overall ranking.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<ClassId>,
}

/// A field that is there, even as `null` (serde reads `null` as `None` otherwise).
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<serde_json::Value>, D::Error> {
    serde_json::Value::deserialize(d).map(Some)
}

/// A time of day, in minutes after midnight (Swiss local time).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TimeOfDay(pub u16);

impl TimeOfDay {
    pub fn new(hour: u16, minute: u16) -> TimeOfDay {
        TimeOfDay(hour * 60 + minute)
    }
}

impl std::fmt::Display for TimeOfDay {
    /// `08:30`
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}", self.0 / 60 % 24, self.0 % 60)
    }
}

/// How a class's start list is laid out (class preparation).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartListSetup {
    pub layout: StartListLayout,
    /// A line of text after the n-th starter (team classes: after the n-th team), e.g. arena
    /// maintenance or the title of a section ("Degré Handicap"). Only with start numbers.
    #[serde(default)]
    pub line_after: Option<(u32, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartListLayout {
    /// One list.
    #[default]
    Plain,
    /// Blocks of `size` starters (teams), each under a heading ("08h00", "à la suite").
    Blocks {
        size: u32,
        headings: Vec<String>,
        /// Starters who do not start give their place up (the next ones move up).
        move_up: bool,
    },
    /// A start time per starter (teams): the first at `first`, then every `minutes`; a start
    /// that would run into a break is moved to its end.
    StartTimes {
        first: TimeOfDay,
        minutes: u32,
        breaks: Vec<(TimeOfDay, TimeOfDay)>,
        /// Starters who do not start give their time up.
        move_up: bool,
    },
}

/// An additional line of a class, e.g. "Plaques et flots offerts" / "par l'Assurance …".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtraField {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryStatus {
    Entered,
    /// Withdrawn, refused or a wrong entry: does not start.
    NotStarting,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: EntryId,
    pub published: PublishedClassId,
    pub rider: PersonId,
    pub horse: Option<HorseId>,
    /// The name of a horse without Swiss Equestrian passport (guest horse). Rosson gives all of them the
    /// placeholder horse id 5, so the name belongs to the entry, not to the horse.
    pub guest_horse: Option<String>,
    pub status: EntryStatus,
    pub hors_concours: bool,
    pub team: Option<TeamId>,
    /// Dressage classes where every rider chooses the programme.
    pub programme: Option<u32>,
    /// The entry's horses as written on the entry when it was loaded (`pferde`: name, signalement,
    /// owners, breeder, pedigree, pony data, `einsaetze`), built from Swiss Equestrian's horse
    /// records the way Rosson builds them (`pferdeAnreichern`). Public data; sent with results and
    /// start lists. Like the rider's name on the entry, it can differ between entries of the same
    /// horse (Rosson's update paths build it slightly differently).
    #[serde(default)]
    pub horses_on_entry: Vec<serde_json::Value>,
    /// Swiss Equestrian `stufe` of the entry (as delivered; mostly none).
    #[serde(default)]
    pub stage: Option<i64>,
    /// The rider as named on the entry when it was made. Usually the same as the person record,
    /// but not always (a rider who moved); Rosson sends the entry's copy with results.
    #[serde(default)]
    pub rider_on_entry: Option<RiderOnEntry>,
    /// Refused for lack of places: on the waiting list (Swiss Equestrian status `zurueckgewiesen`).
    #[serde(default)]
    pub waiting_list: bool,
    /// Entry fee paid (`nenngeld_bezahlt`), reported for the waiting list.
    #[serde(default)]
    pub fee_paid: Option<Decimal>,
    /// A place the organiser gives out (`ist_vergebener_startplatz`).
    #[serde(default)]
    pub organiser_place: bool,
    /// Swiss Equestrian's status of the entry (`status`): `definitiv`, `abgemeldet` (withdrawn),
    /// `zurueckgewiesen` (waiting list), `falschnennung` (wrong entry). Empty for entries made at
    /// the show.
    #[serde(default)]
    pub se_status: String,
    /// Who made the entry (`nenner_id`).
    #[serde(default)]
    pub entered_by: Option<PersonId>,
    /// Further horses (driving: up to six in all; `pferd2_id` … `pferd6_id`).
    #[serde(default)]
    pub other_horses: Vec<HorseId>,
    /// Winning points (Gewinnpunkte) of rider, horse and pair, for start orders and lists.
    #[serde(default)]
    pub winning_points: WinningPoints,
    /// Swiss Equestrian's time of the entry's last change (`geaendert_am`).
    #[serde(default)]
    pub se_changed_at: Option<String>,
    /// The rider was changed at the show (Reiterwechsel, printed "RW").
    #[serde(default)]
    pub rider_changed: bool,
    /// The office's own note on the entry (not sent anywhere).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
    /// The record as Swiss Equestrian delivered it, in Rosson's shape, without what is not
    /// public: kept to write the show in Rosson's format (`rosson_import::to_rosson`).
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub record: serde_json::Value,
}

/// Winning points (Gewinnpunkte, `gwp_reiter`, `gwp_pferd`, `gwp_paar`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WinningPoints {
    pub rider: Option<Decimal>,
    pub horse: Option<Decimal>,
    pub pair: Option<Decimal>,
}

/// Non-public data of a person: address, phones, email, birth date. Only the show office needs
/// it; it travels in its own operations (`Change::PrivateDataDelivered`) so that it can be kept
/// off devices and out of exports that do not need it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    pub person: PersonId,
    /// `anrede`
    pub salutation: String,
    pub street: String,
    pub postcode: String,
    /// Country code, e.g. "CH" (`land`).
    pub country: String,
    pub email: String,
    pub phone_private: String,
    pub phone_mobile: String,
    /// Correspondence language (`sprache`: D, F, I).
    pub language: String,
    /// `YYYY-MM-DD`
    pub birth_date: Option<String>,
}

/// Non-public notes on an entry, written by whoever entered: remarks to the organiser
/// (`bemerkungen`) and an accompanying person (`begleit_person`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryNote {
    pub entry: EntryId,
    pub remarks: String,
    pub companion: String,
}

/// A rider's name and town as written on an entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiderOnEntry {
    pub first_name: String,
    pub last_name: String,
    pub town: String,
    #[serde(default)]
    pub club: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Team {
    pub id: TeamId,
    pub published: PublishedClassId,
    pub name: String,
    #[serde(default)]
    pub start_number: Option<u32>,
}

/// An entry's place in a class's start list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Start {
    pub entry: EntryId,
    /// Start order; in team classes the rider's position in the team rotation (1st riders of all
    /// teams, then 2nd riders, …).
    pub order: u32,
    pub start_number: Option<u32>,
    /// Team classes: the team's place in the rotation.
    #[serde(default)]
    pub team_order: Option<u32>,
}

/// What was entered for one entry in one class: the score card.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScoreCard {
    /// Jumping fields as in the scoring tables (`strafpunkte_initial_1`, `zeit_stechen_1`, ...).
    pub values: BTreeMap<String, Decimal>,
    /// Eliminated, retired, withdrawn: the reason code (Aufgabegrund).
    pub out: Option<u32>,
}

/// Who is ranked in a class: a rider's entry, or a team in team classes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Competitor {
    Entry(EntryId),
    Team(TeamId),
}

impl Competitor {
    /// The id the scoring engine uses for it: the entry id, or the team id.
    pub fn scoring_id(&self) -> String {
        match self {
            Competitor::Entry(e) => e.0.to_string(),
            Competitor::Team(t) => t.0.clone(),
        }
    }
}

/// A rank set by hand by the show office (docs/deviations.md S18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualRank {
    pub rank: u32,
    /// Why, for the record (e.g. "jury decision", "rank from the timekeeper's file").
    pub reason: String,
}

/// One dressage judge's sheet for one entry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct JudgeSheet {
    pub marks: BTreeMap<String, Decimal>,
    /// Ticked penalties: points (positive) or percentages. One set for the rider, the same on
    /// every judge's sheet (S40).
    pub penalty_points: Vec<Decimal>,
    pub penalty_percent: Vec<Decimal>,
    /// The programme's exclusion box ticked (more errors of course than it allows): the rider is
    /// eliminated (Dressurreglement 8.3 (4); S40).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub excluded: bool,
    /// The judge's total as entered (classes with totals per judge, Rosson's `manuelle_eingabe`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    pub id: AssignmentId,
    /// `None`: assigned to the whole show.
    pub class: Option<ClassId>,
    pub person: PersonId,
    /// Swiss Equestrian role code, e.g. "DRI" (dressage judge).
    pub role: String,
    /// Judge position (E, H, C, M, B).
    pub letter: Option<String>,
    /// Swiss Equestrian role id (`rolle_id`, `funktion_id`).
    #[serde(default)]
    pub role_id: Option<u32>,
    /// The event the assignment belongs to (`veranstaltung_id`).
    #[serde(default)]
    pub event: Option<EventId>,
    /// Swiss Equestrian's id of the assignment (`equis_id`), once known (`lookupZuweisung`).
    #[serde(default)]
    pub se_id: Option<u64>,
    /// Assigned to the event itself (`auf_veranstaltung`), not to a class.
    #[serde(default)]
    pub on_event: Option<bool>,
    /// Responsible person for the role (`verantwortlicher`).
    #[serde(default)]
    pub responsible: Option<bool>,
    /// Delivered by Swiss Equestrian, not editable at the show (`readonly`).
    #[serde(default)]
    pub readonly: Option<bool>,
    /// Days of work (`anzahl_einsatztage`).
    #[serde(default)]
    pub days: Option<u32>,
    /// Last change, milliseconds since 1970 (`updated_at`; Rosson sends it to Swiss Equestrian).
    #[serde(default)]
    pub updated_at: Option<u64>,
    /// Swiss Equestrian was asked for its id (`lookupZuweisung`), with or without an answer;
    /// asked again only when the assignment changes.
    #[serde(default)]
    pub looked_up: bool,
}

impl Assignment {
    /// A Swiss Equestrian role, sent to it; EquiEvents' own roles (the style judge) never are.
    pub fn for_swiss_equestrian(&self) -> bool {
        !is_own_role(&self.role)
    }
}

/// The style judge: who fills a style class's style sheets. EquiEvents' own role, unknown to
/// Swiss Equestrian and never sent to it. Rosson gave style judges Swiss Equestrian's "R",
/// which is the jumping judge (deviation S21).
pub const STYLE_JUDGE: &str = "eq-style";

/// EquiEvents' own roles' codes start with "eq-" (Swiss Equestrian's are letters).
pub fn is_own_role(code: &str) -> bool {
    code.starts_with("eq-")
}

/// EquiEvents' own roles in a class, after Swiss Equestrian's (`id` 0: they have none there).
pub fn own_class_roles() -> Vec<RoleDef> {
    vec![RoleDef { id: 0, code: STYLE_JUDGE.into(), in_class: true, name_de: "Stilrichter".into(), name_fr: "Juge de style".into(), name_it: "Giudice di stile".into() }]
}

/// A role an official can have (Swiss Equestrian master data): in a class (Prüfungsfunktion,
/// e.g. P president, R judge, DRI dressage judge) or at the event (Veranstaltungsfunktion).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleDef {
    pub id: u32,
    pub code: String,
    /// In a class (else at the event).
    pub in_class: bool,
    pub name_de: String,
    pub name_fr: String,
    pub name_it: String,
}

/// An official of Swiss Equestrian's national list (`getOffizielle`): who can be given a role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Official {
    pub person: PersonId,
    pub first_name: String,
    pub last_name: String,
    pub town: String,
    pub canton: String,
    /// The roles the official holds, by their German titles (`rollen[].rolle_bezeichnung_de`).
    pub roles: Vec<String>,
    /// The same in French and Italian (`rolle_bezeichnung_fr`, `_it`), one for each role; empty
    /// in a show that kept only the German.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles_fr: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles_it: Vec<String>,
}

/// What the office changes on an entry (`Change::EntryEdited`); what is `None` stays as it is.
/// The horses of a driving entry that start in a phase (`einsaetze` naming it).
pub fn horses_in_phase(entry: &Entry, kind: PhaseKind) -> Vec<&serde_json::Value> {
    entry.horses_on_entry.iter().filter(|h| h["einsaetze"].as_array().is_some_and(|p| p.iter().any(|x| x.as_str() == Some(kind.se_name())))).collect()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntryEdit {
    /// Does not start, or starts again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<EntryStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hors_concours: Option<bool>,
    /// A change of horse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horse: Option<HorseOnEntry>,
    /// A change of rider (Reiterwechsel), or back to the rider entered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rider: Option<RiderChange>,
    /// The entry moved to another published class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published: Option<PublishedClassId>,
    /// The level (Stufe); `Some(0)`: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<i64>,
    /// The office's note (`Some("")` clears it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Driving: the phases each horse of the entry starts in, horse by horse (`einsaetze`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horse_phases: Option<Vec<Vec<String>>>,
}

/// Who rides an entry, and what is written on it about them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiderChange {
    pub rider: PersonId,
    pub rider_on_entry: Option<RiderOnEntry>,
    /// Not the rider entered (printed "RW").
    pub changed: bool,
}

/// The horse an entry starts with, and what is written on the entry about it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HorseOnEntry {
    pub horse: Option<HorseId>,
    /// A horse without Swiss Equestrian passport, by name.
    pub guest_horse: Option<String>,
    /// As Rosson writes a horse on an entry (`Entry::horses_on_entry`).
    pub horses_on_entry: Vec<serde_json::Value>,
}

/// Where a judge's sheet is: on a rider, for a judge; or put aside (`Change::SheetMoved`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SheetPlace {
    Rider { entry: EntryId, judge: AssignmentId },
    /// Put aside in the class, in a numbered place, to be placed later.
    Aside { slot: u32 },
}

/// Ticked penalties of a sheet: points and percentages, and the exclusion box (as `JudgeSheet`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SheetPenalties {
    pub points: Vec<rust_decimal::Decimal>,
    pub percent: Vec<rust_decimal::Decimal>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub excluded: bool,
}

/// What the office sent to Swiss Equestrian (`Change::Uploaded`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Upload {
    /// A class's results (`putResultate`).
    Results(ClassId),
    /// A published class's start lists (`putStartlisten`).
    StartLists(PublishedClassId),
    /// A published class's start lists withdrawn (`deleteStartlisten`).
    StartListsWithdrawn(PublishedClassId),
    /// The event: officials, entry counts (`putVeranstaltung`).
    Event,
    /// A ranking over several classes (a final, by its id): its results (`putResultate`).
    Combined(String),
}

/// When and by whom something was last sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sent {
    pub millis: u64,
    pub author: String,
    /// The operation that recorded it.
    pub op: String,
}

/// Swiss Equestrian master data, versioned by what the show received.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoringTableDef {
    pub id: u32,
    pub code: String,
    /// In German, as Swiss Equestrian names it; in French and Italian when it gave those.
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name_fr: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name_it: String,
    /// The published definition, verbatim.
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgrammeDef {
    pub id: u32,
    pub name: String,
    pub max_points: Decimal,
    /// The published definition (movements, coefficients, penalties), verbatim.
    pub definition: String,
    /// Swiss Equestrian's discipline of the test (`disziplin_id`): a class is offered its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discipline_id: Option<u32>,
}

/// A Swiss Equestrian category (Prüfungskategorie), e.g. "R/N120" or a dressage programme's category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryDef {
    pub id: u32,
    pub code: String,
    /// The programme this category stands for (dressage), if any.
    pub programme: Option<u32>,
    /// The record as Swiss Equestrian delivered it: Rosson sends it whole in one place (se-api).
    pub record: serde_json::Value,
}

/// A Swiss Equestrian reason for being out (Aufgabegrund).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasonDef {
    pub id: u32,
    /// Discipline code the reason belongs to (CS, CD, ...); `None`: general reasons.
    #[serde(default)]
    pub discipline: Option<String>,
    /// German label, e.g. "ausgeschieden"; its kind is derived from it (`master::reason_kind`).
    pub name: String,
}

/// A ranking over several classes (U8), e.g. a championship over two or three classes. Who is
/// in it: a rider with the same horse in every class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombinedRanking {
    pub id: String,
    pub name: String,
    /// In the order they are ridden: the last one breaks ties of penalties.
    pub classes: Vec<ClassId>,
    pub method: CombinedMethod,
    /// Its number, as a class's (Rosson: the first class's with ".1", e.g. "6.1"): the number
    /// Swiss Equestrian gets its results under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    /// Lines printed with it and sent with its results (Rosson's `weitereFelder`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_fields: Vec<ExtraField>,
    /// Rosson's record of it (`finalpruefungen`), when it came from Rosson: written back as it
    /// was, with what the show changed.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub record: serde_json::Value,
}

/// What breaks a tie of penalties added over several classes (decided by the user, 2026-11-15:
/// nothing by default).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum PenaltiesTie {
    /// Nothing: tied riders share the rank (ex aequo). The default; `last_time` (the time in the
    /// last class, offered for a day, then taken away by the user) reads as this.
    #[default]
    #[serde(alias = "last_time")]
    None,
    /// The penalties in one class, the fewest first; ties may remain.
    Faults { class: ClassId },
    /// The result in one class: its penalties, then its time.
    Result { class: ClassId },
}

impl PenaltiesTie {
    pub fn is_ex_aequo(&self) -> bool {
        *self == PenaltiesTie::None
    }

    /// The class that breaks ties, when one is chosen.
    pub fn class(&self) -> Option<&ClassId> {
        match self {
            PenaltiesTie::Faults { class } | PenaltiesTie::Result { class } => Some(class),
            _ => None,
        }
    }
}

#[cfg(test)]
mod penalties_tie_tests {
    use super::*;

    #[test]
    fn ex_aequo_unless_chosen() {
        // Not set (rankings made before the choice), or the time in the last class (offered for
        // a day): ex aequo; written without it.
        for old in [r#"{"kind":"penalties"}"#, r#"{"kind":"penalties","tie":{"by":"last_time"}}"#] {
            assert_eq!(serde_json::from_str::<CombinedMethod>(old).unwrap(), CombinedMethod::Penalties { tie: PenaltiesTie::None });
        }
        assert_eq!(serde_json::to_string(&CombinedMethod::Penalties { tie: PenaltiesTie::None }).unwrap(), r#"{"kind":"penalties"}"#);
        // Percentages: Rosson's (no class chosen) as before.
        assert_eq!(serde_json::from_str::<CombinedMethod>(r#"{"kind":"percentages"}"#).unwrap(), CombinedMethod::Percentages { tie_break: None });
        let result = CombinedMethod::Penalties { tie: PenaltiesTie::Result { class: ClassId("1".into()) } };
        assert_eq!(serde_json::from_str::<CombinedMethod>(&serde_json::to_string(&result).unwrap()).unwrap(), result);
    }
}

/// How a ranking over several classes adds up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CombinedMethod {
    /// The penalties of every class added; a tie broken as `tie` says (not set: ex aequo).
    Penalties {
        #[serde(default, skip_serializing_if = "PenaltiesTie::is_ex_aequo")]
        tie: PenaltiesTie,
    },
    /// The ranks in every class added; a tie is broken by the rank in `tie_break`, then in the
    /// other classes in order (Rosson's *Rangpunkte*).
    Ranks { tie_break: ClassId },
    /// Dressage: the mean of the classes' percentages, the highest first; a tie is broken by the
    /// percentage in `tie_break` (chosen, 2026-11-15), then in the other classes in order; not
    /// chosen (Rosson's *Prozentpunkte*): the classes in order.
    Percentages {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tie_break: Option<ClassId>,
    },
}
