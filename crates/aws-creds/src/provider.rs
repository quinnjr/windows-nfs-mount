use std::sync::Arc;
use tokio::sync::RwLock;
use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_credential_types::provider::ProvideCredentials;

use crate::error::CredentialError;

/// Resolved AWS credentials with optional expiry.
#[derive(Debug, Clone)]
pub struct ResolvedCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
    pub expiry: Option<std::time::SystemTime>,
}

impl From<Credentials> for ResolvedCredentials {
    fn from(creds: Credentials) -> Self {
        Self {
            access_key_id: creds.access_key_id().to_string(),
            secret_access_key: creds.secret_access_key().to_string(),
            session_token: creds.session_token().map(|s| s.to_string()),
            expiry: creds.expiry(),
        }
    }
}

/// AWS credential provider wrapping the SDK default chain.
pub struct AwsCredentialProvider {
    provider: Arc<dyn ProvideCredentials>,
    cached: RwLock<Option<ResolvedCredentials>>,
    profile: Option<String>,
}

impl AwsCredentialProvider {
    /// Create a new provider using the default credential chain.
    pub async fn new(profile: Option<String>) -> Result<Self, CredentialError> {
        let mut loader = aws_config::defaults(BehaviorVersion::latest());
        if let Some(ref p) = profile {
            loader = loader.profile_name(p);
        }
        let config = loader.load().await;
        let provider = config.credentials_provider()
            .ok_or_else(|| CredentialError::NoCredentials(
                "no credential provider configured".into()
            ))?
            .clone();

        Ok(Self {
            provider: Arc::from(provider),
            cached: RwLock::new(None),
            profile,
        })
    }

    /// Create a provider with explicit credentials (for testing).
    pub fn from_static(
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
        session_token: Option<String>,
    ) -> Self {
        let creds = Credentials::new(
            access_key_id,
            secret_access_key,
            session_token,
            None,
            "static",
        );
        let provider = aws_credential_types::provider::SharedCredentialsProvider::new(creds);
        Self {
            provider: Arc::from(provider),
            cached: RwLock::new(None),
            profile: None,
        }
    }

    /// Resolve current credentials. Returns cached if still valid.
    pub async fn resolve(&self) -> Result<ResolvedCredentials, CredentialError> {
        {
            let cached = self.cached.read().await;
            if let Some(ref creds) = *cached {
                if !Self::is_expired(creds) {
                    return Ok(creds.clone());
                }
            }
        }
        self.refresh().await
    }

    /// Force a credential refresh from the provider chain.
    pub async fn refresh(&self) -> Result<ResolvedCredentials, CredentialError> {
        let creds = self.provider
            .provide_credentials()
            .await
            .map_err(|e| CredentialError::RefreshFailed(e.to_string()))?;

        let resolved = ResolvedCredentials::from(creds);
        let mut cached = self.cached.write().await;
        *cached = Some(resolved.clone());
        Ok(resolved)
    }

    /// Check if credentials are expired or within 5 minutes of expiry.
    fn is_expired(creds: &ResolvedCredentials) -> bool {
        match creds.expiry {
            None => false,
            Some(expiry) => {
                let buffer = std::time::Duration::from_secs(300);
                let now = std::time::SystemTime::now();
                expiry.duration_since(now).unwrap_or_default() < buffer
            }
        }
    }

    /// Get the profile name (if set).
    pub fn profile(&self) -> Option<&str> {
        self.profile.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn static_credentials_resolve() {
        let provider = AwsCredentialProvider::from_static(
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            Some("token123".to_string()),
        );
        let creds = provider.resolve().await.unwrap();
        assert_eq!(creds.access_key_id, "AKIAIOSFODNN7EXAMPLE");
        assert_eq!(creds.secret_access_key, "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
        assert_eq!(creds.session_token, Some("token123".to_string()));
        assert!(creds.expiry.is_none());
    }

    #[tokio::test]
    async fn static_credentials_cached() {
        let provider = AwsCredentialProvider::from_static("AKID", "SECRET", None);
        let c1 = provider.resolve().await.unwrap();
        let c2 = provider.resolve().await.unwrap();
        assert_eq!(c1.access_key_id, c2.access_key_id);
    }

    #[tokio::test]
    async fn force_refresh() {
        let provider = AwsCredentialProvider::from_static("AKID", "SECRET", None);
        let creds = provider.refresh().await.unwrap();
        assert_eq!(creds.access_key_id, "AKID");
    }

    #[test]
    fn not_expired_without_expiry() {
        let creds = ResolvedCredentials {
            access_key_id: "AKID".into(), secret_access_key: "S".into(),
            session_token: None, expiry: None,
        };
        assert!(!AwsCredentialProvider::is_expired(&creds));
    }

    #[test]
    fn expired_when_past() {
        let creds = ResolvedCredentials {
            access_key_id: "AKID".into(), secret_access_key: "S".into(),
            session_token: None, expiry: Some(std::time::SystemTime::UNIX_EPOCH),
        };
        assert!(AwsCredentialProvider::is_expired(&creds));
    }

    #[test]
    fn expired_within_buffer() {
        let creds = ResolvedCredentials {
            access_key_id: "AKID".into(), secret_access_key: "S".into(),
            session_token: None,
            expiry: Some(std::time::SystemTime::now() + std::time::Duration::from_secs(60)),
        };
        assert!(AwsCredentialProvider::is_expired(&creds));
    }

    #[test]
    fn not_expired_when_far_future() {
        let creds = ResolvedCredentials {
            access_key_id: "AKID".into(), secret_access_key: "S".into(),
            session_token: None,
            expiry: Some(std::time::SystemTime::now() + std::time::Duration::from_secs(3600)),
        };
        assert!(!AwsCredentialProvider::is_expired(&creds));
    }
}
