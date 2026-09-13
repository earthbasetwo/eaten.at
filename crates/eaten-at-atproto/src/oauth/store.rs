//! Where the OAuth client keeps its state between requests and restarts.
//!
//! Two kinds of entry: authorization *state* (the PKCE verifier and `DPoP`
//! key minted when a login starts, consumed by the callback) and
//! *sessions* (tokens and the `DPoP` key, keyed by DID). The application
//! supplies the storage; [`MemoryStore`] is for tests.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use atrium_common::store::Store;
use atrium_oauth::store::session::{Session, SessionStore};
use atrium_oauth::store::state::{InternalStateData, StateStore};

/// How long a started-but-unfinished login is remembered.
pub const STATE_TTL: Duration = Duration::from_secs(15 * 60);

/// Which table an entry belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// In-flight authorization requests, keyed by the `state` parameter.
    State,
    /// Established sessions, keyed by DID.
    Session,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::State => "oauth_state",
            Self::Session => "oauth_session",
        }
    }
}

/// A storage failure. The message is for logs; nothing parses it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("OAuth store: {0}")]
pub struct StoreError(pub String);

/// The future type store methods return.
pub type StoreFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, StoreError>> + Send + 'a>>;

/// Durable key-value storage for the OAuth client. Values are opaque
/// bytes (JSON, in practice); the client never needs to query them.
pub trait OAuthStore: Send + Sync + fmt::Debug {
    fn get<'a>(&'a self, kind: Kind, key: &'a str) -> StoreFuture<'a, Option<Vec<u8>>>;
    /// Store `value`; with a `ttl`, the entry disappears after that long.
    fn set<'a>(
        &'a self,
        kind: Kind,
        key: &'a str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> StoreFuture<'a, ()>;
    fn delete<'a>(&'a self, kind: Kind, key: &'a str) -> StoreFuture<'a, ()>;
    fn clear(&self, kind: Kind) -> StoreFuture<'_, ()>;
}

/// A stored value and when it stops being valid.
type Entry = (Vec<u8>, Option<Instant>);

/// An in-process store. State lives as long as the value does.
#[derive(Debug, Default)]
pub struct MemoryStore {
    entries: Mutex<HashMap<(Kind, String), Entry>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<(Kind, String), Entry>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl OAuthStore for MemoryStore {
    fn get<'a>(&'a self, kind: Kind, key: &'a str) -> StoreFuture<'a, Option<Vec<u8>>> {
        let value = self
            .lock()
            .get(&(kind, key.to_owned()))
            .filter(|(_, expires)| expires.is_none_or(|at| at > Instant::now()))
            .map(|(value, _)| value.clone());
        Box::pin(async move { Ok(value) })
    }

    fn set<'a>(
        &'a self,
        kind: Kind,
        key: &'a str,
        value: Vec<u8>,
        ttl: Option<Duration>,
    ) -> StoreFuture<'a, ()> {
        let expires = ttl.map(|ttl| Instant::now() + ttl);
        self.lock().insert((kind, key.to_owned()), (value, expires));
        Box::pin(async { Ok(()) })
    }

    fn delete<'a>(&'a self, kind: Kind, key: &'a str) -> StoreFuture<'a, ()> {
        self.lock().remove(&(kind, key.to_owned()));
        Box::pin(async { Ok(()) })
    }

    fn clear(&self, kind: Kind) -> StoreFuture<'_, ()> {
        self.lock().retain(|(k, _), _| *k != kind);
        Box::pin(async { Ok(()) })
    }
}

/// The library's view of the state table.
pub(super) struct StateStoreAdapter(pub(super) std::sync::Arc<dyn OAuthStore>);

impl Store<String, InternalStateData> for StateStoreAdapter {
    type Error = StoreError;

    async fn get(&self, key: &String) -> Result<Option<InternalStateData>, StoreError> {
        Ok(decode(self.0.get(Kind::State, key).await?))
    }

    async fn set(&self, key: String, value: InternalStateData) -> Result<(), StoreError> {
        let bytes = encode(&value)?;
        self.0.set(Kind::State, &key, bytes, Some(STATE_TTL)).await
    }

    async fn del(&self, key: &String) -> Result<(), StoreError> {
        self.0.delete(Kind::State, key).await
    }

    async fn clear(&self) -> Result<(), StoreError> {
        self.0.clear(Kind::State).await
    }
}

impl StateStore for StateStoreAdapter {}

/// The library's view of the session table.
pub(super) struct SessionStoreAdapter(pub(super) std::sync::Arc<dyn OAuthStore>);

impl Store<atrium_api::types::string::Did, Session> for SessionStoreAdapter {
    type Error = StoreError;

    async fn get(
        &self,
        key: &atrium_api::types::string::Did,
    ) -> Result<Option<Session>, StoreError> {
        Ok(decode(self.0.get(Kind::Session, key.as_str()).await?))
    }

    async fn set(
        &self,
        key: atrium_api::types::string::Did,
        value: Session,
    ) -> Result<(), StoreError> {
        let bytes = encode(&value)?;
        self.0.set(Kind::Session, key.as_str(), bytes, None).await
    }

    async fn del(&self, key: &atrium_api::types::string::Did) -> Result<(), StoreError> {
        self.0.delete(Kind::Session, key.as_str()).await
    }

    async fn clear(&self) -> Result<(), StoreError> {
        self.0.clear(Kind::Session).await
    }
}

impl SessionStore for SessionStoreAdapter {}

fn encode<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    serde_json::to_vec(value).map_err(|e| StoreError(format!("cannot encode entry: {e}")))
}

/// An entry that no longer decodes (written by an older build) is treated
/// as absent; the user signs in again rather than seeing an error forever.
fn decode<T: serde::de::DeserializeOwned>(bytes: Option<Vec<u8>>) -> Option<T> {
    bytes.and_then(|bytes| match serde_json::from_slice(&bytes) {
        Ok(value) => Some(value),
        Err(err) => {
            tracing::warn!(%err, "discarding undecodable OAuth store entry");
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_store_round_trips_and_expires() {
        let store = MemoryStore::new();
        store
            .set(Kind::State, "k", b"v".to_vec(), None)
            .await
            .unwrap();
        assert_eq!(
            store.get(Kind::State, "k").await.unwrap(),
            Some(b"v".to_vec())
        );
        assert_eq!(store.get(Kind::Session, "k").await.unwrap(), None);
        store
            .set(Kind::Session, "x", b"y".to_vec(), Some(Duration::ZERO))
            .await
            .unwrap();
        assert_eq!(store.get(Kind::Session, "x").await.unwrap(), None);
        store.delete(Kind::State, "k").await.unwrap();
        assert_eq!(store.get(Kind::State, "k").await.unwrap(), None);
    }
}
