// SPDX-License-Identifier: AGPL-3.0-only

//! Telegram surfaces as pure `teloxide_ui::Ui` projections.
//!
//! Every semantic button carries a project-owned icon id from
//! `assets/icons/manifest.toml` plus an accessible text label. No Unicode
//! bicycle emoji is used as an icon.

use teloxide_ui::{ButtonLabel, Ui};

use super::Action;
use crate::domain::{HelpRequest, HelpRequestStatus};

/// Stable icon id for one action. All ids exist in the manifest (SVG sources
/// are a Codex follow-up; text labels carry meaning meanwhile).
#[must_use]
pub const fn icon_id(action: &Action) -> &'static str {
    match action {
        Action::OpenCreateRequest => "request.help",
        Action::OpenHelperSettings => "helper.profile",
        Action::ToggleArea(_) => "location.area",
        Action::ToggleCapability(_) => "problem.tools",
        Action::WizardBack => "nav.back",
        Action::WizardNext => "nav.next",
        // Publish reuses the resolve checkmark until the icon pack lands.
        Action::PublishRequest | Action::Resolve(_) => "action.resolve",
        Action::Accept(_) => "action.accept",
        Action::Decline(_) => "action.decline",
        Action::Cancel(_) => "action.cancel",
        Action::OpenSource => "action.source",
    }
}

/// Accessible text label for one action.
#[must_use]
pub fn action_label(action: &Action) -> String {
    match action {
        Action::OpenCreateRequest => "Need help".to_owned(),
        Action::OpenHelperSettings => "I can help".to_owned(),
        Action::ToggleArea(area) => format!("Area: {area}"),
        Action::ToggleCapability(cap) => format!("Help: {cap}"),
        Action::WizardBack => "Back".to_owned(),
        Action::WizardNext => "Next".to_owned(),
        Action::PublishRequest => "Publish".to_owned(),
        Action::Accept(_) => "Accept".to_owned(),
        Action::Decline(_) => "Decline".to_owned(),
        Action::Cancel(_) => "Cancel".to_owned(),
        Action::Resolve(_) => "Resolve".to_owned(),
        Action::OpenSource => "Source code".to_owned(),
    }
}

fn button_row(action: Action) -> teloxide_ui::ButtonRow<Action> {
    let label = format!("[{}] {}", icon_id(&action), action_label(&action));
    teloxide_ui::Ui::button_row().push(teloxide_ui::Ui::button(ButtonLabel::from(label), action))
}

/// Home menu (SPEC §4).
#[must_use]
pub fn home_view(show_active: bool, source_url: &str) -> Ui<Action> {
    let mut ui = Ui::column()
        .push(Ui::heading("VeloHelp"))
        .push(Ui::paragraph(
            "Voluntary peer assistance. The bot is not an emergency service.",
        ))
        .push(button_row(Action::OpenCreateRequest))
        .push(button_row(Action::OpenHelperSettings));
    if show_active {
        ui = ui.push(Ui::paragraph("You have an active request."));
    }
    ui.push(Ui::paragraph(format!("Source: {source_url}")))
        .push(button_row(Action::OpenSource))
}

/// Helper setup preview (SPEC §5). Areas/kinds are data-driven strings.
#[must_use]
pub fn helper_setup_view(areas: &[String], kinds: &[String], enabled: bool) -> Ui<Action> {
    let mut ui = Ui::column()
        .push(Ui::heading("Help settings"))
        .push(Ui::paragraph(format!(
            "Enabled: {}",
            if enabled { "yes" } else { "no" }
        )));
    for area in areas {
        ui = ui.push(button_row(Action::ToggleArea(area.clone())));
    }
    for kind in kinds {
        ui = ui.push(button_row(Action::ToggleCapability(kind.clone())));
    }
    ui.push(button_row(Action::WizardNext))
}

/// Dispatch card for helpers (SPEC §7-8). Coarse area only: the exact point
/// MUST NOT appear here.
#[must_use]
pub fn dispatch_card_view(request: &HelpRequest, area: &str) -> Ui<Action> {
    Ui::column()
        .push(Ui::heading("Help needed"))
        .push(Ui::paragraph(format!("Area: {area}")))
        .push(Ui::paragraph(format!("Kind: {:?}", request.help_kind)))
        .push(Ui::paragraph(request.description.clone()))
        .push(button_row(Action::Accept(request.id)))
        .push(button_row(Action::Decline(request.id)))
}

/// Matched helper view: the only surface that discloses the exact point,
/// and only after a won accept.
#[must_use]
pub fn matched_helper_view(request: &HelpRequest) -> Ui<Action> {
    let mut ui = Ui::column()
        .push(Ui::heading("Matched"))
        .push(Ui::paragraph(format!("Status: {:?}", request.status)));
    if let Some(point) = request.exact_point {
        ui = ui.push(Ui::paragraph(format!(
            "Exact point: {:.5}, {:.5}",
            point.lat, point.lon
        )));
    }
    ui.push(button_row(Action::Cancel(request.id)))
}

/// Requester view of a matched request (no extra disclosure needed).
#[must_use]
pub fn matched_requester_view(request: &HelpRequest) -> Ui<Action> {
    Ui::column()
        .push(Ui::heading("Helper found"))
        .push(Ui::paragraph(format!("Status: {:?}", request.status)))
        .push(button_row(Action::Resolve(request.id)))
        .push(button_row(Action::Cancel(request.id)))
}

/// Status badge text for a request.
#[must_use]
pub fn status_text(status: HelpRequestStatus) -> &'static str {
    match status {
        HelpRequestStatus::Draft => "Draft",
        HelpRequestStatus::Open => "Open",
        HelpRequestStatus::Matched => "Matched",
        HelpRequestStatus::Resolved => "Resolved",
        HelpRequestStatus::Cancelled => "Cancelled",
        HelpRequestStatus::Expired => "Expired",
    }
}

/// Source/legal view: visible Corresponding Source path (AGPL).
#[must_use]
pub fn source_view(source_url: &str) -> Ui<Action> {
    Ui::column()
        .push(Ui::heading("Source code"))
        .push(Ui::paragraph(format!("Corresponding Source: {source_url}")))
        .push(button_row(Action::OpenSource))
}

/// Privacy view: minimal data summary + deletion path pointer.
#[must_use]
pub fn privacy_view(consent_version: &str) -> Ui<Action> {
    Ui::column()
        .push(Ui::heading("My data"))
        .push(Ui::paragraph(format!("Consent version: {consent_version}")))
        .push(Ui::paragraph(
            "We store Telegram ID, helper settings, coarse areas, requests and the exact point of the active request only.",
        ))
        .push(button_row(Action::OpenHelperSettings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{HelpKind, HelpRequestId, TelegramUserId};
    use chrono::{TimeDelta, Utc};
    use uuid::Uuid;

    fn sample_request() -> HelpRequest {
        HelpRequest {
            id: HelpRequestId(Uuid::new_v4()),
            requester_id: TelegramUserId(1),
            help_kind: HelpKind::Technical,
            problem_tags: vec![],
            description: "test".to_owned(),
            bike_description: None,
            status: HelpRequestStatus::Open,
            matched_helper_id: None,
            exact_point: crate::domain::GeoPoint::new(59.9, 30.3),
            area_id: Some("spb-center".to_owned()),
            expires_at: Utc::now() + TimeDelta::minutes(60),
            version: 0,
        }
    }

    #[test]
    fn dispatch_card_hides_exact_point() {
        let req = sample_request();
        let ui = dispatch_card_view(&req, "spb-center");
        let rendered = format!("{ui:?}");
        assert!(rendered.contains("spb-center"));
        assert!(!rendered.contains("59.9"), "exact point must not fan out");
    }

    #[test]
    fn matched_helper_shows_exact_point() {
        let req = sample_request();
        let ui = matched_helper_view(&req);
        let rendered = format!("{ui:?}");
        assert!(rendered.contains("59.9"));
    }

    #[test]
    fn no_bicycle_emoji_in_labels() {
        let actions = [
            Action::OpenCreateRequest,
            Action::Accept(sample_request().id),
        ];
        for action in actions {
            let label = action_label(&action);
            assert!(!label.contains('🚲'));
            assert!(!label.contains('🚴'));
        }
    }
}
