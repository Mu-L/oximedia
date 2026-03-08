//! Single-point and multi-point 2D tracking.

use crate::{Frame, VfxError, VfxResult};
use serde::{Deserialize, Serialize};

/// A tracked point in 2D space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackPoint {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// Tracking confidence (0.0 - 1.0).
    pub confidence: f32,
}

/// Result of tracking operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackingResult {
    /// Tracked point.
    pub point: TrackPoint,
    /// Whether tracking was successful.
    pub success: bool,
    /// Search offset applied.
    pub offset_x: f32,
    /// Search offset applied.
    pub offset_y: f32,
}

/// Configuration for point tracking.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackingConfig {
    /// Template size for matching.
    pub template_size: u32,
    /// Search area size.
    pub search_size: u32,
    /// Minimum confidence threshold.
    pub min_confidence: f32,
    /// Subpixel accuracy.
    pub subpixel: bool,
}

impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            template_size: 31,
            search_size: 61,
            min_confidence: 0.7,
            subpixel: true,
        }
    }
}

/// Single-point tracker using template matching.
#[derive(Debug, Clone)]
pub struct PointTracker {
    config: TrackingConfig,
    template: Option<Vec<u8>>,
    template_point: Option<TrackPoint>,
}

impl PointTracker {
    /// Create a new point tracker.
    #[must_use]
    pub const fn new(config: TrackingConfig) -> Self {
        Self {
            config,
            template: None,
            template_point: None,
        }
    }

    /// Initialize tracking with a point in the reference frame.
    pub fn initialize(&mut self, frame: &Frame, point: TrackPoint) -> VfxResult<()> {
        // Extract template around point
        let template_size = self.config.template_size as i32;
        let half_size = template_size / 2;

        let x = point.x as i32;
        let y = point.y as i32;

        // Validate bounds
        if x - half_size < 0
            || x + half_size >= frame.width as i32
            || y - half_size < 0
            || y + half_size >= frame.height as i32
        {
            return Err(VfxError::ProcessingError(
                "Point too close to frame edge".to_string(),
            ));
        }

        // Extract template
        let mut template = Vec::with_capacity((template_size * template_size * 4) as usize);
        for ty in (y - half_size)..=(y + half_size) {
            for tx in (x - half_size)..=(x + half_size) {
                if let Some(pixel) = frame.get_pixel(tx as u32, ty as u32) {
                    template.extend_from_slice(&pixel);
                }
            }
        }

        self.template = Some(template);
        self.template_point = Some(point);
        Ok(())
    }

    /// Track point in new frame.
    pub fn track(&self, frame: &Frame) -> VfxResult<TrackingResult> {
        let template = self
            .template
            .as_ref()
            .ok_or_else(|| VfxError::ProcessingError("Tracker not initialized".to_string()))?;
        let template_point = self
            .template_point
            .ok_or_else(|| VfxError::ProcessingError("Tracker not initialized".to_string()))?;

        let template_size = self.config.template_size as i32;
        let search_size = self.config.search_size as i32;
        let half_search = search_size / 2;

        let center_x = template_point.x as i32;
        let center_y = template_point.y as i32;

        // Search for best match
        let mut best_score = f32::NEG_INFINITY;
        let mut best_x = center_x;
        let mut best_y = center_y;

        for sy in (center_y - half_search)..=(center_y + half_search) {
            for sx in (center_x - half_search)..=(center_x + half_search) {
                if !self.is_valid_search_pos(frame, sx, sy, template_size) {
                    continue;
                }

                let score = self.compute_ncc(frame, sx, sy, template)?;
                if score > best_score {
                    best_score = score;
                    best_x = sx;
                    best_y = sy;
                }
            }
        }

        // Subpixel refinement
        let (final_x, final_y) = if self.config.subpixel && best_score > 0.0 {
            self.refine_subpixel(frame, best_x, best_y, template)?
        } else {
            (best_x as f32, best_y as f32)
        };

        let confidence = (best_score + 1.0) / 2.0; // Map from [-1, 1] to [0, 1]
        let success = confidence >= self.config.min_confidence;

        Ok(TrackingResult {
            point: TrackPoint {
                x: final_x,
                y: final_y,
                confidence,
            },
            success,
            offset_x: final_x - template_point.x,
            offset_y: final_y - template_point.y,
        })
    }

    /// Update template with new frame (for long-term tracking).
    pub fn update_template(&mut self, frame: &Frame, point: TrackPoint) -> VfxResult<()> {
        self.initialize(frame, point)
    }

    fn is_valid_search_pos(&self, frame: &Frame, x: i32, y: i32, template_size: i32) -> bool {
        let half_size = template_size / 2;
        x - half_size >= 0
            && x + half_size < frame.width as i32
            && y - half_size >= 0
            && y + half_size < frame.height as i32
    }

    /// Compute normalized cross-correlation.
    fn compute_ncc(&self, frame: &Frame, x: i32, y: i32, template: &[u8]) -> VfxResult<f32> {
        let template_size = self.config.template_size as i32;
        let half_size = template_size / 2;

        let mut sum_template = 0.0;
        let mut sum_image = 0.0;
        let mut sum_sq_template = 0.0;
        let mut sum_sq_image = 0.0;
        let mut sum_product = 0.0;
        let mut count = 0;

        for ty in 0..template_size {
            for tx in 0..template_size {
                let fx = x + tx - half_size;
                let fy = y + ty - half_size;

                if let Some(pixel) = frame.get_pixel(fx as u32, fy as u32) {
                    let tidx = ((ty * template_size + tx) * 4) as usize;
                    if tidx + 3 < template.len() {
                        // Use grayscale values
                        let template_val = (f32::from(template[tidx]) * 0.299
                            + f32::from(template[tidx + 1]) * 0.587
                            + f32::from(template[tidx + 2]) * 0.114)
                            / 255.0;
                        let image_val = (f32::from(pixel[0]) * 0.299
                            + f32::from(pixel[1]) * 0.587
                            + f32::from(pixel[2]) * 0.114)
                            / 255.0;

                        sum_template += template_val;
                        sum_image += image_val;
                        sum_sq_template += template_val * template_val;
                        sum_sq_image += image_val * image_val;
                        sum_product += template_val * image_val;
                        count += 1;
                    }
                }
            }
        }

        if count == 0 {
            return Ok(0.0);
        }

        let n = count as f32;
        let numerator = sum_product - (sum_template * sum_image) / n;
        let denominator = ((sum_sq_template - sum_template * sum_template / n)
            * (sum_sq_image - sum_image * sum_image / n))
            .sqrt();

        if denominator > 0.0 {
            Ok(numerator / denominator)
        } else {
            Ok(0.0)
        }
    }

    /// Refine position to subpixel accuracy using parabolic interpolation.
    fn refine_subpixel(
        &self,
        frame: &Frame,
        x: i32,
        y: i32,
        template: &[u8],
    ) -> VfxResult<(f32, f32)> {
        // Sample 3x3 neighborhood
        let c = self.compute_ncc(frame, x, y, template)?;
        let l = if x > 0 {
            self.compute_ncc(frame, x - 1, y, template)?
        } else {
            0.0
        };
        let r = if x < frame.width as i32 - 1 {
            self.compute_ncc(frame, x + 1, y, template)?
        } else {
            0.0
        };
        let t = if y > 0 {
            self.compute_ncc(frame, x, y - 1, template)?
        } else {
            0.0
        };
        let b = if y < frame.height as i32 - 1 {
            self.compute_ncc(frame, x, y + 1, template)?
        } else {
            0.0
        };

        // Parabolic interpolation
        let dx = if (l + r - 2.0 * c).abs() > 1e-6 {
            (l - r) / (2.0 * (l + r - 2.0 * c))
        } else {
            0.0
        };

        let dy = if (t + b - 2.0 * c).abs() > 1e-6 {
            (t - b) / (2.0 * (t + b - 2.0 * c))
        } else {
            0.0
        };

        Ok((
            x as f32 + dx.clamp(-0.5, 0.5),
            y as f32 + dy.clamp(-0.5, 0.5),
        ))
    }
}

/// Two-point tracker for position and rotation.
#[derive(Debug, Clone)]
pub struct TwoPointTracker {
    tracker1: PointTracker,
    tracker2: PointTracker,
}

impl TwoPointTracker {
    /// Create a new two-point tracker.
    #[must_use]
    pub fn new(config: TrackingConfig) -> Self {
        Self {
            tracker1: PointTracker::new(config),
            tracker2: PointTracker::new(config),
        }
    }

    /// Initialize with two points.
    pub fn initialize(
        &mut self,
        frame: &Frame,
        point1: TrackPoint,
        point2: TrackPoint,
    ) -> VfxResult<()> {
        self.tracker1.initialize(frame, point1)?;
        self.tracker2.initialize(frame, point2)?;
        Ok(())
    }

    /// Track both points.
    pub fn track(&self, frame: &Frame) -> VfxResult<(TrackingResult, TrackingResult)> {
        let result1 = self.tracker1.track(frame)?;
        let result2 = self.tracker2.track(frame)?;
        Ok((result1, result2))
    }

    /// Calculate rotation between tracked points.
    #[must_use]
    pub fn calculate_rotation(&self, result1: &TrackingResult, result2: &TrackingResult) -> f32 {
        let dx = result2.point.x - result1.point.x;
        let dy = result2.point.y - result1.point.y;
        dy.atan2(dx).to_degrees()
    }

    /// Calculate scale between tracked points.
    #[must_use]
    pub fn calculate_scale(
        &self,
        result1: &TrackingResult,
        result2: &TrackingResult,
        initial_distance: f32,
    ) -> f32 {
        let dx = result2.point.x - result1.point.x;
        let dy = result2.point.y - result1.point.y;
        let current_distance = (dx * dx + dy * dy).sqrt();
        current_distance / initial_distance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracking_config_default() {
        let config = TrackingConfig::default();
        assert_eq!(config.template_size, 31);
        assert_eq!(config.search_size, 61);
        assert!(config.subpixel);
    }

    #[test]
    fn test_point_tracker_initialization() -> VfxResult<()> {
        let frame = Frame::new(100, 100)?;
        let mut tracker = PointTracker::new(TrackingConfig::default());
        let point = TrackPoint {
            x: 50.0,
            y: 50.0,
            confidence: 1.0,
        };
        tracker.initialize(&frame, point)?;
        Ok(())
    }

    #[test]
    fn test_rotation_calculation() {
        let tracker = TwoPointTracker::new(TrackingConfig::default());
        let result1 = TrackingResult {
            point: TrackPoint {
                x: 0.0,
                y: 0.0,
                confidence: 1.0,
            },
            success: true,
            offset_x: 0.0,
            offset_y: 0.0,
        };
        let result2 = TrackingResult {
            point: TrackPoint {
                x: 1.0,
                y: 0.0,
                confidence: 1.0,
            },
            success: true,
            offset_x: 0.0,
            offset_y: 0.0,
        };
        let rotation = tracker.calculate_rotation(&result1, &result2);
        assert!((rotation - 0.0).abs() < 0.01);
    }
}
