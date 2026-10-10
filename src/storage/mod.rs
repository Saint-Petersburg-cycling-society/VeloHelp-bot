// SPDX-License-Identifier: AGPL-3.0-only

use std::{str::FromStr, time::Duration};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};

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
