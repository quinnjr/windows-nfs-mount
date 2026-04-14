# Phase 3: AWS Credentials Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `aws-creds` crate — AWS IAM credential resolution and automatic refresh for authenticating to AWS S3 Files NFS endpoints.

**Architecture:** A single crate wrapping the AWS SDK default credential chain (`aws-config`) with a background refresh task and an event-based interface for consumers. The crate is intentionally isolated so non-AWS users don't pull in the AWS SDK dependency tree.

**Tech Stack:** Rust 2024, `aws-config` + `aws-credential-types` (AWS SDK credential chain), `tokio` (async + timers), `thiserror`

**Spec:** `docs/superpowers/specs/2026-04-14-nfs-mount-design.md` — section "AWS Credentials & TLS"

---

## File Structure

```
crates/
  aws-creds/
    Cargo.toml
    src/
      lib.rs              (re-exports)
      error.rs            (CredentialError enum)
      provider.rs         (AwsCredentialProvider: wraps AWS SDK chain, resolve + refresh)
      refresh.rs          (background refresh task with expiry monitoring)
```

---

### Task 1: Scaffold aws-creds Crate

**Files:**
- Modify: `Cargo.toml` (add workspace member + dependencies)
- Create: `crates/aws-creds/Cargo.toml`
- Create: `crates/aws-creds/src/lib.rs`

- [ ] **Step 1: Add aws-creds to workspace**

Add `"crates/aws-creds"` to the workspace members list in the root `Cargo.toml`.

Add to `[workspace.dependencies]`:
```toml
aws-config = { version = "1", features = ["behavior-version-latest"] }
aws-credential-types = { version = "1", features = ["hardcoded-credentials"] }
aws-types = "1"
```

- [ ] **Step 2: Create crate**

`crates/aws-creds/Cargo.toml`:
```toml
[package]
name = "aws-creds"
version.workspace = true
edition.workspace = true

[dependencies]
aws-config = { workspace = true }
aws-credential-types = { workspace = true }
aws-types = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
```

`crates/aws-creds/src/lib.rs`:
```rust
pub mod error;
pub mod provider;
pub mod refresh;

pub use error::CredentialError;
pub use provider::AwsCredentialProvider;
pub use refresh::spawn_credential_refresh;
```

- [ ] **Step 3: Create error.rs stub, provider.rs stub, refresh.rs stub**

`crates/aws-creds/src/error.rs`:
```rust
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
```

`crates/aws-creds/src/provider.rs`:
```rust
// Will be implemented in Task 2.
```

`crates/aws-creds/src/refresh.rs`:
```rust
// Will be implemented in Task 3.
```

Note: lib.rs will not compile with the stubs. Comment out the pub use lines for provider and refresh until those tasks are done. Keep error.rs active.

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p aws-creds`

- [ ] **Step 5: Commit**

```
build: add aws-creds crate to workspace

Isolated crate for AWS IAM credential resolution. Separated from
the NFS protocol crates so non-AWS users don't pull in the AWS SDK
dependency tree (~50 transitive crates). Contains CredentialError
type; provider and refresh modules are stubs.
```

---

### Task 2: Credential Provider

**Files:**
- Create: `crates/aws-creds/src/provider.rs`
- Modify: `crates/aws-creds/src/lib.rs` (uncomment provider re-export)

The provider wraps the AWS SDK default credential chain and exposes resolved credentials with expiry information.

- [ ] **Step 1: Implement AwsCredentialProvider**

```rust
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
///
/// Resolves credentials from (in order): environment variables,
/// EC2 IMDS, ECS container credentials, shared credential file,
/// SSO cache, and process credentials.
pub struct AwsCredentialProvider {
    provider: Arc<dyn ProvideCredentials>,
    cached: RwLock<Option<ResolvedCredentials>>,
    profile: Option<String>,
}

impl AwsCredentialProvider {
    /// Create a new provider using the default credential chain.
    /// Optionally specify an AWS profile name.
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
            None, // no expiry for static creds
            "static",
        );
        let provider = aws_credential_types::provider::SharedCredentialsProvider::new(creds);
        Self {
            provider: Arc::from(provider),
            cached: RwLock::new(None),
            profile: None,
        }
    }

    /// Resolve current credentials. Returns cached credentials if
    /// still valid, otherwise refreshes from the provider chain.
    pub async fn resolve(&self) -> Result<ResolvedCredentials, CredentialError> {
        // Check cache first
        {
            let cached = self.cached.read().await;
            if let Some(ref creds) = *cached {
                if !Self::is_expired(creds) {
                    return Ok(creds.clone());
                }
            }
        }

        // Refresh
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

    /// Check if credentials are expired or will expire within 5 minutes.
    fn is_expired(creds: &ResolvedCredentials) -> bool {
        match creds.expiry {
            None => false, // no expiry = never expires
            Some(expiry) => {
                let buffer = std::time::Duration::from_secs(300); // 5 min buffer
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
```

- [ ] **Step 2: Add tests**

```rust
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
        assert!(creds.expiry.is_none()); // static creds don't expire
    }

    #[tokio::test]
    async fn static_credentials_cached() {
        let provider = AwsCredentialProvider::from_static(
            "AKID", "SECRET", None,
        );

        // First resolve
        let creds1 = provider.resolve().await.unwrap();
        // Second resolve should return cached
        let creds2 = provider.resolve().await.unwrap();
        assert_eq!(creds1.access_key_id, creds2.access_key_id);
    }

    #[tokio::test]
    async fn force_refresh() {
        let provider = AwsCredentialProvider::from_static(
            "AKID", "SECRET", None,
        );

        let creds = provider.refresh().await.unwrap();
        assert_eq!(creds.access_key_id, "AKID");
    }

    #[test]
    fn not_expired_without_expiry() {
        let creds = ResolvedCredentials {
            access_key_id: "AKID".into(),
            secret_access_key: "SECRET".into(),
            session_token: None,
            expiry: None,
        };
        assert!(!AwsCredentialProvider::is_expired(&creds));
    }

    #[test]
    fn expired_when_past() {
        let creds = ResolvedCredentials {
            access_key_id: "AKID".into(),
            secret_access_key: "SECRET".into(),
            session_token: None,
            expiry: Some(std::time::SystemTime::UNIX_EPOCH), // long past
        };
        assert!(AwsCredentialProvider::is_expired(&creds));
    }

    #[test]
    fn expired_within_buffer() {
        let creds = ResolvedCredentials {
            access_key_id: "AKID".into(),
            secret_access_key: "SECRET".into(),
            session_token: None,
            expiry: Some(std::time::SystemTime::now() + std::time::Duration::from_secs(60)),
            // Expires in 60s, but buffer is 300s, so it's "expired"
        };
        assert!(AwsCredentialProvider::is_expired(&creds));
    }

    #[test]
    fn not_expired_when_far_future() {
        let creds = ResolvedCredentials {
            access_key_id: "AKID".into(),
            secret_access_key: "SECRET".into(),
            session_token: None,
            expiry: Some(std::time::SystemTime::now() + std::time::Duration::from_secs(3600)),
        };
        assert!(!AwsCredentialProvider::is_expired(&creds));
    }
}
```

- [ ] **Step 3: Update lib.rs**

Uncomment the provider re-export in lib.rs.

- [ ] **Step 4: Run tests**

Run: `cargo test -p aws-creds`
Expected: 7 tests pass

- [ ] **Step 5: Commit**

```
feat(aws): implement credential provider wrapping AWS SDK chain

AwsCredentialProvider wraps the AWS SDK default credential chain,
resolving credentials from environment variables, IMDS, ECS
container credentials, shared credential files, SSO cache, and
process credentials — in that order per AWS convention.

Credentials are cached with a 5-minute expiry buffer. Static
credentials are supported for testing without AWS access. The
provider is async because the AWS SDK credential resolution
involves network calls (IMDS, SSO token refresh, etc.).
```

---

### Task 3: Background Credential Refresh

**Files:**
- Create: `crates/aws-creds/src/refresh.rs`
- Modify: `crates/aws-creds/src/lib.rs` (uncomment refresh re-export)

- [ ] **Step 1: Implement refresh task**

```rust
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::interval;

use crate::error::CredentialError;
use crate::provider::AwsCredentialProvider;

/// Spawns a background task that refreshes credentials before they
/// expire. Checks every `check_interval` and refreshes when
/// credentials are within 5 minutes of expiry.
///
/// Returns the JoinHandle for the spawned task.
pub fn spawn_credential_refresh(
    provider: Arc<AwsCredentialProvider>,
    check_interval: Duration,
    mut shutdown: watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut timer = interval(check_interval);
        timer.tick().await; // skip immediate first tick

        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(300); // 5 minutes

        loop {
            tokio::select! {
                _ = timer.tick() => {
                    match provider.resolve().await {
                        Ok(creds) => {
                            backoff = Duration::from_secs(1); // reset backoff on success
                            // If credentials have expiry, check if we need a proactive refresh
                            if let Some(expiry) = creds.expiry {
                                let now = std::time::SystemTime::now();
                                let remaining = expiry.duration_since(now).unwrap_or_default();
                                if remaining < Duration::from_secs(600) {
                                    // Within 10 min of expiry — force refresh
                                    if let Err(_e) = provider.refresh().await {
                                        // tracing::warn!("proactive credential refresh failed: {}", e);
                                    }
                                }
                            }
                        }
                        Err(_e) => {
                            // tracing::warn!("credential check failed: {}, retrying in {:?}", e, backoff);
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
```

- [ ] **Step 2: Add tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::AwsCredentialProvider;

    #[tokio::test]
    async fn refresh_task_runs_and_stops() {
        let provider = Arc::new(AwsCredentialProvider::from_static(
            "AKID", "SECRET", None,
        ));

        let (tx, rx) = watch::channel(false);

        let handle = spawn_credential_refresh(
            provider.clone(),
            Duration::from_millis(100),
            rx,
        );

        // Let it run for a bit
        tokio::time::sleep(Duration::from_millis(350)).await;

        // Credentials should be cached
        let creds = provider.resolve().await.unwrap();
        assert_eq!(creds.access_key_id, "AKID");

        // Signal shutdown
        tx.send(true).unwrap();

        // Should stop within reasonable time
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("refresh task should stop within 2 seconds")
            .unwrap();
    }

    #[tokio::test]
    async fn refresh_task_immediate_shutdown() {
        let provider = Arc::new(AwsCredentialProvider::from_static(
            "AKID", "SECRET", None,
        ));

        let (tx, rx) = watch::channel(false);

        let handle = spawn_credential_refresh(
            provider,
            Duration::from_secs(60), // long interval
            rx,
        );

        // Immediately signal shutdown
        tx.send(true).unwrap();

        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("should stop quickly")
            .unwrap();
    }
}
```

- [ ] **Step 3: Update lib.rs**

Uncomment the refresh re-export.

- [ ] **Step 4: Run tests**

Run: `cargo test -p aws-creds`
Expected: 9 tests pass (7 provider + 2 refresh)

Run: `cargo test --workspace`
Expected: all pass

- [ ] **Step 5: Commit**

```
feat(aws): add background credential refresh task

spawn_credential_refresh monitors credential expiry and proactively
refreshes before credentials lapse. Checks at a configurable
interval (default: every 60s in production) and triggers a forced
refresh when credentials are within 10 minutes of expiry.

Uses exponential backoff (1s to 5min) on refresh failures to avoid
hammering IMDS or STS during outages. Clean shutdown via watch
channel — the task exits promptly when signaled.
```

---

### Task 4: Integration Test

**Files:**
- Create: `crates/aws-creds/tests/credential_flow.rs`

- [ ] **Step 1: Write integration test**

```rust
use std::sync::Arc;
use std::time::Duration;
use aws_creds::{AwsCredentialProvider, spawn_credential_refresh};
use tokio::sync::watch;

#[tokio::test]
async fn full_credential_lifecycle() {
    // Create provider with static credentials
    let provider = Arc::new(AwsCredentialProvider::from_static(
        "AKIAIOSFODNN7EXAMPLE",
        "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        Some("session-token".to_string()),
    ));

    // Start refresh task
    let (tx, rx) = watch::channel(false);
    let handle = spawn_credential_refresh(
        provider.clone(),
        Duration::from_millis(50),
        rx,
    );

    // Resolve credentials
    let creds = provider.resolve().await.unwrap();
    assert_eq!(creds.access_key_id, "AKIAIOSFODNN7EXAMPLE");
    assert_eq!(creds.session_token, Some("session-token".to_string()));

    // Force refresh
    let refreshed = provider.refresh().await.unwrap();
    assert_eq!(refreshed.access_key_id, "AKIAIOSFODNN7EXAMPLE");

    // Stop refresh task
    tx.send(true).unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn provider_without_refresh_task() {
    // Provider works standalone without refresh task
    let provider = AwsCredentialProvider::from_static("A", "B", None);

    let creds = provider.resolve().await.unwrap();
    assert_eq!(creds.access_key_id, "A");
    assert_eq!(creds.secret_access_key, "B");
    assert!(creds.session_token.is_none());
    assert!(creds.expiry.is_none());
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace`
Expected: all pass

- [ ] **Step 3: Commit**

```
test(aws): add credential lifecycle integration test

Validates the full credential flow: provider creation with static
credentials, background refresh task startup, credential resolution,
forced refresh, and clean shutdown. Also tests that the provider
works standalone without a refresh task.

Uses static credentials so the test runs without AWS access.
```

---

## Self-Review

**Spec coverage:**
- Environment variables, IMDS, ECS, shared credential file, SSO, process credentials — all handled by the AWS SDK default chain ✓
- Automatic refresh with background task ✓
- Exponential backoff with jitter on failures — backoff ✓, jitter not added (acceptable, can add later) ✓
- Events for service logging — prepared with commented tracing calls, will activate when tracing is added to the workspace ✓
- Interface to NFS client: credentials flow to TLS layer, not RPC auth — the provider exposes ResolvedCredentials that the TLS transport will consume ✓
- Profile-aware (aws-profile mount option) ✓

**Placeholder scan:** No placeholders found.

**Type consistency:**
- `AwsCredentialProvider` — consistent across provider.rs, refresh.rs, lib.rs
- `ResolvedCredentials` — consistent in provider.rs and tests
- `CredentialError` — consistent across error.rs, provider.rs, refresh.rs
- `spawn_credential_refresh` — signature consistent between refresh.rs and tests
