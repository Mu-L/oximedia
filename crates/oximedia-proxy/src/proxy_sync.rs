//! Proxy synchronization for OxiMedia proxy system.
//!
//! Provides proxy-to-original timecode alignment, sync verification,
//! and drift detection for maintaining accurate proxy-original correspondence.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

/// A timecode value expressed as total frames at a given frame rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrameTimecode {
    /// Total frame count from zero.
    pub frame_number: u64,
    /// Frame rate numerator.
    pub fps_num: u32,
    /// Frame rate denominator.
    pub fps_den: u32,
}

impl FrameTimecode {
    /// Create a new frame timecode.
    #[must_use]
    pub fn new(frame_number: u64, fps_num: u32, fps_den: u32) -> Self {
        Self {
            frame_number,
            fps_num,
            fps_den,
        }
    }

    /// Create a timecode from hours, minutes, seconds, frames at 24fps.
    #[must_use]
    pub fn from_hmsf(hours: u64, minutes: u64, seconds: u64, frames: u64, fps: u64) -> Self {
        let total_frames = ((hours * 3600 + minutes * 60 + seconds) * fps) + frames;
        Self {
            frame_number: total_frames,
            fps_num: fps as u32,
            fps_den: 1,
        }
    }

    /// Frame rate as a floating-point value.
    #[must_use]
    pub fn fps_f64(&self) -> f64 {
        self.fps_num as f64 / self.fps_den as f64
    }

    /// Duration in seconds.
    #[must_use]
    pub fn as_seconds(&self) -> f64 {
        self.frame_number as f64 / self.fps_f64()
    }

    /// Compute the difference in frames from another timecode.
    ///
    /// Returns a positive value if `self` is later than `other`.
    #[must_use]
    pub fn frame_diff(&self, other: &Self) -> i64 {
        self.frame_number as i64 - other.frame_number as i64
    }
}

/// A sync point linking a proxy frame to an original frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPoint {
    /// Timecode in the proxy media.
    pub proxy_tc: FrameTimecode,
    /// Corresponding timecode in the original media.
    pub original_tc: FrameTimecode,
    /// Whether this sync point was verified by checksum comparison.
    pub verified: bool,
}

impl SyncPoint {
    /// Create a new sync point.
    #[must_use]
    pub fn new(proxy_tc: FrameTimecode, original_tc: FrameTimecode) -> Self {
        Self {
            proxy_tc,
            original_tc,
            verified: false,
        }
    }

    /// Mark this sync point as verified.
    #[must_use]
    pub fn verified(mut self) -> Self {
        self.verified = true;
        self
    }

    /// Frame offset between proxy and original (in proxy frames).
    #[must_use]
    pub fn frame_offset(&self) -> i64 {
        self.proxy_tc.frame_number as i64 - self.original_tc.frame_number as i64
    }
}

/// Result of a sync verification check.
#[derive(Debug, Clone)]
pub struct SyncVerificationResult {
    /// Clip or session identifier.
    pub clip_id: String,
    /// Whether the sync is valid within tolerance.
    pub in_sync: bool,
    /// Maximum frame drift detected.
    pub max_drift_frames: i64,
    /// Number of sync points checked.
    pub points_checked: usize,
    /// Number of sync points that passed verification.
    pub points_passed: usize,
}

impl SyncVerificationResult {
    /// Fraction of sync points that passed [0.0, 1.0].
    #[must_use]
    pub fn pass_rate(&self) -> f32 {
        if self.points_checked == 0 {
            return 1.0;
        }
        self.points_passed as f32 / self.points_checked as f32
    }
}

/// Tolerance settings for sync verification.
#[derive(Debug, Clone, Copy)]
pub struct SyncTolerance {
    /// Maximum allowed frame drift (inclusive).
    pub max_drift_frames: u32,
    /// Minimum fraction of sync points that must pass.
    pub min_pass_rate: f32,
}

impl SyncTolerance {
    /// Create a tolerance with given drift and pass rate.
    #[must_use]
    pub fn new(max_drift_frames: u32, min_pass_rate: f32) -> Self {
        Self {
            max_drift_frames,
            min_pass_rate: min_pass_rate.clamp(0.0, 1.0),
        }
    }

    /// Strict tolerance: zero drift allowed, 100% pass rate required.
    #[must_use]
    pub fn strict() -> Self {
        Self::new(0, 1.0)
    }

    /// Lenient tolerance: up to 2 frames drift, 90% pass rate.
    #[must_use]
    pub fn lenient() -> Self {
        Self::new(2, 0.9)
    }
}

impl Default for SyncTolerance {
    fn default() -> Self {
        Self::new(1, 0.95)
    }
}

/// Verifies proxy-to-original synchronization.
#[allow(dead_code)]
pub struct ProxySyncVerifier {
    /// Sync points to check.
    sync_points: Vec<SyncPoint>,
    /// Tolerance settings.
    tolerance: SyncTolerance,
}

impl ProxySyncVerifier {
    /// Create a new verifier with default tolerance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sync_points: Vec::new(),
            tolerance: SyncTolerance::default(),
        }
    }

    /// Set the tolerance.
    #[must_use]
    pub fn with_tolerance(mut self, tolerance: SyncTolerance) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Add a sync point.
    pub fn add_sync_point(&mut self, point: SyncPoint) {
        self.sync_points.push(point);
    }

    /// Run verification and return the result for the given clip id.
    #[must_use]
    pub fn verify(&self, clip_id: impl Into<String>) -> SyncVerificationResult {
        let clip_id = clip_id.into();
        if self.sync_points.is_empty() {
            return SyncVerificationResult {
                clip_id,
                in_sync: true,
                max_drift_frames: 0,
                points_checked: 0,
                points_passed: 0,
            };
        }

        let mut max_drift = 0i64;
        let mut passed = 0usize;

        for sp in &self.sync_points {
            let drift = sp.frame_offset().abs();
            if drift > max_drift {
                max_drift = drift;
            }
            if drift <= self.tolerance.max_drift_frames as i64 {
                passed += 1;
            }
        }

        let total = self.sync_points.len();
        let pass_rate = passed as f32 / total as f32;

        SyncVerificationResult {
            clip_id,
            in_sync: pass_rate >= self.tolerance.min_pass_rate,
            max_drift_frames: max_drift,
            points_checked: total,
            points_passed: passed,
        }
    }

    /// Number of sync points registered.
    #[must_use]
    pub fn point_count(&self) -> usize {
        self.sync_points.len()
    }
}

impl Default for ProxySyncVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Aligns proxy timecode to original timecode using a known offset.
#[allow(dead_code)]
pub struct TimecodeAligner {
    /// Known frame offset (proxy_frame = original_frame + offset).
    offset_frames: i64,
    /// Frame rate used for alignment.
    fps_num: u32,
    /// Frame rate denominator.
    fps_den: u32,
}

impl TimecodeAligner {
    /// Create an aligner with a known offset.
    #[must_use]
    pub fn new(offset_frames: i64, fps_num: u32, fps_den: u32) -> Self {
        Self {
            offset_frames,
            fps_num,
            fps_den,
        }
    }

    /// Create an aligner with zero offset.
    #[must_use]
    pub fn zero(fps_num: u32, fps_den: u32) -> Self {
        Self::new(0, fps_num, fps_den)
    }

    /// Compute the proxy frame number for a given original frame number.
    #[must_use]
    pub fn original_to_proxy(&self, original_frame: u64) -> u64 {
        (original_frame as i64 + self.offset_frames).max(0) as u64
    }

    /// Compute the original frame number for a given proxy frame number.
    #[must_use]
    pub fn proxy_to_original(&self, proxy_frame: u64) -> u64 {
        (proxy_frame as i64 - self.offset_frames).max(0) as u64
    }

    /// Get the frame offset.
    #[must_use]
    pub fn offset_frames(&self) -> i64 {
        self.offset_frames
    }

    /// Offset expressed in seconds.
    #[must_use]
    pub fn offset_seconds(&self) -> f64 {
        self.offset_frames as f64 / (self.fps_num as f64 / self.fps_den as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_timecode_fps_f64() {
        let tc = FrameTimecode::new(0, 24, 1);
        assert!((tc.fps_f64() - 24.0).abs() < 1e-10);
    }

    #[test]
    fn test_frame_timecode_from_hmsf() {
        // 1 hour at 24fps = 86400 frames
        let tc = FrameTimecode::from_hmsf(1, 0, 0, 0, 24);
        assert_eq!(tc.frame_number, 86400);
    }

    #[test]
    fn test_frame_timecode_as_seconds() {
        let tc = FrameTimecode::new(240, 24, 1);
        assert!((tc.as_seconds() - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_frame_timecode_frame_diff() {
        let tc1 = FrameTimecode::new(100, 24, 1);
        let tc2 = FrameTimecode::new(95, 24, 1);
        assert_eq!(tc1.frame_diff(&tc2), 5);
        assert_eq!(tc2.frame_diff(&tc1), -5);
    }

    #[test]
    fn test_sync_point_frame_offset_zero() {
        let tc = FrameTimecode::new(100, 24, 1);
        let sp = SyncPoint::new(tc, tc);
        assert_eq!(sp.frame_offset(), 0);
    }

    #[test]
    fn test_sync_point_frame_offset_nonzero() {
        let proxy_tc = FrameTimecode::new(105, 24, 1);
        let orig_tc = FrameTimecode::new(100, 24, 1);
        let sp = SyncPoint::new(proxy_tc, orig_tc);
        assert_eq!(sp.frame_offset(), 5);
    }

    #[test]
    fn test_sync_point_verified() {
        let tc = FrameTimecode::new(100, 24, 1);
        let sp = SyncPoint::new(tc, tc).verified();
        assert!(sp.verified);
    }

    #[test]
    fn test_sync_tolerance_default() {
        let t = SyncTolerance::default();
        assert_eq!(t.max_drift_frames, 1);
        assert!((t.min_pass_rate - 0.95).abs() < 1e-5);
    }

    #[test]
    fn test_sync_tolerance_strict() {
        let t = SyncTolerance::strict();
        assert_eq!(t.max_drift_frames, 0);
        assert!((t.min_pass_rate - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_proxy_sync_verifier_empty() {
        let verifier = ProxySyncVerifier::new();
        let result = verifier.verify("clip001");
        assert!(result.in_sync);
        assert_eq!(result.points_checked, 0);
    }

    #[test]
    fn test_proxy_sync_verifier_all_in_sync() {
        let mut verifier = ProxySyncVerifier::new();
        let tc = FrameTimecode::new(100, 24, 1);
        verifier.add_sync_point(SyncPoint::new(tc, tc));
        verifier.add_sync_point(SyncPoint::new(
            FrameTimecode::new(200, 24, 1),
            FrameTimecode::new(200, 24, 1),
        ));
        let result = verifier.verify("clip001");
        assert!(result.in_sync);
        assert_eq!(result.max_drift_frames, 0);
    }

    #[test]
    fn test_proxy_sync_verifier_drift_exceeds_tolerance() {
        let mut verifier = ProxySyncVerifier::new().with_tolerance(SyncTolerance::strict());
        verifier.add_sync_point(SyncPoint::new(
            FrameTimecode::new(105, 24, 1),
            FrameTimecode::new(100, 24, 1),
        ));
        let result = verifier.verify("clip001");
        assert!(!result.in_sync);
        assert_eq!(result.max_drift_frames, 5);
    }

    #[test]
    fn test_timecode_aligner_original_to_proxy() {
        let aligner = TimecodeAligner::new(10, 24, 1);
        assert_eq!(aligner.original_to_proxy(100), 110);
    }

    #[test]
    fn test_timecode_aligner_proxy_to_original() {
        let aligner = TimecodeAligner::new(10, 24, 1);
        assert_eq!(aligner.proxy_to_original(110), 100);
    }

    #[test]
    fn test_timecode_aligner_zero() {
        let aligner = TimecodeAligner::zero(25, 1);
        assert_eq!(aligner.original_to_proxy(500), 500);
        assert_eq!(aligner.proxy_to_original(500), 500);
    }

    #[test]
    fn test_timecode_aligner_offset_seconds() {
        let aligner = TimecodeAligner::new(48, 24, 1); // 2 seconds at 24fps
        assert!((aligner.offset_seconds() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_sync_verification_pass_rate() {
        let result = SyncVerificationResult {
            clip_id: "c1".to_string(),
            in_sync: true,
            max_drift_frames: 0,
            points_checked: 10,
            points_passed: 9,
        };
        assert!((result.pass_rate() - 0.9).abs() < 1e-5);
    }
}
