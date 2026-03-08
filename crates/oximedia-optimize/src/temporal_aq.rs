#![allow(dead_code)]
//! Temporal adaptive quantization for video encoding.
//!
//! This module extends standard spatial AQ by incorporating temporal information
//! (motion activity, scene changes, fade detection) to dynamically adjust QP values
//! across frames. It helps allocate more bits to high-motion or visually complex
//! temporal segments while saving bits on static or slowly-changing regions.

/// Number of frames kept in the sliding temporal window.
const DEFAULT_WINDOW_SIZE: usize = 16;

/// Per-frame temporal activity metrics.
#[derive(Debug, Clone)]
pub struct TemporalActivity {
    /// Frame index in decode order.
    pub frame_index: u64,
    /// Average absolute motion vector magnitude.
    pub avg_motion_magnitude: f64,
    /// Percentage of macroblocks with significant motion (0.0-1.0).
    pub motion_coverage: f64,
    /// Frame-level SAD (Sum of Absolute Differences) vs. previous frame.
    pub frame_sad: f64,
    /// Whether this frame is detected as a scene change.
    pub is_scene_change: bool,
    /// Fade strength (0.0 = no fade, 1.0 = full fade).
    pub fade_strength: f64,
}

impl TemporalActivity {
    /// Creates a new temporal activity measurement.
    pub fn new(frame_index: u64) -> Self {
        Self {
            frame_index,
            avg_motion_magnitude: 0.0,
            motion_coverage: 0.0,
            frame_sad: 0.0,
            is_scene_change: false,
            fade_strength: 0.0,
        }
    }

    /// Returns a composite temporal complexity score in [0.0, 1.0].
    #[allow(clippy::cast_precision_loss)]
    pub fn complexity_score(&self) -> f64 {
        let motion_factor = (self.avg_motion_magnitude / 32.0).min(1.0);
        let coverage_factor = self.motion_coverage;
        let fade_factor = self.fade_strength * 0.5;
        ((motion_factor * 0.5 + coverage_factor * 0.3 + fade_factor * 0.2) as f64).min(1.0)
    }
}

/// Configuration for the temporal AQ engine.
#[derive(Debug, Clone)]
pub struct TemporalAqConfig {
    /// Number of frames in the sliding analysis window.
    pub window_size: usize,
    /// Maximum QP delta that can be applied temporally.
    pub max_qp_delta: i8,
    /// Sensitivity to motion changes (0.0-2.0, 1.0 = normal).
    pub motion_sensitivity: f64,
    /// SAD threshold for scene change detection.
    pub scene_change_threshold: f64,
    /// Fade detection sensitivity.
    pub fade_sensitivity: f64,
    /// Strength of temporal smoothing (0.0 = no smoothing, 1.0 = max).
    pub smoothing_factor: f64,
}

impl Default for TemporalAqConfig {
    fn default() -> Self {
        Self {
            window_size: DEFAULT_WINDOW_SIZE,
            max_qp_delta: 6,
            motion_sensitivity: 1.0,
            scene_change_threshold: 50_000.0,
            fade_sensitivity: 0.5,
            smoothing_factor: 0.3,
        }
    }
}

/// Result of temporal AQ analysis for a single frame.
#[derive(Debug, Clone)]
pub struct TemporalAqResult {
    /// Recommended QP delta for this frame.
    pub qp_delta: i8,
    /// Temporal complexity score used for the decision.
    pub complexity: f64,
    /// Whether the frame is at a scene boundary.
    pub at_scene_boundary: bool,
    /// Smoothed complexity after temporal filtering.
    pub smoothed_complexity: f64,
}

/// Temporal adaptive quantization engine.
#[derive(Debug)]
pub struct TemporalAqEngine {
    /// Engine configuration.
    config: TemporalAqConfig,
    /// Sliding window of recent frame activities.
    window: Vec<TemporalActivity>,
    /// Running average of complexity scores.
    running_avg: f64,
    /// Number of frames processed.
    frames_processed: u64,
}

impl TemporalAqEngine {
    /// Creates a new temporal AQ engine with the given configuration.
    pub fn new(config: TemporalAqConfig) -> Self {
        Self {
            config,
            window: Vec::new(),
            running_avg: 0.5,
            frames_processed: 0,
        }
    }

    /// Creates a temporal AQ engine with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(TemporalAqConfig::default())
    }

    /// Processes a new frame's temporal activity and returns AQ recommendations.
    #[allow(clippy::cast_precision_loss)]
    pub fn process_frame(&mut self, activity: TemporalActivity) -> TemporalAqResult {
        let raw_complexity = activity.complexity_score() * self.config.motion_sensitivity;
        let is_scene_change = activity.is_scene_change
            || activity.frame_sad > self.config.scene_change_threshold;

        // Temporal smoothing
        let smoothed = if is_scene_change {
            raw_complexity
        } else {
            self.running_avg * self.config.smoothing_factor
                + raw_complexity * (1.0 - self.config.smoothing_factor)
        };

        // Update running average
        let alpha = 2.0 / (self.config.window_size as f64 + 1.0);
        self.running_avg = self.running_avg * (1.0 - alpha) + raw_complexity * alpha;

        // Compute QP delta: more complex = lower QP (more bits)
        let deviation = smoothed - 0.5;
        let delta_f = -deviation * f64::from(self.config.max_qp_delta) * 2.0;
        let clamped = delta_f
            .round()
            .max(f64::from(-self.config.max_qp_delta))
            .min(f64::from(self.config.max_qp_delta));

        #[allow(clippy::cast_possible_truncation)]
        let qp_delta = clamped as i8;

        // Maintain window
        self.window.push(activity);
        if self.window.len() > self.config.window_size {
            self.window.remove(0);
        }
        self.frames_processed += 1;

        TemporalAqResult {
            qp_delta,
            complexity: raw_complexity,
            at_scene_boundary: is_scene_change,
            smoothed_complexity: smoothed,
        }
    }

    /// Returns the number of frames processed so far.
    pub fn frames_processed(&self) -> u64 {
        self.frames_processed
    }

    /// Returns the current running average complexity.
    pub fn running_average(&self) -> f64 {
        self.running_avg
    }

    /// Returns the current window size (number of buffered frames).
    pub fn current_window_len(&self) -> usize {
        self.window.len()
    }

    /// Resets the engine state.
    pub fn reset(&mut self) {
        self.window.clear();
        self.running_avg = 0.5;
        self.frames_processed = 0;
    }

    /// Computes the variance of complexity scores in the current window.
    #[allow(clippy::cast_precision_loss)]
    pub fn window_complexity_variance(&self) -> f64 {
        if self.window.len() < 2 {
            return 0.0;
        }
        let scores: Vec<f64> = self.window.iter().map(|a| a.complexity_score()).collect();
        let mean = scores.iter().sum::<f64>() / scores.len() as f64;
        let var = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64;
        var
    }

    /// Detects whether the current window represents a fade.
    pub fn detect_fade_in_window(&self) -> bool {
        if self.window.len() < 3 {
            return false;
        }
        let fade_count = self.window.iter().filter(|a| a.fade_strength > 0.1).count();
        #[allow(clippy::cast_precision_loss)]
        let ratio = fade_count as f64 / self.window.len() as f64;
        ratio > self.config.fade_sensitivity
    }
}

/// Estimates optimal QP adjustment for a given temporal complexity value.
#[allow(clippy::cast_precision_loss)]
#[allow(clippy::cast_possible_truncation)]
pub fn estimate_qp_for_complexity(complexity: f64, base_qp: u8, max_delta: i8) -> u8 {
    let delta_f = -(complexity - 0.5) * f64::from(max_delta) * 2.0;
    let adjusted = f64::from(base_qp) + delta_f;
    adjusted.round().max(1.0).min(51.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_activity_new() {
        let ta = TemporalActivity::new(42);
        assert_eq!(ta.frame_index, 42);
        assert!((ta.avg_motion_magnitude - 0.0).abs() < f64::EPSILON);
        assert!(!ta.is_scene_change);
    }

    #[test]
    fn test_temporal_activity_complexity_score() {
        let mut ta = TemporalActivity::new(0);
        ta.avg_motion_magnitude = 16.0; // 0.5 normalized
        ta.motion_coverage = 0.5;
        ta.fade_strength = 0.0;
        let score = ta.complexity_score();
        // 0.5*0.5 + 0.5*0.3 + 0.0 = 0.25 + 0.15 = 0.40
        assert!((score - 0.40).abs() < 0.01);
    }

    #[test]
    fn test_temporal_activity_complexity_clamped() {
        let mut ta = TemporalActivity::new(0);
        ta.avg_motion_magnitude = 1000.0;
        ta.motion_coverage = 1.0;
        ta.fade_strength = 1.0;
        let score = ta.complexity_score();
        assert!(score <= 1.0);
    }

    #[test]
    fn test_temporal_aq_config_default() {
        let cfg = TemporalAqConfig::default();
        assert_eq!(cfg.window_size, DEFAULT_WINDOW_SIZE);
        assert_eq!(cfg.max_qp_delta, 6);
    }

    #[test]
    fn test_temporal_aq_engine_new() {
        let engine = TemporalAqEngine::with_defaults();
        assert_eq!(engine.frames_processed(), 0);
        assert_eq!(engine.current_window_len(), 0);
    }

    #[test]
    fn test_temporal_aq_process_static_frame() {
        let mut engine = TemporalAqEngine::with_defaults();
        let ta = TemporalActivity::new(0);
        let result = engine.process_frame(ta);
        // Low complexity -> positive QP delta (save bits)
        assert!(result.qp_delta >= 0);
        assert_eq!(engine.frames_processed(), 1);
    }

    #[test]
    fn test_temporal_aq_process_high_motion() {
        let mut engine = TemporalAqEngine::with_defaults();
        let mut ta = TemporalActivity::new(0);
        ta.avg_motion_magnitude = 64.0;
        ta.motion_coverage = 0.9;
        let result = engine.process_frame(ta);
        // High complexity -> negative QP delta (more bits)
        assert!(result.qp_delta <= 0);
    }

    #[test]
    fn test_temporal_aq_scene_change() {
        let mut engine = TemporalAqEngine::with_defaults();
        let mut ta = TemporalActivity::new(0);
        ta.is_scene_change = true;
        ta.frame_sad = 100_000.0;
        let result = engine.process_frame(ta);
        assert!(result.at_scene_boundary);
    }

    #[test]
    fn test_temporal_aq_window_fill() {
        let config = TemporalAqConfig {
            window_size: 4,
            ..Default::default()
        };
        let mut engine = TemporalAqEngine::new(config);
        for i in 0..10 {
            let ta = TemporalActivity::new(i);
            engine.process_frame(ta);
        }
        assert_eq!(engine.current_window_len(), 4);
        assert_eq!(engine.frames_processed(), 10);
    }

    #[test]
    fn test_temporal_aq_reset() {
        let mut engine = TemporalAqEngine::with_defaults();
        engine.process_frame(TemporalActivity::new(0));
        engine.process_frame(TemporalActivity::new(1));
        engine.reset();
        assert_eq!(engine.frames_processed(), 0);
        assert_eq!(engine.current_window_len(), 0);
    }

    #[test]
    fn test_window_complexity_variance_empty() {
        let engine = TemporalAqEngine::with_defaults();
        assert!((engine.window_complexity_variance() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_window_complexity_variance_uniform() {
        let mut engine = TemporalAqEngine::with_defaults();
        for i in 0..5 {
            engine.process_frame(TemporalActivity::new(i));
        }
        // All frames have same zero complexity
        let var = engine.window_complexity_variance();
        assert!(var < 0.01);
    }

    #[test]
    fn test_detect_fade_empty_window() {
        let engine = TemporalAqEngine::with_defaults();
        assert!(!engine.detect_fade_in_window());
    }

    #[test]
    fn test_estimate_qp_for_complexity() {
        let qp = estimate_qp_for_complexity(0.5, 28, 6);
        assert_eq!(qp, 28); // neutral complexity = no change

        let qp_high = estimate_qp_for_complexity(1.0, 28, 6);
        assert!(qp_high < 28); // high complexity = lower QP

        let qp_low = estimate_qp_for_complexity(0.0, 28, 6);
        assert!(qp_low > 28); // low complexity = higher QP
    }

    #[test]
    fn test_estimate_qp_clamped() {
        let qp = estimate_qp_for_complexity(100.0, 1, 50);
        assert!(qp >= 1);
        assert!(qp <= 51);
    }
}
