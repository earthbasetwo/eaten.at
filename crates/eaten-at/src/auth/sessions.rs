//! Browser sessions: a random token, handed to the browser in a cookie,
//! mapped to a DID for a fixed lifetime. Only a hash of the token is
//! stored, so the table alone cannot impersonate anyone.

use std::sync::Arc;
use std::time::Duration;

use eaten_at_atproto::identity::Did;
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::cache::Clock;
use crate::db::{Database, DbError};

/// How long a sign-in lasts before the user must sign in again.
pub const SESSION_LIFETIME: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// The session table.
#[derive(Debug, Clone)]
pub struct WebSessions {
    db: Database,
    clock: Arc<dyn Clock>,
}

impl WebSessions {
    pub fn new(db: Database, clock: Arc<dyn Clock>) -> Self {
        Self { db, clock }
    }

    /// Start a session for `did`. Returns the token for the cookie.
    pub async fn create(&self, did: &Did) -> Result<String, DbError> {
        let token = hex(&rand::random::<[u8; 32]>());
        let hash = hash(&token);
        let now = self.clock.now();
        let expires_at = now + i64::try_from(SESSION_LIFETIME.as_secs()).unwrap_or(i64::MAX / 2);
        let did = did.as_str().to_owned();
        self.db
            .run(move |conn| {
                conn.execute(
                    "INSERT INTO web_sessions (token_hash, did, created_at, expires_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![hash, did, now, expires_at],
                )
                .map(|_| ())
            })
            .await?;
        Ok(token)
    }

    /// The DID a live token belongs to. A database failure is logged and
    /// reads as signed out, so the site keeps working for readers.
    pub async fn lookup(&self, token: &str) -> Option<Did> {
        let hash = hash(token);
        let now = self.clock.now();
        let found = self
            .db
            .run(move |conn| {
                conn.query_row(
                    "SELECT did FROM web_sessions WHERE token_hash = ?1 AND expires_at > ?2",
                    params![hash, now],
                    |row| row.get::<_, String>(0),
                )
                .optional()
            })
            .await;
        match found {
            Ok(did) => did.and_then(|d| Did::parse(&d).ok()),
            Err(err) => {
                tracing::warn!(%err, "session lookup failed; treating as signed out");
                None
            }
        }
    }

    /// End a session.
    pub async fn delete(&self, token: &str) -> Result<(), DbError> {
        let hash = hash(token);
        self.db
            .run(move |conn| {
                conn.execute(
                    "DELETE FROM web_sessions WHERE token_hash = ?1",
                    params![hash],
                )
                .map(|_| ())
            })
            .await
    }

    /// Delete expired rows. Returns how many were removed.
    pub async fn purge_expired(&self) -> usize {
        let now = self.clock.now();
        self.db
            .run(move |conn| {
                conn.execute(
                    "DELETE FROM web_sessions WHERE expires_at <= ?1",
                    params![now],
                )
            })
            .await
            .unwrap_or(0)
    }
}

fn hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::ManualClock;

    fn sessions() -> (WebSessions, Arc<ManualClock>) {
        let clock = Arc::new(ManualClock::starting_at(1_000_000));
        let sessions = WebSessions::new(Database::in_memory().unwrap(), clock.clone());
        (sessions, clock)
    }

    #[tokio::test]
    async fn create_lookup_delete() {
        let (sessions, _) = sessions();
        let did = Did::parse("did:plc:re3ebnp5v7ffagz6rb6xfei4").unwrap();
        let token = sessions.create(&did).await.unwrap();
        assert_eq!(token.len(), 64);
        assert_eq!(sessions.lookup(&token).await, Some(did.clone()));
        assert_eq!(sessions.lookup("not-a-token").await, None);
        let other = sessions.create(&did).await.unwrap();
        assert_ne!(token, other, "tokens are unique");
        sessions.delete(&token).await.unwrap();
        assert_eq!(sessions.lookup(&token).await, None);
        assert_eq!(sessions.lookup(&other).await, Some(did));
    }

    #[tokio::test]
    async fn sessions_expire_and_are_purged() {
        let (sessions, clock) = sessions();
        let did = Did::parse("did:plc:re3ebnp5v7ffagz6rb6xfei4").unwrap();
        let token = sessions.create(&did).await.unwrap();
        clock.advance(
            SESSION_LIFETIME
                .checked_sub(Duration::from_secs(1))
                .unwrap(),
        );
        assert_eq!(sessions.lookup(&token).await, Some(did));
        clock.advance(Duration::from_secs(1));
        assert_eq!(sessions.lookup(&token).await, None);
        assert_eq!(sessions.purge_expired().await, 1);
        assert_eq!(sessions.purge_expired().await, 0);
    }

    #[test]
    fn only_a_hash_is_stored() {
        assert_ne!(hash("abc"), "abc");
        assert_eq!(hash("abc").len(), 64);
        assert_eq!(hex(&[0, 255, 16]), "00ff10");
    }
}
