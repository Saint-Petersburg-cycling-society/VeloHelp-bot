// SPDX-License-Identifier: AGPL-3.0-only

//! Minimal operational counters. Exposed via logs in the MVP; a Prometheus
//! exporter is explicitly out of scope until Phase 4 metrics demand it.

use std::sync::atomic::{AtomicU64, Ordering};

/// Process-local counters.
#[derive(Debug, Default)]
pub struct Metrics {
    pub requests_created: AtomicU64,
    pub requests_matched: AtomicU64,
    pub requests_expired: AtomicU64,
    pub accepts_won: AtomicU64,
    pub accepts_taken: AtomicU64,
}

impl Metrics {
    /// Creates zeroed counters.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            requests_created: AtomicU64::new(0),
            requests_matched: AtomicU64::new(0),
            requests_expired: AtomicU64::new(0),
            accepts_won: AtomicU64::new(0),
            accepts_taken: AtomicU64::new(0),
        }
    }

    /// Records an accept outcome.
    pub fn record_accept(&self, won: bool) {
        if won {
            self.accepts_won.fetch_add(1, Ordering::Relaxed);
            self.requests_matched.fetch_add(1, Ordering::Relaxed);
        } else {
            self.accepts_taken.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Renders counters for logs.
    #[must_use]
    pub fn snapshot(&self) -> String {
        format!(
            "created={} matched={} expired={} won={} taken={}",
            self.requests_created.load(Ordering::Relaxed),
            self.requests_matched.load(Ordering::Relaxed),
            self.requests_expired.load(Ordering::Relaxed),
            self.accepts_won.load(Ordering::Relaxed),
            self.accepts_taken.load(Ordering::Relaxed),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_accumulate() {
        let metrics = Metrics::new();
        metrics.record_accept(true);
        metrics.record_accept(false);
        assert!(metrics.snapshot().contains("won=1"));
    }
}
