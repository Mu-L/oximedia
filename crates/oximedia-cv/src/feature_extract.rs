//! Feature extraction: SIFT-like keypoints, ORB-like descriptors, and BRIEF.
//!
//! This module provides CPU-based feature extraction algorithms used for
//! image matching, object recognition, and visual odometry.

use std::f32;

/// A 2-D image keypoint with scale and orientation.
#[derive(Debug, Clone, PartialEq)]
pub struct Keypoint {
    /// Pixel column (x coordinate).
    pub x: f32,
    /// Pixel row (y coordinate).
    pub y: f32,
    /// Scale (sigma of the Gaussian that detected this point).
    pub scale: f32,
    /// Orientation in radians.
    pub angle: f32,
    /// Detector response strength.
    pub response: f32,
    /// Octave index in the scale-space pyramid.
    pub octave: i32,
}

impl Keypoint {
    /// Create a new keypoint.
    #[must_use]
    pub fn new(x: f32, y: f32, scale: f32, angle: f32, response: f32, octave: i32) -> Self {
        Self {
            x,
            y,
            scale,
            angle,
            response,
            octave,
        }
    }

    /// Euclidean distance to another keypoint (spatial only).
    #[must_use]
    pub fn distance_to(&self, other: &Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Binary descriptor (256 bits = 32 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefDescriptor {
    /// 32-byte bit string.
    pub bits: [u8; 32],
}

impl BriefDescriptor {
    /// Create a zero descriptor.
    #[must_use]
    pub fn zero() -> Self {
        Self { bits: [0u8; 32] }
    }

    /// Hamming distance to another descriptor.
    #[must_use]
    pub fn hamming_distance(&self, other: &Self) -> u32 {
        self.bits
            .iter()
            .zip(other.bits.iter())
            .map(|(a, b)| (a ^ b).count_ones())
            .sum()
    }

    /// Set bit at position `pos` (0–255).
    pub fn set_bit(&mut self, pos: usize) {
        if pos < 256 {
            self.bits[pos / 8] |= 1 << (pos % 8);
        }
    }

    /// Test bit at position `pos`.
    #[must_use]
    pub fn test_bit(&self, pos: usize) -> bool {
        pos < 256 && (self.bits[pos / 8] & (1 << (pos % 8))) != 0
    }

    /// Number of set bits.
    #[must_use]
    pub fn popcount(&self) -> u32 {
        self.bits.iter().map(|b| b.count_ones()).sum()
    }
}

/// SIFT-like scale-space keypoint detector.
///
/// This is a simplified stand-alone implementation suitable for testing and
/// demonstration; it does not require an external vision library.
#[derive(Debug, Default)]
pub struct SiftDetector {
    /// Number of scale-space octaves.
    pub octaves: usize,
    /// Number of DoG intervals per octave.
    pub intervals: usize,
    /// Peak-response threshold (contrast).
    pub contrast_threshold: f32,
    /// Edge-response threshold.
    pub edge_threshold: f32,
}

impl SiftDetector {
    /// Create a detector with default parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            octaves: 4,
            intervals: 3,
            contrast_threshold: 0.04,
            edge_threshold: 10.0,
        }
    }

    /// Detect keypoints in a grayscale image (row-major, row×col).
    ///
    /// The implementation uses a simplified DoG-inspired heuristic
    /// (local max of squared pixel value) to locate candidate keypoints.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn detect(&self, image: &[f32], width: usize, height: usize) -> Vec<Keypoint> {
        if image.len() != width * height || width < 3 || height < 3 {
            return Vec::new();
        }

        let mut keypoints = Vec::new();
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let center = image[y * width + x];
                if center < self.contrast_threshold {
                    continue;
                }
                // Simple 3×3 local-max check
                let is_max = (-1i32..=1).all(|dy| {
                    (-1i32..=1).all(|dx| {
                        if dx == 0 && dy == 0 {
                            return true;
                        }
                        let ny = (y as i32 + dy) as usize;
                        let nx = (x as i32 + dx) as usize;
                        image[ny * width + nx] <= center
                    })
                });
                if is_max {
                    keypoints.push(Keypoint::new(x as f32, y as f32, 1.6, 0.0, center, 0));
                }
            }
        }
        keypoints
    }
}

/// ORB-like feature extractor (keypoints + binary descriptors).
#[derive(Debug, Default)]
pub struct OrbExtractor {
    /// Maximum number of keypoints to return.
    pub max_features: usize,
    /// FAST corner detection threshold.
    pub fast_threshold: f32,
}

impl OrbExtractor {
    /// Create a new extractor.
    #[must_use]
    pub fn new(max_features: usize) -> Self {
        Self {
            max_features,
            fast_threshold: 20.0 / 255.0,
        }
    }

    /// Extract keypoints and BRIEF descriptors from a grayscale image.
    ///
    /// Returns `(keypoints, descriptors)`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn extract(
        &self,
        image: &[f32],
        width: usize,
        height: usize,
    ) -> (Vec<Keypoint>, Vec<BriefDescriptor>) {
        let detector = SiftDetector {
            contrast_threshold: self.fast_threshold,
            ..SiftDetector::new()
        };
        let mut kps = detector.detect(image, width, height);

        // Sort by response descending, then limit
        kps.sort_by(|a, b| {
            b.response
                .partial_cmp(&a.response)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        kps.truncate(self.max_features);

        let descs: Vec<BriefDescriptor> = kps
            .iter()
            .map(|kp| brief_descriptor(image, width, height, kp))
            .collect();

        (kps, descs)
    }
}

/// Compute a BRIEF-style binary descriptor around `kp`.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
fn brief_descriptor(image: &[f32], width: usize, height: usize, kp: &Keypoint) -> BriefDescriptor {
    let mut desc = BriefDescriptor::zero();
    let cx = kp.x as usize;
    let cy = kp.y as usize;
    // Deterministic pseudo-random pattern based on bit position
    for bit in 0usize..256 {
        let (p1, p2) = brief_pair(bit, cx, cy, width, height);
        let v1 = image[p1];
        let v2 = image[p2];
        if v1 < v2 {
            desc.set_bit(bit);
        }
    }
    desc
}

/// Map a bit index to two pixel indices using a fixed pseudo-random pattern.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
fn brief_pair(bit: usize, cx: usize, cy: usize, width: usize, height: usize) -> (usize, usize) {
    // Simple deterministic pattern (Gaussian-like grid sampling)
    let offsets: [(i32, i32, i32, i32); 4] =
        [(-4, -4, 4, 4), (0, -4, 0, 4), (-4, 0, 4, 0), (-2, -2, 2, 2)];
    let o = offsets[bit % 4];
    let scale = (bit / 4 + 1) as i32;

    let y1 = (cy as i32 + o.1 * scale).clamp(0, height as i32 - 1) as usize;
    let x1 = (cx as i32 + o.0 * scale).clamp(0, width as i32 - 1) as usize;
    let y2 = (cy as i32 + o.3 * scale).clamp(0, height as i32 - 1) as usize;
    let x2 = (cx as i32 + o.2 * scale).clamp(0, width as i32 - 1) as usize;

    (y1 * width + x1, y2 * width + x2)
}

/// Match two sets of descriptors by minimum Hamming distance.
///
/// Returns a list of `(index_in_a, index_in_b, distance)` pairs.
#[must_use]
pub fn match_descriptors(
    descs_a: &[BriefDescriptor],
    descs_b: &[BriefDescriptor],
    max_distance: u32,
) -> Vec<(usize, usize, u32)> {
    let mut matches = Vec::new();
    for (i, da) in descs_a.iter().enumerate() {
        let best = descs_b
            .iter()
            .enumerate()
            .map(|(j, db)| (j, da.hamming_distance(db)))
            .min_by_key(|&(_, d)| d);
        if let Some((j, dist)) = best {
            if dist <= max_distance {
                matches.push((i, j, dist));
            }
        }
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_image(w: usize, h: usize) -> Vec<f32> {
        vec![0.0f32; w * h]
    }

    fn image_with_peak(w: usize, h: usize, px: usize, py: usize, val: f32) -> Vec<f32> {
        let mut img = blank_image(w, h);
        img[py * w + px] = val;
        img
    }

    #[test]
    fn test_keypoint_creation() {
        let kp = Keypoint::new(10.0, 20.0, 1.6, 0.5, 0.9, 0);
        assert!((kp.x - 10.0).abs() < 1e-6);
        assert!((kp.y - 20.0).abs() < 1e-6);
        assert!((kp.scale - 1.6).abs() < 1e-6);
    }

    #[test]
    fn test_keypoint_distance_to_self() {
        let kp = Keypoint::new(5.0, 5.0, 1.0, 0.0, 0.5, 0);
        assert!((kp.distance_to(&kp) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_keypoint_distance() {
        let a = Keypoint::new(0.0, 0.0, 1.0, 0.0, 0.5, 0);
        let b = Keypoint::new(3.0, 4.0, 1.0, 0.0, 0.5, 0);
        assert!((a.distance_to(&b) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_brief_descriptor_zero() {
        let d = BriefDescriptor::zero();
        assert_eq!(d.popcount(), 0);
    }

    #[test]
    fn test_brief_descriptor_set_and_test_bit() {
        let mut d = BriefDescriptor::zero();
        d.set_bit(7);
        d.set_bit(255);
        assert!(d.test_bit(7));
        assert!(d.test_bit(255));
        assert!(!d.test_bit(6));
    }

    #[test]
    fn test_brief_descriptor_hamming_distance_zero() {
        let d = BriefDescriptor::zero();
        assert_eq!(d.hamming_distance(&d), 0);
    }

    #[test]
    fn test_brief_descriptor_hamming_distance_one() {
        let mut d2 = BriefDescriptor::zero();
        d2.set_bit(0);
        assert_eq!(BriefDescriptor::zero().hamming_distance(&d2), 1);
    }

    #[test]
    fn test_sift_detector_empty_image_returns_empty() {
        let det = SiftDetector::new();
        let kps = det.detect(&[], 0, 0);
        assert!(kps.is_empty());
    }

    #[test]
    fn test_sift_detector_detects_peak() {
        let det = SiftDetector {
            contrast_threshold: 0.5,
            ..SiftDetector::new()
        };
        let img = image_with_peak(10, 10, 5, 5, 1.0);
        let kps = det.detect(&img, 10, 10);
        assert!(!kps.is_empty());
        let kp = &kps[0];
        assert_eq!(kp.x as usize, 5);
        assert_eq!(kp.y as usize, 5);
    }

    #[test]
    fn test_sift_detector_below_threshold_no_keypoints() {
        let det = SiftDetector {
            contrast_threshold: 0.9,
            ..SiftDetector::new()
        };
        // Peak is 0.1 < 0.9
        let img = image_with_peak(10, 10, 5, 5, 0.1);
        let kps = det.detect(&img, 10, 10);
        assert!(kps.is_empty());
    }

    #[test]
    fn test_orb_extractor_limits_features() {
        let img = {
            let mut v = blank_image(20, 20);
            // Place several peaks
            for &(x, y) in &[(3, 3), (7, 7), (12, 12), (16, 16)] {
                v[y * 20 + x] = 1.0;
            }
            v
        };
        let ext = OrbExtractor::new(2);
        let (kps, descs) = ext.extract(&img, 20, 20);
        assert!(kps.len() <= 2);
        assert_eq!(kps.len(), descs.len());
    }

    #[test]
    fn test_match_descriptors_identical() {
        let d: Vec<BriefDescriptor> = vec![BriefDescriptor::zero()];
        let matches = match_descriptors(&d, &d, 0);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].2, 0);
    }

    #[test]
    fn test_match_descriptors_too_far() {
        let mut d2 = BriefDescriptor::zero();
        // Set 200 bits
        for i in 0..200 {
            d2.set_bit(i);
        }
        let d1 = vec![BriefDescriptor::zero()];
        let d2 = vec![d2];
        let matches = match_descriptors(&d1, &d2, 10); // max_dist=10
        assert!(matches.is_empty());
    }

    #[test]
    fn test_brief_popcount() {
        let mut d = BriefDescriptor::zero();
        d.set_bit(0);
        d.set_bit(1);
        d.set_bit(2);
        assert_eq!(d.popcount(), 3);
    }
}
