// SPDX-License-Identifier: AGPL-3.0-only

use anyhow::Context;
use teloxide::Bot;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,bike_aid=debug".into()),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let token = std::env::var("TELOXIDE_TOKEN").context("TELOXIDE_TOKEN is required")?;

    let db = bike_aid::storage::connect(&database_url).await?;
    sqlx::migrate!("./migrations").run(&db).await?;

    // `Bot::new()` would use reqwest's platform verifier. The production image
    // is `FROM scratch`, so there is no platform CA store. Use compiled-in
    // Mozilla roots instead.
    let http = bike_aid::telegram::telegram_http_client()?;
    let _bot = Bot::with_client(token, http);

    tracing::info!("bike-aid bootstrap started");

    // TODO:
    // construct repositories/application service/teloxide-ui runtime/dispatcher.
    Ok(())
}
