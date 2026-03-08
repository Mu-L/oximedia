//! Mesh-based warping for rolling shutter correction and local distortion removal.
//!
//! A `WarpMesh` is a regular grid of control points; each point stores a
//! displacement `(dx, dy)` that is applied to pixels in its cell via bilinear
//! interpolation.

#![allow(dead_code)]

/// A single control point in the warp mesh.
///
/// `(u, v)` are the normalised [0, 1] grid coordinates of this point.
/// `(dx, dy)` are the displacement values in pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshPoint {
    /// Normalised horizontal grid coordinate in [0, 1].
    pub u: f64,
    /// Normalised vertical grid coordinate in [0, 1].
    pub v: f64,
    /// Horizontal displacement in pixels.
    pub dx: f64,
    /// Vertical displacement in pixels.
    pub dy: f64,
}

impl MeshPoint {
    /// Creates a new `MeshPoint` with zero displacement.
    #[must_use]
    pub const fn new(u: f64, v: f64) -> Self {
        Self {
            u,
            v,
            dx: 0.0,
            dy: 0.0,
        }
    }
}

/// A regular grid of warp control points.
///
/// The grid is laid out in row-major order: `points[row * cols + col]`.
/// `u` runs left-to-right (cols), `v` runs top-to-bottom (rows).
#[derive(Debug, Clone)]
pub struct WarpMesh {
    /// Control points in row-major order.
    pub points: Vec<MeshPoint>,
    /// Number of columns in the grid.
    pub cols: usize,
    /// Number of rows in the grid.
    pub rows: usize,
}

impl WarpMesh {
    /// Creates a new `WarpMesh` with `cols × rows` zero-displacement points.
    ///
    /// Coordinates are evenly spaced in `[0, 1]`.
    #[must_use]
    pub fn new(cols: usize, rows: usize) -> Self {
        let mut points = Vec::with_capacity(cols * rows);
        for row in 0..rows {
            for col in 0..cols {
                let u = if cols > 1 {
                    col as f64 / (cols - 1) as f64
                } else {
                    0.0
                };
                let v = if rows > 1 {
                    row as f64 / (rows - 1) as f64
                } else {
                    0.0
                };
                points.push(MeshPoint::new(u, v));
            }
        }
        Self { points, cols, rows }
    }

    /// Returns the flat index for `(col, row)`.
    fn idx(&self, col: usize, row: usize) -> usize {
        row * self.cols + col
    }

    /// Sets the displacement at grid position `(col, row)`.
    ///
    /// Does nothing if the indices are out of bounds.
    pub fn set_offset(&mut self, col: usize, row: usize, dx: f64, dy: f64) {
        if col < self.cols && row < self.rows {
            let i = self.idx(col, row);
            self.points[i].dx = dx;
            self.points[i].dy = dy;
        }
    }

    /// Returns the displacement at grid position `(col, row)`.
    ///
    /// Returns `(0.0, 0.0)` if the indices are out of bounds.
    #[must_use]
    pub fn get_offset(&self, col: usize, row: usize) -> (f64, f64) {
        if col < self.cols && row < self.rows {
            let p = &self.points[self.idx(col, row)];
            (p.dx, p.dy)
        } else {
            (0.0, 0.0)
        }
    }

    /// Interpolates the displacement at the normalised position `(u, v)` using
    /// bilinear interpolation over the four surrounding grid cells.
    ///
    /// `u` and `v` are clamped to `[0, 1]`.
    #[must_use]
    pub fn interpolate_at(&self, u: f64, v: f64) -> (f64, f64) {
        if self.cols == 0 || self.rows == 0 {
            return (0.0, 0.0);
        }

        let u = u.clamp(0.0, 1.0);
        let v = v.clamp(0.0, 1.0);

        if self.cols == 1 && self.rows == 1 {
            let p = &self.points[0];
            return (p.dx, p.dy);
        }

        // Map (u, v) to fractional grid coordinates.
        let gx = u * (self.cols.saturating_sub(1)) as f64;
        let gy = v * (self.rows.saturating_sub(1)) as f64;

        let col0 = (gx.floor() as usize).min(self.cols.saturating_sub(2));
        let row0 = (gy.floor() as usize).min(self.rows.saturating_sub(2));
        let col1 = (col0 + 1).min(self.cols - 1);
        let row1 = (row0 + 1).min(self.rows - 1);

        let tx = gx - col0 as f64;
        let ty = gy - row0 as f64;

        let p00 = &self.points[self.idx(col0, row0)];
        let p10 = &self.points[self.idx(col1, row0)];
        let p01 = &self.points[self.idx(col0, row1)];
        let p11 = &self.points[self.idx(col1, row1)];

        let dx = p00.dx * (1.0 - tx) * (1.0 - ty)
            + p10.dx * tx * (1.0 - ty)
            + p01.dx * (1.0 - tx) * ty
            + p11.dx * tx * ty;

        let dy = p00.dy * (1.0 - tx) * (1.0 - ty)
            + p10.dy * tx * (1.0 - ty)
            + p01.dy * (1.0 - tx) * ty
            + p11.dy * tx * ty;

        (dx, dy)
    }

    /// Applies a simple Gaussian-like smoothing to the mesh displacements
    /// using a box-filter approximation with radius `sigma` (in grid cells).
    ///
    /// Three passes of a box filter approximate a Gaussian.
    pub fn smooth(&mut self, sigma: f64) {
        let radius = (sigma * 2.0).round().max(1.0) as usize;
        // Three-pass box filter approximation of Gaussian.
        for _ in 0..3 {
            self.box_filter_pass(radius);
        }
    }

    /// Single box-filter pass over the displacement field.
    fn box_filter_pass(&mut self, radius: usize) {
        let cols = self.cols;
        let rows = self.rows;
        let mut smoothed_dx = vec![0.0f64; cols * rows];
        let mut smoothed_dy = vec![0.0f64; cols * rows];

        for row in 0..rows {
            for col in 0..cols {
                let r0 = row.saturating_sub(radius);
                let r1 = (row + radius + 1).min(rows);
                let c0 = col.saturating_sub(radius);
                let c1 = (col + radius + 1).min(cols);

                let mut sum_dx = 0.0f64;
                let mut sum_dy = 0.0f64;
                let mut count = 0usize;

                for r in r0..r1 {
                    for c in c0..c1 {
                        let p = &self.points[r * cols + c];
                        sum_dx += p.dx;
                        sum_dy += p.dy;
                        count += 1;
                    }
                }

                let n = count as f64;
                smoothed_dx[row * cols + col] = sum_dx / n;
                smoothed_dy[row * cols + col] = sum_dy / n;
            }
        }

        for (i, p) in self.points.iter_mut().enumerate() {
            p.dx = smoothed_dx[i];
            p.dy = smoothed_dy[i];
        }
    }
}

/// Applies rolling-shutter correction offsets to a `WarpMesh`.
///
/// Rolling shutter cameras expose successive rows at slightly different times,
/// causing skew when the camera moves during exposure.  This function adds a
/// vertical shear to the mesh proportional to:
///
/// * `readout_time_ms` – time in ms to read the entire frame.
/// * `motion_deg_per_s` – apparent angular velocity of the camera (deg/s).
///
/// The correction is applied as a horizontal displacement that varies linearly
/// with the vertical position in the frame.
pub fn rolling_shutter_correction(
    mesh: &mut WarpMesh,
    readout_time_ms: f64,
    motion_deg_per_s: f64,
) {
    if mesh.rows == 0 {
        return;
    }
    // Maximum horizontal skew at the bottom of the frame (in normalised units).
    // We use a simplified linear model: skew = readout_time_s * motion_rad_per_s.
    let readout_s = readout_time_ms * 1e-3;
    let motion_rad_per_s = motion_deg_per_s.to_radians();
    let max_skew = readout_s * motion_rad_per_s; // radians; we treat 1 rad ≈ 1 pixel-fraction

    for row in 0..mesh.rows {
        let v = if mesh.rows > 1 {
            row as f64 / (mesh.rows - 1) as f64
        } else {
            0.0
        };
        // Skew increases linearly from 0 at the top to max_skew at the bottom.
        let skew_dx = max_skew * v;
        for col in 0..mesh.cols {
            let i = row * mesh.cols + col;
            mesh.points[i].dx += skew_dx;
        }
    }
}

/// Applies a `WarpMesh` to a raw pixel buffer using bilinear sampling.
///
/// * `src` – Source pixel data (4 bytes per pixel, row-major).
/// * `dst` – Destination buffer (same size as source).
/// * `width`, `height` – Frame dimensions.
/// * `mesh` – The warp mesh to apply.
///
/// For each destination pixel, the mesh displacement is looked up and the
/// source pixel is sampled with bilinear interpolation.
pub fn bilinear_warp(src: &[u8], dst: &mut [u8], width: u32, height: u32, mesh: &WarpMesh) {
    let bpp = 4usize;
    let w = width as usize;
    let h = height as usize;

    for dy in 0..h {
        for dx in 0..w {
            let u = if w > 1 {
                dx as f64 / (w - 1) as f64
            } else {
                0.0
            };
            let v = if h > 1 {
                dy as f64 / (h - 1) as f64
            } else {
                0.0
            };

            let (offset_dx, offset_dy) = mesh.interpolate_at(u, v);

            // Source location (floating point).
            let sx_f = dx as f64 + offset_dx;
            let sy_f = dy as f64 + offset_dy;

            // Clamp to frame.
            let sx_f = sx_f.clamp(0.0, (w - 1) as f64);
            let sy_f = sy_f.clamp(0.0, (h - 1) as f64);

            let sx0 = sx_f.floor() as usize;
            let sy0 = sy_f.floor() as usize;
            let sx1 = (sx0 + 1).min(w - 1);
            let sy1 = (sy0 + 1).min(h - 1);

            let tx = sx_f - sx0 as f64;
            let ty = sy_f - sy0 as f64;

            let dst_idx = (dy * w + dx) * bpp;

            for c in 0..bpp {
                let p00 = src[(sy0 * w + sx0) * bpp + c] as f64;
                let p10 = src[(sy0 * w + sx1) * bpp + c] as f64;
                let p01 = src[(sy1 * w + sx0) * bpp + c] as f64;
                let p11 = src[(sy1 * w + sx1) * bpp + c] as f64;

                let val = p00 * (1.0 - tx) * (1.0 - ty)
                    + p10 * tx * (1.0 - ty)
                    + p01 * (1.0 - tx) * ty
                    + p11 * tx * ty;

                if dst_idx + c < dst.len() {
                    dst[dst_idx + c] = val.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_mesh_zero_displacement() {
        let mesh = WarpMesh::new(4, 4);
        for p in &mesh.points {
            assert!((p.dx).abs() < 1e-10);
            assert!((p.dy).abs() < 1e-10);
        }
    }

    #[test]
    fn test_new_mesh_dimensions() {
        let mesh = WarpMesh::new(5, 3);
        assert_eq!(mesh.cols, 5);
        assert_eq!(mesh.rows, 3);
        assert_eq!(mesh.points.len(), 15);
    }

    #[test]
    fn test_set_and_get_offset() {
        let mut mesh = WarpMesh::new(4, 4);
        mesh.set_offset(2, 1, 5.0, -3.0);
        let (dx, dy) = mesh.get_offset(2, 1);
        assert!((dx - 5.0).abs() < 1e-10);
        assert!((dy + 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_get_offset_out_of_bounds() {
        let mesh = WarpMesh::new(4, 4);
        let (dx, dy) = mesh.get_offset(10, 10);
        assert!((dx).abs() < 1e-10);
        assert!((dy).abs() < 1e-10);
    }

    #[test]
    fn test_set_offset_out_of_bounds_noop() {
        let mut mesh = WarpMesh::new(4, 4);
        mesh.set_offset(100, 100, 99.0, 99.0); // should not panic
                                               // Mesh unchanged
        for p in &mesh.points {
            assert!((p.dx).abs() < 1e-10);
        }
    }

    #[test]
    fn test_interpolate_at_corner() {
        let mut mesh = WarpMesh::new(2, 2);
        mesh.set_offset(0, 0, 10.0, 0.0);
        let (dx, _dy) = mesh.interpolate_at(0.0, 0.0);
        assert!((dx - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_interpolate_at_midpoint_uniform_field() {
        let mut mesh = WarpMesh::new(3, 3);
        // Set all offsets to 4.0
        for row in 0..3 {
            for col in 0..3 {
                mesh.set_offset(col, row, 4.0, 2.0);
            }
        }
        let (dx, dy) = mesh.interpolate_at(0.5, 0.5);
        assert!((dx - 4.0).abs() < 1e-10);
        assert!((dy - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_smooth_reduces_peak() {
        let mut mesh = WarpMesh::new(5, 5);
        // Set a spike in the centre
        mesh.set_offset(2, 2, 100.0, 100.0);
        mesh.smooth(1.0);
        let (dx, _) = mesh.get_offset(2, 2);
        assert!(dx < 100.0, "smoothing should reduce peak displacement");
    }

    #[test]
    fn test_rolling_shutter_correction_increases_dx_with_row() {
        let mut mesh = WarpMesh::new(4, 4);
        rolling_shutter_correction(&mut mesh, 16.0, 90.0);
        let (dx_top, _) = mesh.get_offset(0, 0);
        let (dx_bottom, _) = mesh.get_offset(0, 3);
        assert!(
            dx_bottom > dx_top,
            "correction must increase with row index"
        );
    }

    #[test]
    fn test_bilinear_warp_identity_mesh() {
        let width = 4u32;
        let height = 4u32;
        let src: Vec<u8> = (0..width * height * 4).map(|i| (i % 256) as u8).collect();
        let mesh = WarpMesh::new(4, 4); // all zeros → identity warp
        let mut dst = vec![0u8; src.len()];
        bilinear_warp(&src, &mut dst, width, height, &mesh);
        // With zero displacement the output should equal the input.
        assert_eq!(src, dst);
    }

    #[test]
    fn test_mesh_point_construction() {
        let p = MeshPoint::new(0.5, 0.25);
        assert!((p.u - 0.5).abs() < 1e-10);
        assert!((p.v - 0.25).abs() < 1e-10);
        assert!((p.dx).abs() < 1e-10);
        assert!((p.dy).abs() < 1e-10);
    }
}
