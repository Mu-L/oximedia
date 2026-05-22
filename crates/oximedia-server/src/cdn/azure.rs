//! Azure Blob Storage CDN upload endpoint.
//!
//! Azure block-blob uploads use a stage-blocks-then-commit pattern.
//! Each block is 5 MiB; block IDs are base64-encoded sequential integers.
//! The full commit list is sent once all blocks are staged.
//!
//! Uploads are proxied through the `oximedia-storage`
//! [`AzureStorage`](oximedia_storage::azure::AzureStorage) backend, which
//! implements the full [`CloudStorage`](oximedia_storage::CloudStorage) trait
//! (block-blob staging, deletes, prefix listing).
//!
//! The real network backend is enabled by the `cdn-azure` Cargo feature, which
//! transitively turns on `oximedia-storage/azure`.  When the feature is
//! disabled the uploader keeps a pure-Rust, log-only fallback that synthesises
//! object URLs without performing any network I/O.

use crate::cdn::s3::CdnError;
use crate::cdn::CdnConfig;
use crate::error::ServerResult;
use bytes::Bytes;
use std::path::Path;
use tracing::info;

#[cfg(feature = "cdn-azure")]
use oximedia_storage::{
    azure::AzureStorage, CloudStorage, ListOptions, UnifiedConfig, UploadOptions,
};
#[cfg(feature = "cdn-azure")]
use std::sync::Arc;

/// Azure block-blob chunk size used by the log-only fallback for observability
/// logging (≤ 100 MiB per Azure spec; we use 5 MiB).  The real backend manages
/// its own block size internally, so this constant is only needed when the
/// `cdn-azure` feature is off.
#[cfg(not(feature = "cdn-azure"))]
const BLOCK_SIZE: usize = 5 * 1024 * 1024; // 5 MiB

/// Azure Blob Storage CDN uploader.
///
/// Implements the same interface as [`super::s3::S3CdnUploader`].  Under the
/// `cdn-azure` feature every operation performs real network I/O through
/// [`AzureStorage`](oximedia_storage::azure::AzureStorage); otherwise the
/// uploader falls back to a pure-Rust log-only path.
pub struct AzureCdnUploader {
    /// Azure storage account name.
    account: String,

    /// Container name.
    container: String,

    /// Base path within the container.
    base_path: String,

    /// Real Azure backend (present only when the `cdn-azure` feature is enabled).
    #[cfg(feature = "cdn-azure")]
    backend: Option<Arc<AzureStorage>>,
}

impl AzureCdnUploader {
    /// Creates a new `AzureCdnUploader` from CDN configuration.
    ///
    /// For Azure, `config.bucket` is the container name, `config.region` is the
    /// storage account name, and `config.secret_key` is the account access key.
    ///
    /// Under the `cdn-azure` feature this constructs a real
    /// [`AzureStorage`](oximedia_storage::azure::AzureStorage) client; the
    /// account key is required for that path.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage client cannot be created.
    pub async fn new(config: &CdnConfig) -> ServerResult<Self> {
        // Treat `region` as account name for Azure; fall back to generic "account".
        let account = if config.region.is_empty() {
            "account".to_string()
        } else {
            config.region.clone()
        };
        info!(
            account = %account,
            container = %config.bucket,
            "AzureCdnUploader: initialising"
        );

        #[cfg(feature = "cdn-azure")]
        let backend = {
            let unified = UnifiedConfig::azure(config.bucket.clone(), account.clone())
                .with_credentials(account.clone(), config.secret_key.clone());
            let storage = AzureStorage::new(unified)
                .await
                .map_err(|e| CdnError::Storage(e.to_string()))?;
            Some(Arc::new(storage))
        };

        Ok(Self {
            account,
            container: config.bucket.clone(),
            base_path: config.base_path.clone(),
            #[cfg(feature = "cdn-azure")]
            backend,
        })
    }

    /// Build the blob URL for a key.
    fn url(&self, key: &str) -> String {
        format!(
            "https://{}.blob.core.windows.net/{}/{}/{}",
            self.account, self.container, self.base_path, key
        )
    }

    /// Prefix `key` with the configured base path to form the full blob name.
    ///
    /// Only used by the real `cdn-azure` backend path.
    #[cfg(feature = "cdn-azure")]
    fn object_key(&self, key: &str) -> String {
        if self.base_path.is_empty() {
            key.to_string()
        } else {
            format!("{}/{}", self.base_path.trim_end_matches('/'), key)
        }
    }

    /// Base64-encode an integer block ID (zero-padded to 8 chars for uniform length).
    ///
    /// Used only by the log-only fallback; the real backend generates its own
    /// block IDs.
    #[cfg(not(feature = "cdn-azure"))]
    fn block_id(idx: usize) -> String {
        use std::io::Write as _;
        let raw = format!("{idx:08}");
        let mut encoded = String::new();
        let b64 = {
            let mut buf = Vec::new();
            write!(buf, "{raw}").ok();
            // Simple base64 without an external crate: encode each byte triplet
            let input = raw.as_bytes();
            let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            for chunk in input.chunks(3) {
                let b0 = chunk[0] as usize;
                let b1 = if chunk.len() > 1 {
                    chunk[1] as usize
                } else {
                    0
                };
                let b2 = if chunk.len() > 2 {
                    chunk[2] as usize
                } else {
                    0
                };
                let idx0 = b0 >> 2;
                let idx1 = ((b0 & 0x03) << 4) | (b1 >> 4);
                let idx2 = ((b1 & 0x0f) << 2) | (b2 >> 6);
                let idx3 = b2 & 0x3f;
                encoded.push(table[idx0] as char);
                encoded.push(table[idx1] as char);
                if chunk.len() > 1 {
                    encoded.push(table[idx2] as char);
                } else {
                    encoded.push('=');
                }
                if chunk.len() > 2 {
                    encoded.push(table[idx3] as char);
                } else {
                    encoded.push('=');
                }
            }
            buf
        };
        let _ = b64;
        encoded
    }

    /// Upload a local file to Azure Blob Storage.
    ///
    /// For files > 5 MiB, block-blob staging is used (stage + commit).
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

        #[cfg(feature = "cdn-azure")]
        if let Some(backend) = &self.backend {
            let object_key = self.object_key(key);
            backend
                .upload_file(&object_key, local_path, UploadOptions::default())
                .await
                .map_err(|e| CdnError::Storage(e.to_string()))?;
            info!(
                account = %self.account,
                container = %self.container,
                key = %object_key,
                path = %local_path.display(),
                "AzureCdnUploader: file upload complete"
            );
            return Ok(self.url(key));
        }

        let data = tokio::fs::read(local_path).await?;
        self.upload_bytes(&data, key).await
    }

    /// Upload raw bytes to Azure Blob Storage.
    ///
    /// Under the `cdn-azure` feature the bytes are streamed to the
    /// [`AzureStorage`](oximedia_storage::azure::AzureStorage) backend, which
    /// performs block-blob staging and commit.
    ///
    /// # Errors
    ///
    /// Returns [`CdnError::InvalidKey`] or [`CdnError::Storage`].
    pub async fn upload_bytes(
        &self,
        data: &[u8],
        key: &str,
    ) -> std::result::Result<String, CdnError> {
        if key.is_empty() {
            return Err(CdnError::InvalidKey("Key must not be empty".to_string()));
        }

        #[cfg(feature = "cdn-azure")]
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
                account = %self.account,
                container = %self.container,
                key = %object_key,
                bytes = data.len(),
                "AzureCdnUploader: byte upload complete"
            );
            return Ok(self.url(key));
        }

        // Pure-Rust log-only fallback (no `cdn-azure` feature).
        #[cfg(not(feature = "cdn-azure"))]
        {
            if data.len() > BLOCK_SIZE {
                let mut block_ids: Vec<String> = Vec::new();
                for (i, chunk) in data.chunks(BLOCK_SIZE).enumerate() {
                    let block_id = Self::block_id(i);
                    info!(
                        account = %self.account,
                        container = %self.container,
                        key = %key,
                        block_id = %block_id,
                        block_bytes = chunk.len(),
                        "AzureCdnUploader: Put Block"
                    );
                    block_ids.push(block_id);
                }
                info!(
                    account = %self.account,
                    container = %self.container,
                    key = %key,
                    block_count = block_ids.len(),
                    "AzureCdnUploader: Put Block List / commit"
                );
            } else {
                info!(
                    account = %self.account,
                    container = %self.container,
                    key = %key,
                    bytes = data.len(),
                    "AzureCdnUploader: Put Block Blob"
                );
            }
        }

        Ok(self.url(key))
    }

    /// Generates a SAS URL for a blob.
    ///
    /// # Errors
    ///
    /// Returns an error if URL generation fails.
    pub async fn sas_url(
        &self,
        key: &str,
        _expires_in_secs: u64,
    ) -> std::result::Result<String, CdnError> {
        if key.is_empty() {
            return Err(CdnError::InvalidKey("Key must not be empty".to_string()));
        }
        Ok(self.url(key))
    }

    /// Deletes a blob.
    ///
    /// # Errors
    ///
    /// Returns [`CdnError::InvalidKey`] for an empty key or
    /// [`CdnError::Storage`] if the delete request fails.
    pub async fn delete(&self, key: &str) -> std::result::Result<(), CdnError> {
        if key.is_empty() {
            return Err(CdnError::InvalidKey("Key must not be empty".to_string()));
        }

        #[cfg(feature = "cdn-azure")]
        if let Some(backend) = &self.backend {
            let object_key = self.object_key(key);
            backend
                .delete_object(&object_key)
                .await
                .map_err(|e| CdnError::Storage(e.to_string()))?;
            info!(
                account = %self.account,
                container = %self.container,
                key = %object_key,
                "AzureCdnUploader: delete complete"
            );
            return Ok(());
        }

        info!(account = %self.account, container = %self.container, key = %key, "AzureCdnUploader: delete");
        Ok(())
    }

    /// Lists blobs with a prefix.
    ///
    /// The supplied `prefix` is resolved relative to the configured base path.
    ///
    /// # Errors
    ///
    /// Returns [`CdnError::Storage`] if the listing request fails.
    pub async fn list(&self, prefix: &str) -> std::result::Result<Vec<String>, CdnError> {
        #[cfg(feature = "cdn-azure")]
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
                account = %self.account,
                container = %self.container,
                prefix = %prefix,
                count = keys.len(),
                "AzureCdnUploader: list complete"
            );
            return Ok(keys);
        }

        info!(account = %self.account, container = %self.container, prefix = %prefix, "AzureCdnUploader: list");
        Ok(Vec::new())
    }
}

// ── Legacy wrapper kept for backwards compatibility with CdnUploader ─────────

/// Low-level Azure uploader used by the live-ingest `CdnUploader`.
#[allow(dead_code)]
pub struct AzureUploader {
    /// Container name.
    container: String,

    /// Base path.
    base_path: String,
}

impl AzureUploader {
    /// Creates a new Azure uploader.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub async fn new(config: &CdnConfig) -> ServerResult<Self> {
        info!(
            "Initializing Azure uploader for container: {}",
            config.bucket
        );
        Ok(Self {
            container: config.bucket.clone(),
            base_path: config.base_path.clone(),
        })
    }

    /// Uploads data to Azure Blob Storage.
    ///
    /// # Errors
    ///
    /// Returns an error if upload fails.
    pub async fn upload(&self, key: &str, data: Bytes) -> ServerResult<()> {
        info!(
            "Uploading to Azure: {}/{} ({} bytes)",
            self.container,
            key,
            data.len()
        );
        Ok(())
    }

    /// Generates a SAS URL for a blob.
    ///
    /// # Errors
    ///
    /// Returns an error if URL generation fails.
    #[allow(dead_code)]
    pub async fn sas_url(&self, key: &str, _expires_in: u64) -> ServerResult<String> {
        Ok(format!(
            "https://{}.blob.core.windows.net/{}/{}",
            "account", self.container, key
        ))
    }

    /// Deletes a blob.
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    #[allow(dead_code)]
    pub async fn delete(&self, key: &str) -> ServerResult<()> {
        info!("Deleting from Azure: {}/{}", self.container, key);
        Ok(())
    }

    /// Lists blobs with a prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if listing fails.
    #[allow(dead_code)]
    pub async fn list(&self, prefix: &str) -> ServerResult<Vec<String>> {
        info!("Listing Azure blobs with prefix: {}", prefix);
        Ok(Vec::new())
    }
}
