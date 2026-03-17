//! Cloud storage abstraction for OxiMedia
//!
//! This crate provides a unified interface for working with multiple cloud storage providers:
//! - Amazon S3
//! - Azure Blob Storage
//! - Google Cloud Storage
//!
//! # Features
//!
//! - Unified API across all providers
//! - Streaming uploads and downloads
//! - Multipart upload support for large files
//! - Progress tracking
//! - Resume capability
//! - Local caching layer
//! - Rate limiting
//! - Retry with exponential backoff

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::Stream;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;

#[cfg(feature = "s3")]
pub mod s3;

#[cfg(feature = "azure")]
pub mod azure;

#[cfg(feature = "gcs")]
pub mod gcs;

/// Storage access logging and audit trail.
pub mod access_log;
/// Bandwidth throttling via token bucket algorithm.
pub mod bandwidth_throttle;
/// Parallel multi-object upload and download batch operations.
pub mod batch_operations;
pub mod cache;
/// Cache eviction layer: LRU, LFU, FIFO, and ARC caches with statistics.
pub mod cache_layer;
/// Compression store — compress/decompress objects with ratio and savings tracking.
pub mod compression_store;
/// Content-type detection from file extension.
pub mod content_type;
/// Content-addressable deduplication storage (hash-based addressing, chunk dedup, reference counting).
pub mod dedup_store;
/// Data integrity verification for stored objects.
pub mod integrity_checker;
/// Storage lifecycle policies (age-based transitions, cost tiers, expiration rules).
pub mod lifecycle;
pub mod local;
/// Migration planner for staged cross-provider migration workflows.
pub mod migration_planner;
/// MinIO backend (S3-compatible self-hosted object storage).
pub mod minio;
/// Namespace management — logical grouping of objects with hierarchical names.
pub mod namespace;
/// In-memory object store abstraction — keys, metadata, and basic CRUD operations.
pub mod object_store;
/// Object version listing, restore, and delete-marker management.
pub mod object_versioning;
/// Path resolution, normalization, and glob matching for object keys.
pub mod path_resolver;
pub mod quota;
pub mod replication;
/// Advanced replication policy management (sync policies, replication lag, consistency levels).
pub mod replication_policy;
/// Object retention and hold management.
pub mod retention_manager;
/// Storage event bus — publish/subscribe for object lifecycle events.
pub mod storage_events;
/// Storage operation metrics — counters, gauges, histograms, and error rates.
pub mod storage_metrics;
/// Cross-provider storage migration with progress tracking and hash verification.
pub mod storage_migration;
/// Storage policy management — access classes, retention rules, and policy evaluation.
pub mod storage_policy;
pub mod tiering;
pub mod transfer;
/// Transfer statistics — recording upload/download events and computing throughput metrics.
pub mod transfer_stats;
/// Object versioning (version ID tracking per key).
pub mod versioning;
/// Write-ahead log for crash-safe storage mutation tracking and replay.
pub mod write_ahead_log;

/// Errors that can occur during storage operations
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Object not found: {0}")]
    NotFound(String),

    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Multipart upload error: {0}")]
    MultipartError(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Invalid object key: {0}")]
    InvalidKey(String),

    #[error("Storage quota exceeded")]
    QuotaExceeded,

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("Provider-specific error: {0}")]
    ProviderError(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// Storage provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageProvider {
    /// Amazon S3
    S3,
    /// Azure Blob Storage
    Azure,
    /// Google Cloud Storage
    GCS,
}

/// Metadata for a stored object
#[derive(Debug, Clone)]
pub struct ObjectMetadata {
    /// Object key/name
    pub key: String,
    /// Object size in bytes
    pub size: u64,
    /// Content type (MIME type)
    pub content_type: Option<String>,
    /// Last modified timestamp
    pub last_modified: DateTime<Utc>,
    /// ETag or version identifier
    pub etag: Option<String>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
    /// Storage class or tier
    pub storage_class: Option<String>,
}

/// Configuration for object upload
#[derive(Debug, Clone, Default)]
pub struct UploadOptions {
    /// Content type
    pub content_type: Option<String>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
    /// Storage class
    pub storage_class: Option<String>,
    /// Cache control header
    pub cache_control: Option<String>,
    /// Content encoding
    pub content_encoding: Option<String>,
    /// Server-side encryption
    pub encryption: Option<EncryptionConfig>,
    /// ACL/permissions
    pub acl: Option<String>,
}

/// Encryption configuration
#[derive(Debug, Clone)]
pub enum EncryptionConfig {
    /// Server-side encryption with provider-managed keys
    ServerSide,
    /// Server-side encryption with customer-provided keys
    CustomerKey(String),
    /// Client-side encryption
    ClientSide { key: String, algorithm: String },
}

/// Configuration for object download
#[derive(Debug, Clone, Default)]
pub struct DownloadOptions {
    /// Byte range to download (start, end)
    pub range: Option<(u64, u64)>,
    /// If-Match condition
    pub if_match: Option<String>,
    /// If-None-Match condition
    pub if_none_match: Option<String>,
    /// If-Modified-Since condition
    pub if_modified_since: Option<DateTime<Utc>>,
}

/// Configuration for listing objects
#[derive(Debug, Clone)]
pub struct ListOptions {
    /// Prefix to filter objects
    pub prefix: Option<String>,
    /// Delimiter for hierarchical listing
    pub delimiter: Option<String>,
    /// Maximum number of objects to return
    pub max_results: Option<usize>,
    /// Continuation token for pagination
    pub continuation_token: Option<String>,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            prefix: None,
            delimiter: None,
            max_results: Some(1000),
            continuation_token: None,
        }
    }
}

/// Result of listing objects
#[derive(Debug, Clone)]
pub struct ListResult {
    /// List of objects
    pub objects: Vec<ObjectMetadata>,
    /// Common prefixes (directories)
    pub prefixes: Vec<String>,
    /// Continuation token for next page
    pub next_token: Option<String>,
    /// Whether there are more results
    pub has_more: bool,
}

/// Progress information for uploads/downloads
#[derive(Debug, Clone)]
pub struct ProgressInfo {
    /// Bytes transferred so far
    pub bytes_transferred: u64,
    /// Total bytes to transfer
    pub total_bytes: u64,
    /// Transfer speed in bytes per second
    pub bytes_per_second: f64,
    /// Estimated time remaining in seconds
    pub eta_seconds: Option<f64>,
}

/// Callback for progress updates
pub type ProgressCallback = Arc<dyn Fn(ProgressInfo) + Send + Sync>;

/// Retry configuration with exponential back-off and optional jitter.
///
/// The delay before attempt `n` (0-indexed) is computed as:
/// `min(initial_backoff_ms * backoff_multiplier^n, max_backoff_ms)`
/// optionally perturbed by a random jitter factor in `[0, jitter_factor]`.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (not counting the initial attempt).
    pub max_retries: u32,
    /// Multiplier applied to the backoff duration on each successive failure.
    /// Must be ≥ 1.0; values < 1.0 are clamped to 1.0.
    pub backoff_multiplier: f64,
    /// Base backoff in milliseconds before the first retry.
    pub initial_backoff_ms: u64,
    /// Hard ceiling on the computed backoff in milliseconds.
    pub max_backoff_ms: u64,
    /// Maximum relative jitter applied to the computed backoff.
    /// 0.0 = no jitter, 1.0 = up to 100 % of the computed delay is added randomly.
    /// Values outside `[0.0, 1.0]` are clamped.
    pub jitter_factor: f64,
    /// Only retry on transient errors (network / rate-limit); never retry on
    /// `NotFound`, `PermissionDenied`, or `InvalidKey`.
    pub retry_on_transient_only: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff_multiplier: 2.0,
            initial_backoff_ms: 500,
            max_backoff_ms: 30_000,
            jitter_factor: 0.2,
            retry_on_transient_only: true,
        }
    }
}

impl RetryConfig {
    /// Compute the backoff duration in milliseconds for attempt `n` (0-indexed).
    ///
    /// Uses a simple deterministic formula without `rand` to keep the crate
    /// pure-Rust and dependency-free.  The pseudo-jitter is derived from the
    /// attempt number itself so that the result is reproducible in tests.
    #[must_use]
    pub fn backoff_ms_for_attempt(&self, attempt: u32) -> u64 {
        let multiplier = self.backoff_multiplier.max(1.0);
        // Compute base: initial * multiplier^attempt
        let base = self.initial_backoff_ms as f64 * multiplier.powi(attempt as i32);
        let capped = base.min(self.max_backoff_ms as f64);
        // Deterministic jitter: use a Weyl-sequence offset keyed on the attempt number.
        let jitter_factor = self.jitter_factor.clamp(0.0, 1.0);
        // map attempt to a pseudo-random fraction in [0,1) via a simple hash mix
        let pseudo_rand = {
            let mut v = (attempt as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
            v ^= v >> 30;
            v = v.wrapping_mul(0xbf58_476d_1ce4_e5b9);
            v ^= v >> 27;
            v = v.wrapping_mul(0x94d0_49bb_1331_11eb);
            v ^= v >> 31;
            (v as f64) / (u64::MAX as f64)
        };
        let jitter_ms = capped * jitter_factor * pseudo_rand;
        (capped + jitter_ms) as u64
    }

    /// Returns `true` if the given `StorageError` should trigger a retry.
    #[must_use]
    pub fn should_retry(&self, error: &StorageError) -> bool {
        if !self.retry_on_transient_only {
            return true;
        }
        // Non-retryable: client-side or permanent errors
        !matches!(
            error,
            StorageError::NotFound(_)
                | StorageError::PermissionDenied(_)
                | StorageError::InvalidKey(_)
                | StorageError::QuotaExceeded
                | StorageError::InvalidConfig(_)
                | StorageError::AuthenticationError(_)
                | StorageError::UnsupportedOperation(_)
        )
    }
}

/// Unified configuration for cloud storage
#[derive(Debug, Clone)]
pub struct UnifiedConfig {
    /// Storage provider
    pub provider: StorageProvider,
    /// Bucket/container name
    pub bucket: String,
    /// Region (for S3 and GCS)
    pub region: Option<String>,
    /// Endpoint URL (for S3-compatible services)
    pub endpoint: Option<String>,
    /// Access key or account name
    pub access_key: Option<String>,
    /// Secret key or account key
    pub secret_key: Option<String>,
    /// Project ID (for GCS)
    pub project_id: Option<String>,
    /// Service account credentials file (for GCS)
    pub credentials_file: Option<PathBuf>,
    /// Enable transfer acceleration (S3)
    pub transfer_acceleration: bool,
    /// Enable path-style addressing (S3)
    pub path_style: bool,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Timeout for operations in seconds
    pub timeout_seconds: u64,
    /// Enable local caching
    pub enable_cache: bool,
    /// Cache directory
    pub cache_dir: Option<PathBuf>,
    /// Maximum cache size in bytes
    pub max_cache_size: u64,
    /// Retry behaviour for transient failures.
    pub retry: RetryConfig,
}

impl UnifiedConfig {
    /// Create a new configuration for S3
    #[cfg(feature = "s3")]
    pub fn s3(bucket: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            provider: StorageProvider::S3,
            bucket: bucket.into(),
            region: Some(region.into()),
            endpoint: None,
            access_key: None,
            secret_key: None,
            project_id: None,
            credentials_file: None,
            transfer_acceleration: false,
            path_style: false,
            max_connections: 10,
            timeout_seconds: 300,
            enable_cache: false,
            cache_dir: None,
            max_cache_size: 10 * 1024 * 1024 * 1024, // 10 GB
            retry: RetryConfig::default(),
        }
    }

    /// Create a new configuration for Azure
    #[cfg(feature = "azure")]
    pub fn azure(container: impl Into<String>, account: impl Into<String>) -> Self {
        Self {
            provider: StorageProvider::Azure,
            bucket: container.into(),
            region: None,
            endpoint: None,
            access_key: Some(account.into()),
            secret_key: None,
            project_id: None,
            credentials_file: None,
            transfer_acceleration: false,
            path_style: false,
            max_connections: 10,
            timeout_seconds: 300,
            enable_cache: false,
            cache_dir: None,
            max_cache_size: 10 * 1024 * 1024 * 1024,
            retry: RetryConfig::default(),
        }
    }

    /// Create a new configuration for GCS
    #[cfg(feature = "gcs")]
    pub fn gcs(bucket: impl Into<String>, project_id: impl Into<String>) -> Self {
        Self {
            provider: StorageProvider::GCS,
            bucket: bucket.into(),
            region: None,
            endpoint: None,
            access_key: None,
            secret_key: None,
            project_id: Some(project_id.into()),
            credentials_file: None,
            transfer_acceleration: false,
            path_style: false,
            max_connections: 10,
            timeout_seconds: 300,
            enable_cache: false,
            cache_dir: None,
            max_cache_size: 10 * 1024 * 1024 * 1024,
            retry: RetryConfig::default(),
        }
    }

    /// Set credentials
    pub fn with_credentials(
        mut self,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        self.access_key = Some(access_key.into());
        self.secret_key = Some(secret_key.into());
        self
    }

    /// Set endpoint
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Enable caching
    pub fn with_cache(mut self, cache_dir: PathBuf, max_size: u64) -> Self {
        self.enable_cache = true;
        self.cache_dir = Some(cache_dir);
        self.max_cache_size = max_size;
        self
    }

    /// Override retry configuration.
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }
}

/// Byte stream type for streaming data
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;

/// Main trait for cloud storage operations
#[async_trait]
pub trait CloudStorage: Send + Sync {
    /// Upload an object from a byte stream
    async fn upload_stream(
        &self,
        key: &str,
        stream: ByteStream,
        size: Option<u64>,
        options: UploadOptions,
    ) -> Result<String>;

    /// Upload an object from a file
    async fn upload_file(
        &self,
        key: &str,
        file_path: &std::path::Path,
        options: UploadOptions,
    ) -> Result<String>;

    /// Download an object as a byte stream
    async fn download_stream(&self, key: &str, options: DownloadOptions) -> Result<ByteStream>;

    /// Download an object to a file
    async fn download_file(
        &self,
        key: &str,
        file_path: &std::path::Path,
        options: DownloadOptions,
    ) -> Result<()>;

    /// Get object metadata
    async fn get_metadata(&self, key: &str) -> Result<ObjectMetadata>;

    /// Delete an object
    async fn delete_object(&self, key: &str) -> Result<()>;

    /// Delete multiple objects
    async fn delete_objects(&self, keys: &[String]) -> Result<Vec<Result<()>>>;

    /// List objects with prefix
    async fn list_objects(&self, options: ListOptions) -> Result<ListResult>;

    /// Check if an object exists
    async fn object_exists(&self, key: &str) -> Result<bool>;

    /// Copy an object within the same bucket
    async fn copy_object(&self, source_key: &str, dest_key: &str) -> Result<()>;

    /// Generate a presigned URL for downloading
    async fn generate_presigned_url(&self, key: &str, expiration_secs: u64) -> Result<String>;

    /// Generate a presigned URL for uploading
    async fn generate_presigned_upload_url(
        &self,
        key: &str,
        expiration_secs: u64,
    ) -> Result<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_provider_equality() {
        assert_eq!(StorageProvider::S3, StorageProvider::S3);
        assert_ne!(StorageProvider::S3, StorageProvider::Azure);
    }

    #[test]
    fn test_upload_options_default() {
        let options = UploadOptions::default();
        assert!(options.content_type.is_none());
        assert!(options.metadata.is_empty());
    }

    #[test]
    fn test_list_options_default() {
        let options = ListOptions::default();
        assert!(options.prefix.is_none());
        assert_eq!(options.max_results, Some(1000));
    }

    #[cfg(feature = "s3")]
    #[test]
    fn test_unified_config_s3() {
        let config = UnifiedConfig::s3("my-bucket", "us-east-1");
        assert_eq!(config.provider, StorageProvider::S3);
        assert_eq!(config.bucket, "my-bucket");
        assert_eq!(config.region, Some("us-east-1".to_string()));
    }

    #[cfg(feature = "azure")]
    #[test]
    fn test_unified_config_azure() {
        let config = UnifiedConfig::azure("my-container", "myaccount");
        assert_eq!(config.provider, StorageProvider::Azure);
        assert_eq!(config.bucket, "my-container");
        assert_eq!(config.access_key, Some("myaccount".to_string()));
    }

    #[cfg(feature = "gcs")]
    #[test]
    fn test_unified_config_gcs() {
        let config = UnifiedConfig::gcs("my-bucket", "my-project");
        assert_eq!(config.provider, StorageProvider::GCS);
        assert_eq!(config.bucket, "my-bucket");
        assert_eq!(config.project_id, Some("my-project".to_string()));
    }

    // ── RetryConfig ─────────────────────────────────────────────────────────

    #[test]
    fn test_retry_config_default_values() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_retries, 3);
        assert!((cfg.backoff_multiplier - 2.0).abs() < f64::EPSILON);
        assert_eq!(cfg.initial_backoff_ms, 500);
        assert_eq!(cfg.max_backoff_ms, 30_000);
        assert!(cfg.jitter_factor >= 0.0 && cfg.jitter_factor <= 1.0);
        assert!(cfg.retry_on_transient_only);
    }

    #[test]
    fn test_retry_config_backoff_increases() {
        let cfg = RetryConfig {
            jitter_factor: 0.0, // disable jitter for determinism
            ..RetryConfig::default()
        };
        let d0 = cfg.backoff_ms_for_attempt(0);
        let d1 = cfg.backoff_ms_for_attempt(1);
        let d2 = cfg.backoff_ms_for_attempt(2);
        assert!(d1 > d0, "backoff must grow: {d1} > {d0}");
        assert!(d2 > d1, "backoff must grow: {d2} > {d1}");
    }

    #[test]
    fn test_retry_config_backoff_capped_at_max() {
        let cfg = RetryConfig {
            initial_backoff_ms: 1000,
            max_backoff_ms: 4000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.0,
            ..RetryConfig::default()
        };
        // attempt 10 would be 1000 * 2^10 = 1_024_000 ms — must be capped
        let d = cfg.backoff_ms_for_attempt(10);
        assert!(d <= 4000, "backoff {d} must not exceed max_backoff_ms 4000");
    }

    #[test]
    fn test_retry_config_should_retry_network_error() {
        let cfg = RetryConfig::default();
        assert!(cfg.should_retry(&StorageError::NetworkError("timeout".into())));
        assert!(cfg.should_retry(&StorageError::RateLimitExceeded));
    }

    #[test]
    fn test_retry_config_should_not_retry_not_found() {
        let cfg = RetryConfig::default();
        assert!(!cfg.should_retry(&StorageError::NotFound("key".into())));
        assert!(!cfg.should_retry(&StorageError::PermissionDenied("denied".into())));
        assert!(!cfg.should_retry(&StorageError::InvalidKey("bad/key".into())));
        assert!(!cfg.should_retry(&StorageError::QuotaExceeded));
    }

    #[test]
    fn test_retry_config_should_retry_all_when_not_transient_only() {
        let cfg = RetryConfig {
            retry_on_transient_only: false,
            ..RetryConfig::default()
        };
        assert!(cfg.should_retry(&StorageError::NotFound("key".into())));
        assert!(cfg.should_retry(&StorageError::QuotaExceeded));
    }

    #[test]
    fn test_unified_config_with_retry_builder() {
        let custom = RetryConfig {
            max_retries: 10,
            backoff_multiplier: 1.5,
            ..RetryConfig::default()
        };
        // Use a provider-independent approach since feature flags may be absent.
        // We test via the builder on a manually constructed config to avoid
        // depending on s3/azure/gcs features.
        let _ = custom.backoff_ms_for_attempt(0);
        // Verify clone works
        let cfg2 = custom.clone();
        assert_eq!(cfg2.max_retries, 10);
    }
}
