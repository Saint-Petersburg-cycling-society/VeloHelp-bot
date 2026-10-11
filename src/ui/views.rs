// SPDX-License-Identifier: AGPL-3.0-only

//! Telegram surfaces as pure `teloxide_ui::Ui` projections.
//!
//! Every semantic button carries a project-owned icon id from
//! `assets/icons/manifest.toml` plus an accessible text label. No Unicode
//! bicycle emoji is used as an icon.

use teloxide_ui::{ButtonLabel, Ui};

use super::Action;
use crate::domain::{HelpRequest, HelpRequestStatus, TelegramUserId};

/// Stable icon id for one action. All IDs have sources and exports in the manifest.
#[must_use]
pub fn icon_id(action: &Action) -> &'static str {
    match action {
        Action::OpenCreateRequest => "request.help",
        Action::OpenHelperSettings => "helper.profile",
        Action::ToggleArea(area) => match area.as_str() {
            "spb-admiralteysky" => "area.admiralteysky",
            "spb-frunzensky" => "area.frunzensky",
            "spb-kalininsky" => "area.kalininsky",
            "spb-kirovsky" => "area.kirovsky",
            "spb-kolpinsky" => "area.kolpinsky",
            "spb-krasnogvardeysky" => "area.krasnogvardeysky",
            "spb-krasnoselsky" => "area.krasnoselsky",
            "spb-kronshtadtsky" => "area.kronshtadtsky",
            "spb-kurortny" => "area.kurortny",
            "spb-moskovsky" => "area.moskovsky",
            "spb-nevsky" => "area.nevsky",
            "spb-petrodvortsovy" => "area.petrodvortsovy",
            "spb-petrogradsky" => "area.petrogradsky",
            "spb-primorsky" => "area.primorsky",
            "spb-pushkinsky" => "area.pushkinsky",
            "spb-tsentralny" => "area.tsentralny",
            "spb-vasileostrovsky" => "area.vasileostrovsky",
            "spb-vyborgsky" => "area.vyborgsky",
            _ => "location.area",
        },
        Action::ToggleCapability(kind) => {
            if kind == "accident" {
                "problem.accident"
            } else {
                "problem.tools"
            }
        }
        Action::SelectProblem(problem) | Action::ToggleHelperProblem(problem) => {
            match problem.as_str() {
                "puncture" => "problem.puncture",
                "drivetrain" => "problem.drivetrain",
                "brakes" => "problem.brakes",
                "wheel" => "problem.wheel",
                "transport" => "problem.transport",
                "accident" => "problem.accident",
                _ => "problem.tools",
            }
        }
        Action::EditDescription | Action::EditAddress => "action.edit",
        Action::SelectAddress(_) => "location.point",
        Action::WizardBack | Action::ArrivalBack(_, _) | Action::AbortAccept(_) => "nav.back",
        Action::WizardNext => "nav.next",
        // Publish reuses the resolve checkmark until the icon pack lands.
        Action::PublishRequest | Action::Resolve(_) => "action.resolve",
        Action::Accept(_, _) | Action::AcceptAt(_, _, _) | Action::SaveHelper => "action.accept",
        Action::Decline(_, _) | Action::Withdraw(_) => "action.decline",
        Action::Cancel(_) => "action.cancel",
        Action::OpenSource => "action.source",
        Action::Home => "nav.home",
        Action::HelperAreas => "location.area",
        Action::More
        | Action::HelperOptions
        | Action::ToggleEnabled
        | Action::EnterArrival(_, _) => "nav.settings",
        Action::Consent(_) | Action::Privacy | Action::ExportData => "privacy.data",
        Action::ActiveRequests | Action::OpenRequest(_) => "status.open",
        Action::ConfirmDelete | Action::DeleteData => "privacy.delete",
    }
}

/// Accessible text label for one action.
#[must_use]
pub fn action_label(action: &Action) -> String {
    match action {
        Action::OpenCreateRequest => "Нужна помощь".to_owned(),
        Action::OpenHelperSettings => "Я могу помочь".to_owned(),
        Action::ToggleArea(area) => format!("Район: {area}"),
        Action::ToggleCapability(cap) => format!("Help: {cap}"),
        Action::WizardBack | Action::AbortAccept(_) => "Назад".to_owned(),
        Action::WizardNext => "Далее".to_owned(),
        Action::PublishRequest => "Опубликовать".to_owned(),
        Action::Accept(_, _) => "Помогу через…".to_owned(),
        Action::AcceptAt(_, _, estimate) => estimate.label(),
        Action::EnterArrival(_, _) => "Свои минуты".to_owned(),
        Action::ArrivalBack(_, _) => "Варианты времени".to_owned(),
        Action::Decline(_, _) => "Отклонить".to_owned(),
        Action::Cancel(_) => "Отменить".to_owned(),
        Action::Resolve(_) => "Помощь оказана".to_owned(),
        Action::OpenSource => "Исходный код".to_owned(),
        Action::Home => "Главная".to_owned(),
        Action::More => "Ещё".to_owned(),
        Action::HelperAreas => "Районы".to_owned(),
        Action::HelperOptions => "Настройки".to_owned(),
        Action::Consent(_) => "Согласен, продолжить".to_owned(),
        Action::SelectProblem(problem) | Action::ToggleHelperProblem(problem) => {
            match problem.as_str() {
                "puncture" => "Прокол",
                "drivetrain" => "Цепь / передачи",
                "brakes" => "Тормоза",
                "wheel" => "Колесо",
                "transport" => "Перевезти велосипед",
                "accident" => "Происшествие",
                _ => "Инструменты",
            }
            .to_owned()
        }
        Action::EditDescription => "Изменить описание".to_owned(),
        Action::EditAddress => "Уточнить адрес".to_owned(),
        Action::SelectAddress(_) => "Выбрать адрес".to_owned(),
        Action::ToggleEnabled => "Включить / приостановить".to_owned(),
        Action::SaveHelper => "Сохранить".to_owned(),
        Action::ActiveRequests => "Активные заявки".to_owned(),
        Action::OpenRequest(_) => "Открыть заявку".to_owned(),
        Action::Privacy => "Мои данные".to_owned(),
        Action::ExportData => "Скачать мои данные".to_owned(),
        Action::ConfirmDelete => "Удалить мои данные".to_owned(),
        Action::DeleteData => "Подтверждаю удаление".to_owned(),
        Action::Withdraw(_) => "Не смогу помочь".to_owned(),
    }
}

#[must_use]
pub fn button_row(action: Action) -> teloxide_ui::ButtonRow<Action> {
    let label = action_label(&action);
    teloxide_ui::Ui::button_row().push(teloxide_ui::Ui::button(ButtonLabel::from(label), action))
}

/// Home menu (SPEC §4).
#[must_use]
pub fn home_view(show_active: bool, source_url: &str) -> Ui<Action> {
    let mut ui = Ui::column()
        .push(Ui::paragraph("VeloHelp"))
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
        .push(Ui::paragraph("Help settings"))
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
        .push(Ui::paragraph("Нужна помощь"))
        .push(Ui::paragraph(format!("Район: {area}")))
        .push(Ui::paragraph(format!(
            "Тип: {}",
            match request.help_kind {
                crate::domain::HelpKind::Technical => "Техническая помощь",
                crate::domain::HelpKind::Accident => "Происшествие",
            }
        )))
        .push(Ui::paragraph(request.description.clone()))
        .push(Ui::paragraph(format!(
            "Статус: {}",
            status_text(request.status)
        )))
        .push(
            if request.status == HelpRequestStatus::Open && request.expires_at > chrono::Utc::now()
            {
                Ui::fragment([
                    button_row(Action::Accept(request.id, request.version)).into(),
                    button_row(Action::Decline(request.id, request.version)).into(),
                ])
            } else {
                Ui::paragraph("Заявка уже недоступна для приёма.")
            },
        )
        .push(button_row(Action::Home))
}

/// Matched helper view: offers a native map action after a won accept.
/// The adapter re-checks authoritative membership before sending the point.
#[must_use]
pub fn matched_helper_view(request: &HelpRequest, helper: TelegramUserId) -> Ui<Action> {
    let mut ui = Ui::column()
        .push(Ui::paragraph("Вы приняли заявку"))
        .push(Ui::paragraph(format!(
            "Статус: {}",
            status_text(request.status)
        )));
    if request.status != HelpRequestStatus::Matched || request.matched_helper_id != Some(helper) {
        return dispatch_card_view(request, request.area_id.as_deref().unwrap_or("—"));
    }
    ui = ui.push(Ui::paragraph(request.description.clone()));
    if let Some(bike) = &request.bike_description {
        if !bike.is_empty() {
            ui = ui.push(Ui::paragraph(format!("Велосипед: {bike}")));
        }
    }
    ui.push(button_row(Action::Withdraw(request.id)))
        .push(button_row(Action::Home))
}

/// Requester view of a matched request (no extra disclosure needed).
#[must_use]
pub fn matched_requester_view(request: &HelpRequest) -> Ui<Action> {
    let mut ui = Ui::column()
        .push(Ui::paragraph("Ваша заявка"))
        .push(Ui::paragraph(format!(
            "Статус: {}",
            status_text(request.status)
        )))
        .push(Ui::paragraph(request.description.clone()));
    if request.status == HelpRequestStatus::Matched {
        ui = ui.push(button_row(Action::Resolve(request.id)));
    }
    if matches!(
        request.status,
        HelpRequestStatus::Draft | HelpRequestStatus::Open | HelpRequestStatus::Matched
    ) {
        ui = ui.push(button_row(Action::Cancel(request.id)));
    }
    ui.push(button_row(Action::Home))
}

/// Status badge text for a request.
#[must_use]
pub fn status_text(status: HelpRequestStatus) -> &'static str {
    match status {
        HelpRequestStatus::Draft => "Черновик",
        HelpRequestStatus::Open => "Открыта",
        HelpRequestStatus::Matched => "Помощник найден",
        HelpRequestStatus::Resolved => "Помощь оказана",
        HelpRequestStatus::Cancelled => "Отменена",
        HelpRequestStatus::Expired => "Срок истёк",
    }
}

/// Source/legal view: visible Corresponding Source path (AGPL).
#[must_use]
pub fn source_view(source_url: &str) -> Ui<Action> {
    Ui::column()
        .push(Ui::paragraph("Source code"))
        .push(Ui::paragraph(format!("Corresponding Source: {source_url}")))
        .push(button_row(Action::OpenSource))
}

/// Privacy view: minimal data summary + deletion path pointer.
#[must_use]
pub fn privacy_view(consent_version: &str) -> Ui<Action> {
    Ui::column()
        .push(Ui::paragraph("My data"))
        .push(Ui::paragraph(format!("Consent version: {consent_version}")))
        .push(Ui::paragraph(
            "We store Telegram ID, helper settings, coarse areas, requests and the exact point of the active request only.",
        ))
        .push(button_row(Action::OpenHelperSettings))
}

/// Human readable category name; custom emoji never carries the only meaning.
#[must_use]
pub fn tag_label(tag: &str) -> &str {
    match tag {
        "puncture" => "Прокол",
        "drivetrain" => "Трансмиссия",
        "brakes" => "Тормоза",
        "wheel" => "Колесо",
        "tools" => "Инструменты",
        "transport" => "Перевозка",
        "other" => "Другое",
        _ => tag,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{HelpKind, HelpRequestId, TelegramUserId};
    use chrono::{TimeDelta, Utc};
    use uuid::Uuid;

    fn sample_request() -> HelpRequest {
        HelpRequest {
            exact_address: None,
            id: HelpRequestId(Uuid::new_v4()),
            requester_id: TelegramUserId(1),
            help_kind: HelpKind::Technical,
            problem_tags: vec![],
            description: "test".to_owned(),
            bike_description: None,
            status: HelpRequestStatus::Open,
            matched_helper_id: None,
            arrival_window: None,
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
    fn matched_helper_has_no_extra_contact_or_location_button() {
        let req = sample_request();
        let mut req = req;
        req.status = HelpRequestStatus::Matched;
        req.matched_helper_id = Some(TelegramUserId(2));
        let ui = matched_helper_view(&req, TelegramUserId(2));
        let rendered = format!("{ui:?}");
        assert!(rendered.contains("Withdraw"));
        assert!(!rendered.contains("Связаться в Telegram"));
        assert!(!rendered.contains("59.9"));
        assert!(!rendered.contains("30.3"));
    }

    #[test]
    fn no_bicycle_emoji_in_labels() {
        let actions = [
            Action::OpenCreateRequest,
            Action::Accept(sample_request().id, 0),
        ];
        for action in actions {
            let label = action_label(&action);
            assert!(!label.contains('🚲'));
            assert!(!label.contains('🚴'));
        }
    }
}
