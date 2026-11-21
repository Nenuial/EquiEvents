//! The event: `putVeranstaltung` (officials, entry counts, waiting lists), `deleteStartlisten`,
//! `lookupZuweisung`, and the sequence of requests each upload makes.

use rust_decimal::Decimal;
use serde_json::{json, Value};
use show::model::*;
use show::State;

use crate::results::{event_officials, number};
use crate::{Keys, Method, Request};

/// `putVeranstaltung`, as Rosson sends it after every upload and when the event is closed.
pub fn put_veranstaltung(state: &State, keys: &Keys) -> Request {
    // Published classes in their number order.
    let mut published: Vec<&PublishedClass> = state.published_classes.values().collect();
    published.sort_by_key(|p| (p.number.trim().parse::<i64>().unwrap_or(i64::MAX), p.id.0));
    let counts: Vec<Value> = published
        .iter()
        .map(|p| {
            let entries: Vec<&Entry> = state.entries.values().filter(|e| e.published == p.id).collect();
            let waiting: Vec<&&Entry> = entries.iter().filter(|e| e.waiting_list).collect();
            json!({
                "id": p.id.0,
                "anzahl_nennungen": entries.len(),
                "anzahl_warteliste": waiting.len(),
                "kosten_warteliste": number(waiting.iter().filter_map(|e| e.fee_paid).sum::<Decimal>()),
                "anzahl_veranstalter_plaetze": entries.iter().filter(|e| e.organiser_place).count(),
            })
        })
        .collect();

    // Final classes with their officials, by published class.
    let mut classes: Vec<&Class> = state.classes.values().filter(|c| c.status == ClassStatus::Final).collect();
    classes.sort_by_key(|c| (c.published.0, c.position.unwrap_or(u32::MAX), c.number.clone()));
    let finals: Vec<Value> = classes
        .iter()
        .filter_map(|c| {
            let officials: Vec<Value> = state
                .assignments
                .values()
                .filter(|a| a.class.as_ref() == Some(&c.id) && a.for_swiss_equestrian())
                .map(|a| json!({ "id": a.id.0, "person_id": a.person.0, "funktion_id": a.role_id, "buchstabe": a.letter.clone().unwrap_or_default() }))
                .collect();
            (!officials.is_empty()).then(|| json!({ "ausschreibung_pruefung_id": c.published.0.to_string(), "nummer": c.number, "offizielle": officials }))
        })
        .collect();

    let mut body = keys.envelope();
    body["ausschreibung_pruefungen"] = Value::Array(counts);
    body["pruefungen"] = Value::Array(finals);
    body["veranstaltungsbeteiligungen"] = Value::Array(event_officials(state));
    body["delete_veranstaltungsbeteiligungen"] = json!([]);
    Request { method: Method::Post, path: "/putVeranstaltung".into(), query: Vec::new(), body: Some(body) }
}

/// `deleteStartlisten`: withdraw a published class's start lists.
pub fn delete_startlisten(published: &PublishedClassId, keys: &Keys) -> Request {
    Request {
        method: Method::Delete,
        path: format!("/deleteStartlisten/{}", published.0),
        query: vec![("access_key".into(), keys.access_key.clone()), ("rosson_key".into(), keys.rosson_key.clone())],
        body: None,
    }
}

/// `lookupZuweisung`: Swiss Equestrian's id for an assignment. Only for assignments without
/// one that were not looked up yet (decision 2026-11-13; Rosson asked for every official of the
/// event after every class). The answer is recorded with `Change::AssignmentLookedUp`.
pub fn lookup_zuweisung(assignment: &Assignment, keys: &Keys) -> Option<Request> {
    if assignment.se_id.is_some() || assignment.looked_up || !assignment.for_swiss_equestrian() {
        return None;
    }
    Some(Request {
        method: Method::Get,
        path: "/lookupZuweisung".into(),
        // Alphabetical, as Rosson's HTTP layer (AngularJS) orders query parameters.
        query: vec![
            ("access_key".into(), keys.access_key.clone()),
            ("funktion_id".into(), assignment.role_id.map(|r| r.to_string()).unwrap_or_default()),
            ("person_id".into(), assignment.person.0.to_string()),
        ],
        body: None,
    })
}

/// Everything uploading a class's results sends, in order (see [`crate::actions::results`]).
pub fn upload_results(state: &State, class: &ClassId, keys: &Keys) -> Result<Vec<Request>, String> {
    Ok(crate::actions::results(state, class, keys)?.into_iter().map(|s| s.request).collect())
}

/// Everything uploading a published class's start lists sends (see [`crate::actions::start_lists`]).
pub fn upload_start_lists(state: &State, published: &PublishedClassId, keys: &Keys, language: crate::startlists::Language) -> Result<Vec<Request>, String> {
    Ok(crate::actions::start_lists(state, published, keys, language)?.into_iter().map(|s| s.request).collect())
}
