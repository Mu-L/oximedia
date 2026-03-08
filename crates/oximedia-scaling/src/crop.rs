#![allow(dead_code)]
//! Crop rectangle and crop operation helpers.

/// A rectangular crop region within a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropRect {
    /// Left offset in pixels.
    pub x: u32,
    /// Top offset in pixels.
    pub y: u32,
    /// Crop width in pixels.
    pub w: u32,
    /// Crop height in pixels.
    pub h: u32,
}

impl CropRect {
    /// Create a new [`CropRect`].
    pub fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// Width of the crop rectangle.
    pub fn width(&self) -> u32 {
        self.w
    }

    /// Height of the crop rectangle.
    pub fn height(&self) -> u32 {
        self.h
    }

    /// Area of the crop rectangle in pixels.
    pub fn area(&self) -> u64 {
        self.w as u64 * self.h as u64
    }

    /// Whether this crop rectangle has non-zero dimensions.
    pub fn is_valid(&self) -> bool {
        self.w > 0 && self.h > 0
    }

    /// Whether this crop fits inside a frame of the given dimensions.
    pub fn fits_in(&self, frame_w: u32, frame_h: u32) -> bool {
        self.x + self.w <= frame_w && self.y + self.h <= frame_h
    }
}

/// Strategy for choosing the crop region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropMode {
    /// Crop coordinates are specified explicitly.
    Manual,
    /// Centre the crop rectangle in the source frame.
    Centered,
    /// Crop to fit a target aspect ratio (width / height).
    AspectRatio,
    /// Smart crop: use saliency / subject detection (placeholder).
    Smart,
}

impl CropMode {
    /// Whether this mode preserves the aspect ratio of the crop region.
    pub fn preserves_aspect_ratio(&self) -> bool {
        matches!(self, CropMode::AspectRatio | CropMode::Smart)
    }
}

/// Applies a crop to a source frame and reports output dimensions.
#[derive(Debug, Clone)]
pub struct CropOperation {
    /// The crop region to apply.
    pub rect: CropRect,
    /// The crop selection strategy.
    pub mode: CropMode,
}

impl CropOperation {
    /// Create a new [`CropOperation`].
    pub fn new(rect: CropRect, mode: CropMode) -> Self {
        Self { rect, mode }
    }

    /// Compute output dimensions after applying this crop to a source frame.
    ///
    /// Returns `None` if the crop rect does not fit within `(src_w, src_h)`.
    pub fn apply_to_dimensions(&self, src_w: u32, src_h: u32) -> Option<(u32, u32)> {
        if !self.rect.fits_in(src_w, src_h) || !self.rect.is_valid() {
            return None;
        }
        Some((self.rect.w, self.rect.h))
    }

    /// Build a centred crop rect for the given source and target dimensions.
    pub fn centered(src_w: u32, src_h: u32, target_w: u32, target_h: u32) -> Option<CropRect> {
        if target_w > src_w || target_h > src_h {
            return None;
        }
        let x = (src_w - target_w) / 2;
        let y = (src_h - target_h) / 2;
        Some(CropRect::new(x, y, target_w, target_h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crop_rect_dimensions() {
        let r = CropRect::new(0, 0, 1920, 1080);
        assert_eq!(r.width(), 1920);
        assert_eq!(r.height(), 1080);
    }

    #[test]
    fn test_crop_rect_area() {
        let r = CropRect::new(0, 0, 100, 50);
        assert_eq!(r.area(), 5000);
    }

    #[test]
    fn test_crop_rect_is_valid_true() {
        let r = CropRect::new(10, 10, 100, 100);
        assert!(r.is_valid());
    }

    #[test]
    fn test_crop_rect_is_valid_zero_width() {
        let r = CropRect::new(0, 0, 0, 100);
        assert!(!r.is_valid());
    }

    #[test]
    fn test_crop_rect_is_valid_zero_height() {
        let r = CropRect::new(0, 0, 100, 0);
        assert!(!r.is_valid());
    }

    #[test]
    fn test_fits_in_true() {
        let r = CropRect::new(0, 0, 1920, 1080);
        assert!(r.fits_in(1920, 1080));
        assert!(r.fits_in(3840, 2160));
    }

    #[test]
    fn test_fits_in_false() {
        let r = CropRect::new(100, 0, 1920, 1080);
        // x + w = 2020 > 1920
        assert!(!r.fits_in(1920, 1080));
    }

    #[test]
    fn test_crop_mode_preserves_aspect_ratio() {
        assert!(CropMode::AspectRatio.preserves_aspect_ratio());
        assert!(CropMode::Smart.preserves_aspect_ratio());
        assert!(!CropMode::Manual.preserves_aspect_ratio());
        assert!(!CropMode::Centered.preserves_aspect_ratio());
    }

    #[test]
    fn test_apply_to_dimensions_valid() {
        let rect = CropRect::new(0, 0, 640, 480);
        let op = CropOperation::new(rect, CropMode::Manual);
        assert_eq!(op.apply_to_dimensions(1920, 1080), Some((640, 480)));
    }

    #[test]
    fn test_apply_to_dimensions_out_of_bounds() {
        let rect = CropRect::new(0, 0, 2000, 1080);
        let op = CropOperation::new(rect, CropMode::Manual);
        assert!(op.apply_to_dimensions(1920, 1080).is_none());
    }

    #[test]
    fn test_centered_crop() {
        let rect = CropOperation::centered(1920, 1080, 1280, 720).expect("should succeed in test");
        assert_eq!(rect.x, 320);
        assert_eq!(rect.y, 180);
        assert_eq!(rect.w, 1280);
        assert_eq!(rect.h, 720);
    }

    #[test]
    fn test_centered_crop_too_large() {
        assert!(CropOperation::centered(1280, 720, 1920, 1080).is_none());
    }
}
