//! YUV colour-space conversion helpers.
//!
//! Implements BT.601 YUV ↔ RGB conversions and a simple `YuvPlane` container.

#![allow(dead_code)]

/// Convert a single YUV sample (BT.601) to RGB.
///
/// All inputs are the raw byte values as stored in YUV 4:2:0/4:2:2/4:4:4 buffers:
/// - Y is in [16, 235], U/V are in [16, 240] (studio swing).
///
/// The function uses the standard BT.601 limited-range matrix.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn yuv_to_rgb_bt601(y: u8, u: u8, v: u8) -> (u8, u8, u8) {
    let y = f32::from(y) - 16.0;
    let u = f32::from(u) - 128.0;
    let v = f32::from(v) - 128.0;

    let r = 1.164 * y + 1.596 * v;
    let g = 1.164 * y - 0.392 * u - 0.813 * v;
    let b = 1.164 * y + 2.017 * u;

    let clamp = |x: f32| x.clamp(0.0, 255.0) as u8;
    (clamp(r), clamp(g), clamp(b))
}

/// Convert a single RGB sample to YUV (BT.601, limited range).
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn rgb_to_yuv_bt601(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r = f32::from(r);
    let g = f32::from(g);
    let b = f32::from(b);

    let y = 16.0 + 0.257 * r + 0.504 * g + 0.098 * b;
    let u = 128.0 - 0.148 * r - 0.291 * g + 0.439 * b;
    let v = 128.0 + 0.439 * r - 0.368 * g - 0.071 * b;

    let clamp = |x: f32| x.clamp(0.0, 255.0) as u8;
    (clamp(y), clamp(u), clamp(v))
}

/// Convert one row of packed YUV 4:2:2 (YUYV) pixels to interleaved RGB.
///
/// `src` must contain `width * 2` bytes (YUYV pairs).
/// `dst` must contain at least `width * 3` bytes (RGB triples).
///
/// # Panics
///
/// Panics if `src` or `dst` are too short for the given `width`.
pub fn convert_yuv422_row_to_rgb(src: &[u8], dst: &mut [u8], width: usize) {
    assert!(src.len() >= width * 2, "src too short");
    assert!(dst.len() >= width * 3, "dst too short");

    let mut si = 0usize;
    let mut di = 0usize;
    let pairs = width / 2;
    for _ in 0..pairs {
        let y0 = src[si];
        let u = src[si + 1];
        let y1 = src[si + 2];
        let v = src[si + 3];
        si += 4;

        let (r0, g0, b0) = yuv_to_rgb_bt601(y0, u, v);
        let (r1, g1, b1) = yuv_to_rgb_bt601(y1, u, v);

        dst[di] = r0;
        dst[di + 1] = g0;
        dst[di + 2] = b0;
        dst[di + 3] = r1;
        dst[di + 4] = g1;
        dst[di + 5] = b1;
        di += 6;
    }
    // Handle odd pixel if width is odd
    if width & 1 != 0 {
        let y = src[si];
        let u = src[si + 1];
        let v = src[si + 3];
        let (r, g, b) = yuv_to_rgb_bt601(y, u, v);
        dst[di] = r;
        dst[di + 1] = g;
        dst[di + 2] = b;
    }
}

/// A planar YUV buffer (Y, U, V planes stored separately).
#[derive(Debug, Clone)]
pub struct YuvPlane {
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
    width: usize,
    height: usize,
}

impl YuvPlane {
    /// Create a new `YuvPlane` with all samples initialised to black.
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        let luma_len = width * height;
        let chroma_len = (width / 2) * (height / 2);
        Self {
            y: vec![16u8; luma_len],
            u: vec![128u8; chroma_len],
            v: vec![128u8; chroma_len],
            width,
            height,
        }
    }

    /// Returns the width in pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the height in pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Sample the RGB colour at pixel `(x, y)`.
    ///
    /// Returns `None` if the coordinates are out of bounds.
    #[must_use]
    pub fn sample(&self, x: usize, y: usize) -> Option<(u8, u8, u8)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let luma = self.y[y * self.width + x];
        let cx = x / 2;
        let cy = y / 2;
        let cw = self.width / 2;
        let chroma_u = self.u[cy * cw + cx];
        let chroma_v = self.v[cy * cw + cx];
        Some(yuv_to_rgb_bt601(luma, chroma_u, chroma_v))
    }

    /// Returns a reference to the luma (Y) plane.
    #[must_use]
    pub fn y_plane(&self) -> &[u8] {
        &self.y
    }

    /// Returns a reference to the Cb (U) plane.
    #[must_use]
    pub fn u_plane(&self) -> &[u8] {
        &self.u
    }

    /// Returns a reference to the Cr (V) plane.
    #[must_use]
    pub fn v_plane(&self) -> &[u8] {
        &self.v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yuv_to_rgb_black() {
        // Y=16, U=128, V=128 → near-black
        let (r, g, b) = yuv_to_rgb_bt601(16, 128, 128);
        assert!(r < 5, "r={r}");
        assert!(g < 5, "g={g}");
        assert!(b < 5, "b={b}");
    }

    #[test]
    fn test_yuv_to_rgb_white() {
        // Y=235, U=128, V=128 → near-white
        let (r, g, b) = yuv_to_rgb_bt601(235, 128, 128);
        assert!(r > 250, "r={r}");
        assert!(g > 250, "g={g}");
        assert!(b > 250, "b={b}");
    }

    #[test]
    fn test_rgb_to_yuv_black() {
        let (y, u, v) = rgb_to_yuv_bt601(0, 0, 0);
        assert!(y <= 20, "y={y}");
        assert!((i16::from(u) - 128).abs() < 5, "u={u}");
        assert!((i16::from(v) - 128).abs() < 5, "v={v}");
    }

    #[test]
    fn test_rgb_to_yuv_white() {
        let (y, u, v) = rgb_to_yuv_bt601(255, 255, 255);
        assert!(y > 230, "y={y}");
        assert!((i16::from(u) - 128).abs() < 10, "u={u}");
        assert!((i16::from(v) - 128).abs() < 10, "v={v}");
    }

    #[test]
    fn test_roundtrip_grey() {
        let (y, u, v) = rgb_to_yuv_bt601(128, 128, 128);
        let (r, g, b) = yuv_to_rgb_bt601(y, u, v);
        assert!((i16::from(r) - 128).abs() < 5, "r={r}");
        assert!((i16::from(g) - 128).abs() < 5, "g={g}");
        assert!((i16::from(b) - 128).abs() < 5, "b={b}");
    }

    #[test]
    fn test_convert_yuv422_row_basic() {
        // Two pixels: Y0=16, U=128, Y1=16, V=128 → should be near black
        let src = [16u8, 128, 16, 128];
        let mut dst = [0u8; 6];
        convert_yuv422_row_to_rgb(&src, &mut dst, 2);
        assert!(dst[0] < 10);
    }

    #[test]
    fn test_yuv_plane_new() {
        let plane = YuvPlane::new(4, 4);
        assert_eq!(plane.width(), 4);
        assert_eq!(plane.height(), 4);
    }

    #[test]
    fn test_yuv_plane_sample_bounds() {
        let plane = YuvPlane::new(8, 8);
        assert!(plane.sample(0, 0).is_some());
        assert!(plane.sample(7, 7).is_some());
        assert!(plane.sample(8, 0).is_none());
        assert!(plane.sample(0, 8).is_none());
    }

    #[test]
    fn test_yuv_plane_default_black() {
        let plane = YuvPlane::new(4, 4);
        let (r, g, b) = plane.sample(0, 0).expect("should succeed in test");
        assert!(r < 5, "r={r}");
        assert!(g < 5, "g={g}");
        assert!(b < 5, "b={b}");
    }

    #[test]
    fn test_yuv_plane_sizes() {
        let plane = YuvPlane::new(16, 8);
        assert_eq!(plane.y_plane().len(), 128);
        assert_eq!(plane.u_plane().len(), 32);
        assert_eq!(plane.v_plane().len(), 32);
    }

    #[test]
    fn test_yuv_to_rgb_clamp_no_panic() {
        // Extreme values should not panic — just clamp
        let (r, g, b) = yuv_to_rgb_bt601(0, 0, 0);
        let _ = (r, g, b);
        let (r, g, b) = yuv_to_rgb_bt601(255, 255, 255);
        let _ = (r, g, b);
    }

    #[test]
    fn test_rgb_to_yuv_red_dominant_channel() {
        // Pure red has high V (Cr) in BT.601
        let (_, _, v) = rgb_to_yuv_bt601(255, 0, 0);
        assert!(v > 128, "v={v}");
    }

    #[test]
    fn test_rgb_to_yuv_blue_dominant_channel() {
        // Pure blue has high U (Cb) in BT.601
        let (_, u, _) = rgb_to_yuv_bt601(0, 0, 255);
        assert!(u > 128, "u={u}");
    }
}
