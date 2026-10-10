// SPDX-License-Identifier: AGPL-3.0-only

//! Deletion/export and retention helpers.
//!
//! Every new persisted personal-data field requires purpose + retention
//! (AGENTS §12). Exact points are purpose-limited: after close they MUST be
//! scorched. Full user deletion removes identity, settings and requests.

use sqlx::SqlitePool;

use crate::{application::AppError, domain::TelegramUserId};

/// Clears exact coordinates of closed requests (retention scorch).
///
/// # Errors
///
/// Returns [`AppError`] when storage fails.
pub async fn scorch_exact_points(pool: &SqlitePool) -> Result<u64, AppError> {
    let rows = sqlx::query(
        "UPDATE help_requests SET exact_lat=NULL, exact_lon=NULL
         WHERE status IN ('resolved','cancelled','expired')
           AND (exact_lat IS NOT NULL OR exact_lon IS NOT NULL)",
    )
    .execute(pool)
    .await
    .map_err(AppError::from)?
    .rows_affected();
    Ok(rows)
}

/// Exports a user's data as JSON (access request).
///
/// # Errors
///
/// Returns [`AppError`] when storage fails.
pub async fn export_user_json(
    pool: &SqlitePool,
    user_id: TelegramUserId,
) -> Result<String, AppError> {
    let requests: Vec<(String, String, String)> =
        sqlx::query_as("SELECT id, status, description FROM help_requests WHERE requester_id=?")
            .bind(user_id.0)
            .fetch_all(pool)
            .await
            .map_err(AppError::from)?;
    let body = requests
        .iter()
        .map(|(id, status, desc)| format!("{id}:{status}:{desc}"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(format!("user:{}\n{body}", user_id.0))
}

/// Deletes a user and all cascaded rows (withdrawal/deletion request).
///
/// # Errors
///
/// Returns [`AppError`] when storage fails.
pub async fn delete_user(pool: &SqlitePool, user_id: TelegramUserId) -> Result<(), AppError> {
    sqlx::query("DELETE FROM users WHERE telegram_user_id=?")
        .bind(user_id.0)
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}
