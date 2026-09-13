//! The one SQLite file (D16): the read cache, OAuth state, and web
//! sessions share a connection and a migration sequence.
//!
//! Every query runs under `spawn_blocking` so SQLite never blocks the
//! async runtime. Callers decide what a failure means: the cache treats
//! one as a miss, the session tables surface it.

use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

/// Current schema version. Bump with every migration added to [`migrate`].
const SCHEMA_VERSION: i64 = 3;

/// A database failure.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("database task failed: {0}")]
    Join(String),
}

/// Handle to the database. Cheap to clone; clones share one connection.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Database")
    }
}

impl Database {
    /// Open (creating if needed) the database at `path` and run migrations.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    /// A private in-memory database. For tests.
    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> rusqlite::Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Run `f` on the connection off the async runtime.
    pub async fn run<R, F>(&self, f: F) -> Result<R, DbError>
    where
        R: Send + 'static,
        F: FnOnce(&Connection) -> rusqlite::Result<R> + Send + 'static,
    {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || {
            let guard = conn
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            f(&guard)
        })
        .await
        .map_err(|e| DbError::Join(e.to_string()))?
        .map_err(DbError::from)
    }
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS cache (
                namespace  TEXT    NOT NULL,
                key        TEXT    NOT NULL,
                value      BLOB,
                fetched_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                PRIMARY KEY (namespace, key)
            ) WITHOUT ROWID;
            CREATE INDEX IF NOT EXISTS cache_expires ON cache (expires_at);",
        )?;
    }
    if version < 2 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS oauth_kv (
                kind       TEXT    NOT NULL,
                key        TEXT    NOT NULL,
                value      BLOB    NOT NULL,
                expires_at INTEGER,
                PRIMARY KEY (kind, key)
            ) WITHOUT ROWID;
            CREATE TABLE IF NOT EXISTS web_sessions (
                token_hash TEXT    NOT NULL PRIMARY KEY,
                did        TEXT    NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL
            ) WITHOUT ROWID;
            CREATE INDEX IF NOT EXISTS web_sessions_expires ON web_sessions (expires_at);",
        )?;
    }
    if version < 3 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS subdomain_claims (
                name            TEXT    NOT NULL PRIMARY KEY,
                publication_uri TEXT    NOT NULL UNIQUE,
                did             TEXT    NOT NULL,
                created_at      INTEGER NOT NULL
            ) WITHOUT ROWID;
            CREATE TABLE IF NOT EXISTS origin_moves (
                old_host        TEXT    NOT NULL PRIMARY KEY,
                new_origin      TEXT    NOT NULL,
                moved_at        INTEGER NOT NULL
            ) WITHOUT ROWID;",
        )?;
    }
    if version < SCHEMA_VERSION {
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrates_to_the_current_version_and_runs_queries() {
        let db = Database::in_memory().unwrap();
        let version: i64 = db
            .run(|conn| conn.pragma_query_value(None, "user_version", |row| row.get(0)))
            .await
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let tables: Vec<String> = db
            .run(|conn| {
                conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")?
                    .query_map([], |row| row.get(0))?
                    .collect()
            })
            .await
            .unwrap();
        assert_eq!(
            tables,
            [
                "cache",
                "oauth_kv",
                "origin_moves",
                "subdomain_claims",
                "web_sessions"
            ]
        );
        // Migrating again is a no-op.
        db.run(migrate).await.unwrap();
    }
}
