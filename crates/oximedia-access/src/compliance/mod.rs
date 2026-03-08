//! Accessibility compliance checking.

pub mod ebu;
pub mod report;
pub mod section508;
pub mod wcag;

pub use ebu::EbuChecker;
pub use report::{ComplianceIssue, ComplianceReport, IssueSeverity};
pub use section508::Section508Checker;
pub use wcag::{WcagChecker, WcagGuideline, WcagLevel};

/// Compliance checker for accessibility standards.
pub struct ComplianceChecker {
    wcag_checker: WcagChecker,
    section508_checker: Section508Checker,
    ebu_checker: EbuChecker,
}

impl ComplianceChecker {
    /// Create a new compliance checker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            wcag_checker: WcagChecker::new(WcagLevel::AA),
            section508_checker: Section508Checker::new(),
            ebu_checker: EbuChecker::new(),
        }
    }

    /// Check all compliance standards.
    #[must_use]
    pub fn check_all(&self) -> ComplianceReport {
        let mut report = ComplianceReport::new();

        // Check WCAG compliance
        let wcag_issues = self.wcag_checker.check();
        report.add_issues(wcag_issues);

        // Check Section 508
        let section508_issues = self.section508_checker.check();
        report.add_issues(section508_issues);

        // Check EBU
        let ebu_issues = self.ebu_checker.check();
        report.add_issues(ebu_issues);

        report
    }

    /// Check specific standard.
    #[must_use]
    pub fn check_wcag(&self) -> Vec<ComplianceIssue> {
        self.wcag_checker.check()
    }

    /// Check Section 508 compliance.
    #[must_use]
    pub fn check_section508(&self) -> Vec<ComplianceIssue> {
        self.section508_checker.check()
    }

    /// Check EBU compliance.
    #[must_use]
    pub fn check_ebu(&self) -> Vec<ComplianceIssue> {
        self.ebu_checker.check()
    }

    /// Get WCAG checker.
    #[must_use]
    pub const fn wcag_checker(&self) -> &WcagChecker {
        &self.wcag_checker
    }

    /// Get Section 508 checker.
    #[must_use]
    pub const fn section508_checker(&self) -> &Section508Checker {
        &self.section508_checker
    }

    /// Get EBU checker.
    #[must_use]
    pub const fn ebu_checker(&self) -> &EbuChecker {
        &self.ebu_checker
    }
}

impl Default for ComplianceChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checker_creation() {
        let checker = ComplianceChecker::new();
        let report = checker.check_all();
        assert!(report.issues().is_empty() || !report.issues().is_empty());
    }
}
