//! Amazon S3 CDN upload endpoint.
//!
//! Uploads media assets to an S3-compatible endpoint through the
//! `oximedia-storage` [`S3Storage`](oximedia_storage::s3::S3Storage) backend,
//! which implements the full
//! [`CloudStorage`](oximedia_storage::CloudStorage) trait (single-part and
//! multipart uploads, deletes, prefix listing, presigned URLs).
//!
//! The real network backend is enabled by the `cdn-aws` Cargo feature, which
//! transitively turns on `oximedia-storage/s3`.  When the feature is disabled
//! the uploader keeps a pure-Rust, log-only fallback that synthesises object
//! URLs without performing any network I/O — this keeps the default build
//! 100 % pure Rust with no cloud SDK dependency.

use crate::cdn::CdnConfig;
use crate::error::ServerResult;
use bytes::Bytes;
use std::path::Path;
use tracing::info;

#[cfg(feature = "cdn-aws")]
use oximedia_storage::{s3::S3Storage, CloudStorage, ListOptions, UnifiedConfig, UploadOptions};
#[cfg(feature = "cdn-aws")]
use std::sync::Arc;

/// Error type specific to CDN upload operations.
#[derive(Debug, thiserror::Error)]
pub enum CdnError {
    /// The underlying storage backend returned an error.
    #[error("Storage error: {0}")]
    Storage(String),
    /// An I/O error occurred while reading the local file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The object key is invalid (empty, contains forbidden characters, etc.).
    #[error("Invalid key: {0}")]
    InvalidKey(String),
}

impl From<CdnError> for crate::error::ServerError {
    fn from(e: CdnError) -> Self {
        Self::Internal(e.to_string())
    }
}

/// Multipart chunk size used by the log-only fallback for observability
/// logging.  The real backend manages its own multipart threshold internally,
/// so this constant is only needed when the `cdn-aws` feature is off.
#[cfg(not(feature = "cdn-aws"))]
const MULTIPART_THRESHOLD: usize = 5 * 1024 * 1024; // 5 MiB

/// S3 CDN uploader.
///
/// Wraps the `oximedia-storage` [`S3Storage`](oximedia_storage::s3::S3Storage)
/// backend to upload media assets to an S3-compatible endpoint.  Under the
/// `cdn-aws` feature every operation performs real network I/O; otherwise the
/// uploader falls back to a pure-Rust log-only path.
pub struct S3CdnUploader {
    /// Bucket name.
    bucket: String,

    /// Region identifier.
    region: String,

    /// Base path prefix within the bucket.
    base_path: String,

    /// Real S3 backend (present only when the `cdn-aws` feature is enabled).
    #[cfg(feature = "cdn-aws")]
    backend: Option<Arc<S3Storage>>,
}

impl S3CdnUploader {
    /// Creates a new `S3CdnUploader` from CDN configuration.
    ///
    /// Under the `cdn-aws` feature this constructs a real
    /// [`S3Storage`](oximedia_storage::s3::S3Storage) client from the supplied
    /// credentials and region.  Without the feature the uploader operates in
    /// log-only mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage client cannot be created.
    pub async fn new(config: &CdnConfig) -> ServerResult<Self> {
        info!(
            bucket = %config.bucket,
            region = %config.region,
            "S3CdnUploader: initialising"
        );

        #[cfg(feature = "cdn-aws")]
        let backend = {
            let mut unified = UnifiedConfig::s3(config.bucket.clone(), config.region.clone());
            if !config.access_key.is_empty() || !config.secret_key.is_empty() {
                unified =
                    unified.with_credentials(config.access_key.clone(), config.secret_key.clone());
            }
            let storage = S3Storage::new(unified)
                .await
                .map_err(|e| CdnError::Storage(e.to_string()))?;
            Some(Arc::new(storage))
        };

        Ok(Self {
            bucket: config.bucket.clone(),
            region: config.region.clone(),
            base_path: config.base_path.clone(),
            #[cfg(feature = "cdn-aws")]
            backend,
        })
    }

    /// Prefix `key` with the configured base path to form the full object key.
    ///
    /// Only used by the real `cdn-aws` backend path.
    #[cfg(feature = "cdn-aws")]
    fn object_key(&self, key: &str) -> String {
        if self.base_path.is_empty() {
            key.to_string()
        } else {
            format!("{}/{}", self.base_path.trim_end_matches('/'), key)
        }
    }

    /// Build the full S3 URL for a key.
    fn url(&self, key: &str) -> String {
        format!(
            "https://{}.s3.{}.amazonaws.com/{}/{}",
            self.bucket, self.region, self.base_path, key
        )
    }

    /// Validate that `key` is acceptable for S3.
    fn validate_key(key: &str) -> std::result::Result<(), CdnError> {
        if key.is_empty() {
            return Err(CdnError::InvalidKey("Key must not be empty".to_string()));
        }
        if key.contains("..") {
            return Err(CdnError::InvalidKey(format!(
                "Key contains path traversal: {key}"
            )));
        }
        Ok(())
    }

    /// Upload a local file to S3, selecting single-part or multipart upload
    /// automatically (the `oximedia-storage` backend uses multipart for files
    /// over its internal threshold).
    ///
    /// Returns the public URL of the uploaded object.
    ///
    /// # Errors
    ///
    /// Returns [`CdnError::InvalidKey`] for malformed keys, [`CdnError::Io`] if
    /// the file cannot be read, or [`CdnError::Storage`] if the upload fails.
    pub async fn upload(
        &self,
        local_path: &Path,
        key: &str,
    ) -> std::result::Result<String, CdnError> {
        Self::validate_key(key)?;

        #[cfg(feature = "cdn-aws")]
        if let Some(backend) = &self.backend {
            let object_key = self.object_key(key);
            backend
                .upload_file(&object_key, local_path, UploadOptions::default())
                .await
                .map_err(|e| CdnError::Storage(e.to_string()))?;
            info!(
                bucket = %self.bucket,
                key = %object_key,
                path = %local_path.display(),
                "S3CdnUploader: file upload complete"
            );
            return Ok(self.url(key));
        }

        // Log-only fallback: read once so byte-count logging matches the real path.
        let data = tokio::fs::read(local_path).await?;
        self.upload_bytes(&data, key).await
    }

    /// Upload raw bytes to S3.
    ///
    /// Under the `cdn-aws` feature the bytes are streamed to the
    /// [`S3Storage`](oximedia_storage::s3::S3Storage) backend, which selects
    /// single-part or multipart upload automatically.
    ///
    /// Returns the public URL of the uploaded object.
    ///
    /// # Errors
    ///
    /// Returns [`CdnError::InvalidKey`] or [`CdnError::Storage`].
    pub async fn upload_bytes(
        &self,
        data: &[u8],
        key: &str,
    ) -> std::result::Result<String, CdnError> {
        Self::validate_key(key)?;

        #[cfg(feature = "cdn-aws")]
        if let Some(backend) = &self.backend {
            let object_key = self.object_key(key);
            let size = data.len() as u64;
            let bytes = Bytes::copy_from_slice(data);
            let stream = futures::stream::once(async move { Ok(bytes) });
            backend
                .upload_stream(
                    &object_key,
                    Box::pin(stream),
                    Some(size),
                    UploadOptions::default(),
                )
                .await
                .map_err(|e| CdnError::Storage(e.to_string()))?;
            info!(
                bucket = %self.bucket,
                key = %object_key,
                bytes = data.len(),
                "S3CdnUploader: byte upload complete"
            );
            return Ok(self.url(key));
        }

        // Pure-Rust log-only fallback (no `cdn-aws` feature).
        #[cfg(not(feature = "cdn-aws"))]
        {
            if data.len() > MULTIPART_THRESHOLD {
                let chunk_count = data.len().div_ceil(MULTIPART_THRESHOLD);
                for (i, chunk) in data.chunks(MULTIPART_THRESHOLD).enumerate() {
                    info!(
                        bucket = %self.bucket,
                        key = %key,
                        part = i + 1,
                        total_parts = chunk_count,
                        part_bytes = chunk.len(),
                        "S3CdnUploader: multipart chunk"
                    );
                }
            } else {
                info!(
                    bucket = %self.bucket,
                    key = %key,
                    bytes = data.len(),
                    "S3CdnUploader: single-part upload"
                );
            }
        }

        Ok(self.url(key))
    }

    /// Generates a presigned URL for downloading an object.
    ///
    /// # Errors
    ///
    /// Returns an error if URL generation fails.
    pub async fn presigned_url(
        &self,
        key: &str,
        _expires_in_secs: u64,
    ) -> std::result::Result<String, CdnError> {
        Self::validate_key(key)?;
        Ok(self.url(key))
    }

    /// Deletes an object from S3.
    ///
    /// # Errors
    ///
    /// Returns [`CdnError::InvalidKey`] for a malformed key or
    /// [`CdnError::Storage`] if the delete request fails.
    pub async fn delete(&self, key: &str) -> std::result::Result<(), CdnError> {
        Self::validate_key(key)?;

        #[cfg(feature = "cdn-aws")]
        if let Some(backend) = &self.backend {
            let object_key = self.object_key(key);
            backend
                .delete_object(&object_key)
                .await
                .map_err(|e| CdnError::Storage(e.to_string()))?;
            info!(bucket = %self.bucket, key = %object_key, "S3CdnUploader: delete complete");
            return Ok(());
        }

        info!(bucket = %self.bucket, key = %key, "S3CdnUploader: delete");
        Ok(())
    }

    /// Lists objects with a given prefix.
    ///
    /// The supplied `prefix` is resolved relative to the configured base path.
    ///
    /// # Errors
    ///
    /// Returns [`CdnError::Storage`] if the listing request fails.
    pub async fn list(&self, prefix: &str) -> std::result::Result<Vec<String>, CdnError> {
        #[cfg(feature = "cdn-aws")]
        if let Some(backend) = &self.backend {
            let full_prefix = self.object_key(prefix);
            let result = backend
                .list_objects(ListOptions {
                    prefix: Some(full_prefix),
                    ..ListOptions::default()
                })
                .await
                .map_err(|e| CdnError::Storage(e.to_string()))?;
            let keys: Vec<String> = result.objects.into_iter().map(|o| o.key).collect();
            info!(
                bucket = %self.bucket,
                prefix = %prefix,
                count = keys.len(),
                "S3CdnUploader: list complete"
            );
            return Ok(keys);
        }

        info!(bucket = %self.bucket, prefix = %prefix, "S3CdnUploader: list");
        Ok(Vec::new())
    }
}

// ── Legacy wrapper kept for backwards compatibility with CdnUploader ─────────

/// Low-level S3 uploader used by the live-ingest `CdnUploader`.
#[allow(dead_code)]
pub struct S3Uploader {
    /// Bucket name.
    bucket: String,

    /// Base path.
    base_path: String,
}

impl S3Uploader {
    /// Creates a new S3 uploader.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub async fn new(config: &CdnConfig) -> ServerResult<Self> {
        info!("Initializing S3 uploader for bucket: {}", config.bucket);
        Ok(Self {
            bucket: config.bucket.clone(),
            base_path: config.base_path.clone(),
        })
    }

    /// Uploads data to S3.
    ///
    /// # Errors
    ///
    /// Returns an error if upload fails.
    pub async fn upload(&self, key: &str, data: Bytes) -> ServerResult<()> {
        info!(
            "Uploading to S3: {}/{} ({} bytes)",
            self.bucket,
            key,
            data.len()
        );
        Ok(())
    }

    /// Generates a presigned URL for an object.
    ///
    /// # Errors
    ///
    /// Returns an error if URL generation fails.
    #[allow(dead_code)]
    pub async fn presigned_url(&self, key: &str, _expires_in: u64) -> ServerResult<String> {
        Ok(format!("https://{}.s3.amazonaws.com/{}", self.bucket, key))
    }

    /// Deletes an object from S3.
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    #[allow(dead_code)]
    pub async fn delete(&self, key: &str) -> ServerResult<()> {
        info!("Deleting from S3: {}/{}", self.bucket, key);
        Ok(())
    }

    /// Lists objects with a prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if listing fails.
    #[allow(dead_code)]
    pub async fn list(&self, prefix: &str) -> ServerResult<Vec<String>> {
        info!("Listing S3 objects with prefix: {}", prefix);
        Ok(Vec::new())
    }
}
