//! The show office with equievents.ch (docs/architecture/server.md): the account, putting a show
//! on the server, its owner's requests and people, and the sync itself.
//!
//! - **The account**: made or signed in here (`sign_up`, `sign_in`); its token kept with the
//!   access keys (the system keychain in the desktop app), never in a file of the app.
//! - **A show on the server** (`put_online`): with its event's key, the first to enter it owns
//!   the show; the others learn who owns it and ask them (`ask_owner`) for access or for the show.
//!   The server checks the key with Swiss Equestrian first, which can take minutes: it answers
//!   with a job, asked again until done ([`Office::claim`]).
//! - **An event from Swiss Equestrian** (`open_with_key`, the user, 2026-11-21): the key alone;
//!   equievents.ch downloads the event, and the first to enter the key owns the show made from
//!   it, which then comes to this computer as any show of the server.
//! - **Swiss Equestrian through equievents.ch** (only the server talks to it): a show of the
//!   server sends from there (`send_online`: this computer's changes go up first, the server
//!   sends what the show is there, the answer comes down), and is updated from there
//!   (`update_from_se`: what Swiss Equestrian changed since the event was loaded). The server
//!   sends to the simulation only (this version never sends to Swiss Equestrian), and shows what
//!   it received (`received_online`).
//! - **The riders and horses** (`registry_via_server`): downloaded by the server with a show's
//!   key, kept there for the account, and copied here (`pull_registry`: after signing in, and
//!   with the library's sync whenever the account's copy is newer). Swiss Equestrian's demo
//!   events come from the server too (`demo_via_server`): the app never reaches Swiss Equestrian.
//!   A rehearsal goes up as a show without an event (no key: the real event stays free). A show
//!   on the server is brought to this computer by downloading it (`download_show`), never merged
//!   with a copy made here on its own (two imports of one event are two different logs).
//! - **The sync** (`sync`, `sync_all`; the app runs it in the background): the operations made
//!   here go up (they wait in the show file while offline, across restarts), the others' come
//!   down after the show's cursor. The show file keeps its link: the server, the show's id
//!   there, the cursor (`show::Store::meta`).
//!
//! - **Deleting a show of the server** (`delete_with`, the user, 2026-11-21): its owner keeps the
//!   rights (only this copy goes), gives the event to a collaborator (and leaves it), or
//!   releases it (deleted from equievents.ch for everyone: the next to enter the key downloads
//!   it fresh and owns it); a collaborator keeps their access or leaves. This copy's last
//!   changes go up first. Offline, only this copy goes.
//!
//! - **Archives** (`archive_show`): a show's responsable archives it on the server, and every
//!   member's library lists it there after its next sync (the show file keeps it, with one's
//!   role, for the library offline); a show not on the server is archived here only. The
//!   account archives its shows on its own some weeks after their last day
//!   (`account_settings`, `set_account_settings`). Archived is only how shows are listed.
//!
//! - **The online version** (the user, 2026-11-21: a show office that never works offline works
//!   in the browser or on a phone): equievents.ch runs an office for each account on its own
//!   server (`host`), its account the site's session. It works straight on the master: each
//!   change goes up at once, what others did comes down before a command (every 2 seconds at
//!   most), every show of the account is kept, and it never signs in or out by itself or talks
//!   to another server.
//!
//! Expected refusals come back as codes the app translates (`{ok: false, code}`, `outcome`), not
//! as errors: `offline`, `signed_out`, `too_many`, the server's codes (`taken`, `key`, …).

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use show::oplog::Operation;
use show::Store;

use crate::{arg, connection, now_millis, Office, REHEARSAL, SHOW_EXTENSION};

/// The server, unless the library's settings or `EQUIEVENTS_SERVER` name another (development).
pub const DEFAULT_SERVER: &str = "https://www.equievents.ch";

/// The desktop app's update check on the server (Tauri's updater fills in the braces).
pub const UPDATES: &str = "/api/updates/{{target}}/{{arch}}/{{current_version}}";
/// Operations sent at a time (the server takes up to 5000).
const PUSH_BATCH: usize = 1000;
/// A job of the server (waiting for Swiss Equestrian): asked again this often, this long at most.
const JOB_EVERY: Duration = Duration::from_secs(2);
const JOB_AT_MOST: Duration = Duration::from_secs(20 * 60);
/// The show file's own values: the server, the show's id there, how far it has pulled.
pub(crate) const META_SERVER: &str = "server";
pub(crate) const META_REMOTE: &str = "remote";
const META_CURSOR: &str = "cursor";
/// In the archives ("1"), and one's role on the server's show (`owner`, `collaborator`), as the
/// last sync said: the library lists the show by them, also offline.
pub(crate) const META_ARCHIVED: &str = "archived";
pub(crate) const META_ROLE: &str = "role";

/// The commands of this module.
pub const COMMANDS: [&str; 26] = [
    "account",
    "open_with_key",
    "import_online",
    "update_from_se",
    "set_server",
    "sign_up",
    "sign_in",
    "sign_out",
    "forgot_password",
    "online_shows",
    "put_online",
    "ask_owner",
    "withdraw_request",
    "show_requests",
    "answer_request",
    "show_people",
    "add_person",
    "remove_person",
    "download_show",
    "unlink_show",
    "sync_status",
    "sync",
    "sync_all",
    "archive_show",
    "account_settings",
    "set_account_settings",
];

/// A file to import through equievents.ch (`import_online`): an event of a Rosson export, or an
/// EquiEvents show file.
enum Import<'a> {
    Rosson { path: &'a std::path::Path, event: &'a str },
    Show { path: &'a std::path::Path },
}

/// Who is signed in on this computer (the library's settings; the token is a secret).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub server: String,
    pub id: i64,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub language: String,
}

/// How a show's last sync went (kept while the app runs).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Note {
    /// `local` (not on the server), `synced`, `offline`, `signed_out`, `no_access` (taken off
    /// the show), `clock` (this computer's clock runs ahead: some changes wait), `error`.
    pub status: String,
    /// When the server last answered, milliseconds since 1970.
    pub last_ok: Option<u64>,
    pub error: Option<String>,
    /// How far this computer's clock runs ahead of the server's (milliseconds), from a minute.
    pub clock_ahead: Option<i64>,
    /// On the server: `owner` or `collaborator`, the owner's name, requests waiting (owner).
    pub role: Option<String>,
    pub owner: Option<String>,
    pub requests: i64,
    /// What the last sync sent and received.
    pub pushed: usize,
    pub pulled: usize,
}

/// The sync's state while the app runs.
pub struct Syncing {
    agent: ureq::Agent,
    notes: Mutex<HashMap<String, Note>>,
    /// Shows being synced now (a second sync of one waits for the next round).
    busy: Mutex<HashSet<String>>,
    /// The online version's office, on equievents.ch (`Office::host`).
    hosted: std::sync::atomic::AtomicBool,
    /// When each show last came down from the server (the online version).
    pulled: Mutex<HashMap<String, std::time::Instant>>,
}

impl Default for Syncing {
    fn default() -> Self {
        Syncing {
            agent: ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(5)).timeout(Duration::from_secs(120)).build(),
            notes: Mutex::default(),
            busy: Mutex::default(),
            hosted: std::sync::atomic::AtomicBool::new(false),
            pulled: Mutex::default(),
        }
    }
}

impl Syncing {
    pub(crate) fn hosted(&self) -> bool {
        self.hosted.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// What the online version does not do: its account is the site's session, its server
/// equievents.ch itself.
const NOT_ONLINE: [&str; 6] = ["set_server", "sign_up", "sign_in", "sign_out", "forgot_password", "unlink_show"];
/// The online version reads what others did at most this often, before a command.
const PULL_EVERY: Duration = Duration::from_secs(2);

/// A show being synced; free again when dropped.
struct Busy<'a> {
    set: &'a Mutex<HashSet<String>>,
    show: String,
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        if let Ok(mut set) = self.set.lock() {
            set.remove(&self.show);
        }
    }
}

/// The server did not answer, or answered no.
#[derive(Debug)]
enum Failure {
    Offline(String),
    Status { status: u16, body: Value },
}

impl Failure {
    /// For the app: `offline`, `signed_out`, `too_many`, `no_access`, the server's code, or `error`.
    fn code(&self) -> String {
        match self {
            Failure::Offline(_) => "offline".into(),
            Failure::Status { status: 401, .. } => "signed_out".into(),
            Failure::Status { status: 429, .. } => "too_many".into(),
            Failure::Status { body, .. } if body["code"].is_string() => body["code"].as_str().unwrap_or_default().to_string(),
            Failure::Status { status: 404, .. } => "no_access".into(),
            Failure::Status { .. } => "error".into(),
        }
    }

    fn message(&self) -> String {
        match self {
            Failure::Offline(e) => e.clone(),
            Failure::Status { status, body } => body["error"].as_str().map(str::to_string).unwrap_or_else(|| format!("HTTP {status}")),
        }
    }

    /// `{ok: false, code, error}`.
    fn answer(&self) -> Value {
        json!({ "ok": false, "code": self.code(), "error": self.message() })
    }
}

/// Whose riders and horses a copy is (`registry::About::account`).
fn account_key(account: &Account) -> String {
    format!("{}#{}", account.server, account.id)
}

fn token_name(server: &str) -> String {
    format!("equievents token {server}")
}

impl Office {
    /// A command of this module.
    pub(crate) fn sync_call(&self, command: &str, args: &Value) -> Result<Value, String> {
        match command {
            "account" => Ok(self.account_view()),
            "set_server" => {
                let mut settings = self.settings();
                settings.server_url = args["url"].as_str().map(|u| u.trim().trim_end_matches('/').to_string()).filter(|u| !u.is_empty());
                self.save_settings(&settings)?;
                Ok(self.account_view())
            }
            "sign_up" => {
                let language = if args["language"] == "de" { "de" } else { "fr" };
                let body = json!({ "name": arg(args, "name")?, "email": arg(args, "email")?, "password": arg(args, "password")?, "device": self.device, "language": language });
                self.signed_in_with("/api/register", &body, language)
            }
            "sign_in" => {
                let body = json!({ "email": arg(args, "email")?, "password": arg(args, "password")?, "device": self.device });
                self.signed_in_with("/api/login", &body, "")
            }
            "sign_out" => {
                let server = self.server_url();
                if let Ok(Some(token)) = self.token() {
                    // Gone from this computer also when the server cannot be told.
                    let _ = self.http("POST", &format!("{server}/api/logout"), Some(&token), None);
                }
                self.secrets.set(&token_name(&server), None)?;
                let mut settings = self.settings();
                settings.account = None;
                self.save_settings(&settings)?;
                // The account's riders and horses (with their contact data) go with it.
                self.registry.clear()?;
                self.sync.notes.lock().map_err(|_| "busy")?.clear();
                Ok(self.account_view())
            }
            "forgot_password" => {
                let body = json!({ "email": arg(args, "email")?, "language": if args["language"] == "de" { "de" } else { "fr" } });
                Ok(match self.http("POST", &format!("{}/api/password-reset", self.server_url()), None, Some(&body)) {
                    Ok(_) => json!({ "ok": true }),
                    Err(f) => f.answer(),
                })
            }
            "online_shows" => self.online_shows(),
            "put_online" => self.put_online(arg(args, "show")?, args["access_key"].as_str()),
            "open_with_key" => self.open_with_key(arg(args, "access_key")?),
            "import_online" => {
                let path = std::path::Path::new(arg(args, "path")?);
                let file = match args["event"].as_str() {
                    Some(event) => Import::Rosson { path, event },
                    None => Import::Show { path },
                };
                self.import_online(file, args["access_key"].as_str())
            }
            "update_from_se" => self.update_from_se(arg(args, "show")?),
            "ask_owner" => {
                let kind = if args["kind"] == "ownership" { "ownership" } else { "access" };
                // For a show here, its event's key; for one not here yet (`open_with_key`), the key given.
                let key = match args["show"].as_str() {
                    Some(show) => self.event_key(show, args["access_key"].as_str())?,
                    None => args["access_key"].as_str().map(str::trim).filter(|k| !k.is_empty()).map(str::to_string),
                };
                let Some(key) = key else { return Ok(json!({ "ok": false, "code": "need_key" })) };
                self.answer(self.api("POST", &format!("/api/shows/{}/requests", arg(args, "remote")?), Some(&json!({ "kind": kind, "access_key": key }))), "request")
            }
            "withdraw_request" => self.answer(self.api("DELETE", &format!("/api/requests/{}", number(args, "id")?), None), "request"),
            "show_requests" => {
                let remote = self.remote_of(arg(args, "show")?)?.ok_or("the show is not on equievents.ch")?;
                self.answer(self.api("GET", &format!("/api/shows/{remote}/requests"), None), "requests")
            }
            "answer_request" => {
                let body = json!({ "accept": args["accept"] == true, "stay": args["stay"].as_bool() });
                let answer = self.answer(self.api("POST", &format!("/api/requests/{}", number(args, "id")?), Some(&body)), "request");
                // The owner's view of the show may have changed (a transfer).
                if let Some(show) = args["show"].as_str() {
                    let _ = self.sync(show);
                }
                answer
            }
            "show_people" => {
                let remote = self.remote_of(arg(args, "show")?)?.ok_or("the show is not on equievents.ch")?;
                self.answer(self.api("GET", &format!("/api/shows/{remote}/members"), None), "people")
            }
            "add_person" => {
                let remote = self.remote_of(arg(args, "show")?)?.ok_or("the show is not on equievents.ch")?;
                self.answer(self.api("POST", &format!("/api/shows/{remote}/members"), Some(&json!({ "email": arg(args, "email")? }))), "person")
            }
            "remove_person" => {
                let show = arg(args, "show")?;
                let remote = self.remote_of(show)?.ok_or("the show is not on equievents.ch")?;
                let answer = self.answer(self.api("DELETE", &format!("/api/shows/{remote}/members/{}", number(args, "user")?), None), "removed");
                let _ = self.sync(show);
                answer
            }
            "download_show" => self.download_show(arg(args, "remote")?),
            "unlink_show" => {
                let show = arg(args, "show")?;
                self.with(show, |_, store| {
                    for key in [META_SERVER, META_REMOTE, META_CURSOR, META_ROLE] {
                        store.set_meta(key, None).map_err(|e| e.0)?;
                    }
                    store.mark_all_unsynced().map_err(|e| e.0)
                })?;
                self.sync.notes.lock().map_err(|_| "busy")?.remove(show);
                self.sync_status(show)
            }
            "sync_status" => self.sync_status(arg(args, "show")?),
            "archive_show" => self.archive_show(args["show"].as_str(), args["remote"].as_str(), args["archived"] == true),
            "account_settings" => self.answer(self.api("GET", "/api/me", None), "account"),
            "set_account_settings" => {
                let weeks = match &args["archive_after_weeks"] {
                    Value::Null => None,
                    w => Some(w.as_i64().filter(|w| (1..=104).contains(w)).ok_or("archive after 1 to 104 weeks")?),
                };
                self.answer(self.api("POST", "/api/me", Some(&json!({ "archive_after_weeks": weeks }))), "account")
            }
            "sync" => {
                let show = arg(args, "show")?;
                self.sync(show)?;
                self.sync_status(show)
            }
            "sync_all" => {
                let mut out = serde_json::Map::new();
                // The online version keeps every show of the account.
                if self.sync.hosted() {
                    self.download_all()?;
                }
                for show in self.linked()?.into_values() {
                    out.insert(show.clone(), json!(self.sync(&show)?));
                }
                // The account's riders and horses, when the server has newer ones (quietly).
                if self.signed_in()?.is_some() {
                    let _ = self.pull_registry(false);
                }
                Ok(Value::Object(out))
            }
            other => Err(format!("unknown command {other}")),
        }
    }

    /// The server: the library's choice, `EQUIEVENTS_SERVER`, or equievents.ch.
    pub fn server_url(&self) -> String {
        self.settings()
            .server_url
            .or_else(|| std::env::var("EQUIEVENTS_SERVER").ok().filter(|s| !s.trim().is_empty()))
            .unwrap_or_else(|| DEFAULT_SERVER.to_string())
            .trim_end_matches('/')
            .to_string()
    }

    /// The account signed in on this server, with its token.
    fn signed_in(&self) -> Result<Option<(Account, String)>, String> {
        let server = self.server_url();
        let Some(account) = self.settings().account.filter(|a| a.server == server) else { return Ok(None) };
        Ok(self.token()?.map(|t| (account, t)))
    }

    fn token(&self) -> Result<Option<String>, String> {
        self.secrets.get(&token_name(&self.server_url()))
    }

    fn account_view(&self) -> Value {
        let server = self.server_url();
        let hosted = self.sync.hosted();
        match self.signed_in() {
            Ok(Some((account, _))) => json!({ "server": server, "signed_in": true, "account": account, "hosted": hosted }),
            _ => json!({ "server": server, "signed_in": false, "hosted": hosted }),
        }
    }

    /// The online version: this office works for `account` on equievents.ch (`server`: how it
    /// reaches it from there), signed in with `token`, straight on the master.
    pub fn host(&self, server: &str, account: Account, token: &str) -> Result<(), String> {
        let server = server.trim_end_matches('/').to_string();
        let mut settings = self.settings();
        settings.server_url = Some(server.clone());
        settings.account = Some(Account { server: server.clone(), ..account });
        self.save_settings(&settings)?;
        self.secrets.set(&token_name(&server), Some(token))?;
        self.sync.hosted.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// The token this office keeps for `server`, if any (the online version reuses it while valid).
    pub fn token_for(&self, server: &str) -> Option<String> {
        self.secrets.get(&token_name(server.trim_end_matches('/'))).ok().flatten()
    }

    /// A command of the online version: none that signs in or out or changes server; a show of
    /// the server comes down first (when last done more than 2 seconds ago) and what changed goes
    /// up at once.
    pub(crate) fn call_hosted(&self, command: &str, args: &Value) -> Result<Value, String> {
        if NOT_ONLINE.contains(&command) {
            return Err("not in the online version: equievents.ch's site signs you in and out".into());
        }
        let show = match args["show"].as_str() {
            Some(show) if self.remote_of(show).is_ok_and(|r| r.is_some()) => Some(show.to_string()),
            _ => None,
        };
        if let Some(show) = &show {
            let due = self.sync.pulled.lock().map_err(|_| "busy")?.get(show).is_none_or(|t| t.elapsed() >= PULL_EVERY);
            if due {
                self.sync(show)?;
                self.sync.pulled.lock().map_err(|_| "busy")?.insert(show.clone(), std::time::Instant::now());
            }
        }
        let answer = self.run(command, args);
        if let (Ok(_), Some(show)) = (&answer, &show) {
            // (Not a show just deleted.)
            if self.path(show).is_file() && self.with(show, |_, store| store.unsynced_count().map_err(|e| e.0))? > 0 {
                self.sync(show)?;
            }
        }
        answer
    }

    /// Every show of the account on the server, here (the online version).
    fn download_all(&self) -> Result<(), String> {
        let Ok(shows) = self.api("GET", "/api/shows", None) else { return Ok(()) };
        let linked = self.linked()?;
        for remote in shows.as_array().into_iter().flatten().filter_map(|s| s["id"].as_str()) {
            if !linked.contains_key(remote) {
                self.download_show(remote)?;
            }
        }
        Ok(())
    }

    /// Sign up or in: the token kept, the account remembered.
    fn signed_in_with(&self, path: &str, body: &Value, language: &str) -> Result<Value, String> {
        let server = self.server_url();
        let answer = match self.http("POST", &format!("{server}{path}"), None, Some(body)) {
            Ok(answer) => answer,
            Err(f) => return Ok(f.answer()),
        };
        let token = answer["token"].as_str().ok_or("the server gave no token")?;
        let user = &answer["user"];
        let account = Account {
            server: server.clone(),
            id: user["id"].as_i64().unwrap_or_default(),
            name: user["name"].as_str().unwrap_or_default().into(),
            email: user["email"].as_str().unwrap_or_default().into(),
            language: user["language"].as_str().unwrap_or(language).into(),
        };
        self.secrets.set(&token_name(&server), Some(token))?;
        let mut settings = self.settings();
        settings.account = Some(account.clone());
        self.save_settings(&settings)?;
        // Another account's riders and horses go; this one's come from the server.
        if self.registry.about().account.is_some_and(|a| a != account_key(&account)) {
            self.registry.clear()?;
        }
        let _ = self.pull_registry(false);
        Ok(json!({ "ok": true, "account": account }))
    }

    /// One request to the server (`url` whole).
    fn http(&self, method: &str, url: &str, token: Option<&str>, body: Option<&Value>) -> Result<Value, Failure> {
        let mut request = self.sync.agent.request(method, url).set("Accept", "application/json");
        if let Some(token) = token {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        let response = match body {
            Some(body) => request.set("Content-Type", "application/json").send_string(&body.to_string()),
            None => request.call(),
        };
        // The whole answer (a show's first pull can be large).
        let read = |r: ureq::Response| serde_json::from_reader::<_, Value>(r.into_reader()).unwrap_or(Value::Null);
        match response {
            Ok(r) => Ok(read(r)),
            Err(ureq::Error::Status(status, r)) => Err(Failure::Status { status, body: read(r) }),
            Err(e) => Err(Failure::Offline(e.to_string())),
        }
    }

    /// A request as the signed-in account (`path` below the server).
    fn api(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, Failure> {
        let token = match self.token() {
            Ok(Some(t)) if self.settings().account.is_some_and(|a| a.server == self.server_url()) => t,
            _ => return Err(Failure::Status { status: 401, body: Value::Null }),
        };
        self.http(method, &format!("{}{path}", self.server_url()), Some(&token), body)
    }

    /// `{ok: true, <name>: answer}` or `{ok: false, code, error}`.
    fn answer(&self, result: Result<Value, Failure>, name: &str) -> Result<Value, String> {
        Ok(match result {
            Ok(v) => json!({ "ok": true, name: v }),
            Err(f) => f.answer(),
        })
    }

    /// The show's id on the server, if it is linked to this server.
    fn remote_of(&self, show: &str) -> Result<Option<String>, String> {
        let server = self.server_url();
        self.with(show, |_, store| {
            let on = store.meta(META_SERVER).map_err(|e| e.0)?;
            Ok(if on.as_deref() == Some(server.as_str()) { store.meta(META_REMOTE).map_err(|e| e.0)? } else { None })
        })
    }

    /// The library's shows on this server: their id there → here.
    fn linked(&self) -> Result<HashMap<String, String>, String> {
        let mut out = HashMap::new();
        for entry in std::fs::read_dir(&self.library).map_err(|e| e.to_string())?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some(SHOW_EXTENSION) {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            if let Some(remote) = self.remote_of(id)? {
                out.insert(remote, id.to_string());
            }
        }
        Ok(out)
    }

    /// The event's key for a show: given now (and kept), or kept already.
    fn event_key(&self, show: &str, given: Option<&str>) -> Result<Option<String>, String> {
        let event = self.event_id(show)?;
        let name = connection::access_key_name(event);
        if let Some(key) = given.map(str::trim).filter(|k| !k.is_empty()) {
            self.secrets.set(&name, Some(key))?;
            return Ok(Some(key.to_string()));
        }
        self.secrets.get(&name)
    }

    /// The shows one works on at equievents.ch (and which are on this computer), and one's
    /// requests.
    fn online_shows(&self) -> Result<Value, String> {
        if self.signed_in()?.is_none() {
            return Ok(json!({ "ok": false, "code": "signed_out" }));
        }
        let shows = match self.api("GET", "/api/shows", None) {
            Ok(v) => v,
            Err(f) => return Ok(f.answer()),
        };
        let requests = self.api("GET", "/api/requests", None).unwrap_or(Value::Array(Vec::new()));
        let linked = self.linked()?;
        let shows: Vec<Value> = shows
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|mut s| {
                let local = s["id"].as_str().and_then(|id| linked.get(id)).cloned();
                s["local"] = json!(local);
                s
            })
            .collect();
        Ok(json!({ "ok": true, "shows": shows, "requests": requests }))
    }

    /// Put a show on equievents.ch: an event's show with its key (`made`: it is ours now, and
    /// goes up; `member`: we work on it already: download it rather than merge this copy;
    /// `owned`: someone else owns it, ask them; `key`: not the event's key; `need_key`), or a
    /// show without an event (a rehearsal).
    fn put_online(&self, show: &str, key: Option<&str>) -> Result<Value, String> {
        // A training show stays on this computer.
        if self.is_training(show)? {
            return Ok(json!({ "outcome": "training" }));
        }
        if self.signed_in()?.is_none() {
            return Ok(json!({ "outcome": "signed_out" }));
        }
        if let Some(remote) = self.remote_of(show)? {
            return Ok(json!({ "outcome": "linked", "remote": remote }));
        }
        let (event, rehearsal, from_export) = self.with(show, |_, store| {
            let first = store.log().map_err(|e| e.0)?.first().map(|op| op.author.clone());
            Ok((store.state().event.clone(), first.as_deref() == Some(REHEARSAL), first.as_deref() == Some(rosson_import::ops::AUTHOR)))
        })?;
        let body = match &event {
            // A rehearsal's event is real: it stays free for its show office.
            Some(e) if !rehearsal => {
                let Some(key) = self.event_key(show, key)? else { return Ok(json!({ "outcome": "need_key" })) };
                // Imported from the event's Rosson export: a past event's key, which Swiss
                // Equestrian no longer accepts, is taken on trust (whoever has the export has it).
                json!({ "access_key": key, "event_id": e.id.0, "name": e.title.first().cloned().unwrap_or_default(), "number": e.number, "from_export": from_export })
            }
            Some(e) => json!({ "name": format!("{} ({})", e.title.first().cloned().unwrap_or_default(), e.number), "number": e.number }),
            None => json!({ "name": show, "number": show }),
        };
        match self.claim(&body) {
            Ok(made) if made["created"] == true => {
                let remote = made["id"].as_str().ok_or("the server gave no show id")?.to_string();
                self.link(show, &remote)?;
                let note = self.sync(show)?;
                Ok(json!({ "outcome": "made", "remote": remote, "sync": note }))
            }
            Ok(member) => Ok(json!({ "outcome": "member", "remote": member["id"], "role": member["role"] })),
            Err(Failure::Status { status: 409, body }) if body["code"] == "claimed" => {
                Ok(json!({ "outcome": "owned", "remote": body["id"], "name": body["name"], "owner": body["owner"], "request": body["request"] }))
            }
            Err(f) => Ok(json!({ "outcome": f.code(), "error": f.message() })),
        }
    }

    /// An event's show asked for on the server (`POST /api/shows`): when the server checks the
    /// key with Swiss Equestrian, its job, asked again until done (every 2 seconds after the
    /// first moments: Swiss Equestrian may take minutes to prepare an event), at most 20 minutes.
    fn claim(&self, body: &Value) -> Result<Value, Failure> {
        self.api_job("POST", "/api/shows", Some(body))
    }

    /// A request that may answer with a job of the server (waiting for Swiss Equestrian): the
    /// job's answer, once done.
    fn api_job(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, Failure> {
        let answer = self.api(method, path, body)?;
        let Some(job) = answer["job"].as_str() else { return Ok(answer) };
        let started = std::time::Instant::now();
        for n in 0.. {
            // Quick at first (a key checked in a moment), then every 2 seconds.
            std::thread::sleep(JOB_EVERY.min(Duration::from_millis(250 << n.min(4))));
            let now = self.api("GET", &format!("/api/jobs/{job}"), None)?;
            if now["working"] != true {
                return Ok(now);
            }
            if started.elapsed() > JOB_AT_MOST {
                return Err(Failure::Status { status: 504, body: json!({ "code": "swiss_equestrian", "error": "Swiss Equestrian is still preparing the event; try again in a few minutes" }) });
            }
        }
        unreachable!()
    }

    /// An event from Swiss Equestrian, by its key: equievents.ch downloads it, the first to enter
    /// the key owns its show, which comes to this computer (`made`, or `member`: one works on it
    /// already; `show`: here). Someone else's: `owned`, to ask them (`ask_owner` with the key).
    /// The key is kept, as for a show put online.
    fn open_with_key(&self, key: &str) -> Result<Value, String> {
        let key = key.trim();
        if key.is_empty() {
            return Ok(json!({ "outcome": "need_key" }));
        }
        // Swiss Equestrian's demo events: from the settings (Exercice), never on equievents.ch.
        if connection::DEMO_KEYS.contains(&key) {
            return Ok(json!({ "outcome": "training" }));
        }
        if self.signed_in()?.is_none() {
            return Ok(json!({ "outcome": "signed_out" }));
        }
        let keep = |event: &Value| -> Result<(), String> {
            match event.as_u64() {
                Some(event) => self.secrets.set(&connection::access_key_name(event), Some(key)),
                None => Ok(()),
            }
        };
        match self.claim(&json!({ "access_key": key })) {
            Ok(show) => {
                keep(&show["event_id"])?;
                let remote = show["id"].as_str().ok_or("the server gave no show id")?;
                let outcome = if show["created"] == true { "made" } else { "member" };
                let down = self.download_show(remote)?;
                if down["ok"] != true {
                    return Ok(json!({ "outcome": down["code"], "error": down["error"] }));
                }
                Ok(json!({ "outcome": outcome, "show": down["show"], "remote": remote }))
            }
            Err(Failure::Status { status: 409, body }) if body["code"] == "claimed" => {
                keep(&body["event_id"])?;
                Ok(json!({ "outcome": "owned", "remote": body["id"], "name": body["name"], "owner": body["owner"], "request": body["request"] }))
            }
            Err(f) => Ok(json!({ "outcome": f.code(), "error": f.message() })),
        }
    }

    /// An import goes through equievents.ch first (the user, 2026-11-22: no copy of an event
    /// someone holds; imports only with equievents.ch, which checks who holds the event). The
    /// first to bring an event owns it: `made` (imported, on equievents.ch, synced). One works on
    /// it already: `member`, that show (downloaded when not here), the file not imported.
    /// Someone else holds it: `owned`, nothing imported; ask them (`ask_owner` with the key given
    /// back). No key in the file: `need_key`; `key`: refused; no connection: `offline`, etc.,
    /// nothing imported either. A show file without a real event (a rehearsal, a training show)
    /// stays on this computer: `local`.
    fn import_online(&self, file: Import, given: Option<&str>) -> Result<Value, String> {
        let given = given.map(str::trim).filter(|k| !k.is_empty());
        // The event (its id, number, title) and the key the file brings.
        let (event_id, number, title, in_file) = match &file {
            Import::Rosson { path, event } => {
                let export = crate::read_rosson(path)?;
                let found = export.events().into_iter().find(|e| e.id == *event).ok_or_else(|| format!("no event {event} in the file"))?;
                let id: u64 = event.parse().map_err(|_| format!("event {event}: not an event of Swiss Equestrian"))?;
                (id, found.number, found.title, export.access_key(event))
            }
            Import::Show { path } => {
                let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
                let (manifest, ops) = show::export::read(&bytes).map_err(|e| e.0)?;
                let first = ops.first().map(|op| op.author.as_str());
                match manifest.event {
                    Some(e) if first != Some(REHEARSAL) && !crate::training::is_training(first) => (e.id, e.number, e.title.first().cloned().unwrap_or_default(), None),
                    _ => return Ok(json!({ "outcome": "local", "show": self.import_show(path)? })),
                }
            }
        };
        let about = |mut answer: Value| {
            answer["number"] = json!(number);
            answer["title"] = json!(title);
            Ok(answer)
        };
        if self.signed_in()?.is_none() {
            return about(json!({ "outcome": "signed_out" }));
        }
        let name = connection::access_key_name(event_id);
        let Some(key) = given.map(str::to_string).or(in_file.clone()).or(self.secrets.get(&name)?) else { return about(json!({ "outcome": "need_key" })) };
        // A past event's key, from its Rosson export, is taken on trust (whoever has the export
        // has the key); a key typed or kept here is Swiss Equestrian's to check.
        let from_export = matches!(file, Import::Rosson { .. }) && given.is_none() && in_file.is_some();
        let body = json!({ "access_key": key, "event_id": event_id, "name": title, "number": number, "from_export": from_export });
        match self.claim(&body) {
            Ok(made) if made["created"] == true => {
                let remote = made["id"].as_str().ok_or("the server gave no show id")?.to_string();
                let imported = match &file {
                    Import::Rosson { path, event } => self.import_rosson(path, event, false),
                    Import::Show { path } => self.import_show(path),
                };
                let show = match imported {
                    Ok(show) => show,
                    Err(e) => {
                        // Not here after all: the event is free again for whoever brings it.
                        let _ = self.api("DELETE", &format!("/api/shows/{remote}"), None);
                        return Err(e);
                    }
                };
                self.secrets.set(&name, Some(&key))?;
                self.link(&show, &remote)?;
                let note = self.sync(&show)?;
                about(json!({ "outcome": "made", "show": show, "remote": remote, "sync": note }))
            }
            Ok(member) => {
                self.secrets.set(&name, Some(&key))?;
                let remote = member["id"].as_str().ok_or("the server gave no show id")?.to_string();
                let down = self.download_show(&remote)?;
                if down["ok"] != true {
                    return about(json!({ "outcome": down["code"], "error": down["error"] }));
                }
                about(json!({ "outcome": "member", "show": down["show"], "remote": remote, "role": member["role"] }))
            }
            Err(Failure::Status { status: 409, body }) if body["code"] == "claimed" => {
                self.secrets.set(&name, Some(&key))?;
                about(json!({ "outcome": "owned", "remote": body["id"], "owner": body["owner"], "request": body["request"], "access_key": key }))
            }
            Err(f) => about(json!({ "outcome": f.code(), "error": f.message() })),
        }
    }

    /// Where a show's sends go: through equievents.ch (its id there) for a show of the server,
    /// unless it is a rehearsal or a training show (those only ever reach the simulation, here);
    /// `None`: the simulation here.
    pub(crate) fn sends_online(&self, show: &str) -> Result<Option<String>, String> {
        let first = self.with(show, |_, store| store.first_author().map_err(|e| e.0))?;
        if first.as_deref() == Some(REHEARSAL) || crate::training::is_training(first.as_deref()) {
            return Ok(None);
        }
        self.remote_of(show)
    }

    /// Where equievents.ch sends (`simulation`, `stage`, `production`), if it answers.
    pub(crate) fn server_sends(&self) -> Option<String> {
        let about = self.http("GET", &format!("{}/api/swiss-equestrian", self.server_url()), None, None).ok()?;
        about["sends"].as_str().map(str::to_string)
    }

    /// Why a show of the server cannot send now, in words the app translates.
    fn not_now(code: &str, error: String) -> String {
        match code {
            "offline" => "equievents.ch does not answer: sending to Swiss Equestrian waits for the connection".into(),
            "signed_out" => "sign in to equievents.ch to send to Swiss Equestrian".into(),
            "no_access" => "you no longer work on this show on equievents.ch".into(),
            "clock" | "not_synced" => "this computer's clock runs ahead: some changes wait; set the computer's time, then send".into(),
            "need_key" => "the show has no event's key on equievents.ch".into(),
            _ => error,
        }
    }

    /// Send through equievents.ch: this computer's changes go up first, then the server sends
    /// what the show is there (the latest of them included) and its answer comes down.
    pub(crate) fn send_online(&self, show: &str, remote: &str, args: &Value) -> Result<Value, String> {
        let note = self.sync(show)?;
        let pending = self.with(show, |_, store| store.unsynced_count().map_err(|e| e.0))?;
        if note.status != "synced" || pending > 0 {
            let code = if note.status == "synced" { "not_synced" } else { note.status.as_str() };
            return Err(Self::not_now(code, note.error.clone().unwrap_or_default()));
        }
        let through = self.with(show, |_, store| Ok(store.log().map_err(|e| e.0)?.into_iter().max_by(|a, b| a.stamp.cmp(&b.stamp)).map(|o| o.id)))?;
        let body = json!({ "what": args["what"], "class": args["class"], "published": args["published"], "id": args["id"], "language": args["language"], "through": through });
        let answer = self.api_job("POST", &format!("/api/shows/{remote}/send"), Some(&body)).map_err(|f| Self::not_now(&f.code(), f.message()))?;
        // What it sent, and when: from the server.
        let _ = self.sync(show);
        Ok(answer)
    }

    /// What the simulation of equievents.ch received for a show of the server.
    pub(crate) fn received_online(&self, remote: &str) -> Result<Value, String> {
        self.api("GET", &format!("/api/shows/{remote}/received"), None).map_err(|f| Self::not_now(&f.code(), f.message()))
    }

    /// What Swiss Equestrian changed since the event was loaded (Rosson's update), through
    /// equievents.ch: `{added}` (how many changes), then the show syncs.
    fn update_from_se(&self, show: &str) -> Result<Value, String> {
        let Some(remote) = self.sends_online(show)? else {
            return Ok(json!({ "ok": false, "code": "not_online" }));
        };
        let answer = match self.api_job("POST", &format!("/api/shows/{remote}/update"), None) {
            Ok(answer) => answer,
            Err(f) => return Ok(f.answer()),
        };
        let note = self.sync(show)?;
        Ok(json!({ "ok": true, "added": answer["added"], "sync": note }))
    }

    /// Why a download through equievents.ch failed, in words the app translates.
    fn not_downloaded(f: &Failure) -> String {
        match f.code().as_str() {
            "offline" => "equievents.ch does not answer; try again once the connection is back".into(),
            "signed_out" => "sign in to equievents.ch first".into(),
            "no_access" => "you no longer work on this show on equievents.ch".into(),
            "key" => "Swiss Equestrian does not know the show's key: it only accepts the key of an event that is not over yet".into(),
            "need_key" => "the show has no event's key on equievents.ch".into(),
            "too_many" => "too many downloads from Swiss Equestrian for this account; try again in an hour".into(),
            _ => f.message(),
        }
    }

    /// Swiss Equestrian's riders and horses, through equievents.ch: downloaded there with the
    /// show's key and kept for the account, then the account's copy here.
    pub(crate) fn registry_via_server(&self, show: &str) -> Result<Value, String> {
        let remote = self.remote_of(show)?.ok_or("put the show on equievents.ch first: the riders and horses are downloaded there, with its key")?;
        self.api_job("POST", &format!("/api/shows/{remote}/registry"), None).map_err(|f| Self::not_downloaded(&f))?;
        self.pull_registry(true)
    }

    /// The account's riders and horses from equievents.ch, kept here (`always`: even when this
    /// computer's copy is as recent); this computer's copy, as it is now.
    pub(crate) fn pull_registry(&self, always: bool) -> Result<Value, String> {
        let Some((account, _)) = self.signed_in()? else { return Err("sign in to equievents.ch first".into()) };
        let whose = account_key(&account);
        let here = self.registry.about();
        if !always {
            let there = self.api("GET", "/api/registry/about", None).map_err(|f| Self::not_downloaded(&f))?;
            let Some(at) = there["at"].as_u64() else { return serde_json::to_value(&here).map_err(|e| e.to_string()) };
            if here.account.as_deref() == Some(whose.as_str()) && here.persons_at.is_some_and(|t| t >= at) {
                return serde_json::to_value(&here).map_err(|e| e.to_string());
            }
        }
        let all = self.api("GET", "/api/registry", None).map_err(|f| Self::not_downloaded(&f))?;
        let list = |name: &str| serde_json::from_value::<Vec<Value>>(all[name].clone()).map_err(|e| e.to_string());
        let Some(at) = all["at"].as_u64() else { return serde_json::to_value(&here).map_err(|e| e.to_string()) };
        let about = self.registry.store(Some(list("persons")?), Some(list("horses")?), at, false, Some(&whose))?;
        serde_json::to_value(&about).map_err(|e| e.to_string())
    }

    /// One of Swiss Equestrian's demo events (`driving`: its driving one), as equievents.ch
    /// downloads it: its three answers (`getStammdaten`, `getOffizielle`, `getVeranstaltung`).
    pub(crate) fn demo_via_server(&self, driving: bool) -> Result<Value, String> {
        if self.signed_in()?.is_none() {
            return Err("sign in to equievents.ch first".into());
        }
        self.api_job("POST", "/api/demo", Some(&json!({ "driving": driving }))).map_err(|f| Self::not_downloaded(&f))
    }

    /// Delete a show here and, for a show of the server, `choice` there: `keep` (the rights:
    /// only this copy goes), `give` (the owner gives it to `to`, a collaborator, and leaves it),
    /// `release` (the owner deletes it from equievents.ch for everyone), `leave` (a collaborator
    /// leaves it). This copy's last changes go up first.
    pub(crate) fn delete_with(&self, show: &str, choice: &str, to: Option<i64>) -> Result<Value, String> {
        if let Some(remote) = self.remote_of(show)? {
            let _ = self.sync(show);
            let done = match choice {
                "keep" => Ok(Value::Null),
                "give" => self.api("POST", &format!("/api/shows/{remote}/handover"), Some(&json!({ "to": to.ok_or("missing argument to")? }))),
                "release" => self.api("DELETE", &format!("/api/shows/{remote}"), None),
                "leave" => {
                    let me = self.signed_in()?.map(|(account, _)| account.id).ok_or("sign in to equievents.ch first")?;
                    self.api("DELETE", &format!("/api/shows/{remote}/members/{me}"), None)
                }
                other => return Err(format!("no way to delete a show called {other}")),
            };
            if let Err(f) = done {
                return Ok(f.answer());
            }
        }
        self.delete_show(show)?;
        self.sync.notes.lock().map_err(|_| "busy")?.remove(show);
        Ok(json!({ "ok": true }))
    }

    /// The show file now follows a show of the server: everything in it goes up.
    fn link(&self, show: &str, remote: &str) -> Result<(), String> {
        let server = self.server_url();
        self.with(show, |_, store| {
            store.set_meta(META_SERVER, Some(&server)).map_err(|e| e.0)?;
            store.set_meta(META_REMOTE, Some(remote)).map_err(|e| e.0)?;
            store.set_meta(META_CURSOR, Some("0")).map_err(|e| e.0)?;
            store.mark_all_unsynced().map_err(|e| e.0)
        })
    }

    /// Bring a show of the server to this computer: a new show file, everything pulled.
    fn download_show(&self, remote: &str) -> Result<Value, String> {
        if let Some(local) = self.linked()?.get(remote) {
            return Ok(json!({ "ok": true, "show": local }));
        }
        if self.signed_in()?.is_none() {
            return Ok(json!({ "ok": false, "code": "signed_out" }));
        }
        let base = remote.rsplit_once('-').map_or(remote, |(b, _)| b);
        let local = self.new_id(base);
        let mut store = Store::open(&self.path(&local)).map_err(|e| e.0)?;
        let server = self.server_url();
        for (key, value) in [(META_SERVER, server.as_str()), (META_REMOTE, remote), (META_CURSOR, "0")] {
            store.set_meta(key, Some(value)).map_err(|e| e.0)?;
        }
        self.open.lock().map_err(|_| "the library is busy".to_string())?.insert(local.clone(), store);
        let note = self.sync(&local)?;
        let empty = self.with(&local, |_, store| Ok(store.log().map_err(|e| e.0)?.is_empty()))?;
        if empty {
            // Nothing came: no empty show left behind.
            self.open.lock().map_err(|_| "the library is busy".to_string())?.remove(&local);
            let _ = std::fs::remove_file(self.path(&local));
            return Ok(json!({ "ok": false, "code": if note.status == "synced" { "empty".to_string() } else { note.status.clone() }, "error": note.error }));
        }
        Ok(json!({ "ok": true, "show": local }))
    }

    /// Archive a show (`archived`) or take it out of the archives: on the server when it is
    /// there (its responsable only: `not_owner` otherwise), the show file following; a show only
    /// here, here. `show` here, or `remote` (a show on the server, here or not).
    fn archive_show(&self, show: Option<&str>, remote: Option<&str>, archived: bool) -> Result<Value, String> {
        let (show, remote) = match (show, remote) {
            (Some(show), _) => (Some(show.to_string()), self.remote_of(show)?),
            (None, Some(remote)) => (self.linked()?.get(remote).cloned(), Some(remote.to_string())),
            (None, None) => return Err("missing argument show".into()),
        };
        if let Some(remote) = &remote {
            if let Err(f) = self.api("POST", &format!("/api/shows/{remote}/archive"), Some(&json!({ "archived": archived }))) {
                return Ok(f.answer());
            }
        }
        if let Some(show) = &show {
            self.with(show, |_, store| store.set_meta(META_ARCHIVED, archived.then_some("1")).map_err(|e| e.0))?;
        }
        Ok(json!({ "ok": true, "archived": archived }))
    }

    /// A show's link and the last sync, without asking the server.
    fn sync_status(&self, show: &str) -> Result<Value, String> {
        let remote = self.remote_of(show)?;
        let pending = self.with(show, |_, store| store.unsynced_count().map_err(|e| e.0))?;
        let note = self.sync.notes.lock().map_err(|_| "busy")?.get(show).cloned();
        let rehearsal = self.with(show, |_, store| Ok(store.log().map_err(|e| e.0)?.first().is_some_and(|op| op.author == REHEARSAL)))?;
        let event = self.with(show, |_, store| Ok(store.state().event.is_some()))?;
        Ok(json!({
            // A training show: never synced.
            "training": self.is_training(show)?,
            // The online version (on equievents.ch).
            "hosted": self.sync.hosted(),
            "server": self.server_url(),
            "signed_in": self.signed_in()?.is_some(),
            "remote": remote,
            "pending": if remote.is_some() { pending } else { 0 },
            // Goes up with the event's key (not a rehearsal, which goes up without one).
            "needs_key": event && !rehearsal,
            "note": note,
        }))
    }

    /// Sync one show with the server: what was made here goes up, what others made comes down.
    /// The outcome is also kept for `sync_status`.
    pub(crate) fn sync(&self, show: &str) -> Result<Note, String> {
        let Some(remote) = self.remote_of(show)? else {
            return Ok(Note { status: "local".into(), ..Note::default() });
        };
        // One sync of a show at a time.
        {
            let mut busy = self.sync.busy.lock().map_err(|_| "busy")?;
            if !busy.insert(show.to_string()) {
                return Ok(self.sync.notes.lock().map_err(|_| "busy")?.get(show).cloned().unwrap_or_default());
            }
        }
        let _busy = Busy { set: &self.sync.busy, show: show.to_string() };
        let previous = self.sync.notes.lock().map_err(|_| "busy")?.get(show).cloned().unwrap_or_default();
        let note = match self.exchange(show, &remote) {
            Ok(note) => note,
            Err(f) => Note {
                status: match f.code().as_str() {
                    "offline" | "signed_out" | "no_access" => f.code(),
                    _ => "error".into(),
                },
                error: Some(f.message()),
                ..previous
            },
        };
        // The online version keeps no copy of a show the account no longer works on (released by
        // its owner, or taken off it).
        if self.sync.hosted() && note.status == "no_access" {
            drop(_busy);
            self.delete_show(show)?;
            self.sync.notes.lock().map_err(|_| "busy")?.remove(show);
            return Ok(note);
        }
        self.sync.notes.lock().map_err(|_| "busy")?.insert(show.to_string(), note.clone());
        Ok(note)
    }

    fn exchange(&self, show: &str, remote: &str) -> Result<Note, Failure> {
        let local = |e: String| Failure::Status { status: 500, body: json!({ "error": e }) };
        // Who we are on the show (and whether we still are).
        let shows = self.api("GET", "/api/shows", None)?;
        let Some(summary) = shows.as_array().and_then(|a| a.iter().find(|s| s["id"] == remote)).cloned() else {
            return Err(Failure::Status { status: 404, body: json!({ "code": "no_access", "error": "no longer on this show" }) });
        };
        // Archived or not, and one's role: kept for the library.
        let (archived, role) = (summary["archived"] == true, summary["role"].as_str().map(str::to_string));
        self.with(show, |_, store| {
            if (store.meta(META_ARCHIVED).map_err(|e| e.0)?.as_deref() == Some("1")) != archived {
                store.set_meta(META_ARCHIVED, archived.then_some("1")).map_err(|e| e.0)?;
            }
            if store.meta(META_ROLE).map_err(|e| e.0)? != role {
                store.set_meta(META_ROLE, role.as_deref()).map_err(|e| e.0)?;
            }
            Ok(())
        })
        .map_err(local)?;
        let mut note = Note {
            status: "synced".into(),
            role: summary["role"].as_str().map(str::to_string),
            owner: summary["owner"].as_str().map(str::to_string),
            requests: summary["requests"].as_i64().unwrap_or_default(),
            ..Note::default()
        };
        // Up: what waits in the file.
        loop {
            let ops = self.with(show, |_, store| store.unsynced(PUSH_BATCH).map_err(|e| e.0)).map_err(local)?;
            if ops.is_empty() {
                break;
            }
            let pushed = self.api("POST", &format!("/api/shows/{remote}/ops"), Some(&json!({ "ops": ops })))?;
            let refused: HashSet<String> = pushed["refused"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
            let stored: Vec<String> = ops.iter().map(|o| o.id.clone()).filter(|id| !refused.contains(id)).collect();
            note.pushed += stored.len();
            self.with(show, |_, store| store.mark_synced(&stored).map_err(|e| e.0)).map_err(local)?;
            if let Some(server_now) = pushed["now"].as_u64() {
                let ahead = now_millis() as i64 - server_now as i64;
                note.clock_ahead = (ahead > 60_000).then_some(ahead);
            }
            if !refused.is_empty() {
                // Stamped in the server's future: they wait until the time has come.
                note.status = "clock".into();
                break;
            }
        }
        // Down: what came after the cursor.
        loop {
            let cursor = self.with(show, |_, store| Ok(store.meta(META_CURSOR).map_err(|e| e.0)?.and_then(|c| c.parse::<i64>().ok()).unwrap_or(0))).map_err(local)?;
            let pulled = self.api("GET", &format!("/api/shows/{remote}/ops?after={cursor}"), None)?;
            let ops: Vec<Operation> = serde_json::from_value(pulled["ops"].clone()).map_err(|e| local(e.to_string()))?;
            let next = pulled["cursor"].as_i64().unwrap_or(cursor);
            self.with(show, |_, store| {
                note.pulled += store.receive(&ops).map_err(|e| e.0)?;
                store.set_meta(META_CURSOR, Some(&next.to_string())).map_err(|e| e.0)
            })
            .map_err(local)?;
            if pulled["more"] != true {
                break;
            }
        }
        note.last_ok = Some(now_millis());
        Ok(note)
    }
}

fn number(args: &Value, name: &str) -> Result<i64, String> {
    args[name].as_i64().or_else(|| args[name].as_str().and_then(|s| s.parse().ok())).ok_or_else(|| format!("missing argument {name}"))
}
