//! The operation log: every change to a show is an operation, appended and never edited.
//!
//! Operations are ordered by a hybrid logical clock ([`Stamp`]): wall-clock time, a counter for
//! operations in the same millisecond, and the device id to break the last ties. Every device
//! applying the same set of operations in stamp order ends up with the same state, whatever
//! order they arrived in.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::model::*;

/// Hybrid logical clock stamp. Sorts in time order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Stamp {
    pub millis: u64,
    pub counter: u32,
    pub device: String,
}

/// A device's clock. Never goes backwards, also when the wall clock does.
#[derive(Debug, Clone)]
pub struct Clock {
    device: String,
    last: (u64, u32),
}

impl Clock {
    pub fn new(device: impl Into<String>) -> Clock {
        Clock { device: device.into(), last: (0, 0) }
    }

    /// A stamp for a new local operation at wall-clock time `now_millis`.
    pub fn tick(&mut self, now_millis: u64) -> Stamp {
        self.last = if now_millis > self.last.0 { (now_millis, 0) } else { (self.last.0, self.last.1 + 1) };
        Stamp { millis: self.last.0, counter: self.last.1, device: self.device.clone() }
    }

    /// Take a stamp received from another device into account, so later local stamps sort after it.
    pub fn observe(&mut self, seen: &Stamp) {
        if (seen.millis, seen.counter) > self.last {
            self.last = (seen.millis, seen.counter);
        }
    }
}

/// How far ahead of the hub's clock a received operation may be (rule R5,
/// docs/architecture/reconciliation.md).
pub const MAX_CLOCK_LEAD_MILLIS: u64 = 5 * 60 * 1000;

/// Rule R5: ids of operations stamped too far in the future of `now_millis` (the hub's clock).
pub fn too_far_ahead(ops: &[Operation], now_millis: u64) -> Vec<String> {
    ops.iter().filter(|o| o.stamp.millis > now_millis + MAX_CLOCK_LEAD_MILLIS).map(|o| o.id.clone()).collect()
}

/// One field of a score card changed by hand (rule R1): the value the author saw and the new one.
/// The field `out` holds the reason code for being out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldChange {
    pub field: String,
    pub from: Option<Decimal>,
    pub to: Option<Decimal>,
}

/// How the apps sync operations (sent as `X-EquiEvents-Sync`). 2: operations carry what their
/// author saw (`saw`, R12–R14); equievents.ch takes changes from version 2 on only, else clashes
/// with them would go unsaid (the user, 2026-11-22).
pub const SYNC_VERSION: u32 = 2;

/// One change to the show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    /// Unique, stable across devices.
    pub id: String,
    pub stamp: Stamp,
    /// Who did it: a person, a device code, or the importer.
    pub author: String,
    pub change: Change,
    /// What its author saw of the things the change sets (rule R12, `touched::Saw`): a thing
    /// that is something else when the operation is applied was changed meanwhile, unknown to
    /// the author: a conflict. Empty: not looked at (imports, deliveries, operations made before
    /// 2026-11-22).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub saw: crate::touched::Saw,
}

/// What an operation changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
// `EventImported` is much larger than the others, and rare (once per delivery): not boxed.
#[allow(clippy::large_enum_variant)]
pub enum Change {
    /// The event as delivered by Swiss Equestrian (or imported from Rosson): replaces what it contains.
    EventImported {
        event: Event,
        published_classes: Vec<PublishedClass>,
        persons: Vec<Person>,
        horses: Vec<Horse>,
        entries: Vec<Entry>,
        teams: Vec<Team>,
        scoring_tables: Vec<ScoringTableDef>,
        programmes: Vec<ProgrammeDef>,
        #[serde(default)]
        reasons: Vec<ReasonDef>,
        #[serde(default)]
        categories: Vec<CategoryDef>,
        /// Swiss Equestrian's master data as delivered, in Rosson's stores (`wertungen`,
        /// `programme`, …; the national list of officials without what is not public): kept to
        /// write the show in Rosson's format. Replaces the stores it contains.
        #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        master: std::collections::BTreeMap<String, Vec<serde_json::Value>>,
    },
    /// Non-public data delivered with the event (the event key unlocks it): contacts of riders
    /// and officials, notes on entries. Replaces what it contains.
    PrivateDataDelivered { contacts: Vec<Contact>, notes: Vec<EntryNote> },
    /// Swiss Equestrian delivered a new version of an entry (an update), or a new entry. Its
    /// changes are merged with the office's (`State::merge_delivered`).
    EntryDelivered { entry: Entry },
    /// The office changes an entry (does not start, hors concours, another horse).
    EntryEdited { entry: EntryId, edit: EntryEdit },
    /// A class set up or changed. `seen`: the class as its author saw it (rule R11): only what
    /// they changed is applied, field by field, over what others changed meanwhile. `None`: the
    /// class as given (a class set up, imports, operations made before 2026-11-16).
    ClassSet {
        class: Class,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seen: Option<Class>,
    },
    /// A class is no longer run (e.g. replaced by its sections). Refused once it has results.
    ClassRemoved { class: ClassId },
    /// Swiss Equestrian answered a lookup for an assignment (`lookupZuweisung`), with its id or
    /// without.
    AssignmentLookedUp { assignment: AssignmentId, se_id: Option<u64> },
    /// A class's start list as its author made it. `seen`: the list they saw (rule R7): their
    /// moves and numbers are applied over what others changed meanwhile, entry by entry. `None`:
    /// the list as given (imports, operations made before 2026-11-16).
    StartsSet {
        class: ClassId,
        starts: Vec<Start>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seen: Option<Vec<Start>>,
        /// Entries put at the end with the next number (a late entry): the number is the next one
        /// when the change is applied, also after others gave numbers meanwhile.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        next_numbers: Vec<EntryId>,
        /// The class numbered: riders its author did not know get the next numbers.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        numbered: bool,
    },
    AssignmentSet { assignment: Assignment },
    /// An official no longer has a role (in a class, or at the event).
    AssignmentRemoved { assignment: AssignmentId },
    /// Swiss Equestrian's roles of officials (master data): replace the show's.
    RolesDelivered { roles: Vec<RoleDef> },
    /// Swiss Equestrian's national list of officials: replaces the one the show had.
    OfficialsDelivered { officials: Vec<Official> },
    /// Fields of a score card changed by hand (rules R1, R2).
    ResultEdited { class: ClassId, entry: EntryId, changes: Vec<FieldChange> },
    /// The whole score card of an entry in a class, as now entered (imports, timing files).
    ResultEntered { class: ClassId, entry: EntryId, values: BTreeMap<String, Decimal>, out: Option<u32> },
    /// An entry has no result in a class any more (all of it cleared, or an entry undone).
    ResultRemoved { class: ClassId, entry: EntryId },
    JudgeSheetEntered { class: ClassId, entry: EntryId, judge: AssignmentId, sheet: JudgeSheet },
    /// Marks of one judge's sheet entered or cleared (`None`), one or a few at a time; the
    /// penalties replaced when given. Two people entering other marks do not clash.
    SheetEdited {
        class: ClassId,
        entry: EntryId,
        judge: AssignmentId,
        marks: BTreeMap<String, Option<Decimal>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        penalties: Option<SheetPenalties>,
        /// The judge's total as entered (totals per judge); `Some(None)`: cleared.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        total: Option<Option<Decimal>>,
    },
    /// A judge's sheet moved to another rider or judge, or put aside, or placed from aside;
    /// a sheet already there takes its place (the two are swapped). Marks are never lost.
    SheetMoved { class: ClassId, from: SheetPlace, to: SheetPlace },
    /// A whole judge's sheet put in a place (`Some`), or thrown away (`None`): a sheet deleted,
    /// on a rider or cut, and its undo.
    SheetSet { class: ClassId, place: SheetPlace, sheet: Option<JudgeSheet> },
    /// Team dressage: marks of one judge's sheet for a team, as [`Change::SheetEdited`].
    TeamSheetEdited {
        class: ClassId,
        team: TeamId,
        judge: AssignmentId,
        marks: BTreeMap<String, Option<Decimal>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        penalties: Option<SheetPenalties>,
    },
    /// Team-only classes: the team's score card.
    TeamResultEntered { class: ClassId, team: TeamId, values: BTreeMap<String, Decimal>, out: Option<u32> },
    /// The office sets a rank by hand (`Some`), or removes it (`None`) (docs/deviations.md S18).
    RankSet { class: ClassId, competitor: Competitor, rank: Option<u32>, reason: String },
    /// Eventing: penalties the jury gives a rider in a phase (25 for a yellow card, dangerous
    /// riding, mistreatment), added to the phase's penalties overall (`None`: taken back;
    /// Eventing-Reglement 2.2, 8.4, 8.5; docs/deviations.md S41).
    JuryPenaltySet { class: ClassId, entry: EntryId, points: Option<Decimal> },
    /// The class's results are official. `based_on`: the operations on its results the
    /// finalising device knew (rule R4).
    ClassFinalised {
        class: ClassId,
        #[serde(default)]
        based_on: Vec<String>,
    },
    ClassReopened { class: ClassId, reason: String },
    /// Swiss Equestrian accepted an upload (or a withdrawal of start lists). `based_on`: the
    /// operations on what was sent the sending device knew (R14, `State::fill_uploads`): one
    /// made before, unknown to it, means Swiss Equestrian does not have it. `None`: not known
    /// (operations made before 2026-11-22).
    Uploaded {
        upload: Upload,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        based_on: Option<Vec<String>>,
    },
    /// An entry made by the office (a late entry, an organiser's place).
    EntryAdded { entry: Entry },
    /// An office entry taken back.
    EntryRemoved { entry: EntryId },
    /// A rider or horse the office typed in (not delivered by Swiss Equestrian): kept if one
    /// with that id is known already.
    PersonAdded { person: Person },
    HorseAdded { horse: Horse },
    /// A rider's address and contacts set by the office (Rosson: written on all their entries,
    /// kept when Swiss Equestrian delivers them again).
    AddressSet { person: PersonId, town: String, contact: Contact },
    /// A team made or renamed at the show (team classes: a late entry's new team).
    TeamSet { team: Team },
    /// A team made at the show taken away again (its last entry gone).
    TeamRemoved { team: TeamId },
    /// Start numbers fixed for the whole event (one per entry), and whether they are given out.
    EventNumbersSet { fixed: bool, assigned: bool },
    /// A published class's prizes set (`None`: none).
    PrizesSet { published: PublishedClassId, prizes: Option<Prizes> },
    /// A published class changed at the show (as in Rosson): its category as printed, how many
    /// levels it has; what is `None` stays as it is.
    PublishedClassEdited {
        published: PublishedClassId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        category_text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        levels: Option<u32>,
    },
    /// A dressage test's definition changed at the show (Rosson's developer mode: when Swiss
    /// Equestrian's is wrong).
    ProgrammeSet { programme: ProgrammeDef },
    /// A scoring table's definition changed at the show (Rosson's developer mode).
    ScoringTableSet { table: ScoringTableDef },
    /// A ranking over several classes set up or changed (U8).
    CombinedSet { ranking: CombinedRanking },
    CombinedRemoved { id: String },
}

impl Change {
    /// A class set up or changed: what its author saw is filled in when it becomes an operation
    /// (`State::fill_seen`).
    pub fn class_set(class: Class) -> Change {
        Change::ClassSet { class, seen: None }
    }

    /// A class's start list as its author made it: what they saw is filled in when it becomes an
    /// operation (`State::fill_seen`).
    pub fn starts_set(class: ClassId, starts: Vec<Start>) -> Change {
        Change::StartsSet { class, starts, seen: None, next_numbers: Vec::new(), numbered: false }
    }
}
