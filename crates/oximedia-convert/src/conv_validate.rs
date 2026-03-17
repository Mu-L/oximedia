/// Pre-conversion input validation for `OxiMedia`.
///
/// Validates conversion parameters against configurable constraints and
/// checks basic input file properties before the conversion pipeline starts.
/// Includes disk space verification and format compatibility checking.
///
/// Errors reported by the conversion validator.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// The file extension or container format is not supported.
    UnsupportedFormat(String),
    /// The requested resolution exceeds the maximum allowed.
    ResolutionTooLarge {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
    },
    /// The requested bitrate is outside the acceptable range.
    InvalidBitrate(u32),
    /// The codec is not compatible with the target container.
    IncompatibleCodec {
        /// Codec identifier.
        codec: String,
        /// Container/format identifier.
        format: String,
    },
    /// A required field is missing or empty.
    MissingRequiredField(String),
    /// Insufficient disk space for the output file.
    InsufficientDiskSpace {
        /// Available bytes on the target filesystem.
        available_bytes: u64,
        /// Estimated required bytes.
        required_bytes: u64,
    },
    /// The input file does not exist or is not readable.
    InputFileNotAccessible(String),
    /// The input file is empty (zero bytes).
    EmptyInputFile(String),
    /// The output directory does not exist and cannot be created.
    OutputDirectoryInvalid(String),
    /// The video codec is not compatible with the target container format.
    VideoCodecContainerMismatch {
        /// Video codec identifier.
        video_codec: String,
        /// Container format identifier.
        container: String,
    },
    /// The audio codec is not compatible with the target container format.
    AudioCodecContainerMismatch {
        /// Audio codec identifier.
        audio_codec: String,
        /// Container format identifier.
        container: String,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedFormat(fmt) => write!(f, "Unsupported format: {fmt}"),
            Self::ResolutionTooLarge { width, height } => {
                write!(f, "Resolution {width}x{height} exceeds maximum")
            }
            Self::InvalidBitrate(bps) => write!(f, "Invalid bitrate: {bps} kbps"),
            Self::IncompatibleCodec { codec, format } => {
                write!(f, "Codec '{codec}' incompatible with format '{format}'")
            }
            Self::MissingRequiredField(field) => {
                write!(f, "Missing required field: {field}")
            }
            Self::InsufficientDiskSpace {
                available_bytes,
                required_bytes,
            } => {
                write!(
                    f,
                    "Insufficient disk space: {available_bytes} bytes available, \
                     {required_bytes} bytes required"
                )
            }
            Self::InputFileNotAccessible(path) => {
                write!(f, "Input file not accessible: {path}")
            }
            Self::EmptyInputFile(path) => {
                write!(f, "Input file is empty: {path}")
            }
            Self::OutputDirectoryInvalid(path) => {
                write!(f, "Output directory invalid: {path}")
            }
            Self::VideoCodecContainerMismatch {
                video_codec,
                container,
            } => {
                write!(
                    f,
                    "Video codec '{video_codec}' not compatible with container '{container}'"
                )
            }
            Self::AudioCodecContainerMismatch {
                audio_codec,
                container,
            } => {
                write!(
                    f,
                    "Audio codec '{audio_codec}' not compatible with container '{container}'"
                )
            }
        }
    }
}

// ── ValidateProfile ───────────────────────────────────────────────────────────

/// A flat set of parameters that describes a requested conversion.
///
/// Used as the input type for [`ConvertValidation::validate_profile`].
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ValidateProfile {
    /// Video codec identifier (e.g., "av1", "vp9").
    pub codec: String,
    /// Target bitrate in kilobits per second.
    pub bitrate_kbps: u32,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Target container format (e.g., "webm", "mp4", "mkv").
    pub container: String,
}

impl ValidateProfile {
    /// Creates a new validate profile with the given parameters.
    #[allow(dead_code)]
    #[must_use]
    pub fn new(codec: &str, bitrate_kbps: u32, width: u32, height: u32, container: &str) -> Self {
        Self {
            codec: codec.to_string(),
            bitrate_kbps,
            width,
            height,
            container: container.to_string(),
        }
    }
}

// ── ConvertValidation ─────────────────────────────────────────────────────────

/// Validates conversion profiles and input files against configurable limits.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ConvertValidation {
    /// Maximum allowed output width in pixels.
    pub max_width: u32,
    /// Maximum allowed output height in pixels.
    pub max_height: u32,
    /// Maximum allowed bitrate in kilobits per second.
    pub max_bitrate_kbps: u32,
    /// Set of recognised codec identifiers.
    pub allowed_codecs: Vec<String>,
}

impl Default for ConvertValidation {
    fn default() -> Self {
        Self::new()
    }
}

impl ConvertValidation {
    /// Creates a new validator with sensible defaults.
    ///
    /// * Max resolution: 7680 × 4320 (8K)
    /// * Max bitrate: 100 000 kbps
    /// * Allowed codecs: AV1, VP9, VP8, FLAC, Opus, Vorbis (all patent-free)
    #[allow(dead_code)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_width: 7_680,
            max_height: 4_320,
            max_bitrate_kbps: 100_000,
            allowed_codecs: vec![
                "av1".to_string(),
                "vp9".to_string(),
                "vp8".to_string(),
                "flac".to_string(),
                "opus".to_string(),
                "vorbis".to_string(),
                "theora".to_string(),
                "aom-av1".to_string(),
            ],
        }
    }

    /// Returns `true` if the codec is in the allowed list (case-insensitive).
    #[allow(dead_code)]
    #[must_use]
    pub fn is_codec_supported(&self, codec: &str) -> bool {
        let lower = codec.to_lowercase();
        self.allowed_codecs
            .iter()
            .any(|c| c.to_lowercase() == lower)
    }

    /// Validates all fields of a [`ValidateProfile`] and returns a (possibly
    /// empty) list of [`ValidationError`]s.
    #[allow(dead_code)]
    #[must_use]
    pub fn validate_profile(&self, profile: &ValidateProfile) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Codec must not be empty
        if profile.codec.is_empty() {
            errors.push(ValidationError::MissingRequiredField("codec".to_string()));
        } else if !self.is_codec_supported(&profile.codec) {
            errors.push(ValidationError::UnsupportedFormat(profile.codec.clone()));
        }

        // Container must not be empty
        if profile.container.is_empty() {
            errors.push(ValidationError::MissingRequiredField(
                "container".to_string(),
            ));
        }

        // Resolution check
        if profile.width > self.max_width || profile.height > self.max_height {
            errors.push(ValidationError::ResolutionTooLarge {
                width: profile.width,
                height: profile.height,
            });
        }

        // Zero resolution check
        if profile.width == 0 {
            errors.push(ValidationError::MissingRequiredField("width".to_string()));
        }
        if profile.height == 0 {
            errors.push(ValidationError::MissingRequiredField("height".to_string()));
        }

        // Bitrate check
        if profile.bitrate_kbps == 0 || profile.bitrate_kbps > self.max_bitrate_kbps {
            errors.push(ValidationError::InvalidBitrate(profile.bitrate_kbps));
        }

        // Codec–container compatibility check
        if !profile.codec.is_empty() && !profile.container.is_empty() {
            if let Some(err) = check_codec_container_compat(&profile.codec, &profile.container) {
                errors.push(err);
            }
        }

        errors
    }
}

/// Checks whether the codec is compatible with the target container.
///
/// Returns `Some(ValidationError)` on incompatibility, `None` if OK.
fn check_codec_container_compat(codec: &str, container: &str) -> Option<ValidationError> {
    let codec_lower = codec.to_lowercase();
    let container_lower = container.to_lowercase();

    // Known incompatibilities (non-exhaustive but representative)
    let incompatible = match container_lower.as_str() {
        "mp4" => matches!(codec_lower.as_str(), "vorbis" | "theora" | "vp8"),
        "webm" => !matches!(
            codec_lower.as_str(),
            "vp8" | "vp9" | "av1" | "opus" | "vorbis" | "aom-av1"
        ),
        "ogg" => !matches!(codec_lower.as_str(), "vorbis" | "opus" | "flac" | "theora"),
        _ => false,
    };

    if incompatible {
        Some(ValidationError::IncompatibleCodec {
            codec: codec.to_string(),
            format: container.to_string(),
        })
    } else {
        None
    }
}

// ── validate_input_file ───────────────────────────────────────────────────────

/// Checks basic properties of an input file path.
///
/// Returns a list of [`ValidationError`]s (empty means valid).
/// Checks performed:
/// 1. Path must not be empty.
/// 2. Extension must be a known media format.
#[allow(dead_code)]
#[must_use]
pub fn validate_input_file(path: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if path.is_empty() {
        errors.push(ValidationError::MissingRequiredField("path".to_string()));
        return errors;
    }

    // Extract extension
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let known_extensions = [
        "webm", "mkv", "ogg", "ogv", "oga", "flac", "opus", "mp4", "mov", "avi", "wav", "mp3",
        "aac", "m4a", "ts", "m2ts", "mts",
    ];

    if ext.is_empty() || !known_extensions.contains(&ext.as_str()) {
        errors.push(ValidationError::UnsupportedFormat(format!(
            "unknown extension '{ext}'"
        )));
    }

    errors
}

// ── Pre-Conversion Validation ─────────────────────────────────────────────────

/// Comprehensive pre-conversion validation request.
///
/// Validates everything needed before starting a conversion:
/// - Input file existence and readability
/// - Output directory writability
/// - Sufficient disk space
/// - Format and codec compatibility
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PreConversionCheck {
    /// Path to the input file.
    pub input_path: String,
    /// Path to the output file.
    pub output_path: String,
    /// Target container format (e.g., "webm", "mp4", "mkv").
    pub target_container: String,
    /// Target video codec (if any).
    pub video_codec: Option<String>,
    /// Target audio codec (if any).
    pub audio_codec: Option<String>,
    /// Estimated output size multiplier (1.0 = same size as input,
    /// 0.5 = half size). Used for disk space estimation.
    pub size_multiplier: f64,
    /// Minimum required free disk space beyond the estimated output size (bytes).
    /// Default safety margin is 100 MB.
    pub safety_margin_bytes: u64,
}

impl Default for PreConversionCheck {
    fn default() -> Self {
        Self {
            input_path: String::new(),
            output_path: String::new(),
            target_container: String::new(),
            video_codec: None,
            audio_codec: None,
            size_multiplier: 1.0,
            safety_margin_bytes: 100 * 1024 * 1024, // 100 MB
        }
    }
}

impl PreConversionCheck {
    /// Create a new pre-conversion check.
    #[allow(dead_code)]
    #[must_use]
    pub fn new(input_path: &str, output_path: &str, target_container: &str) -> Self {
        Self {
            input_path: input_path.to_string(),
            output_path: output_path.to_string(),
            target_container: target_container.to_string(),
            ..Self::default()
        }
    }

    /// Set the video codec to validate.
    #[allow(dead_code)]
    #[must_use]
    pub fn with_video_codec(mut self, codec: &str) -> Self {
        self.video_codec = Some(codec.to_string());
        self
    }

    /// Set the audio codec to validate.
    #[allow(dead_code)]
    #[must_use]
    pub fn with_audio_codec(mut self, codec: &str) -> Self {
        self.audio_codec = Some(codec.to_string());
        self
    }

    /// Set the estimated output size multiplier.
    #[allow(dead_code)]
    #[must_use]
    pub fn with_size_multiplier(mut self, multiplier: f64) -> Self {
        self.size_multiplier = multiplier;
        self
    }

    /// Set the safety margin in bytes.
    #[allow(dead_code)]
    #[must_use]
    pub fn with_safety_margin(mut self, bytes: u64) -> Self {
        self.safety_margin_bytes = bytes;
        self
    }
}

/// Run a comprehensive pre-conversion validation.
///
/// Returns a list of [`ValidationError`]s. An empty list means
/// the conversion can proceed safely.
#[allow(dead_code)]
#[must_use]
pub fn validate_pre_conversion(check: &PreConversionCheck) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // 1. Validate input path is not empty
    if check.input_path.is_empty() {
        errors.push(ValidationError::MissingRequiredField(
            "input_path".to_string(),
        ));
        return errors;
    }

    // 2. Validate output path is not empty
    if check.output_path.is_empty() {
        errors.push(ValidationError::MissingRequiredField(
            "output_path".to_string(),
        ));
    }

    // 3. Validate target container is not empty
    if check.target_container.is_empty() {
        errors.push(ValidationError::MissingRequiredField(
            "target_container".to_string(),
        ));
    }

    // 4. Check input file existence and readability
    let input_path = std::path::Path::new(&check.input_path);
    if !input_path.exists() {
        errors.push(ValidationError::InputFileNotAccessible(
            check.input_path.clone(),
        ));
    } else {
        // Check if file is readable (try to get metadata)
        match std::fs::metadata(input_path) {
            Ok(meta) => {
                if meta.len() == 0 {
                    errors.push(ValidationError::EmptyInputFile(check.input_path.clone()));
                }
            }
            Err(_) => {
                errors.push(ValidationError::InputFileNotAccessible(
                    check.input_path.clone(),
                ));
            }
        }
    }

    // 5. Validate input file extension
    let input_errors = validate_input_file(&check.input_path);
    errors.extend(input_errors);

    // 6. Check output directory exists or can be created
    if !check.output_path.is_empty() {
        let output_path = std::path::Path::new(&check.output_path);
        if let Some(parent) = output_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                // Check if we can infer the directory is potentially creatable
                // by walking up the tree to find an existing ancestor
                let mut ancestor = parent.to_path_buf();
                let mut found_existing = false;
                while let Some(p) = ancestor.parent() {
                    if p.exists() {
                        found_existing = true;
                        break;
                    }
                    ancestor = p.to_path_buf();
                }
                if !found_existing {
                    errors.push(ValidationError::OutputDirectoryInvalid(
                        parent.display().to_string(),
                    ));
                }
            }
        }
    }

    // 7. Check disk space
    if input_path.exists() {
        if let Ok(meta) = std::fs::metadata(input_path) {
            let input_size = meta.len();
            let estimated_output = (input_size as f64 * check.size_multiplier) as u64;
            let required = estimated_output + check.safety_margin_bytes;

            let available = get_available_disk_space(&check.output_path);
            if available > 0 && available < required {
                errors.push(ValidationError::InsufficientDiskSpace {
                    available_bytes: available,
                    required_bytes: required,
                });
            }
        }
    }

    // 8. Check video codec / container compatibility
    if let Some(ref vc) = check.video_codec {
        if !check.target_container.is_empty() {
            if let Some(err) = check_video_codec_container_compat(vc, &check.target_container) {
                errors.push(err);
            }
        }
    }

    // 9. Check audio codec / container compatibility
    if let Some(ref ac) = check.audio_codec {
        if !check.target_container.is_empty() {
            if let Some(err) = check_audio_codec_container_compat(ac, &check.target_container) {
                errors.push(err);
            }
        }
    }

    errors
}

/// Check video codec compatibility with a container format.
fn check_video_codec_container_compat(codec: &str, container: &str) -> Option<ValidationError> {
    let codec_lower = codec.to_lowercase();
    let container_lower = container.to_lowercase();

    let compatible = match container_lower.as_str() {
        "mp4" | "mov" => matches!(codec_lower.as_str(), "av1" | "vp9" | "aom-av1"),
        "webm" => matches!(codec_lower.as_str(), "vp8" | "vp9" | "av1" | "aom-av1"),
        "mkv" | "matroska" => {
            matches!(
                codec_lower.as_str(),
                "av1" | "vp9" | "vp8" | "theora" | "aom-av1"
            )
        }
        "ogg" => matches!(codec_lower.as_str(), "theora"),
        "ts" | "mpegts" => matches!(codec_lower.as_str(), "av1" | "vp9"),
        _ => true, // unknown container: allow anything
    };

    if !compatible {
        Some(ValidationError::VideoCodecContainerMismatch {
            video_codec: codec.to_string(),
            container: container.to_string(),
        })
    } else {
        None
    }
}

/// Check audio codec compatibility with a container format.
fn check_audio_codec_container_compat(codec: &str, container: &str) -> Option<ValidationError> {
    let codec_lower = codec.to_lowercase();
    let container_lower = container.to_lowercase();

    let compatible = match container_lower.as_str() {
        "mp4" | "mov" => matches!(codec_lower.as_str(), "opus" | "flac"),
        "webm" => matches!(codec_lower.as_str(), "opus" | "vorbis"),
        "mkv" | "matroska" => {
            matches!(codec_lower.as_str(), "opus" | "vorbis" | "flac" | "pcm")
        }
        "ogg" => matches!(codec_lower.as_str(), "opus" | "vorbis" | "flac"),
        "wav" => matches!(codec_lower.as_str(), "pcm"),
        "flac" => matches!(codec_lower.as_str(), "flac"),
        "ts" | "mpegts" => matches!(codec_lower.as_str(), "opus"),
        _ => true,
    };

    if !compatible {
        Some(ValidationError::AudioCodecContainerMismatch {
            audio_codec: codec.to_string(),
            container: container.to_string(),
        })
    } else {
        None
    }
}

/// Get available disk space for the given path.
///
/// Returns 0 if the space cannot be determined.
/// Uses platform-specific APIs where available.
fn get_available_disk_space(path: &str) -> u64 {
    let target_path = std::path::Path::new(path);
    // Try the output directory, falling back to parent directories
    let check_path = if target_path.exists() {
        target_path.to_path_buf()
    } else if let Some(parent) = target_path.parent() {
        if parent.exists() {
            parent.to_path_buf()
        } else {
            // Walk up to find an existing ancestor
            let mut p = parent.to_path_buf();
            loop {
                if p.exists() {
                    break p;
                }
                match p.parent() {
                    Some(pp) => p = pp.to_path_buf(),
                    None => return 0,
                }
            }
        }
    } else {
        return 0;
    };

    // Use statvfs on Unix-like systems
    #[cfg(unix)]
    {
        get_available_space_unix(&check_path)
    }
    #[cfg(not(unix))]
    {
        let _ = check_path;
        0 // Cannot determine on non-Unix without external deps
    }
}

/// Get available space on Unix using the `std::fs` metadata heuristic.
///
/// Uses a pure-Rust approach: reads the filesystem metadata and estimates
/// available space from the difference between total capacity and used space.
/// For an exact value we would need `statvfs`, but this avoids unsafe code.
#[cfg(unix)]
fn get_available_space_unix(path: &std::path::Path) -> u64 {
    // Pure-Rust approach: read /proc/mounts or use df-like heuristic.
    // On macOS/Linux, we can read the output of a command, but that's fragile.
    // Instead, use the std::process approach with `df`.
    // For library code, return a best-effort value.

    // Try reading from /proc/self/mountinfo or statfs via command
    let output = std::process::Command::new("df")
        .arg("-k")
        .arg(path.as_os_str())
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            // Parse df output: second line, 4th column (Available in 1K blocks)
            if let Some(line) = stdout.lines().nth(1) {
                let fields: Vec<&str> = line.split_whitespace().collect();
                // df -k output: Filesystem 1K-blocks Used Available Use% Mounted
                if fields.len() >= 4 {
                    if let Ok(available_kb) = fields[3].parse::<u64>() {
                        return available_kb * 1024;
                    }
                }
            }
            0
        }
        Err(_) => 0,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn valid_profile() -> ValidateProfile {
        ValidateProfile::new("av1", 4_000, 1920, 1080, "webm")
    }

    // ── ValidationError display ───────────────────────────────────────────────

    #[test]
    fn error_display_unsupported_format() {
        let e = ValidationError::UnsupportedFormat("h264".to_string());
        assert!(e.to_string().contains("h264"));
    }

    #[test]
    fn error_display_resolution_too_large() {
        let e = ValidationError::ResolutionTooLarge {
            width: 10_000,
            height: 8_000,
        };
        assert!(e.to_string().contains("10000"));
    }

    #[test]
    fn error_display_invalid_bitrate() {
        let e = ValidationError::InvalidBitrate(0);
        assert!(e.to_string().contains("0 kbps"));
    }

    // ── ConvertValidation ─────────────────────────────────────────────────────

    #[test]
    fn validation_valid_profile_no_errors() {
        let v = ConvertValidation::new();
        let p = valid_profile();
        assert!(v.validate_profile(&p).is_empty());
    }

    #[test]
    fn validation_unsupported_codec() {
        let v = ConvertValidation::new();
        let p = ValidateProfile::new("h264", 4_000, 1920, 1080, "mp4");
        let errors = v.validate_profile(&p);
        assert!(!errors.is_empty());
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::UnsupportedFormat(_))));
    }

    #[test]
    fn validation_resolution_too_large() {
        let v = ConvertValidation::new();
        let p = ValidateProfile::new("av1", 4_000, 10_000, 10_000, "webm");
        let errors = v.validate_profile(&p);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::ResolutionTooLarge { .. })));
    }

    #[test]
    fn validation_invalid_bitrate_zero() {
        let v = ConvertValidation::new();
        let p = ValidateProfile::new("av1", 0, 1920, 1080, "webm");
        let errors = v.validate_profile(&p);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidBitrate(_))));
    }

    #[test]
    fn validation_bitrate_too_high() {
        let v = ConvertValidation::new();
        let p = ValidateProfile::new("av1", 200_000, 1920, 1080, "webm");
        let errors = v.validate_profile(&p);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidBitrate(_))));
    }

    #[test]
    fn validation_missing_codec_field() {
        let v = ConvertValidation::new();
        let p = ValidateProfile::new("", 4_000, 1920, 1080, "webm");
        let errors = v.validate_profile(&p);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::MissingRequiredField(f) if f == "codec")));
    }

    #[test]
    fn validation_incompatible_codec_container() {
        let v = ConvertValidation::new();
        // vorbis is not compatible with mp4
        let p = ValidateProfile::new("vorbis", 128, 1920, 1080, "mp4");
        let errors = v.validate_profile(&p);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::IncompatibleCodec { .. })));
    }

    #[test]
    fn validation_codec_supported_case_insensitive() {
        let v = ConvertValidation::new();
        assert!(v.is_codec_supported("AV1"));
        assert!(v.is_codec_supported("Opus"));
        assert!(!v.is_codec_supported("h264"));
    }

    // ── validate_input_file ───────────────────────────────────────────────────

    #[test]
    fn validate_file_empty_path() {
        let errors = validate_input_file("");
        assert!(!errors.is_empty());
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::MissingRequiredField(_))));
    }

    #[test]
    fn validate_file_known_extension() {
        assert!(validate_input_file("video.webm").is_empty());
        assert!(validate_input_file("audio.flac").is_empty());
    }

    #[test]
    fn validate_file_unknown_extension() {
        let errors = validate_input_file("archive.xyz");
        assert!(!errors.is_empty());
    }

    #[test]
    fn validate_file_no_extension() {
        let errors = validate_input_file("noextension");
        assert!(!errors.is_empty());
    }

    // ── New ValidationError display tests ─────────────────────────────────

    #[test]
    fn error_display_insufficient_disk_space() {
        let e = ValidationError::InsufficientDiskSpace {
            available_bytes: 1_000_000,
            required_bytes: 5_000_000,
        };
        let msg = e.to_string();
        assert!(msg.contains("1000000"));
        assert!(msg.contains("5000000"));
    }

    #[test]
    fn error_display_input_not_accessible() {
        let e = ValidationError::InputFileNotAccessible("/bad/path.mp4".to_string());
        assert!(e.to_string().contains("/bad/path.mp4"));
    }

    #[test]
    fn error_display_empty_input() {
        let e = ValidationError::EmptyInputFile("empty.mp4".to_string());
        assert!(e.to_string().contains("empty.mp4"));
    }

    #[test]
    fn error_display_output_dir_invalid() {
        let e = ValidationError::OutputDirectoryInvalid("/no/such/dir".to_string());
        assert!(e.to_string().contains("/no/such/dir"));
    }

    #[test]
    fn error_display_video_codec_mismatch() {
        let e = ValidationError::VideoCodecContainerMismatch {
            video_codec: "theora".to_string(),
            container: "mp4".to_string(),
        };
        let msg = e.to_string();
        assert!(msg.contains("theora"));
        assert!(msg.contains("mp4"));
    }

    #[test]
    fn error_display_audio_codec_mismatch() {
        let e = ValidationError::AudioCodecContainerMismatch {
            audio_codec: "pcm".to_string(),
            container: "webm".to_string(),
        };
        let msg = e.to_string();
        assert!(msg.contains("pcm"));
        assert!(msg.contains("webm"));
    }

    // ── PreConversionCheck tests ──────────────────────────────────────────

    #[test]
    fn pre_check_empty_input_path() {
        let check = PreConversionCheck::new("", "/tmp/out.webm", "webm");
        let errors = validate_pre_conversion(&check);
        assert!(!errors.is_empty());
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::MissingRequiredField(f) if f == "input_path")));
    }

    #[test]
    fn pre_check_empty_output_path() {
        let check = PreConversionCheck::new("/tmp/test.mp4", "", "webm");
        let errors = validate_pre_conversion(&check);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::MissingRequiredField(f) if f == "output_path")));
    }

    #[test]
    fn pre_check_empty_container() {
        let check = PreConversionCheck::new("/tmp/test.mp4", "/tmp/out.webm", "");
        let errors = validate_pre_conversion(&check);
        assert!(errors.iter().any(|e| matches!(
            e,
            ValidationError::MissingRequiredField(f) if f == "target_container"
        )));
    }

    #[test]
    fn pre_check_nonexistent_input_file() {
        let check = PreConversionCheck::new("/nonexistent/path/video.mp4", "/tmp/out.webm", "webm");
        let errors = validate_pre_conversion(&check);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::InputFileNotAccessible(_))));
    }

    #[test]
    fn pre_check_valid_input_file() {
        // Create a temporary valid input file
        let input = std::env::temp_dir().join("oximedia_precheck_valid.webm");
        std::fs::write(&input, &[0xAA; 1024]).expect("write temp file");
        let output = std::env::temp_dir().join("oximedia_precheck_out.webm");

        let check = PreConversionCheck::new(
            &input.display().to_string(),
            &output.display().to_string(),
            "webm",
        );
        let errors = validate_pre_conversion(&check);

        // Should not have input accessibility errors
        assert!(!errors
            .iter()
            .any(|e| matches!(e, ValidationError::InputFileNotAccessible(_))));
        assert!(!errors
            .iter()
            .any(|e| matches!(e, ValidationError::EmptyInputFile(_))));

        let _ = std::fs::remove_file(&input);
    }

    #[test]
    fn pre_check_empty_input_file() {
        let input = std::env::temp_dir().join("oximedia_precheck_empty.webm");
        std::fs::write(&input, &[]).expect("write temp file");
        let output = std::env::temp_dir().join("oximedia_precheck_empty_out.webm");

        let check = PreConversionCheck::new(
            &input.display().to_string(),
            &output.display().to_string(),
            "webm",
        );
        let errors = validate_pre_conversion(&check);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::EmptyInputFile(_))));

        let _ = std::fs::remove_file(&input);
    }

    #[test]
    fn pre_check_video_codec_container_compat() {
        let input = std::env::temp_dir().join("oximedia_precheck_compat.webm");
        std::fs::write(&input, &[0xBB; 512]).expect("write temp file");
        let output = std::env::temp_dir().join("oximedia_precheck_compat_out.mp4");

        // Theora is not compatible with MP4
        let check = PreConversionCheck::new(
            &input.display().to_string(),
            &output.display().to_string(),
            "mp4",
        )
        .with_video_codec("theora");

        let errors = validate_pre_conversion(&check);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::VideoCodecContainerMismatch { .. })));

        let _ = std::fs::remove_file(&input);
    }

    #[test]
    fn pre_check_audio_codec_container_compat() {
        let input = std::env::temp_dir().join("oximedia_precheck_audio.webm");
        std::fs::write(&input, &[0xCC; 512]).expect("write temp file");
        let output = std::env::temp_dir().join("oximedia_precheck_audio_out.webm");

        // PCM is not compatible with WebM
        let check = PreConversionCheck::new(
            &input.display().to_string(),
            &output.display().to_string(),
            "webm",
        )
        .with_audio_codec("pcm");

        let errors = validate_pre_conversion(&check);
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::AudioCodecContainerMismatch { .. })));

        let _ = std::fs::remove_file(&input);
    }

    #[test]
    fn pre_check_compatible_codecs_no_errors() {
        let input = std::env::temp_dir().join("oximedia_precheck_ok.webm");
        std::fs::write(&input, &[0xDD; 1024]).expect("write temp file");
        let output = std::env::temp_dir().join("oximedia_precheck_ok_out.webm");

        let check = PreConversionCheck::new(
            &input.display().to_string(),
            &output.display().to_string(),
            "webm",
        )
        .with_video_codec("vp9")
        .with_audio_codec("opus");

        let errors = validate_pre_conversion(&check);
        // Should have no codec mismatch errors
        assert!(!errors
            .iter()
            .any(|e| matches!(e, ValidationError::VideoCodecContainerMismatch { .. })));
        assert!(!errors
            .iter()
            .any(|e| matches!(e, ValidationError::AudioCodecContainerMismatch { .. })));

        let _ = std::fs::remove_file(&input);
    }

    #[test]
    fn pre_check_size_multiplier() {
        let check = PreConversionCheck::new("/tmp/test.webm", "/tmp/out.webm", "webm")
            .with_size_multiplier(2.0);
        assert!((check.size_multiplier - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn pre_check_safety_margin() {
        let check = PreConversionCheck::new("/tmp/test.webm", "/tmp/out.webm", "webm")
            .with_safety_margin(50 * 1024 * 1024);
        assert_eq!(check.safety_margin_bytes, 50 * 1024 * 1024);
    }

    #[test]
    fn pre_check_builder_chain() {
        let check = PreConversionCheck::new("/tmp/in.webm", "/tmp/out.mp4", "mp4")
            .with_video_codec("av1")
            .with_audio_codec("opus")
            .with_size_multiplier(0.8)
            .with_safety_margin(200 * 1024 * 1024);

        assert_eq!(check.video_codec, Some("av1".to_string()));
        assert_eq!(check.audio_codec, Some("opus".to_string()));
        assert!((check.size_multiplier - 0.8).abs() < f64::EPSILON);
        assert_eq!(check.safety_margin_bytes, 200 * 1024 * 1024);
    }

    #[test]
    fn test_disk_space_function() {
        // Test with existing path (should return > 0 on unix)
        let available = get_available_disk_space("/tmp");
        #[cfg(unix)]
        assert!(available > 0);
        let _ = available;
    }

    #[test]
    fn test_video_codec_compat_checks() {
        // VP9 in WebM: compatible
        assert!(check_video_codec_container_compat("vp9", "webm").is_none());
        // AV1 in MP4: compatible
        assert!(check_video_codec_container_compat("av1", "mp4").is_none());
        // Theora in MP4: incompatible
        assert!(check_video_codec_container_compat("theora", "mp4").is_some());
        // VP8 in MP4: incompatible
        assert!(check_video_codec_container_compat("vp8", "mp4").is_some());
        // VP8 in WebM: compatible
        assert!(check_video_codec_container_compat("vp8", "webm").is_none());
        // Theora in OGG: compatible
        assert!(check_video_codec_container_compat("theora", "ogg").is_none());
        // Unknown container allows anything
        assert!(check_video_codec_container_compat("vp9", "custom").is_none());
    }

    #[test]
    fn test_audio_codec_compat_checks() {
        // Opus in WebM: compatible
        assert!(check_audio_codec_container_compat("opus", "webm").is_none());
        // Vorbis in WebM: compatible
        assert!(check_audio_codec_container_compat("vorbis", "webm").is_none());
        // PCM in WebM: incompatible
        assert!(check_audio_codec_container_compat("pcm", "webm").is_some());
        // PCM in WAV: compatible
        assert!(check_audio_codec_container_compat("pcm", "wav").is_none());
        // FLAC in FLAC: compatible
        assert!(check_audio_codec_container_compat("flac", "flac").is_none());
        // Opus in MP4: compatible
        assert!(check_audio_codec_container_compat("opus", "mp4").is_none());
        // Vorbis in MP4: incompatible
        assert!(check_audio_codec_container_compat("vorbis", "mp4").is_some());
    }
}
