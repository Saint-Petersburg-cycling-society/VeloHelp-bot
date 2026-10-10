// SPDX-License-Identifier: AGPL-3.0-only

//! TTL worker: periodically flips `Open -> Expired` and expires pending
//! dispatches. Single-host MVP runs one worker; interval is configurable.

use std::time::Duration;

use crate::application::RequestRepository;

/// Runs one expiry pass.
///
/// # Errors
///
/// Returns [`crate::application::AppError`] when storage fails.
pub async fn expire_once<R>(repo: &R) -> Result<u64, crate::application::AppError>
where
    R: RequestRepository,
{
    repo.expire_due(chrono::Utc::now().timestamp()).await
}

/// Long-running TTL loop. Returns on cancellation via `stop` flag.
pub async fn run_ttl_worker<R>(
    repo: R,
    interval: Duration,
    mut stop: tokio::sync::watch::Receiver<bool>,
    metrics: std::sync::Arc<crate::ops::Metrics>,
) where
    R: RequestRepository,
{
    if *stop.borrow() {
        return;
    }
    let mut ticker = tokio::time::interval(interval);
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                match expire_once(&repo).await {
                    Ok(count) => { metrics.requests_expired.fetch_add(count, std::sync::atomic::Ordering::Relaxed); }
                    Err(err) => tracing::error!(error = %err, "request expiry failed"),
                }
            }
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    break;
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn worker_stops_when_shutdown_was_already_requested() {
        let pool = crate::storage::connect("sqlite::memory:").await.unwrap();
        let repo = crate::storage::SqliteRepositories::new(pool);
        let (_sender, stop) = tokio::sync::watch::channel(true);
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            super::run_ttl_worker(
                repo,
                std::time::Duration::from_secs(60),
                stop,
                std::sync::Arc::new(crate::ops::Metrics::new()),
            ),
        )
        .await
        .expect("already cancelled worker must stop");
    }
}
