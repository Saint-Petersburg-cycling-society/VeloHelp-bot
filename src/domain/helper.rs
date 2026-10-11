// SPDX-License-Identifier: AGPL-3.0-only

use serde::{Deserialize, Serialize};

use super::TelegramUserId;

/// Stable problem catalog shared by matching and the two setup screens.
pub const HELP_PROBLEMS: &[&str] = &[
    "puncture",
    "drivetrain",
    "brakes",
    "wheel",
    "tools",
    "transport",
    "accident",
];

/// Maps a known problem to its help kind.
#[must_use]
pub fn problem_kind(tag: &str) -> Option<HelpKind> {
    HELP_PROBLEMS
        .contains(&tag)
        .then_some(if tag == "accident" {
            HelpKind::Accident
        } else {
            HelpKind::Technical
        })
}

/// Kind of help, stored as `technical`/`accident` in `SQLite`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelpKind {
    Technical,
    Accident,
}

impl HelpKind {
    /// Canonical storage representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Technical => "technical",
            Self::Accident => "accident",
        }
    }

    /// Parses the canonical storage representation.
    #[must_use]
    pub const fn from_str(raw: &str) -> Option<Self> {
        match raw.as_bytes() {
            b"technical" => Some(Self::Technical),
            b"accident" => Some(Self::Accident),
            _ => None,
        }
    }
}

/// Optional weekly availability window in the helper's local time.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct AvailabilityWindow {
    /// ISO weekday 1..=7 (Monday = 1).
    pub weekday: u8,
    /// Start minute of day 0..=1439.
    pub local_start_minutes: u16,
    /// End minute of day 0..=1439, must differ from start.
    pub local_end_minutes: u16,
}

impl AvailabilityWindow {
    /// Validates a window without allocating.
    #[must_use]
    pub const fn is_valid(weekday: u8, start: u16, end: u16) -> bool {
        weekday >= 1 && weekday <= 7 && start <= 1439 && end <= 1439 && start != end
    }
}

/// Helper settings. Coarse `AreaId`s only; no home address, no tracking.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HelperProfile {
    pub user_id: TelegramUserId,
    pub enabled: bool,
    pub areas: Vec<String>,
    pub help_kinds: Vec<HelpKind>,
    /// Empty means all problems of supported kinds, preserving older profiles.
    #[serde(default)]
    pub problem_tags: Vec<String>,
}

impl HelperProfile {
    /// Returns true when the helper accepts this kind of help.
    #[must_use]
    pub fn supports_kind(&self, kind: HelpKind) -> bool {
        self.help_kinds.contains(&kind)
    }
}
