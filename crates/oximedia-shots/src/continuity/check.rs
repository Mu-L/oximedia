//! Continuity checking between shots.

use crate::types::Shot;

/// Continuity checker.
pub struct ContinuityChecker;

impl ContinuityChecker {
    /// Create a new continuity checker.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Check continuity between consecutive shots.
    #[must_use]
    pub fn check_continuity(&self, shots: &[Shot]) -> Vec<ContinuityIssue> {
        let mut issues = Vec::new();

        for i in 1..shots.len() {
            // Check for jump cuts (same shot type and angle in sequence)
            if shots[i - 1].shot_type == shots[i].shot_type
                && shots[i - 1].angle == shots[i].angle
                && shots[i - 1].coverage == shots[i].coverage
            {
                issues.push(ContinuityIssue {
                    shot_id: shots[i].id,
                    issue_type: IssueType::JumpCut,
                    severity: Severity::Medium,
                    description: "Potential jump cut detected".to_string(),
                });
            }

            // Check for crossing the line (180-degree rule violation)
            // This would require spatial analysis which we'll simplify
        }

        issues
    }
}

impl Default for ContinuityChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Continuity issue found during analysis.
#[derive(Debug, Clone)]
pub struct ContinuityIssue {
    /// Shot ID where issue occurs.
    pub shot_id: u64,
    /// Type of issue.
    pub issue_type: IssueType,
    /// Severity of issue.
    pub severity: Severity,
    /// Description of issue.
    pub description: String,
}

/// Type of continuity issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueType {
    /// Jump cut (similar consecutive shots).
    JumpCut,
    /// Crossing the line (180-degree rule).
    CrossingTheLine,
    /// Screen direction mismatch.
    ScreenDirection,
    /// Eyeline mismatch.
    EyelineMismatch,
    /// Temporal discontinuity.
    TemporalDiscontinuity,
}

/// Severity of continuity issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Low severity.
    Low,
    /// Medium severity.
    Medium,
    /// High severity.
    High,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_continuity_checker_creation() {
        let _checker = ContinuityChecker::new();
    }

    #[test]
    fn test_check_empty() {
        let checker = ContinuityChecker::new();
        let issues = checker.check_continuity(&[]);
        assert!(issues.is_empty());
    }
}
