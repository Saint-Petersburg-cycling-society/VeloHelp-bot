// SPDX-License-Identifier: AGPL-3.0-only

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

    tracing::info!(
        source_url = %config.source_url,
        consent_version = %config.consent_version,
        ttl_secs = config.default_ttl.as_secs(),
        "bike-aid bootstrap started"
    );

    // TODO:
    // construct repositories/application service/teloxide-ui runtime/dispatcher.
    Ok(())
}
