//! What of Rosson's records is public: the show keeps Swiss Equestrian's records as delivered
//! (to write the show in Rosson's format again), without what only the show office may see —
//! addresses, phones, emails, birth dates, remarks — which goes to `Contact` and `EntryNote`,
//! and never the event's access key.

use serde_json::Value;

/// Not public on an entry (`nennungen`).
pub const ENTRY_PRIVATE: [&str; 8] = ["reiter_strasse", "reiter_plz", "telefon_privat", "telefon_mobil", "email", "geboren_am", "bemerkungen", "begleit_person"];
/// Not public on a person (officials, `offizielle_personen`, `offizielle`).
pub const PERSON_PRIVATE: [&str; 9] = ["anrede", "strasse", "postleitzahl", "email", "telefon_privat", "telefon_mobil", "telefon_arbeit", "sprache", "geboren_am"];

fn without(record: &Value, keys: &[&str]) -> Value {
    let mut record = record.clone();
    if let Some(map) = record.as_object_mut() {
        for k in keys {
            map.remove(*k);
        }
    }
    record
}

/// An entry's record without its private fields, its horses and the office's note (both kept
/// on the entry).
pub fn entry(record: &Value) -> Value {
    let mut r = without(record, &ENTRY_PRIVATE);
    if let Some(map) = r.as_object_mut() {
        map.remove("pferde");
        map.remove("kommentar");
    }
    r
}

/// A person's record (an official) without its private fields.
pub fn person(record: &Value) -> Value {
    without(record, &PERSON_PRIVATE)
}

/// The event's record without its access key, its officials without their private fields,
/// their roles without the number given with them (a phone).
pub fn event(record: &Value) -> Value {
    let mut r = without(record, &["access_key"]);
    if let Some(people) = r.get_mut("offizielle_personen").and_then(Value::as_array_mut) {
        for p in people.iter_mut() {
            *p = person(p);
        }
    }
    // An official's role may carry a phone number (`nummer`).
    if let Some(roles) = r.get_mut("beteiligungen").and_then(Value::as_array_mut) {
        for b in roles.iter_mut() {
            *b = without(b, &["nummer"]);
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn private_fields_are_left_out() {
        let e = super::event(&json!({ "id": 1, "access_key": "secret", "offizielle_personen": [{ "id": 2, "vorname": "Ann", "email": "a@b.ch", "strasse": "Rue 1" }] }));
        assert_eq!(e, json!({ "id": 1, "offizielle_personen": [{ "id": 2, "vorname": "Ann" }] }));
        let n = super::entry(&json!({ "id": 3, "reiter_vorname": "Bea", "reiter_strasse": "Rue 2", "email": "b@c.ch", "bemerkungen": "x", "pferde": [], "status": "definitiv" }));
        assert_eq!(n, json!({ "id": 3, "reiter_vorname": "Bea", "status": "definitiv" }));
    }
}
