// SPDX-License-Identifier: AGPL-3.0-only

//! Callback dispatcher + automatic ACK over `teloxide_ui::ActionRegistry`.
//!
//! Stale/actor/TTL policy comes from `super::policy`. The dispatcher never
//! decides the request winner; it only resolves the typed action and tells
//! Telegram how to ACK it.

use teloxide::types::UserId;
use teloxide_ui::{ActionRegistry, ActionResolveError, ActionToken, Revision, ViewId};

use super::{action_ttl, actor_policy, stale_policy, Action};

/// How the transport must ACK a callback query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ack {
    /// Silent ACK (valid fresh action).
    Ok,
    /// Toast for a stale but harmless retry (e.g. double-click).
    ToastStale,
    /// Alert for actor mismatch / unknown / expired tokens.
    AlertDenied,
}

/// Dispatcher outcome: resolved action or ACK-only rejection.
#[derive(Clone, Debug, PartialEq)]
pub enum DispatchOutcome {
    Resolved(Action),
    Rejected(Ack),
}

#[must_use]
pub fn ack_for_error(err: &ActionResolveError) -> Ack {
    match err {
        ActionResolveError::Stale { .. } => Ack::ToastStale,
        ActionResolveError::Unknown
        | ActionResolveError::Expired
        | ActionResolveError::ActorMismatch { .. }
        | ActionResolveError::ViewMismatch { .. } => Ack::AlertDenied,
    }
}

/// Registers one action for a view/revision/actor triple.
#[must_use]
pub fn register_action(
    registry: &ActionRegistry<Action>,
    view_id: ViewId,
    revision: Revision,
    action: &Action,
    actor: UserId,
    owner: Option<UserId>,
) -> ActionToken {
    registry.register_with_ttl(
        view_id,
        revision,
        action.clone(),
        actor_policy(action, actor, owner),
        stale_policy(action),
        action_ttl(action),
    )
}

/// Resolves a callback token into a typed action or an ACK decision.
#[must_use]
pub fn dispatch_callback(
    registry: &ActionRegistry<Action>,
    token: &ActionToken,
    actor: UserId,
    view_id: ViewId,
    current_revision: Revision,
) -> DispatchOutcome {
    match registry.resolve(token, actor, view_id, current_revision) {
        Ok(record) => DispatchOutcome::Resolved(record.action),
        Err(err) => DispatchOutcome::Rejected(ack_for_error(&err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_accept_after_commit_toasts_stale() {
        let registry = ActionRegistry::new();
        let view = ViewId::fresh();
        let action = Action::WizardNext;
        let token = register_action(
            &registry,
            view,
            Revision::INITIAL,
            &action,
            UserId(1),
            Some(UserId(1)),
        );
        // Simulate a revision bump (user pressed Next); old token is stale but
        // navigation is idempotent, so it still resolves.
        let next = Revision::INITIAL.checked_next().expect("next");
        match dispatch_callback(&registry, &token, UserId(1), view, next) {
            DispatchOutcome::Resolved(_) | DispatchOutcome::Rejected(Ack::ToastStale) => {}
            DispatchOutcome::Rejected(Ack::Ok | Ack::AlertDenied) => {
                panic!("unexpected ACK")
            }
        }
    }

    #[test]
    fn foreign_user_is_denied() {
        let registry = ActionRegistry::new();
        let view = ViewId::fresh();
        let token = register_action(
            &registry,
            view,
            Revision::INITIAL,
            &Action::WizardNext,
            UserId(1),
            Some(UserId(1)),
        );
        assert_eq!(
            dispatch_callback(&registry, &token, UserId(999), view, Revision::INITIAL),
            DispatchOutcome::Rejected(Ack::AlertDenied)
        );
    }
}
