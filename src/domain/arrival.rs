// SPDX-License-Identifier: AGPL-3.0-only
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Helper's explicit travel estimate; no tracking or route inference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrivalEstimate {
    pub min_minutes: u16,
    pub max_minutes: u16,
}
impl ArrivalEstimate {
    #[must_use]
    pub const fn new(min_minutes: u16, max_minutes: u16) -> Option<Self> {
        if min_minutes >= 1 && min_minutes <= max_minutes && max_minutes <= 240 {
            Some(Self {
                min_minutes,
                max_minutes,
            })
        } else {
            None
        }
    }
    #[must_use]
    pub fn label(self) -> String {
        if self.min_minutes == self.max_minutes {
            format!("{} мин", self.min_minutes)
        } else {
            format!("{}–{} мин", self.min_minutes, self.max_minutes)
        }
    }
}

/// Absolute arrival window anchored to the successful atomic match time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrivalWindow {
    pub earliest: DateTime<Utc>,
    pub latest: DateTime<Utc>,
}
