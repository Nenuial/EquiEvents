//! Merging what Swiss Equestrian delivers again (an update) with what the show office changed.
//!
//! Each entry is kept twice: as the office has it, and as Swiss Equestrian last delivered it.
//! When a new version arrives, a field takes Swiss Equestrian's new value **if Swiss Equestrian
//! changed it** since its last delivery; a field it did not change keeps the office's value.
//! When both changed the same field, Swiss Equestrian's change is the later one and wins, and
//! the office is told (a [`Notice`]), like concurrent edits between devices (rule R2).
//!
//! Some fields go together:
//!
//! - the rider, and the rider as written on the entry (name, town, club);
//! - the horses (first horse, further horses, guest horse's name), and the horses as written on
//!   the entry. When Swiss Equestrian did not change the horse but the office did, the office's
//!   horse and its description stay; when neither changed it, the description is refreshed
//!   from Swiss Equestrian's records, as Rosson does for every changed entry;
//! - Swiss Equestrian's status, and whether the entry starts: a withdrawal (or a confirmation
//!   after one) changes it; otherwise the office's decision stands.
//!
//! *Better than Rosson* (docs/deviations.md E39): Rosson overwrote the rider, horses, status,
//! team, fee and winning points with Swiss Equestrian's values whenever the entry had changed
//! there, so an unrelated change (a fee paid) undid a change of horse made at the show; it never
//! took changes of hors concours, level or chosen programme; and an entry confirmed again after
//! a withdrawal stayed "does not start". Without changes by the office, the result is Rosson's,
//! apart from those three.

use crate::model::*;

/// Something the office should know about a delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The operation that brought the delivery.
    pub op: String,
    pub entry: EntryId,
    /// The entry's field the office had changed (`touched::Thing::EntryField`'s): `rider`,
    /// `horse`, `status`, `hors_concours`, `team`, `programme`, `stage`, `fee_paid`,
    /// `winning_points`, `entered_by`.
    pub field: String,
    pub detail: String,
}

/// The statuses with which an entry does not start.
pub const NOT_STARTING: [&str; 3] = ["abgemeldet", "zurueckgewiesen", "falschnennung"];

/// Whether an entry with Swiss Equestrian's status starts.
pub fn status_of(se_status: &str) -> EntryStatus {
    if NOT_STARTING.contains(&se_status) {
        EntryStatus::NotStarting
    } else {
        EntryStatus::Entered
    }
}

/// Merge a new delivery `now` of an entry, last delivered as `before`, into the office's
/// `current`. Returns the merged entry and what the office should be told.
/// The merged entry, and the fields where Swiss Equestrian replaced a change of the office
/// (field, what happened).
pub fn merge(current: &Entry, before: &Entry, now: &Entry) -> (Entry, Vec<(&'static str, String)>) {
    let mut merged = current.clone();
    let mut notices = Vec::new();
    // Swiss Equestrian's own record: always the latest (the office's changes are the fields).
    merged.record = now.record.clone();

    // A group of fields: Swiss Equestrian's change wins; the office is told if it had changed it too.
    macro_rules! group {
        ($code:expr, $what:expr, $($field:ident),+) => {{
            let changed_there = ($(&now.$field),+) != ($(&before.$field),+);
            let changed_here = ($(&current.$field),+) != ($(&before.$field),+);
            if changed_there {
                if changed_here && ($(&current.$field),+) != ($(&now.$field),+) {
                    notices.push(($code, format!("{}: changed by Swiss Equestrian, replacing the office's change", $what)));
                }
                $(merged.$field = now.$field.clone();)+
            }
            (changed_there, changed_here)
        }};
    }

    // The rider; the rider as written on the entry follows.
    let (rider_there, rider_here) = group!("rider", "rider", rider);
    if rider_there || !rider_here {
        merged.rider_on_entry = now.rider_on_entry.clone();
    }
    // Swiss Equestrian's rider: no longer a change made at the show.
    if rider_there {
        merged.rider_changed = false;
    }

    // The horses; their description follows.
    let (horses_there, horses_here) = group!("horse", "horse", horse, other_horses, guest_horse);
    if horses_there || !horses_here {
        merged.horses_on_entry = now.horses_on_entry.clone();
    }

    // Swiss Equestrian's status, and with it whether the entry starts.
    if now.se_status != before.se_status {
        let starts = status_of(&now.se_status);
        if current.status != status_of(&before.se_status) && current.status != starts {
            notices.push(("status", format!("status: Swiss Equestrian now says {}, replacing the office's decision", now.se_status)));
        }
        merged.se_status = now.se_status.clone();
        merged.status = starts;
        merged.waiting_list = now.waiting_list;
    }

    group!("hors_concours", "hors concours", hors_concours);
    group!("team", "team", team);
    group!("programme", "programme", programme);
    group!("stage", "level", stage);
    group!("fee_paid", "entry fee paid", fee_paid);
    group!("winning_points", "winning points", winning_points);
    group!("entered_by", "entered by", entered_by);
    merged.se_changed_at = now.se_changed_at.clone();
    (merged, notices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> Entry {
        Entry {
            id: EntryId(1),
            published: PublishedClassId(10),
            rider: PersonId(100),
            horse: Some(HorseId(200)),
            guest_horse: None,
            status: EntryStatus::Entered,
            hors_concours: false,
            team: None,
            programme: None,
            horses_on_entry: vec![serde_json::json!({ "pferd_id": 200, "pferdename": "ALPHA" })],
            stage: None,
            rider_on_entry: Some(RiderOnEntry { first_name: "Anna".into(), last_name: "Muster".into(), town: "Bern".into(), club: String::new() }),
            waiting_list: false,
            fee_paid: None,
            organiser_place: false,
            se_status: "definitiv".into(),
            entered_by: None,
            other_horses: Vec::new(),
            winning_points: WinningPoints::default(),
            se_changed_at: Some("2027-04-01T10:00:00+02:00".into()),
            rider_changed: false,
            comment: String::new(),
            record: serde_json::Value::Null,
        }
    }

    #[test]
    fn the_offices_note_stays_and_a_rider_from_swiss_equestrian_is_no_change() {
        let before = entry();
        let mut current = before.clone();
        current.comment = "pays on site".into();
        current.rider = PersonId(101);
        current.rider_changed = true;
        // Swiss Equestrian changes something else: the note and the change of rider stay.
        let mut now = before.clone();
        now.hors_concours = true;
        let (merged, _) = merge(&current, &before, &now);
        assert_eq!((merged.comment.as_str(), merged.rider_changed, merged.rider.clone()), ("pays on site", true, PersonId(101)));
        // Swiss Equestrian changes the rider: its rider, no longer a change made at the show.
        now.rider = PersonId(102);
        let (merged, _) = merge(&current, &before, &now);
        assert_eq!((merged.rider.clone(), merged.rider_changed), (PersonId(102), false));
    }

    fn with_horse(mut e: Entry, id: u64, name: &str) -> Entry {
        e.horse = Some(HorseId(id));
        e.horses_on_entry = vec![serde_json::json!({ "pferd_id": id, "pferdename": name })];
        e
    }

    #[test]
    fn without_office_changes_the_delivery_is_taken() {
        let before = entry();
        let mut now = with_horse(entry(), 201, "BRAVO");
        now.fee_paid = Some(50.into());
        now.se_status = "abgemeldet".into();
        let (merged, notices) = merge(&before, &before, &now);
        assert_eq!(merged, Entry { status: EntryStatus::NotStarting, ..now });
        assert!(notices.is_empty());
    }

    #[test]
    fn an_unrelated_change_keeps_the_office_horse() {
        let before = entry();
        let current = with_horse(entry(), 202, "CHARLIE"); // changed at the show
        let mut now = entry();
        now.fee_paid = Some(50.into());
        let (merged, notices) = merge(&current, &before, &now);
        assert_eq!(merged.horse, Some(HorseId(202)));
        assert_eq!(merged.horses_on_entry[0]["pferdename"], "CHARLIE");
        assert_eq!(merged.fee_paid, Some(50.into()));
        assert!(notices.is_empty());
    }

    #[test]
    fn a_horse_changed_on_both_sides_takes_swiss_equestrians_and_tells() {
        let before = entry();
        let current = with_horse(entry(), 202, "CHARLIE");
        let now = with_horse(entry(), 201, "BRAVO");
        let (merged, notices) = merge(&current, &before, &now);
        assert_eq!(merged.horse, Some(HorseId(201)));
        assert_eq!(merged.horses_on_entry[0]["pferdename"], "BRAVO");
        assert_eq!(notices.len(), 1, "{notices:?}");
    }

    #[test]
    fn a_horse_description_is_refreshed_when_nobody_changed_the_horse() {
        let before = entry();
        let mut now = entry();
        now.horses_on_entry[0]["pferd_besitzer"] = "Neue Besitzerin".into();
        let (merged, _) = merge(&before, &before, &now);
        assert_eq!(merged.horses_on_entry, now.horses_on_entry);
    }

    #[test]
    fn the_office_decision_on_starting_stands_unless_the_status_changes() {
        let before = entry();
        let current = Entry { status: EntryStatus::NotStarting, ..entry() }; // rider called in sick
        let mut now = entry();
        now.winning_points.rider = Some(12.into());
        assert_eq!(merge(&current, &before, &now).0.status, EntryStatus::NotStarting);
        // Withdrawn on Swiss Equestrian: does not start either way, nothing to tell.
        now.se_status = "abgemeldet".into();
        let (merged, notices) = merge(&current, &before, &now);
        assert_eq!(merged.status, EntryStatus::NotStarting);
        assert!(notices.is_empty());
    }

    #[test]
    fn confirmed_again_after_a_withdrawal_starts() {
        let before = Entry { se_status: "abgemeldet".into(), status: EntryStatus::NotStarting, ..entry() };
        let now = entry();
        assert_eq!(merge(&before, &before, &now).0.status, EntryStatus::Entered);
    }

    #[test]
    fn the_rider_written_on_the_entry_follows_the_rider() {
        let before = entry();
        let mut now = entry();
        now.rider = PersonId(101);
        now.rider_on_entry = Some(RiderOnEntry { first_name: "Berta".into(), last_name: "Beispiel".into(), town: "Thun".into(), club: String::new() });
        let (merged, _) = merge(&before, &before, &now);
        assert_eq!(merged.rider_on_entry, now.rider_on_entry);
        // The office changed the rider; Swiss Equestrian only moved the rider's town: the office's rider stays, with its name.
        let current = Entry { rider: PersonId(102), rider_on_entry: Some(RiderOnEntry { first_name: "Clara".into(), last_name: "C".into(), town: "Biel".into(), club: String::new() }), ..entry() };
        let mut now = entry();
        now.rider_on_entry.as_mut().unwrap().town = "Neuenstadt".into();
        let (merged, _) = merge(&current, &before, &now);
        assert_eq!((merged.rider.clone(), merged.rider_on_entry.unwrap().town), (PersonId(102), "Biel".to_string()));
    }
}
