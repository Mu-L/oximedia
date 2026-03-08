//! Inverse Telecine (IVTC) filter.
//!
//! This filter removes telecine artifacts from video, converting telecined content
//! back to its original progressive frame rate. It handles various pulldown patterns
//! including 3:2 pulldown (29.97fps → 23.976fps), 2:2 pulldown, and advanced patterns.
//!
//! # Telecine Overview
//!
//! Telecine is the process of converting film (typically 24fps) to video formats:
//! - **3:2 pulldown**: Converts 24fps film to 29.97fps NTSC video
//! - **2:2 pulldown**: Converts 25fps to 25fps PAL (field duplication)
//! - **2:3:3:2 pulldown**: Advanced pattern for better motion handling
//!
//! # Pattern Detection
//!
//! The filter can automatically detect telecine patterns by analyzing:
//! - Field matching scores across consecutive frames
//! - Comb artifact metrics
//! - Frame difference patterns
//! - Cadence consistency over time
//!
//! # Field Matching
//!
//! For each output frame, the filter:
//! 1. Analyzes multiple field pair combinations
//! 2. Calculates combing metrics for each combination
//! 3. Selects the best field match
//! 4. Determines which frames to decimate (drop)
//!
//! # Example
//!
//! ```ignore
//! use oximedia_graph::filters::video::{IvtcFilter, IvtcConfig, TelecinePattern};
//! use oximedia_graph::node::NodeId;
//!
//! let config = IvtcConfig::new()
//!     .with_pattern(TelecinePattern::Auto)
//!     .with_post_processing(true);
//!
//! let filter = IvtcFilter::new(NodeId(0), "ivtc", config);
//! ```

#![forbid(unsafe_code)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]
#![allow(clippy::many_single_char_names)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::unused_self)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::bool_to_int_with_if)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::too_many_arguments)]
#![allow(dead_code)]

use std::collections::VecDeque;

use crate::error::{GraphError, GraphResult};
use crate::frame::FilterFrame;
use crate::node::{Node, NodeId, NodeState, NodeType};
use crate::port::{InputPort, OutputPort, PortFormat, PortId, PortType, VideoPortFormat};
use oximedia_codec::{Plane, VideoFrame};
use oximedia_core::Timestamp;

/// Telecine pattern type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TelecinePattern {
    /// Automatic pattern detection.
    #[default]
    Auto,
    /// 3:2 pulldown (NTSC film: 24fps → 29.97fps).
    /// Pattern: top-top-top-bottom-bottom (TTTBB)
    Pattern32,
    /// 2:2 pulldown (PAL film: 24fps → 25fps).
    /// Simple field duplication pattern.
    Pattern22,
    /// 2:3:3:2 advanced pulldown pattern.
    /// Pattern: TT-BBB-BBB-TT for smoother motion.
    Pattern2332,
    /// Euro pulldown (24fps → 25fps with speed-up).
    EuroPulldown,
    /// Custom pattern specified as string (e.g., "TBTBT").
    Custom(CustomPattern),
}

/// Custom telecine pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CustomPattern {
    /// Pattern as field order indices (0 = top, 1 = bottom).
    fields: [u8; 10],
    /// Length of the pattern.
    length: usize,
}

#[allow(clippy::derivable_impls)]
impl Default for CustomPattern {
    fn default() -> Self {
        Self {
            fields: [0; 10],
            length: 0,
        }
    }
}

impl CustomPattern {
    /// Create a new custom pattern from a string.
    /// Pattern string uses 'T' for top field, 'B' for bottom field.
    #[must_use]
    pub fn from_string(pattern: &str) -> Self {
        let mut fields = [0u8; 10];
        let mut length = 0;

        for (i, ch) in pattern.chars().take(10).enumerate() {
            fields[i] = match ch {
                'T' | 't' => 0,
                'B' | 'b' => 1,
                _ => 0,
            };
            length = i + 1;
        }

        Self { fields, length }
    }

    /// Get the field at a specific position in the pattern.
    #[must_use]
    pub fn field_at(&self, position: usize) -> u8 {
        if self.length == 0 {
            return 0;
        }
        self.fields[position % self.length]
    }
}

impl TelecinePattern {
    /// Get the pattern length (number of frames in one cycle).
    #[must_use]
    pub fn cycle_length(&self) -> usize {
        match self {
            Self::Auto => 5, // Default to 3:2 for detection
            Self::Pattern32 => 5,
            Self::Pattern22 => 2,
            Self::Pattern2332 => 10,
            Self::EuroPulldown => 1,
            Self::Custom(p) => p.length,
        }
    }

    /// Get the number of frames to output per cycle.
    #[must_use]
    pub fn output_frames(&self) -> usize {
        match self {
            Self::Auto => 4,
            Self::Pattern32 => 4,   // 5 frames → 4 frames (drop 1 per cycle)
            Self::Pattern22 => 2,   // No decimation
            Self::Pattern2332 => 8, // 10 frames → 8 frames
            Self::EuroPulldown => 1,
            Self::Custom(p) => p.length.saturating_sub(p.length / 5),
        }
    }

    /// Check if this pattern requires decimation.
    #[must_use]
    pub fn requires_decimation(&self) -> bool {
        self.cycle_length() != self.output_frames()
    }
}

/// Field matching strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MatchMode {
    /// Match fields from current and next frames only.
    #[default]
    TwoWay,
    /// Match fields from previous, current, and next frames.
    ThreeWay,
    /// Match with extended temporal window (5 frames).
    FiveWay,
}

/// Post-processing mode for residual combing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PostProcessMode {
    /// No post-processing.
    None,
    /// Light deinterlacing for residual combing.
    #[default]
    Light,
    /// Medium strength post-processing.
    Medium,
    /// Aggressive post-processing.
    Aggressive,
}

/// IVTC detection sensitivity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DetectionSensitivity {
    /// Low sensitivity (fewer false positives).
    Low,
    /// Medium sensitivity.
    #[default]
    Medium,
    /// High sensitivity (catch more patterns).
    High,
}

impl DetectionSensitivity {
    /// Get the comb threshold for this sensitivity.
    #[must_use]
    fn comb_threshold(&self) -> f64 {
        match self {
            Self::Low => 30.0,
            Self::Medium => 20.0,
            Self::High => 10.0,
        }
    }

    /// Get the match threshold for this sensitivity.
    #[must_use]
    fn match_threshold(&self) -> f64 {
        match self {
            Self::Low => 0.8,
            Self::Medium => 0.6,
            Self::High => 0.4,
        }
    }
}

/// Configuration for the IVTC filter.
#[derive(Clone, Debug)]
pub struct IvtcConfig {
    /// Telecine pattern to detect/use.
    pub pattern: TelecinePattern,
    /// Field matching mode.
    pub match_mode: MatchMode,
    /// Post-processing mode.
    pub post_process: PostProcessMode,
    /// Detection sensitivity.
    pub sensitivity: DetectionSensitivity,
    /// Enable pattern lock after detection.
    pub pattern_lock: bool,
    /// Number of frames to analyze before locking pattern.
    pub lock_threshold: usize,
    /// Enable orphan field handling.
    pub handle_orphans: bool,
    /// Threshold for scene change detection (0.0-1.0).
    pub scene_change_threshold: f64,
}

impl Default for IvtcConfig {
    fn default() -> Self {
        Self {
            pattern: TelecinePattern::default(),
            match_mode: MatchMode::default(),
            post_process: PostProcessMode::default(),
            sensitivity: DetectionSensitivity::default(),
            pattern_lock: true,
            lock_threshold: 30,
            handle_orphans: true,
            scene_change_threshold: 0.3,
        }
    }
}

impl IvtcConfig {
    /// Create a new IVTC configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the telecine pattern.
    #[must_use]
    pub fn with_pattern(mut self, pattern: TelecinePattern) -> Self {
        self.pattern = pattern;
        self
    }

    /// Set the field matching mode.
    #[must_use]
    pub fn with_match_mode(mut self, mode: MatchMode) -> Self {
        self.match_mode = mode;
        self
    }

    /// Enable post-processing.
    #[must_use]
    pub fn with_post_processing(mut self, mode: PostProcessMode) -> Self {
        self.post_process = mode;
        self
    }

    /// Set detection sensitivity.
    #[must_use]
    pub fn with_sensitivity(mut self, sensitivity: DetectionSensitivity) -> Self {
        self.sensitivity = sensitivity;
        self
    }

    /// Enable pattern locking.
    #[must_use]
    pub fn with_pattern_lock(mut self, enabled: bool) -> Self {
        self.pattern_lock = enabled;
        self
    }

    /// Set the lock threshold.
    #[must_use]
    pub fn with_lock_threshold(mut self, threshold: usize) -> Self {
        self.lock_threshold = threshold;
        self
    }

    /// Enable orphan field handling.
    #[must_use]
    pub fn with_orphan_handling(mut self, enabled: bool) -> Self {
        self.handle_orphans = enabled;
        self
    }

    /// Set scene change threshold.
    #[must_use]
    pub fn with_scene_change_threshold(mut self, threshold: f64) -> Self {
        self.scene_change_threshold = threshold.clamp(0.0, 1.0);
        self
    }
}

/// Field match candidate.
#[derive(Clone, Debug)]
struct FieldMatch {
    /// Index of the first frame.
    frame1_idx: usize,
    /// Field from first frame (0 = top, 1 = bottom).
    field1: u8,
    /// Index of the second frame.
    frame2_idx: usize,
    /// Field from second frame.
    field2: u8,
    /// Combing score (lower is better).
    comb_score: f64,
    /// Difference score.
    diff_score: f64,
    /// Combined match quality (higher is better).
    match_quality: f64,
}

impl FieldMatch {
    /// Create a new field match.
    fn new(frame1_idx: usize, field1: u8, frame2_idx: usize, field2: u8) -> Self {
        Self {
            frame1_idx,
            field1,
            frame2_idx,
            field2,
            comb_score: f64::MAX,
            diff_score: f64::MAX,
            match_quality: 0.0,
        }
    }
}

/// Pattern detection state.
#[derive(Clone, Debug)]
struct PatternState {
    /// Detected pattern.
    pattern: TelecinePattern,
    /// Pattern confidence (0.0-1.0).
    confidence: f64,
    /// Current position in pattern cycle.
    cycle_position: usize,
    /// Number of consecutive frames matching pattern.
    match_streak: usize,
    /// Is pattern locked?
    locked: bool,
}

impl Default for PatternState {
    fn default() -> Self {
        Self {
            pattern: TelecinePattern::Auto,
            confidence: 0.0,
            cycle_position: 0,
            match_streak: 0,
            locked: false,
        }
    }
}

/// Field matching statistics.
#[derive(Clone, Debug, Default)]
pub struct MatchStats {
    /// Total frames processed.
    pub frames_processed: u64,
    /// Frames decimated (dropped).
    pub frames_decimated: u64,
    /// Average comb score.
    pub avg_comb_score: f64,
    /// Average match quality.
    pub avg_match_quality: f64,
    /// Number of scene changes detected.
    pub scene_changes: u64,
    /// Number of orphan fields handled.
    pub orphans_handled: u64,
}

/// IVTC filter for removing telecine artifacts.
///
/// This filter performs inverse telecine by detecting and removing pulldown patterns,
/// restoring video to its original progressive frame rate.
///
/// # Features
///
/// - Automatic pattern detection (3:2, 2:2, 2:3:3:2)
/// - Manual pattern specification
/// - Field matching with multiple algorithms
/// - Post-processing for residual combing
/// - Scene change detection
/// - Orphan field handling
/// - Pattern locking for consistent detection
///
/// # Example
///
/// ```ignore
/// use oximedia_graph::filters::video::{IvtcFilter, IvtcConfig, TelecinePattern};
/// use oximedia_graph::node::NodeId;
///
/// // Automatic detection
/// let config = IvtcConfig::new()
///     .with_pattern(TelecinePattern::Auto)
///     .with_post_processing(PostProcessMode::Light);
///
/// let filter = IvtcFilter::new(NodeId(0), "ivtc", config);
///
/// // Manual 3:2 pattern
/// let config = IvtcConfig::new()
///     .with_pattern(TelecinePattern::Pattern32)
///     .with_pattern_lock(true);
///
/// let filter = IvtcFilter::new(NodeId(1), "ivtc_32", config);
/// ```
pub struct IvtcFilter {
    id: NodeId,
    name: String,
    state: NodeState,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    config: IvtcConfig,
    /// Frame buffer for analysis and matching.
    frame_buffer: VecDeque<VideoFrame>,
    /// Pattern detection state.
    pattern_state: PatternState,
    /// Match statistics.
    stats: MatchStats,
    /// Output frame index.
    output_frame_idx: u64,
    /// Pending output frames.
    pending_output: VecDeque<VideoFrame>,
    /// Previous frame for scene change detection.
    prev_frame: Option<VideoFrame>,
    /// Match history for pattern detection.
    match_history: VecDeque<f64>,
}

impl IvtcFilter {
    /// Create a new IVTC filter.
    #[must_use]
    pub fn new(id: NodeId, name: impl Into<String>, config: IvtcConfig) -> Self {
        let buffer_size = match config.match_mode {
            MatchMode::TwoWay => 5,
            MatchMode::ThreeWay => 7,
            MatchMode::FiveWay => 10,
        };

        Self {
            id,
            name: name.into(),
            state: NodeState::Idle,
            inputs: vec![InputPort::new(PortId(0), "input", PortType::Video)
                .with_format(PortFormat::Video(VideoPortFormat::any()))],
            outputs: vec![OutputPort::new(PortId(0), "output", PortType::Video)
                .with_format(PortFormat::Video(VideoPortFormat::any()))],
            config,
            frame_buffer: VecDeque::with_capacity(buffer_size),
            pattern_state: PatternState::default(),
            stats: MatchStats::default(),
            output_frame_idx: 0,
            pending_output: VecDeque::new(),
            prev_frame: None,
            match_history: VecDeque::with_capacity(30),
        }
    }

    /// Get the current configuration.
    #[must_use]
    pub fn config(&self) -> &IvtcConfig {
        &self.config
    }

    /// Get statistics.
    #[must_use]
    pub fn stats(&self) -> &MatchStats {
        &self.stats
    }

    /// Detect if there's a scene change between two frames.
    fn detect_scene_change(&self, frame1: &VideoFrame, frame2: &VideoFrame) -> bool {
        if frame1.planes.is_empty() || frame2.planes.is_empty() {
            return false;
        }

        let plane1 = &frame1.planes[0];
        let plane2 = &frame2.planes[0];
        let height = frame1.height.min(frame2.height) as usize;
        let width = frame1.width.min(frame2.width) as usize;

        let mut diff_sum = 0u64;
        let mut samples = 0u64;

        // Sample every 4th pixel for efficiency
        for y in (0..height).step_by(4) {
            let row1 = plane1.row(y);
            let row2 = plane2.row(y);

            for x in (0..width).step_by(4) {
                let p1 = row1.get(x).copied().unwrap_or(0) as i32;
                let p2 = row2.get(x).copied().unwrap_or(0) as i32;
                diff_sum += (p1 - p2).unsigned_abs() as u64;
                samples += 1;
            }
        }

        if samples == 0 {
            return false;
        }

        let avg_diff = diff_sum as f64 / samples as f64;
        avg_diff > self.config.scene_change_threshold * 255.0
    }

    /// Calculate combing metric for a frame.
    fn calculate_comb_metric(&self, frame: &VideoFrame) -> f64 {
        if frame.planes.is_empty() {
            return 0.0;
        }

        let plane = &frame.planes[0];
        let height = frame.height as usize;
        let width = frame.width as usize;

        let mut comb_score = 0u64;
        let mut samples = 0u64;

        // Analyze combing artifacts by checking vertical frequency
        for y in 2..height - 2 {
            let row = plane.row(y);
            let row_prev = plane.row(y - 1);
            let row_next = plane.row(y + 1);
            let row_prev2 = plane.row(y - 2);
            let row_next2 = plane.row(y + 2);

            for x in 0..width {
                let curr = row.get(x).copied().unwrap_or(0) as i32;
                let prev1 = row_prev.get(x).copied().unwrap_or(0) as i32;
                let next1 = row_next.get(x).copied().unwrap_or(0) as i32;
                let prev2 = row_prev2.get(x).copied().unwrap_or(0) as i32;
                let next2 = row_next2.get(x).copied().unwrap_or(0) as i32;

                // Combing detection: alternating line pattern
                let interp = (prev1 + next1) / 2;
                let diff1 = (curr - interp).abs();

                // Check for field structure
                let field_diff = (prev1 - next1).abs();
                let same_field_diff = (prev2 - curr).abs() + (curr - next2).abs();

                if diff1 > 15 && field_diff > same_field_diff {
                    comb_score += diff1 as u64;
                }
                samples += 1;
            }
        }

        if samples == 0 {
            return 0.0;
        }

        comb_score as f64 / samples as f64
    }

    /// Calculate field match quality between two frames.
    fn calculate_field_match(
        &self,
        frame1: &VideoFrame,
        field1: u8,
        frame2: &VideoFrame,
        field2: u8,
    ) -> FieldMatch {
        let mut field_match = FieldMatch::new(0, field1, 0, field2);

        if frame1.planes.is_empty() || frame2.planes.is_empty() {
            return field_match;
        }

        let plane1 = &frame1.planes[0];
        let plane2 = &frame2.planes[0];
        let height = frame1.height.min(frame2.height) as usize;
        let width = frame1.width.min(frame2.width) as usize;

        let mut comb_score = 0u64;
        let mut diff_score = 0u64;
        let mut samples = 0u64;

        // Analyze field match by comparing field lines
        for y in 2..height - 2 {
            let is_field1_line = (y % 2) == field1 as usize;
            let is_field2_line = (y % 2) == field2 as usize;

            if !is_field1_line && !is_field2_line {
                continue;
            }

            let row1 = plane1.row(y);
            let row2 = plane2.row(y);

            for x in 0..width {
                let p1 = row1.get(x).copied().unwrap_or(0) as i32;
                let p2 = row2.get(x).copied().unwrap_or(0) as i32;

                // Field difference
                let diff = (p1 - p2).abs();
                diff_score += diff as u64;

                // Check for combing in the combined result
                if y > 0 && y < height - 1 {
                    let row_prev = if is_field1_line {
                        plane1.row(y - 1)
                    } else {
                        plane2.row(y - 1)
                    };
                    let row_next = if is_field1_line {
                        plane1.row(y + 1)
                    } else {
                        plane2.row(y + 1)
                    };

                    let prev = row_prev.get(x).copied().unwrap_or(0) as i32;
                    let next = row_next.get(x).copied().unwrap_or(0) as i32;
                    let curr = if is_field1_line { p1 } else { p2 };

                    let interp = (prev + next) / 2;
                    let comb = (curr - interp).abs();

                    if comb > 15 {
                        comb_score += comb as u64;
                    }
                }

                samples += 1;
            }
        }

        if samples > 0 {
            field_match.comb_score = comb_score as f64 / samples as f64;
            field_match.diff_score = diff_score as f64 / samples as f64;

            // Match quality: lower comb and diff is better
            // Normalize to 0-1 range where 1 is best
            let comb_norm = 1.0 / (1.0 + field_match.comb_score / 10.0);
            let diff_norm = 1.0 / (1.0 + field_match.diff_score / 50.0);
            field_match.match_quality = (comb_norm * 0.7 + diff_norm * 0.3).clamp(0.0, 1.0);
        }

        field_match
    }

    /// Find the best field match from available frames.
    fn find_best_field_match(&self) -> Option<FieldMatch> {
        if self.frame_buffer.len() < 2 {
            return None;
        }

        let mut best_match: Option<FieldMatch> = None;
        let frames_to_check = match self.config.match_mode {
            MatchMode::TwoWay => 2,
            MatchMode::ThreeWay => 3.min(self.frame_buffer.len()),
            MatchMode::FiveWay => 5.min(self.frame_buffer.len()),
        };

        // Try all field pair combinations
        for i in 0..frames_to_check.saturating_sub(1) {
            for j in (i + 1)..frames_to_check {
                // Try all field combinations
                for field1 in 0..2 {
                    for field2 in 0..2 {
                        let mut field_match = self.calculate_field_match(
                            &self.frame_buffer[i],
                            field1,
                            &self.frame_buffer[j],
                            field2,
                        );
                        field_match.frame1_idx = i;
                        field_match.frame2_idx = j;

                        if let Some(ref current_best) = best_match {
                            if field_match.match_quality > current_best.match_quality {
                                best_match = Some(field_match);
                            }
                        } else {
                            best_match = Some(field_match);
                        }
                    }
                }
            }
        }

        best_match
    }

    /// Detect telecine pattern from match history.
    fn detect_pattern(&mut self) {
        if self.match_history.len() < 10 {
            return;
        }

        // Try to detect 3:2 pattern (most common)
        let pattern_32 = self.check_32_pattern();
        let pattern_22 = self.check_22_pattern();
        let pattern_2332 = self.check_2332_pattern();

        // Select pattern with highest confidence
        let (detected, confidence) = if pattern_32 > pattern_22 && pattern_32 > pattern_2332 {
            (TelecinePattern::Pattern32, pattern_32)
        } else if pattern_22 > pattern_2332 {
            (TelecinePattern::Pattern22, pattern_22)
        } else if pattern_2332 > 0.5 {
            (TelecinePattern::Pattern2332, pattern_2332)
        } else {
            return;
        };

        // Update pattern state
        if detected == self.pattern_state.pattern {
            self.pattern_state.match_streak += 1;
        } else {
            self.pattern_state.pattern = detected;
            self.pattern_state.match_streak = 1;
        }

        self.pattern_state.confidence = confidence;

        // Lock pattern if threshold reached
        if self.config.pattern_lock
            && self.pattern_state.match_streak >= self.config.lock_threshold
            && !self.pattern_state.locked
        {
            self.pattern_state.locked = true;
        }
    }

    /// Check for 3:2 pulldown pattern.
    fn check_32_pattern(&self) -> f64 {
        let window_size = 10;
        let history: Vec<f64> = self
            .match_history
            .iter()
            .rev()
            .take(window_size)
            .copied()
            .collect();

        if history.len() < 5 {
            return 0.0;
        }

        // 3:2 pattern has a repeating 5-frame cycle
        // High-low-high-low-low pattern in match quality
        let mut pattern_matches = 0;
        let mut total_checks = 0;

        for i in 0..history.len().saturating_sub(4) {
            let cycle = &history[i..i + 5];

            // Expected pattern: frame 0 and 2 are duplicate fields (low diff)
            // frames 1, 3, 4 are new fields (higher diff)
            let is_match = cycle[0] < 0.4 && cycle[2] < 0.4 && cycle[1] > 0.5 && cycle[3] > 0.5;

            if is_match {
                pattern_matches += 1;
            }
            total_checks += 1;
        }

        if total_checks > 0 {
            pattern_matches as f64 / total_checks as f64
        } else {
            0.0
        }
    }

    /// Check for 2:2 pulldown pattern.
    fn check_22_pattern(&self) -> f64 {
        let history: Vec<f64> = self.match_history.iter().rev().take(10).copied().collect();

        if history.len() < 4 {
            return 0.0;
        }

        // 2:2 pattern has simple alternating pairs
        let mut consistent_count = 0;
        let mut total = 0;

        for i in 0..history.len().saturating_sub(1) {
            let is_consistent = (history[i] < 0.5 && history[i + 1] < 0.5)
                || (history[i] > 0.5 && history[i + 1] > 0.5);

            if is_consistent {
                consistent_count += 1;
            }
            total += 1;
        }

        if total > 0 {
            consistent_count as f64 / total as f64
        } else {
            0.0
        }
    }

    /// Check for 2:3:3:2 advanced pulldown pattern.
    fn check_2332_pattern(&self) -> f64 {
        let history: Vec<f64> = self.match_history.iter().rev().take(10).copied().collect();

        if history.len() < 10 {
            return 0.0;
        }

        // 2:3:3:2 pattern: 2 frames, 3 frames, 3 frames, 2 frames
        // Pattern signature in match quality
        let pattern = [0.3, 0.3, 0.6, 0.6, 0.6, 0.6, 0.6, 0.6, 0.3, 0.3];
        let mut similarity = 0.0;

        for i in 0..10 {
            let diff = (history[i] - pattern[i]).abs();
            similarity += 1.0 - diff.min(1.0);
        }

        similarity / 10.0
    }

    /// Determine if current frame should be decimated.
    fn should_decimate(&self) -> bool {
        if !self.config.pattern.requires_decimation() {
            return false;
        }

        let cycle_len = self.config.pattern.cycle_length();
        let position = self.pattern_state.cycle_position % cycle_len;

        match self.config.pattern {
            TelecinePattern::Pattern32 => {
                // Drop frame at position 3 or 4 in 5-frame cycle (duplicate field)
                position == 3 || position == 4
            }
            TelecinePattern::Pattern2332 => {
                // Drop frames at positions that have duplicate fields
                matches!(position, 2 | 5)
            }
            _ => false,
        }
    }

    /// Reconstruct a progressive frame from field matches.
    fn reconstruct_frame(&self, field_match: &FieldMatch) -> Option<VideoFrame> {
        if field_match.frame1_idx >= self.frame_buffer.len()
            || field_match.frame2_idx >= self.frame_buffer.len()
        {
            return None;
        }

        let frame1 = &self.frame_buffer[field_match.frame1_idx];
        let frame2 = &self.frame_buffer[field_match.frame2_idx];

        let mut output = VideoFrame::new(frame1.format, frame1.width, frame1.height);
        output.frame_type = frame1.frame_type;
        output.color_info = frame1.color_info;
        output.timestamp = frame1.timestamp;

        // Combine fields from matched frames
        for (plane_idx, _) in frame1.planes.iter().enumerate() {
            let (width, height) = frame1.plane_dimensions(plane_idx);
            let mut dst_data = vec![0u8; (width * height) as usize];

            let plane1 = &frame1.planes[plane_idx];
            let plane2 = frame2.planes.get(plane_idx)?;

            for y in 0..height as usize {
                let use_field1 = (y % 2) == field_match.field1 as usize;

                let src_plane = if use_field1 { plane1 } else { plane2 };
                let src_row = src_plane.row(y);

                for x in 0..width as usize {
                    dst_data[y * width as usize + x] = src_row.get(x).copied().unwrap_or(0);
                }
            }

            output.planes.push(Plane::new(dst_data, width as usize));
        }

        Some(output)
    }

    /// Apply post-processing to remove residual combing.
    fn post_process_frame(&self, frame: &VideoFrame) -> VideoFrame {
        if matches!(self.config.post_process, PostProcessMode::None) {
            return frame.clone();
        }

        let strength = match self.config.post_process {
            PostProcessMode::Light => 0.3,
            PostProcessMode::Medium => 0.6,
            PostProcessMode::Aggressive => 0.9,
            PostProcessMode::None => return frame.clone(),
        };

        let mut output = VideoFrame::new(frame.format, frame.width, frame.height);
        output.timestamp = frame.timestamp;
        output.frame_type = frame.frame_type;
        output.color_info = frame.color_info;

        for (plane_idx, src_plane) in frame.planes.iter().enumerate() {
            let (width, height) = frame.plane_dimensions(plane_idx);
            let mut dst_data = vec![0u8; (width * height) as usize];

            for y in 0..height as usize {
                let curr_row = src_plane.row(y);

                if y == 0 || y == height as usize - 1 {
                    // Copy edge lines
                    for x in 0..width as usize {
                        dst_data[y * width as usize + x] = curr_row.get(x).copied().unwrap_or(0);
                    }
                } else {
                    let prev_row = src_plane.row(y - 1);
                    let next_row = src_plane.row(y + 1);

                    for x in 0..width as usize {
                        let prev = prev_row.get(x).copied().unwrap_or(0) as i32;
                        let curr = curr_row.get(x).copied().unwrap_or(0) as i32;
                        let next = next_row.get(x).copied().unwrap_or(0) as i32;

                        // Detect combing
                        let interp = (prev + next) / 2;
                        let diff = (curr - interp).abs();

                        let result = if diff > 15 {
                            // Apply deinterlacing based on strength
                            let blend =
                                (curr as f64 * (1.0 - strength) + interp as f64 * strength) as i32;
                            blend.clamp(0, 255)
                        } else {
                            curr
                        };

                        dst_data[y * width as usize + x] = result as u8;
                    }
                }
            }

            output.planes.push(Plane::new(dst_data, width as usize));
        }

        output
    }

    /// Handle orphan fields (single fields without matches).
    fn handle_orphan_field(&self, frame: &VideoFrame) -> VideoFrame {
        // Simple deinterlacing for orphan fields
        let mut output = VideoFrame::new(frame.format, frame.width, frame.height);
        output.timestamp = frame.timestamp;
        output.frame_type = frame.frame_type;
        output.color_info = frame.color_info;

        for (plane_idx, src_plane) in frame.planes.iter().enumerate() {
            let (width, height) = frame.plane_dimensions(plane_idx);
            let mut dst_data = vec![0u8; (width * height) as usize];

            for y in 0..height as usize {
                let curr_row = src_plane.row(y);

                if y == 0 || y == height as usize - 1 {
                    for x in 0..width as usize {
                        dst_data[y * width as usize + x] = curr_row.get(x).copied().unwrap_or(0);
                    }
                } else {
                    let prev_row = src_plane.row(y - 1);
                    let next_row = src_plane.row(y + 1);

                    for x in 0..width as usize {
                        let prev = prev_row.get(x).copied().unwrap_or(0) as u16;
                        let curr = curr_row.get(x).copied().unwrap_or(0) as u16;
                        let next = next_row.get(x).copied().unwrap_or(0) as u16;

                        // Blend for smooth interpolation
                        let result = ((prev + curr * 2 + next) / 4) as u8;
                        dst_data[y * width as usize + x] = result;
                    }
                }
            }

            output.planes.push(Plane::new(dst_data, width as usize));
        }

        output
    }

    /// Process incoming frame and produce IVTC output.
    fn process_frame(&mut self, frame: VideoFrame) -> Vec<VideoFrame> {
        // Check for scene change
        if let Some(ref prev) = self.prev_frame {
            if self.detect_scene_change(prev, &frame) {
                self.stats.scene_changes += 1;
                // Reset pattern on scene change
                if !self.pattern_state.locked {
                    self.pattern_state.match_streak = 0;
                    self.pattern_state.cycle_position = 0;
                }
            }
        }
        self.prev_frame = Some(frame.clone());

        // Add frame to buffer
        self.frame_buffer.push_back(frame);

        // Maintain buffer size
        let max_buffer = match self.config.match_mode {
            MatchMode::TwoWay => 5,
            MatchMode::ThreeWay => 7,
            MatchMode::FiveWay => 10,
        };

        while self.frame_buffer.len() > max_buffer {
            self.frame_buffer.pop_front();
        }

        // Need minimum frames for analysis
        let min_frames = match self.config.match_mode {
            MatchMode::TwoWay => 2,
            MatchMode::ThreeWay => 3,
            MatchMode::FiveWay => 5,
        };

        if self.frame_buffer.len() < min_frames {
            return Vec::new();
        }

        let mut output = Vec::new();

        // Find best field match
        if let Some(field_match) = self.find_best_field_match() {
            // Update match history for pattern detection
            self.match_history.push_back(field_match.match_quality);
            if self.match_history.len() > 30 {
                self.match_history.pop_front();
            }

            // Detect pattern if in auto mode
            if matches!(self.config.pattern, TelecinePattern::Auto) {
                self.detect_pattern();
            }

            // Update statistics
            self.stats.frames_processed += 1;
            self.stats.avg_comb_score = (self.stats.avg_comb_score
                * (self.stats.frames_processed - 1) as f64
                + field_match.comb_score)
                / self.stats.frames_processed as f64;
            self.stats.avg_match_quality = (self.stats.avg_match_quality
                * (self.stats.frames_processed - 1) as f64
                + field_match.match_quality)
                / self.stats.frames_processed as f64;

            // Determine if frame should be decimated
            let should_drop = self.should_decimate();
            self.pattern_state.cycle_position += 1;

            if should_drop {
                self.stats.frames_decimated += 1;
            } else {
                // Reconstruct frame from field match
                if let Some(mut reconstructed) = self.reconstruct_frame(&field_match) {
                    // Apply post-processing if enabled
                    if !matches!(self.config.post_process, PostProcessMode::None) {
                        reconstructed = self.post_process_frame(&reconstructed);
                    }

                    // Update timestamp for output frame rate
                    let pts_adjustment = self.stats.frames_decimated as i64;
                    let new_pts = reconstructed.timestamp.pts - pts_adjustment;
                    reconstructed.timestamp =
                        Timestamp::new(new_pts.max(0), reconstructed.timestamp.timebase);

                    output.push(reconstructed);
                }
            }
        } else if self.config.handle_orphans {
            // Handle orphan field
            if let Some(frame) = self.frame_buffer.back() {
                let orphan = self.handle_orphan_field(frame);
                output.push(orphan);
                self.stats.orphans_handled += 1;
            }
        }

        self.output_frame_idx += output.len() as u64;
        output
    }
}

impl Node for IvtcFilter {
    fn id(&self) -> NodeId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> NodeType {
        NodeType::Filter
    }

    fn state(&self) -> NodeState {
        self.state
    }

    fn set_state(&mut self, state: NodeState) -> GraphResult<()> {
        if !self.state.can_transition_to(state) {
            return Err(GraphError::InvalidStateTransition {
                node: self.id,
                from: self.state.to_string(),
                to: state.to_string(),
            });
        }
        self.state = state;
        Ok(())
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn process(&mut self, input: Option<FilterFrame>) -> GraphResult<Option<FilterFrame>> {
        // First, check for pending output from previous processing
        if let Some(frame) = self.pending_output.pop_front() {
            return Ok(Some(FilterFrame::Video(frame)));
        }

        match input {
            Some(FilterFrame::Video(frame)) => {
                let mut output_frames = self.process_frame(frame);

                if output_frames.is_empty() {
                    Ok(None)
                } else {
                    let first = output_frames.remove(0);
                    self.pending_output.extend(output_frames);
                    Ok(Some(FilterFrame::Video(first)))
                }
            }
            Some(_) => Err(GraphError::PortTypeMismatch {
                expected: "Video".to_string(),
                actual: "Audio".to_string(),
            }),
            None => Ok(None),
        }
    }

    fn flush(&mut self) -> GraphResult<Vec<FilterFrame>> {
        let mut output: Vec<FilterFrame> = self
            .pending_output
            .drain(..)
            .map(FilterFrame::Video)
            .collect();

        // Process remaining buffered frames
        while let Some(frame) = self.frame_buffer.pop_front() {
            if self.config.handle_orphans {
                output.push(FilterFrame::Video(self.handle_orphan_field(&frame)));
            } else {
                output.push(FilterFrame::Video(frame));
            }
        }

        Ok(output)
    }

    fn reset(&mut self) -> GraphResult<()> {
        self.frame_buffer.clear();
        self.pending_output.clear();
        self.match_history.clear();
        self.prev_frame = None;
        self.pattern_state = PatternState::default();
        self.stats = MatchStats::default();
        self.output_frame_idx = 0;
        self.set_state(NodeState::Idle)
    }
}

/// Pattern analyzer for offline analysis.
///
/// This analyzer can be used to analyze video content and determine
/// the best IVTC settings without processing the entire video.
#[derive(Debug)]
pub struct PatternAnalyzer {
    /// Frames analyzed.
    frames_analyzed: usize,
    /// Detected patterns and their confidence.
    detected_patterns: Vec<(TelecinePattern, f64)>,
    /// Average comb score.
    avg_comb_score: f64,
    /// Match quality history.
    match_history: Vec<f64>,
}

impl Default for PatternAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternAnalyzer {
    /// Create a new pattern analyzer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            frames_analyzed: 0,
            detected_patterns: Vec::new(),
            avg_comb_score: 0.0,
            match_history: Vec::new(),
        }
    }

    /// Analyze a frame.
    pub fn analyze_frame(&mut self, frame: &VideoFrame) {
        self.frames_analyzed += 1;

        // Calculate comb metric
        let comb_score = self.calculate_comb_metric(frame);
        self.avg_comb_score = (self.avg_comb_score * (self.frames_analyzed - 1) as f64
            + comb_score)
            / self.frames_analyzed as f64;

        self.match_history.push(comb_score);
    }

    /// Calculate combing metric for a frame.
    fn calculate_comb_metric(&self, frame: &VideoFrame) -> f64 {
        if frame.planes.is_empty() {
            return 0.0;
        }

        let plane = &frame.planes[0];
        let height = frame.height as usize;
        let width = frame.width as usize;

        let mut comb_score = 0u64;
        let mut samples = 0u64;

        for y in 1..height - 1 {
            let row_prev = plane.row(y - 1);
            let row_curr = plane.row(y);
            let row_next = plane.row(y + 1);

            for x in 0..width {
                let prev = row_prev.get(x).copied().unwrap_or(0) as i32;
                let curr = row_curr.get(x).copied().unwrap_or(0) as i32;
                let next = row_next.get(x).copied().unwrap_or(0) as i32;

                let interp = (prev + next) / 2;
                let diff = (curr - interp).abs();

                if diff > 15 {
                    comb_score += diff as u64;
                }
                samples += 1;
            }
        }

        if samples > 0 {
            comb_score as f64 / samples as f64
        } else {
            0.0
        }
    }

    /// Get analysis results.
    #[must_use]
    pub fn get_results(&self) -> PatternAnalysisResults {
        PatternAnalysisResults {
            frames_analyzed: self.frames_analyzed,
            avg_comb_score: self.avg_comb_score,
            recommended_pattern: self.recommend_pattern(),
            confidence: self.calculate_confidence(),
        }
    }

    /// Recommend a pattern based on analysis.
    fn recommend_pattern(&self) -> TelecinePattern {
        if self.match_history.len() < 10 {
            return TelecinePattern::Auto;
        }

        // Simple heuristic based on comb score variation
        let variance = self.calculate_variance();

        if variance > 15.0 {
            TelecinePattern::Pattern32
        } else if variance > 5.0 {
            TelecinePattern::Pattern22
        } else {
            TelecinePattern::Auto
        }
    }

    /// Calculate variance in match history.
    fn calculate_variance(&self) -> f64 {
        if self.match_history.is_empty() {
            return 0.0;
        }

        let mean = self.match_history.iter().sum::<f64>() / self.match_history.len() as f64;
        let variance: f64 = self
            .match_history
            .iter()
            .map(|&x| {
                let diff = x - mean;
                diff * diff
            })
            .sum::<f64>()
            / self.match_history.len() as f64;

        variance.sqrt()
    }

    /// Calculate detection confidence.
    fn calculate_confidence(&self) -> f64 {
        if self.frames_analyzed < 30 {
            return 0.0;
        }

        let variance = self.calculate_variance();
        (variance / 20.0).min(1.0)
    }
}

/// Results from pattern analysis.
#[derive(Clone, Debug)]
pub struct PatternAnalysisResults {
    /// Number of frames analyzed.
    pub frames_analyzed: usize,
    /// Average combing score.
    pub avg_comb_score: f64,
    /// Recommended telecine pattern.
    pub recommended_pattern: TelecinePattern,
    /// Detection confidence (0.0-1.0).
    pub confidence: f64,
}

impl PatternAnalysisResults {
    /// Check if content likely has telecine.
    #[must_use]
    pub fn has_telecine(&self) -> bool {
        self.avg_comb_score > 5.0 && self.confidence > 0.5
    }

    /// Get recommended configuration.
    #[must_use]
    pub fn recommended_config(&self) -> IvtcConfig {
        IvtcConfig::new()
            .with_pattern(self.recommended_pattern)
            .with_post_processing(if self.avg_comb_score > 10.0 {
                PostProcessMode::Medium
            } else {
                PostProcessMode::Light
            })
            .with_sensitivity(if self.confidence > 0.8 {
                DetectionSensitivity::Low
            } else if self.confidence > 0.5 {
                DetectionSensitivity::Medium
            } else {
                DetectionSensitivity::High
            })
    }
}

/// Frame rate conversion helpers.
pub mod framerate {
    use oximedia_core::Rational;

    /// Common frame rates.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum StandardFrameRate {
        /// Film rate: 24 fps.
        Film24,
        /// Film rate: 23.976 fps (24000/1001).
        Film23976,
        /// PAL rate: 25 fps.
        Pal25,
        /// NTSC rate: 29.97 fps (30000/1001).
        Ntsc2997,
        /// NTSC rate: 30 fps.
        Ntsc30,
        /// High frame rate: 50 fps.
        High50,
        /// High frame rate: 59.94 fps (60000/1001).
        High5994,
        /// High frame rate: 60 fps.
        High60,
    }

    impl StandardFrameRate {
        /// Get the rational representation.
        #[must_use]
        pub fn as_rational(&self) -> Rational {
            match self {
                Self::Film24 => Rational { num: 24, den: 1 },
                Self::Film23976 => Rational {
                    num: 24000,
                    den: 1001,
                },
                Self::Pal25 => Rational { num: 25, den: 1 },
                Self::Ntsc2997 => Rational {
                    num: 30000,
                    den: 1001,
                },
                Self::Ntsc30 => Rational { num: 30, den: 1 },
                Self::High50 => Rational { num: 50, den: 1 },
                Self::High5994 => Rational {
                    num: 60000,
                    den: 1001,
                },
                Self::High60 => Rational { num: 60, den: 1 },
            }
        }

        /// Get the decimal representation.
        #[must_use]
        pub fn as_f64(&self) -> f64 {
            let r = self.as_rational();
            r.num as f64 / r.den as f64
        }

        /// Detect frame rate from rational.
        #[must_use]
        pub fn from_rational(rate: Rational) -> Option<Self> {
            let fps = rate.num as f64 / rate.den as f64;

            if (fps - 23.976).abs() < 0.01 {
                Some(Self::Film23976)
            } else if (fps - 24.0).abs() < 0.01 {
                Some(Self::Film24)
            } else if (fps - 25.0).abs() < 0.01 {
                Some(Self::Pal25)
            } else if (fps - 29.97).abs() < 0.01 {
                Some(Self::Ntsc2997)
            } else if (fps - 30.0).abs() < 0.01 {
                Some(Self::Ntsc30)
            } else if (fps - 50.0).abs() < 0.01 {
                Some(Self::High50)
            } else if (fps - 59.94).abs() < 0.01 {
                Some(Self::High5994)
            } else if (fps - 60.0).abs() < 0.01 {
                Some(Self::High60)
            } else {
                None
            }
        }
    }

    /// Calculate target frame rate after IVTC.
    #[must_use]
    pub fn calculate_ivtc_framerate(
        source_rate: Rational,
        pattern: super::TelecinePattern,
    ) -> Rational {
        let cycle_len = pattern.cycle_length();
        let output_frames = pattern.output_frames();

        if cycle_len == 0 || output_frames == 0 {
            return source_rate;
        }

        // Calculate decimation ratio
        let ratio = output_frames as i64 / cycle_len as i64;

        Rational {
            num: source_rate.num * ratio,
            den: source_rate.den,
        }
    }

    /// Detect telecine pattern from frame rate ratio.
    #[must_use]
    pub fn detect_pattern_from_rates(
        source: StandardFrameRate,
        expected_output: StandardFrameRate,
    ) -> Option<super::TelecinePattern> {
        let src_fps = source.as_f64();
        let out_fps = expected_output.as_f64();
        let ratio = src_fps / out_fps;

        // 3:2 pulldown: 29.97 -> 23.976 (ratio ~1.25)
        if (ratio - 1.25).abs() < 0.01 {
            Some(super::TelecinePattern::Pattern32)
        }
        // 2:2 pulldown: 25 -> 25 (ratio 1.0)
        else if (ratio - 1.0).abs() < 0.01 {
            Some(super::TelecinePattern::Pattern22)
        }
        // Euro pulldown: 25 -> 24 (ratio ~1.04)
        else if (ratio - 1.04167).abs() < 0.01 {
            Some(super::TelecinePattern::EuroPulldown)
        } else {
            None
        }
    }
}

/// Field metrics for detailed analysis.
#[derive(Clone, Debug, Default)]
pub struct FieldMetrics {
    /// Top field comb score.
    pub top_field_comb: f64,
    /// Bottom field comb score.
    pub bottom_field_comb: f64,
    /// Interlaced score (higher = more interlaced).
    pub interlace_score: f64,
    /// Progressive score (higher = more progressive).
    pub progressive_score: f64,
    /// Motion score (amount of motion detected).
    pub motion_score: f64,
    /// Spatial complexity.
    pub spatial_complexity: f64,
}

impl FieldMetrics {
    /// Calculate comprehensive field metrics for a frame.
    #[must_use]
    pub fn calculate(frame: &VideoFrame) -> Self {
        let mut metrics = Self::default();

        if frame.planes.is_empty() {
            return metrics;
        }

        let plane = &frame.planes[0];
        let height = frame.height as usize;
        let width = frame.width as usize;

        let mut top_comb = 0u64;
        let mut bottom_comb = 0u64;
        let mut interlace = 0u64;
        let mut progressive = 0u64;
        let mut spatial = 0u64;
        let mut samples = 0u64;

        for y in 2..height - 2 {
            let row_m2 = plane.row(y - 2);
            let row_m1 = plane.row(y - 1);
            let row_0 = plane.row(y);
            let row_p1 = plane.row(y + 1);
            let row_p2 = plane.row(y + 2);

            for x in 0..width {
                let m2 = row_m2.get(x).copied().unwrap_or(0) as i32;
                let m1 = row_m1.get(x).copied().unwrap_or(0) as i32;
                let c = row_0.get(x).copied().unwrap_or(0) as i32;
                let p1 = row_p1.get(x).copied().unwrap_or(0) as i32;
                let p2 = row_p2.get(x).copied().unwrap_or(0) as i32;

                // Field combing
                let interp = (m1 + p1) / 2;
                let comb = (c - interp).abs();

                if y % 2 == 0 {
                    top_comb += comb as u64;
                } else {
                    bottom_comb += comb as u64;
                }

                // Interlace vs progressive detection
                let field_diff = (m1 - p1).abs();
                let same_field_diff = (m2 - c).abs() + (c - p2).abs();

                if field_diff > same_field_diff + 10 {
                    interlace += field_diff as u64;
                } else {
                    progressive += same_field_diff as u64;
                }

                // Spatial complexity
                let grad_h =
                    (c - row_0.get(x.saturating_sub(1)).copied().unwrap_or(0) as i32).abs();
                let grad_v = (c - m1).abs();
                spatial += (grad_h + grad_v) as u64;

                samples += 1;
            }
        }

        if samples > 0 {
            metrics.top_field_comb = top_comb as f64 / samples as f64;
            metrics.bottom_field_comb = bottom_comb as f64 / samples as f64;
            metrics.interlace_score = interlace as f64 / samples as f64;
            metrics.progressive_score = progressive as f64 / samples as f64;
            metrics.spatial_complexity = spatial as f64 / samples as f64;
        }

        metrics
    }

    /// Check if frame is likely interlaced.
    #[must_use]
    pub fn is_interlaced(&self) -> bool {
        self.interlace_score > self.progressive_score * 1.5
    }

    /// Get the preferred field (0 = top, 1 = bottom).
    #[must_use]
    pub fn preferred_field(&self) -> u8 {
        if self.top_field_comb < self.bottom_field_comb {
            0
        } else {
            1
        }
    }

    /// Get overall quality score (0.0-1.0, higher is better).
    #[must_use]
    pub fn quality_score(&self) -> f64 {
        let comb_score = 1.0 / (1.0 + (self.top_field_comb + self.bottom_field_comb) / 2.0);
        let prog_score =
            self.progressive_score / (self.progressive_score + self.interlace_score + 1.0);
        (comb_score * 0.6 + prog_score * 0.4).clamp(0.0, 1.0)
    }
}

/// Cadence detector for pattern locking.
#[derive(Debug)]
pub struct CadenceDetector {
    /// Pattern history.
    history: VecDeque<u8>,
    /// Current detected cadence.
    cadence: Option<Vec<u8>>,
    /// Confidence level.
    confidence: f64,
    /// Number of consecutive matches.
    match_count: usize,
}

impl Default for CadenceDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl CadenceDetector {
    /// Create a new cadence detector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(30),
            cadence: None,
            confidence: 0.0,
            match_count: 0,
        }
    }

    /// Add a frame match result.
    pub fn add_match(&mut self, is_duplicate: bool) {
        let value = if is_duplicate { 1 } else { 0 };
        self.history.push_back(value);

        if self.history.len() > 30 {
            self.history.pop_front();
        }

        self.detect_cadence();
    }

    /// Detect repeating cadence pattern.
    fn detect_cadence(&mut self) {
        if self.history.len() < 10 {
            return;
        }

        let history: Vec<u8> = self.history.iter().copied().collect();

        // Try different pattern lengths
        for pattern_len in 2..=10 {
            if let Some(pattern) = self.check_pattern(&history, pattern_len) {
                if self.cadence.as_ref() == Some(&pattern) {
                    self.match_count += 1;
                } else {
                    self.cadence = Some(pattern);
                    self.match_count = 1;
                }

                self.confidence = (self.match_count as f64 / 10.0).min(1.0);
                return;
            }
        }
    }

    /// Check if history matches a pattern of given length.
    fn check_pattern(&self, history: &[u8], pattern_len: usize) -> Option<Vec<u8>> {
        if history.len() < pattern_len * 2 {
            return None;
        }

        let pattern: Vec<u8> = history[..pattern_len].to_vec();
        let mut matches = 0;
        let mut total = 0;

        for i in 0..history.len() - pattern_len {
            let window = &history[i..i + pattern_len];
            if window == pattern.as_slice() {
                matches += 1;
            }
            total += 1;
        }

        if matches as f64 / total as f64 > 0.7 {
            Some(pattern)
        } else {
            None
        }
    }

    /// Get detected cadence.
    #[must_use]
    pub fn cadence(&self) -> Option<&[u8]> {
        self.cadence.as_deref()
    }

    /// Get confidence level.
    #[must_use]
    pub fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Check if cadence is locked.
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.confidence > 0.8 && self.match_count >= 5
    }

    /// Reset detector.
    pub fn reset(&mut self) {
        self.history.clear();
        self.cadence = None;
        self.confidence = 0.0;
        self.match_count = 0;
    }
}

/// Motion compensation for better field matching.
#[derive(Debug)]
pub struct MotionCompensation {
    /// Enable motion compensation.
    enabled: bool,
    /// Search range in pixels.
    search_range: i32,
}

impl Default for MotionCompensation {
    fn default() -> Self {
        Self::new()
    }
}

impl MotionCompensation {
    /// Create new motion compensation.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled: true,
            search_range: 8,
        }
    }

    /// Set search range.
    #[must_use]
    pub fn with_search_range(mut self, range: i32) -> Self {
        self.search_range = range;
        self
    }

    /// Calculate motion vector between two frames.
    #[must_use]
    pub fn calculate_motion_vector(
        &self,
        frame1: &VideoFrame,
        frame2: &VideoFrame,
        x: usize,
        y: usize,
    ) -> (i32, i32) {
        if !self.enabled || frame1.planes.is_empty() || frame2.planes.is_empty() {
            return (0, 0);
        }

        let plane1 = &frame1.planes[0];
        let plane2 = &frame2.planes[0];
        let height = frame1.height as usize;
        let width = frame1.width as usize;

        let block_size = 8;
        let mut best_mv = (0, 0);
        let mut best_sad = u64::MAX;

        for dy in -self.search_range..=self.search_range {
            for dx in -self.search_range..=self.search_range {
                let mut sad = 0u64;
                let mut samples = 0;

                for by in 0..block_size {
                    for bx in 0..block_size {
                        let y1 = y + by;
                        let x1 = x + bx;
                        let y2 = (y as i32 + by as i32 + dy).clamp(0, height as i32 - 1) as usize;
                        let x2 = (x as i32 + bx as i32 + dx).clamp(0, width as i32 - 1) as usize;

                        if y1 < height && x1 < width {
                            let p1 = plane1.row(y1).get(x1).copied().unwrap_or(0) as i32;
                            let p2 = plane2.row(y2).get(x2).copied().unwrap_or(0) as i32;
                            sad += (p1 - p2).unsigned_abs() as u64;
                            samples += 1;
                        }
                    }
                }

                if samples > 0 && sad < best_sad {
                    best_sad = sad;
                    best_mv = (dx, dy);
                }
            }
        }

        best_mv
    }

    /// Apply motion compensation to improve field matching.
    #[must_use]
    pub fn compensate_field(
        &self,
        source: &VideoFrame,
        _reference: &VideoFrame,
        motion_vector: (i32, i32),
    ) -> VideoFrame {
        if !self.enabled {
            return source.clone();
        }

        let mut output = VideoFrame::new(source.format, source.width, source.height);
        output.timestamp = source.timestamp;
        output.frame_type = source.frame_type;
        output.color_info = source.color_info;

        for (plane_idx, src_plane) in source.planes.iter().enumerate() {
            let (width, height) = source.plane_dimensions(plane_idx);
            let mut dst_data = vec![0u8; (width * height) as usize];

            let (mvx, mvy) = motion_vector;

            for y in 0..height as usize {
                for x in 0..width as usize {
                    let src_y = (y as i32 + mvy).clamp(0, height as i32 - 1) as usize;
                    let src_x = (x as i32 + mvx).clamp(0, width as i32 - 1) as usize;

                    let pixel = src_plane.row(src_y).get(src_x).copied().unwrap_or(0);
                    dst_data[y * width as usize + x] = pixel;
                }
            }

            output.planes.push(Plane::new(dst_data, width as usize));
        }

        output
    }
}

/// Debug utilities for IVTC analysis.
pub mod debug {
    use super::*;

    /// Generate a visual representation of field combing.
    #[must_use]
    pub fn visualize_combing(frame: &VideoFrame) -> Option<VideoFrame> {
        if frame.planes.is_empty() {
            return None;
        }

        let mut output = VideoFrame::new(frame.format, frame.width, frame.height);
        output.timestamp = frame.timestamp;
        output.frame_type = frame.frame_type;
        output.color_info = frame.color_info;

        let plane = &frame.planes[0];
        let height = frame.height as usize;
        let width = frame.width as usize;

        let mut dst_data = vec![0u8; (width * height) as usize];

        for y in 1..height - 1 {
            let row_prev = plane.row(y - 1);
            let row_curr = plane.row(y);
            let row_next = plane.row(y + 1);

            for x in 0..width {
                let prev = row_prev.get(x).copied().unwrap_or(0) as i32;
                let curr = row_curr.get(x).copied().unwrap_or(0) as i32;
                let next = row_next.get(x).copied().unwrap_or(0) as i32;

                let interp = (prev + next) / 2;
                let diff = (curr - interp).abs();

                // Highlight combing artifacts
                let vis = if diff > 15 { 255 } else { (diff * 10).min(255) };

                dst_data[y * width + x] = vis as u8;
            }
        }

        output.planes.push(Plane::new(dst_data, width));
        Some(output)
    }

    /// Generate field separation visualization.
    #[must_use]
    pub fn visualize_fields(frame: &VideoFrame, field: u8) -> Option<VideoFrame> {
        if frame.planes.is_empty() {
            return None;
        }

        let mut output = VideoFrame::new(frame.format, frame.width, frame.height);
        output.timestamp = frame.timestamp;
        output.frame_type = frame.frame_type;
        output.color_info = frame.color_info;

        let plane = &frame.planes[0];
        let height = frame.height as usize;
        let width = frame.width as usize;

        let mut dst_data = vec![0u8; (width * height) as usize];

        for y in 0..height {
            let row = plane.row(y);

            for x in 0..width {
                let pixel = if (y % 2) == field as usize {
                    row.get(x).copied().unwrap_or(0)
                } else {
                    128 // Gray for opposite field
                };

                dst_data[y * width + x] = pixel;
            }
        }

        output.planes.push(Plane::new(dst_data, width));
        Some(output)
    }

    /// Calculate detailed statistics for debugging.
    #[must_use]
    pub fn calculate_debug_stats(frame: &VideoFrame) -> DebugStats {
        let metrics = FieldMetrics::calculate(frame);

        DebugStats {
            width: frame.width,
            height: frame.height,
            field_metrics: metrics,
            plane_count: frame.planes.len(),
        }
    }

    /// Debug statistics.
    #[derive(Clone, Debug)]
    pub struct DebugStats {
        /// Frame width.
        pub width: u32,
        /// Frame height.
        pub height: u32,
        /// Field metrics.
        pub field_metrics: FieldMetrics,
        /// Number of planes.
        pub plane_count: usize,
    }
}
