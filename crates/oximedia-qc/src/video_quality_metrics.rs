//! Video quality measurement algorithms.
//!
//! Provides pure-Rust implementations of common objective video quality metrics:
//! - **MSE / PSNR** – Mean Squared Error and Peak Signal-to-Noise Ratio
//! - **SSIM patch** – Structural Similarity Index on an 8-bit patch
//! - **Blockiness** – DCT-block edge artefact score

// ── MSE / PSNR ────────────────────────────────────────────────────────────────

/// Computes Mean Squared Error between two equal-length byte slices.
///
/// Returns 0.0 if either slice is empty or they differ in length.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn compute_mse(original: &[u8], compressed: &[u8]) -> f64 {
    if original.is_empty() || original.len() != compressed.len() {
        return 0.0;
    }
    let sum: f64 = original
        .iter()
        .zip(compressed.iter())
        .map(|(&a, &b)| {
            let diff = f64::from(a) - f64::from(b);
            diff * diff
        })
        .sum();
    sum / original.len() as f64
}

/// Computes Peak Signal-to-Noise Ratio from MSE.
///
/// Formula: `10 * log10(max_val² / mse)`.
///
/// Returns `f64::INFINITY` when `mse == 0.0` (lossless).
#[must_use]
pub fn compute_psnr(mse: f64, max_val: f64) -> f64 {
    if mse == 0.0 {
        return f64::INFINITY;
    }
    10.0 * (max_val * max_val / mse).log10()
}

/// Per-component PSNR result for a YUV frame.
#[derive(Debug, Clone, PartialEq)]
pub struct PsnrResult {
    /// PSNR of the luma (Y) plane in dB.
    pub y_psnr: f64,
    /// PSNR of the blue-difference chroma (U/Cb) plane in dB.
    pub u_psnr: f64,
    /// PSNR of the red-difference chroma (V/Cr) plane in dB.
    pub v_psnr: f64,
    /// Weighted average PSNR (6:1:1 weighting matching YUV 4:2:0 area).
    pub psnr_avg: f64,
}

impl PsnrResult {
    /// Creates a new PSNR result.
    #[must_use]
    pub fn new(y_psnr: f64, u_psnr: f64, v_psnr: f64) -> Self {
        // Weighted average: Y contributes 4× more area in 4:2:0
        let avg = (4.0 * y_psnr + u_psnr + v_psnr) / 6.0;
        Self {
            y_psnr,
            u_psnr,
            v_psnr,
            psnr_avg: avg,
        }
    }

    /// Returns `true` if the average PSNR meets or exceeds `threshold` dB.
    #[must_use]
    pub fn is_acceptable(&self, threshold: f64) -> bool {
        self.psnr_avg >= threshold
    }
}

// ── SSIM patch ────────────────────────────────────────────────────────────────

/// Computes the Structural Similarity Index (SSIM) between two patches.
///
/// Both slices must be equal-length. Returns 0.0 for empty or mismatched input.
///
/// Stabilisation constants: C₁ = (0.01 × 255)², C₂ = (0.03 × 255)².
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn compute_ssim_patch(patch1: &[f32], patch2: &[f32]) -> f64 {
    if patch1.is_empty() || patch1.len() != patch2.len() {
        return 0.0;
    }

    let n = patch1.len() as f64;
    let mu1: f64 = patch1.iter().map(|&v| f64::from(v)).sum::<f64>() / n;
    let mu2: f64 = patch2.iter().map(|&v| f64::from(v)).sum::<f64>() / n;

    let var1: f64 = patch1
        .iter()
        .map(|&v| {
            let d = f64::from(v) - mu1;
            d * d
        })
        .sum::<f64>()
        / n;
    let var2: f64 = patch2
        .iter()
        .map(|&v| {
            let d = f64::from(v) - mu2;
            d * d
        })
        .sum::<f64>()
        / n;

    let cov: f64 = patch1
        .iter()
        .zip(patch2.iter())
        .map(|(&a, &b)| (f64::from(a) - mu1) * (f64::from(b) - mu2))
        .sum::<f64>()
        / n;

    // SSIM stabilisation constants for 8-bit images (L = 255)
    const C1: f64 = (0.01 * 255.0) * (0.01 * 255.0);
    const C2: f64 = (0.03 * 255.0) * (0.03 * 255.0);

    let num = (2.0 * mu1 * mu2 + C1) * (2.0 * cov + C2);
    let den = (mu1 * mu1 + mu2 * mu2 + C1) * (var1 + var2 + C2);

    if den == 0.0 {
        return 1.0;
    }
    num / den
}

/// Result of an SSIM computation over a frame.
#[derive(Debug, Clone, PartialEq)]
pub struct SsimResult {
    /// Mean SSIM value over all evaluated patches (0.0–1.0).
    pub value: f64,
    /// Number of patches that were evaluated.
    pub map_size: usize,
}

impl SsimResult {
    /// Creates a new SSIM result.
    #[must_use]
    pub fn new(value: f64, map_size: usize) -> Self {
        Self { value, map_size }
    }

    /// Returns `true` if the SSIM value is above the "good quality" threshold (0.95).
    #[must_use]
    pub fn is_good(&self) -> bool {
        self.value > 0.95
    }
}

// ── Blockiness ────────────────────────────────────────────────────────────────

/// Blockiness score for a video frame, indicating DCT-block edge artefacts.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockinessScore {
    /// Mean absolute difference across block boundaries (higher = more blocking).
    pub score: f32,
    /// Block size used during analysis (e.g., 8 for DCT-8).
    pub block_size: u32,
}

impl BlockinessScore {
    /// Creates a new blockiness score.
    #[must_use]
    pub fn new(score: f32, block_size: u32) -> Self {
        Self { score, block_size }
    }

    /// Returns `true` if the blockiness score exceeds the significance threshold (5.0).
    #[must_use]
    pub fn is_significant(&self) -> bool {
        self.score > 5.0
    }
}

/// Computes a blockiness score for a luma plane stored as a flat byte slice.
///
/// The metric is the mean absolute difference between adjacent rows/columns at
/// every DCT-block boundary. A higher value indicates more visible blocking.
///
/// # Arguments
/// * `pixels` – Row-major luma plane (length must be a multiple of `width`).
/// * `width`  – Scanline width in pixels.
/// * `block_size` – Block size to test for boundaries (typically 8 or 16).
///
/// Returns a zero score when the input is empty or `width` is 0.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn compute_blockiness(pixels: &[u8], width: usize, block_size: u32) -> BlockinessScore {
    if pixels.is_empty() || width == 0 || block_size == 0 {
        return BlockinessScore::new(0.0, block_size);
    }

    let height = pixels.len() / width;
    if height == 0 {
        return BlockinessScore::new(0.0, block_size);
    }

    let bs = block_size as usize;
    let mut total_diff: f64 = 0.0;
    let mut count: usize = 0;

    // Horizontal block boundaries: difference between row y and row y-1
    // at each multiple-of-bs row.
    let mut row = bs;
    while row < height {
        for col in 0..width {
            let above = pixels[(row - 1) * width + col] as f64;
            let below = pixels[row * width + col] as f64;
            total_diff += (below - above).abs();
            count += 1;
        }
        row += bs;
    }

    // Vertical block boundaries: difference between col x and col x-1
    // at each multiple-of-bs column.
    let mut col = bs;
    while col < width {
        for r in 0..height {
            let left = pixels[r * width + (col - 1)] as f64;
            let right = pixels[r * width + col] as f64;
            total_diff += (right - left).abs();
            count += 1;
        }
        col += bs;
    }

    let score = if count > 0 {
        (total_diff / count as f64) as f32
    } else {
        0.0
    };

    BlockinessScore::new(score, block_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── compute_mse ───────────────────────────────────────────────────────────

    #[test]
    fn test_mse_identical_slices() {
        let data: Vec<u8> = (0..16).collect();
        let mse = compute_mse(&data, &data);
        assert_eq!(mse, 0.0);
    }

    #[test]
    fn test_mse_known_value() {
        let orig = vec![0u8, 0, 0, 0];
        let comp = vec![2u8, 2, 2, 2];
        // MSE = (4 + 4 + 4 + 4) / 4 = 4.0
        let mse = compute_mse(&orig, &comp);
        assert!((mse - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_mse_empty_returns_zero() {
        assert_eq!(compute_mse(&[], &[]), 0.0);
    }

    #[test]
    fn test_mse_mismatched_lengths_returns_zero() {
        assert_eq!(compute_mse(&[1, 2], &[1, 2, 3]), 0.0);
    }

    // ── compute_psnr ─────────────────────────────────────────────────────────

    #[test]
    fn test_psnr_zero_mse_is_infinity() {
        assert!(compute_psnr(0.0, 255.0).is_infinite());
    }

    #[test]
    fn test_psnr_known_value() {
        // MSE=4, max=255 → PSNR = 10*log10(255²/4) = 10*log10(65025/4)
        let psnr = compute_psnr(4.0, 255.0);
        let expected = 10.0 * (255.0_f64 * 255.0 / 4.0).log10();
        assert!((psnr - expected).abs() < 1e-9);
    }

    // ── PsnrResult ────────────────────────────────────────────────────────────

    #[test]
    fn test_psnr_result_average_weight() {
        let r = PsnrResult::new(40.0, 40.0, 40.0);
        assert!((r.psnr_avg - 40.0).abs() < 1e-9);
    }

    #[test]
    fn test_psnr_result_is_acceptable_true() {
        let r = PsnrResult::new(45.0, 42.0, 42.0);
        assert!(r.is_acceptable(30.0));
    }

    #[test]
    fn test_psnr_result_is_acceptable_false() {
        let r = PsnrResult::new(20.0, 18.0, 18.0);
        assert!(!r.is_acceptable(30.0));
    }

    // ── compute_ssim_patch ────────────────────────────────────────────────────

    #[test]
    fn test_ssim_identical_patches() {
        let patch: Vec<f32> = (0..64).map(|i| i as f32).collect();
        let ssim = compute_ssim_patch(&patch, &patch);
        assert!((ssim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_ssim_empty_returns_zero() {
        assert_eq!(compute_ssim_patch(&[], &[]), 0.0);
    }

    #[test]
    fn test_ssim_mismatched_returns_zero() {
        assert_eq!(compute_ssim_patch(&[1.0, 2.0], &[1.0, 2.0, 3.0]), 0.0);
    }

    #[test]
    fn test_ssim_range_bounds() {
        let p1: Vec<f32> = vec![0.0; 64];
        let p2: Vec<f32> = vec![255.0; 64];
        let ssim = compute_ssim_patch(&p1, &p2);
        assert!(ssim >= -1.0 && ssim <= 1.0);
    }

    // ── SsimResult ────────────────────────────────────────────────────────────

    #[test]
    fn test_ssim_result_is_good_true() {
        let r = SsimResult::new(0.97, 100);
        assert!(r.is_good());
    }

    #[test]
    fn test_ssim_result_is_good_false() {
        let r = SsimResult::new(0.90, 100);
        assert!(!r.is_good());
    }

    // ── compute_blockiness ────────────────────────────────────────────────────

    #[test]
    fn test_blockiness_empty_returns_zero() {
        let bs = compute_blockiness(&[], 8, 8);
        assert_eq!(bs.score, 0.0);
    }

    #[test]
    fn test_blockiness_uniform_frame_is_zero() {
        // A flat grey frame: no differences at boundaries
        let pixels = vec![128u8; 64 * 64];
        let bs = compute_blockiness(&pixels, 64, 8);
        assert_eq!(bs.score, 0.0);
        assert!(!bs.is_significant());
    }

    #[test]
    fn test_blockiness_score_stored() {
        let pixels = vec![128u8; 64 * 64];
        let bs = compute_blockiness(&pixels, 64, 8);
        assert_eq!(bs.block_size, 8);
    }

    #[test]
    fn test_blockiness_is_significant_false_below_threshold() {
        let bs = BlockinessScore::new(3.0, 8);
        assert!(!bs.is_significant());
    }

    #[test]
    fn test_blockiness_is_significant_true_above_threshold() {
        let bs = BlockinessScore::new(10.0, 8);
        assert!(bs.is_significant());
    }
}
