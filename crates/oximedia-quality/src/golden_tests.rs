//! Golden reference tests comparing metric outputs against known-correct values.
//!
//! These tests assert that the quality metric implementations produce outputs
//! within a tight tolerance of analytically computable or externally verified
//! reference values.  They serve as regression guards: if a refactor changes
//! the numerical output, these tests will catch it immediately.
//!
//! # Reference derivations
//!
//! ## PSNR
//! For two uniform-grey frames of value A and B on an 8-bit plane:
//! - MSE = (A − B)²
//! - PSNR = 10 · log₁₀(255² / MSE)
//!
//! ## SSIM (uniform planes)
//! When both planes have constant pixel values μ₁ and μ₂ and σ = 0:
//! - SSIM = (2μ₁μ₂ + C1) / (μ₁² + μ₂² + C1)
//!
//! ## VIF
//! Identical uniform frames → VIF = 1.0 (lossless).

#![allow(dead_code)]

#[cfg(test)]
mod tests {
    use crate::{Frame, PsnrCalculator, SsimCalculator, VifCalculator, VmafCalculator};
    use oximedia_core::PixelFormat;

    // ── Helpers ───────────────────────────────────────────────────────────

    fn uniform_yuv420(width: usize, height: usize, y: u8, cb: u8, cr: u8) -> Frame {
        let mut f =
            Frame::new(width, height, PixelFormat::Yuv420p).expect("should succeed in test");
        f.planes[0].fill(y);
        f.planes[1].fill(cb);
        f.planes[2].fill(cr);
        f
    }

    fn grey_yuv420(width: usize, height: usize, y: u8) -> Frame {
        uniform_yuv420(width, height, y, 128, 128)
    }

    /// Analytically compute weighted PSNR for two uniform YUV420 frames.
    ///
    /// For plane P, MSE = (y1 − y2)² and PSNR_P = 10·log10(255²/MSE).
    /// The luma (Y) plane uses weight 4/6; each chroma plane uses 1/6.
    fn expected_psnr(y1: u8, y2: u8, cb1: u8, cb2: u8, cr1: u8, cr2: u8) -> f64 {
        fn plane_psnr(a: u8, b: u8) -> f64 {
            let mse = (i32::from(a) - i32::from(b)).pow(2) as f64;
            if mse < 1e-10 {
                return 100.0;
            }
            10.0 * (255.0_f64.powi(2) / mse).log10()
        }
        let y_psnr = plane_psnr(y1, y2);
        let cb_psnr = plane_psnr(cb1, cb2);
        let cr_psnr = plane_psnr(cr1, cr2);
        (4.0 / 6.0) * y_psnr + (1.0 / 6.0) * (cb_psnr + cr_psnr)
    }

    /// Analytically compute SSIM for two uniform constant-value planes.
    fn expected_ssim_uniform(mu1: f64, mu2: f64) -> f64 {
        // σ = 0, σ_xy = 0
        let l = 255.0_f64;
        let c1 = (0.01 * l).powi(2);
        let c2 = (0.03 * l).powi(2);
        // luminance component
        let lum = (2.0 * mu1 * mu2 + c1) / (mu1.powi(2) + mu2.powi(2) + c1);
        // contrast + structure = C2 / C2 = 1 when σ = 0
        let cs = c2 / c2;
        lum * cs
    }

    // ── PSNR golden tests ─────────────────────────────────────────────────

    #[test]
    fn golden_psnr_uniform_diff_10() {
        // Y: 100 vs 110 → MSE = 100, PSNR_Y = 10·log10(65025/100) ≈ 28.13
        // Cb/Cr identical → PSNR = 100
        let calc = PsnrCalculator::new();
        let f1 = grey_yuv420(32, 32, 100);
        let f2 = grey_yuv420(32, 32, 110);

        let result = calc.calculate(&f1, &f2).expect("should succeed");
        let expected = expected_psnr(100, 110, 128, 128, 128, 128);

        assert!(
            (result.score - expected).abs() < 0.05,
            "PSNR={:.4} expected={:.4}",
            result.score,
            expected
        );
    }

    #[test]
    fn golden_psnr_identical_returns_high_value() {
        let calc = PsnrCalculator::new();
        let f = grey_yuv420(32, 32, 128);
        let result = calc.calculate(&f, &f).expect("should succeed");
        assert!(
            result.score >= 99.0,
            "Identical frames must yield very high PSNR, got {}",
            result.score
        );
    }

    #[test]
    fn golden_psnr_known_mse() {
        // Y: 0 vs 255 → MSE = 65025 → PSNR_Y = 10·log10(65025/65025) = 0 dB
        let calc = PsnrCalculator::new();
        let f1 = grey_yuv420(32, 32, 0);
        let f2 = grey_yuv420(32, 32, 255);

        let y_psnr = result_y_component(&calc.calculate(&f1, &f2).expect("should succeed"));
        assert!(
            y_psnr.abs() < 0.1,
            "PSNR for max difference must be ≈ 0 dB, got {y_psnr}"
        );
    }

    fn result_y_component(score: &crate::QualityScore) -> f64 {
        *score.components.get("Y").unwrap_or(&0.0)
    }

    #[test]
    fn golden_psnr_diff_1() {
        // Y: 127 vs 128 → MSE = 1 → PSNR_Y = 10·log10(65025) ≈ 48.13
        let calc = PsnrCalculator::new();
        let f1 = grey_yuv420(64, 64, 127);
        let f2 = grey_yuv420(64, 64, 128);

        let y_psnr = result_y_component(&calc.calculate(&f1, &f2).expect("should succeed"));
        let expected_y = 10.0 * (255.0_f64.powi(2) / 1.0).log10();
        assert!(
            (y_psnr - expected_y).abs() < 0.05,
            "PSNR_Y={y_psnr:.4} expected={expected_y:.4}"
        );
    }

    // ── SSIM golden tests ─────────────────────────────────────────────────

    #[test]
    fn golden_ssim_identical_frames() {
        let calc = SsimCalculator::new();
        let f = grey_yuv420(64, 64, 128);
        let result = calc.calculate(&f, &f).expect("should succeed");
        assert!(
            (result.score - 1.0).abs() < 0.02,
            "SSIM of identical frames must be ~1.0, got {}",
            result.score
        );
    }

    #[test]
    fn golden_ssim_uniform_frames_analytical() {
        let calc = SsimCalculator::new();
        let f1 = grey_yuv420(64, 64, 100);
        let f2 = grey_yuv420(64, 64, 200);

        let result = calc.calculate(&f1, &f2).expect("should succeed");
        let y_ssim = *result.components.get("Y").unwrap_or(&0.0);

        let analytical = expected_ssim_uniform(100.0, 200.0);
        assert!(
            (y_ssim - analytical).abs() < 0.02,
            "SSIM_Y={y_ssim:.6} analytical={analytical:.6}"
        );
    }

    #[test]
    fn golden_ssim_max_difference_low() {
        let calc = SsimCalculator::new();
        let f1 = grey_yuv420(64, 64, 0);
        let f2 = grey_yuv420(64, 64, 255);
        let result = calc.calculate(&f1, &f2).expect("should succeed");
        // For max contrast difference, SSIM must be clearly below 0.5
        assert!(
            result.score < 0.5,
            "Max-difference SSIM must be < 0.5, got {}",
            result.score
        );
    }

    // ── VIF golden tests ──────────────────────────────────────────────────

    #[test]
    fn golden_vif_identical_frames_returns_one() {
        let calc = VifCalculator::new();
        let f = grey_yuv420(64, 64, 128);
        let result = calc.calculate(&f, &f).expect("should succeed");
        assert!(
            (result.score - 1.0).abs() < 0.05,
            "VIF of identical frames must be ~1.0, got {}",
            result.score
        );
    }

    #[test]
    fn golden_vif_distorted_less_than_identical() {
        let calc = VifCalculator::new();
        let ref_frame = grey_yuv420(64, 64, 128);
        let identical = grey_yuv420(64, 64, 128);
        let distorted = grey_yuv420(64, 64, 90);

        let vif_identical = calc
            .calculate(&ref_frame, &identical)
            .expect("should succeed")
            .score;
        let vif_distorted = calc
            .calculate(&ref_frame, &distorted)
            .expect("should succeed")
            .score;

        assert!(
            vif_identical >= vif_distorted,
            "identical VIF={vif_identical} must be >= distorted VIF={vif_distorted}"
        );
    }

    // ── VMAF golden tests ─────────────────────────────────────────────────

    #[test]
    fn golden_vmaf_in_range() {
        let calc = VmafCalculator::new();
        let f1 = grey_yuv420(128, 128, 128);
        let f2 = grey_yuv420(128, 128, 128);
        let result = calc.calculate(&f1, &f2).expect("should succeed");
        assert!(
            result.score >= 0.0 && result.score <= 100.0,
            "VMAF must be in [0, 100], got {}",
            result.score
        );
    }

    #[test]
    fn golden_vmaf_identical_frames_high() {
        let calc = VmafCalculator::new();
        let f = grey_yuv420(128, 128, 128);
        let result = calc.calculate(&f, &f).expect("should succeed");
        // Identical frames should yield a high VMAF
        assert!(
            result.score >= 50.0,
            "VMAF of identical frames must be >= 50, got {}",
            result.score
        );
    }

    #[test]
    fn golden_vmaf_components_present() {
        let calc = VmafCalculator::new();
        let f1 = grey_yuv420(64, 64, 100);
        let f2 = grey_yuv420(64, 64, 120);
        let result = calc.calculate(&f1, &f2).expect("should succeed");
        assert!(result.components.contains_key("VIF"));
        assert!(result.components.contains_key("DLM"));
        assert!(result.components.contains_key("Motion"));
    }

    // ── Region PSNR / SSIM golden tests ───────────────────────────────────

    #[test]
    fn golden_region_psnr_matches_full_for_uniform() {
        // For a uniform frame, PSNR over any region should equal PSNR over full frame.
        let calc = PsnrCalculator::new();
        let f1 = grey_yuv420(64, 64, 100);
        let f2 = grey_yuv420(64, 64, 110);

        let full = calc.calculate(&f1, &f2).expect("full frame");
        let region = calc
            .calculate_region(&f1, &f2, (8, 8, 32, 32))
            .expect("region");

        // For uniform frames the Y-plane PSNR should be identical regardless of region
        let full_y = *full.components.get("Y").unwrap_or(&0.0);
        let region_y = *region.components.get("Y").unwrap_or(&0.0);
        assert!(
            (full_y - region_y).abs() < 0.1,
            "full_Y={full_y:.4} region_Y={region_y:.4}"
        );
    }

    #[test]
    fn golden_region_ssim_identical_region_is_one() {
        let calc = SsimCalculator::new();
        let f = grey_yuv420(64, 64, 128);
        let result = calc
            .calculate_region(&f, &f, (4, 4, 32, 32))
            .expect("should succeed");
        assert!(
            (result.score - 1.0).abs() < 0.02,
            "SSIM of identical region must be ~1.0, got {}",
            result.score
        );
    }
}
