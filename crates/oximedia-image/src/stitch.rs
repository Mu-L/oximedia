//! Panorama image stitching support.
//!
//! Provides data structures for describing image patches, computing their
//! overlap regions, and generating blend weights for seamless panorama output.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

/// Describes a single image patch placed on a panorama canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct ImagePatch {
    /// Unique identifier.
    pub id: u32,
    /// Horizontal offset on the canvas (can be negative for cropped sources).
    pub x_offset: i32,
    /// Vertical offset on the canvas.
    pub y_offset: i32,
    /// Width of the patch in pixels.
    pub width: u32,
    /// Height of the patch in pixels.
    pub height: u32,
    /// Rotation in radians (positive = counter-clockwise).
    pub rotation: f64,
}

impl ImagePatch {
    /// Creates a new `ImagePatch`.
    #[must_use]
    pub fn new(id: u32, x_offset: i32, y_offset: i32, width: u32, height: u32) -> Self {
        Self {
            id,
            x_offset,
            y_offset,
            width,
            height,
            rotation: 0.0,
        }
    }

    /// Returns the area of this patch in pixels.
    #[must_use]
    pub fn area(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// Returns the right-edge x coordinate (exclusive).
    #[must_use]
    pub fn x_end(&self) -> i32 {
        self.x_offset + self.width as i32
    }

    /// Returns the bottom-edge y coordinate (exclusive).
    #[must_use]
    pub fn y_end(&self) -> i32 {
        self.y_offset + self.height as i32
    }

    /// Computes the overlap rectangle with another patch.
    ///
    /// Returns `Some((x, y, w, h))` where (x, y) is the top-left corner of the
    /// overlap and (w, h) are its dimensions, or `None` if there is no overlap.
    #[must_use]
    pub fn overlap_with(&self, other: &ImagePatch) -> Option<(i32, i32, u32, u32)> {
        find_overlap_region(self, other)
    }
}

/// Configuration for the panorama stitcher.
#[derive(Debug, Clone)]
pub struct StitchConfig {
    /// Width of the feathering blend zone at patch boundaries (pixels).
    pub blend_width: u32,
    /// Apply warp/lens-distortion correction before blending.
    pub warp_correction: bool,
    /// Blend exposure levels between patches.
    pub exposure_blend: bool,
}

impl Default for StitchConfig {
    fn default() -> Self {
        Self {
            blend_width: 64,
            warp_correction: false,
            exposure_blend: true,
        }
    }
}

/// Computes per-pixel linear blend weights across an overlap zone.
///
/// Returns `overlap_pixels` weights linearly ramping from `0.0` to `1.0`.
/// If `overlap_pixels == 0`, returns an empty vector.
/// The `blend_width` parameter limits how quickly the weights saturate; if
/// `overlap_pixels > blend_width` the outer pixels receive full weight 1.0.
#[must_use]
pub fn compute_overlap_blend_weights(overlap_pixels: u32, blend_width: u32) -> Vec<f64> {
    if overlap_pixels == 0 {
        return Vec::new();
    }
    let effective = blend_width.min(overlap_pixels);
    let mut weights = Vec::with_capacity(overlap_pixels as usize);

    for i in 0..overlap_pixels {
        let w = if effective == 0 {
            1.0
        } else if i < effective {
            i as f64 / effective as f64
        } else {
            1.0
        };
        weights.push(w);
    }
    weights
}

/// Computes the axis-aligned overlap rectangle between two patches.
///
/// Returns `Some((x, y, w, h))` or `None` if there is no overlap.
#[must_use]
pub fn find_overlap_region(a: &ImagePatch, b: &ImagePatch) -> Option<(i32, i32, u32, u32)> {
    let x_start = a.x_offset.max(b.x_offset);
    let y_start = a.y_offset.max(b.y_offset);
    let x_end = a.x_end().min(b.x_end());
    let y_end = a.y_end().min(b.y_end());

    if x_end > x_start && y_end > y_start {
        Some((
            x_start,
            y_start,
            (x_end - x_start) as u32,
            (y_end - y_start) as u32,
        ))
    } else {
        None
    }
}

/// Builds a panorama from a collection of `ImagePatch` objects.
pub struct PanoramaBuilder {
    /// All registered patches.
    pub patches: Vec<ImagePatch>,
    /// Desired output canvas width.
    pub output_width: u32,
    /// Desired output canvas height.
    pub output_height: u32,
}

impl PanoramaBuilder {
    /// Creates a new `PanoramaBuilder` with a fixed output canvas size.
    #[must_use]
    pub fn new(output_w: u32, output_h: u32) -> Self {
        Self {
            patches: Vec::new(),
            output_width: output_w,
            output_height: output_h,
        }
    }

    /// Registers a patch with the builder.
    pub fn add_patch(&mut self, patch: ImagePatch) {
        self.patches.push(patch);
    }

    /// Computes the tightest bounding box that contains all registered patches.
    ///
    /// Returns `(width, height)` of the canvas needed.
    /// Returns `(0, 0)` when no patches are registered.
    #[must_use]
    pub fn compute_canvas_size(&self) -> (u32, u32) {
        if self.patches.is_empty() {
            return (0, 0);
        }
        let min_x = self.patches.iter().map(|p| p.x_offset).min().unwrap_or(0);
        let min_y = self.patches.iter().map(|p| p.y_offset).min().unwrap_or(0);
        let max_x = self.patches.iter().map(|p| p.x_end()).max().unwrap_or(0);
        let max_y = self.patches.iter().map(|p| p.y_end()).max().unwrap_or(0);

        let w = (max_x - min_x).max(0) as u32;
        let h = (max_y - min_y).max(0) as u32;
        (w, h)
    }

    /// Returns patch IDs in back-to-front render order (leftmost first).
    #[must_use]
    pub fn patch_order(&self) -> Vec<u32> {
        let mut indexed: Vec<(i32, u32)> =
            self.patches.iter().map(|p| (p.x_offset, p.id)).collect();
        indexed.sort_by_key(|&(x, _)| x);
        indexed.into_iter().map(|(_, id)| id).collect()
    }

    /// Returns a reference to the patch with the given id, if it exists.
    #[must_use]
    pub fn get_patch(&self, id: u32) -> Option<&ImagePatch> {
        self.patches.iter().find(|p| p.id == id)
    }

    /// Returns the number of patches registered.
    #[must_use]
    pub fn patch_count(&self) -> usize {
        self.patches.len()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_patch(id: u32, x: i32, y: i32, w: u32, h: u32) -> ImagePatch {
        ImagePatch::new(id, x, y, w, h)
    }

    #[test]
    fn test_patch_area() {
        let p = make_patch(1, 0, 0, 100, 200);
        assert_eq!(p.area(), 20_000);
    }

    #[test]
    fn test_patch_x_end() {
        let p = make_patch(1, 50, 0, 200, 100);
        assert_eq!(p.x_end(), 250);
    }

    #[test]
    fn test_patch_y_end() {
        let p = make_patch(1, 0, 30, 100, 70);
        assert_eq!(p.y_end(), 100);
    }

    #[test]
    fn test_overlap_region_adjacent_no_overlap() {
        let a = make_patch(1, 0, 0, 100, 100);
        let b = make_patch(2, 100, 0, 100, 100);
        assert!(find_overlap_region(&a, &b).is_none());
    }

    #[test]
    fn test_overlap_region_partial() {
        let a = make_patch(1, 0, 0, 150, 100);
        let b = make_patch(2, 100, 0, 150, 100);
        let overlap = find_overlap_region(&a, &b);
        assert_eq!(overlap, Some((100, 0, 50, 100)));
    }

    #[test]
    fn test_overlap_region_full_containment() {
        let outer = make_patch(1, 0, 0, 200, 200);
        let inner = make_patch(2, 50, 50, 50, 50);
        let overlap = find_overlap_region(&outer, &inner);
        assert_eq!(overlap, Some((50, 50, 50, 50)));
    }

    #[test]
    fn test_patch_overlap_method_delegates() {
        let a = make_patch(1, 0, 0, 150, 100);
        let b = make_patch(2, 100, 0, 150, 100);
        assert_eq!(a.overlap_with(&b), find_overlap_region(&a, &b));
    }

    #[test]
    fn test_blend_weights_length() {
        let w = compute_overlap_blend_weights(50, 64);
        assert_eq!(w.len(), 50);
    }

    #[test]
    fn test_blend_weights_ramp() {
        let w = compute_overlap_blend_weights(10, 10);
        // First weight should be near 0, last near 1
        assert!(w[0] < 0.2);
        assert!(w[9] > 0.8);
    }

    #[test]
    fn test_blend_weights_empty() {
        let w = compute_overlap_blend_weights(0, 64);
        assert!(w.is_empty());
    }

    #[test]
    fn test_panorama_builder_canvas_size() {
        let mut builder = PanoramaBuilder::new(1920, 1080);
        builder.add_patch(make_patch(1, 0, 0, 1000, 800));
        builder.add_patch(make_patch(2, 800, 0, 1000, 800));
        let (w, h) = builder.compute_canvas_size();
        assert_eq!(w, 1800);
        assert_eq!(h, 800);
    }

    #[test]
    fn test_panorama_builder_patch_order() {
        let mut builder = PanoramaBuilder::new(3000, 1000);
        builder.add_patch(make_patch(10, 500, 0, 500, 500));
        builder.add_patch(make_patch(20, 0, 0, 500, 500));
        builder.add_patch(make_patch(30, 1000, 0, 500, 500));
        let order = builder.patch_order();
        assert_eq!(order, vec![20, 10, 30]);
    }

    #[test]
    fn test_panorama_builder_empty_canvas() {
        let builder = PanoramaBuilder::new(1920, 1080);
        assert_eq!(builder.compute_canvas_size(), (0, 0));
    }

    #[test]
    fn test_panorama_builder_patch_count() {
        let mut builder = PanoramaBuilder::new(1920, 1080);
        builder.add_patch(make_patch(1, 0, 0, 100, 100));
        builder.add_patch(make_patch(2, 100, 0, 100, 100));
        assert_eq!(builder.patch_count(), 2);
    }
}
