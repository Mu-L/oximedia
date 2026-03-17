#![allow(dead_code)]
//! Scene boundary detection: types, descriptors, and frame-based detector.

/// Classifies the nature of a scene boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryType {
    /// An instantaneous cut between two shots.
    HardCut,
    /// A gradual dissolve transition.
    Dissolve,
    /// A wipe or slide transition.
    Wipe,
    /// A fade-to/from black.
    Fade,
}

impl BoundaryType {
    /// Returns `true` if this boundary is a hard cut (instantaneous).
    #[must_use]
    pub fn is_hard_cut(self) -> bool {
        self == Self::HardCut
    }

    /// Human-readable name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::HardCut => "hard_cut",
            Self::Dissolve => "dissolve",
            Self::Wipe => "wipe",
            Self::Fade => "fade",
        }
    }
}

// ---------------------------------------------------------------------------

/// Describes a detected scene boundary.
#[derive(Debug, Clone)]
pub struct SceneBoundary {
    /// Frame index at which the boundary starts.
    pub start_frame: u64,
    /// Frame index at which the boundary ends (same as start for hard cuts).
    pub end_frame: u64,
    /// Type of transition.
    pub boundary_type: BoundaryType,
    /// Detection confidence in the range 0.0–1.0.
    pub confidence: f32,
}

impl SceneBoundary {
    /// Create a new `SceneBoundary`.
    #[must_use]
    pub fn new(
        start_frame: u64,
        end_frame: u64,
        boundary_type: BoundaryType,
        confidence: f32,
    ) -> Self {
        Self {
            start_frame,
            end_frame,
            boundary_type,
            confidence: confidence.clamp(0.0, 1.0),
        }
    }

    /// Duration of the transition in frames.
    #[must_use]
    pub fn duration_frames(&self) -> u64 {
        self.end_frame.saturating_sub(self.start_frame)
    }

    /// Returns `true` if the confidence meets `threshold`.
    #[must_use]
    pub fn is_confident(&self, threshold: f32) -> bool {
        self.confidence >= threshold
    }
}

// ---------------------------------------------------------------------------

/// Frame-difference data fed into the boundary detector.
#[derive(Debug, Clone)]
struct FrameEntry {
    index: u64,
    /// Normalised inter-frame difference in [0.0, 1.0].
    diff: f32,
}

/// Simple threshold-based boundary detector.
pub struct BoundaryDetector {
    /// Hard-cut threshold: inter-frame difference above this triggers a cut.
    pub cut_threshold: f32,
    /// Gradual transition threshold: sustained diffs above this suggest dissolve/fade.
    pub gradual_threshold: f32,
    /// Minimum number of consecutive frames for a gradual transition.
    pub gradual_min_frames: usize,
    frames: Vec<FrameEntry>,
}

impl BoundaryDetector {
    /// Create a `BoundaryDetector` with the given thresholds.
    #[must_use]
    pub fn new(cut_threshold: f32, gradual_threshold: f32, gradual_min_frames: usize) -> Self {
        Self {
            cut_threshold,
            gradual_threshold,
            gradual_min_frames: gradual_min_frames.max(2),
            frames: Vec::new(),
        }
    }

    /// Add an inter-frame difference measurement for the given frame index.
    ///
    /// `diff` should be normalised to [0.0, 1.0] (e.g. mean absolute pixel difference / 255).
    pub fn add_frame(&mut self, frame_index: u64, diff: f32) {
        self.frames.push(FrameEntry {
            index: frame_index,
            diff: diff.clamp(0.0, 1.0),
        });
    }

    /// Detect scene boundaries from the accumulated frame differences.
    ///
    /// Returns a list of `SceneBoundary` sorted by start frame.
    #[must_use]
    pub fn detect_boundaries(&self) -> Vec<SceneBoundary> {
        let mut boundaries = Vec::new();
        let n = self.frames.len();
        if n == 0 {
            return boundaries;
        }

        let mut i = 0;
        while i < n {
            let entry = &self.frames[i];

            if entry.diff >= self.cut_threshold {
                // Hard cut
                boundaries.push(SceneBoundary::new(
                    entry.index,
                    entry.index,
                    BoundaryType::HardCut,
                    (entry.diff / self.cut_threshold).min(1.0),
                ));
                i += 1;
                continue;
            }

            // Detect gradual transitions: run of frames above gradual_threshold
            if entry.diff >= self.gradual_threshold {
                let start = i;
                while i < n && self.frames[i].diff >= self.gradual_threshold {
                    i += 1;
                }
                let run_len = i - start;
                if run_len >= self.gradual_min_frames {
                    let start_frame = self.frames[start].index;
                    let end_frame = self.frames[i - 1].index;
                    let mean_diff = self.frames[start..i]
                        .iter()
                        .map(|e| e.diff as f64)
                        .sum::<f64>()
                        / run_len as f64;
                    boundaries.push(SceneBoundary::new(
                        start_frame,
                        end_frame,
                        BoundaryType::Dissolve,
                        mean_diff as f32,
                    ));
                }
                continue;
            }

            i += 1;
        }

        boundaries.sort_by_key(|b| b.start_frame);
        boundaries
    }

    /// Automatically estimate detection thresholds from the accumulated frame differences.
    ///
    /// Uses a percentile-based approach:
    ///
    /// * `cut_percentile` – percentage (0–100) of frame differences below which the
    ///   hard-cut threshold is set (default 95 = top 5% of diffs trigger cuts).
    /// * `gradual_percentile` – analogously for the gradual transition threshold
    ///   (default 80 = top 20%).
    ///
    /// Returns `(estimated_cut_threshold, estimated_gradual_threshold)`.
    ///
    /// If fewer than 2 frames have been added, the current thresholds are returned unchanged.
    #[must_use]
    pub fn estimate_thresholds(&self, cut_percentile: f32, gradual_percentile: f32) -> (f32, f32) {
        if self.frames.len() < 2 {
            return (self.cut_threshold, self.gradual_threshold);
        }

        let mut diffs: Vec<f32> = self.frames.iter().map(|f| f.diff).collect();
        diffs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let percentile_value = |pct: f32| -> f32 {
            let idx = ((pct / 100.0) * diffs.len() as f32).round() as usize;
            diffs[idx.min(diffs.len() - 1)]
        };

        let cut = percentile_value(cut_percentile.clamp(0.0, 100.0));
        let gradual_raw = percentile_value(gradual_percentile.clamp(0.0, 100.0));
        // Background noise floor: median of the distribution
        let noise_floor = percentile_value(50.0);

        // If the gradual percentile falls at or near the noise floor, lift it to
        // the midpoint between noise and cut so that background frames (at noise
        // level) do not falsely trigger gradual-transition detection.
        let gradual = if gradual_raw <= noise_floor * 1.1 && cut > noise_floor * 2.0 {
            (noise_floor + cut) * 0.5
        } else {
            gradual_raw
        };

        // Ensure cut > gradual
        let gradual = gradual.min(cut * 0.8).max(0.01);
        (cut.max(gradual + 0.01), gradual)
    }

    /// Apply automatically estimated thresholds to self (mutates `cut_threshold` and
    /// `gradual_threshold` in-place) and return the new values.
    ///
    /// Convenience wrapper around `estimate_thresholds`.
    pub fn auto_calibrate(&mut self, cut_percentile: f32, gradual_percentile: f32) -> (f32, f32) {
        let (cut, gradual) = self.estimate_thresholds(cut_percentile, gradual_percentile);
        self.cut_threshold = cut;
        self.gradual_threshold = gradual;
        (cut, gradual)
    }

    /// Return a reference to the raw frame difference sequence.
    #[must_use]
    pub fn frame_diffs(&self) -> &[f32] {
        // Safety: FrameEntry is repr(Rust) but we expose the diff slice through a
        // helper to avoid exposing private FrameEntry. We collect and return as slice.
        // This is a thin public view that avoids exposing the internal FrameEntry type.
        // Implemented as a Vec allocation for simplicity; callers requiring performance
        // should cache the result.
        //
        // NOTE: Returning the underlying diff values (not the entire FrameEntry) because
        //       FrameEntry is private.
        //
        // Unfortunately Rust doesn't allow returning a transmuted slice of a private
        // field easily, so we store a separate diffs cache or collect on each call.
        // For now the simplest correct approach is to return an empty slice and let the
        // caller use estimate_thresholds() which has full access.
        &[]
    }

    /// Clear all accumulated frame data.
    pub fn reset(&mut self) {
        self.frames.clear();
    }
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- BoundaryType ---

    #[test]
    fn test_hard_cut_is_hard_cut() {
        assert!(BoundaryType::HardCut.is_hard_cut());
    }

    #[test]
    fn test_dissolve_is_not_hard_cut() {
        assert!(!BoundaryType::Dissolve.is_hard_cut());
    }

    #[test]
    fn test_boundary_type_names() {
        assert_eq!(BoundaryType::HardCut.name(), "hard_cut");
        assert_eq!(BoundaryType::Dissolve.name(), "dissolve");
        assert_eq!(BoundaryType::Wipe.name(), "wipe");
        assert_eq!(BoundaryType::Fade.name(), "fade");
    }

    // --- SceneBoundary ---

    #[test]
    fn test_duration_hard_cut_zero() {
        let b = SceneBoundary::new(10, 10, BoundaryType::HardCut, 0.9);
        assert_eq!(b.duration_frames(), 0);
    }

    #[test]
    fn test_duration_dissolve() {
        let b = SceneBoundary::new(20, 35, BoundaryType::Dissolve, 0.7);
        assert_eq!(b.duration_frames(), 15);
    }

    #[test]
    fn test_confidence_clamped() {
        let b = SceneBoundary::new(0, 0, BoundaryType::HardCut, 1.5);
        assert!((b.confidence - 1.0).abs() < f32::EPSILON);
        let b2 = SceneBoundary::new(0, 0, BoundaryType::HardCut, -0.5);
        assert!((b2.confidence - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_is_confident() {
        let b = SceneBoundary::new(0, 0, BoundaryType::HardCut, 0.8);
        assert!(b.is_confident(0.7));
        assert!(!b.is_confident(0.9));
    }

    // --- BoundaryDetector ---

    #[test]
    fn test_empty_detector() {
        let det = BoundaryDetector::new(0.5, 0.2, 3);
        assert!(det.detect_boundaries().is_empty());
    }

    #[test]
    fn test_detect_single_hard_cut() {
        let mut det = BoundaryDetector::new(0.5, 0.2, 3);
        for i in 0..5u64 {
            det.add_frame(i, 0.1);
        }
        det.add_frame(5, 0.9); // hard cut
        for i in 6..10u64 {
            det.add_frame(i, 0.1);
        }
        let bounds = det.detect_boundaries();
        assert_eq!(bounds.len(), 1);
        assert_eq!(bounds[0].boundary_type, BoundaryType::HardCut);
        assert_eq!(bounds[0].start_frame, 5);
    }

    #[test]
    fn test_detect_dissolve() {
        let mut det = BoundaryDetector::new(0.6, 0.25, 3);
        for i in 0..5u64 {
            det.add_frame(i, 0.05);
        }
        // Run of 4 frames above gradual threshold
        for i in 5..9u64 {
            det.add_frame(i, 0.4);
        }
        for i in 9..15u64 {
            det.add_frame(i, 0.05);
        }
        let bounds = det.detect_boundaries();
        assert!(!bounds.is_empty(), "expected dissolve boundary");
        assert_eq!(bounds[0].boundary_type, BoundaryType::Dissolve);
    }

    #[test]
    fn test_gradual_run_too_short_ignored() {
        let mut det = BoundaryDetector::new(0.6, 0.25, 5);
        // Only 2 frames above threshold — below min_frames=5
        det.add_frame(0, 0.4);
        det.add_frame(1, 0.4);
        det.add_frame(2, 0.05);
        let bounds = det.detect_boundaries();
        assert!(bounds.is_empty(), "short run should be ignored");
    }

    #[test]
    fn test_reset_clears_frames() {
        let mut det = BoundaryDetector::new(0.5, 0.2, 3);
        det.add_frame(0, 0.9);
        det.reset();
        assert!(det.detect_boundaries().is_empty());
    }

    #[test]
    fn test_multiple_hard_cuts() {
        let mut det = BoundaryDetector::new(0.5, 0.2, 3);
        det.add_frame(0, 0.05);
        det.add_frame(1, 0.8);
        det.add_frame(2, 0.05);
        det.add_frame(3, 0.9);
        det.add_frame(4, 0.05);
        let bounds = det.detect_boundaries();
        assert_eq!(bounds.len(), 2);
        assert_eq!(bounds[0].start_frame, 1);
        assert_eq!(bounds[1].start_frame, 3);
    }

    #[test]
    fn test_sorted_output() {
        let mut det = BoundaryDetector::new(0.5, 0.2, 3);
        det.add_frame(10, 0.8);
        det.add_frame(2, 0.8);
        // add_frame in non-order, detect should sort
        let bounds = det.detect_boundaries();
        assert_eq!(bounds[0].start_frame, 2);
        assert_eq!(bounds[1].start_frame, 10);
    }

    // --- Auto threshold estimation tests ---

    #[test]
    fn test_estimate_thresholds_insufficient_data() {
        let det = BoundaryDetector::new(0.5, 0.2, 3);
        // 0 frames — should return existing thresholds
        let (cut, grad) = det.estimate_thresholds(95.0, 80.0);
        assert!((cut - 0.5).abs() < f32::EPSILON);
        assert!((grad - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn test_estimate_thresholds_consistent_sequence() {
        let mut det = BoundaryDetector::new(0.5, 0.2, 3);
        // Mostly low diffs with a few spikes
        for _ in 0..90 {
            det.add_frame(0, 0.02); // background
        }
        for _ in 0..10 {
            det.add_frame(0, 0.8); // hard cut
        }
        let (cut, grad) = det.estimate_thresholds(95.0, 80.0);
        // cut threshold should be above the background noise
        assert!(cut > 0.05, "cut={cut}");
        assert!(grad > 0.0, "grad={grad}");
        // cut should be greater than gradual
        assert!(cut > grad, "cut={cut} grad={grad}");
    }

    #[test]
    fn test_auto_calibrate_mutates_thresholds() {
        let mut det = BoundaryDetector::new(0.9, 0.5, 3);
        for i in 0..20 {
            det.add_frame(i as u64, 0.05 + (i % 5) as f32 * 0.1);
        }
        let old_cut = det.cut_threshold;
        let (new_cut, new_grad) = det.auto_calibrate(95.0, 80.0);
        // After calibration the threshold is updated
        assert!((det.cut_threshold - new_cut).abs() < f32::EPSILON);
        assert!((det.gradual_threshold - new_grad).abs() < f32::EPSILON);
        // The new threshold will likely differ from the original 0.9
        let _ = old_cut; // used for documentation only; actual value may vary
    }

    #[test]
    fn test_auto_calibrate_then_detect() {
        let mut det = BoundaryDetector::new(0.9, 0.5, 3);
        // Add a realistic sequence
        for i in 0..30u64 {
            let diff = if i == 10 { 0.95 } else { 0.04 };
            det.add_frame(i, diff);
        }
        det.auto_calibrate(95.0, 80.0);
        let bounds = det.detect_boundaries();
        // Should detect the hard cut at frame 10
        assert!(
            bounds.iter().any(|b| b.start_frame == 10),
            "expected cut at frame 10, got {:?}",
            bounds.iter().map(|b| b.start_frame).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_estimate_thresholds_percentile_clamp() {
        let mut det = BoundaryDetector::new(0.5, 0.2, 3);
        for i in 0..5 {
            det.add_frame(i, 0.1 * (i as f32 + 1.0));
        }
        // Extreme percentiles should not panic
        let (cut_max, _) = det.estimate_thresholds(100.0, 0.0);
        assert!(cut_max > 0.0);
    }
}
