// SPDX-License-Identifier: AGPL-3.0-only

//! Frontend: reusable components, views, policies and session state.
//!
//! Must not access SQL directly. Must not decide the request winner; the
//! authoritative DB does that. Callback data is never domain truth.

use crate::domain::HelpRequestId;

pub mod components;
pub mod policy;
pub mod session;
pub mod views;

pub use components::{Confirm, MultiSelect, StatusBadge, Wizard};
pub use policy::{action_ttl, actor_policy, stale_policy};
pub use session::{HelperDraft, RequestDraft, RequestStep};
pub use views::{
    action_label, dispatch_card_view, helper_setup_view, home_view, icon_id, matched_helper_view,
    matched_requester_view, status_text,
};

/// Typed UI actions. Generic mechanics only; no `Bike*` component names and
/// no bicycle emoji icons (see `views::icon_id`).
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    OpenCreateRequest,
    OpenHelperSettings,
    ToggleArea(String),
    ToggleCapability(String),
    WizardBack,
    WizardNext,
    PublishRequest,
    Accept(HelpRequestId),
    Decline(HelpRequestId),
    Cancel(HelpRequestId),
    Resolve(HelpRequestId),
    OpenSource,
}
