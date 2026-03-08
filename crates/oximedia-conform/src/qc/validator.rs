//! Validation for conform sessions and matches.

use crate::config::ConformConfig;
use crate::error::ConformResult;
use crate::types::{ClipMatch, ClipReference};
use serde::{Deserialize, Serialize};

/// Validation report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Validation errors.
    pub errors: Vec<ValidationError>,
    /// Validation warnings.
    pub warnings: Vec<ValidationWarning>,
    /// Is validation successful.
    pub is_valid: bool,
}

/// Validation error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Clip ID.
    pub clip_id: String,
    /// Error message.
    pub message: String,
    /// Error severity.
    pub severity: ErrorSeverity,
}

/// Validation warning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// Clip ID.
    pub clip_id: String,
    /// Warning message.
    pub message: String,
}

/// Error severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Critical error.
    Critical,
    /// Error.
    Error,
    /// Warning.
    Warning,
}

/// Validator for conform sessions.
pub struct Validator {
    config: ConformConfig,
}

impl Validator {
    /// Create a new validator.
    #[must_use]
    pub fn new(config: ConformConfig) -> Self {
        Self { config }
    }

    /// Validate a match.
    #[must_use]
    pub fn validate_match(&self, clip_match: &ClipMatch) -> ValidationReport {
        let mut errors = Vec::new();
        let warnings = Vec::new();

        // Check match score
        if clip_match.score < self.config.match_threshold {
            errors.push(ValidationError {
                clip_id: clip_match.clip.id.clone(),
                message: format!(
                    "Match score {:.2} below threshold {:.2}",
                    clip_match.score, self.config.match_threshold
                ),
                severity: ErrorSeverity::Error,
            });
        }

        // Check handles
        if !self.config.allow_missing_handles {
            if let Err(e) = self.check_handles(&clip_match.clip, &clip_match.media) {
                errors.push(ValidationError {
                    clip_id: clip_match.clip.id.clone(),
                    message: e.to_string(),
                    severity: ErrorSeverity::Warning,
                });
            }
        }

        // Check file existence
        if !clip_match.media.path.exists() {
            errors.push(ValidationError {
                clip_id: clip_match.clip.id.clone(),
                message: format!("Media file not found: {}", clip_match.media.path.display()),
                severity: ErrorSeverity::Critical,
            });
        }

        ValidationReport {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    /// Check if source has sufficient handles.
    fn check_handles(
        &self,
        _clip: &ClipReference,
        _media: &crate::types::MediaFile,
    ) -> ConformResult<()> {
        // Placeholder: would check if media has sufficient pre/post roll
        Ok(())
    }

    /// Validate all matches.
    #[must_use]
    pub fn validate_all(&self, matches: &[ClipMatch]) -> ValidationReport {
        let mut all_errors = Vec::new();
        let mut all_warnings = Vec::new();

        for clip_match in matches {
            let report = self.validate_match(clip_match);
            all_errors.extend(report.errors);
            all_warnings.extend(report.warnings);
        }

        ValidationReport {
            is_valid: all_errors.is_empty(),
            errors: all_errors,
            warnings: all_warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FrameRate, MatchMethod, MediaFile, Timecode, TrackType};
    use std::path::PathBuf;

    fn create_test_clip() -> ClipReference {
        ClipReference {
            id: "test".to_string(),
            source_file: Some("test.mov".to_string()),
            source_in: Timecode::new(1, 0, 0, 0),
            source_out: Timecode::new(1, 0, 10, 0),
            record_in: Timecode::new(1, 0, 0, 0),
            record_out: Timecode::new(1, 0, 10, 0),
            track: TrackType::Video,
            fps: FrameRate::Fps25,
            metadata: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_validator_creation() {
        let config = ConformConfig::default();
        let _validator = Validator::new(config);
    }

    #[test]
    fn test_validate_match_score() {
        let config = ConformConfig::default();
        let validator = Validator::new(config);

        let clip = create_test_clip();
        let media = MediaFile::new(PathBuf::from("/nonexistent/test.mov"));

        let clip_match = ClipMatch {
            clip,
            media,
            score: 0.5,
            method: MatchMethod::ExactFilename,
            details: String::new(),
        };

        let report = validator.validate_match(&clip_match);
        assert!(!report.is_valid);
        assert!(!report.errors.is_empty());
    }
}
