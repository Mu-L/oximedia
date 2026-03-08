//! Illumination and Lighting Inconsistency Analysis
//!
//! This module detects tampering by analyzing lighting, shadows, reflections,
//! and illumination consistency across an image.

use crate::{ForensicTest, ForensicsResult};
use image::RgbImage;
use ndarray::Array2;
use std::f64::consts::PI;

/// Illumination analysis result
#[derive(Debug, Clone)]
pub struct IlluminationResult {
    /// Detected light sources
    pub light_sources: Vec<LightSource>,
    /// Inconsistent regions
    pub inconsistent_regions: Vec<(usize, usize, usize, usize)>,
    /// Confidence score
    pub confidence: f64,
}

/// Light source estimation
#[derive(Debug, Clone)]
pub struct LightSource {
    /// Direction (azimuth angle)
    pub azimuth: f64,
    /// Direction (elevation angle)
    pub elevation: f64,
    /// Intensity
    pub intensity: f64,
}

/// Shadow analysis result
#[derive(Debug, Clone)]
pub struct ShadowAnalysis {
    /// Detected shadows
    pub shadows: Vec<ShadowRegion>,
    /// Inconsistency score
    pub inconsistency_score: f64,
}

/// Shadow region
#[derive(Debug, Clone)]
pub struct ShadowRegion {
    /// X coordinate
    pub x: usize,
    /// Y coordinate
    pub y: usize,
    /// Width
    pub width: usize,
    /// Height
    pub height: usize,
    /// Shadow direction
    pub direction: f64,
    /// Shadow intensity
    pub intensity: f64,
}

/// Analyze lighting for tampering detection
#[allow(unused_variables)]
pub fn analyze_lighting(image: &RgbImage) -> ForensicsResult<ForensicTest> {
    let mut test = ForensicTest::new("Lighting Analysis");

    let (width, height) = image.dimensions();

    // Convert to grayscale for intensity analysis
    let gray = rgb_to_grayscale(image);

    // Analyze illumination consistency
    let illum_result = analyze_illumination_consistency(&gray)?;

    if !illum_result.inconsistent_regions.is_empty() {
        test.tampering_detected = true;
        test.add_finding(format!(
            "Detected {} regions with inconsistent illumination",
            illum_result.inconsistent_regions.len()
        ));
    }

    test.add_finding(format!(
        "Estimated {} light sources",
        illum_result.light_sources.len()
    ));

    // Analyze shadows
    let shadow_analysis = analyze_shadows(&gray)?;

    if shadow_analysis.inconsistency_score > 0.3 {
        test.tampering_detected = true;
        test.add_finding(format!(
            "Shadow inconsistency detected (score: {:.3})",
            shadow_analysis.inconsistency_score
        ));
    }

    test.add_finding(format!(
        "Found {} shadow regions",
        shadow_analysis.shadows.len()
    ));

    // Detect physically impossible lighting
    let impossible_lighting = detect_impossible_lighting(&gray)?;

    if impossible_lighting {
        test.tampering_detected = true;
        test.add_finding("Physically impossible lighting detected".to_string());
    }

    // Calculate confidence
    let mut confidence = illum_result.confidence;
    confidence = (confidence + shadow_analysis.inconsistency_score) / 2.0;

    if impossible_lighting {
        confidence = (confidence + 0.5).min(1.0);
    }

    test.set_confidence(confidence);

    // Create anomaly map
    let anomaly_map = create_lighting_anomaly_map(image, &illum_result, &shadow_analysis)?;
    test.anomaly_map = Some(anomaly_map);

    Ok(test)
}

/// Convert RGB to grayscale
fn rgb_to_grayscale(image: &RgbImage) -> Array2<f64> {
    let (width, height) = image.dimensions();
    let mut gray = Array2::zeros((height as usize, width as usize));

    for (x, y, pixel) in image.enumerate_pixels() {
        let r = pixel[0] as f64;
        let g = pixel[1] as f64;
        let b = pixel[2] as f64;

        gray[[y as usize, x as usize]] = 0.299 * r + 0.587 * g + 0.114 * b;
    }

    gray
}

/// Analyze illumination consistency
fn analyze_illumination_consistency(gray: &Array2<f64>) -> ForensicsResult<IlluminationResult> {
    let (height, width) = gray.dim();

    // Estimate light sources from gradients
    let light_sources = estimate_light_sources(gray);

    // Divide image into regions and check consistency
    let region_size = 64;
    let mut regional_directions = Vec::new();

    for y in (0..height - region_size).step_by(region_size / 2) {
        for x in (0..width - region_size).step_by(region_size / 2) {
            let direction = estimate_local_light_direction(gray, x, y, region_size);
            regional_directions.push((x, y, direction));
        }
    }

    // Find regions with inconsistent lighting
    let mut inconsistent_regions = Vec::new();

    if !light_sources.is_empty() {
        let primary_direction = light_sources[0].azimuth;

        for (x, y, direction) in &regional_directions {
            let angle_diff = (direction - primary_direction).abs();

            // Normalize to [0, PI]
            let angle_diff = if angle_diff > PI {
                2.0 * PI - angle_diff
            } else {
                angle_diff
            };

            if angle_diff > PI / 3.0 {
                inconsistent_regions.push((*x, *y, region_size, region_size));
            }
        }
    }

    let confidence = if !regional_directions.is_empty() {
        (inconsistent_regions.len() as f64 / regional_directions.len() as f64).min(1.0)
    } else {
        0.0
    };

    Ok(IlluminationResult {
        light_sources,
        inconsistent_regions,
        confidence,
    })
}

/// Estimate light sources from image
fn estimate_light_sources(gray: &Array2<f64>) -> Vec<LightSource> {
    let (height, width) = gray.dim();

    // Simple approach: analyze gradient directions
    let mut gradient_directions = Vec::new();

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let gx = gray[[y, x + 1]] - gray[[y, x - 1]];
            let gy = gray[[y + 1, x]] - gray[[y - 1, x]];

            let magnitude = (gx * gx + gy * gy).sqrt();

            if magnitude > 10.0 {
                let direction = gy.atan2(gx);
                gradient_directions.push(direction);
            }
        }
    }

    if gradient_directions.is_empty() {
        return Vec::new();
    }

    // Find dominant direction (simplified - would use clustering in production)
    let mean_direction = gradient_directions.iter().sum::<f64>() / gradient_directions.len() as f64;

    // Estimate intensity from mean brightness
    let mean_intensity = gray.iter().sum::<f64>() / (height * width) as f64;

    vec![LightSource {
        azimuth: mean_direction,
        elevation: PI / 4.0, // Assume 45 degrees
        intensity: mean_intensity,
    }]
}

/// Estimate local light direction in a region
fn estimate_local_light_direction(gray: &Array2<f64>, x: usize, y: usize, size: usize) -> f64 {
    let (height, width) = gray.dim();
    let mut gx_sum = 0.0;
    let mut gy_sum = 0.0;
    let mut count = 0;

    for dy in 1..size - 1 {
        for dx in 1..size - 1 {
            let px = x + dx;
            let py = y + dy;

            if px > 0 && px < width - 1 && py > 0 && py < height - 1 {
                let gx = gray[[py, px + 1]] - gray[[py, px - 1]];
                let gy = gray[[py + 1, px]] - gray[[py - 1, px]];

                gx_sum += gx;
                gy_sum += gy;
                count += 1;
            }
        }
    }

    if count > 0 {
        let avg_gx = gx_sum / count as f64;
        let avg_gy = gy_sum / count as f64;
        avg_gy.atan2(avg_gx)
    } else {
        0.0
    }
}

/// Analyze shadows in the image
fn analyze_shadows(gray: &Array2<f64>) -> ForensicsResult<ShadowAnalysis> {
    let (height, width) = gray.dim();

    // Detect dark regions (potential shadows)
    let threshold = 80.0;
    let mut shadow_regions = Vec::new();

    let region_size = 32;

    for y in (0..height - region_size).step_by(region_size / 2) {
        for x in (0..width - region_size).step_by(region_size / 2) {
            let mut sum = 0.0;
            let mut count = 0;

            for dy in 0..region_size {
                for dx in 0..region_size {
                    if y + dy < height && x + dx < width {
                        sum += gray[[y + dy, x + dx]];
                        count += 1;
                    }
                }
            }

            let mean = if count > 0 { sum / count as f64 } else { 0.0 };

            if mean < threshold {
                // Estimate shadow direction
                let direction = estimate_shadow_direction(gray, x, y, region_size);

                shadow_regions.push(ShadowRegion {
                    x,
                    y,
                    width: region_size,
                    height: region_size,
                    direction,
                    intensity: threshold - mean,
                });
            }
        }
    }

    // Check consistency of shadow directions
    let inconsistency_score = if shadow_regions.len() > 1 {
        compute_shadow_inconsistency(&shadow_regions)
    } else {
        0.0
    };

    Ok(ShadowAnalysis {
        shadows: shadow_regions,
        inconsistency_score,
    })
}

/// Estimate shadow direction
fn estimate_shadow_direction(gray: &Array2<f64>, x: usize, y: usize, size: usize) -> f64 {
    // Use gradient at shadow boundary
    estimate_local_light_direction(gray, x, y, size)
}

/// Compute shadow direction inconsistency
fn compute_shadow_inconsistency(shadows: &[ShadowRegion]) -> f64 {
    if shadows.len() < 2 {
        return 0.0;
    }

    let mean_direction = shadows.iter().map(|s| s.direction).sum::<f64>() / shadows.len() as f64;

    let mut variance = 0.0;
    for shadow in shadows {
        let mut diff = (shadow.direction - mean_direction).abs();

        // Normalize to [0, PI]
        if diff > PI {
            diff = 2.0 * PI - diff;
        }

        variance += diff * diff;
    }

    variance /= shadows.len() as f64;

    let std_dev = variance.sqrt();

    // Normalize to [0, 1]
    (std_dev / PI).min(1.0)
}

/// Detect physically impossible lighting
fn detect_impossible_lighting(gray: &Array2<f64>) -> ForensicsResult<bool> {
    let (height, width) = gray.dim();

    // Check for contradictory highlights and shadows
    let mean_brightness = gray.iter().sum::<f64>() / (height * width) as f64;

    let mut bright_regions = 0;
    let mut dark_regions = 0;

    let region_size = 64;

    for y in (0..height - region_size).step_by(region_size) {
        for x in (0..width - region_size).step_by(region_size) {
            let mut sum = 0.0;
            let mut count = 0;

            for dy in 0..region_size {
                for dx in 0..region_size {
                    if y + dy < height && x + dx < width {
                        sum += gray[[y + dy, x + dx]];
                        count += 1;
                    }
                }
            }

            let mean = if count > 0 { sum / count as f64 } else { 0.0 };

            if mean > mean_brightness + 50.0 {
                bright_regions += 1;
            } else if mean < mean_brightness - 50.0 {
                dark_regions += 1;
            }
        }
    }

    // If we have many both very bright and very dark regions, might be suspicious
    let total_regions = ((height / region_size) * (width / region_size)) as f64;
    let bright_ratio = bright_regions as f64 / total_regions;
    let dark_ratio = dark_regions as f64 / total_regions;

    // Both high ratios might indicate compositing
    Ok(bright_ratio > 0.3 && dark_ratio > 0.3)
}

/// Create anomaly map from lighting analysis
fn create_lighting_anomaly_map(
    image: &RgbImage,
    illum_result: &IlluminationResult,
    shadow_analysis: &ShadowAnalysis,
) -> ForensicsResult<Array2<f64>> {
    let (width, height) = image.dimensions();
    let mut anomaly_map: Array2<f64> = Array2::zeros((height as usize, width as usize));

    // Mark inconsistent illumination regions
    for (x, y, w, h) in &illum_result.inconsistent_regions {
        for dy in 0..*h {
            for dx in 0..*w {
                let px = x + dx;
                let py = y + dy;

                if px < width as usize && py < height as usize {
                    anomaly_map[[py, px]] = 0.8;
                }
            }
        }
    }

    // Mark shadow regions with high inconsistency
    if shadow_analysis.inconsistency_score > 0.3 {
        for shadow in &shadow_analysis.shadows {
            for dy in 0..shadow.height {
                for dx in 0..shadow.width {
                    let px = shadow.x + dx;
                    let py = shadow.y + dy;

                    if px < width as usize && py < height as usize {
                        anomaly_map[[py, px]] = anomaly_map[[py, px]].max(0.5_f64);
                    }
                }
            }
        }
    }

    Ok(anomaly_map)
}

/// Analyze reflections for consistency
pub fn analyze_reflections(image: &RgbImage) -> ForensicsResult<f64> {
    let gray = rgb_to_grayscale(image);
    let (height, width) = gray.dim();

    // Look for bright spots (potential reflections/specularities)
    let mut reflection_points = Vec::new();

    for y in 0..height {
        for x in 0..width {
            if gray[[y, x]] > 200.0 {
                reflection_points.push((x, y, gray[[y, x]]));
            }
        }
    }

    // Analyze distribution of reflections
    if reflection_points.is_empty() {
        return Ok(0.0);
    }

    // Check if reflections are consistent with a single light source
    // (simplified - would analyze geometric consistency in production)
    let inconsistency_score = if reflection_points.len() > 10 {
        0.3
    } else {
        0.0
    };

    Ok(inconsistency_score)
}

/// Estimate ambient occlusion map
pub fn estimate_ambient_occlusion(gray: &Array2<f64>) -> Array2<f64> {
    let (height, width) = gray.dim();
    let mut ao_map = Array2::zeros((height, width));

    let kernel_size = 5;
    let half_kernel = kernel_size / 2;

    for y in half_kernel..height - half_kernel {
        for x in half_kernel..width - half_kernel {
            let mut darker_count = 0;
            let center = gray[[y, x]];

            for dy in -(half_kernel as i32)..=half_kernel as i32 {
                for dx in -(half_kernel as i32)..=half_kernel as i32 {
                    let ny = (y as i32 + dy) as usize;
                    let nx = (x as i32 + dx) as usize;

                    if gray[[ny, nx]] < center {
                        darker_count += 1;
                    }
                }
            }

            let total = kernel_size * kernel_size;
            ao_map[[y, x]] = darker_count as f64 / total as f64;
        }
    }

    ao_map
}

/// Detect light source direction from specular highlights
pub fn detect_light_from_specular(gray: &Array2<f64>) -> Option<(f64, f64)> {
    let (height, width) = gray.dim();

    // Find bright spots
    let mut highlights = Vec::new();

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            if gray[[y, x]] > 220.0 {
                // Check if it's a local maximum
                let mut is_max = true;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let ny = (y as i32 + dy) as usize;
                        let nx = (x as i32 + dx) as usize;
                        if gray[[ny, nx]] > gray[[y, x]] {
                            is_max = false;
                            break;
                        }
                    }
                    if !is_max {
                        break;
                    }
                }

                if is_max {
                    highlights.push((x, y));
                }
            }
        }
    }

    if highlights.is_empty() {
        return None;
    }

    // Estimate direction from centroid
    let cx = highlights.iter().map(|(x, _)| *x).sum::<usize>() as f64 / highlights.len() as f64;
    let cy = highlights.iter().map(|(_, y)| *y).sum::<usize>() as f64 / highlights.len() as f64;

    Some((cx, cy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;

    #[test]
    fn test_rgb_to_grayscale() {
        let img = RgbImage::new(10, 10);
        let gray = rgb_to_grayscale(&img);
        assert_eq!(gray.dim(), (10, 10));
    }

    #[test]
    fn test_light_source_estimation() {
        // Create an image with strong gradients
        let mut gray = Array2::zeros((64, 64));
        for y in 0..64 {
            for x in 0..64 {
                gray[[y, x]] = (x * 4 + y * 4) as f64; // Stronger gradient
            }
        }
        let sources = estimate_light_sources(&gray);
        // With strong gradients, should find at least one source
        assert!(sources.len() >= 1);
    }

    #[test]
    fn test_local_light_direction() {
        let gray = Array2::from_elem((64, 64), 128.0);
        let direction = estimate_local_light_direction(&gray, 16, 16, 32);
        assert!(direction >= -PI && direction <= PI);
    }

    #[test]
    fn test_shadow_inconsistency() {
        let shadow1 = ShadowRegion {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
            direction: 0.0,
            intensity: 50.0,
        };

        let shadow2 = ShadowRegion {
            x: 20,
            y: 20,
            width: 10,
            height: 10,
            direction: PI / 2.0,
            intensity: 50.0,
        };

        let score = compute_shadow_inconsistency(&vec![shadow1, shadow2]);
        assert!(score > 0.0);
    }

    #[test]
    fn test_ambient_occlusion() {
        let gray = Array2::from_elem((20, 20), 128.0);
        let ao = estimate_ambient_occlusion(&gray);
        assert_eq!(ao.dim(), gray.dim());
    }
}
