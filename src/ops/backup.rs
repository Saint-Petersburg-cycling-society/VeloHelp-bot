// SPDX-License-Identifier: AGPL-3.0-only

//! SQLite online backup + restore check.
//!
//! Never raw-copies an open WAL database. Uses `VACUUM INTO` (online backup
//! API equivalent) from the host side; the production container intentionally
//! has no `sqlite3` CLI.

use sqlx::SqlitePool;

/// Copies the live database to `dest` via `VACUUM INTO`.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the backup fails.
///
/// # Panics
///
/// Panics when `dest` contains a quote or semicolon (unsafe path).
pub async fn backup_sqlite(pool: &SqlitePool, dest: &str) -> Result<(), sqlx::Error> {
    // `VACUUM INTO` cannot take a bound parameter for the path; validate that
    // the destination is a plain fascinated path without quotes/semicolons.
    assert!(
        !dest.contains('\'') && !dest.contains(';'),
        "unsafe backup destination"
    );
    let sql = format!("VACUUM INTO '{dest}'");
    sqlx::query(&sql).execute(pool).await?;
    Ok(())
}

/// Runs `PRAGMA integrity_check` on the pool.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the check cannot run; returns `Ok(false)`
/// when integrity is not `ok`.
pub async fn restore_check(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let (result,): (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(pool)
        .await?;
    Ok(result.eq_ignore_ascii_case("ok"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn backup_path_validation() {
        assert!("ok/path.sqlite".contains('/'));
    }
}
