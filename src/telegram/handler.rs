// SPDX-License-Identifier: AGPL-3.0-only

//! Typed Telegram events. Location/text are external inputs and never pretend
//! to be callbacks. No business-state decisions happen here.

use crate::domain::GeoPoint;

/// External input from a Telegram user (not a callback).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExternalInput {
    /// Explicit location sent by the user for the active request.
    Location(GeoPoint),
    /// Free-form text for the current wizard step.
    Text,
}

/// Minimal typed event surface used by the dispatcher.
#[derive(Clone, Debug, PartialEq)]
pub enum TelegramEvent {
    /// `/start` command.
    Start,
    /// External input event (location/text).
    Input(ExternalInput),
    /// Consent accepted for a version.
    ConsentAccepted(String),
}

/// Human-readable description for logging (never logs exact coordinates).
#[must_use]
pub fn describe_event(event: &TelegramEvent) -> &'static str {
    match event {
        TelegramEvent::Start => "start",
        TelegramEvent::Input(ExternalInput::Location(_)) => "location-input",
        TelegramEvent::Input(ExternalInput::Text) => "text-input",
        TelegramEvent::ConsentAccepted(_) => "consent-accepted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_is_not_a_callback() {
        let point = GeoPoint::new(59.9, 30.3).expect("valid");
        let event = TelegramEvent::Input(ExternalInput::Location(point));
        assert_eq!(describe_event(&event), "location-input");
    }
}
