//! The current state of a show, computed from its operation log.
//!
//! Operations are applied in stamp order, following the reconciliation rules in
//! docs/architecture/reconciliation.md (R1–R4, R7, R11 and R12 here). Every device makes the same decisions,
//! because they apply the same operations in the same order.

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;

use crate::model::*;
use crate::oplog::{Change, Operation, Stamp};

/// Something the show office should look at: two devices changed the same thing (R2, R7,
/// R11), or a change reached a class after it was finalised (R4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// Stable on every device: the operation, then its place among that operation's conflicts.
    pub id: String,
    /// The operation that caused it (the later change).
    pub op: String,
    pub class: ClassId,
    pub kind: ConflictKind,
    pub detail: String,
    /// The earlier change it went over, made apart and unknown to its author (when known).
    pub over: Option<String>,
    /// What both changed: a class's fields (`name`, `options.placed_count`), riders (entry ids)
    /// moved or numbered on both, a result's field, start numbers given twice.
    pub what: Vec<String>,
    /// What was replaced: by field, or by entry id (a rider's start number); R12: by thing's key.
    pub lost: serde_json::Value,
    /// R12: the things both changed (`what` holds their keys).
    pub things: Vec<crate::touched::Thing>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// R2: the field had changed since the author saw it. The later change was applied.
    ConcurrentEdit,
    /// R4: made before the class was finalised but unknown to the finalisation. Not applied.
    LateForFinalClass,
    /// R11: fields of a class changed on two devices. The later change was applied.
    ClassEdited,
    /// R7: riders moved in the start list on two devices. The later places were kept.
    StartOrder,
    /// R7: start numbers given or changed on two devices. The later numbers were kept.
    StartNumbers,
    /// R7: two riders of a start list have the same number after putting changes together.
    DuplicateNumbers,
    /// A class changed on one device was taken away on another (e.g. sections put back
    /// together). The change was not applied.
    ClassGone,
    /// R12: something else (an entry's horse, an official's role, a judge's mark…) changed on
    /// two devices. The later change was applied.
    Changed,
    /// R12: what a change set had changed meanwhile and is gone, or its class was finalised
    /// meanwhile. The change was not applied.
    ChangedGone,
    /// R13: what a change relies on changed meanwhile on another device (a result entered for a
    /// rider withdrawn there…). Both were applied; they may not go together.
    Basis,
    /// R14: changes made before something was sent to Swiss Equestrian, unknown to the send
    /// (made offline): Swiss Equestrian does not have them; to send again.
    LateForUpload,
    /// R8: Swiss Equestrian's new version of an entry replaced a change of the office.
    Delivered,
}

impl ConflictKind {
    /// Its name for the app.
    pub fn code(self) -> &'static str {
        match self {
            ConflictKind::ConcurrentEdit => "result",
            ConflictKind::LateForFinalClass => "late_final",
            ConflictKind::ClassEdited => "class",
            ConflictKind::StartOrder => "order",
            ConflictKind::StartNumbers => "numbers",
            ConflictKind::DuplicateNumbers => "duplicates",
            ConflictKind::ClassGone => "class_gone",
            ConflictKind::Changed => "change",
            ConflictKind::ChangedGone => "change_gone",
            ConflictKind::Basis => "basis",
            ConflictKind::LateForUpload => "late_upload",
            ConflictKind::Delivered => "delivered",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub event: Option<Event>,
    pub published_classes: BTreeMap<PublishedClassId, PublishedClass>,
    pub persons: BTreeMap<PersonId, Person>,
    /// Non-public: contacts and notes on entries (`Change::PrivateDataDelivered`).
    pub contacts: BTreeMap<PersonId, Contact>,
    pub entry_notes: BTreeMap<EntryId, EntryNote>,
    pub horses: BTreeMap<HorseId, Horse>,
    pub entries: BTreeMap<EntryId, Entry>,
    /// Each entry as Swiss Equestrian last delivered it, to merge the next delivery with the
    /// office's changes (`delivered::merge`).
    pub delivered_entries: BTreeMap<EntryId, Entry>,
    /// What deliveries changed that the office had changed too.
    pub notices: Vec<crate::delivered::Notice>,
    pub teams: BTreeMap<TeamId, Team>,
    pub scoring_tables: BTreeMap<u32, ScoringTableDef>,
    pub programmes: BTreeMap<u32, ProgrammeDef>,
    pub reasons: BTreeMap<u32, ReasonDef>,
    pub categories: BTreeMap<u32, CategoryDef>,
    /// Swiss Equestrian's master data as delivered, by Rosson store (`Change::EventImported`).
    pub master_records: BTreeMap<String, Vec<serde_json::Value>>,
    /// People whose address the office set: later deliveries keep it (Rosson's rule).
    pub addresses_set: BTreeSet<PersonId>,
    pub classes: BTreeMap<ClassId, Class>,
    pub starts: BTreeMap<ClassId, Vec<Start>>,
    pub assignments: BTreeMap<AssignmentId, Assignment>,
    /// Roles of officials, in a class or at the event (by role id; ids of the two kinds overlap).
    pub roles: Vec<RoleDef>,
    /// Swiss Equestrian's national list of officials, to give roles from.
    pub officials: BTreeMap<PersonId, Official>,
    pub results: BTreeMap<(ClassId, EntryId), ScoreCard>,
    pub sheets: BTreeMap<(ClassId, EntryId, AssignmentId), JudgeSheet>,
    /// Judges' sheets put aside, per class and place.
    pub aside: BTreeMap<(ClassId, u32), JudgeSheet>,
    pub team_results: BTreeMap<(ClassId, TeamId), ScoreCard>,
    /// Team dressage: a sheet per team and judge.
    pub team_sheets: BTreeMap<(ClassId, TeamId, AssignmentId), JudgeSheet>,
    /// Rankings over several classes (U8), by id.
    pub combined: BTreeMap<String, CombinedRanking>,
    /// Prizes, by published class.
    pub prizes: BTreeMap<PublishedClassId, Prizes>,
    /// What was last sent to Swiss Equestrian, when and by whom. Withdrawn start lists are no
    /// longer listed as sent.
    pub uploaded: BTreeMap<Upload, Sent>,
    /// Ranks set by hand (S18).
    pub manual_ranks: BTreeMap<(ClassId, Competitor), ManualRank>,
    /// Eventing: the jury's penalties for a rider in a phase (S41).
    pub jury_penalties: BTreeMap<(ClassId, EntryId), Decimal>,
    /// When a class's results last changed (ms): sent to Swiss Equestrian before that, they
    /// need sending again.
    pub results_changed: BTreeMap<ClassId, u64>,
    /// When a published class's start lists last changed (ms), as far as the show can tell
    /// (order, numbers, times, who starts, the class, its officials).
    pub start_lists_changed: BTreeMap<PublishedClassId, u64>,
    pub conflicts: Vec<Conflict>,
    /// Operations applied to each class's results (what a finalisation is based on, R4).
    pub class_inputs: BTreeMap<ClassId, BTreeSet<String>>,
    /// Operations that changed what is sent of a class's results, of a published class's start
    /// lists (what an upload is based on, R14).
    result_inputs: BTreeMap<ClassId, BTreeSet<String>>,
    list_inputs: BTreeMap<PublishedClassId, BTreeSet<String>>,
    /// Finalisations in the log: class, stamp, the operations they were based on (R4).
    finalisations: Vec<(ClassId, Stamp, BTreeSet<String>)>,
    /// The operation that last changed each thing a conflict can be about (a class's field, a
    /// rider's place or number in a start list, a result's field): what a conflict went over.
    writers: BTreeMap<String, (Stamp, String)>,
    /// Stamp of the last applied operation.
    pub last: Option<Stamp>,
    /// Stamps of the operations what is sent is made of (R14's conflicts: the latest missing).
    stamp_of: BTreeMap<String, Stamp>,
}

/// What happened to an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Applied,
    /// Not applied, with the reason (e.g. the class was already final).
    Ignored(String),
}

impl State {
    /// The roles an official can have in a class: Swiss Equestrian's, then EquiEvents' own.
    pub fn class_roles(&self) -> Vec<RoleDef> {
        self.roles.iter().filter(|r| r.in_class).cloned().chain(own_class_roles()).collect()
    }

    /// Fold a log into a state. The operations are sorted by stamp first.
    pub fn from_log(mut ops: Vec<Operation>) -> (State, Vec<(String, String)>) {
        ops.sort_by(|a, b| a.stamp.cmp(&b.stamp));
        let finalisations = ops
            .iter()
            .filter_map(|o| match &o.change {
                Change::ClassFinalised { class, based_on } => Some((class.clone(), o.stamp.clone(), based_on.iter().cloned().collect())),
                _ => None,
            })
            .collect();
        let mut state = State { finalisations, ..State::default() };
        let mut ignored = Vec::new();
        for op in &ops {
            if let Outcome::Ignored(why) = state.apply(op) {
                ignored.push((op.id.clone(), why));
            }
        }
        (state, ignored)
    }

    /// Apply one operation. Operations must come in stamp order.
    pub fn apply(&mut self, op: &Operation) -> Outcome {
        // R12, R13: what changed meanwhile, unknown to its author (before it is applied).
        let (clashes, basis) = self.clashes(op);
        let before = self.conflicts.len();
        let outcome = match class_of_results(&op.change) {
            Some(class) => {
                // R4: made before a finalisation that did not know it.
                let late = self.finalisations.iter().find(|(c, stamp, _)| c == class && *stamp > op.stamp).filter(|(_, _, based_on)| !based_on.contains(&op.id));
                if late.is_some() {
                    self.conflict(op, class, ConflictKind::LateForFinalClass, "made before the class was finalised, but not known to the finalisation".into(), None, Vec::new(), serde_json::Value::Null);
                    Outcome::Ignored(format!("late change for final class {}", self.class_number(class)))
                } else {
                    let outcome = self.apply_change(op);
                    if outcome == Outcome::Applied {
                        self.class_inputs.entry(class.clone()).or_default().insert(op.id.clone());
                    }
                    outcome
                }
            }
            None => self.apply_change(op),
        };
        // Not a late change for a final class (R4 says it). Applied, what it made what was
        // there already replaced nothing (both gave the same horse); what it relies on changed:
        // both kept (R13). Not applied: said with all of it.
        let applied = outcome == Outcome::Applied;
        let mut clashes: Vec<_> = clashes.into_iter().filter(|(t, before)| !applied || &t.value(self) != before).collect();
        if !self.conflicts[before..].iter().any(|c| c.kind == ConflictKind::LateForFinalClass) {
            if applied {
                self.clashed(op, basis, ConflictKind::Basis);
            } else {
                clashes.extend(basis);
            }
            self.clashed(op, clashes, if applied { ConflictKind::Changed } else { ConflictKind::ChangedGone });
        }
        if outcome == Outcome::Applied {
            self.note_changed(op);
            for t in crate::touched::things(&op.change) {
                self.wrote(t.key(), op);
            }
            for key in crate::touched::basis_written(&op.change) {
                self.wrote(key, op);
            }
            // Swiss Equestrian's new version of an entry: what an office change may go over.
            if let Change::EntryDelivered { entry } = &op.change {
                self.wrote(format!("e/{}", entry.id), op);
            }
        }
        self.last = Some(op.stamp.clone());
        outcome
    }

    /// Note what a change changes of what is sent to Swiss Equestrian (results, start lists):
    /// when (for « to send again »), and that it did (what a later send is based on, R14).
    fn note_changed(&mut self, op: &Operation) {
        let (change, millis) = (&op.change, op.stamp.millis);
        let mut results: Vec<ClassId> = Vec::new();
        let mut lists: Vec<PublishedClassId> = Vec::new();
        let published_of = |s: &State, c: &ClassId| s.classes.get(c).map(|c| c.published.clone());
        match change {
            Change::ResultEdited { class, .. }
            | Change::ResultEntered { class, .. }
            | Change::ResultRemoved { class, .. }
            | Change::JudgeSheetEntered { class, .. }
            | Change::SheetEdited { class, .. }
            | Change::SheetMoved { class, .. }
            | Change::SheetSet { class, .. }
            | Change::TeamSheetEdited { class, .. }
            | Change::TeamResultEntered { class, .. }
            | Change::RankSet { class, .. } => results.push(class.clone()),
            // A phase's jury penalties change the overall results too.
            Change::JuryPenaltySet { class, .. } => {
                results.push(class.clone());
                results.extend(self.classes.get(class).and_then(|c| c.phase.as_ref()).map(|p| p.of.clone()));
            }
            Change::StartsSet { class, .. } => lists.extend(published_of(self, class)),
            Change::ClassSet { class, .. } => lists.push(class.published.clone()),
            Change::EntryEdited { entry, .. } | Change::EntryDelivered { entry: Entry { id: entry, .. }, .. } | Change::EntryAdded { entry: Entry { id: entry, .. } } => {
                lists.extend(self.entries.get(entry).map(|e| e.published.clone()));
                results.extend(self.starts.iter().filter(|(c, s)| s.iter().any(|s| &s.entry == entry) && self.results.contains_key(&((*c).clone(), entry.clone()))).map(|(c, _)| c.clone()));
            }
            Change::AssignmentSet { assignment: Assignment { class: Some(class), .. } } => {
                results.push(class.clone());
                lists.extend(published_of(self, class));
            }
            _ => {}
        }
        if !results.is_empty() || !lists.is_empty() {
            self.stamp_of.insert(op.id.clone(), op.stamp.clone());
        }
        for c in results {
            self.result_inputs.entry(c.clone()).or_default().insert(op.id.clone());
            let at = self.results_changed.entry(c).or_default();
            *at = (*at).max(millis);
        }
        for p in lists {
            self.list_inputs.entry(p.clone()).or_default().insert(op.id.clone());
            let at = self.start_lists_changed.entry(p).or_default();
            *at = (*at).max(millis);
        }
    }

    /// The operations what is sent in an upload is made of (R14): a class's results, a
    /// published class's start lists, a ranking over several classes' classes. `None`: not
    /// followed (the event, a withdrawal).
    fn upload_inputs(&self, upload: &Upload) -> Option<BTreeSet<String>> {
        match upload {
            Upload::Results(class) => Some(self.result_inputs.get(class).cloned().unwrap_or_default()),
            Upload::StartLists(published) => Some(self.list_inputs.get(published).cloned().unwrap_or_default()),
            Upload::Combined(id) => Some(self.combined.get(id).map(|r| r.classes.iter().filter_map(|c| self.result_inputs.get(c)).flatten().cloned().collect()).unwrap_or_default()),
            Upload::StartListsWithdrawn(_) | Upload::Event => None,
        }
    }

    /// What uploads among these changes are based on (R14), filled in before they become
    /// operations: what this device knows of what was sent.
    pub fn fill_uploads(&self, changes: &mut [Change]) {
        for change in changes {
            if let Change::Uploaded { upload, based_on } = change {
                if based_on.is_none() {
                    *based_on = self.upload_inputs(upload).map(|s| s.into_iter().collect());
                }
            }
        }
    }

    /// R14: an upload that did not know changes made before it (offline, synced later): Swiss
    /// Equestrian does not have them. What was sent counts as changed after it (« to send
    /// again »), and it is said.
    fn late_for_upload(&mut self, op: &Operation, upload: &Upload, based_on: &[String]) {
        let Some(inputs) = self.upload_inputs(upload) else { return };
        let known: BTreeSet<&String> = based_on.iter().collect();
        let missing: Vec<String> = inputs.into_iter().filter(|i| !known.contains(i)).collect();
        if missing.is_empty() {
            return;
        }
        let after = op.stamp.millis + 1;
        let (class, published) = match upload {
            Upload::Results(class) => (class.clone(), None),
            Upload::StartLists(p) => (ClassId::default(), Some(p.clone())),
            Upload::Combined(id) => {
                let classes = self.combined.get(id).map(|r| r.classes.clone()).unwrap_or_default();
                for c in &classes {
                    let at = self.results_changed.entry(c.clone()).or_default();
                    *at = (*at).max(after);
                }
                (classes.first().cloned().unwrap_or_default(), None)
            }
            _ => return,
        };
        match (upload, &published) {
            (Upload::Results(c), _) => {
                let at = self.results_changed.entry(c.clone()).or_default();
                *at = (*at).max(after);
            }
            (Upload::StartLists(_), Some(p)) => {
                let at = self.start_lists_changed.entry(p.clone()).or_default();
                *at = (*at).max(after);
            }
            _ => {}
        }
        // The latest of them: the change Swiss Equestrian lacks.
        let over = missing.iter().filter_map(|id| self.stamp_of.get(id).map(|s| (s, id))).max().map(|(_, id)| id.clone());
        let detail = format!("{} change(s) made before the send, unknown to it", missing.len());
        let lost = serde_json::json!({ "upload": upload, "published": published });
        self.conflict(op, &class, ConflictKind::LateForUpload, detail, over, missing, lost);
    }

    /// The operation that finalises a class on this device: based on everything it knows (R4).
    pub fn finalise(&self, class: &ClassId) -> Change {
        Change::ClassFinalised { class: class.clone(), based_on: self.class_inputs.get(class).map(|s| s.iter().cloned().collect()).unwrap_or_default() }
    }

    /// A conflict of `op`, with its id.
    #[allow(clippy::too_many_arguments)]
    fn conflict(&mut self, op: &Operation, class: &ClassId, kind: ConflictKind, detail: String, over: Option<String>, what: Vec<String>, lost: serde_json::Value) {
        let n = self.conflicts.iter().filter(|c| c.op == op.id).count();
        self.conflicts.push(Conflict { id: format!("{}#{n}", op.id), op: op.id.clone(), class: class.clone(), kind, detail, over, what, lost, things: Vec::new() });
    }

    /// R12: the things of `op` that are no longer what its author saw (looked at before it is
    /// applied), with what they are now.
    /// R13: and what it relies on that is no longer what the author saw (fingerprints).
    #[allow(clippy::type_complexity)]
    fn clashes(&self, op: &Operation) -> (Vec<(crate::touched::Thing, serde_json::Value)>, Vec<(crate::touched::Thing, serde_json::Value)>) {
        if op.saw.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let set = crate::touched::things(&op.change)
            .into_iter()
            .filter_map(|t| {
                let seen = op.saw.get(&t.key())?;
                let now = t.value(self);
                (seen != &now).then_some((t, now))
            })
            .collect();
        let basis = crate::touched::needs(&op.change)
            .into_iter()
            .filter_map(|t| {
                let seen = op.saw.get(&t.key())?.as_str()?;
                let now = t.value(self);
                (seen != crate::touched::fingerprint(&now)).then_some((t, now))
            })
            .collect();
        (set, basis)
    }

    /// R12: `op` went over what changed meanwhile (`applied`), or was not applied because of it.
    fn clashed(&mut self, op: &Operation, clashes: Vec<(crate::touched::Thing, serde_json::Value)>, kind: ConflictKind) {
        use crate::touched::Thing;
        let applied = kind != ConflictKind::ChangedGone;
        // Applied, a class's status does not matter (it was running).
        let clashes: Vec<_> = clashes.into_iter().filter(|(t, _)| !applied || !matches!(t, Thing::ClassStatus { .. })).collect();
        if clashes.is_empty() {
            return;
        }
        let keys: Vec<String> = clashes.iter().map(|(t, _)| t.key()).collect();
        let over = keys.iter().filter_map(|k| self.writer_near(k)).max_by(|a, b| a.0.cmp(&b.0)).map(|w| w.1.clone());
        // Its class: of what clashed, else of the change itself (a result's class).
        let class = clashes.iter().find_map(|(t, _)| t.class().cloned()).or_else(|| crate::touched::things(&op.change).iter().find_map(|t| t.class().cloned())).unwrap_or_default();
        let detail = format!("{} changed meanwhile, unknown to the author{}", keys.join(", "), if applied { "" } else { "; not applied" });
        let lost = serde_json::Value::Object(clashes.iter().map(|(t, now)| (t.key(), now.clone())).collect());
        let things = clashes.into_iter().map(|(t, _)| t).collect();
        self.conflict(op, &class, kind, detail, over, keys, lost);
        if let Some(c) = self.conflicts.last_mut() {
            c.things = things;
        }
    }

    /// The latest operation that changed this key, a key inside it, or one it is inside.
    fn writer_near(&self, key: &str) -> Option<&(Stamp, String)> {
        let mut found: Vec<&(Stamp, String)> = Vec::new();
        let mut at = key;
        loop {
            found.extend(self.writers.get(at));
            match at.rfind('/') {
                Some(i) => at = &at[..i],
                None => break,
            }
        }
        let inside = format!("{key}/");
        found.extend(self.writers.range(inside.clone()..).take_while(|(k, _)| k.starts_with(&inside)).map(|(_, w)| w));
        found.into_iter().max_by(|a, b| a.0.cmp(&b.0))
    }

    /// `op` changed this (see `writers`).
    fn wrote(&mut self, key: String, op: &Operation) {
        self.writers.insert(key, (op.stamp.clone(), op.id.clone()));
    }

    /// The latest operation that changed one of these.
    fn last_writer(&self, keys: impl Iterator<Item = String>) -> Option<String> {
        keys.filter_map(|k| self.writers.get(&k)).max_by(|a, b| a.0.cmp(&b.0)).map(|w| w.1.clone())
    }

    /// What the author saw of each class and start list the changes set, filled in before they
    /// become operations (R7, R11): applied, they then change only what the author changed, over
    /// what others changed meanwhile. A change sees what the ones before it made.
    pub fn fill_seen(&self, changes: &mut [Change]) {
        let mut classes: BTreeMap<ClassId, Option<Class>> = BTreeMap::new();
        let mut lists: BTreeMap<ClassId, Vec<Start>> = BTreeMap::new();
        let mut gone: BTreeSet<EntryId> = BTreeSet::new();
        for change in changes.iter_mut() {
            match change {
                Change::ClassSet { class, seen, .. } => {
                    let now = classes.entry(class.id.clone()).or_insert_with(|| self.classes.get(&class.id).cloned());
                    if seen.is_none() {
                        seen.clone_from(now);
                    }
                    *now = Some(class.clone());
                }
                Change::StartsSet { class, starts, seen, .. } => {
                    let now = lists.entry(class.clone()).or_insert_with(|| self.starts.get(class).into_iter().flatten().filter(|s| !gone.contains(&s.entry)).cloned().collect());
                    if seen.is_none() {
                        *seen = Some(now.clone());
                    }
                    now.clone_from(starts);
                }
                Change::ClassRemoved { class } => {
                    classes.insert(class.clone(), None);
                    lists.insert(class.clone(), Vec::new());
                }
                Change::EntryRemoved { entry } => {
                    gone.insert(entry.clone());
                    for l in lists.values_mut() {
                        l.retain(|s| &s.entry != entry);
                    }
                }
                _ => {}
            }
        }
    }

    /// A class changed as its author saw it (R11): field by field over what others changed.
    fn set_class(&mut self, op: &Operation, class: &Class, seen: &Option<Class>) -> Outcome {
        let current = self.classes.get(&class.id).cloned();
        let merged = match (seen, current) {
            (Some(_), None) => {
                // Taken away meanwhile (sections put back together): it stays away.
                self.conflict(op, &class.id, ConflictKind::ClassGone, format!("class {} was taken away on another device", class.number), self.last_writer(std::iter::once(format!("x/{}", class.id))), Vec::new(), serde_json::Value::Null);
                return Outcome::Ignored(format!("class {} was taken away", class.number));
            }
            (Some(seen), Some(current)) => {
                let m = crate::merge::class(seen, &current, class);
                if !m.both.is_empty() {
                    let over = self.last_writer(m.both.iter().map(|(f, _)| format!("c/{}/{f}", class.id)));
                    let what: Vec<String> = m.both.iter().map(|(f, _)| f.clone()).collect();
                    let detail = format!("class {}: {} changed on two devices", class.number, what.join(", "));
                    self.conflict(op, &class.id, ConflictKind::ClassEdited, detail, over, what, serde_json::Value::Object(m.both.into_iter().collect()));
                }
                for f in &m.changed {
                    self.wrote(format!("c/{}/{f}", class.id), op);
                }
                m.class
            }
            (None, current) => Class { status: current.map_or(class.status, |c| c.status), ..class.clone() },
        };
        self.classes.insert(class.id.clone(), merged);
        Outcome::Applied
    }

    /// A start list changed as its author saw it (R7): their moves and numbers over what others
    /// changed meanwhile; a numbering goes on to riders its author did not know; a late entry
    /// takes the next number.
    fn set_starts(&mut self, op: &Operation, class_id: &ClassId, starts: &[Start], seen: &[Start], next_numbers: &[EntryId], numbered: bool) -> Outcome {
        let Some(class) = self.classes.get(class_id).cloned() else {
            self.conflict(op, class_id, ConflictKind::ClassGone, format!("class {class_id} was taken away on another device"), self.last_writer(std::iter::once(format!("x/{class_id}"))), Vec::new(), serde_json::Value::Null);
            return Outcome::Ignored(format!("class {} was taken away", class_id));
        };
        let current = self.starts.get(class_id).cloned().unwrap_or_default();
        let m = crate::merge::starts(seen, &current, starts, !class.teams.is_empty());
        let key = |kind: &str, e: &EntryId| format!("{kind}/{class_id}/{e}");
        if !m.moved_both.is_empty() {
            let over = self.last_writer(m.moved_both.iter().map(|e| key("o", e)));
            let what: Vec<String> = m.moved_both.iter().map(|e| e.to_string()).collect();
            self.conflict(op, class_id, ConflictKind::StartOrder, format!("class {}: {} riders moved on two devices", class.number, what.len()), over, what, serde_json::Value::Null);
        }
        if !m.numbers_both.is_empty() {
            let over = self.last_writer(m.numbers_both.iter().map(|(e, _)| key("n", e)));
            let what: Vec<String> = m.numbers_both.iter().map(|(e, _)| e.to_string()).collect();
            let lost = m.numbers_both.iter().map(|(e, n)| (e.to_string(), serde_json::json!(n))).collect();
            self.conflict(op, class_id, ConflictKind::StartNumbers, format!("class {}: {} start numbers given on two devices", class.number, what.len()), over, what, serde_json::Value::Object(lost));
        }
        for e in &m.moved {
            self.wrote(key("o", e), op);
        }
        for e in &m.numbered {
            self.wrote(key("n", e), op);
        }
        let mut list = m.starts;
        // The next numbers: riders a numbering did not know, late entries.
        let known: BTreeSet<&EntryId> = starts.iter().map(|s| &s.entry).collect();
        let mut pending: BTreeSet<EntryId> = if numbered { list.iter().filter(|s| !known.contains(&s.entry)).map(|s| s.entry.clone()).collect() } else { BTreeSet::new() };
        pending.extend(next_numbers.iter().filter(|e| list.iter().any(|s| &s.entry == *e)).cloned());
        if !pending.is_empty() {
            let assigned = self.start_numbers_assigned(&class);
            let fixed = self.event.as_ref().is_some_and(|e| e.fixed_start_numbers);
            let elsewhere = self.starts.iter().filter(|(c, _)| fixed && *c != class_id).flat_map(|(_, l)| l.iter().filter_map(|s| s.start_number));
            let mut next = list.iter().filter(|s| !pending.contains(&s.entry)).filter_map(|s| s.start_number).chain(elsewhere).max().unwrap_or(0);
            for s in list.iter_mut().filter(|s| pending.contains(&s.entry)) {
                s.start_number = assigned.then(|| {
                    next += 1;
                    next
                });
                self.writers.insert(key("n", &s.entry), (op.stamp.clone(), op.id.clone()));
            }
        }
        // Put together: two riders with one number are for the office to sort out.
        if current.as_slice() != seen {
            let mut by_number: BTreeMap<u32, usize> = BTreeMap::new();
            for n in list.iter().filter_map(|s| s.start_number) {
                *by_number.entry(n).or_default() += 1;
            }
            let twice: Vec<String> = by_number.iter().filter(|(_, k)| **k > 1).map(|(n, _)| n.to_string()).collect();
            if !twice.is_empty() {
                self.conflict(op, class_id, ConflictKind::DuplicateNumbers, format!("class {}: numbers {} given twice", class.number, twice.join(", ")), None, twice, serde_json::Value::Null);
            }
        }
        self.starts.insert(class_id.clone(), list);
        Outcome::Applied
    }

    fn take_sheet(&mut self, class: &ClassId, place: &SheetPlace) -> Option<JudgeSheet> {
        match place {
            SheetPlace::Rider { entry, judge } => self.sheets.remove(&(class.clone(), entry.clone(), judge.clone())),
            SheetPlace::Aside { slot } => self.aside.remove(&(class.clone(), *slot)),
        }
    }

    fn put_sheet(&mut self, class: &ClassId, place: &SheetPlace, sheet: JudgeSheet) {
        match place {
            SheetPlace::Rider { entry, judge } => self.sheets.insert((class.clone(), entry.clone(), judge.clone()), sheet),
            SheetPlace::Aside { slot } => self.aside.insert((class.clone(), *slot), sheet),
        };
    }

    /// A class as the office knows it, in a refusal: its number (its id when it is gone).
    fn class_number(&self, class: &ClassId) -> String {
        self.classes.get(class).map_or_else(|| class.0.clone(), |c| c.number.clone())
    }

    fn is_final(&self, class: &ClassId) -> bool {
        self.classes.get(class).is_some_and(|c| self.status_of(c) == ClassStatus::Final)
    }

    /// A class's status; a phase's is its class's when that is final (Rosson keeps `definitiv`
    /// on the class only: closing a class made of phases closes them all).
    pub fn status_of(&self, class: &Class) -> ClassStatus {
        match class.phase.as_ref().and_then(|p| self.classes.get(&p.of)) {
            Some(parent) if parent.status == ClassStatus::Final => ClassStatus::Final,
            _ => class.status,
        }
    }

    fn apply_change(&mut self, op: &Operation) -> Outcome {
        let change = &op.change;
        match change {
            Change::EventImported { event, published_classes, persons, horses, entries, teams, scoring_tables, programmes, reasons, categories, master } => {
                self.master_records.extend(master.iter().map(|(k, v)| (k.clone(), v.clone())));
                self.reasons.extend(reasons.iter().map(|r| (r.id, r.clone())));
                self.categories.extend(categories.iter().map(|c| (c.id, c.clone())));
                self.event = Some(event.clone());
                self.published_classes.extend(published_classes.iter().map(|c| (c.id.clone(), c.clone())));
                for p in persons {
                    let town = self.addresses_set.contains(&p.id).then(|| self.persons.get(&p.id).map(|x| x.town.clone())).flatten();
                    self.persons.insert(p.id.clone(), Person { town: town.unwrap_or_else(|| p.town.clone()), ..p.clone() });
                }
                self.horses.extend(horses.iter().map(|h| (h.id.clone(), h.clone())));
                self.entries.extend(entries.iter().map(|e| (e.id.clone(), e.clone())));
                self.delivered_entries.extend(entries.iter().map(|e| (e.id.clone(), e.clone())));
                self.teams.extend(teams.iter().map(|t| (t.id.clone(), t.clone())));
                self.scoring_tables.extend(scoring_tables.iter().map(|t| (t.id, t.clone())));
                self.programmes.extend(programmes.iter().map(|p| (p.id, p.clone())));
            }
            Change::PrivateDataDelivered { contacts, notes } => {
                // An address the office set stays (Rosson keeps it when the registry comes again).
                self.contacts.extend(contacts.iter().filter(|c| !self.addresses_set.contains(&c.person)).map(|c| (c.person.clone(), c.clone())));
                for n in notes {
                    if n.remarks.is_empty() && n.companion.is_empty() {
                        self.entry_notes.remove(&n.entry);
                    } else {
                        self.entry_notes.insert(n.entry.clone(), n.clone());
                    }
                }
            }
            Change::EntryDelivered { entry } => {
                let merged = match (self.entries.get(&entry.id), self.delivered_entries.get(&entry.id)) {
                    (Some(current), Some(before)) => {
                        let (merged, notices) = crate::delivered::merge(current, before, entry);
                        // R8: the office's change replaced: said as a conflict, its version kept to
                        // be put back.
                        let current = current.clone();
                        for (field, detail) in &notices {
                            let thing = crate::touched::Thing::EntryField { entry: entry.id.clone(), field: (*field).to_string() };
                            let key = thing.key();
                            let over = self.writer_near(&key).map(|w| w.1.clone());
                            let lost = serde_json::json!({ key.clone(): crate::touched::entry_field(&current, field) });
                            self.conflict(op, &ClassId::default(), ConflictKind::Delivered, detail.clone(), over, vec![key], lost);
                            if let Some(c) = self.conflicts.last_mut() {
                                c.things = vec![thing];
                            }
                        }
                        self.notices.extend(notices.into_iter().map(|(field, detail)| crate::delivered::Notice { op: op.id.clone(), entry: entry.id.clone(), field: field.to_string(), detail }));
                        merged
                    }
                    _ => entry.clone(),
                };
                let mut merged = merged;
                // The town of an address the office set stays on the entry.
                if self.addresses_set.contains(&merged.rider) {
                    let town = self.persons.get(&merged.rider).map(|p| p.town.clone());
                    if let (Some(r), Some(town)) = (merged.rider_on_entry.as_mut(), town) {
                        r.town = town;
                    }
                }
                self.entries.insert(entry.id.clone(), merged);
                self.delivered_entries.insert(entry.id.clone(), entry.clone());
            }
            Change::EntryEdited { entry, edit } => {
                let Some(e) = self.entries.get_mut(entry) else {
                    return Outcome::Ignored(format!("unknown entry {entry}"));
                };
                if let Some(status) = edit.status {
                    e.status = status;
                }
                if let Some(hc) = edit.hors_concours {
                    e.hors_concours = hc;
                }
                if let Some(r) = &edit.rider {
                    e.rider = r.rider.clone();
                    e.rider_on_entry = r.rider_on_entry.clone();
                    e.rider_changed = r.changed;
                }
                if let Some(p) = &edit.published {
                    e.published = p.clone();
                }
                if let Some(stage) = edit.stage {
                    e.stage = (stage != 0).then_some(stage);
                }
                if let Some(comment) = &edit.comment {
                    e.comment = comment.clone();
                }
                if let Some(h) = &edit.horse {
                    e.horse = h.horse.clone();
                    e.guest_horse = h.guest_horse.clone();
                    e.horses_on_entry = h.horses_on_entry.clone();
                }
                if let Some(phases) = &edit.horse_phases {
                    for (h, p) in e.horses_on_entry.iter_mut().zip(phases) {
                        if let Some(h) = h.as_object_mut() {
                            h.insert("einsaetze".into(), serde_json::json!(p));
                        }
                    }
                }
            }
            Change::ClassRemoved { class } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                if self.results.keys().any(|(c, _)| c == class) || self.sheets.keys().any(|(c, _, _)| c == class) || self.team_results.keys().any(|(c, _)| c == class) || self.team_sheets.keys().any(|(c, _, _)| c == class) {
                    return Outcome::Ignored(format!("class {} has results", self.class_number(class)));
                }
                self.classes.remove(class);
                self.starts.remove(class);
                self.wrote(format!("x/{class}"), op);
            }
            Change::ClassSet { class, seen, .. } => return self.set_class(op, class, seen),
            Change::StartsSet { class, starts, seen, next_numbers, numbered, .. } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                match seen {
                    Some(seen) => return self.set_starts(op, class, starts, seen, next_numbers, *numbered),
                    // As given (imports, operations made before 2026-11-16).
                    None => {
                        self.starts.insert(class.clone(), starts.clone());
                    }
                }
            }
            Change::AssignmentSet { assignment } => {
                // A changed assignment is looked up again.
                self.assignments.insert(assignment.id.clone(), Assignment { looked_up: false, ..assignment.clone() });
            }
            Change::AssignmentRemoved { assignment } => {
                if let Some(class) = self.assignments.get(assignment).and_then(|a| a.class.clone()) {
                    if self.is_final(&class) {
                        return Outcome::Ignored(format!("class {} is final", self.class_number(&class)));
                    }
                }
                self.assignments.remove(assignment);
            }
            Change::RolesDelivered { roles } => {
                self.roles = roles.clone();
            }
            Change::OfficialsDelivered { officials } => {
                self.officials = officials.iter().map(|o| (o.person.clone(), o.clone())).collect();
            }
            Change::AssignmentLookedUp { assignment, se_id } => {
                let Some(a) = self.assignments.get_mut(assignment) else {
                    return Outcome::Ignored(format!("unknown assignment {assignment}"));
                };
                a.looked_up = true;
                if se_id.is_some() {
                    a.se_id = *se_id;
                }
            }
            Change::ResultEdited { class, entry, changes } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                for c in changes {
                    let card = self.results.entry((class.clone(), entry.clone())).or_default();
                    let current = if c.field == "out" { card.out.map(Decimal::from) } else { card.values.get(&c.field).copied() };
                    let key = format!("r/{class}/{entry}/{}", c.field);
                    if current != c.from {
                        // R2: someone changed the field since the author saw it; the later change wins.
                        let show = |v: Option<Decimal>| v.map_or("empty".to_string(), |d| d.to_string());
                        let detail = format!("entry {entry}, {}: author saw {}, was {}, now {}", c.field, show(c.from), show(current), show(c.to));
                        let over = self.last_writer(std::iter::once(key.clone()));
                        self.conflict(op, class, ConflictKind::ConcurrentEdit, detail, over, vec![entry.to_string(), c.field.clone()], serde_json::json!({ c.field.clone(): current }));
                    }
                    self.wrote(key, op);
                    let card = self.results.entry((class.clone(), entry.clone())).or_default();
                    if c.field == "out" {
                        card.out = c.to.and_then(|d| u32::try_from(d).ok());
                    } else if let Some(v) = c.to {
                        card.values.insert(c.field.clone(), v);
                    } else {
                        card.values.remove(&c.field);
                    }
                }
            }
            Change::ResultEntered { class, entry, values, out } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                for field in values.keys().map(String::as_str).chain(["out"]) {
                    self.wrote(format!("r/{class}/{entry}/{field}"), op);
                }
                self.results.insert((class.clone(), entry.clone()), ScoreCard { values: values.clone(), out: *out });
            }
            Change::ResultRemoved { class, entry } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                self.results.remove(&(class.clone(), entry.clone()));
            }
            Change::EntryAdded { entry } => {
                if self.entries.contains_key(&entry.id) {
                    return Outcome::Ignored(format!("entry {} exists", entry.id));
                }
                self.entries.insert(entry.id.clone(), entry.clone());
            }
            Change::EntryRemoved { entry } => {
                if self.entries.remove(entry).is_none() {
                    return Outcome::Ignored(format!("unknown entry {entry}"));
                }
                for starts in self.starts.values_mut() {
                    starts.retain(|s| &s.entry != entry);
                }
            }
            Change::PersonAdded { person } => {
                self.persons.entry(person.id.clone()).or_insert_with(|| person.clone());
            }
            Change::HorseAdded { horse } => {
                self.horses.entry(horse.id.clone()).or_insert_with(|| horse.clone());
            }
            Change::AddressSet { person, town, contact } => {
                self.contacts.insert(person.clone(), Contact { person: person.clone(), ..contact.clone() });
                if let Some(p) = self.persons.get_mut(person) {
                    p.town = town.clone();
                }
                for e in self.entries.values_mut().filter(|e| &e.rider == person) {
                    if let Some(r) = e.rider_on_entry.as_mut() {
                        r.town = town.clone();
                    }
                }
                self.addresses_set.insert(person.clone());
            }
            Change::TeamSet { team } => {
                self.teams.insert(team.id.clone(), team.clone());
            }
            Change::TeamRemoved { team } => {
                if self.entries.values().any(|e| e.team.as_ref() == Some(team)) {
                    return Outcome::Ignored(format!("team {team} has entries"));
                }
                self.teams.remove(team);
                for class in self.classes.values_mut() {
                    class.teams.retain(|t| t != team);
                }
            }
            Change::EventNumbersSet { fixed, assigned } => {
                let Some(e) = self.event.as_mut() else { return Outcome::Ignored("no event".into()) };
                e.fixed_start_numbers = *fixed;
                e.start_numbers_assigned = *assigned;
            }
            Change::PrizesSet { published, prizes } => match prizes {
                Some(p) => {
                    self.prizes.insert(published.clone(), p.clone());
                }
                None => {
                    self.prizes.remove(published);
                }
            },
            Change::PublishedClassEdited { published, category_text, levels } => {
                let Some(p) = self.published_classes.get_mut(published) else { return Outcome::Ignored(format!("no published class {published}")) };
                if let Some(text) = category_text {
                    p.category_text = text.clone();
                }
                if let Some(levels) = levels {
                    p.levels = *levels;
                }
            }
            Change::ProgrammeSet { programme } => {
                self.programmes.insert(programme.id, programme.clone());
            }
            Change::ScoringTableSet { table } => {
                self.scoring_tables.insert(table.id, table.clone());
            }
            Change::CombinedSet { ranking } => {
                self.combined.insert(ranking.id.clone(), ranking.clone());
            }
            Change::CombinedRemoved { id } => {
                self.combined.remove(id);
            }
            Change::JudgeSheetEntered { class, entry, judge, sheet } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                self.sheets.insert((class.clone(), entry.clone(), judge.clone()), sheet.clone());
            }
            Change::SheetEdited { class, entry, judge, marks, penalties, total } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                let key = (class.clone(), entry.clone(), judge.clone());
                let sheet = self.sheets.entry(key.clone()).or_default();
                for (mark, value) in marks {
                    match value {
                        Some(v) => sheet.marks.insert(mark.clone(), *v),
                        None => sheet.marks.remove(mark),
                    };
                }
                if let Some(p) = penalties {
                    sheet.penalty_points = p.points.clone();
                    sheet.penalty_percent = p.percent.clone();
                    sheet.excluded = p.excluded;
                }
                if let Some(t) = total {
                    sheet.total = *t;
                }
                if sheet.marks.is_empty() && sheet.penalty_points.is_empty() && sheet.penalty_percent.is_empty() && !sheet.excluded && sheet.total.is_none() {
                    self.sheets.remove(&key);
                }
            }
            Change::SheetMoved { class, from, to } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                let moving = self.take_sheet(class, from);
                let there = self.take_sheet(class, to);
                if let Some(sheet) = moving {
                    self.put_sheet(class, to, sheet);
                }
                if let Some(sheet) = there {
                    self.put_sheet(class, from, sheet);
                }
            }
            Change::SheetSet { class, place, sheet } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                self.take_sheet(class, place);
                if let Some(sheet) = sheet {
                    self.put_sheet(class, place, sheet.clone());
                }
            }
            Change::TeamSheetEdited { class, team, judge, marks, penalties } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                let key = (class.clone(), team.clone(), judge.clone());
                let sheet = self.team_sheets.entry(key.clone()).or_default();
                for (mark, value) in marks {
                    match value {
                        Some(v) => sheet.marks.insert(mark.clone(), *v),
                        None => sheet.marks.remove(mark),
                    };
                }
                if let Some(p) = penalties {
                    sheet.penalty_points = p.points.clone();
                    sheet.penalty_percent = p.percent.clone();
                    sheet.excluded = p.excluded;
                }
                if sheet.marks.is_empty() && sheet.penalty_points.is_empty() && sheet.penalty_percent.is_empty() && !sheet.excluded && sheet.total.is_none() {
                    self.team_sheets.remove(&key);
                }
            }
            Change::TeamResultEntered { class, team, values, out } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                self.team_results.insert((class.clone(), team.clone()), ScoreCard { values: values.clone(), out: *out });
            }
            Change::JuryPenaltySet { class, entry, points } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                match points {
                    Some(p) => self.jury_penalties.insert((class.clone(), entry.clone()), *p),
                    None => self.jury_penalties.remove(&(class.clone(), entry.clone())),
                };
            }
            Change::RankSet { class, competitor, rank, reason } => {
                if self.is_final(class) {
                    return Outcome::Ignored(format!("class {} is final", self.class_number(class)));
                }
                let key = (class.clone(), competitor.clone());
                match rank {
                    Some(rank) => {
                        self.manual_ranks.insert(key, ManualRank { rank: *rank, reason: reason.clone() });
                    }
                    None => {
                        self.manual_ranks.remove(&key);
                    }
                }
            }
            Change::Uploaded { upload, based_on } => {
                if let Upload::StartListsWithdrawn(published) = upload {
                    self.uploaded.remove(&Upload::StartLists(published.clone()));
                }
                self.uploaded.insert(upload.clone(), Sent { millis: op.stamp.millis, author: op.author.clone(), op: op.id.clone() });
                if let Some(based_on) = based_on {
                    self.late_for_upload(op, upload, based_on);
                }
            }
            Change::ClassFinalised { class, .. } | Change::ClassReopened { class, .. } => {
                let Some(c) = self.classes.get_mut(class) else {
                    return Outcome::Ignored(format!("unknown class {class}"));
                };
                c.status = if matches!(change, Change::ClassFinalised { .. }) { ClassStatus::Final } else { ClassStatus::Running };
            }
        }
        Outcome::Applied
    }
}

/// The class whose results an operation changes, if any (for R4).
fn class_of_results(change: &Change) -> Option<&ClassId> {
    match change {
        Change::ResultEdited { class, .. }
        | Change::ResultEntered { class, .. }
        | Change::ResultRemoved { class, .. }
        | Change::JudgeSheetEntered { class, .. }
        | Change::SheetEdited { class, .. }
        | Change::SheetMoved { class, .. }
        | Change::SheetSet { class, .. }
        | Change::TeamSheetEdited { class, .. }
        | Change::TeamResultEntered { class, .. }
        | Change::RankSet { class, .. }
        | Change::JuryPenaltySet { class, .. }
        | Change::StartsSet { class, .. } => Some(class),
        _ => None,
    }
}
