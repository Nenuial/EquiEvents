//! The Swiss Equestrian show-organiser API: `my.swiss-equestrian.ch/api/veranstaltersoftware/v2`.
//!
//! The API is the one frozen contract of EquiEvents (docs/architecture/se-api.md): requests
//! are made exactly as Rosson 3.3.15 makes them, checked against Rosson's own requests captured
//! offline (`golden/corpus/<show>/api/`, made by `golden/src/api-fixtures.mjs`).
//!
//! Every request carries the event key (`access_key`, given to the organiser by Swiss Equestrian) and the
//! date-based key [`rosson_key`]. Uploads also carry `rosson_version`: the latest Rosson version,
//! found as Rosson's own update check finds it ([`version`]).
//!
//! Nothing here opens a connection: requests are built as [`Request`] values, and [`http`] says
//! how to send them (polling, retries, errors); the app does the sending. Real calls are only
//! possible from the start of the 2027 season (pilot).

pub mod actions;
pub mod event;
pub mod http;
pub mod load;
pub mod results;
pub mod simulation;
pub mod startlists;
pub mod titles;
pub mod version;

use serde_json::{json, Value};

/// What every upload carries: the event key, the date-based key and the Rosson version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keys {
    pub access_key: String,
    pub rosson_key: String,
    /// See [`version`].
    pub rosson_version: String,
}

impl Keys {
    /// The start of every upload body.
    pub fn envelope(&self) -> Value {
        json!({ "access_key": self.access_key, "rosson_key": self.rosson_key, "rosson_version": self.rosson_version })
    }
}

/// Production.
pub const PRODUCTION: &str = "https://my.swiss-equestrian.ch/api/veranstaltersoftware/v2";
/// Swiss Equestrian's test system.
pub const STAGE: &str = "https://my-stage.swiss-equestrian.ch/api/veranstaltersoftware/v2";

/// Swiss Equestrian's demo events (Rosson's « Manifestation test »; `_fa`: driving), by their
/// keys: anyone may load them, so no one owns a show of them.
pub const DEMO_KEYS: [&str; 2] = ["demoveranstaltung", "demoveranstaltung_fa"];

/// What loading an event downloads (`getStammdaten`, `getOffizielle`, `getVeranstaltung`).
pub const LOADING_ENDPOINTS: [&str; 3] = ["getStammdaten", "getOffizielle", "getVeranstaltung"];

/// The date-based key every request carries, for the calendar day the request is made (local
/// time in Switzerland, as Rosson): the date as `ddmmyyyy`, Base64-encoded, letters shifted by 13
/// (ROT13), the first 10 characters.
///
/// This is how the API was set up to work; it is part of the contract, not a secret. The event
/// key (`access_key`) is what protects an event's non-public data.
pub fn rosson_key(year: u32, month: u32, day: u32) -> String {
    let date = format!("{day:02}{month:02}{year:04}");
    base64(date.as_bytes()).chars().map(rot13).take(10).collect()
}

fn rot13(c: char) -> char {
    match c {
        'a'..='z' => (((c as u8 - b'a') + 13) % 26 + b'a') as char,
        'A'..='Z' => (((c as u8 - b'A') + 13) % 26 + b'A') as char,
        _ => c,
    }
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// An HTTP method used by the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Patch,
    Delete,
}

/// One request to the API.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: Method,
    /// Below the base URL, e.g. `/putResultate`.
    pub path: String,
    pub query: Vec<(String, String)>,
    /// JSON body (POST, PATCH).
    pub body: Option<Value>,
}

/// The endpoints, as Rosson 3.3.15 uses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    /// GET, polled every 4 s while it answers 202: the event with classes, entries, riders, horses.
    GetVeranstaltung,
    /// GET (polled): every person of Swiss Equestrian's registry (Rosson's `alle_personen`).
    GetPersonen,
    /// GET (polled): every horse of the registry (Rosson's `pferde`).
    GetPferde,
    /// GET (polled): officials.
    GetOffizielle,
    /// GET: master data (scoring tables, programmes, reasons, categories, roles).
    GetStammdaten,
    /// GET: the Swiss Equestrian id of an official's assignment (`person_id`, `funktion_id`).
    LookupZuweisung,
    /// POST: the start lists of a published class.
    PutStartlisten,
    /// DELETE `/deleteStartlisten/<published class id>`.
    DeleteStartlisten,
    /// POST: the results of a class.
    PutResultate,
    /// POST: results of a part of a combined class (driving, eventing).
    PutZwischenresultate,
    /// POST: the event: officials, entry counts, waiting lists.
    PutVeranstaltung,
    /// POST: distances from riders' towns to the show ground.
    PutGeolocation,
    /// GET `/getBackups`, GET `/getBackup/<id>`, POST `/putBackup`: Rosson's backups on the server.
    Backups,
    /// PATCH `/updateProgramme/<id>`, `/updateWertungen/<id>`: master-data editing (Rosson's
    /// developer mode only).
    MasterDataEditing,
}

impl Endpoint {
    /// Asked again every 4 seconds while Swiss Equestrian answers 202 (Rosson: `Xhr.getWithInterval`).
    pub fn polled(self) -> bool {
        matches!(self, Endpoint::GetVeranstaltung | Endpoint::GetPersonen | Endpoint::GetPferde | Endpoint::GetOffizielle)
    }

    /// The endpoint of a request path.
    pub fn of_path(path: &str) -> Option<Endpoint> {
        let first = path.trim_start_matches('/').split('/').next().unwrap_or("");
        Some(match first {
            "getVeranstaltung" => Endpoint::GetVeranstaltung,
            "getPersonen" => Endpoint::GetPersonen,
            "getPferde" => Endpoint::GetPferde,
            "getOffizielle" => Endpoint::GetOffizielle,
            "getStammdaten" => Endpoint::GetStammdaten,
            "lookupZuweisung" => Endpoint::LookupZuweisung,
            "putStartlisten" => Endpoint::PutStartlisten,
            "deleteStartlisten" => Endpoint::DeleteStartlisten,
            "putResultate" => Endpoint::PutResultate,
            "putZwischenresultate" => Endpoint::PutZwischenresultate,
            "putVeranstaltung" => Endpoint::PutVeranstaltung,
            "putGeolocation" => Endpoint::PutGeolocation,
            "getBackups" | "getBackup" | "putBackup" => Endpoint::Backups,
            "updateProgramme" | "updateWertungen" => Endpoint::MasterDataEditing,
            _ => return None,
        })
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_as_the_browser() {
        assert_eq!(base64(b"13112026"), "MTMxMTIwMjY=");
        assert_eq!(base64(b"a"), "YQ==");
        assert_eq!(base64(b"ab"), "YWI=");
    }
}
