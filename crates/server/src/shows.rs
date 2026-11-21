//! Shows, who works on them, and their operation logs.
//!
//! An event's show is opened with its Swiss Equestrian key (decision 2026-11-14): the first
//! account to enter the key owns the show; other accounts with the key ask its owner for access
//! (to work on it as a collaborator) or for the ownership (the owner, accepting, stays as a
//! collaborator or leaves). The owner also gives access without a request, and takes it away; a
//! collaborator can leave. A show has one owner. A show without an event (a rehearsal) belongs
//! to the account that made it. The server asks Swiss Equestrian which event a key opens
//! (`getVeranstaltung`, `crate::se`) before anyone becomes an owner, and takes a new key Swiss
//! Equestrian accepts for the event; a show can also start from Swiss Equestrian's delivery
//! (the key alone: the event downloaded). Without Swiss Equestrian (`SE_SERVER=none`:
//! development) the app says which event (`Claim::event_id`), unchecked.
//!
//! Archives (decided 2026-11-15): the owner archives a show, or their account does it on its
//! own some weeks after the show's last day (`archive_after_weeks`; checked whenever someone
//! lists their shows). Archived is how every member's library lists the show, nothing more. A
//! show the owner took out of the archives is not archived on its own again.
//!
//! An operation is stored once (by its id): pushing the same again changes nothing. They are
//! kept in the order they arrived (`seq`); an app remembers the last `seq` it has and asks for
//! what came after. Pushes to one show are taken one at a time, so a later `seq` is never
//! visible before an earlier one (no operation slips behind an app's cursor). The server checks
//! that each operation is one the show core reads; which apply and which are set aside is the
//! same everywhere (the reconciliation rules), so none is refused for that.

use deadpool_postgres::{GenericClient, Pool};
use serde::Serialize;
use show::oplog::{Change, Operation};
use tokio_postgres::error::SqlState;

use crate::auth::{random_hex, User};
use crate::db::Error;
use crate::lang::Lang;

/// Most operations in one pull (an app asks again while `more`).
pub const PULL_LIMIT: i64 = 2000;
/// Most operations in one push.
pub const PUSH_LIMIT: usize = 5000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShowSummary {
    pub id: String,
    pub name: String,
    pub event_id: Option<i64>,
    /// `owner` or `collaborator`.
    pub role: String,
    /// The owner's name.
    pub owner: String,
    /// Requests waiting for an answer (counted for the owner only).
    pub requests: i64,
    pub operations: i64,
    /// The latest operation's `seq` (an app compares it with its cursor).
    pub cursor: i64,
    /// In the archives (every member's library lists it there).
    pub archived: bool,
    /// The show's last day (`YYYY-MM-DD`), once an app pushed its event.
    pub last_day: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pushed {
    /// New to the server.
    pub added: usize,
    /// It had them already.
    pub known: usize,
    pub cursor: i64,
    /// Not stored: stamped more than 5 minutes in the server's future (rule R5, the device's
    /// clock is wrong); the app sends them again later.
    pub refused: Vec<String>,
    /// The server's clock, milliseconds since 1970 (the app compares its own).
    pub now: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pulled {
    pub ops: Vec<Operation>,
    /// The `seq` of the last one given (the app's next `after`).
    pub cursor: i64,
    /// More after these.
    pub more: bool,
}

/// What a user may do on a show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    None,
    Collaborator,
    Owner,
}

/// An event's show asked for with its key.
#[derive(Debug, Clone)]
pub struct Claim {
    pub access_key: String,
    /// Swiss Equestrian's event id.
    pub event_id: i64,
    pub name: String,
    /// The event's number (e.g. 2026.8352), for the show's id.
    pub number: Option<String>,
    /// Swiss Equestrian accepted the key for this event (else the app's word, `SE_SERVER=none`):
    /// it replaces another key kept for the event.
    pub checked: bool,
    /// A show made now starts with these (Swiss Equestrian's delivery), by the one who claims it.
    pub ops: Vec<Operation>,
}

/// What entering an event's key gives.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Claimed {
    /// The first with this key: the show is made, theirs.
    Made { id: String },
    /// They work on it already.
    Member { id: String, role: String },
    /// Someone else owns it: ask them (`request`: their open request, if any).
    Owned { id: String, name: String, owner: String, request: Option<Box<Request>> },
    /// Not this event's key.
    WrongKey,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Person {
    pub id: i64,
    pub name: String,
    pub email: String,
}

/// Asking a show's owner for access or for the ownership.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Request {
    pub id: i64,
    pub show: String,
    pub show_name: String,
    /// `access` or `ownership`.
    pub kind: String,
    /// `pending`, `accepted`, `refused` or `withdrawn`.
    pub status: String,
    /// Who asks.
    pub person: Person,
    /// UTC, `YYYY-MM-DDTHH:MM:SSZ`.
    pub created_at: String,
    pub decided_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub role: String,
}

/// Someone to write to.
#[derive(Debug, Clone, PartialEq)]
pub struct Contact {
    pub name: String,
    pub email: String,
    pub lang: Lang,
}

/// Why a request, an answer or a change of members is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// Not this event's key.
    Key,
    /// They work on the show already (asking for access).
    Member,
    /// The owner cannot do that (ask for the ownership, be removed: hand the show over first).
    Owner,
    /// No such show or request (or not one's to answer).
    NotFound,
    /// Answered or withdrawn already.
    Answered,
    /// Accepting a transfer: the owner says whether they stay as a collaborator.
    Stay,
    /// No account for this address.
    Account,
}

impl Refused {
    /// For the apps.
    pub fn code(self) -> &'static str {
        match self {
            Refused::Key => "key",
            Refused::Member => "member",
            Refused::Owner => "owner",
            Refused::NotFound => "not_found",
            Refused::Answered => "answered",
            Refused::Stay => "stay",
            Refused::Account => "account",
        }
    }
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Refused::Key => "not this event's key",
            Refused::Member => "already works on this show",
            Refused::Owner => "the show's owner cannot do that (hand the show over first)",
            Refused::NotFound => "not found",
            Refused::Answered => "already answered or withdrawn",
            Refused::Stay => "say whether you stay on the show as a collaborator",
            Refused::Account => "no account for this address",
        })
    }
}

/// A database answer, or why not.
pub type Outcome<T> = Result<Result<T, Refused>, Error>;

const REQUEST_SELECT: &str = "SELECT r.id, r.show_id, s.name, r.kind, r.status, u.id, u.name, u.email,
        to_char(r.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"'),
        to_char(r.decided_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')
     FROM show_requests r JOIN shows s ON s.id = r.show_id JOIN users u ON u.id = r.user_id";

fn request_of(r: &tokio_postgres::Row) -> Request {
    Request {
        id: r.get(0),
        show: r.get(1),
        show_name: r.get(2),
        kind: r.get(3),
        status: r.get(4),
        person: Person { id: r.get(5), name: r.get(6), email: r.get(7) },
        created_at: r.get(8),
        decided_at: r.get(9),
    }
}

async fn request_by_id(client: &impl GenericClient, id: i64) -> Result<Option<Request>, Error> {
    Ok(client.query_opt(&format!("{REQUEST_SELECT} WHERE r.id = $1"), &[&id]).await?.as_ref().map(request_of))
}

/// A user's role on a show, if any.
async fn role_of(client: &impl GenericClient, show: &str, user: i64) -> Result<Option<String>, Error> {
    Ok(client.query_opt("SELECT role FROM show_members WHERE show_id = $1 AND user_id = $2", &[&show, &user]).await?.map(|r| r.get(0)))
}

pub async fn access(pool: &Pool, user: &User, show: &str) -> Result<Access, Error> {
    let client = pool.get().await?;
    Ok(match role_of(&client, show, user.id).await?.as_deref() {
        Some("owner") => Access::Owner,
        Some(_) => Access::Collaborator,
        // Administrators see every show (support), as a collaborator.
        None if user.admin && client.query_opt("SELECT 1 FROM shows WHERE id = $1", &[&show]).await?.is_some() => Access::Collaborator,
        None => Access::None,
    })
}

/// A show's id: from the event number, made unique.
fn new_id(number: Option<&str>) -> String {
    let base: String = number.unwrap_or("show").chars().filter(|c| c.is_ascii_alphanumeric() || *c == '.').take(20).collect();
    format!("{}-{}", if base.is_empty() { "show" } else { &base }, random_hex(4))
}

/// A show without a Swiss Equestrian event (a rehearsal), its maker its owner; its id.
pub async fn create(pool: &Pool, user: &User, name: &str, number: Option<&str>) -> Result<String, Error> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a show needs a name".into());
    }
    let id = new_id(number);
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    tx.execute("INSERT INTO shows (id, name, created_by) VALUES ($1, $2, $3)", &[&id, &name, &user.id]).await?;
    tx.execute("INSERT INTO show_members (show_id, user_id, role) VALUES ($1, $2, 'owner')", &[&id, &user.id]).await?;
    tx.commit().await?;
    Ok(id)
}

/// An event's show, with its key: made and owned by the first to enter it; for the others,
/// who owns it (to ask them).
pub async fn claim(pool: &Pool, user: &User, claim: &Claim) -> Result<Claimed, Error> {
    let key = claim.access_key.trim();
    if key.is_empty() {
        return Ok(Claimed::WrongKey);
    }
    let name = match claim.name.trim() {
        "" => format!("{}", claim.event_id),
        n => n.to_string(),
    };
    // Twice at most: someone else may make it at the same moment.
    for _ in 0..2 {
        let mut client = pool.get().await?;
        if let Some(row) = client.query_opt("SELECT id, name, access_key FROM shows WHERE event_id = $1", &[&claim.event_id]).await? {
            let (id, show_name, stored): (String, String, Option<String>) = (row.get(0), row.get(1), row.get(2));
            if stored.as_deref().map(str::trim) != Some(key) {
                if !claim.checked {
                    return Ok(Claimed::WrongKey);
                }
                // A new key of the event (Swiss Equestrian accepted it): the one kept from now on.
                client.execute("UPDATE shows SET access_key = $2 WHERE id = $1", &[&id, &key]).await?;
            }
            if let Some(role) = role_of(&client, &id, user.id).await? {
                return Ok(Claimed::Member { id, role });
            }
            let owner: String = client
                .query_opt("SELECT u.name FROM show_members m JOIN users u ON u.id = m.user_id WHERE m.show_id = $1 AND m.role = 'owner'", &[&id])
                .await?
                .map(|r| r.get(0))
                .unwrap_or_default();
            let request = client.query_opt(&format!("{REQUEST_SELECT} WHERE r.show_id = $1 AND r.user_id = $2 AND r.status = 'pending'"), &[&id, &user.id]).await?.as_ref().map(|r| Box::new(request_of(r)));
            return Ok(Claimed::Owned { id, name: show_name, owner, request });
        }
        let id = new_id(claim.number.as_deref());
        let tx = client.transaction().await?;
        let made = tx
            .query_opt(
                "INSERT INTO shows (id, name, event_id, access_key, created_by) VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (event_id) WHERE event_id IS NOT NULL DO NOTHING RETURNING id",
                &[&id, &name, &claim.event_id, &key, &user.id],
            )
            .await?;
        if made.is_some() {
            tx.execute("INSERT INTO show_members (show_id, user_id, role) VALUES ($1, $2, 'owner')", &[&id, &user.id]).await?;
            insert(&tx, &id, &claim.ops.iter().collect::<Vec<_>>(), user).await?;
            tx.commit().await?;
            return Ok(Claimed::Made { id });
        }
    }
    Err("the show was being made by someone else; try again".into())
}

/// The show of a Swiss Equestrian event, if there is one.
pub async fn of_event(pool: &Pool, event_id: i64) -> Result<Option<String>, Error> {
    let client = pool.get().await?;
    Ok(client.query_opt("SELECT id FROM shows WHERE event_id = $1", &[&event_id]).await?.map(|r| r.get(0)))
}

/// The shows a user works on (those their owners' accounts archive on their own, archived
/// first).
pub async fn list(pool: &Pool, user: &User) -> Result<Vec<ShowSummary>, Error> {
    let client = pool.get().await?;
    client
        .execute(
            "UPDATE shows s SET archived_at = now()
             FROM show_members o JOIN users u ON u.id = o.user_id
             WHERE o.show_id = s.id AND o.role = 'owner' AND u.archive_after_weeks IS NOT NULL
               AND s.archived_at IS NULL AND s.unarchived_at IS NULL AND s.last_day IS NOT NULL
               AND s.last_day + u.archive_after_weeks * 7 <= current_date
               AND s.id IN (SELECT show_id FROM show_members WHERE user_id = $1)",
            &[&user.id],
        )
        .await?;
    let rows = client
        .query(
            "SELECT s.id, s.name, s.event_id, m.role,
                    coalesce((SELECT u.name FROM show_members o JOIN users u ON u.id = o.user_id WHERE o.show_id = s.id AND o.role = 'owner'), ''),
                    CASE WHEN m.role = 'owner' THEN (SELECT count(*) FROM show_requests r WHERE r.show_id = s.id AND r.status = 'pending') ELSE 0 END,
                    (SELECT count(*) FROM operations o WHERE o.show_id = s.id),
                    coalesce((SELECT max(seq) FROM operations o WHERE o.show_id = s.id), 0),
                    s.archived_at IS NOT NULL, to_char(s.last_day, 'YYYY-MM-DD')
             FROM shows s JOIN show_members m ON m.show_id = s.id AND m.user_id = $1
             ORDER BY s.created_at DESC",
            &[&user.id],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| ShowSummary {
            id: r.get(0),
            name: r.get(1),
            event_id: r.get(2),
            role: r.get(3),
            owner: r.get(4),
            requests: r.get(5),
            operations: r.get(6),
            cursor: r.get(7),
            archived: r.get(8),
            last_day: r.get(9),
        })
        .collect())
}

/// The owner archives a show, or takes it out of the archives (then it is not archived on its
/// own again).
pub async fn archive(pool: &Pool, show: &str, archived: bool) -> Result<(), Error> {
    let client = pool.get().await?;
    client
        .execute(
            "UPDATE shows SET archived_at = CASE WHEN $2 THEN coalesce(archived_at, now()) END,
                              unarchived_at = CASE WHEN $2 THEN unarchived_at ELSE now() END
             WHERE id = $1",
            &[&show, &archived],
        )
        .await?;
    Ok(())
}

/// How many weeks after a show's last day the account archives the shows it owns (none: by
/// hand only).
pub async fn archive_after(pool: &Pool, user: &User) -> Result<Option<i32>, Error> {
    let client = pool.get().await?;
    Ok(client.query_one("SELECT archive_after_weeks FROM users WHERE id = $1", &[&user.id]).await?.get(0))
}

pub async fn set_archive_after(pool: &Pool, user: &User, weeks: Option<i32>) -> Result<(), Error> {
    if weeks.is_some_and(|w| !(1..=104).contains(&w)) {
        return Err("archive after 1 to 104 weeks".into());
    }
    let client = pool.get().await?;
    client.execute("UPDATE users SET archive_after_weeks = $2 WHERE id = $1", &[&user.id, &weeks]).await?;
    Ok(())
}

/// The last day the operations give: the event's, or a later day a class was moved to.
fn last_day(ops: &[&Operation]) -> Option<String> {
    let day = |d: &str| {
        let d = d.get(..10)?;
        let ok = d.bytes().enumerate().all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() });
        ok.then(|| d.to_string())
    };
    ops.iter()
        .filter_map(|o| match &o.change {
            Change::EventImported { event, .. } => day(&event.to),
            Change::ClassSet { class, .. } => day(&class.date),
            _ => None,
        })
        .max()
}

/// Who works on a show, the owner first.
pub async fn members(pool: &Pool, show: &str) -> Result<Vec<Member>, Error> {
    let client = pool.get().await?;
    let rows = client
        .query(
            "SELECT u.id, u.name, u.email, m.role FROM show_members m JOIN users u ON u.id = m.user_id
             WHERE m.show_id = $1 ORDER BY m.role = 'owner' DESC, u.name",
            &[&show],
        )
        .await?;
    Ok(rows.iter().map(|r| Member { id: r.get(0), name: r.get(1), email: r.get(2), role: r.get(3) }).collect())
}

/// A show's name and its owner (to write to).
pub async fn owner_of(pool: &Pool, show: &str) -> Result<Option<(String, Contact)>, Error> {
    let client = pool.get().await?;
    let row = client
        .query_opt(
            "SELECT s.name, u.name, u.email, u.language FROM shows s JOIN show_members m ON m.show_id = s.id AND m.role = 'owner' JOIN users u ON u.id = m.user_id WHERE s.id = $1",
            &[&show],
        )
        .await?;
    Ok(row.map(|r| (r.get(0), Contact { name: r.get(1), email: r.get(2), lang: Lang::of(r.get(3)) })))
}

/// Someone's contact, by account id.
pub async fn contact(pool: &Pool, user: i64) -> Result<Option<Contact>, Error> {
    let client = pool.get().await?;
    let row = client.query_opt("SELECT name, email, language FROM users WHERE id = $1", &[&user]).await?;
    Ok(row.map(|r| Contact { name: r.get(0), email: r.get(1), lang: Lang::of(r.get(2)) }))
}

/// Ask a show's owner for access (`ownership`: for the show itself), with the event's key. An
/// open request of the same person is replaced.
pub async fn request(pool: &Pool, user: &User, show: &str, ownership: bool, access_key: &str) -> Outcome<Request> {
    let client = pool.get().await?;
    let Some(row) = client.query_opt("SELECT access_key FROM shows WHERE id = $1", &[&show]).await? else {
        return Ok(Err(Refused::NotFound));
    };
    let stored: Option<String> = row.get(0);
    if stored.as_deref().map(str::trim).is_none_or(|k| k != access_key.trim()) {
        return Ok(Err(Refused::Key));
    }
    match role_of(&client, show, user.id).await?.as_deref() {
        Some("owner") => return Ok(Err(Refused::Owner)),
        Some(_) if !ownership => return Ok(Err(Refused::Member)),
        _ => {}
    }
    let kind = if ownership { "ownership" } else { "access" };
    for _ in 0..2 {
        let id = match client
            .query_opt("UPDATE show_requests SET kind = $3, created_at = now() WHERE show_id = $1 AND user_id = $2 AND status = 'pending' RETURNING id", &[&show, &user.id, &kind])
            .await?
        {
            Some(row) => row.get(0),
            None => match client.query_one("INSERT INTO show_requests (show_id, user_id, kind) VALUES ($1, $2, $3) RETURNING id", &[&show, &user.id, &kind]).await {
                Ok(row) => row.get(0),
                // Asked twice at the same moment: the other one is there now.
                Err(e) if e.code() == Some(&SqlState::UNIQUE_VIOLATION) => continue,
                Err(e) => return Err(e.into()),
            },
        };
        return Ok(Ok(request_by_id(&client, id).await?.ok_or("the request went missing")?));
    }
    Err("the request could not be recorded; try again".into())
}

/// The requests waiting for a show's owner, oldest first.
pub async fn pending(pool: &Pool, show: &str) -> Result<Vec<Request>, Error> {
    let client = pool.get().await?;
    Ok(client.query(&format!("{REQUEST_SELECT} WHERE r.show_id = $1 AND r.status = 'pending' ORDER BY r.created_at"), &[&show]).await?.iter().map(request_of).collect())
}

/// A person's own requests, the newest first (the last 50).
pub async fn requests_of(pool: &Pool, user: &User) -> Result<Vec<Request>, Error> {
    let client = pool.get().await?;
    Ok(client.query(&format!("{REQUEST_SELECT} WHERE r.user_id = $1 ORDER BY r.created_at DESC, r.id DESC LIMIT 50"), &[&user.id]).await?.iter().map(request_of).collect())
}

/// The owner answers a request. Accepting a transfer, they say whether they `stay` on the show
/// as a collaborator or leave it.
pub async fn decide(pool: &Pool, owner: &User, request: i64, accept: bool, stay: Option<bool>) -> Outcome<Request> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let Some(row) = tx.query_opt("SELECT show_id, user_id, kind, status FROM show_requests WHERE id = $1 FOR UPDATE", &[&request]).await? else {
        return Ok(Err(Refused::NotFound));
    };
    let (show, asker, kind, status): (String, i64, String, String) = (row.get(0), row.get(1), row.get(2), row.get(3));
    // One owner at a time decides (the show's owner row locked).
    if tx.query_opt("SELECT 1 FROM show_members WHERE show_id = $1 AND user_id = $2 AND role = 'owner' FOR UPDATE", &[&show, &owner.id]).await?.is_none() {
        return Ok(Err(Refused::NotFound));
    }
    if status != "pending" {
        return Ok(Err(Refused::Answered));
    }
    if accept && kind == "ownership" {
        let Some(stay) = stay else { return Ok(Err(Refused::Stay)) };
        // The owner steps down first: a show has one owner at every moment.
        if stay {
            tx.execute("UPDATE show_members SET role = 'collaborator' WHERE show_id = $1 AND user_id = $2", &[&show, &owner.id]).await?;
        } else {
            tx.execute("DELETE FROM show_members WHERE show_id = $1 AND user_id = $2", &[&show, &owner.id]).await?;
        }
        tx.execute(
            "INSERT INTO show_members (show_id, user_id, role) VALUES ($1, $2, 'owner') ON CONFLICT (show_id, user_id) DO UPDATE SET role = 'owner'",
            &[&show, &asker],
        )
        .await?;
    } else if accept {
        tx.execute("INSERT INTO show_members (show_id, user_id, role) VALUES ($1, $2, 'collaborator') ON CONFLICT (show_id, user_id) DO NOTHING", &[&show, &asker]).await?;
    }
    let status = if accept { "accepted" } else { "refused" };
    tx.execute("UPDATE show_requests SET status = $2, decided_at = now(), decided_by = $3 WHERE id = $1", &[&request, &status, &owner.id]).await?;
    tx.commit().await?;
    Ok(Ok(request_by_id(&client, request).await?.ok_or("the request went missing")?))
}

/// Who asked takes their request back.
pub async fn withdraw(pool: &Pool, user: &User, request: i64) -> Outcome<Request> {
    let client = pool.get().await?;
    let Some(row) = client.query_opt("SELECT status FROM show_requests WHERE id = $1 AND user_id = $2", &[&request, &user.id]).await? else {
        return Ok(Err(Refused::NotFound));
    };
    if row.get::<_, String>(0) != "pending" {
        return Ok(Err(Refused::Answered));
    }
    client.execute("UPDATE show_requests SET status = 'withdrawn', decided_at = now() WHERE id = $1 AND status = 'pending'", &[&request]).await?;
    Ok(Ok(request_by_id(&client, request).await?.ok_or("the request went missing")?))
}

/// The owner lets someone with an account work on the show, without a request (an open
/// request of theirs is answered with it).
pub async fn add_member(pool: &Pool, owner: &User, show: &str, email: &str) -> Outcome<Member> {
    let client = pool.get().await?;
    let Some(row) = client.query_opt("SELECT id, name, email FROM users WHERE email = $1", &[&email.trim().to_lowercase()]).await? else {
        return Ok(Err(Refused::Account));
    };
    let (id, name, email): (i64, String, String) = (row.get(0), row.get(1), row.get(2));
    client.execute("INSERT INTO show_members (show_id, user_id, role) VALUES ($1, $2, 'collaborator') ON CONFLICT (show_id, user_id) DO NOTHING", &[&show, &id]).await?;
    client
        .execute(
            "UPDATE show_requests SET status = 'accepted', decided_at = now(), decided_by = $3 WHERE show_id = $1 AND user_id = $2 AND status = 'pending' AND kind = 'access'",
            &[&show, &id, &owner.id],
        )
        .await?;
    let role = role_of(&client, show, id).await?.unwrap_or_default();
    Ok(Ok(Member { id, name, email, role }))
}

/// Take someone off a show: the owner removes a collaborator, a collaborator leaves. The owner
/// hands the show over first.
pub async fn remove_member(pool: &Pool, actor: &User, show: &str, member: i64) -> Outcome<()> {
    let client = pool.get().await?;
    let Some(role) = role_of(&client, show, member).await? else {
        return Ok(Err(Refused::NotFound));
    };
    if role == "owner" {
        return Ok(Err(Refused::Owner));
    }
    let owner = role_of(&client, show, actor.id).await?.as_deref() == Some("owner");
    if !owner && actor.id != member {
        return Ok(Err(Refused::NotFound));
    }
    client.execute("DELETE FROM show_members WHERE show_id = $1 AND user_id = $2", &[&show, &member]).await?;
    Ok(Ok(()))
}

/// Store an app's operations: the new ones added, the known ones skipped.
pub async fn push(pool: &Pool, user: &User, show: &str, ops: &[Operation]) -> Result<Pushed, Error> {
    if ops.len() > PUSH_LIMIT {
        return Err(format!("at most {PUSH_LIMIT} operations at a time").into());
    }
    // Rule R5: a device whose clock runs ahead would win every conflict.
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let refused = show::oplog::too_far_ahead(ops, now);
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    // One push at a time per show: `seq` follows the commits.
    tx.query_one("SELECT id FROM shows WHERE id = $1 FOR UPDATE", &[&show]).await?;
    let kept: Vec<&Operation> = ops.iter().filter(|o| !refused.contains(&o.id)).collect();
    let added = insert(&tx, show, &kept, user).await?;
    let cursor: i64 = tx.query_one("SELECT coalesce(max(seq), 0) FROM operations WHERE show_id = $1", &[&show]).await?.get(0);
    tx.commit().await?;
    Ok(Pushed { added, known: ops.len() - added - refused.len(), cursor, refused, now })
}

/// Operations made on the server (Swiss Equestrian's answers), by `user`: stored as a push is.
pub async fn add(pool: &Pool, user: &User, show: &str, ops: &[Operation]) -> Result<usize, Error> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    tx.query_one("SELECT id FROM shows WHERE id = $1 FOR UPDATE", &[&show]).await?;
    let added = insert(&tx, show, &ops.iter().collect::<Vec<_>>(), user).await?;
    tx.commit().await?;
    Ok(added)
}

/// Store operations the show does not have yet (and the show's last day they give); how many
/// were new.
async fn insert(tx: &deadpool_postgres::Transaction<'_>, show: &str, ops: &[&Operation], user: &User) -> Result<usize, Error> {
    let statement = tx.prepare("INSERT INTO operations (show_id, op_id, op, pushed_by) VALUES ($1, $2, $3, $4) ON CONFLICT (show_id, op_id) DO NOTHING").await?;
    if let Some(day) = last_day(ops) {
        tx.execute("UPDATE shows SET last_day = GREATEST(last_day, $2::text::date) WHERE id = $1", &[&show, &day]).await?;
    }
    let mut added = 0;
    for op in ops {
        if op.id.is_empty() {
            return Err("an operation without id".into());
        }
        let json = serde_json::to_value(op)?;
        added += tx.execute(&statement, &[&show, &op.id, &json, &user.id]).await? as usize;
    }
    Ok(added)
}

/// A show's whole log, in the order it arrived (the state is computed from it,
/// `show::State::from_log`).
pub async fn log(pool: &Pool, show: &str) -> Result<Vec<Operation>, Error> {
    let client = pool.get().await?;
    let rows = client.query("SELECT op FROM operations WHERE show_id = $1 ORDER BY seq", &[&show]).await?;
    Ok(rows.iter().map(|r| serde_json::from_value(r.get::<_, serde_json::Value>(0))).collect::<Result<Vec<Operation>, _>>()?)
}

/// The show has this operation (an app's, pushed).
pub async fn has(pool: &Pool, show: &str, op: &str) -> Result<bool, Error> {
    let client = pool.get().await?;
    Ok(client.query_opt("SELECT 1 FROM operations WHERE show_id = $1 AND op_id = $2", &[&show, &op]).await?.is_some())
}

/// A show's event and its key, as kept.
pub async fn event_key(pool: &Pool, show: &str) -> Result<Option<(i64, String)>, Error> {
    let client = pool.get().await?;
    let row = client.query_opt("SELECT event_id, access_key FROM shows WHERE id = $1", &[&show]).await?;
    Ok(row.and_then(|r| Some((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<String>>(1)?))))
}

/// The operations of a show after `after`, in the order they arrived.
pub async fn pull(pool: &Pool, show: &str, after: i64) -> Result<Pulled, Error> {
    let client = pool.get().await?;
    let rows = client.query("SELECT seq, op FROM operations WHERE show_id = $1 AND seq > $2 ORDER BY seq LIMIT $3", &[&show, &after, &(PULL_LIMIT + 1)]).await?;
    let more = rows.len() as i64 > PULL_LIMIT;
    let rows = &rows[..rows.len().min(PULL_LIMIT as usize)];
    let cursor = rows.last().map(|r| r.get::<_, i64>(0)).unwrap_or(after);
    let ops = rows.iter().map(|r| serde_json::from_value(r.get::<_, serde_json::Value>(1))).collect::<Result<Vec<Operation>, _>>()?;
    Ok(Pulled { ops, cursor, more })
}
