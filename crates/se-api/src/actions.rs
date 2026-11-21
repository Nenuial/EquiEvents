//! What the office does with Swiss Equestrian, each as Rosson does it: the requests in order,
//! and what the answers change in the show.
//!
//! | Action | Requests |
//! |---|---|
//! | load the event, or its updates | `getOffizielle`, `getStammdaten`, `getVeranstaltung` |
//! | upload a class's results | `putResultate`, `putVeranstaltung`, a lookup per official not looked up yet |
//! | upload a published class's start lists | `putStartlisten`, `putVeranstaltung` |
//! | withdraw them | `deleteStartlisten` |
//! | upload the event (closing it) | `putVeranstaltung` |
//!
//! An action stops when its main request fails, and nothing is recorded. Once it succeeded, a
//! failed follow-up (the event, a lookup) is reported and the others are still sent. Rosson never
//! reported a failed event upload, and stopped at the first failed lookup (docs/deviations.md E42).
//! Swiss Equestrian's answers become operations: the upload itself (`Change::Uploaded`: when, by
//! whom), the ids it gives officials (`Change::AssignmentLookedUp`), the event it delivers.

use std::future::Future;
use std::time::Duration;

use serde_json::Value;
use show::model::*;
use show::oplog::Change;
use show::State;

use crate::http::{run, ApiError, HttpRequest, HttpResponse, Limits};
use crate::startlists::Language;
use crate::{event, load, Keys, Request};

/// What a request is for in its action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// The action itself: recorded when it succeeds.
    Main(Upload),
    /// The event sent after it (`putVeranstaltung`).
    Event,
    /// Swiss Equestrian's id for an assignment (`lookupZuweisung`).
    Lookup(AssignmentId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub request: Request,
    pub role: Role,
}

/// Uploading a class's results: the results, the event, then a lookup for each of the event's
/// assignments not looked up yet (decision 5; Rosson: every official, every time).
pub fn results(state: &State, class: &ClassId, keys: &Keys) -> Result<Vec<Step>, String> {
    let mut steps = vec![
        // Eventing: a phase's results go as partial results; the class made of phases, overall.
        Step {
            request: if state.classes.get(class).is_some_and(|c| c.phase.is_some()) { crate::results::put_zwischenresultate(state, class, keys)? } else { crate::results::put_resultate(state, class, keys)? },
            role: Role::Main(Upload::Results(class.clone())),
        },
        Step { request: event::put_veranstaltung(state, keys), role: Role::Event },
    ];
    for a in state.assignments.values().filter(|a| a.event.is_some() && a.for_swiss_equestrian()) {
        if let Some(request) = event::lookup_zuweisung(a, keys) {
            steps.push(Step { request, role: Role::Lookup(a.id.clone()) });
        }
    }
    Ok(steps)
}

/// Uploading a published class's start lists, then the event.
pub fn start_lists(state: &State, published: &PublishedClassId, keys: &Keys, language: Language) -> Result<Vec<Step>, String> {
    Ok(vec![
        Step { request: crate::startlists::put_startlisten(state, published, keys, language)?, role: Role::Main(Upload::StartLists(published.clone())) },
        Step { request: event::put_veranstaltung(state, keys), role: Role::Event },
    ])
}

/// Withdrawing a published class's start lists.
pub fn withdraw_start_lists(published: &PublishedClassId, keys: &Keys) -> Vec<Step> {
    vec![Step { request: event::delete_startlisten(published, keys), role: Role::Main(Upload::StartListsWithdrawn(published.clone())) }]
}

/// Uploading a ranking over several classes (a final): its results alone, as Rosson.
pub fn combined(state: &State, id: &str, keys: &Keys) -> Result<Vec<Step>, String> {
    Ok(vec![Step { request: crate::results::put_final(state, id, keys)?, role: Role::Main(Upload::Combined(id.to_string())) }])
}

/// Uploading the event (when it is closed).
pub fn event(state: &State, keys: &Keys) -> Vec<Step> {
    vec![Step { request: event::put_veranstaltung(state, keys), role: Role::Main(Upload::Event) }]
}

/// What an action did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outcome {
    /// To append to the show's log.
    pub changes: Vec<Change>,
    /// The main request failed: nothing was done.
    pub failed: Option<ApiError>,
    /// Follow-ups that failed, for the office.
    pub warnings: Vec<String>,
}

/// Send an action's requests (see the module's rules). `send` does one HTTP request, `wait`
/// waits (see [`crate::http::run`]).
pub async fn perform<S, SF, W, WF>(steps: Vec<Step>, base: &str, limits: Limits, mut send: S, mut wait: W) -> Outcome
where
    S: FnMut(HttpRequest) -> SF,
    SF: Future<Output = Result<HttpResponse, String>>,
    W: FnMut(Duration) -> WF,
    WF: Future<Output = ()>,
{
    let mut outcome = Outcome::default();
    for step in steps {
        let answer = run(base, &step.request, limits, &mut send, &mut wait).await;
        match (step.role, answer) {
            (Role::Main(upload), Ok(_)) => outcome.changes.push(Change::Uploaded { upload }),
            (Role::Main(_), Err(e)) => {
                outcome.failed = Some(e);
                return outcome;
            }
            (Role::Event, Ok(_)) => {}
            (Role::Event, Err(e)) => outcome.warnings.push(format!("The event's officials and entry counts were not updated at Swiss Equestrian. {e}")),
            (Role::Lookup(assignment), Ok(answer)) => outcome.changes.push(looked_up(assignment, &answer)),
            (Role::Lookup(assignment), Err(e)) => outcome.warnings.push(format!("Swiss Equestrian's id for assignment {assignment} was not asked; it will be next time. {e}")),
        }
    }
    outcome
}

/// A lookup's answer: `{"id": …}`, or nothing when Swiss Equestrian does not know the assignment.
pub fn looked_up(assignment: AssignmentId, answer: &Value) -> Change {
    let id = answer["id"].as_u64().or_else(|| answer["id"].as_str().and_then(|s| s.parse().ok())).filter(|id| *id > 0);
    Change::AssignmentLookedUp { assignment, se_id: id }
}

/// Load the event (`state`: `None`) or its updates (the show so far): the national list of
/// officials, the master data, then the event, all three or nothing. `now_millis` and `new_id`
/// as in [`load::read_event`].
#[allow(clippy::too_many_arguments)]
pub async fn fetch<S, SF, W, WF>(state: Option<&State>, keys: &Keys, now_millis: u64, new_id: &mut dyn FnMut() -> String, base: &str, limits: Limits, mut send: S, mut wait: W) -> Result<Vec<Change>, ApiError>
where
    S: FnMut(HttpRequest) -> SF,
    SF: Future<Output = Result<HttpResponse, String>>,
    W: FnMut(Duration) -> WF,
    WF: Future<Output = ()>,
{
    let unreadable = |what: &str, e: String| ApiError::BadAnswer(format!("{what}: {e}"));
    let officials = run(base, &load::get_offizielle(keys), limits, &mut send, &mut wait).await?;
    let officials = load::read_officials(&officials).map_err(|e| unreadable("officials", e))?;
    let master = run(base, &load::get_stammdaten(keys), limits, &mut send, &mut wait).await?;
    let master = load::read_master_data(&master, now_millis).map_err(|e| unreadable("master data", e))?;
    let since = state.and_then(|s| s.event.as_ref()).and_then(|e| e.se_last_change.clone());
    let response = run(base, &load::get_veranstaltung(keys, since.as_deref()), limits, &mut send, &mut wait).await?;
    match state {
        None => load::read_event(&response, &master, &officials, now_millis, new_id),
        Some(state) => load::read_update(&response, state, &master, &officials, now_millis, new_id),
    }
    .map_err(|e| unreadable("event", e))
}
