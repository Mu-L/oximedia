//! Media container and codec validation
//!
//! This module provides comprehensive validation for:
//! - Container formats (Matroska/MKV, MP4, AVI, etc.)
//! - Codec parameters
//! - Stream integrity
//! - Metadata validation
//! - Structural validation
//! - Format compliance checking

use crate::{ArchiveError, ArchiveResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use tokio::fs;
use tracing::{debug, info};

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub file_path: String,
    pub container_format: Option<String>,
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
    pub streams: Vec<StreamInfo>,
    pub metadata: Option<MediaMetadata>,
}

/// Validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub severity: ErrorSeverity,
    pub error_type: ErrorType,
    pub message: String,
    pub stream_index: Option<usize>,
}

/// Error severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Critical,
    Major,
    Minor,
    Warning,
}

/// Error type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorType {
    ContainerCorruption,
    StreamCorruption,
    CodecError,
    MetadataError,
    StructuralError,
    ComplianceViolation,
    Unknown,
}

/// Stream information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub index: usize,
    pub codec_type: String,
    pub codec_name: String,
    pub duration: Option<f64>,
    pub bitrate: Option<u64>,
    pub valid: bool,
    pub errors: Vec<String>,
}

/// Media metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub title: Option<String>,
    pub duration: Option<f64>,
    pub bitrate: Option<u64>,
    pub format_name: Option<String>,
    pub format_long_name: Option<String>,
    pub size: Option<u64>,
}

/// Validate a media file
pub async fn validate_file(path: &Path) -> ArchiveResult<ValidationResult> {
    info!("Validating file: {}", path.display());

    if !path.exists() {
        return Err(ArchiveError::Validation("File does not exist".to_string()));
    }

    let metadata = fs::metadata(path).await?;
    if !metadata.is_file() {
        return Err(ArchiveError::Validation("Path is not a file".to_string()));
    }

    // Detect container format
    let container_format = detect_container_format(path).await?;

    let mut result = ValidationResult {
        file_path: path.to_string_lossy().to_string(),
        container_format: Some(container_format.clone()),
        is_valid: true,
        errors: Vec::new(),
        warnings: Vec::new(),
        streams: Vec::new(),
        metadata: None,
    };

    // Validate using ffprobe (if available)
    if let Ok(ffprobe_result) = validate_with_ffprobe(path).await {
        result.streams = ffprobe_result.streams;
        result.metadata = Some(ffprobe_result.metadata);

        // Check for errors in streams
        for stream in &result.streams {
            if !stream.valid {
                result.is_valid = false;
                for error in &stream.errors {
                    result.errors.push(ValidationError {
                        severity: ErrorSeverity::Major,
                        error_type: ErrorType::StreamCorruption,
                        message: error.clone(),
                        stream_index: Some(stream.index),
                    });
                }
            }
        }
    }

    // Format-specific validation
    match container_format.as_str() {
        "matroska" | "mkv" => validate_matroska(path, &mut result).await?,
        "mp4" | "mov" => validate_mp4(path, &mut result).await?,
        "avi" => validate_avi(path, &mut result).await?,
        _ => {
            debug!("No specific validator for format: {}", container_format);
        }
    }

    // Structural validation
    validate_structure(path, &mut result).await?;

    // Compliance checking
    check_compliance(&container_format, &mut result).await?;

    if !result.errors.is_empty() {
        result.is_valid = false;
    }

    Ok(result)
}

/// Detect container format
pub async fn detect_container_format(path: &Path) -> ArchiveResult<String> {
    // Read magic bytes
    let mut file = fs::File::open(path).await?;
    let mut buffer = vec![0u8; 12];

    use tokio::io::AsyncReadExt;
    file.read_exact(&mut buffer).await?;

    // Check for common formats
    if buffer.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Ok("matroska".to_string());
    }

    if buffer[4..8] == [b'f', b't', b'y', b'p'] {
        return Ok("mp4".to_string());
    }

    if buffer.starts_with(b"RIFF") && buffer[8..12] == *b"AVI " {
        return Ok("avi".to_string());
    }

    if buffer.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok("jpeg".to_string());
    }

    if buffer.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Ok("png".to_string());
    }

    // Try to detect from extension
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        match ext_str.as_str() {
            "mkv" | "mka" | "mks" => return Ok("matroska".to_string()),
            "mp4" | "m4v" | "m4a" => return Ok("mp4".to_string()),
            "mov" => return Ok("mov".to_string()),
            "avi" => return Ok("avi".to_string()),
            "webm" => return Ok("webm".to_string()),
            _ => {}
        }
    }

    Ok("unknown".to_string())
}

/// Validate using ffprobe
#[derive(Debug)]
struct FfprobeResult {
    streams: Vec<StreamInfo>,
    metadata: MediaMetadata,
}

async fn validate_with_ffprobe(path: &Path) -> ArchiveResult<FfprobeResult> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
            path.to_str()
                .ok_or_else(|| ArchiveError::Validation("Invalid path".to_string()))?,
        ])
        .output()
        .map_err(|e| ArchiveError::Validation(format!("ffprobe not available: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ArchiveError::Validation(format!(
            "ffprobe failed: {stderr}"
        )));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| ArchiveError::Validation(format!("Failed to parse ffprobe output: {e}")))?;

    // Parse streams
    let mut streams = Vec::new();
    if let Some(streams_array) = json["streams"].as_array() {
        for (index, stream) in streams_array.iter().enumerate() {
            let codec_type = stream["codec_type"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let codec_name = stream["codec_name"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();
            let duration = stream["duration"]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok());
            let bitrate = stream["bit_rate"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok());

            streams.push(StreamInfo {
                index,
                codec_type,
                codec_name,
                duration,
                bitrate,
                valid: true,
                errors: Vec::new(),
            });
        }
    }

    // Parse format metadata
    let format = &json["format"];
    let metadata = MediaMetadata {
        title: format["tags"]["title"].as_str().map(String::from),
        duration: format["duration"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok()),
        bitrate: format["bit_rate"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok()),
        format_name: format["format_name"].as_str().map(String::from),
        format_long_name: format["format_long_name"].as_str().map(String::from),
        size: format["size"].as_str().and_then(|s| s.parse::<u64>().ok()),
    };

    Ok(FfprobeResult { streams, metadata })
}

/// Validate Matroska/MKV container
async fn validate_matroska(path: &Path, result: &mut ValidationResult) -> ArchiveResult<()> {
    debug!(
        "Performing Matroska-specific validation for {}",
        path.display()
    );

    // Check EBML header
    let mut file = fs::File::open(path).await?;
    let mut header = vec![0u8; 4];

    use tokio::io::AsyncReadExt;
    file.read_exact(&mut header).await?;

    if header != [0x1A, 0x45, 0xDF, 0xA3] {
        result.errors.push(ValidationError {
            severity: ErrorSeverity::Critical,
            error_type: ErrorType::ContainerCorruption,
            message: "Invalid EBML header".to_string(),
            stream_index: None,
        });
        return Ok(());
    }

    // Use mkvinfo if available
    if let Ok(output) = Command::new("mkvinfo")
        .arg(
            path.to_str()
                .ok_or_else(|| ArchiveError::Validation("Invalid path".to_string()))?,
        )
        .output()
    {
        if !output.status.success() {
            result.errors.push(ValidationError {
                severity: ErrorSeverity::Major,
                error_type: ErrorType::StructuralError,
                message: "mkvinfo validation failed".to_string(),
                stream_index: None,
            });
        }
    }

    Ok(())
}

/// Validate MP4 container
async fn validate_mp4(path: &Path, result: &mut ValidationResult) -> ArchiveResult<()> {
    debug!("Performing MP4-specific validation for {}", path.display());

    // Check ftyp box
    let mut file = fs::File::open(path).await?;
    let mut header = vec![0u8; 12];

    use tokio::io::AsyncReadExt;
    file.read_exact(&mut header).await?;

    if header[4..8] != [b'f', b't', b'y', b'p'] {
        result.errors.push(ValidationError {
            severity: ErrorSeverity::Critical,
            error_type: ErrorType::ContainerCorruption,
            message: "Invalid MP4 ftyp box".to_string(),
            stream_index: None,
        });
        return Ok(());
    }

    // Validate atom structure
    validate_mp4_atoms(path, result).await?;

    Ok(())
}

/// Validate MP4 atom structure
async fn validate_mp4_atoms(path: &Path, result: &mut ValidationResult) -> ArchiveResult<()> {
    // Basic atom validation
    let file = fs::File::open(path).await?;
    let metadata = file.metadata().await?;
    let file_size = metadata.len();

    // Check if file is large enough to contain required atoms
    if file_size < 32 {
        result.errors.push(ValidationError {
            severity: ErrorSeverity::Critical,
            error_type: ErrorType::StructuralError,
            message: "File too small to be a valid MP4".to_string(),
            stream_index: None,
        });
    }

    // Use MP4Box if available for detailed validation
    if let Ok(output) = Command::new("MP4Box")
        .args([
            "-info",
            path.to_str()
                .ok_or_else(|| ArchiveError::Validation("Invalid path".to_string()))?,
        ])
        .output()
    {
        if !output.status.success() {
            result
                .warnings
                .push("MP4Box validation reported issues".to_string());
        }
    }

    Ok(())
}

/// Validate AVI container
async fn validate_avi(path: &Path, result: &mut ValidationResult) -> ArchiveResult<()> {
    debug!("Performing AVI-specific validation for {}", path.display());

    // Check RIFF header
    let mut file = fs::File::open(path).await?;
    let mut header = vec![0u8; 12];

    use tokio::io::AsyncReadExt;
    file.read_exact(&mut header).await?;

    if &header[0..4] != b"RIFF" {
        result.errors.push(ValidationError {
            severity: ErrorSeverity::Critical,
            error_type: ErrorType::ContainerCorruption,
            message: "Invalid RIFF header".to_string(),
            stream_index: None,
        });
        return Ok(());
    }

    if &header[8..12] != b"AVI " {
        result.errors.push(ValidationError {
            severity: ErrorSeverity::Critical,
            error_type: ErrorType::ContainerCorruption,
            message: "Invalid AVI signature".to_string(),
            stream_index: None,
        });
        return Ok(());
    }

    Ok(())
}

/// Validate file structure
async fn validate_structure(path: &Path, result: &mut ValidationResult) -> ArchiveResult<()> {
    let metadata = fs::metadata(path).await?;
    let file_size = metadata.len();

    // Check minimum file size
    if file_size < 1024 {
        result
            .warnings
            .push(format!("File size is very small: {file_size} bytes"));
    }

    // Check if file is readable throughout
    let mut file = fs::File::open(path).await?;
    let mut buffer = vec![0u8; 8192];
    let mut total_read = 0u64;

    use tokio::io::AsyncReadExt;
    loop {
        match file.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => total_read += n as u64,
            Err(e) => {
                result.errors.push(ValidationError {
                    severity: ErrorSeverity::Critical,
                    error_type: ErrorType::StructuralError,
                    message: format!("Read error at byte {total_read}: {e}"),
                    stream_index: None,
                });
                return Ok(());
            }
        }
    }

    if total_read != file_size {
        result.errors.push(ValidationError {
            severity: ErrorSeverity::Major,
            error_type: ErrorType::StructuralError,
            message: format!(
                "File size mismatch: expected {file_size} bytes, read {total_read} bytes"
            ),
            stream_index: None,
        });
    }

    Ok(())
}

/// Check format compliance
async fn check_compliance(
    container_format: &str,
    result: &mut ValidationResult,
) -> ArchiveResult<()> {
    match container_format {
        "matroska" | "mkv" => {
            // Check Matroska compliance
            check_matroska_compliance(result).await?;
        }
        "mp4" | "mov" => {
            // Check MP4 compliance
            check_mp4_compliance(result).await?;
        }
        _ => {
            debug!("No compliance checks for format: {}", container_format);
        }
    }

    Ok(())
}

/// Check Matroska compliance
async fn check_matroska_compliance(result: &mut ValidationResult) -> ArchiveResult<()> {
    // Check for required Matroska elements
    // This is a simplified check - full compliance checking would require parsing EBML

    if result.streams.is_empty() {
        result
            .warnings
            .push("No streams found in Matroska file".to_string());
    }

    // Check for video stream if this is a video file
    let has_video = result.streams.iter().any(|s| s.codec_type == "video");
    let has_audio = result.streams.iter().any(|s| s.codec_type == "audio");

    if !has_video && !has_audio {
        result.errors.push(ValidationError {
            severity: ErrorSeverity::Major,
            error_type: ErrorType::ComplianceViolation,
            message: "No video or audio streams found".to_string(),
            stream_index: None,
        });
    }

    Ok(())
}

/// Check MP4 compliance
async fn check_mp4_compliance(result: &mut ValidationResult) -> ArchiveResult<()> {
    // Check for required MP4 boxes (simplified)

    if result.streams.is_empty() {
        result
            .warnings
            .push("No streams found in MP4 file".to_string());
    }

    // Check for common codec compliance
    for stream in &result.streams {
        if stream.codec_type == "video" {
            match stream.codec_name.as_str() {
                "h264" | "hevc" | "vp9" | "av1" => {
                    // Common codecs - OK
                }
                _ => {
                    result.warnings.push(format!(
                        "Uncommon video codec in MP4: {}",
                        stream.codec_name
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Codec parameter validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecValidation {
    pub codec_name: String,
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Validate codec parameters
#[allow(dead_code)]
pub async fn validate_codec_parameters(
    codec_name: &str,
    stream_info: &StreamInfo,
) -> ArchiveResult<CodecValidation> {
    let mut validation = CodecValidation {
        codec_name: codec_name.to_string(),
        is_valid: true,
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    // Check bitrate
    if stream_info.codec_type == "video" {
        if let Some(bitrate) = stream_info.bitrate {
            if bitrate < 100_000 {
                validation
                    .warnings
                    .push(format!("Very low bitrate: {bitrate} bps"));
            }
            if bitrate > 100_000_000 {
                validation
                    .warnings
                    .push(format!("Very high bitrate: {bitrate} bps"));
            }
        }
    }

    // Check duration
    if let Some(duration) = stream_info.duration {
        if duration <= 0.0 {
            validation
                .errors
                .push("Invalid duration: must be positive".to_string());
            validation.is_valid = false;
        }
    }

    Ok(validation)
}

/// Stream integrity check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamIntegrityResult {
    pub stream_index: usize,
    pub is_intact: bool,
    pub frame_errors: u32,
    pub packet_errors: u32,
    pub details: Vec<String>,
}

/// Check stream integrity using ffmpeg
#[allow(dead_code)]
pub async fn check_stream_integrity(
    path: &Path,
    stream_index: usize,
) -> ArchiveResult<StreamIntegrityResult> {
    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            path.to_str()
                .ok_or_else(|| ArchiveError::Validation("Invalid path".to_string()))?,
            "-map",
            &format!("0:{stream_index}"),
            "-f",
            "null",
            "-",
        ])
        .output()
        .map_err(|e| ArchiveError::Validation(format!("ffmpeg not available: {e}")))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let frame_errors = stderr.matches("error").count() as u32;
    let packet_errors = stderr.matches("corrupt").count() as u32;

    let is_intact = frame_errors == 0 && packet_errors == 0;

    let mut details = Vec::new();
    if !is_intact {
        for line in stderr.lines() {
            if line.contains("error") || line.contains("corrupt") {
                details.push(line.to_string());
            }
        }
    }

    Ok(StreamIntegrityResult {
        stream_index,
        is_intact,
        frame_errors,
        packet_errors,
        details,
    })
}

/// Metadata validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataValidation {
    pub has_duration: bool,
    pub has_bitrate: bool,
    pub has_title: bool,
    pub is_complete: bool,
    pub missing_fields: Vec<String>,
}

/// Validate metadata completeness
pub fn validate_metadata(metadata: &MediaMetadata) -> MetadataValidation {
    let has_duration = metadata.duration.is_some();
    let has_bitrate = metadata.bitrate.is_some();
    let has_title = metadata.title.is_some();

    let mut missing_fields = Vec::new();
    if !has_duration {
        missing_fields.push("duration".to_string());
    }
    if !has_bitrate {
        missing_fields.push("bitrate".to_string());
    }

    let is_complete = missing_fields.is_empty();

    MetadataValidation {
        has_duration,
        has_bitrate,
        has_title,
        is_complete,
        missing_fields,
    }
}

/// Deep validation (comprehensive check)
#[allow(dead_code)]
pub async fn deep_validate(path: &Path) -> ArchiveResult<DeepValidationResult> {
    info!("Performing deep validation for {}", path.display());

    let mut result = DeepValidationResult {
        file_path: path.to_string_lossy().to_string(),
        validation_result: validate_file(path).await?,
        stream_integrity: Vec::new(),
        codec_validations: Vec::new(),
        metadata_validation: None,
    };

    // Check each stream's integrity
    for stream in &result.validation_result.streams {
        if let Ok(integrity) = check_stream_integrity(path, stream.index).await {
            result.stream_integrity.push(integrity);
        }
    }

    // Validate codec parameters
    for stream in &result.validation_result.streams {
        if let Ok(codec_val) = validate_codec_parameters(&stream.codec_name, stream).await {
            result.codec_validations.push(codec_val);
        }
    }

    // Validate metadata
    if let Some(ref metadata) = result.validation_result.metadata {
        result.metadata_validation = Some(validate_metadata(metadata));
    }

    Ok(result)
}

/// Deep validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepValidationResult {
    pub file_path: String,
    pub validation_result: ValidationResult,
    pub stream_integrity: Vec<StreamIntegrityResult>,
    pub codec_validations: Vec<CodecValidation>,
    pub metadata_validation: Option<MetadataValidation>,
}

impl DeepValidationResult {
    /// Check if all validations passed
    pub fn all_passed(&self) -> bool {
        self.validation_result.is_valid
            && self.stream_integrity.iter().all(|s| s.is_intact)
            && self.codec_validations.iter().all(|c| c.is_valid)
    }
}
