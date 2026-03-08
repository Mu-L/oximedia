//! Temporal video quality metrics.
//!
//! Provides tools for measuring flickering, motion blur, and frame-rate consistency
//! across a sequence of video frames.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// ─── Temporal flickering ───────────────────────────────────────────────────────

/// Frame-to-frame luminance variance (flickering) statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalFlickering {
    /// Mean inter-frame luminance change
    pub mean: f32,
    /// Standard deviation of inter-frame luminance changes
    pub std_dev: f32,
    /// Maximum single-frame luminance delta
    pub max_delta: f32,
    /// Number of frames where `|delta| > 2 * std_dev`
    pub flickering_frames: u32,
}

impl TemporalFlickering {
    /// Returns `true` when the flickering standard deviation is below 3.0.
    #[must_use]
    pub fn is_acceptable(&self) -> bool {
        self.std_dev < 3.0
    }

    /// Analyse a sequence of per-frame luma means to compute flickering stats.
    ///
    /// `luma_means` must contain at least two values; otherwise all fields are zero.
    #[must_use]
    pub fn analyze(luma_means: &[f32]) -> Self {
        if luma_means.len() < 2 {
            return Self {
                mean: 0.0,
                std_dev: 0.0,
                max_delta: 0.0,
                flickering_frames: 0,
            };
        }

        let deltas: Vec<f32> = luma_means.windows(2).map(|w| (w[1] - w[0]).abs()).collect();

        let n = deltas.len() as f32;
        let mean = deltas.iter().sum::<f32>() / n;
        let variance = deltas.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / n;
        let std_dev = variance.sqrt();

        let max_delta = deltas.iter().copied().fold(0.0f32, f32::max);

        // If std_dev is zero all deltas are identical — no flickering by definition.
        let flickering_frames = if std_dev < 1e-6 {
            0
        } else {
            let threshold = 2.0 * std_dev;
            deltas.iter().filter(|&&d| d > threshold).count() as u32
        };

        Self {
            mean,
            std_dev,
            max_delta,
            flickering_frames,
        }
    }
}

// ─── Motion blur score ────────────────────────────────────────────────────────

/// Aggregated motion-blur scores across a video.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotionBlurScore {
    /// Average blur score across all frames
    pub average: f32,
    /// Index of the sharpest frame (highest Laplacian variance)
    pub sharpest_frame: usize,
    /// Index of the blurriest frame (lowest Laplacian variance)
    pub blurriest_frame: usize,
}

impl MotionBlurScore {
    /// Map the average score to a letter grade (A–F).
    ///
    /// Higher Laplacian variance → sharper → better grade.
    #[must_use]
    pub fn quality_grade(&self) -> char {
        match self.average as u32 {
            s if s >= 500 => 'A',
            s if s >= 300 => 'B',
            s if s >= 150 => 'C',
            s if s >= 50 => 'D',
            s if s >= 10 => 'E',
            _ => 'F',
        }
    }

    /// Build a `MotionBlurScore` from a list of per-frame sharpness scores.
    #[must_use]
    pub fn from_frame_scores(scores: &[f32]) -> Self {
        if scores.is_empty() {
            return Self {
                average: 0.0,
                sharpest_frame: 0,
                blurriest_frame: 0,
            };
        }

        let average = scores.iter().sum::<f32>() / scores.len() as f32;

        let sharpest_frame = scores
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.partial_cmp(b)
                    .expect("invariant: sharpness values are finite f32")
            })
            .map_or(0, |(i, _)| i);

        let blurriest_frame = scores
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                a.partial_cmp(b)
                    .expect("invariant: sharpness values are finite f32")
            })
            .map_or(0, |(i, _)| i);

        Self {
            average,
            sharpest_frame,
            blurriest_frame,
        }
    }
}

/// Compute per-frame sharpness scores.
pub struct MotionBlurDetector;

impl MotionBlurDetector {
    /// Compute a sharpness score for a single frame using Laplacian variance.
    ///
    /// Higher variance = sharper image.
    /// `frame` is a slice of normalised luma samples (0.0–1.0), arranged
    /// row-major with dimensions `width × height`.
    #[must_use]
    pub fn compute_score(frame: &[f32], width: u32, height: u32) -> f32 {
        if width < 3 || height < 3 || frame.len() < (width * height) as usize {
            return 0.0;
        }

        let w = width as usize;
        let h = height as usize;

        let mut lap_values = Vec::with_capacity((w - 2) * (h - 2));

        for row in 1..h - 1 {
            for col in 1..w - 1 {
                // 5-point discrete Laplacian
                let c = frame[row * w + col];
                let n = frame[(row - 1) * w + col];
                let s = frame[(row + 1) * w + col];
                let e = frame[row * w + col + 1];
                let ww = frame[row * w + col - 1];
                let lap = (4.0 * c - n - s - e - ww).abs();
                lap_values.push(lap);
            }
        }

        if lap_values.is_empty() {
            return 0.0;
        }

        // Return mean-squared Laplacian * 1000 so typical values are in the hundreds.
        // Using mean-squared rather than variance ensures a non-zero score for any
        // frame that has non-zero gradients (e.g. a checkerboard).
        let n = lap_values.len() as f32;
        let mean_sq = lap_values.iter().map(|v| v * v).sum::<f32>() / n;
        mean_sq * 1000.0
    }
}

// ─── Frame-rate consistency ────────────────────────────────────────────────────

/// Frame-rate consistency measured from presentation timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameRateConsistency {
    /// Expected (nominal) frame rate
    pub nominal_fps: f32,
    /// Measured per-frame instantaneous rates
    pub measured_fps: Vec<f32>,
    /// RMS jitter in milliseconds
    pub jitter_ms: f32,
    /// Number of frames with timing more than 50 % off the nominal interval
    pub dropped_frames: u32,
}

impl FrameRateConsistency {
    /// Analyse a sequence of presentation timestamps (in milliseconds).
    ///
    /// `timestamps_ms` must be sorted ascending with at least two entries.
    #[must_use]
    pub fn analyze(timestamps_ms: &[u64]) -> Self {
        if timestamps_ms.len() < 2 {
            return Self {
                nominal_fps: 0.0,
                measured_fps: Vec::new(),
                jitter_ms: 0.0,
                dropped_frames: 0,
            };
        }

        // Compute inter-frame intervals
        let intervals_ms: Vec<f64> = timestamps_ms
            .windows(2)
            .map(|w| (w[1] - w[0]) as f64)
            .collect();

        let n = intervals_ms.len() as f64;
        let mean_interval = intervals_ms.iter().sum::<f64>() / n;
        let nominal_fps = if mean_interval > 0.0 {
            (1000.0 / mean_interval) as f32
        } else {
            0.0
        };

        // Per-frame measured rates
        let measured_fps: Vec<f32> = intervals_ms
            .iter()
            .map(|&dt| if dt > 0.0 { (1000.0 / dt) as f32 } else { 0.0 })
            .collect();

        // RMS jitter
        let variance = intervals_ms
            .iter()
            .map(|dt| (dt - mean_interval).powi(2))
            .sum::<f64>()
            / n;
        let jitter_ms = variance.sqrt() as f32;

        // Dropped frames: interval > 1.5× nominal
        let threshold = mean_interval * 1.5;
        let dropped_frames = intervals_ms.iter().filter(|&&dt| dt > threshold).count() as u32;

        Self {
            nominal_fps,
            measured_fps,
            jitter_ms,
            dropped_frames,
        }
    }

    /// Whether the frame rate is consistent (jitter < 2 ms and no drops).
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.jitter_ms < 2.0 && self.dropped_frames == 0
    }
}

// ─── Temporal quality report ──────────────────────────────────────────────────

/// Combined temporal quality report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalQualityReport {
    /// Flickering analysis
    pub flickering: TemporalFlickering,
    /// Motion-blur analysis
    pub motion_blur: MotionBlurScore,
    /// Frame-rate consistency analysis
    pub frame_rate: FrameRateConsistency,
    /// Overall temporal quality score (0–100)
    pub overall_score: f32,
}

impl TemporalQualityReport {
    /// Build a report from pre-computed sub-metrics.
    #[must_use]
    pub fn new(
        flickering: TemporalFlickering,
        motion_blur: MotionBlurScore,
        frame_rate: FrameRateConsistency,
    ) -> Self {
        let overall_score = compute_overall(&flickering, &motion_blur, &frame_rate);
        Self {
            flickering,
            motion_blur,
            frame_rate,
            overall_score,
        }
    }
}

/// Compute a 0–100 overall temporal quality score.
fn compute_overall(
    flickering: &TemporalFlickering,
    motion_blur: &MotionBlurScore,
    frame_rate: &FrameRateConsistency,
) -> f32 {
    // Flickering component (0–40): perfect when std_dev = 0, 0 when std_dev ≥ 10
    let flicker_score = (1.0 - (flickering.std_dev / 10.0).min(1.0)) * 40.0;

    // Sharpness component (0–30): grade A → 30, F → 0
    let sharpness_score = match motion_blur.quality_grade() {
        'A' => 30.0,
        'B' => 24.0,
        'C' => 18.0,
        'D' => 12.0,
        'E' => 6.0,
        _ => 0.0,
    };

    // Frame-rate component (0–30): jitter < 1 ms and no drops → 30
    let jitter_penalty = (frame_rate.jitter_ms / 10.0).min(1.0) * 15.0;
    let drop_penalty = (frame_rate.dropped_frames as f32 * 3.0).min(15.0);
    let fr_score = (30.0 - jitter_penalty - drop_penalty).max(0.0);

    flicker_score + sharpness_score + fr_score
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── TemporalFlickering ──────────────────────────────────────────────────

    #[test]
    fn test_flickering_stable_sequence() {
        let lumas = vec![100.0f32; 30]; // constant brightness
        let f = TemporalFlickering::analyze(&lumas);
        assert_eq!(f.mean, 0.0);
        assert_eq!(f.std_dev, 0.0);
        assert_eq!(f.max_delta, 0.0);
        assert_eq!(f.flickering_frames, 0);
        assert!(f.is_acceptable());
    }

    #[test]
    fn test_flickering_single_frame() {
        let lumas = vec![100.0f32];
        let f = TemporalFlickering::analyze(&lumas);
        assert_eq!(f.std_dev, 0.0);
    }

    #[test]
    fn test_flickering_large_variance() {
        let lumas: Vec<f32> = (0..20)
            .map(|i| if i % 2 == 0 { 100.0 } else { 120.0 })
            .collect();
        let f = TemporalFlickering::analyze(&lumas);
        assert!(f.mean > 0.0);
        // std_dev should be very low (all deltas equal) → 0 flickering_frames
        assert_eq!(f.flickering_frames, 0);
        assert!(f.is_acceptable());
    }

    #[test]
    fn test_flickering_is_acceptable_threshold() {
        let f_ok = TemporalFlickering {
            mean: 0.5,
            std_dev: 2.9,
            max_delta: 3.0,
            flickering_frames: 0,
        };
        assert!(f_ok.is_acceptable());

        let f_fail = TemporalFlickering {
            mean: 1.0,
            std_dev: 3.1,
            max_delta: 5.0,
            flickering_frames: 2,
        };
        assert!(!f_fail.is_acceptable());
    }

    #[test]
    fn test_flickering_max_delta() {
        let lumas = vec![100.0, 115.0, 110.0, 100.0];
        let f = TemporalFlickering::analyze(&lumas);
        assert!((f.max_delta - 15.0).abs() < 1e-4);
    }

    // ── MotionBlurDetector ──────────────────────────────────────────────────

    #[test]
    fn test_motion_blur_detector_sharp_frame() {
        // Checkerboard pattern → high Laplacian variance
        let w = 8u32;
        let h = 8u32;
        let frame: Vec<f32> = (0..w * h)
            .map(|i| {
                let row = i / w;
                let col = i % w;
                if (row + col) % 2 == 0 {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();
        let score = MotionBlurDetector::compute_score(&frame, w, h);
        assert!(
            score > 0.0,
            "sharp checkerboard should have a positive score"
        );
    }

    #[test]
    fn test_motion_blur_detector_flat_frame() {
        let frame = vec![0.5f32; 16 * 16];
        let score = MotionBlurDetector::compute_score(&frame, 16, 16);
        assert_eq!(
            score, 0.0,
            "uniform frame should have zero Laplacian variance"
        );
    }

    #[test]
    fn test_motion_blur_detector_small_frame() {
        let frame = vec![0.5f32; 4];
        let score = MotionBlurDetector::compute_score(&frame, 2, 2);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_motion_blur_score_grade() {
        assert_eq!(
            MotionBlurScore {
                average: 600.0,
                sharpest_frame: 0,
                blurriest_frame: 0
            }
            .quality_grade(),
            'A'
        );
        assert_eq!(
            MotionBlurScore {
                average: 350.0,
                sharpest_frame: 0,
                blurriest_frame: 0
            }
            .quality_grade(),
            'B'
        );
        assert_eq!(
            MotionBlurScore {
                average: 200.0,
                sharpest_frame: 0,
                blurriest_frame: 0
            }
            .quality_grade(),
            'C'
        );
        assert_eq!(
            MotionBlurScore {
                average: 80.0,
                sharpest_frame: 0,
                blurriest_frame: 0
            }
            .quality_grade(),
            'D'
        );
        assert_eq!(
            MotionBlurScore {
                average: 25.0,
                sharpest_frame: 0,
                blurriest_frame: 0
            }
            .quality_grade(),
            'E'
        );
        assert_eq!(
            MotionBlurScore {
                average: 1.0,
                sharpest_frame: 0,
                blurriest_frame: 0
            }
            .quality_grade(),
            'F'
        );
    }

    #[test]
    fn test_motion_blur_score_from_frame_scores() {
        let scores = vec![100.0f32, 200.0, 300.0, 50.0];
        let s = MotionBlurScore::from_frame_scores(&scores);
        assert!((s.average - 162.5).abs() < 1e-3);
        assert_eq!(s.sharpest_frame, 2);
        assert_eq!(s.blurriest_frame, 3);
    }

    #[test]
    fn test_motion_blur_score_empty() {
        let s = MotionBlurScore::from_frame_scores(&[]);
        assert_eq!(s.average, 0.0);
    }

    // ── FrameRateConsistency ───────────────────────────────────────────────

    #[test]
    fn test_frame_rate_perfect() {
        // Exactly 25 fps: 40 ms per frame
        let ts: Vec<u64> = (0..=25).map(|i| i * 40).collect();
        let fr = FrameRateConsistency::analyze(&ts);
        assert!(
            (fr.nominal_fps - 25.0).abs() < 0.1,
            "fps={}",
            fr.nominal_fps
        );
        assert!(fr.jitter_ms < 1.0);
        assert_eq!(fr.dropped_frames, 0);
        assert!(fr.is_consistent());
    }

    #[test]
    fn test_frame_rate_dropped() {
        // One frame is double-duration (dropped)
        let mut ts: Vec<u64> = (0..10).map(|i| i * 33).collect();
        ts[5] = ts[4] + 66; // simulate drop
        let fr = FrameRateConsistency::analyze(&ts);
        assert!(fr.dropped_frames >= 1);
    }

    #[test]
    fn test_frame_rate_single_ts() {
        let ts = vec![0u64];
        let fr = FrameRateConsistency::analyze(&ts);
        assert_eq!(fr.nominal_fps, 0.0);
    }

    #[test]
    fn test_frame_rate_empty() {
        let fr = FrameRateConsistency::analyze(&[]);
        assert_eq!(fr.nominal_fps, 0.0);
    }

    // ── TemporalQualityReport ──────────────────────────────────────────────

    #[test]
    fn test_temporal_quality_report_construction() {
        let flickering = TemporalFlickering::analyze(&[100.0; 30]);
        let blur_scores = vec![500.0f32; 10];
        let motion_blur = MotionBlurScore::from_frame_scores(&blur_scores);
        let ts: Vec<u64> = (0..=25).map(|i| i * 40).collect();
        let frame_rate = FrameRateConsistency::analyze(&ts);

        let report = TemporalQualityReport::new(flickering, motion_blur, frame_rate);
        assert!(report.overall_score >= 0.0 && report.overall_score <= 100.0);
    }

    #[test]
    fn test_temporal_quality_report_high_quality() {
        let flickering = TemporalFlickering {
            mean: 0.0,
            std_dev: 0.0,
            max_delta: 0.0,
            flickering_frames: 0,
        };
        let motion_blur = MotionBlurScore {
            average: 600.0,
            sharpest_frame: 0,
            blurriest_frame: 0,
        };
        let frame_rate = FrameRateConsistency {
            nominal_fps: 25.0,
            measured_fps: vec![25.0; 25],
            jitter_ms: 0.0,
            dropped_frames: 0,
        };
        let report = TemporalQualityReport::new(flickering, motion_blur, frame_rate);
        assert!(
            report.overall_score > 80.0,
            "score={}",
            report.overall_score
        );
    }
}
