//! Lookahead buffer for frame analysis.
//!
//! The lookahead system buffers future frames to enable:
//! - Scene change detection
//! - Mini-GOP structure optimization
//! - Adaptive B-frame decisions
//! - Complexity pre-analysis for better bit allocation

#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![forbid(unsafe_code)]

use crate::frame::FrameType;

use super::complexity::{ComplexityEstimator, ComplexityResult};
use super::types::RcConfig;

/// Lookahead buffer for frame pre-analysis.
#[derive(Clone, Debug)]
pub struct Lookahead {
    /// Maximum lookahead depth.
    depth: usize,
    /// Buffered frame info.
    frames: Vec<LookaheadFrame>,
    /// Scene cut threshold.
    scene_cut_threshold: f32,
    /// Complexity estimator.
    complexity_estimator: ComplexityEstimator,
    /// Frame width (reserved for frame analysis).
    #[allow(dead_code)]
    width: u32,
    /// Frame height (reserved for frame analysis).
    #[allow(dead_code)]
    height: u32,
    /// Enable adaptive B-frames.
    adaptive_b_frames: bool,
    /// Maximum consecutive B-frames.
    max_b_frames: u32,
    /// GOP length.
    gop_length: u32,
    /// Frames since last keyframe.
    frames_since_keyframe: u32,
    /// Total frames processed.
    frame_count: u64,
}

/// Information about a frame in the lookahead buffer.
#[derive(Clone, Debug)]
pub struct LookaheadFrame {
    /// Frame index (presentation order).
    pub frame_index: u64,
    /// Complexity analysis result.
    pub complexity: ComplexityResult,
    /// Detected as scene cut.
    pub is_scene_cut: bool,
    /// Determined frame type.
    pub frame_type: FrameType,
    /// Cost to encode as P-frame.
    pub p_cost: f32,
    /// Cost to encode as B-frame.
    pub b_cost: f32,
    /// Suggested target bits.
    pub suggested_bits: u64,
}

impl LookaheadFrame {
    /// Create a new lookahead frame entry.
    #[must_use]
    pub fn new(frame_index: u64) -> Self {
        Self {
            frame_index,
            complexity: ComplexityResult::default_complexity(),
            is_scene_cut: false,
            frame_type: FrameType::Inter,
            p_cost: 0.0,
            b_cost: 0.0,
            suggested_bits: 0,
        }
    }
}

impl Lookahead {
    /// Create a new lookahead buffer from configuration.
    #[must_use]
    pub fn new(config: &RcConfig, width: u32, height: u32) -> Self {
        Self {
            depth: config.lookahead_depth,
            frames: Vec::with_capacity(config.lookahead_depth),
            scene_cut_threshold: config.scene_cut_threshold,
            complexity_estimator: ComplexityEstimator::new(width, height),
            width,
            height,
            adaptive_b_frames: true,
            max_b_frames: 3,
            gop_length: config.gop_length,
            frames_since_keyframe: 0,
            frame_count: 0,
        }
    }

    /// Create with specific parameters.
    #[must_use]
    pub fn with_params(depth: usize, width: u32, height: u32) -> Self {
        Self {
            depth,
            frames: Vec::with_capacity(depth),
            scene_cut_threshold: 0.4,
            complexity_estimator: ComplexityEstimator::new(width, height),
            width,
            height,
            adaptive_b_frames: true,
            max_b_frames: 3,
            gop_length: 250,
            frames_since_keyframe: 0,
            frame_count: 0,
        }
    }

    /// Set scene cut threshold.
    pub fn set_scene_cut_threshold(&mut self, threshold: f32) {
        self.scene_cut_threshold = threshold.clamp(0.0, 1.0);
    }

    /// Enable or disable adaptive B-frames.
    pub fn set_adaptive_b_frames(&mut self, enable: bool) {
        self.adaptive_b_frames = enable;
    }

    /// Set maximum consecutive B-frames.
    pub fn set_max_b_frames(&mut self, max: u32) {
        self.max_b_frames = max;
    }

    /// Push a new frame into the lookahead buffer.
    ///
    /// Returns an analyzed frame if the buffer was full.
    pub fn push_frame(&mut self, luma: &[u8], stride: usize) -> Option<LookaheadFrame> {
        // Analyze complexity
        let complexity = self.complexity_estimator.estimate(luma, stride);

        let mut frame = LookaheadFrame::new(self.frame_count);
        frame.complexity = complexity;

        // Detect scene cut
        frame.is_scene_cut = self.detect_scene_cut(&complexity);

        self.frame_count += 1;
        self.frames.push(frame);

        // If buffer is full, analyze and return the oldest frame
        if self.frames.len() > self.depth {
            self.analyze_mini_gop();
            return self.frames.drain(..1).next();
        }

        None
    }

    /// Push frame with pre-computed complexity.
    pub fn push_frame_with_complexity(
        &mut self,
        complexity: ComplexityResult,
    ) -> Option<LookaheadFrame> {
        let mut frame = LookaheadFrame::new(self.frame_count);
        frame.complexity = complexity;
        frame.is_scene_cut = self.detect_scene_cut(&complexity);

        self.frame_count += 1;
        self.frames.push(frame);

        if self.frames.len() > self.depth {
            self.analyze_mini_gop();
            return self.frames.drain(..1).next();
        }

        None
    }

    /// Detect if frame is a scene cut.
    fn detect_scene_cut(&self, complexity: &ComplexityResult) -> bool {
        // Scene cuts typically have high temporal complexity
        // and normalized complexity significantly above average
        complexity.temporal
            > self.complexity_estimator.avg_temporal() * (1.0 + self.scene_cut_threshold)
            || complexity.normalized > (1.0 + self.scene_cut_threshold * 2.0)
    }

    /// Analyze frames in buffer and determine mini-GOP structure.
    fn analyze_mini_gop(&mut self) {
        if self.frames.is_empty() {
            return;
        }

        // Find scene cuts
        let scene_cut_indices: Vec<usize> = self
            .frames
            .iter()
            .enumerate()
            .filter(|(_, f)| f.is_scene_cut)
            .map(|(i, _)| i)
            .collect();

        // Determine frame types
        self.determine_frame_types(&scene_cut_indices);
    }

    /// Determine frame types based on analysis.
    fn determine_frame_types(&mut self, scene_cuts: &[usize]) {
        let mut consecutive_b = 0u32;

        // Cache values needed for B-frame decision
        let avg_temporal = self.complexity_estimator.avg_temporal();
        let adaptive_b = self.adaptive_b_frames;
        let max_b = self.max_b_frames;
        let gop_len = self.gop_length;

        let frame_count = self.frames.len();
        for i in 0..frame_count {
            let is_scene_cut = self.frames[i].is_scene_cut;
            let complexity = self.frames[i].complexity;

            // Check if this should be a keyframe
            let is_keyframe = i == 0 && self.frames_since_keyframe == 0
                || is_scene_cut
                || self.frames_since_keyframe >= gop_len;

            if is_keyframe {
                self.frames[i].frame_type = FrameType::Key;
                self.frames_since_keyframe = 0;
                consecutive_b = 0;
                continue;
            }

            // Check if this is near a scene cut (should be P-frame)
            let near_scene_cut = scene_cuts.iter().any(|&sc| {
                let diff = i.abs_diff(sc);
                diff <= 2
            });

            if near_scene_cut || !adaptive_b {
                self.frames[i].frame_type = FrameType::Inter;
                consecutive_b = 0;
            } else if consecutive_b < max_b
                && Self::should_be_b_frame_static(&complexity, avg_temporal)
            {
                self.frames[i].frame_type = FrameType::BiDir;
                consecutive_b += 1;
            } else {
                self.frames[i].frame_type = FrameType::Inter;
                consecutive_b = 0;
            }

            self.frames_since_keyframe += 1;
        }
    }

    /// Determine if a frame should be a B-frame (static method to avoid borrow issues).
    fn should_be_b_frame_static(complexity: &ComplexityResult, avg_temporal: f32) -> bool {
        // Low complexity frames benefit from being B-frames
        // High motion frames are better as P-frames
        complexity.normalized < 1.2 && complexity.temporal < avg_temporal * 1.3
    }

    /// Flush remaining frames from buffer.
    pub fn flush(&mut self) -> Vec<LookaheadFrame> {
        self.analyze_mini_gop();
        self.frames.drain(..).collect()
    }

    /// Get the number of frames in buffer.
    #[must_use]
    pub fn buffered_count(&self) -> usize {
        self.frames.len()
    }

    /// Check if buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Check if buffer is full.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.frames.len() >= self.depth
    }

    /// Get lookahead depth.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Peek at frames in buffer (read-only).
    #[must_use]
    pub fn peek(&self) -> &[LookaheadFrame] {
        &self.frames
    }

    /// Get average complexity of buffered frames.
    #[must_use]
    pub fn average_complexity(&self) -> f32 {
        if self.frames.is_empty() {
            return 1.0;
        }

        let sum: f32 = self.frames.iter().map(|f| f.complexity.combined).sum();
        sum / self.frames.len() as f32
    }

    /// Get count of detected scene cuts in buffer.
    #[must_use]
    pub fn scene_cut_count(&self) -> usize {
        self.frames.iter().filter(|f| f.is_scene_cut).count()
    }

    /// Reset the lookahead buffer.
    pub fn reset(&mut self) {
        self.frames.clear();
        self.complexity_estimator.reset();
        self.frames_since_keyframe = 0;
        self.frame_count = 0;
    }

    /// Get frame count.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

impl Default for Lookahead {
    fn default() -> Self {
        Self::with_params(40, 1920, 1080)
    }
}

/// Mini-GOP structure information.
#[derive(Clone, Debug, Default)]
pub struct MiniGopInfo {
    /// Starting frame index.
    pub start_frame: u64,
    /// Number of frames in mini-GOP.
    pub frame_count: u32,
    /// Number of I-frames.
    pub i_frames: u32,
    /// Number of P-frames.
    pub p_frames: u32,
    /// Number of B-frames.
    pub b_frames: u32,
    /// Total complexity.
    pub total_complexity: f32,
    /// Contains a scene cut.
    pub has_scene_cut: bool,
}

impl MiniGopInfo {
    /// Calculate suggested bit allocation for this mini-GOP.
    #[must_use]
    pub fn suggested_bits(&self, target_bits_per_frame: u64) -> u64 {
        let base = target_bits_per_frame * self.frame_count as u64;

        // Adjust based on frame type distribution and complexity
        let i_overhead = self.i_frames as f64 * 2.0; // I-frames use more bits
        let b_savings = self.b_frames as f64 * 0.3; // B-frames use fewer bits

        let adjustment = 1.0 + (i_overhead - b_savings) / self.frame_count as f64;
        let complexity_factor = self.total_complexity / self.frame_count as f32;

        (base as f64 * adjustment * complexity_factor as f64) as u64
    }

    /// Get average complexity per frame.
    #[must_use]
    pub fn average_complexity(&self) -> f32 {
        if self.frame_count == 0 {
            return 1.0;
        }
        self.total_complexity / self.frame_count as f32
    }
}

/// Scene change detector.
#[derive(Clone, Debug)]
pub struct SceneChangeDetector {
    /// Detection threshold.
    threshold: f32,
    /// Minimum frames between scene changes.
    min_interval: u32,
    /// Frames since last scene change.
    frames_since_change: u32,
    /// Previous frame complexity.
    prev_complexity: f32,
    /// Running average complexity.
    avg_complexity: f32,
    /// Total scene changes detected.
    total_changes: u64,
}

impl SceneChangeDetector {
    /// Create a new scene change detector.
    #[must_use]
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold: threshold.clamp(0.0, 1.0),
            min_interval: 5,
            frames_since_change: 0,
            prev_complexity: 1.0,
            avg_complexity: 1.0,
            total_changes: 0,
        }
    }

    /// Set minimum interval between scene changes.
    pub fn set_min_interval(&mut self, interval: u32) {
        self.min_interval = interval;
    }

    /// Detect scene change from complexity.
    #[must_use]
    pub fn detect(&mut self, complexity: &ComplexityResult) -> bool {
        let is_scene_change = self.is_scene_change(complexity);

        // Update state
        if is_scene_change {
            self.frames_since_change = 0;
            self.total_changes += 1;
        } else {
            self.frames_since_change += 1;
        }

        // Update running average
        self.avg_complexity = self.avg_complexity * 0.9 + complexity.combined * 0.1;
        self.prev_complexity = complexity.combined;

        is_scene_change
    }

    /// Internal scene change detection logic.
    fn is_scene_change(&self, complexity: &ComplexityResult) -> bool {
        // Don't detect scene changes too frequently
        if self.frames_since_change < self.min_interval {
            return false;
        }

        // Check for sudden complexity increase
        let complexity_ratio = complexity.combined / self.prev_complexity;
        let avg_ratio = complexity.combined / self.avg_complexity;

        // Scene change if complexity spikes significantly
        complexity_ratio > (1.0 + self.threshold) || avg_ratio > (1.0 + self.threshold * 1.5)
    }

    /// Get total scene changes detected.
    #[must_use]
    pub fn total_changes(&self) -> u64 {
        self.total_changes
    }

    /// Reset the detector.
    pub fn reset(&mut self) {
        self.frames_since_change = 0;
        self.prev_complexity = 1.0;
        self.avg_complexity = 1.0;
        self.total_changes = 0;
    }
}

impl Default for SceneChangeDetector {
    fn default() -> Self {
        Self::new(0.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_frame(width: u32, height: u32) -> Vec<u8> {
        vec![128u8; (width * height) as usize]
    }

    #[test]
    fn test_lookahead_creation() {
        let lookahead = Lookahead::with_params(40, 1920, 1080);
        assert_eq!(lookahead.depth(), 40);
        assert!(lookahead.is_empty());
    }

    #[test]
    fn test_push_frame() {
        let mut lookahead = Lookahead::with_params(10, 64, 64);
        let frame = create_test_frame(64, 64);

        for _ in 0..5 {
            let result = lookahead.push_frame(&frame, 64);
            assert!(result.is_none()); // Buffer not full yet
        }

        assert_eq!(lookahead.buffered_count(), 5);
    }

    #[test]
    fn test_buffer_full() {
        let mut lookahead = Lookahead::with_params(5, 64, 64);
        let frame = create_test_frame(64, 64);

        for _ in 0..5 {
            let _ = lookahead.push_frame(&frame, 64);
        }

        assert!(lookahead.is_full());

        // Next push should return a frame
        let result = lookahead.push_frame(&frame, 64);
        assert!(result.is_some());
    }

    #[test]
    fn test_flush() {
        let mut lookahead = Lookahead::with_params(10, 64, 64);
        let frame = create_test_frame(64, 64);

        for _ in 0..5 {
            let _ = lookahead.push_frame(&frame, 64);
        }

        let flushed = lookahead.flush();
        assert_eq!(flushed.len(), 5);
        assert!(lookahead.is_empty());
    }

    #[test]
    fn test_push_with_complexity() {
        let mut lookahead = Lookahead::with_params(10, 64, 64);

        let complexity = ComplexityResult {
            spatial: 1.0,
            temporal: 1.0,
            combined: 1.0,
            normalized: 1.0,
        };

        for _ in 0..5 {
            let _ = lookahead.push_frame_with_complexity(complexity);
        }

        assert_eq!(lookahead.buffered_count(), 5);
    }

    #[test]
    fn test_scene_cut_detection() {
        let mut detector = SceneChangeDetector::new(0.3);
        detector.set_min_interval(0); // Allow immediate detection

        let normal = ComplexityResult {
            spatial: 1.0,
            temporal: 1.0,
            combined: 1.0,
            normalized: 1.0,
        };

        let scene_cut = ComplexityResult {
            spatial: 3.0,
            temporal: 3.0,
            combined: 3.0,
            normalized: 3.0,
        };

        // First frame establishes baseline
        assert!(!detector.detect(&normal));

        // Normal frame
        assert!(!detector.detect(&normal));

        // Scene cut - significant complexity increase
        assert!(detector.detect(&scene_cut));
    }

    #[test]
    fn test_scene_cut_min_interval() {
        let mut detector = SceneChangeDetector::new(0.3);
        detector.set_min_interval(5);

        let normal = ComplexityResult::default_complexity();
        let scene_cut = ComplexityResult {
            spatial: 5.0,
            temporal: 5.0,
            combined: 5.0,
            normalized: 5.0,
        };

        // Trigger a scene change
        for _ in 0..6 {
            let _ = detector.detect(&normal);
        }
        assert!(detector.detect(&scene_cut));

        // Immediate next frame should not be detected even with high complexity
        assert!(!detector.detect(&scene_cut));
    }

    #[test]
    fn test_lookahead_frame() {
        let frame = LookaheadFrame::new(42);
        assert_eq!(frame.frame_index, 42);
        assert_eq!(frame.frame_type, FrameType::Inter);
        assert!(!frame.is_scene_cut);
    }

    #[test]
    fn test_mini_gop_info() {
        let info = MiniGopInfo {
            start_frame: 0,
            frame_count: 15,
            i_frames: 1,
            p_frames: 4,
            b_frames: 10,
            total_complexity: 15.0,
            has_scene_cut: false,
        };

        assert!((info.average_complexity() - 1.0).abs() < f32::EPSILON);

        let bits = info.suggested_bits(100_000);
        assert!(bits > 0);
    }

    #[test]
    fn test_lookahead_reset() {
        let mut lookahead = Lookahead::with_params(10, 64, 64);
        let frame = create_test_frame(64, 64);

        for _ in 0..5 {
            let _ = lookahead.push_frame(&frame, 64);
        }

        lookahead.reset();

        assert!(lookahead.is_empty());
        assert_eq!(lookahead.frame_count(), 0);
    }

    #[test]
    fn test_average_complexity() {
        let mut lookahead = Lookahead::with_params(10, 64, 64);

        let complexity = ComplexityResult {
            spatial: 1.0,
            temporal: 1.0,
            combined: 2.0,
            normalized: 1.0,
        };

        for _ in 0..5 {
            let _ = lookahead.push_frame_with_complexity(complexity);
        }

        assert!((lookahead.average_complexity() - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_scene_change_detector_reset() {
        let mut detector = SceneChangeDetector::new(0.3);

        let complexity = ComplexityResult::default_complexity();
        for _ in 0..10 {
            let _ = detector.detect(&complexity);
        }

        detector.reset();
        assert_eq!(detector.total_changes(), 0);
    }
}
