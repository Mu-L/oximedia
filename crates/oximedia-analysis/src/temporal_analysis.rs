//! Temporal analysis: motion scoring, cut detection, and windowed feature extraction.

#![allow(dead_code)]

/// A temporal feature that can be extracted from a frame window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalFeature {
    /// Camera or scene motion level.
    Motion,
    /// Luminance flicker across frames.
    Flicker,
    /// Hard cut (abrupt scene change).
    Cut,
    /// Gradual transition (fade/dissolve).
    Transition,
    /// Temporal noise / grain.
    Grain,
}

impl TemporalFeature {
    /// Human-readable label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Motion => "Motion",
            Self::Flicker => "Flicker",
            Self::Cut => "Cut",
            Self::Transition => "Transition",
            Self::Grain => "Grain",
        }
    }
}

/// A sliding window of per-frame luma mean values used for temporal analysis.
#[derive(Debug, Clone)]
pub struct TemporalWindow {
    /// Luma means in presentation order.
    means: Vec<f64>,
    /// Frame rate of the source material (frames per second).
    fps: f64,
}

impl TemporalWindow {
    /// Create a new empty window.
    #[must_use]
    pub fn new(fps: f64) -> Self {
        Self {
            means: Vec::new(),
            fps: fps.max(1.0),
        }
    }

    /// Push the mean luma of the next frame.
    pub fn push(&mut self, mean_luma: f64) {
        self.means.push(mean_luma);
    }

    /// Duration of the window in milliseconds.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn duration_ms(&self) -> f64 {
        self.means.len() as f64 / self.fps * 1_000.0
    }

    /// Number of frames in the window.
    #[must_use]
    pub fn len(&self) -> usize {
        self.means.len()
    }

    /// `true` when the window holds no frames.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.means.is_empty()
    }

    /// Mean luma across all frames.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn mean_luma(&self) -> f64 {
        if self.means.is_empty() {
            return 0.0;
        }
        self.means.iter().sum::<f64>() / self.means.len() as f64
    }
}

/// Detected cut event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutEvent {
    /// Index of the frame immediately after the cut.
    pub frame_index: usize,
    /// Absolute luma difference that triggered detection.
    pub luma_diff: f64,
}

/// Result of a full temporal analysis pass.
#[derive(Debug, Clone)]
pub struct TemporalAnalysisResult {
    /// Aggregate motion score (0.0 – 1.0).
    pub motion_score: f64,
    /// Detected cut positions.
    pub cuts: Vec<CutEvent>,
    /// Flicker score (0.0 – 1.0, higher = more flicker).
    pub flicker_score: f64,
}

/// Performs temporal analysis over a sequence of frames.
#[derive(Debug)]
pub struct TemporalAnalysis {
    /// Per-frame luma means added so far.
    luma_means: Vec<f64>,
    /// Frame rate.
    fps: f64,
    /// Threshold above which a luma difference is classified as a cut.
    cut_threshold: f64,
    /// Threshold for inter-frame difference contributing to motion score.
    motion_threshold: f64,
}

impl TemporalAnalysis {
    /// Create a new analyzer.
    ///
    /// * `fps` – frames per second of the source.
    /// * `cut_threshold` – luma difference (0–255) that triggers a cut.
    /// * `motion_threshold` – luma difference considered "motion".
    #[must_use]
    pub fn new(fps: f64, cut_threshold: f64, motion_threshold: f64) -> Self {
        Self {
            luma_means: Vec::new(),
            fps: fps.max(1.0),
            cut_threshold,
            motion_threshold,
        }
    }

    /// Add the mean luma of the next frame.
    pub fn add_frame(&mut self, mean_luma: f64) {
        self.luma_means.push(mean_luma);
    }

    /// Compute the aggregate motion score (0.0 – 1.0) from accumulated frames.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn compute_motion_score(&self) -> f64 {
        if self.luma_means.len() < 2 {
            return 0.0;
        }
        let motion_frames = self
            .luma_means
            .windows(2)
            .filter(|w| (w[1] - w[0]).abs() >= self.motion_threshold)
            .count();
        motion_frames as f64 / (self.luma_means.len() - 1) as f64
    }

    /// Detect hard cuts and return them as a list of `CutEvent`.
    #[must_use]
    pub fn detect_cuts(&self) -> Vec<CutEvent> {
        self.luma_means
            .windows(2)
            .enumerate()
            .filter_map(|(i, w)| {
                let diff = (w[1] - w[0]).abs();
                if diff >= self.cut_threshold {
                    Some(CutEvent {
                        frame_index: i + 1,
                        luma_diff: diff,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Compute a flicker score based on sign alternation of inter-frame differences.
    ///
    /// Rapid brightness oscillations produce a high score (0.0 – 1.0).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn compute_flicker_score(&self) -> f64 {
        if self.luma_means.len() < 3 {
            return 0.0;
        }
        let diffs: Vec<f64> = self.luma_means.windows(2).map(|w| w[1] - w[0]).collect();
        let sign_alternations = diffs.windows(2).filter(|w| w[0] * w[1] < 0.0).count();
        sign_alternations as f64 / (diffs.len() - 1) as f64
    }

    /// Finalise and return the complete analysis result.
    #[must_use]
    pub fn finalize(&self) -> TemporalAnalysisResult {
        TemporalAnalysisResult {
            motion_score: self.compute_motion_score(),
            cuts: self.detect_cuts(),
            flicker_score: self.compute_flicker_score(),
        }
    }

    /// Number of frames added so far.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.luma_means.len()
    }
}

impl Default for TemporalAnalysis {
    fn default() -> Self {
        Self::new(25.0, 30.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_feature_label_motion() {
        assert_eq!(TemporalFeature::Motion.label(), "Motion");
    }

    #[test]
    fn test_temporal_feature_label_cut() {
        assert_eq!(TemporalFeature::Cut.label(), "Cut");
    }

    #[test]
    fn test_window_duration_ms_empty() {
        let w = TemporalWindow::new(25.0);
        assert_eq!(w.duration_ms(), 0.0);
    }

    #[test]
    fn test_window_duration_ms_one_second() {
        let mut w = TemporalWindow::new(25.0);
        for _ in 0..25 {
            w.push(128.0);
        }
        assert!((w.duration_ms() - 1000.0).abs() < 1.0);
    }

    #[test]
    fn test_window_mean_luma() {
        let mut w = TemporalWindow::new(25.0);
        w.push(100.0);
        w.push(200.0);
        assert!((w.mean_luma() - 150.0).abs() < 1e-9);
    }

    #[test]
    fn test_window_is_empty() {
        let w = TemporalWindow::new(25.0);
        assert!(w.is_empty());
    }

    #[test]
    fn test_analysis_motion_score_zero_no_motion() {
        let mut a = TemporalAnalysis::new(25.0, 30.0, 5.0);
        for _ in 0..10 {
            a.add_frame(128.0);
        }
        assert_eq!(a.compute_motion_score(), 0.0);
    }

    #[test]
    fn test_analysis_motion_score_full_motion() {
        let mut a = TemporalAnalysis::new(25.0, 30.0, 5.0);
        for i in 0..10 {
            a.add_frame(if i % 2 == 0 { 50.0 } else { 200.0 });
        }
        assert!(a.compute_motion_score() > 0.9);
    }

    #[test]
    fn test_analysis_detect_cut() {
        let mut a = TemporalAnalysis::new(25.0, 30.0, 5.0);
        for _ in 0..5 {
            a.add_frame(50.0);
        }
        a.add_frame(200.0); // abrupt change
        for _ in 0..5 {
            a.add_frame(200.0);
        }
        let cuts = a.detect_cuts();
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].frame_index, 5);
    }

    #[test]
    fn test_analysis_no_cuts_static() {
        let mut a = TemporalAnalysis::new(25.0, 30.0, 5.0);
        for _ in 0..10 {
            a.add_frame(128.0);
        }
        assert!(a.detect_cuts().is_empty());
    }

    #[test]
    fn test_analysis_flicker_score_alternating() {
        let mut a = TemporalAnalysis::new(25.0, 200.0, 5.0);
        for i in 0..10 {
            a.add_frame(if i % 2 == 0 { 50.0 } else { 200.0 });
        }
        assert!(a.compute_flicker_score() > 0.8);
    }

    #[test]
    fn test_analysis_flicker_score_static() {
        let mut a = TemporalAnalysis::new(25.0, 30.0, 5.0);
        for _ in 0..10 {
            a.add_frame(128.0);
        }
        assert_eq!(a.compute_flicker_score(), 0.0);
    }

    #[test]
    fn test_analysis_frame_count() {
        let mut a = TemporalAnalysis::default();
        a.add_frame(100.0);
        a.add_frame(110.0);
        assert_eq!(a.frame_count(), 2);
    }

    #[test]
    fn test_analysis_finalize_returns_result() {
        let mut a = TemporalAnalysis::default();
        for _ in 0..5 {
            a.add_frame(128.0);
        }
        let result = a.finalize();
        assert_eq!(result.motion_score, 0.0);
        assert!(result.cuts.is_empty());
    }

    #[test]
    fn test_cut_event_luma_diff() {
        let mut a = TemporalAnalysis::new(25.0, 30.0, 5.0);
        a.add_frame(50.0);
        a.add_frame(150.0); // diff = 100
        let cuts = a.detect_cuts();
        assert_eq!(cuts.len(), 1);
        assert!((cuts[0].luma_diff - 100.0).abs() < 1e-9);
    }
}
