// SPDX-License-Identifier: AGPL-3.0-only

use crate::domain::HelpRequestId;

#[derive(Clone, Debug)]
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

// TODO dogfood:
// - per-action ActorPolicy/StalePolicy/TTL;
// - location-input event;
// - callback ACK dispatcher;
// - multi-surface reproject effect.
