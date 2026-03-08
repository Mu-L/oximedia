//! Security features including credentials management and encryption

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{CloudError, Result};

/// Cloud credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    /// Access key ID
    pub access_key: String,
    /// Secret access key
    pub secret_key: String,
    /// Session token (optional, for temporary credentials)
    pub session_token: Option<String>,
    /// Additional provider-specific credentials
    pub extra: HashMap<String, String>,
}

impl Credentials {
    /// Create new credentials
    #[must_use]
    pub fn new(access_key: String, secret_key: String) -> Self {
        Self {
            access_key,
            secret_key,
            session_token: None,
            extra: HashMap::new(),
        }
    }

    /// Create credentials with session token
    #[must_use]
    pub fn with_session_token(
        access_key: String,
        secret_key: String,
        session_token: String,
    ) -> Self {
        Self {
            access_key,
            secret_key,
            session_token: Some(session_token),
            extra: HashMap::new(),
        }
    }

    /// Validate credentials
    pub fn validate(&self) -> Result<()> {
        if self.access_key.is_empty() {
            return Err(CloudError::InvalidConfig("Access key is empty".to_string()));
        }
        if self.secret_key.is_empty() {
            return Err(CloudError::InvalidConfig("Secret key is empty".to_string()));
        }
        Ok(())
    }

    /// Check if credentials are temporary (have session token)
    #[must_use]
    pub fn is_temporary(&self) -> bool {
        self.session_token.is_some()
    }
}

/// Encryption configuration
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// Encryption algorithm
    pub algorithm: EncryptionAlgorithm,
    /// KMS configuration (if using KMS)
    pub kms_config: Option<KmsConfig>,
    /// Customer-provided key (if using client-side encryption)
    pub customer_key: Option<Vec<u8>>,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            algorithm: EncryptionAlgorithm::AES256,
            kms_config: None,
            customer_key: None,
        }
    }
}

/// Encryption algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    /// AES-256
    AES256,
    /// AWS KMS
    AwsKms,
    /// Azure Key Vault
    AzureKeyVault,
    /// GCP KMS
    GcpKms,
}

/// KMS (Key Management Service) configuration
#[derive(Debug, Clone)]
pub struct KmsConfig {
    /// KMS key ID or ARN
    pub key_id: String,
    /// KMS endpoint (optional)
    pub endpoint: Option<String>,
    /// Additional context
    pub context: HashMap<String, String>,
}

impl KmsConfig {
    /// Create new KMS configuration
    #[must_use]
    pub fn new(key_id: String) -> Self {
        Self {
            key_id,
            endpoint: None,
            context: HashMap::new(),
        }
    }

    /// Add encryption context
    pub fn add_context(&mut self, key: String, value: String) {
        self.context.insert(key, value);
    }
}

/// IAM role configuration for AWS
#[derive(Debug, Clone)]
pub struct IamRoleConfig {
    /// Role ARN
    pub role_arn: String,
    /// Session name
    pub session_name: String,
    /// External ID (for cross-account access)
    pub external_id: Option<String>,
    /// Session duration in seconds
    pub duration_secs: u32,
}

impl IamRoleConfig {
    /// Create new IAM role configuration
    #[must_use]
    pub fn new(role_arn: String, session_name: String) -> Self {
        Self {
            role_arn,
            session_name,
            external_id: None,
            duration_secs: 3600, // 1 hour default
        }
    }

    /// Set external ID
    #[must_use]
    pub fn with_external_id(mut self, external_id: String) -> Self {
        self.external_id = Some(external_id);
        self
    }

    /// Set session duration
    #[must_use]
    pub fn with_duration(mut self, duration_secs: u32) -> Self {
        self.duration_secs = duration_secs;
        self
    }
}

/// Service principal configuration for Azure
#[derive(Debug, Clone)]
pub struct ServicePrincipalConfig {
    /// Tenant ID
    pub tenant_id: String,
    /// Client ID
    pub client_id: String,
    /// Client secret
    pub client_secret: String,
}

impl ServicePrincipalConfig {
    /// Create new service principal configuration
    #[must_use]
    pub fn new(tenant_id: String, client_id: String, client_secret: String) -> Self {
        Self {
            tenant_id,
            client_id,
            client_secret,
        }
    }
}

/// Service account configuration for GCP
#[derive(Debug, Clone)]
pub struct ServiceAccountConfig {
    /// Project ID
    pub project_id: String,
    /// Service account email
    pub email: String,
    /// Private key (PEM format)
    pub private_key: String,
}

impl ServiceAccountConfig {
    /// Create new service account configuration
    #[must_use]
    pub fn new(project_id: String, email: String, private_key: String) -> Self {
        Self {
            project_id,
            email,
            private_key,
        }
    }
}

/// Credential rotation manager
pub struct CredentialRotation {
    /// Current credentials
    current: Credentials,
    /// Rotation interval in seconds
    rotation_interval_secs: u64,
    /// Last rotation timestamp
    last_rotation: std::time::Instant,
}

impl CredentialRotation {
    /// Create new credential rotation manager
    #[must_use]
    pub fn new(credentials: Credentials, rotation_interval_secs: u64) -> Self {
        Self {
            current: credentials,
            rotation_interval_secs,
            last_rotation: std::time::Instant::now(),
        }
    }

    /// Check if rotation is needed
    #[must_use]
    pub fn needs_rotation(&self) -> bool {
        self.last_rotation.elapsed().as_secs() >= self.rotation_interval_secs
    }

    /// Get current credentials
    #[must_use]
    pub fn current(&self) -> &Credentials {
        &self.current
    }

    /// Update credentials
    pub fn rotate(&mut self, new_credentials: Credentials) {
        self.current = new_credentials;
        self.last_rotation = std::time::Instant::now();
    }
}

/// Access Control List (ACL) options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acl {
    /// Private (owner only)
    Private,
    /// Public read
    PublicRead,
    /// Public read-write
    PublicReadWrite,
    /// Authenticated read
    AuthenticatedRead,
    /// Bucket owner read
    BucketOwnerRead,
    /// Bucket owner full control
    BucketOwnerFullControl,
}

impl Acl {
    /// Convert to AWS S3 ACL string
    #[must_use]
    pub fn to_s3_string(&self) -> &str {
        match self {
            Acl::Private => "private",
            Acl::PublicRead => "public-read",
            Acl::PublicReadWrite => "public-read-write",
            Acl::AuthenticatedRead => "authenticated-read",
            Acl::BucketOwnerRead => "bucket-owner-read",
            Acl::BucketOwnerFullControl => "bucket-owner-full-control",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_validation() {
        let valid = Credentials::new("access".to_string(), "secret".to_string());
        assert!(valid.validate().is_ok());

        let invalid = Credentials::new("".to_string(), "secret".to_string());
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_credentials_temporary() {
        let permanent = Credentials::new("access".to_string(), "secret".to_string());
        assert!(!permanent.is_temporary());

        let temporary = Credentials::with_session_token(
            "access".to_string(),
            "secret".to_string(),
            "token".to_string(),
        );
        assert!(temporary.is_temporary());
    }

    #[test]
    fn test_kms_config() {
        let mut kms = KmsConfig::new("key-id".to_string());
        kms.add_context("env".to_string(), "prod".to_string());
        assert_eq!(kms.context.len(), 1);
    }

    #[test]
    fn test_iam_role_config() {
        let role = IamRoleConfig::new(
            "arn:aws:iam::123456789012:role/test".to_string(),
            "session".to_string(),
        )
        .with_external_id("external".to_string())
        .with_duration(7200);

        assert_eq!(role.external_id, Some("external".to_string()));
        assert_eq!(role.duration_secs, 7200);
    }

    #[test]
    fn test_credential_rotation() {
        let creds = Credentials::new("access".to_string(), "secret".to_string());
        let mut rotation = CredentialRotation::new(creds.clone(), 60);

        assert!(!rotation.needs_rotation());

        let new_creds = Credentials::new("new_access".to_string(), "new_secret".to_string());
        rotation.rotate(new_creds);
        assert_eq!(rotation.current().access_key, "new_access");
    }

    #[test]
    fn test_acl_to_string() {
        assert_eq!(Acl::Private.to_s3_string(), "private");
        assert_eq!(Acl::PublicRead.to_s3_string(), "public-read");
    }
}
