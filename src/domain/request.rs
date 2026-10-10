// SPDX-License-Identifier: AGPL-3.0-only

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct TelegramUserId(pub i64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct HelpRequestId(pub Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
}

impl GeoPoint {
    pub fn new(lat: f64, lon: f64) -> Option<Self> {
        (-90.0..=90.0)
            .contains(&lat)
            .then_some(())
            .and_then(|()| (-180.0..=180.0).contains(&lon).then_some(Self { lat, lon }))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelpRequestStatus {
    Draft,
    Open,
    Matched,
    Resolved,
    Cancelled,
    Expired,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HelpRequest {
    pub id: HelpRequestId,
    pub requester_id: TelegramUserId,
    pub status: HelpRequestStatus,
    pub matched_helper_id: Option<TelegramUserId>,
    pub exact_point: Option<GeoPoint>,
    pub area_id: Option<String>,
    pub description: String,
    pub expires_at: DateTime<Utc>,
    pub version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptOutcome {
    Won,
    AlreadyTaken,
    NotOpen,
    Expired,
}
