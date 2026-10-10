// SPDX-License-Identifier: AGPL-3.0-only

use serde::{Deserialize, Serialize};

use super::TelegramUserId;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelpKind {
    Technical,
    Accident,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HelperProfile {
    pub user_id: TelegramUserId,
    pub enabled: bool,
    pub areas: Vec<String>,
    pub help_kinds: Vec<HelpKind>,
}
