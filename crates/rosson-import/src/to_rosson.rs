//! Write a show in Rosson's format: one event as Rosson exports it (`<nr>_export_<date>.txt`),
//! for a show office's own copy that also opens in Rosson 3.3.15 — the reverse of
//! [`crate::ops::to_operations`], which reads it back.
//!
//! Each store starts from Swiss Equestrian's record as the show kept it (`record`, the master
//! data) and gets the show's state on top: the classes as run, start lists and numbers, results
//! and judges' sheets, officials, teams, prizes, what the office changed on entries. A show
//! imported before records were kept gets records made from what it knows.
//!
//! What it carries: the show as it is, not its history; the event's access key when the office
//! has it (as Rosson's own export: Rosson needs it to work with Swiss Equestrian, and the key is
//! not a secret, P. Burkhard 2026-11-13). Contacts (addresses, phones, emails, birth dates) and
//! remarks only when `private` is asked.
//! Not written yet: rankings over several classes (Rosson's Finalprüfungen) and judges' sheets
//! put aside.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde_json::{json, Map, Value};
use show::model::*;
use show::{ScoringInput, State};

/// The Rosson version written in the file's first line (what EquiEvents follows).
pub const VERSION: &str = "3.3.15";

/// Rosson's placeholder horse for guest horses.
const PLACEHOLDER_HORSE: u64 = 5;

/// The file: `Rosson: <version>`, then the stores as JSON, URL-encoded as Rosson writes them.
pub fn file(stores: &Value) -> Vec<u8> {
    format!("Rosson: {VERSION}\n{}", encode_uri_component(&stores.to_string())).into_bytes()
}

/// The file's name as Rosson names an event's export.
pub fn file_name(state: &State, date: &str) -> String {
    let nr = state.event.as_ref().map_or("show", |e| e.number.as_str());
    format!("{nr}_export_{}.txt", date.replace('-', ""))
}

/// JavaScript's `encodeURIComponent`.
/// An official's roles as Swiss Equestrian delivers them: the German title, and the French and
/// Italian when the show has them.
fn rollen(o: &Official) -> Vec<Value> {
    o.roles
        .iter()
        .enumerate()
        .map(|(i, de)| {
            let mut r = json!({ "rolle_bezeichnung_de": de });
            for (key, said) in [("rolle_bezeichnung_fr", &o.roles_fr), ("rolle_bezeichnung_it", &o.roles_it)] {
                if let Some(s) = said.get(i) {
                    r[key] = json!(s);
                }
            }
            r
        })
        .collect()
}

fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// An id as Rosson keeps it: a number when it is one.
fn id_value(id: &str) -> Value {
    id.parse::<u64>().map(Value::from).unwrap_or_else(|_| Value::String(id.to_string()))
}

fn decimal_text(d: &Decimal) -> String {
    d.normalize().to_string()
}

/// A number as JavaScript writes it: whole numbers without decimals.
fn decimal_number(d: &Decimal) -> Value {
    let d = d.normalize();
    if d.scale() == 0 {
        if let Ok(n) = i64::try_from(d) {
            return Value::from(n);
        }
    }
    d.to_string().parse::<f64>().map(Value::from).unwrap_or(Value::Null)
}

fn opt_decimal(d: &Option<Decimal>) -> Value {
    d.as_ref().map_or(Value::Null, decimal_number)
}

/// A date as Rosson keeps it: the day at midnight UTC.
fn rosson_date(date: &str) -> String {
    format!("{}T00:00:00.000Z", date.get(..10).unwrap_or(date))
}

/// A time of day as Rosson keeps it: on 1 January 1970, Swiss winter time, in UTC.
fn rosson_time(t: TimeOfDay) -> String {
    let utc = (i32::from(t.0) - 60).rem_euclid(24 * 60);
    format!("1970-01-01T{:02}:{:02}:00.000Z", utc / 60, utc % 60)
}

fn object(record: &Value) -> Map<String, Value> {
    record.as_object().cloned().unwrap_or_default()
}

/// The event's stores, ready for [`file`].
pub fn event_stores(state: &State, private: bool, access_key: Option<&str>) -> Result<Value, String> {
    let event = state.event.as_ref().ok_or("the show has no event")?;
    let event_id = event.id.0;
    let mut out = Map::new();
    out.insert("veranstaltungen".into(), Value::Array(vec![veranstaltung(state, event, private, access_key)]));

    // Start numbers by entry (Rosson keeps one per entry).
    let mut numbers: BTreeMap<EntryId, u32> = BTreeMap::new();
    for starts in state.starts.values() {
        for s in starts {
            if let Some(n) = s.start_number {
                numbers.entry(s.entry.clone()).or_insert(n);
            }
        }
    }
    let nennungen: BTreeMap<EntryId, Value> = state.entries.values().map(|e| (e.id.clone(), nennung(state, e, numbers.get(&e.id).copied(), event_id, private))).collect();

    out.insert("ausschreibung_pruefungen".into(), Value::Array(state.published_classes.values().map(|p| ausschreibung(state, p, event_id)).collect()));
    let team_record = |t: &Team, order: Option<usize>| {
        let mut r = json!({
            "id": id_value(&t.id.0),
            "name": t.name,
            "veranstaltung_id": event_id,
            "ausschreibung_pruefung_id": t.published.0,
            "nennungen": nennungen.values().filter(|n| n["equipe_id"] == id_value(&t.id.0)).cloned().collect::<Vec<_>>(),
        });
        if let Some(n) = t.start_number {
            r["startnummer"] = json!(n);
        }
        if let Some(o) = order {
            r["reihenfolge"] = json!(o + 1);
        }
        r
    };
    out.insert("equipen".into(), Value::Array(state.teams.values().map(|t| team_record(t, None)).collect()));

    let mut pruefungen = Vec::new();
    // Eventing's phases: Rosson's Zwischenprüfungen.
    let mut zwischen = Vec::new();
    let mut starter = Vec::new();
    let mut resultate = Vec::new();
    // Rosson has no presentation phase (S44): left out of its file.
    for class in state.classes.values().filter(|c| c.phase.as_ref().is_none_or(|p| p.kind != PhaseKind::Presentation)) {
        let teams: Vec<Value> = class.teams.iter().enumerate().filter_map(|(i, t)| state.teams.get(t).map(|t| team_record(t, Some(i)))).collect();
        let record = pruefung(state, class, teams.clone(), event_id);
        match &class.phase {
            Some(phase) => zwischen.push(phase_record(state, class, phase, record)),
            None => pruefungen.push(record),
        }
        // A phase's starters and results name the class made of it, and the phase.
        let (pruefung_id, phase_id) = rosson_ids(class);
        // As the show keeps them (in team classes the same place recurs: one per team).
        let starts = state.starts.get(&class.id).cloned().unwrap_or_default();
        for s in &starts {
            let mut r = json!({
                "id": format!("{event_id}_s_{}_{}", class.id.0, s.entry.0),
                "reihenfolge": s.order,
                "nennung_id": s.entry.0,
                "pruefung_id": pruefung_id,
                "ausschreibung_pruefung_id": class.published.0,
                "veranstaltung_id": event_id,
            });
            if let Some(team) = state.entries.get(&s.entry).and_then(|e| e.team.as_ref()) {
                r["equipe_id"] = id_value(&team.0);
            }
            if let Some(o) = s.team_order {
                r["equipe_reihenfolge"] = json!(o);
            }
            r["startnummer"] = json!(s.start_number);
            if let Some(phase) = &phase_id {
                r["zwischenpruefung_id"] = json!(phase);
            }
            starter.push(r);
        }
        resultate.extend(results(state, class, &starts, &nennungen, event_id, &teams));
        if !class.phases.is_empty() {
            resultate.extend(overall_rows(state, class, event_id));
        }
    }
    out.insert("pruefungen".into(), Value::Array(pruefungen));
    out.insert("starter".into(), Value::Array(starter));
    out.insert("nennungen".into(), Value::Array(nennungen.into_values().collect()));
    out.insert("resultate".into(), Value::Array(resultate));
    out.insert("offizielle_zuweisungen".into(), Value::Array(state.assignments.values().filter(|a| a.for_swiss_equestrian()).map(|a| zuweisung(state, a, event_id)).collect()));
    out.insert("zwischenpruefungen".into(), Value::Array(zwischen));
    // Legacy (Rosson 2 moved it into resultate).
    out.insert("zwischenresultate".into(), Value::Array(Vec::new()));
    out.insert("finalpruefungen".into(), Value::Array(state.combined.values().map(|r| final_record(state, r, event_id)).collect()));
    for (store, records) in master_stores(state) {
        out.insert(store, Value::Array(records));
    }
    out.insert("localstore".into(), json!({}));
    Ok(Value::Object(out))
}

fn veranstaltung(state: &State, e: &Event, private: bool, access_key: Option<&str>) -> Value {
    let mut r = object(&e.record);
    r.insert("id".into(), json!(e.id.0.to_string()));
    r.insert("nr".into(), json!(e.number));
    for (i, key) in ["erste_titelzeile", "zweite_titelzeile", "dritte_titelzeile"].iter().enumerate() {
        r.insert(key.to_string(), json!(e.title.get(i).cloned().unwrap_or_default()));
    }
    r.insert("ort".into(), json!(e.place));
    r.insert("von".into(), json!(e.from));
    r.insert("bis".into(), json!(e.to));
    r.insert("veranstalter".into(), json!(e.organiser));
    r.insert("postleitzahl".into(), json!(e.postcode));
    if !r.contains_key("disziplin_code") {
        r.insert("disziplin_code".into(), json!(e.disciplines));
    }
    r.insert("letzte_aenderung".into(), json!(e.se_last_change));
    r.insert("fixe_startnummern".into(), json!(e.fixed_start_numbers));
    r.insert("startnummern_schon_vergeben".into(), json!(e.start_numbers_assigned));
    // The event's officials: as delivered, or made from the people with a role here.
    let mut people: Vec<Value> = r.get("offizielle_personen").and_then(Value::as_array).cloned().unwrap_or_else(|| {
        let mut ids: Vec<&PersonId> = state.assignments.values().filter(|a| a.for_swiss_equestrian()).map(|a| &a.person).collect();
        ids.sort();
        ids.dedup();
        ids.into_iter()
            .filter_map(|id| state.persons.get(id))
            .map(|p| json!({ "id": p.id.0, "vorname": p.first_name, "nachname": p.last_name, "ort": p.town, "land": p.nationality, "vereinsname": p.club, "lizenzen": p.licences, "nachwuchsreiter": p.youth }))
            .collect()
    });
    if private {
        for p in people.iter_mut() {
            if let Some(c) = p["id"].as_u64().and_then(|id| state.contacts.get(&PersonId(id))) {
                for (k, v) in [
                    ("anrede", &c.salutation),
                    ("strasse", &c.street),
                    ("postleitzahl", &c.postcode),
                    ("email", &c.email),
                    ("telefon_privat", &c.phone_private),
                    ("telefon_mobil", &c.phone_mobile),
                    ("sprache", &c.language),
                ] {
                    p[k] = json!(v);
                }
                p["geboren_am"] = json!(c.birth_date);
            }
        }
    }
    r.insert("offizielle_personen".into(), Value::Array(people));
    match access_key {
        Some(key) => r.insert("access_key".into(), json!(key)),
        None => r.remove("access_key"),
    };
    Value::Object(r)
}

fn ausschreibung(state: &State, p: &PublishedClass, event_id: u64) -> Value {
    let mut r = object(&p.record);
    r.insert("id".into(), json!(p.id.0));
    r.insert("veranstaltung_id".into(), json!(event_id));
    r.insert("nummer".into(), json!(p.number));
    r.insert("disziplin_id".into(), json!(p.discipline_id));
    r.insert("disziplin_code".into(), json!(p.discipline));
    r.insert("kategorie_id".into(), json!(p.category_id));
    r.insert("kategorie_code".into(), json!(p.category));
    r.insert("kategorie_text".into(), json!(p.category_text));
    r.insert("wertung_id".into(), json!(p.scoring_table));
    r.insert("wertung_text".into(), json!(p.scoring_text));
    r.insert("equipe_anzahl_nennungen".into(), json!(p.team_size.unwrap_or(0)));
    r.insert("springen_anzahl_stufen".into(), json!(p.levels));
    if !r.contains_key("datum") {
        r.insert("datum".into(), json!(p.date));
    }
    r.insert("bemerkungen".into(), json!(p.description));
    r.insert("fahren_zwischenpruefungen".into(), json!(p.driving_phases));
    r.insert("nenngeld1".into(), opt_decimal(&p.fee));
    // The most starters (Rosson keeps it on the published class).
    if let Some(max) = state.classes.values().filter(|c| c.published == p.id).find_map(|c| c.options.max_starters) {
        r.insert("max_startende".into(), json!(max));
    }
    // Prizes, as Rosson's prize dialog keeps them.
    if let Some(prizes) = state.prizes.get(&p.id) {
        r.insert("min_preisgeld".into(), decimal_number(&prizes.minimum));
        r.insert("preisgelder".into(), Value::Array(prizes.amounts.iter().map(decimal_number).collect()));
        r.insert("naturalpreise".into(), json!(prizes.natural));
        r.insert("anzahl_klassiert_fuer_preise".into(), json!(prizes.amounts.len().max(prizes.natural.len())));
    }
    Value::Object(r)
}

fn nennung(state: &State, e: &Entry, number: Option<u32>, event_id: u64, private: bool) -> Value {
    let mut r = object(&e.record);
    let person = state.persons.get(&e.rider);
    let rider = e.rider_on_entry.clone().or_else(|| person.map(|p| RiderOnEntry { first_name: p.first_name.clone(), last_name: p.last_name.clone(), town: p.town.clone(), club: p.club.clone() }));
    r.insert("id".into(), json!(e.id.0));
    r.insert("veranstaltung_id".into(), json!(event_id));
    r.insert("ausschreibung_pruefung_id".into(), json!(e.published.0));
    r.insert("reiter_id".into(), json!(e.rider.0));
    if let Some(rider) = &rider {
        r.insert("reiter_vorname".into(), json!(rider.first_name));
        r.insert("reiter_nachname".into(), json!(rider.last_name));
        r.insert("reiter_ort".into(), json!(rider.town));
        r.insert("vereinsname".into(), json!(rider.club));
    }
    if let Some(p) = person {
        r.insert("reiter_land".into(), json!(p.nationality));
        r.insert("lizenzen".into(), json!(p.licences));
        r.insert("nachwuchsreiter".into(), json!(p.youth));
    }
    let horse_name = e.guest_horse.clone().or_else(|| e.horse.as_ref().and_then(|h| state.horses.get(h)).map(|h| h.name.clone())).unwrap_or_default();
    if let Some(rider) = &rider {
        r.insert("sortable_name".into(), json!(format!("{} {} {}", rider.last_name, rider.first_name, horse_name).to_lowercase()));
    }
    r.insert("pferd_id".into(), json!(e.horse.as_ref().map(|h| h.0)));
    r.insert("gastpferd_name".into(), if e.horse.as_ref().map(|h| h.0) == Some(PLACEHOLDER_HORSE) { Value::Null } else { json!(e.guest_horse) });
    r.insert("pferde".into(), Value::Array(e.horses_on_entry.clone()));
    for (i, key) in ["pferd2_id", "pferd3_id", "pferd4_id", "pferd5_id", "pferd6_id"].iter().enumerate() {
        r.insert(key.to_string(), json!(e.other_horses.get(i).map(|h| h.0)));
    }
    r.insert("status".into(), json!(e.se_status));
    r.insert("kein_start".into(), json!(e.status == EntryStatus::NotStarting));
    r.insert("hors_concours".into(), json!(e.hors_concours));
    r.insert("equipe_id".into(), e.team.as_ref().map_or(Value::Null, |t| id_value(&t.0)));
    r.insert("programm_id".into(), json!(e.programme));
    r.insert("stufe".into(), json!(e.stage));
    r.insert("nenngeld_bezahlt".into(), opt_decimal(&e.fee_paid));
    r.insert("ist_vergebener_startplatz".into(), json!(e.organiser_place));
    r.insert("nenner_id".into(), json!(e.entered_by.as_ref().map(|p| p.0)));
    r.insert("gwp_reiter".into(), opt_decimal(&e.winning_points.rider));
    r.insert("gwp_pferd".into(), opt_decimal(&e.winning_points.horse));
    r.insert("gwp_paar".into(), opt_decimal(&e.winning_points.pair));
    r.insert("geaendert_am".into(), json!(e.se_changed_at));
    r.insert("reiterwechsel".into(), json!(e.rider_changed));
    let delivered_horse = state.delivered_entries.get(&e.id).map(|d| (&d.horse, &d.guest_horse));
    r.insert("pferdewechsel".into(), json!(delivered_horse.is_some_and(|(h, g)| h != &e.horse || g != &e.guest_horse)));
    r.insert("startnummer".into(), json!(number));
    if private {
        if let Some(c) = state.contacts.get(&e.rider) {
            r.insert("reiter_strasse".into(), json!(c.street));
            r.insert("reiter_plz".into(), json!(c.postcode));
            r.insert("email".into(), json!(c.email));
            r.insert("telefon_privat".into(), json!(c.phone_private));
            r.insert("telefon_mobil".into(), json!(c.phone_mobile));
            r.insert("geboren_am".into(), json!(c.birth_date));
        }
        let note = state.entry_notes.get(&e.id);
        r.insert("bemerkungen".into(), json!(note.map(|n| n.remarks.clone()).unwrap_or_default()));
        r.insert("begleit_person".into(), json!(note.map(|n| n.companion.clone()).filter(|c| !c.is_empty())));
        if !e.comment.is_empty() {
            r.insert("kommentar".into(), json!(e.comment));
        }
    }
    Value::Object(r)
}

fn pruefung(state: &State, c: &Class, teams: Vec<Value>, event_id: u64) -> Value {
    let mut r = Map::new();
    r.insert("id".into(), json!(c.id.0));
    r.insert("ausschreibung_pruefung_id".into(), json!(c.published.0));
    r.insert("veranstaltung_id".into(), json!(event_id));
    r.insert("nummer".into(), json!(c.number));
    r.insert("name".into(), json!(c.name));
    r.insert("datum".into(), json!(rosson_date(&c.date)));
    r.insert("uhrzeit".into(), json!(c.time_text));
    r.insert("wertung_id".into(), json!(c.scoring_table));
    r.insert("wertung_text".into(), json!(c.scoring_text));
    r.insert("programm_id".into(), json!(c.programme));
    if let Some(name) = c.programme.and_then(|p| state.programmes.get(&p)) {
        r.insert("programmName".into(), json!(name.name));
    }
    r.insert("position".into(), json!(c.position));
    if let Some(logo) = &c.logo {
        r.insert("logo".into(), logo.clone());
    }
    if let Some(fields) = &c.extra_fields {
        r.insert("weitere_felder".into(), Value::Array(fields.iter().map(|f| json!({ "key": f.key, "value": f.value })).collect()));
    }
    r.insert("sponsor_etiketten".into(), json!(c.sponsor));
    r.insert("startnummern_schon_vergeben".into(), json!(c.start_numbers_assigned));
    r.insert("definitiv".into(), json!(c.status == ClassStatus::Final));
    r.insert("customKlassierte".into(), json!(c.options.placed_count.is_some()));
    r.insert("manuelle_eingabe".into(), json!(c.options.totals_per_judge));
    r.insert("mannschaft".into(), json!(c.options.team_3_2));
    // The ideal time as Rosson keeps it: "mm:ss".
    if let Some(ideal) = c.options.ideal_time {
        let whole = ideal.round().to_string().parse::<u64>().unwrap_or(0);
        r.insert("idealzeit".into(), json!(format!("{:02}:{:02}", whole / 60, whole % 60)));
    }
    for flag in &c.options.not_covered {
        r.insert(flag.clone(), json!(true));
    }
    // Eventing: a class made of phases, ranked by penalties or by ranks.
    // A driving class of phases is Rosson's `fahren` (its judges' letters C B E H M, as Rosson's
    // own files have them; its phases keep Rosson's C R S P V), eventing's `kombination`.
    let driving = !c.phases.is_empty() && state.published_classes.get(&c.published).is_some_and(|p| p.discipline == "CA");
    r.insert("kombination".into(), json!((!c.phases.is_empty() && !driving) || c.options.not_covered.iter().any(|f| f == "kombination")));
    if driving {
        r.insert("fahren".into(), json!(true));
        r.insert("customBuchstaben".into(), json!("CBEHM"));
    }
    if !c.phases.is_empty() {
        r.insert("wertung_nach".into(), json!(wertung_nach(state, c)));
    }
    // The start list's layout.
    let move_up = match &c.start_list.layout {
        StartListLayout::Plain => {
            r.insert("type".into(), json!("springen"));
            false
        }
        StartListLayout::Blocks { size, headings, move_up } => {
            r.insert("type".into(), json!("block"));
            r.insert("proBlock".into(), json!(size));
            r.insert("bloecke".into(), json!(headings));
            *move_up
        }
        StartListLayout::StartTimes { first, minutes, breaks, move_up } => {
            r.insert("type".into(), json!("time"));
            r.insert("zeit".into(), json!(rosson_time(*first)));
            r.insert("programmDuration".into(), json!(minutes));
            r.insert("pausen".into(), Value::Array(breaks.iter().map(|(a, b)| json!({ "start": rosson_time(*a), "end": rosson_time(*b) })).collect()));
            *move_up
        }
    };
    // Rosson keeps the breaks of every class, none without start times.
    r.entry("pausen").or_insert_with(|| json!([]));
    // A class of phases and its phases: start list with or without start times (Rosson's
    // driving start lists read it).
    if !c.phases.is_empty() || c.phase.is_some() {
        let timed = matches!(c.start_list.layout, StartListLayout::StartTimes { .. });
        r.insert("zwischenpruefung_typ".into(), json!(if timed { "mitStartzeit" } else { "ohneStartzeit" }));
    }
    r.insert("nachruecken".into(), json!(move_up));
    match &c.start_list.line_after {
        Some((after, text)) => {
            r.insert("platzpflege_nach".into(), json!(after));
            r.insert("platzpflege_text".into(), json!(text));
        }
        None => {
            r.insert("platzpflege_nach".into(), json!(0));
            r.insert("platzpflege_text".into(), json!(""));
        }
    }
    if let Ok(ranking) = state.ranking(&c.id) {
        r.insert("anzahlKlassiert".into(), json!(c.options.placed_count.map_or(ranking.placed_count, |n| n as usize)));
    }
    r.insert("equipen".into(), Value::Array(teams));
    r.insert("zwischenpruefungen".into(), json!({}));
    Value::Object(r)
}

/// A class's results: one record per starter with a result or judges' sheets (rank and
/// whether placed as the show ranks them), and a team's line where its rank was set by hand;
/// team dressage, a record per team with its judges' sheets (and, as Rosson's, the team).
fn results(state: &State, c: &Class, starts: &[Start], nennungen: &BTreeMap<EntryId, Value>, event_id: u64, team_records: &[Value]) -> Vec<Value> {
    let dressage = state.class_discipline(c) == "CD";
    let cross = state.class_discipline(c) == "CROSS";
    // Driving phases: their own result shapes, their names as `typ`.
    let driving_kind = c.phase.as_ref().map(|p| p.kind).filter(|k| matches!(k, PhaseKind::Marathon | PhaseKind::Cones | PhaseKind::Derby | PhaseKind::Open | PhaseKind::Multitest));
    let (pruefung_id, phase_id) = rosson_ids(c);
    let ranking = state.ranking(&c.id).ok();
    let line = |id: &str, team: bool| ranking.as_ref().and_then(|r| r.lines.iter().find(|l| l.id == id && l.team == team));
    let judges: Vec<&Assignment> = state.assignments.values().filter(|a| a.class.as_ref() == Some(&c.id) && a.for_swiss_equestrian()).collect();
    // The programme's penalty tables, for the judges' ticked penalties.
    let tables: Vec<Vec<Value>> = c
        .programme
        .and_then(|p| state.programmes.get(&p))
        .and_then(|p| serde_json::from_str::<Value>(&p.definition).ok())
        .map(|d| d["tabellen"].as_array().into_iter().flatten().filter(|t| t["layout"] == "strafpunkte").map(|t| t["felder"].as_array().cloned().unwrap_or_default()).collect())
        .unwrap_or_default();

    let mut out = Vec::new();
    for s in starts {
        let card = state.results.get(&(c.id.clone(), s.entry.clone()));
        let sheets: Vec<(&Assignment, &JudgeSheet)> = judges.iter().filter_map(|j| state.sheets.get(&(c.id.clone(), s.entry.clone(), j.id.clone())).map(|sheet| (*j, sheet))).collect();
        let manual = state.manual_ranks.get(&(c.id.clone(), Competitor::Entry(s.entry.clone())));
        if card.is_none() && sheets.is_empty() && manual.is_none() {
            continue;
        }
        let mut details = Map::new();
        details.insert("aufgabe".into(), json!(state.out_reason(&c.id, &s.entry).unwrap_or(0)));
        if let (Some(kind), Some(card)) = (driving_kind, card) {
            write_driving_values(&mut details, kind, &card.values, &c.date);
        }
        for (field, value) in card.map(|c| &c.values).into_iter().flatten().filter(|_| driving_kind.is_none()) {
            // Cross-country times as Rosson keeps them: "mm:ss".
            let text = if cross && field.starts_with("zeit") { minutes_seconds(value) } else { decimal_text(value) };
            details.insert(field.clone(), json!(text));
        }
        let penalties = write_sheets(&sheets, &tables, &mut details);
        let mut r = json!({
            "id": format!("{event_id}_r_{}_{}", c.id.0, s.entry.0),
            "typ": match driving_kind {
                Some(PhaseKind::Marathon) => "gelaende",
                Some(PhaseKind::Cones) => "hindernis",
                Some(PhaseKind::Derby) => "derby",
                Some(PhaseKind::Open) => "offene",
                Some(_) => "multitest",
                None if dressage => "dressur",
                None if cross => "cross",
                None => "springen",
            },
            "pruefung_id": pruefung_id,
            "starter_id": format!("{event_id}_s_{}_{}", c.id.0, s.entry.0),
            "veranstaltung_id": event_id,
            "details": details,
            "stufe": state.entries.get(&s.entry).and_then(|e| e.stage),
            "equipe_id": state.entries.get(&s.entry).and_then(|e| e.team.as_ref()).map_or(Value::Null, |t| id_value(&t.0)),
            "nennung": nennungen.get(&s.entry).cloned().unwrap_or(Value::Null),
        });
        if dressage {
            r["strafpunkte"] = Value::Object(penalties);
        }
        if let Some(phase) = &phase_id {
            r["zwischenpruefung_id"] = json!(phase);
        }
        if let Some(l) = line(&s.entry.0.to_string(), false) {
            r["rang"] = json!(rosson_rank(l.placement));
            r["klassiert"] = json!(l.placed);
        }
        if let Some(m) = manual {
            r["customRang"] = json!(m.rank);
        }
        out.push(r);
    }
    // Team dressage: the team's record holds its sheets.
    let mut written: Vec<TeamId> = Vec::new();
    if dressage {
        for (team, _) in state.class_teams(&c.id) {
            let sheets: Vec<(&Assignment, &JudgeSheet)> = judges.iter().filter_map(|j| state.team_sheets.get(&(c.id.clone(), team.clone(), j.id.clone())).map(|sheet| (*j, sheet))).collect();
            let card = state.team_results.get(&(c.id.clone(), team.clone()));
            let manual = state.manual_ranks.get(&(c.id.clone(), Competitor::Team(team.clone())));
            if sheets.is_empty() && card.is_none() && manual.is_none() {
                continue;
            }
            let mut details = Map::new();
            details.insert("aufgabe".into(), json!(card.and_then(|c| c.out).unwrap_or(0)));
            let penalties = write_sheets(&sheets, &tables, &mut details);
            let l = line(&team.0, true);
            let mut r = json!({
                "id": format!("{event_id}_t_{}_{}", c.id.0, team.0),
                "typ": "dressur",
                "pruefung_id": c.id.0,
                "equipe_id": id_value(&team.0),
                "veranstaltung_id": event_id,
                "details": details,
                "strafpunkte": penalties,
                "total": l.map(|l| Value::Object(l.judge_totals.iter().map(|(j, t)| (j.clone(), decimal_number(t))).collect())).unwrap_or_else(|| json!({})),
                "gesamttotal": l.and_then(|l| l.percentage).map_or(Value::Null, |p| json!(format!("{p:.2}"))),
                "anzahlResultate": sheets.len(),
                "equipe": team_records.iter().find(|t| t["id"] == id_value(&team.0)).cloned().unwrap_or(Value::Null),
            });
            if let Some(l) = l {
                r["rang"] = json!(rosson_rank(l.placement));
                r["klassiert"] = json!(l.placed);
            }
            if let Some(m) = manual {
                r["customRang"] = json!(m.rank);
            }
            out.push(r);
            written.push(team);
        }
    }
    // Teams: computed by Rosson, except a rank set by hand.
    for ((class, who), m) in state.manual_ranks.range((c.id.clone(), Competitor::Entry(EntryId(0)))..).take_while(|((k, _), _)| k == &c.id) {
        let (Competitor::Team(team), rank) = (who, m.rank) else { continue };
        if written.contains(team) {
            continue;
        }
        out.push(json!({
            "id": format!("{event_id}_t_{}_{}", class.0, team.0),
            "typ": if dressage { "dressur" } else { "springen" },
            "pruefung_id": class.0,
            "equipe_id": id_value(&team.0),
            "veranstaltung_id": event_id,
            "details": {},
            "customRang": rank,
        }));
    }
    out
}

/// How a class of phases ranks, as Rosson names it: by ranks (set by the office, or forced by an
/// open phase or a multitest) or by penalties.
fn wertung_nach(state: &State, c: &Class) -> &'static str {
    if state.overall_by_rank(c) { "rangpunkte" } else { "strafpunkte" }
}

/// The overall rows of a class of phases (`typ: 'gesamt'`), as Rosson's `prepareGesamtResultate`
/// fills them: per phase code its value (penalties or rank, by `wertung_nach`), its gap and the
/// reason for being out; the totals; the overall rank. Rosson builds them only when a result is
/// saved, so a file without them shows no overall ranking and uploads none.
fn overall_rows(state: &State, c: &Class, event_id: u64) -> Vec<Value> {
    let ScoringInput::Overall { entries } = state.scoring_input(&c.id) else { return Vec::new() };
    let by_rank = state.overall_by_rank(c);
    let ranking = state.ranking(&c.id).ok();
    // Rosson has no presentation phase (S44): its values left out of the rows.
    let codes: Vec<(&ClassId, &'static str)> = c.phases.iter().filter_map(|p| Some((p, state.classes.get(p)?.phase.as_ref()?.kind.code()))).collect();
    let mut out = Vec::new();
    for e in &entries {
        let Ok(entry) = e.id.parse::<u64>().map(EntryId) else { continue };
        let (mut aufgaben, mut rangpunkte, mut strafpunkte, mut abweichungen, mut punkte) = (Map::new(), Map::new(), Map::new(), Map::new(), Map::new());
        let (mut penalty_total, mut rank_total) = (Decimal::ZERO, Decimal::ZERO);
        let mut starter = None;
        for ((phase, code), value) in codes.iter().zip(&e.phases) {
            let Some(v) = value.filter(|_| *code != PhaseKind::Presentation.code()) else { continue };
            starter = starter.or_else(|| state.starts.get(*phase).is_some_and(|s| s.iter().any(|s| s.entry == entry)).then(|| format!("{event_id}_s_{}_{}", phase.0, entry.0)));
            aufgaben.insert((*code).into(), json!(state.out_reason(phase, &entry).unwrap_or(0)));
            let (rank, penalties) = if by_rank { (v.value, Decimal::ZERO) } else { (Decimal::ZERO, v.value) };
            rank_total += rank;
            penalty_total += penalties;
            rangpunkte.insert((*code).into(), decimal_number(&rank));
            strafpunkte.insert((*code).into(), decimal_number(&penalties));
            abweichungen.insert((*code).into(), decimal_number(&v.deviation.unwrap_or_default()));
            punkte.insert((*code).into(), json!(0));
        }
        let out_reason = aufgaben.values().find(|v| v.as_u64().is_some_and(|r| r > 0)).cloned().unwrap_or(json!(0));
        let line = ranking.as_ref().and_then(|r| r.lines.iter().find(|l| !l.team && l.id == e.id));
        let mut r = json!({
            "id": format!("{event_id}_g_{}_{}", c.id.0, entry.0),
            "pruefung_id": c.id.0,
            "starter_id": starter,
            "veranstaltung_id": event_id,
            "typ": "gesamt",
            "aufgaben": aufgaben,
            "details": { "aufgabe": out_reason },
            "rangpunkte": rangpunkte,
            "strafpunkte": strafpunkte,
            "abweichungen": abweichungen,
            "punkte": punkte,
            "strafpunkte_total": decimal_number(&scoring::number::round(penalty_total, 2)),
            "rangpunkte_total": decimal_number(&rank_total),
            "punkte_total": 0,
            "duplikat": false,
        });
        if let Some(l) = line {
            r["rang"] = json!(rosson_rank(l.placement));
            r["klassiert"] = json!(l.placed);
        }
        if let Some(m) = state.manual_ranks.get(&(c.id.clone(), Competitor::Entry(entry.clone()))) {
            r["customRang"] = json!(m.rank);
        }
        out.push(r);
    }
    out
}

/// Judges' sheets into a Rosson result: each judge's marks (or total) in `details`; the ticked
/// penalties, by judge, as the programme's penalty tables with `checked` (returned).
fn write_sheets(sheets: &[(&Assignment, &JudgeSheet)], tables: &[Vec<Value>], details: &mut Map<String, Value>) -> Map<String, Value> {
    let mut penalties = Map::new();
    for (judge, sheet) in sheets {
        // Totals entered per judge: Rosson keeps the number itself.
        let marks = match sheet.total {
            Some(total) => decimal_number(&total),
            None => Value::Object(sheet.marks.iter().map(|(m, v)| (m.clone(), decimal_number(v))).collect()),
        };
        details.insert(judge.id.0.clone(), marks);
        let mut points = sheet.penalty_points.clone();
        let mut percent = sheet.penalty_percent.clone();
        let mut lists = Map::new();
        for (i, fields) in tables.iter().enumerate() {
            let ticked: Vec<Value> = fields
                .iter()
                .map(|f| {
                    let mut f = f.clone();
                    let value = |k: &str| crate::decimal(&f[k]).filter(|d| !d.is_zero());
                    let hit = match (value("strafpunkte"), value("strafprozente")) {
                        (Some(p), _) => points.iter().position(|x| *x == p).map(|at| points.remove(at)).is_some(),
                        (None, Some(p)) => percent.iter().position(|x| *x == p).map(|at| percent.remove(at)).is_some(),
                        // The exclusion box (S40).
                        _ => sheet.excluded,
                    };
                    if hit {
                        f["checked"] = json!(true);
                    }
                    f
                })
                .collect();
            lists.insert(i.to_string(), Value::Array(ticked));
        }
        // Penalties the programme does not list (or no programme): as they are.
        if !points.is_empty() || !percent.is_empty() {
            let rest = points.iter().map(|p| json!({ "strafpunkte": decimal_number(p), "checked": true })).chain(percent.iter().map(|p| json!({ "strafprozente": decimal_number(p), "checked": true })));
            lists.insert(tables.len().to_string(), Value::Array(rest.collect()));
        }
        penalties.insert(judge.id.0.clone(), Value::Object(lists));
    }
    penalties
}

/// The ids Rosson gives a class's starters and results: its own, or for a phase the class made of
/// it and the phase.
/// A rank as Rosson stores it: 99 for riders who are out.
fn rosson_rank(p: scoring::ranking::Placement) -> u32 {
    p.rank().unwrap_or(99)
}

fn rosson_ids(c: &Class) -> (String, Option<String>) {
    match &c.phase {
        Some(p) => (p.of.0.clone(), Some(c.id.0.clone())),
        None => (c.id.0.clone(), None),
    }
}

/// A phase as Rosson keeps it (`zwischenpruefungen`): the class's record with its parent, code,
/// order, number "12_cd" and the time settings.
fn phase_record(state: &State, c: &Class, phase: &Phase, record: Value) -> Value {
    let Value::Object(mut r) = record else { return record };
    for k in ["equipen", "zwischenpruefungen", "kombination"] {
        r.remove(k);
    }
    let sorter = state.classes.get(&phase.of).and_then(|p| p.phases.iter().position(|x| x == &c.id)).unwrap_or(0);
    r.insert("parent_pruefung_id".into(), json!(phase.of.0));
    // Copies of the class's: how it ranks, and the driving judges' letters.
    if let Some(parent) = state.classes.get(&phase.of) {
        r.insert("wertung_nach".into(), json!(wertung_nach(state, parent)));
    }
    if state.driving(c) {
        r.insert("customBuchstaben".into(), json!("CRSPV"));
    }
    r.insert("code".into(), json!(phase.kind.code()));
    r.insert("zwischenpruefung".into(), json!(true));
    r.insert("sorter".into(), json!(sorter));
    r.insert("nummer".into(), json!(format!("{}_{}", c.number, phase.kind.code())));
    r.insert("idealzeit_berechnen".into(), json!(c.options.ideal_time_penalties));
    r.insert("max_zeit_berechnen".into(), json!(c.options.max_time_penalties));
    if let Some(settings) = &c.options.driving {
        write_driving_settings(&mut r, phase.kind, settings);
    }
    if let Some(max) = c.options.max_time {
        // Rosson's "mm:ss:hh".
        let hundredths = (max * Decimal::ONE_HUNDRED).round().to_string().parse::<u64>().unwrap_or(0);
        r.insert("max_zeit".into(), json!(format!("{:02}:{:02}:{:02}", hundredths / 6000, hundredths / 100 % 60, hundredths % 100)));
    }
    Value::Object(r)
}

/// Seconds as Rosson's "h:m:s" (marathon settings).
fn hms(seconds: &Decimal) -> String {
    let whole = seconds.trunc().to_string().parse::<u64>().unwrap_or(0);
    format!("{}:{}:{}", whole / 3600, whole / 60 % 60, whole % 60)
}

/// A driving phase's settings into Rosson's phase record.
fn write_driving_settings(r: &mut Map<String, Value>, kind: PhaseKind, s: &DrivingSettings) {
    match kind {
        PhaseKind::Marathon => {
            // The time limit as set, else computed (Rosson keeps it computed: `max_zeit`).
            let (limit_a, limit_b) = s.marathon_limits();
            let phase = |allowed: &Option<Decimal>, minimum: &Option<Decimal>, limit: Option<Decimal>| {
                let mut p = Map::new();
                if let Some(a) = allowed {
                    p.insert("erlaubte_zeit".into(), json!(hms(a)));
                }
                if let Some(m) = minimum {
                    p.insert("min_zeit".into(), json!(hms(m)));
                }
                if let Some(l) = limit {
                    p.insert("max_zeit".into(), json!(hms(&l)));
                }
                p
            };
            r.insert("phase_a".into(), Value::Object(phase(&s.a_allowed, &s.a_minimum, limit_a)));
            let mut b = phase(&s.b_allowed, &s.b_minimum, limit_b);
            b.insert("faktor".into(), json!(s.factor.map(|f| decimal_number(&f)).unwrap_or(json!(0.25))));
            b.insert("anzahl_hindernisse".into(), json!(s.obstacles.unwrap_or(5).to_string()));
            if let Some(t) = s.transfer {
                b.insert("ueberfuehrungszeit".into(), json!(decimal_text(&t)));
            }
            r.insert("phase_b".into(), Value::Object(b));
        }
        PhaseKind::Cones => {
            let table = match (s.cones.unwrap_or(ConesKind::A), s.rounds.unwrap_or(1)) {
                (ConesKind::A, 1) => 1,
                (ConesKind::A, _) => 2,
                (ConesKind::B, 1) => 5,
                (ConesKind::B, _) => 6,
                (ConesKind::JuniorCup, _) => 7,
            };
            r.insert("wertung_id".into(), json!(table));
            for round_ in 1..=2 {
                let prefix = format!("{round_}:");
                let mut times: Map<String, Value> = s.allowed.iter().filter_map(|(k, v)| k.strip_prefix(&prefix).map(|l| (format!("erlaubte_zeit_{l}"), decimal_number(v)))).collect();
                // The maximum as set, else twice the allowed time (what Rosson computes and keeps).
                for (k, v) in &s.allowed {
                    if let Some(l) = k.strip_prefix(&prefix) {
                        let max = s.maximum.get(k).copied().unwrap_or(*v * Decimal::TWO);
                        times.insert(format!("max_zeit_{l}"), decimal_number(&max));
                    }
                }
                for (k, v) in &s.maximum {
                    if let Some(l) = k.strip_prefix(&prefix) {
                        times.insert(format!("max_zeit_{l}"), decimal_number(v));
                    }
                }
                if !times.is_empty() {
                    r.insert(format!("umgang{round_}"), Value::Object(times));
                }
            }
            r.insert("mitInternationalerWertung".into(), json!(s.international));
            if !s.bonus.is_empty() {
                r.insert("bonus".into(), Value::Object(s.bonus.iter().map(|(g, v)| (g.clone(), decimal_number(v))).collect()));
            }
        }
        PhaseKind::Derby => {
            r.insert("zeitfaktor".into(), s.time_factor.map(|f| decimal_number(&f)).unwrap_or(json!(0.25)));
        }
        PhaseKind::Open => {
            let table = match s.open.unwrap_or(OpenKind::Time) {
                OpenKind::Time => 1,
                OpenKind::Points => 2,
                OpenKind::PenaltyPoints => 3,
                OpenKind::PointsThenTime => 4,
            };
            r.insert("wertung_id".into(), json!(table));
        }
        PhaseKind::Multitest => {
            r.insert("multitest_typ".into(), json!(s.multitest.clone().unwrap_or_else(|| "B4".into())));
        }
        _ => {}
    }
}

/// A driving phase's result as Rosson keeps its details (the reverse of the import's reading).
fn write_driving_values(details: &mut Map<String, Value>, kind: PhaseKind, values: &BTreeMap<String, Decimal>, date: &str) {
    let v = |k: &str| values.get(k);
    match kind {
        PhaseKind::Marathon => {
            for p in ["a", "b"] {
                let mut phase = Map::new();
                if let Some(t) = v(&format!("{p}_start")) {
                    phase.insert("startzeit".into(), json!(crate::swiss_time::iso_at(date, *t)));
                }
                if let Some(t) = v(&format!("{p}_finish")) {
                    phase.insert("zielzeit".into(), json!(crate::swiss_time::iso_at(date, *t)));
                }
                for (ours, theirs) in [("penalties", "punkte"), ("neutralised", "neutralisation")] {
                    if let Some(x) = v(&format!("{p}_{ours}")) {
                        phase.insert(theirs.into(), decimal_number(x));
                    }
                }
                if p == "b" {
                    for i in 1..=12 {
                        let mut o = Map::new();
                        for (ours, theirs) in [("time", "zeit"), ("penalties", "punkte"), ("neutralised", "neutralisation")] {
                            if let Some(x) = v(&format!("h{i}_{ours}")) {
                                o.insert(theirs.into(), decimal_number(x));
                            }
                        }
                        if !o.is_empty() {
                            phase.insert(format!("hindernis_{}", i - 1), Value::Object(o));
                        }
                    }
                }
                details.insert(format!("phase_{p}"), Value::Object(phase));
            }
        }
        PhaseKind::Cones => {
            for round_ in 1..=2 {
                let mut r = Map::new();
                for (ours, theirs) in [("zeit", "zeit"), ("hindernis", "hindernis_fehler"), ("andere", "andere_fehler")] {
                    if let Some(x) = v(&format!("{ours}_{round_}")) {
                        r.insert(theirs.into(), decimal_number(x));
                    }
                }
                if !r.is_empty() {
                    details.insert(format!("umgang{round_}"), Value::Object(r));
                }
            }
        }
        PhaseKind::Derby => {
            for (ours, theirs) in [("zeit", "gefahrene_zeit"), ("abwuerfe", "strafsekunden_abwuerfe"), ("uebrige", "strafsekunden_uebrige")] {
                if let Some(x) = v(ours) {
                    details.insert(theirs.into(), decimal_number(x));
                }
            }
        }
        PhaseKind::Open => {
            for k in ["sekunden", "punkte"] {
                if let Some(x) = v(k) {
                    details.insert(k.into(), decimal_number(x));
                }
            }
        }
        _ => {}
    }
}

/// Seconds as "m:ss" (cross-country times).
fn minutes_seconds(seconds: &Decimal) -> String {
    let whole = seconds.trunc().to_string().parse::<u64>().unwrap_or(0);
    let rest = seconds.fract();
    let tail = if rest.is_zero() { String::new() } else { rest.normalize().to_string().trim_start_matches('0').to_string() };
    format!("{:02}:{:02}{tail}", whole / 60, whole % 60)
}

fn zuweisung(state: &State, a: &Assignment, event_id: u64) -> Value {
    // Eventing: the judges of a dressage phase are the class's (Rosson).
    let class = a.class.as_ref().map(|c| state.classes.get(c).and_then(|k| k.phase.as_ref()).map_or_else(|| c.0.clone(), |p| p.of.0.clone()));
    json!({
        "id": a.id.0,
        "person_id": a.person.0,
        "pruefung_id": class,
        "rolle_id": a.role_id,
        "buchstabe": a.letter,
        "veranstaltung_id": a.event.as_ref().map_or(event_id, |e| e.0),
        "equis_id": a.se_id,
        "auf_veranstaltung": a.on_event,
        "verantwortlicher": a.responsible,
        "readonly": a.readonly,
        "anzahl_einsatztage": a.days,
        "updated_at": a.updated_at,
    })
}

/// Swiss Equestrian's master data: as delivered, or made from what the show knows.
fn master_stores(state: &State) -> Vec<(String, Vec<Value>)> {
    let kept = |name: &str| state.master_records.get(name).filter(|r| !r.is_empty()).cloned();
    let disciplines: BTreeMap<u32, String> = state.published_classes.values().map(|p| (p.discipline_id, p.discipline.clone())).collect();
    let discipline_id = |code: &Option<String>| code.as_ref().and_then(|c| disciplines.iter().find(|(_, d)| *d == c).map(|(id, _)| *id));
    let roles = |in_class: bool| -> Vec<Value> {
        state.roles.iter().filter(|r| r.in_class == in_class).map(|r| json!({ "id": r.id, "code": r.code, "bezeichnung_de": r.name_de, "bezeichnung_fr": r.name_fr, "bezeichnung_it": r.name_it })).collect()
    };
    vec![
        ("disziplinen".into(), kept("disziplinen").unwrap_or_else(|| disciplines.iter().map(|(id, code)| json!({ "id": id, "code": code })).collect())),
        (
            "wertungen".into(),
            kept("wertungen").unwrap_or_else(|| state.scoring_tables.values().map(|w| json!({ "id": w.id, "code": w.code, "bezeichnung_de": w.name, "bezeichnung_fr": w.name_fr, "bezeichnung_it": w.name_it, "bewertung_json": w.definition })).collect()),
        ),
        (
            "programme".into(),
            kept("programme").unwrap_or_else(|| state.programmes.values().map(|p| json!({ "id": p.id, "name": p.name, "max_punkte": decimal_number(&p.max_points), "programm_json": p.definition, "disziplin_id": p.discipline_id })).collect()),
        ),
        (
            "aufgabegruende".into(),
            kept("aufgabegruende").unwrap_or_else(|| state.reasons.values().map(|g| json!({ "id": g.id, "disziplin_id": discipline_id(&g.discipline), "bezeichnung_de": g.name })).collect()),
        ),
        ("pruefungsfunktionen".into(), kept("pruefungsfunktionen").unwrap_or_else(|| roles(true))),
        ("veranstaltungsfunktionen".into(), kept("veranstaltungsfunktionen").unwrap_or_else(|| roles(false))),
        ("pruefungskategorien".into(), kept("pruefungskategorien").unwrap_or_else(|| state.categories.values().map(|k| k.record.clone()).collect())),
        (
            "offizielle".into(),
            kept("offizielle").unwrap_or_else(|| {
                state
                    .officials
                    .values()
                    .map(|o| json!({ "id": o.person.0, "vorname": o.first_name, "nachname": o.last_name, "ort": o.town, "kanton": o.canton, "rollen": rollen(o) }))
                    .collect()
            }),
        ),
        ("veranstalter".into(), kept("veranstalter").unwrap_or_default()),
    ]
}

/// A ranking over several classes as Rosson keeps its finals (`finalpruefungen`): Rosson's own
/// record when it came from Rosson, with what the show changed. Rosson knows ranks added and
/// percentages; a ranking by penalties added (EquiEvents' U8) goes as ranks added.
fn final_record(state: &State, r: &CombinedRanking, event_id: u64) -> Value {
    let mut f = r.record.as_object().cloned().unwrap_or_default();
    // Made here: Rosson's fields filled in; from Rosson: its record as it was.
    let made_here = f.is_empty();
    let first = r.classes.first().and_then(|c| state.classes.get(c));
    let dressage = |c: &ClassId| state.classes.get(c).and_then(|k| state.published_classes.get(&k.published)).is_some_and(|p| p.discipline == "CD");
    f.insert("id".into(), r.id.parse::<u64>().map_or_else(|_| json!(r.id), |n| json!(n)));
    f.insert("titel".into(), json!(r.name));
    f.insert("nummer".into(), json!(state.combined_number(&r.id)));
    f.insert("wertung_nach".into(), json!(if matches!(r.method, CombinedMethod::Percentages { .. }) { "prozentpunkte" } else { "rangpunkte" }));
    // The classes: Rosson's list kept while they are the same classes in the same order.
    let same = f.get("teilpruefungen").and_then(Value::as_array).is_some_and(|t| {
        let mut t: Vec<&Value> = t.iter().collect();
        t.sort_by_key(|x| x["sorter"].as_u64().unwrap_or(0));
        t.iter().map(|x| x["id"].as_str().map(str::to_string).unwrap_or_else(|| x["id"].to_string())).eq(r.classes.iter().map(|c| c.0.clone()))
    });
    if !same {
        let parts = r
            .classes
            .iter()
            .enumerate()
            .map(|(i, c)| json!({ "id": c.0, "sorter": i, "nummer": state.classes.get(c).map(|k| k.number.clone()).unwrap_or_default(), "istDressur": dressage(c) }))
            .collect();
        f.insert("teilpruefungen".into(), Value::Array(parts));
    }
    if !r.extra_fields.is_empty() || !f.contains_key("weitereFelder") {
        let fields: Vec<Value> = r.extra_fields.iter().map(|x| json!({ "key": x.key, "value": x.value })).collect();
        f.insert("weitereFelder".into(), if fields.is_empty() { json!([{}]) } else { Value::Array(fields) });
    }
    if made_here {
        if let Some(c) = first {
            f.insert("ausschreibung_pruefung_id".into(), json!(c.published.0));
            f.insert("datum".into(), json!(format!("{}T00:00:00.000Z", c.date)));
            if let Some(programme) = c.programme {
                f.insert("programm_id".into(), json!(programme));
            }
        }
        f.insert("veranstaltung_id".into(), json!(event_id));
        f.insert("logo".into(), Value::Null);
    }
    Value::Object(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_times_as_rosson_keeps_them() {
        assert_eq!(rosson_date("2026-09-25"), "2026-09-25T00:00:00.000Z");
        assert_eq!(rosson_time(TimeOfDay::new(8, 30)), "1970-01-01T07:30:00.000Z");
        assert_eq!(rosson_time(TimeOfDay::new(0, 30)), "1970-01-01T23:30:00.000Z");
        assert_eq!(crate::swiss_time::time_of_day(&rosson_time(TimeOfDay::new(14, 5))), Some(TimeOfDay::new(14, 5)));
        assert_eq!(encode_uri_component("é a"), "%C3%A9%20a");
    }
}
