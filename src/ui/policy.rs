// SPDX-License-Identifier: AGPL-3.0-only

//! Per-action actor/stale/TTL policy for bike-aid actions.
//!
//! Accept is strictly actor-bound and stale-rejecting: an old callback must
//! never reverse a newer domain state. Idempotent navigation tolerates stale
//! tokens.

use std::time::Duration;

use teloxide::types::UserId;
use teloxide_ui::{ActorPolicy, StalePolicy};

use super::Action;

/// TTL for short-lived dispatch actions.
pub const DISPATCH_ACTION_TTL: Duration = Duration::from_secs(3600);

/// Actor restriction for one action, given the invoking user and the owner
/// (requester or helper) the action is bound to.
#[must_use]
pub fn actor_policy(action: &Action, actor: UserId, owner: Option<UserId>) -> ActorPolicy {
    match action {
        // Public link: anyone may open it.
        Action::OpenSource => ActorPolicy::Any,
        // Everything else is bound to the owner when known, otherwise to the
        // rendering actor so a foreign Telegram user cannot click it.
        _ => ActorPolicy::User(owner.unwrap_or(actor)),
    }
}

/// Stale-revision behavior per action.
#[must_use]
pub const fn stale_policy(action: &Action) -> StalePolicy {
    match action {
        // Winner selection must reject stale tokens.
        Action::Accept(_, _)
        | Action::AcceptAt(_, _, _)
        | Action::EnterArrival(_, _)
        | Action::ArrivalBack(_, _)
        | Action::AbortAccept(_)
        | Action::Withdraw(_)
        | Action::PublishRequest
        | Action::DeleteData
        | Action::SaveHelper
        | Action::Consent(_)
        | Action::ToggleArea(_)
        | Action::ToggleCapability(_)
        | Action::HelperAreas
        | Action::HelperOptions
        | Action::ToggleEnabled
        | Action::WizardNext
        | Action::WizardBack
        | Action::SelectProblem(_)
        | Action::EditDescription => StalePolicy::Reject,
        // Decline/cancel/resolve apply to the latest state safely.
        Action::Decline(_, _) | Action::Cancel(_) | Action::Resolve(_) => {
            StalePolicy::ApplyToLatest
        }
        // Navigation is idempotent.
        _ => StalePolicy::Idempotent,
    }
}

/// Entry TTL per action. Dispatch actions expire with the request TTL window.
#[must_use]
pub const fn action_ttl(action: &Action) -> Option<Duration> {
    match action {
        Action::Accept(_, _) | Action::Decline(_, _) => Some(DISPATCH_ACTION_TTL),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::HelpRequestId;
    use uuid::Uuid;

    #[test]
    fn accept_rejects_stale() {
        let id = HelpRequestId(Uuid::new_v4());
        assert_eq!(stale_policy(&Action::Accept(id, 0)), StalePolicy::Reject);
    }

    #[test]
    fn navigation_is_idempotent() {
        assert_eq!(stale_policy(&Action::Home), StalePolicy::Idempotent);
    }

    #[test]
    fn telegram_id_is_identity() {
        // Telegram ID is identity, username is presentation (AGENTS §12).
        // Public source link is open to anyone.
        let policy = actor_policy(&Action::OpenSource, UserId(42), Some(UserId(7)));
        assert_eq!(policy, ActorPolicy::Any);
        // Dispatch actions bind to the owner, foreign users are rejected.
        let id = HelpRequestId(Uuid::new_v4());
        let policy = actor_policy(&Action::Accept(id, 0), UserId(99), Some(UserId(7)));
        assert_eq!(policy, ActorPolicy::User(UserId(7)));
    }
}
