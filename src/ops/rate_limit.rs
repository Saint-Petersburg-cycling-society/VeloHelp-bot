// SPDX-License-Identifier: AGPL-3.0-only

//! Simple per-user rate limiter (fixed window, in-memory).
//!
//! MVP defaults: 10 publishes/hour, 30 accepts/minute per user. Limits are
//! enforced before touching storage; rejections are normal domain outcomes,
//! not panics.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Rate-limit scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Scope {
    Publish,
    Accept,
}

impl Scope {
    const fn capacity(self) -> u32 {
        match self {
            Self::Publish => 10,
            Self::Accept => 30,
        }
    }

    const fn window(self) -> Duration {
        match self {
            Self::Publish => Duration::from_secs(3600),
            Self::Accept => Duration::from_secs(60),
        }
    }
}

#[derive(Debug)]
struct Bucket {
    tokens: u32,
    reset_at: Instant,
}

/// In-memory per-user limiter.
#[derive(Clone, Debug, Default)]
pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<(i64, Scope), Bucket>>>,
}

impl RateLimiter {
    /// Creates an empty limiter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` when the action is allowed (and consumes a token).
    #[must_use]
    pub fn allow(&self, user_id: i64, scope: Scope) -> bool {
        let mut buckets = self.lock();
        let now = Instant::now();
        buckets.retain(|_, bucket| bucket.reset_at > now);
        let bucket = buckets.entry((user_id, scope)).or_insert(Bucket {
            tokens: scope.capacity(),
            reset_at: now + scope.window(),
        });
        if now >= bucket.reset_at {
            bucket.tokens = scope.capacity();
            bucket.reset_at = now + scope.window();
        }
        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            true
        } else {
            false
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<(i64, Scope), Bucket>> {
        self.buckets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_burst_is_capped() {
        let limiter = RateLimiter::new();
        for _ in 0..10 {
            assert!(limiter.allow(1, Scope::Publish));
        }
        assert!(!limiter.allow(1, Scope::Publish));
        // Another user is unaffected.
        assert!(limiter.allow(2, Scope::Publish));
    }
}
