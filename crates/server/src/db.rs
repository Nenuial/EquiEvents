//! The database: a pool of connections to Postgres, and the migrations, applied in order and
//! remembered in `migrations` (each once; a changed migration is not run again).

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;

/// The migrations, in order: (version, SQL).
const MIGRATIONS: [(&str, &str); 5] = [
    ("0001_init", include_str!("../migrations/0001_init.sql")),
    ("0002_password_resets", include_str!("../migrations/0002_password_resets.sql")),
    ("0003_owners_and_requests", include_str!("../migrations/0003_owners_and_requests.sql")),
    ("0004_archives", include_str!("../migrations/0004_archives.sql")),
    ("0005_registries", include_str!("../migrations/0005_registries.sql")),
];

pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// A pool for `url` (`postgres://user:password@host:port/db`, or Postgres' key=value form).
pub fn pool(url: &str) -> Result<Pool, Error> {
    let config: tokio_postgres::Config = url.parse()?;
    let manager = Manager::from_config(config, NoTls, ManagerConfig { recycling_method: RecyclingMethod::Fast });
    Ok(Pool::builder(manager).max_size(16).build()?)
}

/// Apply the migrations not applied yet; the versions applied now.
pub async fn migrate(pool: &Pool) -> Result<Vec<String>, Error> {
    let mut client = pool.get().await?;
    client.batch_execute("CREATE TABLE IF NOT EXISTS migrations (version text PRIMARY KEY, applied_at timestamptz NOT NULL DEFAULT now())").await?;
    let mut applied = Vec::new();
    for (version, sql) in MIGRATIONS {
        let tx = client.transaction().await?;
        // One migration at a time, also when two containers start together.
        tx.batch_execute("LOCK TABLE migrations IN EXCLUSIVE MODE").await?;
        if tx.query_opt("SELECT 1 FROM migrations WHERE version = $1", &[&version]).await?.is_none() {
            tx.batch_execute(sql).await?;
            tx.execute("INSERT INTO migrations (version) VALUES ($1)", &[&version]).await?;
            applied.push(version.to_string());
        }
        tx.commit().await?;
    }
    Ok(applied)
}
