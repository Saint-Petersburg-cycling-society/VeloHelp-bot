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
        "UPDATE help_requests SET exact_lat=NULL, exact_lon=NULL, arrival_from_at=NULL, arrival_until_at=NULL
         WHERE status IN ('resolved','cancelled','expired')
           AND (exact_lat IS NOT NULL OR exact_lon IS NOT NULL OR arrival_from_at IS NOT NULL OR arrival_until_at IS NOT NULL)",
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
    use sqlx::Row;
    async fn rows_json(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        query: &str,
        user_id: i64,
    ) -> Result<Vec<serde_json::Value>, AppError> {
        use sqlx::{Column, TypeInfo, ValueRef};
        let rows = sqlx::query(query)
            .bind(user_id)
            .fetch_all(&mut **tx)
            .await?;
        rows.iter()
            .map(|row| {
                let mut object = serde_json::Map::new();
                for column in row.columns() {
                    let name = column.name();
                    let raw = row.try_get_raw(name)?;
                    let value = if raw.is_null() {
                        serde_json::Value::Null
                    } else {
                        match raw.type_info().name() {
                            "INTEGER" => serde_json::json!(row.try_get::<i64, _>(name)?),
                            "REAL" => serde_json::json!(row.try_get::<f64, _>(name)?),
                            _ => serde_json::json!(row.try_get::<String, _>(name)?),
                        }
                    };
                    object.insert(name.to_owned(), value);
                }
                Ok(serde_json::Value::Object(object))
            })
            .collect()
    }
    // One consistent snapshot; only rows owned by the requesting user.
    let mut tx = pool.begin().await?;
    let mut object = serde_json::Map::new();
    for (key, query) in [
        ("user", "SELECT * FROM users WHERE telegram_user_id=?"),
        ("helper_profile", "SELECT * FROM helper_profiles WHERE user_id=?"),
        ("areas", "SELECT * FROM helper_areas WHERE user_id=?"),
        ("help_kinds", "SELECT * FROM helper_help_kinds WHERE user_id=?"),
        ("availability", "SELECT * FROM availability_windows WHERE user_id=?"),
        ("requests", "SELECT * FROM help_requests WHERE requester_id=?"),
        ("problem_tags", "SELECT t.* FROM request_problem_tags t JOIN help_requests r ON r.id=t.request_id WHERE r.requester_id=?"),
        ("dispatches", "SELECT * FROM dispatches WHERE helper_id=?"),
        ("arrival_estimates", "SELECT id AS request_id, arrival_from_at, arrival_until_at FROM help_requests WHERE matched_helper_id=? AND arrival_from_at IS NOT NULL"),
    ] {
        object.insert(key.to_owned(), serde_json::json!(rows_json(&mut tx, query, user_id.0).await?));
    }
    tx.commit().await?;
    serde_json::to_string_pretty(&object).map_err(|err| AppError::Storage(err.to_string()))
}

/// Deletes a user and all cascaded rows (withdrawal/deletion request).
///
/// # Errors
///
/// Returns [`AppError`] when storage fails.
pub async fn delete_user(pool: &SqlitePool, user_id: TelegramUserId) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    // Close matches involving the deleting helper before releasing the FK.
    sqlx::query(
        "UPDATE dispatches SET status='superseded' WHERE status IN ('pending','accepted')
         AND request_id IN (SELECT id FROM help_requests WHERE matched_helper_id=?)",
    )
    .bind(user_id.0)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE help_requests SET
         status=CASE WHEN status='matched' THEN 'cancelled' ELSE status END,
         matched_helper_id=NULL, exact_lat=NULL, exact_lon=NULL, arrival_from_at=NULL, arrival_until_at=NULL, version=version+1
         WHERE matched_helper_id=?",
    )
    .bind(user_id.0)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM help_requests WHERE requester_id=?")
        .bind(user_id.0)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM users WHERE telegram_user_id=?")
        .bind(user_id.0)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
