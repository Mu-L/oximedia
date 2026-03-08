//! Histogram-based image analysis: contrast detection and clipping checks.

#![allow(dead_code)]

/// A single histogram bucket covering a contiguous intensity range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistogramBucket {
    /// Inclusive lower bound of the bucket (0–255).
    pub lower: u8,
    /// Inclusive upper bound of the bucket (0–255).
    pub upper: u8,
    /// Number of pixels that fell in this bucket.
    pub count: u64,
    /// Total number of pixels considered.
    pub total: u64,
}

impl HistogramBucket {
    /// Create a new bucket.
    #[must_use]
    pub fn new(lower: u8, upper: u8, count: u64, total: u64) -> Self {
        Self {
            lower,
            upper,
            count,
            total,
        }
    }

    /// Fraction of pixels in this bucket (0.0 – 1.0).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn fill_ratio(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.count as f64 / self.total as f64
        }
    }
}

/// A 256-bin luminance histogram for a single image or frame.
#[derive(Debug, Clone)]
pub struct ImageHistogram {
    /// Bins indexed 0–255 by luma value.
    pub bins: [u64; 256],
    /// Total number of pixels.
    pub total_pixels: u64,
}

impl ImageHistogram {
    /// Build a histogram from an 8-bit luma plane.
    #[must_use]
    pub fn from_luma(data: &[u8]) -> Self {
        let mut bins = [0u64; 256];
        for &px in data {
            bins[px as usize] += 1;
        }
        Self {
            bins,
            total_pixels: data.len() as u64,
        }
    }

    /// Weighted mean luma value (0–255).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn mean(&self) -> f64 {
        if self.total_pixels == 0 {
            return 0.0;
        }
        let sum: u64 = self
            .bins
            .iter()
            .enumerate()
            .map(|(i, &c)| i as u64 * c)
            .sum();
        sum as f64 / self.total_pixels as f64
    }

    /// Standard deviation of luma values.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn std_dev(&self) -> f64 {
        if self.total_pixels == 0 {
            return 0.0;
        }
        let mean = self.mean();
        let variance: f64 = self
            .bins
            .iter()
            .enumerate()
            .map(|(i, &c)| {
                let diff = i as f64 - mean;
                diff * diff * c as f64
            })
            .sum::<f64>()
            / self.total_pixels as f64;
        variance.sqrt()
    }

    /// `true` when the image appears low-contrast (std dev < threshold).
    #[must_use]
    pub fn is_low_contrast(&self, threshold: f64) -> bool {
        self.std_dev() < threshold
    }

    /// Fraction of pixels at or above `level` (potential highlight clipping).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn highlight_clip_ratio(&self, level: u8) -> f64 {
        if self.total_pixels == 0 {
            return 0.0;
        }
        let clipped: u64 = self.bins[level as usize..].iter().sum();
        clipped as f64 / self.total_pixels as f64
    }

    /// Fraction of pixels at or below `level` (potential shadow clipping).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn shadow_clip_ratio(&self, level: u8) -> f64 {
        if self.total_pixels == 0 {
            return 0.0;
        }
        let clipped: u64 = self.bins[..=level as usize].iter().sum();
        clipped as f64 / self.total_pixels as f64
    }
}

impl Default for ImageHistogram {
    fn default() -> Self {
        Self {
            bins: [0; 256],
            total_pixels: 0,
        }
    }
}

/// Result of a histogram analysis pass.
#[derive(Debug, Clone)]
pub struct HistogramAnalysisResult {
    /// Mean luma.
    pub mean: f64,
    /// Standard deviation.
    pub std_dev: f64,
    /// Shadow clipping ratio (below level 16).
    pub shadow_clip: f64,
    /// Highlight clipping ratio (above level 235).
    pub highlight_clip: f64,
    /// Whether the image is low-contrast.
    pub low_contrast: bool,
}

/// Stateless analyzer that processes `ImageHistogram` values.
#[derive(Debug, Default)]
pub struct HistogramAnalyzer {
    /// Threshold below which an image is considered low-contrast.
    pub contrast_threshold: f64,
    /// Luma level above which pixels count as highlight clipping.
    pub highlight_level: u8,
    /// Luma level below which pixels count as shadow clipping.
    pub shadow_level: u8,
}

impl HistogramAnalyzer {
    /// Create a new analyzer with broadcast-safe defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            contrast_threshold: 20.0,
            highlight_level: 235,
            shadow_level: 16,
        }
    }

    /// Analyze a histogram and return the result.
    #[must_use]
    pub fn analyze(&self, h: &ImageHistogram) -> HistogramAnalysisResult {
        HistogramAnalysisResult {
            mean: h.mean(),
            std_dev: h.std_dev(),
            shadow_clip: h.shadow_clip_ratio(self.shadow_level),
            highlight_clip: h.highlight_clip_ratio(self.highlight_level),
            low_contrast: h.is_low_contrast(self.contrast_threshold),
        }
    }

    /// Detect clipping: returns `(shadow_clipped, highlight_clipped)`.
    #[must_use]
    pub fn detect_clipping(&self, h: &ImageHistogram) -> (bool, bool) {
        let shadow = h.shadow_clip_ratio(self.shadow_level) > 0.01;
        let highlight = h.highlight_clip_ratio(self.highlight_level) > 0.01;
        (shadow, highlight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform_histogram(value: u8, count: u64) -> ImageHistogram {
        let data: Vec<u8> = vec![value; count as usize];
        ImageHistogram::from_luma(&data)
    }

    #[test]
    fn test_bucket_fill_ratio_zero_total() {
        let b = HistogramBucket::new(0, 10, 0, 0);
        assert_eq!(b.fill_ratio(), 0.0);
    }

    #[test]
    fn test_bucket_fill_ratio() {
        let b = HistogramBucket::new(0, 10, 50, 100);
        assert!((b.fill_ratio() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_histogram_from_luma_single_value() {
        let h = uniform_histogram(128, 10);
        assert_eq!(h.bins[128], 10);
        assert_eq!(h.total_pixels, 10);
    }

    #[test]
    fn test_histogram_mean_uniform() {
        let h = uniform_histogram(100, 100);
        assert!((h.mean() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_histogram_std_dev_uniform() {
        // All pixels same value → std dev = 0.
        let h = uniform_histogram(128, 50);
        assert!(h.std_dev() < 1e-9);
    }

    #[test]
    fn test_histogram_std_dev_bimodal() {
        // Half black, half white.
        let mut data = vec![0u8; 50];
        data.extend(vec![255u8; 50]);
        let h = ImageHistogram::from_luma(&data);
        assert!(h.std_dev() > 100.0);
    }

    #[test]
    fn test_is_low_contrast_true() {
        let h = uniform_histogram(128, 100);
        assert!(h.is_low_contrast(20.0));
    }

    #[test]
    fn test_is_low_contrast_false() {
        let mut data = vec![0u8; 50];
        data.extend(vec![255u8; 50]);
        let h = ImageHistogram::from_luma(&data);
        assert!(!h.is_low_contrast(20.0));
    }

    #[test]
    fn test_highlight_clip_ratio_all_white() {
        let h = uniform_histogram(255, 100);
        assert!(h.highlight_clip_ratio(235) > 0.99);
    }

    #[test]
    fn test_shadow_clip_ratio_all_black() {
        let h = uniform_histogram(0, 100);
        assert!(h.shadow_clip_ratio(16) > 0.99);
    }

    #[test]
    fn test_analyzer_low_contrast_detected() {
        let analyzer = HistogramAnalyzer::new();
        let h = uniform_histogram(128, 100);
        let result = analyzer.analyze(&h);
        assert!(result.low_contrast);
    }

    #[test]
    fn test_analyzer_detect_highlight_clipping() {
        let analyzer = HistogramAnalyzer::new();
        let h = uniform_histogram(255, 100);
        let (_, highlight) = analyzer.detect_clipping(&h);
        assert!(highlight);
    }

    #[test]
    fn test_analyzer_detect_shadow_clipping() {
        let analyzer = HistogramAnalyzer::new();
        let h = uniform_histogram(0, 100);
        let (shadow, _) = analyzer.detect_clipping(&h);
        assert!(shadow);
    }

    #[test]
    fn test_default_histogram_empty() {
        let h = ImageHistogram::default();
        assert_eq!(h.total_pixels, 0);
        assert_eq!(h.mean(), 0.0);
    }
}
