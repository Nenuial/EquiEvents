//! A simulated Swiss Equestrian, for sending before the pilot (April 2027): it answers as Swiss
//! Equestrian would (as far as the contract tells) and keeps every request it received, the
//! access key masked, so the office sees exactly what would be sent. It checks what Swiss
//! Equestrian checks first: an access key, and on writes today's [`crate::rosson_key`]. No I/O:
//! the office answers with it on the computer, equievents.ch on the server.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::{json, Value};

use crate::http::{HttpRequest, HttpResponse};
use crate::Method;

/// The simulation's address (never resolved: requests to it are answered where they are made).
pub const SIMULATION: &str = "https://simulation.equievents.invalid/api/veranstaltersoftware/v2";

/// What the simulation received.
#[derive(Debug, Clone, Serialize)]
pub struct Received {
    pub millis: u64,
    pub method: &'static str,
    /// The endpoint, e.g. `putResultate` or `deleteStartlisten/108196`.
    pub path: String,
    /// The query, the access key masked.
    pub query: Vec<(String, String)>,
    /// The body, the access key masked.
    pub body: Option<Value>,
    /// What it answered.
    pub status: u16,
    pub answer: Value,
}

/// The simulated Swiss Equestrian.
#[derive(Default)]
pub struct Simulation {
    /// By show.
    received: Mutex<BTreeMap<String, Vec<Received>>>,
    lookups: Mutex<u64>,
}

/// How many requests the simulation keeps per show.
const KEPT: usize = 200;

impl Simulation {
    /// Answer a request for a show, `today_key`: today's rosson_key; `reads`: the answer to a
    /// read by its endpoint, when the simulation has one (the registry).
    pub fn answer(&self, show: &str, request: &HttpRequest, today_key: &str, millis: u64, reads: &dyn Fn(&str) -> Option<Value>) -> HttpResponse {
        let rest = request.url.strip_prefix(SIMULATION).unwrap_or(&request.url);
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        let path = path.trim_start_matches('/').to_string();
        let query: Vec<(String, String)> = query.split('&').filter(|q| !q.is_empty()).map(|q| q.split_once('=').map_or((q.to_string(), String::new()), |(k, v)| (k.to_string(), decode(v)))).collect();
        let mut body: Option<Value> = request.body.as_ref().and_then(|b| serde_json::from_slice(b).ok());
        let field = |name: &str| query.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()).or_else(|| body.as_ref().and_then(|b| b[name].as_str().map(str::to_string))).unwrap_or_default();
        let (access_key, rosson_key) = (field("access_key"), field("rosson_key"));

        let endpoint = path.split('/').next().unwrap_or("");
        let (status, answer) = if access_key.trim().is_empty() {
            (403, json!({ "message": "Simulation: no access_key" }))
        } else if matches!(request.method, Method::Post | Method::Delete) && rosson_key != today_key {
            // Writes carry the date-based key (reads only the access key, as Rosson sends them).
            (403, json!({ "message": format!("Simulation: rosson_key {rosson_key:?} is not today's") }))
        } else {
            match (request.method, endpoint) {
                (Method::Post, "putResultate" | "putStartlisten" | "putVeranstaltung" | "putZwischenresultate") => (200, json!({})),
                (Method::Delete, "deleteStartlisten") => (200, json!({})),
                (Method::Get, "lookupZuweisung") => {
                    let mut n = self.lookups.lock().unwrap_or_else(|e| e.into_inner());
                    *n += 1;
                    (200, json!({ "id": 9_000_000 + *n }))
                }
                (Method::Get, endpoint) => match reads(endpoint) {
                    Some(answer) => (200, answer),
                    None => (501, json!({ "message": "Simulation: loading from Swiss Equestrian is not simulated" })),
                },
                _ => (404, json!({ "message": format!("Simulation: no endpoint {endpoint}") })),
            }
        };

        let mask = |v: &str| if v.is_empty() { String::new() } else { "•••".to_string() };
        if let Some(b) = body.as_mut().and_then(Value::as_object_mut) {
            if let Some(k) = b.get_mut("access_key") {
                *k = json!(mask(k.as_str().unwrap_or("")));
            }
        }
        let query = query.into_iter().map(|(k, v)| if k == "access_key" { (k, mask(&v)) } else { (k, v) }).collect();
        let method = match request.method {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        };
        let mut received = self.received.lock().unwrap_or_else(|e| e.into_inner());
        let list = received.entry(show.to_string()).or_default();
        // A long answer (the registry) is kept as its size only: it is not what the office looks at.
        let text = answer.to_string();
        let kept = if text.len() > 20_000 { json!({ "bytes": text.len() }) } else { answer.clone() };
        list.push(Received { millis, method, path, query, body, status, answer: kept });
        if list.len() > KEPT {
            list.remove(0);
        }
        HttpResponse { status, status_text: String::new(), body: text.into_bytes() }
    }

    /// What the simulation received for a show, the latest first.
    pub fn received(&self, show: &str) -> Vec<Received> {
        let received = self.received.lock().unwrap_or_else(|e| e.into_inner());
        received.get(show).map(|l| l.iter().rev().cloned().collect()).unwrap_or_default()
    }
}

/// `%XX` and `+` as AngularJS encodes them.
fn decode(v: &str) -> String {
    let bytes = v.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_angular_queries() {
        assert_eq!(decode("a+b%2Cc"), "a b,c");
        assert_eq!(decode("100%"), "100%");
    }
}
