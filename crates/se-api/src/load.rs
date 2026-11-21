//! Loading an event from Swiss Equestrian: the master data (`getStammdaten`), the national list
//! of officials (`getOffizielle`) and the event itself (`getVeranstaltung`), read into show
//! operations.
//!
//! The event is read the way Rosson 3.3.15 reads it (`VeranstaltungService.get` and what it
//! calls), because some of what Rosson builds while loading goes back to Swiss Equestrian with
//! the uploads:
//!
//! - **entries** (`nennungen` and `nennungen_fa`): withdrawn, refused and wrong entries do not
//!   start; the rider's name, town and club are written on the entry; so are the horses, built
//!   from the horse records (`pferdeAnreichern`, see [`horses_on_entry`]);
//! - **officials** of the event (`beteiligungen`) become assignments, if Swiss Equestrian
//!   delivered the person (`zuweisungErstellen`); an official who is not on the national list
//!   is `readonly`;
//! - Rosson's placeholder horse (id 5, "PSEUDOPFERD", for horses without a Swiss Equestrian
//!   passport) is known even when the response lacks it;
//! - master data is stamped with the time it arrived (`updated_at`): the category record goes
//!   back whole with dressage results.
//!
//! Non-public data (addresses, phones, email, birth dates, riders' remarks) goes into its own
//! operation, [`Change::PrivateDataDelivered`].
//!
//! The response format is not documented; it is read from Rosson's code and checked by giving
//! Rosson and this module the same response (`tests/load_contract.rs`, fixtures from
//! `golden/src/load-fixtures.mjs`).

use std::collections::{HashMap, HashSet};

use rust_decimal::Decimal;
use serde_json::{json, Value};
use show::model::*;
use show::oplog::Change;
use show::State;

use crate::{Keys, Method, Request};

/// Rosson's placeholder horse, for horses without a Swiss Equestrian passport (guest horses).
pub const PLACEHOLDER_HORSE: u64 = 5;

/// `getStammdaten`: the master data.
pub fn get_stammdaten(keys: &Keys) -> Request {
    get("/getStammdaten", keys, None)
}

/// `getOffizielle`: the national list of officials.
pub fn get_offizielle(keys: &Keys) -> Request {
    get("/getOffizielle", keys, None)
}

/// `getPersonen`: every person of Swiss Equestrian's registry, with contact data (Rosson keeps
/// them as `alle_personen`, for a change of rider). Polled while it answers 202.
pub fn get_personen(keys: &Keys) -> Request {
    get("/getPersonen", keys, None)
}

/// `getPferde`: every horse of the registry (Rosson's `pferde`, for a change of horse). Polled.
pub fn get_pferde(keys: &Keys) -> Request {
    get("/getPferde", keys, None)
}

/// The records of a registry answer: `data.personen` or `data.pferde` (`list`), each with an id.
pub fn read_registry(response: &Value, list: &str) -> Result<Vec<Value>, String> {
    let records = response["data"][list].as_array().or_else(|| response[list].as_array()).ok_or_else(|| format!("no {list} in the answer"))?;
    Ok(records.iter().filter(|r| number(&r["id"]).is_some()).cloned().collect())
}

/// A person of the registry (or of an event) as the show keeps it: its public part and its
/// contact data (private).
pub fn person_of(id: u64, p: &Value) -> (Person, Contact) {
    let person = Person {
        id: PersonId(id),
        first_name: text(&p["vorname"]),
        last_name: text(&p["nachname"]),
        town: text(&p["ort"]),
        nationality: text(&p["land"]),
        club: text(&p["vereinsname"]),
        licences: text(&p["lizenzen"]),
        youth: text(&p["nachwuchsreiter"]),
    };
    let contact = Contact {
        person: PersonId(id),
        salutation: text(&p["anrede"]),
        street: text(&p["strasse"]),
        postcode: text(&p["postleitzahl"]),
        country: text(&p["land"]),
        email: text(&p["email"]),
        phone_private: text(&p["telefon_privat"]),
        phone_mobile: text(&p["telefon_mobil"]),
        language: text(&p["sprache"]),
        birth_date: p["geboren_am"].as_str().filter(|s| s.len() >= 10).map(|s| s[..10].to_string()),
    };
    (person, contact)
}

/// A horse of the registry (or of an event) as the show keeps it, its record with it.
pub fn horse_of(id: u64, h: &Value) -> Horse {
    Horse { id: HorseId(id), name: text(&h["name"]), signalement: text(&h["signalement_label"]), owner: text(&h["besitzer"]), breeder: text(&h["zuechter"]), record: h.clone() }
}

/// `getVeranstaltung`: the event, or with `changed_after` (Swiss Equestrian's time of the last
/// delivery, [`Event::se_last_change`]) what changed since. Swiss Equestrian may answer 202
/// (being prepared): ask again every 4 seconds, as Rosson does.
pub fn get_veranstaltung(keys: &Keys, changed_after: Option<&str>) -> Request {
    get("/getVeranstaltung", keys, changed_after)
}

/// Rosson's GET requests carry the event key and its version (`Xhr.get`), in alphabetical order.
fn get(path: &str, keys: &Keys, changed_after: Option<&str>) -> Request {
    let mut query = vec![("access_key".to_string(), keys.access_key.clone()), ("app_version".to_string(), keys.rosson_version.clone())];
    if let Some(after) = changed_after {
        query.push(("geaendert_nach".into(), after.into()));
    }
    Request { method: Method::Get, path: path.into(), query, body: None }
}

/// Master data, as delivered (`getStammdaten`).
#[derive(Debug, Clone, Default)]
pub struct MasterData {
    pub scoring_tables: Vec<ScoringTableDef>,
    pub programmes: Vec<ProgrammeDef>,
    pub reasons: Vec<ReasonDef>,
    pub categories: Vec<CategoryDef>,
    /// Discipline id → code (CS, CD, ...).
    pub disciplines: HashMap<u64, String>,
    /// Category id → code.
    pub category_codes: HashMap<u64, String>,
    /// Roles at the event (Veranstaltungsfunktion): id → code.
    pub event_roles: HashMap<u64, String>,
    /// Roles in a class (Prüfungsfunktion): id → code.
    pub class_roles: HashMap<u64, String>,
    /// Every role, with its names.
    pub roles: Vec<RoleDef>,
    /// The master data as delivered, by Rosson store, stamped as Rosson stamps what it stores.
    pub records: std::collections::BTreeMap<String, Vec<Value>>,
}

/// Read `getStammdaten`. `received_millis`: when it arrived (Rosson stamps what it stores).
pub fn read_master_data(response: &Value, received_millis: u64) -> Result<MasterData, String> {
    let data = &response["data"]["stammdaten"];
    if !data.is_object() {
        return Err("getStammdaten: no master data in the response".into());
    }
    let list = |name: &str| data[name].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let codes = |name: &str| -> HashMap<u64, String> { list(name).iter().filter_map(|r| Some((number(&r["id"])?, text(&r["code"])))).collect() };
    let disciplines = codes("disziplinen");
    Ok(MasterData {
        scoring_tables: list("wertungen")
            .iter()
            .filter_map(|w| Some(ScoringTableDef { id: number(&w["id"])? as u32, code: text(&w["code"]), name: text(&w["bezeichnung_de"]), name_fr: text(&w["bezeichnung_fr"]), name_it: text(&w["bezeichnung_it"]), definition: text(&w["bewertung_json"]) }))
            .collect(),
        programmes: list("programme")
            .iter()
            .filter_map(|p| {
                Some(ProgrammeDef { id: number(&p["id"])? as u32, name: text(&p["name"]), max_points: decimal(&p["max_punkte"]).unwrap_or_default(), definition: text(&p["programm_json"]), discipline_id: number(&p["disziplin_id"]).map(|d| d as u32) })
            })
            .collect(),
        reasons: list("aufgabegruende")
            .iter()
            .filter_map(|g| Some(ReasonDef { id: number(&g["id"])? as u32, discipline: number(&g["disziplin_id"]).and_then(|d| disciplines.get(&d).cloned()), name: text(&g["bezeichnung_de"]) }))
            .collect(),
        categories: list("pruefungskategorien")
            .iter()
            .filter_map(|k| {
                let mut record = k.clone();
                record["updated_at"] = json!(received_millis);
                Some(CategoryDef { id: number(&k["id"])? as u32, code: text(&k["code"]), programme: number(&k["programm_id"]).map(|p| p as u32), record })
            })
            .collect(),
        roles: [("pruefungsfunktionen", true), ("veranstaltungsfunktionen", false)]
            .iter()
            .flat_map(|(name, in_class)| {
                list(name).iter().filter_map(move |r| {
                    Some(RoleDef { id: number(&r["id"])? as u32, code: text(&r["code"]), in_class: *in_class, name_de: text(&r["bezeichnung_de"]), name_fr: text(&r["bezeichnung_fr"]), name_it: text(&r["bezeichnung_it"]) })
                })
            })
            .collect(),
        records: ["disziplinen", "wertungen", "programme", "aufgabegruende", "pruefungsfunktionen", "veranstaltungsfunktionen", "pruefungskategorien"]
            .iter()
            .filter(|name| data[**name].is_array())
            .map(|name| {
                let stamped = list(name).iter().map(|r| {
                    let mut r = r.clone();
                    r["updated_at"] = json!(received_millis);
                    r
                });
                (name.to_string(), stamped.collect())
            })
            .collect(),
        category_codes: codes("pruefungskategorien"),
        event_roles: codes("veranstaltungsfunktionen"),
        class_roles: codes("pruefungsfunktionen"),
        disciplines,
    })
}

/// An official of the national list (`getOffizielle`).
pub use show::model::Official;

/// Read `getOffizielle`.
pub fn read_officials(response: &Value) -> Result<Vec<Official>, String> {
    let list = response["data"]["offizielle"].as_array().ok_or("getOffizielle: no officials in the response")?;
    Ok(list
        .iter()
        .filter_map(|o| {
            Some(Official {
                person: PersonId(number(&o["id"])?),
                first_name: text(&o["vorname"]),
                last_name: text(&o["nachname"]),
                town: text(&o["ort"]),
                canton: text(&o["kanton"]),
                roles: o["rollen"].as_array().into_iter().flatten().map(|r| text(&r["rolle_bezeichnung_de"])).collect(),
                roles_fr: o["rollen"].as_array().into_iter().flatten().map(|r| text(&r["rolle_bezeichnung_fr"])).collect(),
                roles_it: o["rollen"].as_array().into_iter().flatten().map(|r| text(&r["rolle_bezeichnung_it"])).collect(),
            })
        })
        .collect())
}

/// Read `getVeranstaltung` (a first delivery) into the changes that set the show up: the event
/// with its master data, the private data, the event's officials.
///
/// `now_millis`: when the response arrived. `new_id`: a new assignment id, as Rosson makes them
/// (`<event id>_<millis>_<random>`; see [`assignment_id`]), for officials Swiss Equestrian does
/// not know a Rosson id for.
pub fn read_event(response: &Value, master: &MasterData, officials: &[Official], now_millis: u64, new_id: &mut dyn FnMut() -> String) -> Result<Vec<Change>, String> {
    read(response, None, master, officials, now_millis, new_id)
}

/// Read an update (`getVeranstaltung` with `geaendert_nach`) into changes to the show `state`.
///
/// As Rosson (`checkForUpdates`), an entry the show has is taken only if Swiss Equestrian
/// changed it after its last delivery; its rider and horses are written on it again from the
/// current records (on that entry only). Unlike Rosson, all its delivered fields are taken,
/// merged with the office's changes (`show::delivered`), and so are all fields of the published
/// classes (Rosson took the category, date, team size and driving licences, and new classes got
/// lost). Nothing is removed: an update lists what changed, not what is gone.
pub fn read_update(response: &Value, state: &State, master: &MasterData, officials: &[Official], now_millis: u64, new_id: &mut dyn FnMut() -> String) -> Result<Vec<Change>, String> {
    read(response, Some(state), master, officials, now_millis, new_id)
}

fn read(response: &Value, state: Option<&State>, master: &MasterData, officials: &[Official], now_millis: u64, new_id: &mut dyn FnMut() -> String) -> Result<Vec<Change>, String> {
    let data = &response["data"];
    let v = &data["veranstaltung"];
    let event_id = number(&v["id"]).ok_or("getVeranstaltung: no event in the response")?;
    let list = |value: &Value| value.as_array().map(Vec::as_slice).unwrap_or(&[]).to_vec();

    let personen: HashMap<u64, &Value> = data["personen"].as_array().into_iter().flatten().filter_map(|p| Some((number(&p["id"])?, p))).collect();
    let delivered_horses: Vec<(u64, &Value)> = data["pferde"].as_array().into_iter().flatten().filter_map(|p| Some((number(&p["id"])?, p))).collect();
    // Horse records: those delivered before, then this response's; and Rosson's placeholder.
    let mut pferde: HashMap<u64, Value> = state.map(|s| s.horses.values().filter(|h| !h.record.is_null()).map(|h| (h.id.0, h.record.clone())).collect()).unwrap_or_default();
    pferde.extend(delivered_horses.iter().map(|(id, p)| (*id, (*p).clone())));
    pferde.entry(PLACEHOLDER_HORSE).or_insert_with(placeholder_horse);

    let published_classes: Vec<PublishedClass> = list(&v["ausschreibung_pruefungen"]).iter().map(|a| published_class(a, master)).collect();
    let mut phases: HashMap<u64, Vec<Value>> = state.map(|s| s.published_classes.values().map(|p| (p.id.0, p.driving_phases.clone())).collect()).unwrap_or_default();
    phases.extend(published_classes.iter().map(|p| (p.id.0, p.driving_phases.clone())));

    // The rider as written on an entry: from this response's person record, else the show's.
    let rider = |id: u64| -> Option<RiderOnEntry> {
        match personen.get(&id) {
            Some(p) => Some(RiderOnEntry { first_name: text(&p["vorname"]), last_name: text(&p["nachname"]), town: text(&p["ort"]), club: text(&p["vereinsname"]) }),
            None => state?.persons.get(&PersonId(id)).map(|p| RiderOnEntry { first_name: p.first_name.clone(), last_name: p.last_name.clone(), town: p.town.clone(), club: p.club.clone() }),
        }
    };

    // Entries.
    let mut entries = Vec::new();
    let mut entry_changes = Vec::new();
    let mut notes = Vec::new();
    for n in list(&v["nennungen"]).iter().chain(list(&v["nennungen_fa"]).iter()) {
        let entry = read_entry(n, &rider, &pferde, &phases)?;
        let note = EntryNote { entry: entry.id.clone(), remarks: text(&n["bemerkungen"]), companion: text(&n["begleit_person"]) };
        match state {
            None => {
                if !note.remarks.is_empty() || !note.companion.is_empty() {
                    notes.push(note);
                }
                entries.push(entry);
            }
            Some(s) => {
                let newer = s.delivered_entries.get(&entry.id).is_none_or(|before| before.se_changed_at < entry.se_changed_at);
                if newer {
                    notes.push(note);
                    entry_changes.push(Change::EntryDelivered { entry });
                }
            }
        }
    }

    // People and horses delivered.
    let mut persons = Vec::new();
    let mut contacts = Vec::new();
    for (pid, p) in &personen {
        let (person, contact) = person_of(*pid, p);
        persons.push(person);
        contacts.push(contact);
    }
    persons.sort_by_key(|p| p.id.clone());
    contacts.sort_by_key(|c| c.person.clone());
    let mut horses: Vec<Horse> = delivered_horses
        .iter()
        .filter(|(id, _)| *id != PLACEHOLDER_HORSE)
        .map(|(id, h)| horse_of(*id, h))
        .collect();
    horses.sort_by_key(|h| h.id.clone());

    // The event and its teams; what the office set on them stays.
    let previous = state.and_then(|s| s.event.as_ref());
    let event = Event {
        id: EventId(event_id),
        number: text(&v["nr"]),
        title: ["erste_titelzeile", "zweite_titelzeile", "dritte_titelzeile"].iter().map(|k| text(&v[*k])).filter(|s| !s.is_empty()).collect(),
        place: text(&v["ort"]),
        from: text(&v["von"]),
        to: text(&v["bis"]),
        fixed_start_numbers: previous.is_some_and(|e| e.fixed_start_numbers),
        start_numbers_assigned: previous.is_some_and(|e| e.start_numbers_assigned),
        organiser: text(&v["veranstalter"]),
        postcode: text(&v["postleitzahl"]),
        disciplines: match &v["disziplin_code"] {
            Value::Array(codes) => codes.iter().map(text).collect(),
            Value::Null => Vec::new(),
            code => vec![text(code)],
        },
        se_last_change: response["letzte_aenderung"].as_str().map(str::to_string).or_else(|| previous.and_then(|e| e.se_last_change.clone())),
        record: {
            // The event as delivered, without its lists (kept as classes, entries, teams).
            let mut r = show::public::event(v);
            if let Some(map) = r.as_object_mut() {
                for list in ["ausschreibung_pruefungen", "nennungen", "nennungen_fa", "equipen"] {
                    map.remove(list);
                }
            }
            r
        },
    };
    let teams = list(&v["equipen"])
        .iter()
        .map(|e| {
            let id = TeamId(id_text(&e["id"]));
            let start_number = state.and_then(|s| s.teams.get(&id)).and_then(|t| t.start_number);
            Team { id, published: PublishedClassId(number(&e["ausschreibung_pruefung_id"]).unwrap_or_default()), name: text(&e["name"]), start_number }
        })
        .collect();

    // The event's officials (zuweisungErstellen), if their person is known; an assignment
    // keeps its id (Swiss Equestrian's id finds it).
    let national: HashSet<&PersonId> = officials.iter().map(|o| &o.person).collect();
    let mut assignments = Vec::new();
    for b in list(&v["beteiligungen"]) {
        let Some(person) = js_parse_int(&b["person_id"]).map(|p| PersonId(p as u64)) else { continue };
        if !personen.contains_key(&person.0) && !state.is_some_and(|s| s.persons.contains_key(&person)) {
            continue;
        }
        let se_id = number(&b["id"]);
        let existing = state.and_then(|s| s.assignments.values().find(|a| a.se_id.is_some() && a.se_id == se_id));
        let role_id = js_parse_int(&b["veranstaltungsfunktion_id"]).map(|r| r as u32);
        let mut assignment = Assignment {
            id: match existing {
                Some(a) => a.id.clone(),
                None => AssignmentId(Some(id_text(&b["rosson_id"])).filter(|s| !s.is_empty()).unwrap_or_else(&mut *new_id)),
            },
            class: None,
            role: role_id.and_then(|r| master.event_roles.get(&(r as u64)).cloned()).unwrap_or_default(),
            letter: None,
            role_id,
            event: Some(EventId(event_id)),
            se_id,
            on_event: Some(true),
            responsible: Some(truthy(&b["verantwortlicher"])),
            readonly: Some(!national.contains(&person)),
            days: js_parse_int(&b["anzahl_einsatztage"]).filter(|d| *d != 0).map(|d| d as u32),
            updated_at: Some(now_millis),
            looked_up: false,
            person,
        };
        if let Some(a) = existing {
            // Unchanged: nothing to do (a change would also ask Swiss Equestrian for its id again).
            if (Assignment { updated_at: a.updated_at, looked_up: a.looked_up, ..assignment.clone() }) == *a {
                continue;
            }
            assignment.looked_up = false;
        }
        assignments.push(Change::AssignmentSet { assignment });
    }

    let mut changes = vec![
        Change::EventImported {
            event,
            published_classes,
            persons,
            horses,
            entries,
            teams,
            scoring_tables: master.scoring_tables.clone(),
            programmes: master.programmes.clone(),
            reasons: master.reasons.clone(),
            categories: master.categories.clone(),
            master: master.records.clone(),
        },
        Change::PrivateDataDelivered { contacts, notes },
        Change::RolesDelivered { roles: master.roles.clone() },
        Change::OfficialsDelivered { officials: officials.to_vec() },
    ];
    changes.extend(entry_changes);
    changes.extend(assignments);
    Ok(changes)
}

/// A published class (`ausschreibung_pruefungen`).
fn published_class(a: &Value, master: &MasterData) -> PublishedClass {
    PublishedClass {
        id: PublishedClassId(number(&a["id"]).unwrap_or_default()),
        number: text(&a["nummer"]),
        discipline: number(&a["disziplin_id"]).and_then(|d| master.disciplines.get(&d).cloned()).unwrap_or_default(),
        category: number(&a["kategorie_id"]).and_then(|k| master.category_codes.get(&k).cloned()).unwrap_or_default(),
        scoring_table: number(&a["wertung_id"]).map(|w| w as u32),
        team_size: number(&a["equipe_anzahl_nennungen"]).map(|n| n as u32).filter(|n| *n > 0),
        discipline_id: number(&a["disziplin_id"]).unwrap_or_default() as u32,
        category_id: number(&a["kategorie_id"]).map(|k| k as u32),
        category_text: text(&a["kategorie_text"]),
        levels: number(&a["springen_anzahl_stufen"]).unwrap_or_default() as u32,
        date: text(&a["datum"]).get(..10).unwrap_or_default().to_string(),
        scoring_text: a["wertung_text"].as_str().map(str::to_string),
        description: text(&a["bemerkungen"]),
        driving_phases: a["fahren_zwischenpruefungen"].as_array().cloned().unwrap_or_default(),
        fee: decimal(&a["nenngeld1"]),
        record: a.clone(),
    }
}

/// An entry (`nennungAnreichern`): withdrawn, refused and wrong entries do not start; the
/// rider and the horses are written on it.
fn read_entry(n: &Value, rider: &dyn Fn(u64) -> Option<RiderOnEntry>, pferde: &HashMap<u64, Value>, phases: &HashMap<u64, Vec<Value>>) -> Result<Entry, String> {
    let id = number(&n["id"]).ok_or("getVeranstaltung: an entry without id")?;
    let rider_id = number(&n["reiter_id"]).ok_or_else(|| format!("getVeranstaltung: entry {id} without rider"))?;
    let published = number(&n["ausschreibung_pruefung_id"]).unwrap_or_default();
    let horses_on_entry = horses_on_entry(n, pferde, phases.get(&published).map(Vec::as_slice).unwrap_or(&[]));
    let first = horses_on_entry.first();
    let status = text(&n["status"]);
    Ok(Entry {
        id: EntryId(id),
        published: PublishedClassId(published),
        rider: PersonId(rider_id),
        horse: first.and_then(|h| number(&h["pferd_id"])).or_else(|| number(&n["pferd_id"]).filter(|h| *h > 0)).map(HorseId),
        guest_horse: if first.is_some_and(|h| number(&h["pferd_id"]) == Some(PLACEHOLDER_HORSE)) {
            first.map(|h| text(&h["pferdename"]))
        } else {
            Some(text(&n["gastpferd_name"])).filter(|s| !s.is_empty())
        },
        status: show::delivered::status_of(&status),
        hors_concours: n["hors_concours"] == true,
        team: Some(id_text(&n["equipe_id"])).filter(|s| !s.is_empty()).map(TeamId),
        programme: number(&n["programm_id"]).map(|p| p as u32),
        stage: if truthy(&n["stufe"]) { js_parse_int(&n["stufe"]) } else { n["stufe"].as_i64() },
        rider_on_entry: rider(rider_id),
        horses_on_entry,
        waiting_list: status == "zurueckgewiesen",
        fee_paid: decimal(&n["nenngeld_bezahlt"]),
        organiser_place: false,
        se_status: status,
        entered_by: number(&n["nenner_id"]).map(PersonId),
        other_horses: ["pferd2_id", "pferd3_id", "pferd4_id", "pferd5_id", "pferd6_id"].iter().filter_map(|f| number(&n[*f]).filter(|h| *h > 0)).map(HorseId).collect(),
        winning_points: WinningPoints { rider: decimal(&n["gwp_reiter"]), horse: decimal(&n["gwp_pferd"]), pair: decimal(&n["gwp_paar"]) },
        se_changed_at: n["geaendert_am"].as_str().map(str::to_string),
        // The office's own: not in Swiss Equestrian's data.
        rider_changed: false,
        comment: String::new(),
        record: show::public::entry(n),
    })
}

/// An assignment id as Rosson makes them (`HelperService.createId`): the event id, the time in
/// milliseconds and a random number in base 36. Swiss Equestrian keeps it (`rosson_id`).
pub fn assignment_id(event: u64, millis: u64, random: u64) -> String {
    let mut digits = Vec::new();
    let mut n = random;
    while n > 0 || digits.is_empty() {
        digits.push(std::char::from_digit((n % 36) as u32, 36).unwrap_or('0'));
        n /= 36;
    }
    format!("{event}_{millis}_{}", digits.iter().rev().collect::<String>())
}

/// The horses written on an entry (`pferdeAnreichern`): for each of `pferd_id` … `pferd6_id`
/// that Swiss Equestrian delivered a horse record for, the horse's public data and the class's
/// driving phases (`einsaetze`). An entry with a guest horse's name gets the guest horse in
/// every place (Rosson's behaviour; entries have one horse outside driving).
pub fn horses_on_entry(n: &Value, pferde: &HashMap<u64, Value>, phases: &[Value]) -> Vec<Value> {
    let guest = Some(text(&n["gastpferd_name"])).filter(|s| !s.is_empty());
    let mut out = Vec::new();
    for field in ["pferd_id", "pferd2_id", "pferd3_id", "pferd4_id", "pferd5_id", "pferd6_id"] {
        let Some(horse_id) = number(&n[field]).filter(|_| truthy(&n[field])) else { continue };
        let Some(pferd) = pferde.get(&horse_id) else { continue };
        out.push(match &guest {
            Some(name) => json!({
                "pferdename": name, "gastpferd": true, "pferd_signalement": "", "pferd_besitzer": "", "pferd_mitbesitzer": "",
                "pferd_zuechter": "", "hat_pony_startbewilligung": false, "pony_groessenkategorie": "", "vater_name": "",
                "mutter_name": "", "vater_der_mutter_name": "", "pferd_id": PLACEHOLDER_HORSE, "einsaetze": phases.to_vec(),
            }),
            None => {
                let mut h = serde_json::Map::new();
                // Fields the record does not have are left out, as Rosson's copy leaves them.
                for (to, from) in [
                    ("pferdename", "name"),
                    ("pferd_signalement", "signalement_label"),
                    ("pferd_besitzer", "besitzer"),
                    ("pferd_mitbesitzer", "mitbesitzer"),
                    ("pferd_zuechter", "zuechter"),
                    ("hat_pony_startbewilligung", "hat_pony_startbewilligung"),
                    ("pony_groessenkategorie", "pony_groessenkategorie"),
                    ("vater_name", "vater_name"),
                    ("mutter_name", "mutter_name"),
                    ("vater_der_mutter_name", "vater_der_mutter_name"),
                ] {
                    if let Some(value) = pferd.get(from) {
                        h.insert(to.into(), value.clone());
                    }
                }
                h.insert("pferd_id".into(), json!(horse_id));
                h.insert("einsaetze".into(), Value::Array(phases.to_vec()));
                Value::Object(h)
            }
        });
    }
    out
}

/// Rosson's placeholder horse, as it adds it when the response lacks it.
pub fn placeholder_horse() -> Value {
    json!({
        "besitzer": "PseudoReiter, Bern 22", "beziehungen": [{"person_id": 5, "code": "EIG"}], "geaendert_am": "2017-01-03T19:09:50+01:00",
        "geboren_am": "2001-01-01", "hat_pony_startbewilligung": false, "id": PLACEHOLDER_HORSE, "mutter_name": null, "name": "PSEUDOPFERD",
        "pony": false, "pony_groessenkategorie": null, "signalement_label": "Z,schF,16,LETT", "ueln": "756012000000005",
        "vater_der_mutter_name": null, "vater_name": null, "zuchtnummer": null
    })
}

// Reading values as Swiss Equestrian sends them (numbers sometimes as text).

/// Text; numbers as written; null → empty.
fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// An id as text (number or string), else empty.
fn id_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// A whole number (an id) as Swiss Equestrian writes it: a number or text.
pub fn number(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

fn decimal(v: &Value) -> Option<Decimal> {
    match v {
        Value::Number(n) => scoring::number::parse(&n.to_string()).ok().flatten(),
        Value::String(s) => scoring::number::parse(s).ok().flatten(),
        _ => None,
    }
}

/// JavaScript's truthiness, which Rosson's checks rely on.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// JavaScript's `parseInt`: the leading whole number of a text ("0.0" → 0, "2.5" → 2).
fn js_parse_int(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f.trunc() as i64)),
        Value::String(s) => {
            let s = s.trim_start();
            let (sign, digits) = match s.strip_prefix('-') {
                Some(rest) => (-1, rest),
                None => (1, s.strip_prefix('+').unwrap_or(s)),
            };
            let end = digits.find(|c: char| !c.is_ascii_digit()).unwrap_or(digits.len());
            digits[..end].parse::<i64>().ok().map(|n| sign * n)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_int_like_javascript() {
        assert_eq!(js_parse_int(&json!("0.0")), Some(0));
        assert_eq!(js_parse_int(&json!("2.5")), Some(2));
        assert_eq!(js_parse_int(&json!(" 12abc")), Some(12));
        assert_eq!(js_parse_int(&json!("abc")), None);
        assert_eq!(js_parse_int(&json!(3.9)), Some(3));
        assert_eq!(js_parse_int(&Value::Null), None);
    }

    #[test]
    fn assignment_ids_look_like_rossons() {
        assert_eq!(assignment_id(59010, 1788372108923, 36 * 36 + 35), "59010_1788372108923_10z");
    }

    #[test]
    fn a_guest_horse_on_an_entry() {
        let pferde: HashMap<u64, Value> = [(PLACEHOLDER_HORSE, placeholder_horse())].into();
        let n = json!({ "pferd_id": 5, "gastpferd_name": "Fleur" });
        let horses = horses_on_entry(&n, &pferde, &[]);
        assert_eq!(horses.len(), 1);
        assert_eq!(horses[0]["pferdename"], "Fleur");
        assert_eq!(horses[0]["gastpferd"], true);
        // Without a name, the placeholder's own.
        let horses = horses_on_entry(&json!({ "pferd_id": 5 }), &pferde, &[]);
        assert_eq!(horses[0]["pferdename"], "PSEUDOPFERD");
    }

    #[test]
    fn entries_of_both_lists_are_read() {
        let response = json!({
            "data": {
                "veranstaltung": {
                    "id": 1, "nr": "2027.1", "ausschreibung_pruefungen": [], "equipen": [], "beteiligungen": [],
                    "nennungen": [{ "id": 10, "reiter_id": 7, "status": "definitiv" }],
                    "nennungen_fa": [{ "id": 11, "reiter_id": 7, "status": "abgemeldet" }]
                },
                "personen": [{ "id": 7, "vorname": "Anna", "nachname": "Muster", "ort": "Bern" }],
                "pferde": []
            },
            "letzte_aenderung": "2027-04-01 10:00:00"
        });
        let changes = read_event(&response, &MasterData::default(), &[], 0, &mut || "x".into()).unwrap();
        let Change::EventImported { entries, event, .. } = &changes[0] else { panic!() };
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].status, EntryStatus::NotStarting);
        assert_eq!(entries[0].rider_on_entry.as_ref().map(|r| r.town.as_str()), Some("Bern"));
        assert_eq!(event.se_last_change.as_deref(), Some("2027-04-01 10:00:00"));
    }
}
