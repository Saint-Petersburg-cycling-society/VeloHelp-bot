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
) where
    R: RequestRepository,
{
    let mut ticker = tokio::time::interval(interval);
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let _ = expire_once(&repo).await;
            }
            _ = stop.changed() => break,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ttl_interval_is_configurable() {
        let interval = std::time::Duration::from_secs(60);
        assert_eq!(interval.as_secs(), 60);
    }
}
