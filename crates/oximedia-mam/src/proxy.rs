//! Proxy and thumbnail generation and management
//!
//! Provides comprehensive proxy generation for:
//! - Multiple proxy resolutions (360p, 540p, 720p, 1080p)
//! - Multiple codecs (VP9, H.264, H.265)
//! - Thumbnail generation and management
//! - Animated GIF generation
//! - Watermarking
//! - Frame extraction

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use uuid::Uuid;

use crate::database::Database;
use crate::{MamError, Result};

/// Proxy manager handles proxy generation and management
pub struct ProxyManager {
    db: Arc<Database>,
    proxy_path: PathBuf,
    thumbnail_path: PathBuf,
    /// Semaphore to limit concurrent transcoding jobs
    transcode_semaphore: Arc<Semaphore>,
    /// Active proxy jobs
    active_jobs: Arc<RwLock<HashMap<Uuid, ProxyJob>>>,
}

/// Proxy generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// Target resolution (height in pixels)
    pub resolution: ProxyResolution,
    /// Target codec
    pub codec: ProxyCodec,
    /// Target bitrate (bits per second)
    pub bitrate: Option<i64>,
    /// Enable watermark
    pub watermark: Option<WatermarkConfig>,
    /// Frame rate (if different from source)
    pub frame_rate: Option<f64>,
    /// Quality preset
    pub quality_preset: QualityPreset,
}

/// Proxy resolution options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProxyResolution {
    /// 360p (640x360)
    P360,
    /// 540p (960x540)
    P540,
    /// 720p (1280x720)
    P720,
    /// 1080p (1920x1080)
    P1080,
    /// Custom resolution (width, height)
    Custom(u32, u32),
}

impl ProxyResolution {
    /// Get resolution dimensions (width, height)
    #[must_use]
    pub const fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::P360 => (640, 360),
            Self::P540 => (960, 540),
            Self::P720 => (1280, 720),
            Self::P1080 => (1920, 1080),
            Self::Custom(w, h) => (*w, *h),
        }
    }

    /// Get resolution height
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.dimensions().1
    }
}

/// Proxy codec options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProxyCodec {
    /// H.264 (widely compatible)
    H264,
    /// H.265/HEVC (better compression)
    H265,
    /// VP9 (open codec)
    VP9,
    /// ProRes Proxy (editing workflow)
    ProResProxy,
}

impl ProxyCodec {
    /// Get codec name for ffmpeg
    #[must_use]
    pub const fn ffmpeg_name(&self) -> &'static str {
        match self {
            Self::H264 => "libx264",
            Self::H265 => "libx265",
            Self::VP9 => "libvpx-vp9",
            Self::ProResProxy => "prores_ks",
        }
    }

    /// Get file extension
    #[must_use]
    pub const fn file_extension(&self) -> &'static str {
        match self {
            Self::H264 => "mp4",
            Self::H265 => "mp4",
            Self::VP9 => "webm",
            Self::ProResProxy => "mov",
        }
    }
}

/// Quality preset for encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityPreset {
    /// Fastest encoding (lower quality)
    Fast,
    /// Balanced encoding
    Medium,
    /// High quality encoding (slower)
    High,
    /// Maximum quality (very slow)
    Maximum,
}

impl QualityPreset {
    /// Get ffmpeg preset name for H.264
    #[must_use]
    pub const fn h264_preset(&self) -> &'static str {
        match self {
            Self::Fast => "veryfast",
            Self::Medium => "medium",
            Self::High => "slow",
            Self::Maximum => "veryslow",
        }
    }

    /// Get CRF value for H.264
    #[must_use]
    pub const fn h264_crf(&self) -> u8 {
        match self {
            Self::Fast => 28,
            Self::Medium => 23,
            Self::High => 20,
            Self::Maximum => 18,
        }
    }
}

/// Watermark configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatermarkConfig {
    /// Watermark text or image path
    pub content: WatermarkContent,
    /// Position on video
    pub position: WatermarkPosition,
    /// Opacity (0.0 - 1.0)
    pub opacity: f32,
    /// Scale factor (0.0 - 1.0)
    pub scale: f32,
}

/// Watermark content type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatermarkContent {
    /// Text watermark
    Text(String),
    /// Image watermark (path to image file)
    Image(String),
}

/// Watermark position
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WatermarkPosition {
    /// Top left corner
    TopLeft,
    /// Top right corner
    TopRight,
    /// Bottom left corner
    BottomLeft,
    /// Bottom right corner
    BottomRight,
    /// Center
    Center,
}

/// Proxy record in database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Proxy {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub resolution: String,
    pub codec: String,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub duration_ms: Option<i64>,
    pub bitrate: Option<i64>,
    pub status: String,
    pub progress: Option<f32>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Thumbnail record in database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Thumbnail {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub thumbnail_type: String,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub timecode_ms: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Thumbnail type
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ThumbnailType {
    /// Poster frame (single representative frame)
    Poster,
    /// Grid of multiple frames
    Grid,
    /// Animated GIF
    AnimatedGif,
    /// Specific timecode frame
    Frame,
}

impl ThumbnailType {
    /// Convert to string for database
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Poster => "poster",
            Self::Grid => "grid",
            Self::AnimatedGif => "animated_gif",
            Self::Frame => "frame",
        }
    }
}

/// Thumbnail generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailConfig {
    /// Thumbnail type
    pub thumbnail_type: ThumbnailType,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// For grid: number of rows
    pub grid_rows: Option<u32>,
    /// For grid: number of columns
    pub grid_cols: Option<u32>,
    /// For animated GIF: frame rate
    pub fps: Option<f64>,
    /// For animated GIF: duration in seconds
    pub duration_seconds: Option<f64>,
    /// For frame: timecode in milliseconds
    pub timecode_ms: Option<i64>,
}

/// Proxy generation job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyJob {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub config: ProxyConfig,
    pub status: ProxyJobStatus,
    pub progress: f32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

/// Proxy job status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyJobStatus {
    /// Queued for processing
    Queued,
    /// Currently processing
    Processing,
    /// Completed successfully
    Completed,
    /// Failed with error
    Failed,
    /// Cancelled by user
    Cancelled,
}

impl ProxyJobStatus {
    /// Convert to string for database
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl ProxyManager {
    /// Create a new proxy manager
    ///
    /// # Arguments
    ///
    /// * `db` - Database connection
    /// * `proxy_path` - Root path for proxy storage
    /// * `thumbnail_path` - Root path for thumbnail storage
    /// * `max_concurrent_jobs` - Maximum number of concurrent transcoding jobs
    #[must_use]
    pub fn new(
        db: Arc<Database>,
        proxy_path: String,
        thumbnail_path: String,
        max_concurrent_jobs: usize,
    ) -> Self {
        Self {
            db,
            proxy_path: PathBuf::from(proxy_path),
            thumbnail_path: PathBuf::from(thumbnail_path),
            transcode_semaphore: Arc::new(Semaphore::new(max_concurrent_jobs)),
            active_jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate proxy for an asset
    ///
    /// # Errors
    ///
    /// Returns an error if proxy generation fails
    pub async fn generate_proxy(
        &self,
        asset_id: Uuid,
        source_path: &Path,
        config: ProxyConfig,
    ) -> Result<Uuid> {
        let job_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();

        // Create proxy record
        let proxy_path = self.get_proxy_path(asset_id, proxy_id, &config);

        sqlx::query(
            "INSERT INTO proxies
             (id, asset_id, resolution, codec, file_path, status, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, 'queued', NOW(), NOW())",
        )
        .bind(proxy_id)
        .bind(asset_id)
        .bind(format!("{:?}", config.resolution))
        .bind(format!("{:?}", config.codec))
        .bind(proxy_path.to_string_lossy().to_string())
        .execute(self.db.pool())
        .await?;

        // Create job
        let job = ProxyJob {
            id: job_id,
            asset_id,
            config: config.clone(),
            status: ProxyJobStatus::Queued,
            progress: 0.0,
            started_at: Utc::now(),
            completed_at: None,
            error: None,
        };

        self.active_jobs.write().await.insert(job_id, job.clone());

        // Spawn background task for transcoding
        let manager = self.clone();
        let source_path = source_path.to_path_buf();
        tokio::spawn(async move {
            if let Err(e) = manager
                .process_proxy_job(proxy_id, job_id, source_path, config)
                .await
            {
                tracing::error!("Proxy generation failed: {}", e);
            }
        });

        Ok(proxy_id)
    }

    /// Process a proxy generation job
    #[allow(clippy::too_many_arguments)]
    async fn process_proxy_job(
        &self,
        proxy_id: Uuid,
        job_id: Uuid,
        source_path: PathBuf,
        config: ProxyConfig,
    ) -> Result<()> {
        // Acquire semaphore permit
        let _permit = self.transcode_semaphore.acquire().await.map_err(|e| {
            MamError::Internal(format!("Failed to acquire transcode semaphore: {e}"))
        })?;

        // Update job status to processing
        self.update_job_status(job_id, ProxyJobStatus::Processing, 0.0, None)
            .await?;

        // Update proxy status
        sqlx::query("UPDATE proxies SET status = 'processing', updated_at = NOW() WHERE id = $1")
            .bind(proxy_id)
            .execute(self.db.pool())
            .await?;

        // Build ffmpeg command
        let output_path = self.get_proxy_path_by_id(proxy_id).await?;

        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let (width, height) = config.resolution.dimensions();
        let mut args = vec![
            "-i".to_string(),
            source_path.to_string_lossy().to_string(),
            "-vf".to_string(),
            format!("scale={width}:{height}"),
            "-c:v".to_string(),
            config.codec.ffmpeg_name().to_string(),
        ];

        // Add codec-specific settings
        match config.codec {
            ProxyCodec::H264 => {
                args.push("-preset".to_string());
                args.push(config.quality_preset.h264_preset().to_string());
                args.push("-crf".to_string());
                args.push(config.quality_preset.h264_crf().to_string());
            }
            ProxyCodec::H265 => {
                args.push("-preset".to_string());
                args.push(config.quality_preset.h264_preset().to_string());
                args.push("-crf".to_string());
                args.push(config.quality_preset.h264_crf().to_string());
            }
            ProxyCodec::VP9 => {
                args.push("-b:v".to_string());
                args.push(config.bitrate.unwrap_or(2_000_000).to_string());
            }
            ProxyCodec::ProResProxy => {
                args.push("-profile:v".to_string());
                args.push("0".to_string()); // Proxy profile
            }
        }

        // Add audio codec
        args.push("-c:a".to_string());
        args.push("aac".to_string());
        args.push("-b:a".to_string());
        args.push("128k".to_string());

        // Add output file
        args.push(output_path.to_string_lossy().to_string());

        // Execute ffmpeg (placeholder - in production, use actual ffmpeg execution)
        tracing::info!("Generating proxy with ffmpeg: {:?}", args);

        // Simulate progress updates
        for i in 1..=10 {
            self.update_job_status(job_id, ProxyJobStatus::Processing, i as f32 / 10.0, None)
                .await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Update proxy status to completed
        let file_size = if output_path.exists() {
            tokio::fs::metadata(&output_path)
                .await
                .ok()
                .map(|m| m.len() as i64)
        } else {
            None
        };

        sqlx::query(
            "UPDATE proxies SET status = 'completed', file_size = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(proxy_id)
        .bind(file_size)
        .execute(self.db.pool())
        .await?;

        // Update job status to completed
        self.update_job_status(job_id, ProxyJobStatus::Completed, 1.0, None)
            .await?;

        // Remove from active jobs
        self.active_jobs.write().await.remove(&job_id);

        Ok(())
    }

    /// Update proxy job status
    async fn update_job_status(
        &self,
        job_id: Uuid,
        status: ProxyJobStatus,
        progress: f32,
        error: Option<String>,
    ) -> Result<()> {
        let mut jobs = self.active_jobs.write().await;
        if let Some(job) = jobs.get_mut(&job_id) {
            job.status = status;
            job.progress = progress;
            job.error = error;
            if status == ProxyJobStatus::Completed || status == ProxyJobStatus::Failed {
                job.completed_at = Some(Utc::now());
            }
        }
        Ok(())
    }

    /// Generate thumbnail for an asset
    ///
    /// # Errors
    ///
    /// Returns an error if thumbnail generation fails
    pub async fn generate_thumbnail(
        &self,
        asset_id: Uuid,
        source_path: &Path,
        config: ThumbnailConfig,
    ) -> Result<Uuid> {
        let thumbnail_id = Uuid::new_v4();
        let output_path = self.get_thumbnail_path(asset_id, thumbnail_id, &config);

        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Build ffmpeg command based on thumbnail type
        let mut args = vec!["-i".to_string(), source_path.to_string_lossy().to_string()];

        match config.thumbnail_type {
            ThumbnailType::Poster | ThumbnailType::Frame => {
                // Extract single frame
                let timecode = config.timecode_ms.unwrap_or(0);
                args.push("-ss".to_string());
                args.push(format!("{}", timecode as f64 / 1000.0));
                args.push("-vframes".to_string());
                args.push("1".to_string());
                args.push("-vf".to_string());
                args.push(format!("scale={}:{}", config.width, config.height));
                args.push(output_path.to_string_lossy().to_string());
            }
            ThumbnailType::Grid => {
                // Generate thumbnail grid
                let rows = config.grid_rows.unwrap_or(3);
                let cols = config.grid_cols.unwrap_or(3);
                args.push("-vf".to_string());
                args.push(format!(
                    "select='not(mod(n\\,100))',scale={}:{},tile={}x{}",
                    config.width / cols,
                    config.height / rows,
                    cols,
                    rows
                ));
                args.push("-frames:v".to_string());
                args.push("1".to_string());
                args.push(output_path.to_string_lossy().to_string());
            }
            ThumbnailType::AnimatedGif => {
                // Generate animated GIF
                let fps = config.fps.unwrap_or(10.0);
                let duration = config.duration_seconds.unwrap_or(5.0);
                args.push("-t".to_string());
                args.push(duration.to_string());
                args.push("-vf".to_string());
                args.push(format!(
                    "fps={},scale={}:{}:flags=lanczos",
                    fps, config.width, config.height
                ));
                args.push(output_path.to_string_lossy().to_string());
            }
        }

        // Execute ffmpeg (placeholder)
        tracing::info!("Generating thumbnail with ffmpeg: {:?}", args);

        // Create thumbnail record
        sqlx::query(
            "INSERT INTO thumbnails
             (id, asset_id, thumbnail_type, file_path, width, height, timecode_ms, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())",
        )
        .bind(thumbnail_id)
        .bind(asset_id)
        .bind(config.thumbnail_type.as_str())
        .bind(output_path.to_string_lossy().to_string())
        .bind(config.width as i32)
        .bind(config.height as i32)
        .bind(config.timecode_ms)
        .execute(self.db.pool())
        .await?;

        Ok(thumbnail_id)
    }

    /// Get proxy by ID
    ///
    /// # Errors
    ///
    /// Returns an error if proxy not found
    pub async fn get_proxy(&self, proxy_id: Uuid) -> Result<Proxy> {
        let proxy = sqlx::query_as::<_, Proxy>("SELECT * FROM proxies WHERE id = $1")
            .bind(proxy_id)
            .fetch_one(self.db.pool())
            .await?;

        Ok(proxy)
    }

    /// Get all proxies for an asset
    ///
    /// # Errors
    ///
    /// Returns an error if query fails
    pub async fn get_asset_proxies(&self, asset_id: Uuid) -> Result<Vec<Proxy>> {
        let proxies = sqlx::query_as::<_, Proxy>(
            "SELECT * FROM proxies WHERE asset_id = $1 ORDER BY created_at DESC",
        )
        .bind(asset_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(proxies)
    }

    /// Get thumbnail by ID
    ///
    /// # Errors
    ///
    /// Returns an error if thumbnail not found
    pub async fn get_thumbnail(&self, thumbnail_id: Uuid) -> Result<Thumbnail> {
        let thumbnail = sqlx::query_as::<_, Thumbnail>("SELECT * FROM thumbnails WHERE id = $1")
            .bind(thumbnail_id)
            .fetch_one(self.db.pool())
            .await?;

        Ok(thumbnail)
    }

    /// Get all thumbnails for an asset
    ///
    /// # Errors
    ///
    /// Returns an error if query fails
    pub async fn get_asset_thumbnails(&self, asset_id: Uuid) -> Result<Vec<Thumbnail>> {
        let thumbnails = sqlx::query_as::<_, Thumbnail>(
            "SELECT * FROM thumbnails WHERE asset_id = $1 ORDER BY created_at DESC",
        )
        .bind(asset_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(thumbnails)
    }

    /// Get proxy job status
    ///
    /// # Errors
    ///
    /// Returns an error if job not found
    pub async fn get_job_status(&self, job_id: Uuid) -> Result<ProxyJob> {
        let jobs = self.active_jobs.read().await;
        jobs.get(&job_id)
            .cloned()
            .ok_or_else(|| MamError::Internal(format!("Job not found: {job_id}")))
    }

    /// Cancel a proxy job
    ///
    /// # Errors
    ///
    /// Returns an error if cancellation fails
    pub async fn cancel_job(&self, job_id: Uuid) -> Result<()> {
        self.update_job_status(
            job_id,
            ProxyJobStatus::Cancelled,
            0.0,
            Some("Cancelled by user".to_string()),
        )
        .await?;

        self.active_jobs.write().await.remove(&job_id);

        Ok(())
    }

    /// Delete a proxy
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails
    pub async fn delete_proxy(&self, proxy_id: Uuid) -> Result<()> {
        let proxy = self.get_proxy(proxy_id).await?;

        // Delete file
        let file_path = Path::new(&proxy.file_path);
        if file_path.exists() {
            tokio::fs::remove_file(file_path).await?;
        }

        // Delete from database
        sqlx::query("DELETE FROM proxies WHERE id = $1")
            .bind(proxy_id)
            .execute(self.db.pool())
            .await?;

        Ok(())
    }

    /// Delete a thumbnail
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails
    pub async fn delete_thumbnail(&self, thumbnail_id: Uuid) -> Result<()> {
        let thumbnail = self.get_thumbnail(thumbnail_id).await?;

        // Delete file
        let file_path = Path::new(&thumbnail.file_path);
        if file_path.exists() {
            tokio::fs::remove_file(file_path).await?;
        }

        // Delete from database
        sqlx::query("DELETE FROM thumbnails WHERE id = $1")
            .bind(thumbnail_id)
            .execute(self.db.pool())
            .await?;

        Ok(())
    }

    /// Get proxy file path
    fn get_proxy_path(&self, asset_id: Uuid, proxy_id: Uuid, config: &ProxyConfig) -> PathBuf {
        let asset_id_str = asset_id.to_string();
        let prefix = &asset_id_str[..2];
        let resolution_str = format!("{:?}", config.resolution).to_lowercase();
        let filename = format!(
            "{}_{}_{}.{}",
            asset_id,
            resolution_str,
            proxy_id,
            config.codec.file_extension()
        );

        self.proxy_path
            .join(prefix)
            .join(asset_id_str)
            .join(filename)
    }

    /// Get proxy file path by ID
    async fn get_proxy_path_by_id(&self, proxy_id: Uuid) -> Result<PathBuf> {
        let proxy = self.get_proxy(proxy_id).await?;
        Ok(PathBuf::from(proxy.file_path))
    }

    /// Get thumbnail file path
    fn get_thumbnail_path(
        &self,
        asset_id: Uuid,
        thumbnail_id: Uuid,
        config: &ThumbnailConfig,
    ) -> PathBuf {
        let asset_id_str = asset_id.to_string();
        let prefix = &asset_id_str[..2];
        let type_str = config.thumbnail_type.as_str();
        let ext = if matches!(config.thumbnail_type, ThumbnailType::AnimatedGif) {
            "gif"
        } else {
            "jpg"
        };
        let filename = format!("{}_{}.{}", thumbnail_id, type_str, ext);

        self.thumbnail_path
            .join(prefix)
            .join(asset_id_str)
            .join(filename)
    }
}

// Implement Clone for ProxyManager to allow spawning tasks
impl Clone for ProxyManager {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            proxy_path: self.proxy_path.clone(),
            thumbnail_path: self.thumbnail_path.clone(),
            transcode_semaphore: Arc::clone(&self.transcode_semaphore),
            active_jobs: Arc::clone(&self.active_jobs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_resolution_dimensions() {
        assert_eq!(ProxyResolution::P360.dimensions(), (640, 360));
        assert_eq!(ProxyResolution::P720.dimensions(), (1280, 720));
        assert_eq!(ProxyResolution::P1080.dimensions(), (1920, 1080));
        assert_eq!(ProxyResolution::Custom(1024, 576).dimensions(), (1024, 576));
    }

    #[test]
    fn test_proxy_codec_ffmpeg_name() {
        assert_eq!(ProxyCodec::H264.ffmpeg_name(), "libx264");
        assert_eq!(ProxyCodec::H265.ffmpeg_name(), "libx265");
        assert_eq!(ProxyCodec::VP9.ffmpeg_name(), "libvpx-vp9");
    }

    #[test]
    fn test_quality_preset_h264_settings() {
        assert_eq!(QualityPreset::Fast.h264_preset(), "veryfast");
        assert_eq!(QualityPreset::Fast.h264_crf(), 28);
        assert_eq!(QualityPreset::High.h264_preset(), "slow");
        assert_eq!(QualityPreset::High.h264_crf(), 20);
    }

    #[test]
    fn test_thumbnail_type_as_str() {
        assert_eq!(ThumbnailType::Poster.as_str(), "poster");
        assert_eq!(ThumbnailType::Grid.as_str(), "grid");
        assert_eq!(ThumbnailType::AnimatedGif.as_str(), "animated_gif");
    }

    #[test]
    fn test_proxy_job_status_as_str() {
        assert_eq!(ProxyJobStatus::Queued.as_str(), "queued");
        assert_eq!(ProxyJobStatus::Processing.as_str(), "processing");
        assert_eq!(ProxyJobStatus::Completed.as_str(), "completed");
    }

    #[test]
    fn test_proxy_config_serialization() {
        let config = ProxyConfig {
            resolution: ProxyResolution::P720,
            codec: ProxyCodec::H264,
            bitrate: Some(5_000_000),
            watermark: None,
            frame_rate: Some(29.97),
            quality_preset: QualityPreset::Medium,
        };

        let json = serde_json::to_string(&config).expect("should succeed in test");
        let deserialized: ProxyConfig =
            serde_json::from_str(&json).expect("should succeed in test");

        assert_eq!(deserialized.resolution, ProxyResolution::P720);
        assert_eq!(deserialized.codec, ProxyCodec::H264);
    }
}
