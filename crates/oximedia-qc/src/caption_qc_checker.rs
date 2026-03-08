//! QC checker for closed captions and subtitles.
//!
//! This module validates caption lines for common broadcast and streaming issues
//! such as excessive reading speed, overlapping timing, and line length.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

/// Category of error found during caption QC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptionError {
    /// Caption line exceeds the maximum allowed character count.
    TooLong,
    /// Words per minute exceeds the maximum reading speed.
    TooFast,
    /// Caption timing is invalid (start >= end).
    BadTiming,
    /// Caption overlaps with another caption in time.
    Overlap,
    /// Caption line does not end with a period or sentence-ending punctuation.
    MissingPeriod,
    /// Caption contains a detected spelling error (placeholder).
    SpellingError,
}

impl CaptionError {
    /// Returns a numeric severity score for this error type.
    ///
    /// Higher values indicate more critical failures.
    #[must_use]
    pub fn severity(&self) -> u32 {
        match self {
            Self::BadTiming => 100,
            Self::Overlap => 90,
            Self::TooFast => 70,
            Self::TooLong => 60,
            Self::MissingPeriod => 30,
            Self::SpellingError => 20,
        }
    }
}

/// A single line of closed caption text with timing information.
#[derive(Debug, Clone)]
pub struct CaptionLine {
    /// The caption text.
    pub text: String,
    /// Start frame (inclusive).
    pub start_frame: u64,
    /// End frame (exclusive).
    pub end_frame: u64,
}

impl CaptionLine {
    /// Creates a new caption line.
    #[must_use]
    pub fn new(text: impl Into<String>, start_frame: u64, end_frame: u64) -> Self {
        Self {
            text: text.into(),
            start_frame,
            end_frame,
        }
    }

    /// Returns the number of frames this caption is displayed.
    #[must_use]
    pub fn duration_frames(&self) -> u64 {
        self.end_frame.saturating_sub(self.start_frame)
    }

    /// Returns the number of characters in the caption text.
    #[must_use]
    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    /// Calculates words per minute at the given frame rate.
    ///
    /// Returns `0.0` if duration is zero or `fps` is non-positive.
    #[must_use]
    pub fn words_per_minute(&self, fps: f32) -> f32 {
        if fps <= 0.0 {
            return 0.0;
        }
        let duration_frames = self.duration_frames();
        if duration_frames == 0 {
            return 0.0;
        }
        let duration_secs = duration_frames as f32 / fps;
        let word_count = self.text.split_whitespace().count() as f32;
        word_count / duration_secs * 60.0
    }
}

/// A QC report for a collection of caption lines.
#[derive(Debug, Clone)]
pub struct CaptionQcReport {
    /// The caption lines that were checked.
    pub lines: Vec<CaptionLine>,
    /// Errors found, as (line_index, error) pairs.
    pub errors: Vec<(usize, CaptionError)>,
}

impl CaptionQcReport {
    /// Creates a new report.
    #[must_use]
    pub fn new(lines: Vec<CaptionLine>, errors: Vec<(usize, CaptionError)>) -> Self {
        Self { lines, errors }
    }

    /// Returns `true` if any errors were found.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Returns the number of errors found.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    /// Returns the ratio of erroneous lines to total lines.
    ///
    /// Returns `0.0` if there are no lines.
    #[must_use]
    pub fn error_rate(&self) -> f32 {
        if self.lines.is_empty() {
            return 0.0;
        }
        // Count unique line indices with errors
        let mut errored_lines: Vec<usize> = self.errors.iter().map(|(i, _)| *i).collect();
        errored_lines.sort_unstable();
        errored_lines.dedup();
        errored_lines.len() as f32 / self.lines.len() as f32
    }
}

/// Checker that validates caption lines against configurable thresholds.
#[derive(Debug, Clone)]
pub struct CaptionQcChecker {
    /// Maximum allowed characters per caption line.
    pub max_chars_per_line: usize,
    /// Maximum allowed reading speed in words per minute.
    pub max_wpm: f32,
    /// Maximum allowed caption duration in frames.
    pub max_duration_frames: u64,
}

impl CaptionQcChecker {
    /// Returns a checker configured with typical broadcast defaults:
    /// - 42 characters per line
    /// - 180 WPM
    /// - 6 seconds at 25fps = 150 frames
    #[must_use]
    pub fn broadcast_default() -> Self {
        Self {
            max_chars_per_line: 42,
            max_wpm: 180.0,
            max_duration_frames: 150,
        }
    }

    /// Runs all checks against the provided caption lines and returns a report.
    ///
    /// Assumes 25 fps for WPM calculations when not otherwise specified.
    #[must_use]
    pub fn check(&self, lines: &[CaptionLine]) -> CaptionQcReport {
        const FPS: f32 = 25.0;
        let mut errors: Vec<(usize, CaptionError)> = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            // Bad timing
            if line.start_frame >= line.end_frame {
                errors.push((i, CaptionError::BadTiming));
                continue; // Skip further checks on a bad-timing line
            }

            // Too long
            if line.char_count() > self.max_chars_per_line {
                errors.push((i, CaptionError::TooLong));
            }

            // Too fast
            let wpm = line.words_per_minute(FPS);
            if wpm > self.max_wpm {
                errors.push((i, CaptionError::TooFast));
            }

            // Duration too long
            if line.duration_frames() > self.max_duration_frames {
                errors.push((i, CaptionError::TooFast));
            }

            // Overlap check with next line
            if let Some(next) = lines.get(i + 1) {
                if next.start_frame < line.end_frame {
                    errors.push((i, CaptionError::Overlap));
                }
            }
        }

        CaptionQcReport::new(lines.to_vec(), errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- CaptionError tests ---

    #[test]
    fn test_error_severity_ordering() {
        assert!(CaptionError::BadTiming.severity() > CaptionError::TooFast.severity());
        assert!(CaptionError::TooFast.severity() > CaptionError::MissingPeriod.severity());
        assert!(CaptionError::MissingPeriod.severity() > CaptionError::SpellingError.severity());
    }

    #[test]
    fn test_error_overlap_severity() {
        assert_eq!(CaptionError::Overlap.severity(), 90);
    }

    // --- CaptionLine tests ---

    #[test]
    fn test_caption_line_duration() {
        let line = CaptionLine::new("Hello world", 0, 50);
        assert_eq!(line.duration_frames(), 50);
    }

    #[test]
    fn test_caption_line_duration_inverted() {
        let line = CaptionLine::new("Hello", 100, 50);
        assert_eq!(line.duration_frames(), 0);
    }

    #[test]
    fn test_caption_line_char_count() {
        let line = CaptionLine::new("Hello", 0, 50);
        assert_eq!(line.char_count(), 5);
    }

    #[test]
    fn test_caption_line_words_per_minute() {
        // 2 words in 25 frames at 25fps => 2 words / 1 sec => 120 WPM
        let line = CaptionLine::new("Hello world", 0, 25);
        let wpm = line.words_per_minute(25.0);
        assert!((wpm - 120.0).abs() < 1.0);
    }

    #[test]
    fn test_caption_line_wpm_zero_fps() {
        let line = CaptionLine::new("Hello", 0, 25);
        assert!((line.words_per_minute(0.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_caption_line_wpm_zero_duration() {
        let line = CaptionLine::new("Hello", 50, 50);
        assert!((line.words_per_minute(25.0) - 0.0).abs() < 1e-6);
    }

    // --- CaptionQcReport tests ---

    #[test]
    fn test_report_has_no_errors() {
        let report = CaptionQcReport::new(vec![], vec![]);
        assert!(!report.has_errors());
    }

    #[test]
    fn test_report_has_errors() {
        let line = CaptionLine::new("Bad", 10, 5);
        let report = CaptionQcReport::new(vec![line], vec![(0, CaptionError::BadTiming)]);
        assert!(report.has_errors());
        assert_eq!(report.error_count(), 1);
    }

    #[test]
    fn test_report_error_rate_no_lines() {
        let report = CaptionQcReport::new(vec![], vec![]);
        assert!((report.error_rate() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_report_error_rate_half() {
        let lines = vec![
            CaptionLine::new("OK", 0, 50),
            CaptionLine::new("Bad", 50, 40),
        ];
        let errors = vec![(1, CaptionError::BadTiming)];
        let report = CaptionQcReport::new(lines, errors);
        assert!((report.error_rate() - 0.5).abs() < 1e-6);
    }

    // --- CaptionQcChecker tests ---

    #[test]
    fn test_checker_broadcast_default() {
        let checker = CaptionQcChecker::broadcast_default();
        assert_eq!(checker.max_chars_per_line, 42);
        assert!((checker.max_wpm - 180.0).abs() < 1e-6);
    }

    #[test]
    fn test_checker_clean_lines() {
        let checker = CaptionQcChecker::broadcast_default();
        let lines = vec![CaptionLine::new("Hello world.", 0, 50)];
        let report = checker.check(&lines);
        assert!(!report.has_errors());
    }

    #[test]
    fn test_checker_detects_bad_timing() {
        let checker = CaptionQcChecker::broadcast_default();
        let lines = vec![CaptionLine::new("Bad timing", 100, 50)];
        let report = checker.check(&lines);
        assert!(report
            .errors
            .iter()
            .any(|(_, e)| *e == CaptionError::BadTiming));
    }

    #[test]
    fn test_checker_detects_overlap() {
        let checker = CaptionQcChecker::broadcast_default();
        let lines = vec![
            CaptionLine::new("Line one.", 0, 60),
            CaptionLine::new("Line two.", 50, 100),
        ];
        let report = checker.check(&lines);
        assert!(report
            .errors
            .iter()
            .any(|(_, e)| *e == CaptionError::Overlap));
    }
}
