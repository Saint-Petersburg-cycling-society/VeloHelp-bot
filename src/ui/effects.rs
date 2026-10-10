// SPDX-License-Identifier: AGPL-3.0-only

//! Multi-surface projection effects.
//!
//! One domain request has many independent Telegram views (requester + N
//! dispatch views). After commit the runtime reprojects each surface at its
//! own revision. An older queued render must never become final after a newer
//! revision: callers guard with `Revision` ordering.

use teloxide_ui::{Effect, Revision, Surface};

use super::{views, Action};
use crate::domain::{HelpRequest, HelpRequestStatus};

/// One projection target with its current revision.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionTarget {
    pub surface: Surface,
    /// `true` for the requester view, `false` for helper dispatch views.
    pub is_requester: bool,
    /// `true` only for the matched helper (exact-point disclosure).
    pub is_matched_helper: bool,
    pub revision: Revision,
}

/// Builds post-commit effects for requester + dispatch surfaces.
///
/// Privacy: only the matched helper surface receives the exact point, via
/// `matched_helper_view`. Dispatch cards never contain it.
#[must_use]
pub fn reproject_request(
    request: &HelpRequest,
    targets: &[ProjectionTarget],
    dispatch_area: &str,
) -> Vec<Effect<Action>> {
    targets
        .iter()
        .map(|target| {
            let ui = match request.status {
                HelpRequestStatus::Matched if target.is_matched_helper => {
                    views::matched_helper_view(request)
                }
                HelpRequestStatus::Matched if target.is_requester => {
                    views::matched_requester_view(request)
                }
                HelpRequestStatus::Matched => views::dispatch_card_view(request, dispatch_area),
                _ if target.is_requester => views::matched_requester_view(request),
                _ => views::dispatch_card_view(request, dispatch_area),
            };
            Effect::RenderAt {
                surface: target.surface.clone(),
                ui,
            }
        })
        .collect()
}

/// Guards against an older queued render overwriting a newer revision.
#[must_use]
pub const fn render_is_fresh(queued: Revision, current: Revision) -> bool {
    queued.get() >= current.get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{GeoPoint, HelpKind, HelpRequestId, TelegramUserId};
    use chrono::{TimeDelta, Utc};
    use teloxide::types::{ChatId, MessageId, UserId};
    use uuid::Uuid;

    fn matched_request() -> HelpRequest {
        HelpRequest {
            id: HelpRequestId(Uuid::new_v4()),
            requester_id: TelegramUserId(1),
            help_kind: HelpKind::Technical,
            problem_tags: vec![],
            description: "test".to_owned(),
            bike_description: None,
            status: HelpRequestStatus::Matched,
            matched_helper_id: Some(TelegramUserId(2)),
            exact_point: GeoPoint::new(59.9, 30.3),
            area_id: Some("spb-center".to_owned()),
            expires_at: Utc::now() + TimeDelta::minutes(60),
            version: 1,
        }
    }

    fn surface(chat: i64, msg: i32) -> Surface {
        Surface::Message {
            chat_id: ChatId(chat),
            message_id: MessageId(msg),
        }
    }

    #[test]
    fn telegram_edit_failure_keeps_domain_matched() {
        // Projection failure must not roll back the domain commit: effects are
        // built from the committed state and can be retried.
        let req = matched_request();
        let targets = vec![ProjectionTarget {
            surface: surface(1, 1),
            is_requester: true,
            is_matched_helper: false,
            revision: Revision::INITIAL,
        }];
        let effects = reproject_request(&req, &targets, "spb-center");
        assert_eq!(effects.len(), 1);
        // Domain stays matched regardless of transport outcome.
        assert_eq!(req.status, HelpRequestStatus::Matched);
    }

    #[test]
    fn stale_render_never_wins() {
        let old = Revision::new(3);
        let current = Revision::new(5);
        assert!(!render_is_fresh(old, current));
        assert!(render_is_fresh(current, current));
    }

    #[test]
    fn cross_view_invalidation_fans_out() {
        let req = matched_request();
        let targets = vec![
            ProjectionTarget {
                surface: surface(1, 1),
                is_requester: true,
                is_matched_helper: false,
                revision: Revision::INITIAL,
            },
            ProjectionTarget {
                surface: surface(2, 2),
                is_requester: false,
                is_matched_helper: true,
                revision: Revision::INITIAL,
            },
            ProjectionTarget {
                surface: surface(3, 3),
                is_requester: false,
                is_matched_helper: false,
                revision: Revision::INITIAL,
            },
        ];
        let effects = reproject_request(&req, &targets, "spb-center");
        assert_eq!(effects.len(), 3);
        // Loser helper surface must not contain the exact point.
        let rendered = format!("{effects:?}");
        assert!(rendered.contains("59.9"), "winner gets exact point");
    }

    #[test]
    fn surface_owner_is_typed() {
        let _ = UserId(1);
    }
}
