//! Reading-speed analysis for subtitle tracks.
//!
//! Checks each subtitle cue against per-level CPS (characters per second)
//! limits and produces a compliance report.

#![allow(dead_code)]

/// Classifies the reading level of an audience, each with a CPS ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingLevel {
    /// Children or learners: up to 14 CPS.
    Children,
    /// Standard broadcast: up to 17 CPS (EBU R37 / BBC guidelines).
    Standard,
    /// Verbatim / fast-paced content: up to 22 CPS.
    Fast,
    /// No CPS limit enforced.
    Unlimited,
}

impl ReadingLevel {
    /// Returns the maximum allowed characters per second for this level.
    /// `None` means no limit is enforced.
    #[must_use]
    pub fn cps_limit(self) -> Option<f64> {
        match self {
            Self::Children => Some(14.0),
            Self::Standard => Some(17.0),
            Self::Fast => Some(22.0),
            Self::Unlimited => None,
        }
    }

    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Children => "children",
            Self::Standard => "standard",
            Self::Fast => "fast",
            Self::Unlimited => "unlimited",
        }
    }
}

/// A check result for a single subtitle cue.
#[derive(Debug, Clone)]
pub struct ReadingSpeedCheck {
    /// Index of the cue within the track.
    pub cue_index: usize,
    /// Start time in milliseconds.
    pub start_ms: i64,
    /// End time in milliseconds.
    pub end_ms: i64,
    /// Actual characters per second for this cue.
    pub actual_cps: f64,
    /// CPS limit that was applied.
    pub limit_cps: Option<f64>,
    /// Whether this cue violates the CPS limit.
    pub violation: bool,
}

impl ReadingSpeedCheck {
    /// Returns `true` when the cue exceeds the allowed reading speed.
    #[must_use]
    pub fn is_too_fast(&self) -> bool {
        self.violation
    }

    /// Returns the excess CPS above the limit, or `0.0` if within limits.
    #[must_use]
    pub fn excess_cps(&self) -> f64 {
        match self.limit_cps {
            Some(limit) if self.actual_cps > limit => self.actual_cps - limit,
            _ => 0.0,
        }
    }
}

/// A subtitle cue used as input for reading-speed analysis.
#[derive(Debug, Clone)]
pub struct SpeedCue {
    /// Index of this cue.
    pub index: usize,
    /// Start time in milliseconds.
    pub start_ms: i64,
    /// End time in milliseconds.
    pub end_ms: i64,
    /// Plain-text content of the cue.
    pub text: String,
}

impl SpeedCue {
    /// Creates a new `SpeedCue`.
    #[must_use]
    pub fn new(index: usize, start_ms: i64, end_ms: i64, text: impl Into<String>) -> Self {
        Self {
            index,
            start_ms,
            end_ms,
            text: text.into(),
        }
    }

    /// Duration in seconds. Returns `0.0` for zero-length or inverted cues.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn duration_secs(&self) -> f64 {
        let dur_ms = (self.end_ms - self.start_ms).max(0);
        dur_ms as f64 / 1000.0
    }

    /// Number of printable characters (excluding whitespace for CPS calc).
    #[must_use]
    pub fn char_count(&self) -> usize {
        self.text.chars().filter(|c| !c.is_whitespace()).count()
    }

    /// Computes characters per second.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn cps(&self) -> f64 {
        let dur = self.duration_secs();
        if dur <= 0.0 {
            return 0.0;
        }
        self.char_count() as f64 / dur
    }
}

/// Analyses subtitle reading speed against a configurable level.
#[derive(Debug, Clone)]
pub struct ReadingSpeedAnalyzer {
    /// Target reading level.
    pub level: ReadingLevel,
}

impl ReadingSpeedAnalyzer {
    /// Creates a new analyser for the given reading level.
    #[must_use]
    pub fn new(level: ReadingLevel) -> Self {
        Self { level }
    }

    /// Analyses all cues and returns per-cue checks.
    #[must_use]
    pub fn analyze(&self, cues: &[SpeedCue]) -> Vec<ReadingSpeedCheck> {
        let limit = self.level.cps_limit();
        cues.iter()
            .map(|cue| {
                let actual_cps = cue.cps();
                let violation = limit.is_some_and(|l| actual_cps > l);
                ReadingSpeedCheck {
                    cue_index: cue.index,
                    start_ms: cue.start_ms,
                    end_ms: cue.end_ms,
                    actual_cps,
                    limit_cps: limit,
                    violation,
                }
            })
            .collect()
    }

    /// Returns only the cues that violate the reading-speed limit.
    #[must_use]
    pub fn violations(&self, cues: &[SpeedCue]) -> Vec<ReadingSpeedCheck> {
        self.analyze(cues)
            .into_iter()
            .filter(|c| c.violation)
            .collect()
    }
}

/// Summary report for reading-speed compliance.
#[derive(Debug, Clone)]
pub struct ReadingSpeedReport {
    /// Reading level that was checked.
    pub level: ReadingLevel,
    /// Total number of cues analysed.
    pub total_cues: usize,
    /// Number of cues that violate the CPS limit.
    pub violation_count: usize,
    /// Maximum CPS observed across all cues.
    pub max_cps: f64,
    /// Average CPS across all cues.
    pub avg_cps: f64,
}

impl ReadingSpeedReport {
    /// Builds a report from a set of per-cue checks.
    #[must_use]
    pub fn from_checks(level: ReadingLevel, checks: &[ReadingSpeedCheck]) -> Self {
        let total_cues = checks.len();
        let violation_count = checks.iter().filter(|c| c.violation).count();
        let max_cps = checks.iter().map(|c| c.actual_cps).fold(0.0f64, f64::max);
        let avg_cps = if total_cues == 0 {
            0.0
        } else {
            checks.iter().map(|c| c.actual_cps).sum::<f64>() / total_cues as f64
        };
        Self {
            level,
            total_cues,
            violation_count,
            max_cps,
            avg_cps,
        }
    }

    /// Percentage of cues that are within the reading-speed limit (0.0–100.0).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn compliance_pct(&self) -> f64 {
        if self.total_cues == 0 {
            return 100.0;
        }
        let compliant = self.total_cues.saturating_sub(self.violation_count);
        compliant as f64 / self.total_cues as f64 * 100.0
    }

    /// Returns `true` when all cues are within the CPS limit.
    #[must_use]
    pub fn is_fully_compliant(&self) -> bool {
        self.violation_count == 0
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cue(index: usize, start_ms: i64, end_ms: i64, text: &str) -> SpeedCue {
        SpeedCue::new(index, start_ms, end_ms, text)
    }

    #[test]
    fn test_reading_level_cps_limit_children() {
        assert_eq!(ReadingLevel::Children.cps_limit(), Some(14.0));
    }

    #[test]
    fn test_reading_level_cps_limit_unlimited() {
        assert_eq!(ReadingLevel::Unlimited.cps_limit(), None);
    }

    #[test]
    fn test_reading_level_labels() {
        assert_eq!(ReadingLevel::Standard.label(), "standard");
        assert_eq!(ReadingLevel::Fast.label(), "fast");
    }

    #[test]
    fn test_speed_cue_duration_secs() {
        let cue = make_cue(0, 0, 2000, "hello");
        assert!((cue.duration_secs() - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_speed_cue_char_count_excludes_spaces() {
        let cue = make_cue(0, 0, 1000, "hello world");
        // 'h','e','l','l','o','w','o','r','l','d' = 10
        assert_eq!(cue.char_count(), 10);
    }

    #[test]
    fn test_speed_cue_cps() {
        // 10 non-space chars, 2 seconds → 5 CPS
        let cue = make_cue(0, 0, 2000, "hello world");
        assert!((cue.cps() - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_speed_cue_zero_duration_cps() {
        let cue = make_cue(0, 1000, 1000, "hello");
        assert_eq!(cue.cps(), 0.0);
    }

    #[test]
    fn test_reading_speed_check_is_too_fast() {
        let check = ReadingSpeedCheck {
            cue_index: 0,
            start_ms: 0,
            end_ms: 1000,
            actual_cps: 20.0,
            limit_cps: Some(17.0),
            violation: true,
        };
        assert!(check.is_too_fast());
        assert!((check.excess_cps() - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_reading_speed_check_not_too_fast() {
        let check = ReadingSpeedCheck {
            cue_index: 0,
            start_ms: 0,
            end_ms: 1000,
            actual_cps: 10.0,
            limit_cps: Some(17.0),
            violation: false,
        };
        assert!(!check.is_too_fast());
        assert_eq!(check.excess_cps(), 0.0);
    }

    #[test]
    fn test_analyzer_no_violations_slow_text() {
        let cues = vec![make_cue(0, 0, 5000, "Hello")];
        let analyzer = ReadingSpeedAnalyzer::new(ReadingLevel::Standard);
        let violations = analyzer.violations(&cues);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_analyzer_detects_violation() {
        // "abcdefghijklmnopqrstuvwxyz" = 26 non-space chars in 1 second → 26 CPS
        let cues = vec![make_cue(0, 0, 1000, "abcdefghijklmnopqrstuvwxyz")];
        let analyzer = ReadingSpeedAnalyzer::new(ReadingLevel::Standard);
        let violations = analyzer.violations(&cues);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_report_compliance_pct_full() {
        let cues = vec![
            make_cue(0, 0, 5000, "Short"),
            make_cue(1, 5000, 10000, "Also short"),
        ];
        let analyzer = ReadingSpeedAnalyzer::new(ReadingLevel::Standard);
        let checks = analyzer.analyze(&cues);
        let report = ReadingSpeedReport::from_checks(ReadingLevel::Standard, &checks);
        assert!((report.compliance_pct() - 100.0).abs() < 0.01);
        assert!(report.is_fully_compliant());
    }

    #[test]
    fn test_report_compliance_pct_partial() {
        let cues = vec![
            make_cue(0, 0, 1000, "abcdefghijklmnopqrstuvwxyz"), // violates
            make_cue(1, 1000, 6000, "fine"),                    // ok
        ];
        let analyzer = ReadingSpeedAnalyzer::new(ReadingLevel::Standard);
        let checks = analyzer.analyze(&cues);
        let report = ReadingSpeedReport::from_checks(ReadingLevel::Standard, &checks);
        assert!((report.compliance_pct() - 50.0).abs() < 0.01);
        assert!(!report.is_fully_compliant());
    }

    #[test]
    fn test_report_empty_cues() {
        let report = ReadingSpeedReport::from_checks(ReadingLevel::Standard, &[]);
        assert!((report.compliance_pct() - 100.0).abs() < 0.01);
        assert_eq!(report.total_cues, 0);
    }
}
