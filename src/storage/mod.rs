// SPDX-License-Identifier: AGPL-3.0-only

use std::{str::FromStr, time::Duration};

use async_trait::async_trait;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};

use crate::{
    application::{AppError, DispatchRepository, HelperRepository, RequestRepository},
    domain::{
        AcceptOutcome, HelpRequest, HelpRequestId, HelpRequestStatus, HelperProfile, TelegramUserId,
    },
};

/// Connects to the SQLite database with the MVP contract (WAL, FULL synchronous,
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

/// SQLite-backed repositories. Holds the single-writer pool.
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
              version, expires_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)",
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
        .bind(i64::try_from(request.version).map_err(|e| AppError::Storage(e.to_string()))?)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;
        for tag in &request.problem_tags {
            sqlx::query(
                "INSERT OR IGNORE INTO request_problem_tags (request_id, tag) VALUES (?, ?)",
            )
            .bind(id.clone())
            .bind(tag.as_str())
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        }
        Ok(())
    }

    async fn publish(&self, request_id: HelpRequestId) -> Result<(), AppError> {
        let now = chrono::Utc::now().timestamp();
        let rows = sqlx::query(
            "UPDATE help_requests SET status='open', published_at=?, version=version+1
             WHERE id=? AND status='draft'",
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
    ) -> Result<AcceptOutcome, AppError> {
        let now = chrono::Utc::now().timestamp();
        let id = uuid_text(request_id);
        // Short write transaction: conditional request update + dispatch updates.
        // No Telegram/network I/O inside.
        let mut tx = self.pool.begin().await.map_err(AppError::from)?;
        let won = sqlx::query(
            "UPDATE help_requests
             SET status='matched', matched_helper_id=?, matched_at=?, version=version+1
             WHERE id=? AND status='open' AND expires_at > ?",
        )
        .bind(helper_id.0)
        .bind(now)
        .bind(id.clone())
        .bind(now)
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
        let row: Option<(String, i64)> =
            sqlx::query_as("SELECT status, expires_at FROM help_requests WHERE id=?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(AppError::from)?;
        tx.rollback().await.map_err(AppError::from)?;
        match row {
            None => Ok(AcceptOutcome::NotOpen),
            Some((status, expires_at)) => {
                if status == HelpRequestStatus::Matched.as_str() {
                    Ok(AcceptOutcome::AlreadyTaken)
                } else if status == HelpRequestStatus::Expired.as_str()
                    || (status == HelpRequestStatus::Open.as_str() && expires_at <= now)
                {
                    Ok(AcceptOutcome::Expired)
                } else {
                    Ok(AcceptOutcome::NotOpen)
                }
            }
        }
    }

    async fn cancel(
        &self,
        request_id: HelpRequestId,
        requester_id: TelegramUserId,
    ) -> Result<(), AppError> {
        let rows = sqlx::query(
            "UPDATE help_requests SET status='cancelled', version=version+1
             WHERE id=? AND requester_id=? AND status IN ('draft','open','matched')",
        )
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
            sqlx::query("UPDATE help_requests SET status='expired', version=version+1 WHERE id=?")
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
            "UPDATE help_requests SET status='resolved', resolved_at=?, version=version+1
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
             SET status='open', matched_helper_id=NULL, matched_at=NULL, version=version+1
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
             ON CONFLICT(telegram_user_id) DO UPDATE SET consent_version=excluded.consent_version",
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
        let id = uuid_text(request_id);
        for helper in helper_ids {
            sqlx::query(
                "INSERT OR IGNORE INTO dispatches (request_id, helper_id, status) VALUES (?, ?, 'pending')",
            )
            .bind(id.clone())
            .bind(helper.0)
            .execute(&self.pool)
            .await
            .map_err(AppError::from)?;
        }
        Ok(())
    }
}
