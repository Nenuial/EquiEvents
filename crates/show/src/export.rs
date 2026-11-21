//! Show exports: one show in one file, to keep, to hand over, or to continue elsewhere.
//!
//! A `.equievents` file is a zip archive with two files:
//!
//! - `manifest.json`: the format and its version, the event (number, title, place, dates), when
//!   and by which version of the app it was written, how many operations it holds, and whether
//!   private data is included;
//! - `operations.jsonl`: the show's operation log, one operation per line, in stamp order.
//!
//! The log is the show: importing it gives back the same state, with its whole history (who
//! entered or changed what, and when). Operations are identified by their id, so importing into
//! a show that already has some of them adds only the missing ones: an export can also bring a
//! copy up to date.
//!
//! [`Privacy::Public`] leaves out the operations that carry non-public data (addresses, phones,
//! emails, birth dates, riders' notes; `Change::PrivateDataDelivered`), for handing a show to
//! someone who does not need them.
//!
//! One show per file; there is no export of everything at once (Rosson's database export).
//! Writing is deterministic: the same log gives the same bytes.

use std::io::{Cursor, Read, Write};

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use crate::oplog::{Change, Operation};
use crate::State;

pub const FORMAT: &str = "EquiEvents show";
/// The version this app writes and the newest it reads.
pub const VERSION: u32 = 1;
/// File name extension, without the dot.
pub const EXTENSION: &str = "equievents";

const MANIFEST: &str = "manifest.json";
const OPERATIONS: &str = "operations.jsonl";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    /// The app that wrote the file.
    pub app_version: String,
    /// When, RFC 3339 (UTC).
    pub exported_at: String,
    /// The event, for showing what a file holds before importing it.
    pub event: Option<EventSummary>,
    /// Number of operations in `operations.jsonl`.
    pub operations: usize,
    /// Whether the operations with non-public data are included.
    pub private_data: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventSummary {
    pub id: u64,
    pub number: String,
    pub title: Vec<String>,
    pub place: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Privacy {
    /// Everything.
    All,
    /// Without non-public data.
    Public,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportError(pub String);

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn error(e: impl std::fmt::Display) -> ExportError {
    ExportError(e.to_string())
}

/// Whether an operation carries non-public data.
pub fn is_private(op: &Operation) -> bool {
    matches!(op.change, Change::PrivateDataDelivered { .. })
}

/// Write a show's log as an export file. `exported_at`: RFC 3339, UTC.
pub fn write(log: &[Operation], privacy: Privacy, app_version: &str, exported_at: &str) -> Result<Vec<u8>, ExportError> {
    let mut operations: Vec<&Operation> = log.iter().filter(|op| privacy == Privacy::All || !is_private(op)).collect();
    operations.sort_by(|a, b| a.stamp.cmp(&b.stamp));
    let (state, _) = State::from_log(log.to_vec());
    let manifest = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        app_version: app_version.into(),
        exported_at: exported_at.into(),
        event: state.event.map(|e| EventSummary { id: e.id.0, number: e.number, title: e.title, place: e.place, from: e.from, to: e.to }),
        operations: operations.len(),
        private_data: privacy == Privacy::All,
    };

    let mut lines = Vec::new();
    for op in operations {
        serde_json::to_writer(&mut lines, op).map_err(error)?;
        lines.push(b'\n');
    }
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated).last_modified_time(DateTime::default());
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(MANIFEST, options).map_err(error)?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest).map_err(error)?).map_err(error)?;
    zip.start_file(OPERATIONS, options).map_err(error)?;
    zip.write_all(&lines).map_err(error)?;
    Ok(zip.finish().map_err(error)?.into_inner())
}

/// Read only the manifest, to show what a file holds.
pub fn read_manifest(bytes: &[u8]) -> Result<Manifest, ExportError> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|_| ExportError("not an EquiEvents show file".into()))?;
    let manifest: Manifest = serde_json::from_slice(&file(&mut zip, MANIFEST)?).map_err(|e| ExportError(format!("{MANIFEST}: {e}")))?;
    if manifest.format != FORMAT {
        return Err(ExportError(format!("not an EquiEvents show file ({:?})", manifest.format)));
    }
    if manifest.version > VERSION {
        return Err(ExportError(format!("written by a newer version of EquiEvents (format {}, this app reads up to {VERSION}); update the app", manifest.version)));
    }
    Ok(manifest)
}

/// Read an export file: its manifest and the operations, checked against each other.
pub fn read(bytes: &[u8]) -> Result<(Manifest, Vec<Operation>), ExportError> {
    let manifest = read_manifest(bytes)?;
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(error)?;
    let lines = file(&mut zip, OPERATIONS)?;
    let mut operations = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for (n, line) in lines.split(|b| *b == b'\n').enumerate().filter(|(_, l)| !l.is_empty()) {
        let op: Operation = serde_json::from_slice(line).map_err(|e| ExportError(format!("{OPERATIONS}, line {}: {e}", n + 1)))?;
        if !ids.insert(op.id.clone()) {
            return Err(ExportError(format!("{OPERATIONS}, line {}: operation {} twice", n + 1, op.id)));
        }
        if !manifest.private_data && is_private(&op) {
            return Err(ExportError(format!("{OPERATIONS}, line {}: private data in a file that says it has none", n + 1)));
        }
        operations.push(op);
    }
    if operations.len() != manifest.operations {
        return Err(ExportError(format!("incomplete file: {} operations, the manifest says {}", operations.len(), manifest.operations)));
    }
    Ok((manifest, operations))
}

/// Import an export file into a show file (new, or a copy of the same show).
pub fn import_into(store: &mut crate::Store, bytes: &[u8]) -> Result<Manifest, ExportError> {
    let (manifest, operations) = read(bytes)?;
    if let (Some(have), Some(file)) = (&store.state().event, &manifest.event) {
        if have.id.0 != file.id {
            return Err(ExportError(format!("the file holds event {} ({}), this show is {} ({})", file.number, file.id, have.number, have.id)));
        }
    }
    store.append(&operations).map_err(|e| ExportError(e.0))?;
    Ok(manifest)
}

fn file(zip: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>, ExportError> {
    let mut f = zip.by_name(name).map_err(|_| ExportError(format!("not an EquiEvents show file (no {name})")))?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).map_err(|e| ExportError(format!("{name}: {e}")))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use crate::oplog::Clock;

    fn log() -> Vec<Operation> {
        let mut clock = Clock::new("office");
        let event = Event {
            id: EventId(1),
            number: "2027.1".into(),
            title: vec!["Concours".into()],
            place: "Avenches".into(),
            from: "2027-04-10".into(),
            to: "2027-04-11".into(),
            fixed_start_numbers: false,
            start_numbers_assigned: false,
            organiser: String::new(),
            postcode: String::new(),
            disciplines: vec!["CS".into()],
            se_last_change: None,
            record: serde_json::Value::Null,
        };
        let changes = vec![
            Change::EventImported {
                event,
                published_classes: vec![],
                persons: vec![],
                horses: vec![],
                entries: vec![],
                teams: vec![],
                scoring_tables: vec![],
                programmes: vec![],
                reasons: vec![],
                categories: vec![],
                master: Default::default(),
            },
            Change::PrivateDataDelivered { contacts: vec![Contact { person: PersonId(7), email: "a@b.invalid".into(), ..Default::default() }], notes: vec![] },
        ];
        changes.into_iter().enumerate().map(|(i, change)| Operation { id: format!("op{i}"), stamp: clock.tick(1000 + i as u64), author: "office".into(), change }).collect()
    }

    #[test]
    fn round_trip_and_deterministic() {
        let bytes = write(&log(), Privacy::All, "0.1.0", "2027-04-11T18:00:00Z").unwrap();
        assert_eq!(bytes, write(&log(), Privacy::All, "0.1.0", "2027-04-11T18:00:00Z").unwrap());
        let (manifest, ops) = read(&bytes).unwrap();
        assert_eq!(ops, log());
        assert_eq!(manifest.event.unwrap().number, "2027.1");
        assert!(manifest.private_data);
    }

    #[test]
    fn without_private_data() {
        let bytes = write(&log(), Privacy::Public, "0.1.0", "2027-04-11T18:00:00Z").unwrap();
        let (manifest, ops) = read(&bytes).unwrap();
        assert!(!manifest.private_data);
        assert_eq!(ops.len(), 1);
        assert!(!ops.iter().any(is_private));
    }

    /// Rebuild an archive with a changed manifest or log.
    fn rebuilt(bytes: &[u8], edit: impl Fn(&str, Vec<u8>) -> Vec<u8>) -> Vec<u8> {
        let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut out = ZipWriter::new(Cursor::new(Vec::new()));
        for name in [MANIFEST, OPERATIONS] {
            let content = file(&mut zip, name).unwrap();
            out.start_file(name, SimpleFileOptions::default()).unwrap();
            out.write_all(&edit(name, content)).unwrap();
        }
        out.finish().unwrap().into_inner()
    }

    #[test]
    fn damaged_or_foreign_files_are_refused() {
        let bytes = write(&log(), Privacy::All, "0.1.0", "2027-04-11T18:00:00Z").unwrap();
        assert!(read(b"Rosson: 3.3.15\n%7B%7D").unwrap_err().0.contains("not an EquiEvents show file"));
        // A line lost.
        let cut = rebuilt(&bytes, |name, c| if name == OPERATIONS { c.splitn(2, |b| *b == b'\n').nth(1).unwrap().to_vec() } else { c });
        assert!(read(&cut).unwrap_err().0.contains("incomplete file"), "{:?}", read(&cut));
        // Made by a newer app.
        let newer = rebuilt(&bytes, |name, c| if name == MANIFEST { String::from_utf8(c).unwrap().replace("\"version\": 1", "\"version\": 2").into_bytes() } else { c });
        assert!(read(&newer).unwrap_err().0.contains("newer version"));
        // Says public, holds private data.
        let lying = rebuilt(&bytes, |name, c| if name == MANIFEST { String::from_utf8(c).unwrap().replace("\"private_data\": true", "\"private_data\": false").into_bytes() } else { c });
        assert!(read(&lying).unwrap_err().0.contains("private data"));
    }

    #[test]
    fn importing_twice_adds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = write(&log(), Privacy::All, "0.1.0", "2027-04-11T18:00:00Z").unwrap();
        let mut store = crate::Store::open(&dir.path().join("show.sqlite")).unwrap();
        import_into(&mut store, &bytes).unwrap();
        import_into(&mut store, &bytes).unwrap();
        assert_eq!(store.log().unwrap().len(), 2);
        assert_eq!(store.state().contacts.len(), 1);
        // Another event's file is refused.
        let mut other = log();
        if let Change::EventImported { event, .. } = &mut other[0].change {
            event.id = EventId(2);
        }
        let other = write(&other, Privacy::All, "0.1.0", "2027-04-11T18:00:00Z").unwrap();
        assert!(import_into(&mut store, &other).unwrap_err().0.contains("holds event"));
    }
}
