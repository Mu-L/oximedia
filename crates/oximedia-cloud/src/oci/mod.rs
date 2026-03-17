//! Oracle Cloud Infrastructure (OCI) Object Storage provider.
//!
//! This module provides a structural stub for the OCI Object Storage API.
//! The design follows the same pattern as the AWS, Azure, and GCP providers
//! already present in this crate.
//!
//! ## Structure
//!
//! - [`OciCredentials`] — OCI API key / tenancy authentication.
//! - [`OciRegion`] — well-known OCI region identifiers.
//! - [`OciObjectStorageConfig`] — configuration for an OCI namespace + bucket.
//! - [`OciObjectStorage`] — implements [`CloudStorage`] for OCI Object Storage.
//!
//! ## Authentication
//!
//! OCI uses **API key authentication**: a PEM private key is used to sign
//! every HTTP request with the OCI Signature V1 scheme (HTTP signature with
//! SHA-256 body digest). The fields on [`OciCredentials`] map directly to the
//! OCI SDK configuration file entries.
//!
//! ## Example (struct construction only — no live calls)
//!
//! ```rust
//! use oximedia_cloud::oci::{OciCredentials, OciObjectStorageConfig, OciRegion};
//!
//! let creds = OciCredentials::new(
//!     "ocid1.tenancy.oc1..aaa".to_string(),
//!     "ocid1.user.oc1..bbb".to_string(),
//!     "aa:bb:cc:dd".to_string(),
//!     "-----BEGIN RSA PRIVATE KEY-----\nfake\n-----END RSA PRIVATE KEY-----".to_string(),
//! );
//!
//! let config = OciObjectStorageConfig::new(
//!     creds,
//!     OciRegion::EuFrankfurt1,
//!     "my-namespace".to_string(),
//!     "my-bucket".to_string(),
//! );
//! ```

use crate::error::{CloudError, Result};
use crate::types::{
    CloudStorage, DeleteResult, ListResult, ObjectInfo, ObjectMetadata, StorageClass, StorageStats,
    UploadOptions,
};

use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── OCI Credentials ───────────────────────────────────────────────────────────

/// Authentication credentials for OCI Object Storage.
///
/// These map directly to entries in the OCI `~/.oci/config` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciCredentials {
    /// Tenancy OCID (`ocid1.tenancy.oc1..xxx`).
    pub tenancy_id: String,
    /// User OCID (`ocid1.user.oc1..xxx`).
    pub user_id: String,
    /// Fingerprint of the uploaded API public key (colon-separated hex pairs).
    pub fingerprint: String,
    /// PEM-encoded RSA private key corresponding to the uploaded public key.
    pub private_key_pem: String,
    /// Optional passphrase for an encrypted private key.
    pub private_key_passphrase: Option<String>,
}

impl OciCredentials {
    /// Create new OCI API key credentials.
    #[must_use]
    pub fn new(
        tenancy_id: impl Into<String>,
        user_id: impl Into<String>,
        fingerprint: impl Into<String>,
        private_key_pem: impl Into<String>,
    ) -> Self {
        Self {
            tenancy_id: tenancy_id.into(),
            user_id: user_id.into(),
            fingerprint: fingerprint.into(),
            private_key_pem: private_key_pem.into(),
            private_key_passphrase: None,
        }
    }

    /// Set an optional passphrase for an encrypted private key.
    #[must_use]
    pub fn with_passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.private_key_passphrase = Some(passphrase.into());
        self
    }

    /// Validate that required fields are non-empty.
    pub fn validate(&self) -> Result<()> {
        if self.tenancy_id.is_empty() {
            return Err(CloudError::InvalidConfig(
                "OCI tenancy_id is empty".to_string(),
            ));
        }
        if self.user_id.is_empty() {
            return Err(CloudError::InvalidConfig(
                "OCI user_id is empty".to_string(),
            ));
        }
        if self.fingerprint.is_empty() {
            return Err(CloudError::InvalidConfig(
                "OCI fingerprint is empty".to_string(),
            ));
        }
        if self.private_key_pem.is_empty() {
            return Err(CloudError::InvalidConfig(
                "OCI private_key_pem is empty".to_string(),
            ));
        }
        Ok(())
    }
}

// ── OCI Regions ───────────────────────────────────────────────────────────────

/// Well-known OCI region identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OciRegion {
    /// US East (Ashburn)
    UsAshburn1,
    /// US West (Phoenix)
    UsPhoenix1,
    /// US West (San Jose)
    UsSanJose1,
    /// Canada Southeast (Toronto)
    CaTorontoMontreal1,
    /// UK South (London)
    UkLondon1,
    /// EU West (Amsterdam)
    EuAmsterdam1,
    /// EU Central (Frankfurt)
    EuFrankfurt1,
    /// EU South (Milan)
    EuMilan1,
    /// AP East (Tokyo)
    ApTokyo1,
    /// AP Southeast (Sydney)
    ApSydney1,
    /// AP Southeast (Singapore)
    ApSingapore1,
    /// AP South (Mumbai)
    ApMumbai1,
    /// South Africa (Johannesburg)
    AfJohannesburg1,
    /// Middle East (Dubai)
    MeDubai1,
    /// Custom / private region.
    Custom(String),
}

impl OciRegion {
    /// Returns the OCI region identifier string (e.g. `us-ashburn-1`).
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::UsAshburn1 => "us-ashburn-1",
            Self::UsPhoenix1 => "us-phoenix-1",
            Self::UsSanJose1 => "us-sanjose-1",
            Self::CaTorontoMontreal1 => "ca-toronto-1",
            Self::UkLondon1 => "uk-london-1",
            Self::EuAmsterdam1 => "eu-amsterdam-1",
            Self::EuFrankfurt1 => "eu-frankfurt-1",
            Self::EuMilan1 => "eu-milan-1",
            Self::ApTokyo1 => "ap-tokyo-1",
            Self::ApSydney1 => "ap-sydney-1",
            Self::ApSingapore1 => "ap-singapore-1",
            Self::ApMumbai1 => "ap-mumbai-1",
            Self::AfJohannesburg1 => "af-johannesburg-1",
            Self::MeDubai1 => "me-dubai-1",
            Self::Custom(s) => s.as_str(),
        }
    }

    /// Build the Object Storage endpoint URL for this region.
    ///
    /// Format: `https://objectstorage.{region}.oraclecloud.com`
    #[must_use]
    pub fn endpoint_url(&self) -> String {
        format!("https://objectstorage.{}.oraclecloud.com", self.id())
    }
}

impl std::fmt::Display for OciRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

// ── OCI Storage class mapping ─────────────────────────────────────────────────

/// OCI Object Storage tier.
///
/// OCI uses the term *storage tier* rather than *storage class*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OciStorageTier {
    /// Standard (hot) storage — low latency, higher cost.
    Standard,
    /// Infrequent access — reduced storage cost, retrieval fee applies.
    InfrequentAccess,
    /// Archive — lowest cost, long restore time (hours).
    Archive,
}

impl OciStorageTier {
    /// Map an [`OciStorageTier`] to the generic [`StorageClass`].
    #[must_use]
    pub fn to_storage_class(self) -> StorageClass {
        match self {
            Self::Standard => StorageClass::Standard,
            Self::InfrequentAccess => StorageClass::InfrequentAccess,
            Self::Archive => StorageClass::Glacier,
        }
    }

    /// Map a generic [`StorageClass`] to the closest [`OciStorageTier`].
    #[must_use]
    pub fn from_storage_class(class: StorageClass) -> Self {
        match class {
            StorageClass::Standard
            | StorageClass::ReducedRedundancy
            | StorageClass::IntelligentTiering => Self::Standard,
            StorageClass::InfrequentAccess | StorageClass::OneZoneIA => Self::InfrequentAccess,
            StorageClass::Glacier | StorageClass::DeepArchive => Self::Archive,
        }
    }

    /// OCI API tier name string.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::InfrequentAccess => "InfrequentAccess",
            Self::Archive => "Archive",
        }
    }
}

// ── OCI Object Storage Configuration ─────────────────────────────────────────

/// Configuration for an OCI Object Storage bucket.
#[derive(Debug, Clone)]
pub struct OciObjectStorageConfig {
    /// OCI API key credentials.
    pub credentials: OciCredentials,
    /// OCI region.
    pub region: OciRegion,
    /// OCI namespace (also called the *tenancy name*).
    pub namespace: String,
    /// Bucket name.
    pub bucket: String,
    /// Default storage tier for new objects.
    pub default_tier: OciStorageTier,
    /// Optional request timeout in seconds.
    pub request_timeout_secs: Option<u64>,
}

impl OciObjectStorageConfig {
    /// Create a new configuration.
    #[must_use]
    pub fn new(
        credentials: OciCredentials,
        region: OciRegion,
        namespace: impl Into<String>,
        bucket: impl Into<String>,
    ) -> Self {
        Self {
            credentials,
            region,
            namespace: namespace.into(),
            bucket: bucket.into(),
            default_tier: OciStorageTier::Standard,
            request_timeout_secs: Some(30),
        }
    }

    /// Override the default storage tier.
    #[must_use]
    pub fn with_default_tier(mut self, tier: OciStorageTier) -> Self {
        self.default_tier = tier;
        self
    }

    /// Override the request timeout.
    #[must_use]
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.request_timeout_secs = Some(secs);
        self
    }

    /// Build the base endpoint URL for this configuration.
    #[must_use]
    pub fn endpoint_url(&self) -> String {
        self.region.endpoint_url()
    }

    /// Build the Object Storage service URL for the configured namespace.
    ///
    /// Format: `https://objectstorage.{region}.oraclecloud.com/n/{namespace}/b/{bucket}/o`
    #[must_use]
    pub fn object_base_url(&self) -> String {
        format!(
            "{}/n/{}/b/{}/o",
            self.region.endpoint_url(),
            urlencoding::encode(&self.namespace),
            urlencoding::encode(&self.bucket),
        )
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        self.credentials.validate()?;
        if self.namespace.is_empty() {
            return Err(CloudError::InvalidConfig(
                "OCI namespace is empty".to_string(),
            ));
        }
        if self.bucket.is_empty() {
            return Err(CloudError::InvalidConfig("OCI bucket is empty".to_string()));
        }
        Ok(())
    }
}

// ── OCI Object Storage client ─────────────────────────────────────────────────

/// OCI Object Storage client.
///
/// Implements [`CloudStorage`] so it can be used anywhere a cloud storage
/// backend is expected.  All operations return
/// [`CloudError::ServiceUnavailable`] in this stub build; a full
/// implementation would sign requests with the OCI HTTP Signature scheme and
/// call the OCI REST API.
pub struct OciObjectStorage {
    config: OciObjectStorageConfig,
}

impl OciObjectStorage {
    /// Create a new OCI Object Storage client.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::InvalidConfig`] if the configuration is invalid.
    pub fn new(config: OciObjectStorageConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Returns a reference to the configuration.
    #[must_use]
    pub fn config(&self) -> &OciObjectStorageConfig {
        &self.config
    }

    /// Returns the base URL for objects in the configured bucket.
    #[must_use]
    pub fn object_base_url(&self) -> String {
        self.config.object_base_url()
    }

    // Stub helper used by the CloudStorage trait impl.
    fn not_implemented(op: &str) -> CloudError {
        CloudError::ServiceUnavailable(format!(
            "OCI Object Storage: '{op}' is not yet implemented (stub)"
        ))
    }
}

#[async_trait]
impl CloudStorage for OciObjectStorage {
    async fn upload(&self, key: &str, _data: Bytes) -> Result<()> {
        tracing::debug!("OCI stub upload: bucket={} key={}", self.config.bucket, key);
        Err(Self::not_implemented("upload"))
    }

    async fn upload_with_options(
        &self,
        key: &str,
        _data: Bytes,
        _options: UploadOptions,
    ) -> Result<()> {
        Err(Self::not_implemented(&format!(
            "upload_with_options({key})"
        )))
    }

    async fn download(&self, key: &str) -> Result<Bytes> {
        Err(Self::not_implemented(&format!("download({key})")))
    }

    async fn download_range(&self, key: &str, _start: u64, _end: u64) -> Result<Bytes> {
        Err(Self::not_implemented(&format!("download_range({key})")))
    }

    async fn list(&self, prefix: &str) -> Result<Vec<ObjectInfo>> {
        Err(Self::not_implemented(&format!("list({prefix})")))
    }

    async fn list_paginated(
        &self,
        prefix: &str,
        _continuation_token: Option<String>,
        _max_keys: usize,
    ) -> Result<ListResult> {
        Err(Self::not_implemented(&format!("list_paginated({prefix})")))
    }

    async fn delete(&self, key: &str) -> Result<()> {
        Err(Self::not_implemented(&format!("delete({key})")))
    }

    async fn delete_batch(&self, keys: &[String]) -> Result<Vec<DeleteResult>> {
        Err(Self::not_implemented(&format!(
            "delete_batch({} keys)",
            keys.len()
        )))
    }

    async fn get_metadata(&self, key: &str) -> Result<ObjectMetadata> {
        Err(Self::not_implemented(&format!("get_metadata({key})")))
    }

    async fn update_metadata(&self, key: &str, _metadata: HashMap<String, String>) -> Result<()> {
        Err(Self::not_implemented(&format!("update_metadata({key})")))
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        Err(Self::not_implemented(&format!("exists({key})")))
    }

    async fn copy(&self, src: &str, dst: &str) -> Result<()> {
        Err(Self::not_implemented(&format!("copy({src} → {dst})")))
    }

    async fn presigned_download_url(&self, key: &str, _expires_in_secs: u64) -> Result<String> {
        Err(Self::not_implemented(&format!(
            "presigned_download_url({key})"
        )))
    }

    async fn presigned_upload_url(&self, key: &str, _expires_in_secs: u64) -> Result<String> {
        Err(Self::not_implemented(&format!(
            "presigned_upload_url({key})"
        )))
    }

    async fn set_storage_class(&self, key: &str, class: StorageClass) -> Result<()> {
        Err(Self::not_implemented(&format!(
            "set_storage_class({key}, {class})"
        )))
    }

    async fn get_stats(&self, prefix: &str) -> Result<StorageStats> {
        Err(Self::not_implemented(&format!("get_stats({prefix})")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_creds() -> OciCredentials {
        OciCredentials::new(
            "ocid1.tenancy.oc1..aaa",
            "ocid1.user.oc1..bbb",
            "aa:bb:cc:dd",
            "-----BEGIN RSA PRIVATE KEY-----\nfake\n-----END RSA PRIVATE KEY-----",
        )
    }

    // ── OciCredentials ───────────────────────────────────────────────────────

    #[test]
    fn test_oci_credentials_validate_ok() {
        assert!(make_creds().validate().is_ok());
    }

    #[test]
    fn test_oci_credentials_validate_empty_tenancy() {
        let creds = OciCredentials::new("", "user", "fp", "key");
        assert!(creds.validate().is_err());
    }

    #[test]
    fn test_oci_credentials_validate_empty_user() {
        let creds = OciCredentials::new("tenancy", "", "fp", "key");
        assert!(creds.validate().is_err());
    }

    #[test]
    fn test_oci_credentials_validate_empty_fingerprint() {
        let creds = OciCredentials::new("tenancy", "user", "", "key");
        assert!(creds.validate().is_err());
    }

    #[test]
    fn test_oci_credentials_validate_empty_key() {
        let creds = OciCredentials::new("tenancy", "user", "fp", "");
        assert!(creds.validate().is_err());
    }

    #[test]
    fn test_oci_credentials_with_passphrase() {
        let creds = make_creds().with_passphrase("s3cr3t");
        assert_eq!(creds.private_key_passphrase, Some("s3cr3t".to_string()));
    }

    // ── OciRegion ────────────────────────────────────────────────────────────

    #[test]
    fn test_oci_region_id_strings() {
        assert_eq!(OciRegion::UsAshburn1.id(), "us-ashburn-1");
        assert_eq!(OciRegion::EuFrankfurt1.id(), "eu-frankfurt-1");
        assert_eq!(OciRegion::ApTokyo1.id(), "ap-tokyo-1");
    }

    #[test]
    fn test_oci_region_custom_id() {
        let r = OciRegion::Custom("us-gov-chicago-1".to_string());
        assert_eq!(r.id(), "us-gov-chicago-1");
    }

    #[test]
    fn test_oci_region_endpoint_url() {
        let url = OciRegion::UsAshburn1.endpoint_url();
        assert_eq!(url, "https://objectstorage.us-ashburn-1.oraclecloud.com");
    }

    #[test]
    fn test_oci_region_display() {
        assert_eq!(OciRegion::UkLondon1.to_string(), "uk-london-1");
    }

    // ── OciStorageTier ────────────────────────────────────────────────────────

    #[test]
    fn test_oci_storage_tier_to_storage_class() {
        assert_eq!(
            OciStorageTier::Standard.to_storage_class(),
            StorageClass::Standard
        );
        assert_eq!(
            OciStorageTier::InfrequentAccess.to_storage_class(),
            StorageClass::InfrequentAccess
        );
        assert_eq!(
            OciStorageTier::Archive.to_storage_class(),
            StorageClass::Glacier
        );
    }

    #[test]
    fn test_oci_storage_tier_from_storage_class() {
        assert_eq!(
            OciStorageTier::from_storage_class(StorageClass::Standard),
            OciStorageTier::Standard
        );
        assert_eq!(
            OciStorageTier::from_storage_class(StorageClass::InfrequentAccess),
            OciStorageTier::InfrequentAccess
        );
        assert_eq!(
            OciStorageTier::from_storage_class(StorageClass::DeepArchive),
            OciStorageTier::Archive
        );
    }

    #[test]
    fn test_oci_storage_tier_as_str() {
        assert_eq!(OciStorageTier::Standard.as_str(), "Standard");
        assert_eq!(
            OciStorageTier::InfrequentAccess.as_str(),
            "InfrequentAccess"
        );
        assert_eq!(OciStorageTier::Archive.as_str(), "Archive");
    }

    // ── OciObjectStorageConfig ────────────────────────────────────────────────

    #[test]
    fn test_oci_config_validate_ok() {
        let config = OciObjectStorageConfig::new(
            make_creds(),
            OciRegion::UsAshburn1,
            "my-namespace",
            "my-bucket",
        );
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_oci_config_validate_empty_namespace() {
        let config = OciObjectStorageConfig::new(make_creds(), OciRegion::UsAshburn1, "", "bucket");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_oci_config_validate_empty_bucket() {
        let config = OciObjectStorageConfig::new(make_creds(), OciRegion::UsAshburn1, "ns", "");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_oci_config_endpoint_url() {
        let config =
            OciObjectStorageConfig::new(make_creds(), OciRegion::EuFrankfurt1, "ns", "bucket");
        assert_eq!(
            config.endpoint_url(),
            "https://objectstorage.eu-frankfurt-1.oraclecloud.com"
        );
    }

    #[test]
    fn test_oci_config_object_base_url() {
        let config =
            OciObjectStorageConfig::new(make_creds(), OciRegion::UsAshburn1, "myns", "mybucket");
        let url = config.object_base_url();
        assert!(
            url.contains("/n/myns/b/mybucket/o"),
            "unexpected url: {url}"
        );
    }

    #[test]
    fn test_oci_config_with_default_tier() {
        let config = OciObjectStorageConfig::new(make_creds(), OciRegion::UsAshburn1, "ns", "b")
            .with_default_tier(OciStorageTier::Archive);
        assert_eq!(config.default_tier, OciStorageTier::Archive);
    }

    // ── OciObjectStorage (stub) ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_oci_stub_upload_returns_not_implemented() {
        let config =
            OciObjectStorageConfig::new(make_creds(), OciRegion::UsAshburn1, "ns", "bucket");
        let storage = OciObjectStorage::new(config).expect("new should succeed");
        let result = storage.upload("key.mp4", Bytes::from("data")).await;
        assert!(result.is_err());
        let err_msg = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(
            err_msg.contains("stub"),
            "expected stub error, got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_oci_stub_download_returns_not_implemented() {
        let config = OciObjectStorageConfig::new(make_creds(), OciRegion::UsAshburn1, "ns", "b");
        let storage = OciObjectStorage::new(config).expect("new ok");
        let result = storage.download("key.mp4").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_oci_new_rejects_invalid_config() {
        let bad_creds = OciCredentials::new("", "user", "fp", "key");
        let config = OciObjectStorageConfig::new(bad_creds, OciRegion::UsAshburn1, "ns", "b");
        assert!(OciObjectStorage::new(config).is_err());
    }
}
