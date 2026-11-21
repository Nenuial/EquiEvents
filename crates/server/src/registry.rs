//! Swiss Equestrian's riders and horses, per account (the user, 2026-11-21): the server downloads
//! them with the key of one of the account's shows (`se::Se::registry`) and keeps them for the
//! account, shared by all its shows and computers; the apps keep a copy to search offline. The
//! riders carry their contact data: only their account's apps get them.

use deadpool_postgres::Pool;
use serde::Serialize;
use serde_json::Value;

use crate::auth::User;
use crate::db::Error;

/// How many, of when (milliseconds since 1970), without the lists.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct About {
    pub persons: i32,
    pub horses: i32,
    pub at: Option<i64>,
}

/// Keep an account's riders and horses (the previous ones replaced).
pub async fn store(pool: &Pool, user: &User, persons: &[Value], horses: &[Value]) -> Result<About, Error> {
    let client = pool.get().await?;
    let (p, h) = (serde_json::to_value(persons)?, serde_json::to_value(horses)?);
    let (np, nh) = (persons.len() as i32, horses.len() as i32);
    client
        .execute(
            "INSERT INTO registries (user_id, persons, horses, persons_count, horses_count) VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (user_id) DO UPDATE SET persons = $2, horses = $3, persons_count = $4, horses_count = $5, downloaded_at = now()",
            &[&user.id, &p, &h, &np, &nh],
        )
        .await?;
    about(pool, user).await
}

pub async fn about(pool: &Pool, user: &User) -> Result<About, Error> {
    let client = pool.get().await?;
    let row = client
        .query_opt("SELECT persons_count, horses_count, (extract(epoch FROM downloaded_at) * 1000)::bigint FROM registries WHERE user_id = $1", &[&user.id])
        .await?;
    Ok(row.map_or(About { persons: 0, horses: 0, at: None }, |r| About { persons: r.get(0), horses: r.get(1), at: Some(r.get(2)) }))
}

/// The account's riders and horses, and of when (`None`: never downloaded).
pub async fn get(pool: &Pool, user: &User) -> Result<Option<(Value, Value, i64)>, Error> {
    let client = pool.get().await?;
    let row = client.query_opt("SELECT persons, horses, (extract(epoch FROM downloaded_at) * 1000)::bigint FROM registries WHERE user_id = $1", &[&user.id]).await?;
    Ok(row.map(|r| (r.get(0), r.get(1), r.get(2))))
}
