//! Accounts, for administrators and support (the user, 2026-11-21), in the online version only
//! (`/app/admin/…`, signed in on the site):
//!
//! - **Support** sees the accounts and acts as one (to help it: its events, as it): its
//!   session works for that account in the online version, its changes signed with both names
//!   (`web`), until it comes back to its own. Never as an administrator or another support
//!   account.
//! - **Administrators** do the same, and administer: a new password (typed or made: shown once;
//!   the account signed out everywhere), a reset link by e-mail (when e-mail is on), the roles
//!   (administrator, support; never their own), deleting an account with all its data: the shows
//!   it owns released (deleted for everyone, who worked on them told), taken off the others, its
//!   riders and horses, its online version's files. Its changes in others' shows stay in their
//!   history.
//!
//! What staff may do is always the session's own account's, never the one it acts as.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::api::App;
use crate::auth::{self, User};
use crate::db::Error;
use crate::{notices, session, shows};

pub fn routes() -> Router<App> {
    Router::new()
        .route("/app/admin/me", get(me))
        .route("/app/admin/stop", post(stop))
        .route("/app/admin/accounts", get(accounts))
        .route("/app/admin/accounts/{id}", get(account).delete(delete))
        .route("/app/admin/accounts/{id}/act", post(act))
        .route("/app/admin/accounts/{id}/password", post(password))
        .route("/app/admin/accounts/{id}/reset-link", post(reset_link))
        .route("/app/admin/accounts/{id}/role", post(role))
}

/// An account as the panel lists it.
#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub email: String,
    /// `admin`, `support` or empty.
    pub role: String,
    pub created_at: String,
    /// When one of its sessions was last used (UTC), if ever.
    pub last_seen: Option<String>,
    /// Shows it owns, shows it works on as a collaborator.
    pub owns: i64,
    pub works_on: i64,
}

const ACCOUNT_SELECT: &str = "SELECT u.id, u.name, u.email,
        CASE WHEN u.admin THEN 'admin' WHEN u.support THEN 'support' ELSE '' END,
        to_char(u.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),
        (SELECT to_char(max(s.last_used) AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') FROM sessions s WHERE s.user_id = u.id),
        (SELECT count(*) FROM show_members m WHERE m.user_id = u.id AND m.role = 'owner'),
        (SELECT count(*) FROM show_members m WHERE m.user_id = u.id AND m.role = 'collaborator')
     FROM users u";

fn account_of(r: &tokio_postgres::Row) -> Account {
    Account { id: r.get(0), name: r.get(1), email: r.get(2), role: r.get(3), created_at: r.get(4), last_seen: r.get(5), owns: r.get(6), works_on: r.get(7) }
}

/// The accounts whose name or e-mail holds `query` (all when empty), by name; 500 at most.
pub async fn list(pool: &deadpool_postgres::Pool, query: &str) -> Result<Vec<Account>, Error> {
    let client = pool.get().await?;
    let like = format!("%{}%", query.trim().replace(['%', '_', '\\'], ""));
    let rows = client.query(&format!("{ACCOUNT_SELECT} WHERE u.name ILIKE $1 OR u.email ILIKE $1 ORDER BY lower(u.name), u.id LIMIT 500"), &[&like]).await?;
    Ok(rows.iter().map(account_of).collect())
}

/// The session's own account (what it may do here) and its token, refused unless staff
/// (`admin`: administrators only) and from this site.
async fn staff(app: &App, headers: &HeaderMap, admin: bool) -> Result<(User, String), Response> {
    if !crate::web::same_site(headers) {
        return Err(refused(StatusCode::FORBIDDEN, "not from equievents.ch", "origin"));
    }
    let Some(token) = session::token(headers) else { return Err(refused(StatusCode::UNAUTHORIZED, "not signed in", "signed_out")) };
    let me = match auth::session_of(&app.pool, &token).await {
        Ok(Some((me, _))) => me,
        Ok(None) => return Err(refused(StatusCode::UNAUTHORIZED, "not signed in", "signed_out")),
        Err(e) => return Err(failed(e)),
    };
    let allowed = if admin { me.admin } else { me.admin || me.support };
    if !allowed {
        return Err(refused(StatusCode::FORBIDDEN, if admin { "only an administrator can" } else { "only administrators and support can" }, "not_staff"));
    }
    Ok((me, token))
}

fn refused(status: StatusCode, error: &str, code: &str) -> Response {
    (status, Json(json!({ "error": error, "code": code }))).into_response()
}

fn failed(e: Error) -> Response {
    eprintln!("error: {e}");
    refused(StatusCode::INTERNAL_SERVER_ERROR, "the server could not do it", "error")
}

/// The account `id`, or 404.
async fn target(app: &App, id: i64) -> Result<User, Response> {
    match auth::user(&app.pool, id).await {
        Ok(Some(user)) => Ok(user),
        Ok(None) => Err(refused(StatusCode::NOT_FOUND, "no such account", "not_found")),
        Err(e) => Err(failed(e)),
    }
}

/// Who the session is, what it may do here, and whom it acts as (any signed-in session: the
/// online version asks, to show the panel or the banner).
async fn me(State(app): State<App>, headers: HeaderMap) -> Response {
    let Some(token) = session::token(&headers) else { return refused(StatusCode::UNAUTHORIZED, "not signed in", "signed_out") };
    match auth::session_of(&app.pool, &token).await {
        Ok(Some((me, acting))) => {
            let role = if me.admin { "admin" } else if me.support { "support" } else { "" };
            Json(json!({ "id": me.id, "name": me.name, "role": role, "acting_as": acting.map(|a| json!({ "id": a.id, "name": a.name, "email": a.email })) })).into_response()
        }
        Ok(None) => refused(StatusCode::UNAUTHORIZED, "not signed in", "signed_out"),
        Err(e) => failed(e),
    }
}

#[derive(Deserialize)]
struct Search {
    #[serde(default)]
    q: String,
}

async fn accounts(State(app): State<App>, headers: HeaderMap, Query(search): Query<Search>) -> Response {
    if let Err(r) = staff(&app, &headers, false).await {
        return r;
    }
    match list(&app.pool, &search.q).await {
        Ok(accounts) => Json(json!(accounts)).into_response(),
        Err(e) => failed(e),
    }
}

/// One account: the shows it owns (who else works on each), those it works on.
async fn account(State(app): State<App>, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    if let Err(r) = staff(&app, &headers, false).await {
        return r;
    }
    let found = async {
        let client = app.pool.get().await?;
        let Some(row) = client.query_opt(&format!("{ACCOUNT_SELECT} WHERE u.id = $1"), &[&id]).await? else { return Ok::<_, Error>(None) };
        let owns = client
            .query(
                "SELECT s.id, s.name, (SELECT count(*) FROM show_members o WHERE o.show_id = s.id AND o.role = 'collaborator')
                 FROM shows s JOIN show_members m ON m.show_id = s.id AND m.user_id = $1 AND m.role = 'owner' ORDER BY s.created_at DESC",
                &[&id],
            )
            .await?
            .iter()
            .map(|r| json!({ "id": r.get::<_, String>(0), "name": r.get::<_, String>(1), "collaborators": r.get::<_, i64>(2) }))
            .collect::<Vec<_>>();
        let works_on = client
            .query(
                "SELECT s.id, s.name, coalesce((SELECT u.name FROM show_members o JOIN users u ON u.id = o.user_id WHERE o.show_id = s.id AND o.role = 'owner'), '')
                 FROM shows s JOIN show_members m ON m.show_id = s.id AND m.user_id = $1 AND m.role = 'collaborator' ORDER BY s.created_at DESC",
                &[&id],
            )
            .await?
            .iter()
            .map(|r| json!({ "id": r.get::<_, String>(0), "name": r.get::<_, String>(1), "owner": r.get::<_, String>(2) }))
            .collect::<Vec<_>>();
        Ok(Some(json!({ "account": account_of(&row), "owns": owns, "works_on": works_on })))
    };
    match found.await {
        Ok(Some(v)) => Json(v).into_response(),
        Ok(None) => refused(StatusCode::NOT_FOUND, "no such account", "not_found"),
        Err(e) => failed(e),
    }
}

/// Act as an account in the online version (support: never as staff).
async fn act(State(app): State<App>, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let (me, token) = match staff(&app, &headers, false).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let account = match target(&app, id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    if account.id == me.id {
        return refused(StatusCode::BAD_REQUEST, "that is your own account", "self");
    }
    if (account.admin || account.support) && !me.admin {
        return refused(StatusCode::FORBIDDEN, "only an administrator can act as an administrator or support account", "not_staff");
    }
    if let Err(e) = auth::act_as(&app.pool, &token, Some(account.id)).await {
        return failed(e);
    }
    println!("{} acts as {}", me.email, account.email);
    Json(json!({ "ok": true, "acting_as": { "id": account.id, "name": account.name, "email": account.email } })).into_response()
}

/// Back to one's own account.
async fn stop(State(app): State<App>, headers: HeaderMap) -> Response {
    if !crate::web::same_site(&headers) {
        return refused(StatusCode::FORBIDDEN, "not from equievents.ch", "origin");
    }
    let Some(token) = session::token(&headers) else { return refused(StatusCode::UNAUTHORIZED, "not signed in", "signed_out") };
    match auth::act_as(&app.pool, &token, None).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => failed(e),
    }
}

#[derive(Deserialize)]
struct NewPassword {
    /// None: one is made (and shown once).
    password: Option<String>,
}

/// A new password for an account (administrators): typed, or made and shown once. The account
/// is signed out everywhere.
async fn password(State(app): State<App>, headers: HeaderMap, Path(id): Path<i64>, Json(body): Json<NewPassword>) -> Response {
    let (me, _) = match staff(&app, &headers, true).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let account = match target(&app, id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let made = body.password.is_none();
    let password = body.password.unwrap_or_else(auth::new_password);
    if let Err(e) = auth::check_password(&password) {
        return refused(StatusCode::BAD_REQUEST, &e.to_string(), e.code());
    }
    if let Err(e) = auth::set_password(&app.pool, &account.email, &password).await {
        return failed(e);
    }
    if let Some(web) = &app.web {
        web.forget(account.id);
    }
    println!("{} set a new password for {}", me.email, account.email);
    Json(json!({ "ok": true, "password": made.then_some(password) })).into_response()
}

/// A reset link by e-mail (administrators), as the site sends it; sent only when e-mail is on.
async fn reset_link(State(app): State<App>, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let (me, _) = match staff(&app, &headers, true).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let account = match target(&app, id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    if let Err(e) = crate::site::send_reset_link(&app, account.lang(), &account.email).await {
        return failed(e);
    }
    println!("{} sent a reset link to {}", me.email, account.email);
    // Whether it left: e-mail off (kept, not sent), all to the test address, or sent.
    let mail = match app.mailer.mode {
        crate::mail::Mode::Off => "off",
        crate::mail::Mode::Redirect(_) => "redirect",
        crate::mail::Mode::Send => "on",
    };
    Json(json!({ "ok": true, "mail": mail })).into_response()
}

#[derive(Deserialize)]
struct Role {
    /// `admin`, `support` or `none`.
    role: String,
}

/// An account's role (administrators, never their own).
async fn role(State(app): State<App>, headers: HeaderMap, Path(id): Path<i64>, Json(body): Json<Role>) -> Response {
    let (me, _) = match staff(&app, &headers, true).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if id == me.id {
        return refused(StatusCode::BAD_REQUEST, "an administrator does not change their own role", "self");
    }
    let (admin, support) = match body.role.as_str() {
        "admin" => (true, false),
        "support" => (false, true),
        "none" => (false, false),
        other => return refused(StatusCode::BAD_REQUEST, &format!("role {other}: admin, support or none"), "role"),
    };
    let done = async {
        let client = app.pool.get().await?;
        Ok::<_, Error>(client.execute("UPDATE users SET admin = $2, support = $3 WHERE id = $1", &[&id, &admin, &support]).await? == 1)
    };
    match done.await {
        Ok(true) => {
            println!("{} made account {id} {}", me.email, body.role);
            Json(json!({ "ok": true })).into_response()
        }
        Ok(false) => refused(StatusCode::NOT_FOUND, "no such account", "not_found"),
        Err(e) => failed(e),
    }
}

/// Delete an account and all its data (administrators, never their own): the shows it owns
/// released (deleted for everyone, who worked on them told), its memberships, sessions, riders
/// and horses, its online version's files.
async fn delete(State(app): State<App>, headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let (me, _) = match staff(&app, &headers, true).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if id == me.id {
        return refused(StatusCode::BAD_REQUEST, "an administrator does not delete their own account", "self");
    }
    let account = match target(&app, id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let deleted = async {
        let owned: Vec<String> = app.pool.get().await?.query("SELECT show_id FROM show_members WHERE user_id = $1 AND role = 'owner'", &[&id]).await?.iter().map(|r| r.get(0)).collect();
        let mut released = 0;
        for show in owned {
            if let Ok((name, others)) = shows::release(&app.pool, &account, &show).await? {
                released += 1;
                for to in others {
                    crate::api::notify(&app, &to.email, notices::account_deleted(to.lang, &to.name, &account.name, &name)).await;
                }
            }
        }
        app.pool.get().await?.execute("DELETE FROM users WHERE id = $1", &[&id]).await?;
        Ok::<_, Error>(released)
    };
    match deleted.await {
        Ok(released) => {
            if let Some(web) = &app.web {
                web.remove(id).await;
            }
            println!("{} deleted the account {} ({released} shows released)", me.email, account.email);
            Json(json!({ "ok": true, "released": released })).into_response()
        }
        Err(e) => failed(e),
    }
}

