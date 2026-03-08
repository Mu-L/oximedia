//! High-level transcoding pipeline for `OxiMedia`.
//!
//! This crate provides a comprehensive, professional-grade transcoding system with:
//!
//! # Features
//!
//! ## High-Level API
//!
//! - **Simple One-Liner Transcoding** - Quick transcoding with sensible defaults
//! - **Preset Library** - Industry-standard presets for major platforms
//! - **Fluent Builder API** - Complex workflows with readable code
//!
//! ## Professional Features
//!
//! - **Multi-Pass Encoding** - 2-pass and 3-pass encoding for optimal quality
//! - **ABR Ladder Generation** - Adaptive bitrate encoding for HLS/DASH
//! - **Parallel Encoding** - Encode multiple outputs simultaneously
//! - **Hardware Acceleration** - Auto-detection and use of GPU encoders
//! - **Progress Tracking** - Real-time progress with ETA estimation
//! - **Audio Normalization** - Automatic loudness normalization (EBU R128/ATSC A/85)
//! - **Quality Control** - CRF, CBR, VBR, and constrained VBR modes
//! - **Subtitle Support** - Burn-in or soft subtitle embedding
//! - **Chapter Markers** - Preserve or add chapter information
//! - **Metadata Preservation** - Copy or map metadata fields
//!
//! ## Job Management
//!
//! - **Job Queuing** - Queue multiple transcode jobs
//! - **Priority Scheduling** - High, normal, and low priority jobs
//! - **Resource Management** - CPU/GPU limits and throttling
//! - **Error Recovery** - Automatic retry logic with exponential backoff
//! - **Validation** - Input/output validation before processing
//!
//! # Supported Platforms
//!
//! ## Streaming Platforms
//!
//! - **`YouTube`** - 1080p60, 4K, VP9/H.264 variants
//! - **Vimeo** - Professional quality presets
//! - **Twitch** - Live streaming optimized
//! - **Social Media** - Instagram, `TikTok`, Twitter optimized
//!
//! ## Broadcast
//!
//! - **`ProRes` Proxy** - High-quality editing proxies
//! - **`DNxHD` Proxy** - Avid editing proxies
//! - **Broadcast HD/4K** - Broadcast-ready deliverables
//!
//! ## Streaming Protocols
//!
//! - **HLS** - HTTP Live Streaming ABR ladders
//! - **DASH** - MPEG-DASH ABR ladders
//! - **CMAF** - Common Media Application Format
//!
//! ## Archive
//!
//! - **Lossless** - FFV1 lossless preservation
//! - **High Quality** - VP9/AV1 archival encoding
//!
//! # Quick Start
//!
//! ## Simple Transcoding
//!
//! ```rust,no_run
//! use oximedia_transcode::{Transcoder, presets};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Simple transcode to YouTube 1080p
//! Transcoder::new()
//!     .input("input.mp4")
//!     .output("output.mp4")
//!     .preset(presets::youtube::youtube_1080p())
//!     .transcode()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Complex Pipeline
//!
//! ```rust,ignore
//! use oximedia_transcode::{TranscodePipeline, Quality};
//! use oximedia_transcode::presets::streaming;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create HLS ABR ladder with multiple qualities
//! TranscodePipeline::builder()
//!     .input("source.mp4")
//!     .abr_ladder(streaming::hls_ladder())
//!     .audio_normalize(true)
//!     .quality(Quality::High)
//!     .parallel_encode(true)
//!     .progress(|p| {
//!         println!("Progress: {}% - ETA: {:?}", p.percent, p.eta);
//!     })
//!     .execute()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Multi-Pass Encoding
//!
//! ```rust,no_run
//! use oximedia_transcode::{Transcoder, MultiPassMode};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // 2-pass encoding for optimal quality
//! Transcoder::new()
//!     .input("input.mp4")
//!     .output("output.webm")
//!     .multi_pass(MultiPassMode::TwoPass)
//!     .target_bitrate(5_000_000) // 5 Mbps
//!     .transcode()
//!     .await?;
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::too_many_arguments)]

mod abr;
pub mod adaptive_bitrate;
pub mod audio_transcode;
pub mod bitrate_estimator;
mod builder;
mod codec_config;
pub mod codec_mapping;
pub mod crf_optimizer;
mod filters;
mod hw_accel;
mod multipass;
mod normalization;
mod parallel;
mod pipeline;
mod progress;
mod quality;
pub mod segment_encoder;
pub mod segment_transcoder;
pub mod thumbnail;
mod transcode_job;
pub mod two_pass;
pub mod validation;

pub mod ab_compare;
pub mod abr_ladder;
pub mod audio_channel_map;
pub mod bitrate_control;
pub mod burn_subs;
pub mod codec_profile;
/// Concatenation and joining of multiple media sources.
pub mod concat_transcode;
pub mod crop_scale;
pub mod encoding_log;
pub mod examples;
pub mod frame_stats;
pub mod output_verify;
pub mod presets;
/// Rate-distortion analysis for optimal encoding parameter selection.
pub mod rate_distortion;
pub mod resolution_select;
pub mod scene_cut;
pub mod stage_graph;
pub mod transcode_metrics;
pub mod transcode_session;
pub mod utils;
/// Watermark and graphic overlay embedding during transcoding.
pub mod watermark_overlay;
pub use codec_config::{
    codec_config_from_quality, Av1Config, Av1Usage, CodecConfig, H264Config, H264Profile,
    OpusApplication, OpusConfig, Vp9Config,
};
pub use filters::{AudioFilter, FilterNode, VideoFilter};
pub use hw_accel::{
    detect_available_hw_accel, detect_best_hw_accel_for_codec, get_available_encoders,
    HwAccelConfig, HwAccelType, HwEncoder, HwFeature,
};

pub use abr::{AbrLadder, AbrLadderBuilder, AbrRung, AbrStrategy};
pub use builder::TranscodeBuilder;
pub use multipass::{MultiPassConfig, MultiPassEncoder, MultiPassMode};
pub use normalization::{AudioNormalizer, LoudnessStandard, LoudnessTarget, NormalizationConfig};
pub use parallel::{ParallelConfig, ParallelEncodeBuilder, ParallelEncoder};
pub use pipeline::{Pipeline, PipelineStage, TranscodePipeline};
pub use progress::{ProgressCallback, ProgressInfo, ProgressTracker};
pub use quality::{QualityConfig, QualityMode, QualityPreset, RateControlMode, TuneMode};
pub use transcode_job::{JobPriority, JobQueue, TranscodeJob, TranscodeJobConfig, TranscodeStatus};
pub use validation::{InputValidator, OutputValidator, ValidationError};

use thiserror::Error;

/// Errors that can occur during transcoding operations.
#[derive(Debug, Clone, Error)]
pub enum TranscodeError {
    /// Invalid input file or format.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Invalid output configuration.
    #[error("Invalid output: {0}")]
    InvalidOutput(String),

    /// Codec error during encoding/decoding.
    #[error("Codec error: {0}")]
    CodecError(String),

    /// Container format error.
    #[error("Container error: {0}")]
    ContainerError(String),

    /// I/O error during transcoding.
    #[error("I/O error: {0}")]
    IoError(String),

    /// Pipeline execution error.
    #[error("Pipeline error: {0}")]
    PipelineError(String),

    /// Multi-pass encoding error.
    #[error("Multi-pass error: {0}")]
    MultiPassError(String),

    /// Audio normalization error.
    #[error("Normalization error: {0}")]
    NormalizationError(String),

    /// Validation error.
    #[error("Validation error: {0}")]
    ValidationError(#[from] ValidationError),

    /// Job execution error.
    #[error("Job error: {0}")]
    JobError(String),

    /// Unsupported operation or feature.
    #[error("Unsupported: {0}")]
    Unsupported(String),
}

impl From<std::io::Error> for TranscodeError {
    fn from(err: std::io::Error) -> Self {
        TranscodeError::IoError(err.to_string())
    }
}

/// Result type for transcoding operations.
pub type Result<T> = std::result::Result<T, TranscodeError>;

/// Main transcoding interface with simple API.
///
/// # Example
///
/// ```rust,no_run
/// use oximedia_transcode::Transcoder;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// Transcoder::new()
///     .input("input.mp4")
///     .output("output.webm")
///     .video_codec("vp9")
///     .audio_codec("opus")
///     .transcode()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct Transcoder {
    config: TranscodeConfig,
}

/// Transcoding configuration.
#[derive(Debug, Clone)]
pub struct TranscodeConfig {
    /// Input file path.
    pub input: Option<String>,
    /// Output file path.
    pub output: Option<String>,
    /// Video codec name.
    pub video_codec: Option<String>,
    /// Audio codec name.
    pub audio_codec: Option<String>,
    /// Target video bitrate in bits per second.
    pub video_bitrate: Option<u64>,
    /// Target audio bitrate in bits per second.
    pub audio_bitrate: Option<u64>,
    /// Video width in pixels.
    pub width: Option<u32>,
    /// Video height in pixels.
    pub height: Option<u32>,
    /// Frame rate as a rational number (numerator, denominator).
    pub frame_rate: Option<(u32, u32)>,
    /// Multi-pass encoding mode.
    pub multi_pass: Option<MultiPassMode>,
    /// Quality mode for encoding.
    pub quality_mode: Option<QualityMode>,
    /// Enable audio normalization.
    pub normalize_audio: bool,
    /// Loudness normalization standard.
    pub loudness_standard: Option<LoudnessStandard>,
    /// Enable hardware acceleration.
    pub hw_accel: bool,
    /// Preserve metadata from input.
    pub preserve_metadata: bool,
    /// Subtitle handling mode.
    pub subtitle_mode: Option<SubtitleMode>,
    /// Chapter handling mode.
    pub chapter_mode: Option<ChapterMode>,
}

/// Subtitle handling modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleMode {
    /// Ignore subtitles.
    Ignore,
    /// Copy subtitles as separate stream.
    Copy,
    /// Burn subtitles into video.
    BurnIn,
}

/// Chapter handling modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterMode {
    /// Ignore chapters.
    Ignore,
    /// Copy chapters from input.
    Copy,
    /// Add custom chapters.
    Custom,
}

impl Default for TranscodeConfig {
    fn default() -> Self {
        Self {
            input: None,
            output: None,
            video_codec: None,
            audio_codec: None,
            video_bitrate: None,
            audio_bitrate: None,
            width: None,
            height: None,
            frame_rate: None,
            multi_pass: None,
            quality_mode: None,
            normalize_audio: false,
            loudness_standard: None,
            hw_accel: true,
            preserve_metadata: true,
            subtitle_mode: None,
            chapter_mode: None,
        }
    }
}

impl Transcoder {
    /// Get a reference to the transcoder configuration.
    #[must_use]
    pub fn config(&self) -> &TranscodeConfig {
        &self.config
    }

    /// Creates a new transcoder with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: TranscodeConfig::default(),
        }
    }

    /// Sets the input file path.
    #[must_use]
    pub fn input(mut self, path: impl Into<String>) -> Self {
        self.config.input = Some(path.into());
        self
    }

    /// Sets the output file path.
    #[must_use]
    pub fn output(mut self, path: impl Into<String>) -> Self {
        self.config.output = Some(path.into());
        self
    }

    /// Sets the video codec.
    #[must_use]
    pub fn video_codec(mut self, codec: impl Into<String>) -> Self {
        self.config.video_codec = Some(codec.into());
        self
    }

    /// Sets the audio codec.
    #[must_use]
    pub fn audio_codec(mut self, codec: impl Into<String>) -> Self {
        self.config.audio_codec = Some(codec.into());
        self
    }

    /// Sets the target video bitrate.
    #[must_use]
    pub fn video_bitrate(mut self, bitrate: u64) -> Self {
        self.config.video_bitrate = Some(bitrate);
        self
    }

    /// Sets the target audio bitrate.
    #[must_use]
    pub fn audio_bitrate(mut self, bitrate: u64) -> Self {
        self.config.audio_bitrate = Some(bitrate);
        self
    }

    /// Sets the output resolution.
    #[must_use]
    pub fn resolution(mut self, width: u32, height: u32) -> Self {
        self.config.width = Some(width);
        self.config.height = Some(height);
        self
    }

    /// Sets the output frame rate.
    #[must_use]
    pub fn frame_rate(mut self, num: u32, den: u32) -> Self {
        self.config.frame_rate = Some((num, den));
        self
    }

    /// Sets the multi-pass encoding mode.
    #[must_use]
    pub fn multi_pass(mut self, mode: MultiPassMode) -> Self {
        self.config.multi_pass = Some(mode);
        self
    }

    /// Sets the quality mode.
    #[must_use]
    pub fn quality(mut self, mode: QualityMode) -> Self {
        self.config.quality_mode = Some(mode);
        self
    }

    /// Sets the target bitrate (convenience method for video bitrate).
    #[must_use]
    pub fn target_bitrate(mut self, bitrate: u64) -> Self {
        self.config.video_bitrate = Some(bitrate);
        self
    }

    /// Enables or disables audio normalization.
    #[must_use]
    pub fn normalize_audio(mut self, enable: bool) -> Self {
        self.config.normalize_audio = enable;
        self
    }

    /// Sets the loudness normalization standard.
    #[must_use]
    pub fn loudness_standard(mut self, standard: LoudnessStandard) -> Self {
        self.config.loudness_standard = Some(standard);
        self.config.normalize_audio = true;
        self
    }

    /// Enables or disables hardware acceleration.
    #[must_use]
    pub fn hw_accel(mut self, enable: bool) -> Self {
        self.config.hw_accel = enable;
        self
    }

    /// Applies a preset configuration.
    #[must_use]
    pub fn preset(mut self, preset: PresetConfig) -> Self {
        if let Some(codec) = preset.video_codec {
            self.config.video_codec = Some(codec);
        }
        if let Some(codec) = preset.audio_codec {
            self.config.audio_codec = Some(codec);
        }
        if let Some(bitrate) = preset.video_bitrate {
            self.config.video_bitrate = Some(bitrate);
        }
        if let Some(bitrate) = preset.audio_bitrate {
            self.config.audio_bitrate = Some(bitrate);
        }
        if let Some(width) = preset.width {
            self.config.width = Some(width);
        }
        if let Some(height) = preset.height {
            self.config.height = Some(height);
        }
        if let Some(fps) = preset.frame_rate {
            self.config.frame_rate = Some(fps);
        }
        if let Some(mode) = preset.quality_mode {
            self.config.quality_mode = Some(mode);
        }
        self
    }

    /// Executes the transcode operation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Input or output path is not set
    /// - Input file is invalid or cannot be opened
    /// - Output configuration is invalid
    /// - Transcoding fails
    pub async fn transcode(self) -> Result<TranscodeOutput> {
        // Validate configuration
        let input = self
            .config
            .input
            .ok_or_else(|| TranscodeError::InvalidInput("No input file specified".to_string()))?;
        let output = self
            .config
            .output
            .ok_or_else(|| TranscodeError::InvalidOutput("No output file specified".to_string()))?;

        // Create a basic pipeline and execute
        let mut pipeline = TranscodePipeline::builder()
            .input(&input)
            .output(&output)
            .build()?;

        // Apply configuration to pipeline
        if let Some(codec) = &self.config.video_codec {
            pipeline.set_video_codec(codec);
        }
        if let Some(codec) = &self.config.audio_codec {
            pipeline.set_audio_codec(codec);
        }

        // Execute pipeline
        pipeline.execute().await
    }
}

impl Default for Transcoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Preset configuration for common transcoding scenarios.
#[derive(Debug, Clone, Default)]
pub struct PresetConfig {
    /// Video codec name.
    pub video_codec: Option<String>,
    /// Audio codec name.
    pub audio_codec: Option<String>,
    /// Video bitrate.
    pub video_bitrate: Option<u64>,
    /// Audio bitrate.
    pub audio_bitrate: Option<u64>,
    /// Video width.
    pub width: Option<u32>,
    /// Video height.
    pub height: Option<u32>,
    /// Frame rate.
    pub frame_rate: Option<(u32, u32)>,
    /// Quality mode.
    pub quality_mode: Option<QualityMode>,
    /// Container format.
    pub container: Option<String>,
}

/// Output from a successful transcode operation.
#[derive(Debug, Clone)]
pub struct TranscodeOutput {
    /// Output file path.
    pub output_path: String,
    /// File size in bytes.
    pub file_size: u64,
    /// Duration in seconds.
    pub duration: f64,
    /// Video bitrate in bits per second.
    pub video_bitrate: u64,
    /// Audio bitrate in bits per second.
    pub audio_bitrate: u64,
    /// Actual encoding time in seconds.
    pub encoding_time: f64,
    /// Speed factor (input duration / encoding time).
    pub speed_factor: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcoder_builder() {
        let transcoder = Transcoder::new()
            .input("input.mp4")
            .output("output.webm")
            .video_codec("vp9")
            .audio_codec("opus")
            .resolution(1920, 1080)
            .frame_rate(30, 1);

        assert_eq!(transcoder.config.input, Some("input.mp4".to_string()));
        assert_eq!(transcoder.config.output, Some("output.webm".to_string()));
        assert_eq!(transcoder.config.video_codec, Some("vp9".to_string()));
        assert_eq!(transcoder.config.audio_codec, Some("opus".to_string()));
        assert_eq!(transcoder.config.width, Some(1920));
        assert_eq!(transcoder.config.height, Some(1080));
        assert_eq!(transcoder.config.frame_rate, Some((30, 1)));
    }

    #[test]
    fn test_default_config() {
        let config = TranscodeConfig::default();
        assert!(config.input.is_none());
        assert!(config.output.is_none());
        assert!(config.hw_accel);
        assert!(config.preserve_metadata);
        assert!(!config.normalize_audio);
    }

    #[test]
    fn test_preset_application() {
        let preset = PresetConfig {
            video_codec: Some("vp9".to_string()),
            audio_codec: Some("opus".to_string()),
            video_bitrate: Some(5_000_000),
            audio_bitrate: Some(128_000),
            width: Some(1920),
            height: Some(1080),
            frame_rate: Some((60, 1)),
            quality_mode: Some(QualityMode::High),
            container: Some("webm".to_string()),
        };

        let transcoder = Transcoder::new().preset(preset);

        assert_eq!(transcoder.config.video_codec, Some("vp9".to_string()));
        assert_eq!(transcoder.config.audio_codec, Some("opus".to_string()));
        assert_eq!(transcoder.config.video_bitrate, Some(5_000_000));
        assert_eq!(transcoder.config.audio_bitrate, Some(128_000));
        assert_eq!(transcoder.config.width, Some(1920));
        assert_eq!(transcoder.config.height, Some(1080));
        assert_eq!(transcoder.config.frame_rate, Some((60, 1)));
        assert_eq!(transcoder.config.quality_mode, Some(QualityMode::High));
    }

    #[test]
    fn test_subtitle_modes() {
        assert_eq!(SubtitleMode::Ignore, SubtitleMode::Ignore);
        assert_ne!(SubtitleMode::Ignore, SubtitleMode::Copy);
        assert_ne!(SubtitleMode::Copy, SubtitleMode::BurnIn);
    }

    #[test]
    fn test_chapter_modes() {
        assert_eq!(ChapterMode::Ignore, ChapterMode::Ignore);
        assert_ne!(ChapterMode::Ignore, ChapterMode::Copy);
        assert_ne!(ChapterMode::Copy, ChapterMode::Custom);
    }
}
