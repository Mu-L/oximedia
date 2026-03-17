// Copyright 2025 OxiMedia Contributors
// Licensed under the Apache License, Version 2.0

//! Smart conversion features with automatic optimization.
//!
//! Provides content-aware codec selection that analyzes input media
//! characteristics (animation vs live action, resolution, frame rate, HDR)
//! and selects the optimal codec/container/settings automatically.

use crate::formats::{AudioCodec, ContainerFormat, VideoCodec};
use crate::pipeline::{AudioSettings, BitrateMode, VideoSettings};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Smart converter that automatically selects optimal settings.
#[derive(Debug, Clone)]
pub struct SmartConverter {
    analyzer: MediaAnalyzer,
    optimizer: SettingsOptimizer,
    content_classifier: ContentClassifier,
}

impl SmartConverter {
    /// Create a new smart converter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            analyzer: MediaAnalyzer::new(),
            optimizer: SettingsOptimizer::new(),
            content_classifier: ContentClassifier::new(),
        }
    }

    /// Analyze input and determine optimal conversion settings.
    pub async fn analyze_and_optimize(
        &self,
        input: &Path,
        target: ConversionTarget,
    ) -> Result<OptimizedSettings> {
        let analysis = self.analyzer.analyze(input).await?;
        self.optimizer.optimize(&analysis, target)
    }

    /// Analyze input and determine optimal settings using content-aware codec selection.
    ///
    /// This method goes beyond basic target-based optimization by classifying
    /// the content type (animation, live action, screen recording, etc.) and
    /// selecting the most appropriate codec for each content type.
    pub async fn smart_optimize(
        &self,
        input: &Path,
        target: ConversionTarget,
    ) -> Result<OptimizedSettings> {
        let analysis = self.analyzer.analyze(input).await?;
        let content_type = self.content_classifier.classify(input, &analysis);
        self.optimizer
            .optimize_with_content_type(&analysis, target, content_type)
    }

    /// Get content classifier for external use.
    #[must_use]
    pub const fn classifier(&self) -> &ContentClassifier {
        &self.content_classifier
    }
}

impl Default for SmartConverter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Content Classification ──────────────────────────────────────────────────

/// Detected content type for intelligent codec selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentType {
    /// Animated content (cartoons, anime, motion graphics)
    /// Characterized by flat colors, sharp edges, low color variance.
    Animation,
    /// Live action footage (camera-captured video)
    /// High detail, film grain, natural noise.
    LiveAction,
    /// Screen recording or presentation capture
    /// Sharp text, large uniform regions, mouse cursor movement.
    ScreenRecording,
    /// Slideshow or static-heavy content
    /// Very low motion, long static frames.
    Slideshow,
    /// High-motion sports or action content
    /// Fast movement, frequent scene changes.
    HighMotion,
    /// Audio-only content with a static or absent video track.
    AudioOnly,
    /// Unknown or unclassifiable content.
    Unknown,
}

impl ContentType {
    /// Human-readable description of the content type.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Animation => "Animated content (cartoons, anime, motion graphics)",
            Self::LiveAction => "Live action footage (camera-captured)",
            Self::ScreenRecording => "Screen recording or presentation",
            Self::Slideshow => "Slideshow or static-heavy content",
            Self::HighMotion => "High-motion sports or action content",
            Self::AudioOnly => "Audio-only content",
            Self::Unknown => "Unknown content type",
        }
    }
}

/// Content classifier that determines the type of media content.
///
/// Uses heuristics based on file properties, extension, file size patterns,
/// and bitrate ratios to classify content without full decode.
#[derive(Debug, Clone)]
pub struct ContentClassifier {
    /// Bytes-per-second threshold below which content is likely a slideshow.
    slideshow_bps_threshold: f64,
    /// Bytes-per-second threshold above which content is likely high-motion.
    high_motion_bps_threshold: f64,
}

impl ContentClassifier {
    /// Create a new content classifier with default thresholds.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slideshow_bps_threshold: 50_000.0,
            high_motion_bps_threshold: 2_000_000.0,
        }
    }

    /// Create a classifier with custom thresholds.
    #[must_use]
    pub const fn with_thresholds(slideshow_bps: f64, high_motion_bps: f64) -> Self {
        Self {
            slideshow_bps_threshold: slideshow_bps,
            high_motion_bps_threshold: high_motion_bps,
        }
    }

    /// Classify content based on file properties and analysis data.
    #[must_use]
    pub fn classify(&self, path: &Path, analysis: &MediaAnalysis) -> ContentType {
        // Audio-only content
        if !analysis.has_video {
            return ContentType::AudioOnly;
        }

        // Extension-based hints
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // GIF and animated image formats are almost always animation
        if ext == "gif" || ext == "apng" {
            return ContentType::Animation;
        }

        // Screen recording formats / common screen capture extensions
        if ext == "mkv" || ext == "avi" {
            // MKV/AVI from screen capture tools often have very specific properties;
            // defer to bitrate analysis below.
        }

        // Use bitrate analysis if duration is available
        if let (Some(duration), file_size) = (analysis.duration_seconds, analysis.file_size) {
            if duration > 0.0 {
                let bytes_per_second = file_size as f64 / duration;

                // Very low data rate with video = slideshow
                if bytes_per_second < self.slideshow_bps_threshold {
                    return ContentType::Slideshow;
                }

                // Very high data rate = high motion
                if bytes_per_second > self.high_motion_bps_threshold {
                    return ContentType::HighMotion;
                }
            }
        }

        // Resolution-based heuristics for screen recording detection
        if let Some((width, height)) = analysis.resolution {
            // Common screen recording resolutions (exact monitor sizes)
            let is_screen_res = matches!(
                (width, height),
                (2560, 1440)
                    | (1920, 1080)
                    | (1440, 900)
                    | (1366, 768)
                    | (3840, 2160)
                    | (2560, 1600)
                    | (1680, 1050)
                    | (1280, 800)
            );

            // Screen recordings typically have low frame rates (15-30) and
            // moderate bitrates with very sharp edges.
            if is_screen_res {
                if let Some(fps) = analysis.frame_rate {
                    // Screen recordings often use 15 or 20 fps
                    if fps <= 20.0 {
                        return ContentType::ScreenRecording;
                    }
                }
            }
        }

        // Frame rate heuristics
        if let Some(fps) = analysis.frame_rate {
            // Very low frame rate suggests animation or slideshow
            if fps <= 12.0 {
                return ContentType::Animation;
            }
            // Very high frame rate (60+) often indicates gaming or high-motion
            if fps >= 60.0 {
                return ContentType::HighMotion;
            }
        }

        // Bitrate-based classification without duration
        if let Some(bitrate) = analysis.bitrate {
            if let Some((width, height)) = analysis.resolution {
                let pixels = u64::from(width) * u64::from(height);
                if pixels > 0 {
                    // Bits per pixel per frame
                    let fps = analysis.frame_rate.unwrap_or(30.0);
                    let bpp = bitrate as f64 / (pixels as f64 * fps);

                    // Animation has very efficient compression (low bpp)
                    if bpp < 0.02 {
                        return ContentType::Animation;
                    }
                    // High bpp suggests complex live action or high-motion
                    if bpp > 0.15 {
                        return ContentType::HighMotion;
                    }
                }
            }
        }

        // Default to live action as the most common content type
        ContentType::LiveAction
    }

    /// Recommend the best video codec for a given content type.
    #[must_use]
    pub const fn recommend_video_codec(content_type: ContentType) -> VideoCodec {
        match content_type {
            // Animation benefits from VP8's simpler block structure and alpha support.
            // For flat colors and sharp edges, VP8 is very efficient and fast.
            ContentType::Animation => VideoCodec::Vp8,
            // Live action benefits from AV1's advanced prediction modes,
            // film grain synthesis, and superior compression efficiency.
            ContentType::LiveAction => VideoCodec::Av1,
            // Screen recording: VP9 handles text and sharp edges well
            // with its sharper quantization modes.
            ContentType::ScreenRecording => VideoCodec::Vp9,
            // Slideshow: AV1 excels at still images with intra-frame coding.
            ContentType::Slideshow => VideoCodec::Av1,
            // High motion: VP9 offers good speed/quality trade-off for fast content.
            ContentType::HighMotion => VideoCodec::Vp9,
            // Audio-only: no video codec needed, but if forced, use VP8 (lightest).
            ContentType::AudioOnly => VideoCodec::Vp8,
            // Unknown: VP9 as the safe middle ground.
            ContentType::Unknown => VideoCodec::Vp9,
        }
    }

    /// Recommend the best container for a given content type and target.
    #[must_use]
    pub const fn recommend_container(
        content_type: ContentType,
        target: ConversionTarget,
    ) -> ContainerFormat {
        match (content_type, target) {
            // Animation going to web = WebM (VP8/VP9 native container)
            (ContentType::Animation, ConversionTarget::WebStreaming) => ContainerFormat::Webm,
            // High quality animation = Matroska (supports everything)
            (ContentType::Animation, ConversionTarget::MaxQuality) => ContainerFormat::Matroska,
            // Screen recording = Matroska (chapter support, flexible)
            (ContentType::ScreenRecording, _) => ContainerFormat::Matroska,
            // Everything else follows target-based selection
            (_, ConversionTarget::WebStreaming) => ContainerFormat::Webm,
            (_, ConversionTarget::Mobile) => ContainerFormat::Mp4,
            (_, ConversionTarget::MaxQuality) => ContainerFormat::Matroska,
            (_, ConversionTarget::MinSize) => ContainerFormat::Webm,
            (_, ConversionTarget::FastEncoding) => ContainerFormat::Mp4,
        }
    }

    /// Recommend bitrate mode for content type and target combination.
    #[must_use]
    pub const fn recommend_bitrate_mode(
        content_type: ContentType,
        target: ConversionTarget,
    ) -> BitrateMode {
        match (content_type, target) {
            // Animation compresses very well; CRF is ideal
            (ContentType::Animation, ConversionTarget::MinSize) => BitrateMode::Crf(40),
            (ContentType::Animation, ConversionTarget::MaxQuality) => BitrateMode::Crf(18),
            (ContentType::Animation, _) => BitrateMode::Crf(28),
            // Screen recording: lossless-ish for text readability
            (ContentType::ScreenRecording, ConversionTarget::MaxQuality) => BitrateMode::Crf(15),
            (ContentType::ScreenRecording, _) => BitrateMode::Crf(24),
            // Slideshow: very low CRF since frames are near-static
            (ContentType::Slideshow, _) => BitrateMode::Crf(22),
            // High motion needs higher bitrate
            (ContentType::HighMotion, ConversionTarget::MinSize) => BitrateMode::Crf(38),
            (ContentType::HighMotion, ConversionTarget::MaxQuality) => BitrateMode::Crf(18),
            (ContentType::HighMotion, _) => BitrateMode::Crf(28),
            // Live action: standard settings
            (ContentType::LiveAction, ConversionTarget::MinSize) => BitrateMode::Crf(42),
            (ContentType::LiveAction, ConversionTarget::MaxQuality) => BitrateMode::Crf(20),
            (ContentType::LiveAction, ConversionTarget::FastEncoding) => {
                BitrateMode::Cbr(2_000_000)
            }
            (ContentType::LiveAction, _) => BitrateMode::Crf(31),
            // Defaults
            (_, ConversionTarget::MinSize) => BitrateMode::Crf(45),
            (_, ConversionTarget::MaxQuality) => BitrateMode::Crf(20),
            (_, ConversionTarget::FastEncoding) => BitrateMode::Cbr(2_000_000),
            (_, _) => BitrateMode::Crf(31),
        }
    }
}

impl Default for ContentClassifier {
    fn default() -> Self {
        Self::new()
    }
}

// ── Media Analyzer ──────────────────────────────────────────────────────────

/// Media analyzer for examining input files.
#[derive(Debug, Clone)]
pub struct MediaAnalyzer;

impl MediaAnalyzer {
    /// Create a new media analyzer.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Analyze media file.
    ///
    /// Reads file-level metadata (size, extension) and returns a best-effort
    /// `MediaAnalysis`. Full demux / codec probing requires the transcode
    /// pipeline and is deferred; the inferred fields default to common values.
    pub async fn analyze(&self, path: &Path) -> Result<MediaAnalysis> {
        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or_default();

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();

        // Infer container-level media presence from extension.
        let (has_video, has_audio, video_codec, audio_codec) = match ext.to_lowercase().as_str() {
            "mp4" | "m4v" | "mov" => (true, true, Some(VideoCodec::Vp8), Some(AudioCodec::Opus)),
            "webm" => (true, true, Some(VideoCodec::Vp9), Some(AudioCodec::Opus)),
            "mkv" => (true, true, Some(VideoCodec::Av1), Some(AudioCodec::Opus)),
            "mp3" | "aac" | "ogg" | "flac" | "wav" => (false, true, None, Some(AudioCodec::Opus)),
            "gif" | "apng" => (true, false, Some(VideoCodec::Vp8), None),
            "png" | "jpg" | "jpeg" | "webp" | "tiff" | "tif" | "dpx" | "exr" => {
                (true, false, None, None)
            }
            _ => (true, true, Some(VideoCodec::Vp9), Some(AudioCodec::Opus)),
        };

        Ok(MediaAnalysis {
            has_video,
            has_audio,
            video_codec,
            audio_codec,
            resolution: None,
            frame_rate: None,
            bitrate: None,
            duration_seconds: None,
            file_size,
            is_hdr: false,
            is_interlaced: false,
        })
    }
}

impl Default for MediaAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Media analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAnalysis {
    /// Has video stream
    pub has_video: bool,
    /// Has audio stream
    pub has_audio: bool,
    /// Video codec (if present)
    pub video_codec: Option<VideoCodec>,
    /// Audio codec (if present)
    pub audio_codec: Option<AudioCodec>,
    /// Video resolution (width, height)
    pub resolution: Option<(u32, u32)>,
    /// Frame rate
    pub frame_rate: Option<f64>,
    /// Bitrate in bits per second
    pub bitrate: Option<u64>,
    /// Duration in seconds
    pub duration_seconds: Option<f64>,
    /// File size in bytes
    pub file_size: u64,
    /// Is HDR content
    pub is_hdr: bool,
    /// Is interlaced
    pub is_interlaced: bool,
}

/// Conversion target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConversionTarget {
    /// Optimize for web streaming
    WebStreaming,
    /// Optimize for mobile devices
    Mobile,
    /// Optimize for maximum quality
    MaxQuality,
    /// Optimize for smallest file size
    MinSize,
    /// Optimize for fast encoding
    FastEncoding,
}

// ── Settings Optimizer ──────────────────────────────────────────────────────

/// Settings optimizer.
#[derive(Debug, Clone)]
pub struct SettingsOptimizer;

impl SettingsOptimizer {
    /// Create a new settings optimizer.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Optimize settings based on analysis and target.
    pub fn optimize(
        &self,
        analysis: &MediaAnalysis,
        target: ConversionTarget,
    ) -> Result<OptimizedSettings> {
        let container = self.select_container(target);
        let video = if analysis.has_video {
            Some(self.optimize_video(analysis, target)?)
        } else {
            None
        };
        let audio = if analysis.has_audio {
            Some(self.optimize_audio(analysis, target)?)
        } else {
            None
        };

        Ok(OptimizedSettings {
            container,
            video,
            audio,
            content_type: None,
            rationale: self.generate_rationale(analysis, target),
        })
    }

    /// Optimize settings with content-aware codec selection.
    pub fn optimize_with_content_type(
        &self,
        analysis: &MediaAnalysis,
        target: ConversionTarget,
        content_type: ContentType,
    ) -> Result<OptimizedSettings> {
        let container = ContentClassifier::recommend_container(content_type, target);
        let video = if analysis.has_video {
            Some(self.optimize_video_for_content(analysis, target, content_type)?)
        } else {
            None
        };
        let audio = if analysis.has_audio {
            Some(self.optimize_audio(analysis, target)?)
        } else {
            None
        };

        Ok(OptimizedSettings {
            container,
            video,
            audio,
            content_type: Some(content_type),
            rationale: self.generate_content_aware_rationale(content_type, target),
        })
    }

    fn select_container(&self, target: ConversionTarget) -> ContainerFormat {
        match target {
            ConversionTarget::WebStreaming => ContainerFormat::Webm,
            ConversionTarget::Mobile => ContainerFormat::Mp4,
            ConversionTarget::MaxQuality => ContainerFormat::Matroska,
            ConversionTarget::MinSize => ContainerFormat::Webm,
            ConversionTarget::FastEncoding => ContainerFormat::Mp4,
        }
    }

    fn optimize_video(
        &self,
        analysis: &MediaAnalysis,
        target: ConversionTarget,
    ) -> Result<VideoSettings> {
        let codec = match target {
            ConversionTarget::WebStreaming | ConversionTarget::MinSize => VideoCodec::Vp9,
            ConversionTarget::Mobile | ConversionTarget::FastEncoding => VideoCodec::Vp8,
            ConversionTarget::MaxQuality => VideoCodec::Av1,
        };

        let bitrate = match target {
            ConversionTarget::MinSize => BitrateMode::Crf(45),
            ConversionTarget::FastEncoding => BitrateMode::Cbr(2_000_000),
            ConversionTarget::MaxQuality => BitrateMode::Crf(20),
            _ => BitrateMode::Crf(31),
        };

        Ok(VideoSettings {
            codec,
            resolution: analysis.resolution,
            frame_rate: analysis.frame_rate,
            bitrate,
            quality: None,
            two_pass: matches!(
                target,
                ConversionTarget::MaxQuality | ConversionTarget::WebStreaming
            ),
            speed: match target {
                ConversionTarget::FastEncoding => crate::pipeline::EncodingSpeed::Fast,
                ConversionTarget::MaxQuality => crate::pipeline::EncodingSpeed::VerySlow,
                _ => crate::pipeline::EncodingSpeed::Medium,
            },
            tone_map: analysis.is_hdr,
        })
    }

    fn optimize_video_for_content(
        &self,
        analysis: &MediaAnalysis,
        target: ConversionTarget,
        content_type: ContentType,
    ) -> Result<VideoSettings> {
        let codec = ContentClassifier::recommend_video_codec(content_type);
        let bitrate = ContentClassifier::recommend_bitrate_mode(content_type, target);

        // Encoding speed adapts to content type
        let speed = match (content_type, target) {
            (_, ConversionTarget::FastEncoding) => crate::pipeline::EncodingSpeed::Fast,
            (_, ConversionTarget::MaxQuality) => crate::pipeline::EncodingSpeed::VerySlow,
            // Animation encodes fast even at "slow" speed due to simple content
            (ContentType::Animation, _) => crate::pipeline::EncodingSpeed::Medium,
            // Screen recording benefits from slower encoding for text sharpness
            (ContentType::ScreenRecording, _) => crate::pipeline::EncodingSpeed::Slow,
            _ => crate::pipeline::EncodingSpeed::Medium,
        };

        // Two-pass is beneficial for target bitrate scenarios, max quality,
        // and web streaming. Animation usually does not need two-pass.
        let two_pass = match (content_type, target) {
            (ContentType::Animation, _) => false,
            (_, ConversionTarget::MaxQuality | ConversionTarget::WebStreaming) => true,
            _ => false,
        };

        Ok(VideoSettings {
            codec,
            resolution: analysis.resolution,
            frame_rate: analysis.frame_rate,
            bitrate,
            quality: None,
            two_pass,
            speed,
            tone_map: analysis.is_hdr,
        })
    }

    fn optimize_audio(
        &self,
        _analysis: &MediaAnalysis,
        target: ConversionTarget,
    ) -> Result<AudioSettings> {
        let codec = match target {
            ConversionTarget::MaxQuality => AudioCodec::Flac,
            _ => AudioCodec::Opus,
        };

        let bitrate = match target {
            ConversionTarget::MinSize => 96_000,
            ConversionTarget::MaxQuality => 256_000,
            _ => 128_000,
        };

        Ok(AudioSettings {
            codec,
            sample_rate: 48000,
            channels: crate::formats::ChannelLayout::Stereo,
            bitrate: if codec == AudioCodec::Flac {
                None
            } else {
                Some(bitrate)
            },
            normalize: false,
            normalization_target: -23.0,
        })
    }

    fn generate_rationale(&self, _analysis: &MediaAnalysis, target: ConversionTarget) -> String {
        match target {
            ConversionTarget::WebStreaming => {
                "Optimized for web streaming with VP9 codec for good quality and browser compatibility"
            }
            ConversionTarget::Mobile => {
                "Optimized for mobile devices with efficient encoding and reasonable file size"
            }
            ConversionTarget::MaxQuality => {
                "Optimized for maximum quality using AV1 codec and high bitrate settings"
            }
            ConversionTarget::MinSize => {
                "Optimized for minimum file size using aggressive compression"
            }
            ConversionTarget::FastEncoding => {
                "Optimized for fast encoding with VP8 codec and single-pass encoding"
            }
        }
        .to_string()
    }

    fn generate_content_aware_rationale(
        &self,
        content_type: ContentType,
        target: ConversionTarget,
    ) -> String {
        let codec = ContentClassifier::recommend_video_codec(content_type);
        format!(
            "Content detected as '{}'. Selected {} codec optimized for {} target. {}",
            content_type.description(),
            codec.name(),
            match target {
                ConversionTarget::WebStreaming => "web streaming",
                ConversionTarget::Mobile => "mobile",
                ConversionTarget::MaxQuality => "maximum quality",
                ConversionTarget::MinSize => "minimum size",
                ConversionTarget::FastEncoding => "fast encoding",
            },
            match content_type {
                ContentType::Animation =>
                    "VP8 excels with flat colors and sharp edges typical of animation.",
                ContentType::LiveAction =>
                    "AV1 provides superior compression for complex natural imagery.",
                ContentType::ScreenRecording => "VP9's quantization modes preserve text sharpness.",
                ContentType::Slideshow =>
                    "AV1's intra-frame coding is ideal for near-static content.",
                ContentType::HighMotion =>
                    "VP9 balances encoding speed with quality for fast-moving content.",
                ContentType::AudioOnly => "Minimal video codec selected for audio-focused content.",
                ContentType::Unknown => "VP9 selected as a safe general-purpose codec.",
            }
        )
    }
}

impl Default for SettingsOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Optimized conversion settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedSettings {
    /// Container format
    pub container: ContainerFormat,
    /// Video settings
    pub video: Option<VideoSettings>,
    /// Audio settings
    pub audio: Option<AudioSettings>,
    /// Detected content type (if content-aware optimization was used)
    pub content_type: Option<ContentType>,
    /// Rationale for these settings
    pub rationale: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_analysis() -> MediaAnalysis {
        MediaAnalysis {
            has_video: true,
            has_audio: true,
            video_codec: Some(VideoCodec::Vp9),
            audio_codec: Some(AudioCodec::Opus),
            resolution: Some((1920, 1080)),
            frame_rate: Some(30.0),
            bitrate: Some(5_000_000),
            duration_seconds: Some(300.0),
            file_size: 625_000_000,
            is_hdr: false,
            is_interlaced: false,
        }
    }

    #[test]
    fn test_smart_converter() {
        let converter = SmartConverter::new();
        // SmartConverter now contains ContentClassifier (ZST), still small
        assert!(std::mem::size_of_val(&converter) > 0);
    }

    #[test]
    fn test_settings_optimizer() {
        let optimizer = SettingsOptimizer::new();
        let analysis = make_analysis();

        let result = optimizer.optimize(&analysis, ConversionTarget::WebStreaming);
        assert!(result.is_ok());

        let settings = result.expect("optimization should succeed");
        assert_eq!(settings.container, ContainerFormat::Webm);
        assert!(settings.video.is_some());
        assert!(settings.audio.is_some());
    }

    #[test]
    fn test_all_conversion_targets() {
        let optimizer = SettingsOptimizer::new();
        let analysis = make_analysis();

        assert!(optimizer
            .optimize(&analysis, ConversionTarget::WebStreaming)
            .is_ok());
        assert!(optimizer
            .optimize(&analysis, ConversionTarget::Mobile)
            .is_ok());
        assert!(optimizer
            .optimize(&analysis, ConversionTarget::MaxQuality)
            .is_ok());
        assert!(optimizer
            .optimize(&analysis, ConversionTarget::MinSize)
            .is_ok());
        assert!(optimizer
            .optimize(&analysis, ConversionTarget::FastEncoding)
            .is_ok());
    }

    // ── Content Classification Tests ────────────────────────────────────────

    #[test]
    fn test_classify_audio_only() {
        let classifier = ContentClassifier::new();
        let analysis = MediaAnalysis {
            has_video: false,
            has_audio: true,
            video_codec: None,
            audio_codec: Some(AudioCodec::Opus),
            resolution: None,
            frame_rate: None,
            bitrate: None,
            duration_seconds: Some(180.0),
            file_size: 5_000_000,
            is_hdr: false,
            is_interlaced: false,
        };
        let result = classifier.classify(Path::new("audio.ogg"), &analysis);
        assert_eq!(result, ContentType::AudioOnly);
    }

    #[test]
    fn test_classify_gif_as_animation() {
        let classifier = ContentClassifier::new();
        let analysis = MediaAnalysis {
            has_video: true,
            has_audio: false,
            video_codec: Some(VideoCodec::Vp8),
            audio_codec: None,
            resolution: Some((320, 240)),
            frame_rate: Some(15.0),
            bitrate: None,
            duration_seconds: Some(5.0),
            file_size: 500_000,
            is_hdr: false,
            is_interlaced: false,
        };
        let result = classifier.classify(Path::new("funny.gif"), &analysis);
        assert_eq!(result, ContentType::Animation);
    }

    #[test]
    fn test_classify_slideshow_low_data_rate() {
        let classifier = ContentClassifier::new();
        let analysis = MediaAnalysis {
            has_video: true,
            has_audio: true,
            video_codec: Some(VideoCodec::Vp9),
            audio_codec: Some(AudioCodec::Opus),
            resolution: Some((1920, 1080)),
            frame_rate: Some(30.0),
            bitrate: None,
            duration_seconds: Some(600.0),
            file_size: 10_000_000, // ~16KB/s -> very low for video
            is_hdr: false,
            is_interlaced: false,
        };
        let result = classifier.classify(Path::new("presentation.mp4"), &analysis);
        assert_eq!(result, ContentType::Slideshow);
    }

    #[test]
    fn test_classify_high_motion() {
        let classifier = ContentClassifier::new();
        let analysis = MediaAnalysis {
            has_video: true,
            has_audio: true,
            video_codec: Some(VideoCodec::Vp9),
            audio_codec: Some(AudioCodec::Opus),
            resolution: Some((1920, 1080)),
            frame_rate: Some(60.0),
            bitrate: None,
            duration_seconds: Some(300.0),
            file_size: 900_000_000, // 3MB/s -> high
            is_hdr: false,
            is_interlaced: false,
        };
        let result = classifier.classify(Path::new("sports.webm"), &analysis);
        assert_eq!(result, ContentType::HighMotion);
    }

    #[test]
    fn test_classify_screen_recording() {
        let classifier = ContentClassifier::new();
        let analysis = MediaAnalysis {
            has_video: true,
            has_audio: false,
            video_codec: Some(VideoCodec::Vp9),
            audio_codec: None,
            resolution: Some((2560, 1440)),
            frame_rate: Some(15.0),
            bitrate: None,
            duration_seconds: None,
            file_size: 50_000_000,
            is_hdr: false,
            is_interlaced: false,
        };
        let result = classifier.classify(Path::new("tutorial.mkv"), &analysis);
        assert_eq!(result, ContentType::ScreenRecording);
    }

    #[test]
    fn test_classify_low_fps_as_animation() {
        let classifier = ContentClassifier::new();
        let analysis = MediaAnalysis {
            has_video: true,
            has_audio: true,
            video_codec: Some(VideoCodec::Vp9),
            audio_codec: Some(AudioCodec::Opus),
            resolution: Some((1280, 720)),
            frame_rate: Some(10.0),
            bitrate: None,
            duration_seconds: None,
            file_size: 50_000_000,
            is_hdr: false,
            is_interlaced: false,
        };
        let result = classifier.classify(Path::new("cartoon.webm"), &analysis);
        assert_eq!(result, ContentType::Animation);
    }

    #[test]
    fn test_classify_default_live_action() {
        let classifier = ContentClassifier::new();
        let mut analysis = make_analysis();
        // Use moderate bitrate and file size that won't trigger HighMotion
        analysis.bitrate = Some(3_000_000);
        analysis.file_size = 200_000_000; // ~667KB/s with 300s duration
        let result = classifier.classify(Path::new("movie.mp4"), &analysis);
        // Without strong signals, defaults to LiveAction
        assert_eq!(result, ContentType::LiveAction);
    }

    #[test]
    fn test_classify_by_bitrate_low_bpp() {
        let classifier = ContentClassifier::new();
        let analysis = MediaAnalysis {
            has_video: true,
            has_audio: true,
            video_codec: Some(VideoCodec::Vp9),
            audio_codec: Some(AudioCodec::Opus),
            resolution: Some((1920, 1080)),
            frame_rate: Some(24.0),
            bitrate: Some(500_000), // Very low bitrate for 1080p = animation-like
            duration_seconds: None,
            file_size: 50_000_000,
            is_hdr: false,
            is_interlaced: false,
        };
        let result = classifier.classify(Path::new("anime.mp4"), &analysis);
        assert_eq!(result, ContentType::Animation);
    }

    #[test]
    fn test_classify_by_bitrate_high_bpp() {
        let classifier = ContentClassifier::new();
        let analysis = MediaAnalysis {
            has_video: true,
            has_audio: true,
            video_codec: Some(VideoCodec::Vp9),
            audio_codec: Some(AudioCodec::Opus),
            resolution: Some((1920, 1080)),
            frame_rate: Some(30.0),
            bitrate: Some(50_000_000), // Very high bitrate = high motion
            duration_seconds: None,
            file_size: 500_000_000,
            is_hdr: false,
            is_interlaced: false,
        };
        let result = classifier.classify(Path::new("action.mp4"), &analysis);
        assert_eq!(result, ContentType::HighMotion);
    }

    // ── Codec Recommendation Tests ──────────────────────────────────────────

    #[test]
    fn test_recommend_codec_animation() {
        assert_eq!(
            ContentClassifier::recommend_video_codec(ContentType::Animation),
            VideoCodec::Vp8
        );
    }

    #[test]
    fn test_recommend_codec_live_action() {
        assert_eq!(
            ContentClassifier::recommend_video_codec(ContentType::LiveAction),
            VideoCodec::Av1
        );
    }

    #[test]
    fn test_recommend_codec_screen_recording() {
        assert_eq!(
            ContentClassifier::recommend_video_codec(ContentType::ScreenRecording),
            VideoCodec::Vp9
        );
    }

    #[test]
    fn test_recommend_codec_all_types() {
        // Every content type should return a valid codec
        let types = [
            ContentType::Animation,
            ContentType::LiveAction,
            ContentType::ScreenRecording,
            ContentType::Slideshow,
            ContentType::HighMotion,
            ContentType::AudioOnly,
            ContentType::Unknown,
        ];
        for ct in &types {
            let codec = ContentClassifier::recommend_video_codec(*ct);
            assert!(!codec.name().is_empty());
        }
    }

    // ── Content-Aware Optimization Tests ────────────────────────────────────

    #[test]
    fn test_optimize_with_content_type_animation() {
        let optimizer = SettingsOptimizer::new();
        let analysis = make_analysis();
        let result = optimizer.optimize_with_content_type(
            &analysis,
            ConversionTarget::WebStreaming,
            ContentType::Animation,
        );
        assert!(result.is_ok());
        let settings = result.expect("should succeed");
        assert_eq!(settings.content_type, Some(ContentType::Animation));
        let video = settings.video.expect("should have video");
        assert_eq!(video.codec, VideoCodec::Vp8);
        // Animation should not use two-pass
        assert!(!video.two_pass);
    }

    #[test]
    fn test_optimize_with_content_type_live_action_max_quality() {
        let optimizer = SettingsOptimizer::new();
        let analysis = make_analysis();
        let result = optimizer.optimize_with_content_type(
            &analysis,
            ConversionTarget::MaxQuality,
            ContentType::LiveAction,
        );
        assert!(result.is_ok());
        let settings = result.expect("should succeed");
        let video = settings.video.expect("should have video");
        assert_eq!(video.codec, VideoCodec::Av1);
        assert!(video.two_pass);
    }

    #[test]
    fn test_optimize_with_content_type_screen_recording() {
        let optimizer = SettingsOptimizer::new();
        let analysis = make_analysis();
        let result = optimizer.optimize_with_content_type(
            &analysis,
            ConversionTarget::WebStreaming,
            ContentType::ScreenRecording,
        );
        assert!(result.is_ok());
        let settings = result.expect("should succeed");
        assert_eq!(settings.container, ContainerFormat::Matroska);
        let video = settings.video.expect("should have video");
        assert_eq!(video.codec, VideoCodec::Vp9);
        assert_eq!(video.speed, crate::pipeline::EncodingSpeed::Slow);
    }

    #[test]
    fn test_content_type_description_non_empty() {
        let types = [
            ContentType::Animation,
            ContentType::LiveAction,
            ContentType::ScreenRecording,
            ContentType::Slideshow,
            ContentType::HighMotion,
            ContentType::AudioOnly,
            ContentType::Unknown,
        ];
        for ct in &types {
            assert!(!ct.description().is_empty());
        }
    }

    #[test]
    fn test_rationale_includes_content_type() {
        let optimizer = SettingsOptimizer::new();
        let analysis = make_analysis();
        let result = optimizer
            .optimize_with_content_type(
                &analysis,
                ConversionTarget::WebStreaming,
                ContentType::Animation,
            )
            .expect("should succeed");
        // Rationale should mention animated content and the selected codec
        assert!(
            result.rationale.contains("Animated"),
            "rationale: {}",
            result.rationale
        );
        assert!(
            result.rationale.contains("vp8"),
            "rationale: {}",
            result.rationale
        );
    }

    #[test]
    fn test_bitrate_mode_recommendations() {
        // Min size should always use high CRF values
        let br = ContentClassifier::recommend_bitrate_mode(
            ContentType::LiveAction,
            ConversionTarget::MinSize,
        );
        match br {
            BitrateMode::Crf(v) => assert!(v >= 35),
            _ => panic!("Expected CRF for min size"),
        }

        // Max quality should use low CRF values
        let br = ContentClassifier::recommend_bitrate_mode(
            ContentType::LiveAction,
            ConversionTarget::MaxQuality,
        );
        match br {
            BitrateMode::Crf(v) => assert!(v <= 25),
            _ => panic!("Expected CRF for max quality"),
        }

        // Fast encoding for live action should use CBR
        let br = ContentClassifier::recommend_bitrate_mode(
            ContentType::LiveAction,
            ConversionTarget::FastEncoding,
        );
        assert!(matches!(br, BitrateMode::Cbr(_)));
    }

    #[test]
    fn test_container_recommendation_animation_web() {
        let container = ContentClassifier::recommend_container(
            ContentType::Animation,
            ConversionTarget::WebStreaming,
        );
        assert_eq!(container, ContainerFormat::Webm);
    }

    #[test]
    fn test_container_recommendation_screen_recording() {
        let container = ContentClassifier::recommend_container(
            ContentType::ScreenRecording,
            ConversionTarget::Mobile,
        );
        assert_eq!(container, ContainerFormat::Matroska);
    }

    #[test]
    fn test_custom_classifier_thresholds() {
        let classifier = ContentClassifier::with_thresholds(100_000.0, 5_000_000.0);
        let analysis = MediaAnalysis {
            has_video: true,
            has_audio: true,
            video_codec: Some(VideoCodec::Vp9),
            audio_codec: Some(AudioCodec::Opus),
            resolution: Some((1920, 1080)),
            frame_rate: Some(30.0),
            bitrate: None,
            duration_seconds: Some(100.0),
            file_size: 5_000_000, // 50KB/s
            is_hdr: false,
            is_interlaced: false,
        };
        let result = classifier.classify(Path::new("test.mp4"), &analysis);
        assert_eq!(result, ContentType::Slideshow);
    }

    #[test]
    fn test_audio_only_optimization() {
        let optimizer = SettingsOptimizer::new();
        let analysis = MediaAnalysis {
            has_video: false,
            has_audio: true,
            video_codec: None,
            audio_codec: Some(AudioCodec::Opus),
            resolution: None,
            frame_rate: None,
            bitrate: None,
            duration_seconds: Some(300.0),
            file_size: 5_000_000,
            is_hdr: false,
            is_interlaced: false,
        };
        let result = optimizer.optimize_with_content_type(
            &analysis,
            ConversionTarget::WebStreaming,
            ContentType::AudioOnly,
        );
        assert!(result.is_ok());
        let settings = result.expect("should succeed");
        assert!(settings.video.is_none());
        assert!(settings.audio.is_some());
    }
}
