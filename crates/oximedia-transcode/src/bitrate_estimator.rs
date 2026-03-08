//! Bitrate estimation from QP/CRF, resolution, and frame-rate parameters.

/// Estimates output bitrate (bits per second) from encode parameters.
///
/// The model is a simplified empirical formula:
/// `bitrate ≈ base_bpp * pixels_per_frame * frame_rate * quality_factor`
///
/// where `quality_factor` decreases as QP/CRF increases.
///
/// # Example
///
/// ```
/// use oximedia_transcode::bitrate_estimator::BitrateEstimator;
///
/// let est = BitrateEstimator::new();
/// let bps = est.estimate_from_crf(23, 1920, 1080, 30.0);
/// assert!(bps > 0);
/// ```
#[derive(Debug, Clone)]
pub struct BitrateEstimator {
    /// Base bits-per-pixel at CRF/QP = 0.
    base_bpp: f64,
    /// Exponential decay coefficient for quality degradation with QP.
    decay: f64,
}

impl Default for BitrateEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl BitrateEstimator {
    /// Creates a `BitrateEstimator` with default empirical parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            base_bpp: 0.10, // 0.10 bits per pixel at lossless
            decay: 0.065,   // tuned to match real-world H.264/VP9 behaviour
        }
    }

    /// Creates a `BitrateEstimator` with custom parameters.
    ///
    /// * `base_bpp` – bits per pixel at QP = 0.
    /// * `decay` – exponential decay constant; higher = steeper quality/bitrate curve.
    #[must_use]
    pub fn with_params(base_bpp: f64, decay: f64) -> Self {
        Self { base_bpp, decay }
    }

    /// Estimates output bitrate in bits/s from a CRF value (0–51 for H.264/H.265).
    ///
    /// * `crf`        – Constant Rate Factor (0 = lossless, 51 = worst).
    /// * `width`      – Frame width in pixels.
    /// * `height`     – Frame height in pixels.
    /// * `frame_rate` – Frames per second.
    #[must_use]
    pub fn estimate_from_crf(&self, crf: u8, width: u32, height: u32, frame_rate: f64) -> u64 {
        self.estimate_from_qp(f64::from(crf), width, height, frame_rate)
    }

    /// Estimates output bitrate in bits/s from a floating-point QP value.
    ///
    /// * `qp`         – Quantization parameter.
    /// * `width`      – Frame width in pixels.
    /// * `height`     – Frame height in pixels.
    /// * `frame_rate` – Frames per second.
    #[must_use]
    pub fn estimate_from_qp(&self, qp: f64, width: u32, height: u32, frame_rate: f64) -> u64 {
        if frame_rate <= 0.0 || width == 0 || height == 0 {
            return 0;
        }
        let pixels = f64::from(width) * f64::from(height);
        let quality_factor = (-self.decay * qp).exp();
        let bps = self.base_bpp * pixels * frame_rate * quality_factor;
        bps.round() as u64
    }

    /// Estimates bitrate from a target VMAF score (0–100).
    ///
    /// Linearly maps VMAF → effective QP, then delegates to `estimate_from_qp`.
    /// VMAF 100 ≈ QP 0 (lossless), VMAF 0 ≈ QP 51 (worst).
    #[must_use]
    pub fn estimate_from_vmaf(&self, vmaf: f64, width: u32, height: u32, frame_rate: f64) -> u64 {
        let vmaf_clamped = vmaf.clamp(0.0, 100.0);
        let qp = 51.0 * (1.0 - vmaf_clamped / 100.0);
        self.estimate_from_qp(qp, width, height, frame_rate)
    }

    /// Infers the CRF value that would be required to hit a target bitrate.
    ///
    /// Returns `None` when the target cannot be reached with valid QP values (0–63).
    #[must_use]
    pub fn crf_for_target_bitrate(
        &self,
        target_bps: u64,
        width: u32,
        height: u32,
        frame_rate: f64,
    ) -> Option<u8> {
        if frame_rate <= 0.0 || width == 0 || height == 0 || target_bps == 0 {
            return None;
        }
        let pixels = f64::from(width) * f64::from(height);
        // bps = base_bpp * pixels * fps * e^(-decay * qp)
        // qp = -ln(bps / (base_bpp * pixels * fps)) / decay
        let denominator = self.base_bpp * pixels * frame_rate;
        if denominator <= 0.0 {
            return None;
        }
        let qp = -(target_bps as f64 / denominator).ln() / self.decay;
        if !(0.0..=63.0).contains(&qp) {
            return None;
        }
        Some(qp.round() as u8)
    }

    /// Returns an estimated size in bytes for encoding `duration_secs` of video.
    #[must_use]
    pub fn estimate_file_size(
        &self,
        crf: u8,
        width: u32,
        height: u32,
        frame_rate: f64,
        duration_secs: f64,
    ) -> u64 {
        let bps = self.estimate_from_crf(crf, width, height, frame_rate);
        ((bps as f64 * duration_secs) / 8.0).round() as u64
    }
}

/// A helper that bundles video parameters for convenience.
#[derive(Debug, Clone, Copy)]
pub struct VideoParams {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Frames per second.
    pub frame_rate: f64,
    /// CRF value (0–51 for most codecs).
    pub crf: u8,
}

impl VideoParams {
    /// Creates new video params.
    #[must_use]
    pub fn new(width: u32, height: u32, frame_rate: f64, crf: u8) -> Self {
        Self {
            width,
            height,
            frame_rate,
            crf,
        }
    }

    /// Estimates bitrate using a `BitrateEstimator`.
    #[must_use]
    pub fn estimate_bitrate(&self, estimator: &BitrateEstimator) -> u64 {
        estimator.estimate_from_crf(self.crf, self.width, self.height, self.frame_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_from_crf_positive() {
        let est = BitrateEstimator::new();
        let bps = est.estimate_from_crf(23, 1920, 1080, 30.0);
        assert!(bps > 0, "Expected positive bitrate, got {bps}");
    }

    #[test]
    fn test_lower_crf_higher_bitrate() {
        let est = BitrateEstimator::new();
        let high_quality = est.estimate_from_crf(18, 1920, 1080, 30.0);
        let low_quality = est.estimate_from_crf(28, 1920, 1080, 30.0);
        assert!(
            high_quality > low_quality,
            "CRF 18 should yield more bits than CRF 28"
        );
    }

    #[test]
    fn test_higher_resolution_higher_bitrate() {
        let est = BitrateEstimator::new();
        let fhd = est.estimate_from_crf(23, 1920, 1080, 30.0);
        let uhd = est.estimate_from_crf(23, 3840, 2160, 30.0);
        assert!(uhd > fhd, "4K should require more bits than 1080p");
    }

    #[test]
    fn test_higher_fps_higher_bitrate() {
        let est = BitrateEstimator::new();
        let fps30 = est.estimate_from_crf(23, 1920, 1080, 30.0);
        let fps60 = est.estimate_from_crf(23, 1920, 1080, 60.0);
        assert!(fps60 > fps30, "60 fps should require more bits than 30 fps");
        assert!(
            (fps60 as f64 / fps30 as f64 - 2.0).abs() < 0.01,
            "Should scale linearly with fps"
        );
    }

    #[test]
    fn test_zero_dimensions_returns_zero() {
        let est = BitrateEstimator::new();
        assert_eq!(est.estimate_from_crf(23, 0, 1080, 30.0), 0);
        assert_eq!(est.estimate_from_crf(23, 1920, 0, 30.0), 0);
        assert_eq!(est.estimate_from_crf(23, 1920, 1080, 0.0), 0);
    }

    #[test]
    fn test_vmaf_estimate_high_quality() {
        let est = BitrateEstimator::new();
        let high = est.estimate_from_vmaf(95.0, 1920, 1080, 30.0);
        let low = est.estimate_from_vmaf(50.0, 1920, 1080, 30.0);
        assert!(high > low, "VMAF 95 should need more bits than VMAF 50");
    }

    #[test]
    fn test_crf_for_target_bitrate_roundtrip() {
        let est = BitrateEstimator::new();
        let target_crf: u8 = 23;
        let bps = est.estimate_from_crf(target_crf, 1920, 1080, 30.0);
        if let Some(inferred_crf) = est.crf_for_target_bitrate(bps, 1920, 1080, 30.0) {
            // Allow ±1 due to rounding.
            assert!(
                (inferred_crf as i16 - target_crf as i16).abs() <= 1,
                "Expected CRF ~{target_crf}, got {inferred_crf}"
            );
        }
    }

    #[test]
    fn test_estimate_file_size() {
        let est = BitrateEstimator::new();
        let bytes = est.estimate_file_size(23, 1920, 1080, 30.0, 60.0); // 60 s clip
        assert!(bytes > 0);
        // File size in bytes = bps * duration / 8
        let bps = est.estimate_from_crf(23, 1920, 1080, 30.0);
        let expected = (bps as f64 * 60.0 / 8.0).round() as u64;
        assert_eq!(bytes, expected);
    }

    #[test]
    fn test_video_params_estimate_bitrate() {
        let params = VideoParams::new(1920, 1080, 30.0, 23);
        let est = BitrateEstimator::new();
        let bps = params.estimate_bitrate(&est);
        assert_eq!(bps, est.estimate_from_crf(23, 1920, 1080, 30.0));
    }

    #[test]
    fn test_custom_params() {
        let est = BitrateEstimator::with_params(0.2, 0.05);
        let bps = est.estimate_from_crf(20, 1280, 720, 25.0);
        assert!(bps > 0);
    }
}
