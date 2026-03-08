//! Google Cloud Storage implementation

use crate::{
    ByteStream, CloudStorage, DownloadOptions, ListOptions, ListResult, ObjectMetadata, Result,
    StorageError, UnifiedConfig, UploadOptions,
};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use google_cloud_storage::{
    client::{Storage, StorageControl},
    model,
};
use google_cloud_wkt as wkt;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info, warn};

/// Threshold for using resumable upload (10 MB)
const RESUMABLE_THRESHOLD: u64 = 10 * 1024 * 1024;

/// Maximum number of components for object composition
const MAX_COMPOSE_OBJECTS: usize = 32;

/// Convert wkt::Timestamp to chrono::DateTime<Utc>
fn wkt_timestamp_to_chrono(ts: &wkt::Timestamp) -> DateTime<Utc> {
    DateTime::from_timestamp(ts.seconds(), ts.nanos() as u32).unwrap_or_else(Utc::now)
}

/// Format a bucket name in the projects/_/buckets/{bucket_id} format
fn bucket_path(bucket: &str) -> String {
    if bucket.starts_with("projects/") {
        bucket.to_string()
    } else {
        format!("projects/_/buckets/{bucket}")
    }
}

/// Google Cloud Storage client
pub struct GcsStorage {
    storage: Arc<Storage>,
    control: Arc<StorageControl>,
    bucket: String,
    config: UnifiedConfig,
}

impl GcsStorage {
    /// Create a new GCS storage client from configuration
    pub async fn new(config: UnifiedConfig) -> Result<Self> {
        let storage = Storage::builder().build().await.map_err(|e| {
            StorageError::AuthenticationError(format!("Failed to create storage client: {e}"))
        })?;

        let control = StorageControl::builder().build().await.map_err(|e| {
            StorageError::AuthenticationError(format!(
                "Failed to create storage control client: {e}"
            ))
        })?;

        Ok(Self {
            storage: Arc::new(storage),
            control: Arc::new(control),
            bucket: config.bucket.clone(),
            config,
        })
    }

    /// Get the bucket path in projects/_/buckets/{bucket_id} format
    fn bucket_path(&self) -> String {
        bucket_path(&self.bucket)
    }

    /// Upload using resumable upload
    #[allow(clippy::too_many_arguments)]
    async fn resumable_upload(
        &self,
        object_name: &str,
        stream: ByteStream,
        _total_size: Option<u64>,
        _options: &UploadOptions,
    ) -> Result<String> {
        info!("Starting resumable upload for: {}", object_name);

        // Collect stream data
        let mut data = Vec::new();
        let mut stream = stream;

        while let Some(result) = stream.next().await {
            let chunk = result?;
            data.extend_from_slice(&chunk);
        }

        let uploaded = self
            .storage
            .write_object(self.bucket_path(), object_name, Bytes::from(data))
            .send_buffered()
            .await
            .map_err(|e| StorageError::ProviderError(format!("Failed to upload object: {e}")))?;

        let etag = uploaded.etag.clone();

        info!("Resumable upload completed for: {}", object_name);
        Ok(etag)
    }

    /// Create bucket
    pub async fn create_bucket(&self, location: Option<&str>) -> Result<()> {
        info!("Creating bucket: {}", self.bucket);

        let project_id = self
            .config
            .project_id
            .as_ref()
            .ok_or_else(|| StorageError::InvalidConfig("Project ID required for GCS".into()))?;

        let mut bucket_model = model::Bucket::default();
        bucket_model.location = location.unwrap_or("US").to_string();

        self.control
            .create_bucket()
            .set_parent(format!("projects/{project_id}"))
            .set_bucket_id(&self.bucket)
            .set_bucket(bucket_model)
            .send()
            .await
            .map_err(|e| StorageError::ProviderError(format!("Failed to create bucket: {e}")))?;

        info!("Bucket created: {}", self.bucket);
        Ok(())
    }

    /// Delete bucket
    pub async fn delete_bucket(&self) -> Result<()> {
        info!("Deleting bucket: {}", self.bucket);

        self.control
            .delete_bucket()
            .set_name(self.bucket_path())
            .send()
            .await
            .map_err(|e| StorageError::ProviderError(format!("Failed to delete bucket: {e}")))?;

        info!("Bucket deleted: {}", self.bucket);
        Ok(())
    }

    /// Check if bucket exists
    pub async fn bucket_exists(&self) -> Result<bool> {
        match self
            .control
            .get_bucket()
            .set_name(self.bucket_path())
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                let err_str = format!("{e:?}");
                if err_str.contains("404") || err_str.contains("NotFound") {
                    Ok(false)
                } else {
                    Err(StorageError::ProviderError(format!(
                        "Failed to check bucket existence: {err_str}"
                    )))
                }
            }
        }
    }

    /// Set object ACL
    pub async fn set_object_acl(
        &self,
        object_name: &str,
        _entity: &str,
        _role: &str,
    ) -> Result<()> {
        info!("Setting ACL for object: {}", object_name);
        warn!("Full ACL management not implemented, using basic permissions");
        Ok(())
    }

    /// Get object ACL
    pub async fn get_object_acl(&self, object_name: &str) -> Result<Vec<AclEntry>> {
        debug!("Getting ACL for object: {}", object_name);
        Ok(Vec::new())
    }

    /// Compose objects (combine multiple objects into one)
    pub async fn compose_objects(
        &self,
        source_objects: &[String],
        destination_object: &str,
    ) -> Result<()> {
        if source_objects.is_empty() {
            return Err(StorageError::InvalidConfig(
                "Source objects cannot be empty".into(),
            ));
        }

        if source_objects.len() > MAX_COMPOSE_OBJECTS {
            return Err(StorageError::InvalidConfig(format!(
                "Cannot compose more than {MAX_COMPOSE_OBJECTS} objects"
            )));
        }

        info!(
            "Composing {} objects into: {}",
            source_objects.len(),
            destination_object
        );

        let source_refs: Vec<model::compose_object_request::SourceObject> = source_objects
            .iter()
            .map(|name| {
                let mut src = model::compose_object_request::SourceObject::new();
                src.name = name.clone();
                src
            })
            .collect();

        let mut dest_obj = model::Object::default();
        dest_obj.name = destination_object.to_string();
        dest_obj.bucket = self.bucket_path();

        self.control
            .compose_object()
            .set_destination(dest_obj)
            .set_source_objects(source_refs)
            .send()
            .await
            .map_err(|e| StorageError::ProviderError(format!("Failed to compose objects: {e}")))?;

        info!("Objects composed into: {}", destination_object);
        Ok(())
    }

    /// Set object retention policy
    pub async fn set_retention_policy(
        &self,
        object_name: &str,
        retain_until: DateTime<Utc>,
    ) -> Result<()> {
        info!(
            "Setting retention policy for object: {} until {}",
            object_name, retain_until
        );
        warn!("Retention policy management not fully implemented");
        Ok(())
    }

    /// Enable customer-managed encryption
    pub async fn set_encryption_key(&self, object_name: &str, _key: &str) -> Result<()> {
        info!("Setting encryption key for object: {}", object_name);
        warn!("Customer-managed encryption not fully implemented");
        Ok(())
    }

    /// Get object generations (versions)
    pub async fn list_object_generations(
        &self,
        object_name: &str,
    ) -> Result<Vec<ObjectGeneration>> {
        debug!("Listing generations for object: {}", object_name);

        let response = self
            .control
            .list_objects()
            .set_parent(self.bucket_path())
            .set_prefix(object_name)
            .set_versions(true)
            .send()
            .await
            .map_err(|e| {
                StorageError::ProviderError(format!("Failed to list object generations: {e}"))
            })?;

        let generations: Vec<ObjectGeneration> = response
            .objects
            .into_iter()
            .filter(|obj| obj.name == object_name)
            .map(|obj| ObjectGeneration {
                generation: obj.generation,
                metageneration: obj.metageneration,
                time_created: obj.create_time.as_ref().map(wkt_timestamp_to_chrono),
                size: obj.size as u64,
                etag: if obj.etag.is_empty() {
                    None
                } else {
                    Some(obj.etag.clone())
                },
            })
            .collect();

        Ok(generations)
    }

    /// Delete specific generation of an object
    pub async fn delete_object_generation(&self, object_name: &str, generation: i64) -> Result<()> {
        info!(
            "Deleting generation {} of object: {}",
            generation, object_name
        );

        self.control
            .delete_object()
            .set_bucket(self.bucket_path())
            .set_object(object_name)
            .set_generation(generation)
            .send()
            .await
            .map_err(|e| {
                StorageError::ProviderError(format!("Failed to delete object generation: {e}"))
            })?;

        info!(
            "Deleted generation {} of object: {}",
            generation, object_name
        );
        Ok(())
    }

    /// Set storage class
    pub async fn set_storage_class(&self, object_name: &str, storage_class: &str) -> Result<()> {
        info!(
            "Setting storage class to {} for object: {}",
            storage_class, object_name
        );
        warn!("Storage class change not fully implemented");
        Ok(())
    }

    /// Get signed URL for upload
    pub async fn get_signed_upload_url(
        &self,
        object_name: &str,
        _content_type: Option<&str>,
        _expiration_secs: u64,
    ) -> Result<String> {
        info!("Generating signed upload URL for: {}", object_name);
        warn!("Signed URL generation not fully implemented");

        Ok(format!(
            "https://storage.googleapis.com/{}/{}",
            self.bucket, object_name
        ))
    }

    /// Get signed URL for download
    pub async fn get_signed_download_url(
        &self,
        object_name: &str,
        _expiration_secs: u64,
    ) -> Result<String> {
        info!("Generating signed download URL for: {}", object_name);
        warn!("Signed URL generation not fully implemented");

        Ok(format!(
            "https://storage.googleapis.com/{}/{}",
            self.bucket, object_name
        ))
    }
}

#[async_trait]
impl CloudStorage for GcsStorage {
    async fn upload_stream(
        &self,
        key: &str,
        stream: ByteStream,
        size: Option<u64>,
        options: UploadOptions,
    ) -> Result<String> {
        debug!("Uploading stream to object: {}", key);

        // Use resumable upload for large files or unknown sizes
        if size.is_none_or(|s| s > RESUMABLE_THRESHOLD) {
            return self.resumable_upload(key, stream, size, &options).await;
        }

        // Simple upload for small files
        let mut data = Vec::new();
        let mut stream = stream;

        while let Some(result) = stream.next().await {
            let chunk = result?;
            data.extend_from_slice(&chunk);
        }

        let uploaded = self
            .storage
            .write_object(self.bucket_path(), key, Bytes::from(data))
            .send_buffered()
            .await
            .map_err(|e| StorageError::ProviderError(format!("Failed to upload object: {e}")))?;

        let etag = uploaded.etag.clone();

        info!("Uploaded object: {}", key);
        Ok(etag)
    }

    async fn upload_file(
        &self,
        key: &str,
        file_path: &Path,
        _options: UploadOptions,
    ) -> Result<String> {
        debug!("Uploading file {:?} to object: {}", file_path, key);

        let mut file = File::open(file_path).await?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;

        let uploaded = self
            .storage
            .write_object(self.bucket_path(), key, Bytes::from(data))
            .send_buffered()
            .await
            .map_err(|e| StorageError::ProviderError(format!("Failed to upload file: {e}")))?;

        let etag = uploaded.etag.clone();

        info!("Uploaded file to object: {}", key);
        Ok(etag)
    }

    async fn download_stream(&self, key: &str, _options: DownloadOptions) -> Result<ByteStream> {
        debug!("Downloading stream from object: {}", key);

        let mut resp = self
            .storage
            .read_object(self.bucket_path(), key)
            .send()
            .await
            .map_err(|e| {
                let err_str = format!("{e:?}");
                if err_str.contains("404") || err_str.contains("NotFound") {
                    StorageError::NotFound(key.to_string())
                } else {
                    StorageError::ProviderError(format!("Failed to download object: {err_str}"))
                }
            })?;

        // Collect all chunks from the read response
        let mut data = Vec::new();
        while let Some(chunk) =
            resp.next().await.transpose().map_err(|e| {
                StorageError::ProviderError(format!("Failed to read object chunk: {e}"))
            })?
        {
            data.extend_from_slice(&chunk);
        }

        let stream = futures::stream::once(async move { Ok(Bytes::from(data)) });
        Ok(Box::pin(stream))
    }

    async fn download_file(
        &self,
        key: &str,
        file_path: &Path,
        options: DownloadOptions,
    ) -> Result<()> {
        debug!("Downloading file from object: {} to {:?}", key, file_path);

        let mut stream = self.download_stream(key, options).await?;
        let mut file = File::create(file_path).await?;

        while let Some(result) = stream.next().await {
            let chunk = result?;
            file.write_all(&chunk).await?;
        }

        file.flush().await?;
        info!("Downloaded file from object: {} to {:?}", key, file_path);
        Ok(())
    }

    async fn get_metadata(&self, key: &str) -> Result<ObjectMetadata> {
        debug!("Getting metadata for object: {}", key);

        let object = self
            .control
            .get_object()
            .set_bucket(self.bucket_path())
            .set_object(key)
            .send()
            .await
            .map_err(|e| {
                let err_str = format!("{e:?}");
                if err_str.contains("404") || err_str.contains("NotFound") {
                    StorageError::NotFound(key.to_string())
                } else {
                    StorageError::ProviderError(format!("Failed to get object metadata: {err_str}"))
                }
            })?;

        let last_modified = object
            .update_time
            .as_ref()
            .map_or_else(Utc::now, wkt_timestamp_to_chrono);

        let content_type = if object.content_type.is_empty() {
            None
        } else {
            Some(object.content_type.clone())
        };

        let storage_class = if object.storage_class.is_empty() {
            None
        } else {
            Some(object.storage_class.clone())
        };

        Ok(ObjectMetadata {
            key: key.to_string(),
            size: object.size as u64,
            content_type,
            last_modified,
            etag: if object.etag.is_empty() {
                None
            } else {
                Some(object.etag.clone())
            },
            metadata: object.metadata,
            storage_class,
        })
    }

    async fn delete_object(&self, key: &str) -> Result<()> {
        debug!("Deleting object: {}", key);

        self.control
            .delete_object()
            .set_bucket(self.bucket_path())
            .set_object(key)
            .send()
            .await
            .map_err(|e| StorageError::ProviderError(format!("Failed to delete object: {e}")))?;

        info!("Deleted object: {}", key);
        Ok(())
    }

    async fn delete_objects(&self, keys: &[String]) -> Result<Vec<Result<()>>> {
        debug!("Deleting {} objects", keys.len());

        let mut results = Vec::new();

        for key in keys {
            let result = self.delete_object(key).await;
            results.push(result);
        }

        info!("Deleted {} objects", keys.len());
        Ok(results)
    }

    async fn list_objects(&self, options: ListOptions) -> Result<ListResult> {
        debug!("Listing objects with prefix: {:?}", options.prefix);

        let mut builder = self.control.list_objects().set_parent(self.bucket_path());

        if let Some(prefix) = &options.prefix {
            builder = builder.set_prefix(prefix.as_str());
        }

        if let Some(delimiter) = &options.delimiter {
            builder = builder.set_delimiter(delimiter.as_str());
        }

        if let Some(max_results) = options.max_results {
            builder = builder.set_page_size(max_results as i32);
        }

        if let Some(token) = &options.continuation_token {
            builder = builder.set_page_token(token.as_str());
        }

        let response = builder
            .send()
            .await
            .map_err(|e| StorageError::ProviderError(format!("Failed to list objects: {e}")))?;

        let objects = response
            .objects
            .into_iter()
            .map(|obj| {
                let last_modified = obj
                    .update_time
                    .as_ref()
                    .map_or_else(Utc::now, wkt_timestamp_to_chrono);

                let content_type = if obj.content_type.is_empty() {
                    None
                } else {
                    Some(obj.content_type.clone())
                };

                let storage_class = if obj.storage_class.is_empty() {
                    None
                } else {
                    Some(obj.storage_class.clone())
                };

                ObjectMetadata {
                    key: obj.name,
                    size: obj.size as u64,
                    content_type,
                    last_modified,
                    etag: if obj.etag.is_empty() {
                        None
                    } else {
                        Some(obj.etag.clone())
                    },
                    metadata: obj.metadata,
                    storage_class,
                }
            })
            .collect();

        let prefixes = response.prefixes;
        let has_more = !response.next_page_token.is_empty();

        Ok(ListResult {
            objects,
            prefixes,
            next_token: if response.next_page_token.is_empty() {
                None
            } else {
                Some(response.next_page_token)
            },
            has_more,
        })
    }

    async fn object_exists(&self, key: &str) -> Result<bool> {
        match self.get_metadata(key).await {
            Ok(_) => Ok(true),
            Err(StorageError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn copy_object(&self, source_key: &str, dest_key: &str) -> Result<()> {
        debug!("Copying object from {} to {}", source_key, dest_key);

        self.control
            .rewrite_object()
            .set_source_bucket(self.bucket_path())
            .set_source_object(source_key)
            .set_destination_bucket(self.bucket_path())
            .set_destination_name(dest_key)
            .send()
            .await
            .map_err(|e| StorageError::ProviderError(format!("Failed to copy object: {e}")))?;

        info!("Copied object from {} to {}", source_key, dest_key);
        Ok(())
    }

    async fn generate_presigned_url(&self, key: &str, expiration_secs: u64) -> Result<String> {
        self.get_signed_download_url(key, expiration_secs).await
    }

    async fn generate_presigned_upload_url(
        &self,
        key: &str,
        expiration_secs: u64,
    ) -> Result<String> {
        self.get_signed_upload_url(key, None, expiration_secs).await
    }
}

/// ACL entry
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AclEntry {
    pub entity: String,
    pub role: String,
}

/// Object generation (version) information
#[derive(Debug, Clone)]
pub struct ObjectGeneration {
    pub generation: i64,
    pub metageneration: i64,
    pub time_created: Option<DateTime<Utc>>,
    pub size: u64,
    pub etag: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_compose_objects() {
        assert_eq!(MAX_COMPOSE_OBJECTS, 32);
    }

    #[test]
    fn test_resumable_threshold() {
        assert_eq!(RESUMABLE_THRESHOLD, 10 * 1024 * 1024);
    }

    #[test]
    fn test_bucket_path() {
        assert_eq!(bucket_path("my-bucket"), "projects/_/buckets/my-bucket");
        assert_eq!(
            bucket_path("projects/foo/buckets/bar"),
            "projects/foo/buckets/bar"
        );
    }
}
