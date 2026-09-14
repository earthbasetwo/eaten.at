//! A small in-memory rate limiter (plan 12): a token bucket per key,
//! for the place-suggestion endpoint, whose every call can spend Open
//! Places quota. One process, one map; a restart forgets it, which is
//! fine for a limit that exists to stop a runaway keyboard.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Entries kept before the oldest full ones are dropped.
const MAX_KEYS: usize = 10_000;

#[derive(Debug)]
pub struct RateLimiter {
    /// Tokens a full bucket holds, and how many refill per minute.
    per_minute: u32,
    buckets: Mutex<HashMap<String, Bucket>>,
}

#[derive(Debug, Clone, Copy)]
struct Bucket {
    tokens: f64,
    refilled: Instant,
}

impl RateLimiter {
    pub fn per_minute(per_minute: u32) -> Self {
        Self {
            per_minute,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Whether `key` may make a request now.
    pub fn allow(&self, key: &str) -> bool {
        self.allow_at(key, Instant::now())
    }

    /// [`allow`](Self::allow) at a given instant, for tests.
    pub fn allow_at(&self, key: &str, now: Instant) -> bool {
        let capacity = f64::from(self.per_minute);
        let per_second = capacity / 60.0;
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if buckets.len() >= MAX_KEYS && !buckets.contains_key(key) {
            // Full buckets are idle ones; dropping them costs nothing.
            buckets.retain(|_, b| b.tokens < capacity - 1.0);
        }
        let bucket = buckets.entry(key.to_owned()).or_insert(Bucket {
            tokens: capacity,
            refilled: now,
        });
        let elapsed = now.saturating_duration_since(bucket.refilled);
        bucket.tokens = (bucket.tokens + elapsed.as_secs_f64() * per_second).min(capacity);
        bucket.refilled = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// For tests: how long until `key` may go again, roughly.
    #[cfg(test)]
    fn wait(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(60.0 / f64::from(self.per_minute))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bucket_empties_then_refills_with_time() {
        let limiter = RateLimiter::per_minute(3);
        let start = Instant::now();
        assert!(limiter.allow_at("a", start));
        assert!(limiter.allow_at("a", start));
        assert!(limiter.allow_at("a", start));
        assert!(!limiter.allow_at("a", start), "empty");
        assert!(limiter.allow_at("b", start), "keys are separate");
        assert!(!limiter.allow_at("a", start + limiter.wait() / 2));
        assert!(limiter.allow_at("a", start + limiter.wait()));
        assert!(!limiter.allow_at("a", start + limiter.wait()));
        // A long rest fills the bucket back up, but no further.
        let later = start + std::time::Duration::from_secs(600);
        assert!(limiter.allow_at("a", later));
        assert!(limiter.allow_at("a", later));
        assert!(limiter.allow_at("a", later));
        assert!(!limiter.allow_at("a", later));
    }
}
