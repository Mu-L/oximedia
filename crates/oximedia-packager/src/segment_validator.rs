#![allow(dead_code)]
//! Segment validation and integrity checking for adaptive streaming.
//!
//! This module provides tools for validating segment files, checking
//! segment consistency within a manifest, verifying duration tolerances,
//! and detecting common packaging errors.

use std::time::Duration;

/// Result of validating a single segment.
#[derive(Debug, Clone)]
pub struct SegmentValidationResult {
    /// Segment index.
    pub index: u64,
    /// Whether the segment is valid.
    pub is_valid: bool,
    /// Validation issues found.
    pub issues: Vec<ValidationIssue>,
}

impl SegmentValidationResult {
    /// Create a passing validation result.
    #[must_use]
    pub fn pass(index: u64) -> Self {
        Self {
            index,
            is_valid: true,
            issues: Vec::new(),
        }
    }

    /// Create a validation result with issues.
    #[must_use]
    pub fn with_issues(index: u64, issues: Vec<ValidationIssue>) -> Self {
        let is_valid = issues.iter().all(|i| i.severity != IssueSeverity::Error);
        Self {
            index,
            is_valid,
            issues,
        }
    }

    /// Check if there are any warnings.
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Warning)
    }

    /// Check if there are any errors.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Error)
    }
}

/// Severity level for a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
    /// Informational note.
    Info,
    /// Warning that may cause playback issues.
    Warning,
    /// Error that will cause playback failure.
    Error,
}

/// A specific validation issue found in a segment.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Severity level.
    pub severity: IssueSeverity,
    /// Issue code for programmatic handling.
    pub code: ValidationCode,
    /// Human-readable description of the issue.
    pub message: String,
}

impl ValidationIssue {
    /// Create a new validation issue.
    #[must_use]
    pub fn new(severity: IssueSeverity, code: ValidationCode, message: impl Into<String>) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
        }
    }

    /// Create an error issue.
    #[must_use]
    pub fn error(code: ValidationCode, message: impl Into<String>) -> Self {
        Self::new(IssueSeverity::Error, code, message)
    }

    /// Create a warning issue.
    #[must_use]
    pub fn warning(code: ValidationCode, message: impl Into<String>) -> Self {
        Self::new(IssueSeverity::Warning, code, message)
    }

    /// Create an info issue.
    #[must_use]
    pub fn info(code: ValidationCode, message: impl Into<String>) -> Self {
        Self::new(IssueSeverity::Info, code, message)
    }
}

/// Validation issue codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationCode {
    /// Segment size is zero.
    EmptySegment,
    /// Segment duration exceeds tolerance.
    DurationOutOfRange,
    /// Segment does not start with a keyframe.
    MissingKeyframe,
    /// Segment index gap detected.
    IndexGap,
    /// Segment index is duplicated.
    DuplicateIndex,
    /// Segment size is abnormally large.
    OversizedSegment,
    /// Segment size is abnormally small.
    UndersizedSegment,
    /// Duration drift accumulated beyond threshold.
    DurationDrift,
    /// Timestamp discontinuity detected.
    TimestampDiscontinuity,
}

/// Metadata about a segment for validation purposes.
#[derive(Debug, Clone)]
pub struct SegmentMetadata {
    /// Segment index.
    pub index: u64,
    /// Segment duration.
    pub duration: Duration,
    /// Segment size in bytes.
    pub size_bytes: u64,
    /// Whether the segment starts with a keyframe.
    pub starts_with_keyframe: bool,
    /// Presentation timestamp of the segment start.
    pub pts_start_ms: u64,
}

impl SegmentMetadata {
    /// Create new segment metadata.
    #[must_use]
    pub fn new(index: u64, duration: Duration, size_bytes: u64) -> Self {
        Self {
            index,
            duration,
            size_bytes,
            starts_with_keyframe: true,
            pts_start_ms: 0,
        }
    }

    /// Set whether the segment starts with a keyframe.
    #[must_use]
    pub fn with_keyframe(mut self, starts_with_keyframe: bool) -> Self {
        self.starts_with_keyframe = starts_with_keyframe;
        self
    }

    /// Set the presentation timestamp.
    #[must_use]
    pub fn with_pts(mut self, pts_start_ms: u64) -> Self {
        self.pts_start_ms = pts_start_ms;
        self
    }
}

/// Configuration for segment validation.
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Target segment duration.
    pub target_duration: Duration,
    /// Allowed duration tolerance as a fraction (e.g., 0.1 for 10%).
    pub duration_tolerance: f64,
    /// Maximum allowed segment size in bytes.
    pub max_segment_size: u64,
    /// Minimum allowed segment size in bytes.
    pub min_segment_size: u64,
    /// Maximum accumulated duration drift in milliseconds.
    pub max_drift_ms: u64,
    /// Whether keyframe-at-start is required.
    pub require_keyframe_start: bool,
}

impl ValidationConfig {
    /// Create a default validation config with a target duration.
    #[must_use]
    pub fn new(target_duration: Duration) -> Self {
        Self {
            target_duration,
            duration_tolerance: 0.1,
            max_segment_size: 50 * 1024 * 1024, // 50 MB
            min_segment_size: 100,
            max_drift_ms: 500,
            require_keyframe_start: true,
        }
    }

    /// Set the duration tolerance.
    #[must_use]
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.duration_tolerance = tolerance.clamp(0.0, 1.0);
        self
    }

    /// Set whether keyframe-at-start is required.
    #[must_use]
    pub fn with_keyframe_required(mut self, required: bool) -> Self {
        self.require_keyframe_start = required;
        self
    }
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self::new(Duration::from_secs(6))
    }
}

/// Segment validator that checks a sequence of segments.
pub struct SegmentValidator {
    /// Validation configuration.
    config: ValidationConfig,
}

impl SegmentValidator {
    /// Create a new segment validator.
    #[must_use]
    pub fn new(config: ValidationConfig) -> Self {
        Self { config }
    }

    /// Validate a single segment.
    #[must_use]
    pub fn validate_segment(&self, metadata: &SegmentMetadata) -> SegmentValidationResult {
        let mut issues = Vec::new();

        // Check empty segment
        if metadata.size_bytes == 0 {
            issues.push(ValidationIssue::error(
                ValidationCode::EmptySegment,
                format!("Segment {} has zero bytes", metadata.index),
            ));
        }

        // Check size bounds
        if metadata.size_bytes > self.config.max_segment_size {
            issues.push(ValidationIssue::warning(
                ValidationCode::OversizedSegment,
                format!(
                    "Segment {} is {} bytes, exceeds max {}",
                    metadata.index, metadata.size_bytes, self.config.max_segment_size
                ),
            ));
        }

        if metadata.size_bytes > 0 && metadata.size_bytes < self.config.min_segment_size {
            issues.push(ValidationIssue::warning(
                ValidationCode::UndersizedSegment,
                format!(
                    "Segment {} is {} bytes, below min {}",
                    metadata.index, metadata.size_bytes, self.config.min_segment_size
                ),
            ));
        }

        // Check duration tolerance
        self.check_duration(metadata, &mut issues);

        // Check keyframe
        if self.config.require_keyframe_start && !metadata.starts_with_keyframe {
            issues.push(ValidationIssue::error(
                ValidationCode::MissingKeyframe,
                format!("Segment {} does not start with a keyframe", metadata.index),
            ));
        }

        SegmentValidationResult::with_issues(metadata.index, issues)
    }

    /// Check duration is within tolerance.
    #[allow(clippy::cast_precision_loss)]
    fn check_duration(&self, metadata: &SegmentMetadata, issues: &mut Vec<ValidationIssue>) {
        let target_ms = self.config.target_duration.as_millis() as f64;
        let actual_ms = metadata.duration.as_millis() as f64;

        if target_ms > 0.0 {
            let deviation = (actual_ms - target_ms).abs() / target_ms;
            if deviation > self.config.duration_tolerance {
                issues.push(ValidationIssue::warning(
                    ValidationCode::DurationOutOfRange,
                    format!(
                        "Segment {} duration {:.0}ms deviates {:.1}% from target {:.0}ms",
                        metadata.index,
                        actual_ms,
                        deviation * 100.0,
                        target_ms
                    ),
                ));
            }
        }
    }

    /// Validate a sequence of segments for consistency.
    #[must_use]
    pub fn validate_sequence(&self, segments: &[SegmentMetadata]) -> Vec<SegmentValidationResult> {
        let mut results = Vec::with_capacity(segments.len());

        for segment in segments {
            results.push(self.validate_segment(segment));
        }

        // Check for index gaps and duplicates
        self.check_index_continuity(segments, &mut results);

        results
    }

    /// Check index continuity across segments.
    fn check_index_continuity(
        &self,
        segments: &[SegmentMetadata],
        results: &mut [SegmentValidationResult],
    ) {
        if segments.len() < 2 {
            return;
        }

        for i in 1..segments.len() {
            let expected = segments[i - 1].index + 1;
            let actual = segments[i].index;

            if actual != expected {
                results[i].issues.push(ValidationIssue::warning(
                    ValidationCode::IndexGap,
                    format!(
                        "Index gap: expected {} after {}, got {}",
                        expected,
                        segments[i - 1].index,
                        actual
                    ),
                ));
            }
        }
    }

    /// Compute total validated duration.
    #[must_use]
    pub fn total_duration(&self, segments: &[SegmentMetadata]) -> Duration {
        segments.iter().map(|s| s.duration).sum()
    }
}

impl Default for SegmentValidator {
    fn default() -> Self {
        Self::new(ValidationConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(index: u64, duration_ms: u64, size: u64) -> SegmentMetadata {
        SegmentMetadata::new(index, Duration::from_millis(duration_ms), size)
    }

    #[test]
    fn test_valid_segment() {
        let validator = SegmentValidator::new(ValidationConfig::new(Duration::from_secs(6)));
        let result = validator.validate_segment(&seg(0, 6000, 500_000));
        assert!(result.is_valid);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_empty_segment() {
        let validator = SegmentValidator::default();
        let result = validator.validate_segment(&seg(0, 6000, 0));
        assert!(!result.is_valid);
        assert!(result.has_errors());
    }

    #[test]
    fn test_oversized_segment() {
        let validator = SegmentValidator::default();
        let result = validator.validate_segment(&seg(0, 6000, 100 * 1024 * 1024));
        assert!(result.has_warnings());
    }

    #[test]
    fn test_undersized_segment() {
        let validator = SegmentValidator::default();
        let result = validator.validate_segment(&seg(0, 6000, 50));
        assert!(result.has_warnings());
    }

    #[test]
    fn test_duration_out_of_range() {
        let validator = SegmentValidator::new(ValidationConfig::new(Duration::from_secs(6)));
        let result = validator.validate_segment(&seg(0, 10_000, 500_000));
        assert!(result.has_warnings());
        let has_dur_issue = result
            .issues
            .iter()
            .any(|i| i.code == ValidationCode::DurationOutOfRange);
        assert!(has_dur_issue);
    }

    #[test]
    fn test_missing_keyframe() {
        let validator = SegmentValidator::default();
        let meta = SegmentMetadata::new(0, Duration::from_secs(6), 500_000).with_keyframe(false);
        let result = validator.validate_segment(&meta);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_validation_pass_result() {
        let result = SegmentValidationResult::pass(42);
        assert!(result.is_valid);
        assert!(!result.has_warnings());
        assert!(!result.has_errors());
    }

    #[test]
    fn test_validation_issue_constructors() {
        let err = ValidationIssue::error(ValidationCode::EmptySegment, "empty");
        assert_eq!(err.severity, IssueSeverity::Error);

        let warn = ValidationIssue::warning(ValidationCode::OversizedSegment, "big");
        assert_eq!(warn.severity, IssueSeverity::Warning);

        let info = ValidationIssue::info(ValidationCode::DurationDrift, "drift");
        assert_eq!(info.severity, IssueSeverity::Info);
    }

    #[test]
    fn test_validate_sequence() {
        let validator = SegmentValidator::new(ValidationConfig::new(Duration::from_secs(6)));
        let segments = vec![
            seg(0, 6000, 500_000),
            seg(1, 6000, 500_000),
            seg(2, 6000, 500_000),
        ];
        let results = validator.validate_sequence(&segments);
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_valid));
    }

    #[test]
    fn test_validate_sequence_index_gap() {
        let validator = SegmentValidator::default();
        let segments = vec![seg(0, 6000, 500_000), seg(2, 6000, 500_000)];
        let results = validator.validate_sequence(&segments);
        let gap_issues: Vec<_> = results[1]
            .issues
            .iter()
            .filter(|i| i.code == ValidationCode::IndexGap)
            .collect();
        assert_eq!(gap_issues.len(), 1);
    }

    #[test]
    fn test_total_duration() {
        let validator = SegmentValidator::default();
        let segments = vec![seg(0, 6000, 500_000), seg(1, 6000, 500_000)];
        let total = validator.total_duration(&segments);
        assert_eq!(total, Duration::from_secs(12));
    }

    #[test]
    fn test_validation_config_builder() {
        let config = ValidationConfig::new(Duration::from_secs(4))
            .with_tolerance(0.2)
            .with_keyframe_required(false);
        assert_eq!(config.target_duration, Duration::from_secs(4));
        assert!((config.duration_tolerance - 0.2).abs() < f64::EPSILON);
        assert!(!config.require_keyframe_start);
    }

    #[test]
    fn test_segment_metadata_builder() {
        let meta = SegmentMetadata::new(5, Duration::from_secs(6), 1000)
            .with_keyframe(false)
            .with_pts(5000);
        assert_eq!(meta.index, 5);
        assert!(!meta.starts_with_keyframe);
        assert_eq!(meta.pts_start_ms, 5000);
    }

    #[test]
    fn test_default_validation_config() {
        let config = ValidationConfig::default();
        assert_eq!(config.target_duration, Duration::from_secs(6));
        assert!((config.duration_tolerance - 0.1).abs() < f64::EPSILON);
        assert!(config.require_keyframe_start);
    }
}
