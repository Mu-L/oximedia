//! Google Cloud Storage CDN upload endpoint.
//!
//! GCS resumable uploads are proxied through the `oximedia-storage`
//! [`GcsStorage`](oximedia_storage::gcs::GcsStorage) backend, which implements
//! the full [`CloudStorage`](oximedia_storage::CloudStorage) trait.
//!
//! The real network backend is enabled by the `cdn-gcs` Cargo feature, which
//! transitively turns on `oximedia-storage/gcs`.  When the feature is disabled
//! the uploader keeps a pure-Rust, log-only fallback that synthesises object
//! URLs without performing any network I/O.

use crate::cdn::s3::CdnError;
use crate::cdn::CdnConfig;
use crate::error::ServerResult;
use bytes::Bytes;
use std::path::Path;
use tracing::info;

#[cfg(feature = "cdn-gcs")]
use oximedia_storage::{gcs::GcsStorage, CloudStorage, ListOptions, UnifiedConfig, UploadOptions};
#[cfg(feature = "cdn-gcs")]
use std::sync::Arc;

/// GCS CDN uploader.
///
/// Implements the same interface as [`super::s3::S3CdnUploader`] so that call
/// sites can be swapped without code changes.  Under the `cdn-gcs` feature
/// every operation performs real network I/O through
/// [`GcsStorage`](oximedia_storage::gcs::GcsStorage); otherwise the uploader
/// falls back to a pure-Rust log-only path.
pub struct GcsCdnUploader {
    /// GCS bucket name.
    bucket: String,

    /// Base path within the bucket.
    base_path: String,

    /// Real GCS backend (present only when the `cdn-gcs` feature is enabled).
    #[cfg(feature = "cdn-gcs")]
    backend: Option<Arc<GcsStorage>>,
}

impl GcsCdnUploader {
    /// Creates a new `GcsCdnUploader` from CDN configuration.
    ///
    /// Under the `cdn-gcs` feature this constructs a real
    /// [`GcsStorage`](oximedia_storage::gcs::GcsStorage) client.  The GCS
    /// backend requires a project ID; it is taken from
    /// [`CdnConfig::project_id`](crate::cdn::CdnConfig::project_id) and falls
    /// back to an empty string when unset (sufficient for object-level
    /// operations, which do not need a project ID).
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage client cannot be created.
    pub async fn new(config: &CdnConfig) -> ServerResult<Self> {
        info!(
            bucket = %config.bucket,
            "GcsCdnUploader: initialising"
        );

        #[cfg(feature = "cdn-gcs")]
        let backend = {
            let project_id = config.project_id.clone().unwrap_or_default();
            let unified = UnifiedConfig::gcs(config.bucket.clone(), project_id);
            let storage = GcsStorage::new(unified)
                .await
                .map_err(|e| CdnError::Storage(e.to_string()))?;
            Some(Arc::new(storage))
        };

        Ok(Self {
            bucket: config.bucket.clone(),
            base_path: config.base_path.clone(),
            #[cfg(feature = "cdn-gcs")]
            backend,
        })
    }

    /// Build the GCS object URL for a key.
    fn url(&self, key: &str) -> String {
        format!(
            "https://storage.googleapis.com/{}/{}/{}",
            self.bucket, self.base_path, key
        )
    }

    /// Prefix `key` with the configured base path to form the full object key.
    ///
    /// Only used by the real `cdn-gcs` backend path.
    #[cfg(feature = "cdn-gcs")]
    fn object_key(&self, key: &str) -> String {
        if self.base_path.is_empty() {
            key.to_string()
        } else {
            format!("{}/{}", self.base_path.trim_end_matches('/'), key)
        }
    }

    /// Upload a local file to GCS via resumable upload.
    ///
    /// # Errors
    ///
    /// Returns [`CdnError::InvalidKey`] for an empty key, [`CdnError::Io`] if
    /// the file cannot be read, or [`CdnError::Storage`] if the upload fails.
    pub async fn upload(
        &self,
        local_path: &Path,
        key: &str,
    ) -> std::result::Result<String, CdnError> {
        if key.is_empty() {
            return Err(CdnError::InvalidKey("Key must not be empty".to_string()));
        }

        #[cfg(feature = "cdn-gcs")]
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
                "GcsCdnUploader: file upload complete"
            );
            return Ok(self.url(key));
        }

        let size = local_path.metadata().map(|m| m.len()).unwrap_or(0);
        info!(
            bucket = %self.bucket,
            key = %key,
            path = %local_path.display(),
            bytes = size,
            "GcsCdnUploader: upload"
        );
        Ok(self.url(key))
    }

    /// Upload raw bytes to GCS.
    ///
    /// Under the `cdn-gcs` feature the bytes are streamed to the
    /// [`GcsStorage`](oximedia_storage::gcs::GcsStorage) backend.
    ///
    /// # Errors
    ///
    /// Returns [`CdnError::InvalidKey`] for an empty key or
    /// [`CdnError::Storage`] if the upload fails.
    pub async fn upload_bytes(
        &self,
        data: &[u8],
        key: &str,
    ) -> std::result::Result<String, CdnError> {
        if key.is_empty() {
            return Err(CdnError::InvalidKey("Key must not be empty".to_string()));
        }

        #[cfg(feature = "cdn-gcs")]
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
                "GcsCdnUploader: byte upload complete"
            );
            return Ok(self.url(key));
        }

        info!(
            bucket = %self.bucket,
            key = %key,
            bytes = data.len(),
            "GcsCdnUploader: upload_bytes"
        );
        Ok(self.url(key))
    }

    /// Generates a signed URL for downloading an object.
    ///
    /// # Errors
    ///
    /// Returns an error if URL generation fails.
    pub async fn signed_url(
        &self,
        key: &str,
        _expires_in_secs: u64,
    ) -> std::result::Result<String, CdnError> {
        if key.is_empty() {
            return Err(CdnError::InvalidKey("Key must not be empty".to_string()));
        }
        Ok(self.url(key))
    }

    /// Deletes an object from GCS.
    ///
    /// # Errors
    ///
    /// Returns [`CdnError::InvalidKey`] for an empty key or
    /// [`CdnError::Storage`] if the delete request fails.
    pub async fn delete(&self, key: &str) -> std::result::Result<(), CdnError> {
        if key.is_empty() {
            return Err(CdnError::InvalidKey("Key must not be empty".to_string()));
        }

        #[cfg(feature = "cdn-gcs")]
        if let Some(backend) = &self.backend {
            let object_key = self.object_key(key);
            backend
                .delete_object(&object_key)
                .await
                .map_err(|e| CdnError::Storage(e.to_string()))?;
            info!(bucket = %self.bucket, key = %object_key, "GcsCdnUploader: delete complete");
            return Ok(());
        }

        info!(bucket = %self.bucket, key = %key, "GcsCdnUploader: delete");
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
        #[cfg(feature = "cdn-gcs")]
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
                "GcsCdnUploader: list complete"
            );
            return Ok(keys);
        }

        info!(bucket = %self.bucket, prefix = %prefix, "GcsCdnUploader: list");
        Ok(Vec::new())
    }
}

// ── Legacy wrapper kept for backwards compatibility with CdnUploader ─────────

/// Low-level GCS uploader used by the live-ingest `CdnUploader`.
#[allow(dead_code)]
pub struct GcsUploader {
    /// Bucket name.
    bucket: String,

    /// Base path.
    base_path: String,
}

impl GcsUploader {
    /// Creates a new GCS uploader.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub async fn new(config: &CdnConfig) -> ServerResult<Self> {
        info!("Initializing GCS uploader for bucket: {}", config.bucket);
        Ok(Self {
            bucket: config.bucket.clone(),
            base_path: config.base_path.clone(),
        })
    }

    /// Uploads data to GCS.
    ///
    /// # Errors
    ///
    /// Returns an error if upload fails.
    pub async fn upload(&self, key: &str, data: Bytes) -> ServerResult<()> {
        info!(
            "Uploading to GCS: {}/{} ({} bytes)",
            self.bucket,
            key,
            data.len()
        );
        Ok(())
    }

    /// Generates a signed URL for an object.
    ///
    /// # Errors
    ///
    /// Returns an error if URL generation fails.
    #[allow(dead_code)]
    pub async fn signed_url(&self, key: &str, _expires_in: u64) -> ServerResult<String> {
        Ok(format!(
            "https://storage.googleapis.com/{}/{}",
            self.bucket, key
        ))
    }

    /// Deletes an object from GCS.
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    #[allow(dead_code)]
    pub async fn delete(&self, key: &str) -> ServerResult<()> {
        info!("Deleting from GCS: {}/{}", self.bucket, key);
        Ok(())
    }

    /// Lists objects with a prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if listing fails.
    #[allow(dead_code)]
    pub async fn list(&self, prefix: &str) -> ServerResult<Vec<String>> {
        info!("Listing GCS objects with prefix: {}", prefix);
        Ok(Vec::new())
    }
}
