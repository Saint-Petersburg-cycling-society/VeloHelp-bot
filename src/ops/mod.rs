// SPDX-License-Identifier: AGPL-3.0-only

//! Operations: TTL worker, rate limits, metrics, backup, deletion/export.
//!
//! All background work uses short DB transactions and never holds a lock
//! across network I/O.

pub mod backup;
pub mod metrics;
pub mod privacy;
pub mod rate_limit;
pub mod ttl;

pub use backup::{backup_sqlite, restore_check};
pub use metrics::Metrics;
pub use privacy::{export_user_json, scorch_exact_points};
pub use rate_limit::RateLimiter;
pub use ttl::expire_once;
