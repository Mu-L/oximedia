//! Aesthetic quality scoring.

use crate::error::{SceneError, SceneResult};
use serde::{Deserialize, Serialize};

/// Aesthetic quality score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AestheticScore {
    /// Overall aesthetic score (0.0-1.0).
    pub overall: f32,
    /// Color harmony (0.0-1.0).
    pub color_harmony: f32,
    /// Sharpness (0.0-1.0).
    pub sharpness: f32,
    /// Contrast (0.0-1.0).
    pub contrast: f32,
    /// Composition (0.0-1.0).
    pub composition: f32,
    /// Lighting quality (0.0-1.0).
    pub lighting: f32,
    /// Uniqueness (0.0-1.0).
    pub uniqueness: f32,
}

/// Aesthetic scorer.
pub struct AestheticScorer;

impl AestheticScorer {
    /// Create a new aesthetic scorer.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Score aesthetic quality.
    ///
    /// # Errors
    ///
    /// Returns error if scoring fails.
    pub fn score(
        &self,
        rgb_data: &[u8],
        width: usize,
        height: usize,
    ) -> SceneResult<AestheticScore> {
        if rgb_data.len() != width * height * 3 {
            return Err(SceneError::InvalidDimensions(
                "RGB data size mismatch".to_string(),
            ));
        }

        let color_harmony = self.score_color_harmony(rgb_data, width, height);
        let sharpness = self.score_sharpness(rgb_data, width, height);
        let contrast = self.score_contrast(rgb_data, width, height);
        let composition = self.score_composition(rgb_data, width, height);
        let lighting = self.score_lighting(rgb_data, width, height);
        let uniqueness = self.score_uniqueness(rgb_data, width, height);

        let overall = (color_harmony * 0.2
            + sharpness * 0.2
            + contrast * 0.15
            + composition * 0.2
            + lighting * 0.15
            + uniqueness * 0.1)
            .clamp(0.0, 1.0);

        Ok(AestheticScore {
            overall,
            color_harmony,
            sharpness,
            contrast,
            composition,
            lighting,
            uniqueness,
        })
    }

    fn score_color_harmony(&self, rgb_data: &[u8], _width: usize, _height: usize) -> f32 {
        // Analyze color distribution and harmony
        let mut histogram = vec![vec![0u32; 16]; 3];

        for i in (0..rgb_data.len()).step_by(3) {
            for c in 0..3 {
                let bin = (rgb_data[i + c] / 16) as usize;
                histogram[c][bin] += 1;
            }
        }

        // Calculate entropy (lower entropy = more harmonious)
        let mut entropy = 0.0;
        let total = rgb_data.len() / 3;

        for c in 0..3 {
            for &count in &histogram[c] {
                if count > 0 {
                    let p = count as f32 / total as f32;
                    entropy -= p * p.log2();
                }
            }
        }

        (1.0 - (entropy / 12.0).min(1.0)).clamp(0.0, 1.0)
    }

    fn score_sharpness(&self, rgb_data: &[u8], width: usize, height: usize) -> f32 {
        let mut edge_sum = 0.0;
        let mut count = 0;

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let idx = (y * width + x) * 3;
                for c in 0..3 {
                    let center = rgb_data[idx + c] as i32;
                    let left = rgb_data[idx - 3 + c] as i32;
                    let right = rgb_data[idx + 3 + c] as i32;
                    edge_sum += ((center - left).abs() + (center - right).abs()) as f32;
                }
                count += 3;
            }
        }

        if count > 0 {
            (edge_sum / count as f32 / 255.0 * 2.0).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn score_contrast(&self, rgb_data: &[u8], _width: usize, _height: usize) -> f32 {
        let mut min_val = 255u8;
        let mut max_val = 0u8;

        for i in (0..rgb_data.len()).step_by(3) {
            let gray =
                ((rgb_data[i] as u16 + rgb_data[i + 1] as u16 + rgb_data[i + 2] as u16) / 3) as u8;
            min_val = min_val.min(gray);
            max_val = max_val.max(gray);
        }

        (max_val - min_val) as f32 / 255.0
    }

    fn score_composition(&self, rgb_data: &[u8], width: usize, height: usize) -> f32 {
        // Use rule of thirds heuristic
        let third_w = width / 3;
        let third_h = height / 3;

        let mut interest_score = 0.0;

        // Check interest points at rule of thirds intersections
        for y in [third_h, third_h * 2] {
            for x in [third_w, third_w * 2] {
                let idx = (y * width + x) * 3;
                if idx + 2 < rgb_data.len() {
                    // Measure local complexity
                    let mut complexity = 0.0;
                    for dy in 0..10.min(height - y) {
                        for dx in 0..10.min(width - x) {
                            let pidx = ((y + dy) * width + (x + dx)) * 3;
                            if pidx + 2 < rgb_data.len() {
                                for c in 0..3 {
                                    complexity += (rgb_data[pidx + c] as i32
                                        - rgb_data[idx + c] as i32)
                                        .unsigned_abs()
                                        as f32;
                                }
                            }
                        }
                    }
                    interest_score += complexity;
                }
            }
        }

        (interest_score / 100.0 / 255.0 / 12.0).clamp(0.0, 1.0)
    }

    fn score_lighting(&self, rgb_data: &[u8], _width: usize, _height: usize) -> f32 {
        let mut brightness_sum = 0.0;
        let mut count = 0;

        for i in (0..rgb_data.len()).step_by(3) {
            let brightness =
                (rgb_data[i] as f32 + rgb_data[i + 1] as f32 + rgb_data[i + 2] as f32) / 3.0;
            brightness_sum += brightness;
            count += 1;
        }

        if count > 0 {
            let avg_brightness = brightness_sum / count as f32;
            // Good lighting is around 127 (mid-range)
            (1.0 - ((avg_brightness - 127.0).abs() / 127.0)).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn score_uniqueness(&self, rgb_data: &[u8], _width: usize, _height: usize) -> f32 {
        // Measure color diversity as proxy for uniqueness
        let mut unique_colors = std::collections::HashSet::new();

        for i in (0..rgb_data.len()).step_by(3) {
            let color = (rgb_data[i] / 32, rgb_data[i + 1] / 32, rgb_data[i + 2] / 32);
            unique_colors.insert(color);
        }

        (unique_colors.len() as f32 / 512.0).clamp(0.0, 1.0)
    }
}

impl Default for AestheticScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aesthetic_scorer() {
        let scorer = AestheticScorer::new();
        let width = 320;
        let height = 240;
        let rgb_data = vec![128u8; width * height * 3];

        let result = scorer.score(&rgb_data, width, height);
        assert!(result.is_ok());

        let score = result.expect("should succeed in test");
        assert!(score.overall >= 0.0 && score.overall <= 1.0);
    }
}
