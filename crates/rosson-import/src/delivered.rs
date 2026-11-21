//! An event from a Rosson export, as Swiss Equestrian delivered it before the show: to rehearse
//! a show with a past event, through the same code that loads a real one (`se_api::load`).
//!
//! What Rosson stored is what it received plus what it adds itself (`VeranstaltungService.get`,
//! `nennungAnreichern`, `pferdeAnreichern`, `zuweisungErstellen`, `DatabaseFactory.put`); taking
//! its additions off gives back the responses: [`event`] (`getVeranstaltung`), [`master_data`]
//! (`getStammdaten`), [`officials`] (`getOffizielle`). What happened at the show is left out:
//! entries made there, start numbers, "does not start" set by the office, classes, results.
//! Entries come with the horse they were ridden with: Rosson's change of horse at the show
//! updates the horses written on the entry but not `pferd_id` (docs/deviations.md E41), and the
//! horse first entered is often not known any more.
//!
//! The same rebuild makes the test fixtures (`golden/src/load-fixtures.mjs`, `delivered`); the
//! tests check that both give the same responses.

use std::collections::{BTreeMap, HashSet};

use serde_json::{json, Map, Value};

/// Fields as Swiss Equestrian delivers them; the others are Rosson's own.
const EVENT: [&str; 13] = ["id", "nr", "disziplin_code", "von", "bis", "erste_titelzeile", "zweite_titelzeile", "dritte_titelzeile", "veranstalter", "ort", "postleitzahl", "nennschluss", "beteiligungen"];
const PUBLISHED_CLASS: [&str; 24] = [
    "id", "datum", "nummer", "index", "disziplin_id", "kategorie_id", "kategorie_text", "wertung_id", "wertung_text", "springen_anzahl_stufen", "equipe_anzahl_nennungen", "kombipruefung_disziplinen",
    "fahren_zwischenpruefungen", "fahren_freier_text", "fahren_gespanne", "fahren_lizenzen", "fahren_nur_pony", "bemerkungen", "nenngeld1", "nenngeld2", "status", "geaendert_am", "reservierte_plaetze",
    "anzahl_veranstalter_startplaetze",
];
const ENTRY: [&str; 28] = [
    "id", "ausschreibung_pruefung_id", "nenner_id", "reiter_id", "pferd_id", "pferd2_id", "pferd3_id", "pferd4_id", "pferd5_id", "pferd6_id", "gastpferd_name", "gwp_reiter", "gwp_pferd", "gwp_paar", "stufe",
    "equipe_id", "fahren_gespann", "wagenbreite", "hors_concours", "programm_an_besitzer", "bemerkungen", "begleit_person", "nenngeld_bezahlt", "status", "programm_id", "programm_name", "erstellt_am", "geaendert_am",
];
const TEAM: [&str; 4] = ["id", "name", "veranstaltung_id", "ausschreibung_pruefung_id"];
const PERSON: [&str; 22] = [
    "id", "anrede", "vorname", "nachname", "strasse", "postleitzahl", "ort", "email", "telefon_privat", "telefon_mobil", "sprache", "land", "land_de", "land_fr", "land_it", "land_en", "lizenzen", "lizenzen_status",
    "nachwuchsreiter", "vereinsname", "geboren_am", "geaendert_am",
];
/// A rider's record as Rosson copies it onto entries: delivered field ← field on the entry.
const RIDER: [(&str, &str); 13] = [
    ("vorname", "reiter_vorname"),
    ("nachname", "reiter_nachname"),
    ("strasse", "reiter_strasse"),
    ("postleitzahl", "reiter_plz"),
    ("ort", "reiter_ort"),
    ("land", "reiter_land"),
    ("telefon_privat", "telefon_privat"),
    ("telefon_mobil", "telefon_mobil"),
    ("email", "email"),
    ("geboren_am", "geboren_am"),
    ("nachwuchsreiter", "nachwuchsreiter"),
    ("lizenzen", "lizenzen"),
    ("vereinsname", "vereinsname"),
];
/// A horse's record as Rosson copies it onto entries.
pub(crate) const HORSE: [(&str, &str); 10] = [
    ("name", "pferdename"),
    ("signalement_label", "pferd_signalement"),
    ("besitzer", "pferd_besitzer"),
    ("mitbesitzer", "pferd_mitbesitzer"),
    ("zuechter", "pferd_zuechter"),
    ("hat_pony_startbewilligung", "hat_pony_startbewilligung"),
    ("pony_groessenkategorie", "pony_groessenkategorie"),
    ("vater_name", "vater_name"),
    ("mutter_name", "mutter_name"),
    ("vater_der_mutter_name", "vater_der_mutter_name"),
];
const HORSE_FIELDS: [&str; 6] = ["pferd_id", "pferd2_id", "pferd3_id", "pferd4_id", "pferd5_id", "pferd6_id"];
/// The stores Rosson fills from `getStammdaten`.
const MASTER_DATA: [&str; 8] = ["aufgabegruende", "disziplinen", "pruefungsfunktionen", "pruefungskategorien", "programme", "veranstalter", "veranstaltungsfunktionen", "wertungen"];
const PLACEHOLDER_HORSE: u64 = 5;

fn pick(record: &Value, fields: &[&str]) -> Map<String, Value> {
    fields.iter().filter_map(|f| Some((f.to_string(), record.get(*f)?.clone()))).collect()
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

fn by_id(rows: &mut [Value]) {
    rows.sort_by(|a, b| a["id"].as_f64().unwrap_or(0.0).total_cmp(&b["id"].as_f64().unwrap_or(0.0)));
}

/// An entry as delivered, with the horses it was ridden with.
fn as_ridden(n: &Value) -> Value {
    let mut out = pick(n, &ENTRY);
    let horses = n["pferde"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    for (field, p) in HORSE_FIELDS.iter().zip(horses) {
        let id = if truthy(&p["gastpferd"]) { Some(json!(PLACEHOLDER_HORSE)) } else { p.get("pferd_id").cloned() };
        match id {
            Some(id) => out.insert(field.to_string(), id),
            None => out.remove(*field),
        };
    }
    if let Some(first) = horses.first() {
        out.insert("gastpferd_name".into(), if truthy(&first["gastpferd"]) { first["pferdename"].clone() } else { Value::Null });
    }
    Value::Object(out)
}

/// The `getVeranstaltung` response the event (one event, as cut out of an export) was loaded from.
pub fn event(snapshot: &Value) -> Value {
    let list = |name: &str| snapshot[name].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let stored = &list("veranstaltungen")[0];
    // Entries made at the show have no Swiss Equestrian creation time.
    let entries: Vec<&Value> = list("nennungen").iter().filter(|n| truthy(&n["erstellt_am"])).collect();
    let mut latest_first = entries.clone();
    latest_first.sort_by(|a, b| crate::text(&b["geaendert_am"]).cmp(&crate::text(&a["geaendert_am"])));

    // People: riders as copied onto their entries, officials the event still lists.
    let mut personen: BTreeMap<String, Value> = BTreeMap::new();
    for n in &latest_first {
        let key = n["reiter_id"].to_string();
        if personen.contains_key(&key) {
            continue;
        }
        let mut person = Map::from_iter([("id".to_string(), n["reiter_id"].clone())]);
        for (to, from) in RIDER {
            if let Some(v) = n.get(from) {
                person.insert(to.into(), v.clone());
            }
        }
        if truthy(&n["reiter_laender"]) {
            for l in ["de", "fr", "it", "en"] {
                if let Some(v) = n["reiter_laender"].get(l) {
                    person.insert(format!("land_{l}"), v.clone());
                }
            }
        }
        personen.insert(key, Value::Object(person));
    }
    let officials: HashSet<String> = stored["beteiligungen"].as_array().into_iter().flatten().map(|b| crate::id(&b["person_id"])).collect();
    for p in stored["offizielle_personen"].as_array().into_iter().flatten() {
        let key = p["id"].to_string();
        if !personen.contains_key(&key) && officials.contains(&crate::id(&p["id"])) {
            personen.insert(key, Value::Object(pick(p, &PERSON)));
        }
    }

    // Horses as copied onto entries (the guest horse is Rosson's placeholder).
    let mut pferde: BTreeMap<String, Value> = BTreeMap::new();
    for n in &latest_first {
        for p in n["pferde"].as_array().into_iter().flatten() {
            let key = p["pferd_id"].to_string();
            if truthy(&p["gastpferd"]) || p["pferd_id"] == json!(PLACEHOLDER_HORSE) || pferde.contains_key(&key) {
                continue;
            }
            let mut pferd = Map::from_iter([("id".to_string(), p["pferd_id"].clone())]);
            for (to, from) in HORSE {
                if let Some(v) = p.get(from) {
                    pferd.insert(to.into(), v.clone());
                }
            }
            pferde.insert(key, Value::Object(pferd));
        }
    }

    let sorted = |mut rows: Vec<Value>| {
        by_id(&mut rows);
        Value::Array(rows)
    };
    let mut veranstaltung = pick(stored, &EVENT);
    veranstaltung.insert("ausschreibung_pruefungen".into(), sorted(list("ausschreibung_pruefungen").iter().map(|a| Value::Object(pick(a, &PUBLISHED_CLASS))).collect()));
    veranstaltung.insert("nennungen".into(), sorted(entries.iter().map(|n| as_ridden(n)).collect()));
    veranstaltung.insert("nennungen_fa".into(), json!([]));
    veranstaltung.insert("equipen".into(), sorted(list("equipen").iter().map(|e| Value::Object(pick(e, &TEAM))).collect()));
    let mut response = json!({
        "data": { "veranstaltung": veranstaltung, "personen": sorted(personen.into_values().collect()), "pferde": sorted(pferde.into_values().collect()) },
    });
    if let Some(last) = stored.get("letzte_aenderung") {
        response["letzte_aenderung"] = last.clone();
    }
    response
}

fn without(rows: &Value, fields: &[&str]) -> Value {
    Value::Array(rows.as_array().into_iter().flatten().map(|r| Value::Object(r.as_object().map(|o| o.iter().filter(|(k, _)| !fields.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default())).collect())
}

/// The `getStammdaten` response: the master data as Rosson stored it, without its own fields.
pub fn master_data(snapshot: &Value) -> Value {
    let mut stammdaten: Map<String, Value> = MASTER_DATA.iter().map(|s| (s.to_string(), without(&snapshot[*s], &["updated_at", "searchterms"]))).collect();
    stammdaten.insert("etiketten_vorlagen".into(), json!([]));
    json!({ "data": { "stammdaten": stammdaten } })
}

/// The `getOffizielle` response: the officials as Rosson stored them, without its own fields.
pub fn officials(snapshot: &Value) -> Value {
    json!({ "data": { "offizielle": without(&snapshot["offizielle"], &["updated_at", "searchterms"]) } })
}
