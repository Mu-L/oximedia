//! Comprehensive media analysis and quality assessment for `OxiMedia`.
//!
//! This crate provides professional-grade tools for analyzing video and audio content,
//! detecting quality issues, classifying content, and generating detailed reports.
//!
//! # Features
//!
//! ## Video Analysis
//!
//! - **Scene Detection** - Shot boundary detection (cuts, fades, dissolves, wipes)
//! - **Black Frame Detection** - Detect black or near-black frames
//! - **Quality Assessment** - Blockiness, blur, noise, artifacts
//! - **Content Classification** - Action, still, talking head, sports, animation
//! - **Thumbnail Generation** - Intelligent representative frame selection
//! - **Motion Analysis** - Motion vectors, camera motion estimation
//! - **Color Analysis** - Dominant colors, color grading, palette extraction
//! - **Temporal Analysis** - Flicker detection, judder, temporal artifacts
//!
//! ## Audio Analysis
//!
//! - **Silence Detection** - Detect silent or near-silent segments
//! - **Loudness Analysis** - ITU-R BS.1770-4 compliant (via oximedia-metering)
//! - **Clipping Detection** - Digital clipping and distortion
//! - **Phase Issues** - Phase correlation and mono compatibility
//! - **Spectral Analysis** - Frequency content, spectral flatness
//! - **Dynamic Range** - Peak-to-RMS ratio, crest factor
//!
//! ## Report Generation
//!
//! - **JSON Reports** - Machine-readable analysis results
//! - **HTML Reports** - Human-readable detailed reports
//! - **Timeline Visualization** - Scene markers, quality graphs
//!
//! # Example Usage
//!
//! ```rust,no_run
//! use oximedia_analysis::{Analyzer, AnalysisConfig};
//! use oximedia_core::types::Rational;
//!
//! // Create analyzer with default config
//! let config = AnalysisConfig::default()
//!     .with_scene_detection(true)
//!     .with_quality_assessment(true)
//!     .with_black_frame_detection(true);
//!
//! let mut analyzer = Analyzer::new(config);
//!
//! // Analyze frames (in a real scenario, these would come from a decoder)
//! # let frame_data: Vec<u8> = vec![0; 1920 * 1080 * 3];
//! # let audio_data: Vec<f32> = vec![0.0; 48000];
//! // analyzer.process_video_frame(&frame_data, 1920, 1080, Rational::new(1, 30))?;
//! // analyzer.process_audio_samples(&audio_data, 48000)?;
//!
//! // Get results
//! let results = analyzer.finalize();
//! println!("Detected {} scenes", results.scenes.len());
//! println!("Average quality score: {:.2}", results.quality_stats.average_score);
//!
//! // Generate report
//! let report_json = results.to_json()?;
//! let report_html = results.to_html()?;
//! # Ok::<(), oximedia_analysis::AnalysisError>(())
//! ```
//!
//! # Architecture
//!
//! The analyzer is built on a modular architecture:
//!
//! - **Scene Detector**: Histogram difference, edge change ratio, motion analysis
//! - **Quality Assessor**: Blockiness (DCT), blur (Laplacian), noise (spectral)
//! - **Content Classifier**: Motion patterns, temporal complexity, spatial features
//! - **Color Analyzer**: K-means clustering, histogram analysis
//! - **Audio Analyzer**: FFT-based spectral analysis, waveform analysis
//!
//! All analyzers operate on a single-pass basis where possible for efficiency.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::similar_names)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::struct_excessive_bools)]

pub mod action_detect;
pub mod audio;
pub mod audio_spectrum;
pub mod audio_video_sync;
pub mod black;
pub mod brand_detection;
pub mod color;
pub mod color_analysis;
pub mod complexity;
pub mod content;
pub mod content_complexity;
pub mod content_rating;
pub mod crowd_analysis;
pub mod edge_density;
pub mod event_detection;
pub mod facial_analysis;
pub mod flicker_detect;
pub mod frame_cadence;
pub mod histogram_analysis;
pub mod logo_detect;
pub mod motion;
pub mod motion_analysis;
pub mod noise_profile;
pub mod object_tracking;
pub mod quality;
pub mod quality_score;
pub mod report;
pub mod saliency_map;
pub mod scene;
pub mod scene_stats;
pub mod shot_list;
pub mod speech;
pub mod temporal;
pub mod temporal_analysis;
pub mod temporal_stats;
pub mod text_detection;
pub mod thumbnail;
pub mod utils;
pub mod visual_attention;

use oximedia_core::types::Rational;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

/// Custom serialization support for Rational.
mod rational_serde {
    use oximedia_core::types::Rational;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// Serialize Rational as a tuple of (num, den).
    pub fn serialize<S>(rational: &Rational, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (rational.num, rational.den).serialize(serializer)
    }

    /// Deserialize Rational from a tuple of (num, den).
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Rational, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (num, den) = <(i64, i64)>::deserialize(deserializer)?;
        Ok(Rational::new(num, den))
    }
}

/// Analysis error types.
#[derive(Error, Debug)]
pub enum AnalysisError {
    /// Invalid configuration
    #[error("Invalid analysis configuration: {0}")]
    InvalidConfig(String),

    /// Invalid input data
    #[error("Invalid input data: {0}")]
    InvalidInput(String),

    /// Processing error
    #[error("Processing error: {0}")]
    ProcessingError(String),

    /// Insufficient data for analysis
    #[error("Insufficient data: {0}")]
    InsufficientData(String),

    /// Report generation error
    #[error("Report generation error: {0}")]
    ReportError(String),

    /// Core library error
    #[error("Core error: {0}")]
    CoreError(#[from] oximedia_core::OxiError),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

/// Result type for analysis operations.
pub type AnalysisResult<T> = Result<T, AnalysisError>;

/// Analysis configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// Enable scene detection
    pub scene_detection: bool,
    /// Scene detection sensitivity (0.0-1.0)
    pub scene_threshold: f64,

    /// Enable black frame detection
    pub black_detection: bool,
    /// Black frame threshold (0-255)
    pub black_threshold: u8,
    /// Minimum black frame duration (frames)
    pub black_min_duration: usize,

    /// Enable quality assessment
    pub quality_assessment: bool,

    /// Enable content classification
    pub content_classification: bool,

    /// Enable thumbnail generation
    pub thumbnail_generation: bool,
    /// Number of thumbnails to generate
    pub thumbnail_count: usize,

    /// Enable motion analysis
    pub motion_analysis: bool,

    /// Enable color analysis
    pub color_analysis: bool,
    /// Number of dominant colors to extract
    pub dominant_color_count: usize,

    /// Enable audio analysis
    pub audio_analysis: bool,
    /// Silence detection threshold (dB)
    pub silence_threshold_db: f64,
    /// Minimum silence duration
    pub silence_min_duration: Duration,

    /// Enable temporal analysis
    pub temporal_analysis: bool,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            scene_detection: true,
            scene_threshold: 0.3,
            black_detection: true,
            black_threshold: 16,
            black_min_duration: 10,
            quality_assessment: true,
            content_classification: false,
            thumbnail_generation: false,
            thumbnail_count: 10,
            motion_analysis: false,
            color_analysis: false,
            dominant_color_count: 5,
            audio_analysis: false,
            silence_threshold_db: -60.0,
            silence_min_duration: Duration::from_millis(500),
            temporal_analysis: false,
        }
    }
}

impl AnalysisConfig {
    /// Create a new configuration with all features disabled.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scene_detection: false,
            scene_threshold: 0.3,
            black_detection: false,
            black_threshold: 16,
            black_min_duration: 10,
            quality_assessment: false,
            content_classification: false,
            thumbnail_generation: false,
            thumbnail_count: 10,
            motion_analysis: false,
            color_analysis: false,
            dominant_color_count: 5,
            audio_analysis: false,
            silence_threshold_db: -60.0,
            silence_min_duration: Duration::from_millis(500),
            temporal_analysis: false,
        }
    }

    /// Enable scene detection.
    #[must_use]
    pub fn with_scene_detection(mut self, enabled: bool) -> Self {
        self.scene_detection = enabled;
        self
    }

    /// Enable black frame detection.
    #[must_use]
    pub fn with_black_frame_detection(mut self, enabled: bool) -> Self {
        self.black_detection = enabled;
        self
    }

    /// Enable quality assessment.
    #[must_use]
    pub fn with_quality_assessment(mut self, enabled: bool) -> Self {
        self.quality_assessment = enabled;
        self
    }

    /// Enable content classification.
    #[must_use]
    pub fn with_content_classification(mut self, enabled: bool) -> Self {
        self.content_classification = enabled;
        self
    }

    /// Enable thumbnail generation.
    #[must_use]
    pub fn with_thumbnail_generation(mut self, count: usize) -> Self {
        self.thumbnail_generation = true;
        self.thumbnail_count = count;
        self
    }

    /// Enable motion analysis.
    #[must_use]
    pub fn with_motion_analysis(mut self, enabled: bool) -> Self {
        self.motion_analysis = enabled;
        self
    }

    /// Enable color analysis.
    #[must_use]
    pub fn with_color_analysis(mut self, enabled: bool) -> Self {
        self.color_analysis = enabled;
        self
    }

    /// Enable audio analysis.
    #[must_use]
    pub fn with_audio_analysis(mut self, enabled: bool) -> Self {
        self.audio_analysis = enabled;
        self
    }

    /// Enable temporal analysis.
    #[must_use]
    pub fn with_temporal_analysis(mut self, enabled: bool) -> Self {
        self.temporal_analysis = enabled;
        self
    }
}

/// Main analyzer interface.
pub struct Analyzer {
    #[allow(dead_code)]
    config: AnalysisConfig,
    scene_detector: Option<scene::SceneDetector>,
    black_detector: Option<black::BlackFrameDetector>,
    quality_assessor: Option<quality::QualityAssessor>,
    content_classifier: Option<content::ContentClassifier>,
    thumbnail_selector: Option<thumbnail::ThumbnailSelector>,
    motion_analyzer: Option<motion::MotionAnalyzer>,
    color_analyzer: Option<color::ColorAnalyzer>,
    audio_analyzer: Option<audio::AudioAnalyzer>,
    temporal_analyzer: Option<temporal::TemporalAnalyzer>,
    frame_count: usize,
    frame_rate: Rational,
}

impl Analyzer {
    /// Create a new analyzer with the given configuration.
    #[must_use]
    pub fn new(config: AnalysisConfig) -> Self {
        let scene_detector = if config.scene_detection {
            Some(scene::SceneDetector::new(config.scene_threshold))
        } else {
            None
        };

        let black_detector = if config.black_detection {
            Some(black::BlackFrameDetector::new(
                config.black_threshold,
                config.black_min_duration,
            ))
        } else {
            None
        };

        let quality_assessor = if config.quality_assessment {
            Some(quality::QualityAssessor::new())
        } else {
            None
        };

        let content_classifier = if config.content_classification {
            Some(content::ContentClassifier::new())
        } else {
            None
        };

        let thumbnail_selector = if config.thumbnail_generation {
            Some(thumbnail::ThumbnailSelector::new(config.thumbnail_count))
        } else {
            None
        };

        let motion_analyzer = if config.motion_analysis {
            Some(motion::MotionAnalyzer::new())
        } else {
            None
        };

        let color_analyzer = if config.color_analysis {
            Some(color::ColorAnalyzer::new(config.dominant_color_count))
        } else {
            None
        };

        let audio_analyzer = if config.audio_analysis {
            Some(audio::AudioAnalyzer::new(
                config.silence_threshold_db,
                config.silence_min_duration,
            ))
        } else {
            None
        };

        let temporal_analyzer = if config.temporal_analysis {
            Some(temporal::TemporalAnalyzer::new())
        } else {
            None
        };

        Self {
            config,
            scene_detector,
            black_detector,
            quality_assessor,
            content_classifier,
            thumbnail_selector,
            motion_analyzer,
            color_analyzer,
            audio_analyzer,
            temporal_analyzer,
            frame_count: 0,
            frame_rate: Rational::new(25, 1),
        }
    }

    /// Process a video frame (`YUV420p` format).
    pub fn process_video_frame(
        &mut self,
        y_plane: &[u8],
        u_plane: &[u8],
        v_plane: &[u8],
        width: usize,
        height: usize,
        frame_rate: Rational,
    ) -> AnalysisResult<()> {
        self.frame_rate = frame_rate;

        if let Some(ref mut detector) = self.scene_detector {
            detector.process_frame(y_plane, width, height, self.frame_count)?;
        }

        if let Some(ref mut detector) = self.black_detector {
            detector.process_frame(y_plane, width, height, self.frame_count)?;
        }

        if let Some(ref mut assessor) = self.quality_assessor {
            assessor.process_frame(y_plane, width, height, self.frame_count)?;
        }

        if let Some(ref mut classifier) = self.content_classifier {
            classifier.process_frame(y_plane, width, height, self.frame_count)?;
        }

        if let Some(ref mut selector) = self.thumbnail_selector {
            selector.process_frame(y_plane, width, height, self.frame_count)?;
        }

        if let Some(ref mut analyzer) = self.motion_analyzer {
            analyzer.process_frame(y_plane, width, height, self.frame_count)?;
        }

        if let Some(ref mut analyzer) = self.color_analyzer {
            analyzer.process_frame(y_plane, u_plane, v_plane, width, height, self.frame_count)?;
        }

        if let Some(ref mut analyzer) = self.temporal_analyzer {
            analyzer.process_frame(y_plane, width, height, self.frame_count)?;
        }

        self.frame_count += 1;
        Ok(())
    }

    /// Process audio samples (interleaved f32).
    pub fn process_audio_samples(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> AnalysisResult<()> {
        if let Some(ref mut analyzer) = self.audio_analyzer {
            analyzer.process_samples(samples, sample_rate)?;
        }
        Ok(())
    }

    /// Finalize analysis and get results.
    pub fn finalize(self) -> AnalysisResults {
        AnalysisResults {
            frame_count: self.frame_count,
            frame_rate: self.frame_rate,
            scenes: self
                .scene_detector
                .map_or_else(Vec::new, scene::SceneDetector::finalize),
            black_frames: self
                .black_detector
                .map_or_else(Vec::new, black::BlackFrameDetector::finalize),
            quality_stats: self.quality_assessor.map_or_else(
                quality::QualityStats::default,
                quality::QualityAssessor::finalize,
            ),
            content_classification: self
                .content_classifier
                .map(content::ContentClassifier::finalize),
            thumbnails: self
                .thumbnail_selector
                .map_or_else(Vec::new, thumbnail::ThumbnailSelector::finalize),
            motion_stats: self.motion_analyzer.map(motion::MotionAnalyzer::finalize),
            color_analysis: self.color_analyzer.map(color::ColorAnalyzer::finalize),
            audio_analysis: self.audio_analyzer.map(audio::AudioAnalyzer::finalize),
            temporal_analysis: self
                .temporal_analyzer
                .map(temporal::TemporalAnalyzer::finalize),
        }
    }
}

/// Complete analysis results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResults {
    /// Total number of frames analyzed
    pub frame_count: usize,
    /// Frame rate
    #[serde(with = "rational_serde")]
    pub frame_rate: Rational,
    /// Detected scenes
    pub scenes: Vec<scene::Scene>,
    /// Detected black frames
    pub black_frames: Vec<black::BlackSegment>,
    /// Quality statistics
    pub quality_stats: quality::QualityStats,
    /// Content classification results
    pub content_classification: Option<content::ContentClassification>,
    /// Selected thumbnail frames
    pub thumbnails: Vec<thumbnail::ThumbnailInfo>,
    /// Motion analysis statistics
    pub motion_stats: Option<motion::MotionStats>,
    /// Color analysis results
    pub color_analysis: Option<color::ColorAnalysis>,
    /// Audio analysis results
    pub audio_analysis: Option<audio::AudioAnalysis>,
    /// Temporal analysis results
    pub temporal_analysis: Option<temporal::TemporalAnalysis>,
}

impl AnalysisResults {
    /// Convert results to JSON.
    pub fn to_json(&self) -> AnalysisResult<String> {
        serde_json::to_string_pretty(self).map_err(Into::into)
    }

    /// Convert results to HTML report.
    pub fn to_html(&self) -> AnalysisResult<String> {
        report::generate_html_report(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = AnalysisConfig::new()
            .with_scene_detection(true)
            .with_quality_assessment(true);

        assert!(config.scene_detection);
        assert!(config.quality_assessment);
        assert!(!config.black_detection);
    }

    #[test]
    fn test_analyzer_creation() {
        let config = AnalysisConfig::default();
        let analyzer = Analyzer::new(config);
        assert_eq!(analyzer.frame_count, 0);
    }

    #[test]
    fn test_empty_analysis() {
        let config = AnalysisConfig::new();
        let analyzer = Analyzer::new(config);
        let results = analyzer.finalize();
        assert_eq!(results.frame_count, 0);
        assert!(results.scenes.is_empty());
    }
}
