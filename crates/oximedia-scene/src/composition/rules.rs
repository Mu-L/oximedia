//! Composition rules analysis (rule of thirds, golden ratio, etc.).

use crate::common::Point;
use crate::error::{SceneError, SceneResult};
use serde::{Deserialize, Serialize};

/// Composition analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositionScore {
    /// Overall composition quality (0.0-1.0).
    pub overall_score: f32,
    /// Rule of thirds adherence (0.0-1.0).
    pub rule_of_thirds: f32,
    /// Golden ratio adherence (0.0-1.0).
    pub golden_ratio: f32,
    /// Symmetry score (0.0-1.0).
    pub symmetry: f32,
    /// Leading lines present (0.0-1.0).
    pub leading_lines: f32,
    /// Frame within frame (0.0-1.0).
    pub frame_in_frame: f32,
    /// Points of interest.
    pub interest_points: Vec<Point>,
}

/// Composition analyzer using classical rules.
pub struct CompositionAnalyzer;

impl CompositionAnalyzer {
    /// Create a new composition analyzer.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Analyze composition of an image.
    ///
    /// # Errors
    ///
    /// Returns error if analysis fails.
    pub fn analyze(
        &self,
        rgb_data: &[u8],
        width: usize,
        height: usize,
    ) -> SceneResult<CompositionScore> {
        if rgb_data.len() != width * height * 3 {
            return Err(SceneError::InvalidDimensions(
                "RGB data size mismatch".to_string(),
            ));
        }

        // Detect interest points using edge density
        let interest_points = self.detect_interest_points(rgb_data, width, height);

        // Analyze rule of thirds
        let rule_of_thirds = self.analyze_rule_of_thirds(&interest_points, width, height);

        // Analyze golden ratio
        let golden_ratio = self.analyze_golden_ratio(&interest_points, width, height);

        // Analyze symmetry
        let symmetry = self.analyze_symmetry(rgb_data, width, height);

        // Detect leading lines
        let leading_lines = self.detect_leading_lines(rgb_data, width, height);

        // Detect frames within frame
        let frame_in_frame = self.detect_frame_in_frame(rgb_data, width, height);

        // Calculate overall score
        let overall_score = (rule_of_thirds * 0.3
            + golden_ratio * 0.2
            + symmetry * 0.2
            + leading_lines * 0.15
            + frame_in_frame * 0.15)
            .clamp(0.0, 1.0);

        Ok(CompositionScore {
            overall_score,
            rule_of_thirds,
            golden_ratio,
            symmetry,
            leading_lines,
            frame_in_frame,
            interest_points,
        })
    }

    /// Detect points of interest using edge density.
    fn detect_interest_points(&self, rgb_data: &[u8], width: usize, height: usize) -> Vec<Point> {
        let mut points = Vec::new();
        let block_size = width.min(height) / 10;

        for y in (0..height - block_size).step_by(block_size) {
            for x in (0..width - block_size).step_by(block_size) {
                let edge_density = self.compute_edge_density(rgb_data, width, x, y, block_size);

                if edge_density > 0.3 {
                    points.push(Point::new(
                        (x + block_size / 2) as f32,
                        (y + block_size / 2) as f32,
                    ));
                }
            }
        }

        points
    }

    /// Compute edge density in a block.
    fn compute_edge_density(
        &self,
        rgb_data: &[u8],
        width: usize,
        x: usize,
        y: usize,
        size: usize,
    ) -> f32 {
        let mut edge_count = 0;
        let mut total = 0;

        for dy in 0..size {
            for dx in 0..size.saturating_sub(1) {
                let idx = ((y + dy) * width + (x + dx)) * 3;
                let idx_next = ((y + dy) * width + (x + dx + 1)) * 3;

                if idx + 2 < rgb_data.len() && idx_next + 2 < rgb_data.len() {
                    let diff = ((rgb_data[idx] as i32 - rgb_data[idx_next] as i32).abs()
                        + (rgb_data[idx + 1] as i32 - rgb_data[idx_next + 1] as i32).abs()
                        + (rgb_data[idx + 2] as i32 - rgb_data[idx_next + 2] as i32).abs())
                        as u32;

                    if diff > 30 {
                        edge_count += 1;
                    }
                    total += 1;
                }
            }
        }

        if total > 0 {
            edge_count as f32 / total as f32
        } else {
            0.0
        }
    }

    /// Analyze adherence to rule of thirds.
    fn analyze_rule_of_thirds(
        &self,
        interest_points: &[Point],
        width: usize,
        height: usize,
    ) -> f32 {
        // Rule of thirds divides image into 3x3 grid
        let third_w = width as f32 / 3.0;
        let third_h = height as f32 / 3.0;

        let power_points = [
            Point::new(third_w, third_h),
            Point::new(third_w * 2.0, third_h),
            Point::new(third_w, third_h * 2.0),
            Point::new(third_w * 2.0, third_h * 2.0),
        ];

        let threshold = width.min(height) as f32 * 0.1;
        let mut score = 0.0;

        for power_point in &power_points {
            let mut closest_dist = f32::MAX;
            for interest_point in interest_points {
                let dist = power_point.distance(interest_point);
                closest_dist = closest_dist.min(dist);
            }

            if closest_dist < threshold {
                score += 0.25;
            }
        }

        score
    }

    /// Analyze adherence to golden ratio.
    fn analyze_golden_ratio(&self, interest_points: &[Point], width: usize, height: usize) -> f32 {
        const GOLDEN_RATIO: f32 = 1.618;
        let golden_w = width as f32 / GOLDEN_RATIO;
        let golden_h = height as f32 / GOLDEN_RATIO;

        let golden_points = [
            Point::new(golden_w, golden_h),
            Point::new(width as f32 - golden_w, golden_h),
            Point::new(golden_w, height as f32 - golden_h),
            Point::new(width as f32 - golden_w, height as f32 - golden_h),
        ];

        let threshold = width.min(height) as f32 * 0.1;
        let mut score = 0.0;

        for golden_point in &golden_points {
            let mut closest_dist = f32::MAX;
            for interest_point in interest_points {
                let dist = golden_point.distance(interest_point);
                closest_dist = closest_dist.min(dist);
            }

            if closest_dist < threshold {
                score += 0.25;
            }
        }

        score
    }

    /// Analyze symmetry.
    fn analyze_symmetry(&self, rgb_data: &[u8], width: usize, height: usize) -> f32 {
        let mut diff_sum = 0u64;
        let mut count = 0u64;

        // Check horizontal symmetry
        for y in 0..height {
            for x in 0..width / 2 {
                let left_idx = (y * width + x) * 3;
                let right_idx = (y * width + (width - 1 - x)) * 3;

                if right_idx + 2 < rgb_data.len() {
                    for c in 0..3 {
                        diff_sum += (rgb_data[left_idx + c] as i32 - rgb_data[right_idx + c] as i32)
                            .unsigned_abs() as u64;
                    }
                    count += 3;
                }
            }
        }

        if count > 0 {
            let avg_diff = diff_sum as f32 / count as f32;
            (1.0 - avg_diff / 255.0).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Detect leading lines.
    fn detect_leading_lines(&self, rgb_data: &[u8], width: usize, height: usize) -> f32 {
        // Simplified: look for strong diagonal edges
        let mut diagonal_strength = 0.0;
        let mut count = 0;

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let idx = (y * width + x) * 3;
                let diag1_idx = ((y - 1) * width + (x - 1)) * 3;
                let diag2_idx = ((y - 1) * width + (x + 1)) * 3;

                if diag1_idx + 2 < rgb_data.len() && diag2_idx + 2 < rgb_data.len() {
                    let mut diag_diff = 0.0;
                    for c in 0..3 {
                        diag_diff += ((rgb_data[idx + c] as i32 - rgb_data[diag1_idx + c] as i32)
                            .abs()
                            + (rgb_data[idx + c] as i32 - rgb_data[diag2_idx + c] as i32).abs())
                            as f32;
                    }
                    diagonal_strength += diag_diff;
                    count += 1;
                }
            }
        }

        if count > 0 {
            (diagonal_strength / count as f32 / 255.0 / 6.0).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Detect frame within frame.
    fn detect_frame_in_frame(&self, rgb_data: &[u8], width: usize, height: usize) -> f32 {
        // Look for rectangular structures in the image
        let border_width = width / 10;
        let border_height = height / 10;

        let mut edge_density_border = 0.0;
        let mut edge_density_center = 0.0;

        // Check border regions
        for y in 0..border_height {
            for x in 0..width {
                let idx = (y * width + x) * 3;
                if idx + width * 3 < rgb_data.len() {
                    edge_density_border += self.compute_pixel_edge_strength(rgb_data, width, x, y);
                }
            }
        }

        // Check center
        for y in border_height..height - border_height {
            for x in border_width..width - border_width {
                edge_density_center += self.compute_pixel_edge_strength(rgb_data, width, x, y);
            }
        }

        let border_pixels = (border_height * width * 2) as f32;
        let center_pixels = ((height - 2 * border_height) * (width - 2 * border_width)) as f32;

        if border_pixels > 0.0 && center_pixels > 0.0 {
            let border_avg = edge_density_border / border_pixels;
            let center_avg = edge_density_center / center_pixels;

            // Frame within frame has strong edges at border
            if border_avg > center_avg * 1.5 {
                (border_avg / center_avg / 3.0).clamp(0.0, 1.0)
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Compute edge strength for a pixel.
    fn compute_pixel_edge_strength(
        &self,
        rgb_data: &[u8],
        width: usize,
        x: usize,
        y: usize,
    ) -> f32 {
        let idx = (y * width + x) * 3;
        if idx + width * 3 + 3 < rgb_data.len() && x + 1 < width {
            let mut edge = 0.0;
            for c in 0..3 {
                edge += ((rgb_data[idx + c] as i32 - rgb_data[idx + 3 + c] as i32).abs()
                    + (rgb_data[idx + c] as i32 - rgb_data[idx + width * 3 + c] as i32).abs())
                    as f32;
            }
            edge / 6.0 / 255.0
        } else {
            0.0
        }
    }
}

impl Default for CompositionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composition_analyzer() {
        let analyzer = CompositionAnalyzer::new();
        let width = 320;
        let height = 240;
        let rgb_data = vec![128u8; width * height * 3];

        let result = analyzer.analyze(&rgb_data, width, height);
        assert!(result.is_ok());

        let score = result.expect("should succeed in test");
        assert!(score.overall_score >= 0.0 && score.overall_score <= 1.0);
    }
}
