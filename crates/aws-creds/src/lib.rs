pub mod error;
pub mod provider;

pub use error::CredentialError;
pub use provider::{AwsCredentialProvider, ResolvedCredentials};
