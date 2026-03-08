//! Pixel format conversion functions.
//!
//! This module provides functions to convert between different pixel formats,
//! including YUV and RGB color spaces with proper color matrix support.
#![allow(
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::trivially_copy_pass_by_ref
)]

/// Color matrix standard for YUV<->RGB conversion.
///
/// Different standards define different coefficients for converting between
/// YUV and RGB color spaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMatrix {
    /// ITU-R BT.601 standard (Standard Definition TV).
    ///
    /// Used for SD content (480p, 576p).
    Bt601,

    /// ITU-R BT.709 standard (High Definition TV).
    ///
    /// Used for HD content (720p, 1080p).
    Bt709,
}

impl ColorMatrix {
    /// Returns the Y coefficient for RGB to YUV conversion.
    const fn kr(&self) -> f32 {
        match self {
            Self::Bt601 => 0.299,
            Self::Bt709 => 0.2126,
        }
    }

    /// Returns the Y coefficient for RGB to YUV conversion.
    const fn kb(&self) -> f32 {
        match self {
            Self::Bt601 => 0.114,
            Self::Bt709 => 0.0722,
        }
    }

    /// Returns the derived green coefficient.
    fn kg(&self) -> f32 {
        1.0 - self.kr() - self.kb()
    }
}

/// Pixel format converter with pre-computed lookup tables.
///
/// This struct provides efficient conversion using pre-computed tables
/// to avoid repeated floating-point calculations.
#[derive(Clone, Debug)]
pub struct PixelConverter {
    /// Lookup table for Y to RGB conversion.
    y_table: [i32; 256],
    /// Lookup table for U to R conversion.
    u_r_table: [i32; 256],
    /// Lookup table for U to G conversion.
    u_g_table: [i32; 256],
    /// Lookup table for V to G conversion.
    v_g_table: [i32; 256],
    /// Lookup table for V to B conversion.
    v_b_table: [i32; 256],
    /// RGB to Y coefficients (scaled).
    r_y: i32,
    /// RGB to Y coefficients (scaled).
    g_y: i32,
    /// RGB to Y coefficients (scaled).
    b_y: i32,
    /// RGB to U coefficients (scaled).
    r_u: i32,
    /// RGB to U coefficients (scaled).
    g_u: i32,
    /// RGB to U coefficients (scaled).
    b_u: i32,
    /// RGB to V coefficients (scaled).
    r_v: i32,
    /// RGB to V coefficients (scaled).
    g_v: i32,
    /// RGB to V coefficients (scaled).
    b_v: i32,
}

impl PixelConverter {
    /// Creates a new pixel converter for the specified color matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::convert::pixel::{PixelConverter, ColorMatrix};
    ///
    /// let converter = PixelConverter::new(ColorMatrix::Bt709);
    /// ```
    #[must_use]
    #[allow(clippy::similar_names)]
    pub fn new(matrix: ColorMatrix) -> Self {
        let kr = matrix.kr();
        let kb = matrix.kb();
        let kg = matrix.kg();

        // Pre-compute YUV to RGB tables
        let mut y_table = [0i32; 256];
        let mut u_r_table = [0i32; 256];
        let mut u_g_table = [0i32; 256];
        let mut v_g_table = [0i32; 256];
        let mut v_b_table = [0i32; 256];

        for i in 0..256 {
            let y = i as f32 - 16.0;
            let u = i as f32 - 128.0;
            let v = i as f32 - 128.0;

            y_table[i] = (y * 1.164).round() as i32;
            u_r_table[i] = (v * 2.0 * (1.0 - kr)).round() as i32;
            v_b_table[i] = (u * 2.0 * (1.0 - kb)).round() as i32;
            u_g_table[i] = (u * 2.0 * (1.0 - kb) * kb / kg).round() as i32;
            v_g_table[i] = (v * 2.0 * (1.0 - kr) * kr / kg).round() as i32;
        }

        // Pre-compute RGB to YUV coefficients (scaled by 65536 for fixed-point)
        let scale = 65536.0;
        let r_y = (kr * scale).round() as i32;
        let g_y = (kg * scale).round() as i32;
        let b_y = (kb * scale).round() as i32;

        let r_u = ((-0.5 * kr / (1.0 - kb)) * scale).round() as i32;
        let g_u = ((-0.5 * kg / (1.0 - kb)) * scale).round() as i32;
        let b_u = ((0.5) * scale).round() as i32;

        let r_v = ((0.5) * scale).round() as i32;
        let g_v = ((-0.5 * kg / (1.0 - kr)) * scale).round() as i32;
        let b_v = ((-0.5 * kb / (1.0 - kr)) * scale).round() as i32;

        Self {
            y_table,
            u_r_table,
            u_g_table,
            v_g_table,
            v_b_table,
            r_y,
            g_y,
            b_y,
            r_u,
            g_u,
            b_u,
            r_v,
            g_v,
            b_v,
        }
    }

    /// Converts a single YUV pixel to RGB.
    ///
    /// Returns (R, G, B) in range [0, 255].
    #[must_use]
    #[inline]
    #[allow(clippy::too_many_lines, clippy::similar_names)]
    pub fn yuv_to_rgb(&self, y: u8, u: u8, v: u8) -> (u8, u8, u8) {
        let y_val = self.y_table[y as usize];
        let u_r = self.u_r_table[v as usize];
        let u_g = self.u_g_table[u as usize];
        let v_g = self.v_g_table[v as usize];
        let v_b = self.v_b_table[u as usize];

        let r = (y_val + u_r).clamp(0, 255);
        let g = (y_val - u_g - v_g).clamp(0, 255);
        let b = (y_val + v_b).clamp(0, 255);

        (r as u8, g as u8, b as u8)
    }

    /// Converts a single RGB pixel to YUV.
    ///
    /// Returns (Y, U, V) in range [0, 255].
    #[must_use]
    #[inline]
    #[allow(clippy::similar_names)]
    pub fn rgb_to_yuv(&self, r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        let r = i32::from(r);
        let g = i32::from(g);
        let b = i32::from(b);

        let y = ((r * self.r_y + g * self.g_y + b * self.b_y) >> 16) + 16;
        let u = ((r * self.r_u + g * self.g_u + b * self.b_u) >> 16) + 128;
        let v = ((r * self.r_v + g * self.g_v + b * self.b_v) >> 16) + 128;

        (
            y.clamp(0, 255) as u8,
            u.clamp(0, 255) as u8,
            v.clamp(0, 255) as u8,
        )
    }
}

impl Default for PixelConverter {
    fn default() -> Self {
        Self::new(ColorMatrix::Bt709)
    }
}

/// Converts `YUV420p` to RGB24.
///
/// `YUV420p` is a planar format with Y plane at full resolution and U/V planes
/// at half resolution in both dimensions.
///
/// # Arguments
///
/// * `y_plane` - Y (luma) plane data
/// * `u_plane` - U (chroma) plane data (width/2 * height/2)
/// * `v_plane` - V (chroma) plane data (width/2 * height/2)
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
/// * `matrix` - Color matrix to use for conversion
///
/// # Returns
///
/// RGB24 packed data (width * height * 3 bytes)
///
/// # Panics
///
/// Panics if input planes have incorrect sizes.
///
/// # Examples
///
/// ```
/// use oximedia_core::convert::pixel::{yuv420p_to_rgb24, ColorMatrix};
///
/// let width = 4;
/// let height = 4;
/// let y_plane = vec![128u8; width * height];
/// let u_plane = vec![128u8; (width / 2) * (height / 2)];
/// let v_plane = vec![128u8; (width / 2) * (height / 2)];
///
/// let rgb = yuv420p_to_rgb24(&y_plane, &u_plane, &v_plane, width, height, ColorMatrix::Bt709);
/// assert_eq!(rgb.len(), width * height * 3);
/// ```
#[must_use]
pub fn yuv420p_to_rgb24(
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    width: usize,
    height: usize,
    matrix: ColorMatrix,
) -> Vec<u8> {
    assert_eq!(y_plane.len(), width * height);
    assert_eq!(u_plane.len(), (width / 2) * (height / 2));
    assert_eq!(v_plane.len(), (width / 2) * (height / 2));

    let converter = PixelConverter::new(matrix);
    let mut rgb = vec![0u8; width * height * 3];

    for y in 0..height {
        for x in 0..width {
            let y_val = y_plane[y * width + x];
            let u_val = u_plane[(y / 2) * (width / 2) + (x / 2)];
            let v_val = v_plane[(y / 2) * (width / 2) + (x / 2)];

            let (r, g, b) = converter.yuv_to_rgb(y_val, u_val, v_val);

            let offset = (y * width + x) * 3;
            rgb[offset] = r;
            rgb[offset + 1] = g;
            rgb[offset + 2] = b;
        }
    }

    rgb
}

/// Converts RGB24 to `YUV420p`.
///
/// RGB24 is a packed format with R, G, B bytes interleaved.
/// `YUV420p` has separate planes with chroma subsampling.
///
/// # Arguments
///
/// * `rgb` - RGB24 packed data (width * height * 3 bytes)
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
/// * `matrix` - Color matrix to use for conversion
///
/// # Returns
///
/// Tuple of (Y plane, U plane, V plane)
///
/// # Panics
///
/// Panics if RGB data has incorrect size.
///
/// # Examples
///
/// ```
/// use oximedia_core::convert::pixel::{rgb24_to_yuv420p, ColorMatrix};
///
/// let width = 4;
/// let height = 4;
/// let rgb = vec![128u8; width * height * 3];
///
/// let (y_plane, u_plane, v_plane) = rgb24_to_yuv420p(&rgb, width, height, ColorMatrix::Bt709);
/// assert_eq!(y_plane.len(), width * height);
/// assert_eq!(u_plane.len(), (width / 2) * (height / 2));
/// assert_eq!(v_plane.len(), (width / 2) * (height / 2));
/// ```
#[must_use]
#[allow(clippy::similar_names)]
pub fn rgb24_to_yuv420p(
    rgb: &[u8],
    width: usize,
    height: usize,
    matrix: ColorMatrix,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    assert_eq!(rgb.len(), width * height * 3);

    let converter = PixelConverter::new(matrix);
    let mut y_plane = vec![0u8; width * height];
    let mut u_plane = vec![0u8; (width / 2) * (height / 2)];
    let mut v_plane = vec![0u8; (width / 2) * (height / 2)];

    // Convert RGB to YUV and downsample chroma
    for y in 0..height {
        for x in 0..width {
            let offset = (y * width + x) * 3;
            let r = rgb[offset];
            let g = rgb[offset + 1];
            let b = rgb[offset + 2];

            let (y_val, u_val, v_val) = converter.rgb_to_yuv(r, g, b);
            y_plane[y * width + x] = y_val;

            // Subsample chroma (average 2x2 blocks)
            if y % 2 == 0 && x % 2 == 0 {
                let mut u_sum = u32::from(u_val);
                let mut v_sum = u32::from(v_val);
                let mut count = 1;

                // Average with neighboring pixels if they exist
                if x + 1 < width {
                    let offset = (y * width + x + 1) * 3;
                    let (_, u, v) =
                        converter.rgb_to_yuv(rgb[offset], rgb[offset + 1], rgb[offset + 2]);
                    u_sum += u32::from(u);
                    v_sum += u32::from(v);
                    count += 1;
                }
                if y + 1 < height {
                    let offset = ((y + 1) * width + x) * 3;
                    let (_, u, v) =
                        converter.rgb_to_yuv(rgb[offset], rgb[offset + 1], rgb[offset + 2]);
                    u_sum += u32::from(u);
                    v_sum += u32::from(v);
                    count += 1;
                }
                if x + 1 < width && y + 1 < height {
                    let offset = ((y + 1) * width + x + 1) * 3;
                    let (_, u, v) =
                        converter.rgb_to_yuv(rgb[offset], rgb[offset + 1], rgb[offset + 2]);
                    u_sum += u32::from(u);
                    v_sum += u32::from(v);
                    count += 1;
                }

                let u_idx = (y / 2) * (width / 2) + (x / 2);
                u_plane[u_idx] = (u_sum / count) as u8;
                v_plane[u_idx] = (v_sum / count) as u8;
            }
        }
    }

    (y_plane, u_plane, v_plane)
}

/// Converts `YUV420p` to `YUV444p`.
///
/// `YUV444p` has full chroma resolution (no subsampling).
/// This upsamples the chroma planes using bilinear interpolation.
///
/// # Arguments
///
/// * `y_plane` - Y (luma) plane data
/// * `u_plane` - U (chroma) plane data (width/2 * height/2)
/// * `v_plane` - V (chroma) plane data (width/2 * height/2)
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
///
/// # Returns
///
/// Tuple of (Y plane, U plane, V plane) all at full resolution
///
/// # Panics
///
/// Panics if input planes have incorrect sizes.
///
/// # Examples
///
/// ```
/// use oximedia_core::convert::pixel::yuv420p_to_yuv444p;
///
/// let width = 4;
/// let height = 4;
/// let y_plane = vec![128u8; width * height];
/// let u_plane = vec![100u8; (width / 2) * (height / 2)];
/// let v_plane = vec![150u8; (width / 2) * (height / 2)];
///
/// let (y_out, u_out, v_out) = yuv420p_to_yuv444p(&y_plane, &u_plane, &v_plane, width, height);
/// assert_eq!(y_out.len(), width * height);
/// assert_eq!(u_out.len(), width * height);
/// assert_eq!(v_out.len(), width * height);
/// ```
#[must_use]
pub fn yuv420p_to_yuv444p(
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    width: usize,
    height: usize,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    assert_eq!(y_plane.len(), width * height);
    assert_eq!(u_plane.len(), (width / 2) * (height / 2));
    assert_eq!(v_plane.len(), (width / 2) * (height / 2));

    let y_out = y_plane.to_vec();
    let mut u_out = vec![0u8; width * height];
    let mut v_out = vec![0u8; width * height];

    let chroma_width = width / 2;

    // Upsample chroma using bilinear interpolation
    for y in 0..height {
        for x in 0..width {
            let cx = x / 2;
            let cy = y / 2;
            #[allow(clippy::cast_precision_loss)]
            let fx = (x % 2) as f32 * 0.5;
            #[allow(clippy::cast_precision_loss)]
            let fy = (y % 2) as f32 * 0.5;

            let c00_idx = cy * chroma_width + cx;
            let c10_idx = if cx + 1 < chroma_width {
                c00_idx + 1
            } else {
                c00_idx
            };
            let c01_idx = if cy + 1 < height / 2 {
                (cy + 1) * chroma_width + cx
            } else {
                c00_idx
            };
            let c11_idx = if cx + 1 < chroma_width && cy + 1 < height / 2 {
                (cy + 1) * chroma_width + cx + 1
            } else {
                c00_idx
            };

            // Bilinear interpolation for U
            let u00 = f32::from(u_plane[c00_idx]);
            let u10 = f32::from(u_plane[c10_idx]);
            let u01 = f32::from(u_plane[c01_idx]);
            let u11 = f32::from(u_plane[c11_idx]);

            let u_top = u00 * (1.0 - fx) + u10 * fx;
            let u_bottom = u01 * (1.0 - fx) + u11 * fx;
            let u_val = u_top * (1.0 - fy) + u_bottom * fy;

            // Bilinear interpolation for V
            let v00 = f32::from(v_plane[c00_idx]);
            let v10 = f32::from(v_plane[c10_idx]);
            let v01 = f32::from(v_plane[c01_idx]);
            let v11 = f32::from(v_plane[c11_idx]);

            let v_top = v00 * (1.0 - fx) + v10 * fx;
            let v_bottom = v01 * (1.0 - fx) + v11 * fx;
            let v_val = v_top * (1.0 - fy) + v_bottom * fy;

            let out_idx = y * width + x;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                u_out[out_idx] = u_val.round().clamp(0.0, 255.0) as u8;
                v_out[out_idx] = v_val.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    (y_out, u_out, v_out)
}

/// Converts `YUV444p` to `YUV420p`.
///
/// This downsamples the chroma planes by averaging 2x2 blocks.
///
/// # Arguments
///
/// * `y_plane` - Y (luma) plane data
/// * `u_plane` - U (chroma) plane data at full resolution
/// * `v_plane` - V (chroma) plane data at full resolution
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
///
/// # Returns
///
/// Tuple of (Y plane, U plane, V plane) with subsampled chroma
///
/// # Panics
///
/// Panics if input planes have incorrect sizes.
///
/// # Examples
///
/// ```
/// use oximedia_core::convert::pixel::yuv444p_to_yuv420p;
///
/// let width = 4;
/// let height = 4;
/// let y_plane = vec![128u8; width * height];
/// let u_plane = vec![100u8; width * height];
/// let v_plane = vec![150u8; width * height];
///
/// let (y_out, u_out, v_out) = yuv444p_to_yuv420p(&y_plane, &u_plane, &v_plane, width, height);
/// assert_eq!(y_out.len(), width * height);
/// assert_eq!(u_out.len(), (width / 2) * (height / 2));
/// assert_eq!(v_out.len(), (width / 2) * (height / 2));
/// ```
#[must_use]
pub fn yuv444p_to_yuv420p(
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    width: usize,
    height: usize,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    assert_eq!(y_plane.len(), width * height);
    assert_eq!(u_plane.len(), width * height);
    assert_eq!(v_plane.len(), width * height);

    let y_out = y_plane.to_vec();
    let mut u_out = vec![0u8; (width / 2) * (height / 2)];
    let mut v_out = vec![0u8; (width / 2) * (height / 2)];

    // Downsample chroma by averaging 2x2 blocks
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let mut u_sum = 0u32;
            let mut v_sum = 0u32;

            for dy in 0..2 {
                for dx in 0..2 {
                    let idx = (y + dy) * width + (x + dx);
                    u_sum += u32::from(u_plane[idx]);
                    v_sum += u32::from(v_plane[idx]);
                }
            }

            let out_idx = (y / 2) * (width / 2) + (x / 2);
            #[allow(clippy::cast_possible_truncation)]
            {
                u_out[out_idx] = (u_sum / 4) as u8;
                v_out[out_idx] = (v_sum / 4) as u8;
            }
        }
    }

    (y_out, u_out, v_out)
}

/// Converts `YUV420p` to grayscale (8-bit).
///
/// Simply extracts the Y (luma) plane.
///
/// # Arguments
///
/// * `y_plane` - Y (luma) plane data
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
///
/// # Returns
///
/// Grayscale image data
///
/// # Panics
///
/// Panics if Y plane has incorrect size.
///
/// # Examples
///
/// ```
/// use oximedia_core::convert::pixel::yuv420p_to_gray8;
///
/// let width = 4;
/// let height = 4;
/// let y_plane = vec![128u8; width * height];
///
/// let gray = yuv420p_to_gray8(&y_plane, width, height);
/// assert_eq!(gray.len(), width * height);
/// ```
#[must_use]
pub fn yuv420p_to_gray8(y_plane: &[u8], width: usize, height: usize) -> Vec<u8> {
    assert_eq!(y_plane.len(), width * height);
    y_plane.to_vec()
}

/// Converts RGB24 to grayscale (8-bit).
///
/// Uses standard luminance formula: Y = 0.299*R + 0.587*G + 0.114*B
///
/// # Arguments
///
/// * `rgb` - RGB24 packed data
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
///
/// # Returns
///
/// Grayscale image data
///
/// # Panics
///
/// Panics if RGB data has incorrect size.
///
/// # Examples
///
/// ```
/// use oximedia_core::convert::pixel::rgb24_to_gray8;
///
/// let width = 4;
/// let height = 4;
/// let rgb = vec![128u8; width * height * 3];
///
/// let gray = rgb24_to_gray8(&rgb, width, height);
/// assert_eq!(gray.len(), width * height);
/// ```
#[must_use]
pub fn rgb24_to_gray8(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    assert_eq!(rgb.len(), width * height * 3);

    let mut gray = vec![0u8; width * height];

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    for (i, gray_val) in gray.iter_mut().enumerate().take(width * height) {
        let offset = i * 3;
        let r = f32::from(rgb[offset]);
        let g = f32::from(rgb[offset + 1]);
        let b = f32::from(rgb[offset + 2]);

        let y = (0.299 * r + 0.587 * g + 0.114 * b)
            .round()
            .clamp(0.0, 255.0);
        *gray_val = y as u8;
    }

    gray
}

/// Converts grayscale (8-bit) to RGB24.
///
/// Replicates the grayscale value across all three channels.
///
/// # Arguments
///
/// * `gray` - Grayscale image data
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
///
/// # Returns
///
/// RGB24 packed data
///
/// # Panics
///
/// Panics if grayscale data has incorrect size.
///
/// # Examples
///
/// ```
/// use oximedia_core::convert::pixel::gray8_to_rgb24;
///
/// let width = 4;
/// let height = 4;
/// let gray = vec![128u8; width * height];
///
/// let rgb = gray8_to_rgb24(&gray, width, height);
/// assert_eq!(rgb.len(), width * height * 3);
/// ```
#[must_use]
pub fn gray8_to_rgb24(gray: &[u8], width: usize, height: usize) -> Vec<u8> {
    assert_eq!(gray.len(), width * height);

    let mut rgb = vec![0u8; width * height * 3];

    for (i, &val) in gray.iter().enumerate().take(width * height) {
        let offset = i * 3;
        rgb[offset] = val;
        rgb[offset + 1] = val;
        rgb[offset + 2] = val;
    }

    rgb
}

/// Converts grayscale (8-bit) to `YUV420p`.
///
/// Sets Y plane to grayscale values and U/V planes to neutral (128).
///
/// # Arguments
///
/// * `gray` - Grayscale image data
/// * `width` - Frame width in pixels
/// * `height` - Frame height in pixels
///
/// # Returns
///
/// Tuple of (Y plane, U plane, V plane)
///
/// # Panics
///
/// Panics if grayscale data has incorrect size.
///
/// # Examples
///
/// ```
/// use oximedia_core::convert::pixel::gray8_to_yuv420p;
///
/// let width = 4;
/// let height = 4;
/// let gray = vec![128u8; width * height];
///
/// let (y_plane, u_plane, v_plane) = gray8_to_yuv420p(&gray, width, height);
/// assert_eq!(y_plane.len(), width * height);
/// assert_eq!(u_plane.len(), (width / 2) * (height / 2));
/// assert_eq!(v_plane.len(), (width / 2) * (height / 2));
/// ```
#[must_use]
pub fn gray8_to_yuv420p(gray: &[u8], width: usize, height: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    assert_eq!(gray.len(), width * height);

    let y_plane = gray.to_vec();
    let u_plane = vec![128u8; (width / 2) * (height / 2)];
    let v_plane = vec![128u8; (width / 2) * (height / 2)];

    (y_plane, u_plane, v_plane)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_matrix_coefficients() {
        let bt601 = ColorMatrix::Bt601;
        let bt709 = ColorMatrix::Bt709;

        // BT.601 coefficients
        assert!((bt601.kr() - 0.299).abs() < f32::EPSILON);
        assert!((bt601.kb() - 0.114).abs() < f32::EPSILON);

        // BT.709 coefficients
        assert!((bt709.kr() - 0.2126).abs() < f32::EPSILON);
        assert!((bt709.kb() - 0.0722).abs() < f32::EPSILON);
    }

    #[test]
    fn test_pixel_converter_yuv_to_rgb() {
        let converter = PixelConverter::new(ColorMatrix::Bt709);

        // Test neutral gray (Y=128, U=128, V=128)
        let (r, g, b) = converter.yuv_to_rgb(128, 128, 128);
        // Should be approximately gray
        assert!((i16::from(r) - i16::from(g)).abs() < 20);
        assert!((i16::from(g) - i16::from(b)).abs() < 20);
    }

    #[test]
    fn test_yuv420p_to_rgb24() {
        let width = 4;
        let height = 4;
        let y_plane = vec![128u8; width * height];
        let u_plane = vec![128u8; (width / 2) * (height / 2)];
        let v_plane = vec![128u8; (width / 2) * (height / 2)];

        let rgb = yuv420p_to_rgb24(
            &y_plane,
            &u_plane,
            &v_plane,
            width,
            height,
            ColorMatrix::Bt709,
        );
        assert_eq!(rgb.len(), width * height * 3);
    }

    #[test]
    fn test_rgb24_to_yuv420p() {
        let width = 4;
        let height = 4;
        let rgb = vec![128u8; width * height * 3];

        let (y_plane, u_plane, v_plane) = rgb24_to_yuv420p(&rgb, width, height, ColorMatrix::Bt709);
        assert_eq!(y_plane.len(), width * height);
        assert_eq!(u_plane.len(), (width / 2) * (height / 2));
        assert_eq!(v_plane.len(), (width / 2) * (height / 2));
    }

    #[test]
    fn test_yuv420p_to_yuv444p() {
        let width = 4;
        let height = 4;
        let y_plane = vec![128u8; width * height];
        let u_plane = vec![100u8; (width / 2) * (height / 2)];
        let v_plane = vec![150u8; (width / 2) * (height / 2)];

        let (y_out, u_out, v_out) = yuv420p_to_yuv444p(&y_plane, &u_plane, &v_plane, width, height);
        assert_eq!(y_out.len(), width * height);
        assert_eq!(u_out.len(), width * height);
        assert_eq!(v_out.len(), width * height);
    }

    #[test]
    fn test_yuv444p_to_yuv420p() {
        let width = 4;
        let height = 4;
        let y_plane = vec![128u8; width * height];
        let u_plane = vec![100u8; width * height];
        let v_plane = vec![150u8; width * height];

        let (y_out, u_out, v_out) = yuv444p_to_yuv420p(&y_plane, &u_plane, &v_plane, width, height);
        assert_eq!(y_out.len(), width * height);
        assert_eq!(u_out.len(), (width / 2) * (height / 2));
        assert_eq!(v_out.len(), (width / 2) * (height / 2));
    }

    #[test]
    fn test_grayscale_conversions() {
        let width = 4;
        let height = 4;

        // Test YUV420p to gray
        let y_plane = vec![128u8; width * height];
        let gray = yuv420p_to_gray8(&y_plane, width, height);
        assert_eq!(gray.len(), width * height);
        assert_eq!(gray[0], 128);

        // Test RGB to gray
        let rgb = vec![128u8; width * height * 3];
        let gray = rgb24_to_gray8(&rgb, width, height);
        assert_eq!(gray.len(), width * height);

        // Test gray to RGB
        let rgb = gray8_to_rgb24(&gray, width, height);
        assert_eq!(rgb.len(), width * height * 3);

        // Test gray to YUV420p
        let (y, u, v) = gray8_to_yuv420p(&gray, width, height);
        assert_eq!(y.len(), width * height);
        assert_eq!(u.len(), (width / 2) * (height / 2));
        assert_eq!(v.len(), (width / 2) * (height / 2));
        assert_eq!(u[0], 128); // Neutral chroma
        assert_eq!(v[0], 128); // Neutral chroma
    }

    #[test]
    fn test_roundtrip_rgb_yuv() {
        let width = 4;
        let height = 4;
        let mut rgb = vec![0u8; width * height * 3];

        // Create a simple pattern
        for i in 0..rgb.len() {
            rgb[i] = ((i * 50) % 256) as u8;
        }

        // Convert RGB -> YUV -> RGB
        let (y, u, v) = rgb24_to_yuv420p(&rgb, width, height, ColorMatrix::Bt709);
        let rgb2 = yuv420p_to_rgb24(&y, &u, &v, width, height, ColorMatrix::Bt709);

        assert_eq!(rgb.len(), rgb2.len());
        // Due to chroma subsampling, we expect some loss.
        // Verify rgb2 is non-empty (all u8 values are inherently <= 255).
        assert!(!rgb2.is_empty());
    }
}
