#![allow(dead_code)]
//! LUT validation and integrity checking.
//!
//! Provides tools to validate 1D and 3D LUT data for correctness,
//! range compliance, monotonicity, and structural integrity before
//! applying them in production color pipelines.

use std::fmt;

/// Result of a single validation check.
#[derive(Clone, Debug, PartialEq)]
pub enum ValidationSeverity {
    /// Informational note that does not affect correctness.
    Info,
    /// Warning about potential quality issues.
    Warning,
    /// Error that renders the LUT unusable.
    Error,
}

impl fmt::Display for ValidationSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARNING"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

/// A single validation diagnostic message.
#[derive(Clone, Debug)]
pub struct ValidationDiagnostic {
    /// Severity of this diagnostic.
    pub severity: ValidationSeverity,
    /// Human-readable message describing the issue.
    pub message: String,
    /// Optional index or coordinate where the issue was found.
    pub location: Option<String>,
}

impl ValidationDiagnostic {
    /// Create a new diagnostic.
    pub fn new(severity: ValidationSeverity, message: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            location: None,
        }
    }

    /// Create a diagnostic with a location hint.
    pub fn with_location(
        severity: ValidationSeverity,
        message: impl Into<String>,
        location: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            message: message.into(),
            location: Some(location.into()),
        }
    }

    /// Returns true if this diagnostic is an error.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity == ValidationSeverity::Error
    }

    /// Returns true if this diagnostic is a warning.
    #[must_use]
    pub fn is_warning(&self) -> bool {
        self.severity == ValidationSeverity::Warning
    }
}

/// Result of a full LUT validation pass.
#[derive(Clone, Debug)]
pub struct ValidationReport {
    /// All diagnostics collected during validation.
    pub diagnostics: Vec<ValidationDiagnostic>,
    /// Whether the LUT passes all critical checks.
    pub passed: bool,
    /// Name or identifier of the LUT that was validated.
    pub lut_name: String,
}

impl ValidationReport {
    /// Create a new empty validation report.
    pub fn new(lut_name: impl Into<String>) -> Self {
        Self {
            diagnostics: Vec::new(),
            passed: true,
            lut_name: lut_name.into(),
        }
    }

    /// Add a diagnostic to the report.
    pub fn add(&mut self, diagnostic: ValidationDiagnostic) {
        if diagnostic.is_error() {
            self.passed = false;
        }
        self.diagnostics.push(diagnostic);
    }

    /// Count diagnostics of a given severity.
    #[must_use]
    pub fn count_severity(&self, severity: &ValidationSeverity) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| &d.severity == severity)
            .count()
    }

    /// Return all error diagnostics.
    #[must_use]
    pub fn errors(&self) -> Vec<&ValidationDiagnostic> {
        self.diagnostics.iter().filter(|d| d.is_error()).collect()
    }

    /// Return all warning diagnostics.
    #[must_use]
    pub fn warnings(&self) -> Vec<&ValidationDiagnostic> {
        self.diagnostics.iter().filter(|d| d.is_warning()).collect()
    }

    /// Total number of diagnostics.
    #[must_use]
    pub fn total_diagnostics(&self) -> usize {
        self.diagnostics.len()
    }
}

/// Configuration for the LUT validator.
#[derive(Clone, Debug)]
pub struct ValidatorConfig {
    /// Minimum allowed value in LUT entries (default: 0.0).
    pub min_value: f64,
    /// Maximum allowed value in LUT entries (default: 1.0).
    pub max_value: f64,
    /// Tolerance for out-of-range checks.
    pub range_tolerance: f64,
    /// Whether to check for monotonicity in 1D LUTs.
    pub check_monotonicity: bool,
    /// Whether to check for NaN or Infinity values.
    pub check_nan_inf: bool,
    /// Maximum allowed deviation from identity LUT (for identity checks).
    pub identity_tolerance: f64,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            min_value: 0.0,
            max_value: 1.0,
            range_tolerance: 0.001,
            check_monotonicity: true,
            check_nan_inf: true,
            identity_tolerance: 0.0001,
        }
    }
}

/// Validates LUT data for correctness and quality.
#[derive(Clone, Debug)]
pub struct LutValidator {
    /// Configuration for validation rules.
    pub config: ValidatorConfig,
}

impl LutValidator {
    /// Create a new validator with default config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: ValidatorConfig::default(),
        }
    }

    /// Create a new validator with a custom config.
    #[must_use]
    pub fn with_config(config: ValidatorConfig) -> Self {
        Self { config }
    }

    /// Validate a 1D LUT channel (single channel array).
    #[must_use]
    pub fn validate_1d_channel(&self, data: &[f64], channel_name: &str) -> ValidationReport {
        let mut report = ValidationReport::new(format!("1D LUT channel: {channel_name}"));

        if data.is_empty() {
            report.add(ValidationDiagnostic::new(
                ValidationSeverity::Error,
                "LUT channel is empty",
            ));
            return report;
        }

        // Check for NaN / Infinity
        if self.config.check_nan_inf {
            for (i, &v) in data.iter().enumerate() {
                if v.is_nan() {
                    report.add(ValidationDiagnostic::with_location(
                        ValidationSeverity::Error,
                        "NaN value found",
                        format!("index {i}"),
                    ));
                } else if v.is_infinite() {
                    report.add(ValidationDiagnostic::with_location(
                        ValidationSeverity::Error,
                        "Infinite value found",
                        format!("index {i}"),
                    ));
                }
            }
        }

        // Range check
        for (i, &v) in data.iter().enumerate() {
            if v < self.config.min_value - self.config.range_tolerance {
                report.add(ValidationDiagnostic::with_location(
                    ValidationSeverity::Warning,
                    format!("Value {v:.6} below minimum {:.6}", self.config.min_value),
                    format!("index {i}"),
                ));
            }
            if v > self.config.max_value + self.config.range_tolerance {
                report.add(ValidationDiagnostic::with_location(
                    ValidationSeverity::Warning,
                    format!("Value {v:.6} above maximum {:.6}", self.config.max_value),
                    format!("index {i}"),
                ));
            }
        }

        // Monotonicity check
        if self.config.check_monotonicity && data.len() > 1 {
            let mut is_increasing = true;
            let mut is_decreasing = true;
            for w in data.windows(2) {
                if w[1] < w[0] {
                    is_increasing = false;
                }
                if w[1] > w[0] {
                    is_decreasing = false;
                }
            }
            if !is_increasing && !is_decreasing {
                report.add(ValidationDiagnostic::new(
                    ValidationSeverity::Info,
                    "Channel is neither monotonically increasing nor decreasing",
                ));
            }
        }

        report
    }

    /// Validate 3D LUT data given as a flat array of RGB triplets.
    ///
    /// `size` is the number of entries per dimension (e.g. 17 or 33).
    #[must_use]
    pub fn validate_3d_data(&self, data: &[f64], size: usize) -> ValidationReport {
        let mut report = ValidationReport::new(format!("3D LUT ({size}x{size}x{size})"));
        let expected_len = size * size * size * 3;

        if data.len() != expected_len {
            report.add(ValidationDiagnostic::new(
                ValidationSeverity::Error,
                format!(
                    "Expected {expected_len} values for {size}^3 * 3, got {}",
                    data.len()
                ),
            ));
            return report;
        }

        let mut nan_count = 0_usize;
        let mut inf_count = 0_usize;
        let mut below_count = 0_usize;
        let mut above_count = 0_usize;

        for &v in data {
            if v.is_nan() {
                nan_count += 1;
            } else if v.is_infinite() {
                inf_count += 1;
            } else {
                if v < self.config.min_value - self.config.range_tolerance {
                    below_count += 1;
                }
                if v > self.config.max_value + self.config.range_tolerance {
                    above_count += 1;
                }
            }
        }

        if nan_count > 0 {
            report.add(ValidationDiagnostic::new(
                ValidationSeverity::Error,
                format!("{nan_count} NaN values found in 3D LUT data"),
            ));
        }
        if inf_count > 0 {
            report.add(ValidationDiagnostic::new(
                ValidationSeverity::Error,
                format!("{inf_count} Infinite values found in 3D LUT data"),
            ));
        }
        if below_count > 0 {
            report.add(ValidationDiagnostic::new(
                ValidationSeverity::Warning,
                format!("{below_count} values below minimum range"),
            ));
        }
        if above_count > 0 {
            report.add(ValidationDiagnostic::new(
                ValidationSeverity::Warning,
                format!("{above_count} values above maximum range"),
            ));
        }

        report
    }

    /// Check if 3D LUT data is close to an identity transform.
    ///
    /// `size` is the number of entries per dimension.
    #[must_use]
    pub fn check_identity_3d(&self, data: &[f64], size: usize) -> bool {
        let expected_len = size * size * size * 3;
        if data.len() != expected_len {
            return false;
        }

        let step = if size > 1 {
            1.0 / (size as f64 - 1.0)
        } else {
            0.0
        };

        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    let idx = (b * size * size + g * size + r) * 3;
                    let expected_r = r as f64 * step;
                    let expected_g = g as f64 * step;
                    let expected_b = b as f64 * step;

                    if (data[idx] - expected_r).abs() > self.config.identity_tolerance
                        || (data[idx + 1] - expected_g).abs() > self.config.identity_tolerance
                        || (data[idx + 2] - expected_b).abs() > self.config.identity_tolerance
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Compute the maximum deviation from identity for 3D LUT data.
    ///
    /// `size` is the number of entries per dimension.
    #[must_use]
    pub fn max_identity_deviation_3d(&self, data: &[f64], size: usize) -> Option<f64> {
        let expected_len = size * size * size * 3;
        if data.len() != expected_len || size == 0 {
            return None;
        }

        let step = if size > 1 {
            1.0 / (size as f64 - 1.0)
        } else {
            0.0
        };

        let mut max_dev = 0.0_f64;

        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    let idx = (b * size * size + g * size + r) * 3;
                    let expected_r = r as f64 * step;
                    let expected_g = g as f64 * step;
                    let expected_b = b as f64 * step;

                    let dev_r = (data[idx] - expected_r).abs();
                    let dev_g = (data[idx + 1] - expected_g).abs();
                    let dev_b = (data[idx + 2] - expected_b).abs();

                    max_dev = max_dev.max(dev_r).max(dev_g).max(dev_b);
                }
            }
        }
        Some(max_dev)
    }
}

impl Default for LutValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a LUT size is a power-of-two-plus-one (common for 3D LUTs: 17, 33, 65).
#[must_use]
pub fn is_standard_lut_size(size: usize) -> bool {
    matches!(size, 17 | 33 | 65 | 129)
}

/// Generate identity 3D LUT data for a given size.
#[must_use]
pub fn generate_identity_3d(size: usize) -> Vec<f64> {
    let step = if size > 1 {
        1.0 / (size as f64 - 1.0)
    } else {
        0.0
    };
    let mut data = Vec::with_capacity(size * size * size * 3);
    for b in 0..size {
        for g in 0..size {
            for r in 0..size {
                data.push(r as f64 * step);
                data.push(g as f64 * step);
                data.push(b as f64 * step);
            }
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_severity_display() {
        assert_eq!(ValidationSeverity::Info.to_string(), "INFO");
        assert_eq!(ValidationSeverity::Warning.to_string(), "WARNING");
        assert_eq!(ValidationSeverity::Error.to_string(), "ERROR");
    }

    #[test]
    fn test_diagnostic_is_error() {
        let d = ValidationDiagnostic::new(ValidationSeverity::Error, "bad");
        assert!(d.is_error());
        assert!(!d.is_warning());
    }

    #[test]
    fn test_diagnostic_is_warning() {
        let d = ValidationDiagnostic::new(ValidationSeverity::Warning, "warn");
        assert!(!d.is_error());
        assert!(d.is_warning());
    }

    #[test]
    fn test_diagnostic_with_location() {
        let d = ValidationDiagnostic::with_location(ValidationSeverity::Info, "note", "index 42");
        assert_eq!(d.location.as_deref(), Some("index 42"));
    }

    #[test]
    fn test_report_add_error_marks_failed() {
        let mut report = ValidationReport::new("test");
        assert!(report.passed);
        report.add(ValidationDiagnostic::new(ValidationSeverity::Error, "fail"));
        assert!(!report.passed);
    }

    #[test]
    fn test_report_count_severity() {
        let mut report = ValidationReport::new("test");
        report.add(ValidationDiagnostic::new(ValidationSeverity::Warning, "w1"));
        report.add(ValidationDiagnostic::new(ValidationSeverity::Warning, "w2"));
        report.add(ValidationDiagnostic::new(ValidationSeverity::Info, "i1"));
        assert_eq!(report.count_severity(&ValidationSeverity::Warning), 2);
        assert_eq!(report.count_severity(&ValidationSeverity::Info), 1);
        assert_eq!(report.count_severity(&ValidationSeverity::Error), 0);
    }

    #[test]
    fn test_validate_1d_empty_channel() {
        let v = LutValidator::new();
        let report = v.validate_1d_channel(&[], "R");
        assert!(!report.passed);
        assert_eq!(report.errors().len(), 1);
    }

    #[test]
    fn test_validate_1d_good_channel() {
        let v = LutValidator::new();
        let data: Vec<f64> = (0..256).map(|i| i as f64 / 255.0).collect();
        let report = v.validate_1d_channel(&data, "R");
        assert!(report.passed);
        assert_eq!(report.errors().len(), 0);
    }

    #[test]
    fn test_validate_1d_nan_detected() {
        let v = LutValidator::new();
        let data = vec![0.0, f64::NAN, 1.0];
        let report = v.validate_1d_channel(&data, "G");
        assert!(!report.passed);
        assert!(!report.errors().is_empty());
    }

    #[test]
    fn test_validate_3d_wrong_size() {
        let v = LutValidator::new();
        let data = vec![0.0; 100];
        let report = v.validate_3d_data(&data, 5);
        assert!(!report.passed);
    }

    #[test]
    fn test_validate_3d_identity_passes() {
        let v = LutValidator::new();
        let size = 5;
        let data = generate_identity_3d(size);
        let report = v.validate_3d_data(&data, size);
        assert!(report.passed);
        assert_eq!(report.warnings().len(), 0);
    }

    #[test]
    fn test_check_identity_3d_true() {
        let v = LutValidator::new();
        let size = 5;
        let data = generate_identity_3d(size);
        assert!(v.check_identity_3d(&data, size));
    }

    #[test]
    fn test_check_identity_3d_false() {
        let v = LutValidator::new();
        let size = 5;
        let mut data = generate_identity_3d(size);
        data[0] = 0.5; // perturb
        assert!(!v.check_identity_3d(&data, size));
    }

    #[test]
    fn test_max_identity_deviation() {
        let v = LutValidator::new();
        let size = 5;
        let data = generate_identity_3d(size);
        let dev = v.max_identity_deviation_3d(&data, size);
        assert!(dev.is_some());
        assert!(dev.expect("should succeed in test") < 1e-12);
    }

    #[test]
    fn test_is_standard_lut_size() {
        assert!(is_standard_lut_size(17));
        assert!(is_standard_lut_size(33));
        assert!(is_standard_lut_size(65));
        assert!(is_standard_lut_size(129));
        assert!(!is_standard_lut_size(16));
        assert!(!is_standard_lut_size(32));
    }

    #[test]
    fn test_generate_identity_3d_length() {
        let data = generate_identity_3d(5);
        assert_eq!(data.len(), 5 * 5 * 5 * 3);
    }

    #[test]
    fn test_validator_default() {
        let v = LutValidator::default();
        assert!((v.config.min_value - 0.0).abs() < f64::EPSILON);
        assert!((v.config.max_value - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_validate_1d_out_of_range() {
        let v = LutValidator::new();
        let data = vec![0.0, 0.5, 1.5];
        let report = v.validate_1d_channel(&data, "B");
        assert!(report.passed); // out-of-range is a warning, not error
        assert_eq!(report.warnings().len(), 1);
    }
}
