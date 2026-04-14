use std::sync::Arc;
use std::time::Duration;
use aws_creds::{AwsCredentialProvider, spawn_credential_refresh};
use tokio::sync::watch;

#[tokio::test]
async fn full_credential_lifecycle() {
    let provider = Arc::new(AwsCredentialProvider::from_static(
        "AKIAIOSFODNN7EXAMPLE",
        "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        Some("session-token".to_string()),
    ));

    let (tx, rx) = watch::channel(false);
    let handle = spawn_credential_refresh(
        provider.clone(),
        Duration::from_millis(50),
        rx,
    );

    let creds = provider.resolve().await.unwrap();
    assert_eq!(creds.access_key_id, "AKIAIOSFODNN7EXAMPLE");
    assert_eq!(creds.session_token, Some("session-token".to_string()));

    let refreshed = provider.refresh().await.unwrap();
    assert_eq!(refreshed.access_key_id, "AKIAIOSFODNN7EXAMPLE");

    tx.send(true).unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn provider_without_refresh_task() {
    let provider = AwsCredentialProvider::from_static("A", "B", None);
    let creds = provider.resolve().await.unwrap();
    assert_eq!(creds.access_key_id, "A");
    assert_eq!(creds.secret_access_key, "B");
    assert!(creds.session_token.is_none());
    assert!(creds.expiry.is_none());
}
