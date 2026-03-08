//! Saliency detection using spectral methods.

use crate::error::{SceneError, SceneResult};
use serde::{Deserialize, Serialize};

/// Saliency map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaliencyMap {
    /// Saliency values (0.0-1.0).
    pub data: Vec<f32>,
    /// Width.
    pub width: usize,
    /// Height.
    pub height: usize,
}

/// Saliency detector using spectral residual method.
pub struct SaliencyDetector;

impl SaliencyDetector {
    /// Create a new saliency detector.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Detect salient regions.
    ///
    /// # Errors
    ///
    /// Returns error if detection fails.
    pub fn detect(&self, rgb_data: &[u8], width: usize, height: usize) -> SceneResult<SaliencyMap> {
        if rgb_data.len() != width * height * 3 {
            return Err(SceneError::InvalidDimensions(
                "RGB data size mismatch".to_string(),
            ));
        }

        // Convert to grayscale
        let mut gray = Vec::with_capacity(width * height);
        for i in (0..rgb_data.len()).step_by(3) {
            let r = rgb_data[i] as f32;
            let g = rgb_data[i + 1] as f32;
            let b = rgb_data[i + 2] as f32;
            let y = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
            gray.push(y);
        }

        // Compute saliency using center-surround difference
        let saliency = self.compute_saliency(&gray, width, height);

        Ok(SaliencyMap {
            data: saliency,
            width,
            height,
        })
    }

    /// Compute saliency using multi-scale center-surround.
    fn compute_saliency(&self, gray: &[f32], width: usize, height: usize) -> Vec<f32> {
        let mut saliency = vec![0.0; width * height];

        // Multiple scales
        for scale in [8, 16, 32] {
            for y in scale..height - scale {
                for x in scale..width - scale {
                    let idx = y * width + x;
                    let center = gray[idx];

                    // Compute surround average
                    let mut surround_sum = 0.0;
                    let mut count = 0;

                    for dy in -(scale as i32)..=scale as i32 {
                        for dx in -(scale as i32)..=scale as i32 {
                            if dx.abs() < scale as i32 / 2 && dy.abs() < scale as i32 / 2 {
                                continue; // Skip center region
                            }

                            let nx = x as i32 + dx;
                            let ny = y as i32 + dy;

                            if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                                surround_sum += gray[ny as usize * width + nx as usize];
                                count += 1;
                            }
                        }
                    }

                    if count > 0 {
                        let surround = surround_sum / count as f32;
                        saliency[idx] += (center - surround).abs();
                    }
                }
            }
        }

        // Normalize
        let max_sal = saliency.iter().copied().fold(f32::MIN, f32::max);
        if max_sal > 0.0 {
            for s in &mut saliency {
                *s /= max_sal;
            }
        }

        saliency
    }
}

impl Default for SaliencyDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saliency_detector() {
        let detector = SaliencyDetector::new();
        let width = 100;
        let height = 100;
        let rgb_data = vec![128u8; width * height * 3];

        let result = detector.detect(&rgb_data, width, height);
        assert!(result.is_ok());

        let map = result.expect("should succeed in test");
        assert_eq!(map.data.len(), width * height);
    }
}
