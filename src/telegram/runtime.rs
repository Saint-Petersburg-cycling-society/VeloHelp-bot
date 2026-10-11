// SPDX-License-Identifier: AGPL-3.0-only
//! Single-host polling adapter. All SQL and authoritative transitions stay in repositories.
use crate::{
    application::{HelperRepository, RequestRepository},
    config::Config,
    domain::{
        AcceptOutcome, AddressMap, ApproximateLocation, ArrivalEstimate, DistrictMap, GeoPoint,
        HelpKind, HelpRequest, HelpRequestId, HelpRequestStatus, HelperProfile, TelegramUserId,
    },
    ops::{rate_limit::Scope, RateLimiter},
    storage::SqliteRepositories,
    ui::{
        views::{self, button_row},
        Action, HelperDraft, RequestDraft, RequestStep,
    },
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use teloxide::{
    payloads::{
        AnswerCallbackQuerySetters, EditMessageTextSetters, GetUpdatesSetters, SendDocumentSetters,
        SendMessageSetters, SendRichMessageSetters,
    },
    prelude::*,
    types::{
        AllowedUpdate, ButtonRequest, InputFile, KeyboardButton, KeyboardMarkup, MessageId,
        ReplyMarkup, UpdateKind,
    },
};
use teloxide_ui::{
    ActionRegistry, ActionToken, ActorPolicy, RenderContext, Revision, RichRenderer, StalePolicy,
    Ui, ViewId,
};

const SESSION_TTL: Duration = Duration::from_secs(3600);

#[derive(Default)]
struct Session {
    request: RequestDraft,
    helper: Option<HelperDraft>,
    menu: Option<MessageId>,
    callback_menu: Option<MessageId>,
    resurface_menu: bool,
    replaced_menus: Vec<MessageId>,
    editing_request: bool,
    location_warning: Option<String>,
    location_accuracy: Option<f64>,
    last_location: Option<GeoPoint>,
    address_candidates: Vec<crate::domain::AddressCandidate>,
    // None means the client keyboard state is unknown after a restart.
    location_keyboard: Option<bool>,
    keyboard_messages: Vec<MessageId>,
    display_name: Option<String>,
    pending_arrival: Option<(HelpRequestId, u64)>,
    arrival_text_input: bool,
}
struct SurfaceState {
    view: ViewId,
    revision: Revision,
    tokens: Vec<ActionToken>,
    owner: TelegramUserId,
    request: Option<HelpRequestId>,
    expires: Instant,
    request_version: Option<u64>,
}
struct Runtime {
    bot: Bot,
    repo: SqliteRepositories,
    config: Config,
    registry: ActionRegistry<Action>,
    sessions: HashMap<i64, (Session, Instant)>,
    surfaces: HashMap<(i64, MessageId), SurfaceState>,
    limiter: RateLimiter,
    icons: crate::ui::icons::IconCatalog,
    metrics: std::sync::Arc<crate::ops::Metrics>,
    areas: Vec<crate::storage::Area>,
    ignored_users: std::collections::HashSet<i64>,
    icons_enabled: bool,
    districts: DistrictMap,
    addresses: AddressMap,
}

/// Runs real Telegram polling until the caller cancels the future.
/// # Errors
/// Returns sanitized startup errors; transient transport failures are retried.
pub async fn run_polling(
    bot: Bot,
    repo: SqliteRepositories,
    config: Config,
    metrics: std::sync::Arc<crate::ops::Metrics>,
) -> anyhow::Result<()> {
    bot.get_me().await.map_err(|_| {
        anyhow::anyhow!("Telegram authentication failed; verify the bot token and network")
    })?;
    if let Ok(path) = std::env::var("SOURCE_ARCHIVE_PATH") {
        anyhow::ensure!(
            std::path::Path::new(&path).is_file(),
            "SOURCE_ARCHIVE_PATH must point to the running version's source archive"
        );
    }
    let webhook = bot
        .get_webhook_info()
        .await
        .map_err(|_| anyhow::anyhow!("cannot check Telegram webhook configuration"))?;
    anyhow::ensure!(
        webhook.url.is_none(),
        "a webhook is configured; remove it before using long polling"
    );
    tracing::info!("Telegram authenticated; webhook absent; starting long polling");
    repo.recover_deliveries().await?;
    let areas = repo.areas().await?;
    let ignored_users: std::collections::HashSet<i64> = std::env::var("IGNORED_TELEGRAM_USER_IDS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| anyhow::anyhow!("IGNORED_TELEGRAM_USER_IDS must be numeric IDs"))?;
    let mut runtime = Runtime {
        bot,
        repo,
        config,
        registry: ActionRegistry::new(),
        sessions: HashMap::new(),
        surfaces: HashMap::new(),
        limiter: RateLimiter::new(),
        icons: crate::ui::icons::IconCatalog::bundled()?,
        metrics,
        areas,
        ignored_users,
        icons_enabled: true,
        districts: DistrictMap::bundled()?,
        addresses: AddressMap::bundled()?,
    };
    let mut offset = 0;
    loop {
        runtime.prune();
        // Rediscover matching waves after partial failures; writes are idempotent.
        if let Err(error) = runtime.deliver().await {
            tracing::warn!(
                operation = "notification_pass",
                reason = error_reason(&error),
                "notification pass failed; will retry"
            );
        }
        let updates = match runtime
            .bot
            .get_updates()
            .offset(offset)
            .timeout(25)
            .allowed_updates(vec![AllowedUpdate::Message, AllowedUpdate::CallbackQuery])
            .await
        {
            Ok(updates) => updates,
            Err(error) => {
                tracing::warn!(
                    operation = "get_updates",
                    reason = error_reason(&error.into()),
                    "Telegram polling failed; retrying in 3 seconds"
                );
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        };
        for update in updates {
            offset = i32::try_from(update.id.0)
                .unwrap_or(i32::MAX)
                .saturating_add(1);
            let outcome = match update.kind {
                UpdateKind::Message(message) => Box::pin(runtime.message(message)).await,
                UpdateKind::CallbackQuery(query) => Box::pin(runtime.callback(query)).await,
                _ => Ok(()),
            };
            if let Err(error) = outcome {
                tracing::warn!(
                    reason = error_reason(&error),
                    "Telegram update handling failed; domain state retained"
                );
            }
        }
    }
}

impl Runtime {
    fn session(&mut self, user: TelegramUserId) -> &mut Session {
        let entry = self
            .sessions
            .entry(user.0)
            .or_insert_with(|| (Session::default(), Instant::now()));
        entry.1 = Instant::now();
        &mut entry.0
    }
    fn prune(&mut self) {
        self.sessions
            .retain(|_, (_, time)| time.elapsed() < SESSION_TTL);
        let registry = self.registry.clone();
        self.surfaces.retain(|_, surface| {
            if Instant::now() >= surface.expires {
                for token in &surface.tokens {
                    registry.remove(token);
                }
                false
            } else {
                true
            }
        });
        self.registry.prune_expired();
    }
    async fn project(
        &mut self,
        user: TelegramUserId,
        ui: Ui<Action>,
        request: Option<HelpRequestId>,
        message: Option<MessageId>,
    ) -> anyhow::Result<MessageId> {
        let state = if let Some(id) = request {
            Some(self.repo.request(id).await?)
        } else {
            None
        };
        let action_ttl = state
            .as_ref()
            .filter(|r| r.status == HelpRequestStatus::Open)
            .map_or(SESSION_TTL, |r| {
                Duration::from_secs(
                    u64::try_from((r.expires_at - chrono::Utc::now()).num_seconds()).unwrap_or(0),
                )
            });
        let (view, revision) = message
            .and_then(|m| self.surfaces.get(&(user.0, m)))
            .map_or((ViewId::fresh(), Revision::INITIAL), |s| {
                (
                    s.view,
                    s.revision.checked_next().unwrap_or(Revision::INITIAL),
                )
            });
        let mut rendered = RichRenderer::new(self.registry.clone())
            .action_ttl(Some(action_ttl))
            .render(
                &ui,
                RenderContext::new(view, revision)
                    .actor(ActorPolicy::User(UserId(u64::try_from(user.0)?)))
                    .stale_policy(StalePolicy::Reject),
            )?;
        if let Some(request) = &state {
            decorate_invitation(user, request, &mut rendered.rich_message);
            self.decorate_match(user, request, &mut rendered.rich_message)
                .await?;
        }
        let text_only = rendered.rich_message.clone();
        self.decorate_labels(user, view, revision, &mut rendered.rich_message);
        let mut sent = self
            .send_surface(user, message, rendered.rich_message.clone())
            .await;
        if self.icons_enabled
            && matches!(&sent, Err(teloxide::RequestError::Api(error)) if ["emoji", "premium", "button_icon"].iter().any(|part| error.to_string().to_ascii_lowercase().contains(part)))
        {
            self.icons_enabled = false;
            tracing::warn!("Telegram denied custom emoji; using accessible text labels");
            sent = self.send_surface(user, message, text_only).await;
        }
        let sent = match sent {
            Ok(message) => message,
            Err(err) => {
                for token in &rendered.action_tokens {
                    self.registry.remove(token);
                }
                let error = anyhow::Error::from(err);
                tracing::warn!(
                    operation = if message.is_some() {
                        "edit_card"
                    } else {
                        "send_card"
                    },
                    reason = error_reason(&error),
                    "Telegram card delivery failed"
                );
                return Err(error);
            }
        };
        if let Some(old) = self.surfaces.insert(
            (user.0, sent.id),
            SurfaceState {
                view,
                revision,
                tokens: rendered.action_tokens,
                owner: user,
                request,
                // Keep expired projection metadata briefly so the TTL worker's
                // terminal state can still replace the old card. Tokens retain
                // their original expiry and never gain this grace period.
                expires: Instant::now() + action_ttl + Duration::from_secs(120),
                request_version: state.map(|r| r.version),
            },
        ) {
            for token in old.tokens {
                self.registry.remove(&token);
            }
        }
        Ok(sent.id)
    }
    fn decorate_labels(
        &self,
        user: TelegramUserId,
        view: ViewId,
        revision: Revision,
        rich: &mut teloxide::types::InputRichMessage,
    ) {
        if let Some(mut blocks) = rich
            .blocks_ref()
            .filter(|_| self.icons_enabled)
            .map(<[_]>::to_vec)
        {
            for block in &mut blocks {
                if let teloxide::types::InputRichBlock::Buttons(row) = block {
                    for button in &mut row.buttons {
                        if let Some(record) = button
                            .callback_data
                            .as_deref()
                            .and_then(|raw| ActionToken::try_from(raw).ok())
                            .and_then(|token| {
                                self.registry
                                    .resolve(
                                        &token,
                                        UserId(u64::try_from(user.0).unwrap_or(0)),
                                        view,
                                        revision,
                                    )
                                    .ok()
                            })
                        {
                            button.text = self
                                .icons
                                .label(views::icon_id(&record.action), button.text.clone());
                        }
                    }
                }
            }
            *rich = teloxide::types::InputRichMessage::blocks(blocks);
        }
    }
    async fn send_surface(
        &self,
        user: TelegramUserId,
        message: Option<MessageId>,
        rich: teloxide::types::InputRichMessage,
    ) -> Result<Message, teloxide::RequestError> {
        if let Some(message) = message {
            self.bot
                .edit_message_text(ChatId(user.0), message, "")
                .rich_message(rich)
                .await
        } else {
            self.bot
                .send_rich_message(ChatId(user.0), rich)
                .protect_content(true)
                .await
        }
    }
    fn location_reply_markup(&self, requested: bool) -> ReplyMarkup {
        if !requested {
            return ReplyMarkup::kb_remove();
        }
        let button = if self.icons_enabled {
            self.icons
                .keyboard_button("location.point", "Отправить местоположение")
        } else {
            KeyboardButton::new("Отправить местоположение")
        }
        .request(ButtonRequest::Location);
        KeyboardMarkup::new([[button]])
            .resize_keyboard()
            .one_time_keyboard()
            .into()
    }
    async fn menu(&mut self, user: TelegramUserId, ui: Ui<Action>) -> anyhow::Result<()> {
        let session = self.session(user);
        session.pending_arrival = None;
        session.arrival_text_input = false;
        let requested =
            session.editing_request && session.request.step == Some(RequestStep::Location);
        self.show(user, ui, None).await?;
        Box::pin(self.sync_location_keyboard(user, requested)).await
    }
    async fn show(
        &mut self,
        user: TelegramUserId,
        ui: Ui<Action>,
        request: Option<HelpRequestId>,
    ) -> anyhow::Result<()> {
        let previous = self.session(user).menu;
        let session = self.session(user);
        let target = if session.resurface_menu {
            None
        } else {
            session.callback_menu.or(previous)
        };
        let message = self.project(user, ui.clone(), request, target).await;
        let message = match message {
            Ok(message) => message,
            Err(err) if target.is_some() && missing_or_uneditable(&err) => {
                self.project(user, ui, request, None).await?
            }
            Err(err) => return Err(err),
        };
        let mut replaced: Vec<_> = previous.filter(|old| *old != message).into_iter().collect();
        replaced.extend(target.filter(|old| *old != message && Some(*old) != previous));
        for old in replaced {
            self.retire_menu(user, old);
            self.session(user).replaced_menus.push(old);
        }
        let session = self.session(user);
        session.menu = Some(message);
        session.resurface_menu = false;
        self.cleanup_replaced_menus(user).await;
        Ok(())
    }
    async fn cleanup_replaced_menus(&mut self, user: TelegramUserId) {
        let messages = std::mem::take(&mut self.session(user).replaced_menus);
        for message in messages {
            if let Err(err) = self.bot.delete_message(ChatId(user.0), message).await {
                if !matches!(
                    err,
                    teloxide::RequestError::Api(teloxide::ApiError::MessageToDeleteNotFound)
                ) {
                    self.session(user).replaced_menus.push(message);
                    tracing::warn!("retired menu cleanup failed; will retry");
                }
            }
        }
    }
    // Telegram cannot edit messages carrying a reply keyboard. Keep the Rich
    // surface editable; a short-lived carrier owns the native location button.
    async fn sync_location_keyboard(
        &mut self,
        user: TelegramUserId,
        requested: bool,
    ) -> anyhow::Result<()> {
        let known_state = self.session(user).location_keyboard;
        if known_state == Some(requested) {
            return Ok(());
        }
        // At a fresh /start (including after a restart), there may be no reply
        // keyboard to remove. Do not send a visible "Готово" carrier to guess.
        if !requested && known_state.is_none() {
            self.session(user).location_keyboard = Some(false);
            return Ok(());
        }
        let text = if requested {
            "Отправьте местоположение кнопкой ниже."
        } else {
            "Готово"
        };
        let mut result = self
            .bot
            .send_message(ChatId(user.0), text)
            .disable_notification(true)
            .protect_content(true)
            .reply_markup(self.location_reply_markup(requested))
            .await;
        if self.icons_enabled && emoji_denied(&result) {
            self.icons_enabled = false;
            result = self
                .bot
                .send_message(ChatId(user.0), text)
                .disable_notification(true)
                .protect_content(true)
                .reply_markup(self.location_reply_markup(requested))
                .await;
        }
        let sent = result?;
        let session = self.session(user);
        session.location_keyboard = Some(requested);
        session.keyboard_messages.push(sent.id);
        if !requested {
            self.cleanup_keyboard_messages(user).await;
        }
        Ok(())
    }
    async fn cleanup_keyboard_messages(&mut self, user: TelegramUserId) {
        let messages = std::mem::take(&mut self.session(user).keyboard_messages);
        for message in messages {
            if self
                .bot
                .delete_message(ChatId(user.0), message)
                .await
                .is_err()
            {
                self.session(user).keyboard_messages.push(message);
                tracing::warn!("temporary keyboard cleanup failed; will retry");
            }
        }
    }
    fn retire_menu(&mut self, user: TelegramUserId, message: MessageId) {
        if let Some(surface) = self.surfaces.remove(&(user.0, message)) {
            for token in surface.tokens {
                self.registry.remove(&token);
            }
        }
    }
    async fn notice(&self, user: TelegramUserId, text: &str) -> anyhow::Result<()> {
        self.bot.send_message(ChatId(user.0), text).await?;
        Ok(())
    }
    async fn source(&self, user: TelegramUserId) -> anyhow::Result<()> {
        if let Ok(path) = std::env::var("SOURCE_ARCHIVE_PATH") {
            self.bot
                .send_document(ChatId(user.0), InputFile::file(path))
                .caption("Исходники работающей версии VeloHelp, инструкции сборки и лицензия. Зависимости закреплены в Cargo.lock.")
                .await?;
        } else {
            self.notice(user, &self.config.source_url).await?;
        }
        Ok(())
    }
    async fn home(&mut self, user: TelegramUserId) -> anyhow::Result<()> {
        self.home_with_notice(user, None).await
    }
    async fn home_with_notice(
        &mut self,
        user: TelegramUserId,
        notice: Option<&str>,
    ) -> anyhow::Result<()> {
        let session = self.session(user);
        session.editing_request = false;
        session.pending_arrival = None;
        session.arrival_text_input = false;
        let mut ui = Ui::column().push(Ui::paragraph("VeloHelp"));
        if let Some(notice) = notice {
            ui = ui.push(Ui::paragraph(notice));
        }
        if let Some(helper) = self.repo.helper(user).await? {
            ui = ui.push(Ui::paragraph(if helper.enabled {
                format!(
                    "Вы зарегистрированы как помощник · районов: {}. Уведомления включены.",
                    helper.areas.len()
                )
            } else {
                "Вы зарегистрированы как помощник. Уведомления на паузе.".to_owned()
            }));
        }
        ui = ui
            .push(button_row(Action::OpenCreateRequest))
            .push(button_row(Action::OpenHelperSettings));
        if !self.repo.active_requests(user).await?.is_empty() {
            ui = ui.push(button_row(Action::ActiveRequests));
        }
        self.menu(user, ui.push(button_row(Action::More))).await
    }
    async fn more(&mut self, user: TelegramUserId) -> anyhow::Result<()> {
        self.session(user).editing_request = false;
        self.menu(user, Ui::column()
            .push(Ui::paragraph("Ещё"))
            .push(button_row(Action::Privacy))
            .push(button_row(Action::OpenSource))
            .push(Ui::paragraph("Районы: © OpenStreetMap contributors, ODbL · https://www.openstreetmap.org/copyright"))
            .push(button_row(Action::Home))).await
    }
    async fn consent(&mut self, user: TelegramUserId) -> anyhow::Result<()> {
        let ui = Ui::column().push(Ui::paragraph("Перед началом"))
            .push(Ui::paragraph("Это тестовый бот взаимопомощи, не экстренная служба. Храним Telegram ID, настройки, заявки и время прибытия до закрытия пары. В приглашении помощники видят примерную зону около 1 км; точную точку и адрес — только принявший помощник. В паре имена открывают профили друг друга. При закрытии точка и введённый адрес удаляются из БД. Сообщения остаются в Telegram. Скачать или удалить данные можно в «Ещё → Мои данные»."))
            .push(button_row(Action::Consent(self.config.consent_version.clone())))
            .push(button_row(Action::OpenSource));
        self.menu(user, ui).await
    }
    #[allow(clippy::too_many_lines)]
    async fn message(&mut self, message: Message) -> anyhow::Result<()> {
        if !message.chat.is_private() {
            return Ok(());
        }
        let Some(from) = message.from.as_ref() else {
            return Ok(());
        };
        if from.is_bot || i64::try_from(from.id.0)? != message.chat.id.0 {
            return Ok(());
        }
        let user = TelegramUserId(i64::try_from(from.id.0)?);
        if self.ignored_users.contains(&user.0) {
            return Ok(());
        }
        let text = message.text().unwrap_or("");
        // Commands create a new primary screen at the bottom of the chat.
        // Callback navigation edits the message that carried the button.
        self.session(user).resurface_menu = matches!(
            text.split_whitespace().next(),
            Some("/start" | "/help" | "/active" | "/cancel")
        );
        // Source access must not require consent or persisted user identity.
        if text == "/source" || text == "/start source" {
            return self.source(user).await;
        }
        if !self
            .repo
            .has_consent(user, &self.config.consent_version)
            .await?
        {
            return self.consent(user).await;
        }
        self.session(user).display_name = Some(from.full_name());
        let operation = match text.split_whitespace().next().unwrap_or("") {
            "/start" | "/help" => "open_home",
            "/active" => "open_active",
            "/cancel" => "cancel_draft",
            _ if message.location().is_some() => "location_input",
            _ => "text_input",
        };
        tracing::info!(operation, "Telegram input received");
        match text.split_whitespace().next().unwrap_or("") {
            "/start" | "/help" => return self.home(user).await,
            "/active" => return self.active(user).await,
            "/source" => {
                return self.source(user).await;
            }
            "/cancel" => {
                let session = self.session(user);
                session.request = RequestDraft::default();
                session.location_warning = None;
                session.location_accuracy = None;
                session.last_location = None;
                return self.home(user).await;
            }
            _ => {}
        }
        let session = self.session(user);
        if session.arrival_text_input {
            if let Some((id, version)) = session.pending_arrival {
                let Some(estimate) = text
                    .trim()
                    .parse::<u16>()
                    .ok()
                    .and_then(|minutes| ArrivalEstimate::new(minutes, minutes))
                else {
                    return self
                        .notice(user, "Напишите число минут от 1 до 240, например 20.")
                        .await;
                };
                return self.confirm_accept(user, id, version, estimate).await;
            }
        }
        if !self.session(user).editing_request {
            return self
                .notice(user, "Нажмите «Нужна помощь», чтобы создать заявку.")
                .await;
        }
        let point = message
            .location()
            .and_then(|p| GeoPoint::new(p.latitude, p.longitude));
        if self.session(user).request.step == Some(RequestStep::Address) {
            let address = text.trim();
            if address.is_empty() || address.chars().count() > 200 {
                return self
                    .notice(user, "Напишите адрес длиной до 200 символов.")
                    .await;
            }
            let draft = &mut self.session(user).request;
            draft.exact_address = Some(address.to_owned());
            draft.step = Some(RequestStep::Preview);
            return self.wizard(user).await;
        }
        if self.session(user).request.step == Some(RequestStep::Location)
            && point.is_none()
            && !text.is_empty()
        {
            if text.chars().count() > 200 {
                return self
                    .notice(user, "Напишите улицу, дом и корпус — до 200 символов.")
                    .await;
            }
            let candidates = self.addresses.search(text);
            if candidates.is_empty() {
                return self.notice(user, "Адрес не найден. Укажите улицу, дом и корпус, например «Дачный проспект, 17 к3», или отправьте точку на карте.").await;
            }
            let unique = candidates.len() == 1;
            self.session(user).address_candidates = candidates;
            return if unique {
                self.select_address(user, 0).await
            } else {
                self.wizard(user).await
            };
        }
        let draft = &mut self
            .sessions
            .get_mut(&user.0)
            .expect("session exists")
            .0
            .request;
        let was_location = draft.step == Some(RequestStep::Location);
        let result = apply_input(draft, text, point, &self.districts);
        if let Err(reason) = result {
            return self.notice(user, reason).await;
        }
        if was_location {
            self.session(user).request.exact_address = None;
            self.session(user).address_candidates.clear();
            if point.is_some()
                && self
                    .bot
                    .delete_message(ChatId(user.0), message.id)
                    .await
                    .is_err()
            {
                tracing::warn!("requester location message cleanup failed");
            }
            let warning = point.and_then(|point| {
                location_warning(
                    &self.districts,
                    point,
                    message.location().and_then(|p| p.horizontal_accuracy),
                )
            });
            self.session(user).location_warning = warning;
            self.session(user).location_accuracy =
                message.location().and_then(|p| p.horizontal_accuracy);
            self.session(user).last_location = point;
        }
        self.wizard(user).await
    }
    async fn callback(&mut self, query: CallbackQuery) -> anyhow::Result<()> {
        let user = TelegramUserId(i64::try_from(query.from.id.0)?);
        if self.ignored_users.contains(&user.0) {
            self.bot.answer_callback_query(query.id).await?;
            return Ok(());
        }
        let resolved = query
            .message
            .as_ref()
            .filter(|m| m.chat().is_private() && m.chat().id.0 == user.0)
            .and_then(|message| self.surfaces.get(&(user.0, message.id())))
            .and_then(|surface| {
                query
                    .data
                    .as_deref()
                    .and_then(|data| ActionToken::try_from(data).ok())
                    .and_then(|token| {
                        self.registry
                            .resolve(&token, query.from.id, surface.view, surface.revision)
                            .ok()
                    })
            });
        let Some(record) = resolved else {
            self.bot
                .answer_callback_query(query.id)
                .text("Кнопка устарела. Откройте /start или /active.")
                .await?;
            return Ok(());
        };
        self.bot.answer_callback_query(query.id).await?;
        let operation = action_operation(&record.action);
        tracing::info!(operation, "Telegram action received");
        let needs_consent = !matches!(record.action, Action::Consent(_) | Action::OpenSource)
            && !self
                .repo
                .has_consent(user, &self.config.consent_version)
                .await?;
        self.session(user).display_name = Some(query.from.full_name());
        let session = self.session(user);
        session.resurface_menu = false;
        session.callback_menu = query
            .message
            .as_ref()
            .map(teloxide::types::MaybeInaccessibleMessage::id);
        // Keep the command router's large future off the polling/test stack.
        let result = if needs_consent {
            Box::pin(self.consent(user)).await
        } else {
            Box::pin(self.action(user, record.action)).await
        };
        if let Some((session, _)) = self.sessions.get_mut(&user.0) {
            session.callback_menu = None;
        }
        if let Err(error) = &result {
            tracing::warn!(
                operation,
                reason = error_reason(error),
                "Telegram action failed"
            );
            self.notice(user,"Действие не выполнено или заявка уже изменилась. Откройте /active; черновик можно продолжить.").await?;
        }
        result
    }
    // Keep typed command routing exhaustive in one place.
    #[allow(clippy::too_many_lines)]
    async fn action(&mut self, user: TelegramUserId, action: Action) -> anyhow::Result<()> {
        match action {
            Action::Home => self.home(user).await?,
            Action::More => self.more(user).await?,
            Action::HelperOptions => self.helper_options_view(user).await?,
            Action::HelperAreas => self.helper_view(user).await?,
            Action::Consent(version) => {
                anyhow::ensure!(version == self.config.consent_version, "old consent");
                self.repo
                    .ensure_user(user, &version, chrono::Utc::now().timestamp())
                    .await?;
                self.home(user).await?;
            }
            Action::OpenSource => {
                self.source(user).await?;
            }
            Action::OpenCreateRequest => {
                let session = self.session(user);
                session.editing_request = true;
                session.request.kind.get_or_insert(HelpKind::Technical);
                if session.request.step.is_none() {
                    session.request.next();
                }
                self.wizard(user).await?;
            }
            Action::OpenHelperSettings => {
                if self.session(user).helper.is_some() {
                    self.session(user).editing_request = false;
                    return self.helper_view(user).await;
                }
                let profile = self.repo.helper(user).await?;
                let draft = profile.map_or(
                    HelperDraft {
                        enabled: true,
                        kinds: vec![HelpKind::Technical],
                        problem_tags: crate::domain::HELP_PROBLEMS
                            .iter()
                            .filter(|tag| **tag != "accident")
                            .map(|tag| (*tag).to_owned())
                            .collect(),
                        ..Default::default()
                    },
                    |p| {
                        let problems = if p.problem_tags.is_empty() {
                            crate::domain::HELP_PROBLEMS
                                .iter()
                                .filter(|tag| {
                                    crate::domain::problem_kind(tag)
                                        .is_some_and(|kind| p.help_kinds.contains(&kind))
                                })
                                .map(|tag| (*tag).to_owned())
                                .collect()
                        } else {
                            p.problem_tags
                        };
                        HelperDraft {
                            areas: p.areas,
                            kinds: p.help_kinds,
                            enabled: p.enabled,
                            problem_tags: problems,
                        }
                    },
                );
                let session = self.session(user);
                session.helper = Some(draft);
                session.editing_request = false;
                self.helper_view(user).await?;
            }
            Action::ToggleArea(area) => {
                anyhow::ensure!(
                    self.repo.areas().await?.iter().any(|a| a.id == area),
                    "unknown area"
                );
                self.session(user)
                    .helper
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("missing helper draft"))?
                    .toggle_area(&area);
                self.helper_view(user).await?;
            }
            Action::ToggleCapability(raw) => {
                let kind =
                    HelpKind::from_str(&raw).ok_or_else(|| anyhow::anyhow!("invalid kind"))?;
                self.session(user)
                    .helper
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("missing helper draft"))?
                    .toggle_kind(kind);
                self.helper_options_view(user).await?;
            }
            Action::ToggleHelperProblem(tag) => {
                anyhow::ensure!(
                    crate::domain::problem_kind(&tag).is_some(),
                    "unknown helper problem"
                );
                self.session(user)
                    .helper
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("missing helper draft"))?
                    .toggle_problem(&tag);
                self.helper_view(user).await?;
            }
            Action::ToggleEnabled => {
                let helper = self
                    .session(user)
                    .helper
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("missing helper draft"))?;
                helper.enabled = !helper.enabled;
                self.helper_options_view(user).await?;
            }
            Action::SaveHelper => {
                let helper = self
                    .session(user)
                    .helper
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("missing helper draft"))?;
                if helper.enabled && helper.areas.is_empty() {
                    return self.notice(user, "Выберите хотя бы один район.").await;
                }
                if helper.enabled && helper.kinds.is_empty() {
                    return self
                        .notice(
                            user,
                            "Выберите тип помощи: техническая помощь или происшествие.",
                        )
                        .await;
                }
                self.repo
                    .upsert_helper(&HelperProfile {
                        user_id: user,
                        enabled: helper.enabled,
                        areas: helper.areas,
                        help_kinds: helper.kinds,
                        problem_tags: helper.problem_tags,
                    })
                    .await?;
                self.session(user).helper = None;
                self.home_with_notice(user, Some("Настройки помощника сохранены."))
                    .await?;
            }
            Action::SelectProblem(problem) => {
                select_problem(&mut self.session(user).request, &problem)
                    .map_err(anyhow::Error::msg)?;
                self.wizard(user).await?;
            }
            Action::EditDescription => {
                self.session(user).request.step = Some(RequestStep::Description);
                self.wizard(user).await?;
            }
            Action::EditAddress => {
                anyhow::ensure!(
                    self.session(user).request.can_publish(),
                    "address needs location"
                );
                self.session(user).request.step = Some(RequestStep::Address);
                self.wizard(user).await?;
            }
            Action::SelectAddress(index) => self.select_address(user, index).await?,
            Action::WizardBack => {
                self.session(user).request.back();
                self.wizard(user).await?;
            }
            Action::WizardNext => {
                let draft = &mut self.session(user).request;
                if can_advance(draft) {
                    draft.next();
                    self.wizard(user).await?;
                } else {
                    self.notice(
                        user,
                        "Заполните текущий шаг; для места нужна отправленная геопозиция.",
                    )
                    .await?;
                }
            }
            Action::PublishRequest => {
                if !self.limiter.allow(user.0, Scope::Publish) {
                    return self
                        .notice(user, "Лимит публикаций исчерпан. Попробуйте позже.")
                        .await;
                }
                let draft = self.session(user).request.clone();
                anyhow::ensure!(
                    draft.step == Some(RequestStep::Preview) && draft.can_publish(),
                    "invalid draft"
                );
                let mut request = HelpRequest::draft(
                    user,
                    draft.kind.ok_or_else(|| anyhow::anyhow!("missing kind"))?,
                    draft.description,
                    chrono::Utc::now() + chrono::TimeDelta::from_std(self.config.default_ttl)?,
                );
                request.status = HelpRequestStatus::Open;
                request.problem_tags = draft.tags;
                request.bike_description = draft.bike_description;
                request.exact_point = draft.exact_point;
                request.exact_address = draft.exact_address;
                request.area_id = draft.area_id;
                self.repo.insert(&request).await?;
                tracing::info!(
                    operation = "publish_request",
                    "Request committed; updating card and notifying helpers"
                );
                self.metrics
                    .requests_created
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let session = self.session(user);
                session.request = RequestDraft::default();
                session.location_warning = None;
                session.editing_request = false;
                let projection = self.show_request(user, &request).await;
                let delivery = self.deliver().await;
                if let Err(error) = projection {
                    tracing::warn!(
                        operation = "publish_request_card",
                        reason = error_reason(&error),
                        "Request committed but requester card failed"
                    );
                    return self.notice(user, "Заявка опубликована. Карточка временно не обновилась — откройте /active. Повторно отправлять заявку не нужно.").await;
                }
                delivery?;
            }
            Action::Accept(id, version) => {
                if self.session(user).location_keyboard == Some(true) {
                    self.home(user).await?;
                }
                let request = self.repo.request(id).await?;
                anyhow::ensure!(
                    request.status == HelpRequestStatus::Open && request.version == version,
                    "stale arrival choice"
                );
                let session = self.session(user);
                session.pending_arrival = Some((id, version));
                session.arrival_text_input = false;
                session.editing_request = false;
                self.refresh_actor(user, id).await?;
            }
            Action::AcceptAt(id, version, estimate) => {
                self.confirm_accept(user, id, version, estimate).await?;
            }
            Action::EnterArrival(id, version) => {
                let session = self.session(user);
                session.pending_arrival = Some((id, version));
                session.arrival_text_input = true;
                self.refresh_actor(user, id).await?;
            }
            Action::ArrivalBack(id, version) => {
                let session = self.session(user);
                session.pending_arrival = Some((id, version));
                session.arrival_text_input = false;
                self.refresh_actor(user, id).await?;
            }
            Action::AbortAccept(id) => {
                let session = self.session(user);
                session.pending_arrival = None;
                session.arrival_text_input = false;
                self.refresh_actor(user, id).await?;
            }
            Action::Decline(id, version) => {
                let session = self.session(user);
                session.pending_arrival = None;
                session.arrival_text_input = false;
                self.repo.decline(id, user, version).await?;
                self.notice(user, "Уведомление отклонено.").await?;
            }
            Action::Cancel(id) => {
                self.repo.cancel(id, user).await?;
                self.refresh(id).await?;
            }
            Action::Resolve(id) => {
                self.repo.resolve(id, user).await?;
                self.refresh(id).await?;
            }
            Action::Withdraw(id) => {
                self.repo.withdraw(id, user).await?;
                self.refresh(id).await?;
                self.deliver().await?;
            }
            Action::ActiveRequests => self.active(user).await?,
            Action::OpenRequest(id) => {
                let request = self
                    .repo
                    .active_requests(user)
                    .await?
                    .into_iter()
                    .find(|r| r.id == id)
                    .ok_or_else(|| anyhow::anyhow!("request unavailable"))?;
                self.show_request(user, &request).await?;
            }
            Action::Privacy => {
                let ui=Ui::column().push(Ui::paragraph("Мои данные")).push(Ui::paragraph("Можно скачать все хранимые данные или удалить их. Удаление отменит заявки и активную пару. Резервные копии истекают по политике хранения; сообщения в Telegram этим действием не удаляются.")) .push(button_row(Action::ExportData)).push(button_row(Action::ConfirmDelete)).push(button_row(Action::Home));
                self.menu(user, ui).await?;
            }
            Action::ExportData => {
                let data = self.repo.export_user(user).await?;
                self.bot
                    .send_document(
                        ChatId(user.0),
                        InputFile::memory(data.into_bytes()).file_name("velohelp-data.json"),
                    )
                    .protect_content(true)
                    .await?;
            }
            Action::ConfirmDelete => {
                self.menu(user,Ui::column().push(Ui::paragraph("Удалить данные?" )).push(Ui::paragraph("Заявки и настройки будут удалены, активная пара отменена. Это действие необратимо." )).push(button_row(Action::DeleteData)).push(button_row(Action::Home))).await?;
            }
            Action::DeleteData => {
                let requests = self.repo.active_requests(user).await?;
                self.repo.delete_user(user).await?;
                // Erase ephemeral personal data immediately after commit, even
                // when another participant's subsequent projection fails.
                self.sessions.remove(&user.0);
                self.surfaces.retain(|_, s| {
                    if s.owner == user {
                        for token in &s.tokens {
                            self.registry.remove(token);
                        }
                        false
                    } else {
                        true
                    }
                });
                for request in requests {
                    self.refresh(request.id).await?;
                }
                self.notice(user, "Ваши данные удалены. /start — начать заново.")
                    .await?;
            }
        }
        Ok(())
    }
    async fn helper_view(&mut self, user: TelegramUserId) -> anyhow::Result<()> {
        let session = self.session(user);
        let helper = session
            .helper
            .clone()
            .ok_or_else(|| anyhow::anyhow!("missing helper draft"))?;
        let mut ui = Ui::column()
            .push(Ui::paragraph("Где и чем можете помочь?"))
            .push(Ui::paragraph(format!(
                "{} районов · выбрано {}. Нажмите «Сохранить», чтобы получать заявки.",
                self.areas.len(),
                helper.areas.len()
            )));
        for pair in self.areas.chunks(2) {
            let mut row = Ui::button_row();
            for area in pair {
                let mark = if helper.areas.contains(&area.id) {
                    "✓ "
                } else {
                    ""
                };
                row = row.push(Ui::button(
                    format!("{mark}{}", area.name),
                    Action::ToggleArea(area.id.clone()),
                ));
            }
            ui = ui.push(row);
        }
        ui = ui.push(helper_options(&helper)).push(
            Ui::button_row()
                .push(helper_save_button(&helper))
                .push(Ui::button("Главная", Action::Home)),
        );
        self.menu(user, ui).await
    }
    async fn helper_options_view(&mut self, user: TelegramUserId) -> anyhow::Result<()> {
        self.helper_view(user).await
    }
    async fn select_address(&mut self, user: TelegramUserId, index: usize) -> anyhow::Result<()> {
        let candidate = self
            .session(user)
            .address_candidates
            .get(index)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("address selection expired"))?;
        let draft = &mut self
            .sessions
            .get_mut(&user.0)
            .expect("session exists")
            .0
            .request;
        if let Err(reason) = apply_input(draft, "", Some(candidate.point), &self.districts) {
            return self.notice(user, reason).await;
        }
        let session = self.session(user);
        session.request.exact_address = Some(candidate.label);
        session.address_candidates.clear();
        session.location_accuracy = None;
        session.last_location = Some(candidate.point);
        session.location_warning = Some("Точка найдена по адресу здания, а не по GPS. При необходимости уточните место на карте.".to_owned());
        self.wizard(user).await
    }
    async fn wizard(&mut self, user: TelegramUserId) -> anyhow::Result<()> {
        let draft = self.session(user).request.clone();
        let location_warning = self.session(user).location_warning.clone();
        let accuracy = self.session(user).location_accuracy;
        let mut ui = Ui::column();
        match draft.step.unwrap_or(RequestStep::Description) {
            RequestStep::Description => {
                ui = ui.push(Ui::paragraph("Что случилось?"))
                    .push(Ui::paragraph("Выберите проблему или напишите свою. Без адреса, координат и телефона — описание увидят помощники."));
                for problems in PROBLEMS.chunks(2) {
                    let mut row = Ui::button_row();
                    for (problem, _) in problems {
                        let action = Action::SelectProblem((*problem).to_owned());
                        row = row.push(Ui::button(views::action_label(&action), action));
                    }
                    ui = ui.push(row);
                }
            }
            RequestStep::Location => {
                ui = ui.push(Ui::paragraph("Где вы?"))
                    .push(Ui::paragraph("Отправьте геопозицию кнопкой ниже или напишите улицу, дом и корпус. Точку на карте можно выбрать через скрепку → Геопозиция. Район определим сами; точку и адрес увидит только принявший помощник."));
                let candidates = self.session(user).address_candidates.clone();
                if !candidates.is_empty() {
                    ui = ui.push(Ui::paragraph("Какой адрес?"));
                    for (index, candidate) in candidates.iter().enumerate() {
                        ui = ui.push(Ui::button_row().push(Ui::button(
                            candidate.label.as_str(),
                            Action::SelectAddress(index),
                        )));
                    }
                }
                if draft.kind == Some(HelpKind::Accident) {
                    ui = ui.push(Ui::paragraph(
                        "При угрозе жизни обращайтесь в экстренную службу.",
                    ));
                }
                if draft.can_publish() {
                    ui = ui.push(
                        Ui::button_row()
                            .push(Ui::button("Оставить прежнюю точку", Action::WizardNext)),
                    );
                }
                ui = ui.push(button_row(Action::WizardBack));
            }
            RequestStep::Preview => {
                ui = ui
                    .push(Ui::paragraph("Отправить заявку?"))
                    .push(Ui::paragraph(format!(
                        "{}\n{}\nРайон: {}",
                        draft.kind.map_or("—", kind_label),
                        draft.description,
                        self.area_name(draft.area_id.as_deref())
                    )));
                if let Some(address) = &draft.exact_address {
                    ui = ui.push(Ui::paragraph(format!("Адрес (указан вами): {address}")));
                } else if let Some(address) = draft
                    .exact_point
                    .and_then(|point| self.addresses.resolve_with_accuracy(point, accuracy))
                {
                    ui = ui.push(Ui::paragraph(address));
                }
                if let Some(warning) = location_warning {
                    ui = ui.push(Ui::paragraph(warning));
                }
                ui = ui
                    .push(Ui::paragraph("До принятия помощники увидят примерную зону около 1 км на карте; точную точку и адрес — после принятия."))
                    .push(button_row(Action::PublishRequest))
                    .push(button_row(Action::EditDescription))
                    .push(button_row(Action::EditAddress))
                    .push(Ui::button_row().push(Ui::button("Изменить место", Action::WizardBack)));
            }
            RequestStep::Address => {
                ui = ui.push(Ui::paragraph("Уточните адрес"))
                    .push(Ui::paragraph("Напишите улицу, дом и корпус, можно добавить ориентир или подъезд. Точка на карте останется прежней. Адрес увидит только принявший помощник."))
                    .push(button_row(Action::WizardBack));
            }
        }
        self.menu(user, ui.push(button_row(Action::Home))).await
    }
    fn area_name(&self, id: Option<&str>) -> &str {
        self.areas
            .iter()
            .find(|area| Some(area.id.as_str()) == id)
            .map_or("Район не указан", |area| area.name.as_str())
    }
    fn request_ui(&self, request: &HelpRequest, viewer: TelegramUserId) -> Ui<Action> {
        if request.status == HelpRequestStatus::Open && request.requester_id != viewer {
            if let Some((session, _)) = self.sessions.get(&viewer.0) {
                if session.pending_arrival == Some((request.id, request.version)) {
                    return arrival_view(request, session.arrival_text_input);
                }
            }
        }
        if request.requester_id == viewer {
            views::matched_requester_view(request)
        } else if request.status == HelpRequestStatus::Matched
            && request.matched_helper_id == Some(viewer)
        {
            views::matched_helper_view(request, viewer)
        } else {
            views::dispatch_card_view(request, self.area_name(request.area_id.as_deref()))
        }
    }
    async fn decorate_match(
        &mut self,
        user: TelegramUserId,
        request: &HelpRequest,
        rich: &mut teloxide::types::InputRichMessage,
    ) -> anyhow::Result<()> {
        use teloxide::types::{InputRichBlock, InputRichBlockParagraph, RichText, RichTextUrl};
        if request.status != HelpRequestStatus::Matched {
            return Ok(());
        }
        let (peer, role) = if request.requester_id == user {
            let Some(helper) = request.matched_helper_id else {
                return Ok(());
            };
            (helper, "Поможет: ")
        } else if request.matched_helper_id == Some(user) {
            (request.requester_id, "Кому помочь: ")
        } else {
            return Ok(());
        };
        let name = self.peer_name(peer).await;
        let mut details = vec![InputRichBlock::Paragraph(InputRichBlockParagraph {
            text: RichText::List(vec![
                RichText::from(role),
                RichText::from(RichTextUrl {
                    text: Box::new(RichText::from(name)),
                    url: format!("tg://user?id={}", peer.0),
                }),
            ]),
        })];
        if let Some(window) = request.arrival_window {
            let offset = chrono::FixedOffset::east_opt(3 * 3600).expect("Moscow UTC offset");
            let from = window
                .earliest
                .with_timezone(&offset)
                .format("%H:%M")
                .to_string();
            let until = window
                .latest
                .with_timezone(&offset)
                .format("%H:%M")
                .to_string();
            details.push(InputRichBlock::Paragraph(InputRichBlockParagraph {
                text: RichText::from(if from == until {
                    format!("Прибытие: {from}")
                } else {
                    format!("Прибытие: {from}–{until}")
                }),
            }));
        }
        if let Some(point) = request.exact_point {
            let accuracy = self
                .sessions
                .get(&request.requester_id.0)
                .filter(|(s, _)| s.last_location == Some(point))
                .and_then(|(s, _)| s.location_accuracy);
            let address = request
                .exact_address
                .as_ref()
                .map(|address| format!("Адрес (указан заявителем): {address}"))
                .or_else(|| self.addresses.resolve_with_accuracy(point, accuracy))
                .unwrap_or_else(|| "Адрес не найден — ориентируйтесь по карте.".to_owned());
            details.push(InputRichBlock::Paragraph(InputRichBlockParagraph {
                text: RichText::from(address),
            }));
            details.push(map_block(point, None, 16));
        }
        insert_before_actions(rich, details);
        Ok(())
    }
    async fn peer_name(&mut self, peer: TelegramUserId) -> String {
        if let Some(name) = self
            .sessions
            .get(&peer.0)
            .and_then(|(s, _)| s.display_name.clone())
        {
            return name;
        }
        let name = self
            .bot
            .get_chat(ChatId(peer.0))
            .await
            .ok()
            .and_then(|chat| {
                chat.first_name().map(|first| {
                    chat.last_name()
                        .map_or_else(|| first.to_owned(), |last| format!("{first} {last}"))
                })
            })
            .unwrap_or_else(|| "Участник".to_owned());
        self.session(peer).display_name = Some(name.clone());
        name
    }
    async fn confirm_accept(
        &mut self,
        user: TelegramUserId,
        id: HelpRequestId,
        version: u64,
        estimate: ArrivalEstimate,
    ) -> anyhow::Result<()> {
        if !self.limiter.allow(user.0, Scope::Accept) {
            return self
                .notice(user, "Слишком много попыток. Подождите минуту.")
                .await;
        }
        let outcome = self
            .repo
            .accept_with_arrival(id, user, version, Some(estimate))
            .await?;
        let session = self.session(user);
        session.pending_arrival = None;
        session.arrival_text_input = false;
        if matches!(outcome, AcceptOutcome::Won | AcceptOutcome::AlreadyTaken) {
            self.metrics.record_accept(outcome == AcceptOutcome::Won);
        }
        if outcome != AcceptOutcome::Won {
            self.notice(
                user,
                match outcome {
                    AcceptOutcome::AlreadyTaken => "Заявку уже принял другой помощник.",
                    AcceptOutcome::Expired => "Срок заявки истёк.",
                    _ => "Кнопка устарела или заявка недоступна.",
                },
            )
            .await?;
        }
        self.refresh(id).await
    }
    async fn refresh_actor(
        &mut self,
        user: TelegramUserId,
        id: HelpRequestId,
    ) -> anyhow::Result<()> {
        let request = self.repo.request(id).await?;
        let messages: Vec<_> = self
            .surfaces
            .iter()
            .filter(|(_, s)| s.owner == user && s.request == Some(id))
            .map(|((_, m), _)| *m)
            .collect();
        for message in messages {
            self.project(
                user,
                self.request_ui(&request, user),
                Some(id),
                Some(message),
            )
            .await?;
        }
        Ok(())
    }
    async fn active(&mut self, user: TelegramUserId) -> anyhow::Result<()> {
        let session = self.session(user);
        session.pending_arrival = None;
        session.arrival_text_input = false;
        session.editing_request = false;
        let requests = self.repo.active_requests(user).await?;
        if let [request] = requests.as_slice() {
            return self.show_request(user, request).await;
        }
        let mut ui = Ui::column().push(Ui::paragraph(if requests.is_empty() {
            "У вас нет активных заявок или принятых пар."
        } else {
            "Активные заявки"
        }));
        if requests.is_empty() {
            ui = ui.push(button_row(Action::OpenCreateRequest));
        }
        for request in requests {
            ui = ui.push(Ui::button_row().push(Ui::button(
                format!(
                    "{} · {}",
                    views::status_text(request.status),
                    request.description.chars().take(70).collect::<String>()
                ),
                Action::OpenRequest(request.id),
            )));
        }
        self.menu(user, ui.push(button_row(Action::Home))).await
    }
    async fn show_request(
        &mut self,
        user: TelegramUserId,
        request: &HelpRequest,
    ) -> anyhow::Result<()> {
        let session = self.session(user);
        session.editing_request = false;
        session.pending_arrival = None;
        session.arrival_text_input = false;
        self.show(user, self.request_ui(request, user), Some(request.id))
            .await?;
        Box::pin(self.sync_location_keyboard(user, false)).await?;
        // Opening an invitation through /active must not leave another copy
        // of the same card. Retire it only after the primary surface succeeds.
        let menu = self.session(user).menu;
        let duplicates: Vec<_> = self
            .surfaces
            .iter()
            .filter(|((owner, message), s)| {
                *owner == user.0 && Some(*message) != menu && s.request == Some(request.id)
            })
            .map(|((_, message), _)| *message)
            .collect();
        for message in duplicates {
            self.retire_menu(user, message);
            let _ = self.bot.delete_message(ChatId(user.0), message).await;
        }
        Ok(())
    }
    async fn refresh(&mut self, id: HelpRequestId) -> anyhow::Result<()> {
        let request = match self.repo.request(id).await {
            Ok(request) => Some(request),
            Err(crate::application::AppError::NotFound) => None,
            Err(err) => return Err(err.into()),
        };
        for (session, _) in self.sessions.values_mut() {
            if let Some((pending, version)) = session.pending_arrival {
                if pending == id
                    && request
                        .as_ref()
                        .is_none_or(|r| r.status != HelpRequestStatus::Open || r.version != version)
                {
                    session.pending_arrival = None;
                    session.arrival_text_input = false;
                }
            }
        }
        let surfaces: Vec<_> = self
            .surfaces
            .iter()
            .filter(|(_, s)| s.request == Some(id))
            .map(|((_, m), s)| (s.owner, *m))
            .collect();
        for (user, message) in surfaces {
            let ui = match &request {
                Some(request) => self.request_ui(request, user),
                None => Ui::column()
                    .push(Ui::paragraph("Заявка удалена."))
                    .push(button_row(Action::Home)),
            };
            if self
                .project(user, ui, request.as_ref().map(|r| r.id), Some(message))
                .await
                .is_err()
            {
                tracing::warn!("request projection failed; recover with /active");
            }
        }
        Ok(())
    }
    async fn deliver(&mut self) -> anyhow::Result<()> {
        let retired: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, (s, _))| !s.replaced_menus.is_empty())
            .map(|(user, _)| TelegramUserId(*user))
            .collect();
        for user in retired {
            self.cleanup_replaced_menus(user).await;
        }
        let cleanup: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, (s, _))| {
                s.location_keyboard == Some(false) && !s.keyboard_messages.is_empty()
            })
            .map(|(user, _)| TelegramUserId(*user))
            .collect();
        for user in cleanup {
            self.cleanup_keyboard_messages(user).await;
        }
        let ids: std::collections::HashSet<_> =
            self.surfaces.values().filter_map(|s| s.request).collect();
        for id in ids {
            let version = match self.repo.request(id).await {
                Ok(request) => Some(request.version),
                Err(crate::application::AppError::NotFound) => None,
                Err(err) => return Err(err.into()),
            };
            if self.surfaces.values().any(|s| {
                s.request == Some(id) && (s.request_version != version || version.is_none())
            }) {
                self.refresh(id).await?;
            }
        }
        self.repo.prepare_dispatches().await?;
        for delivery in self.repo.pending_deliveries().await? {
            let ui = views::dispatch_card_view(
                &delivery.request,
                self.area_name(delivery.request.area_id.as_deref()),
            );
            let previous = self
                .surfaces
                .iter()
                .find(|(_, s)| s.owner == delivery.helper && s.request == Some(delivery.request.id))
                .map(|((_, message), _)| *message);
            let message = match self
                .project(delivery.helper, ui, Some(delivery.request.id), previous)
                .await
            {
                Ok(message) => message,
                Err(error) => {
                    let reason = error_reason(&error);
                    tracing::warn!(
                        operation = "notify_helper",
                        reason,
                        "notification delivery failed; retrying on the next polling pass"
                    );
                    // An outage must not serially spend one HTTP timeout on
                    // every helper before returning to interactive polling.
                    if matches!(
                        reason,
                        "network_timeout"
                            | "network_connect"
                            | "network_transport"
                            | "telegram_response_decode"
                            | "telegram_rate_limit"
                    ) {
                        return Err(error);
                    }
                    continue;
                }
            };
            self.repo
                .mark_delivered(
                    delivery.request.id,
                    delivery.helper,
                    delivery.request.version,
                    message.0,
                )
                .await?;
        }
        Ok(())
    }
}

// Only fixed categories reach logs: transport errors can contain token URLs,
// response bodies can contain coordinates and names, and SQL can contain IDs.
fn action_operation(action: &Action) -> &'static str {
    match action {
        Action::OpenCreateRequest => "open_create_request",
        Action::OpenHelperSettings => "open_helper_settings",
        Action::ToggleArea(_) => "toggle_helper_area",
        Action::ToggleCapability(_) => "toggle_helper_kind",
        Action::ToggleHelperProblem(_) => "toggle_helper_problem",
        Action::ToggleEnabled => "toggle_notifications",
        Action::SaveHelper => "save_helper",
        Action::SelectProblem(_) => "select_request_problem",
        Action::SelectAddress(_) => "select_address",
        Action::EditAddress => "edit_address",
        Action::EditDescription => "edit_description",
        Action::WizardNext => "wizard_next",
        Action::WizardBack => "wizard_back",
        Action::PublishRequest => "publish_request",
        Action::Accept(_, _) => "open_arrival",
        Action::AcceptAt(_, _, _) => "accept_request",
        Action::EnterArrival(_, _) => "enter_arrival",
        Action::ArrivalBack(_, _) | Action::AbortAccept(_) => "leave_arrival",
        Action::Decline(_, _) => "decline_request",
        Action::Cancel(_) => "cancel_request",
        Action::Resolve(_) => "resolve_request",
        Action::Withdraw(_) => "withdraw_help",
        Action::Home => "open_home",
        Action::More => "open_more",
        Action::HelperAreas | Action::HelperOptions => "open_helper_options",
        Action::ActiveRequests => "open_active",
        Action::OpenRequest(_) => "open_request",
        Action::OpenSource => "open_source",
        Action::Consent(_) => "consent",
        Action::Privacy => "open_privacy",
        Action::ExportData => "export_data",
        Action::ConfirmDelete => "confirm_delete",
        Action::DeleteData => "delete_data",
    }
}

fn error_reason(error: &anyhow::Error) -> &'static str {
    use teloxide::RequestError;
    if let Some(error) = error.downcast_ref::<RequestError>() {
        return match error {
            RequestError::Validation(_) => "telegram_request_validation",
            RequestError::Api(error) => {
                let description = error.to_string().to_ascii_lowercase();
                if description.contains("message is not modified") {
                    "message_not_modified"
                } else if description.contains("message to edit not found") {
                    "message_missing"
                } else if description.contains("can't be edited") {
                    "message_uneditable"
                } else if description.contains("blocked") || description.contains("forbidden") {
                    "telegram_forbidden"
                } else if description.contains("emoji") || description.contains("premium") {
                    "custom_emoji_denied"
                } else {
                    "telegram_api_rejected"
                }
            }
            RequestError::Network(error) if error.is_timeout() => "network_timeout",
            RequestError::Network(error) if error.is_connect() => "network_connect",
            RequestError::Network(_) => "network_transport",
            RequestError::InvalidJson { .. } => "telegram_response_decode",
            RequestError::RetryAfter(_) => "telegram_rate_limit",
            RequestError::MigrateToChatId(_) => "chat_migrated",
            RequestError::Io(_) => "transport_io",
        };
    }
    if error
        .downcast_ref::<crate::application::AppError>()
        .is_some()
    {
        "domain_or_storage"
    } else if error.downcast_ref::<sqlx::Error>().is_some() {
        "storage"
    } else {
        "internal_validation"
    }
}

fn decorate_invitation(
    user: TelegramUserId,
    request: &HelpRequest,
    rich: &mut teloxide::types::InputRichMessage,
) {
    use teloxide::types::{InputRichBlock, InputRichBlockParagraph, RichText};
    if request.status != HelpRequestStatus::Open
        || request.requester_id == user
        || request.expires_at <= chrono::Utc::now()
    {
        return;
    }
    let Some(zone) = request
        .exact_point
        .and_then(ApproximateLocation::from_point)
    else {
        return;
    };
    insert_before_actions(rich, vec![
        InputRichBlock::Paragraph(InputRichBlockParagraph {
            text: RichText::from("Примерное место · зона около 1 × 1 км. Маркер — ориентир; точная точка и адрес появятся после принятия."),
        }),
        map_block(zone.center, Some(f64::from(zone.radius_meters)), 13),
    ]);
}

fn map_block(point: GeoPoint, accuracy: Option<f64>, zoom: u8) -> teloxide::types::InputRichBlock {
    use teloxide::types::{InputRichBlock, InputRichBlockMap, Location};
    InputRichBlock::Map(InputRichBlockMap {
        location: Location {
            latitude: point.lat,
            longitude: point.lon,
            horizontal_accuracy: accuracy,
            live_period: None,
            heading: None,
            proximity_alert_radius: None,
        },
        zoom,
        width: 512,
        height: 256,
        caption: None,
    })
}

fn insert_before_actions(
    rich: &mut teloxide::types::InputRichMessage,
    details: Vec<teloxide::types::InputRichBlock>,
) {
    use teloxide::types::{InputRichBlock, InputRichMessage};
    if let Some(blocks) = rich.blocks_ref() {
        let mut blocks = blocks.to_vec();
        let position = blocks
            .iter()
            .position(|b| matches!(b, InputRichBlock::Buttons(_)))
            .unwrap_or(blocks.len());
        blocks.splice(position..position, details);
        *rich = InputRichMessage::blocks(blocks);
    }
}

fn missing_or_uneditable(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<teloxide::RequestError>(),
        Some(teloxide::RequestError::Api(
            teloxide::ApiError::MessageToEditNotFound | teloxide::ApiError::MessageCantBeEdited
        ))
    )
}
fn emoji_denied(result: &Result<Message, teloxide::RequestError>) -> bool {
    matches!(result, Err(teloxide::RequestError::Api(error)) if ["emoji", "premium", "button_icon"].iter()
        .any(|part| error.to_string().to_ascii_lowercase().contains(part)))
}

fn arrival_view(request: &HelpRequest, input: bool) -> Ui<Action> {
    let mut ui = Ui::column()
        .push(Ui::paragraph(if input {
            "Через сколько минут сможете помочь?"
        } else {
            "Помогу через…"
        }))
        .push(Ui::paragraph(request.description.clone()));
    if input {
        ui = ui
            .push(Ui::paragraph(
                "Напишите число минут от 1 до 240, например 20.",
            ))
            .push(button_row(Action::ArrivalBack(request.id, request.version)));
    } else {
        for pair in [(5, 15), (15, 30), (30, 60), (60, 120)].chunks(2) {
            let mut row = Ui::button_row();
            for &(min, max) in pair {
                row = row.push(Ui::button(
                    ArrivalEstimate::new(min, max)
                        .expect("valid preset")
                        .label(),
                    Action::AcceptAt(
                        request.id,
                        request.version,
                        ArrivalEstimate::new(min, max).expect("valid preset"),
                    ),
                ));
            }
            ui = ui.push(row);
        }
        ui = ui.push(button_row(Action::EnterArrival(
            request.id,
            request.version,
        )));
    }
    ui.push(
        Ui::button_row()
            .push(Ui::button("Назад", Action::AbortAccept(request.id)))
            .push(Ui::button(
                "Отклонить",
                Action::Decline(request.id, request.version),
            )),
    )
}

const PROBLEMS: &[(&str, &str)] = &[
    ("puncture", "Прокол. Нужна помощь."),
    (
        "drivetrain",
        "Проблема с цепью или передачами. Нужна помощь.",
    ),
    ("brakes", "Проблема с тормозами. Нужна помощь."),
    ("wheel", "Проблема с колесом. Нужна помощь."),
    ("tools", "Нужны инструменты."),
    ("transport", "Нужна помощь с перевозкой велосипеда."),
    ("accident", "Происшествие. Нужна помощь."),
];

fn helper_options(helper: &HelperDraft) -> teloxide_ui::UiNode<Action> {
    let mut kinds = Ui::button_row();
    for kind in [HelpKind::Technical, HelpKind::Accident] {
        let mark = if crate::domain::HELP_PROBLEMS
            .iter()
            .filter(|tag| crate::domain::problem_kind(tag) == Some(kind))
            .all(|tag| helper.problem_tags.iter().any(|selected| selected == tag))
        {
            "✓ "
        } else {
            ""
        };
        kinds = kinds.push(Ui::button(
            format!(
                "{mark}{}",
                if kind == HelpKind::Technical {
                    "Все поломки"
                } else {
                    "Происшествие"
                }
            ),
            Action::ToggleCapability(kind.as_str().to_owned()),
        ));
    }
    let mut ui = Ui::column().push(kinds);
    for tags in crate::domain::HELP_PROBLEMS[..6].chunks(2) {
        let mut row = Ui::button_row();
        for tag in tags {
            let action = Action::ToggleHelperProblem((*tag).to_owned());
            let mark = if helper.problem_tags.iter().any(|selected| selected == tag) {
                "✓ "
            } else {
                ""
            };
            row = row.push(Ui::button(
                format!("{mark}{}", views::action_label(&action)),
                action,
            ));
        }
        ui = ui.push(row);
    }
    ui = ui.push(Ui::button_row().push(Ui::button(
        if helper.enabled {
            "Уведомления: включены"
        } else {
            "Уведомления: пауза"
        },
        Action::ToggleEnabled,
    )));
    Ui::fragment(ui.nodes)
}

fn helper_save_button(helper: &HelperDraft) -> teloxide_ui::Button<Action> {
    let ready = !helper.enabled || (!helper.areas.is_empty() && !helper.kinds.is_empty());
    Ui::button("Сохранить", Action::SaveHelper).disabled(!ready)
}

fn location_warning(map: &DistrictMap, point: GeoPoint, accuracy: Option<f64>) -> Option<String> {
    let accuracy = accuracy.filter(|a| a.is_finite() && (0.0..=1500.0).contains(a));
    if map.districts_near(point, accuracy.unwrap_or(0.0)).len() > 1 {
        Some(
            "Рядом граница района. Проверьте точку; при необходимости нажмите «Изменить место»."
                .to_owned(),
        )
    } else {
        accuracy
            .filter(|a| *a >= 100.0)
            .map(|a| format!("Точность GPS: ±{a:.0} м. Проверьте точку перед отправкой."))
    }
}

fn select_problem(draft: &mut RequestDraft, problem: &str) -> Result<(), &'static str> {
    if draft.step != Some(RequestStep::Description) {
        return Err("Выбор проблемы доступен только на первом шаге.");
    }
    let (_, description) = PROBLEMS
        .iter()
        .find(|(id, _)| *id == problem)
        .ok_or("Неизвестная проблема.")?;
    draft.kind = Some(if problem == "accident" {
        HelpKind::Accident
    } else {
        HelpKind::Technical
    });
    draft.tags = vec![problem.to_owned()];
    (*description).clone_into(&mut draft.description);
    draft.next();
    if draft.can_publish() {
        draft.next();
    }
    Ok(())
}

fn kind_label(kind: HelpKind) -> &'static str {
    match kind {
        HelpKind::Technical => "Поломка",
        HelpKind::Accident => "Происшествие",
    }
}
fn can_advance(draft: &RequestDraft) -> bool {
    match draft.step {
        Some(RequestStep::Description) => {
            draft.kind.is_some() && !draft.description.trim().is_empty()
        }
        Some(RequestStep::Location) => draft.can_publish(),
        _ => false,
    }
}
fn apply_input(
    draft: &mut RequestDraft,
    text: &str,
    point: Option<GeoPoint>,
    districts: &DistrictMap,
) -> Result<(), &'static str> {
    match draft.step {
        Some(RequestStep::Location) => {
            let point = point.ok_or("Отправьте геопозицию вложением Telegram.")?;
            let district = districts
                .district_at(point)
                .ok_or("Бот пока работает в Петербурге. Пришлите точку в пределах города.")?;
            draft.area_id = Some(district.to_owned());
            draft.exact_point = Some(point);
            draft.next();
        }
        Some(RequestStep::Description) => {
            if text.trim().is_empty() || text.chars().count() > 4000 {
                return Err("Описание должно содержать 1–4000 символов.");
            }
            text.clone_into(&mut draft.description);
            draft.next();
            if draft.can_publish() {
                draft.next();
            }
        }
        _ => return Err("Откройте /start и выберите действие; /active — ваши заявки."),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_categories_never_echo_remote_payloads() {
        let error: anyhow::Error = teloxide::RequestError::Api(teloxide::ApiError::Unknown(
            "Rejected payload: private address, coordinates, token URL".into(),
        ))
        .into();
        assert_eq!(error_reason(&error), "telegram_api_rejected");
        assert_eq!(
            action_operation(&Action::ToggleHelperProblem("private input".into())),
            "toggle_helper_problem"
        );
        assert_eq!(
            error_reason(&anyhow::anyhow!("private raw error")),
            "internal_validation"
        );
    }
    #[test]
    fn problem_selection_is_single_and_advances_immediately() {
        let mut draft = RequestDraft {
            step: Some(RequestStep::Description),
            ..Default::default()
        };
        assert!(select_problem(&mut draft, "unknown").is_err());
        assert_eq!(draft.tags, Vec::<String>::new());
        select_problem(&mut draft, "puncture").unwrap();
        assert_eq!(draft.step, Some(RequestStep::Location));
        assert_eq!(draft.tags, vec!["puncture"]);
        assert_eq!(draft.kind, Some(HelpKind::Technical));
        assert!(select_problem(&mut draft, "brakes").is_err());
        draft.back();
        select_problem(&mut draft, "accident").unwrap();
        assert_eq!(draft.tags, vec!["accident"]);
        assert_eq!(draft.kind, Some(HelpKind::Accident));
        let map = DistrictMap::bundled().unwrap();
        apply_input(&mut draft, "", GeoPoint::new(59.938, 30.35), &map).unwrap();
        draft.step = Some(RequestStep::Description);
        select_problem(&mut draft, "tools").unwrap();
        assert_eq!(draft.step, Some(RequestStep::Preview));
        assert_eq!(draft.tags, vec!["tools"]);
        draft.step = Some(RequestStep::Description);
        apply_input(&mut draft, "Нужен насос и заплатка", None, &map).unwrap();
        assert_eq!(draft.step, Some(RequestStep::Preview));
        assert_eq!(draft.description, "Нужен насос и заплатка");
    }
    #[test]
    fn inputs_are_validated_and_location_advances_without_district_selection() {
        let map = DistrictMap::bundled().unwrap();
        let mut draft = RequestDraft {
            kind: Some(HelpKind::Technical),
            step: Some(RequestStep::Description),
            ..Default::default()
        };
        assert!(apply_input(&mut draft, "   ", None, &map).is_err());
        apply_input(&mut draft, "Нужен насос", None, &map).unwrap();
        assert_eq!(draft.step, Some(RequestStep::Location));
        assert!(apply_input(&mut draft, "59.9,30.3", None, &map).is_err());
        assert!(apply_input(&mut draft, "", GeoPoint::new(55.75, 37.62), &map).is_err());
        assert!(draft.exact_point.is_none());
        apply_input(&mut draft, "", GeoPoint::new(59.938, 30.35), &map).unwrap();
        assert_eq!(draft.step, Some(RequestStep::Preview));
        assert_eq!(draft.area_id.as_deref(), Some("spb-tsentralny"));
        assert!(draft.can_publish());
        assert!(location_warning(&map, draft.exact_point.unwrap(), Some(1500.0)).is_some());
        assert!(location_warning(&map, draft.exact_point.unwrap(), Some(1.0)).is_none());
        assert!(location_warning(&map, draft.exact_point.unwrap(), Some(f64::NAN)).is_none());
    }
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod transport_tests;
