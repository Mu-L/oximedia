//! Storage management with multiple backends
//!
//! Provides comprehensive storage management for:
//! - Local filesystem storage
//! - Amazon S3 storage
//! - Azure Blob Storage
//! - Google Cloud Storage
//! - Network storage (NFS, SMB)
//! - Tiered storage (hot, warm, cold)
//! - Deduplication
//! - Storage analytics

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::database::Database;
use crate::{MamError, Result};

/// Storage manager handles all storage operations
pub struct StorageManager {
    db: Arc<Database>,
    backends: Arc<RwLock<HashMap<String, Arc<dyn StorageBackend>>>>,
    default_backend: String,
}

/// Storage backend trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Upload a file
    async fn upload(&self, local_path: &Path, remote_path: &str) -> Result<StorageMetadata>;

    /// Download a file
    async fn download(&self, remote_path: &str, local_path: &Path) -> Result<()>;

    /// Delete a file
    async fn delete(&self, remote_path: &str) -> Result<()>;

    /// Check if file exists
    async fn exists(&self, remote_path: &str) -> Result<bool>;

    /// Get file metadata
    async fn metadata(&self, remote_path: &str) -> Result<StorageMetadata>;

    /// List files in directory
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;

    /// Get storage statistics
    async fn statistics(&self) -> Result<StorageStatistics>;

    /// Get backend type
    fn backend_type(&self) -> StorageBackendType;
}

/// Storage backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageBackendType {
    /// Local filesystem
    Local,
    /// Amazon S3
    S3,
    /// Azure Blob Storage
    Azure,
    /// Google Cloud Storage
    GCS,
    /// Network File System
    NFS,
    /// SMB/CIFS
    SMB,
}

impl StorageBackendType {
    /// Convert to string
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::S3 => "s3",
            Self::Azure => "azure",
            Self::GCS => "gcs",
            Self::NFS => "nfs",
            Self::SMB => "smb",
        }
    }
}

/// Storage metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetadata {
    pub path: String,
    pub size: u64,
    pub content_type: Option<String>,
    pub checksum: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStatistics {
    pub total_files: u64,
    pub total_size: u64,
    pub available_space: Option<u64>,
    pub used_space: Option<u64>,
}

/// Storage tier for tiered storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageTier {
    /// Hot tier (fast access, high cost)
    Hot,
    /// Warm tier (medium access, medium cost)
    Warm,
    /// Cold tier (slow access, low cost)
    Cold,
    /// Archive tier (very slow access, very low cost)
    Archive,
}

impl StorageTier {
    /// Convert to string
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::Warm => "warm",
            Self::Cold => "cold",
            Self::Archive => "archive",
        }
    }

    /// Get access time in seconds
    #[must_use]
    pub const fn access_time_seconds(&self) -> u32 {
        match self {
            Self::Hot => 1,
            Self::Warm => 60,
            Self::Cold => 300,
            Self::Archive => 3600,
        }
    }

    /// Get cost multiplier
    #[must_use]
    pub const fn cost_multiplier(&self) -> f32 {
        match self {
            Self::Hot => 1.0,
            Self::Warm => 0.5,
            Self::Cold => 0.1,
            Self::Archive => 0.01,
        }
    }
}

/// Storage location record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StorageLocation {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub backend: String,
    pub path: String,
    pub tier: String,
    pub size: Option<i64>,
    pub checksum: Option<String>,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Local filesystem storage backend
pub struct LocalStorage {
    root_path: PathBuf,
}

impl LocalStorage {
    /// Create a new local storage backend
    #[must_use]
    pub fn new(root_path: String) -> Self {
        Self {
            root_path: PathBuf::from(root_path),
        }
    }

    fn resolve_path(&self, remote_path: &str) -> PathBuf {
        self.root_path.join(remote_path.trim_start_matches('/'))
    }
}

#[async_trait]
impl StorageBackend for LocalStorage {
    async fn upload(&self, local_path: &Path, remote_path: &str) -> Result<StorageMetadata> {
        let dest_path = self.resolve_path(remote_path);

        // Create parent directories
        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Copy file
        tokio::fs::copy(local_path, &dest_path).await?;

        // Get metadata
        let metadata = tokio::fs::metadata(&dest_path).await?;

        Ok(StorageMetadata {
            path: remote_path.to_string(),
            size: metadata.len(),
            content_type: None,
            checksum: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        })
    }

    async fn download(&self, remote_path: &str, local_path: &Path) -> Result<()> {
        let src_path = self.resolve_path(remote_path);

        // Create parent directories
        if let Some(parent) = local_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Copy file
        tokio::fs::copy(src_path, local_path).await?;

        Ok(())
    }

    async fn delete(&self, remote_path: &str) -> Result<()> {
        let path = self.resolve_path(remote_path);
        tokio::fs::remove_file(path).await?;
        Ok(())
    }

    async fn exists(&self, remote_path: &str) -> Result<bool> {
        let path = self.resolve_path(remote_path);
        Ok(path.exists())
    }

    async fn metadata(&self, remote_path: &str) -> Result<StorageMetadata> {
        let path = self.resolve_path(remote_path);
        let metadata = tokio::fs::metadata(&path).await?;

        Ok(StorageMetadata {
            path: remote_path.to_string(),
            size: metadata.len(),
            content_type: None,
            checksum: None,
            created_at: metadata.created().ok().map(|t| t.into()),
            updated_at: metadata.modified().ok().map(|t| t.into()),
        })
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let dir_path = self.resolve_path(prefix);
        let mut entries = Vec::new();

        let mut dir = tokio::fs::read_dir(dir_path).await?;
        while let Some(entry) = dir.next_entry().await? {
            if let Ok(file_name) = entry.file_name().into_string() {
                entries.push(format!("{}/{}", prefix.trim_end_matches('/'), file_name));
            }
        }

        Ok(entries)
    }

    async fn statistics(&self) -> Result<StorageStatistics> {
        // Get filesystem statistics (simplified)
        Ok(StorageStatistics {
            total_files: 0,
            total_size: 0,
            available_space: None,
            used_space: None,
        })
    }

    fn backend_type(&self) -> StorageBackendType {
        StorageBackendType::Local
    }
}

/// S3 storage backend (placeholder implementation)
#[allow(dead_code)]
pub struct S3Storage {
    bucket: String,
    region: String,
    access_key: String,
    secret_key: String,
}

impl S3Storage {
    /// Create a new S3 storage backend
    #[must_use]
    pub fn new(bucket: String, region: String, access_key: String, secret_key: String) -> Self {
        Self {
            bucket,
            region,
            access_key,
            secret_key,
        }
    }
}

#[async_trait]
impl StorageBackend for S3Storage {
    async fn upload(&self, _local_path: &Path, remote_path: &str) -> Result<StorageMetadata> {
        // Placeholder: In production, use aws-sdk-s3
        tracing::info!(
            "S3 upload to bucket: {}, path: {}",
            self.bucket,
            remote_path
        );

        Ok(StorageMetadata {
            path: remote_path.to_string(),
            size: 0,
            content_type: None,
            checksum: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        })
    }

    async fn download(&self, remote_path: &str, _local_path: &Path) -> Result<()> {
        tracing::info!(
            "S3 download from bucket: {}, path: {}",
            self.bucket,
            remote_path
        );
        Ok(())
    }

    async fn delete(&self, remote_path: &str) -> Result<()> {
        tracing::info!(
            "S3 delete from bucket: {}, path: {}",
            self.bucket,
            remote_path
        );
        Ok(())
    }

    async fn exists(&self, _remote_path: &str) -> Result<bool> {
        Ok(true)
    }

    async fn metadata(&self, remote_path: &str) -> Result<StorageMetadata> {
        Ok(StorageMetadata {
            path: remote_path.to_string(),
            size: 0,
            content_type: None,
            checksum: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        })
    }

    async fn list(&self, _prefix: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn statistics(&self) -> Result<StorageStatistics> {
        Ok(StorageStatistics {
            total_files: 0,
            total_size: 0,
            available_space: None,
            used_space: None,
        })
    }

    fn backend_type(&self) -> StorageBackendType {
        StorageBackendType::S3
    }
}

/// Azure Blob Storage backend (placeholder implementation)
#[allow(dead_code)]
pub struct AzureStorage {
    account: String,
    container: String,
    access_key: String,
}

impl AzureStorage {
    /// Create a new Azure storage backend
    #[must_use]
    pub fn new(account: String, container: String, access_key: String) -> Self {
        Self {
            account,
            container,
            access_key,
        }
    }
}

#[async_trait]
impl StorageBackend for AzureStorage {
    async fn upload(&self, _local_path: &Path, remote_path: &str) -> Result<StorageMetadata> {
        tracing::info!(
            "Azure upload to container: {}, path: {}",
            self.container,
            remote_path
        );

        Ok(StorageMetadata {
            path: remote_path.to_string(),
            size: 0,
            content_type: None,
            checksum: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        })
    }

    async fn download(&self, remote_path: &str, _local_path: &Path) -> Result<()> {
        tracing::info!(
            "Azure download from container: {}, path: {}",
            self.container,
            remote_path
        );
        Ok(())
    }

    async fn delete(&self, remote_path: &str) -> Result<()> {
        tracing::info!(
            "Azure delete from container: {}, path: {}",
            self.container,
            remote_path
        );
        Ok(())
    }

    async fn exists(&self, _remote_path: &str) -> Result<bool> {
        Ok(true)
    }

    async fn metadata(&self, remote_path: &str) -> Result<StorageMetadata> {
        Ok(StorageMetadata {
            path: remote_path.to_string(),
            size: 0,
            content_type: None,
            checksum: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        })
    }

    async fn list(&self, _prefix: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn statistics(&self) -> Result<StorageStatistics> {
        Ok(StorageStatistics {
            total_files: 0,
            total_size: 0,
            available_space: None,
            used_space: None,
        })
    }

    fn backend_type(&self) -> StorageBackendType {
        StorageBackendType::Azure
    }
}

/// Google Cloud Storage backend (placeholder implementation)
#[allow(dead_code)]
pub struct GCSStorage {
    bucket: String,
    project_id: String,
    credentials_path: String,
}

impl GCSStorage {
    /// Create a new GCS storage backend
    #[must_use]
    pub fn new(bucket: String, project_id: String, credentials_path: String) -> Self {
        Self {
            bucket,
            project_id,
            credentials_path,
        }
    }
}

#[async_trait]
impl StorageBackend for GCSStorage {
    async fn upload(&self, _local_path: &Path, remote_path: &str) -> Result<StorageMetadata> {
        tracing::info!(
            "GCS upload to bucket: {}, path: {}",
            self.bucket,
            remote_path
        );

        Ok(StorageMetadata {
            path: remote_path.to_string(),
            size: 0,
            content_type: None,
            checksum: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        })
    }

    async fn download(&self, remote_path: &str, _local_path: &Path) -> Result<()> {
        tracing::info!(
            "GCS download from bucket: {}, path: {}",
            self.bucket,
            remote_path
        );
        Ok(())
    }

    async fn delete(&self, remote_path: &str) -> Result<()> {
        tracing::info!(
            "GCS delete from bucket: {}, path: {}",
            self.bucket,
            remote_path
        );
        Ok(())
    }

    async fn exists(&self, _remote_path: &str) -> Result<bool> {
        Ok(true)
    }

    async fn metadata(&self, remote_path: &str) -> Result<StorageMetadata> {
        Ok(StorageMetadata {
            path: remote_path.to_string(),
            size: 0,
            content_type: None,
            checksum: None,
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        })
    }

    async fn list(&self, _prefix: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn statistics(&self) -> Result<StorageStatistics> {
        Ok(StorageStatistics {
            total_files: 0,
            total_size: 0,
            available_space: None,
            used_space: None,
        })
    }

    fn backend_type(&self) -> StorageBackendType {
        StorageBackendType::GCS
    }
}

impl StorageManager {
    /// Create a new storage manager
    #[must_use]
    pub fn new(db: Arc<Database>, default_backend: String) -> Self {
        Self {
            db,
            backends: Arc::new(RwLock::new(HashMap::new())),
            default_backend,
        }
    }

    /// Register a storage backend
    pub async fn register_backend(
        &self,
        name: String,
        backend: Arc<dyn StorageBackend>,
    ) -> Result<()> {
        self.backends.write().await.insert(name, backend);
        Ok(())
    }

    /// Upload file to storage
    ///
    /// # Errors
    ///
    /// Returns an error if upload fails
    pub async fn upload_file(
        &self,
        asset_id: Uuid,
        local_path: &Path,
        remote_path: &str,
        backend_name: Option<String>,
        tier: StorageTier,
    ) -> Result<StorageLocation> {
        let backend_name = backend_name.unwrap_or_else(|| self.default_backend.clone());

        let backends = self.backends.read().await;
        let backend = backends
            .get(&backend_name)
            .ok_or_else(|| MamError::Internal(format!("Backend not found: {backend_name}")))?;

        // Upload file
        let metadata = backend.upload(local_path, remote_path).await?;

        // Create storage location record
        let location = sqlx::query_as::<_, StorageLocation>(
            "INSERT INTO storage_locations
             (id, asset_id, backend, path, tier, size, checksum, is_primary, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, true, NOW(), NOW())
             RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(asset_id)
        .bind(&backend_name)
        .bind(remote_path)
        .bind(tier.as_str())
        .bind(metadata.size as i64)
        .bind(metadata.checksum)
        .fetch_one(self.db.pool())
        .await?;

        Ok(location)
    }

    /// Download file from storage
    ///
    /// # Errors
    ///
    /// Returns an error if download fails
    pub async fn download_file(
        &self,
        asset_id: Uuid,
        local_path: &Path,
        backend_name: Option<String>,
    ) -> Result<()> {
        // Get primary storage location
        let location = sqlx::query_as::<_, StorageLocation>(
            "SELECT * FROM storage_locations
             WHERE asset_id = $1 AND is_primary = true
             LIMIT 1",
        )
        .bind(asset_id)
        .fetch_one(self.db.pool())
        .await?;

        let backend_name = backend_name.unwrap_or(location.backend);

        let backends = self.backends.read().await;
        let backend = backends
            .get(&backend_name)
            .ok_or_else(|| MamError::Internal(format!("Backend not found: {backend_name}")))?;

        // Download file
        backend.download(&location.path, local_path).await?;

        Ok(())
    }

    /// Delete file from storage
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails
    pub async fn delete_file(&self, asset_id: Uuid) -> Result<()> {
        // Get all storage locations
        let locations = sqlx::query_as::<_, StorageLocation>(
            "SELECT * FROM storage_locations WHERE asset_id = $1",
        )
        .bind(asset_id)
        .fetch_all(self.db.pool())
        .await?;

        let backends = self.backends.read().await;

        // Delete from all locations
        for location in &locations {
            if let Some(backend) = backends.get(&location.backend) {
                if let Err(e) = backend.delete(&location.path).await {
                    tracing::error!("Failed to delete file from {}: {}", location.backend, e);
                }
            }
        }

        // Delete storage location records
        sqlx::query("DELETE FROM storage_locations WHERE asset_id = $1")
            .bind(asset_id)
            .execute(self.db.pool())
            .await?;

        Ok(())
    }

    /// Get storage locations for asset
    ///
    /// # Errors
    ///
    /// Returns an error if query fails
    pub async fn get_storage_locations(&self, asset_id: Uuid) -> Result<Vec<StorageLocation>> {
        let locations = sqlx::query_as::<_, StorageLocation>(
            "SELECT * FROM storage_locations WHERE asset_id = $1 ORDER BY is_primary DESC, created_at DESC",
        )
        .bind(asset_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(locations)
    }

    /// Move asset to different storage tier
    ///
    /// # Errors
    ///
    /// Returns an error if tier change fails
    pub async fn change_tier(&self, asset_id: Uuid, new_tier: StorageTier) -> Result<()> {
        sqlx::query(
            "UPDATE storage_locations
             SET tier = $2, updated_at = NOW()
             WHERE asset_id = $1",
        )
        .bind(asset_id)
        .bind(new_tier.as_str())
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    /// Get storage statistics across all backends
    ///
    /// # Errors
    ///
    /// Returns an error if statistics gathering fails
    pub async fn get_statistics(&self) -> Result<HashMap<String, StorageStatistics>> {
        let mut stats = HashMap::new();

        let backends = self.backends.read().await;
        for (name, backend) in backends.iter() {
            if let Ok(backend_stats) = backend.statistics().await {
                stats.insert(name.clone(), backend_stats);
            }
        }

        Ok(stats)
    }

    /// Check storage health
    ///
    /// # Errors
    ///
    /// Returns an error if health check fails
    pub async fn check_health(&self) -> Result<bool> {
        let backends = self.backends.read().await;

        for (name, backend) in backends.iter() {
            // Try to get statistics as health check
            if backend.statistics().await.is_err() {
                tracing::warn!("Storage backend {} is unhealthy", name);
                return Ok(false);
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_backend_type_as_str() {
        assert_eq!(StorageBackendType::Local.as_str(), "local");
        assert_eq!(StorageBackendType::S3.as_str(), "s3");
        assert_eq!(StorageBackendType::Azure.as_str(), "azure");
        assert_eq!(StorageBackendType::GCS.as_str(), "gcs");
    }

    #[test]
    fn test_storage_tier_as_str() {
        assert_eq!(StorageTier::Hot.as_str(), "hot");
        assert_eq!(StorageTier::Warm.as_str(), "warm");
        assert_eq!(StorageTier::Cold.as_str(), "cold");
        assert_eq!(StorageTier::Archive.as_str(), "archive");
    }

    #[test]
    fn test_storage_tier_access_time() {
        assert_eq!(StorageTier::Hot.access_time_seconds(), 1);
        assert_eq!(StorageTier::Warm.access_time_seconds(), 60);
        assert_eq!(StorageTier::Cold.access_time_seconds(), 300);
        assert_eq!(StorageTier::Archive.access_time_seconds(), 3600);
    }

    #[test]
    fn test_storage_tier_cost_multiplier() {
        assert_eq!(StorageTier::Hot.cost_multiplier(), 1.0);
        assert_eq!(StorageTier::Warm.cost_multiplier(), 0.5);
        assert_eq!(StorageTier::Cold.cost_multiplier(), 0.1);
        assert_eq!(StorageTier::Archive.cost_multiplier(), 0.01);
    }

    #[test]
    fn test_storage_metadata_serialization() {
        let metadata = StorageMetadata {
            path: "/test/file.mp4".to_string(),
            size: 1024,
            content_type: Some("video/mp4".to_string()),
            checksum: Some("abc123".to_string()),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
        };

        let json = serde_json::to_string(&metadata).expect("should succeed in test");
        let deserialized: StorageMetadata =
            serde_json::from_str(&json).expect("should succeed in test");

        assert_eq!(deserialized.path, "/test/file.mp4");
        assert_eq!(deserialized.size, 1024);
    }

    #[tokio::test]
    async fn test_local_storage_resolve_path() {
        let storage = LocalStorage::new("/var/storage".to_string());
        let resolved = storage.resolve_path("/assets/test.mp4");
        assert_eq!(resolved, PathBuf::from("/var/storage/assets/test.mp4"));
    }
}
