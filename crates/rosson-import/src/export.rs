//! Reading Rosson's export files.
//!
//! Rosson writes two kinds, both `Rosson: <version>` on the first line, then its IndexedDB
//! stores as JSON, URL-encoded (`encodeURIComponent`):
//!
//! - `<nr>_export_<yyyymmdd>.txt`: one event (the "Veranstaltung abschliessen" dialog);
//! - `rosson_export_<yyyymmdd>.txt`: the whole local database, possibly several events.
//!
//! [`RossonExport::events`] lists the events in a file, [`RossonExport::event`] cuts one out,
//! ready for [`crate::ops::to_operations`] (the event as it ended) or [`crate::delivered`] (the
//! event as Swiss Equestrian delivered it, to rehearse a show with). Exports older than Rosson
//! 3.3.0 are refused: their format drifted.

use serde_json::{Map, Value};

use crate::{id, text, ImportError};

/// The oldest Rosson version whose exports are read.
pub const OLDEST_VERSION: [u32; 3] = [3, 3, 0];

/// Stores that belong to an event (filtered by `veranstaltung_id`).
pub const EVENT_STORES: [&str; 10] = ["ausschreibung_pruefungen", "pruefungen", "zwischenpruefungen", "nennungen", "starter", "equipen", "resultate", "zwischenresultate", "finalpruefungen", "offizielle_zuweisungen"];
/// Swiss Equestrian master data, kept whole.
pub const MASTER_STORES: [&str; 9] = ["disziplinen", "wertungen", "programme", "aufgabegruende", "pruefungsfunktionen", "veranstaltungsfunktionen", "pruefungskategorien", "offizielle", "veranstalter"];

#[derive(Debug, Clone)]
pub struct RossonExport {
    /// The Rosson version that wrote it.
    pub version: String,
    /// Rosson's stores.
    pub data: Value,
}

/// An event in an export file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventInExport {
    pub id: String,
    pub number: String,
    pub title: String,
    pub from: String,
    pub to: String,
    pub entries: usize,
    pub results: usize,
}

/// Read an export file.
pub fn read(bytes: &[u8]) -> Result<RossonExport, ImportError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ImportError("not a Rosson export (not text)".into()))?;
    let (header, body) = text.split_once('\n').ok_or_else(|| ImportError("not a Rosson export (no header line)".into()))?;
    let version = header.trim().strip_prefix("Rosson: ").ok_or_else(|| ImportError("not a Rosson export (no \"Rosson: <version>\" line)".into()))?.to_string();
    let parts: Vec<u32> = version.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    if parts.as_slice() < OLDEST_VERSION.as_slice() {
        return Err(ImportError(format!("exports of Rosson {version} cannot be read (from 3.3.0 on)")));
    }
    let json = decode_uri_component(body.trim_end()).map_err(|e| ImportError(format!("not a Rosson export: {e}")))?;
    let data: Value = serde_json::from_str(&json).map_err(|e| ImportError(format!("not a Rosson export: {e}")))?;
    if !data.is_object() {
        return Err(ImportError("not a Rosson export (no stores)".into()));
    }
    Ok(RossonExport { version, data })
}

impl RossonExport {
    /// The event's access key, as Rosson's export carries it (`None`: none, or the corpus's
    /// placeholder).
    pub fn access_key(&self, event_id: &str) -> Option<String> {
        let event = self.store("veranstaltungen").iter().find(|v| id(&v["id"]) == event_id)?;
        event["access_key"].as_str().map(str::trim).filter(|k| !k.is_empty() && *k != "REDACTED").map(str::to_string)
    }

    fn store(&self, name: &str) -> &[Value] {
        self.data[name].as_array().map(Vec::as_slice).unwrap_or(&[])
    }

    /// The events in the file.
    pub fn events(&self) -> Vec<EventInExport> {
        self.store("veranstaltungen")
            .iter()
            .map(|v| {
                let event = id(&v["id"]);
                let count = |store: &str| self.store(store).iter().filter(|r| id(&r["veranstaltung_id"]) == event).count();
                EventInExport {
                    number: text(&v["nr"]),
                    title: text(&v["erste_titelzeile"]),
                    from: text(&v["von"]),
                    to: text(&v["bis"]),
                    entries: count("nennungen"),
                    results: count("resultate"),
                    id: event,
                }
            })
            .collect()
    }

    /// One event with the master data, as one export of that event.
    pub fn event(&self, event_id: &str) -> Result<Value, ImportError> {
        let event = self.store("veranstaltungen").iter().find(|v| id(&v["id"]) == event_id).ok_or_else(|| ImportError(format!("event {event_id} is not in the file")))?;
        let mut out = Map::new();
        out.insert("veranstaltungen".into(), Value::Array(vec![event.clone()]));
        for store in EVENT_STORES {
            out.insert(store.into(), Value::Array(self.store(store).iter().filter(|r| id(&r["veranstaltung_id"]) == event_id).cloned().collect()));
        }
        for store in MASTER_STORES {
            out.insert(store.into(), Value::Array(self.store(store).to_vec()));
        }
        // Per-event settings Rosson keeps in localStorage (some affect prints).
        let local = self.data["localstore"].get(event_id).cloned();
        out.insert("localstore".into(), Value::Object(local.map(|l| Map::from_iter([(event_id.to_string(), l)])).unwrap_or_default()));
        Ok(Value::Object(out))
    }
}

/// JavaScript's `decodeURIComponent`: `%XX` sequences are UTF-8 bytes; everything else as is.
fn decode_uri_component(s: &str) -> Result<String, String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
            out.push(hex.ok_or_else(|| format!("bad escape at {i}"))?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "bad UTF-8 in an escape".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// JavaScript's `encodeURIComponent`.
    fn encode(s: &str) -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    }

    fn file(version: &str, data: &Value) -> Vec<u8> {
        format!("Rosson: {version}\n{}", encode(&data.to_string())).into_bytes()
    }

    #[test]
    fn reads_a_database_export_and_cuts_out_an_event() {
        let data = serde_json::json!({
            "veranstaltungen": [
                { "id": 1, "nr": "2026.1", "erste_titelzeile": "Concours d'été à Môtier", "von": "2026-06-01", "bis": "2026-06-02" },
                { "id": 2, "nr": "2026.2", "erste_titelzeile": "Herbstturnier", "von": "2026-09-01", "bis": "2026-09-01" }
            ],
            "nennungen": [{ "id": 10, "veranstaltung_id": 1 }, { "id": 11, "veranstaltung_id": 2 }, { "id": 12, "veranstaltung_id": "2" }],
            "resultate": [{ "id": "r", "veranstaltung_id": 2 }],
            "wertungen": [{ "id": 14 }],
            "localstore": { "2": { "druck": true } }
        });
        let export = read(&file("3.3.15", &data)).unwrap();
        assert_eq!(export.version, "3.3.15");
        let events = export.events();
        assert_eq!(events[0].title, "Concours d'été à Môtier");
        assert_eq!((events[1].entries, events[1].results), (2, 1));
        let event = export.event("2").unwrap();
        assert_eq!(event["nennungen"].as_array().unwrap().len(), 2);
        assert_eq!(event["wertungen"].as_array().unwrap().len(), 1);
        assert_eq!(event["localstore"]["2"]["druck"], true);
        assert!(export.event("3").is_err());
    }

    #[test]
    fn refuses_what_it_cannot_read() {
        assert!(read(b"hello").is_err());
        assert!(read(&file("3.2.9", &serde_json::json!({}))).unwrap_err().0.contains("3.2.9"));
        assert!(read(b"Rosson: 3.3.15\n%7B%ZZ").is_err());
    }
}
