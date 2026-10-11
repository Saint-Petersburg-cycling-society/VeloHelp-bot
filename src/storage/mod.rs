// SPDX-License-Identifier: AGPL-3.0-only

use std::{str::FromStr, time::Duration};

mod read;
pub use read::{Area, PendingDelivery};

use async_trait::async_trait;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};

use crate::{
    application::{AppError, DispatchRepository, HelperRepository, RequestRepository},
    domain::{
        AcceptOutcome, ArrivalEstimate, HelpRequest, HelpRequestId, HelpRequestStatus,
        HelperProfile, TelegramUserId,
    },
};

/// Connects to the `SQLite` database with the MVP contract (WAL, FULL synchronous,
/// foreign keys, 5s busy timeout, single connection).
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the URL is invalid or the pool cannot be created.
pub async fn connect(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full)
        .busy_timeout(Duration::from_secs(5));

    // MVP deliberately serializes DB access through one connection.
    // Transactions are short and never contain Telegram/network I/O.
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
}

/// `SQLite`-backed repositories. Holds the single-writer pool.
#[derive(Clone, Debug)]
pub struct SqliteRepositories {
    pool: SqlitePool,
}

impl SqliteRepositories {
    /// Wraps an existing pool (created via [`connect`]).
    #[must_use]
    pub const fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Returns the inner pool for migrations/health checks.
    #[must_use]
    pub const fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

fn uuid_text(id: HelpRequestId) -> String {
    id.0.to_string()
}

fn unix(dt: chrono::DateTime<chrono::Utc>) -> i64 {
    dt.timestamp()
}

#[async_trait]
impl RequestRepository for SqliteRepositories {
    async fn insert(&self, request: &HelpRequest) -> Result<(), AppError> {
        if request.description.trim().is_empty()
            || request.description.chars().count() > 4000
            || request
                .bike_description
                .as_ref()
                .is_some_and(|s| s.chars().count() > 2000)
            || request.expires_at.timestamp() <= chrono::Utc::now().timestamp()
            || request
                .exact_point
                .is_some_and(|p| crate::domain::GeoPoint::new(p.lat, p.lon).is_none())
            || request.exact_address.as_ref().is_some_and(|address| {
                address.trim().is_empty()
                    || address.chars().count() > 200
                    || request.exact_point.is_none()
                    || matches!(
                        request.status,
                        HelpRequestStatus::Cancelled
                            | HelpRequestStatus::Resolved
                            | HelpRequestStatus::Expired
                    )
            })
        {
            return Err(AppError::InvalidInput(
                "invalid request fields or expiry".to_owned(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        let id = uuid_text(request.id);
        let status = request.status.as_str();
        let kind = request.help_kind.as_str();
        let expires_at = unix(request.expires_at);
        let bike = request.bike_description.as_deref();
        let (lat, lon) = request
            .exact_point
            .map_or((None, None), |p| (Some(p.lat), Some(p.lon)));
        sqlx::query(
            "INSERT INTO help_requests
             (id, requester_id, help_kind, description, bike_description,
              exact_lat, exact_lon, area_id, status, matched_helper_id,
              version, expires_at, exact_address)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.clone())
        .bind(request.requester_id.0)
        .bind(kind)
        .bind(request.description.as_str())
        .bind(bike)
        .bind(lat)
        .bind(lon)
        .bind(request.area_id.as_deref())
        .bind(status)
        .bind(request.matched_helper_id.map(|id| id.0))
        .bind(i64::try_from(request.version).map_err(|e| AppError::Storage(e.to_string()))?)
        .bind(expires_at)
        .bind(request.exact_address.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;
        if let Some(window) = request.arrival_window {
            if request.status != HelpRequestStatus::Matched || window.earliest > window.latest {
                return Err(AppError::InvalidInput("invalid arrival window".into()));
            }
            sqlx::query(
                "UPDATE help_requests SET arrival_from_at=?, arrival_until_at=? WHERE id=?",
            )
            .bind(window.earliest.timestamp())
            .bind(window.latest.timestamp())
            .bind(&id)
            .execute(&mut *tx)
            .await?;
        }
        for tag in &request.problem_tags {
            sqlx::query(
                "INSERT OR IGNORE INTO request_problem_tags (request_id, tag) VALUES (?, ?)",
            )
            .bind(id.clone())
            .bind(tag.as_str())
            .execute(&mut *tx)
            .await
            .map_err(AppError::from)?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn publish(&self, request_id: HelpRequestId) -> Result<(), AppError> {
        let now = chrono::Utc::now().timestamp();
        let rows = sqlx::query(
            "UPDATE help_requests SET status='open', published_at=?, version=version+1
             WHERE id=? AND status='draft' AND expires_at > unixepoch()",
        )
        .bind(now)
        .bind(uuid_text(request_id))
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?
        .rows_affected();
        if rows == 1 {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }

    async fn accept(
        &self,
        request_id: HelpRequestId,
        helper_id: TelegramUserId,
        request_version: u64,
    ) -> Result<AcceptOutcome, AppError> {
        self.accept_with_arrival(request_id, helper_id, request_version, None)
            .await
    }
    async fn accept_with_arrival(
        &self,
        request_id: HelpRequestId,
        helper_id: TelegramUserId,
        request_version: u64,
        estimate: Option<ArrivalEstimate>,
    ) -> Result<AcceptOutcome, AppError> {
        if estimate.is_some_and(|e| ArrivalEstimate::new(e.min_minutes, e.max_minutes).is_none()) {
            return Err(AppError::InvalidInput(
                "arrival must be within 1..=240 minutes".into(),
            ));
        }
        let now = chrono::Utc::now().timestamp();
        let id = uuid_text(request_id);
        // Short write transaction: conditional request update + dispatch updates.
        // No Telegram/network I/O inside.
        let mut tx = self.pool.begin().await.map_err(AppError::from)?;
        let won = sqlx::query(
            "UPDATE help_requests
             SET status='matched', matched_helper_id=?, matched_at=?, arrival_from_at=?, arrival_until_at=?, version=version+1
             WHERE id=? AND status='open' AND expires_at > unixepoch() AND version=?
               AND requester_id<>?
               AND EXISTS (
                   SELECT 1 FROM dispatches d JOIN helper_profiles h ON h.user_id=d.helper_id
                   WHERE d.request_id=help_requests.id AND d.helper_id=?
                     AND d.status='pending' AND d.request_version=help_requests.version
                     AND h.enabled=1
               )",
        )
        .bind(helper_id.0)
        .bind(now)
        .bind(estimate.map(|e| now + i64::from(e.min_minutes) * 60))
        .bind(estimate.map(|e| now + i64::from(e.max_minutes) * 60))
        .bind(id.clone())
        .bind(i64::try_from(request_version).map_err(|e| AppError::InvalidInput(e.to_string()))?)
        .bind(helper_id.0)
        .bind(helper_id.0)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?
        .rows_affected()
            == 1;
        if won {
            sqlx::query(
                "UPDATE dispatches SET status='accepted', acted_at=?
                 WHERE request_id=? AND helper_id=? AND status='pending'",
            )
            .bind(now)
            .bind(id.clone())
            .bind(helper_id.0)
            .execute(&mut *tx)
            .await
            .map_err(AppError::from)?;
            sqlx::query(
                "UPDATE dispatches SET status='superseded'
                 WHERE request_id=? AND helper_id<>? AND status='pending'",
            )
            .bind(id.clone())
            .bind(helper_id.0)
            .execute(&mut *tx)
            .await
            .map_err(AppError::from)?;
            tx.commit().await.map_err(AppError::from)?;
            return Ok(AcceptOutcome::Won);
        }
        // Loser path: read authoritative status (read-only, after failed write).
        let row: Option<(String, i64, i64)> =
            sqlx::query_as("SELECT status, expires_at, version FROM help_requests WHERE id=?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(AppError::from)?;
        tx.rollback().await.map_err(AppError::from)?;
        match row {
            None => Ok(AcceptOutcome::NotOpen),
            Some((status, expires_at, version)) => {
                if status == HelpRequestStatus::Matched.as_str() {
                    Ok(AcceptOutcome::AlreadyTaken)
                } else if status == HelpRequestStatus::Expired.as_str()
                    || (status == HelpRequestStatus::Open.as_str() && expires_at <= now)
                {
                    Ok(AcceptOutcome::Expired)
                } else if status != HelpRequestStatus::Open.as_str() {
                    Ok(AcceptOutcome::NotOpen)
                } else if u64::try_from(version).ok() != Some(request_version) {
                    Ok(AcceptOutcome::Stale)
                } else {
                    Ok(AcceptOutcome::Forbidden)
                }
            }
        }
    }

    async fn cancel(
        &self,
        request_id: HelpRequestId,
        requester_id: TelegramUserId,
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await?;
        let id = uuid_text(request_id);
        let rows = sqlx::query(
            "UPDATE help_requests SET status='cancelled', matched_helper_id=NULL, matched_at=NULL, exact_lat=NULL, exact_lon=NULL, arrival_from_at=NULL, arrival_until_at=NULL, version=version+1
             WHERE id=? AND requester_id=? AND status IN ('draft','open','matched')",
        )
        .bind(&id)
        .bind(requester_id.0)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if rows != 1 {
            return Err(AppError::NotFound);
        }
        sqlx::query("UPDATE dispatches SET status='superseded' WHERE request_id=? AND status IN ('pending','accepted')")
            .bind(&id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn expire_due(&self, now_unix: i64) -> Result<u64, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::from)?;
        let expired: Vec<String> =
            sqlx::query_as("SELECT id FROM help_requests WHERE status='open' AND expires_at <= ?")
                .bind(now_unix)
                .fetch_all(&mut *tx)
                .await
                .map_err(AppError::from)?
                .into_iter()
                .map(|row: (String,)| row.0)
                .collect();
        for id in &expired {
            sqlx::query("UPDATE help_requests SET status='expired', exact_lat=NULL, exact_lon=NULL, arrival_from_at=NULL, arrival_until_at=NULL, version=version+1 WHERE id=?")
                .bind(id.clone())
                .execute(&mut *tx)
                .await
                .map_err(AppError::from)?;
            sqlx::query(
                "UPDATE dispatches SET status='expired'
                 WHERE request_id=? AND status='pending'",
            )
            .bind(id.clone())
            .execute(&mut *tx)
            .await
            .map_err(AppError::from)?;
        }
        tx.commit().await.map_err(AppError::from)?;
        Ok(u64::try_from(expired.len()).map_err(|e| AppError::Storage(e.to_string()))?)
    }

    async fn resolve(
        &self,
        request_id: HelpRequestId,
        requester_id: TelegramUserId,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().timestamp();
        let rows = sqlx::query(
            "UPDATE help_requests SET status='resolved', resolved_at=?, exact_lat=NULL, exact_lon=NULL, arrival_from_at=NULL, arrival_until_at=NULL, version=version+1
             WHERE id=? AND requester_id=? AND status='matched'",
        )
        .bind(now)
        .bind(uuid_text(request_id))
        .bind(requester_id.0)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?
        .rows_affected();
        if rows == 1 {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }

    async fn withdraw(
        &self,
        request_id: HelpRequestId,
        helper_id: TelegramUserId,
    ) -> Result<(), AppError> {
        // New version / new dispatch wave; old pending capabilities stay dead.
        let mut tx = self.pool.begin().await.map_err(AppError::from)?;
        let rows = sqlx::query(
            "UPDATE help_requests
             SET status=CASE WHEN expires_at > unixepoch() THEN 'open' ELSE 'expired' END,
                 exact_lat=CASE WHEN expires_at > unixepoch() THEN exact_lat END,
                 exact_lon=CASE WHEN expires_at > unixepoch() THEN exact_lon END,
                 matched_helper_id=NULL, matched_at=NULL, arrival_from_at=NULL, arrival_until_at=NULL, version=version+1
             WHERE id=? AND matched_helper_id=? AND status='matched'",
        )
        .bind(uuid_text(request_id))
        .bind(helper_id.0)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?
        .rows_affected();
        if rows != 1 {
            tx.rollback().await.map_err(AppError::from)?;
            return Err(AppError::NotFound);
        }
        let id = uuid_text(request_id);
        sqlx::query(
            "UPDATE dispatches SET status='superseded'
             WHERE request_id=? AND status IN ('pending','accepted')",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;
        tx.commit().await.map_err(AppError::from)?;
        Ok(())
    }
}

#[async_trait]
impl HelperRepository for SqliteRepositories {
    async fn ensure_user(
        &self,
        user_id: TelegramUserId,
        consent_version: &str,
        now_unix: i64,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO users (telegram_user_id, consent_version, consented_at)
             VALUES (?, ?, ?)
             ON CONFLICT(telegram_user_id) DO UPDATE SET consent_version=excluded.consent_version, consented_at=excluded.consented_at",
        )
        .bind(user_id.0)
        .bind(consent_version)
        .bind(now_unix)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        Ok(())
    }

    async fn upsert_helper(&self, profile: &HelperProfile) -> Result<(), AppError> {
        if profile.problem_tags.iter().any(|tag| {
            crate::domain::problem_kind(tag).is_none_or(|kind| !profile.help_kinds.contains(&kind))
        }) {
            return Err(AppError::InvalidInput(
                "invalid helper problem preferences".into(),
            ));
        }
        let now = chrono::Utc::now().timestamp();
        let mut tx = self.pool.begin().await.map_err(AppError::from)?;
        sqlx::query(
            "INSERT INTO helper_profiles (user_id, enabled, updated_at)
             VALUES (?, ?, ?)
             ON CONFLICT(user_id) DO UPDATE SET enabled=excluded.enabled, updated_at=?",
        )
        .bind(profile.user_id.0)
        .bind(i64::from(profile.enabled))
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;
        sqlx::query("DELETE FROM helper_areas WHERE user_id=?")
            .bind(profile.user_id.0)
            .execute(&mut *tx)
            .await
            .map_err(AppError::from)?;
        for area in &profile.areas {
            sqlx::query(
                "INSERT OR IGNORE INTO areas (id, kind, display_name) VALUES (?, 'district', ?)",
            )
            .bind(area.clone())
            .bind(area.clone())
            .execute(&mut *tx)
            .await
            .map_err(AppError::from)?;
            sqlx::query("INSERT INTO helper_areas (user_id, area_id) VALUES (?, ?)")
                .bind(profile.user_id.0)
                .bind(area.clone())
                .execute(&mut *tx)
                .await
                .map_err(AppError::from)?;
        }
        sqlx::query("DELETE FROM helper_help_kinds WHERE user_id=?")
            .bind(profile.user_id.0)
            .execute(&mut *tx)
            .await
            .map_err(AppError::from)?;
        for kind in &profile.help_kinds {
            sqlx::query("INSERT INTO helper_help_kinds (user_id, help_kind) VALUES (?, ?)")
                .bind(profile.user_id.0)
                .bind(kind.as_str())
                .execute(&mut *tx)
                .await
                .map_err(AppError::from)?;
        }
        sqlx::query("DELETE FROM helper_problem_tags WHERE user_id=?")
            .bind(profile.user_id.0)
            .execute(&mut *tx)
            .await?;
        for tag in &profile.problem_tags {
            sqlx::query("INSERT INTO helper_problem_tags (user_id, tag) VALUES (?, ?)")
                .bind(profile.user_id.0)
                .bind(tag)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await.map_err(AppError::from)?;
        Ok(())
    }
}

#[async_trait]
impl DispatchRepository for SqliteRepositories {
    async fn fanout(
        &self,
        request_id: HelpRequestId,
        helper_ids: &[TelegramUserId],
    ) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await?;
        let id = uuid_text(request_id);
        for helper in helper_ids {
            sqlx::query(
                "INSERT INTO dispatches (request_id, helper_id, status, request_version)
                 SELECT r.id, h.user_id, 'pending', r.version
                 FROM help_requests r JOIN helper_profiles h ON h.user_id=?
                 WHERE r.id=? AND r.status='open' AND r.expires_at>unixepoch()
                   AND h.enabled=1 AND h.user_id<>r.requester_id
                   AND EXISTS (SELECT 1 FROM helper_areas a WHERE a.user_id=h.user_id AND a.area_id=r.area_id)
                   AND EXISTS (SELECT 1 FROM helper_help_kinds k WHERE k.user_id=h.user_id AND k.help_kind=r.help_kind) AND (NOT EXISTS (SELECT 1 FROM helper_problem_tags hp WHERE hp.user_id=h.user_id) OR NOT EXISTS (SELECT 1 FROM request_problem_tags rp WHERE rp.request_id=r.id) OR EXISTS (SELECT 1 FROM request_problem_tags rp JOIN helper_problem_tags hp ON hp.tag=rp.tag WHERE rp.request_id=r.id AND hp.user_id=h.user_id))
                 ON CONFLICT(request_id, helper_id) DO UPDATE
                 SET status='pending', request_version=excluded.request_version,
                     view_id=NULL, chat_id=NULL, message_id=NULL, ephemeral_message_id=NULL,
                     sent_at=NULL, acted_at=NULL
                 WHERE dispatches.request_version < excluded.request_version",
            )
            .bind(helper.0)
            .bind(&id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
