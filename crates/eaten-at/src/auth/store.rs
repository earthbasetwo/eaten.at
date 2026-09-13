//! The OAuth client's storage, in the `oauth_kv` table.

use std::sync::Arc;
use std::time::Duration;

use eaten_at_atproto::oauth::{Kind, OAuthStore, StoreError, StoreFuture};
use rusqlite::{params, OptionalExtension};

use crate::cache::Clock;
use crate::db::Database;

/// [`OAuthStore`] over SQLite.
#[derive(Debug, Clone)]
pub struct SqliteOAuthStore {
    db: Database,
    clock: Arc<dyn Clock>,
}

impl SqliteOAuthStore {
    pub fn new(db: Database, clock: Arc<dyn Clock>) -> Self {
        Self { db, clock }
    }

    /// Delete expired rows. Returns how many were removed.
    pub async fn purge_expired(&self) -> usize {
        let now = self.clock.now();
        self.db
            .run(move |conn| {
                conn.execute(
                    "DELETE FROM oauth_kv WHERE expires_at IS NOT NULL AND expires_at <= ?1",
                    params![now],
                )
            })
            .await
            .unwrap_or(0)
    }
}

fn store_error(err: impl std::fmt::Display) -> StoreError {
    StoreError(err.to_string())
}

impl OAuthStore for SqliteOAuthStore {
    fn get<'a>(&'a self, kind: Kind, key: &'a str) -> StoreFuture<'a, Option<Vec<u8>>> {
        let key = key.to_owned();
        let now = self.clock.now();
        Box::pin(async move {
            self.db
                .run(move |conn| {
                    conn.query_row(
                        "SELECT value FROM oauth_kv
                         WHERE kind = ?1 AND key = ?2 AND (expires_at IS NULL OR expires_at > ?3)",
                        params![kind.as_str(), key, now],
                        |row| row.get::<_, Vec<u8>>(0),
                    )
                    .optional()
                })
                .await
                .map_err(store_error)
        })
    }

    fn set<'a>(
        &'a self,
        kind: Kind,
        key: &'a str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> StoreFuture<'a, ()> {
        let key = key.to_owned();
        let expires_at =
            ttl.map(|ttl| self.clock.now() + i64::try_from(ttl.as_secs()).unwrap_or(i64::MAX / 2));
        Box::pin(async move {
            self.db
                .run(move |conn| {
                    conn.execute(
                        "INSERT INTO oauth_kv (kind, key, value, expires_at)
                         VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT(kind, key) DO UPDATE SET
                           value = excluded.value,
                           expires_at = excluded.expires_at",
                        params![kind.as_str(), key, value, expires_at],
                    )
                    .map(|_| ())
                })
                .await
                .map_err(store_error)
        })
    }

    fn delete<'a>(&'a self, kind: Kind, key: &'a str) -> StoreFuture<'a, ()> {
        let key = key.to_owned();
        Box::pin(async move {
            self.db
                .run(move |conn| {
                    conn.execute(
                        "DELETE FROM oauth_kv WHERE kind = ?1 AND key = ?2",
                        params![kind.as_str(), key],
                    )
                    .map(|_| ())
                })
                .await
                .map_err(store_error)
        })
    }

    fn clear(&self, kind: Kind) -> StoreFuture<'_, ()> {
        Box::pin(async move {
            self.db
                .run(move |conn| {
                    conn.execute(
                        "DELETE FROM oauth_kv WHERE kind = ?1",
                        params![kind.as_str()],
                    )
                    .map(|_| ())
                })
                .await
                .map_err(store_error)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::ManualClock;

    #[tokio::test]
    async fn round_trip_ttl_and_clear() {
        let clock = Arc::new(ManualClock::starting_at(1_000));
        let store = SqliteOAuthStore::new(Database::in_memory().unwrap(), clock.clone());
        store
            .set(
                Kind::State,
                "s1",
                b"state".to_vec(),
                Some(Duration::from_secs(60)),
            )
            .await
            .unwrap();
        store
            .set(Kind::Session, "did:plc:x", b"sess".to_vec(), None)
            .await
            .unwrap();
        assert_eq!(
            store.get(Kind::State, "s1").await.unwrap(),
            Some(b"state".to_vec())
        );
        assert_eq!(
            store.get(Kind::Session, "s1").await.unwrap(),
            None,
            "kinds are separate"
        );
        store
            .set(Kind::Session, "did:plc:x", b"sess2".to_vec(), None)
            .await
            .unwrap();
        assert_eq!(
            store.get(Kind::Session, "did:plc:x").await.unwrap(),
            Some(b"sess2".to_vec())
        );

        clock.advance(Duration::from_secs(60));
        assert_eq!(
            store.get(Kind::State, "s1").await.unwrap(),
            None,
            "state expired"
        );
        assert_eq!(
            store.get(Kind::Session, "did:plc:x").await.unwrap(),
            Some(b"sess2".to_vec())
        );
        assert_eq!(store.purge_expired().await, 1);

        store.delete(Kind::Session, "did:plc:x").await.unwrap();
        assert_eq!(store.get(Kind::Session, "did:plc:x").await.unwrap(), None);
        store.set(Kind::State, "a", vec![1], None).await.unwrap();
        store.set(Kind::State, "b", vec![2], None).await.unwrap();
        store.clear(Kind::State).await.unwrap();
        assert_eq!(store.get(Kind::State, "a").await.unwrap(), None);
    }
}
