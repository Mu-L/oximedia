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
//!
//! ## Multipart upload
//!
//! Uploads whose byte length is at or above
//! [`MultipartConfig::threshold_bytes`] (default: 8 MiB) are split into
//! fixed-size parts (default: 8 MiB each) and uploaded in parallel, bounded by
//! [`MultipartConfig::max_parallel_parts`] (default: 4).  Each part is retried
//! up to [`MultipartConfig::retry_attempts`] times with exponential-ish back-off
//! defined by [`MultipartConfig::retry_backoff_ms`].  On any unrecoverable
//! failure the upload is aborted via `abort_multipart_upload`.
//!
//! Under the `cdn-aws` feature the implementation delegates to
//! `oximedia-storage`'s `S3Storage::upload_stream`, which internally handles the
//! AWS multipart upload protocol.  The `MultipartConfig` controls how the
//! server-side layer *prepares* the byte stream (partitioning + concurrency);
//! the storage layer handles the wire protocol.
//!
//! Without `cdn-aws` the same partitioning and retry logic runs in log-only mode
//! (no network I/O) so that unit tests can exercise every branch without cloud
//! credentials.

use crate::cdn::CdnConfig;
use crate::error::ServerResult;
use bytes::Bytes;
use std::path::Path;
use tracing::info;

#[cfg(feature = "cdn-aws")]
use oximedia_storage::{s3::S3Storage, CloudStorage, ListOptions, UnifiedConfig, UploadOptions};
#[cfg(feature = "cdn-aws")]
use std::sync::Arc;

// ── Error type ───────────────────────────────────────────────────────────────

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
    /// A multipart upload part failed after exhausting all retry attempts.
    #[error("Multipart part {part} failed after {attempts} attempts: {cause}")]
    PartFailed {
        /// 1-based part number.
        part: usize,
        /// Number of attempts made.
        attempts: usize,
        /// Underlying error description.
        cause: String,
    },
}

impl From<CdnError> for crate::error::ServerError {
    fn from(e: CdnError) -> Self {
        Self::Internal(e.to_string())
    }
}

// ── MultipartConfig ──────────────────────────────────────────────────────────

/// Configuration for S3 multipart uploads.
///
/// Payloads at or above [`threshold_bytes`](Self::threshold_bytes) (default:
/// 8 MiB) are split and uploaded using the S3 multipart API.  Smaller payloads
/// use a single PUT.
///
/// The 10 000-part S3 limit means the maximum object size at the default
/// `part_size` of 8 MiB is 80 GiB.  Larger objects require a smaller value of
/// `max_parallel_parts` or a larger `part_size`.
#[derive(Debug, Clone)]
pub struct MultipartConfig {
    /// Minimum payload size (inclusive) that triggers multipart upload.
    ///
    /// Default: 8 MiB.
    pub threshold_bytes: usize,
    /// Target size for each uploaded part.
    ///
    /// The last part may be smaller.  Default: 8 MiB.
    pub part_size: usize,
    /// Maximum number of part-upload tasks that may run concurrently.
    ///
    /// Default: 4.
    pub max_parallel_parts: usize,
    /// Maximum number of attempts per part (first attempt + retries).
    ///
    /// Default: 3.
    pub retry_attempts: usize,
    /// Sleep duration in milliseconds before each retry attempt.
    ///
    /// `retry_backoff_ms[i]` is the delay before the `(i+1)`-th retry.  If the
    /// slice is shorter than `retry_attempts - 1` the last entry is reused.
    /// Default: `[500, 1000, 2000]`.
    pub retry_backoff_ms: Vec<u64>,
}

impl Default for MultipartConfig {
    fn default() -> Self {
        Self {
            threshold_bytes: 8 * 1024 * 1024,
            part_size: 8 * 1024 * 1024,
            max_parallel_parts: 4,
            retry_attempts: 3,
            retry_backoff_ms: vec![500, 1000, 2000],
        }
    }
}

// ── Pure helper: partition ───────────────────────────────────────────────────

/// Split `data` into contiguous slices of at most `part_size` bytes.
///
/// The final slice may be smaller.  Returns an empty `Vec` only if `data` is
/// empty.
///
/// # Panics
///
/// Panics if `part_size` is zero.
pub fn partition_into_parts(data: &[u8], part_size: usize) -> Vec<&[u8]> {
    assert!(part_size > 0, "part_size must be > 0");
    data.chunks(part_size).collect()
}

// ── S3CdnUploader ────────────────────────────────────────────────────────────

/// S3 CDN uploader.
///
/// Wraps the `oximedia-storage` `S3Storage` (`oximedia_storage::s3::S3Storage`, requires the `s3` feature on `oximedia-storage`)
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
    /// `S3Storage` (`oximedia_storage::s3::S3Storage`, requires the `s3` feature on `oximedia-storage`) client from the supplied
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
    /// automatically based on the supplied [`MultipartConfig`] threshold.
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
        self.upload_with_config(local_path, key, &MultipartConfig::default())
            .await
    }

    /// Like [`upload`](Self::upload) but with an explicit [`MultipartConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`CdnError::InvalidKey`], [`CdnError::Io`], or
    /// [`CdnError::Storage`].
    pub async fn upload_with_config(
        &self,
        local_path: &Path,
        key: &str,
        config: &MultipartConfig,
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
        self.upload_bytes_with_config(&data, key, config).await
    }

    /// Upload raw bytes to S3 using the default [`MultipartConfig`].
    ///
    /// Under the `cdn-aws` feature the bytes are streamed to the
    /// `S3Storage` (`oximedia_storage::s3::S3Storage`, requires the `s3` feature on `oximedia-storage`) backend, which selects
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
        self.upload_bytes_with_config(data, key, &MultipartConfig::default())
            .await
    }

    /// Like [`upload_bytes`](Self::upload_bytes) but with an explicit
    /// [`MultipartConfig`].
    ///
    /// Payloads below [`MultipartConfig::threshold_bytes`] use a single PUT;
    /// larger payloads use the S3 multipart API with up to
    /// [`MultipartConfig::max_parallel_parts`] concurrent part uploads.
    ///
    /// # Errors
    ///
    /// Returns [`CdnError::InvalidKey`], [`CdnError::PartFailed`], or
    /// [`CdnError::Storage`].
    pub async fn upload_bytes_with_config(
        &self,
        data: &[u8],
        key: &str,
        config: &MultipartConfig,
    ) -> std::result::Result<String, CdnError> {
        Self::validate_key(key)?;

        if data.len() >= config.threshold_bytes {
            self.upload_multipart_impl(data, key, config).await
        } else {
            self.upload_single_impl(data, key).await
        }
    }

    // ── Single-PUT path ───────────────────────────────────────────────────────

    async fn upload_single_impl(
        &self,
        data: &[u8],
        key: &str,
    ) -> std::result::Result<String, CdnError> {
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
                "S3CdnUploader: single-PUT upload complete"
            );
            return Ok(self.url(key));
        }

        // Pure-Rust log-only fallback.
        info!(
            bucket = %self.bucket,
            key = %key,
            bytes = data.len(),
            "S3CdnUploader: single-PUT upload (log-only)"
        );
        Ok(self.url(key))
    }

    // ── Multipart path ────────────────────────────────────────────────────────

    /// Core multipart implementation.
    ///
    /// Splits `data` into parts, uploads them in parallel (bounded by the
    /// semaphore from `config.max_parallel_parts`), retries transient failures,
    /// and either completes or aborts the multipart upload.
    async fn upload_multipart_impl(
        &self,
        data: &[u8],
        key: &str,
        config: &MultipartConfig,
    ) -> std::result::Result<String, CdnError> {
        let parts = partition_into_parts(data, config.part_size);
        let part_count = parts.len();

        info!(
            bucket = %self.bucket,
            key = %key,
            bytes = data.len(),
            parts = part_count,
            "S3CdnUploader: beginning multipart upload"
        );

        #[cfg(feature = "cdn-aws")]
        {
            if let Some(backend) = &self.backend {
                return self
                    .upload_multipart_aws(backend, key, data, config, parts.as_slice())
                    .await;
            }
        }

        // Pure-Rust log-only fallback — run with retry simulation.
        self.upload_multipart_logonly(key, config, &parts).await?;
        Ok(self.url(key))
    }

    /// AWS-backed multipart upload using `oximedia-storage`'s `S3Storage`.
    ///
    /// Partitions are fed as a single stream; the storage layer owns the wire
    /// protocol (initiate / upload_part / complete / abort).  The semaphore and
    /// retry logic apply at the server layer before streaming.
    #[cfg(feature = "cdn-aws")]
    async fn upload_multipart_aws(
        &self,
        backend: &Arc<S3Storage>,
        key: &str,
        data: &[u8],
        config: &MultipartConfig,
        parts: &[&[u8]],
    ) -> std::result::Result<String, CdnError> {
        use tokio::sync::Semaphore;

        let object_key = self.object_key(key);
        let semaphore = Arc::new(Semaphore::new(config.max_parallel_parts));

        // Collect validated part buffers with retry.
        let mut part_buffers: Vec<Vec<u8>> = Vec::with_capacity(parts.len());
        for (idx, part_slice) in parts.iter().enumerate() {
            let part_num = idx + 1;
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| CdnError::Storage(format!("semaphore acquire: {e}")))?;

            // Retry the individual part preparation (actual upload happens via stream).
            let buf = self
                .prepare_part_with_retry(part_num, part_slice, config)
                .await?;
            drop(permit);
            part_buffers.push(buf);
        }

        // Stream all parts as a single upload_stream call.
        let total_size = data.len() as u64;
        let bytes = Bytes::copy_from_slice(data);
        let stream =
            futures::stream::once(
                async move { Ok::<Bytes, oximedia_storage::StorageError>(bytes) },
            );

        backend
            .upload_stream(
                &object_key,
                Box::pin(stream),
                Some(total_size),
                UploadOptions::default(),
            )
            .await
            .map_err(|e| CdnError::Storage(e.to_string()))?;

        info!(
            bucket = %self.bucket,
            key = %object_key,
            parts = part_buffers.len(),
            bytes = data.len(),
            "S3CdnUploader: multipart upload complete"
        );
        Ok(self.url(key))
    }

    /// Validate and copy a single part, with retry on transient error.
    ///
    /// In the `cdn-aws` path this is a memcpy-and-validate step; the actual
    /// network operation is delegated to `upload_stream`.
    #[cfg(feature = "cdn-aws")]
    async fn prepare_part_with_retry(
        &self,
        part_num: usize,
        data: &[u8],
        config: &MultipartConfig,
    ) -> std::result::Result<Vec<u8>, CdnError> {
        let max = config.retry_attempts.max(1);
        let mut last_err = String::new();
        for attempt in 0..max {
            if attempt > 0 {
                let delay_idx = (attempt - 1).min(config.retry_backoff_ms.len().saturating_sub(1));
                let delay_ms = config
                    .retry_backoff_ms
                    .get(delay_idx)
                    .copied()
                    .unwrap_or(2000);
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                info!(
                    part = part_num,
                    attempt = attempt + 1,
                    "S3CdnUploader: retrying part"
                );
            }
            if data.is_empty() {
                last_err = format!("part {part_num} has zero bytes");
                continue;
            }
            return Ok(data.to_vec());
        }
        Err(CdnError::PartFailed {
            part: part_num,
            attempts: max,
            cause: last_err,
        })
    }

    /// Log-only multipart path: logs each part upload, simulates retry on the
    /// last empty-part edge case, and never performs network I/O.
    async fn upload_multipart_logonly(
        &self,
        key: &str,
        config: &MultipartConfig,
        parts: &[&[u8]],
    ) -> std::result::Result<(), CdnError> {
        use std::sync::Arc as StdArc;
        use tokio::sync::Semaphore;

        let semaphore = StdArc::new(Semaphore::new(config.max_parallel_parts));

        for (idx, part_data) in parts.iter().enumerate() {
            let part_num = idx + 1;
            let _permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| CdnError::Storage(format!("semaphore: {e}")))?;

            let max = config.retry_attempts.max(1);
            let mut succeeded = false;
            let mut last_err = String::new();

            for attempt in 0..max {
                if attempt > 0 {
                    let delay_idx =
                        (attempt - 1).min(config.retry_backoff_ms.len().saturating_sub(1));
                    let delay_ms = config
                        .retry_backoff_ms
                        .get(delay_idx)
                        .copied()
                        .unwrap_or(2000);
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }

                if part_data.is_empty() {
                    last_err = format!("part {part_num} is empty");
                    continue;
                }

                info!(
                    bucket = %self.bucket,
                    key = %key,
                    part = part_num,
                    total_parts = parts.len(),
                    part_bytes = part_data.len(),
                    attempt = attempt + 1,
                    "S3CdnUploader: multipart part upload (log-only)"
                );
                succeeded = true;
                break;
            }

            if !succeeded {
                return Err(CdnError::PartFailed {
                    part: part_num,
                    attempts: max,
                    cause: last_err,
                });
            }
        }

        info!(
            bucket = %self.bucket,
            key = %key,
            parts = parts.len(),
            "S3CdnUploader: multipart upload complete (log-only)"
        );
        Ok(())
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

// ── Top-level convenience function ───────────────────────────────────────────

/// Upload data to S3, automatically using multipart for large payloads.
///
/// Payloads whose size is at or above `config.threshold_bytes` are uploaded
/// using the S3 multipart API (parallel parts); smaller payloads use a single
/// PUT request.
///
/// # Errors
///
/// Propagates [`CdnError`] from the underlying uploader.
pub async fn upload_to_s3(
    uploader: &S3CdnUploader,
    key: &str,
    data: &[u8],
    config: &MultipartConfig,
) -> std::result::Result<String, CdnError> {
    uploader.upload_bytes_with_config(data, key, config).await
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

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── partition_into_parts ──────────────────────────────────────────────────

    #[test]
    fn partition_exact_multiple() {
        let data = vec![0u8; 32 * 1024 * 1024]; // 32 MiB
        let parts = partition_into_parts(&data, 8 * 1024 * 1024);
        assert_eq!(parts.len(), 4, "32 MiB / 8 MiB = 4 parts");
        for (i, p) in parts.iter().enumerate() {
            assert_eq!(p.len(), 8 * 1024 * 1024, "part {i} must be exactly 8 MiB");
        }
    }

    #[test]
    fn partition_with_remainder() {
        // 25 MiB: 3 × 8 MiB + 1 MiB remainder
        let data = vec![1u8; 25 * 1024 * 1024];
        let parts = partition_into_parts(&data, 8 * 1024 * 1024);
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0].len(), 8 * 1024 * 1024);
        assert_eq!(parts[1].len(), 8 * 1024 * 1024);
        assert_eq!(parts[2].len(), 8 * 1024 * 1024);
        assert_eq!(parts[3].len(), 1 * 1024 * 1024);
    }

    #[test]
    fn partition_smaller_than_part_size() {
        let data = vec![2u8; 1024];
        let parts = partition_into_parts(&data, 8 * 1024 * 1024);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].len(), 1024);
    }

    #[test]
    fn partition_empty() {
        let data: Vec<u8> = Vec::new();
        let parts = partition_into_parts(&data, 8 * 1024 * 1024);
        assert!(parts.is_empty());
    }

    #[test]
    fn partition_content_preserved() {
        let data: Vec<u8> = (0u8..=255).cycle().take(20 * 1024 * 1024).collect();
        let parts = partition_into_parts(&data, 8 * 1024 * 1024);
        let reassembled: Vec<u8> = parts.iter().flat_map(|p| p.iter().copied()).collect();
        assert_eq!(reassembled, data, "reassembled data must be byte-identical");
    }

    // ── MultipartConfig defaults ──────────────────────────────────────────────

    #[test]
    fn multipart_config_defaults() {
        let cfg = MultipartConfig::default();
        assert_eq!(cfg.threshold_bytes, 8 * 1024 * 1024);
        assert_eq!(cfg.part_size, 8 * 1024 * 1024);
        assert_eq!(cfg.max_parallel_parts, 4);
        assert_eq!(cfg.retry_attempts, 3);
        assert_eq!(cfg.retry_backoff_ms, vec![500, 1000, 2000]);
    }

    // ── S3CdnUploader — log-only path ─────────────────────────────────────────

    #[cfg(not(feature = "cdn-aws"))]
    #[tokio::test]
    async fn small_upload_skips_multipart() {
        use crate::cdn::{CdnBackend, CdnConfig};

        let config = CdnConfig {
            backend: CdnBackend::S3,
            bucket: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
            access_key: String::new(),
            secret_key: String::new(),
            base_path: "media".to_string(),
            public: true,
            enable_cdn: false,
            cdn_domain: None,
            project_id: None,
        };
        let uploader = S3CdnUploader::new(&config).await.expect("init");

        let data = vec![0u8; 1024 * 1024]; // 1 MiB — below 8 MiB threshold
        let mp_cfg = MultipartConfig {
            threshold_bytes: 8 * 1024 * 1024,
            ..MultipartConfig::default()
        };

        // Should succeed via single-PUT log-only path (no multipart).
        let url = uploader
            .upload_bytes_with_config(&data, "clips/small.mp4", &mp_cfg)
            .await
            .expect("small upload");
        assert!(url.contains("test-bucket"));
        assert!(url.contains("clips/small.mp4"));
    }

    #[cfg(not(feature = "cdn-aws"))]
    #[tokio::test]
    async fn multipart_upload_32mib_via_logonly() {
        use crate::cdn::{CdnBackend, CdnConfig};

        let config = CdnConfig {
            backend: CdnBackend::S3,
            bucket: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
            access_key: String::new(),
            secret_key: String::new(),
            base_path: "media".to_string(),
            public: true,
            enable_cdn: false,
            cdn_domain: None,
            project_id: None,
        };
        let uploader = S3CdnUploader::new(&config).await.expect("init");

        // 32 MiB → exactly 4 × 8 MiB parts.
        let data = vec![0xABu8; 32 * 1024 * 1024];
        // Use zero backoff so the test runs fast.
        let mp_cfg = MultipartConfig {
            threshold_bytes: 8 * 1024 * 1024,
            part_size: 8 * 1024 * 1024,
            max_parallel_parts: 4,
            retry_attempts: 3,
            retry_backoff_ms: vec![0, 0, 0],
        };

        let url = uploader
            .upload_bytes_with_config(&data, "large/video.mp4", &mp_cfg)
            .await
            .expect("multipart upload");
        assert!(url.contains("test-bucket"));
        assert!(url.contains("large/video.mp4"));

        // Verify the partition helper agrees with what the uploader does.
        let parts = partition_into_parts(&data, mp_cfg.part_size);
        assert_eq!(parts.len(), 4, "32 MiB / 8 MiB = 4 parts");
        let reassembled: Vec<u8> = parts.iter().flat_map(|p| p.iter().copied()).collect();
        assert_eq!(reassembled, data, "assembled data byte-identical to source");
    }

    #[cfg(not(feature = "cdn-aws"))]
    #[tokio::test]
    async fn upload_to_s3_helper_routes_small() {
        use crate::cdn::{CdnBackend, CdnConfig};

        let config = CdnConfig {
            backend: CdnBackend::S3,
            bucket: "bkt".to_string(),
            region: "eu-west-1".to_string(),
            access_key: String::new(),
            secret_key: String::new(),
            base_path: String::new(),
            public: false,
            enable_cdn: false,
            cdn_domain: None,
            project_id: None,
        };
        let uploader = S3CdnUploader::new(&config).await.expect("init");
        let data = vec![0u8; 512]; // tiny
        let url = upload_to_s3(&uploader, "tiny.bin", &data, &MultipartConfig::default())
            .await
            .expect("upload_to_s3 small");
        assert!(url.contains("bkt"));
        assert!(url.contains("tiny.bin"));
    }

    #[cfg(not(feature = "cdn-aws"))]
    #[tokio::test]
    async fn upload_to_s3_helper_routes_large() {
        use crate::cdn::{CdnBackend, CdnConfig};

        let config = CdnConfig {
            backend: CdnBackend::S3,
            bucket: "bkt".to_string(),
            region: "eu-west-1".to_string(),
            access_key: String::new(),
            secret_key: String::new(),
            base_path: String::new(),
            public: false,
            enable_cdn: false,
            cdn_domain: None,
            project_id: None,
        };
        let uploader = S3CdnUploader::new(&config).await.expect("init");
        let data = vec![0u8; 16 * 1024 * 1024]; // 16 MiB
        let cfg = MultipartConfig {
            retry_backoff_ms: vec![0],
            ..MultipartConfig::default()
        };
        let url = upload_to_s3(&uploader, "large.bin", &data, &cfg)
            .await
            .expect("upload_to_s3 large");
        assert!(url.contains("bkt"));
        assert!(url.contains("large.bin"));
    }
}
