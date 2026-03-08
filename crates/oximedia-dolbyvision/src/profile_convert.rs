//! Dolby Vision profile conversion utilities.
//!
//! Different Dolby Vision profiles encode metadata differently; converting
//! between them (e.g., Profile 7 to Profile 8.1) is a common workflow step.
//! This module provides types for describing conversion paths, executing
//! conversions, and generating reports on what was changed.

#![allow(dead_code)]

use crate::Profile;
use std::fmt;

// ---------------------------------------------------------------------------
// ConversionPath
// ---------------------------------------------------------------------------

/// Describes the source and destination profiles for a DV conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConversionPath {
    /// Source Dolby Vision profile.
    pub from: Profile,
    /// Destination Dolby Vision profile.
    pub to: Profile,
}

impl ConversionPath {
    /// Create a new conversion path.
    #[must_use]
    pub const fn new(from: Profile, to: Profile) -> Self {
        Self { from, to }
    }

    /// Returns `true` when both endpoints are the same (identity conversion).
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.from == self.to
    }

    /// Returns `true` if the conversion drops MEL data
    /// (Profile 7 -> any single-layer profile).
    #[must_use]
    pub fn drops_mel(&self) -> bool {
        self.from.has_mel() && !self.to.has_mel()
    }

    /// Returns `true` if the conversion changes the backward-compatibility
    /// base layer signal type (e.g., HDR10 to HLG or vice-versa).
    #[must_use]
    pub fn changes_base_signal(&self) -> bool {
        self.from.is_hlg() != self.to.is_hlg()
    }

    /// Returns `true` if the destination is a low-latency profile.
    #[must_use]
    pub fn targets_low_latency(&self) -> bool {
        self.to.is_low_latency()
    }

    /// Returns `true` if this conversion path is considered safe (lossless
    /// in terms of metadata fidelity). Currently this means identity
    /// conversions or conversions that don't drop MEL or change base signal.
    #[must_use]
    pub fn is_safe(&self) -> bool {
        self.is_identity() || (!self.drops_mel() && !self.changes_base_signal())
    }
}

impl fmt::Display for ConversionPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "P{:?} -> P{:?}", self.from, self.to)
    }
}

// ---------------------------------------------------------------------------
// ConversionAction
// ---------------------------------------------------------------------------

/// Describes a single action performed during profile conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversionAction {
    /// MEL layer was stripped.
    StripMel,
    /// Base layer signal was remapped (e.g., HDR10 to HLG).
    RemapBaseSignal,
    /// Trim passes were regenerated for the target.
    RegenerateTrimPasses,
    /// Low-latency constraints were applied.
    ApplyLowLatency,
    /// RPU header fields were updated.
    UpdateHeader,
    /// Level metadata block was adjusted.
    AdjustLevel(u8),
    /// No changes needed (identity).
    NoOp,
}

impl ConversionAction {
    /// Short label for the action.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::StripMel => "strip-mel",
            Self::RemapBaseSignal => "remap-base",
            Self::RegenerateTrimPasses => "regen-trims",
            Self::ApplyLowLatency => "low-latency",
            Self::UpdateHeader => "update-header",
            Self::AdjustLevel(_) => "adjust-level",
            Self::NoOp => "no-op",
        }
    }

    /// Returns `true` if this action is potentially lossy.
    #[must_use]
    pub fn is_lossy(&self) -> bool {
        matches!(self, Self::StripMel | Self::RemapBaseSignal)
    }
}

// ---------------------------------------------------------------------------
// DvProfileConverter
// ---------------------------------------------------------------------------

/// Dolby Vision profile converter.
///
/// Plans and records the conversion actions needed to transform RPU metadata
/// from one profile to another.
#[derive(Debug, Clone)]
pub struct DvProfileConverter {
    /// The conversion path being executed.
    pub path: ConversionPath,
    /// Planned actions in execution order.
    actions: Vec<ConversionAction>,
    /// Whether the converter has been finalized (planned).
    planned: bool,
}

impl DvProfileConverter {
    /// Create a new converter for the given path.
    #[must_use]
    pub fn new(path: ConversionPath) -> Self {
        Self {
            path,
            actions: Vec::new(),
            planned: false,
        }
    }

    /// Plan the conversion. Populates the action list based on the path.
    pub fn plan(&mut self) {
        self.actions.clear();

        if self.path.is_identity() {
            self.actions.push(ConversionAction::NoOp);
        } else {
            // Always update headers when changing profiles
            self.actions.push(ConversionAction::UpdateHeader);

            if self.path.drops_mel() {
                self.actions.push(ConversionAction::StripMel);
            }

            if self.path.changes_base_signal() {
                self.actions.push(ConversionAction::RemapBaseSignal);
            }

            if self.path.targets_low_latency() {
                self.actions.push(ConversionAction::ApplyLowLatency);
            }

            // Trim passes always need regeneration when profile changes
            self.actions.push(ConversionAction::RegenerateTrimPasses);
        }

        self.planned = true;
    }

    /// Returns `true` if the converter has been planned.
    #[must_use]
    pub fn is_planned(&self) -> bool {
        self.planned
    }

    /// Number of planned actions.
    #[must_use]
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    /// Returns `true` if any planned action is lossy.
    #[must_use]
    pub fn has_lossy_action(&self) -> bool {
        self.actions.iter().any(|a| a.is_lossy())
    }

    /// Iterate over planned actions.
    pub fn actions(&self) -> &[ConversionAction] {
        &self.actions
    }

    /// Generate a conversion report.
    #[must_use]
    pub fn report(&self) -> ConversionReport {
        let warnings: Vec<String> = self
            .actions
            .iter()
            .filter(|a| a.is_lossy())
            .map(|a| format!("Lossy action: {}", a.label()))
            .collect();

        ConversionReport {
            path: self.path,
            action_count: self.actions.len(),
            has_lossy: self.has_lossy_action(),
            warnings,
            success: self.planned,
        }
    }
}

// ---------------------------------------------------------------------------
// ConversionReport
// ---------------------------------------------------------------------------

/// Summary report of a Dolby Vision profile conversion.
#[derive(Debug, Clone)]
pub struct ConversionReport {
    /// The conversion path that was executed.
    pub path: ConversionPath,
    /// Total number of actions executed.
    pub action_count: usize,
    /// Whether any lossy actions were performed.
    pub has_lossy: bool,
    /// Warnings generated during conversion.
    pub warnings: Vec<String>,
    /// Whether the conversion completed successfully.
    pub success: bool,
}

impl ConversionReport {
    /// Returns `true` if there are no warnings.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.warnings.is_empty() && self.success
    }

    /// Number of warnings.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    /// Format the report as a human-readable summary string.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "Conversion {}: {} actions, {} warnings, lossy={}",
            if self.success { "OK" } else { "FAILED" },
            self.action_count,
            self.warnings.len(),
            self.has_lossy
        )
    }
}

impl fmt::Display for ConversionReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.summary())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversion_path_identity() {
        let p = ConversionPath::new(Profile::Profile8, Profile::Profile8);
        assert!(p.is_identity());
        assert!(p.is_safe());
    }

    #[test]
    fn test_conversion_path_drops_mel() {
        let p = ConversionPath::new(Profile::Profile7, Profile::Profile8);
        assert!(p.drops_mel());
        assert!(!p.is_safe());
    }

    #[test]
    fn test_conversion_path_no_mel_drop() {
        let p = ConversionPath::new(Profile::Profile8, Profile::Profile5);
        assert!(!p.drops_mel());
    }

    #[test]
    fn test_conversion_path_changes_base_signal() {
        let p = ConversionPath::new(Profile::Profile8, Profile::Profile8_4);
        assert!(p.changes_base_signal());
    }

    #[test]
    fn test_conversion_path_same_base_signal() {
        let p = ConversionPath::new(Profile::Profile8, Profile::Profile5);
        assert!(!p.changes_base_signal());
    }

    #[test]
    fn test_conversion_path_targets_low_latency() {
        let p = ConversionPath::new(Profile::Profile8, Profile::Profile8_1);
        assert!(p.targets_low_latency());
        let p2 = ConversionPath::new(Profile::Profile8, Profile::Profile5);
        assert!(!p2.targets_low_latency());
    }

    #[test]
    fn test_conversion_path_display() {
        let p = ConversionPath::new(Profile::Profile7, Profile::Profile8);
        let s = format!("{p}");
        assert!(s.contains("Profile7"));
        assert!(s.contains("Profile8"));
    }

    #[test]
    fn test_action_label() {
        assert_eq!(ConversionAction::StripMel.label(), "strip-mel");
        assert_eq!(ConversionAction::NoOp.label(), "no-op");
        assert_eq!(ConversionAction::AdjustLevel(2).label(), "adjust-level");
    }

    #[test]
    fn test_action_lossy() {
        assert!(ConversionAction::StripMel.is_lossy());
        assert!(ConversionAction::RemapBaseSignal.is_lossy());
        assert!(!ConversionAction::UpdateHeader.is_lossy());
        assert!(!ConversionAction::NoOp.is_lossy());
    }

    #[test]
    fn test_converter_identity() {
        let path = ConversionPath::new(Profile::Profile8, Profile::Profile8);
        let mut conv = DvProfileConverter::new(path);
        assert!(!conv.is_planned());
        conv.plan();
        assert!(conv.is_planned());
        assert_eq!(conv.action_count(), 1);
        assert!(!conv.has_lossy_action());
    }

    #[test]
    fn test_converter_mel_drop() {
        let path = ConversionPath::new(Profile::Profile7, Profile::Profile8);
        let mut conv = DvProfileConverter::new(path);
        conv.plan();
        assert!(conv.has_lossy_action());
        assert!(conv.action_count() >= 3); // update-header, strip-mel, regen-trims
    }

    #[test]
    fn test_converter_low_latency() {
        let path = ConversionPath::new(Profile::Profile8, Profile::Profile8_1);
        let mut conv = DvProfileConverter::new(path);
        conv.plan();
        let labels: Vec<&str> = conv.actions().iter().map(|a| a.label()).collect();
        assert!(labels.contains(&"low-latency"));
    }

    #[test]
    fn test_report_clean_identity() {
        let path = ConversionPath::new(Profile::Profile5, Profile::Profile5);
        let mut conv = DvProfileConverter::new(path);
        conv.plan();
        let report = conv.report();
        assert!(report.is_clean());
        assert_eq!(report.warning_count(), 0);
        assert!(report.success);
    }

    #[test]
    fn test_report_lossy_has_warnings() {
        let path = ConversionPath::new(Profile::Profile7, Profile::Profile8);
        let mut conv = DvProfileConverter::new(path);
        conv.plan();
        let report = conv.report();
        assert!(report.has_lossy);
        assert!(report.warning_count() > 0);
    }

    #[test]
    fn test_report_summary() {
        let path = ConversionPath::new(Profile::Profile8, Profile::Profile8);
        let mut conv = DvProfileConverter::new(path);
        conv.plan();
        let report = conv.report();
        let summary = report.summary();
        assert!(summary.contains("OK"));
        assert!(summary.contains("lossy=false"));
    }

    #[test]
    fn test_report_display() {
        let path = ConversionPath::new(Profile::Profile8, Profile::Profile8_4);
        let mut conv = DvProfileConverter::new(path);
        conv.plan();
        let report = conv.report();
        let s = format!("{report}");
        assert!(s.contains("actions"));
    }
}
