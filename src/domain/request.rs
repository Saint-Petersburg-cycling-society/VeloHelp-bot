// SPDX-License-Identifier: AGPL-3.0-only

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::helper::{HelpKind, HelperProfile};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct TelegramUserId(pub i64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct HelpRequestId(pub Uuid);

impl HelpRequestId {
    /// Generates a new random request id.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
}

impl GeoPoint {
    #[must_use]
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

impl HelpRequestStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Open => "open",
            Self::Matched => "matched",
            Self::Resolved => "resolved",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
        }
    }

    #[must_use]
    pub const fn from_str(raw: &str) -> Option<Self> {
        match raw.as_bytes() {
            b"draft" => Some(Self::Draft),
            b"open" => Some(Self::Open),
            b"matched" => Some(Self::Matched),
            b"resolved" => Some(Self::Resolved),
            b"cancelled" => Some(Self::Cancelled),
            b"expired" => Some(Self::Expired),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HelpRequest {
    pub id: HelpRequestId,
    pub requester_id: TelegramUserId,
    pub help_kind: HelpKind,
    pub problem_tags: Vec<String>,
    pub description: String,
    pub bike_description: Option<String>,
    pub status: HelpRequestStatus,
    pub matched_helper_id: Option<TelegramUserId>,
    pub exact_point: Option<GeoPoint>,
    pub area_id: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub version: u64,
}

impl HelpRequest {
    /// Creates a new draft. `expires_at` must be in the future; callers
    /// enforce `expires_at > created_at` at the storage boundary.
    #[must_use]
    pub fn draft(
        requester_id: TelegramUserId,
        help_kind: HelpKind,
        description: String,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: HelpRequestId::generate(),
            requester_id,
            help_kind,
            problem_tags: Vec::new(),
            description,
            bike_description: None,
            status: HelpRequestStatus::Draft,
            matched_helper_id: None,
            exact_point: None,
            area_id: None,
            expires_at,
            version: 0,
        }
    }

    /// Pure transition guard used by storage before the atomic write.
    #[must_use]
    pub const fn can_accept(&self) -> bool {
        matches!(self.status, HelpRequestStatus::Open) && self.matched_helper_id.is_none()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchStatus {
    Pending,
    Accepted,
    Declined,
    Superseded,
    Expired,
}

impl DispatchStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
            Self::Superseded => "superseded",
            Self::Expired => "expired",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Dispatch {
    pub request_id: HelpRequestId,
    pub helper_id: TelegramUserId,
    pub status: DispatchStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptOutcome {
    Won,
    AlreadyTaken,
    NotOpen,
    Expired,
}

/// MVP matching predicate (SPEC §7): enabled, not the requester, area
/// intersection, capability intersection. Availability windows are checked
/// by the caller when present; absence of windows means available.
#[must_use]
pub fn is_candidate(
    helper: &HelperProfile,
    request: &HelpRequest,
    request_area: Option<&str>,
) -> bool {
    if !helper.enabled {
        return false;
    }
    if helper.user_id == request.requester_id {
        return false;
    }
    if !helper.supports_kind(request.help_kind) {
        return false;
    }
    match (&helper.areas, request_area.or(request.area_id.as_deref())) {
        (areas, Some(area)) => areas.iter().any(|a| a == area),
        // Without area information there is no coarse match in the MVP.
        (_, None) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;

    fn open_request() -> HelpRequest {
        let mut req = HelpRequest::draft(
            TelegramUserId(1),
            HelpKind::Technical,
            "puncture".to_owned(),
            Utc::now() + TimeDelta::minutes(60),
        );
        req.status = HelpRequestStatus::Open;
        req.area_id = Some("spb-center".to_owned());
        req
    }

    #[test]
    fn candidate_requires_area_and_kind() {
        let req = open_request();
        let helper = HelperProfile {
            user_id: TelegramUserId(2),
            enabled: true,
            areas: vec!["spb-center".to_owned()],
            help_kinds: vec![HelpKind::Technical],
        };
        assert!(is_candidate(&helper, &req, None));

        let disabled = HelperProfile {
            enabled: false,
            ..helper.clone()
        };
        assert!(!is_candidate(&disabled, &req, None));

        let self_help = HelperProfile {
            user_id: TelegramUserId(1),
            ..helper.clone()
        };
        assert!(!is_candidate(&self_help, &req, None));

        let wrong_kind = HelperProfile {
            help_kinds: vec![HelpKind::Accident],
            ..helper.clone()
        };
        assert!(!is_candidate(&wrong_kind, &req, None));

        let wrong_area = HelperProfile {
            areas: vec!["msk".to_owned()],
            ..helper.clone()
        };
        assert!(!is_candidate(&wrong_area, &req, None));
    }
}
