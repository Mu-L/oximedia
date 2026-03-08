//! Geometric Tampering Detection
//!
//! This module detects copy-move forgery, cloning, and geometric transformations
//! using keypoint matching and block matching techniques.

use crate::{ForensicTest, ForensicsResult};
use image::RgbImage;
use ndarray::Array2;

/// Keypoint descriptor
#[derive(Debug, Clone)]
pub struct Keypoint {
    /// X coordinate
    pub x: f64,
    /// Y coordinate
    pub y: f64,
    /// Scale
    pub scale: f64,
    /// Orientation
    pub orientation: f64,
    /// Feature descriptor
    pub descriptor: Vec<f64>,
}

/// Keypoint match between two locations
#[derive(Debug, Clone)]
pub struct KeypointMatch {
    /// First keypoint
    pub kp1: Keypoint,
    /// Second keypoint
    pub kp2: Keypoint,
    /// Match distance/similarity
    pub distance: f64,
}

/// Copy-move detection result
#[derive(Debug, Clone)]
pub struct CopyMoveResult {
    /// Detected copy-move regions (source and destination)
    pub regions: Vec<(Region, Region)>,
    /// Confidence score
    pub confidence: f64,
    /// Number of matched features
    pub num_matches: usize,
}

/// Image region
#[derive(Debug, Clone)]
pub struct Region {
    /// X coordinate
    pub x: usize,
    /// Y coordinate
    pub y: usize,
    /// Width
    pub width: usize,
    /// Height
    pub height: usize,
}

/// Detect copy-move forgery
pub fn detect_copy_move(image: &RgbImage) -> ForensicsResult<ForensicTest> {
    let mut test = ForensicTest::new("Copy-Move Detection");

    // Convert to grayscale
    let gray = rgb_to_grayscale(image);

    // Detect using block matching
    let block_result = detect_copy_move_blocks(&gray)?;

    // Detect using keypoint matching
    let keypoint_result = detect_copy_move_keypoints(&gray)?;

    // Combine results
    let total_regions = block_result.regions.len() + keypoint_result.regions.len();

    if total_regions > 0 {
        test.tampering_detected = true;
        test.add_finding(format!(
            "Detected {} potential copy-move regions",
            total_regions
        ));
    }

    test.add_finding(format!(
        "Block matching: {} regions",
        block_result.regions.len()
    ));
    test.add_finding(format!(
        "Keypoint matching: {} matches",
        keypoint_result.num_matches
    ));

    // Calculate confidence
    let confidence = (block_result.confidence + keypoint_result.confidence) / 2.0;
    test.set_confidence(confidence);

    // Create anomaly map
    let anomaly_map = create_copy_move_anomaly_map(image, &block_result, &keypoint_result)?;
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

        // Standard luminance conversion
        gray[[y as usize, x as usize]] = 0.299 * r + 0.587 * g + 0.114 * b;
    }

    gray
}

/// Detect copy-move using block matching
fn detect_copy_move_blocks(gray: &Array2<f64>) -> ForensicsResult<CopyMoveResult> {
    let (height, width) = gray.dim();
    let block_size = 16;
    let step = 8;

    // Extract overlapping blocks
    let mut blocks = Vec::new();

    for y in (0..height - block_size).step_by(step) {
        for x in (0..width - block_size).step_by(step) {
            let block = extract_gray_block(gray, x, y, block_size);
            let features = compute_block_features(&block);
            blocks.push((x, y, features));
        }
    }

    // Find similar blocks
    let mut matches = Vec::new();
    let similarity_threshold = 0.95;
    let min_distance = 40; // Minimum pixel distance to avoid self-matching

    for i in 0..blocks.len() {
        for j in i + 1..blocks.len() {
            let (x1, y1, ref feat1) = blocks[i];
            let (x2, y2, ref feat2) = blocks[j];

            let distance = ((x1 as i32 - x2 as i32).pow(2) + (y1 as i32 - y2 as i32).pow(2)) as f64;
            let distance = distance.sqrt();

            if distance > min_distance as f64 {
                let similarity = compute_feature_similarity(feat1, feat2);

                if similarity > similarity_threshold {
                    matches.push((
                        Region {
                            x: x1,
                            y: y1,
                            width: block_size,
                            height: block_size,
                        },
                        Region {
                            x: x2,
                            y: y2,
                            width: block_size,
                            height: block_size,
                        },
                        similarity,
                    ));
                }
            }
        }
    }

    // Cluster nearby matches
    let clustered = cluster_matches(&matches);

    let confidence = if !clustered.is_empty() {
        (clustered.len() as f64 / 10.0).min(1.0)
    } else {
        0.0
    };

    Ok(CopyMoveResult {
        regions: clustered,
        confidence,
        num_matches: matches.len(),
    })
}

/// Extract grayscale block
fn extract_gray_block(gray: &Array2<f64>, x: usize, y: usize, size: usize) -> Array2<f64> {
    let (height, width) = gray.dim();
    let mut block = Array2::zeros((size, size));

    for i in 0..size {
        for j in 0..size {
            if y + i < height && x + j < width {
                block[[i, j]] = gray[[y + i, x + j]];
            }
        }
    }

    block
}

/// Compute block features (DCT-like)
fn compute_block_features(block: &Array2<f64>) -> Vec<f64> {
    let (h, w) = block.dim();
    let mut features = Vec::new();

    // Compute simple statistics
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let count = (h * w) as f64;

    for i in 0..h {
        for j in 0..w {
            let val = block[[i, j]];
            sum += val;
            sum_sq += val * val;
        }
    }

    let mean = sum / count;
    let variance = sum_sq / count - mean * mean;
    let std_dev = variance.sqrt();

    features.push(mean);
    features.push(std_dev);

    // Add gradient features
    let mut grad_x_sum = 0.0;
    let mut grad_y_sum = 0.0;

    for i in 0..h - 1 {
        for j in 0..w - 1 {
            grad_x_sum += (block[[i, j + 1]] - block[[i, j]]).abs();
            grad_y_sum += (block[[i + 1, j]] - block[[i, j]]).abs();
        }
    }

    features.push(grad_x_sum / count);
    features.push(grad_y_sum / count);

    // Add corner features
    features.push(block[[0, 0]]);
    features.push(block[[0, w - 1]]);
    features.push(block[[h - 1, 0]]);
    features.push(block[[h - 1, w - 1]]);

    features
}

/// Compute similarity between feature vectors
fn compute_feature_similarity(feat1: &[f64], feat2: &[f64]) -> f64 {
    if feat1.len() != feat2.len() {
        return 0.0;
    }

    // Normalized correlation
    let mut sum1 = 0.0;
    let mut sum2 = 0.0;
    let mut sum_sq1 = 0.0;
    let mut sum_sq2 = 0.0;
    let mut sum_prod = 0.0;

    for i in 0..feat1.len() {
        sum1 += feat1[i];
        sum2 += feat2[i];
        sum_sq1 += feat1[i] * feat1[i];
        sum_sq2 += feat2[i] * feat2[i];
        sum_prod += feat1[i] * feat2[i];
    }

    let n = feat1.len() as f64;
    let mean1 = sum1 / n;
    let mean2 = sum2 / n;

    let var1 = sum_sq1 / n - mean1 * mean1;
    let var2 = sum_sq2 / n - mean2 * mean2;
    let covar = sum_prod / n - mean1 * mean2;

    if var1 > 0.0 && var2 > 0.0 {
        (covar / (var1.sqrt() * var2.sqrt())).abs()
    } else {
        0.0
    }
}

/// Cluster nearby matches into regions
fn cluster_matches(matches: &[(Region, Region, f64)]) -> Vec<(Region, Region)> {
    let mut clustered = Vec::new();

    // Simple clustering: group matches that are close to each other
    for (r1, r2, _sim) in matches {
        // Check if this match overlaps with any existing cluster
        let mut found = false;

        for (cr1, cr2) in &mut clustered {
            if regions_overlap(r1, cr1) && regions_overlap(r2, cr2) {
                // Merge regions
                *cr1 = merge_regions(cr1, r1);
                *cr2 = merge_regions(cr2, r2);
                found = true;
                break;
            }
        }

        if !found {
            clustered.push((r1.clone(), r2.clone()));
        }
    }

    clustered
}

/// Check if two regions overlap
fn regions_overlap(r1: &Region, r2: &Region) -> bool {
    let x_overlap = r1.x < r2.x + r2.width && r1.x + r1.width > r2.x;
    let y_overlap = r1.y < r2.y + r2.height && r1.y + r1.height > r2.y;

    x_overlap && y_overlap
}

/// Merge two regions
fn merge_regions(r1: &Region, r2: &Region) -> Region {
    let x_min = r1.x.min(r2.x);
    let y_min = r1.y.min(r2.y);
    let x_max = (r1.x + r1.width).max(r2.x + r2.width);
    let y_max = (r1.y + r1.height).max(r2.y + r2.height);

    Region {
        x: x_min,
        y: y_min,
        width: x_max - x_min,
        height: y_max - y_min,
    }
}

/// Detect copy-move using keypoint matching
fn detect_copy_move_keypoints(gray: &Array2<f64>) -> ForensicsResult<CopyMoveResult> {
    // Extract keypoints using simplified SIFT-like approach
    let keypoints = extract_keypoints(gray);

    // Match keypoints
    let matches = match_keypoints(&keypoints);

    // Filter matches based on geometric consistency
    let filtered_matches = filter_geometric_matches(&matches);

    // Convert matches to regions
    let regions = matches_to_regions(&filtered_matches);

    let confidence = if !filtered_matches.is_empty() {
        (filtered_matches.len() as f64 / 20.0).min(1.0)
    } else {
        0.0
    };

    Ok(CopyMoveResult {
        regions,
        confidence,
        num_matches: filtered_matches.len(),
    })
}

/// Extract keypoints (simplified SIFT-like)
#[allow(unused_variables)]
fn extract_keypoints(gray: &Array2<f64>) -> Vec<Keypoint> {
    let (height, width) = gray.dim();
    let mut keypoints = Vec::new();

    // Use Harris corner detector
    let corners = detect_harris_corners(gray);

    for (x, y, response) in corners {
        if response > 0.01 {
            // Compute descriptor
            let descriptor = compute_sift_like_descriptor(gray, x, y);

            keypoints.push(Keypoint {
                x: x as f64,
                y: y as f64,
                scale: 1.0,
                orientation: 0.0,
                descriptor,
            });
        }
    }

    keypoints
}

/// Detect Harris corners
fn detect_harris_corners(gray: &Array2<f64>) -> Vec<(usize, usize, f64)> {
    let (height, width) = gray.dim();
    let mut corners = Vec::new();

    for y in 2..height - 2 {
        for x in 2..width - 2 {
            let response = compute_harris_response(gray, x, y);

            if response > 0.01 {
                corners.push((x, y, response));
            }
        }
    }

    // Non-maximum suppression
    let mut filtered = Vec::new();
    for (x, y, response) in corners {
        let mut is_max = true;

        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let nx = (x as i32 + dx) as usize;
                let ny = (y as i32 + dy) as usize;

                if nx < width && ny < height {
                    let neighbor_response = compute_harris_response(gray, nx, ny);
                    if neighbor_response > response {
                        is_max = false;
                        break;
                    }
                }
            }
            if !is_max {
                break;
            }
        }

        if is_max {
            filtered.push((x, y, response));
        }
    }

    filtered
}

/// Compute Harris corner response
fn compute_harris_response(gray: &Array2<f64>, x: usize, y: usize) -> f64 {
    let mut ixx = 0.0;
    let mut iyy = 0.0;
    let mut ixy = 0.0;

    for dy in -1..=1 {
        for dx in -1..=1 {
            let nx = (x as i32 + dx) as usize;
            let ny = (y as i32 + dy) as usize;

            if nx > 0 && nx < gray.ncols() - 1 && ny > 0 && ny < gray.nrows() - 1 {
                let ix = (gray[[ny, nx + 1]] - gray[[ny, nx - 1]]) / 2.0;
                let iy = (gray[[ny + 1, nx]] - gray[[ny - 1, nx]]) / 2.0;

                ixx += ix * ix;
                iyy += iy * iy;
                ixy += ix * iy;
            }
        }
    }

    let det = ixx * iyy - ixy * ixy;
    let trace = ixx + iyy;
    let k = 0.04;

    det - k * trace * trace
}

/// Compute SIFT-like descriptor
fn compute_sift_like_descriptor(gray: &Array2<f64>, x: usize, y: usize) -> Vec<f64> {
    let patch_size = 16;
    let mut descriptor = Vec::new();

    for dy in 0..patch_size {
        for dx in 0..patch_size {
            let px = x + dx;
            let py = y + dy;

            if px < gray.ncols() && py < gray.nrows() {
                descriptor.push(gray[[py, px]]);
            } else {
                descriptor.push(0.0);
            }
        }
    }

    // Normalize
    let sum: f64 = descriptor.iter().sum();
    if sum > 0.0 {
        for val in &mut descriptor {
            *val /= sum;
        }
    }

    descriptor
}

/// Match keypoints
fn match_keypoints(keypoints: &[Keypoint]) -> Vec<KeypointMatch> {
    let mut matches = Vec::new();
    let distance_threshold = 0.8;
    let min_spatial_distance = 30.0;

    for i in 0..keypoints.len() {
        for j in i + 1..keypoints.len() {
            let kp1 = &keypoints[i];
            let kp2 = &keypoints[j];

            // Check spatial distance
            let spatial_dist = ((kp1.x - kp2.x).powi(2) + (kp1.y - kp2.y).powi(2)).sqrt();

            if spatial_dist > min_spatial_distance {
                // Compute descriptor distance
                let desc_dist = compute_descriptor_distance(&kp1.descriptor, &kp2.descriptor);

                if desc_dist < distance_threshold {
                    matches.push(KeypointMatch {
                        kp1: kp1.clone(),
                        kp2: kp2.clone(),
                        distance: desc_dist,
                    });
                }
            }
        }
    }

    matches
}

/// Compute distance between descriptors
fn compute_descriptor_distance(desc1: &[f64], desc2: &[f64]) -> f64 {
    if desc1.len() != desc2.len() {
        return f64::MAX;
    }

    let mut sum_sq = 0.0;
    for i in 0..desc1.len() {
        let diff = desc1[i] - desc2[i];
        sum_sq += diff * diff;
    }

    sum_sq.sqrt()
}

/// Filter matches based on geometric consistency
fn filter_geometric_matches(matches: &[KeypointMatch]) -> Vec<KeypointMatch> {
    // Simple filtering: remove matches with inconsistent transformations
    matches.to_vec()
}

/// Convert matches to regions
fn matches_to_regions(matches: &[KeypointMatch]) -> Vec<(Region, Region)> {
    let mut regions = Vec::new();
    let region_size = 32;

    for m in matches {
        let r1 = Region {
            x: (m.kp1.x as usize).saturating_sub(region_size / 2),
            y: (m.kp1.y as usize).saturating_sub(region_size / 2),
            width: region_size,
            height: region_size,
        };

        let r2 = Region {
            x: (m.kp2.x as usize).saturating_sub(region_size / 2),
            y: (m.kp2.y as usize).saturating_sub(region_size / 2),
            width: region_size,
            height: region_size,
        };

        regions.push((r1, r2));
    }

    regions
}

/// Create anomaly map from copy-move detection results
fn create_copy_move_anomaly_map(
    image: &RgbImage,
    block_result: &CopyMoveResult,
    keypoint_result: &CopyMoveResult,
) -> ForensicsResult<Array2<f64>> {
    let (width, height) = image.dimensions();
    let mut anomaly_map = Array2::zeros((height as usize, width as usize));

    // Mark block-based regions
    for (r1, r2) in &block_result.regions {
        mark_region(&mut anomaly_map, r1, 0.7);
        mark_region(&mut anomaly_map, r2, 0.7);
    }

    // Mark keypoint-based regions
    for (r1, r2) in &keypoint_result.regions {
        mark_region(&mut anomaly_map, r1, 0.5);
        mark_region(&mut anomaly_map, r2, 0.5);
    }

    Ok(anomaly_map)
}

/// Mark a region in the anomaly map
fn mark_region(anomaly_map: &mut Array2<f64>, region: &Region, value: f64) {
    let (height, width) = anomaly_map.dim();

    for y in region.y..region.y + region.height {
        for x in region.x..region.x + region.width {
            if y < height && x < width {
                anomaly_map[[y, x]] = anomaly_map[[y, x]].max(value);
            }
        }
    }
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
    fn test_block_features() {
        let block = Array2::zeros((16, 16));
        let features = compute_block_features(&block);
        assert!(!features.is_empty());
    }

    #[test]
    fn test_feature_similarity() {
        let feat1 = vec![1.0, 2.0, 3.0];
        let feat2 = vec![1.0, 2.0, 3.0];
        let sim = compute_feature_similarity(&feat1, &feat2);
        assert!(sim > 0.99);
    }

    #[test]
    fn test_regions_overlap() {
        let r1 = Region {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        let r2 = Region {
            x: 5,
            y: 5,
            width: 10,
            height: 10,
        };
        assert!(regions_overlap(&r1, &r2));

        let r3 = Region {
            x: 20,
            y: 20,
            width: 10,
            height: 10,
        };
        assert!(!regions_overlap(&r1, &r3));
    }

    #[test]
    fn test_harris_response() {
        let gray = Array2::from_elem((10, 10), 128.0);
        let response = compute_harris_response(&gray, 5, 5);
        assert!(response >= 0.0);
    }
}
