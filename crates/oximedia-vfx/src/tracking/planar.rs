//! Planar tracking for perspective-corrected tracking.

use super::point::{PointTracker, TrackPoint, TrackingConfig};
use crate::{Frame, VfxError, VfxResult};
use serde::{Deserialize, Serialize};

/// A corner of a planar surface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Corner {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

impl From<TrackPoint> for Corner {
    fn from(point: TrackPoint) -> Self {
        Self {
            x: point.x,
            y: point.y,
        }
    }
}

/// Planar tracking data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanarData {
    /// Top-left corner.
    pub top_left: Corner,
    /// Top-right corner.
    pub top_right: Corner,
    /// Bottom-right corner.
    pub bottom_right: Corner,
    /// Bottom-left corner.
    pub bottom_left: Corner,
    /// Average confidence.
    pub confidence: f32,
    /// Whether all corners tracked successfully.
    pub success: bool,
}

impl PlanarData {
    /// Calculate 3x3 homography matrix from this planar data to reference.
    #[must_use]
    pub fn calculate_homography(&self, reference: &Self) -> [[f32; 3]; 3] {
        // Simplified homography - in production would use DLT algorithm
        let _src = [
            (self.top_left.x, self.top_left.y),
            (self.top_right.x, self.top_right.y),
            (self.bottom_right.x, self.bottom_right.y),
            (self.bottom_left.x, self.bottom_left.y),
        ];

        let _dst = [
            (reference.top_left.x, reference.top_left.y),
            (reference.top_right.x, reference.top_right.y),
            (reference.bottom_right.x, reference.bottom_right.y),
            (reference.bottom_left.x, reference.bottom_left.y),
        ];

        // Simplified identity matrix as placeholder
        // Real implementation would use solve_homography()
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    }

    /// Get center point of planar surface.
    #[must_use]
    pub fn center(&self) -> Corner {
        Corner {
            x: (self.top_left.x + self.top_right.x + self.bottom_right.x + self.bottom_left.x)
                / 4.0,
            y: (self.top_left.y + self.top_right.y + self.bottom_right.y + self.bottom_left.y)
                / 4.0,
        }
    }
}

/// Four-point planar tracker for perspective tracking.
#[derive(Debug, Clone)]
pub struct PlanarTracker {
    tracker_tl: PointTracker,
    tracker_tr: PointTracker,
    tracker_br: PointTracker,
    tracker_bl: PointTracker,
    reference_data: Option<PlanarData>,
}

impl PlanarTracker {
    /// Create a new planar tracker.
    #[must_use]
    pub fn new(config: TrackingConfig) -> Self {
        Self {
            tracker_tl: PointTracker::new(config),
            tracker_tr: PointTracker::new(config),
            tracker_br: PointTracker::new(config),
            tracker_bl: PointTracker::new(config),
            reference_data: None,
        }
    }

    /// Initialize with four corner points.
    pub fn initialize(&mut self, frame: &Frame, corners: [Corner; 4]) -> VfxResult<()> {
        let [tl, tr, br, bl] = corners;

        self.tracker_tl.initialize(
            frame,
            TrackPoint {
                x: tl.x,
                y: tl.y,
                confidence: 1.0,
            },
        )?;
        self.tracker_tr.initialize(
            frame,
            TrackPoint {
                x: tr.x,
                y: tr.y,
                confidence: 1.0,
            },
        )?;
        self.tracker_br.initialize(
            frame,
            TrackPoint {
                x: br.x,
                y: br.y,
                confidence: 1.0,
            },
        )?;
        self.tracker_bl.initialize(
            frame,
            TrackPoint {
                x: bl.x,
                y: bl.y,
                confidence: 1.0,
            },
        )?;

        self.reference_data = Some(PlanarData {
            top_left: tl,
            top_right: tr,
            bottom_right: br,
            bottom_left: bl,
            confidence: 1.0,
            success: true,
        });

        Ok(())
    }

    /// Track planar surface in new frame.
    pub fn track(&self, frame: &Frame) -> VfxResult<PlanarData> {
        let result_tl = self.tracker_tl.track(frame)?;
        let result_tr = self.tracker_tr.track(frame)?;
        let result_br = self.tracker_br.track(frame)?;
        let result_bl = self.tracker_bl.track(frame)?;

        let confidence = (result_tl.point.confidence
            + result_tr.point.confidence
            + result_br.point.confidence
            + result_bl.point.confidence)
            / 4.0;

        let success =
            result_tl.success && result_tr.success && result_br.success && result_bl.success;

        Ok(PlanarData {
            top_left: result_tl.point.into(),
            top_right: result_tr.point.into(),
            bottom_right: result_br.point.into(),
            bottom_left: result_bl.point.into(),
            confidence,
            success,
        })
    }

    /// Warp frame using planar tracking data.
    pub fn warp_to_reference(
        &self,
        input: &Frame,
        tracking: &PlanarData,
        output: &mut Frame,
    ) -> VfxResult<()> {
        let reference = self
            .reference_data
            .as_ref()
            .ok_or_else(|| VfxError::ProcessingError("Tracker not initialized".to_string()))?;

        let homography = tracking.calculate_homography(reference);

        // Apply homography warp
        for y in 0..output.height {
            for x in 0..output.width {
                let (src_x, src_y) = self.apply_homography(&homography, x as f32, y as f32);

                let pixel = if src_x >= 0.0
                    && src_x < input.width as f32
                    && src_y >= 0.0
                    && src_y < input.height as f32
                {
                    self.bilinear_sample(input, src_x, src_y)
                } else {
                    [0, 0, 0, 0]
                };

                output.set_pixel(x, y, pixel);
            }
        }

        Ok(())
    }

    fn apply_homography(&self, h: &[[f32; 3]; 3], x: f32, y: f32) -> (f32, f32) {
        let w = h[2][0] * x + h[2][1] * y + h[2][2];
        if w.abs() < 1e-6 {
            return (x, y);
        }
        let x_out = (h[0][0] * x + h[0][1] * y + h[0][2]) / w;
        let y_out = (h[1][0] * x + h[1][1] * y + h[1][2]) / w;
        (x_out, y_out)
    }

    fn bilinear_sample(&self, frame: &Frame, x: f32, y: f32) -> [u8; 4] {
        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(frame.width - 1);
        let y1 = (y0 + 1).min(frame.height - 1);

        let fx = x - x0 as f32;
        let fy = y - y0 as f32;

        let p00 = frame.get_pixel(x0, y0).unwrap_or([0, 0, 0, 0]);
        let p10 = frame.get_pixel(x1, y0).unwrap_or([0, 0, 0, 0]);
        let p01 = frame.get_pixel(x0, y1).unwrap_or([0, 0, 0, 0]);
        let p11 = frame.get_pixel(x1, y1).unwrap_or([0, 0, 0, 0]);

        let mut result = [0u8; 4];
        for i in 0..4 {
            let v0 = f32::from(p00[i]) * (1.0 - fx) + f32::from(p10[i]) * fx;
            let v1 = f32::from(p01[i]) * (1.0 - fx) + f32::from(p11[i]) * fx;
            result[i] = (v0 * (1.0 - fy) + v1 * fy) as u8;
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planar_data_center() {
        let data = PlanarData {
            top_left: Corner { x: 0.0, y: 0.0 },
            top_right: Corner { x: 100.0, y: 0.0 },
            bottom_right: Corner { x: 100.0, y: 100.0 },
            bottom_left: Corner { x: 0.0, y: 100.0 },
            confidence: 1.0,
            success: true,
        };

        let center = data.center();
        assert_eq!(center.x, 50.0);
        assert_eq!(center.y, 50.0);
    }

    #[test]
    fn test_planar_tracker_initialization() -> VfxResult<()> {
        let frame = Frame::new(200, 200)?;
        let mut tracker = PlanarTracker::new(TrackingConfig::default());

        let corners = [
            Corner { x: 50.0, y: 50.0 },
            Corner { x: 150.0, y: 50.0 },
            Corner { x: 150.0, y: 150.0 },
            Corner { x: 50.0, y: 150.0 },
        ];

        tracker.initialize(&frame, corners)?;
        Ok(())
    }
}
