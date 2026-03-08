//! CDN uploader for distributing content to cloud storage.

use crate::cdn::{AzureUploader, GcsUploader, S3Uploader};
use crate::error::{ServerError, ServerResult};
use oximedia_net::rtmp::MediaPacket;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};
use uuid::Uuid;

/// CDN backend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdnBackend {
    /// Amazon S3.
    S3,
    /// Azure Blob Storage.
    Azure,
    /// Google Cloud Storage.
    Gcs,
}

/// CDN configuration.
#[derive(Debug, Clone)]
pub struct CdnConfig {
    /// CDN backend.
    pub backend: CdnBackend,

    /// Bucket/container name.
    pub bucket: String,

    /// Region.
    pub region: String,

    /// Access key ID.
    pub access_key: String,

    /// Secret access key.
    pub secret_key: String,

    /// Base path in bucket.
    pub base_path: String,

    /// Enable public access.
    pub public: bool,

    /// Enable CDN distribution.
    pub enable_cdn: bool,

    /// CDN domain.
    pub cdn_domain: Option<String>,
}

impl Default for CdnConfig {
    fn default() -> Self {
        Self {
            backend: CdnBackend::S3,
            bucket: String::new(),
            region: "us-east-1".to_string(),
            access_key: String::new(),
            secret_key: String::new(),
            base_path: "live".to_string(),
            public: true,
            enable_cdn: false,
            cdn_domain: None,
        }
    }
}

/// Upload job.
#[derive(Debug, Clone)]
pub struct UploadJob {
    /// Job ID.
    pub id: Uuid,

    /// Stream key.
    pub stream_key: String,

    /// Object key in bucket.
    pub object_key: String,

    /// Data size.
    pub size: u64,

    /// Upload status.
    pub status: UploadStatus,

    /// Created time.
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Completed time.
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Upload status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadStatus {
    /// Pending.
    Pending,
    /// Uploading.
    Uploading,
    /// Completed.
    Completed,
    /// Failed.
    Failed,
}

/// CDN uploader.
#[allow(dead_code)]
pub struct CdnUploader {
    /// Configuration.
    config: CdnConfig,

    /// S3 uploader.
    s3_uploader: Option<Arc<S3Uploader>>,

    /// Azure uploader.
    azure_uploader: Option<Arc<AzureUploader>>,

    /// GCS uploader.
    gcs_uploader: Option<Arc<GcsUploader>>,

    /// Active upload jobs.
    jobs: Arc<RwLock<HashMap<Uuid, UploadJob>>>,

    /// Upload queue.
    upload_tx: mpsc::UnboundedSender<UploadTask>,
}

/// Upload task.
#[allow(dead_code)]
struct UploadTask {
    /// Stream key.
    stream_key: String,

    /// Object key.
    object_key: String,

    /// Data to upload.
    data: bytes::Bytes,
}

impl CdnUploader {
    /// Creates a new CDN uploader.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub async fn new() -> ServerResult<Self> {
        let config = CdnConfig::default();
        let (upload_tx, mut upload_rx) = mpsc::unbounded_channel::<UploadTask>();

        let uploader = Self {
            config: config.clone(),
            s3_uploader: None,
            azure_uploader: None,
            gcs_uploader: None,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            upload_tx,
        };

        // Spawn upload worker
        tokio::spawn(async move {
            while let Some(task) = upload_rx.recv().await {
                // Process upload task
                info!("Processing upload task: {}", task.object_key);
            }
        });

        Ok(uploader)
    }

    /// Creates a CDN uploader with configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub async fn with_config(config: CdnConfig) -> ServerResult<Self> {
        let (upload_tx, mut upload_rx) = mpsc::unbounded_channel::<UploadTask>();

        let (s3_uploader, azure_uploader, gcs_uploader) = match config.backend {
            CdnBackend::S3 => {
                let uploader = S3Uploader::new(&config).await?;
                (Some(Arc::new(uploader)), None, None)
            }
            CdnBackend::Azure => {
                let uploader = AzureUploader::new(&config).await?;
                (None, Some(Arc::new(uploader)), None)
            }
            CdnBackend::Gcs => {
                let uploader = GcsUploader::new(&config).await?;
                (None, None, Some(Arc::new(uploader)))
            }
        };

        let jobs = Arc::new(RwLock::new(HashMap::new()));

        // Spawn upload worker
        let _jobs_clone = Arc::clone(&jobs);
        let s3 = s3_uploader.clone();
        let azure = azure_uploader.clone();
        let gcs = gcs_uploader.clone();

        tokio::spawn(async move {
            while let Some(task) = upload_rx.recv().await {
                let result = if let Some(ref uploader) = s3 {
                    uploader.upload(&task.object_key, task.data).await
                } else if let Some(ref uploader) = azure {
                    uploader.upload(&task.object_key, task.data).await
                } else if let Some(ref uploader) = gcs {
                    uploader.upload(&task.object_key, task.data).await
                } else {
                    Err(ServerError::Internal("No uploader configured".to_string()))
                };

                if let Err(e) = result {
                    error!("Upload failed: {}", e);
                }
            }
        });

        Ok(Self {
            config,
            s3_uploader,
            azure_uploader,
            gcs_uploader,
            jobs,
            upload_tx,
        })
    }

    /// Uploads a media packet.
    pub async fn upload_packet(&self, stream_key: &str, packet: &MediaPacket) -> ServerResult<()> {
        let object_key = format!(
            "{}/{}/packet_{}.bin",
            self.config.base_path,
            stream_key.replace('/', "_"),
            packet.timestamp
        );

        let task = UploadTask {
            stream_key: stream_key.to_string(),
            object_key,
            data: packet.data.clone(),
        };

        self.upload_tx
            .send(task)
            .map_err(|_| ServerError::Internal("Failed to queue upload".to_string()))?;

        Ok(())
    }

    /// Uploads a segment.
    pub async fn upload_segment(
        &self,
        stream_key: &str,
        segment_name: &str,
        data: bytes::Bytes,
    ) -> ServerResult<()> {
        let object_key = format!(
            "{}/{}/{}",
            self.config.base_path,
            stream_key.replace('/', "_"),
            segment_name
        );

        let task = UploadTask {
            stream_key: stream_key.to_string(),
            object_key,
            data,
        };

        self.upload_tx
            .send(task)
            .map_err(|_| ServerError::Internal("Failed to queue upload".to_string()))?;

        Ok(())
    }

    /// Gets upload job.
    #[must_use]
    pub fn get_job(&self, id: Uuid) -> Option<UploadJob> {
        let jobs = self.jobs.read();
        jobs.get(&id).cloned()
    }

    /// Lists all jobs.
    #[must_use]
    pub fn list_jobs(&self) -> Vec<UploadJob> {
        let jobs = self.jobs.read();
        jobs.values().cloned().collect()
    }
}
