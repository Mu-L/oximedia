//! NDI codec implementations
//!
//! This module provides codec support for NDI, including SpeedHQ compression
//! and YUV422 format handling.
#![allow(dead_code)]

use crate::{NdiError, Result};
use bytes::{Bytes, BytesMut};
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::{Read, Write};
use tracing::{debug, trace};

/// YUV format types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YuvFormat {
    /// YUV 4:2:2 (2 bytes per pixel)
    Yuv422,

    /// YUV 4:2:0 (1.5 bytes per pixel)
    Yuv420,

    /// YUV 4:4:4 (3 bytes per pixel)
    Yuv444,

    /// UYVY (packed YUV 4:2:2)
    Uyvy,

    /// YUYV (packed YUV 4:2:2)
    Yuyv,
}

impl YuvFormat {
    /// Get the number of bytes per pixel for this format
    pub fn bytes_per_pixel(&self) -> f32 {
        match self {
            Self::Yuv422 | Self::Uyvy | Self::Yuyv => 2.0,
            Self::Yuv420 => 1.5,
            Self::Yuv444 => 3.0,
        }
    }

    /// Calculate the buffer size needed for a frame
    pub fn buffer_size(&self, width: u32, height: u32) -> usize {
        (width as f32 * height as f32 * self.bytes_per_pixel()) as usize
    }
}

/// SpeedHQ codec for NDI HX
///
/// This is a simplified implementation of the SpeedHQ codec used in NDI HX mode.
/// For production use, this should be replaced with a proper implementation or
/// hardware acceleration.
pub struct SpeedHqCodec {
    /// Compression quality (0-100)
    quality: u8,

    /// Enable fast mode (lower quality, faster)
    fast_mode: bool,
}

impl SpeedHqCodec {
    /// Create a new SpeedHQ codec
    pub fn new(quality: u8) -> Self {
        Self {
            quality: quality.min(100),
            fast_mode: false,
        }
    }

    /// Create a codec with fast mode enabled
    pub fn new_fast(quality: u8) -> Self {
        Self {
            quality: quality.min(100),
            fast_mode: true,
        }
    }

    /// Compress YUV data
    ///
    /// This is a placeholder implementation using deflate compression.
    /// A real implementation would use proper SpeedHQ compression.
    pub fn compress(&self, data: &[u8], width: u32, height: u32) -> Result<Bytes> {
        trace!(
            "Compressing {}x{} frame with quality {}",
            width,
            height,
            self.quality
        );

        // Calculate compression level based on quality
        let compression_level = if self.fast_mode {
            Compression::fast()
        } else {
            let level = (self.quality as u32 * 9) / 100;
            Compression::new(level)
        };

        // Compress using deflate (placeholder for real SpeedHQ)
        let mut encoder = DeflateEncoder::new(Vec::new(), compression_level);
        encoder
            .write_all(data)
            .map_err(|e| NdiError::Codec(format!("Compression failed: {}", e)))?;

        let compressed = encoder
            .finish()
            .map_err(|e| NdiError::Codec(format!("Compression finalization failed: {}", e)))?;

        debug!(
            "Compressed {} bytes to {} bytes ({:.1}% ratio)",
            data.len(),
            compressed.len(),
            (compressed.len() as f64 / data.len() as f64) * 100.0
        );

        Ok(Bytes::from(compressed))
    }

    /// Decompress YUV data
    ///
    /// This is a placeholder implementation using deflate decompression.
    /// A real implementation would use proper SpeedHQ decompression.
    pub fn decompress(&self, data: &[u8], expected_size: usize) -> Result<Bytes> {
        trace!(
            "Decompressing {} bytes (expected {})",
            data.len(),
            expected_size
        );

        let mut decoder = DeflateDecoder::new(data);
        let mut decompressed = Vec::with_capacity(expected_size);

        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| NdiError::Codec(format!("Decompression failed: {}", e)))?;

        if decompressed.len() != expected_size {
            return Err(NdiError::Codec(format!(
                "Decompressed size mismatch: expected {}, got {}",
                expected_size,
                decompressed.len()
            )));
        }

        debug!("Decompressed {} bytes", decompressed.len());
        Ok(Bytes::from(decompressed))
    }

    /// Set compression quality (0-100)
    pub fn set_quality(&mut self, quality: u8) {
        self.quality = quality.min(100);
    }

    /// Get current quality setting
    pub fn quality(&self) -> u8 {
        self.quality
    }

    /// Enable or disable fast mode
    pub fn set_fast_mode(&mut self, enabled: bool) {
        self.fast_mode = enabled;
    }

    /// Check if fast mode is enabled
    pub fn is_fast_mode(&self) -> bool {
        self.fast_mode
    }
}

impl Default for SpeedHqCodec {
    fn default() -> Self {
        Self::new(80)
    }
}

/// YUV format converter
pub struct YuvConverter;

impl YuvConverter {
    /// Convert RGB to YUV422
    pub fn rgb_to_yuv422(rgb: &[u8], width: u32, height: u32) -> Result<Bytes> {
        if rgb.len() < (width * height * 3) as usize {
            return Err(NdiError::InvalidFrameFormat);
        }

        let mut yuv = BytesMut::with_capacity(YuvFormat::Yuv422.buffer_size(width, height));

        for y in 0..height {
            for x in 0..(width / 2) {
                let idx = ((y * width + x * 2) * 3) as usize;

                let r0 = i32::from(rgb[idx]);
                let g0 = i32::from(rgb[idx + 1]);
                let b0 = i32::from(rgb[idx + 2]);

                let r1 = i32::from(rgb[idx + 3]);
                let g1 = i32::from(rgb[idx + 4]);
                let b1 = i32::from(rgb[idx + 5]);

                // Convert to YUV
                let y0 = Self::rgb_to_y(r0, g0, b0);
                let y1 = Self::rgb_to_y(r1, g1, b1);
                let u = Self::rgb_to_u(r0, g0, b0);
                let v = Self::rgb_to_v(r0, g0, b0);

                yuv.extend_from_slice(&[y0, u, y1, v]);
            }
        }

        Ok(yuv.freeze())
    }

    /// Convert YUV422 to RGB
    pub fn yuv422_to_rgb(yuv: &[u8], width: u32, height: u32) -> Result<Bytes> {
        if yuv.len() < YuvFormat::Yuv422.buffer_size(width, height) {
            return Err(NdiError::InvalidFrameFormat);
        }

        let mut rgb = BytesMut::with_capacity((width * height * 3) as usize);

        for y in 0..height {
            for x in 0..(width / 2) {
                let idx = ((y * width + x * 2) * 2) as usize;

                let y0 = i32::from(yuv[idx]);
                let u = i32::from(yuv[idx + 1]);
                let y1 = i32::from(yuv[idx + 2]);
                let v = i32::from(yuv[idx + 3]);

                // Convert first pixel
                let (r0, g0, b0) = Self::yuv_to_rgb(y0, u, v);
                rgb.extend_from_slice(&[r0, g0, b0]);

                // Convert second pixel
                let (r1, g1, b1) = Self::yuv_to_rgb(y1, u, v);
                rgb.extend_from_slice(&[r1, g1, b1]);
            }
        }

        Ok(rgb.freeze())
    }

    /// Convert RGB to YUV420
    pub fn rgb_to_yuv420(rgb: &[u8], width: u32, height: u32) -> Result<Bytes> {
        if rgb.len() < (width * height * 3) as usize {
            return Err(NdiError::InvalidFrameFormat);
        }

        let mut yuv = BytesMut::with_capacity(YuvFormat::Yuv420.buffer_size(width, height));

        // Y plane
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 3) as usize;
                let r = i32::from(rgb[idx]);
                let g = i32::from(rgb[idx + 1]);
                let b = i32::from(rgb[idx + 2]);
                yuv.extend_from_slice(&[Self::rgb_to_y(r, g, b)]);
            }
        }

        // U plane (subsampled)
        for y in (0..height).step_by(2) {
            for x in (0..width).step_by(2) {
                let idx = ((y * width + x) * 3) as usize;
                let r = i32::from(rgb[idx]);
                let g = i32::from(rgb[idx + 1]);
                let b = i32::from(rgb[idx + 2]);
                yuv.extend_from_slice(&[Self::rgb_to_u(r, g, b)]);
            }
        }

        // V plane (subsampled)
        for y in (0..height).step_by(2) {
            for x in (0..width).step_by(2) {
                let idx = ((y * width + x) * 3) as usize;
                let r = i32::from(rgb[idx]);
                let g = i32::from(rgb[idx + 1]);
                let b = i32::from(rgb[idx + 2]);
                yuv.extend_from_slice(&[Self::rgb_to_v(r, g, b)]);
            }
        }

        Ok(yuv.freeze())
    }

    /// Convert YUV420 to RGB
    pub fn yuv420_to_rgb(yuv: &[u8], width: u32, height: u32) -> Result<Bytes> {
        if yuv.len() < YuvFormat::Yuv420.buffer_size(width, height) {
            return Err(NdiError::InvalidFrameFormat);
        }

        let mut rgb = BytesMut::with_capacity((width * height * 3) as usize);

        let y_plane_size = (width * height) as usize;
        let u_plane_size = (width * height / 4) as usize;

        for y in 0..height {
            for x in 0..width {
                let y_idx = (y * width + x) as usize;
                let uv_idx = ((y / 2) * (width / 2) + (x / 2)) as usize;

                let y_val = i32::from(yuv[y_idx]);
                let u_val = i32::from(yuv[y_plane_size + uv_idx]);
                let v_val = i32::from(yuv[y_plane_size + u_plane_size + uv_idx]);

                let (r, g, b) = Self::yuv_to_rgb(y_val, u_val, v_val);
                rgb.extend_from_slice(&[r, g, b]);
            }
        }

        Ok(rgb.freeze())
    }

    /// Convert RGB to UYVY
    pub fn rgb_to_uyvy(rgb: &[u8], width: u32, height: u32) -> Result<Bytes> {
        if rgb.len() < (width * height * 3) as usize {
            return Err(NdiError::InvalidFrameFormat);
        }

        let mut uyvy = BytesMut::with_capacity(YuvFormat::Uyvy.buffer_size(width, height));

        for y in 0..height {
            for x in 0..(width / 2) {
                let idx = ((y * width + x * 2) * 3) as usize;

                let r0 = i32::from(rgb[idx]);
                let g0 = i32::from(rgb[idx + 1]);
                let b0 = i32::from(rgb[idx + 2]);

                let r1 = i32::from(rgb[idx + 3]);
                let g1 = i32::from(rgb[idx + 4]);
                let b1 = i32::from(rgb[idx + 5]);

                let y0 = Self::rgb_to_y(r0, g0, b0);
                let y1 = Self::rgb_to_y(r1, g1, b1);
                let u = Self::rgb_to_u(r0, g0, b0);
                let v = Self::rgb_to_v(r0, g0, b0);

                // UYVY format: U Y0 V Y1
                uyvy.extend_from_slice(&[u, y0, v, y1]);
            }
        }

        Ok(uyvy.freeze())
    }

    /// Convert UYVY to RGB
    pub fn uyvy_to_rgb(uyvy: &[u8], width: u32, height: u32) -> Result<Bytes> {
        if uyvy.len() < YuvFormat::Uyvy.buffer_size(width, height) {
            return Err(NdiError::InvalidFrameFormat);
        }

        let mut rgb = BytesMut::with_capacity((width * height * 3) as usize);

        for y in 0..height {
            for x in 0..(width / 2) {
                let idx = ((y * width + x * 2) * 2) as usize;

                let u = i32::from(uyvy[idx]);
                let y0 = i32::from(uyvy[idx + 1]);
                let v = i32::from(uyvy[idx + 2]);
                let y1 = i32::from(uyvy[idx + 3]);

                let (r0, g0, b0) = Self::yuv_to_rgb(y0, u, v);
                rgb.extend_from_slice(&[r0, g0, b0]);

                let (r1, g1, b1) = Self::yuv_to_rgb(y1, u, v);
                rgb.extend_from_slice(&[r1, g1, b1]);
            }
        }

        Ok(rgb.freeze())
    }

    /// Convert RGB component values to Y (luminance)
    fn rgb_to_y(r: i32, g: i32, b: i32) -> u8 {
        ((66 * r + 129 * g + 25 * b + 128) >> 8).clamp(16, 235) as u8
    }

    /// Convert RGB component values to U (chrominance)
    fn rgb_to_u(r: i32, g: i32, b: i32) -> u8 {
        ((-38 * r - 74 * g + 112 * b + 128) >> 8)
            .clamp(-112, 112)
            .wrapping_add(128) as u8
    }

    /// Convert RGB component values to V (chrominance)
    fn rgb_to_v(r: i32, g: i32, b: i32) -> u8 {
        ((112 * r - 94 * g - 18 * b + 128) >> 8)
            .clamp(-112, 112)
            .wrapping_add(128) as u8
    }

    /// Convert YUV component values to RGB
    fn yuv_to_rgb(y: i32, u: i32, v: i32) -> (u8, u8, u8) {
        let c = y - 16;
        let d = u - 128;
        let e = v - 128;

        let r = ((298 * c + 409 * e + 128) >> 8).clamp(0, 255) as u8;
        let g = ((298 * c - 100 * d - 208 * e + 128) >> 8).clamp(0, 255) as u8;
        let b = ((298 * c + 516 * d + 128) >> 8).clamp(0, 255) as u8;

        (r, g, b)
    }

    /// Resize YUV422 frame (simple nearest neighbor)
    pub fn resize_yuv422(
        yuv: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
    ) -> Result<Bytes> {
        let mut output =
            BytesMut::with_capacity(YuvFormat::Yuv422.buffer_size(dst_width, dst_height));

        let x_ratio = (src_width << 16) / dst_width;
        let y_ratio = (src_height << 16) / dst_height;

        for y in 0..dst_height {
            let src_y = ((y * y_ratio) >> 16).min(src_height - 1);

            for x in 0..(dst_width / 2) {
                let src_x = ((x * 2 * x_ratio) >> 16).min(src_width - 2);
                let src_idx = ((src_y * src_width + src_x) * 2) as usize;

                if src_idx + 3 < yuv.len() {
                    output.extend_from_slice(&yuv[src_idx..src_idx + 4]);
                }
            }
        }

        Ok(output.freeze())
    }

    /// Crop YUV422 frame
    pub fn crop_yuv422(
        yuv: &[u8],
        width: u32,
        height: u32,
        x: u32,
        y: u32,
        crop_width: u32,
        crop_height: u32,
    ) -> Result<Bytes> {
        if x + crop_width > width || y + crop_height > height {
            return Err(NdiError::InvalidFrameFormat);
        }

        // Ensure even width for YUV422
        let crop_width = crop_width & !1;
        let x = x & !1;

        let mut output =
            BytesMut::with_capacity(YuvFormat::Yuv422.buffer_size(crop_width, crop_height));

        for row in y..(y + crop_height) {
            let src_idx = ((row * width + x) * 2) as usize;
            let len = (crop_width * 2) as usize;

            if src_idx + len <= yuv.len() {
                output.extend_from_slice(&yuv[src_idx..src_idx + len]);
            }
        }

        Ok(output.freeze())
    }
}

/// Hardware acceleration hooks
///
/// These are placeholder functions that can be replaced with actual hardware
/// acceleration implementations (CUDA, Metal, etc.)
pub mod hardware {
    use super::*;

    /// Check if hardware acceleration is available
    pub fn is_available() -> bool {
        // Placeholder - would check for GPU, etc.
        false
    }

    /// Get the name of the hardware accelerator
    pub fn accelerator_name() -> Option<String> {
        // Placeholder
        None
    }

    /// Compress using hardware acceleration
    pub fn hw_compress(_data: &[u8], _width: u32, _height: u32, _quality: u8) -> Result<Bytes> {
        Err(NdiError::Codec(
            "Hardware acceleration not available".to_string(),
        ))
    }

    /// Decompress using hardware acceleration
    pub fn hw_decompress(_data: &[u8], _width: u32, _height: u32) -> Result<Bytes> {
        Err(NdiError::Codec(
            "Hardware acceleration not available".to_string(),
        ))
    }

    /// Convert RGB to YUV using hardware acceleration
    pub fn hw_rgb_to_yuv(_rgb: &[u8], _width: u32, _height: u32) -> Result<Bytes> {
        Err(NdiError::Codec(
            "Hardware acceleration not available".to_string(),
        ))
    }

    /// Convert YUV to RGB using hardware acceleration
    pub fn hw_yuv_to_rgb(_yuv: &[u8], _width: u32, _height: u32) -> Result<Bytes> {
        Err(NdiError::Codec(
            "Hardware acceleration not available".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yuv_format_buffer_size() {
        assert_eq!(YuvFormat::Yuv422.buffer_size(1920, 1080), 1920 * 1080 * 2);
        assert_eq!(
            YuvFormat::Yuv420.buffer_size(1920, 1080),
            (1920 * 1080 * 3) / 2
        );
        assert_eq!(YuvFormat::Yuv444.buffer_size(1920, 1080), 1920 * 1080 * 3);
    }

    #[test]
    fn test_speedhq_codec() {
        let codec = SpeedHqCodec::new(75);
        assert_eq!(codec.quality(), 75);
        assert!(!codec.is_fast_mode());

        let data = vec![128u8; 1920 * 1080 * 2];
        let compressed = codec.compress(&data, 1920, 1080).unwrap();
        assert!(compressed.len() < data.len());

        let decompressed = codec.decompress(&compressed, data.len()).unwrap();
        assert_eq!(decompressed.len(), data.len());
        assert_eq!(decompressed[..], data[..]);
    }

    #[test]
    fn test_rgb_yuv_conversion() {
        let rgb = vec![
            255, 0, 0, 255, 0, 0, // 2 red pixels
            0, 255, 0, 0, 255, 0, // 2 green pixels
        ];

        let yuv = YuvConverter::rgb_to_yuv422(&rgb, 4, 1).unwrap();
        assert_eq!(yuv.len(), 8); // 4 pixels in YUV422 = 8 bytes

        let rgb_back = YuvConverter::yuv422_to_rgb(&yuv, 4, 1).unwrap();
        assert_eq!(rgb_back.len(), rgb.len());
    }

    #[test]
    fn test_yuv422_crop() {
        let yuv = vec![128u8; 1920 * 1080 * 2];
        let cropped = YuvConverter::crop_yuv422(&yuv, 1920, 1080, 100, 100, 640, 480).unwrap();
        assert_eq!(cropped.len(), 640 * 480 * 2);
    }

    #[test]
    fn test_yuv422_resize() {
        let yuv = vec![128u8; 1920 * 1080 * 2];
        let resized = YuvConverter::resize_yuv422(&yuv, 1920, 1080, 640, 480).unwrap();
        assert_eq!(resized.len(), 640 * 480 * 2);
    }

    #[test]
    fn test_hardware_acceleration_not_available() {
        assert!(!hardware::is_available());
        assert!(hardware::accelerator_name().is_none());
    }
}
