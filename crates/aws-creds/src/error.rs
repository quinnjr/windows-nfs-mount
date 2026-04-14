use thiserror::Error;

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("no credentials available: {0}")]
    NoCredentials(String),

    #[error("credential refresh failed: {0}")]
    RefreshFailed(String),

    #[error("AWS SDK error: {0}")]
    AwsSdk(String),
}
