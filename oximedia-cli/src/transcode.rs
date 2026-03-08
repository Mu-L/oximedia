//! Transcoding operations for converting media files.
//!
//! Provides transcode command implementation with:
//! - Codec selection and validation
//! - Filter chain construction
//! - Format detection
//! - Multi-pass encoding
//! - Resume capability

use crate::progress::TranscodeProgress;
use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Options for transcode operation.
#[derive(Debug, Clone)]
pub struct TranscodeOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub preset_name: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub video_bitrate: Option<String>,
    pub audio_bitrate: Option<String>,
    pub scale: Option<String>,
    #[allow(dead_code)]
    pub video_filter: Option<String>,
    #[allow(dead_code)]
    pub start_time: Option<String>,
    #[allow(dead_code)]
    pub duration: Option<String>,
    #[allow(dead_code)]
    pub framerate: Option<String>,
    pub preset: String,
    pub two_pass: bool,
    pub crf: Option<u32>,
    #[allow(dead_code)]
    pub threads: usize,
    pub overwrite: bool,
    #[allow(dead_code)]
    pub resume: bool,
}

/// Supported video codecs (patent-free only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    Av1,
    Vp9,
    Vp8,
}

impl VideoCodec {
    /// Parse codec from string.
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "av1" | "libaom-av1" => Ok(Self::Av1),
            "vp9" | "libvpx-vp9" => Ok(Self::Vp9),
            "vp8" | "libvpx" => Ok(Self::Vp8),
            _ => Err(anyhow!("Unsupported video codec: {}", s)),
        }
    }

    /// Get codec name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Av1 => "AV1",
            Self::Vp9 => "VP9",
            Self::Vp8 => "VP8",
        }
    }

    /// Get default CRF range for this codec.
    #[allow(dead_code)]
    pub fn default_crf(&self) -> u32 {
        match self {
            Self::Av1 => 30, // 0-255 range
            Self::Vp9 => 31, // 0-63 range
            Self::Vp8 => 10, // 0-63 range
        }
    }

    /// Validate CRF value for this codec.
    pub fn validate_crf(&self, crf: u32) -> Result<()> {
        let max = match self {
            Self::Av1 => 255,
            Self::Vp9 | Self::Vp8 => 63,
        };

        if crf > max {
            Err(anyhow!(
                "CRF {} is out of range for {} (max: {})",
                crf,
                self.name(),
                max
            ))
        } else {
            Ok(())
        }
    }
}

/// Supported audio codecs (patent-free only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    Opus,
    Vorbis,
    Flac,
}

impl AudioCodec {
    /// Parse codec from string.
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "opus" | "libopus" => Ok(Self::Opus),
            "vorbis" | "libvorbis" => Ok(Self::Vorbis),
            "flac" => Ok(Self::Flac),
            _ => Err(anyhow!("Unsupported audio codec: {}", s)),
        }
    }

    /// Get codec name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Opus => "Opus",
            Self::Vorbis => "Vorbis",
            Self::Flac => "FLAC",
        }
    }
}

/// Encoder preset (affects speed/quality tradeoff).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderPreset {
    Ultrafast,
    Superfast,
    Veryfast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    Veryslow,
}

impl EncoderPreset {
    /// Parse preset from string.
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "ultrafast" => Ok(Self::Ultrafast),
            "superfast" => Ok(Self::Superfast),
            "veryfast" => Ok(Self::Veryfast),
            "faster" => Ok(Self::Faster),
            "fast" => Ok(Self::Fast),
            "medium" => Ok(Self::Medium),
            "slow" => Ok(Self::Slow),
            "slower" => Ok(Self::Slower),
            "veryslow" => Ok(Self::Veryslow),
            _ => Err(anyhow!("Unknown preset: {}", s)),
        }
    }

    /// Get speed factor (higher = faster but lower quality).
    #[allow(dead_code)]
    pub fn speed_factor(&self) -> u32 {
        match self {
            Self::Ultrafast => 9,
            Self::Superfast => 8,
            Self::Veryfast => 7,
            Self::Faster => 6,
            Self::Fast => 5,
            Self::Medium => 4,
            Self::Slow => 3,
            Self::Slower => 2,
            Self::Veryslow => 1,
        }
    }
}

/// Main transcode function.
pub async fn transcode(mut options: TranscodeOptions) -> Result<()> {
    info!("Starting transcode operation");
    debug!("Options: {:?}", options);

    // Handle preset if specified
    if let Some(ref preset_name) = options.preset_name {
        use crate::presets::PresetManager;

        let custom_dir = PresetManager::default_custom_dir()?;
        let manager = PresetManager::with_custom_dir(&custom_dir)?;
        let preset = manager.get_preset(preset_name)?;

        info!("Using preset: {} - {}", preset.name, preset.description);

        // Apply preset settings to options
        options.video_codec = Some(preset.video.codec.clone());
        options.audio_codec = Some(preset.audio.codec.clone());
        options.video_bitrate = preset.video.bitrate.clone();
        options.audio_bitrate = preset.audio.bitrate.clone();
        options.crf = preset.video.crf;
        options.two_pass = preset.video.two_pass;

        if let Some(ref preset_name) = preset.video.preset {
            options.preset = preset_name.clone();
        }

        // Apply scale if resolution is specified
        if let (Some(width), Some(height)) = (preset.video.width, preset.video.height) {
            options.scale = Some(format!("{}:{}", width, height));
        }
    }

    // Validate input file
    validate_input(&options.input).await?;

    // Check output file
    check_output(&options.output, options.overwrite).await?;

    // Parse and validate codec options
    let video_codec = parse_video_codec(&options)?;
    let audio_codec = parse_audio_codec(&options)?;
    let preset = EncoderPreset::from_str(&options.preset)?;

    // Validate CRF if specified
    if let Some(crf) = options.crf {
        if let Some(codec) = video_codec {
            codec.validate_crf(crf)?;
        }
    }

    // Parse bitrate if specified
    let video_bitrate = if let Some(ref br) = options.video_bitrate {
        Some(parse_bitrate(br)?)
    } else {
        None
    };

    let audio_bitrate = if let Some(ref br) = options.audio_bitrate {
        Some(parse_bitrate(br)?)
    } else {
        None
    };

    // Parse scale if specified
    let scale_dimensions = if let Some(ref scale) = options.scale {
        Some(parse_scale(scale)?)
    } else {
        None
    };

    // Print transcode plan
    print_transcode_plan(
        &options,
        video_codec,
        audio_codec,
        preset,
        video_bitrate,
        audio_bitrate,
        scale_dimensions,
    );

    // Perform the transcode
    if options.two_pass {
        info!("Using two-pass encoding");
        transcode_two_pass(
            &options,
            video_codec,
            audio_codec,
            preset,
            video_bitrate,
            scale_dimensions,
        )
        .await?;
    } else {
        info!("Using single-pass encoding");
        transcode_single_pass(
            &options,
            video_codec,
            audio_codec,
            preset,
            video_bitrate,
            scale_dimensions,
        )
        .await?;
    }

    // Print summary
    print_transcode_summary(&options.output).await?;

    Ok(())
}

/// Validate input file exists and is readable.
async fn validate_input(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!("Input file does not exist: {}", path.display()));
    }

    if !path.is_file() {
        return Err(anyhow!("Input path is not a file: {}", path.display()));
    }

    let metadata = tokio::fs::metadata(path)
        .await
        .context("Failed to read input file metadata")?;

    if metadata.len() == 0 {
        return Err(anyhow!("Input file is empty"));
    }

    Ok(())
}

/// Check if output file exists and handle overwrite logic.
async fn check_output(path: &Path, overwrite: bool) -> Result<()> {
    if path.exists() {
        if overwrite {
            info!(
                "Output file exists, will be overwritten: {}",
                path.display()
            );
        } else {
            return Err(anyhow!(
                "Output file already exists: {}. Use -y to overwrite.",
                path.display()
            ));
        }
    }

    // Ensure output directory exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create output directory")?;
        }
    }

    Ok(())
}

/// Parse video codec from options.
fn parse_video_codec(options: &TranscodeOptions) -> Result<Option<VideoCodec>> {
    if let Some(ref codec) = options.video_codec {
        Ok(Some(VideoCodec::from_str(codec)?))
    } else {
        // Auto-detect from output extension
        if let Some(ext) = options.output.extension() {
            match ext.to_str() {
                Some("webm") => Ok(Some(VideoCodec::Vp9)),
                Some("mkv") => Ok(Some(VideoCodec::Av1)),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
}

/// Parse audio codec from options.
fn parse_audio_codec(options: &TranscodeOptions) -> Result<Option<AudioCodec>> {
    if let Some(ref codec) = options.audio_codec {
        Ok(Some(AudioCodec::from_str(codec)?))
    } else {
        // Auto-detect from output extension
        if let Some(ext) = options.output.extension() {
            match ext.to_str() {
                Some("webm") | Some("mkv") => Ok(Some(AudioCodec::Opus)),
                Some("flac") => Ok(Some(AudioCodec::Flac)),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
}

/// Parse bitrate string (e.g., "2M", "500k") to bits per second.
fn parse_bitrate(s: &str) -> Result<u64> {
    let s = s.trim().to_lowercase();

    if let Some(stripped) = s.strip_suffix('m') {
        let value: f64 = stripped.parse().context("Invalid bitrate format")?;
        Ok((value * 1_000_000.0) as u64)
    } else if let Some(stripped) = s.strip_suffix('k') {
        let value: f64 = stripped.parse().context("Invalid bitrate format")?;
        Ok((value * 1_000.0) as u64)
    } else {
        s.parse::<u64>().context("Invalid bitrate format")
    }
}

/// Parse scale string (e.g., "1280:720", "1920:-1") to dimensions.
fn parse_scale(s: &str) -> Result<(Option<u32>, Option<u32>)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid scale format. Expected 'width:height'"));
    }

    let width = if parts[0] == "-1" {
        None
    } else {
        Some(parts[0].parse().context("Invalid width")?)
    };

    let height = if parts[1] == "-1" {
        None
    } else {
        Some(parts[1].parse().context("Invalid height")?)
    };

    Ok((width, height))
}

/// Print the transcode plan before starting.
#[allow(clippy::too_many_arguments)]
fn print_transcode_plan(
    options: &TranscodeOptions,
    video_codec: Option<VideoCodec>,
    audio_codec: Option<AudioCodec>,
    preset: EncoderPreset,
    video_bitrate: Option<u64>,
    audio_bitrate: Option<u64>,
    scale: Option<(Option<u32>, Option<u32>)>,
) {
    println!("{}", "Transcode Plan".cyan().bold());
    println!("{}", "=".repeat(60));
    println!("{:20} {}", "Input:", options.input.display());
    println!("{:20} {}", "Output:", options.output.display());

    if let Some(codec) = video_codec {
        println!("{:20} {}", "Video Codec:", codec.name());
    }

    if let Some(codec) = audio_codec {
        println!("{:20} {}", "Audio Codec:", codec.name());
    }

    println!("{:20} {:?}", "Preset:", preset);

    if let Some(bitrate) = video_bitrate {
        println!("{:20} {} bps", "Video Bitrate:", bitrate);
    }

    if let Some(bitrate) = audio_bitrate {
        println!("{:20} {} bps", "Audio Bitrate:", bitrate);
    }

    if let Some((w, h)) = scale {
        println!(
            "{:20} {}x{}",
            "Scale:",
            w.map_or("-1".to_string(), |v| v.to_string()),
            h.map_or("-1".to_string(), |v| v.to_string())
        );
    }

    if options.two_pass {
        println!("{:20} {}", "Mode:", "Two-pass".yellow());
    }

    if let Some(crf) = options.crf {
        println!("{:20} {}", "CRF:", crf);
    }

    println!("{}", "=".repeat(60));
    println!();
}

/// Perform single-pass transcode.
#[allow(dead_code)]
async fn transcode_single_pass(
    _options: &TranscodeOptions,
    _video_codec: Option<VideoCodec>,
    _audio_codec: Option<AudioCodec>,
    _preset: EncoderPreset,
    _video_bitrate: Option<u64>,
    _scale: Option<(Option<u32>, Option<u32>)>,
) -> Result<()> {
    info!("Starting single-pass encode");

    // TODO: Implement actual transcoding using oximedia-codec and oximedia-graph
    // For now, this is a placeholder that demonstrates the progress bar

    let total_frames = 1000; // This would come from demuxing the input
    let mut progress = TranscodeProgress::new(total_frames);

    // Simulate encoding
    for i in 0..total_frames {
        // Simulate frame processing
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        progress.update(i + 1);
        progress.set_bytes_written((i + 1) * 10000);
    }

    progress.finish();

    warn!("Note: Actual encoding not yet implemented. This is a placeholder.");

    Ok(())
}

/// Perform two-pass transcode.
#[allow(dead_code)]
async fn transcode_two_pass(
    options: &TranscodeOptions,
    video_codec: Option<VideoCodec>,
    audio_codec: Option<AudioCodec>,
    preset: EncoderPreset,
    video_bitrate: Option<u64>,
    scale: Option<(Option<u32>, Option<u32>)>,
) -> Result<()> {
    info!("Starting two-pass encode");

    // Pass 1: Analysis
    println!("\n{}", "Pass 1/2: Analysis".yellow().bold());
    transcode_single_pass(
        options,
        video_codec,
        audio_codec,
        preset,
        video_bitrate,
        scale,
    )
    .await?;

    // Pass 2: Final encode
    println!("\n{}", "Pass 2/2: Final Encode".yellow().bold());
    transcode_single_pass(
        options,
        video_codec,
        audio_codec,
        preset,
        video_bitrate,
        scale,
    )
    .await?;

    Ok(())
}

/// Print transcode summary after completion.
async fn print_transcode_summary(output: &Path) -> Result<()> {
    let metadata = fs::metadata(output).context("Failed to read output file metadata")?;

    println!();
    println!("{}", "Transcode Complete".green().bold());
    println!("{}", "=".repeat(60));
    println!("{:20} {}", "Output File:", output.display());
    println!(
        "{:20} {:.2} MB",
        "File Size:",
        metadata.len() as f64 / 1_048_576.0
    );
    println!("{}", "=".repeat(60));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bitrate() {
        assert_eq!(parse_bitrate("2M").unwrap(), 2_000_000);
        assert_eq!(parse_bitrate("500k").unwrap(), 500_000);
        assert_eq!(parse_bitrate("1000").unwrap(), 1000);
    }

    #[test]
    fn test_parse_scale() {
        assert_eq!(parse_scale("1280:720").unwrap(), (Some(1280), Some(720)));
        assert_eq!(parse_scale("1920:-1").unwrap(), (Some(1920), None));
        assert_eq!(parse_scale("-1:1080").unwrap(), (None, Some(1080)));
    }

    #[test]
    fn test_video_codec_parsing() {
        assert_eq!(VideoCodec::from_str("av1").unwrap(), VideoCodec::Av1);
        assert_eq!(VideoCodec::from_str("vp9").unwrap(), VideoCodec::Vp9);
        assert_eq!(VideoCodec::from_str("vp8").unwrap(), VideoCodec::Vp8);
        assert!(VideoCodec::from_str("h264").is_err());
    }

    #[test]
    fn test_audio_codec_parsing() {
        assert_eq!(AudioCodec::from_str("opus").unwrap(), AudioCodec::Opus);
        assert_eq!(AudioCodec::from_str("vorbis").unwrap(), AudioCodec::Vorbis);
        assert_eq!(AudioCodec::from_str("flac").unwrap(), AudioCodec::Flac);
        assert!(AudioCodec::from_str("aac").is_err());
    }

    #[test]
    fn test_crf_validation() {
        let av1 = VideoCodec::Av1;
        assert!(av1.validate_crf(30).is_ok());
        assert!(av1.validate_crf(255).is_ok());
        assert!(av1.validate_crf(256).is_err());

        let vp9 = VideoCodec::Vp9;
        assert!(vp9.validate_crf(31).is_ok());
        assert!(vp9.validate_crf(63).is_ok());
        assert!(vp9.validate_crf(64).is_err());
    }
}
