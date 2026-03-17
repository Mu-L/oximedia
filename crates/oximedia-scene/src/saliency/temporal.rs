//! Temporal saliency detection for video using motion-weighted attention maps.
//!
//! Combines spatial saliency with inter-frame motion to produce attention maps
//! that highlight regions of dynamic visual interest (e.g., moving objects).

use crate::error::{SceneError, SceneResult};
use serde::{Deserialize, Serialize};

/// Temporal saliency map that blends spatial saliency with motion information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalSaliencyMap {
    /// Combined saliency values (0.0-1.0).
    pub data: Vec<f32>,
    /// Motion magnitude map (0.0-1.0).
    pub motion_map: Vec<f32>,
    /// Spatial saliency component (0.0-1.0).
    pub spatial_map: Vec<f32>,
    /// Width.
    pub width: usize,
    /// Height.
    pub height: usize,
}

/// Configuration for temporal saliency.
#[derive(Debug, Clone)]
pub struct TemporalSaliencyConfig {
    /// Weight of spatial saliency in final blend (0.0-1.0).
    pub spatial_weight: f32,
    /// Weight of motion saliency in final blend (0.0-1.0).
    pub motion_weight: f32,
    /// Temporal decay factor for exponential moving average (0.0-1.0).
    pub temporal_decay: f32,
    /// Block size for motion estimation.
    pub motion_block_size: usize,
    /// Search range for block matching.
    pub search_range: usize,
}

impl Default for TemporalSaliencyConfig {
    fn default() -> Self {
        Self {
            spatial_weight: 0.4,
            motion_weight: 0.6,
            temporal_decay: 0.7,
            motion_block_size: 8,
            search_range: 4,
        }
    }
}

/// Temporal saliency detector for video sequences.
pub struct TemporalSaliencyDetector {
    config: TemporalSaliencyConfig,
    /// Previous grayscale frame for motion computation.
    prev_gray: Option<Vec<f32>>,
    prev_width: usize,
    prev_height: usize,
    /// Accumulated temporal saliency (EMA).
    accumulated: Option<Vec<f32>>,
}

impl TemporalSaliencyDetector {
    /// Create a new temporal saliency detector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: TemporalSaliencyConfig::default(),
            prev_gray: None,
            prev_width: 0,
            prev_height: 0,
            accumulated: None,
        }
    }

    /// Create with custom configuration.
    #[must_use]
    pub fn with_config(config: TemporalSaliencyConfig) -> Self {
        Self {
            config,
            prev_gray: None,
            prev_width: 0,
            prev_height: 0,
            accumulated: None,
        }
    }

    /// Process a frame and return a temporal saliency map.
    ///
    /// # Errors
    ///
    /// Returns error if dimensions are invalid.
    pub fn process_frame(
        &mut self,
        rgb_data: &[u8],
        width: usize,
        height: usize,
    ) -> SceneResult<TemporalSaliencyMap> {
        if rgb_data.len() != width * height * 3 {
            return Err(SceneError::InvalidDimensions(
                "RGB data size mismatch".to_string(),
            ));
        }

        let gray = rgb_to_gray(rgb_data);
        let spatial = compute_spatial_saliency(&gray, width, height);
        let motion = self.compute_motion_saliency(&gray, width, height);

        // Blend spatial and motion
        let sw = self.config.spatial_weight;
        let mw = self.config.motion_weight;
        let total_weight = sw + mw;
        let mut combined: Vec<f32> = spatial
            .iter()
            .zip(motion.iter())
            .map(|(s, m)| (s * sw + m * mw) / total_weight)
            .collect();

        // Apply temporal accumulation (EMA)
        if let Some(ref acc) = self.accumulated {
            if acc.len() == combined.len() {
                let decay = self.config.temporal_decay;
                for (c, a) in combined.iter_mut().zip(acc.iter()) {
                    *c = *c * (1.0 - decay) + *a * decay;
                }
            }
        }

        // Normalize to [0, 1]
        let max_val = combined.iter().copied().fold(f32::MIN, f32::max);
        if max_val > 0.0 {
            for v in &mut combined {
                *v /= max_val;
            }
        }

        self.accumulated = Some(combined.clone());
        self.prev_gray = Some(gray);
        self.prev_width = width;
        self.prev_height = height;

        Ok(TemporalSaliencyMap {
            data: combined,
            motion_map: motion,
            spatial_map: spatial,
            width,
            height,
        })
    }

    /// Compute motion saliency from inter-frame difference.
    fn compute_motion_saliency(&self, gray: &[f32], width: usize, height: usize) -> Vec<f32> {
        let Some(ref prev) = self.prev_gray else {
            return vec![0.0; width * height];
        };

        if prev.len() != width * height || self.prev_width != width || self.prev_height != height {
            return vec![0.0; width * height];
        }

        let block = self.config.motion_block_size;
        let mut motion = vec![0.0_f32; width * height];

        // Block-level motion estimation
        let blocks_x = width / block;
        let blocks_y = height / block;

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let ox = bx * block;
                let oy = by * block;

                // Compute block-level absolute difference (SAD)
                let mut sad = 0.0_f32;
                let mut count = 0;
                for dy in 0..block {
                    for dx in 0..block {
                        let px = ox + dx;
                        let py = oy + dy;
                        if px < width && py < height {
                            let idx = py * width + px;
                            sad += (gray[idx] - prev[idx]).abs();
                            count += 1;
                        }
                    }
                }

                if count > 0 {
                    let avg_diff = sad / count as f32;
                    // Spread block motion to pixels
                    for dy in 0..block {
                        for dx in 0..block {
                            let px = ox + dx;
                            let py = oy + dy;
                            if px < width && py < height {
                                motion[py * width + px] = avg_diff;
                            }
                        }
                    }
                }
            }
        }

        // Normalize
        let max_motion = motion.iter().copied().fold(0.0_f32, f32::max);
        if max_motion > 0.0 {
            for m in &mut motion {
                *m /= max_motion;
            }
        }

        motion
    }

    /// Reset the detector state.
    pub fn reset(&mut self) {
        self.prev_gray = None;
        self.accumulated = None;
    }

    /// Check if the detector has a previous frame for motion computation.
    #[must_use]
    pub fn has_previous_frame(&self) -> bool {
        self.prev_gray.is_some()
    }
}

impl Default for TemporalSaliencyDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert RGB to grayscale.
fn rgb_to_gray(rgb: &[u8]) -> Vec<f32> {
    let mut gray = Vec::with_capacity(rgb.len() / 3);
    for chunk in rgb.chunks_exact(3) {
        let r = chunk[0] as f32;
        let g = chunk[1] as f32;
        let b = chunk[2] as f32;
        gray.push((0.299 * r + 0.587 * g + 0.114 * b) / 255.0);
    }
    gray
}

/// Compute spatial saliency using center-surround at a single scale.
fn compute_spatial_saliency(gray: &[f32], width: usize, height: usize) -> Vec<f32> {
    let mut saliency = vec![0.0; width * height];
    let scale = 8;

    if width <= scale * 2 || height <= scale * 2 {
        return saliency;
    }

    for y in scale..height - scale {
        for x in scale..width - scale {
            let idx = y * width + x;
            let center = gray[idx];

            let mut surround_sum = 0.0;
            let mut count = 0;
            for dy in -(scale as i32)..=scale as i32 {
                for dx in -(scale as i32)..=scale as i32 {
                    if dx.abs() < (scale as i32) / 2 && dy.abs() < (scale as i32) / 2 {
                        continue;
                    }
                    let nx = (x as i32 + dx) as usize;
                    let ny = (y as i32 + dy) as usize;
                    surround_sum += gray[ny * width + nx];
                    count += 1;
                }
            }

            if count > 0 {
                saliency[idx] = (center - surround_sum / count as f32).abs();
            }
        }
    }

    // Normalize
    let max_s = saliency.iter().copied().fold(f32::MIN, f32::max);
    if max_s > 0.0 {
        for s in &mut saliency {
            *s /= max_s;
        }
    }

    saliency
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporal_saliency_single_frame() {
        let mut detector = TemporalSaliencyDetector::new();
        let width = 100;
        let height = 100;
        let rgb_data = vec![128u8; width * height * 3];

        let result = detector.process_frame(&rgb_data, width, height);
        assert!(result.is_ok());
        let map = result.expect("should succeed");
        assert_eq!(map.data.len(), width * height);
        assert_eq!(map.width, width);
        assert_eq!(map.height, height);
        // First frame: motion should be zero
        assert!(map.motion_map.iter().all(|&m| m == 0.0));
    }

    #[test]
    fn test_temporal_saliency_two_frames() {
        let mut detector = TemporalSaliencyDetector::new();
        let width = 100;
        let height = 100;
        let frame1 = vec![100u8; width * height * 3];
        let frame2 = vec![200u8; width * height * 3];

        let _ = detector.process_frame(&frame1, width, height);
        assert!(detector.has_previous_frame());

        let result = detector.process_frame(&frame2, width, height);
        assert!(result.is_ok());
        let map = result.expect("should succeed");
        // With different frames, there should be some motion
        let max_motion = map.motion_map.iter().copied().fold(0.0_f32, f32::max);
        assert!(max_motion > 0.0);
    }

    #[test]
    fn test_temporal_saliency_identical_frames() {
        let mut detector = TemporalSaliencyDetector::new();
        let width = 100;
        let height = 100;
        let frame = vec![128u8; width * height * 3];

        let _ = detector.process_frame(&frame, width, height);
        let result = detector.process_frame(&frame, width, height);
        assert!(result.is_ok());
        let map = result.expect("should succeed");
        // No motion for identical frames
        assert!(map.motion_map.iter().all(|&m| m == 0.0));
    }

    #[test]
    fn test_temporal_saliency_reset() {
        let mut detector = TemporalSaliencyDetector::new();
        let frame = vec![128u8; 100 * 100 * 3];
        let _ = detector.process_frame(&frame, 100, 100);
        assert!(detector.has_previous_frame());
        detector.reset();
        assert!(!detector.has_previous_frame());
    }

    #[test]
    fn test_temporal_saliency_invalid_size() {
        let mut detector = TemporalSaliencyDetector::new();
        let result = detector.process_frame(&[0u8; 10], 100, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_temporal_saliency_accumulation() {
        let mut detector = TemporalSaliencyDetector::new();
        let width = 50;
        let height = 50;
        let frame1 = vec![100u8; width * height * 3];
        let mut frame2 = vec![100u8; width * height * 3];
        // Add motion in one region
        for y in 10..20 {
            for x in 10..20 {
                let idx = (y * width + x) * 3;
                frame2[idx] = 255;
                frame2[idx + 1] = 255;
                frame2[idx + 2] = 255;
            }
        }

        let _ = detector.process_frame(&frame1, width, height);
        let result = detector.process_frame(&frame2, width, height);
        assert!(result.is_ok());
    }

    #[test]
    fn test_temporal_saliency_custom_config() {
        let config = TemporalSaliencyConfig {
            spatial_weight: 0.5,
            motion_weight: 0.5,
            temporal_decay: 0.5,
            motion_block_size: 4,
            search_range: 2,
        };
        let mut detector = TemporalSaliencyDetector::with_config(config);
        let frame = vec![128u8; 100 * 100 * 3];
        let result = detector.process_frame(&frame, 100, 100);
        assert!(result.is_ok());
    }
}
