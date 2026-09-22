//! SQLite-backed TTL cache for everything read from upstream.
//!
//! The cache is an optimisation, never a source of truth: every database
//! failure is logged and treated as a miss, so deleting the file (or losing
//! it) only makes the app slower. Reads and writes go through
//! `spawn_blocking` so SQLite never blocks the async runtime.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::path::Path;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::db::Database;

/// Source of "now", injectable so tests can move time.
pub trait Clock: Send + Sync + fmt::Debug {
    /// Seconds since the Unix epoch.
    fn now(&self) -> i64;
}

/// Wall-clock time.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        jiff::Timestamp::now().as_second()
    }
}

/// A clock that only moves when told to. For tests.
#[derive(Debug, Default)]
pub struct ManualClock(AtomicI64);

impl ManualClock {
    pub fn starting_at(seconds: i64) -> Self {
        Self(AtomicI64::new(seconds))
    }

    pub fn advance(&self, by: Duration) {
        self.0.fetch_add(
            i64::try_from(by.as_secs()).unwrap_or(i64::MAX),
            Ordering::SeqCst,
        );
    }
}

impl Clock for ManualClock {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// What kind of thing is cached. Each namespace has its own TTLs, taken
/// from the plan (§5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Namespace {
    /// DID → identity (document + PDS).
    Identity,
    /// Handle → DID, for the lookup redirect.
    Handle,
    /// Publication records and per-repo publication lists.
    Publication,
    /// Pages of document listings.
    DocumentList,
    /// Individual document records.
    Document,
    /// Bluesky comment threads read from the `AppView`.
    BlueskyThread,
    /// Re-encoded cover images served by the proxy.
    Image,
    /// Place search results from Open Places, by query and point.
    PlaceSearch,
}

impl Namespace {
    /// How long a successful value stays fresh.
    pub fn ttl(self) -> Duration {
        const MINUTE: u64 = 60;
        const HOUR: u64 = 60 * MINUTE;
        const DAY: u64 = 24 * HOUR;
        Duration::from_secs(match self {
            Self::Identity => DAY,
            Self::Handle | Self::Document | Self::PlaceSearch => HOUR,
            Self::Publication => 15 * MINUTE,
            Self::DocumentList | Self::BlueskyThread => 5 * MINUTE,
            Self::Image => 6 * HOUR,
        })
    }

    /// How long a "not found" answer is remembered. Shorter than [`ttl`]
    /// for things that may appear soon; the same for lookups whose
    /// absence is stable.
    ///
    /// [`ttl`]: Self::ttl
    pub fn negative_ttl(self) -> Duration {
        const MINUTE: u64 = 60;
        Duration::from_secs(match self {
            Self::Identity => 10 * MINUTE,
            Self::Handle
            | Self::Publication
            | Self::DocumentList
            | Self::Document
            | Self::BlueskyThread => 5 * MINUTE,
            Self::Image | Self::PlaceSearch => 60 * MINUTE,
        })
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Handle => "handle",
            Self::Publication => "publication",
            Self::DocumentList => "document_list",
            Self::Document => "document",
            Self::BlueskyThread => "bluesky_thread",
            Self::Image => "image",
            Self::PlaceSearch => "place_search",
        }
    }
}

/// The cache handle. Cheap to clone; clones share one connection.
#[derive(Clone)]
pub struct Cache {
    db: Database,
    clock: Arc<dyn Clock>,
    /// Per-key locks so concurrent misses for the same key fetch once.
    inflight: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
}

impl fmt::Debug for Cache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cache")
            .field("clock", &self.clock)
            .finish_non_exhaustive()
    }
}

/// Raw row as stored.
struct Entry {
    /// `None` marks a negative entry.
    value: Option<Vec<u8>>,
}

impl Cache {
    /// Open (creating if needed) the database at `path` and run migrations.
    pub fn open(path: &Path, clock: Arc<dyn Clock>) -> rusqlite::Result<Self> {
        Ok(Self::new(Database::open(path)?, clock))
    }

    /// A private in-memory database. For tests.
    pub fn in_memory(clock: Arc<dyn Clock>) -> rusqlite::Result<Self> {
        Ok(Self::new(Database::in_memory()?, clock))
    }

    /// The cache over an already-open database.
    pub fn new(db: Database, clock: Arc<dyn Clock>) -> Self {
        Self {
            db,
            clock,
            inflight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// The database this cache lives in, for the other tables.
    pub fn database(&self) -> &Database {
        &self.db
    }

    /// The cache's clock.
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    /// Return the cached value for `key`, or run `fetch` and cache its
    /// result. `Ok(None)` from the fetcher is cached as a negative entry;
    /// an `Err` is returned to the caller and nothing is stored.
    ///
    /// Concurrent calls for the same key wait for the first to finish and
    /// then read its result, so a burst of requests costs one upstream call.
    pub async fn get_or_fetch<T, E, F, Fut>(
        &self,
        namespace: Namespace,
        key: &str,
        fetch: F,
    ) -> Result<Option<T>, E>
    where
        T: Serialize + DeserializeOwned + Send + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<T>, E>>,
    {
        // A cached value that no longer decodes (a serialization from an
        // older build) is evicted so the fetch below replaces it.
        if let Some(Some(bytes)) = self.lookup(namespace, key).await {
            match serde_json::from_slice(&bytes) {
                Ok(value) => return Ok(Some(value)),
                Err(err) => {
                    tracing::debug!(namespace = namespace.as_str(), key, %err, "discarding undecodable cache entry");
                    self.evict(namespace, key).await;
                }
            }
        }
        let bytes = self
            .get_or_fetch_bytes(namespace, key, || async {
                let fetched = fetch().await?;
                Ok(fetched.and_then(|value| match serde_json::to_vec(&value) {
                    Ok(bytes) => Some(bytes),
                    Err(err) => {
                        tracing::warn!(namespace = namespace.as_str(), key, %err, "value not cacheable");
                        None
                    }
                }))
            })
            .await?;
        Ok(bytes.and_then(|bytes| serde_json::from_slice(&bytes).ok()))
    }

    /// [`get_or_fetch`](Self::get_or_fetch) for opaque bytes (images).
    pub async fn get_or_fetch_bytes<E, F, Fut>(
        &self,
        namespace: Namespace,
        key: &str,
        fetch: F,
    ) -> Result<Option<Vec<u8>>, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<Vec<u8>>, E>>,
    {
        if let Some(hit) = self.lookup(namespace, key).await {
            return Ok(hit);
        }

        let lock = self.lock_for(namespace, key);
        let guard = lock.lock().await;
        // Another task may have filled the entry while we waited.
        let result = if let Some(hit) = self.lookup(namespace, key).await {
            Ok(hit)
        } else {
            let fetched = fetch().await;
            if let Ok(value) = &fetched {
                self.store(namespace, key, value.as_deref()).await;
            }
            fetched
        };
        drop(guard);
        self.release_lock(namespace, key, &lock);
        result
    }

    /// The cached value for `key`, without fetching. A negative entry or
    /// an undecodable one reads as absent.
    pub async fn peek<T: DeserializeOwned>(&self, namespace: Namespace, key: &str) -> Option<T> {
        let bytes = self.lookup(namespace, key).await??;
        serde_json::from_slice(&bytes).ok()
    }

    /// Store a value fetched by other means, with the namespace's TTL.
    pub async fn put<T: Serialize>(&self, namespace: Namespace, key: &str, value: &T) {
        match serde_json::to_vec(value) {
            Ok(bytes) => self.store(namespace, key, Some(&bytes)).await,
            Err(err) => {
                tracing::warn!(namespace = namespace.as_str(), key, %err, "value not cacheable");
            }
        }
    }

    /// Store bytes fetched by other means, replacing any negative entry.
    pub async fn put_bytes(&self, namespace: Namespace, key: &str, value: &[u8]) {
        // A fetch already in flight must not overwrite this value with a miss.
        let lock = self.lock_for(namespace, key);
        let guard = lock.lock().await;
        self.store(namespace, key, Some(value)).await;
        drop(guard);
        self.release_lock(namespace, key, &lock);
    }

    /// Forget one entry.
    pub async fn evict(&self, namespace: Namespace, key: &str) {
        let key = key.to_owned();
        let ns = namespace.as_str();
        self.with_conn("evict", move |conn| {
            conn.execute(
                "DELETE FROM cache WHERE namespace = ?1 AND key = ?2",
                params![ns, key],
            )
            .map(|_| ())
        })
        .await;
    }

    /// Forget every entry in `namespace` whose key starts with `prefix`.
    pub async fn evict_prefix(&self, namespace: Namespace, prefix: &str) {
        let pattern = format!("{}%", escape_like(prefix));
        let ns = namespace.as_str();
        self.with_conn("evict_prefix", move |conn| {
            conn.execute(
                "DELETE FROM cache WHERE namespace = ?1 AND key LIKE ?2 ESCAPE '\\'",
                params![ns, pattern],
            )
            .map(|_| ())
        })
        .await;
    }

    /// Delete expired rows. Returns how many were removed.
    pub async fn purge_expired(&self) -> usize {
        let now = self.clock.now();
        self.with_conn("purge", move |conn| {
            conn.execute("DELETE FROM cache WHERE expires_at <= ?1", params![now])
        })
        .await
        .unwrap_or(0)
    }

    /// Number of live rows. For tests and diagnostics.
    pub async fn len(&self) -> usize {
        let now = self.clock.now();
        self.with_conn("len", move |conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM cache WHERE expires_at > ?1",
                params![now],
                |row| row.get::<_, i64>(0),
            )
        })
        .await
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0)
    }

    /// Whether there are no live rows.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// `Some(Some(bytes))` hit, `Some(None)` negative hit, `None` miss.
    async fn lookup(&self, namespace: Namespace, key: &str) -> Option<Option<Vec<u8>>> {
        let now = self.clock.now();
        let owned_key = key.to_owned();
        let ns = namespace.as_str();
        let entry = self
            .with_conn("lookup", move |conn| {
                conn.query_row(
                    "SELECT value FROM cache WHERE namespace = ?1 AND key = ?2 AND expires_at > ?3",
                    params![ns, owned_key, now],
                    |row| Ok(Entry { value: row.get(0)? }),
                )
                .optional()
            })
            .await
            .flatten()?;
        Some(entry.value)
    }

    async fn store(&self, namespace: Namespace, key: &str, value: Option<&[u8]>) {
        let ttl = if value.is_some() {
            namespace.ttl()
        } else {
            namespace.negative_ttl()
        };
        let bytes = value.map(<[u8]>::to_vec);
        let now = self.clock.now();
        let expires_at = now + i64::try_from(ttl.as_secs()).unwrap_or(i64::MAX / 2);
        let key = key.to_owned();
        let ns = namespace.as_str();
        self.with_conn("store", move |conn| {
            conn.execute(
                "INSERT INTO cache (namespace, key, value, fetched_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(namespace, key) DO UPDATE SET
                   value = excluded.value,
                   fetched_at = excluded.fetched_at,
                   expires_at = excluded.expires_at",
                params![ns, key, bytes, now, expires_at],
            )
            .map(|_| ())
        })
        .await;
    }

    /// Run `f` on the connection off the async runtime. Errors are logged
    /// and become `None`: the cache must never fail a request.
    async fn with_conn<R, F>(&self, op: &'static str, f: F) -> Option<R>
    where
        R: Send + 'static,
        F: FnOnce(&Connection) -> rusqlite::Result<R> + Send + 'static,
    {
        match self.db.run(f).await {
            Ok(value) => Some(value),
            Err(err) => {
                tracing::warn!(op, %err, "cache database error; treating as miss");
                None
            }
        }
    }

    fn lock_for(&self, namespace: Namespace, key: &str) -> Arc<tokio::sync::Mutex<()>> {
        let full = format!("{}\u{0}{key}", namespace.as_str());
        let mut inflight = self
            .inflight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(inflight.entry(full).or_default())
    }

    fn release_lock(&self, namespace: Namespace, key: &str, lock: &Arc<tokio::sync::Mutex<()>>) {
        let full = format!("{}\u{0}{key}", namespace.as_str());
        let mut inflight = self
            .inflight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Only the map and this caller hold it: nobody is waiting.
        if Arc::strong_count(lock) <= 2 {
            inflight.remove(&full);
        }
    }
}

/// Escape `%`, `_`, and `\` for use inside a `LIKE ... ESCAPE '\'` pattern.
fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::sync::atomic::AtomicUsize;

    use super::*;

    fn cache() -> (Cache, Arc<ManualClock>) {
        let clock = Arc::new(ManualClock::starting_at(1_000_000));
        let cache = Cache::in_memory(Arc::clone(&clock) as Arc<dyn Clock>).unwrap();
        (cache, clock)
    }

    async fn counted_fetch(
        cache: &Cache,
        ns: Namespace,
        key: &str,
        calls: &AtomicUsize,
        value: Option<&str>,
    ) -> Option<String> {
        cache
            .get_or_fetch::<String, Infallible, _, _>(ns, key, || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(value.map(str::to_owned))
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn hit_after_fetch_until_expiry() {
        let (cache, clock) = cache();
        let calls = AtomicUsize::new(0);
        let a = counted_fetch(&cache, Namespace::Document, "k", &calls, Some("v")).await;
        let b = counted_fetch(&cache, Namespace::Document, "k", &calls, Some("other")).await;
        assert_eq!(a.as_deref(), Some("v"));
        assert_eq!(b.as_deref(), Some("v"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        clock.advance(Namespace::Document.ttl());
        let c = counted_fetch(&cache, Namespace::Document, "k", &calls, Some("fresh")).await;
        assert_eq!(c.as_deref(), Some("fresh"));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn negative_entries_short_circuit_and_expire_sooner() {
        let (cache, clock) = cache();
        let calls = AtomicUsize::new(0);
        assert!(
            counted_fetch(&cache, Namespace::Identity, "gone", &calls, None)
                .await
                .is_none()
        );
        assert!(
            counted_fetch(&cache, Namespace::Identity, "gone", &calls, Some("back"))
                .await
                .is_none()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        clock.advance(Namespace::Identity.negative_ttl());
        let v = counted_fetch(&cache, Namespace::Identity, "gone", &calls, Some("back")).await;
        assert_eq!(v.as_deref(), Some("back"));
    }

    #[tokio::test]
    async fn fetch_errors_are_not_cached() {
        let (cache, _) = cache();
        let calls = AtomicUsize::new(0);
        let attempt = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err::<Option<String>, &str>("upstream down")
        };
        assert_eq!(
            cache
                .get_or_fetch(Namespace::Publication, "k", attempt)
                .await,
            Err("upstream down")
        );
        assert_eq!(
            cache
                .get_or_fetch(Namespace::Publication, "k", attempt)
                .await,
            Err("upstream down")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(cache.is_empty().await);
    }

    #[tokio::test]
    async fn namespaces_do_not_collide() {
        let (cache, _) = cache();
        let calls = AtomicUsize::new(0);
        counted_fetch(&cache, Namespace::Document, "k", &calls, Some("doc")).await;
        let other = counted_fetch(&cache, Namespace::Publication, "k", &calls, Some("pub")).await;
        assert_eq!(other.as_deref(), Some("pub"));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn evict_and_evict_prefix_force_refetch() {
        let (cache, _) = cache();
        let calls = AtomicUsize::new(0);
        for k in ["did:a:1", "did:a:2", "did:b:1", "did_a:3"] {
            counted_fetch(&cache, Namespace::DocumentList, k, &calls, Some("x")).await;
        }
        assert_eq!(cache.len().await, 4);

        cache.evict(Namespace::DocumentList, "did:b:1").await;
        assert_eq!(cache.len().await, 3);

        // `_` in the prefix must match literally, not as a LIKE wildcard.
        cache.evict_prefix(Namespace::DocumentList, "did_a").await;
        assert_eq!(cache.len().await, 2);

        cache.evict_prefix(Namespace::DocumentList, "did:a").await;
        assert_eq!(cache.len().await, 0);

        counted_fetch(
            &cache,
            Namespace::DocumentList,
            "did:a:1",
            &calls,
            Some("x"),
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn purge_removes_only_expired_rows() {
        let (cache, clock) = cache();
        let calls = AtomicUsize::new(0);
        counted_fetch(&cache, Namespace::DocumentList, "short", &calls, Some("x")).await;
        counted_fetch(&cache, Namespace::Identity, "long", &calls, Some("x")).await;
        clock.advance(Namespace::DocumentList.ttl());
        assert_eq!(cache.purge_expired().await, 1);
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn concurrent_misses_fetch_once() {
        let (cache, _) = cache();
        let calls = Arc::new(AtomicUsize::new(0));
        let fetch = |calls: Arc<AtomicUsize>| async move {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok::<_, Infallible>(Some("shared".to_owned()))
        };
        let tasks: Vec<_> = (0..8)
            .map(|_| {
                let cache = cache.clone();
                let calls = Arc::clone(&calls);
                tokio::spawn(async move {
                    cache
                        .get_or_fetch::<String, Infallible, _, _>(Namespace::Document, "k", || {
                            fetch(calls)
                        })
                        .await
                        .unwrap()
                })
            })
            .collect();
        for task in tasks {
            assert_eq!(task.await.unwrap().as_deref(), Some("shared"));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn raw_bytes_round_trip_without_json() {
        let (cache, _) = cache();
        let payload = vec![0xffu8, 0xd8, 0x00, 0x01];
        let stored = cache
            .get_or_fetch_bytes::<Infallible, _, _>(Namespace::Image, "k", || async {
                Ok(Some(payload.clone()))
            })
            .await
            .unwrap();
        assert_eq!(stored.as_deref(), Some(&payload[..]));
        let again = cache
            .get_or_fetch_bytes::<Infallible, _, _>(Namespace::Image, "k", || async { Ok(None) })
            .await
            .unwrap();
        assert_eq!(again.as_deref(), Some(&payload[..]));
    }

    #[tokio::test]
    async fn undecodable_stored_value_is_a_miss() {
        let (cache, _) = cache();
        let calls = AtomicUsize::new(0);
        counted_fetch(&cache, Namespace::Document, "k", &calls, Some("text")).await;
        // Same key, different type: the stored JSON string does not decode as u32.
        let n = cache
            .get_or_fetch::<u32, Infallible, _, _>(Namespace::Document, "k", || async {
                Ok(Some(7))
            })
            .await
            .unwrap();
        assert_eq!(n, Some(7));
    }
}
