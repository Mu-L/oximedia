#![allow(dead_code)]
//! Output validation for rendered frames in the `OxiMedia` render farm.
//!
//! Provides individual output checks, per-frame validation results, a
//! validator that inspects frame ranges, and a consolidated report.

/// A single type of output check that can be applied to a rendered frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputCheck {
    /// Frame file exists on disk.
    FileExists,
    /// File size is above the minimum threshold (non-empty render).
    FileSizeAboveMinimum,
    /// Image dimensions match the expected width/height.
    DimensionsMatch,
    /// Pixel format matches the job specification.
    PixelFormatMatch,
    /// Frame number encoded in the filename matches expected sequence.
    FrameNumberSequential,
    /// Checksum / hash matches a reference (for re-renders).
    ChecksumValid,
    /// No corruption artefacts detected (e.g. all-black, all-white frames).
    NoCorruption,
}

impl OutputCheck {
    /// Short machine-readable name for this check.
    #[must_use]
    pub fn check_name(&self) -> &'static str {
        match self {
            OutputCheck::FileExists => "file_exists",
            OutputCheck::FileSizeAboveMinimum => "file_size_above_minimum",
            OutputCheck::DimensionsMatch => "dimensions_match",
            OutputCheck::PixelFormatMatch => "pixel_format_match",
            OutputCheck::FrameNumberSequential => "frame_number_sequential",
            OutputCheck::ChecksumValid => "checksum_valid",
            OutputCheck::NoCorruption => "no_corruption",
        }
    }

    /// Returns `true` for checks that are considered critical (failure blocks delivery).
    #[must_use]
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            OutputCheck::FileExists | OutputCheck::DimensionsMatch | OutputCheck::NoCorruption
        )
    }
}

/// Result of running all checks on a single frame.
#[derive(Debug, Clone)]
pub struct OutputValidation {
    /// Frame number that was validated.
    pub frame_number: u32,
    /// Map of check → pass/fail.
    pub results: Vec<(OutputCheck, bool)>,
}

impl OutputValidation {
    /// Create a new validation record for `frame_number` with no checks yet.
    #[must_use]
    pub fn new(frame_number: u32) -> Self {
        Self {
            frame_number,
            results: Vec::new(),
        }
    }

    /// Record the outcome of a single check.
    pub fn record(&mut self, check: OutputCheck, passed: bool) {
        self.results.push((check, passed));
    }

    /// Returns `true` when every recorded check passed.
    #[must_use]
    pub fn passes_all(&self) -> bool {
        self.results.iter().all(|(_, passed)| *passed)
    }

    /// Returns `true` when every *critical* check passed.
    #[must_use]
    pub fn passes_critical(&self) -> bool {
        self.results
            .iter()
            .filter(|(c, _)| c.is_critical())
            .all(|(_, passed)| *passed)
    }

    /// Returns a list of checks that failed.
    #[must_use]
    pub fn failed_checks(&self) -> Vec<OutputCheck> {
        self.results
            .iter()
            .filter_map(|(c, passed)| if *passed { None } else { Some(*c) })
            .collect()
    }

    /// Number of checks recorded.
    #[must_use]
    pub fn check_count(&self) -> usize {
        self.results.len()
    }
}

/// Validator that runs a configurable set of checks over a frame range.
#[derive(Debug, Clone)]
pub struct OutputValidator {
    /// Checks that will be applied to every frame.
    pub enabled_checks: Vec<OutputCheck>,
    /// Expected frame width in pixels.
    pub expected_width: u32,
    /// Expected frame height in pixels.
    pub expected_height: u32,
    /// Minimum file size in bytes for the `FileSizeAboveMinimum` check.
    pub min_file_size_bytes: u64,
}

impl Default for OutputValidator {
    fn default() -> Self {
        Self {
            enabled_checks: vec![
                OutputCheck::FileExists,
                OutputCheck::FileSizeAboveMinimum,
                OutputCheck::DimensionsMatch,
                OutputCheck::NoCorruption,
            ],
            expected_width: 1920,
            expected_height: 1080,
            min_file_size_bytes: 1024,
        }
    }
}

impl OutputValidator {
    /// Create a validator with default checks for 1920×1080.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a validator for a specific resolution.
    #[must_use]
    pub fn for_resolution(width: u32, height: u32) -> Self {
        Self {
            expected_width: width,
            expected_height: height,
            ..Default::default()
        }
    }

    /// Validate a single frame given simulated frame metadata.
    ///
    /// `file_size_bytes` of 0 simulates a missing/empty file.
    /// `actual_width` / `actual_height` of 0 simulate dimension errors.
    #[must_use]
    pub fn validate_frame(
        &self,
        frame_number: u32,
        file_size_bytes: u64,
        actual_width: u32,
        actual_height: u32,
        corrupted: bool,
    ) -> OutputValidation {
        let mut v = OutputValidation::new(frame_number);
        for check in &self.enabled_checks {
            let passed = match check {
                OutputCheck::FileExists => file_size_bytes > 0,
                OutputCheck::FileSizeAboveMinimum => file_size_bytes >= self.min_file_size_bytes,
                OutputCheck::DimensionsMatch => {
                    actual_width == self.expected_width && actual_height == self.expected_height
                }
                OutputCheck::PixelFormatMatch => true, // placeholder: assume OK
                OutputCheck::FrameNumberSequential => true, // placeholder
                OutputCheck::ChecksumValid => true,    // placeholder
                OutputCheck::NoCorruption => !corrupted,
            };
            v.record(*check, passed);
        }
        v
    }

    /// Validate a contiguous range of frames.
    ///
    /// For simplicity the simulation assumes all frames are valid unless
    /// `bad_frames` contains the frame number, in which case every check fails.
    #[must_use]
    pub fn validate_frame_range(
        &self,
        frame_start: u32,
        frame_end: u32,
        bad_frames: &[u32],
    ) -> OutputValidationReport {
        let mut validations = Vec::new();
        for f in frame_start..=frame_end {
            let is_bad = bad_frames.contains(&f);
            let v = self.validate_frame(
                f,
                if is_bad { 0 } else { 65_536 },
                if is_bad { 0 } else { self.expected_width },
                if is_bad { 0 } else { self.expected_height },
                is_bad,
            );
            validations.push(v);
        }
        OutputValidationReport { validations }
    }
}

/// Consolidated validation report covering multiple frames.
#[derive(Debug, Clone)]
pub struct OutputValidationReport {
    validations: Vec<OutputValidation>,
}

impl OutputValidationReport {
    /// Create a report from a pre-built list of frame validations.
    #[must_use]
    pub fn from_validations(validations: Vec<OutputValidation>) -> Self {
        Self { validations }
    }

    /// Returns `true` when every frame in the report passes all checks.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.validations.iter().all(OutputValidation::passes_all)
    }

    /// Return frame validations that have at least one failure.
    #[must_use]
    pub fn issues(&self) -> Vec<&OutputValidation> {
        self.validations
            .iter()
            .filter(|v| !v.passes_all())
            .collect()
    }

    /// Number of frames that passed all checks.
    #[must_use]
    pub fn passed_count(&self) -> usize {
        self.validations.iter().filter(|v| v.passes_all()).count()
    }

    /// Number of frames that have at least one failing check.
    #[must_use]
    pub fn failed_count(&self) -> usize {
        self.validations.iter().filter(|v| !v.passes_all()).count()
    }

    /// Total number of frames in the report.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.validations.len()
    }

    /// Return the frame numbers that failed.
    #[must_use]
    pub fn failed_frame_numbers(&self) -> Vec<u32> {
        self.validations
            .iter()
            .filter(|v| !v.passes_all())
            .map(|v| v.frame_number)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_name_file_exists() {
        assert_eq!(OutputCheck::FileExists.check_name(), "file_exists");
    }

    #[test]
    fn test_check_name_no_corruption() {
        assert_eq!(OutputCheck::NoCorruption.check_name(), "no_corruption");
    }

    #[test]
    fn test_check_is_critical_file_exists() {
        assert!(OutputCheck::FileExists.is_critical());
    }

    #[test]
    fn test_check_not_critical_checksum() {
        assert!(!OutputCheck::ChecksumValid.is_critical());
    }

    #[test]
    fn test_check_dimensions_is_critical() {
        assert!(OutputCheck::DimensionsMatch.is_critical());
    }

    #[test]
    fn test_output_validation_passes_all_empty() {
        let v = OutputValidation::new(1);
        assert!(v.passes_all()); // no checks recorded = nothing failing
    }

    #[test]
    fn test_output_validation_passes_all_with_passing_checks() {
        let mut v = OutputValidation::new(1);
        v.record(OutputCheck::FileExists, true);
        v.record(OutputCheck::DimensionsMatch, true);
        assert!(v.passes_all());
    }

    #[test]
    fn test_output_validation_fails_when_one_check_fails() {
        let mut v = OutputValidation::new(1);
        v.record(OutputCheck::FileExists, true);
        v.record(OutputCheck::DimensionsMatch, false);
        assert!(!v.passes_all());
    }

    #[test]
    fn test_output_validation_failed_checks_list() {
        let mut v = OutputValidation::new(5);
        v.record(OutputCheck::FileExists, false);
        v.record(OutputCheck::NoCorruption, true);
        let failed = v.failed_checks();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0], OutputCheck::FileExists);
    }

    #[test]
    fn test_output_validation_check_count() {
        let mut v = OutputValidation::new(10);
        v.record(OutputCheck::FileExists, true);
        v.record(OutputCheck::FileSizeAboveMinimum, true);
        assert_eq!(v.check_count(), 2);
    }

    #[test]
    fn test_validator_clean_frame() {
        let val = OutputValidator::new();
        let v = val.validate_frame(1, 65_536, 1920, 1080, false);
        assert!(v.passes_all());
    }

    #[test]
    fn test_validator_bad_frame_fails() {
        let val = OutputValidator::new();
        let v = val.validate_frame(1, 0, 0, 0, true);
        assert!(!v.passes_all());
    }

    #[test]
    fn test_validator_frame_range_all_clean() {
        let val = OutputValidator::new();
        let report = val.validate_frame_range(1, 10, &[]);
        assert!(report.all_passed());
        assert_eq!(report.frame_count(), 10);
    }

    #[test]
    fn test_validator_frame_range_with_bad_frames() {
        let val = OutputValidator::new();
        let report = val.validate_frame_range(1, 10, &[3, 7]);
        assert!(!report.all_passed());
        assert_eq!(report.failed_count(), 2);
    }

    #[test]
    fn test_report_issues_returns_failing_frames() {
        let val = OutputValidator::new();
        let report = val.validate_frame_range(1, 5, &[2]);
        let issues = report.issues();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].frame_number, 2);
    }

    #[test]
    fn test_report_failed_frame_numbers() {
        let val = OutputValidator::new();
        let report = val.validate_frame_range(1, 5, &[4, 5]);
        let mut bad = report.failed_frame_numbers();
        bad.sort_unstable();
        assert_eq!(bad, vec![4, 5]);
    }

    #[test]
    fn test_report_passed_count() {
        let val = OutputValidator::new();
        let report = val.validate_frame_range(1, 10, &[1, 2]);
        assert_eq!(report.passed_count(), 8);
    }
}
