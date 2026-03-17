// SPDX-License-Identifier: Apache-2.0
// Copyright (c) COOLJAPAN OU (Team Kitasan)

//! Image transformation processing pipeline.
//!
//! Implements the actual pixel-level image transformations using pure Rust algorithms
//! (no C/FFI dependencies). The pipeline processes operations in the correct order:
//!
//! **Decode -> Trim -> Resize/Crop -> Rotate -> Color adjustments -> Sharpen/Blur -> Border/Padding -> Encode**
//!
//! # Architecture
//!
//! The processor works with [`PixelBuffer`] -- a simple container for raw pixel data
//! in either RGB or RGBA layout. A [`TransformParams`] specification is converted
//! into an ordered `Vec<PipelineStep>` via [`build_pipeline`], then each step is
//! applied in sequence by [`apply_transforms`].
//!
//! All image processing algorithms (bilinear/Lanczos resize, separable Gaussian blur,
//! unsharp mask, rotation, gamma LUT, etc.) are implemented from scratch in pure Rust.

use crate::transform::{
    Border, Color, FitMode, Gravity, OutputFormat, Padding, Rotation, TransformParams, Trim,
};

// ============================================================================
// PixelBuffer
// ============================================================================

/// A simple pixel buffer for processing.
///
/// Stores raw pixel data in row-major order with either RGB (3 channels)
/// or RGBA (4 channels) layout.
#[derive(Debug, Clone)]
pub struct PixelBuffer {
    /// Raw pixel data in row-major order (RGB or RGBA).
    pub data: Vec<u8>,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Number of channels per pixel: 3 (RGB) or 4 (RGBA).
    pub channels: u8,
}

impl PixelBuffer {
    /// Create a new pixel buffer filled with zeros.
    pub fn new(width: u32, height: u32, channels: u8) -> Self {
        let len = width as usize * height as usize * channels as usize;
        Self {
            data: vec![0u8; len],
            width,
            height,
            channels,
        }
    }

    /// Create a pixel buffer from existing RGBA data.
    ///
    /// Returns an error if the data length does not match `width * height * 4`.
    pub fn from_rgba(data: Vec<u8>, width: u32, height: u32) -> Result<Self, ProcessingError> {
        let expected = width as usize * height as usize * 4;
        if data.len() != expected {
            return Err(ProcessingError::InvalidDimensions {
                width,
                height,
                data_len: data.len(),
            });
        }
        Ok(Self {
            data,
            width,
            height,
            channels: 4,
        })
    }

    /// Create a pixel buffer from existing RGB data.
    ///
    /// Returns an error if the data length does not match `width * height * 3`.
    pub fn from_rgb(data: Vec<u8>, width: u32, height: u32) -> Result<Self, ProcessingError> {
        let expected = width as usize * height as usize * 3;
        if data.len() != expected {
            return Err(ProcessingError::InvalidDimensions {
                width,
                height,
                data_len: data.len(),
            });
        }
        Ok(Self {
            data,
            width,
            height,
            channels: 3,
        })
    }

    /// Get pixel at (x, y) as a slice of channel values.
    ///
    /// Returns `None` if the coordinates are out of bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<&[u8]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = (y as usize * self.width as usize + x as usize) * self.channels as usize;
        self.data.get(idx..idx + self.channels as usize)
    }

    /// Set pixel at (x, y) from a slice of channel values.
    ///
    /// Does nothing if the coordinates are out of bounds.
    pub fn set_pixel(&mut self, x: u32, y: u32, pixel: &[u8]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let ch = self.channels as usize;
        let idx = (y as usize * self.width as usize + x as usize) * ch;
        if idx + ch <= self.data.len() && pixel.len() >= ch {
            self.data[idx..idx + ch].copy_from_slice(&pixel[..ch]);
        }
    }

    /// Get pixel with bilinear interpolation at fractional coordinates.
    ///
    /// Returns RGBA values. For RGB buffers, alpha is set to 255.
    /// Out-of-bounds coordinates are clamped to the nearest edge pixel.
    pub fn sample_bilinear(&self, x: f64, y: f64) -> [u8; 4] {
        if self.width == 0 || self.height == 0 {
            return [0, 0, 0, 255];
        }

        let max_x = (self.width as f64) - 1.0;
        let max_y = (self.height as f64) - 1.0;
        let x = x.clamp(0.0, max_x);
        let y = y.clamp(0.0, max_y);

        let x0 = x.floor() as u32;
        let y0 = y.floor() as u32;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);

        let fx = x - x.floor();
        let fy = y - y.floor();

        let p00 = self.get_pixel_rgba(x0, y0);
        let p10 = self.get_pixel_rgba(x1, y0);
        let p01 = self.get_pixel_rgba(x0, y1);
        let p11 = self.get_pixel_rgba(x1, y1);

        let mut result = [0u8; 4];
        for i in 0..4 {
            let top = p00[i] as f64 * (1.0 - fx) + p10[i] as f64 * fx;
            let bottom = p01[i] as f64 * (1.0 - fx) + p11[i] as f64 * fx;
            let value = top * (1.0 - fy) + bottom * fy;
            result[i] = value.round().clamp(0.0, 255.0) as u8;
        }
        result
    }

    /// Get pixel as RGBA (pads RGB with alpha=255).
    fn get_pixel_rgba(&self, x: u32, y: u32) -> [u8; 4] {
        match self.get_pixel(x, y) {
            Some(p) if self.channels == 4 => [p[0], p[1], p[2], p[3]],
            Some(p) if self.channels >= 3 => [p[0], p[1], p[2], 255],
            Some(p) if self.channels == 1 => [p[0], p[0], p[0], 255],
            _ => [0, 0, 0, 255],
        }
    }

    /// Row stride in bytes.
    fn stride(&self) -> usize {
        self.width as usize * self.channels as usize
    }
}

// ============================================================================
// ProcessingError
// ============================================================================

/// Errors that can occur during pixel processing.
#[derive(Debug, thiserror::Error)]
pub enum ProcessingError {
    /// Buffer dimensions do not match the data length.
    #[error("invalid buffer dimensions: {width}x{height} with {data_len} bytes")]
    InvalidDimensions {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// Actual data length in bytes.
        data_len: usize,
    },

    /// A processing operation failed.
    #[error("processing failed: {0}")]
    ProcessingFailed(String),

    /// The requested operation is not supported.
    #[error("unsupported operation: {0}")]
    UnsupportedOperation(String),
}

// ============================================================================
// PipelineStep
// ============================================================================

/// A single step in the processing pipeline.
#[derive(Debug, Clone)]
pub enum PipelineStep {
    /// Trim a fixed number of pixels from each edge.
    Trim(Trim),
    /// Resize the image to target dimensions with a fit mode and gravity.
    Resize {
        /// Target width (0 = derive from aspect ratio).
        width: u32,
        /// Target height (0 = derive from aspect ratio).
        height: u32,
        /// How to fit the image into the target dimensions.
        fit: FitMode,
        /// Anchor point for cropping.
        gravity: Gravity,
    },
    /// Rotate by 90-degree increments.
    Rotate(Rotation),
    /// Adjust brightness (-1.0 to 1.0).
    Brightness(f64),
    /// Adjust contrast (-1.0 to 1.0).
    Contrast(f64),
    /// Apply gamma correction (> 0.0).
    Gamma(f64),
    /// Unsharp-mask sharpening amount.
    Sharpen(f64),
    /// Gaussian blur radius (sigma).
    Blur(f64),
    /// Add a coloured border around the image.
    AddBorder(Border),
    /// Add padding with a background colour.
    AddPadding(Padding, Color),
}

// ============================================================================
// Pipeline building
// ============================================================================

/// Build the processing pipeline from transform parameters.
///
/// Steps are ordered:
/// **Trim -> Resize -> Rotate -> Color adjustments -> Sharpen/Blur -> Border -> Padding**
pub fn build_pipeline(params: &TransformParams, _output_format: OutputFormat) -> Vec<PipelineStep> {
    let mut steps = Vec::new();

    // 1. Trim
    if let Some(trim) = params.trim {
        if trim.top > 0 || trim.right > 0 || trim.bottom > 0 || trim.left > 0 {
            steps.push(PipelineStep::Trim(trim));
        }
    }

    // 2. Resize (requires at least width or height)
    let eff_w = params.effective_width();
    let eff_h = params.effective_height();
    if eff_w.is_some() || eff_h.is_some() {
        steps.push(PipelineStep::Resize {
            width: eff_w.unwrap_or(0),
            height: eff_h.unwrap_or(0),
            fit: params.fit,
            gravity: params.gravity.clone(),
        });
    }

    // 3. Rotate
    match params.rotate {
        Rotation::Deg0 => {}
        other => steps.push(PipelineStep::Rotate(other)),
    }

    // 4. Color adjustments
    if params.brightness.abs() > f64::EPSILON {
        steps.push(PipelineStep::Brightness(params.brightness));
    }
    if params.contrast.abs() > f64::EPSILON {
        steps.push(PipelineStep::Contrast(params.contrast));
    }
    if (params.gamma - 1.0).abs() > f64::EPSILON {
        steps.push(PipelineStep::Gamma(params.gamma));
    }

    // 5. Sharpen / Blur
    if params.sharpen > f64::EPSILON {
        steps.push(PipelineStep::Sharpen(params.sharpen));
    }
    if params.blur > f64::EPSILON {
        steps.push(PipelineStep::Blur(params.blur));
    }

    // 6. Border
    if let Some(border) = params.border {
        if border.top > 0 || border.right > 0 || border.bottom > 0 || border.left > 0 {
            steps.push(PipelineStep::AddBorder(border));
        }
    }

    // 7. Padding
    if let Some(pad) = params.pad {
        if pad.top > f64::EPSILON
            || pad.right > f64::EPSILON
            || pad.bottom > f64::EPSILON
            || pad.left > f64::EPSILON
        {
            steps.push(PipelineStep::AddPadding(pad, params.background));
        }
    }

    steps
}

// ============================================================================
// Pipeline execution
// ============================================================================

/// Apply the full transformation pipeline to a pixel buffer.
///
/// Builds the pipeline from the given parameters and executes each step in order.
/// Returns a new buffer with all transformations applied.
pub fn apply_transforms(
    buffer: &mut PixelBuffer,
    params: &TransformParams,
) -> Result<PixelBuffer, ProcessingError> {
    let pipeline = build_pipeline(params, params.format);
    let mut result = buffer.clone();
    for step in &pipeline {
        result = apply_step(result, step)?;
    }
    Ok(result)
}

/// Apply a single pipeline step to a pixel buffer.
fn apply_step(buffer: PixelBuffer, step: &PipelineStep) -> Result<PixelBuffer, ProcessingError> {
    match step {
        PipelineStep::Trim(trim) => apply_trim(buffer, trim),
        PipelineStep::Resize {
            width,
            height,
            fit,
            gravity,
        } => apply_resize(buffer, *width, *height, *fit, gravity),
        PipelineStep::Rotate(rotation) => apply_rotation(buffer, *rotation),
        PipelineStep::Brightness(v) => apply_brightness(buffer, *v),
        PipelineStep::Contrast(v) => apply_contrast(buffer, *v),
        PipelineStep::Gamma(v) => apply_gamma(buffer, *v),
        PipelineStep::Sharpen(v) => apply_sharpen(buffer, *v),
        PipelineStep::Blur(v) => apply_blur(buffer, *v),
        PipelineStep::AddBorder(border) => apply_border(buffer, border),
        PipelineStep::AddPadding(padding, bg) => apply_padding(buffer, padding, *bg),
    }
}

// ============================================================================
// Trim
// ============================================================================

/// Crop a fixed number of pixels from each edge.
fn apply_trim(buffer: PixelBuffer, trim: &Trim) -> Result<PixelBuffer, ProcessingError> {
    if buffer.width == 0 || buffer.height == 0 {
        return Ok(buffer);
    }

    let left = trim.left.min(buffer.width);
    let right = trim.right.min(buffer.width.saturating_sub(left));
    let top = trim.top.min(buffer.height);
    let bottom = trim.bottom.min(buffer.height.saturating_sub(top));

    let new_w = buffer.width.saturating_sub(left + right);
    let new_h = buffer.height.saturating_sub(top + bottom);

    if new_w == 0 || new_h == 0 {
        return Ok(PixelBuffer::new(0, 0, buffer.channels));
    }

    crop_region(&buffer, left, top, new_w, new_h)
}

// ============================================================================
// Resize
// ============================================================================

/// Resize the image, respecting the fit mode and gravity anchor.
fn apply_resize(
    buffer: PixelBuffer,
    target_width: u32,
    target_height: u32,
    fit: FitMode,
    gravity: &Gravity,
) -> Result<PixelBuffer, ProcessingError> {
    if buffer.width == 0 || buffer.height == 0 {
        return Ok(buffer);
    }

    // Resolve zero-sentinel dimensions from aspect ratio
    let src_aspect = buffer.width as f64 / buffer.height as f64;
    let (tw, th) = match (target_width, target_height) {
        (0, 0) => return Ok(buffer),
        (0, h) => ((h as f64 * src_aspect).round().max(1.0) as u32, h),
        (w, 0) => (w, (w as f64 / src_aspect).round().max(1.0) as u32),
        (w, h) => (w, h),
    };

    match fit {
        FitMode::ScaleDown => {
            if buffer.width <= tw && buffer.height <= th {
                return Ok(buffer);
            }
            let (fw, fh) = fit_contain_dims(buffer.width, buffer.height, tw, th);
            Ok(bilinear_resize(&buffer, fw, fh))
        }
        FitMode::Contain => {
            let (fw, fh) = fit_contain_dims(buffer.width, buffer.height, tw, th);
            Ok(bilinear_resize(&buffer, fw, fh))
        }
        FitMode::Cover => {
            let (fw, fh) = fit_cover_dims(buffer.width, buffer.height, tw, th);
            let resized = bilinear_resize(&buffer, fw, fh);
            let (cx, cy, cw, ch) = calculate_crop_rect(fw, fh, tw, th, gravity);
            crop_region(&resized, cx, cy, cw, ch)
        }
        FitMode::Crop => {
            let (cx, cy, cw, ch) = calculate_crop_rect(
                buffer.width,
                buffer.height,
                tw.min(buffer.width),
                th.min(buffer.height),
                gravity,
            );
            crop_region(&buffer, cx, cy, cw, ch)
        }
        FitMode::Pad => {
            let (fw, fh) = fit_contain_dims(buffer.width, buffer.height, tw, th);
            let resized = bilinear_resize(&buffer, fw, fh);
            pad_to_exact_size(resized, tw, th, Color::black())
        }
        FitMode::Fill => Ok(bilinear_resize(&buffer, tw, th)),
    }
}

/// Bilinear interpolation resize.
///
/// Maps each destination pixel back to a fractional source coordinate and uses
/// bilinear interpolation to produce a smooth result.
pub fn bilinear_resize(buffer: &PixelBuffer, new_width: u32, new_height: u32) -> PixelBuffer {
    if new_width == 0 || new_height == 0 {
        return PixelBuffer::new(new_width, new_height, buffer.channels);
    }
    if new_width == buffer.width && new_height == buffer.height {
        return buffer.clone();
    }

    let ch = buffer.channels as usize;
    let mut output = PixelBuffer::new(new_width, new_height, buffer.channels);

    let x_ratio = if new_width > 1 {
        (buffer.width as f64 - 1.0) / (new_width as f64 - 1.0)
    } else {
        0.0
    };
    let y_ratio = if new_height > 1 {
        (buffer.height as f64 - 1.0) / (new_height as f64 - 1.0)
    } else {
        0.0
    };

    for dy in 0..new_height {
        for dx in 0..new_width {
            let sx = dx as f64 * x_ratio;
            let sy = dy as f64 * y_ratio;
            let rgba = buffer.sample_bilinear(sx, sy);
            output.set_pixel(dx, dy, &rgba[..ch]);
        }
    }
    output
}

/// Lanczos3 resize for higher quality output.
///
/// Uses a separable two-pass (horizontal then vertical) approach with a
/// Lanczos3 kernel: `sinc(x) * sinc(x/3)` for `|x| < 3`.
pub fn lanczos_resize(buffer: &PixelBuffer, new_width: u32, new_height: u32) -> PixelBuffer {
    if new_width == 0 || new_height == 0 {
        return PixelBuffer::new(new_width, new_height, buffer.channels);
    }
    if new_width == buffer.width && new_height == buffer.height {
        return buffer.clone();
    }

    let ch = buffer.channels as usize;

    // Pass 1: horizontal resize
    let mut h_resized = PixelBuffer::new(new_width, buffer.height, buffer.channels);
    let x_scale = buffer.width as f64 / new_width as f64;
    let x_support = if x_scale > 1.0 { 3.0 * x_scale } else { 3.0 };

    for y in 0..buffer.height {
        for dx in 0..new_width {
            let center = (dx as f64 + 0.5) * x_scale - 0.5;
            let left = (center - x_support).ceil().max(0.0) as u32;
            let right = (center + x_support).floor().min(buffer.width as f64 - 1.0) as u32;

            let mut accum = [0.0f64; 4];
            let mut weight_sum = 0.0f64;

            for sx in left..=right {
                let dist = (sx as f64 - center) / if x_scale > 1.0 { x_scale } else { 1.0 };
                let w = lanczos3_kernel(dist);
                weight_sum += w;
                let pixel = buffer.get_pixel_rgba(sx, y);
                for c in 0..4 {
                    accum[c] += pixel[c] as f64 * w;
                }
            }

            if weight_sum.abs() > f64::EPSILON {
                let inv = 1.0 / weight_sum;
                let mut pixel = [0u8; 4];
                for c in 0..4 {
                    pixel[c] = (accum[c] * inv).round().clamp(0.0, 255.0) as u8;
                }
                h_resized.set_pixel(dx, y, &pixel[..ch]);
            }
        }
    }

    // Pass 2: vertical resize
    let mut output = PixelBuffer::new(new_width, new_height, buffer.channels);
    let y_scale = buffer.height as f64 / new_height as f64;
    let y_support = if y_scale > 1.0 { 3.0 * y_scale } else { 3.0 };

    for x in 0..new_width {
        for dy in 0..new_height {
            let center = (dy as f64 + 0.5) * y_scale - 0.5;
            let top = (center - y_support).ceil().max(0.0) as u32;
            let bottom = (center + y_support)
                .floor()
                .min(h_resized.height as f64 - 1.0) as u32;

            let mut accum = [0.0f64; 4];
            let mut weight_sum = 0.0f64;

            for sy in top..=bottom {
                let dist = (sy as f64 - center) / if y_scale > 1.0 { y_scale } else { 1.0 };
                let w = lanczos3_kernel(dist);
                weight_sum += w;
                let pixel = h_resized.get_pixel_rgba(x, sy);
                for c in 0..4 {
                    accum[c] += pixel[c] as f64 * w;
                }
            }

            if weight_sum.abs() > f64::EPSILON {
                let inv = 1.0 / weight_sum;
                let mut pixel = [0u8; 4];
                for c in 0..4 {
                    pixel[c] = (accum[c] * inv).round().clamp(0.0, 255.0) as u8;
                }
                output.set_pixel(x, dy, &pixel[..ch]);
            }
        }
    }

    output
}

/// Lanczos3 kernel function: `sinc(x) * sinc(x/3)` for `|x| < 3`, else 0.
fn lanczos3_kernel(x: f64) -> f64 {
    let ax = x.abs();
    if ax < f64::EPSILON {
        return 1.0;
    }
    if ax >= 3.0 {
        return 0.0;
    }
    let pi_x = std::f64::consts::PI * x;
    let sinc_x = pi_x.sin() / pi_x;
    let sinc_x3 = (pi_x / 3.0).sin() / (pi_x / 3.0);
    sinc_x * sinc_x3
}

// ============================================================================
// Rotation
// ============================================================================

/// Rotate by 90/180/270 degrees clockwise.
fn apply_rotation(buffer: PixelBuffer, rotation: Rotation) -> Result<PixelBuffer, ProcessingError> {
    let ch = buffer.channels as usize;

    match rotation {
        Rotation::Deg0 => Ok(buffer),
        Rotation::Deg90 => {
            let mut output = PixelBuffer::new(buffer.height, buffer.width, buffer.channels);
            for y in 0..buffer.height {
                for x in 0..buffer.width {
                    let new_x = buffer.height - 1 - y;
                    let new_y = x;
                    if let Some(p) = buffer.get_pixel(x, y) {
                        output.set_pixel(new_x, new_y, &p[..ch]);
                    }
                }
            }
            Ok(output)
        }
        Rotation::Deg180 => {
            let mut output = PixelBuffer::new(buffer.width, buffer.height, buffer.channels);
            for y in 0..buffer.height {
                for x in 0..buffer.width {
                    if let Some(p) = buffer.get_pixel(x, y) {
                        output.set_pixel(buffer.width - 1 - x, buffer.height - 1 - y, &p[..ch]);
                    }
                }
            }
            Ok(output)
        }
        Rotation::Deg270 => {
            let mut output = PixelBuffer::new(buffer.height, buffer.width, buffer.channels);
            for y in 0..buffer.height {
                for x in 0..buffer.width {
                    let new_x = y;
                    let new_y = buffer.width - 1 - x;
                    if let Some(p) = buffer.get_pixel(x, y) {
                        output.set_pixel(new_x, new_y, &p[..ch]);
                    }
                }
            }
            Ok(output)
        }
        Rotation::Auto => {
            // Auto rotation based on EXIF -- treated as no-op at pixel level
            // (EXIF handling is external)
            Ok(buffer)
        }
    }
}

// ============================================================================
// Color adjustments
// ============================================================================

/// Adjust brightness. Value range: -1.0 (black) to 1.0 (white).
///
/// Per-channel formula: `new = clamp(old + value * 255, 0, 255)`.
/// Alpha channel is preserved unchanged.
fn apply_brightness(mut buffer: PixelBuffer, value: f64) -> Result<PixelBuffer, ProcessingError> {
    let offset = (value * 255.0).round() as i16;
    let ch = buffer.channels as usize;
    let color_ch = if ch >= 4 { 3 } else { ch };

    for pixel in buffer.data.chunks_exact_mut(ch) {
        for c in 0..color_ch {
            let v = pixel[c] as i16 + offset;
            pixel[c] = v.clamp(0, 255) as u8;
        }
    }
    Ok(buffer)
}

/// Adjust contrast. Value range: -1.0 (flat gray) to 1.0 (maximum contrast).
///
/// Per-channel formula: `new = clamp((old - 128) * (1 + value) + 128, 0, 255)`.
/// Alpha channel is preserved unchanged.
fn apply_contrast(mut buffer: PixelBuffer, value: f64) -> Result<PixelBuffer, ProcessingError> {
    let factor = 1.0 + value;
    let ch = buffer.channels as usize;
    let color_ch = if ch >= 4 { 3 } else { ch };

    for pixel in buffer.data.chunks_exact_mut(ch) {
        for c in 0..color_ch {
            let v = ((pixel[c] as f64 - 128.0) * factor + 128.0).round();
            pixel[c] = v.clamp(0.0, 255.0) as u8;
        }
    }
    Ok(buffer)
}

/// Apply gamma correction using a 256-entry lookup table.
///
/// Per-channel formula: `new = 255 * (old / 255) ^ (1 / gamma)`.
/// Alpha channel is preserved unchanged.
fn apply_gamma(mut buffer: PixelBuffer, gamma: f64) -> Result<PixelBuffer, ProcessingError> {
    if gamma <= 0.0 {
        return Err(ProcessingError::ProcessingFailed(
            "gamma must be positive".to_string(),
        ));
    }

    let inv_gamma = 1.0 / gamma;
    let mut lut = [0u8; 256];
    for (i, entry) in lut.iter_mut().enumerate() {
        let normalized = i as f64 / 255.0;
        *entry = (255.0 * normalized.powf(inv_gamma))
            .round()
            .clamp(0.0, 255.0) as u8;
    }

    let ch = buffer.channels as usize;
    let color_ch = if ch >= 4 { 3 } else { ch };

    for pixel in buffer.data.chunks_exact_mut(ch) {
        for c in 0..color_ch {
            pixel[c] = lut[pixel[c] as usize];
        }
    }
    Ok(buffer)
}

// ============================================================================
// Blur
// ============================================================================

/// Gaussian blur with given sigma (radius).
///
/// Uses a separable 2-pass approach (horizontal then vertical) for O(n*k)
/// complexity. Kernel size = `ceil(sigma * 3) * 2 + 1`.
pub fn gaussian_blur(buffer: &PixelBuffer, sigma: f64) -> Result<PixelBuffer, ProcessingError> {
    apply_blur(buffer.clone(), sigma)
}

/// Internal blur implementation.
fn apply_blur(buffer: PixelBuffer, sigma: f64) -> Result<PixelBuffer, ProcessingError> {
    if sigma <= 0.0 || buffer.width == 0 || buffer.height == 0 {
        return Ok(buffer);
    }

    let kernel = build_gaussian_kernel(sigma);
    let half = kernel.len() / 2;
    let ch = buffer.channels as usize;
    let color_ch = if ch >= 4 { 3 } else { ch };

    // Horizontal pass
    let mut h_blur = PixelBuffer::new(buffer.width, buffer.height, buffer.channels);
    for y in 0..buffer.height {
        for x in 0..buffer.width {
            let mut accum = [0.0f64; 4];
            let mut weight_sum = 0.0f64;

            for (ki, &kw) in kernel.iter().enumerate() {
                let sx = x as i64 + ki as i64 - half as i64;
                let sx = sx.clamp(0, buffer.width as i64 - 1) as u32;
                let pixel = buffer.get_pixel_rgba(sx, y);
                weight_sum += kw;
                for c in 0..color_ch {
                    accum[c] += pixel[c] as f64 * kw;
                }
                if ch >= 4 {
                    accum[3] += pixel[3] as f64 * kw;
                }
            }

            if weight_sum.abs() > f64::EPSILON {
                let inv = 1.0 / weight_sum;
                let mut out_pixel = [0u8; 4];
                for c in 0..color_ch {
                    out_pixel[c] = (accum[c] * inv).round().clamp(0.0, 255.0) as u8;
                }
                out_pixel[3] = if ch >= 4 {
                    (accum[3] * inv).round().clamp(0.0, 255.0) as u8
                } else {
                    255
                };
                h_blur.set_pixel(x, y, &out_pixel[..ch]);
            }
        }
    }

    // Vertical pass
    let mut output = PixelBuffer::new(buffer.width, buffer.height, buffer.channels);
    for y in 0..buffer.height {
        for x in 0..buffer.width {
            let mut accum = [0.0f64; 4];
            let mut weight_sum = 0.0f64;

            for (ki, &kw) in kernel.iter().enumerate() {
                let sy = y as i64 + ki as i64 - half as i64;
                let sy = sy.clamp(0, buffer.height as i64 - 1) as u32;
                let pixel = h_blur.get_pixel_rgba(x, sy);
                weight_sum += kw;
                for c in 0..color_ch {
                    accum[c] += pixel[c] as f64 * kw;
                }
                if ch >= 4 {
                    accum[3] += pixel[3] as f64 * kw;
                }
            }

            if weight_sum.abs() > f64::EPSILON {
                let inv = 1.0 / weight_sum;
                let mut out_pixel = [0u8; 4];
                for c in 0..color_ch {
                    out_pixel[c] = (accum[c] * inv).round().clamp(0.0, 255.0) as u8;
                }
                out_pixel[3] = if ch >= 4 {
                    (accum[3] * inv).round().clamp(0.0, 255.0) as u8
                } else {
                    255
                };
                output.set_pixel(x, y, &out_pixel[..ch]);
            }
        }
    }

    Ok(output)
}

// ============================================================================
// Sharpen
// ============================================================================

/// Unsharp mask sharpening.
///
/// 1. Blur a copy with a small fixed radius (sigma=1.0).
/// 2. Compute detail: `detail = original - blurred`.
/// 3. Blend: `sharpened = original + amount * detail`.
pub fn unsharp_mask(buffer: &PixelBuffer, amount: f64) -> Result<PixelBuffer, ProcessingError> {
    apply_sharpen(buffer.clone(), amount)
}

/// Internal sharpen implementation.
fn apply_sharpen(buffer: PixelBuffer, amount: f64) -> Result<PixelBuffer, ProcessingError> {
    if amount <= 0.0 || buffer.width == 0 || buffer.height == 0 {
        return Ok(buffer);
    }

    let blurred = apply_blur(buffer.clone(), 1.0)?;

    let ch = buffer.channels as usize;
    let color_ch = if ch >= 4 { 3 } else { ch };
    let mut output = buffer.clone();

    for (i, chunk) in output.data.chunks_exact_mut(ch).enumerate() {
        let base_idx = i * ch;
        for c in 0..color_ch {
            let original = buffer.data[base_idx + c] as f64;
            let blur_val = blurred.data[base_idx + c] as f64;
            let sharpened = original + (original - blur_val) * amount;
            chunk[c] = sharpened.round().clamp(0.0, 255.0) as u8;
        }
    }

    Ok(output)
}

// ============================================================================
// Border & Padding
// ============================================================================

/// Add a coloured border around the image.
fn apply_border(buffer: PixelBuffer, border: &Border) -> Result<PixelBuffer, ProcessingError> {
    let new_width = buffer.width.saturating_add(border.left + border.right);
    let new_height = buffer.height.saturating_add(border.top + border.bottom);
    if new_width == buffer.width && new_height == buffer.height {
        return Ok(buffer);
    }

    let ch = buffer.channels as usize;
    let border_pixel = color_to_pixel(&border.color, buffer.channels);
    let mut output = PixelBuffer::new(new_width, new_height, buffer.channels);

    // Fill with border colour
    for y in 0..new_height {
        for x in 0..new_width {
            output.set_pixel(x, y, &border_pixel[..ch]);
        }
    }

    // Copy original image into position
    for y in 0..buffer.height {
        let src_start = y as usize * buffer.stride();
        let row_bytes = buffer.stride();
        let dst_start = (y + border.top) as usize * output.stride() + border.left as usize * ch;
        if src_start + row_bytes <= buffer.data.len() && dst_start + row_bytes <= output.data.len()
        {
            output.data[dst_start..dst_start + row_bytes]
                .copy_from_slice(&buffer.data[src_start..src_start + row_bytes]);
        }
    }

    Ok(output)
}

/// Add padding with a background colour.
///
/// Padding values are fractional (0.0..1.0) relative to the current buffer
/// dimensions.
fn apply_padding(
    buffer: PixelBuffer,
    padding: &Padding,
    bg: Color,
) -> Result<PixelBuffer, ProcessingError> {
    let pad_top = (padding.top * buffer.height as f64).round() as u32;
    let pad_right = (padding.right * buffer.width as f64).round() as u32;
    let pad_bottom = (padding.bottom * buffer.height as f64).round() as u32;
    let pad_left = (padding.left * buffer.width as f64).round() as u32;

    let new_width = buffer.width.saturating_add(pad_left + pad_right);
    let new_height = buffer.height.saturating_add(pad_top + pad_bottom);
    let ch = buffer.channels as usize;
    let bg_pixel = color_to_pixel(&bg, buffer.channels);

    let mut output = PixelBuffer::new(new_width, new_height, buffer.channels);

    // Fill with background colour
    for y in 0..new_height {
        for x in 0..new_width {
            output.set_pixel(x, y, &bg_pixel[..ch]);
        }
    }

    // Copy original into padded position
    for y in 0..buffer.height {
        let src_start = y as usize * buffer.stride();
        let row_bytes = buffer.stride();
        let dst_start = (y + pad_top) as usize * output.stride() + pad_left as usize * ch;
        if src_start + row_bytes <= buffer.data.len() && dst_start + row_bytes <= output.data.len()
        {
            output.data[dst_start..dst_start + row_bytes]
                .copy_from_slice(&buffer.data[src_start..src_start + row_bytes]);
        }
    }

    Ok(output)
}

// ============================================================================
// Helper functions
// ============================================================================

/// Crop a rectangular region from the buffer.
fn crop_region(
    buffer: &PixelBuffer,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<PixelBuffer, ProcessingError> {
    if width == 0 || height == 0 {
        return Ok(PixelBuffer::new(0, 0, buffer.channels));
    }
    if x + width > buffer.width || y + height > buffer.height {
        return Err(ProcessingError::ProcessingFailed(format!(
            "crop region ({x},{y},{width},{height}) exceeds buffer ({}x{})",
            buffer.width, buffer.height
        )));
    }

    let ch = buffer.channels as usize;
    let mut output = PixelBuffer::new(width, height, buffer.channels);
    let src_stride = buffer.stride();
    let dst_stride = output.stride();

    for row in 0..height {
        let src_start = (y + row) as usize * src_stride + x as usize * ch;
        let dst_start = row as usize * dst_stride;
        let row_bytes = width as usize * ch;
        output.data[dst_start..dst_start + row_bytes]
            .copy_from_slice(&buffer.data[src_start..src_start + row_bytes]);
    }

    Ok(output)
}

/// Compute dimensions for "contain" fit: fit within bounds, preserve aspect ratio.
fn fit_contain_dims(src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return (dst_w, dst_h);
    }
    let scale = (dst_w as f64 / src_w as f64).min(dst_h as f64 / src_h as f64);
    let w = (src_w as f64 * scale).round().max(1.0) as u32;
    let h = (src_h as f64 * scale).round().max(1.0) as u32;
    (w, h)
}

/// Compute dimensions for "cover" fit: fill bounds, crop excess.
fn fit_cover_dims(src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return (dst_w, dst_h);
    }
    let scale = (dst_w as f64 / src_w as f64).max(dst_h as f64 / src_h as f64);
    let w = (src_w as f64 * scale).round().max(1.0) as u32;
    let h = (src_h as f64 * scale).round().max(1.0) as u32;
    (w, h)
}

/// Calculate crop rectangle for a given gravity.
///
/// Returns `(x, y, crop_width, crop_height)`.
pub fn calculate_crop_rect(
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    gravity: &Gravity,
) -> (u32, u32, u32, u32) {
    let cw = dst_width.min(src_width);
    let ch = dst_height.min(src_height);
    let excess_x = src_width.saturating_sub(cw);
    let excess_y = src_height.saturating_sub(ch);

    let (gx, gy) = gravity_to_fractions(gravity);
    let x = (excess_x as f64 * gx).round() as u32;
    let y = (excess_y as f64 * gy).round() as u32;

    (x, y, cw, ch)
}

/// Convert gravity to fractional offsets (0.0 - 1.0) for x and y.
fn gravity_to_fractions(gravity: &Gravity) -> (f64, f64) {
    match gravity {
        Gravity::Auto | Gravity::Center | Gravity::Face => (0.5, 0.5),
        Gravity::Top => (0.5, 0.0),
        Gravity::Bottom => (0.5, 1.0),
        Gravity::Left => (0.0, 0.5),
        Gravity::Right => (1.0, 0.5),
        Gravity::TopLeft => (0.0, 0.0),
        Gravity::TopRight => (1.0, 0.0),
        Gravity::BottomLeft => (0.0, 1.0),
        Gravity::BottomRight => (1.0, 1.0),
        Gravity::FocalPoint(x, y) => (*x, *y),
    }
}

/// Pad an image to exact dimensions, centering it on a solid background.
fn pad_to_exact_size(
    buffer: PixelBuffer,
    target_width: u32,
    target_height: u32,
    bg: Color,
) -> Result<PixelBuffer, ProcessingError> {
    let pad_x = target_width.saturating_sub(buffer.width);
    let pad_y = target_height.saturating_sub(buffer.height);
    let left = pad_x / 2;
    let top = pad_y / 2;

    let new_width = buffer.width.saturating_add(pad_x);
    let new_height = buffer.height.saturating_add(pad_y);
    let ch = buffer.channels as usize;
    let bg_pixel = color_to_pixel(&bg, buffer.channels);

    let mut output = PixelBuffer::new(new_width, new_height, buffer.channels);

    for y in 0..new_height {
        for x in 0..new_width {
            output.set_pixel(x, y, &bg_pixel[..ch]);
        }
    }

    for y in 0..buffer.height {
        let src_start = y as usize * buffer.stride();
        let row_bytes = buffer.stride();
        let dst_start = (y + top) as usize * output.stride() + left as usize * ch;
        if src_start + row_bytes <= buffer.data.len() && dst_start + row_bytes <= output.data.len()
        {
            output.data[dst_start..dst_start + row_bytes]
                .copy_from_slice(&buffer.data[src_start..src_start + row_bytes]);
        }
    }

    Ok(output)
}

/// Build a 1-D Gaussian kernel for the given sigma.
fn build_gaussian_kernel(sigma: f64) -> Vec<f64> {
    let radius = (sigma * 3.0).ceil() as usize;
    let size = radius * 2 + 1;
    let two_sigma_sq = 2.0 * sigma * sigma;

    let mut kernel = Vec::with_capacity(size);
    let mut sum = 0.0;
    for i in 0..size {
        let x = i as f64 - radius as f64;
        let val = (-x * x / two_sigma_sq).exp();
        kernel.push(val);
        sum += val;
    }

    if sum.abs() > f64::EPSILON {
        for v in &mut kernel {
            *v /= sum;
        }
    }
    kernel
}

/// Convert a [`Color`] to a pixel array suitable for the given channel count.
fn color_to_pixel(color: &Color, channels: u8) -> [u8; 4] {
    match channels {
        4 => [color.r, color.g, color.b, color.a],
        _ => [color.r, color.g, color.b, 255],
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Test helpers ──

    fn make_test_buffer(width: u32, height: u32) -> PixelBuffer {
        let mut buf = PixelBuffer::new(width, height, 4);
        for y in 0..height {
            for x in 0..width {
                let r = ((x * 255) / width.max(1)) as u8;
                let g = ((y * 255) / height.max(1)) as u8;
                buf.set_pixel(x, y, &[r, g, 128, 255]);
            }
        }
        buf
    }

    fn make_solid_buffer(width: u32, height: u32, color: [u8; 4]) -> PixelBuffer {
        let mut buf = PixelBuffer::new(width, height, 4);
        for y in 0..height {
            for x in 0..width {
                buf.set_pixel(x, y, &color);
            }
        }
        buf
    }

    // ── PixelBuffer construction ──

    #[test]
    fn test_pixel_buffer_new() {
        let buf = PixelBuffer::new(10, 20, 4);
        assert_eq!(buf.width, 10);
        assert_eq!(buf.height, 20);
        assert_eq!(buf.channels, 4);
        assert_eq!(buf.data.len(), 10 * 20 * 4);
    }

    #[test]
    fn test_pixel_buffer_from_rgba_valid() {
        let data = vec![0u8; 4 * 4 * 4];
        assert!(PixelBuffer::from_rgba(data, 4, 4).is_ok());
    }

    #[test]
    fn test_pixel_buffer_from_rgba_invalid() {
        let data = vec![0u8; 10];
        assert!(PixelBuffer::from_rgba(data, 4, 4).is_err());
    }

    #[test]
    fn test_pixel_buffer_from_rgb_valid() {
        let data = vec![0u8; 3 * 3 * 3];
        assert!(PixelBuffer::from_rgb(data, 3, 3).is_ok());
    }

    #[test]
    fn test_pixel_buffer_from_rgb_invalid() {
        let data = vec![0u8; 5];
        assert!(PixelBuffer::from_rgb(data, 3, 3).is_err());
    }

    #[test]
    fn test_get_set_pixel() {
        let mut buf = PixelBuffer::new(4, 4, 4);
        buf.set_pixel(2, 3, &[100, 150, 200, 255]);
        let p = buf.get_pixel(2, 3).expect("pixel exists");
        assert_eq!(p, &[100, 150, 200, 255]);
    }

    #[test]
    fn test_get_pixel_out_of_bounds() {
        let buf = PixelBuffer::new(4, 4, 4);
        assert!(buf.get_pixel(4, 0).is_none());
        assert!(buf.get_pixel(0, 4).is_none());
        assert!(buf.get_pixel(100, 100).is_none());
    }

    #[test]
    fn test_set_pixel_out_of_bounds_noop() {
        let mut buf = PixelBuffer::new(4, 4, 4);
        buf.set_pixel(10, 10, &[255, 0, 0, 255]); // should not panic
    }

    #[test]
    fn test_single_pixel_buffer() {
        let mut buf = PixelBuffer::new(1, 1, 4);
        buf.set_pixel(0, 0, &[42, 84, 126, 255]);
        assert_eq!(buf.get_pixel(0, 0).expect("pixel"), &[42, 84, 126, 255]);
    }

    // ── Bilinear sampling ──

    #[test]
    fn test_sample_bilinear_exact_corners() {
        let mut buf = PixelBuffer::new(2, 2, 4);
        buf.set_pixel(0, 0, &[100, 0, 0, 255]);
        buf.set_pixel(1, 0, &[200, 0, 0, 255]);
        assert_eq!(buf.sample_bilinear(0.0, 0.0)[0], 100);
        assert_eq!(buf.sample_bilinear(1.0, 0.0)[0], 200);
    }

    #[test]
    fn test_sample_bilinear_interpolated() {
        let mut buf = PixelBuffer::new(2, 1, 4);
        buf.set_pixel(0, 0, &[0, 0, 0, 255]);
        buf.set_pixel(1, 0, &[200, 0, 0, 255]);
        let p = buf.sample_bilinear(0.5, 0.0);
        assert!((p[0] as i32 - 100).abs() <= 1);
    }

    #[test]
    fn test_sample_bilinear_empty_buffer() {
        let buf = PixelBuffer::new(0, 0, 4);
        assert_eq!(buf.sample_bilinear(0.0, 0.0), [0, 0, 0, 255]);
    }

    // ── Bilinear resize ──

    #[test]
    fn test_bilinear_resize_identity() {
        let buf = make_test_buffer(10, 10);
        let resized = bilinear_resize(&buf, 10, 10);
        assert_eq!(resized.data, buf.data);
    }

    #[test]
    fn test_bilinear_resize_downscale() {
        let buf = make_test_buffer(100, 100);
        let resized = bilinear_resize(&buf, 50, 50);
        assert_eq!(resized.width, 50);
        assert_eq!(resized.height, 50);
        assert_eq!(resized.data.len(), 50 * 50 * 4);
    }

    #[test]
    fn test_bilinear_resize_upscale() {
        let buf = make_test_buffer(10, 10);
        let resized = bilinear_resize(&buf, 20, 20);
        assert_eq!(resized.width, 20);
        assert_eq!(resized.height, 20);
    }

    #[test]
    fn test_bilinear_resize_to_single_pixel() {
        let buf = make_test_buffer(10, 10);
        let resized = bilinear_resize(&buf, 1, 1);
        assert_eq!(resized.width, 1);
        assert_eq!(resized.height, 1);
    }

    #[test]
    fn test_bilinear_resize_zero_target() {
        let buf = make_test_buffer(10, 10);
        let resized = bilinear_resize(&buf, 0, 0);
        assert_eq!(resized.width, 0);
        assert!(resized.data.is_empty());
    }

    // ── Lanczos resize ──

    #[test]
    fn test_lanczos_resize_downscale() {
        let buf = make_test_buffer(100, 100);
        let resized = lanczos_resize(&buf, 50, 50);
        assert_eq!(resized.width, 50);
        assert_eq!(resized.height, 50);
    }

    #[test]
    fn test_lanczos_resize_identity() {
        let buf = make_test_buffer(10, 10);
        let resized = lanczos_resize(&buf, 10, 10);
        assert_eq!(resized.data, buf.data);
    }

    #[test]
    fn test_lanczos_resize_upscale() {
        let buf = make_test_buffer(10, 10);
        let resized = lanczos_resize(&buf, 30, 30);
        assert_eq!(resized.width, 30);
        assert_eq!(resized.height, 30);
    }

    // ── Fit mode calculations ──

    #[test]
    fn test_fit_contain_landscape() {
        assert_eq!(fit_contain_dims(200, 100, 100, 100), (100, 50));
    }

    #[test]
    fn test_fit_contain_portrait() {
        assert_eq!(fit_contain_dims(100, 200, 100, 100), (50, 100));
    }

    #[test]
    fn test_fit_cover_landscape() {
        assert_eq!(fit_cover_dims(200, 100, 100, 100), (200, 100));
    }

    // ── Resize with fit modes ──

    #[test]
    fn test_apply_resize_scale_down_no_change() {
        let buf = make_test_buffer(50, 50);
        let out = apply_resize(buf, 100, 100, FitMode::ScaleDown, &Gravity::Center).expect("ok");
        assert_eq!(out.width, 50);
        assert_eq!(out.height, 50);
    }

    #[test]
    fn test_apply_resize_scale_down_shrinks() {
        let buf = make_test_buffer(100, 100);
        let out = apply_resize(buf, 50, 50, FitMode::ScaleDown, &Gravity::Center).expect("ok");
        assert_eq!(out.width, 50);
        assert_eq!(out.height, 50);
    }

    #[test]
    fn test_apply_resize_contain() {
        let buf = make_test_buffer(200, 100);
        let out = apply_resize(buf, 100, 100, FitMode::Contain, &Gravity::Center).expect("ok");
        assert_eq!(out.width, 100);
        assert_eq!(out.height, 50);
    }

    #[test]
    fn test_apply_resize_cover() {
        let buf = make_test_buffer(200, 100);
        let out = apply_resize(buf, 100, 100, FitMode::Cover, &Gravity::Center).expect("ok");
        assert_eq!(out.width, 100);
        assert_eq!(out.height, 100);
    }

    #[test]
    fn test_apply_resize_fill() {
        let buf = make_test_buffer(200, 100);
        let out = apply_resize(buf, 50, 75, FitMode::Fill, &Gravity::Center).expect("ok");
        assert_eq!(out.width, 50);
        assert_eq!(out.height, 75);
    }

    #[test]
    fn test_apply_resize_pad() {
        let buf = make_test_buffer(100, 50);
        let out = apply_resize(buf, 100, 100, FitMode::Pad, &Gravity::Center).expect("ok");
        assert_eq!(out.width, 100);
        assert_eq!(out.height, 100);
    }

    #[test]
    fn test_apply_resize_crop() {
        let buf = make_test_buffer(200, 200);
        let out = apply_resize(buf, 100, 100, FitMode::Crop, &Gravity::Center).expect("ok");
        assert_eq!(out.width, 100);
        assert_eq!(out.height, 100);
    }

    #[test]
    fn test_apply_resize_width_only() {
        let buf = make_test_buffer(200, 100);
        let out = apply_resize(buf, 100, 0, FitMode::Contain, &Gravity::Center).expect("ok");
        assert_eq!(out.width, 100);
        assert_eq!(out.height, 50);
    }

    #[test]
    fn test_apply_resize_height_only() {
        let buf = make_test_buffer(200, 100);
        let out = apply_resize(buf, 0, 50, FitMode::Contain, &Gravity::Center).expect("ok");
        assert_eq!(out.width, 100);
        assert_eq!(out.height, 50);
    }

    #[test]
    fn test_apply_resize_empty_source() {
        let buf = make_test_buffer(0, 0);
        let out = apply_resize(buf, 100, 100, FitMode::Contain, &Gravity::Center).expect("ok");
        assert_eq!(out.width, 0);
    }

    // ── Rotation ──

    #[test]
    fn test_rotate_deg0_identity() {
        let buf = make_test_buffer(10, 20);
        let out = apply_rotation(buf.clone(), Rotation::Deg0).expect("ok");
        assert_eq!(out.data, buf.data);
    }

    #[test]
    fn test_rotate_90() {
        let buf = make_test_buffer(10, 20);
        let out = apply_rotation(buf, Rotation::Deg90).expect("ok");
        assert_eq!(out.width, 20);
        assert_eq!(out.height, 10);
    }

    #[test]
    fn test_rotate_180() {
        let buf = make_test_buffer(10, 20);
        let out = apply_rotation(buf, Rotation::Deg180).expect("ok");
        assert_eq!(out.width, 10);
        assert_eq!(out.height, 20);
    }

    #[test]
    fn test_rotate_270() {
        let buf = make_test_buffer(10, 20);
        let out = apply_rotation(buf, Rotation::Deg270).expect("ok");
        assert_eq!(out.width, 20);
        assert_eq!(out.height, 10);
    }

    #[test]
    fn test_rotate_auto_noop() {
        let buf = make_test_buffer(10, 20);
        let out = apply_rotation(buf.clone(), Rotation::Auto).expect("ok");
        assert_eq!(out.data, buf.data);
    }

    #[test]
    fn test_rotate_90_pixel_mapping() {
        let mut buf = PixelBuffer::new(3, 2, 4);
        buf.set_pixel(0, 0, &[255, 0, 0, 255]);
        let rotated = apply_rotation(buf, Rotation::Deg90).expect("ok");
        assert_eq!(rotated.width, 2);
        assert_eq!(rotated.height, 3);
        assert_eq!(rotated.get_pixel(1, 0).expect("pixel")[0], 255);
    }

    #[test]
    fn test_rotate_single_pixel() {
        let mut buf = PixelBuffer::new(1, 1, 4);
        buf.set_pixel(0, 0, &[42, 84, 126, 200]);
        for rot in [Rotation::Deg90, Rotation::Deg180, Rotation::Deg270] {
            let rotated = apply_rotation(buf.clone(), rot).expect("ok");
            assert_eq!(rotated.get_pixel(0, 0).expect("pixel"), &[42, 84, 126, 200]);
        }
    }

    // ── Brightness ──

    #[test]
    fn test_brightness_zero_is_noop() {
        let buf = make_solid_buffer(4, 4, [128, 128, 128, 255]);
        let out = apply_brightness(buf.clone(), 0.0).expect("ok");
        assert_eq!(out.data, buf.data);
    }

    #[test]
    fn test_brightness_positive() {
        let buf = make_solid_buffer(2, 2, [100, 100, 100, 255]);
        let out = apply_brightness(buf, 0.5).expect("ok");
        let p = out.get_pixel(0, 0).expect("pixel");
        assert_eq!(p[0], 228); // 100 + round(0.5*255) = 100 + 128 = 228
        assert_eq!(p[3], 255); // alpha unchanged
    }

    #[test]
    fn test_brightness_negative_clamps_to_zero() {
        let buf = make_solid_buffer(2, 2, [100, 100, 100, 255]);
        let out = apply_brightness(buf, -0.5).expect("ok");
        assert_eq!(out.get_pixel(0, 0).expect("pixel")[0], 0);
    }

    #[test]
    fn test_brightness_clamp_to_white() {
        let buf = make_solid_buffer(2, 2, [200, 200, 200, 255]);
        let out = apply_brightness(buf, 1.0).expect("ok");
        assert_eq!(out.get_pixel(0, 0).expect("pixel")[0], 255);
    }

    // ── Contrast ──

    #[test]
    fn test_contrast_zero_is_noop() {
        let buf = make_solid_buffer(4, 4, [128, 128, 128, 255]);
        let out = apply_contrast(buf.clone(), 0.0).expect("ok");
        assert_eq!(out.data, buf.data);
    }

    #[test]
    fn test_contrast_positive_amplifies() {
        let buf = make_solid_buffer(2, 2, [200, 200, 200, 255]);
        let out = apply_contrast(buf, 0.5).expect("ok");
        assert_eq!(out.get_pixel(0, 0).expect("pixel")[0], 236);
    }

    #[test]
    fn test_contrast_negative_reduces() {
        let buf = make_solid_buffer(2, 2, [200, 200, 200, 255]);
        let out = apply_contrast(buf, -0.5).expect("ok");
        assert_eq!(out.get_pixel(0, 0).expect("pixel")[0], 164);
    }

    #[test]
    fn test_contrast_midpoint_unchanged() {
        let buf = make_solid_buffer(2, 2, [128, 128, 128, 255]);
        let out = apply_contrast(buf, 1.0).expect("ok");
        assert_eq!(out.get_pixel(0, 0).expect("pixel")[0], 128);
    }

    // ── Gamma ──

    #[test]
    fn test_gamma_identity() {
        let buf = make_solid_buffer(2, 2, [100, 100, 100, 255]);
        let out = apply_gamma(buf, 1.0).expect("ok");
        assert_eq!(out.get_pixel(0, 0).expect("pixel")[0], 100);
    }

    #[test]
    fn test_gamma_brightens() {
        let buf = make_solid_buffer(2, 2, [100, 100, 100, 255]);
        let out = apply_gamma(buf, 2.2).expect("ok");
        assert!(out.get_pixel(0, 0).expect("pixel")[0] > 100);
    }

    #[test]
    fn test_gamma_darkens() {
        let buf = make_solid_buffer(2, 2, [200, 200, 200, 255]);
        let out = apply_gamma(buf, 0.5).expect("ok");
        assert!(out.get_pixel(0, 0).expect("pixel")[0] < 200);
    }

    #[test]
    fn test_gamma_zero_error() {
        let buf = make_solid_buffer(2, 2, [100, 100, 100, 255]);
        assert!(apply_gamma(buf, 0.0).is_err());
    }

    #[test]
    fn test_gamma_preserves_alpha() {
        let buf = make_solid_buffer(2, 2, [100, 100, 100, 128]);
        let out = apply_gamma(buf, 2.2).expect("ok");
        assert_eq!(out.get_pixel(0, 0).expect("pixel")[3], 128);
    }

    #[test]
    fn test_gamma_fixed_points_black_white() {
        let mut buf = PixelBuffer::new(2, 1, 4);
        buf.set_pixel(0, 0, &[0, 0, 0, 255]);
        buf.set_pixel(1, 0, &[255, 255, 255, 255]);
        let out = apply_gamma(buf, 2.2).expect("ok");
        assert_eq!(out.get_pixel(0, 0).expect("pixel")[0], 0);
        assert_eq!(out.get_pixel(1, 0).expect("pixel")[0], 255);
    }

    // ── Blur ──

    #[test]
    fn test_blur_zero_is_noop() {
        let buf = make_test_buffer(10, 10);
        let out = apply_blur(buf.clone(), 0.0).expect("ok");
        assert_eq!(out.data, buf.data);
    }

    #[test]
    fn test_blur_preserves_dimensions() {
        let buf = make_test_buffer(50, 30);
        let out = apply_blur(buf, 5.0).expect("ok");
        assert_eq!(out.width, 50);
        assert_eq!(out.height, 30);
    }

    #[test]
    fn test_blur_solid_color_unchanged() {
        let buf = make_solid_buffer(10, 10, [128, 128, 128, 255]);
        let out = apply_blur(buf, 3.0).expect("ok");
        let p = out.get_pixel(5, 5).expect("pixel");
        assert_eq!(p[0], 128);
    }

    #[test]
    fn test_blur_reduces_sharp_edge() {
        let mut buf = PixelBuffer::new(20, 1, 4);
        for x in 0..10 {
            buf.set_pixel(x, 0, &[0, 0, 0, 255]);
        }
        for x in 10..20 {
            buf.set_pixel(x, 0, &[255, 255, 255, 255]);
        }
        let out = apply_blur(buf, 2.0).expect("ok");
        assert!(out.get_pixel(9, 0).expect("pixel")[0] > 0);
        assert!(out.get_pixel(10, 0).expect("pixel")[0] < 255);
    }

    // ── Sharpen ──

    #[test]
    fn test_sharpen_zero_is_noop() {
        let buf = make_test_buffer(10, 10);
        let out = apply_sharpen(buf.clone(), 0.0).expect("ok");
        assert_eq!(out.data, buf.data);
    }

    #[test]
    fn test_sharpen_preserves_dimensions() {
        let buf = make_test_buffer(20, 20);
        let out = apply_sharpen(buf, 1.0).expect("ok");
        assert_eq!(out.width, 20);
        assert_eq!(out.height, 20);
    }

    #[test]
    fn test_sharpen_solid_color_unchanged() {
        let buf = make_solid_buffer(10, 10, [128, 128, 128, 255]);
        let out = apply_sharpen(buf, 2.0).expect("ok");
        let p = out.get_pixel(5, 5).expect("pixel");
        assert!((p[0] as i32 - 128).abs() <= 1);
    }

    // ── Trim ──

    #[test]
    fn test_trim_all_sides() {
        let buf = make_test_buffer(20, 20);
        let trim = Trim {
            top: 2,
            right: 3,
            bottom: 4,
            left: 5,
        };
        let out = apply_trim(buf, &trim).expect("ok");
        assert_eq!(out.width, 12); // 20 - 5 - 3
        assert_eq!(out.height, 14); // 20 - 2 - 4
    }

    #[test]
    fn test_trim_uniform() {
        let buf = make_test_buffer(20, 20);
        let trim = Trim::uniform(3);
        let out = apply_trim(buf, &trim).expect("ok");
        assert_eq!(out.width, 14);
        assert_eq!(out.height, 14);
    }

    #[test]
    fn test_trim_exceeds_dimensions() {
        let buf = make_test_buffer(10, 10);
        let trim = Trim::uniform(100);
        let out = apply_trim(buf, &trim).expect("ok");
        assert_eq!(out.width, 0);
    }

    #[test]
    fn test_trim_empty_buffer() {
        let buf = PixelBuffer::new(0, 0, 4);
        let out = apply_trim(buf, &Trim::uniform(5)).expect("ok");
        assert_eq!(out.width, 0);
    }

    // ── Border ──

    #[test]
    fn test_border_adds_size() {
        let buf = make_test_buffer(10, 10);
        let border = Border::uniform(5, Color::new(255, 0, 0, 255));
        let out = apply_border(buf, &border).expect("ok");
        assert_eq!(out.width, 20);
        assert_eq!(out.height, 20);
    }

    #[test]
    fn test_border_color_applied() {
        let buf = make_test_buffer(4, 4);
        let border = Border::uniform(2, Color::new(255, 0, 0, 255));
        let out = apply_border(buf, &border).expect("ok");
        let p = out.get_pixel(0, 0).expect("pixel");
        assert_eq!(p[0], 255);
        assert_eq!(p[1], 0);
        assert_eq!(p[2], 0);
    }

    #[test]
    fn test_border_asymmetric() {
        let buf = make_test_buffer(10, 10);
        let border = Border {
            color: Color::black(),
            top: 1,
            right: 2,
            bottom: 3,
            left: 4,
        };
        let out = apply_border(buf, &border).expect("ok");
        assert_eq!(out.width, 16); // 10 + 4 + 2
        assert_eq!(out.height, 14); // 10 + 1 + 3
    }

    // ── Padding ──

    #[test]
    fn test_padding_uniform() {
        let buf = make_test_buffer(100, 100);
        let padding = Padding::uniform(0.1); // 10% each side
        let out = apply_padding(buf, &padding, Color::white()).expect("ok");
        assert_eq!(out.width, 120); // 100 + 10 + 10
        assert_eq!(out.height, 120);
    }

    #[test]
    fn test_padding_asymmetric() {
        let buf = make_test_buffer(100, 100);
        let padding = Padding {
            top: 0.05,
            right: 0.1,
            bottom: 0.15,
            left: 0.2,
        };
        let out = apply_padding(buf, &padding, Color::black()).expect("ok");
        // left: round(0.2*100)=20, right: round(0.1*100)=10 -> 130
        assert_eq!(out.width, 130);
        // top: round(0.05*100)=5, bottom: round(0.15*100)=15 -> 120
        assert_eq!(out.height, 120);
    }

    // ── Crop rect calculations ──

    #[test]
    fn test_crop_rect_center() {
        let (x, y, w, h) = calculate_crop_rect(200, 200, 100, 100, &Gravity::Center);
        assert_eq!((x, y, w, h), (50, 50, 100, 100));
    }

    #[test]
    fn test_crop_rect_top_left() {
        let (x, y, w, h) = calculate_crop_rect(200, 200, 100, 100, &Gravity::TopLeft);
        assert_eq!((x, y, w, h), (0, 0, 100, 100));
    }

    #[test]
    fn test_crop_rect_bottom_right() {
        let (x, y, w, h) = calculate_crop_rect(200, 200, 100, 100, &Gravity::BottomRight);
        assert_eq!((x, y, w, h), (100, 100, 100, 100));
    }

    #[test]
    fn test_crop_rect_focal_point() {
        let gravity = Gravity::FocalPoint(0.25, 0.75);
        let (x, y, w, h) = calculate_crop_rect(200, 200, 100, 100, &gravity);
        assert_eq!((w, h), (100, 100));
        assert_eq!(x, 25);
        assert_eq!(y, 75);
    }

    #[test]
    fn test_crop_rect_larger_than_source() {
        let (x, y, w, h) = calculate_crop_rect(50, 50, 100, 100, &Gravity::Center);
        assert_eq!((x, y, w, h), (0, 0, 50, 50));
    }

    // ── Gravity fractions ──

    #[test]
    fn test_gravity_to_fractions() {
        assert_eq!(gravity_to_fractions(&Gravity::TopLeft), (0.0, 0.0));
        assert_eq!(gravity_to_fractions(&Gravity::Center), (0.5, 0.5));
        assert_eq!(gravity_to_fractions(&Gravity::BottomRight), (1.0, 1.0));
        assert_eq!(gravity_to_fractions(&Gravity::Top), (0.5, 0.0));
        assert_eq!(gravity_to_fractions(&Gravity::Bottom), (0.5, 1.0));
        assert_eq!(gravity_to_fractions(&Gravity::Left), (0.0, 0.5));
        assert_eq!(gravity_to_fractions(&Gravity::Right), (1.0, 0.5));
    }

    #[test]
    fn test_gravity_focal_point_fractions() {
        let (fx, fy) = gravity_to_fractions(&Gravity::FocalPoint(0.3, 0.7));
        assert!((fx - 0.3).abs() < 1e-6);
        assert!((fy - 0.7).abs() < 1e-6);
    }

    // ── Gaussian kernel ──

    #[test]
    fn test_gaussian_kernel_sums_to_one() {
        let kernel = build_gaussian_kernel(2.0);
        let sum: f64 = kernel.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_gaussian_kernel_symmetric() {
        let kernel = build_gaussian_kernel(3.0);
        let n = kernel.len();
        for i in 0..n / 2 {
            assert!((kernel[i] - kernel[n - 1 - i]).abs() < 1e-10);
        }
    }

    #[test]
    fn test_gaussian_kernel_peak_at_center() {
        let kernel = build_gaussian_kernel(2.0);
        let center = kernel.len() / 2;
        for (i, &v) in kernel.iter().enumerate() {
            if i != center {
                assert!(v <= kernel[center]);
            }
        }
    }

    // ── Lanczos kernel ──

    #[test]
    fn test_lanczos3_kernel_at_zero() {
        assert!((lanczos3_kernel(0.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_lanczos3_kernel_at_boundary() {
        assert!(lanczos3_kernel(3.0).abs() < 1e-10);
        assert!(lanczos3_kernel(-3.0).abs() < 1e-10);
    }

    #[test]
    fn test_lanczos3_kernel_outside_support() {
        assert_eq!(lanczos3_kernel(4.0), 0.0);
        assert_eq!(lanczos3_kernel(-5.0), 0.0);
    }

    // ── Pipeline building and ordering ──

    #[test]
    fn test_build_pipeline_empty() {
        let params = TransformParams::default();
        let pipeline = build_pipeline(&params, OutputFormat::Auto);
        assert!(pipeline.is_empty());
    }

    #[test]
    fn test_build_pipeline_ordering() {
        let mut params = TransformParams::default();
        params.trim = Some(Trim::uniform(5));
        params.width = Some(100);
        params.height = Some(100);
        params.rotate = Rotation::Deg90;
        params.brightness = 0.5;
        params.contrast = 0.2;
        params.gamma = 2.2;
        params.sharpen = 1.0;
        params.blur = 2.0;
        params.border = Some(Border::uniform(5, Color::black()));
        params.pad = Some(Padding::uniform(0.1));

        let pipeline = build_pipeline(&params, OutputFormat::Auto);
        assert!(pipeline.len() >= 10);

        assert!(matches!(pipeline[0], PipelineStep::Trim(_)));
        assert!(matches!(pipeline[1], PipelineStep::Resize { .. }));
        assert!(matches!(pipeline[2], PipelineStep::Rotate(_)));
        assert!(matches!(pipeline[3], PipelineStep::Brightness(_)));
        assert!(matches!(pipeline[4], PipelineStep::Contrast(_)));
        assert!(matches!(pipeline[5], PipelineStep::Gamma(_)));
        assert!(matches!(pipeline[6], PipelineStep::Sharpen(_)));
        assert!(matches!(pipeline[7], PipelineStep::Blur(_)));
        assert!(matches!(pipeline[8], PipelineStep::AddBorder(_)));
        assert!(matches!(pipeline[9], PipelineStep::AddPadding(_, _)));
    }

    #[test]
    fn test_build_pipeline_skips_identity_values() {
        let params = TransformParams::default();
        let pipeline = build_pipeline(&params, OutputFormat::Auto);
        assert!(pipeline.is_empty());
    }

    // ── Full pipeline integration ──

    #[test]
    fn test_apply_transforms_identity() {
        let mut buf = make_test_buffer(50, 50);
        let params = TransformParams::default();
        let out = apply_transforms(&mut buf, &params).expect("ok");
        assert_eq!(out.data, buf.data);
    }

    #[test]
    fn test_apply_transforms_resize_and_rotate() {
        let mut buf = make_test_buffer(100, 50);
        let mut params = TransformParams::default();
        params.width = Some(50);
        params.height = Some(25);
        params.fit = FitMode::Fill;
        params.rotate = Rotation::Deg90;

        let out = apply_transforms(&mut buf, &params).expect("ok");
        assert_eq!(out.width, 25);
        assert_eq!(out.height, 50);
    }

    #[test]
    fn test_apply_transforms_color_adjustments() {
        let mut buf = make_solid_buffer(10, 10, [128, 128, 128, 255]);
        let mut params = TransformParams::default();
        params.brightness = 0.1;
        params.contrast = 0.2;
        params.gamma = 1.5;
        assert!(apply_transforms(&mut buf, &params).is_ok());
    }

    #[test]
    fn test_apply_transforms_border_and_padding() {
        let mut buf = make_test_buffer(10, 10);
        let mut params = TransformParams::default();
        params.border = Some(Border::uniform(2, Color::new(255, 0, 0, 255)));
        params.pad = Some(Padding::uniform(0.5)); // 50% of current dims

        let out = apply_transforms(&mut buf, &params).expect("ok");
        // After border: 14x14
        // Padding: 50% of 14 = 7 each side -> 14 + 14 = 28
        assert_eq!(out.width, 28);
        assert_eq!(out.height, 28);
    }

    // ── Edge cases ──

    #[test]
    fn test_large_resize_from_single_pixel() {
        let buf = make_test_buffer(1, 1);
        let out = apply_resize(buf, 1000, 1000, FitMode::Fill, &Gravity::Center).expect("ok");
        assert_eq!(out.width, 1000);
        assert_eq!(out.height, 1000);
    }

    #[test]
    fn test_rgb_buffer_brightness() {
        let data = vec![128u8; 10 * 10 * 3];
        let buf = PixelBuffer::from_rgb(data, 10, 10).expect("valid");
        let out = apply_brightness(buf, 0.1).expect("ok");
        assert_eq!(out.channels, 3);
        assert!(out.data[0] > 128);
    }

    #[test]
    fn test_rgb_buffer_resize() {
        let data = vec![128u8; 10 * 10 * 3];
        let buf = PixelBuffer::from_rgb(data, 10, 10).expect("valid");
        let resized = bilinear_resize(&buf, 5, 5);
        assert_eq!(resized.width, 5);
        assert_eq!(resized.channels, 3);
    }

    #[test]
    fn test_negative_brightness_per_channel() {
        let buf = make_solid_buffer(4, 4, [50, 100, 150, 255]);
        let out = apply_brightness(buf, -0.3).expect("ok");
        let p = out.get_pixel(0, 0).expect("pixel");
        assert_eq!(p[0], 0); // 50 - 77 < 0 -> 0
        assert_eq!(p[1], 23); // 100 - 77
        assert_eq!(p[2], 73); // 150 - 77
    }
}
