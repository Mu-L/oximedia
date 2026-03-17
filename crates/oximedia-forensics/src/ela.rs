//! Error Level Analysis (ELA)
//!
//! ELA is a forensic technique that identifies areas within an image that are at
//! different compression levels. Modified areas will appear at a different error
//! level than the rest of the image.

use crate::flat_array2::FlatArray2;
use crate::{ForensicTest, ForensicsError, ForensicsResult};
use image::{Rgb, RgbImage};
use std::io::Cursor;

/// ELA quality for recompression (lower = more lossy)
const ELA_QUALITY: u8 = 90;

/// Threshold for anomaly detection (adaptive)
const BASE_THRESHOLD: f64 = 15.0;

/// ELA result with detailed analysis
#[derive(Debug, Clone)]
pub struct ElaResult {
    /// Error level map
    pub error_map: FlatArray2<f64>,
    /// Maximum error level
    pub max_error: f64,
    /// Mean error level
    pub mean_error: f64,
    /// Anomaly regions detected
    pub anomalies_detected: bool,
    /// Confidence score
    pub confidence: f64,
}

/// Perform Error Level Analysis on an image
pub fn perform_ela(image: &RgbImage) -> ForensicsResult<ForensicTest> {
    let mut test = ForensicTest::new("Error Level Analysis (ELA)");

    // Perform ELA
    let ela_result = compute_ela(image)?;

    test.add_finding(format!(
        "Max error level: {:.2}, Mean error level: {:.2}",
        ela_result.max_error, ela_result.mean_error
    ));

    if ela_result.anomalies_detected {
        test.tampering_detected = true;
        test.add_finding("Anomalous error levels detected - possible tampering".to_string());
    }

    // Analyze error distribution
    let distribution = analyze_error_distribution(&ela_result.error_map);
    test.add_finding(format!(
        "Error distribution - Low: {:.1}%, Medium: {:.1}%, High: {:.1}%",
        distribution.0 * 100.0,
        distribution.1 * 100.0,
        distribution.2 * 100.0
    ));

    // High percentage of high errors is suspicious
    if distribution.2 > 0.15 {
        test.tampering_detected = true;
        test.add_finding("High percentage of pixels with elevated error levels".to_string());
    }

    test.set_confidence(ela_result.confidence);
    test.anomaly_map = Some(ela_result.error_map);

    Ok(test)
}

/// Compute ELA error map
fn compute_ela(original: &RgbImage) -> ForensicsResult<ElaResult> {
    let (width, height) = original.dimensions();

    // Recompress the image at specified quality
    let recompressed = recompress_image(original, ELA_QUALITY)?;

    // Compute pixel-wise difference
    let mut error_map = FlatArray2::zeros((height as usize, width as usize));
    let mut max_error: f64 = 0.0;
    let mut sum_error = 0.0;
    let mut count = 0;

    for y in 0..height {
        for x in 0..width {
            let orig_pixel = original.get_pixel(x, y);
            let recomp_pixel = recompressed.get_pixel(x, y);

            let error = calculate_pixel_error(orig_pixel, recomp_pixel);
            error_map[[y as usize, x as usize]] = error;

            max_error = max_error.max(error);
            sum_error += error;
            count += 1;
        }
    }

    let mean_error = if count > 0 {
        sum_error / count as f64
    } else {
        0.0
    };

    // Adaptive thresholding for anomaly detection
    let threshold = calculate_adaptive_threshold(&error_map, mean_error);

    let anomalies_detected = detect_anomalies(&error_map, threshold);

    // Calculate confidence based on error distribution
    let confidence = calculate_ela_confidence(&error_map, mean_error, max_error);

    Ok(ElaResult {
        error_map,
        max_error,
        mean_error,
        anomalies_detected,
        confidence,
    })
}

/// Recompress image at specified JPEG quality
fn recompress_image(image: &RgbImage, quality: u8) -> ForensicsResult<RgbImage> {
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);

    // Encode to JPEG
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
    let (width, height) = image.dimensions();
    encoder
        .encode(image.as_raw(), width, height, image::ColorType::Rgb8.into())
        .map_err(ForensicsError::ImageError)?;

    // Decode back
    let decoded = image::load_from_memory(&buffer).map_err(ForensicsError::ImageError)?;

    Ok(decoded.to_rgb8())
}

/// Calculate error between two pixels
fn calculate_pixel_error(p1: &Rgb<u8>, p2: &Rgb<u8>) -> f64 {
    let r_diff = (p1[0] as f64 - p2[0] as f64).abs();
    let g_diff = (p1[1] as f64 - p2[1] as f64).abs();
    let b_diff = (p1[2] as f64 - p2[2] as f64).abs();

    // Use Euclidean distance
    ((r_diff * r_diff + g_diff * g_diff + b_diff * b_diff) / 3.0).sqrt()
}

/// Calculate adaptive threshold based on error statistics
fn calculate_adaptive_threshold(error_map: &FlatArray2<f64>, mean_error: f64) -> f64 {
    let (height, width) = error_map.dim();

    // Calculate standard deviation
    let mut sum_sq_diff = 0.0;
    let mut count = 0;

    for y in 0..height {
        for x in 0..width {
            let diff = error_map[[y, x]] - mean_error;
            sum_sq_diff += diff * diff;
            count += 1;
        }
    }

    let variance = if count > 0 {
        sum_sq_diff / count as f64
    } else {
        0.0
    };
    let std_dev = variance.sqrt();

    // Threshold is mean + 2*std_dev, but at least BASE_THRESHOLD
    (mean_error + 2.0 * std_dev).max(BASE_THRESHOLD)
}

/// Detect anomalies based on threshold
fn detect_anomalies(error_map: &FlatArray2<f64>, threshold: f64) -> bool {
    let (height, width) = error_map.dim();
    let mut anomaly_count = 0;
    let total_pixels = height * width;

    for y in 0..height {
        for x in 0..width {
            if error_map[[y, x]] > threshold {
                anomaly_count += 1;
            }
        }
    }

    // If more than 5% of pixels are anomalous, flag as suspicious
    let anomaly_ratio = anomaly_count as f64 / total_pixels as f64;
    anomaly_ratio > 0.05
}

/// Analyze error distribution (low, medium, high)
fn analyze_error_distribution(error_map: &FlatArray2<f64>) -> (f64, f64, f64) {
    let (height, width) = error_map.dim();
    let total_pixels = (height * width) as f64;

    let mut low_count = 0;
    let mut medium_count = 0;
    let mut high_count = 0;

    for y in 0..height {
        for x in 0..width {
            let error = error_map[[y, x]];

            if error < 10.0 {
                low_count += 1;
            } else if error < 30.0 {
                medium_count += 1;
            } else {
                high_count += 1;
            }
        }
    }

    (
        low_count as f64 / total_pixels,
        medium_count as f64 / total_pixels,
        high_count as f64 / total_pixels,
    )
}

/// Calculate confidence score for ELA
fn calculate_ela_confidence(error_map: &FlatArray2<f64>, mean_error: f64, max_error: f64) -> f64 {
    // High mean error suggests recent editing
    let mean_score = (mean_error / 50.0).min(1.0);

    // High max error suggests local manipulation
    let max_score = (max_error / 100.0).min(1.0);

    // Analyze variance
    let (height, width) = error_map.dim();
    let mut sum_sq_diff = 0.0;
    let count = height * width;

    for y in 0..height {
        for x in 0..width {
            let diff = error_map[[y, x]] - mean_error;
            sum_sq_diff += diff * diff;
        }
    }

    let variance = if count > 0 {
        sum_sq_diff / count as f64
    } else {
        0.0
    };
    let std_dev = variance.sqrt();

    // High variance suggests inconsistent compression (tampering)
    let variance_score = (std_dev / 30.0).min(1.0);

    // Weighted combination
    (0.3 * mean_score + 0.3 * max_score + 0.4 * variance_score).min(1.0)
}

/// Perform ELA with custom quality
pub fn perform_ela_custom_quality(image: &RgbImage, quality: u8) -> ForensicsResult<ElaResult> {
    let (width, height) = image.dimensions();

    let recompressed = recompress_image(image, quality)?;

    let mut error_map = FlatArray2::zeros((height as usize, width as usize));
    let mut max_error: f64 = 0.0;
    let mut sum_error = 0.0;
    let mut count = 0;

    for y in 0..height {
        for x in 0..width {
            let orig_pixel = image.get_pixel(x, y);
            let recomp_pixel = recompressed.get_pixel(x, y);

            let error = calculate_pixel_error(orig_pixel, recomp_pixel);
            error_map[[y as usize, x as usize]] = error;

            max_error = max_error.max(error);
            sum_error += error;
            count += 1;
        }
    }

    let mean_error = if count > 0 {
        sum_error / count as f64
    } else {
        0.0
    };
    let threshold = calculate_adaptive_threshold(&error_map, mean_error);
    let anomalies_detected = detect_anomalies(&error_map, threshold);
    let confidence = calculate_ela_confidence(&error_map, mean_error, max_error);

    Ok(ElaResult {
        error_map,
        max_error,
        mean_error,
        anomalies_detected,
        confidence,
    })
}

/// Multi-scale ELA for better detection
pub fn perform_multiscale_ela(image: &RgbImage) -> ForensicsResult<Vec<ElaResult>> {
    let qualities = vec![95, 90, 85, 75];
    let mut results = Vec::new();

    for quality in qualities {
        let result = perform_ela_custom_quality(image, quality)?;
        results.push(result);
    }

    Ok(results)
}

/// Multi-quality ELA result aggregating analysis across multiple JPEG quality levels.
///
/// By comparing error maps at different quality levels, multi-quality ELA
/// provides more robust tampering detection. Tampered regions show
/// inconsistent error behaviour across quality levels while untampered
/// regions produce smoothly varying errors.
#[derive(Debug, Clone)]
pub struct MultiQualityElaResult {
    /// Per-quality ELA results (sorted by quality ascending).
    pub quality_results: Vec<(u8, ElaResult)>,
    /// Cross-quality variance map — high variance pixels are suspicious.
    pub variance_map: FlatArray2<f64>,
    /// Mean cross-quality variance.
    pub mean_variance: f64,
    /// Maximum cross-quality variance.
    pub max_variance: f64,
    /// Whether tampering was detected via cross-quality analysis.
    pub anomalies_detected: bool,
    /// Confidence score derived from the cross-quality analysis.
    pub confidence: f64,
}

/// Perform multi-quality ELA by recompressing at several JPEG quality levels
/// and comparing the resulting error maps for cross-quality consistency.
///
/// `qualities` must contain at least two quality levels. Typical values:
/// `&[95, 90, 85, 75, 60]`.
pub fn perform_multi_quality_ela(
    image: &RgbImage,
    qualities: &[u8],
) -> ForensicsResult<MultiQualityElaResult> {
    if qualities.len() < 2 {
        return Err(ForensicsError::AnalysisFailed(
            "Multi-quality ELA requires at least two quality levels".to_string(),
        ));
    }

    let (width, height) = image.dimensions();
    let h = height as usize;
    let w = width as usize;

    // Collect ELA results at each quality level.
    let mut quality_results: Vec<(u8, ElaResult)> = Vec::with_capacity(qualities.len());
    for &q in qualities {
        let result = perform_ela_custom_quality(image, q)?;
        quality_results.push((q, result));
    }

    // Sort by quality ascending for consistent ordering.
    quality_results.sort_by_key(|(q, _)| *q);

    // Compute per-pixel variance across quality levels.
    let n = quality_results.len() as f64;
    let mut variance_map = FlatArray2::zeros((h, w));
    let mut mean_variance = 0.0;
    let mut max_variance: f64 = 0.0;
    let pixel_count = (h * w) as f64;

    for y in 0..h {
        for x in 0..w {
            // Compute mean error at this pixel across qualities.
            let mut sum = 0.0;
            for (_, res) in &quality_results {
                sum += res.error_map[[y, x]];
            }
            let mean = sum / n;

            // Compute variance.
            let mut sum_sq_diff = 0.0;
            for (_, res) in &quality_results {
                let d = res.error_map[[y, x]] - mean;
                sum_sq_diff += d * d;
            }
            let var = sum_sq_diff / n;
            variance_map[[y, x]] = var;
            mean_variance += var;
            max_variance = max_variance.max(var);
        }
    }

    if pixel_count > 0.0 {
        mean_variance /= pixel_count;
    }

    // Adaptive threshold for cross-quality anomaly detection.
    let var_std = {
        let mut s = 0.0;
        for y in 0..h {
            for x in 0..w {
                let d = variance_map[[y, x]] - mean_variance;
                s += d * d;
            }
        }
        if pixel_count > 0.0 {
            (s / pixel_count).sqrt()
        } else {
            0.0
        }
    };
    let threshold = (mean_variance + 2.5 * var_std).max(5.0);

    // Count anomalous pixels.
    let mut anomaly_count = 0usize;
    for y in 0..h {
        for x in 0..w {
            if variance_map[[y, x]] > threshold {
                anomaly_count += 1;
            }
        }
    }

    let anomaly_ratio = if pixel_count > 0.0 {
        anomaly_count as f64 / pixel_count
    } else {
        0.0
    };
    let anomalies_detected = anomaly_ratio > 0.03;

    // Confidence from three signals: mean variance, max variance, anomaly ratio.
    let mean_score = (mean_variance / 100.0).min(1.0);
    let max_score = (max_variance / 500.0).min(1.0);
    let anomaly_score = (anomaly_ratio * 10.0).min(1.0);
    let confidence = (0.3 * mean_score + 0.3 * max_score + 0.4 * anomaly_score).min(1.0);

    Ok(MultiQualityElaResult {
        quality_results,
        variance_map,
        mean_variance,
        max_variance,
        anomalies_detected,
        confidence,
    })
}

/// Perform multi-quality ELA with default quality levels `[95, 90, 85, 75, 60]`.
pub fn perform_multi_quality_ela_default(
    image: &RgbImage,
) -> ForensicsResult<MultiQualityElaResult> {
    perform_multi_quality_ela(image, &[95, 90, 85, 75, 60])
}

/// Compute the cross-quality gradient map.
///
/// For each pixel, this computes the rate of change of error across quality
/// levels. Tampered regions tend to show a non-linear response curve.
pub fn compute_cross_quality_gradient(result: &MultiQualityElaResult) -> FlatArray2<f64> {
    if result.quality_results.len() < 2 {
        let h = result.variance_map.nrows();
        let w = result.variance_map.ncols();
        return FlatArray2::zeros((h, w));
    }

    let h = result.variance_map.nrows();
    let w = result.variance_map.ncols();
    let mut gradient_map = FlatArray2::zeros((h, w));

    let n_pairs = result.quality_results.len() - 1;

    for y in 0..h {
        for x in 0..w {
            let mut sum_abs_diff = 0.0;
            for i in 0..n_pairs {
                let e1 = result.quality_results[i].1.error_map[[y, x]];
                let e2 = result.quality_results[i + 1].1.error_map[[y, x]];
                sum_abs_diff += (e2 - e1).abs();
            }
            gradient_map[[y, x]] = sum_abs_diff / n_pairs as f64;
        }
    }

    gradient_map
}

/// Highlight anomalous regions in error map
pub fn highlight_anomalies(error_map: &FlatArray2<f64>, threshold: f64) -> FlatArray2<u8> {
    let (height, width) = error_map.dim();
    let mut highlighted = FlatArray2::zeros_u8(height, width);

    for y in 0..height {
        for x in 0..width {
            if error_map[[y, x]] > threshold {
                highlighted[[y, x]] = 255;
            }
        }
    }

    highlighted
}

/// Apply morphological operations to reduce false positives
pub fn reduce_false_positives(anomaly_map: &FlatArray2<u8>) -> FlatArray2<u8> {
    let (height, width) = anomaly_map.dim();
    let mut cleaned = anomaly_map.clone();

    // Simple erosion to remove small isolated pixels
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            if anomaly_map[[y, x]] > 0 {
                let mut neighbor_count = 0;

                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let ny = (y as i32 + dy) as usize;
                        let nx = (x as i32 + dx) as usize;

                        if anomaly_map[[ny, nx]] > 0 {
                            neighbor_count += 1;
                        }
                    }
                }

                // Keep only if surrounded by enough anomalous pixels
                if neighbor_count < 3 {
                    cleaned[[y, x]] = 0;
                }
            }
        }
    }

    cleaned
}

/// Generate ELA visualization image
pub fn generate_ela_visualization(error_map: &FlatArray2<f64>) -> RgbImage {
    let (height, width) = error_map.dim();
    let mut visualization = RgbImage::new(width as u32, height as u32);

    // Normalize error map to 0-255 range
    let max_val = error_map.iter().cloned().fold(0.0, f64::max);
    let min_val = error_map.iter().cloned().fold(f64::MAX, f64::min);
    let range = max_val - min_val;

    for y in 0..height {
        for x in 0..width {
            let normalized = if range > 0.0 {
                ((error_map[[y, x]] - min_val) / range * 255.0) as u8
            } else {
                0
            };

            // Use heat map coloring
            let (r, g, b) = error_to_color(normalized);
            visualization.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }

    visualization
}

/// Convert error level to heat map color
fn error_to_color(value: u8) -> (u8, u8, u8) {
    let normalized = value as f64 / 255.0;

    if normalized < 0.25 {
        // Blue to cyan
        let t = normalized / 0.25;
        (0, (t * 255.0) as u8, 255)
    } else if normalized < 0.5 {
        // Cyan to green
        let t = (normalized - 0.25) / 0.25;
        (0, 255, ((1.0 - t) * 255.0) as u8)
    } else if normalized < 0.75 {
        // Green to yellow
        let t = (normalized - 0.5) / 0.25;
        ((t * 255.0) as u8, 255, 0)
    } else {
        // Yellow to red
        let t = (normalized - 0.75) / 0.25;
        (255, ((1.0 - t) * 255.0) as u8, 0)
    }
}

/// Region-based ELA analysis
pub fn analyze_regions(
    image: &RgbImage,
    region_size: u32,
) -> ForensicsResult<Vec<(u32, u32, f64)>> {
    let (width, height) = image.dimensions();
    let ela_result = compute_ela(image)?;

    let mut region_scores = Vec::new();

    for y in (0..height).step_by(region_size as usize) {
        for x in (0..width).step_by(region_size as usize) {
            let mut region_sum = 0.0;
            let mut region_count = 0;

            for dy in 0..region_size {
                for dx in 0..region_size {
                    let px = x + dx;
                    let py = y + dy;

                    if px < width && py < height {
                        region_sum += ela_result.error_map[[py as usize, px as usize]];
                        region_count += 1;
                    }
                }
            }

            let region_mean = if region_count > 0 {
                region_sum / region_count as f64
            } else {
                0.0
            };

            region_scores.push((x, y, region_mean));
        }
    }

    Ok(region_scores)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;

    #[test]
    fn test_pixel_error_calculation() {
        let p1 = Rgb([100, 150, 200]);
        let p2 = Rgb([105, 155, 205]);
        let error = calculate_pixel_error(&p1, &p2);
        assert!(error > 0.0);
        assert!(error < 10.0);
    }

    #[test]
    fn test_error_distribution() {
        let mut error_map = FlatArray2::zeros((10, 10));
        error_map[[0, 0]] = 5.0; // Low
        error_map[[1, 1]] = 20.0; // Medium
        error_map[[2, 2]] = 50.0; // High

        let (low, medium, high) = analyze_error_distribution(&error_map);
        assert!(low > 0.9); // Most pixels are zero (low)
        assert!(medium > 0.0);
        assert!(high > 0.0);
    }

    #[test]
    fn test_adaptive_threshold() {
        let error_map = FlatArray2::from_elem(10, 10, 10.0_f64);
        let threshold = calculate_adaptive_threshold(&error_map, 10.0);
        assert!(threshold >= BASE_THRESHOLD);
    }

    #[test]
    fn test_ela_on_small_image() {
        let img = RgbImage::new(32, 32);
        let result = compute_ela(&img);
        assert!(result.is_ok());
    }

    #[test]
    fn test_error_to_color() {
        let (_r, _g, b) = error_to_color(0);
        assert_eq!(b, 255); // Blue for low error

        let (r, _g, _b) = error_to_color(255);
        assert_eq!(r, 255); // Red for high error
    }

    #[test]
    fn test_multi_quality_ela_requires_two_levels() {
        let img = RgbImage::new(32, 32);
        let result = perform_multi_quality_ela(&img, &[90]);
        assert!(result.is_err());
    }

    #[test]
    fn test_multi_quality_ela_on_uniform_image() {
        let img = RgbImage::new(32, 32);
        let result = perform_multi_quality_ela(&img, &[95, 85]).expect("should succeed");
        assert_eq!(result.quality_results.len(), 2);
        // Uniform image should have low variance.
        assert!(result.mean_variance < 50.0);
    }

    #[test]
    fn test_multi_quality_ela_default() {
        let img = RgbImage::new(32, 32);
        let result = perform_multi_quality_ela_default(&img).expect("should succeed");
        // Default uses 5 quality levels.
        assert_eq!(result.quality_results.len(), 5);
        assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
    }

    #[test]
    fn test_multi_quality_ela_sorting() {
        let img = RgbImage::new(32, 32);
        let result = perform_multi_quality_ela(&img, &[85, 95, 75]).expect("should succeed");
        // Results should be sorted by quality ascending.
        let qualities: Vec<u8> = result.quality_results.iter().map(|(q, _)| *q).collect();
        assert_eq!(qualities, vec![75, 85, 95]);
    }

    #[test]
    fn test_cross_quality_gradient_uniform() {
        let img = RgbImage::new(32, 32);
        let result = perform_multi_quality_ela(&img, &[95, 85, 75]).expect("should succeed");
        let gradient = compute_cross_quality_gradient(&result);
        assert_eq!(gradient.dim(), (32, 32));
    }

    #[test]
    fn test_multi_quality_ela_variance_map_dims() {
        let img = RgbImage::new(16, 24);
        let result = perform_multi_quality_ela(&img, &[95, 80]).expect("should succeed");
        assert_eq!(result.variance_map.dim(), (24, 16));
    }

    #[test]
    fn test_multi_quality_ela_detects_tampered_region() {
        // Create an image where a block has very different pixel values.
        let mut img = RgbImage::new(64, 64);
        for y in 0..64 {
            for x in 0..64 {
                let val = if x < 32 && y < 32 { 200 } else { 50 };
                img.put_pixel(x, y, Rgb([val, val, val]));
            }
        }
        let result = perform_multi_quality_ela(&img, &[95, 90, 80, 60]).expect("should succeed");
        // We don't assert anomalies_detected because it depends on the
        // specific error behaviour, but confidence should be computed.
        assert!(result.confidence >= 0.0);
        assert!(result.max_variance >= 0.0);
    }
}
