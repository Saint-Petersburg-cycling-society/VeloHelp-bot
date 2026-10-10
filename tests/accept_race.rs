// SPDX-License-Identifier: AGPL-3.0-only

//! Atomic accept race: N helpers vs one open request, real SQLite adapter.
//! See `tests/domain_accept_spec.md`.

use std::time::Duration;

use bike_aid::{
    application::{DispatchRepository, HelperRepository, RequestRepository},
    config::Config,
    domain::{
        AcceptOutcome, HelpKind, HelpRequest, HelpRequestId, HelpRequestStatus, HelperProfile,
        TelegramUserId,
    },
    storage::{connect, SqliteRepositories},
};
use chrono::{TimeDelta, Utc};

async fn test_pool(name: &str) -> SqliteRepositories {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp");
    std::fs::create_dir_all(&dir).expect("create test-tmp");
    let path = dir.join(format!(
        "{name}-{}-{}.sqlite",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let pool = connect(&url).await.expect("connect");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    SqliteRepositories::new(pool)
}

fn open_request(requester: TelegramUserId, area: &str) -> HelpRequest {
    let mut req = HelpRequest::draft(
        requester,
        HelpKind::Technical,
        "puncture near park".to_owned(),
        Utc::now() + TimeDelta::minutes(60),
    );
    req.status = HelpRequestStatus::Open;
    req.area_id = Some(area.to_owned());
    req
}

const RACE_N: i64 = 8;

#[tokio::test]
async fn concurrent_accept_has_exactly_one_winner() {
    let repo = test_pool("accept-race").await;
    let now = Utc::now().timestamp();
    let area = "spb-center";
    let requester = TelegramUserId(9001);
    repo.ensure_user(requester, "2026-10-03", now)
        .await
        .unwrap();

    let mut helpers = Vec::with_capacity(usize::try_from(RACE_N).unwrap());
    for i in 0..RACE_N {
        let id = TelegramUserId(9100 + i);
        repo.ensure_user(id, "2026-10-03", now).await.unwrap();
        repo.upsert_helper(&HelperProfile {
            user_id: id,
            enabled: true,
            areas: vec![area.to_owned()],
            help_kinds: vec![HelpKind::Technical],
        })
        .await
        .unwrap();
        helpers.push(id);
    }

    let req = open_request(requester, area);
    let request_id = req.id;
    repo.insert(&req).await.unwrap();
    repo.fanout(request_id, &helpers).await.unwrap();

    let barriers: Vec<_> = helpers
        .iter()
        .map(|h| {
            let repo = repo.clone();
            let helper = *h;
            tokio::spawn(async move { repo.accept(request_id, helper).await })
        })
        .collect();
    let mut won = 0;
    let mut taken = 0;
    for task in barriers {
        match task.await.expect("task").expect("accept") {
            AcceptOutcome::Won => won += 1,
            AcceptOutcome::AlreadyTaken => taken += 1,
            other => panic!("unexpected outcome: {other:?}"),
        }
    }
    assert_eq!(won, 1, "exactly one winner");
    assert_eq!(
        taken,
        usize::try_from(RACE_N - 1).unwrap(),
        "rest observe AlreadyTaken"
    );

    let pool = repo.pool();
    let (status, matched): (String, Option<i64>) =
        sqlx::query_as("SELECT status, matched_helper_id FROM help_requests WHERE id=?")
            .bind(request_id.0.to_string())
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(status, "matched");
    let matched = matched.expect("matched_helper_id");
    assert!(helpers.iter().any(|h| h.0 == matched));

    let counts: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, COUNT(*) FROM dispatches WHERE request_id=? GROUP BY status",
    )
    .bind(request_id.0.to_string())
    .fetch_all(pool)
    .await
    .unwrap();
    let accepted = counts
        .iter()
        .find(|(s, _)| s == "accepted")
        .map_or(0, |(_, c)| *c);
    let superseded = counts
        .iter()
        .find(|(s, _)| s == "superseded")
        .map_or(0, |(_, c)| *c);
    assert_eq!(accepted, 1);
    assert_eq!(superseded, RACE_N - 1);
}

#[tokio::test]
async fn cancel_races_accept_without_panic() {
    let repo = test_pool("cancel-race").await;
    let now = Utc::now().timestamp();
    let area = "spb-north";
    let requester = TelegramUserId(9201);
    repo.ensure_user(requester, "2026-10-03", now)
        .await
        .unwrap();
    let helper = TelegramUserId(9202);
    repo.ensure_user(helper, "2026-10-03", now).await.unwrap();
    repo.upsert_helper(&HelperProfile {
        user_id: helper,
        enabled: true,
        areas: vec![area.to_owned()],
        help_kinds: vec![HelpKind::Technical],
    })
    .await
    .unwrap();

    let req = open_request(requester, area);
    let request_id = req.id;
    repo.insert(&req).await.unwrap();
    repo.fanout(request_id, &[helper]).await.unwrap();

    let r1 = repo.clone();
    let accept = tokio::spawn(async move { r1.accept(request_id, helper).await });
    let r2 = repo.clone();
    let cancel = tokio::spawn(async move { r2.cancel(request_id, requester).await });
    let (accept_outcome, cancel_outcome) = tokio::join!(accept, cancel);
    // Either order is a valid domain outcome; the point is no panic and a
    // consistent terminal state.
    let _ = accept_outcome.expect("accept task").expect("accept");
    let _ = cancel_outcome.expect("cancel task");

    let pool = repo.pool();
    let (status,): (String,) = sqlx::query_as("SELECT status FROM help_requests WHERE id=?")
        .bind(request_id.0.to_string())
        .fetch_one(pool)
        .await
        .unwrap();
    assert!(
        ["matched", "cancelled", "expired"].contains(&status.as_str()),
        "consistent terminal state, got {status}"
    );
}

#[tokio::test]
async fn expiry_boundary_marks_open_expired() {
    let repo = test_pool("expiry").await;
    let now = Utc::now().timestamp();
    let area = "spb-south";
    let requester = TelegramUserId(9301);
    repo.ensure_user(requester, "2026-10-03", now)
        .await
        .unwrap();
    let helper = TelegramUserId(9302);
    repo.ensure_user(helper, "2026-10-03", now).await.unwrap();
    repo.upsert_helper(&HelperProfile {
        user_id: helper,
        enabled: true,
        areas: vec![area.to_owned()],
        help_kinds: vec![HelpKind::Technical],
    })
    .await
    .unwrap();

    // TTL 5 minutes is the minimum accepted by Config.
    let ttl = Config::parse_ttl_minutes("5").unwrap();
    assert_eq!(ttl, Duration::from_secs(300));
    let mut req = HelpRequest::draft(
        requester,
        HelpKind::Technical,
        "expiry check".to_owned(),
        Utc::now() + TimeDelta::seconds(1),
    );
    req.status = HelpRequestStatus::Open;
    req.area_id = Some(area.to_owned());
    let request_id: HelpRequestId = req.id;
    repo.insert(&req).await.unwrap();
    repo.fanout(request_id, &[helper]).await.unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;
    let expired = repo.expire_due(Utc::now().timestamp()).await.unwrap();
    assert_eq!(expired, 1);

    let outcome = repo.accept(request_id, helper).await.unwrap();
    assert_eq!(outcome, AcceptOutcome::Expired);
}
