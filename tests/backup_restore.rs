// SPDX-License-Identifier: AGPL-3.0-only

//! Backup timer + restore test: online backup via `VACUUM INTO`, never
//! raw-copy of an open WAL database.

use bike_aid::{
    application::{HelperRepository, RequestRepository},
    domain::{HelpKind, HelpRequest, HelpRequestStatus, HelperProfile, TelegramUserId},
    ops::{backup_sqlite, restore_check},
    storage::connect,
};
use chrono::{TimeDelta, Utc};

#[tokio::test]
async fn backup_and_restore_roundtrip() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp");
    std::fs::create_dir_all(&dir).expect("test-tmp");
    let stamp = Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let src = dir.join(format!("backup-src-{}-{stamp}.sqlite", std::process::id()));
    let dst = dir.join(format!("backup-dst-{}-{stamp}.sqlite", std::process::id()));
    let src_url = format!("sqlite://{}?mode=rwc", src.display());

    let pool = connect(&src_url).await.expect("connect");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    let repo = bike_aid::storage::SqliteRepositories::new(pool.clone());

    let now = Utc::now().timestamp();
    let requester = TelegramUserId(9501);
    repo.ensure_user(requester, "2026-10-03", now)
        .await
        .unwrap();
    let mut req = HelpRequest::draft(
        requester,
        HelpKind::Technical,
        "backup me".to_owned(),
        Utc::now() + TimeDelta::minutes(60),
    );
    req.status = HelpRequestStatus::Open;
    repo.insert(&req).await.unwrap();
    repo.upsert_helper(&HelperProfile {
        user_id: TelegramUserId(9502),
        enabled: true,
        areas: vec![],
        help_kinds: vec![],
        problem_tags: vec![],
    })
    .await
    .unwrap_or(());

    backup_sqlite(&pool, &dst.display().to_string())
        .await
        .expect("backup");
    assert!(dst.exists(), "backup file created");

    let dst_url = format!("sqlite://{}?mode=rwc", dst.display());
    let restore_pool = connect(&dst_url).await.expect("connect restore");
    assert!(restore_check(&restore_pool).await.expect("integrity"));
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM help_requests")
        .fetch_one(&restore_pool)
        .await
        .expect("count");
    assert_eq!(count, 1);
}
