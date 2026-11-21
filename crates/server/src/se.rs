//! Swiss Equestrian from equievents.ch: only the server talks to it (docs/architecture/server.md).
//!
//! - **Downloads only** (the user, 2026-11-21: real events may be downloaded now): checking an
//!   event's key before anyone owns its show, the event itself (its show made from Swiss
//!   Equestrian's answers), its updates, the riders and horses (per account), Swiss Equestrian's
//!   demo events (for the apps' training shows). GETs only, of the loading and registry
//!   endpoints, with one access key (`se_api::http::download_key`): anything else is refused
//!   before it leaves. To `SE_SERVER`: `production` (the default) or `stage`; `none`
//!   (development, tests): no Swiss Equestrian; the app says which event a key opens,
//!   unchecked, and nothing is downloaded.
//! - **Nothing is ever sent to Swiss Equestrian** (the user, 2026-11-21: "equievents.ch must
//!   NEVER send anything to SE"): this server has no code that uploads. The apps' « Envoyer »
//!   (start lists, results, the event) goes to the simulation (`se_api::simulation`, kept per
//!   show, shown in the apps), which answers as Swiss Equestrian would and sends nothing on.
//! - Swiss Equestrian may take minutes to prepare an event (it answers 202, asked again every
//!   4 s), and Nginx Proxy Manager waits a minute at most for an answer: what needs Swiss
//!   Equestrian runs as a [`Jobs`] job, and the app asks how it goes (`GET /api/jobs/{id}`).
//! - What comes from Swiss Equestrian goes into the show's log as operations of the server
//!   (device `equievents.ch`), by the person who asked.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use se_api::actions::{Outcome, Step};
use se_api::http::{ApiError, HttpRequest, HttpResponse, Limits};
use se_api::simulation::{Simulation, SIMULATION};
use se_api::Keys;
use serde_json::{json, Value};
use show::oplog::{Change, Clock, Operation, Stamp};
use show::State;

use crate::auth::random_hex;

/// The server's operations are stamped by this device.
pub const DEVICE: &str = "equievents.ch";

/// One request to Swiss Equestrian (blocking: run off the async threads).
pub type Transport = Arc<dyn Fn(&HttpRequest) -> Result<HttpResponse, String> + Send + Sync>;

/// What may be downloaded from Swiss Equestrian: an event (and its master data and officials),
/// the riders and horses.
pub const ENDPOINTS: [&str; 5] = ["getStammdaten", "getOffizielle", "getVeranstaltung", "getPersonen", "getPferde"];

#[derive(Clone)]
pub struct Se {
    /// Swiss Equestrian's address; `None`: no Swiss Equestrian (development, tests).
    pub base: Option<String>,
    transport: Transport,
    /// What the simulation received, per show (the apps' « Envoyer »).
    pub simulation: Arc<Simulation>,
    pub limits: Limits,
    /// The latest Rosson version (`rosson_version`, `app_version`): read from Rosson's update
    /// feed once a day, as Rosson's own update check does (`se_api::version`).
    read_feed: bool,
    version: Arc<Mutex<Option<(Instant, String)>>>,
    /// The server's clock: its operations never share a stamp.
    clock: Arc<Mutex<Clock>>,
    pub jobs: Arc<Jobs>,
}

impl Se {
    /// No Swiss Equestrian: keys are not checked, nothing is downloaded.
    pub fn none() -> Se {
        Se::with(None, Arc::new(|_: &HttpRequest| Err("no Swiss Equestrian on this server".to_string())), Limits::default())
    }

    /// Swiss Equestrian at `base` (`None`: none), each request through `transport` (tests: a
    /// stand-in); the Rosson version is the fallback.
    pub fn with(base: Option<&str>, transport: Transport, limits: Limits) -> Se {
        Se {
            base: base.map(str::to_string),
            transport,
            simulation: Arc::default(),
            limits,
            read_feed: false,
            version: Arc::default(),
            clock: Arc::new(Mutex::new(Clock::new(DEVICE))),
            jobs: Arc::default(),
        }
    }

    /// From the environment: `SE_SERVER` = `production` (when not set), `stage` or `none`. A
    /// setting asking to send (`SE_SEND` other than `simulation`) stops the server: this version
    /// never sends to Swiss Equestrian.
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Result<Se, String> {
        let value = |name: &str| get(name).map(|v| v.trim().to_lowercase()).filter(|v| !v.is_empty());
        if let Some(send) = value("SE_SEND").filter(|v| v != "simulation") {
            return Err(format!("SE_SEND={send}: this version of equievents.ch never sends to Swiss Equestrian (only the simulation answers)"));
        }
        let base = match value("SE_SERVER").as_deref() {
            None | Some("production") => se_api::PRODUCTION,
            Some("stage") => se_api::STAGE,
            Some("none") => return Ok(Se::none()),
            Some(other) => return Err(format!("SE_SERVER={other}: production, stage or none")),
        };
        Ok(Se { read_feed: true, ..Se::with(Some(base), downloads_only(base), Limits::default()) })
    }

    /// For the log: which Swiss Equestrian.
    pub fn name(&self) -> &'static str {
        match self.base.as_deref() {
            Some(se_api::PRODUCTION) => "production",
            Some(se_api::STAGE) => "stage",
            Some(_) => "a stand-in",
            None => "none",
        }
    }

    /// Where the apps' « Envoyer » goes: the simulation, always (this version never sends to
    /// Swiss Equestrian).
    pub fn sends_to(&self) -> &'static str {
        "simulation"
    }

    /// An action's requests for a show (`se_api::actions`), answered by the simulation: nothing
    /// leaves the server.
    pub async fn perform(&self, show: &str, steps: Vec<Step>, keys: &Keys) -> Outcome {
        let (simulation, show, today) = (self.simulation.clone(), show.to_string(), keys.rosson_key.clone());
        let send = move |http: HttpRequest| std::future::ready(Ok(simulation.answer(&show, &http, &today, now_millis(), &|_| None)));
        se_api::actions::perform(steps, SIMULATION, self.limits, send, |_| std::future::ready(())).await
    }

    /// Swiss Equestrian's riders and horses with a valid status (`getPersonen`, `getPferde`,
    /// Rosson's `alle_personen` and `pferde`), with an event's key, as Rosson downloads them.
    pub async fn registry(&self, keys: &Keys) -> Result<(Vec<Value>, Vec<Value>), ApiError> {
        let unreadable = |what: &str, e: String| ApiError::BadAnswer(format!("{what}: {e}"));
        let persons = self.get(&se_api::load::get_personen(keys)).await?;
        let persons = se_api::load::read_registry(&persons, "personen").map_err(|e| unreadable("riders", e))?;
        let horses = self.get(&se_api::load::get_pferde(keys)).await?;
        let horses = se_api::load::read_registry(&horses, "pferde").map_err(|e| unreadable("horses", e))?;
        Ok((persons, horses))
    }

    /// One of Swiss Equestrian's demo events (`driving`: its driving one), as its three answers,
    /// for an app's training show (made on the computer, never on the server).
    pub async fn demo(&self, driving: bool) -> Result<Value, ApiError> {
        let keys = self.keys(se_api::DEMO_KEYS[usize::from(driving)]).await;
        let officials = self.get(&se_api::load::get_offizielle(&keys)).await?;
        let master = self.get(&se_api::load::get_stammdaten(&keys)).await?;
        let event = self.get(&se_api::load::get_veranstaltung(&keys, None)).await?;
        Ok(json!({ "getStammdaten": master, "getOffizielle": officials, "getVeranstaltung": event }))
    }

    /// What Swiss Equestrian changed since the show's last delivery (Rosson's update), as
    /// operations of the server by `author`: the officials and the master data again, then the
    /// event with `geaendert_nach` (`se_api::load::read_update`).
    pub async fn update(&self, state: &State, keys: &Keys, author: &str) -> Result<Vec<Operation>, ApiError> {
        let unreadable = |what: &str, e: String| ApiError::BadAnswer(format!("{what}: {e}"));
        let officials = self.get(&se_api::load::get_offizielle(keys)).await?;
        let officials = se_api::load::read_officials(&officials).map_err(|e| unreadable("officials", e))?;
        let master = self.get(&se_api::load::get_stammdaten(keys)).await?;
        let now = now_millis();
        let master = se_api::load::read_master_data(&master, now).map_err(|e| unreadable("master data", e))?;
        let since = state.event.as_ref().and_then(|e| e.se_last_change.clone());
        let event = self.get(&se_api::load::get_veranstaltung(keys, since.as_deref())).await?;
        let event_id = state.event.as_ref().map(|e| e.id.0).unwrap_or(0);
        let changes = se_api::load::read_update(&event, state, &master, &officials, now, &mut || se_api::load::assignment_id(event_id, now, random_id())).map_err(|e| unreadable("event", e))?;
        Ok(self.operations(changes, author, state.last.as_ref()))
    }

    /// A GET to Swiss Equestrian, asked again while it prepares the answer.
    pub async fn get(&self, request: &se_api::Request) -> Result<Value, ApiError> {
        let base = self.base.clone().ok_or_else(|| ApiError::Unreachable("no Swiss Equestrian on this server (SE_SERVER=none)".into()))?;
        let transport = self.transport.clone();
        se_api::http::run(
            &base,
            request,
            self.limits,
            move |http| {
                let transport = transport.clone();
                async move { tokio::task::spawn_blocking(move || transport(&http)).await.unwrap_or_else(|e| Err(e.to_string())) }
            },
            tokio::time::sleep,
        )
        .await
    }

    /// The keys of a request with an event's access key: today's rosson_key (Swiss time: the
    /// server's `TZ`), the latest Rosson version.
    pub async fn keys(&self, access_key: &str) -> Keys {
        use chrono::Datelike;
        let today = chrono::Local::now().date_naive();
        Keys { access_key: access_key.trim().to_string(), rosson_key: se_api::rosson_key(today.year() as u32, today.month(), today.day()), rosson_version: self.rosson_version().await }
    }

    async fn rosson_version(&self) -> String {
        let kept = self.version.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some((when, version)) = &kept {
            if !self.read_feed || when.elapsed() < Duration::from_secs(24 * 3600) {
                return version.clone();
            }
        }
        let seen = kept.map(|(_, v)| v);
        let now = if self.read_feed {
            tokio::task::spawn_blocking(|| {
                let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build();
                agent.get(se_api::version::FEED_WINDOWS).call().ok().and_then(|r| r.into_string().ok()).and_then(|y| se_api::version::from_feed(&y))
            })
            .await
            .ok()
            .flatten()
        } else {
            None
        };
        let version = se_api::version::current(now.as_deref(), seen.as_deref());
        *self.version.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), version.clone()));
        version
    }

    /// Changes as operations of the server, by `author`, stamped after `after` (the show's
    /// latest) and after every operation the server made before.
    pub fn operations(&self, changes: Vec<Change>, author: &str, after: Option<&Stamp>) -> Vec<Operation> {
        let mut clock = self.clock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(stamp) = after {
            clock.observe(stamp);
        }
        let now = now_millis();
        changes
            .into_iter()
            .map(|change| {
                let stamp = clock.tick(now);
                Operation { id: format!("{DEVICE}:{}:{}", stamp.millis, stamp.counter), stamp, author: author.into(), change }
            })
            .collect()
    }

    /// The event Swiss Equestrian delivered (`getVeranstaltung`'s answer) as a new show's
    /// operations: the national list of officials and the master data are asked first, as
    /// Rosson does, and read with it.
    pub async fn new_show(&self, keys: &Keys, event: &Value, author: &str) -> Result<Vec<Operation>, ApiError> {
        let unreadable = |what: &str, e: String| ApiError::BadAnswer(format!("{what}: {e}"));
        let officials = self.get(&se_api::load::get_offizielle(keys)).await?;
        let officials = se_api::load::read_officials(&officials).map_err(|e| unreadable("officials", e))?;
        let master = self.get(&se_api::load::get_stammdaten(keys)).await?;
        let now = now_millis();
        let master = se_api::load::read_master_data(&master, now).map_err(|e| unreadable("master data", e))?;
        let event_id = se_api::load::number(&event["data"]["veranstaltung"]["id"]).unwrap_or(0);
        let changes = se_api::load::read_event(event, &master, &officials, now, &mut || se_api::load::assignment_id(event_id, now, random_id())).map_err(|e| unreadable("event", e))?;
        Ok(self.operations(changes, author, None))
    }
}

/// Swiss Equestrian at `base`, for downloads only: anything but a GET of one of [`ENDPOINTS`]
/// with one access key is refused here, whatever asks.
fn downloads_only(base: &'static str) -> Transport {
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(10)).timeout(Duration::from_secs(180)).build();
    Arc::new(move |request: &HttpRequest| {
        if se_api::http::download_key(request, base, &ENDPOINTS).is_none() {
            return Err("only an event's download goes to Swiss Equestrian".into());
        }
        let mut call = agent.get(&request.url);
        for (name, value) in &request.headers {
            call = call.set(name, value);
        }
        let response = match call.call() {
            Ok(r) | Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(e.to_string()),
        };
        let (status, status_text) = (response.status(), response.status_text().to_string());
        let mut body = Vec::new();
        std::io::Read::read_to_end(&mut response.into_reader(), &mut body).map_err(|e| e.to_string())?;
        Ok(HttpResponse { status, status_text, body })
    })
}

/// A random number for Rosson's assignment ids (not a secret).
fn random_id() -> u64 {
    use rand::RngCore;
    rand::rngs::OsRng.next_u64() % 36u64.pow(11)
}

pub fn now_millis() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Swiss Equestrian refused the key (it does not know it, or not for this): not a failure of
/// Swiss Equestrian.
pub fn key_refused(e: &ApiError) -> bool {
    matches!(e, ApiError::Refused { status: 401 | 403 | 404, .. })
}

/// Work that waits for Swiss Equestrian, in the background: the app asks how it goes. Kept until
/// its answer is read, an hour at most, in memory (a restart forgets them: the app asks again).
#[derive(Default)]
pub struct Jobs {
    jobs: Mutex<HashMap<String, Job>>,
}

struct Job {
    user: i64,
    started: Instant,
    /// The answer, as the request would have given it: HTTP status and body.
    outcome: Option<(u16, Value)>,
}

/// How long a job's answer is kept.
const KEPT: Duration = Duration::from_secs(3600);

impl Jobs {
    /// Start `work` for `user`; its id.
    pub fn start<F>(self: &Arc<Self>, user: i64, work: F) -> String
    where
        F: Future<Output = (u16, Value)> + Send + 'static,
    {
        let id = random_hex(12);
        {
            let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
            jobs.retain(|_, j| j.started.elapsed() < KEPT);
            jobs.insert(id.clone(), Job { user, started: Instant::now(), outcome: None });
        }
        let (jobs, key) = (self.clone(), id.clone());
        tokio::spawn(async move {
            // A failure inside is an answer too (never a job working forever).
            let outcome = tokio::spawn(work).await.unwrap_or_else(|e| {
                eprintln!("error: job {key}: {e}");
                (500, json!({ "error": "the server could not do it" }))
            });
            if let Some(job) = jobs.jobs.lock().unwrap_or_else(|e| e.into_inner()).get_mut(&key) {
                job.outcome = Some(outcome);
            }
        });
        id
    }

    /// A job of `user`: `None` if there is none (or not theirs); `Some(None)` while it works. A
    /// finished job's answer is given once (it can be large: a demo event), then forgotten.
    pub fn outcome(&self, user: i64, id: &str) -> Option<Option<(u16, Value)>> {
        let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
        let job = jobs.get(id).filter(|j| j.user == user)?;
        if job.outcome.is_none() {
            return Some(None);
        }
        jobs.remove(id).map(|j| j.outcome)
    }
}
