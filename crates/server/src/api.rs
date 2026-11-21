//! The HTTP API the apps use (JSON; `Authorization: Bearer <token>` after signing in):
//!
//! | | |
//! |---|---|
//! | `GET /api/health` | the server and its database answer |
//! | `POST /api/register` | `{name, email, password, device}` → `{token, user}`: a new account |
//! | `POST /api/login` | `{email, password, device}` → `{token, user}` |
//! | `POST /api/password-reset` | `{email, language}`: a link by e-mail, if the account exists |
//! | `POST /api/logout` | the token signed out |
//! | `GET /api/me` | who is signed in |
//! | `GET /api/shows` | the shows one works on (role, owner, requests waiting) |
//! | `POST /api/shows` | `{access_key, event_id?, name?, number?}`: an event's show, the first to enter its key its owner (checked with Swiss Equestrian: 202 `{job}`; the key alone: the event downloaded, the show made from it); or `{name, number?}`: a show without an event |
//! | `GET /api/jobs/{id}` | what waits for Swiss Equestrian: 202 `{working}` while it works, then the answer the request would have given |
//! | `GET /api/swiss-equestrian` | `{downloads, sends}`: which Swiss Equestrian (`production`, `stage`, `none`); where « Envoyer » goes (`simulation`: this version never sends to Swiss Equestrian) |
//! | `POST /api/shows/{id}/send` | `{what, class?, published?, id?, language?, through?}`: « Envoyer », to the simulation (202 `{job}` → `{sent, server, warnings}`), once the server has the app's operation `through` |
//! | `POST /api/shows/{id}/registry` | Swiss Equestrian's riders and horses, downloaded with the show's key, kept for one's account (202 `{job}` → `{persons, horses, at}`) |
//! | `GET /api/registry` | one's account's riders and horses `{persons, horses, at}` (`GET /api/registry/about`: the counts and when) |
//! | `POST /api/demo` | `{driving}`: one of Swiss Equestrian's demo events, its three answers, for an app's training show (202 `{job}`) |
//! | `POST /api/shows/{id}/update` | what Swiss Equestrian changed since the event was loaded (202 `{job}` → `{added}`) |
//! | `GET /api/shows/{id}/received` | what the simulation received for the show, the latest first |
//! | `GET /api/shows/{id}/members` | who works on it |
//! | `POST /api/shows/{id}/handover` | `{to}`: the owner gives the show to a collaborator and leaves it |
//! | `DELETE /api/shows/{id}` | the owner releases the event: the show deleted for everyone, its key free again |
//! | `POST /api/shows/{id}/members` | `{email}`: the owner lets someone work on it |
//! | `DELETE /api/shows/{id}/members/{user}` | the owner takes someone off; a collaborator leaves |
//! | `GET /api/shows/{id}/requests` | the requests waiting for the owner |
//! | `POST /api/shows/{id}/requests` | `{kind: access|ownership, access_key}`: ask the owner |
//! | `GET /api/requests` | one's own requests and their answers |
//! | `POST /api/requests/{id}` | `{accept, stay?}`: the owner answers (`stay`: for a transfer) |
//! | `DELETE /api/requests/{id}` | who asked takes it back |
//! | `POST /api/shows/{id}/ops` | `{ops}` → `{added, known, cursor}`: push |
//! | `GET /api/shows/{id}/ops?after=n` | → `{ops, cursor, more}`: pull |
//!
//! The desktop apps' update check (`GET /api/updates/…`) and downloads: `downloads`.
//!
//! Errors: an HTTP status and `{error}` in words (and `code`, for refused account details: see
//! `auth::Invalid::code`). Too many attempts: 429. The site's pages: `site`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, DefaultBodyLimit, FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use deadpool_postgres::Pool;
use serde::Deserialize;
use serde_json::{json, Value};
use show::oplog::Operation;

use crate::auth::{self, Invalid, User};
use crate::limit::{self, Limiter};
use crate::mail::Mailer;
use crate::notices;
use crate::se::{self, Se};
use crate::shows::{self, Access, Claimed, Refused};
use crate::site::{self, Lang};

#[derive(Clone)]
pub struct App {
    pub pool: Pool,
    pub mailer: Mailer,
    pub limits: Arc<Limiter>,
    /// The site's address, for the links in e-mails (`APP_URL`).
    pub site_url: String,
    /// E-mails sent without waiting (tests wait, to read them).
    pub mail_in_background: bool,
    /// The desktop apps' releases (`DOWNLOADS_DIR`; none: no downloads yet).
    pub downloads: Option<std::path::PathBuf>,
    /// Swiss Equestrian (`SE_SERVER`; none: development, keys not checked).
    pub se: Se,
    /// The online version (`web`): none, not served.
    pub web: Option<Arc<crate::web::Web>>,
}

impl App {
    pub fn new(pool: Pool, mailer: Mailer, site_url: &str) -> App {
        App { pool, mailer, limits: Arc::default(), site_url: site_url.trim_end_matches('/').to_string(), mail_in_background: true, downloads: None, se: Se::none(), web: None }
    }
}

/// The client's address: from Nginx Proxy Manager (`X-Real-IP`, else the last
/// `X-Forwarded-For`), else the connection's.
pub struct Client(pub String);

impl<S: Send + Sync> FromRequestParts<S> for Client {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let header = |name: &str| parts.headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim).filter(|v| !v.is_empty());
        let address = header("x-real-ip")
            .map(str::to_string)
            .or_else(|| header("x-forwarded-for").and_then(|v| v.rsplit(',').next()).map(|v| v.trim().to_string()))
            .or_else(|| parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip().to_string()))
            .unwrap_or_else(|| "unknown".into());
        Ok(Client(address))
    }
}

pub struct ApiError(StatusCode, String, Option<&'static str>);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = match self.2 {
            Some(code) => json!({ "error": self.1, "code": code }),
            None => json!({ "error": self.1 }),
        };
        (self.0, Json(body)).into_response()
    }
}

impl From<Invalid> for ApiError {
    fn from(e: Invalid) -> Self {
        let status = if e == Invalid::Taken { StatusCode::CONFLICT } else { StatusCode::BAD_REQUEST };
        ApiError(status, e.to_string(), Some(e.code()))
    }
}

impl From<Refused> for ApiError {
    fn from(e: Refused) -> Self {
        let status = match e {
            Refused::Key => StatusCode::FORBIDDEN,
            Refused::NotFound => StatusCode::NOT_FOUND,
            Refused::Member | Refused::Owner | Refused::Answered => StatusCode::CONFLICT,
            Refused::Stay | Refused::Account => StatusCode::BAD_REQUEST,
        };
        ApiError(status, e.to_string(), Some(e.code()))
    }
}

/// An e-mail (its text and its HTML), in the background (tests wait for it).
pub(crate) async fn notify(app: &App, to: &str, letter: notices::Letter) {
    let (subject, text, html) = (letter.subject(), letter.text(&app.site_url), letter.html(&app.site_url, to));
    let (mailer, to) = (app.mailer.clone(), to.to_string());
    let send = async move {
        if let Err(e) = mailer.send(&to, &subject, &text, Some(&html)).await {
            eprintln!("e-mail to {to} not sent: {e}");
        }
    };
    if app.mail_in_background {
        tokio::spawn(send);
    } else {
        send.await;
    }
}

fn too_many() -> ApiError {
    ApiError(StatusCode::TOO_MANY_REQUESTS, "too many attempts; try again later".into(), None)
}

impl From<crate::db::Error> for ApiError {
    fn from(e: crate::db::Error) -> Self {
        eprintln!("error: {e}");
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, "the server could not do it".into(), None)
    }
}

fn bad(why: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, why.into(), None)
}

type Answer = Result<Json<Value>, ApiError>;

pub fn router(app: App) -> Router {
    Router::new()
        .merge(site::routes())
        .merge(crate::downloads::routes())
        .merge(crate::web::routes())
        .merge(crate::admin::routes())
        .route("/api/health", get(health))
        .route("/api/register", post(register))
        .route("/api/login", post(login))
        .route("/api/password-reset", post(password_reset))
        .route("/api/logout", post(logout))
        .route("/api/me", get(me).post(set_me))
        .route("/api/shows", get(list_shows).post(create_show))
        .route("/api/shows/{id}/members", get(list_members).post(add_member))
        .route("/api/shows/{id}/members/{user}", axum::routing::delete(remove_member))
        .route("/api/shows/{id}/requests", get(show_requests).post(make_request))
        .route("/api/shows/{id}/archive", post(archive))
        .route("/api/shows/{id}/handover", post(hand_over))
        .route("/api/shows/{id}", axum::routing::delete(release))
        .route("/api/requests", get(my_requests))
        .route("/api/requests/{id}", post(decide).delete(withdraw))
        .route("/api/jobs/{id}", get(job))
        .route("/api/swiss-equestrian", get(swiss_equestrian))
        .route("/api/shows/{id}/send", post(send))
        .route("/api/shows/{id}/update", post(update))
        .route("/api/shows/{id}/received", get(received))
        .route("/api/shows/{id}/registry", post(download_registry))
        .route("/api/registry", get(registry))
        .route("/api/registry/about", get(registry_about))
        .route("/api/demo", post(demo))
        // A show's first push carries its whole history (the event's master data included).
        .route("/api/shows/{id}/ops", get(pull).post(push).layer(DefaultBodyLimit::max(256 * 1024 * 1024)))
        .with_state(app)
}

async fn health(State(app): State<App>) -> Response {
    let db = match app.pool.get().await {
        Ok(client) => client.query_one("SELECT 1", &[]).await.is_ok(),
        Err(_) => false,
    };
    let status = if db { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    (status, Json(json!({ "ok": db, "database": db, "version": env!("CARGO_PKG_VERSION") }))).into_response()
}

fn token(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::AUTHORIZATION)?.to_str().ok()?.strip_prefix("Bearer ").map(str::trim).filter(|t| !t.is_empty())
}

async fn signed_in(app: &App, headers: &HeaderMap) -> Result<User, ApiError> {
    let unauthorized = || ApiError(StatusCode::UNAUTHORIZED, "not signed in".into(), None);
    let token = token(headers).ok_or_else(unauthorized)?;
    auth::user_of(&app.pool, token).await?.ok_or_else(unauthorized)
}

/// The signed-in user, who must work on the show (`owner`: as its owner).
async fn member(app: &App, headers: &HeaderMap, show: &str, owner: bool) -> Result<User, ApiError> {
    let user = signed_in(app, headers).await?;
    match shows::access(&app.pool, &user, show).await? {
        Access::None => Err(ApiError(StatusCode::NOT_FOUND, format!("no show {show}"), None)),
        Access::Collaborator if owner => Err(ApiError(StatusCode::FORBIDDEN, "only the show's owner can".into(), None)),
        _ => Ok(user),
    }
}

#[derive(Deserialize)]
struct Login {
    email: String,
    password: String,
    #[serde(default)]
    device: String,
}

async fn login(State(app): State<App>, Client(client): Client, Json(body): Json<Login>) -> Answer {
    let email = body.email.trim().to_lowercase();
    if !app.limits.allow(&format!("login:{client}"), limit::LOGIN_CLIENT) || !app.limits.allow(&format!("login-email:{email}"), limit::LOGIN_EMAIL) {
        return Err(too_many());
    }
    match auth::sign_in(&app.pool, &email, &body.password, &body.device).await? {
        Some((token, user)) => Ok(Json(json!({ "token": token, "user": user }))),
        None => Err(ApiError(StatusCode::UNAUTHORIZED, "wrong e-mail address or password".into(), None)),
    }
}

#[derive(Deserialize)]
struct Register {
    name: String,
    email: String,
    password: String,
    #[serde(default)]
    device: String,
    /// Of the e-mails: `fr` (the default) or `de`.
    #[serde(default)]
    language: String,
}

/// A new account, signed in on the device that made it.
async fn register(State(app): State<App>, Client(client): Client, Json(body): Json<Register>) -> Answer {
    if !app.limits.allow(&format!("register:{client}"), limit::REGISTER_CLIENT) {
        return Err(too_many());
    }
    let (token, user) = auth::register(&app.pool, &body.name, &body.email, &body.password, &body.device, Lang::of(&body.language)).await??;
    println!("account made: {}", user.email);
    Ok(Json(json!({ "token": token, "user": user })))
}

#[derive(Deserialize)]
struct PasswordReset {
    email: String,
    #[serde(default)]
    language: String,
}

/// A reset link by e-mail if the address has an account; the same answer either way.
async fn password_reset(State(app): State<App>, Client(client): Client, Json(body): Json<PasswordReset>) -> Answer {
    if !app.limits.allow(&format!("reset:{client}"), limit::RESET_CLIENT) {
        return Err(too_many());
    }
    auth::check_email(&body.email)?;
    site::send_reset_link(&app, Lang::of(&body.language), &body.email).await?;
    Ok(Json(json!({ "ok": true })))
}

async fn logout(State(app): State<App>, headers: HeaderMap) -> Answer {
    if let Some(t) = token(&headers) {
        auth::sign_out(&app.pool, t).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

/// The account, and its settings (`archive_after_weeks`).
async fn me(State(app): State<App>, headers: HeaderMap) -> Answer {
    let user = signed_in(&app, &headers).await?;
    let mut answer = json!(user);
    answer["archive_after_weeks"] = json!(shows::archive_after(&app.pool, &user).await?);
    Ok(Json(answer))
}

#[derive(Deserialize)]
struct Settings {
    /// Archive the shows one owns this many weeks after their last day (`null`: by hand only).
    archive_after_weeks: Option<i32>,
}

async fn set_me(State(app): State<App>, headers: HeaderMap, Json(body): Json<Settings>) -> Answer {
    let user = signed_in(&app, &headers).await?;
    if body.archive_after_weeks.is_some_and(|w| !(1..=104).contains(&w)) {
        return Err(bad("archive after 1 to 104 weeks"));
    }
    shows::set_archive_after(&app.pool, &user, body.archive_after_weeks).await?;
    me(State(app), headers).await
}

#[derive(Deserialize)]
struct Archive {
    archived: bool,
}

/// The owner archives the show, or takes it out of the archives: every member's library lists
/// it there (nothing else changes).
async fn archive(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<Archive>) -> Answer {
    let user = signed_in(&app, &headers).await?;
    match shows::access(&app.pool, &user, &id).await? {
        Access::Owner => {}
        Access::Collaborator => return Err(ApiError(StatusCode::FORBIDDEN, "only the show's owner archives it".into(), Some("not_owner"))),
        Access::None => return Err(ApiError(StatusCode::NOT_FOUND, format!("no show {id}"), None)),
    }
    shows::archive(&app.pool, &id, body.archived).await?;
    Ok(Json(json!({ "ok": true, "archived": body.archived })))
}

async fn list_shows(State(app): State<App>, headers: HeaderMap) -> Answer {
    let user = signed_in(&app, &headers).await?;
    Ok(Json(json!(shows::list(&app.pool, &user).await?)))
}

#[derive(Deserialize)]
struct NewShow {
    #[serde(default)]
    name: String,
    event_id: Option<i64>,
    number: Option<String>,
    /// The event's key (Swiss Equestrian's): an event's show.
    access_key: Option<String>,
    /// The show and its key come from the event's Rosson export: a key Swiss Equestrian no
    /// longer accepts (a past event's) is taken on trust (the user, 2026-11-21: "if they have the
    /// export, they have the key").
    #[serde(default)]
    from_export: bool,
}

/// An event's show with its key: made, and theirs, for the first to enter it; for the others,
/// 409 with who owns it (to ask them). Without a key: a show without an event (a rehearsal).
///
/// With Swiss Equestrian (`SE_SERVER`), the key is checked there first, in a job (202 `{job}`):
/// the event it opens (`event_id`, when given, must be that one); the key alone downloads the
/// event, and a show made now starts with Swiss Equestrian's delivery. Without (development),
/// the app says which event, unchecked.
async fn create_show(State(app): State<App>, headers: HeaderMap, Json(body): Json<NewShow>) -> Result<Response, ApiError> {
    let user = signed_in(&app, &headers).await?;
    let Some(access_key) = body.access_key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) else {
        if body.event_id.is_some() {
            return Err(bad("an event's show is opened with the event's key (access_key)"));
        }
        if body.name.trim().is_empty() {
            return Err(bad("a show needs a name"));
        }
        let id = shows::create(&app.pool, &user, &body.name, body.number.as_deref()).await?;
        return Ok(Json(json!({ "id": id, "role": "owner", "created": true })).into_response());
    };
    // Swiss Equestrian's demo events: anyone may load them (the app's settings), no one owns them.
    if se_api::DEMO_KEYS.contains(&access_key.as_str()) {
        return Err(ApiError(StatusCode::BAD_REQUEST, "Swiss Equestrian's demo events stay on the computer (the app's settings, Exercice)".into(), Some("training")));
    }
    // Keys are not guessed: so many tries an hour per account.
    if !app.limits.allow(&format!("key:{}", user.id), limit::KEY_USER) {
        return Err(too_many());
    }
    if app.se.base.is_some() {
        let export = body.from_export.then(|| (body.name.clone(), body.number.clone()));
        let job = app.se.jobs.start(user.id, checked_claim(app.clone(), user, access_key, body.event_id, export));
        return Ok((StatusCode::ACCEPTED, Json(json!({ "job": job }))).into_response());
    }
    let Some(event_id) = body.event_id else {
        return Err(ApiError(StatusCode::BAD_REQUEST, "this server does not download from Swiss Equestrian (SE_SERVER=none): give the event's id with its key".into(), Some("no_swiss_equestrian")));
    };
    let claim = shows::Claim { access_key, event_id, name: body.name, number: body.number, checked: false, ops: Vec::new() };
    let (outcome, unlinked) = shows::claim(&app.pool, &user, &claim).await?;
    tell_unlinked(&app, unlinked).await;
    let (status, answer) = claimed(outcome, event_id, &user);
    Ok((status, Json(answer)).into_response())
}

/// The owner of a show no longer the event's (`shows::Unlinked`), told.
async fn tell_unlinked(app: &App, unlinked: Option<shows::Unlinked>) {
    let Some(unlinked) = unlinked else { return };
    println!("show {} unlinked from its event: Swiss Equestrian accepted another key", unlinked.show);
    if let Some(owner) = unlinked.owner {
        notify(app, &owner.email, notices::unlinked(owner.lang, &owner.name, &unlinked.name)).await;
    }
}

/// The answer to an event's key: made (theirs), a member already, someone else's (409), or not
/// the event's key (403).
fn claimed(outcome: Claimed, event_id: i64, user: &User) -> (StatusCode, Value) {
    match outcome {
        Claimed::Made { id } => {
            println!("show {id} (event {event_id}) made by {}", user.email);
            (StatusCode::OK, json!({ "id": id, "role": "owner", "created": true, "event_id": event_id }))
        }
        Claimed::Member { id, role } => (StatusCode::OK, json!({ "id": id, "role": role, "created": false, "event_id": event_id })),
        Claimed::Owned { id, name, owner, request } => (
            StatusCode::CONFLICT,
            json!({ "error": format!("{owner} owns this show: ask them for access or for the show"), "code": "claimed", "id": id, "name": name, "owner": owner, "request": request, "event_id": event_id }),
        ),
        Claimed::WrongKey => (StatusCode::FORBIDDEN, json!({ "error": Refused::Key.to_string(), "code": Refused::Key.code() })),
    }
}

/// An event's key checked with Swiss Equestrian (the job of `create_show`): which event it opens
/// (`getVeranstaltung`); `event_id` given (the app's copy of the show): it must be that one,
/// and the app's copy goes up; else the event downloaded, and a show made now starts with it.
/// `export`: the show and its key come from the event's Rosson export (its name and number): a
/// key Swiss Equestrian refuses (a past event's) is then taken on trust, unchecked.
async fn checked_claim(app: App, user: User, access_key: String, event_id: Option<i64>, export: Option<(String, Option<String>)>) -> (u16, Value) {
    let refused = |why: &str| (StatusCode::FORBIDDEN.as_u16(), json!({ "error": why, "code": Refused::Key.code() }));
    let failed = |e: se_api::http::ApiError| {
        eprintln!("Swiss Equestrian, for {}: {e}", user.email);
        (StatusCode::BAD_GATEWAY.as_u16(), json!({ "error": e.to_string(), "code": "swiss_equestrian" }))
    };
    let db = |e: crate::db::Error| {
        eprintln!("error: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR.as_u16(), json!({ "error": "the server could not do it" }))
    };
    let keys = app.se.keys(&access_key).await;
    let event = match app.se.get(&se_api::load::get_veranstaltung(&keys, None)).await {
        Ok(event) => event,
        // A past event's key, from its Rosson export: whoever has the export has the key.
        Err(e) if se::key_refused(&e) => match (export, event_id) {
            (Some((name, number)), Some(event_id)) => {
                let claim = shows::Claim { access_key: access_key.clone(), event_id, name, number, checked: false, ops: Vec::new() };
                return match shows::claim(&app.pool, &user, &claim).await {
                    Ok((outcome, unlinked)) => {
                        tell_unlinked(&app, unlinked).await;
                        let (status, answer) = claimed(outcome, event_id, &user);
                        (status.as_u16(), answer)
                    }
                    Err(e) => db(e),
                };
            }
            _ => return refused("Swiss Equestrian does not know this key"),
        },
        Err(e) => return failed(e),
    };
    let delivered = &event["data"]["veranstaltung"];
    let Some(se_event) = se_api::load::number(&delivered["id"]).map(|id| id as i64) else {
        return failed(se_api::http::ApiError::BadAnswer("the event has no id".into()));
    };
    if event_id.is_some_and(|id| id != se_event) {
        return refused("this key opens another event");
    }
    let text = |key: &str| delivered[key].as_str().map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
    let mut claim = shows::Claim {
        access_key: keys.access_key.clone(),
        event_id: se_event,
        name: text("erste_titelzeile").unwrap_or_else(|| se_event.to_string()),
        number: text("nr"),
        checked: true,
        ops: Vec::new(),
    };
    // Downloaded: a new show starts with Swiss Equestrian's delivery (someone may make it at the
    // same moment: then theirs).
    if event_id.is_none() {
        match shows::of_event(&app.pool, se_event).await {
            Ok(Some(_)) => {}
            Ok(None) => match app.se.new_show(&keys, &event, &user.name).await {
                Ok(ops) => claim.ops = ops,
                Err(e) if se::key_refused(&e) => return refused("Swiss Equestrian does not know this key"),
                Err(e) => return failed(e),
            },
            Err(e) => return db(e),
        }
    }
    match shows::claim(&app.pool, &user, &claim).await {
        Ok((outcome, unlinked)) => {
            tell_unlinked(&app, unlinked).await;
            let (status, answer) = claimed(outcome, se_event, &user);
            (status.as_u16(), answer)
        }
        Err(e) => db(e),
    }
}

/// What waits for Swiss Equestrian (one's own): 202 while it works, then its answer.
async fn job(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Result<Response, ApiError> {
    let user = signed_in(&app, &headers).await?;
    match app.se.jobs.outcome(user.id, &id) {
        None => Err(ApiError(StatusCode::NOT_FOUND, "no such job (or forgotten: try again)".into(), Some("no_job"))),
        Some(None) => Ok((StatusCode::ACCEPTED, Json(json!({ "working": true }))).into_response()),
        Some(Some((status, answer))) => Ok((StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(answer)).into_response()),
    }
}

/// Which Swiss Equestrian the server downloads from, and where it sends (the apps say it).
async fn swiss_equestrian(State(app): State<App>) -> Json<Value> {
    Json(json!({ "downloads": app.se.name(), "sends": app.se.sends_to() }))
}

#[derive(Deserialize)]
struct SendBody {
    /// `results` of a class, `start_lists` or `withdraw_start_lists` of a published class, `event`,
    /// `combined` (a ranking over several classes).
    what: String,
    class: Option<String>,
    published: Option<u64>,
    id: Option<String>,
    /// Of the start lists' headings: `de` (the default), `fr`, `it`.
    language: Option<String>,
    /// The app's latest operation: the send includes it (the app pushes first).
    through: Option<String>,
}

/// A show's event key as kept, or why there is none (a show without an event: a rehearsal).
async fn kept_key(app: &App, show: &str) -> Result<String, ApiError> {
    match shows::event_key(&app.pool, show).await? {
        Some((_, key)) if !key.trim().is_empty() => Ok(key),
        _ => Err(ApiError(StatusCode::CONFLICT, "the show has no event's key on equievents.ch".into(), Some("need_key"))),
    }
}

/// Send to Swiss Equestrian (the simulation until the pilot), as the apps' « Envoyer »: what
/// the show is on the server, once it has the app's latest operation; the answer goes into the
/// show's log (`Uploaded`, the officials' ids), by the one who sent.
async fn send(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<SendBody>) -> Result<Response, ApiError> {
    let user = member(&app, &headers, &id, false).await?;
    if let Some(op) = body.through.as_deref() {
        if !shows::has(&app.pool, &id, op).await? {
            return Err(ApiError(StatusCode::CONFLICT, "the show's latest changes are not on equievents.ch yet: sync first".into(), Some("not_synced")));
        }
    }
    let key = kept_key(&app, &id).await?;
    let job = app.se.jobs.start(user.id, sent(app.clone(), user, id, key, body));
    Ok((StatusCode::ACCEPTED, Json(json!({ "job": job }))).into_response())
}

async fn sent(app: App, user: User, show: String, key: String, body: SendBody) -> (u16, Value) {
    use se_api::actions;
    use show::model::{ClassId, PublishedClassId};
    let log = match shows::log(&app.pool, &show).await {
        Ok(log) => log,
        Err(e) => {
            eprintln!("error: {e}");
            return (500, json!({ "error": "the server could not do it" }));
        }
    };
    let (state, _) = show::State::from_log(log);
    let keys = app.se.keys(&key).await;
    let language = match body.language.as_deref() {
        Some("fr") => se_api::startlists::Language::Fr,
        Some("it") => se_api::startlists::Language::It,
        _ => se_api::startlists::Language::De,
    };
    let class = || body.class.clone().map(ClassId).ok_or("missing argument class");
    let published = || body.published.map(PublishedClassId).ok_or("missing argument published");
    let steps = match body.what.as_str() {
        "results" => class().map_err(String::from).and_then(|c| actions::results(&state, &c, &keys)),
        "start_lists" => published().map_err(String::from).and_then(|p| actions::start_lists(&state, &p, &keys, language)),
        "withdraw_start_lists" => published().map(|p| actions::withdraw_start_lists(&p, &keys)).map_err(String::from),
        "event" => Ok(actions::event(&state, &keys)),
        "combined" => body.id.as_deref().ok_or_else(|| "missing argument id".to_string()).and_then(|id| actions::combined(&state, id, &keys)),
        other => Err(format!("nothing to send called {other}")),
    };
    let steps = match steps {
        Ok(steps) => steps,
        Err(e) => return (400, json!({ "error": e })),
    };
    let outcome = app.se.perform(&show, steps, &keys).await;
    if let Some(e) = outcome.failed {
        return (502, json!({ "error": e.to_string(), "code": "swiss_equestrian" }));
    }
    let ops = app.se.operations(outcome.changes, &user.name, state.last.as_ref());
    if let Err(e) = shows::add(&app.pool, &user, &show, &ops).await {
        eprintln!("error: {e}");
        return (500, json!({ "error": "the server could not do it" }));
    }
    (200, json!({ "sent": body.what, "server": app.se.sends_to(), "warnings": outcome.warnings }))
}

/// Swiss Equestrian is there to download from, and the account has not asked too often.
fn may_download(app: &App, user: &User) -> Result<(), ApiError> {
    if app.se.base.is_none() {
        return Err(ApiError(StatusCode::BAD_REQUEST, "this server does not download from Swiss Equestrian (SE_SERVER=none)".into(), Some("no_swiss_equestrian")));
    }
    if !app.limits.allow(&format!("se:{}", user.id), limit::SE_USER) {
        return Err(too_many());
    }
    Ok(())
}

/// What Swiss Equestrian changed since the event was loaded (Rosson's update), into the show's
/// log by the one who asked.
async fn update(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Result<Response, ApiError> {
    let user = member(&app, &headers, &id, false).await?;
    may_download(&app, &user)?;
    let key = kept_key(&app, &id).await?;
    let job = app.se.jobs.start(user.id, updated(app.clone(), user, id, key));
    Ok((StatusCode::ACCEPTED, Json(json!({ "job": job }))).into_response())
}

async fn updated(app: App, user: User, show: String, key: String) -> (u16, Value) {
    let failed = |e: crate::db::Error| {
        eprintln!("error: {e}");
        (500, json!({ "error": "the server could not do it" }))
    };
    let (state, _) = match shows::log(&app.pool, &show).await {
        Ok(log) => show::State::from_log(log),
        Err(e) => return failed(e),
    };
    if state.event.is_none() {
        return (409, json!({ "error": "the show has no event from Swiss Equestrian", "code": "need_key" }));
    }
    let keys = app.se.keys(&key).await;
    let ops = match app.se.update(&state, &keys, &user.name).await {
        Ok(ops) => ops,
        Err(e) if se::key_refused(&e) => return (403, json!({ "error": "Swiss Equestrian does not know this key", "code": "key" })),
        Err(e) => return (502, json!({ "error": e.to_string(), "code": "swiss_equestrian" })),
    };
    match shows::add(&app.pool, &user, &show, &ops).await {
        Ok(added) => (200, json!({ "added": added })),
        Err(e) => failed(e),
    }
}

/// Swiss Equestrian's riders and horses, downloaded with the show's key (one works on it), kept
/// for one's account: every show and computer of the account gets them from here.
async fn download_registry(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Result<Response, ApiError> {
    let user = member(&app, &headers, &id, false).await?;
    may_download(&app, &user)?;
    let key = kept_key(&app, &id).await?;
    let job = app.se.jobs.start(user.id, registry_downloaded(app.clone(), user, key));
    Ok((StatusCode::ACCEPTED, Json(json!({ "job": job }))).into_response())
}

async fn registry_downloaded(app: App, user: User, key: String) -> (u16, Value) {
    let keys = app.se.keys(&key).await;
    let (persons, horses) = match app.se.registry(&keys).await {
        Ok(lists) => lists,
        Err(e) if se::key_refused(&e) => return (403, json!({ "error": "Swiss Equestrian does not know this key", "code": "key" })),
        Err(e) => return (502, json!({ "error": e.to_string(), "code": "swiss_equestrian" })),
    };
    match crate::registry::store(&app.pool, &user, &persons, &horses).await {
        Ok(about) => (200, json!(about)),
        Err(e) => {
            eprintln!("error: {e}");
            (500, json!({ "error": "the server could not do it" }))
        }
    }
}

/// One's account's riders and horses (empty lists if never downloaded).
async fn registry(State(app): State<App>, headers: HeaderMap) -> Answer {
    let user = signed_in(&app, &headers).await?;
    Ok(Json(match crate::registry::get(&app.pool, &user).await? {
        Some((persons, horses, at)) => json!({ "persons": persons, "horses": horses, "at": at }),
        None => json!({ "persons": [], "horses": [], "at": null }),
    }))
}

async fn registry_about(State(app): State<App>, headers: HeaderMap) -> Answer {
    let user = signed_in(&app, &headers).await?;
    Ok(Json(json!(crate::registry::about(&app.pool, &user).await?)))
}

#[derive(Deserialize)]
struct Demo {
    #[serde(default)]
    driving: bool,
}

/// One of Swiss Equestrian's demo events (Rosson's « Manifestation test »), its three answers, for
/// an app's training show: the apps never reach Swiss Equestrian themselves. Nothing is kept.
async fn demo(State(app): State<App>, headers: HeaderMap, Json(body): Json<Demo>) -> Result<Response, ApiError> {
    let user = signed_in(&app, &headers).await?;
    may_download(&app, &user)?;
    let se = app.se.clone();
    let job = app.se.jobs.start(user.id, async move {
        match se.demo(body.driving).await {
            Ok(answers) => (200, answers),
            Err(e) => (502, json!({ "error": e.to_string(), "code": "swiss_equestrian" })),
        }
    });
    Ok((StatusCode::ACCEPTED, Json(json!({ "job": job }))).into_response())
}

/// What the simulation received for the show (the apps show it, field by field).
async fn received(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Answer {
    member(&app, &headers, &id, false).await?;
    Ok(Json(json!(app.se.simulation.received(&id))))
}

#[derive(Deserialize)]
struct HandOver {
    to: i64,
}

/// The owner gives the show to a collaborator and leaves it; the new owner gets an e-mail.
async fn hand_over(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<HandOver>) -> Answer {
    let owner = member(&app, &headers, &id, true).await?;
    let name = shows::owner_of(&app.pool, &id).await?.map(|(name, _)| name).unwrap_or_default();
    let to = shows::hand_over(&app.pool, &owner, &id, body.to).await??;
    println!("show {id} handed over by {} to {}", owner.email, to.email);
    notify(&app, &to.email, notices::handed_over(to.lang, &to.name, &owner.name, &name)).await;
    Ok(Json(json!({ "ok": true })))
}

/// The owner releases the event: the show deleted for everyone, its key free again; who worked
/// on it gets an e-mail.
async fn release(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Answer {
    let owner = member(&app, &headers, &id, true).await?;
    let (name, others) = shows::release(&app.pool, &owner, &id).await??;
    println!("show {id} released (deleted) by {}", owner.email);
    for to in &others {
        notify(&app, &to.email, notices::released(to.lang, &to.name, &owner.name, &name)).await;
    }
    Ok(Json(json!({ "ok": true, "removed": others.len() })))
}

async fn list_members(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Answer {
    member(&app, &headers, &id, false).await?;
    Ok(Json(json!(shows::members(&app.pool, &id).await?)))
}

#[derive(Deserialize)]
struct NewMember {
    email: String,
}

/// The owner lets someone with an account work on the show (no request needed).
async fn add_member(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<NewMember>) -> Answer {
    let owner = member(&app, &headers, &id, true).await?;
    let added = shows::add_member(&app.pool, &owner, &id, &body.email).await??;
    if added.id != owner.id {
        if let (Some((show, _)), Some(to)) = (shows::owner_of(&app.pool, &id).await?, shows::contact(&app.pool, added.id).await?) {
            notify(&app, &to.email, notices::access_given(to.lang, &to.name, &owner.name, &show)).await;
        }
    }
    Ok(Json(json!(added)))
}

async fn remove_member(State(app): State<App>, headers: HeaderMap, Path((id, user)): Path<(String, i64)>) -> Answer {
    let actor = member(&app, &headers, &id, false).await?;
    shows::remove_member(&app.pool, &actor, &id, user).await??;
    Ok(Json(json!({ "ok": true })))
}

async fn show_requests(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Answer {
    member(&app, &headers, &id, true).await?;
    Ok(Json(json!(shows::pending(&app.pool, &id).await?)))
}

#[derive(Deserialize)]
struct NewRequest {
    /// `access` or `ownership`.
    kind: String,
    access_key: String,
}

/// Ask the owner, with the event's key; they get an e-mail.
async fn make_request(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<NewRequest>) -> Answer {
    let user = signed_in(&app, &headers).await?;
    let ownership = match body.kind.as_str() {
        "access" => false,
        "ownership" => true,
        other => return Err(bad(format!("kind {other}: access or ownership"))),
    };
    if !app.limits.allow(&format!("key:{}", user.id), limit::KEY_USER) {
        return Err(too_many());
    }
    let request = shows::request(&app.pool, &user, &id, ownership, &body.access_key).await??;
    if let Some((show, owner)) = shows::owner_of(&app.pool, &id).await? {
        notify(&app, &owner.email, notices::request_made(owner.lang, &owner.name, &user.name, &user.email, &show, ownership)).await;
    }
    Ok(Json(json!(request)))
}

async fn my_requests(State(app): State<App>, headers: HeaderMap) -> Answer {
    let user = signed_in(&app, &headers).await?;
    Ok(Json(json!(shows::requests_of(&app.pool, &user).await?)))
}

#[derive(Deserialize)]
struct Decision {
    accept: bool,
    /// Accepting a transfer: the owner stays on the show as a collaborator (or leaves it).
    stay: Option<bool>,
}

/// The owner answers; who asked gets an e-mail.
async fn decide(State(app): State<App>, headers: HeaderMap, Path(id): Path<i64>, Json(body): Json<Decision>) -> Answer {
    let owner = signed_in(&app, &headers).await?;
    let request = shows::decide(&app.pool, &owner, id, body.accept, body.stay).await??;
    if let Some(to) = shows::contact(&app.pool, request.person.id).await? {
        notify(&app, &to.email, notices::request_answered(to.lang, &to.name, &request.show_name, request.kind == "ownership", body.accept)).await;
    }
    Ok(Json(json!(request)))
}

async fn withdraw(State(app): State<App>, headers: HeaderMap, Path(id): Path<i64>) -> Answer {
    let user = signed_in(&app, &headers).await?;
    Ok(Json(json!(shows::withdraw(&app.pool, &user, id).await??)))
}

#[derive(Deserialize)]
struct Push {
    ops: Vec<Value>,
}

async fn push(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>, Json(body): Json<Push>) -> Answer {
    let user = member(&app, &headers, &id, false).await?;
    if body.ops.len() > shows::PUSH_LIMIT {
        return Err(bad(format!("at most {} operations at a time", shows::PUSH_LIMIT)));
    }
    // Each one an operation the show core reads (else nothing is stored).
    let ops = body
        .ops
        .into_iter()
        .enumerate()
        .map(|(i, v)| serde_json::from_value::<Operation>(v).map_err(|e| bad(format!("operation {}: {e}", i + 1))))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(json!(shows::push(&app.pool, &user, &id, &ops).await?)))
}

#[derive(Deserialize)]
struct After {
    #[serde(default)]
    after: i64,
}

async fn pull(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>, Query(q): Query<After>) -> Answer {
    member(&app, &headers, &id, false).await?;
    Ok(Json(json!(shows::pull(&app.pool, &id, q.after).await?)))
}
