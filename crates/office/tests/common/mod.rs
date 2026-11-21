//! Shared by the office's tests: corpus shows as Rosson export files.
#![allow(dead_code)]

use std::fs;
use std::path::Path;

use serde_json::Value;

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A corpus show written as a Rosson export file.
pub fn rosson_file(dir: &Path, show: &str) -> String {
    rosson_file_edited(dir, show, |_| {})
}

/// A corpus show, changed first (to make what the corpus does not have), as a Rosson export file.
pub fn rosson_file_edited(dir: &Path, show: &str, edit: impl FnOnce(&mut Value)) -> String {
    let snapshot = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/corpus").join(show).join("snapshot.json")).unwrap();
    let mut data: Value = serde_json::from_str(&snapshot).unwrap();
    edit(&mut data);
    let path = dir.join(format!("{show}_export_20260101.txt"));
    fs::write(&path, format!("Rosson: 3.3.15\n{}", encode(&data.to_string()))).unwrap();
    path.to_string_lossy().to_string()
}

/// Riders and horses in Swiss Equestrian's registry on this computer (`<library>/registry/`), as
/// `getPersonen` and `getPferde` deliver them: the office picks riders and horses there. Written
/// before the office first searches it.
pub fn registry(library: &Path, persons: Vec<Value>, horses: Vec<Value>) {
    let dir = library.join("registry");
    fs::create_dir_all(&dir).unwrap();
    let about = serde_json::json!({ "persons": persons.len(), "horses": horses.len(), "persons_at": 1_794_000_000_000u64, "horses_at": 1_794_000_000_000u64, "simulated": false });
    fs::write(dir.join("persons.json"), Value::Array(persons).to_string()).unwrap();
    fs::write(dir.join("horses.json"), Value::Array(horses).to_string()).unwrap();
    fs::write(dir.join("about.json"), about.to_string()).unwrap();
}

/// A rider of the registry; `licences` as Swiss Equestrian writes them ("Gast": a guest licence).
pub fn registry_rider(id: u64, first_name: &str, last_name: &str, town: &str, licences: &str) -> Value {
    serde_json::json!({ "id": id, "vorname": first_name, "nachname": last_name, "ort": town, "vereinsname": "", "lizenzen": licences, "land": "CH", "strasse": "Rue du Test 1", "postleitzahl": "1630", "email": "rider@example.invalid" })
}

/// A horse of the registry, its fee paid for `years`.
pub fn registry_horse(id: u64, name: &str, years: &[u32]) -> Value {
    serde_json::json!({ "id": id, "name": name, "signalement_label": "Fuchs, Stute, 2015", "besitzer": "Owner", "zuechter": "Breeder", "bezahlte_sportjahre": years })
}
