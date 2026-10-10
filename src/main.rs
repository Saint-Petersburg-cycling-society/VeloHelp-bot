// SPDX-License-Identifier: AGPL-3.0-only

use std::time::Duration;

use teloxide::Bot;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,bike_aid=debug".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(
        args.is_empty() || args == ["--check"],
        "usage: bike-aid [--check]"
    );
    let preflight = args == ["--check"];
    let config = if preflight {
        bike_aid::config::Config::for_preflight()?
    } else {
        bike_aid::config::Config::from_env()?
    };

    let db = bike_aid::storage::connect(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    bike_aid::ops::privacy::scorch_exact_points(&db).await?;
    if preflight {
        bike_aid::domain::DistrictMap::bundled()?;
        bike_aid::domain::AddressMap::bundled()?;
        anyhow::ensure!(
            bike_aid::ops::restore_check(&db).await?,
            "SQLite integrity check failed"
        );
        let (journal,): (String,) = sqlx::query_as("PRAGMA journal_mode").fetch_one(&db).await?;
        let (foreign_keys,): (i64,) = sqlx::query_as("PRAGMA foreign_keys").fetch_one(&db).await?;
        let (synchronous,): (i64,) = sqlx::query_as("PRAGMA synchronous").fetch_one(&db).await?;
        let (timeout,): (i64,) = sqlx::query_as("PRAGMA busy_timeout").fetch_one(&db).await?;
        anyhow::ensure!(
            journal == "wal" && foreign_keys == 1 && synchronous == 2 && timeout >= 5000,
            "SQLite runtime contract failed; use a local file database"
        );
        tracing::info!(
            "offline preflight passed: config, district/address data, migrations, SQLite integrity, WAL/FULL/FK/timeout"
        );
        return Ok(());
    }

    // `Bot::new()` would use reqwest's platform verifier. The production image
    // is `FROM scratch`, so there is no platform CA store. Use compiled-in
    // Mozilla roots instead.
    let http = bike_aid::telegram::telegram_http_client()?;
    let bot = Bot::with_client(config.teloxide_token.clone(), http);

    let repos = bike_aid::storage::SqliteRepositories::new(db);
    let metrics = std::sync::Arc::new(bike_aid::ops::Metrics::new());

    tracing::info!(
        source_url = %config.source_url,
        consent_version = %config.consent_version,
        ttl_secs = config.default_ttl.as_secs(),
        "bike-aid polling runtime started"
    );

    // TTL and polling operate independently; no network calls under DB locks.
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let worker_repos = repos.clone();
    let worker_metrics = metrics.clone();
    let worker = tokio::spawn(async move {
        bike_aid::ops::ttl::run_ttl_worker(
            worker_repos,
            Duration::from_secs(60),
            stop_rx,
            worker_metrics,
        )
        .await;
    });

    let result = tokio::select! {
        result = bike_aid::telegram::run_polling(bot, repos, config, metrics.clone()) => result,
        result = shutdown_signal() => result,
    };
    let _ = stop_tx.send(true);
    worker.await?;
    tracing::info!(metrics = %metrics.snapshot(), "bike-aid stopped");
    result
}

async fn shutdown_signal() -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! { result = tokio::signal::ctrl_c() => result?, _ = term.recv() => {} }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    Ok(())
}
