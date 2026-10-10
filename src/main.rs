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

    let config = bike_aid::config::Config::from_env()?;

    let db = bike_aid::storage::connect(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&db).await?;

    // `Bot::new()` would use reqwest's platform verifier. The production image
    // is `FROM scratch`, so there is no platform CA store. Use compiled-in
    // Mozilla roots instead.
    let http = bike_aid::telegram::telegram_http_client()?;
    let _bot = Bot::with_client(config.teloxide_token.clone(), http);

    let repos = bike_aid::storage::SqliteRepositories::new(db);
    let metrics = std::sync::Arc::new(bike_aid::ops::Metrics::new());
    let _limiter = bike_aid::ops::RateLimiter::new();

    tracing::info!(
        source_url = %config.source_url,
        consent_version = %config.consent_version,
        ttl_secs = config.default_ttl.as_secs(),
        "bike-aid bootstrap started"
    );

    // Single-host MVP operations: TTL worker until shutdown. Telegram
    // long-polling dispatcher attaches here (Phase 2 surfaces are pure and
    // tested; transport loop is the next vertical slice).
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let worker_repos = repos.clone();
    let worker = tokio::spawn(async move {
        bike_aid::ops::ttl::run_ttl_worker(worker_repos, Duration::from_secs(60), stop_rx).await;
    });

    tokio::signal::ctrl_c().await?;
    let _ = stop_tx.send(true);
    worker.await?;
    tracing::info!(metrics = %metrics.snapshot(), "bike-aid stopped");
    Ok(())
}
