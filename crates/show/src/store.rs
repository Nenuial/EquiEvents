//! Storage of a show: one SQLite file holding its operation log.
//!
//! The log is the truth; the state is rebuilt from it when the file is opened and kept up to
//! date as operations are appended. Appending is idempotent (an operation id is stored once), so
//! receiving the same operation twice during sync is harmless. When an operation arrives that
//! sorts before ones already applied (it was made offline earlier), the state is rebuilt so all
//! devices end up with the same result.
//!
//! The file also knows which operations equievents.ch has (`synced`: received from it, or sent
//! and stored there): the others wait in the file until they go up, also across restarts. And
//! a few values of its own (`meta`): which show it is on the server, how far it has pulled.

use std::path::Path;

use rusqlite::{params, Connection, OpenFlags};

use crate::oplog::{Operation, Stamp};
use crate::state::{Outcome, State};

/// Schema version, in SQLite's `user_version`. 2: which operations the server has, `meta`.
const SCHEMA_VERSION: i64 = 2;

pub struct Store {
    conn: Connection,
    state: State,
    ignored: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct StoreError(pub String);

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError(e.to_string())
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError(e.to_string())
    }
}

impl Store {
    /// Open (or create) a show file and rebuild its state from the log.
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        let conn = Connection::open(path)?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == 0 {
            conn.execute_batch(
                "CREATE TABLE operations (
                    id      TEXT PRIMARY KEY,
                    millis  INTEGER NOT NULL,
                    counter INTEGER NOT NULL,
                    device  TEXT NOT NULL,
                    author  TEXT NOT NULL,
                    change  TEXT NOT NULL  -- JSON
                 );
                 CREATE INDEX operations_by_stamp ON operations (millis, counter, device);
                 PRAGMA user_version = 1;",
            )?;
        }
        if version <= 1 {
            conn.execute_batch(
                "BEGIN;
                 ALTER TABLE operations ADD COLUMN synced INTEGER NOT NULL DEFAULT 0;
                 CREATE INDEX operations_unsynced ON operations (synced) WHERE synced = 0;
                 CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 PRAGMA user_version = 2;
                 COMMIT;",
            )?;
        } else if version != SCHEMA_VERSION {
            return Err(StoreError(format!("show file has schema version {version}, this app knows {SCHEMA_VERSION}")));
        }
        let ops = Self::read_log(&conn)?;
        let (state, ignored) = State::from_log(ops);
        Ok(Store { conn, state, ignored })
    }

    fn read_log(conn: &Connection) -> Result<Vec<Operation>, StoreError> {
        let mut stmt = conn.prepare("SELECT id, millis, counter, device, author, change FROM operations ORDER BY millis, counter, device")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?))
        })?;
        let mut ops = Vec::new();
        for row in rows {
            let (id, millis, counter, device, author, change) = row?;
            ops.push(Operation { id, stamp: Stamp { millis: millis as u64, counter: counter as u32, device }, author, change: serde_json::from_str(&change)? });
        }
        Ok(ops)
    }

    /// Append operations made here (or imported): they wait to go up to the server. Already
    /// known ids are skipped.
    pub fn append(&mut self, ops: &[Operation]) -> Result<(), StoreError> {
        self.add(ops, false).map(|_| ())
    }

    /// Operations received from the server: stored as the server's (known ones too); the new
    /// ones applied. How many were new.
    pub fn receive(&mut self, ops: &[Operation]) -> Result<usize, StoreError> {
        self.add(ops, true)
    }

    fn add(&mut self, ops: &[Operation], synced: bool) -> Result<usize, StoreError> {
        let tx = self.conn.transaction()?;
        let mut new = Vec::new();
        for op in ops {
            let inserted = tx.execute(
                "INSERT OR IGNORE INTO operations (id, millis, counter, device, author, change, synced) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![op.id, op.stamp.millis as i64, op.stamp.counter as i64, op.stamp.device, op.author, serde_json::to_string(&op.change)?, synced],
            )?;
            if inserted == 1 {
                new.push(op.clone());
            } else if synced {
                tx.execute("UPDATE operations SET synced = 1 WHERE id = ?1", params![op.id])?;
            }
        }
        tx.commit()?;
        let added = new.len();

        new.sort_by(|a, b| a.stamp.cmp(&b.stamp));
        // Rebuild when an operation sorts before applied ones, or when a class is finalised
        // (a finalisation can make already applied operations late, rule R4).
        let in_order = match (&self.state.last, new.first()) {
            (Some(last), Some(first)) => first.stamp > *last,
            _ => true,
        } && !new.iter().any(|o| matches!(o.change, crate::oplog::Change::ClassFinalised { .. }));
        if in_order {
            for op in &new {
                if let Outcome::Ignored(why) = self.state.apply(op) {
                    self.ignored.push((op.id.clone(), why));
                }
            }
        } else {
            let (state, ignored) = State::from_log(Self::read_log(&self.conn)?);
            self.state = state;
            self.ignored = ignored;
        }
        Ok(added)
    }

    /// Operations the server does not have yet, the oldest first (at most `limit`).
    pub fn unsynced(&self, limit: usize) -> Result<Vec<Operation>, StoreError> {
        let mut stmt = self.conn.prepare("SELECT id, millis, counter, device, author, change FROM operations WHERE synced = 0 ORDER BY millis, counter, device LIMIT ?1")?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?))
        })?;
        let mut ops = Vec::new();
        for row in rows {
            let (id, millis, counter, device, author, change) = row?;
            ops.push(Operation { id, stamp: Stamp { millis: millis as u64, counter: counter as u32, device }, author, change: serde_json::from_str(&change)? });
        }
        Ok(ops)
    }

    /// How many operations wait to go up.
    pub fn unsynced_count(&self) -> Result<usize, StoreError> {
        Ok(self.conn.query_row("SELECT count(*) FROM operations WHERE synced = 0", [], |r| r.get::<_, i64>(0))? as usize)
    }

    /// The server has these now.
    pub fn mark_synced(&mut self, ids: &[String]) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        for id in ids {
            tx.execute("UPDATE operations SET synced = 1 WHERE id = ?1", params![id])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// All of them wait to go up again (the show goes to another server's show).
    pub fn mark_all_unsynced(&mut self) -> Result<(), StoreError> {
        self.conn.execute("UPDATE operations SET synced = 0", [])?;
        Ok(())
    }

    /// One of the file's own values.
    pub fn meta(&self, key: &str) -> Result<Option<String>, StoreError> {
        let mut stmt = self.conn.prepare("SELECT value FROM meta WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        Ok(match rows.next()? {
            Some(row) => Some(row.get(0)?),
            None => None,
        })
    }

    /// Set (or, `None`, remove) one of the file's own values.
    pub fn set_meta(&mut self, key: &str, value: Option<&str>) -> Result<(), StoreError> {
        match value {
            Some(v) => self.conn.execute("INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = ?2", params![key, v])?,
            None => self.conn.execute("DELETE FROM meta WHERE key = ?1", params![key])?,
        };
        Ok(())
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    /// Operations that were not applied, with the reason (e.g. a change to a final class).
    pub fn ignored(&self) -> &[(String, String)] {
        &self.ignored
    }

    /// The whole log, in stamp order.
    pub fn log(&self) -> Result<Vec<Operation>, StoreError> {
        Self::read_log(&self.conn)
    }

    /// How many operations the show has (it only grows: a summary made at this count still holds).
    pub fn operation_count(&self) -> Result<usize, StoreError> {
        Ok(self.conn.query_row("SELECT count(*) FROM operations", [], |r| r.get::<_, i64>(0))? as usize)
    }

    /// Who made the first operation (an import says how it was imported).
    pub fn first_author(&self) -> Result<Option<String>, StoreError> {
        let mut stmt = self.conn.prepare("SELECT author FROM operations ORDER BY millis, counter, device LIMIT 1")?;
        let mut rows = stmt.query([])?;
        Ok(match rows.next()? {
            Some(row) => Some(row.get(0)?),
            None => None,
        })
    }
}

/// A look into a show file without rebuilding its state (listing the library): how many
/// operations it has, and its own values (a summary kept there).
pub struct Peek {
    conn: Connection,
}

impl Peek {
    pub fn open(path: &Path) -> Result<Peek, StoreError> {
        Ok(Peek { conn: Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)? })
    }

    pub fn operation_count(&self) -> Result<usize, StoreError> {
        Ok(self.conn.query_row("SELECT count(*) FROM operations", [], |r| r.get::<_, i64>(0))? as usize)
    }

    /// One of the file's own values (none in a file from before they existed).
    pub fn meta(&self, key: &str) -> Option<String> {
        self.conn.query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| r.get(0)).ok()
    }
}
