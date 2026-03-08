#![allow(dead_code)]
//! Exposure metering for video frames.
//!
//! This module provides exposure analysis tools similar to a camera's built-in
//! light meter. It supports spot, center-weighted, evaluative (matrix), and
//! average metering modes. The exposure value (EV) is computed from the
//! measured luminance, and over/under-exposure warnings are generated.

use std::f64;

/// Metering mode used to weight different regions of the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeteringMode {
    /// Average (whole frame equally weighted).
    Average,
    /// Center-weighted (center of frame weighted more heavily).
    CenterWeighted,
    /// Spot (small central region only).
    Spot,
    /// Evaluative / matrix (multi-zone intelligent metering).
    Evaluative,
}

/// Configuration for the exposure meter.
#[derive(Debug, Clone)]
pub struct ExposureMeterConfig {
    /// Metering mode.
    pub mode: MeteringMode,
    /// Spot meter radius as fraction of frame diagonal (0.0 - 0.5).
    pub spot_radius: f64,
    /// Over-exposure threshold (0 - 255 for 8-bit, luma).
    pub over_threshold: f64,
    /// Under-exposure threshold (0 - 255 for 8-bit, luma).
    pub under_threshold: f64,
    /// Number of evaluation zones per axis (for evaluative mode).
    pub eval_zones: u32,
}

impl Default for ExposureMeterConfig {
    fn default() -> Self {
        Self {
            mode: MeteringMode::CenterWeighted,
            spot_radius: 0.05,
            over_threshold: 235.0,
            under_threshold: 16.0,
            eval_zones: 4,
        }
    }
}

/// Exposure metering result.
#[derive(Debug, Clone)]
pub struct ExposureReading {
    /// Measured average luminance (0.0 - 255.0 scale).
    pub avg_luminance: f64,
    /// Estimated exposure value (EV) relative to 18% gray.
    pub ev_estimate: f64,
    /// Fraction of pixels that are over-exposed (0.0 - 1.0).
    pub over_exposed_ratio: f64,
    /// Fraction of pixels that are under-exposed (0.0 - 1.0).
    pub under_exposed_ratio: f64,
    /// Exposure recommendation: negative means overexposed, positive means underexposed.
    pub correction_stops: f64,
    /// Number of pixels metered.
    pub metered_pixels: u64,
    /// Per-zone luminance values (for evaluative mode).
    pub zone_luminances: Vec<f64>,
}

/// Compute Rec.709 luma from 8-bit RGB.
#[must_use]
fn rgb_to_luma(r: u8, g: u8, b: u8) -> f64 {
    0.2126 * f64::from(r) + 0.7152 * f64::from(g) + 0.0722 * f64::from(b)
}

/// Compute the weight for center-weighted metering based on distance from center.
#[must_use]
fn center_weight(x: u32, y: u32, width: u32, height: u32) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let cx = width as f64 / 2.0;
    #[allow(clippy::cast_precision_loss)]
    let cy = height as f64 / 2.0;
    #[allow(clippy::cast_precision_loss)]
    let dx = (x as f64 - cx) / cx;
    #[allow(clippy::cast_precision_loss)]
    let dy = (y as f64 - cy) / cy;
    let dist = (dx * dx + dy * dy).sqrt();
    // Gaussian-like falloff
    (-2.0 * dist * dist).exp()
}

/// Compute whether a pixel is inside the spot metering circle.
#[must_use]
#[allow(clippy::cast_precision_loss)]
fn in_spot(x: u32, y: u32, width: u32, height: u32, radius_frac: f64) -> bool {
    let cx = width as f64 / 2.0;
    let cy = height as f64 / 2.0;
    let diag = ((width as f64).powi(2) + (height as f64).powi(2)).sqrt();
    let radius = radius_frac * diag;
    let dx = x as f64 - cx;
    let dy = y as f64 - cy;
    (dx * dx + dy * dy).sqrt() <= radius
}

/// Estimate EV from average luminance (0-255 scale).
///
/// Maps 18% gray (~46 on 0-255 scale) to EV 0.
#[must_use]
fn luminance_to_ev(avg_luma: f64) -> f64 {
    if avg_luma <= 0.0 {
        return -10.0;
    }
    // 18% gray corresponds to ~46 on 0-255 scale
    let gray_18 = 255.0 * 0.18;
    (avg_luma / gray_18).log2()
}

/// Meter a frame using the given configuration.
///
/// * `frame` — RGB24 pixel data
/// * `width` — frame width
/// * `height` — frame height
/// * `config` — metering configuration
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn meter_exposure(
    frame: &[u8],
    width: u32,
    height: u32,
    config: &ExposureMeterConfig,
) -> ExposureReading {
    let num_pixels = (width as usize) * (height as usize);
    let expected = num_pixels * 3;

    if frame.len() < expected || num_pixels == 0 {
        return ExposureReading {
            avg_luminance: 0.0,
            ev_estimate: -10.0,
            over_exposed_ratio: 0.0,
            under_exposed_ratio: 0.0,
            correction_stops: 0.0,
            metered_pixels: 0,
            zone_luminances: Vec::new(),
        };
    }

    match config.mode {
        MeteringMode::Average => meter_average(frame, width, height, config),
        MeteringMode::CenterWeighted => meter_center_weighted(frame, width, height, config),
        MeteringMode::Spot => meter_spot(frame, width, height, config),
        MeteringMode::Evaluative => meter_evaluative(frame, width, height, config),
    }
}

/// Average metering: all pixels weighted equally.
#[allow(clippy::cast_precision_loss)]
fn meter_average(
    frame: &[u8],
    width: u32,
    height: u32,
    config: &ExposureMeterConfig,
) -> ExposureReading {
    let n = (width as usize) * (height as usize);
    let mut sum = 0.0_f64;
    let mut over = 0_u64;
    let mut under = 0_u64;

    for i in 0..n {
        let luma = rgb_to_luma(frame[i * 3], frame[i * 3 + 1], frame[i * 3 + 2]);
        sum += luma;
        if luma >= config.over_threshold {
            over += 1;
        }
        if luma <= config.under_threshold {
            under += 1;
        }
    }

    let avg = sum / n as f64;
    let ev = luminance_to_ev(avg);
    let metered = n as u64;

    ExposureReading {
        avg_luminance: avg,
        ev_estimate: ev,
        over_exposed_ratio: over as f64 / metered as f64,
        under_exposed_ratio: under as f64 / metered as f64,
        correction_stops: -ev,
        metered_pixels: metered,
        zone_luminances: Vec::new(),
    }
}

/// Center-weighted metering.
#[allow(clippy::cast_precision_loss)]
fn meter_center_weighted(
    frame: &[u8],
    width: u32,
    height: u32,
    config: &ExposureMeterConfig,
) -> ExposureReading {
    let n = (width as usize) * (height as usize);
    let mut weighted_sum = 0.0_f64;
    let mut weight_total = 0.0_f64;
    let mut over = 0_u64;
    let mut under = 0_u64;

    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) as usize;
            let luma = rgb_to_luma(frame[i * 3], frame[i * 3 + 1], frame[i * 3 + 2]);
            let w = center_weight(x, y, width, height);
            weighted_sum += luma * w;
            weight_total += w;
            if luma >= config.over_threshold {
                over += 1;
            }
            if luma <= config.under_threshold {
                under += 1;
            }
        }
    }

    let avg = if weight_total > 0.0 {
        weighted_sum / weight_total
    } else {
        0.0
    };
    let ev = luminance_to_ev(avg);
    let metered = n as u64;

    ExposureReading {
        avg_luminance: avg,
        ev_estimate: ev,
        over_exposed_ratio: over as f64 / metered as f64,
        under_exposed_ratio: under as f64 / metered as f64,
        correction_stops: -ev,
        metered_pixels: metered,
        zone_luminances: Vec::new(),
    }
}

/// Spot metering: only a small central circle.
#[allow(clippy::cast_precision_loss)]
fn meter_spot(
    frame: &[u8],
    width: u32,
    height: u32,
    config: &ExposureMeterConfig,
) -> ExposureReading {
    let mut sum = 0.0_f64;
    let mut count = 0_u64;
    let mut over = 0_u64;
    let mut under = 0_u64;

    for y in 0..height {
        for x in 0..width {
            if !in_spot(x, y, width, height, config.spot_radius) {
                continue;
            }
            let i = (y * width + x) as usize;
            let luma = rgb_to_luma(frame[i * 3], frame[i * 3 + 1], frame[i * 3 + 2]);
            sum += luma;
            count += 1;
            if luma >= config.over_threshold {
                over += 1;
            }
            if luma <= config.under_threshold {
                under += 1;
            }
        }
    }

    let avg = if count > 0 { sum / count as f64 } else { 0.0 };
    let ev = luminance_to_ev(avg);
    let denom = if count > 0 { count } else { 1 };

    ExposureReading {
        avg_luminance: avg,
        ev_estimate: ev,
        over_exposed_ratio: over as f64 / denom as f64,
        under_exposed_ratio: under as f64 / denom as f64,
        correction_stops: -ev,
        metered_pixels: count,
        zone_luminances: Vec::new(),
    }
}

/// Evaluative (matrix) metering: divides frame into zones.
#[allow(clippy::cast_precision_loss)]
fn meter_evaluative(
    frame: &[u8],
    width: u32,
    height: u32,
    config: &ExposureMeterConfig,
) -> ExposureReading {
    let total_px = (width as usize) * (height as usize);
    let zones = config.eval_zones.max(1);
    let zone_w = width / zones;
    let zone_h = height / zones;
    let zone_count = (zones * zones) as usize;

    let mut zone_sums = vec![0.0_f64; zone_count];
    let mut zone_counts = vec![0_u64; zone_count];
    let mut over = 0_u64;
    let mut under = 0_u64;
    let mut total_sum = 0.0_f64;

    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) as usize;
            let luma = rgb_to_luma(frame[i * 3], frame[i * 3 + 1], frame[i * 3 + 2]);
            total_sum += luma;

            let zx = (x / zone_w.max(1)).min(zones - 1) as usize;
            let zy = (y / zone_h.max(1)).min(zones - 1) as usize;
            let zi = zy * zones as usize + zx;
            zone_sums[zi] += luma;
            zone_counts[zi] += 1;

            if luma >= config.over_threshold {
                over += 1;
            }
            if luma <= config.under_threshold {
                under += 1;
            }
        }
    }

    let zone_luminances: Vec<f64> = zone_sums
        .iter()
        .zip(zone_counts.iter())
        .map(|(&s, &c)| if c > 0 { s / c as f64 } else { 0.0 })
        .collect();

    let avg = total_sum / total_px as f64;
    let ev = luminance_to_ev(avg);
    let metered = total_px as u64;

    ExposureReading {
        avg_luminance: avg,
        ev_estimate: ev,
        over_exposed_ratio: over as f64 / metered as f64,
        under_exposed_ratio: under as f64 / metered as f64,
        correction_stops: -ev,
        metered_pixels: metered,
        zone_luminances,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let n = (width as usize) * (height as usize);
        let mut data = Vec::with_capacity(n * 3);
        for _ in 0..n {
            data.push(r);
            data.push(g);
            data.push(b);
        }
        data
    }

    #[test]
    fn test_rgb_to_luma_black() {
        assert!((rgb_to_luma(0, 0, 0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rgb_to_luma_white() {
        let y = rgb_to_luma(255, 255, 255);
        assert!((y - 255.0).abs() < 0.01);
    }

    #[test]
    fn test_luminance_to_ev_18_gray() {
        let gray = 255.0 * 0.18;
        let ev = luminance_to_ev(gray);
        assert!(ev.abs() < 0.01);
    }

    #[test]
    fn test_luminance_to_ev_zero() {
        let ev = luminance_to_ev(0.0);
        assert!(ev < -5.0);
    }

    #[test]
    fn test_center_weight_at_center() {
        let w = center_weight(50, 50, 100, 100);
        assert!((w - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_center_weight_at_corner() {
        let w = center_weight(0, 0, 100, 100);
        assert!(w < 0.5);
    }

    #[test]
    fn test_in_spot_center() {
        assert!(in_spot(50, 50, 100, 100, 0.1));
    }

    #[test]
    fn test_in_spot_corner() {
        assert!(!in_spot(0, 0, 100, 100, 0.05));
    }

    #[test]
    fn test_meter_average_black() {
        let frame = make_frame(10, 10, 0, 0, 0);
        let config = ExposureMeterConfig {
            mode: MeteringMode::Average,
            ..Default::default()
        };
        let reading = meter_exposure(&frame, 10, 10, &config);
        assert!(reading.avg_luminance.abs() < f64::EPSILON);
        assert_eq!(reading.metered_pixels, 100);
    }

    #[test]
    fn test_meter_average_midgray() {
        let frame = make_frame(10, 10, 128, 128, 128);
        let config = ExposureMeterConfig {
            mode: MeteringMode::Average,
            ..Default::default()
        };
        let reading = meter_exposure(&frame, 10, 10, &config);
        assert!((reading.avg_luminance - 128.0).abs() < 1.0);
    }

    #[test]
    fn test_meter_center_weighted() {
        let frame = make_frame(20, 20, 100, 100, 100);
        let config = ExposureMeterConfig {
            mode: MeteringMode::CenterWeighted,
            ..Default::default()
        };
        let reading = meter_exposure(&frame, 20, 20, &config);
        assert!((reading.avg_luminance - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_meter_spot() {
        let frame = make_frame(100, 100, 200, 200, 200);
        let config = ExposureMeterConfig {
            mode: MeteringMode::Spot,
            spot_radius: 0.1,
            ..Default::default()
        };
        let reading = meter_exposure(&frame, 100, 100, &config);
        assert!(reading.metered_pixels < 10000);
        assert!((reading.avg_luminance - 200.0).abs() < 1.0);
    }

    #[test]
    fn test_meter_evaluative() {
        let frame = make_frame(16, 16, 128, 128, 128);
        let config = ExposureMeterConfig {
            mode: MeteringMode::Evaluative,
            eval_zones: 4,
            ..Default::default()
        };
        let reading = meter_exposure(&frame, 16, 16, &config);
        assert_eq!(reading.zone_luminances.len(), 16);
        for &z in &reading.zone_luminances {
            assert!((z - 128.0).abs() < 1.0);
        }
    }

    #[test]
    fn test_meter_empty_frame() {
        let config = ExposureMeterConfig::default();
        let reading = meter_exposure(&[], 10, 10, &config);
        assert_eq!(reading.metered_pixels, 0);
    }

    #[test]
    fn test_over_exposure_detection() {
        let frame = make_frame(4, 4, 250, 250, 250);
        let config = ExposureMeterConfig {
            mode: MeteringMode::Average,
            over_threshold: 235.0,
            ..Default::default()
        };
        let reading = meter_exposure(&frame, 4, 4, &config);
        assert!((reading.over_exposed_ratio - 1.0).abs() < f64::EPSILON);
    }
}
