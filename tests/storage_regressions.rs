// SPDX-License-Identifier: AGPL-3.0-only
use bike_aid::{
    application::{DispatchRepository, HelperRepository, RequestRepository},
    domain::{
        AcceptOutcome, ArrivalEstimate, GeoPoint, HelpKind, HelpRequest, HelpRequestStatus,
        HelperProfile, TelegramUserId,
    },
    ops::{
        backup::backup_sqlite,
        privacy::{delete_user, export_user_json},
    },
    storage::{connect, SqliteRepositories},
};
use chrono::{TimeDelta, Utc};

async fn setup() -> (SqliteRepositories, HelpRequest) {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-tmp");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("regression-{}.sqlite", uuid::Uuid::new_v4()));
    let pool = connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let repo = SqliteRepositories::new(pool);
    for id in [1, 2, 3] {
        repo.ensure_user(TelegramUserId(id), "v1", Utc::now().timestamp())
            .await
            .unwrap();
    }
    for id in [2, 3] {
        repo.upsert_helper(&HelperProfile {
            user_id: TelegramUserId(id),
            enabled: true,
            areas: vec!["center".into()],
            help_kinds: vec![HelpKind::Technical],
        })
        .await
        .unwrap();
    }
    let mut request = HelpRequest::draft(
        TelegramUserId(1),
        HelpKind::Technical,
        "Описание с кавычками: \"test\"\nновая строка".into(),
        Utc::now() + TimeDelta::minutes(60),
    );
    request.area_id = Some("center".into());
    request.exact_point = GeoPoint::new(59.9, 30.3);
    request.status = HelpRequestStatus::Open;
    repo.insert(&request).await.unwrap();
    (repo, request)
}

#[tokio::test]
async fn accept_requires_current_pending_dispatch_and_enabled_helper() {
    let (repo, request) = setup().await;
    for id in [1, 2, 999] {
        assert_eq!(
            repo.accept(request.id, TelegramUserId(id), 0)
                .await
                .unwrap(),
            AcceptOutcome::Forbidden
        );
    }
    repo.fanout(request.id, &[TelegramUserId(2)]).await.unwrap();
    sqlx::query("UPDATE helper_profiles SET enabled=0 WHERE user_id=2")
        .execute(repo.pool())
        .await
        .unwrap();
    assert_eq!(
        repo.accept(request.id, TelegramUserId(2), 0).await.unwrap(),
        AcceptOutcome::Forbidden
    );
    sqlx::query("UPDATE helper_profiles SET enabled=1 WHERE user_id=2")
        .execute(repo.pool())
        .await
        .unwrap();
    assert_eq!(
        repo.accept(request.id, TelegramUserId(2), 0).await.unwrap(),
        AcceptOutcome::Won
    );
}

#[tokio::test]
async fn withdrawal_creates_new_wave_and_old_accept_cannot_win() {
    let (repo, request) = setup().await;
    repo.fanout(request.id, &[TelegramUserId(2), TelegramUserId(3)])
        .await
        .unwrap();
    assert_eq!(
        repo.accept(request.id, TelegramUserId(2), 0).await.unwrap(),
        AcceptOutcome::Won
    );
    repo.withdraw(request.id, TelegramUserId(2)).await.unwrap();
    repo.fanout(request.id, &[TelegramUserId(2), TelegramUserId(3)])
        .await
        .unwrap();
    assert_eq!(
        repo.accept(request.id, TelegramUserId(3), 0).await.unwrap(),
        AcceptOutcome::Stale
    );
    assert_eq!(
        repo.accept(request.id, TelegramUserId(3), 2).await.unwrap(),
        AcceptOutcome::Won
    );
    repo.fanout(request.id, &[TelegramUserId(2)]).await.unwrap();
    let (pending,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM dispatches WHERE status='pending'")
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(pending, 0, "fanout must not resurrect a matched request");
}

#[tokio::test]
async fn cancellation_clears_coordinates_and_invalidates_all_dispatches() {
    let (repo, request) = setup().await;
    repo.fanout(request.id, &[TelegramUserId(2), TelegramUserId(3)])
        .await
        .unwrap();
    assert!(repo.cancel(request.id, TelegramUserId(3)).await.is_err());
    repo.cancel(request.id, TelegramUserId(1)).await.unwrap();
    let (lat, lon): (Option<f64>, Option<f64>) =
        sqlx::query_as("SELECT exact_lat, exact_lon FROM help_requests")
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!((lat, lon), (None, None));
    let (pending,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM dispatches WHERE status='pending'")
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(pending, 0);

    let (repo, request) = setup().await;
    repo.fanout(request.id, &[TelegramUserId(2)]).await.unwrap();
    assert_eq!(
        repo.accept(request.id, TelegramUserId(2), 0).await.unwrap(),
        AcceptOutcome::Won
    );
    repo.cancel(request.id, TelegramUserId(1)).await.unwrap();
    let (status, matched_helper, matched_at, lat, lon):
        (String, Option<i64>, Option<i64>, Option<f64>, Option<f64>) = sqlx::query_as(
            "SELECT status, matched_helper_id, matched_at, exact_lat, exact_lon FROM help_requests WHERE id=?",
        )
        .bind(request.id.0.to_string())
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(status, "cancelled");
    assert_eq!(
        (matched_helper, matched_at, lat, lon),
        (None, None, None, None)
    );
    let (accepted, superseded): (i64, i64) = sqlx::query_as(
        "SELECT sum(status='accepted'), sum(status='superseded') FROM dispatches WHERE request_id=?",
    )
    .bind(request.id.0.to_string())
    .fetch_one(repo.pool())
    .await
    .unwrap();
    assert_eq!((accepted, superseded), (0, 1));
}

#[tokio::test]
async fn insert_rolls_back_on_failed_tag_and_preserves_matched_helper() {
    let (repo, mut request) = setup().await;
    sqlx::query("CREATE TRIGGER reject_bad_tag BEFORE INSERT ON request_problem_tags WHEN NEW.tag='bad' BEGIN SELECT RAISE(ABORT, 'bad tag'); END")
        .execute(repo.pool()).await.unwrap();
    request.id = bike_aid::domain::HelpRequestId::generate();
    request.problem_tags = vec!["good".into(), "bad".into()];
    assert!(repo.insert(&request).await.is_err());
    let (rows,): (i64,) = sqlx::query_as("SELECT count(*) FROM help_requests WHERE id=?")
        .bind(request.id.0.to_string())
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(rows, 0);
    request.problem_tags.clear();
    request.status = HelpRequestStatus::Matched;
    request.matched_helper_id = Some(TelegramUserId(2));
    repo.insert(&request).await.unwrap();
}

#[tokio::test]
async fn export_is_valid_json_with_identity_settings_and_exact_point() {
    let (repo, request) = setup().await;
    let json: serde_json::Value = serde_json::from_str(
        &export_user_json(repo.pool(), TelegramUserId(1))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(json["user"][0]["telegram_user_id"], 1);
    assert_eq!(json["requests"][0]["description"], request.description);
    assert_eq!(json["requests"][0]["exact_lat"], 59.9);
    let helper: serde_json::Value = serde_json::from_str(
        &export_user_json(repo.pool(), TelegramUserId(2))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(helper["areas"][0]["area_id"], "center");
    assert_eq!(helper["requests"], serde_json::json!([]));
}

#[tokio::test]
async fn deletion_works_for_requester_and_matched_helper_without_orphans() {
    let (repo, request) = setup().await;
    repo.fanout(request.id, &[TelegramUserId(2), TelegramUserId(3)])
        .await
        .unwrap();
    repo.accept(request.id, TelegramUserId(2), 0).await.unwrap();
    delete_user(repo.pool(), TelegramUserId(2)).await.unwrap();
    let (status, helper, lat): (String, Option<i64>, Option<f64>) =
        sqlx::query_as("SELECT status, matched_helper_id, exact_lat FROM help_requests")
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(status, "cancelled");
    assert_eq!((helper, lat), (None, None));
    delete_user(repo.pool(), TelegramUserId(1)).await.unwrap();
    let (rows,): (i64,) = sqlx::query_as("SELECT count(*) FROM help_requests")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(rows, 0);
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(repo.pool())
        .await
        .unwrap();
    assert!(violations.is_empty());
}

#[tokio::test]
async fn backup_supports_quoted_paths_without_sql_injection() {
    let (repo, _) = setup().await;
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-tmp");
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join(format!("backup-'quote';-{}.sqlite", uuid::Uuid::new_v4()));
    backup_sqlite(repo.pool(), dest.to_str().unwrap())
        .await
        .unwrap();
    let (rows,): (i64,) = sqlx::query_as("SELECT count(*) FROM users")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(rows, 3);
    std::fs::remove_file(dest).unwrap();
}

#[tokio::test]
async fn consent_renewal_updates_acceptance_timestamp() {
    let (repo, _) = setup().await;
    repo.ensure_user(TelegramUserId(1), "v2", 1234)
        .await
        .unwrap();
    let (version, time): (String, i64) =
        sqlx::query_as("SELECT consent_version, consented_at FROM users WHERE telegram_user_id=1")
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!((version.as_str(), time), ("v2", 1234));
}

#[tokio::test]
async fn atomic_accept_also_serializes_independent_sqlite_connections() {
    let (repo, request) = setup().await;
    repo.fanout(request.id, &[TelegramUserId(2), TelegramUserId(3)])
        .await
        .unwrap();
    let path = repo.pool().connect_options().get_filename().to_owned();
    let other = SqliteRepositories::new(
        connect(&format!("sqlite://{}", path.display()))
            .await
            .unwrap(),
    );
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let barrier_other = barrier.clone();
    let id = request.id;
    let left = tokio::spawn(async move {
        barrier.wait().await;
        repo.accept(id, TelegramUserId(2), 0).await.unwrap()
    });
    let right = tokio::spawn(async move {
        barrier_other.wait().await;
        other.accept(id, TelegramUserId(3), 0).await.unwrap()
    });
    let outcomes = [left.await.unwrap(), right.await.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| **o == AcceptOutcome::Won)
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| **o == AcceptOutcome::AlreadyTaken)
            .count(),
        1
    );
}

#[tokio::test]
async fn upgrade_from_second_migration_backfills_live_wave_and_repeats_cleanly() {
    let pool = connect("sqlite::memory:").await.unwrap();
    for sql in [
        include_str!("../migrations/0001_init.sql"),
        include_str!("../migrations/0002_indexes.sql"),
    ] {
        sqlx::raw_sql(sql).execute(&pool).await.unwrap();
    }
    let repo = SqliteRepositories::new(pool);
    for id in [1, 2] {
        repo.ensure_user(TelegramUserId(id), "v1", Utc::now().timestamp())
            .await
            .unwrap();
    }
    repo.upsert_helper(&HelperProfile {
        user_id: TelegramUserId(2),
        enabled: true,
        areas: vec!["center".into()],
        help_kinds: vec![HelpKind::Technical],
    })
    .await
    .unwrap();
    let mut request = HelpRequest::draft(
        TelegramUserId(1),
        HelpKind::Technical,
        "upgrade".into(),
        Utc::now() + TimeDelta::minutes(60),
    );
    request.status = HelpRequestStatus::Open;
    request.area_id = Some("center".into());
    request.version = 7;
    repo.insert(&request).await.unwrap();
    sqlx::query("INSERT INTO dispatches(request_id,helper_id) VALUES(?,2)")
        .bind(request.id.0.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    // Model SQLx history of a deployment that already applied migrations 1 and 2.
    sqlx::raw_sql("CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT NOT NULL, installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL)").execute(repo.pool()).await.unwrap();
    let migrator = sqlx::migrate!("./migrations");
    for migration in migrator.iter().take(2) {
        sqlx::query("INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES(?,?,1,?,0)").bind(migration.version).bind(migration.description.as_ref()).bind(migration.checksum.as_ref()).execute(repo.pool()).await.unwrap();
    }
    migrator.run(repo.pool()).await.unwrap();
    migrator.run(repo.pool()).await.unwrap();
    let (version,): (i64,) = sqlx::query_as("SELECT request_version FROM dispatches")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(version, 7);
    assert_eq!(
        repo.accept(request.id, TelegramUserId(2), 7).await.unwrap(),
        AcceptOutcome::Won
    );
}

#[tokio::test]
async fn withdrawal_after_ttl_expires_and_removes_exact_point() {
    let (repo, request) = setup().await;
    repo.fanout(request.id, &[TelegramUserId(2)]).await.unwrap();
    repo.accept(request.id, TelegramUserId(2), 0).await.unwrap();
    sqlx::query(
        "UPDATE help_requests SET created_at=unixepoch()-100, expires_at=unixepoch()-1 WHERE id=?",
    )
    .bind(request.id.0.to_string())
    .execute(repo.pool())
    .await
    .unwrap();
    repo.withdraw(request.id, TelegramUserId(2)).await.unwrap();
    let request = repo.request(request.id).await.unwrap();
    assert_eq!(request.status, HelpRequestStatus::Expired);
    assert_eq!(request.exact_point, None);
    assert_eq!(request.matched_helper_id, None);
}

#[tokio::test]
async fn arrival_validation_export_and_terminal_retention() {
    for close in ["resolve", "cancel", "withdraw", "delete"] {
        let (repo, request) = setup().await;
        repo.fanout(request.id, &[TelegramUserId(2)]).await.unwrap();
        for estimate in [
            ArrivalEstimate {
                min_minutes: 0,
                max_minutes: 15,
            },
            ArrivalEstimate {
                min_minutes: 30,
                max_minutes: 15,
            },
            ArrivalEstimate {
                min_minutes: 1,
                max_minutes: 241,
            },
        ] {
            assert!(repo
                .accept_with_arrival(request.id, TelegramUserId(2), 0, Some(estimate))
                .await
                .is_err());
            assert_eq!(
                repo.request(request.id).await.unwrap().status,
                HelpRequestStatus::Open
            );
        }
        assert_eq!(
            repo.accept_with_arrival(
                request.id,
                TelegramUserId(2),
                0,
                ArrivalEstimate::new(7, 19)
            )
            .await
            .unwrap(),
            AcceptOutcome::Won
        );
        let window = repo
            .request(request.id)
            .await
            .unwrap()
            .arrival_window
            .unwrap();
        assert_eq!((window.latest - window.earliest).num_minutes(), 12);
        let export: serde_json::Value = serde_json::from_str(
            &export_user_json(repo.pool(), TelegramUserId(2))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            export["arrival_estimates"][0]["arrival_from_at"],
            window.earliest.timestamp()
        );
        assert!(export["arrival_estimates"][0].get("exact_lat").is_none());
        match close {
            "resolve" => repo.resolve(request.id, TelegramUserId(1)).await.unwrap(),
            "cancel" => repo.cancel(request.id, TelegramUserId(1)).await.unwrap(),
            "withdraw" => repo.withdraw(request.id, TelegramUserId(2)).await.unwrap(),
            "delete" => delete_user(repo.pool(), TelegramUserId(2)).await.unwrap(),
            _ => unreachable!(),
        }
        assert_eq!(repo.request(request.id).await.unwrap().arrival_window, None);
    }
}

#[tokio::test]
async fn upgrade_from_fourth_migration_preserves_existing_match() {
    let pool = connect("sqlite::memory:").await.unwrap();
    let migrator = sqlx::migrate!("./migrations");
    for migration in migrator.iter().take(4) {
        sqlx::raw_sql(migration.sql.as_ref())
            .execute(&pool)
            .await
            .unwrap();
    }
    let repo = SqliteRepositories::new(pool);
    for user in [1, 2] {
        repo.ensure_user(TelegramUserId(user), "v1", Utc::now().timestamp())
            .await
            .unwrap();
    }
    let mut request = HelpRequest::draft(
        TelegramUserId(1),
        HelpKind::Technical,
        "legacy match".into(),
        chrono::DateTime::from_timestamp(Utc::now().timestamp() + 3600, 0).unwrap(),
    );
    request.status = HelpRequestStatus::Matched;
    request.matched_helper_id = Some(TelegramUserId(2));
    request.exact_point = GeoPoint::new(59.938, 30.35);
    request.version = 1;
    repo.insert(&request).await.unwrap();
    sqlx::raw_sql("CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT NOT NULL, installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL)").execute(repo.pool()).await.unwrap();
    for migration in migrator.iter().take(4) {
        sqlx::query("INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES(?,?,1,?,0)").bind(migration.version).bind(migration.description.as_ref()).bind(migration.checksum.as_ref()).execute(repo.pool()).await.unwrap();
    }
    migrator.run(repo.pool()).await.unwrap();
    migrator.run(repo.pool()).await.unwrap();
    assert_eq!(repo.request(request.id).await.unwrap(), request);
    assert!(
        sqlx::query("UPDATE help_requests SET arrival_from_at=1 WHERE id=?")
            .bind(request.id.0.to_string())
            .execute(repo.pool())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn accept_and_decline_race_has_one_linearized_result() {
    for _ in 0..12 {
        let (repo, request) = setup().await;
        repo.fanout(request.id, &[TelegramUserId(2), TelegramUserId(3)])
            .await
            .unwrap();
        let path = repo.pool().connect_options().get_filename().to_owned();
        let other = SqliteRepositories::new(
            connect(&format!("sqlite://{}", path.display()))
                .await
                .unwrap(),
        );
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
        let wait = barrier.clone();
        let r1 = repo.clone();
        let accept = tokio::spawn(async move {
            wait.wait().await;
            r1.accept(request.id, TelegramUserId(2), 0).await
        });
        let wait = barrier.clone();
        let decline = tokio::spawn(async move {
            wait.wait().await;
            other.decline(request.id, TelegramUserId(2), 0).await
        });
        let (accept, decline) = tokio::join!(accept, decline);
        let accept = accept.unwrap().unwrap();
        let decline = decline.unwrap();
        let (status, winner): (String, Option<i64>) =
            sqlx::query_as("SELECT status, matched_helper_id FROM help_requests WHERE id=?")
                .bind(request.id.0.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
        let (accepted, declined, pending): (i64, i64, i64) = sqlx::query_as(
            "SELECT sum(status='accepted'), sum(status='declined'), sum(status='pending') FROM dispatches WHERE request_id=? AND helper_id=2",
        )
        .bind(request.id.0.to_string())
        .fetch_one(repo.pool())
        .await
        .unwrap();
        match accept {
            AcceptOutcome::Won => {
                assert!(
                    decline.is_err(),
                    "decline cannot reverse a committed accept"
                );
                assert_eq!((status.as_str(), winner), ("matched", Some(2)));
                assert_eq!((accepted, declined, pending), (1, 0, 0));
            }
            AcceptOutcome::Forbidden => {
                assert!(decline.is_ok(), "the dispatch was declined first");
                assert_eq!((status.as_str(), winner), ("open", None));
                assert_eq!((accepted, declined, pending), (0, 1, 0));
            }
            other => panic!("unexpected accept outcome: {other:?}"),
        }
    }
}

#[tokio::test]
#[allow(clippy::type_complexity, clippy::too_many_lines)]
async fn withdraw_and_cancel_race_never_leaves_a_half_open_match() {
    for _ in 0..12 {
        let (repo, request) = setup().await;
        repo.fanout(request.id, &[TelegramUserId(2), TelegramUserId(3)])
            .await
            .unwrap();
        assert_eq!(
            repo.accept(request.id, TelegramUserId(2), 0).await.unwrap(),
            AcceptOutcome::Won
        );
        let path = repo.pool().connect_options().get_filename().to_owned();
        let other = SqliteRepositories::new(
            connect(&format!("sqlite://{}", path.display()))
                .await
                .unwrap(),
        );
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
        let wait = barrier.clone();
        let r1 = repo.clone();
        let withdraw = tokio::spawn(async move {
            wait.wait().await;
            r1.withdraw(request.id, TelegramUserId(2)).await
        });
        let wait = barrier.clone();
        let cancel = tokio::spawn(async move {
            wait.wait().await;
            other.cancel(request.id, TelegramUserId(1)).await
        });
        let (withdraw, cancel) = tokio::join!(withdraw, cancel);
        let withdraw = withdraw.unwrap();
        let cancel = cancel.unwrap();
        let (status, winner, version, latitude, longitude, arrival_from, arrival_until):
            (String, Option<i64>, i64, Option<f64>, Option<f64>, Option<i64>, Option<i64>) =
            sqlx::query_as("SELECT status, matched_helper_id, version, exact_lat, exact_lon, arrival_from_at, arrival_until_at FROM help_requests WHERE id=?")
                .bind(request.id.0.to_string()).fetch_one(repo.pool()).await.unwrap();
        assert!(["open", "cancelled"].contains(&status.as_str()));
        assert_eq!(winner, None);
        assert!(arrival_from.is_none() && arrival_until.is_none());
        let dispatches: Vec<(String, i64)> = sqlx::query_as(
            "SELECT status, request_version FROM dispatches WHERE request_id=? ORDER BY helper_id",
        )
        .bind(request.id.0.to_string())
        .fetch_all(repo.pool())
        .await
        .unwrap();
        assert!(dispatches.iter().all(|(state, _)| state == "superseded"));
        match status.as_str() {
            "open" => {
                assert!(withdraw.is_ok());
                assert!(cancel.is_err());
                assert_eq!(version, 2);
                assert!(latitude.is_some() && longitude.is_some());
                repo.prepare_dispatches().await.unwrap();
                let (pending, current_wave): (i64, i64) = sqlx::query_as(
                    "SELECT sum(status='pending'), sum(status='pending' AND request_version=2) FROM dispatches WHERE request_id=?",
                )
                .bind(request.id.0.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
                assert_eq!((pending, current_wave), (2, 2));
                assert_eq!(
                    repo.accept(request.id, TelegramUserId(3), 0).await.unwrap(),
                    AcceptOutcome::Stale
                );
                assert_eq!(
                    repo.accept(request.id, TelegramUserId(3), 2).await.unwrap(),
                    AcceptOutcome::Won
                );
                assert_eq!(
                    repo.accept(request.id, TelegramUserId(2), 2).await.unwrap(),
                    AcceptOutcome::AlreadyTaken
                );
                let current = repo.request(request.id).await.unwrap();
                assert_eq!(current.status, HelpRequestStatus::Matched);
                assert_eq!(current.matched_helper_id, Some(TelegramUserId(3)));
                let (pending,): (i64,) = sqlx::query_as(
                    "SELECT count(*) FROM dispatches WHERE request_id=? AND status='pending'",
                )
                .bind(request.id.0.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
                assert_eq!(pending, 0);
            }
            "cancelled" => {
                assert!(cancel.is_ok());
                assert!((2..=3).contains(&version));
                if version == 2 {
                    assert!(withdraw.is_err(), "cancel linearized before withdraw");
                } else {
                    assert!(withdraw.is_ok(), "withdraw linearized before cancel");
                }
                assert_eq!((latitude, longitude), (None, None));
            }
            _ => unreachable!(),
        }
    }
}

#[tokio::test]
async fn expiry_races_accept_without_resurrecting_an_expired_request() {
    for _ in 0..12 {
        let (repo, request) = setup().await;
        repo.fanout(request.id, &[TelegramUserId(2)]).await.unwrap();
        sqlx::query(
            "UPDATE help_requests SET created_at=unixepoch()-100, expires_at=unixepoch()-1 WHERE id=?",
        )
        .bind(request.id.0.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
        let path = repo.pool().connect_options().get_filename().to_owned();
        let other = SqliteRepositories::new(
            connect(&format!("sqlite://{}", path.display()))
                .await
                .unwrap(),
        );
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
        let wait = barrier.clone();
        let r1 = repo.clone();
        let expire = tokio::spawn(async move {
            wait.wait().await;
            r1.expire_due(Utc::now().timestamp()).await
        });
        let wait = barrier.clone();
        let accept = tokio::spawn(async move {
            wait.wait().await;
            other.accept(request.id, TelegramUserId(2), 0).await
        });
        let (expire, accept) = tokio::join!(expire, accept);
        let expire = expire.unwrap();
        let accept = accept.unwrap();
        if expire.is_err() {
            // The TTL worker retries on its next interval after a busy snapshot.
            repo.expire_due(Utc::now().timestamp()).await.unwrap();
        }
        assert!(!matches!(accept, Ok(AcceptOutcome::Won)));
        let current = repo.request(request.id).await.unwrap();
        assert_eq!(current.status, HelpRequestStatus::Expired);
        assert_eq!(current.exact_point, None);
        assert_eq!(current.matched_helper_id, None);
        let (pending, accepted, expired): (i64, i64, i64) = sqlx::query_as(
            "SELECT sum(status='pending'), sum(status='accepted'), sum(status='expired') FROM dispatches WHERE request_id=?",
        )
        .bind(request.id.0.to_string())
        .fetch_one(repo.pool())
        .await
        .unwrap();
        assert_eq!((pending, accepted, expired), (0, 0, 1));
    }
}
