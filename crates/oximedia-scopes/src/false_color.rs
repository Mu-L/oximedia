//! False color exposure visualization for video analysis.
//!
//! False color overlays map pixel luminance values to distinct colors, making it
//! easy to identify exposure levels and ensure proper exposure across the image.
//! This is particularly useful for:
//! - Identifying overexposed (clipped) highlights
//! - Finding underexposed (crushed) shadows
//! - Ensuring skin tones are properly exposed
//! - Checking overall exposure distribution
//!
//! The false color scale typically uses IRE units or f-stops relative to middle gray.

use crate::render::{rgb_to_ycbcr, Canvas};
use crate::{ScopeData, ScopeType};
use oximedia_core::OxiResult;

/// False color display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalseColorMode {
    /// IRE-based (0-100 IRE).
    Ire,

    /// Stop-based (relative to middle gray at 18%).
    Stops,

    /// Exposure zones (under, good, over).
    Zones,

    /// Zebra pattern for highlights only.
    Zebra,
}

/// False color scale configuration.
#[derive(Debug, Clone)]
pub struct FalseColorScale {
    /// Exposure zones with colors.
    pub zones: Vec<(f32, f32, [u8; 4])>,

    /// Zebra stripe threshold (IRE).
    pub zebra_threshold: f32,

    /// Zebra stripe width in pixels.
    pub zebra_width: u32,
}

impl Default for FalseColorScale {
    fn default() -> Self {
        Self {
            zones: default_ire_zones(),
            zebra_threshold: 95.0,
            zebra_width: 4,
        }
    }
}

/// Default IRE-based false color zones.
///
/// These zones represent common exposure levels in video production.
#[must_use]
pub fn default_ire_zones() -> Vec<(f32, f32, [u8; 4])> {
    vec![
        // (min_ire, max_ire, color_rgba)
        (0.0, 5.0, [0, 0, 128, 255]),  // Very dark blue (crushed blacks)
        (5.0, 10.0, [0, 0, 255, 255]), // Blue (very underexposed)
        (10.0, 20.0, [0, 128, 255, 255]), // Cyan-blue (underexposed)
        (20.0, 35.0, [0, 255, 255, 255]), // Cyan (shadows)
        (35.0, 45.0, [0, 255, 0, 255]), // Green (good shadow detail)
        (45.0, 55.0, [128, 255, 0, 255]), // Yellow-green (proper exposure)
        (55.0, 65.0, [255, 255, 0, 255]), // Yellow (skin tones, good midtones)
        (65.0, 75.0, [255, 200, 0, 255]), // Orange (bright midtones)
        (75.0, 85.0, [255, 128, 0, 255]), // Orange-red (highlights)
        (85.0, 95.0, [255, 0, 0, 255]), // Red (near clipping)
        (95.0, 100.0, [255, 0, 128, 255]), // Magenta (clipping)
        (100.0, 110.0, [255, 0, 255, 255]), // Bright magenta (clipped)
    ]
}

/// Stop-based false color zones (relative to middle gray).
///
/// Middle gray is typically at 18% reflectance (about 42 IRE).
#[must_use]
pub fn stop_based_zones() -> Vec<(f32, f32, [u8; 4])> {
    vec![
        // (min_stops, max_stops, color_rgba)
        // Stops are relative to middle gray (0.0 = 18% gray)
        (-6.0, -5.0, [0, 0, 64, 255]),    // Very dark (6 stops under)
        (-5.0, -4.0, [0, 0, 128, 255]),   // Dark blue (5 stops under)
        (-4.0, -3.0, [0, 0, 255, 255]),   // Blue (4 stops under)
        (-3.0, -2.0, [0, 128, 255, 255]), // Cyan (3 stops under)
        (-2.0, -1.0, [0, 255, 255, 255]), // Bright cyan (2 stops under)
        (-1.0, 0.0, [0, 255, 0, 255]),    // Green (1 stop under)
        (0.0, 1.0, [255, 255, 0, 255]),   // Yellow (middle gray to 1 stop over)
        (1.0, 2.0, [255, 200, 0, 255]),   // Orange (1-2 stops over)
        (2.0, 3.0, [255, 128, 0, 255]),   // Orange-red (2-3 stops over)
        (3.0, 4.0, [255, 0, 0, 255]),     // Red (3-4 stops over)
        (4.0, 5.0, [255, 0, 128, 255]),   // Magenta (4-5 stops over)
        (5.0, 10.0, [255, 0, 255, 255]),  // Bright magenta (clipped)
    ]
}

/// Generates a false color overlay from RGB frame data.
///
/// The false color overlay maps luminance values to distinct colors
/// based on the selected mode and scale.
///
/// # Arguments
///
/// * `frame` - RGB24 frame data (width * height * 3 bytes)
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
/// * `mode` - False color display mode
/// * `scale` - False color scale configuration
///
/// # Errors
///
/// Returns an error if frame data is invalid or insufficient.
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_precision_loss)]
pub fn generate_false_color(
    frame: &[u8],
    width: u32,
    height: u32,
    mode: FalseColorMode,
    scale: &FalseColorScale,
) -> OxiResult<ScopeData> {
    let expected_size = (width * height * 3) as usize;
    if frame.len() < expected_size {
        return Err(oximedia_core::OxiError::InvalidData(format!(
            "Frame data too small: expected {expected_size}, got {}",
            frame.len()
        )));
    }

    let mut canvas = Canvas::new(width, height);

    match mode {
        FalseColorMode::Ire => {
            generate_ire_false_color(frame, width, height, &mut canvas, scale);
        }
        FalseColorMode::Stops => {
            generate_stop_false_color(frame, width, height, &mut canvas, scale);
        }
        FalseColorMode::Zones => {
            generate_zone_false_color(frame, width, height, &mut canvas);
        }
        FalseColorMode::Zebra => {
            generate_zebra_pattern(frame, width, height, &mut canvas, scale);
        }
    }

    Ok(ScopeData {
        width,
        height,
        data: canvas.data,
        scope_type: ScopeType::FalseColor,
    })
}

/// Generates IRE-based false color overlay.
fn generate_ire_false_color(
    frame: &[u8],
    width: u32,
    height: u32,
    canvas: &mut Canvas,
    scale: &FalseColorScale,
) {
    for y in 0..height {
        for x in 0..width {
            let pixel_idx = ((y * width + x) * 3) as usize;
            let r = frame[pixel_idx];
            let g = frame[pixel_idx + 1];
            let b = frame[pixel_idx + 2];

            let (luma, _, _) = rgb_to_ycbcr(r, g, b);

            // Convert luma (0-255) to IRE (0-100)
            let ire = (f32::from(luma) / 255.0) * 100.0;

            // Find the appropriate zone color
            let color = find_zone_color(ire, &scale.zones);
            canvas.set_pixel(x, y, color);
        }
    }
}

/// Generates stop-based false color overlay.
fn generate_stop_false_color(
    frame: &[u8],
    width: u32,
    height: u32,
    canvas: &mut Canvas,
    scale: &FalseColorScale,
) {
    // Middle gray reference: 18% reflectance ≈ 42 IRE ≈ 108/255 in linear space
    const MIDDLE_GRAY: f32 = 0.18;

    for y in 0..height {
        for x in 0..width {
            let pixel_idx = ((y * width + x) * 3) as usize;
            let r = frame[pixel_idx];
            let g = frame[pixel_idx + 1];
            let b = frame[pixel_idx + 2];

            let (luma, _, _) = rgb_to_ycbcr(r, g, b);

            // Convert to linear space (assuming sRGB)
            let linear_luma = srgb_to_linear(f32::from(luma) / 255.0);

            // Calculate stops relative to middle gray
            let stops = if linear_luma > 0.0 {
                (linear_luma / MIDDLE_GRAY).log2()
            } else {
                -10.0 // Very dark
            };

            // Find the appropriate zone color
            let color = find_zone_color(stops, &scale.zones);
            canvas.set_pixel(x, y, color);
        }
    }
}

/// Generates simple 3-zone false color (under/good/over).
fn generate_zone_false_color(frame: &[u8], width: u32, height: u32, canvas: &mut Canvas) {
    let zones = vec![
        (0.0, 20.0, [0, 0, 255, 255]),   // Underexposed (blue)
        (20.0, 80.0, [0, 255, 0, 255]),  // Good exposure (green)
        (80.0, 110.0, [255, 0, 0, 255]), // Overexposed (red)
    ];

    for y in 0..height {
        for x in 0..width {
            let pixel_idx = ((y * width + x) * 3) as usize;
            let r = frame[pixel_idx];
            let g = frame[pixel_idx + 1];
            let b = frame[pixel_idx + 2];

            let (luma, _, _) = rgb_to_ycbcr(r, g, b);
            let ire = (f32::from(luma) / 255.0) * 100.0;

            let color = find_zone_color(ire, &zones);
            canvas.set_pixel(x, y, color);
        }
    }
}

/// Generates zebra stripe pattern for highlight detection.
///
/// Zebra stripes are diagonal lines overlaid on areas that exceed the threshold.
fn generate_zebra_pattern(
    frame: &[u8],
    width: u32,
    height: u32,
    canvas: &mut Canvas,
    scale: &FalseColorScale,
) {
    // Copy original frame to canvas first
    for y in 0..height {
        for x in 0..width {
            let pixel_idx = ((y * width + x) * 3) as usize;
            let r = frame[pixel_idx];
            let g = frame[pixel_idx + 1];
            let b = frame[pixel_idx + 2];

            canvas.set_pixel(x, y, [r, g, b, 255]);
        }
    }

    // Overlay zebra stripes on highlights
    let zebra_width = scale.zebra_width;
    for y in 0..height {
        for x in 0..width {
            let pixel_idx = ((y * width + x) * 3) as usize;
            let r = frame[pixel_idx];
            let g = frame[pixel_idx + 1];
            let b = frame[pixel_idx + 2];

            let (luma, _, _) = rgb_to_ycbcr(r, g, b);
            let ire = (f32::from(luma) / 255.0) * 100.0;

            // If above threshold, draw zebra stripe
            if ire >= scale.zebra_threshold {
                // Diagonal stripe pattern
                #[allow(clippy::cast_possible_truncation)]
                let stripe_pos = ((x + y) / zebra_width) % 2;
                if stripe_pos == 0 {
                    // Draw red stripe
                    canvas.set_pixel(x, y, [255, 0, 0, 255]);
                }
            }
        }
    }
}

/// Finds the zone color for a given value.
fn find_zone_color(value: f32, zones: &[(f32, f32, [u8; 4])]) -> [u8; 4] {
    for (min, max, color) in zones {
        if value >= *min && value < *max {
            return *color;
        }
    }

    // Default to black if not in any zone
    [0, 0, 0, 255]
}

/// Converts sRGB to linear space.
#[must_use]
fn srgb_to_linear(srgb: f32) -> f32 {
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts linear to sRGB space.
#[must_use]
#[allow(dead_code)]
fn linear_to_srgb(linear: f32) -> f32 {
    if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

/// False color statistics.
#[derive(Debug, Clone)]
pub struct FalseColorStats {
    /// Percentage of pixels in each zone.
    pub zone_distribution: Vec<(String, f32)>,

    /// Percentage of clipped highlights (> 95 IRE).
    pub highlight_clip_percent: f32,

    /// Percentage of crushed shadows (< 5 IRE).
    pub shadow_clip_percent: f32,

    /// Percentage of properly exposed pixels (20-80 IRE).
    pub good_exposure_percent: f32,
}

/// Computes false color statistics from frame data.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn compute_false_color_stats(frame: &[u8], width: u32, height: u32) -> FalseColorStats {
    let pixel_count = width * height;
    let mut highlight_clip_count = 0u32;
    let mut shadow_clip_count = 0u32;
    let mut good_exposure_count = 0u32;

    // Zone counters
    let zone_names = [
        "Crushed Blacks (<5 IRE)",
        "Underexposed (5-20 IRE)",
        "Shadows (20-35 IRE)",
        "Good (35-65 IRE)",
        "Highlights (65-85 IRE)",
        "Near Clipping (85-95 IRE)",
        "Clipped (>95 IRE)",
    ];
    let mut zone_counts = vec![0u32; zone_names.len()];

    for y in 0..height {
        for x in 0..width {
            let pixel_idx = ((y * width + x) * 3) as usize;
            if pixel_idx + 2 >= frame.len() {
                break;
            }

            let r = frame[pixel_idx];
            let g = frame[pixel_idx + 1];
            let b = frame[pixel_idx + 2];

            let (luma, _, _) = rgb_to_ycbcr(r, g, b);
            let ire = (f32::from(luma) / 255.0) * 100.0;

            // Count clipping
            if ire < 5.0 {
                shadow_clip_count += 1;
            }
            if ire > 95.0 {
                highlight_clip_count += 1;
            }

            // Count good exposure
            if (20.0..=80.0).contains(&ire) {
                good_exposure_count += 1;
            }

            // Count zones
            match ire {
                x if x < 5.0 => zone_counts[0] += 1,
                x if x < 20.0 => zone_counts[1] += 1,
                x if x < 35.0 => zone_counts[2] += 1,
                x if x < 65.0 => zone_counts[3] += 1,
                x if x < 85.0 => zone_counts[4] += 1,
                x if x < 95.0 => zone_counts[5] += 1,
                _ => zone_counts[6] += 1,
            }
        }
    }

    let zone_distribution: Vec<(String, f32)> = zone_names
        .iter()
        .zip(zone_counts.iter())
        .map(|(name, &count)| {
            (
                (*name).to_string(),
                (count as f32 / pixel_count as f32) * 100.0,
            )
        })
        .collect();

    FalseColorStats {
        zone_distribution,
        highlight_clip_percent: (highlight_clip_count as f32 / pixel_count as f32) * 100.0,
        shadow_clip_percent: (shadow_clip_count as f32 / pixel_count as f32) * 100.0,
        good_exposure_percent: (good_exposure_count as f32 / pixel_count as f32) * 100.0,
    }
}

/// Generates a false color legend image.
///
/// The legend shows the color scale used for false color mapping.
#[must_use]
pub fn generate_false_color_legend(width: u32, height: u32, scale: &FalseColorScale) -> Vec<u8> {
    let mut canvas = Canvas::new(width, height);

    // Draw gradient from left (min) to right (max)
    for x in 0..width {
        let value = (x as f32 / width as f32) * 100.0; // 0-100 IRE

        let color = find_zone_color(value, &scale.zones);

        // Fill vertical stripe
        for y in 0..height {
            canvas.set_pixel(x, y, color);
        }
    }

    canvas.data
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_frame(width: u32, height: u32) -> Vec<u8> {
        let mut frame = vec![0u8; (width * height * 3) as usize];

        // Create gradient from black to white
        for y in 0..height {
            let value = ((y * 255) / height) as u8;
            for x in 0..width {
                let idx = ((y * width + x) * 3) as usize;
                frame[idx] = value;
                frame[idx + 1] = value;
                frame[idx + 2] = value;
            }
        }

        frame
    }

    #[test]
    fn test_generate_false_color_ire() {
        let frame = create_test_frame(100, 100);
        let scale = FalseColorScale::default();

        let result = generate_false_color(&frame, 100, 100, FalseColorMode::Ire, &scale);
        assert!(result.is_ok());

        let scope = result.expect("should succeed in test");
        assert_eq!(scope.width, 100);
        assert_eq!(scope.height, 100);
    }

    #[test]
    fn test_generate_false_color_stops() {
        let frame = create_test_frame(100, 100);
        let scale = FalseColorScale {
            zones: stop_based_zones(),
            ..Default::default()
        };

        let result = generate_false_color(&frame, 100, 100, FalseColorMode::Stops, &scale);
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_false_color_zones() {
        let frame = create_test_frame(100, 100);
        let scale = FalseColorScale::default();

        let result = generate_false_color(&frame, 100, 100, FalseColorMode::Zones, &scale);
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_zebra_pattern() {
        let mut frame = vec![0u8; 100 * 100 * 3];

        // Create overexposed area
        for i in (0..frame.len()).step_by(3) {
            frame[i] = 250;
            frame[i + 1] = 250;
            frame[i + 2] = 250;
        }

        let scale = FalseColorScale::default();
        let result = generate_false_color(&frame, 100, 100, FalseColorMode::Zebra, &scale);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compute_false_color_stats() {
        let frame = create_test_frame(100, 100);
        let stats = compute_false_color_stats(&frame, 100, 100);

        assert_eq!(stats.zone_distribution.len(), 7);
        assert!(stats.good_exposure_percent > 0.0);
        assert!(stats.good_exposure_percent <= 100.0);
    }

    #[test]
    fn test_srgb_linear_conversion() {
        let srgb = 0.5;
        let linear = srgb_to_linear(srgb);
        let srgb_back = linear_to_srgb(linear);

        assert!((srgb - srgb_back).abs() < 0.001);
    }

    #[test]
    fn test_find_zone_color() {
        let zones = default_ire_zones();

        // Test black (0-5 IRE)
        let color = find_zone_color(2.0, &zones);
        assert_eq!(color[2], 128); // Dark blue

        // Test white (95-100 IRE)
        let color = find_zone_color(97.0, &zones);
        assert_eq!(color[0], 255); // Magenta
    }

    #[test]
    fn test_generate_false_color_legend() {
        let scale = FalseColorScale::default();
        let legend = generate_false_color_legend(256, 20, &scale);

        assert_eq!(legend.len(), (256 * 20 * 4) as usize);
    }

    #[test]
    fn test_invalid_frame_size() {
        let frame = vec![0u8; 100]; // Too small
        let scale = FalseColorScale::default();

        let result = generate_false_color(&frame, 100, 100, FalseColorMode::Ire, &scale);
        assert!(result.is_err());
    }

    #[test]
    fn test_clipping_detection() {
        let mut frame = vec![0u8; 100 * 100 * 3];

        // Half black, half white
        for i in 0..(50 * 100 * 3) {
            frame[i] = 0; // Black
        }
        for i in (50 * 100 * 3)..frame.len() {
            frame[i] = 255; // White
        }

        let stats = compute_false_color_stats(&frame, 100, 100);

        assert!(stats.shadow_clip_percent > 40.0);
        assert!(stats.highlight_clip_percent > 40.0);
    }
}
