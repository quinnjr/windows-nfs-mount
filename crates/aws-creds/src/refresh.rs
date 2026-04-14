use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::interval;

use crate::provider::AwsCredentialProvider;

/// Spawns a background task that refreshes credentials before they
/// expire. Checks every `check_interval` and refreshes when
/// credentials are within 10 minutes of expiry.
pub fn spawn_credential_refresh(
    provider: Arc<AwsCredentialProvider>,
    check_interval: Duration,
    mut shutdown: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut timer = interval(check_interval);
        timer.tick().await; // skip immediate first tick

        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(300);

        loop {
            tokio::select! {
                _ = timer.tick() => {
                    match provider.resolve().await {
                        Ok(creds) => {
                            backoff = Duration::from_secs(1);
                            if let Some(expiry) = creds.expiry {
                                let now = std::time::SystemTime::now();
                                let remaining = expiry.duration_since(now).unwrap_or_default();
                                if remaining < Duration::from_secs(600) {
                                    let _ = provider.refresh().await;
                                }
                            }
                        }
                        Err(_) => {
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(max_backoff);
                        }
                    }
                }
                _ = shutdown.changed() => {
                    break;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn refresh_task_runs_and_stops() {
        let provider = Arc::new(AwsCredentialProvider::from_static("AKID", "SECRET", None));
        let (tx, rx) = watch::channel(false);

        let handle = spawn_credential_refresh(
            provider.clone(),
            Duration::from_millis(100),
            rx,
        );

        tokio::time::sleep(Duration::from_millis(350)).await;

        let creds = provider.resolve().await.unwrap();
        assert_eq!(creds.access_key_id, "AKID");

        tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await.expect("should stop").unwrap();
    }

    #[tokio::test]
    async fn refresh_task_immediate_shutdown() {
        let provider = Arc::new(AwsCredentialProvider::from_static("AKID", "SECRET", None));
        let (tx, rx) = watch::channel(false);

        let handle = spawn_credential_refresh(provider, Duration::from_secs(60), rx);

        tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await.expect("should stop quickly").unwrap();
    }
}
