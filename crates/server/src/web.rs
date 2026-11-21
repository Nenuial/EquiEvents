//! The online version (the user, 2026-11-21): a show office that never works offline works in
//! the browser or on a phone, at `/app/`, with the same interface as the desktop app.
//!
//! - **Signed in on the site** (`session`: its cookie). Without it, `/app/` sends to the sign-in.
//! - **One office per account** (`office::Office`, its show files under `OFFICES_DIR/<account>`):
//!   the interface's commands (`POST /app/api/<command>`) run there, as the desktop app runs
//!   them. It works straight on the master: each change goes up at once, what others did comes
//!   down before a command (`office::sync`, the online version); it reaches this server at
//!   `server` (its own address, `http://127.0.0.1:<port>`) with a token of the account.
//! - **Files** (a Rosson export imported, a show exported, a print): kept per account
//!   (`POST /app/files/<name>`, `GET /app/files/<n>/<name>`), and the commands' `path` only
//!   ever there. The office cannot sign in or out by itself, nor reach another server.
//! - **The interface** (`WEB_DIR`, the app's build): served under `/app/`; its page only when
//!   signed in.

use std::collections::HashMap;
use std::path::{Component, Path as FsPath, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::api::App;
use crate::auth::{self, User};
use crate::db::Error;
use crate::lang::Lang;
use crate::session;

/// The offices of the online version.
pub struct Web {
    /// `<root>/<account id>/library` (its shows), `…/files` (its files).
    root: PathBuf,
    /// How the offices reach this server.
    server: String,
    /// The interface (the app's build), if this server serves it.
    interface: Option<PathBuf>,
    offices: Mutex<HashMap<i64, Arc<office::Office>>>,
    kept: AtomicU64,
}

impl Web {
    pub fn new(root: PathBuf, server: &str, interface: Option<PathBuf>) -> Web {
        Web { root, server: server.trim_end_matches('/').to_string(), interface, offices: Mutex::default(), kept: AtomicU64::new(0) }
    }

    fn files(&self, user: &User) -> PathBuf {
        self.root.join(user.id.to_string()).join("files")
    }

    /// The account's office, made the first time (signed in with a token of the account, kept
    /// while valid).
    async fn office(&self, app: &App, user: &User) -> Result<Arc<office::Office>, Error> {
        if let Some(office) = self.offices.lock().unwrap_or_else(|e| e.into_inner()).get(&user.id) {
            return Ok(office.clone());
        }
        let library = self.root.join(user.id.to_string()).join("library");
        let device = format!("web-{}", user.id);
        let office = tokio::task::spawn_blocking(move || office::Office::new(&library, &device)).await??;
        let kept = office.token_for(&self.server);
        let token = match kept {
            Some(token) if auth::user_of(&app.pool, &token).await?.is_some_and(|u| u.id == user.id) => token,
            _ => auth::new_session(&app.pool, user.id, "equievents.ch (online version)").await?,
        };
        let account = office::sync::Account { server: self.server.clone(), id: user.id, name: user.name.clone(), email: user.email.clone(), language: user.language.clone() };
        office.host(&self.server, account, &token)?;
        let office = Arc::new(office);
        Ok(self.offices.lock().unwrap_or_else(|e| e.into_inner()).entry(user.id).or_insert(office).clone())
    }
}

pub fn routes() -> Router<App> {
    Router::new()
        .route("/app", get(|| async { Redirect::permanent("/app/") }))
        .route("/app/", get(interface_page))
        .route("/app/{file}", get(interface_file))
        .route("/app/assets/{file}", get(asset))
        .route("/app/api/{command}", post(command))
        // A Rosson export of a whole database can be large.
        .route("/app/files/{name}", post(keep).layer(DefaultBodyLimit::max(256 * 1024 * 1024)))
        .route("/app/files/{folder}/{name}", get(kept))
}

/// The signed-in account, from the site's cookie.
async fn signed_in(app: &App, headers: &HeaderMap) -> Result<Option<User>, Error> {
    match session::token(headers) {
        Some(token) => auth::user_of(&app.pool, &token).await,
        None => Ok(None),
    }
}

/// A request of the page itself, not of another site (the cookie is the account).
fn same_site(headers: &HeaderMap) -> bool {
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    if let Some(site) = header("sec-fetch-site") {
        return site == "same-origin";
    }
    match (header("origin"), header("host")) {
        (Some(origin), Some(host)) => origin.split("://").nth(1) == Some(host),
        (None, _) => true,
        _ => false,
    }
}

fn refused(status: StatusCode, error: &str, code: &str) -> Response {
    (status, Json(json!({ "error": error, "code": code }))).into_response()
}

/// The interface's page: signed in, else the sign-in, back here after.
async fn interface_page(State(app): State<App>, headers: HeaderMap) -> Response {
    let Some(web) = app.web.clone() else { return StatusCode::NOT_FOUND.into_response() };
    match signed_in(&app, &headers).await {
        Ok(Some(_)) => {}
        Ok(None) => return Redirect::to(&session::sign_in_path(session::lang_of(&headers), "/app/")).into_response(),
        Err(e) => return crate::site::server_error(Lang::Fr, e),
    }
    serve(web.interface.as_deref(), "index.html", "no-store").await
}

async fn interface_file(State(app): State<App>, Path(file): Path<String>) -> Response {
    let Some(web) = app.web.clone() else { return StatusCode::NOT_FOUND.into_response() };
    serve(web.interface.as_deref(), &file, "no-cache").await
}

async fn asset(State(app): State<App>, Path(file): Path<String>) -> Response {
    let Some(web) = app.web.clone() else { return StatusCode::NOT_FOUND.into_response() };
    // Their names change with their content (Vite): kept a year.
    serve(web.interface.as_ref().map(|d| d.join("assets")).as_deref(), &file, "public, max-age=31536000, immutable").await
}

/// A file of the interface (one name, nothing below or above).
async fn serve(dir: Option<&FsPath>, name: &str, cache: &str) -> Response {
    let Some(dir) = dir else { return StatusCode::NOT_FOUND.into_response() };
    if !plain(name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(bytes) = tokio::fs::read(dir.join(name)).await else { return StatusCode::NOT_FOUND.into_response() };
    let kind = match name.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "json" | "webmanifest" => "application/json",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        _ => "application/octet-stream",
    };
    ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, cache), (header::X_CONTENT_TYPE_OPTIONS, "nosniff")], bytes).into_response()
}

/// One name: no folder, nothing hidden.
fn plain(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\']) && name != ".."
}

/// A command of the interface, run by the account's office.
async fn command(State(app): State<App>, headers: HeaderMap, Path(command): Path<String>, body: Bytes) -> Response {
    let Some(web) = app.web.clone() else { return StatusCode::NOT_FOUND.into_response() };
    if !same_site(&headers) {
        return refused(StatusCode::FORBIDDEN, "not from equievents.ch", "origin");
    }
    let user = match signed_in(&app, &headers).await {
        Ok(Some(user)) => user,
        Ok(None) => return refused(StatusCode::UNAUTHORIZED, "not signed in", "signed_out"),
        Err(e) => return db_error(e),
    };
    let args: Value = if body.is_empty() { json!({}) } else { serde_json::from_slice(&body).unwrap_or(Value::Null) };
    // A file only among the account's own.
    if let Some(path) = args.get("path").and_then(Value::as_str) {
        if !within(&web.files(&user), FsPath::new(path)) {
            return refused(StatusCode::BAD_REQUEST, "a file the online version does not have", "path");
        }
    }
    let office = match web.office(&app, &user).await {
        Ok(office) => office,
        Err(e) => return db_error(e),
    };
    let answer = tokio::task::spawn_blocking(move || office.call(&command, &args)).await;
    match answer {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        Err(e) => db_error(e.to_string().into()),
    }
}

fn db_error(e: Error) -> Response {
    eprintln!("error: {e}");
    refused(StatusCode::INTERNAL_SERVER_ERROR, "the server could not do it", "error")
}

/// `path` lies in `dir` (no `..`, nothing elsewhere).
fn within(dir: &FsPath, path: &FsPath) -> bool {
    path.starts_with(dir) && path.components().all(|c| !matches!(c, Component::ParentDir | Component::CurDir))
}

/// A file the browser gives (empty: one the office is to write), kept among the account's:
/// `{path, url}`.
async fn keep(State(app): State<App>, headers: HeaderMap, Path(name): Path<String>, body: Bytes) -> Response {
    let Some(web) = app.web.clone() else { return StatusCode::NOT_FOUND.into_response() };
    if !same_site(&headers) {
        return refused(StatusCode::FORBIDDEN, "not from equievents.ch", "origin");
    }
    let user = match signed_in(&app, &headers).await {
        Ok(Some(user)) => user,
        Ok(None) => return refused(StatusCode::UNAUTHORIZED, "not signed in", "signed_out"),
        Err(e) => return db_error(e),
    };
    let name = file_name(&name);
    let folder = format!("{}-{}", crate::se::now_millis(), web.kept.fetch_add(1, Ordering::Relaxed));
    let dir = web.files(&user).join(&folder);
    let path = dir.join(&name);
    let written = async {
        tokio::fs::create_dir_all(&dir).await?;
        if !body.is_empty() {
            tokio::fs::write(&path, &body).await?;
        }
        Ok::<_, std::io::Error>(())
    };
    if let Err(e) = written.await {
        return db_error(e.into());
    }
    Json(json!({ "path": path, "url": format!("/app/files/{folder}/{}", encode(&name)) })).into_response()
}

/// A kept file of the account (a print opens in the browser's viewer under its own name).
async fn kept(State(app): State<App>, headers: HeaderMap, Path((folder, name)): Path<(String, String)>) -> Response {
    let Some(web) = app.web.clone() else { return StatusCode::NOT_FOUND.into_response() };
    let user = match signed_in(&app, &headers).await {
        Ok(Some(user)) => user,
        Ok(None) => return StatusCode::UNAUTHORIZED.into_response(),
        Err(e) => return db_error(e),
    };
    if !plain(&folder) || !plain(&name) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(bytes) = tokio::fs::read(web.files(&user).join(&folder).join(&name)).await else { return StatusCode::NOT_FOUND.into_response() };
    let kind = if name.to_lowercase().ends_with(".pdf") { "application/pdf" } else { "application/octet-stream" };
    let disposition = format!("inline; filename*=UTF-8''{}", encode(&name));
    Response::builder()
        .header(header::CONTENT_TYPE, kind)
        .header(header::CONTENT_DISPOSITION, disposition)
        .header(header::CACHE_CONTROL, "private, no-store")
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// The name of a file without any folder in it.
fn file_name(name: &str) -> String {
    let name = name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if name.is_empty() || name.starts_with('.') {
        "file".into()
    } else {
        name.into()
    }
}

/// A file's name in a URL's path.
fn encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}
