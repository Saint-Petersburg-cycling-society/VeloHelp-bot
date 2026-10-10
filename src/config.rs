// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::Context;
use std::time::Duration;

/// Runtime configuration loaded from the environment.
///
/// Mirrors `.env.example` and the deployment contract in
/// `docs/DEPLOYMENT.md`:
/// - `DATABASE_URL` — SQLite URL (required);
/// - `TELOXIDE_TOKEN` — Telegram bot token (required);
/// - `SOURCE_URL` — public Corresponding Source URL for the AGPL instance
///   (required, shown in the Home `Source code` button);
/// - `CONSENT_VERSION` — consent text version recorded in `users` (required);
/// - `DEFAULT_REQUEST_TTL_MINUTES` — default `Open` TTL (optional, 60).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// SQLite database URL, e.g. `sqlite:///data/bike-aid.sqlite`.
    pub database_url: String,
    /// Telegram bot token.
    pub teloxide_token: String,
    /// Public URL of the Corresponding Source.
    pub source_url: String,
    /// Consent text version, e.g. `2026-10-03`.
    pub consent_version: String,
    /// Default request TTL.
    pub default_ttl: Duration,
}

impl Config {
    /// Default request TTL in minutes when the env var is absent.
    pub const DEFAULT_TTL_MINUTES: u64 = 60;

    /// Loads configuration from the process environment.
    ///
    /// # Errors
    ///
    /// Returns [`anyhow::Error`] when a required variable is missing or
    /// `DEFAULT_REQUEST_TTL_MINUTES` is not a valid TTL.
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
        let teloxide_token =
            std::env::var("TELOXIDE_TOKEN").context("TELOXIDE_TOKEN is required")?;
        let source_url = std::env::var("SOURCE_URL").context("SOURCE_URL is required")?;
        let consent_version =
            std::env::var("CONSENT_VERSION").context("CONSENT_VERSION is required")?;
        let default_ttl = match std::env::var("DEFAULT_REQUEST_TTL_MINUTES") {
            Err(std::env::VarError::NotPresent) => {
                Duration::from_secs(Self::DEFAULT_TTL_MINUTES * 60)
            }
            other => {
                let raw = other.context("invalid DEFAULT_REQUEST_TTL_MINUTES")?;
                Self::parse_ttl_minutes(raw.as_str())?
            }
        };
        Ok(Self {
            database_url,
            teloxide_token,
            source_url,
            consent_version,
            default_ttl,
        })
    }

    /// Parses `DEFAULT_REQUEST_TTL_MINUTES` (5..=1440).
    ///
    /// # Errors
    ///
    /// Returns [`anyhow::Error`] when the value is not an integer in range.
    pub fn parse_ttl_minutes(raw: &str) -> anyhow::Result<Duration> {
        let minutes: u64 = raw
            .trim()
            .parse()
            .context("DEFAULT_REQUEST_TTL_MINUTES must be an integer")?;
        if !(5..=1440).contains(&minutes) {
            anyhow::bail!("DEFAULT_REQUEST_TTL_MINUTES must be 5..=1440");
        }
        Ok(Duration::from_secs(minutes * 60))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttl_defaults_to_60_minutes() {
        let ttl = Config::parse_ttl_minutes("60").expect("valid ttl");
        assert_eq!(ttl, Duration::from_secs(3600));
    }

    #[test]
    fn ttl_rejects_out_of_range() {
        assert!(Config::parse_ttl_minutes("4").is_err());
        assert!(Config::parse_ttl_minutes("1441").is_err());
        assert!(Config::parse_ttl_minutes("abc").is_err());
    }
}
