// SPDX-License-Identifier: AGPL-3.0-only
//! Real adapter requests against a local HTTP stub, using real migrated `SQLite`.
use super::Runtime;
use crate::{
    application::RequestRepository,
    config::Config,
    domain::{ArrivalEstimate, GeoPoint, HelpKind, HelpRequestStatus, TelegramUserId},
    ops::RateLimiter,
    storage::{connect, SqliteRepositories},
    ui::Action,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{BufRead, Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use teloxide::{
    types::{CallbackQuery, Message},
    Bot,
};
use teloxide_ui::ActionRegistry;

struct ApiStub {
    url: String,
    bodies: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    reject_emoji: Arc<AtomicBool>,
    reject_edit: Arc<AtomicBool>,
    reject_map: Arc<AtomicBool>,
    reject_location_delete: Arc<AtomicBool>,
    reject_rich: Arc<AtomicBool>,
    reject_keyboard_delete: Arc<AtomicBool>,
}
impl ApiStub {
    #[allow(clippy::too_many_lines)]
    fn new() -> Self {
        let socket = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", socket.local_addr().unwrap());
        socket.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let stop_worker = stop.clone();
        let bodies_worker = bodies.clone();
        let reject_emoji = Arc::new(AtomicBool::new(false));
        let reject_worker = reject_emoji.clone();
        let reject_edit = Arc::new(AtomicBool::new(false));
        let edit_worker = reject_edit.clone();
        let reject_map = Arc::new(AtomicBool::new(false));
        let map_worker = reject_map.clone();
        let reject_location_delete = Arc::new(AtomicBool::new(false));
        let location_delete_worker = reject_location_delete.clone();
        let reject_rich = Arc::new(AtomicBool::new(false));
        let rich_worker = reject_rich.clone();
        let reject_keyboard_delete = Arc::new(AtomicBool::new(false));
        let keyboard_delete_worker = reject_keyboard_delete.clone();
        let thread = std::thread::spawn(move || {
            let mut next_id = 1;
            let mut messages = HashMap::new();
            while !stop_worker.load(Ordering::Relaxed) {
                let Ok((mut socket, _)) = socket.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = std::io::BufReader::new(socket.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let first = line.clone();
                let mut length = 0;
                let mut content_type = String::new();
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(raw) = line.to_lowercase().strip_prefix("content-length:") {
                        length = raw.trim().parse().unwrap();
                    }
                    if line.to_lowercase().starts_with("content-type:") {
                        content_type = line.split_once(':').unwrap().1.trim().to_owned();
                    }
                }
                let mut data = vec![0; length];
                reader.read_exact(&mut data).unwrap();
                let body: Value = if let Some((_, boundary)) = content_type.split_once("boundary=")
                {
                    let mut object = serde_json::Map::new();
                    for part in std::str::from_utf8(&data)
                        .unwrap()
                        .split(&format!("--{boundary}"))
                    {
                        if let Some((headers, value)) = part.split_once("\r\n\r\n") {
                            let name = headers
                                .split("name=\"")
                                .nth(1)
                                .unwrap()
                                .split('"')
                                .next()
                                .unwrap();
                            let value = value.trim_end_matches("\r\n");
                            object.insert(
                                name.to_owned(),
                                serde_json::from_str(value).unwrap_or_else(|_| json!(value)),
                            );
                        }
                    }
                    Value::Object(object)
                } else {
                    serde_json::from_slice(&data).unwrap()
                };
                let method = first
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .to_ascii_lowercase();
                let key = (
                    body["chat_id"].as_i64().unwrap_or(0),
                    body["message_id"].as_i64().unwrap_or(0),
                );
                let keyboard_edit =
                    method == "editmessagetext" && messages.get(&key) == Some(&true);
                let result = stub_result(&first, &body, &mut next_id);
                let denied = body.to_string().contains("custom_emoji_id")
                    && reject_worker.swap(false, Ordering::Relaxed);
                let edit_denied =
                    method == "editmessagetext" && edit_worker.swap(false, Ordering::Relaxed);
                let map_denied = body["rich_message"]["blocks"]
                    .as_array()
                    .is_some_and(|blocks| {
                        blocks.iter().any(|b| {
                            b["type"] == "map" && b["location"]["horizontal_accuracy"].is_null()
                        })
                    })
                    && map_worker.swap(false, Ordering::Relaxed);
                let location_delete_denied = method == "deletemessage"
                    && body["message_id"] == 500
                    && location_delete_worker.swap(false, Ordering::Relaxed);
                let rich_denied = body.get("rich_message").is_some()
                    && rich_worker.swap(false, Ordering::Relaxed);
                let keyboard_delete_denied = method == "deletemessage"
                    && body["message_id"] != 500
                    && keyboard_delete_worker.swap(false, Ordering::Relaxed);
                let response = if rich_denied {
                    json!({"ok":false,"error_code":500,"description":"Temporarily unavailable"})
                } else if keyboard_edit {
                    json!({"ok":false,"error_code":400,"description":"Bad Request: message can't be edited"})
                } else if map_denied {
                    json!({"ok":false,"error_code":500,"description":"Location delivery temporarily unavailable"})
                } else if edit_denied {
                    json!({"ok":false,"error_code":400,"description":"Bad Request: message to edit not found"})
                } else if denied {
                    json!({"ok":false,"error_code":400,"description":"Bad Request: CUSTOM_EMOJI_NOT_ALLOWED"})
                } else if location_delete_denied || keyboard_delete_denied {
                    json!({"ok":false,"error_code":400,"description":"Bad Request: message can't be deleted"})
                } else {
                    json!({"ok":true,"result":result})
                };
                if response["ok"] == true {
                    if let Some(id) = result["message_id"].as_i64() {
                        messages.entry((key.0, id)).or_insert(
                            body.get("reply_markup").is_some()
                                && body["reply_markup"].get("inline_keyboard").is_none(),
                        );
                    } else if method == "deletemessage" {
                        messages.remove(&key);
                    }
                }
                let mut recorded = body.clone();
                recorded["api_method"] = json!(method);
                recorded["api_success"] = response["ok"].clone();
                recorded["api_message_id"] = result["message_id"].clone();
                bodies_worker.lock().unwrap().push(recorded);
                let response = response.to_string();
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).unwrap();
            }
        });
        Self {
            url,
            bodies,
            stop,
            thread: Some(thread),
            reject_emoji,
            reject_edit,
            reject_map,
            reject_location_delete,
            reject_rich,
            reject_keyboard_delete,
        }
    }
}
fn stub_result(method: &str, body: &Value, next_id: &mut i64) -> Value {
    if method.to_ascii_lowercase().contains("getchat") {
        return json!({"id":body["chat_id"],"type":"private","first_name":"Имя после рестарта","accent_color_id":1,"max_reaction_count":11,"accepted_gift_types":{"unlimited_gifts":false,"limited_gifts":false,"unique_gifts":false,"premium_subscription":false}});
    }
    if method.to_ascii_lowercase().contains("deletemessage")
        || method.contains("AnswerCallbackQuery")
        || method.contains("answerCallbackQuery")
    {
        return json!(true);
    }
    let id = body["message_id"].as_i64().unwrap_or(*next_id);
    *next_id += 1;
    let mut result = json!({"message_id":id,"date":1,"chat":{"id":body["chat_id"].as_i64().unwrap(),"type":"private","first_name":"test"}});
    if body.get("latitude").is_some() {
        result["location"] = json!({"latitude":body["latitude"],"longitude":body["longitude"]});
    } else {
        result["text"] = json!("rendered");
    }
    result
}

impl Drop for ApiStub {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.thread.take().unwrap().join();
    }
}
fn message(user: i64, text: &str, point: Option<GeoPoint>) -> Message {
    let mut value = json!({"message_id":500,"date":1,"chat":{"id":user,"type":"private","first_name":"test"},"from":{"id":user,"is_bot":false,"first_name":"test"}});
    if let Some(point) = point {
        value["location"] = json!({"latitude":point.lat,"longitude":point.lon});
    } else {
        value["text"] = json!(text);
    }
    serde_json::from_value(value).unwrap()
}
fn callback(runtime: &Runtime, user: i64, action: &Action) -> CallbackQuery {
    let (message, token) = runtime
        .surfaces
        .iter()
        .filter(|(_, s)| s.owner == TelegramUserId(user))
        .find_map(|((_, message), s)| {
            s.tokens.iter().find_map(|token| {
                runtime
                    .registry
                    .resolve(
                        token,
                        teloxide::types::UserId(u64::try_from(user).unwrap()),
                        s.view,
                        s.revision,
                    )
                    .ok()
                    .filter(|r| r.action == *action)
                    .map(|_| (*message, token.clone()))
            })
        })
        .expect("action exists");
    serde_json::from_value(json!({"id":"test-query","from":{"id":user,"is_bot":false,"first_name":"test"},"message":{"message_id":message.0,"date":1,"chat":{"id":user,"type":"private","first_name":"test"},"text":"ui"},"chat_instance":"test","data":token.as_str()})).unwrap()
}
async fn press(runtime: &mut Runtime, user: i64, action: Action) {
    let query = callback(runtime, user, &action);
    Box::pin(runtime.callback(query)).await.unwrap();
}

async fn accept_after(
    runtime: &mut Runtime,
    user: i64,
    id: crate::domain::HelpRequestId,
    version: u64,
    min: u16,
    max: u16,
) {
    press(runtime, user, Action::Accept(id, version)).await;
    press(
        runtime,
        user,
        Action::AcceptAt(id, version, ArrivalEstimate::new(min, max).unwrap()),
    )
    .await;
}

fn menu_actions(runtime: &Runtime, user: i64) -> Vec<Action> {
    let message = runtime.sessions.get(&user).unwrap().0.menu.unwrap();
    let surface = runtime.surfaces.get(&(user, message)).unwrap();
    surface
        .tokens
        .iter()
        .filter_map(|token| {
            runtime
                .registry
                .resolve(
                    token,
                    teloxide::types::UserId(u64::try_from(user).unwrap()),
                    surface.view,
                    surface.revision,
                )
                .ok()
                .map(|r| r.action)
        })
        .collect()
}

async fn test_runtime(api: &ApiStub) -> Runtime {
    let pool = connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let config = Config {
        database_url: "sqlite::memory:".into(),
        teloxide_token: "123:test".into(),
        source_url: "https://example.org/source".into(),
        consent_version: "v1".into(),
        default_ttl: Duration::from_secs(3600),
    };
    let areas = SqliteRepositories::new(pool.clone()).areas().await.unwrap();
    Runtime {
        bot: Bot::with_client("123:test", reqwest::Client::new())
            .set_api_url(api.url.parse().unwrap()),
        repo: SqliteRepositories::new(pool),
        config,
        registry: ActionRegistry::new(),
        sessions: std::collections::HashMap::new(),
        surfaces: std::collections::HashMap::new(),
        limiter: RateLimiter::new(),
        icons: crate::ui::icons::IconCatalog::bundled().unwrap(),
        metrics: Arc::new(crate::ops::Metrics::new()),
        areas,
        districts: crate::domain::DistrictMap::bundled().unwrap(),
        addresses: crate::domain::AddressMap::bundled().unwrap(),
        ignored_users: std::collections::HashSet::from([5]),
        icons_enabled: true,
    }
}

#[tokio::test]
#[allow(clippy::too_many_lines, clippy::large_futures)]
async fn telegram_flow_consent_helper_publish_accept_resolve_and_stale_replay() {
    let api = ApiStub::new();
    let mut runtime = test_runtime(&api).await;
    Box::pin(runtime.message(message(5, "/start", None)))
        .await
        .unwrap();
    Box::pin(runtime.message(message(5, "/source", None)))
        .await
        .unwrap();
    assert!(!runtime.sessions.contains_key(&5));
    assert!(!runtime
        .repo
        .has_consent(TelegramUserId(5), "v1")
        .await
        .unwrap());
    assert!(api
        .bodies
        .lock()
        .unwrap()
        .iter()
        .all(|body| body["chat_id"] != 5));
    Box::pin(runtime.message(message(4, "/source", None)))
        .await
        .unwrap();
    assert!(!runtime
        .repo
        .has_consent(TelegramUserId(4), "v1")
        .await
        .unwrap());
    assert!(api
        .bodies
        .lock()
        .unwrap()
        .iter()
        .any(|body| body["chat_id"] == 4 && body["text"] == "https://example.org/source"));
    api.reject_emoji.store(true, Ordering::Relaxed);
    Box::pin(runtime.message(message(2, "/start", None)))
        .await
        .unwrap();
    assert!(!runtime.icons_enabled);
    assert!(!runtime
        .repo
        .has_consent(TelegramUserId(2), "v1")
        .await
        .unwrap());
    press(&mut runtime, 2, Action::Consent("v1".into())).await;
    press(&mut runtime, 2, Action::OpenHelperSettings).await;
    let visible_areas: std::collections::HashSet<_> = menu_actions(&runtime, 2)
        .into_iter()
        .filter_map(|a| match a {
            Action::ToggleArea(area) => Some(area),
            _ => None,
        })
        .collect();
    assert_eq!(visible_areas.len(), 18);
    assert!(runtime
        .areas
        .iter()
        .all(|area| visible_areas.contains(&area.id)));
    press(
        &mut runtime,
        2,
        Action::ToggleArea("spb-admiralteysky".into()),
    )
    .await;
    let replay = callback(&runtime, 2, &Action::ToggleArea("spb-tsentralny".into()));
    Box::pin(runtime.callback(replay.clone())).await.unwrap();
    Box::pin(runtime.callback(replay)).await.unwrap();
    assert_eq!(
        runtime
            .session(TelegramUserId(2))
            .helper
            .as_ref()
            .unwrap()
            .areas,
        vec!["spb-admiralteysky", "spb-tsentralny"]
    );
    let old_menu = runtime.session(TelegramUserId(2)).menu.unwrap();
    let old_area = callback(&runtime, 2, &Action::ToggleArea("spb-tsentralny".into()));
    Box::pin(runtime.message(message(2, "/start", None)))
        .await
        .unwrap();
    assert_ne!(runtime.session(TelegramUserId(2)).menu, Some(old_menu));
    assert!(!runtime.surfaces.contains_key(&(2, old_menu)));
    assert!(menu_actions(&runtime, 2).contains(&Action::OpenHelperSettings));
    assert!(menu_actions(&runtime, 2)
        .iter()
        .all(|a| !matches!(a, Action::ToggleArea(_) | Action::HelperOptions)));
    Box::pin(runtime.callback(old_area)).await.unwrap();
    press(&mut runtime, 2, Action::OpenHelperSettings).await;
    assert_eq!(
        runtime
            .session(TelegramUserId(2))
            .helper
            .as_ref()
            .unwrap()
            .areas
            .len(),
        2
    );
    let replaced_menu = runtime.session(TelegramUserId(2)).menu.unwrap();
    let stale_area = callback(&runtime, 2, &Action::ToggleArea("spb-tsentralny".into()));
    api.reject_edit.store(true, Ordering::Relaxed);
    press(&mut runtime, 2, Action::ToggleEnabled).await;
    press(&mut runtime, 2, Action::ToggleEnabled).await;
    assert!(!runtime.surfaces.contains_key(&(2, replaced_menu)));
    Box::pin(runtime.callback(stale_area)).await.unwrap();
    assert_eq!(
        runtime
            .session(TelegramUserId(2))
            .helper
            .as_ref()
            .unwrap()
            .areas
            .len(),
        2
    );
    press(
        &mut runtime,
        2,
        Action::ToggleCapability("technical".into()),
    )
    .await;
    assert!(!menu_actions(&runtime, 2).contains(&Action::SaveHelper));
    Box::pin(runtime.action(TelegramUserId(2), Action::SaveHelper))
        .await
        .unwrap();
    assert!(api.bodies.lock().unwrap().iter().any(|b| b["chat_id"] == 2
        && b["text"] == "Выберите тип помощи: техническая помощь или происшествие."));
    assert_eq!(
        runtime
            .session(TelegramUserId(2))
            .helper
            .as_ref()
            .unwrap()
            .areas
            .len(),
        2
    );
    press(
        &mut runtime,
        2,
        Action::ToggleCapability("technical".into()),
    )
    .await;
    assert_eq!(
        menu_actions(&runtime, 2)
            .iter()
            .filter(|a| matches!(a, Action::ToggleArea(_)))
            .count(),
        18
    );
    press(&mut runtime, 2, Action::SaveHelper).await;
    assert_eq!(
        runtime
            .repo
            .helper(TelegramUserId(2))
            .await
            .unwrap()
            .unwrap()
            .areas
            .len(),
        2
    );
    Box::pin(runtime.message(message(1, "/start", None)))
        .await
        .unwrap();
    press(&mut runtime, 1, Action::Consent("v1".into())).await;
    press(&mut runtime, 1, Action::OpenCreateRequest).await;
    let problem_replay = callback(&runtime, 1, &Action::SelectProblem("puncture".into()));
    Box::pin(runtime.callback(problem_replay.clone()))
        .await
        .unwrap();
    Box::pin(runtime.callback(problem_replay)).await.unwrap();
    assert_eq!(
        runtime.session(TelegramUserId(1)).request.tags,
        vec!["puncture"]
    );
    assert_eq!(
        runtime.session(TelegramUserId(1)).request.step,
        Some(crate::ui::RequestStep::Location)
    );
    press(&mut runtime, 1, Action::WizardBack).await;
    Box::pin(runtime.message(message(1, "Нужен насос", None)))
        .await
        .unwrap();
    let mut location = serde_json::to_value(message(1, "", GeoPoint::new(59.938, 30.35))).unwrap();
    location["location"]["horizontal_accuracy"] = json!(1500.0);
    Box::pin(runtime.message(serde_json::from_value(location).unwrap()))
        .await
        .unwrap();
    assert!(runtime
        .session(TelegramUserId(1))
        .location_warning
        .is_some());
    press(&mut runtime, 1, Action::EditDescription).await;
    press(&mut runtime, 1, Action::SelectProblem("tools".into())).await;
    assert_eq!(
        runtime.session(TelegramUserId(1)).request.step,
        Some(crate::ui::RequestStep::Preview)
    );
    assert_eq!(
        runtime.session(TelegramUserId(1)).request.tags,
        vec!["tools"]
    );
    press(&mut runtime, 1, Action::EditDescription).await;
    Box::pin(runtime.message(message(1, "Нужен насос", None)))
        .await
        .unwrap();
    let publish_replay = callback(&runtime, 1, &Action::PublishRequest);
    Box::pin(runtime.callback(publish_replay.clone()))
        .await
        .unwrap();
    Box::pin(runtime.callback(publish_replay)).await.unwrap();
    let requests = runtime
        .repo
        .active_requests(TelegramUserId(1))
        .await
        .unwrap();
    assert_eq!(requests.len(), 1);
    assert!(runtime
        .session(TelegramUserId(1))
        .location_warning
        .is_none());
    assert_eq!(requests[0].area_id.as_deref(), Some("spb-tsentralny"));
    assert_eq!(requests[0].help_kind, HelpKind::Technical);
    let id = requests[0].id;
    assert_eq!(
        runtime
            .repo
            .active_requests(TelegramUserId(2))
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        runtime
            .repo
            .active_requests(TelegramUserId(3))
            .await
            .unwrap()
            .len(),
        0
    );
    {
        let sent = api.bodies.lock().unwrap();
        let notifications: Vec<_> = sent
            .iter()
            .filter(|b| {
                b["chat_id"] == 2
                    && b.get("rich_message").is_some()
                    && b.to_string().contains("Нужен насос")
            })
            .collect();
        assert_eq!(notifications.len(), 1);
        assert!(!notifications[0].to_string().contains("59.938"));
        let map = notifications[0]["rich_message"]["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["type"] == "map")
            .unwrap();
        assert_ne!(map["location"]["latitude"], 59.938);
        assert_ne!(map["location"]["longitude"], 30.35);
        assert_eq!(map["location"]["horizontal_accuracy"], 800.0);
    }
    assert!(api
        .bodies
        .lock()
        .unwrap()
        .iter()
        .all(|b| b.get("latitude").is_none()));
    let old_helper_card = runtime
        .surfaces
        .iter()
        .find(|(_, s)| s.owner == TelegramUserId(2) && s.request == Some(id))
        .unwrap()
        .0
         .1;
    let begin = callback(&runtime, 2, &Action::Accept(id, 0));
    let mut foreign = begin.clone();
    foreign.from.id = teloxide::types::UserId(3);
    Box::pin(runtime.callback(foreign)).await.unwrap();
    Box::pin(runtime.callback(begin.clone())).await.unwrap();
    Box::pin(runtime.callback(begin)).await.unwrap();
    assert_eq!(
        runtime.repo.request(id).await.unwrap().status,
        HelpRequestStatus::Open
    );
    let estimate = ArrivalEstimate::new(15, 30).unwrap();
    let accept_replay = callback(&runtime, 2, &Action::AcceptAt(id, 0, estimate));
    Box::pin(runtime.callback(accept_replay.clone()))
        .await
        .unwrap();
    Box::pin(runtime.callback(accept_replay)).await.unwrap();
    assert_eq!(
        runtime.repo.request(id).await.unwrap().matched_helper_id,
        Some(TelegramUserId(2))
    );
    assert!(runtime
        .repo
        .request(id)
        .await
        .unwrap()
        .arrival_window
        .is_some());
    {
        let sent = api.bodies.lock().unwrap();
        let card = sent
            .iter()
            .rev()
            .find(|b| b["chat_id"] == 2 && b["message_id"] == old_helper_card.0)
            .unwrap();
        let blocks = card["rich_message"]["blocks"].as_array().unwrap();
        let map = blocks.iter().find(|b| b["type"] == "map").unwrap();
        assert_eq!(map["location"]["latitude"], 59.938);
        assert_eq!(map["location"]["longitude"], 30.35);
        assert!(card.to_string().contains("tg://user?id=1"));
        assert!(card.to_string().contains("Прибытие:"));
        assert!(!card.to_string().contains("Точная точка:"));
        assert!(
            sent.iter().all(|b| b.get("latitude").is_none()),
            "the map is embedded, no extra location message"
        );
        assert!(sent
            .iter()
            .filter(|b| b["chat_id"] == 3)
            .all(|b| !b.to_string().contains("59.938")));
    }
    assert!(runtime
        .repo
        .match_peer(id, TelegramUserId(3))
        .await
        .is_err());
    press(&mut runtime, 1, Action::Resolve(id)).await;
    {
        let sent = api.bodies.lock().unwrap();
        let closed_card = sent
            .iter()
            .rev()
            .find(|b| b["chat_id"] == 2 && b["message_id"] == old_helper_card.0)
            .unwrap();
        assert!(!closed_card.to_string().contains("tg://user?id=1"));
        assert!(!closed_card.to_string().contains("59.938"));
        assert!(!closed_card.to_string().contains("Прибытие:"));
    }
    assert!(runtime
        .repo
        .match_peer(id, TelegramUserId(2))
        .await
        .is_err());
    let request = runtime.repo.request(id).await.unwrap();
    assert_eq!(request.status, HelpRequestStatus::Resolved);
    assert_eq!(request.exact_point, None);
    press(&mut runtime, 1, Action::Home).await;
    press(&mut runtime, 1, Action::More).await;
    press(&mut runtime, 1, Action::Privacy).await;
    press(&mut runtime, 1, Action::ConfirmDelete).await;
    press(&mut runtime, 1, Action::DeleteData).await;
    assert!(!runtime.sessions.contains_key(&1));
    assert!(runtime
        .surfaces
        .values()
        .all(|surface| surface.owner != TelegramUserId(1)));
    assert!(!runtime
        .repo
        .has_consent(TelegramUserId(1), "v1")
        .await
        .unwrap());
    assert!(runtime.repo.request(id).await.is_err());
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn smoke_all_districts_settings_pause_and_profile_recovery() {
    let api = ApiStub::new();
    let mut runtime = test_runtime(&api).await;
    let user = TelegramUserId(2);
    Box::pin(runtime.message(message(2, "/start", None)))
        .await
        .unwrap();
    press(&mut runtime, 2, Action::Consent("v1".into())).await;
    press(&mut runtime, 2, Action::OpenHelperSettings).await;
    assert!(!menu_actions(&runtime, 2).contains(&Action::SaveHelper));
    let areas: Vec<_> = runtime.areas.iter().map(|a| a.id.clone()).collect();
    assert_eq!(areas.len(), 18);
    for (index, area) in areas.iter().enumerate() {
        press(&mut runtime, 2, Action::ToggleArea(area.clone())).await;
        assert_eq!(
            runtime.session(user).helper.as_ref().unwrap().areas.len(),
            index + 1
        );
    }
    press(&mut runtime, 2, Action::ToggleCapability("accident".into())).await;
    assert!(menu_actions(&runtime, 2)
        .iter()
        .any(|a| matches!(a, Action::ToggleCapability(_))));
    assert!(menu_actions(&runtime, 2).contains(&Action::ToggleEnabled));
    press(&mut runtime, 2, Action::SaveHelper).await;
    let profile = runtime.repo.helper(user).await.unwrap().unwrap();
    assert_eq!(profile.areas.len(), 18);
    assert_eq!(profile.help_kinds.len(), 2);
    assert!(profile.help_kinds.contains(&HelpKind::Technical));
    assert!(profile.help_kinds.contains(&HelpKind::Accident));
    assert!(runtime.session(user).helper.is_none());
    let card = {
        let bodies = api.bodies.lock().unwrap();
        bodies
            .iter()
            .rev()
            .find(|b| b["chat_id"] == 2 && b.get("rich_message").is_some())
            .unwrap()
            .to_string()
    };
    assert!(card.contains("Настройки помощника сохранены"));
    assert!(card.contains("Вы зарегистрированы как помощник"));

    // Fresh adapter/session registry, same authoritative database as after a restart.
    let repo = runtime.repo.clone();
    runtime = test_runtime(&api).await;
    runtime.repo = repo;
    Box::pin(runtime.message(message(2, "/start", None)))
        .await
        .unwrap();
    press(&mut runtime, 2, Action::OpenHelperSettings).await;
    assert_eq!(
        runtime.session(user).helper.as_ref().unwrap().areas.len(),
        18
    );
    let stale_pause = callback(&runtime, 2, &Action::ToggleEnabled);
    Box::pin(runtime.callback(stale_pause.clone()))
        .await
        .unwrap();
    Box::pin(runtime.callback(stale_pause)).await.unwrap();
    assert!(!runtime.session(user).helper.as_ref().unwrap().enabled);
    press(&mut runtime, 2, Action::SaveHelper).await;
    assert!(!runtime.repo.helper(user).await.unwrap().unwrap().enabled);
    press(&mut runtime, 2, Action::OpenHelperSettings).await;
    for area in &areas {
        press(&mut runtime, 2, Action::ToggleArea(area.clone())).await;
    }
    assert!(menu_actions(&runtime, 2).contains(&Action::SaveHelper));
    press(&mut runtime, 2, Action::SaveHelper).await;
    assert_eq!(
        runtime
            .repo
            .helper(user)
            .await
            .unwrap()
            .unwrap()
            .areas
            .len(),
        0
    );
    press(&mut runtime, 2, Action::OpenHelperSettings).await;
    press(&mut runtime, 2, Action::ToggleEnabled).await;
    assert!(!menu_actions(&runtime, 2).contains(&Action::SaveHelper));
    press(&mut runtime, 2, Action::ToggleArea(areas[17].clone())).await;
    press(&mut runtime, 2, Action::SaveHelper).await;
    let restored = runtime.repo.helper(user).await.unwrap().unwrap();
    assert!(restored.enabled);
    assert_eq!(restored.areas, vec![areas[17].clone()]);
}

async fn publish_smoke_request(runtime: &mut Runtime) -> crate::domain::HelpRequestId {
    Box::pin(runtime.home(TelegramUserId(1))).await.unwrap();
    press(runtime, 1, Action::OpenCreateRequest).await;
    assert!(!menu_actions(runtime, 1)
        .iter()
        .any(|a| matches!(a, Action::ToggleArea(_))));
    press(runtime, 1, Action::SelectProblem("puncture".into())).await;
    Box::pin(runtime.message(message(1, "", GeoPoint::new(59.938, 30.35))))
        .await
        .unwrap();
    press(runtime, 1, Action::PublishRequest).await;
    let menu = runtime.session(TelegramUserId(1)).menu.unwrap();
    runtime.surfaces[&(1, menu)].request.unwrap()
}

#[tokio::test]
async fn smoke_decline_withdraw_new_wave_cancel_and_expire() {
    let api = ApiStub::new();
    let mut runtime = test_runtime(&api).await;
    for user in [1, 2, 3] {
        Box::pin(runtime.message(message(user, "/start", None)))
            .await
            .unwrap();
        press(&mut runtime, user, Action::Consent("v1".into())).await;
        if user != 1 {
            press(&mut runtime, user, Action::OpenHelperSettings).await;
            press(
                &mut runtime,
                user,
                Action::ToggleArea("spb-tsentralny".into()),
            )
            .await;
            press(&mut runtime, user, Action::SaveHelper).await;
        }
    }
    let id = publish_smoke_request(&mut runtime).await;
    let old_accept = callback(&runtime, 2, &Action::Accept(id, 0));
    press(&mut runtime, 2, Action::Decline(id, 0)).await;
    Box::pin(runtime.callback(old_accept.clone()))
        .await
        .unwrap();
    assert_eq!(
        runtime.repo.request(id).await.unwrap().status,
        HelpRequestStatus::Open
    );
    api.reject_map.store(true, Ordering::Relaxed);
    accept_after(&mut runtime, 3, id, 0, 5, 15).await;
    assert_eq!(
        runtime.repo.request(id).await.unwrap().status,
        HelpRequestStatus::Matched
    );
    Box::pin(runtime.message(message(3, "/active", None)))
        .await
        .unwrap();
    assert!(api
        .bodies
        .lock()
        .unwrap()
        .iter()
        .any(|b| b["chat_id"] == 3 && b.to_string().contains("tg://user?id=1")));
    press(&mut runtime, 3, Action::Withdraw(id)).await;
    let reopened = runtime.repo.request(id).await.unwrap();
    assert_eq!(reopened.status, HelpRequestStatus::Open);
    assert_eq!(reopened.version, 2);
    Box::pin(runtime.callback(old_accept)).await.unwrap();
    assert_eq!(
        runtime.repo.request(id).await.unwrap().status,
        HelpRequestStatus::Open
    );
    accept_after(&mut runtime, 2, id, reopened.version, 15, 30).await;
    assert_eq!(
        runtime.repo.request(id).await.unwrap().matched_helper_id,
        Some(TelegramUserId(2))
    );
    press(&mut runtime, 1, Action::Cancel(id)).await;
    let closed = runtime.repo.request(id).await.unwrap();
    assert_eq!(closed.status, HelpRequestStatus::Cancelled);
    assert_eq!(closed.exact_point, None);
    assert!(runtime
        .repo
        .match_peer(id, TelegramUserId(2))
        .await
        .is_err());

    let expired_id = publish_smoke_request(&mut runtime).await;
    let late_accept = callback(&runtime, 2, &Action::Accept(expired_id, 0));
    // Exercise the same expiry repository method as the worker, with an advanced clock.
    assert_eq!(
        runtime
            .repo
            .expire_due(chrono::Utc::now().timestamp() + 7200)
            .await
            .unwrap(),
        1
    );
    runtime.deliver().await.unwrap();
    Box::pin(runtime.callback(late_accept)).await.unwrap();
    let expired = runtime.repo.request(expired_id).await.unwrap();
    assert_eq!(expired.status, HelpRequestStatus::Expired);
    assert_eq!(expired.exact_point, None);
    assert_eq!(
        runtime
            .repo
            .active_requests(TelegramUserId(1))
            .await
            .unwrap()
            .len(),
        0
    );
    assert!(runtime.repo.pending_deliveries().await.unwrap().is_empty());
}

fn last_reply_markup(api: &ApiStub, user: i64) -> Value {
    api.bodies
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|b| b["chat_id"] == user && b.get("reply_markup").is_some())
        .unwrap()["reply_markup"]
        .clone()
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn smoke_location_keyboard_lifecycle_and_emoji_fallback() {
    let api = ApiStub::new();
    let mut runtime = test_runtime(&api).await;
    Box::pin(runtime.message(message(1, "/start", None)))
        .await
        .unwrap();
    {
        let bodies = api.bodies.lock().unwrap();
        assert!(!bodies.iter().any(|b| b["chat_id"] == 1
            && b["api_method"] == "sendmessage"
            && b["api_success"] == true));
        assert!(!bodies
            .iter()
            .any(|b| b["chat_id"] == 1 && b.to_string().contains("Готово")));
    }
    assert_eq!(
        runtime.session(TelegramUserId(1)).location_keyboard,
        Some(false)
    );
    press(&mut runtime, 1, Action::Consent("v1".into())).await;
    press(&mut runtime, 1, Action::OpenCreateRequest).await;
    let old_description = callback(&runtime, 1, &Action::SelectProblem("tools".into()));
    press(&mut runtime, 1, Action::SelectProblem("puncture".into())).await;
    let markup = last_reply_markup(&api, 1);
    assert_eq!(markup["keyboard"][0][0]["request_location"], true);
    assert_eq!(markup["keyboard"][0][0]["text"], "Отправить местоположение");
    assert!(markup["keyboard"][0][0]["icon_custom_emoji_id"].is_string());
    assert_eq!(markup["resize_keyboard"], true);
    assert_eq!(markup["one_time_keyboard"], true);
    Box::pin(runtime.callback(old_description)).await.unwrap();
    assert_eq!(
        runtime.session(TelegramUserId(1)).request.tags,
        vec!["puncture"]
    );
    Box::pin(runtime.message(message(1, "", GeoPoint::new(55.75, 37.61))))
        .await
        .unwrap();
    assert_eq!(
        runtime.session(TelegramUserId(1)).request.step,
        Some(crate::ui::RequestStep::Location)
    );
    assert_eq!(
        runtime.session(TelegramUserId(1)).location_keyboard,
        Some(true)
    );
    assert!(!api
        .bodies
        .lock()
        .unwrap()
        .iter()
        .any(|body| { body["api_method"] == "deletemessage" && body["message_id"] == 500 }));
    api.reject_location_delete.store(true, Ordering::Relaxed);
    Box::pin(runtime.message(message(1, "", GeoPoint::new(59.938, 30.35))))
        .await
        .unwrap();
    assert_eq!(
        runtime.session(TelegramUserId(1)).request.step,
        Some(crate::ui::RequestStep::Preview)
    );
    assert!(api.bodies.lock().unwrap().iter().any(|body| {
        body["api_method"] == "deletemessage"
            && body["chat_id"] == 1
            && body["message_id"] == 500
            && body["api_success"] == false
    }));
    assert_eq!(last_reply_markup(&api, 1)["remove_keyboard"], true);
    assert_eq!(
        runtime.session(TelegramUserId(1)).location_keyboard,
        Some(false)
    );
    api.reject_emoji.store(true, Ordering::Relaxed);
    press(&mut runtime, 1, Action::WizardBack).await;
    let plain = last_reply_markup(&api, 1);
    assert_eq!(plain["keyboard"][0][0]["request_location"], true);
    assert!(plain["keyboard"][0][0]
        .get("icon_custom_emoji_id")
        .is_none());
    press(&mut runtime, 1, Action::Home).await;
    assert_eq!(last_reply_markup(&api, 1)["remove_keyboard"], true);
    press(&mut runtime, 1, Action::OpenHelperSettings).await;
    assert_eq!(
        runtime.session(TelegramUserId(1)).location_keyboard,
        Some(false)
    );
    press(&mut runtime, 1, Action::Home).await;
    press(&mut runtime, 1, Action::OpenCreateRequest).await;
    assert_eq!(
        last_reply_markup(&api, 1)["keyboard"][0][0]["request_location"],
        true
    );
    Box::pin(runtime.message(message(1, "/start", None)))
        .await
        .unwrap();
    assert_eq!(last_reply_markup(&api, 1)["remove_keyboard"], true);
    press(&mut runtime, 1, Action::OpenCreateRequest).await;
    Box::pin(runtime.message(message(1, "/active", None)))
        .await
        .unwrap();
    assert_eq!(last_reply_markup(&api, 1)["remove_keyboard"], true);
    assert!(!runtime.session(TelegramUserId(1)).editing_request);
    press(&mut runtime, 1, Action::OpenCreateRequest).await;
    Box::pin(runtime.message(message(1, "/cancel", None)))
        .await
        .unwrap();
    assert_eq!(last_reply_markup(&api, 1)["remove_keyboard"], true);
    assert_eq!(runtime.session(TelegramUserId(1)).request.step, None);
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn smoke_custom_arrival_losing_picker_and_matched_card_recovery() {
    let api = ApiStub::new();
    let mut runtime = test_runtime(&api).await;
    for user in [1, 2, 3] {
        Box::pin(runtime.message(message(user, "/start", None)))
            .await
            .unwrap();
        press(&mut runtime, user, Action::Consent("v1".into())).await;
        if user != 1 {
            press(&mut runtime, user, Action::OpenHelperSettings).await;
            press(
                &mut runtime,
                user,
                Action::ToggleArea("spb-tsentralny".into()),
            )
            .await;
            press(&mut runtime, user, Action::SaveHelper).await;
        }
    }
    let id = publish_smoke_request(&mut runtime).await;
    press(&mut runtime, 2, Action::Accept(id, 0)).await;
    press(&mut runtime, 2, Action::EnterArrival(id, 0)).await;
    for invalid in ["0", "241", "текст", "10-20"] {
        Box::pin(runtime.message(message(2, invalid, None)))
            .await
            .unwrap();
        assert_eq!(
            runtime.repo.request(id).await.unwrap().status,
            HelpRequestStatus::Open
        );
        assert!(runtime.session(TelegramUserId(2)).arrival_text_input);
    }
    accept_after(&mut runtime, 3, id, 0, 5, 15).await;
    let won = runtime.repo.request(id).await.unwrap();
    assert_eq!(won.matched_helper_id, Some(TelegramUserId(3)));
    assert!(!runtime.session(TelegramUserId(2)).arrival_text_input);
    let window = won.arrival_window.unwrap();
    assert_eq!((window.latest - window.earliest).num_minutes(), 10);
    Box::pin(runtime.message(message(2, "20", None)))
        .await
        .unwrap();
    assert_eq!(
        runtime.repo.request(id).await.unwrap().arrival_window,
        Some(window)
    );
    press(&mut runtime, 1, Action::Resolve(id)).await;
    assert_eq!(runtime.repo.request(id).await.unwrap().arrival_window, None);

    let next = publish_smoke_request(&mut runtime).await;
    press(&mut runtime, 2, Action::Accept(next, 0)).await;
    press(&mut runtime, 2, Action::EnterArrival(next, 0)).await;
    Box::pin(runtime.message(message(2, "20", None)))
        .await
        .unwrap();
    let exact = runtime.repo.request(next).await.unwrap();
    assert_eq!(exact.matched_helper_id, Some(TelegramUserId(2)));
    let window = exact.arrival_window.unwrap();
    assert_eq!(window.earliest, window.latest);
    let repo = runtime.repo.clone();
    runtime = test_runtime(&api).await;
    runtime.repo = repo;
    Box::pin(runtime.message(message(2, "/active", None)))
        .await
        .unwrap();
    {
        let sent = api.bodies.lock().unwrap();
        let card = sent
            .iter()
            .rev()
            .find(|b| b["chat_id"] == 2 && b.get("rich_message").is_some())
            .unwrap();
        assert!(card.to_string().contains("Имя после рестарта"));
        assert!(card.to_string().contains("tg://user?id=1"));
        assert!(card.to_string().contains("Прибытие:"));
        assert!(card["rich_message"]["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["type"] == "map"));
    }
}

fn visible_messages(api: &ApiStub, user: i64) -> std::collections::HashSet<i64> {
    let mut visible = std::collections::HashSet::new();
    for body in api
        .bodies
        .lock()
        .unwrap()
        .iter()
        .filter(|b| b["chat_id"] == user && b["api_success"] == true)
    {
        if body["api_method"] == "deletemessage" {
            visible.remove(&body["message_id"].as_i64().unwrap());
        } else if body["api_method"] == "sendmessage" || body["api_method"] == "sendrichmessage" {
            visible.insert(body["api_message_id"].as_i64().unwrap());
        }
    }
    visible
}

#[tokio::test]
async fn smoke_publish_and_navigation_keep_one_editable_message() {
    let api = ApiStub::new();
    let mut runtime = test_runtime(&api).await;
    for user in [1, 2] {
        Box::pin(runtime.message(message(user, "/start", None)))
            .await
            .unwrap();
        press(&mut runtime, user, Action::Consent("v1".into())).await;
        if user == 2 {
            press(&mut runtime, user, Action::OpenHelperSettings).await;
            press(
                &mut runtime,
                user,
                Action::ToggleArea("spb-tsentralny".into()),
            )
            .await;
            press(&mut runtime, user, Action::SaveHelper).await;
        }
    }
    let mut primary = runtime.session(TelegramUserId(1)).menu.unwrap();
    let id = publish_smoke_request(&mut runtime).await;
    assert_eq!(runtime.session(TelegramUserId(1)).menu, Some(primary));
    assert_eq!(
        visible_messages(&api, 1),
        std::collections::HashSet::from([i64::from(primary.0)])
    );
    assert_eq!(
        runtime.session(TelegramUserId(1)).keyboard_messages.len(),
        0
    );
    for text in ["/active", "/active", "/start", "/active"] {
        Box::pin(runtime.message(message(1, text, None)))
            .await
            .unwrap();
        let next = runtime.session(TelegramUserId(1)).menu.unwrap();
        assert_ne!(next, primary);
        assert_eq!(
            visible_messages(&api, 1),
            std::collections::HashSet::from([i64::from(next.0)])
        );
        primary = next;
    }
    accept_after(&mut runtime, 2, id, 0, 5, 15).await;
    // A transient map failure must not create a replacement card.
    api.reject_map.store(true, Ordering::Relaxed);
    assert!(Box::pin(runtime.active(TelegramUserId(1))).await.is_err());
    assert_eq!(runtime.session(TelegramUserId(1)).menu, Some(primary));
    assert_eq!(visible_messages(&api, 1).len(), 1);
    Box::pin(runtime.active(TelegramUserId(1))).await.unwrap();
    press(&mut runtime, 1, Action::Resolve(id)).await;
    press(&mut runtime, 1, Action::Home).await;
    // A detached home menu must not be overwritten by a later request refresh.
    runtime.refresh(id).await.unwrap();
    assert!(menu_actions(&runtime, 1).contains(&Action::OpenCreateRequest));
    assert_eq!(visible_messages(&api, 1).len(), 1);
    let bodies = api.bodies.lock().unwrap();
    assert_eq!(
        bodies
            .iter()
            .filter(|b| b["chat_id"] == 1
                && b["api_method"] == "sendrichmessage"
                && b["api_success"] == true)
            .count(),
        5
    );
    assert!(bodies
        .iter()
        .filter(|b| b.get("rich_message").is_some())
        .all(|b| b.get("reply_markup").is_none()));
    assert!(!bodies
        .iter()
        .any(|b| b.to_string().contains("Заявка опубликована. Подходящие")));
}

#[tokio::test]
async fn smoke_multiple_active_requests_use_one_list_and_card() {
    let api = ApiStub::new();
    let mut runtime = test_runtime(&api).await;
    Box::pin(runtime.message(message(1, "/start", None)))
        .await
        .unwrap();
    press(&mut runtime, 1, Action::Consent("v1".into())).await;
    let primary = runtime.session(TelegramUserId(1)).menu.unwrap();
    let first = publish_smoke_request(&mut runtime).await;
    let second = publish_smoke_request(&mut runtime).await;
    assert_ne!(first, second);
    Box::pin(runtime.active(TelegramUserId(1))).await.unwrap();
    let actions = menu_actions(&runtime, 1);
    assert!(actions.contains(&Action::OpenRequest(first)));
    assert!(actions.contains(&Action::OpenRequest(second)));
    for id in [first, second, first] {
        press(&mut runtime, 1, Action::OpenRequest(id)).await;
        assert!(menu_actions(&runtime, 1).contains(&Action::Cancel(id)));
        press(&mut runtime, 1, Action::Home).await;
        press(&mut runtime, 1, Action::ActiveRequests).await;
    }
    assert_eq!(runtime.session(TelegramUserId(1)).menu, Some(primary));
    assert_eq!(visible_messages(&api, 1).len(), 1);
    assert!(
        Box::pin(runtime.action(TelegramUserId(3), Action::OpenRequest(first)))
            .await
            .is_err()
    );
    assert_eq!(visible_messages(&api, 3).len(), 0);
}

fn latest_map(api: &ApiStub, user: i64) -> Option<Value> {
    api.bodies
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|b| {
            b["chat_id"] == user && b.get("rich_message").is_some() && b["api_success"] == true
        })
        .and_then(|b| b["rich_message"]["blocks"].as_array())
        .and_then(|blocks| blocks.iter().find(|b| b["type"] == "map"))
        .cloned()
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn smoke_coarse_invitation_is_stable_and_becomes_exact_only_for_the_pair() {
    let api = ApiStub::new();
    let mut runtime = test_runtime(&api).await;
    for user in [1, 2, 3] {
        Box::pin(runtime.message(message(user, "/start", None)))
            .await
            .unwrap();
        press(&mut runtime, user, Action::Consent("v1".into())).await;
        if user != 1 {
            press(&mut runtime, user, Action::OpenHelperSettings).await;
            press(
                &mut runtime,
                user,
                Action::ToggleArea("spb-tsentralny".into()),
            )
            .await;
            press(&mut runtime, user, Action::SaveHelper).await;
        }
    }
    let id = publish_smoke_request(&mut runtime).await;
    let original = runtime.repo.request(id).await.unwrap().exact_point.unwrap();
    let zone = crate::domain::ApproximateLocation::from_point(original).unwrap();
    let coarse = latest_map(&api, 2).unwrap();
    assert_eq!(coarse, latest_map(&api, 3).unwrap());
    assert_eq!(coarse["location"]["latitude"], zone.center.lat);
    assert_eq!(coarse["location"]["longitude"], zone.center.lon);
    assert_ne!(zone.center, original);
    assert_eq!(coarse["location"]["horizontal_accuracy"], 800.0);
    assert_eq!(coarse["zoom"], 13);
    assert!(coarse["location"].get("heading").is_none());
    assert!(coarse["location"].get("live_period").is_none());
    let address = runtime.addresses.resolve(original).unwrap();
    for body in api
        .bodies
        .lock()
        .unwrap()
        .iter()
        .filter(|b| b["chat_id"] == 2 || b["chat_id"] == 3)
    {
        assert!(!body.to_string().contains(&address));
        assert!(!body.to_string().contains("tg://user?id=1"));
    }
    let card = callback(&runtime, 2, &Action::Accept(id, 0))
        .message
        .unwrap()
        .id();
    let mut outsider = callback(&runtime, 2, &Action::Accept(id, 0));
    outsider.from.id = teloxide::types::UserId(4);
    Box::pin(runtime.callback(outsider)).await.unwrap();
    assert_eq!(latest_map(&api, 4), None);
    press(&mut runtime, 2, Action::Accept(id, 0)).await;
    assert_eq!(latest_map(&api, 2), Some(coarse.clone()));
    press(&mut runtime, 2, Action::EnterArrival(id, 0)).await;
    assert_eq!(latest_map(&api, 2), Some(coarse.clone()));
    press(&mut runtime, 2, Action::ArrivalBack(id, 0)).await;
    press(
        &mut runtime,
        2,
        Action::AcceptAt(id, 0, ArrivalEstimate::new(5, 15).unwrap()),
    )
    .await;
    let exact = latest_map(&api, 2).unwrap();
    assert_eq!(exact["location"]["latitude"], original.lat);
    assert_eq!(exact["location"]["longitude"], original.lon);
    assert!(exact["location"].get("horizontal_accuracy").is_none());
    assert_eq!(exact["zoom"], 16);
    assert_eq!(latest_map(&api, 3), None);
    assert_eq!(
        api.bodies
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|b| b["chat_id"] == 2 && b.get("rich_message").is_some())
            .unwrap()["message_id"],
        card.0
    );
    press(&mut runtime, 2, Action::Withdraw(id)).await;
    assert_eq!(latest_map(&api, 2), Some(coarse.clone()));
    assert_eq!(latest_map(&api, 3), Some(coarse.clone()));
    let repo = runtime.repo.clone();
    runtime = test_runtime(&api).await;
    runtime.repo = repo;
    Box::pin(runtime.message(message(3, "/active", None)))
        .await
        .unwrap();
    assert_eq!(latest_map(&api, 3), Some(coarse));
    let version = runtime.repo.request(id).await.unwrap().version;
    accept_after(&mut runtime, 3, id, version, 15, 30).await;
    assert_eq!(latest_map(&api, 3), Some(exact));
    Box::pin(runtime.message(message(1, "/active", None)))
        .await
        .unwrap();
    press(&mut runtime, 1, Action::Resolve(id)).await;
    assert_eq!(latest_map(&api, 3), None);
    assert_eq!(latest_map(&api, 1), None);
    assert_eq!(runtime.repo.request(id).await.unwrap().exact_point, None);
}

#[tokio::test]
async fn smoke_commands_reopen_menu_and_buttons_edit_clicked_message() {
    let api = ApiStub::new();
    let mut runtime = test_runtime(&api).await;
    Box::pin(runtime.message(message(1, "/start", None)))
        .await
        .unwrap();
    press(&mut runtime, 1, Action::Consent("v1".into())).await;
    let mut primary = runtime.session(TelegramUserId(1)).menu.unwrap();
    for command in ["/start", "/start", "/help", "/active", "/cancel"] {
        let old_button = callback(&runtime, 1, &Action::OpenCreateRequest);
        Box::pin(runtime.message(message(1, command, None)))
            .await
            .unwrap();
        let next = runtime.session(TelegramUserId(1)).menu.unwrap();
        assert_ne!(next, primary);
        assert!(!runtime.surfaces.contains_key(&(1, primary)));
        assert_eq!(
            visible_messages(&api, 1),
            std::collections::HashSet::from([i64::from(next.0)])
        );
        Box::pin(runtime.callback(old_button)).await.unwrap();
        assert_eq!(runtime.session(TelegramUserId(1)).menu, Some(next));
        assert!(!runtime.session(TelegramUserId(1)).editing_request);
        primary = next;
    }
    press(&mut runtime, 1, Action::More).await;
    press(&mut runtime, 1, Action::Home).await;
    press(&mut runtime, 1, Action::OpenHelperSettings).await;
    press(&mut runtime, 1, Action::Home).await;
    assert_eq!(runtime.session(TelegramUserId(1)).menu, Some(primary));
    assert_eq!(visible_messages(&api, 1).len(), 1);
    // A button on a separate invitation navigates in that invitation, not in
    // an unrelated older main menu. The old primary surface is retired.
    let invitation = runtime
        .project(
            TelegramUserId(1),
            teloxide_ui::Ui::column().push(super::button_row(Action::Home)),
            None,
            None,
        )
        .await
        .unwrap();
    let button = callback(&runtime, 1, &Action::Home);
    assert_eq!(button.message.as_ref().unwrap().id(), invitation);
    Box::pin(runtime.callback(button)).await.unwrap();
    assert_eq!(runtime.session(TelegramUserId(1)).menu, Some(invitation));
    assert!(menu_actions(&runtime, 1).contains(&Action::OpenCreateRequest));
    assert_eq!(
        visible_messages(&api, 1),
        std::collections::HashSet::from([i64::from(invitation.0)])
    );
    let other = runtime
        .project(
            TelegramUserId(1),
            teloxide_ui::Ui::column().push(super::button_row(Action::Home)),
            None,
            None,
        )
        .await
        .unwrap();
    api.reject_edit.store(true, Ordering::Relaxed);
    press(&mut runtime, 1, Action::Home).await;
    let recovered = runtime.session(TelegramUserId(1)).menu.unwrap();
    assert_ne!(recovered, other);
    assert_ne!(recovered, invitation);
    assert!(!runtime.surfaces.contains_key(&(1, other)));
    assert!(!runtime.surfaces.contains_key(&(1, invitation)));
    assert_eq!(
        visible_messages(&api, 1),
        std::collections::HashSet::from([i64::from(recovered.0)])
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn smoke_manual_address_edit_match_withdraw_cancel_and_return_home() {
    let api = ApiStub::new();
    let mut runtime = test_runtime(&api).await;
    for user in [1, 2] {
        Box::pin(runtime.message(message(user, "/start", None)))
            .await
            .unwrap();
        press(&mut runtime, user, Action::Consent("v1".into())).await;
    }
    press(&mut runtime, 2, Action::OpenHelperSettings).await;
    press(&mut runtime, 2, Action::ToggleArea("spb-kirovsky".into())).await;
    press(&mut runtime, 2, Action::SaveHelper).await;
    press(&mut runtime, 1, Action::OpenCreateRequest).await;
    press(&mut runtime, 1, Action::SelectProblem("puncture".into())).await;
    Box::pin(runtime.message(message(1, "Неизвестная улица 9999", None)))
        .await
        .unwrap();
    assert_eq!(
        runtime.session(TelegramUserId(1)).request.step,
        Some(crate::ui::RequestStep::Location)
    );
    Box::pin(runtime.message(message(1, "Дачный 17 к", None)))
        .await
        .unwrap();
    assert!(
        menu_actions(&runtime, 1)
            .iter()
            .filter(|a| matches!(a, Action::SelectAddress(_)))
            .count()
            >= 2
    );
    let old_address = callback(&runtime, 1, &Action::SelectAddress(0));
    Box::pin(runtime.message(message(1, "Дачный проспект, дом 17 корпус 3", None)))
        .await
        .unwrap();
    assert_eq!(
        runtime.session(TelegramUserId(1)).request.step,
        Some(crate::ui::RequestStep::Preview)
    );
    let point = runtime.session(TelegramUserId(1)).request.exact_point;
    Box::pin(runtime.callback(old_address)).await.unwrap();
    assert_eq!(
        runtime.session(TelegramUserId(1)).request.exact_point,
        point
    );
    press(&mut runtime, 1, Action::EditAddress).await;
    Box::pin(runtime.message(message(1, "", None)))
        .await
        .unwrap();
    assert_eq!(
        runtime.session(TelegramUserId(1)).request.step,
        Some(crate::ui::RequestStep::Address)
    );
    Box::pin(runtime.message(message(1, "Дачный проспект, 17 к3, подъезд 2", None)))
        .await
        .unwrap();
    assert_eq!(
        runtime.session(TelegramUserId(1)).request.exact_point,
        point
    );
    press(&mut runtime, 1, Action::PublishRequest).await;
    let request = runtime
        .repo
        .active_requests(TelegramUserId(1))
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(
        request.exact_address.as_deref(),
        Some("Дачный проспект, 17 к3, подъезд 2")
    );
    assert!(!api
        .bodies
        .lock()
        .unwrap()
        .iter()
        .filter(|b| b["chat_id"] == 2)
        .any(|b| b.to_string().contains("подъезд 2")));
    accept_after(&mut runtime, 2, request.id, 0, 15, 30).await;
    assert!(api
        .bodies
        .lock()
        .unwrap()
        .iter()
        .filter(|b| b["chat_id"] == 2)
        .any(|b| b.to_string().contains("подъезд 2")));
    press(&mut runtime, 2, Action::Withdraw(request.id)).await;
    assert_eq!(
        runtime
            .repo
            .request(request.id)
            .await
            .unwrap()
            .exact_address,
        request.exact_address
    );
    let _ = callback(&runtime, 2, &Action::Home);
    press(&mut runtime, 1, Action::Cancel(request.id)).await;
    assert_eq!(
        runtime
            .repo
            .request(request.id)
            .await
            .unwrap()
            .exact_address,
        None
    );
    press(&mut runtime, 2, Action::Home).await;
    assert!(menu_actions(&runtime, 2).contains(&Action::OpenHelperSettings));
}

#[tokio::test]
async fn smoke_failed_publish_card_recovers_without_duplicate_request() {
    let api = ApiStub::new();
    let mut runtime = test_runtime(&api).await;
    Box::pin(runtime.message(message(1, "/start", None)))
        .await
        .unwrap();
    press(&mut runtime, 1, Action::Consent("v1".into())).await;
    press(&mut runtime, 1, Action::OpenCreateRequest).await;
    press(&mut runtime, 1, Action::SelectProblem("puncture".into())).await;
    api.reject_keyboard_delete.store(true, Ordering::Relaxed);
    Box::pin(runtime.message(message(1, "", GeoPoint::new(59.938, 30.35))))
        .await
        .unwrap();
    runtime.deliver().await.unwrap();
    assert_eq!(
        runtime.session(TelegramUserId(1)).keyboard_messages.len(),
        0
    );
    let publish = callback(&runtime, 1, &Action::PublishRequest);
    api.reject_rich.store(true, Ordering::Relaxed);
    Box::pin(runtime.callback(publish.clone())).await.unwrap();
    assert_eq!(
        runtime
            .repo
            .active_requests(TelegramUserId(1))
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(api.bodies.lock().unwrap().iter().any(|b| b["text"]
        .as_str()
        .is_some_and(|text| text.contains("Заявка опубликована"))));
    // The previous callback may remain valid when edit failed; it must never
    // insert a second request even if delivered again before the next /active.
    assert!(Box::pin(runtime.callback(publish)).await.is_err());
    assert_eq!(
        runtime
            .repo
            .active_requests(TelegramUserId(1))
            .await
            .unwrap()
            .len(),
        1
    );
    Box::pin(runtime.message(message(1, "/active", None)))
        .await
        .unwrap();
    assert!(menu_actions(&runtime, 1)
        .iter()
        .any(|a| matches!(a, Action::Cancel(_))));
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn smoke_each_problem_reaches_only_its_selected_helper() {
    let api = ApiStub::new();
    let mut runtime = test_runtime(&api).await;
    let helpers = [2, 3, 4, 6, 7, 8, 9];
    for (problem, helper) in crate::domain::HELP_PROBLEMS.iter().zip(helpers) {
        Box::pin(runtime.message(message(helper, "/start", None)))
            .await
            .unwrap();
        press(&mut runtime, helper, Action::Consent("v1".into())).await;
        press(&mut runtime, helper, Action::OpenHelperSettings).await;
        press(
            &mut runtime,
            helper,
            Action::ToggleArea("spb-kirovsky".into()),
        )
        .await;
        press(
            &mut runtime,
            helper,
            Action::ToggleCapability("technical".into()),
        )
        .await;
        assert!(!menu_actions(&runtime, helper).contains(&Action::SaveHelper));
        let action = if *problem == "accident" {
            Action::ToggleCapability("accident".into())
        } else {
            Action::ToggleHelperProblem((*problem).to_owned())
        };
        press(&mut runtime, helper, action).await;
        press(&mut runtime, helper, Action::SaveHelper).await;
        let profile = runtime
            .repo
            .helper(TelegramUserId(helper))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(profile.problem_tags, vec![(*problem).to_owned()]);
    }
    for (index, (problem, helper)) in crate::domain::HELP_PROBLEMS.iter().zip(helpers).enumerate() {
        let requester = 20 + i64::try_from(index).unwrap();
        Box::pin(runtime.message(message(requester, "/start", None)))
            .await
            .unwrap();
        press(&mut runtime, requester, Action::Consent("v1".into())).await;
        press(&mut runtime, requester, Action::OpenCreateRequest).await;
        press(
            &mut runtime,
            requester,
            Action::SelectProblem((*problem).to_owned()),
        )
        .await;
        Box::pin(runtime.message(message(requester, "Дачный 17к3", None)))
            .await
            .unwrap();
        press(&mut runtime, requester, Action::PublishRequest).await;
        let request = runtime
            .repo
            .active_requests(TelegramUserId(requester))
            .await
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(
            runtime.repo.candidates(request.id).await.unwrap(),
            vec![TelegramUserId(helper)]
        );
        let notified: Vec<_> = runtime
            .surfaces
            .values()
            .filter(|s| s.request == Some(request.id) && s.owner.0 != requester)
            .map(|s| s.owner)
            .collect();
        assert_eq!(notified, vec![TelegramUserId(helper)], "{problem}");
        press(&mut runtime, requester, Action::Cancel(request.id)).await;
        let terminal = callback(&runtime, helper, &Action::Home);
        Box::pin(runtime.callback(terminal)).await.unwrap();
        assert!(menu_actions(&runtime, helper).contains(&Action::OpenHelperSettings));
    }
}
