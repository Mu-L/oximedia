//! IMF package-level validation helpers.
//!
//! Provides a lightweight validator that checks an IMF package structure and
//! collects issues with their severity, then reports which ones are blocking.

#![allow(dead_code)]

/// How serious a validation finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ValidationSeverity {
    /// Informational note – does not prevent delivery.
    Info,
    /// Advisory warning – delivery is possible but not recommended.
    Warning,
    /// Error that blocks delivery or conformance.
    Error,
}

impl ValidationSeverity {
    /// Returns `true` when this severity prevents package delivery.
    #[must_use]
    pub fn is_blocking(self) -> bool {
        self == Self::Error
    }

    /// Short human-readable tag.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARN",
            Self::Error => "ERROR",
        }
    }
}

// ---------------------------------------------------------------------------

/// A single issue found during package validation.
#[derive(Debug, Clone)]
pub struct PackageIssue {
    /// Severity of this issue.
    pub severity: ValidationSeverity,
    /// Machine-readable code (e.g. `"PKL_HASH_MISSING"`).
    pub code: String,
    /// Human-readable description of the problem.
    pub detail: String,
}

impl PackageIssue {
    /// Create a new issue.
    #[must_use]
    pub fn new(
        severity: ValidationSeverity,
        code: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            detail: detail.into(),
        }
    }

    /// Formatted description including severity tag and code.
    #[must_use]
    pub fn description(&self) -> String {
        format!("[{}] {}: {}", self.severity.tag(), self.code, self.detail)
    }

    /// Returns `true` when this issue is blocking.
    #[must_use]
    pub fn is_blocking(&self) -> bool {
        self.severity.is_blocking()
    }
}

// ---------------------------------------------------------------------------

/// Accumulates issues found while checking a package.
#[derive(Debug, Default)]
pub struct PackageValidator {
    issues: Vec<PackageIssue>,
}

impl PackageValidator {
    /// Create a fresh validator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a pre-built issue.
    pub fn record(&mut self, issue: PackageIssue) {
        self.issues.push(issue);
    }

    /// Convenience: record a blocking error.
    pub fn check(&mut self, condition: bool, code: impl Into<String>, detail: impl Into<String>) {
        if !condition {
            self.issues
                .push(PackageIssue::new(ValidationSeverity::Error, code, detail));
        }
    }

    /// Convenience: record a non-blocking warning.
    pub fn warn(&mut self, condition: bool, code: impl Into<String>, detail: impl Into<String>) {
        if !condition {
            self.issues
                .push(PackageIssue::new(ValidationSeverity::Warning, code, detail));
        }
    }

    /// Consume the validator and return the final report.
    #[must_use]
    pub fn finish(self) -> PackageValidationReport {
        PackageValidationReport {
            issues: self.issues,
        }
    }
}

// ---------------------------------------------------------------------------

/// The result of running [`PackageValidator`] over an IMF package.
#[derive(Debug, Clone, Default)]
pub struct PackageValidationReport {
    /// All issues found (may be empty).
    pub issues: Vec<PackageIssue>,
}

impl PackageValidationReport {
    /// All issues whose severity is [`ValidationSeverity::Error`].
    #[must_use]
    pub fn blocking_issues(&self) -> Vec<&PackageIssue> {
        self.issues.iter().filter(|i| i.is_blocking()).collect()
    }

    /// Returns `true` if there are no blocking issues.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.blocking_issues().is_empty()
    }

    /// Total issue count regardless of severity.
    #[must_use]
    pub fn total(&self) -> usize {
        self.issues.len()
    }

    /// Count issues of a specific severity.
    #[must_use]
    pub fn count_severity(&self, sev: ValidationSeverity) -> usize {
        self.issues.iter().filter(|i| i.severity == sev).count()
    }
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_error_is_blocking() {
        assert!(ValidationSeverity::Error.is_blocking());
    }

    #[test]
    fn test_severity_warning_not_blocking() {
        assert!(!ValidationSeverity::Warning.is_blocking());
    }

    #[test]
    fn test_severity_info_not_blocking() {
        assert!(!ValidationSeverity::Info.is_blocking());
    }

    #[test]
    fn test_severity_tags_distinct() {
        let tags = [
            ValidationSeverity::Info.tag(),
            ValidationSeverity::Warning.tag(),
            ValidationSeverity::Error.tag(),
        ];
        let set: std::collections::HashSet<_> = tags.iter().collect();
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_issue_description_contains_code() {
        let issue = PackageIssue::new(
            ValidationSeverity::Error,
            "PKL_MISSING",
            "Packing list not found",
        );
        assert!(issue.description().contains("PKL_MISSING"));
    }

    #[test]
    fn test_issue_is_blocking_for_error() {
        let issue = PackageIssue::new(ValidationSeverity::Error, "X", "detail");
        assert!(issue.is_blocking());
    }

    #[test]
    fn test_issue_not_blocking_for_warning() {
        let issue = PackageIssue::new(ValidationSeverity::Warning, "X", "detail");
        assert!(!issue.is_blocking());
    }

    #[test]
    fn test_validator_check_records_error_on_false() {
        let mut v = PackageValidator::new();
        v.check(false, "CODE", "detail");
        let r = v.finish();
        assert_eq!(r.total(), 1);
        assert!(!r.is_ok());
    }

    #[test]
    fn test_validator_check_no_issue_on_true() {
        let mut v = PackageValidator::new();
        v.check(true, "CODE", "detail");
        let r = v.finish();
        assert_eq!(r.total(), 0);
    }

    #[test]
    fn test_validator_warn_records_warning() {
        let mut v = PackageValidator::new();
        v.warn(false, "WARN_CODE", "advisory");
        let r = v.finish();
        assert_eq!(r.count_severity(ValidationSeverity::Warning), 1);
        assert!(r.is_ok()); // warnings are not blocking
    }

    #[test]
    fn test_report_blocking_issues_only_errors() {
        let mut v = PackageValidator::new();
        v.warn(false, "W1", "warn");
        v.check(false, "E1", "error");
        let r = v.finish();
        assert_eq!(r.blocking_issues().len(), 1);
    }

    #[test]
    fn test_report_count_severity() {
        let mut v = PackageValidator::new();
        v.check(false, "E1", "e");
        v.check(false, "E2", "e");
        v.warn(false, "W1", "w");
        let r = v.finish();
        assert_eq!(r.count_severity(ValidationSeverity::Error), 2);
        assert_eq!(r.count_severity(ValidationSeverity::Warning), 1);
    }

    #[test]
    fn test_empty_report_is_ok() {
        let v = PackageValidator::new();
        let r = v.finish();
        assert!(r.is_ok());
        assert_eq!(r.total(), 0);
    }
}
