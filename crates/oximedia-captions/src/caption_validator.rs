//! Caption content and timing validation against broadcast and web standards.
//!
//! Checks captions for reading-speed compliance, overlapping timecodes,
//! minimum display duration, maximum line length, and empty-text entries.

#![allow(dead_code)]

/// A specific rule that the validator enforces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidationRule {
    /// Caption text must not exceed a maximum character count per line.
    MaxCharsPerLine,
    /// Captions must be displayed for at least a minimum duration.
    MinDisplayDuration,
    /// Adjacent captions must not have overlapping time ranges.
    NoOverlap,
    /// Reading speed (words per minute) must not exceed a threshold.
    MaxReadingSpeed,
    /// Caption text must not be empty or whitespace-only.
    NonEmpty,
    /// Maximum number of rows visible simultaneously.
    MaxRows,
}

impl ValidationRule {
    /// Human-readable description of the rule.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::MaxCharsPerLine => "Line length exceeds the configured maximum",
            Self::MinDisplayDuration => "Caption displayed for less than the minimum duration",
            Self::NoOverlap => "Caption time range overlaps with an adjacent caption",
            Self::MaxReadingSpeed => "Reading speed exceeds the configured maximum WPM",
            Self::NonEmpty => "Caption text is empty or whitespace only",
            Self::MaxRows => "Too many caption rows visible simultaneously",
        }
    }
}

/// A single validation violation found in a caption.
#[derive(Debug, Clone)]
pub struct CaptionViolation {
    /// Index of the offending caption in the track.
    pub caption_index: usize,
    /// The rule that was violated.
    pub rule: ValidationRule,
    /// Human-readable explanation.
    pub message: String,
}

impl CaptionViolation {
    /// Create a new violation.
    #[must_use]
    pub fn new(caption_index: usize, rule: ValidationRule, message: impl Into<String>) -> Self {
        Self {
            caption_index,
            rule,
            message: message.into(),
        }
    }
}

/// A minimal caption entry used by the validator (avoids depending on the
/// full `Caption` type from `crate::types`).
#[derive(Debug, Clone)]
pub struct ValidatorCaption {
    /// Display start in milliseconds.
    pub start_ms: u64,
    /// Display end in milliseconds.
    pub end_ms: u64,
    /// Lines of text.
    pub lines: Vec<String>,
}

impl ValidatorCaption {
    /// Create a caption with a single line.
    #[must_use]
    pub fn single(start_ms: u64, end_ms: u64, text: impl Into<String>) -> Self {
        Self {
            start_ms,
            end_ms,
            lines: vec![text.into()],
        }
    }

    /// Duration in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }

    /// Total text content across all lines.
    #[must_use]
    pub fn full_text(&self) -> String {
        self.lines.join(" ")
    }

    /// Word count across all lines.
    #[must_use]
    pub fn word_count(&self) -> usize {
        self.lines.iter().flat_map(|l| l.split_whitespace()).count()
    }
}

/// Configuration for the caption validator.
#[derive(Debug, Clone)]
pub struct ValidatorConfig {
    /// Maximum characters allowed per line.
    pub max_chars_per_line: usize,
    /// Minimum display duration in milliseconds.
    pub min_duration_ms: u64,
    /// Maximum reading speed in words per minute.
    pub max_wpm: f64,
    /// Maximum simultaneous rows.
    pub max_rows: usize,
    /// Rules that are currently enabled.
    pub enabled_rules: Vec<ValidationRule>,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            max_chars_per_line: 42,
            min_duration_ms: 800,
            max_wpm: 200.0,
            max_rows: 3,
            enabled_rules: vec![
                ValidationRule::MaxCharsPerLine,
                ValidationRule::MinDisplayDuration,
                ValidationRule::NoOverlap,
                ValidationRule::MaxReadingSpeed,
                ValidationRule::NonEmpty,
                ValidationRule::MaxRows,
            ],
        }
    }
}

/// Validates a sequence of captions against configurable rules.
#[derive(Debug)]
pub struct CaptionValidator {
    config: ValidatorConfig,
}

impl CaptionValidator {
    /// Create a new validator with the given configuration.
    #[must_use]
    pub fn new(config: ValidatorConfig) -> Self {
        Self { config }
    }

    /// Access the current configuration.
    #[must_use]
    pub fn config(&self) -> &ValidatorConfig {
        &self.config
    }

    fn rule_enabled(&self, rule: ValidationRule) -> bool {
        self.config.enabled_rules.contains(&rule)
    }

    /// Validate a slice of captions. Returns all found violations.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn validate(&self, captions: &[ValidatorCaption]) -> Vec<CaptionViolation> {
        let mut violations = Vec::new();

        for (idx, cap) in captions.iter().enumerate() {
            // NonEmpty
            if self.rule_enabled(ValidationRule::NonEmpty) {
                let text = cap.full_text();
                if text.trim().is_empty() {
                    violations.push(CaptionViolation::new(
                        idx,
                        ValidationRule::NonEmpty,
                        "Caption has no text",
                    ));
                }
            }

            // MaxCharsPerLine
            if self.rule_enabled(ValidationRule::MaxCharsPerLine) {
                for line in &cap.lines {
                    if line.len() > self.config.max_chars_per_line {
                        violations.push(CaptionViolation::new(
                            idx,
                            ValidationRule::MaxCharsPerLine,
                            format!(
                                "Line has {} chars (max {})",
                                line.len(),
                                self.config.max_chars_per_line
                            ),
                        ));
                        break;
                    }
                }
            }

            // MinDisplayDuration
            if self.rule_enabled(ValidationRule::MinDisplayDuration)
                && cap.duration_ms() < self.config.min_duration_ms
            {
                violations.push(CaptionViolation::new(
                    idx,
                    ValidationRule::MinDisplayDuration,
                    format!(
                        "Duration {}ms < min {}ms",
                        cap.duration_ms(),
                        self.config.min_duration_ms
                    ),
                ));
            }

            // MaxReadingSpeed (WPM)
            if self.rule_enabled(ValidationRule::MaxReadingSpeed) && cap.duration_ms() > 0 {
                let minutes = cap.duration_ms() as f64 / 60_000.0;
                let wpm = cap.word_count() as f64 / minutes;
                if wpm > self.config.max_wpm {
                    violations.push(CaptionViolation::new(
                        idx,
                        ValidationRule::MaxReadingSpeed,
                        format!("Reading speed {wpm:.0} WPM > max {}", self.config.max_wpm),
                    ));
                }
            }

            // MaxRows
            if self.rule_enabled(ValidationRule::MaxRows) && cap.lines.len() > self.config.max_rows
            {
                violations.push(CaptionViolation::new(
                    idx,
                    ValidationRule::MaxRows,
                    format!("{} rows > max {}", cap.lines.len(), self.config.max_rows),
                ));
            }
        }

        // NoOverlap — compare adjacent pairs.
        if self.rule_enabled(ValidationRule::NoOverlap) {
            for i in 1..captions.len() {
                let prev = &captions[i - 1];
                let curr = &captions[i];
                if curr.start_ms < prev.end_ms {
                    violations.push(CaptionViolation::new(
                        i,
                        ValidationRule::NoOverlap,
                        format!(
                            "Starts at {}ms but previous ends at {}ms",
                            curr.start_ms, prev.end_ms
                        ),
                    ));
                }
            }
        }

        violations
    }

    /// Returns `true` when there are no violations.
    #[must_use]
    pub fn is_valid(&self, captions: &[ValidatorCaption]) -> bool {
        self.validate(captions).is_empty()
    }

    /// Count violations grouped by rule.
    #[must_use]
    pub fn violation_counts(&self, captions: &[ValidatorCaption]) -> Vec<(ValidationRule, usize)> {
        let violations = self.validate(captions);
        let rules = [
            ValidationRule::MaxCharsPerLine,
            ValidationRule::MinDisplayDuration,
            ValidationRule::NoOverlap,
            ValidationRule::MaxReadingSpeed,
            ValidationRule::NonEmpty,
            ValidationRule::MaxRows,
        ];
        rules
            .iter()
            .map(|&r| {
                let count = violations.iter().filter(|v| v.rule == r).count();
                (r, count)
            })
            .filter(|(_, c)| *c > 0)
            .collect()
    }
}

impl Default for CaptionValidator {
    fn default() -> Self {
        Self::new(ValidatorConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good_cap(start: u64, end: u64, text: &str) -> ValidatorCaption {
        ValidatorCaption::single(start, end, text)
    }

    #[test]
    fn test_valid_caption_no_violations() {
        let v = CaptionValidator::default();
        let caps = vec![good_cap(0, 2000, "Hello world")];
        assert!(v.is_valid(&caps));
    }

    #[test]
    fn test_empty_text_violation() {
        let v = CaptionValidator::default();
        let caps = vec![good_cap(0, 2000, "   ")];
        let violations = v.validate(&caps);
        assert!(violations
            .iter()
            .any(|v| v.rule == ValidationRule::NonEmpty));
    }

    #[test]
    fn test_line_too_long() {
        let v = CaptionValidator::default();
        let long_line = "a".repeat(50);
        let caps = vec![good_cap(0, 2000, &long_line)];
        let violations = v.validate(&caps);
        assert!(violations
            .iter()
            .any(|v| v.rule == ValidationRule::MaxCharsPerLine));
    }

    #[test]
    fn test_min_duration_violation() {
        let v = CaptionValidator::default();
        let caps = vec![good_cap(0, 400, "Short")]; // 400ms < 800ms
        let violations = v.validate(&caps);
        assert!(violations
            .iter()
            .any(|v| v.rule == ValidationRule::MinDisplayDuration));
    }

    #[test]
    fn test_overlap_violation() {
        let v = CaptionValidator::default();
        let caps = vec![
            good_cap(0, 2000, "First"),
            good_cap(1500, 3500, "Second"), // starts before first ends
        ];
        let violations = v.validate(&caps);
        assert!(violations
            .iter()
            .any(|v| v.rule == ValidationRule::NoOverlap));
    }

    #[test]
    fn test_no_overlap_with_gap() {
        let v = CaptionValidator::default();
        let caps = vec![good_cap(0, 1000, "First"), good_cap(1100, 2100, "Second")];
        assert!(!v
            .validate(&caps)
            .iter()
            .any(|v| v.rule == ValidationRule::NoOverlap));
    }

    #[test]
    fn test_max_reading_speed() {
        let v = CaptionValidator::default();
        // 20 words in 1 second = 1200 WPM → exceeds 200 WPM
        let text = vec![
            "one two three four five six seven eight nine ten".to_string(),
            "a b c d e f g h i j".to_string(),
        ];
        let cap = ValidatorCaption {
            start_ms: 0,
            end_ms: 1000,
            lines: text,
        };
        let violations = v.validate(&[cap]);
        assert!(violations
            .iter()
            .any(|v| v.rule == ValidationRule::MaxReadingSpeed));
    }

    #[test]
    fn test_max_rows_violation() {
        let v = CaptionValidator::default();
        let cap = ValidatorCaption {
            start_ms: 0,
            end_ms: 3000,
            lines: vec!["r1".into(), "r2".into(), "r3".into(), "r4".into()], // 4 > 3
        };
        let violations = v.validate(&[cap]);
        assert!(violations.iter().any(|v| v.rule == ValidationRule::MaxRows));
    }

    #[test]
    fn test_violation_counts() {
        let v = CaptionValidator::default();
        let caps = vec![
            good_cap(0, 400, "Short duration"),
            good_cap(300, 2400, "Overlap with previous"),
        ];
        let counts = v.violation_counts(&caps);
        assert!(!counts.is_empty());
    }

    #[test]
    fn test_empty_track_is_valid() {
        let v = CaptionValidator::default();
        assert!(v.is_valid(&[]));
    }

    #[test]
    fn test_validator_caption_word_count() {
        let c = ValidatorCaption::single(0, 1000, "one two three");
        assert_eq!(c.word_count(), 3);
    }

    #[test]
    fn test_validator_caption_full_text_multiline() {
        let cap = ValidatorCaption {
            start_ms: 0,
            end_ms: 1000,
            lines: vec!["Hello".into(), "World".into()],
        };
        assert_eq!(cap.full_text(), "Hello World");
    }

    #[test]
    fn test_validator_caption_duration() {
        let c = good_cap(500, 1500, "x");
        assert_eq!(c.duration_ms(), 1000);
    }

    #[test]
    fn test_rule_description_non_empty() {
        let desc = ValidationRule::NonEmpty.description();
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_violation_message_stored() {
        let v = CaptionValidator::default();
        let caps = vec![good_cap(0, 400, "Short")];
        let violations = v.validate(&caps);
        assert!(!violations[0].message.is_empty());
    }
}
