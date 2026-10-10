// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::Context;
use std::time::Duration;

/// Runtime configuration loaded from the environment.
///
/// Mirrors `.env.example` and the deployment contract in
/// `docs/DEPLOYMENT.md`:
/// - `DATABASE_URL` — `SQLite` URL (required);
/// - `TELOXIDE_TOKEN` — Telegram bot token (required);
/// - `SOURCE_URL` — public Corresponding Source URL for the AGPL instance
///   (required, shown in the Home `Source code` button);
/// - `CONSENT_VERSION` — consent text version recorded in `users` (required);
/// - `DEFAULT_REQUEST_TTL_MINUTES` — default `Open` TTL (optional, 60).
#[derive(Clone, PartialEq, Eq)]
pub struct Config {
    /// `SQLite` database URL, e.g. `sqlite:///data/bike-aid.sqlite`.
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

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("database_url", &self.database_url)
            .field("teloxide_token", &"[REDACTED]")
            .field("source_url", &self.source_url)
            .field("consent_version", &self.consent_version)
            .field("default_ttl", &self.default_ttl)
            .finish()
    }
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
        Self::load(true)
    }

    /// Loads offline preflight configuration without requiring a bot token.
    ///
    /// # Errors
    /// Returns an error for missing or invalid non-secret configuration.
    pub fn for_preflight() -> anyhow::Result<Self> {
        Self::load(false)
    }

    fn load(require_token: bool) -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
        let teloxide_token = std::env::var("TELOXIDE_TOKEN").unwrap_or_default();
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
        let config = Self {
            database_url,
            teloxide_token,
            source_url,
            consent_version,
            default_ttl,
        };
        config.validate(require_token)?;
        Ok(config)
    }

    /// Validates required values without echoing credentials in errors.
    ///
    /// # Errors
    /// Returns an error for blank fields, non-HTTPS source URLs or invalid TTL.
    pub fn validate(&self, require_token: bool) -> anyhow::Result<()> {
        for (key, value) in [
            ("DATABASE_URL", self.database_url.as_str()),
            ("SOURCE_URL", self.source_url.as_str()),
            ("CONSENT_VERSION", self.consent_version.as_str()),
        ] {
            anyhow::ensure!(!value.trim().is_empty(), "{key} must not be empty");
        }
        if require_token {
            anyhow::ensure!(
                !self.teloxide_token.trim().is_empty(),
                "TELOXIDE_TOKEN must not be empty"
            );
        }
        let url = reqwest::Url::parse(&self.source_url).context("invalid SOURCE_URL")?;
        anyhow::ensure!(
            url.scheme() == "https" && url.host_str().is_some(),
            "SOURCE_URL must be an absolute HTTPS URL"
        );
        anyhow::ensure!(
            (300..=86400).contains(&self.default_ttl.as_secs()),
            "request TTL must be 5..=1440 minutes"
        );
        Ok(())
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
