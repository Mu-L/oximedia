#![allow(dead_code)]
//! Concatenation and joining of multiple media sources into a single output.
//!
//! Handles cross-format joining with optional transition effects between
//! segments, automatic audio/video alignment, and gap filling.

use std::fmt;

/// Strategy for handling format mismatches between segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformStrategy {
    /// Re-encode every segment to match the first segment's format.
    ReEncodeAll,
    /// Re-encode only segments that differ from the target format.
    ReEncodeDiffers,
    /// Attempt stream-copy where possible (fastest, may fail on mismatches).
    StreamCopy,
}

impl fmt::Display for ConformStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReEncodeAll => write!(f, "re-encode-all"),
            Self::ReEncodeDiffers => write!(f, "re-encode-differs"),
            Self::StreamCopy => write!(f, "stream-copy"),
        }
    }
}

/// Transition type between consecutive segments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransitionKind {
    /// Hard cut with no transition.
    Cut,
    /// Crossfade of the specified duration in seconds.
    Crossfade(f64),
    /// Fade to black then fade from black.
    FadeThrough(f64),
}

impl TransitionKind {
    /// Return the duration in seconds (0.0 for a hard cut).
    #[must_use]
    pub fn duration(&self) -> f64 {
        match self {
            Self::Cut => 0.0,
            Self::Crossfade(d) | Self::FadeThrough(d) => *d,
        }
    }
}

/// A single input segment in the concat list.
#[derive(Debug, Clone)]
pub struct ConcatSegment {
    /// Path or URI to the source media.
    pub source: String,
    /// Optional in-point in seconds (trim start).
    pub in_point: Option<f64>,
    /// Optional out-point in seconds (trim end).
    pub out_point: Option<f64>,
    /// Transition to apply *after* this segment (before the next).
    pub transition: TransitionKind,
}

impl ConcatSegment {
    /// Create a segment from a source path with defaults (full duration, hard cut).
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            in_point: None,
            out_point: None,
            transition: TransitionKind::Cut,
        }
    }

    /// Set in-point.
    #[must_use]
    pub fn with_in_point(mut self, seconds: f64) -> Self {
        self.in_point = Some(seconds);
        self
    }

    /// Set out-point.
    #[must_use]
    pub fn with_out_point(mut self, seconds: f64) -> Self {
        self.out_point = Some(seconds);
        self
    }

    /// Set transition after this segment.
    #[must_use]
    pub fn with_transition(mut self, t: TransitionKind) -> Self {
        self.transition = t;
        self
    }

    /// Compute effective duration (returns `None` when both points are absent).
    #[must_use]
    pub fn effective_duration(&self) -> Option<f64> {
        match (self.in_point, self.out_point) {
            (Some(i), Some(o)) => Some((o - i).max(0.0)),
            _ => None,
        }
    }
}

/// Overall concat job configuration.
#[derive(Debug, Clone)]
pub struct ConcatConfig {
    /// Ordered list of segments.
    pub segments: Vec<ConcatSegment>,
    /// Output path.
    pub output: String,
    /// Conforming strategy.
    pub conform: ConformStrategy,
    /// Target video width (if re-encoding).
    pub target_width: Option<u32>,
    /// Target video height (if re-encoding).
    pub target_height: Option<u32>,
    /// Target frame rate numerator / denominator (if re-encoding).
    pub target_fps: Option<(u32, u32)>,
    /// Target audio sample rate.
    pub target_sample_rate: Option<u32>,
}

impl ConcatConfig {
    /// Create a new concat configuration.
    pub fn new(output: impl Into<String>) -> Self {
        Self {
            segments: Vec::new(),
            output: output.into(),
            conform: ConformStrategy::ReEncodeDiffers,
            target_width: None,
            target_height: None,
            target_fps: None,
            target_sample_rate: None,
        }
    }

    /// Add a segment.
    pub fn add_segment(&mut self, seg: ConcatSegment) {
        self.segments.push(seg);
    }

    /// Set conforming strategy.
    #[must_use]
    pub fn with_conform(mut self, strategy: ConformStrategy) -> Self {
        self.conform = strategy;
        self
    }

    /// Set target resolution.
    #[must_use]
    pub fn with_resolution(mut self, w: u32, h: u32) -> Self {
        self.target_width = Some(w);
        self.target_height = Some(h);
        self
    }

    /// Set target frame rate.
    #[must_use]
    pub fn with_fps(mut self, num: u32, den: u32) -> Self {
        self.target_fps = Some((num, den));
        self
    }

    /// Set target audio sample rate.
    #[must_use]
    pub fn with_sample_rate(mut self, rate: u32) -> Self {
        self.target_sample_rate = Some(rate);
        self
    }

    /// Return total number of segments.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Compute the total transition time between segments.
    #[must_use]
    pub fn total_transition_time(&self) -> f64 {
        self.segments.iter().map(|s| s.transition.duration()).sum()
    }

    /// Compute the total known content duration (sum of effective durations).
    /// Segments without known duration are excluded.
    #[must_use]
    pub fn total_known_duration(&self) -> f64 {
        self.segments
            .iter()
            .filter_map(ConcatSegment::effective_duration)
            .sum()
    }
}

/// Result of a concat operation.
#[derive(Debug, Clone)]
pub struct ConcatResult {
    /// Output file path.
    pub output_path: String,
    /// Number of segments joined.
    pub segments_joined: usize,
    /// Total output duration in seconds.
    pub total_duration: f64,
    /// Number of segments that required re-encoding.
    pub re_encoded_count: usize,
}

/// Validate a concat configuration and return a list of issues (empty = valid).
#[must_use]
pub fn validate_concat(config: &ConcatConfig) -> Vec<String> {
    let mut issues = Vec::new();
    if config.segments.is_empty() {
        issues.push("No segments specified".to_string());
    }
    if config.output.is_empty() {
        issues.push("Output path is empty".to_string());
    }
    for (i, seg) in config.segments.iter().enumerate() {
        if seg.source.is_empty() {
            issues.push(format!("Segment {i} has empty source path"));
        }
        if let (Some(inp), Some(out)) = (seg.in_point, seg.out_point) {
            if out <= inp {
                issues.push(format!("Segment {i} out-point ({out}) <= in-point ({inp})"));
            }
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conform_strategy_display() {
        assert_eq!(ConformStrategy::ReEncodeAll.to_string(), "re-encode-all");
        assert_eq!(
            ConformStrategy::ReEncodeDiffers.to_string(),
            "re-encode-differs"
        );
        assert_eq!(ConformStrategy::StreamCopy.to_string(), "stream-copy");
    }

    #[test]
    fn test_transition_duration() {
        assert!((TransitionKind::Cut.duration() - 0.0).abs() < f64::EPSILON);
        assert!((TransitionKind::Crossfade(1.5).duration() - 1.5).abs() < f64::EPSILON);
        assert!((TransitionKind::FadeThrough(2.0).duration() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_segment_new() {
        let seg = ConcatSegment::new("clip.mp4");
        assert_eq!(seg.source, "clip.mp4");
        assert!(seg.in_point.is_none());
        assert!(seg.out_point.is_none());
        assert_eq!(seg.transition, TransitionKind::Cut);
    }

    #[test]
    fn test_segment_trim() {
        let seg = ConcatSegment::new("clip.mp4")
            .with_in_point(5.0)
            .with_out_point(15.0);
        assert!(
            (seg.effective_duration().expect("should succeed in test") - 10.0).abs() < f64::EPSILON
        );
    }

    #[test]
    fn test_segment_no_duration() {
        let seg = ConcatSegment::new("clip.mp4").with_in_point(5.0);
        assert!(seg.effective_duration().is_none());
    }

    #[test]
    fn test_concat_config_builder() {
        let mut config = ConcatConfig::new("output.mp4")
            .with_conform(ConformStrategy::StreamCopy)
            .with_resolution(1920, 1080)
            .with_fps(30, 1)
            .with_sample_rate(48000);
        config.add_segment(ConcatSegment::new("a.mp4"));
        config.add_segment(ConcatSegment::new("b.mp4"));

        assert_eq!(config.segment_count(), 2);
        assert_eq!(config.conform, ConformStrategy::StreamCopy);
        assert_eq!(config.target_width, Some(1920));
        assert_eq!(config.target_height, Some(1080));
        assert_eq!(config.target_fps, Some((30, 1)));
        assert_eq!(config.target_sample_rate, Some(48000));
    }

    #[test]
    fn test_total_transition_time() {
        let mut config = ConcatConfig::new("out.mp4");
        config.add_segment(
            ConcatSegment::new("a.mp4").with_transition(TransitionKind::Crossfade(1.0)),
        );
        config.add_segment(
            ConcatSegment::new("b.mp4").with_transition(TransitionKind::FadeThrough(0.5)),
        );
        config.add_segment(ConcatSegment::new("c.mp4"));
        assert!((config.total_transition_time() - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_total_known_duration() {
        let mut config = ConcatConfig::new("out.mp4");
        config.add_segment(
            ConcatSegment::new("a.mp4")
                .with_in_point(0.0)
                .with_out_point(10.0),
        );
        config.add_segment(ConcatSegment::new("b.mp4")); // unknown duration
        config.add_segment(
            ConcatSegment::new("c.mp4")
                .with_in_point(5.0)
                .with_out_point(20.0),
        );
        assert!((config.total_known_duration() - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_validate_empty_segments() {
        let config = ConcatConfig::new("out.mp4");
        let issues = validate_concat(&config);
        assert!(issues.iter().any(|i| i.contains("No segments")));
    }

    #[test]
    fn test_validate_empty_output() {
        let mut config = ConcatConfig::new("");
        config.add_segment(ConcatSegment::new("a.mp4"));
        let issues = validate_concat(&config);
        assert!(issues.iter().any(|i| i.contains("Output path")));
    }

    #[test]
    fn test_validate_bad_trim() {
        let mut config = ConcatConfig::new("out.mp4");
        config.add_segment(
            ConcatSegment::new("a.mp4")
                .with_in_point(20.0)
                .with_out_point(5.0),
        );
        let issues = validate_concat(&config);
        assert!(issues.iter().any(|i| i.contains("out-point")));
    }

    #[test]
    fn test_validate_valid_config() {
        let mut config = ConcatConfig::new("out.mp4");
        config.add_segment(
            ConcatSegment::new("a.mp4")
                .with_in_point(0.0)
                .with_out_point(10.0),
        );
        let issues = validate_concat(&config);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_concat_result_fields() {
        let result = ConcatResult {
            output_path: "out.mp4".to_string(),
            segments_joined: 3,
            total_duration: 30.0,
            re_encoded_count: 1,
        };
        assert_eq!(result.segments_joined, 3);
        assert!((result.total_duration - 30.0).abs() < f64::EPSILON);
    }
}
