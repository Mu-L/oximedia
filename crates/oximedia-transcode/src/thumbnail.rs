//! Thumbnail and preview image generation.
//!
//! This module provides configuration structures and utilities for generating
//! thumbnail images or sprite sheets from video content. Actual pixel decoding
//! is handled by the caller; this module focuses on timestamp selection, sizing,
//! and nearest-neighbour scaling.

#![allow(dead_code)]

/// Output format for generated thumbnails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailFormat {
    /// JPEG (lossy, small file size).
    Jpeg,
    /// PNG (lossless).
    Png,
    /// WebP (modern, good compression).
    Webp,
}

impl ThumbnailFormat {
    /// Returns the conventional file extension for this format.
    #[must_use]
    pub fn extension(&self) -> &'static str {
        match self {
            ThumbnailFormat::Jpeg => "jpg",
            ThumbnailFormat::Png => "png",
            ThumbnailFormat::Webp => "webp",
        }
    }

    /// Returns the MIME type for this format.
    #[must_use]
    pub fn mime_type(&self) -> &'static str {
        match self {
            ThumbnailFormat::Jpeg => "image/jpeg",
            ThumbnailFormat::Png => "image/png",
            ThumbnailFormat::Webp => "image/webp",
        }
    }
}

/// Strategy for selecting thumbnail timestamps.
#[derive(Debug, Clone)]
pub enum ThumbnailStrategy {
    /// Thumbnails at a fixed interval in milliseconds.
    FixedInterval,
    /// Thumbnails at detected scene-change points (caller must supply timestamps).
    SceneChange,
    /// Thumbnails evenly distributed across the duration.
    Uniform,
    /// Thumbnails at specific caller-supplied timestamps (in milliseconds).
    AtTimestamps(Vec<u64>),
}

/// Configuration for thumbnail generation.
#[derive(Debug, Clone)]
pub struct ThumbnailConfig {
    /// Width of each thumbnail in pixels.
    pub width: u32,
    /// Height of each thumbnail in pixels.
    pub height: u32,
    /// Output image format.
    pub format: ThumbnailFormat,
    /// Quality hint (0–100). Interpretation depends on the format.
    pub quality: u8,
    /// Number of thumbnails to generate (ignored for `AtTimestamps`).
    pub count: usize,
    /// Strategy for selecting frame timestamps.
    pub interval_strategy: ThumbnailStrategy,
}

impl ThumbnailConfig {
    /// Creates a sensible default config suitable for web use (320×180 JPEG).
    #[must_use]
    pub fn default_web() -> Self {
        Self {
            width: 320,
            height: 180,
            format: ThumbnailFormat::Jpeg,
            quality: 80,
            count: 10,
            interval_strategy: ThumbnailStrategy::Uniform,
        }
    }

    /// Creates a config appropriate for building a sprite sheet with the given thumbnail count.
    #[must_use]
    pub fn sprite_sheet(count: usize) -> Self {
        Self {
            width: 160,
            height: 90,
            format: ThumbnailFormat::Jpeg,
            quality: 70,
            count,
            interval_strategy: ThumbnailStrategy::Uniform,
        }
    }

    /// Returns `true` if the configured resolution is non-zero.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0 && self.count > 0
    }
}

/// A single generated thumbnail.
#[derive(Debug, Clone)]
pub struct Thumbnail {
    /// Timestamp in the source video (milliseconds).
    pub timestamp_ms: u64,
    /// Width of this thumbnail in pixels.
    pub width: u32,
    /// Height of this thumbnail in pixels.
    pub height: u32,
    /// Raw pixel data (RGBA, row-major).
    pub data: Vec<u8>,
}

impl Thumbnail {
    /// Creates a new thumbnail with the given parameters.
    #[must_use]
    pub fn new(timestamp_ms: u64, width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            timestamp_ms,
            width,
            height,
            data,
        }
    }

    /// Returns the number of pixels in this thumbnail.
    #[must_use]
    pub fn pixel_count(&self) -> usize {
        (self.width * self.height) as usize
    }

    /// Returns the expected byte length for RGBA data.
    #[must_use]
    pub fn expected_byte_len(&self) -> usize {
        self.pixel_count() * 4
    }
}

/// Computes a list of timestamps (in milliseconds) at which to capture thumbnails.
///
/// # Arguments
///
/// * `duration_ms` - Total content duration in milliseconds.
/// * `strategy` - The selection strategy.
/// * `fps` - Frame rate of the source (used to snap timestamps to frame boundaries).
///   Pass `0.0` to skip frame-snapping.
#[must_use]
pub fn compute_thumbnail_timestamps(
    duration_ms: u64,
    strategy: &ThumbnailStrategy,
    fps: f64,
) -> Vec<u64> {
    if duration_ms == 0 {
        return Vec::new();
    }

    let snap = |ts: f64| -> u64 {
        if fps > 0.0 {
            let frame_ms = 1000.0 / fps;
            ((ts / frame_ms).round() * frame_ms) as u64
        } else {
            ts as u64
        }
    };

    match strategy {
        ThumbnailStrategy::AtTimestamps(ts) => {
            ts.iter().filter(|&&t| t <= duration_ms).copied().collect()
        }

        ThumbnailStrategy::Uniform => {
            // Will return config.count timestamps; here we use duration_ms to infer count
            // We default to 10 when called from the generic form without count.
            // Callers that know the count should use compute_uniform_timestamps.
            compute_uniform_timestamps(duration_ms, 10, fps)
        }

        ThumbnailStrategy::FixedInterval => {
            // Default: one thumbnail every 10 seconds
            let interval_ms = 10_000u64;
            let mut ts = Vec::new();
            let mut t = 0u64;
            while t <= duration_ms {
                ts.push(snap(t as f64));
                t += interval_ms;
            }
            ts
        }

        ThumbnailStrategy::SceneChange => {
            // Scene-change timestamps must be provided by the caller.
            // In the generic form, return empty (caller supplies via AtTimestamps).
            Vec::new()
        }
    }
}

/// Computes `count` uniformly spaced timestamps across `duration_ms`.
#[must_use]
pub fn compute_uniform_timestamps(duration_ms: u64, count: usize, fps: f64) -> Vec<u64> {
    if count == 0 || duration_ms == 0 {
        return Vec::new();
    }

    let snap = |ts: f64| -> u64 {
        if fps > 0.0 {
            let frame_ms = 1000.0 / fps;
            ((ts / frame_ms).round() * frame_ms) as u64
        } else {
            ts as u64
        }
    };

    if count == 1 {
        return vec![snap(duration_ms as f64 / 2.0)];
    }

    (0..count)
        .map(|i| {
            let t = (duration_ms as f64 * i as f64) / (count - 1) as f64;
            snap(t).min(duration_ms)
        })
        .collect()
}

/// Scales a source image buffer to the destination dimensions using nearest-neighbour sampling.
///
/// The buffers are expected to be RGBA (4 bytes per pixel), stored row-major.
///
/// Returns the scaled pixel data, or an empty `Vec` if any dimension is zero.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn scale_thumbnail(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return Vec::new();
    }

    let expected_len = (src_w * src_h * 4) as usize;
    if src.len() < expected_len {
        return Vec::new();
    }

    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            // Nearest-neighbour mapping
            let sx = (f64::from(dx) * f64::from(src_w) / f64::from(dst_w)) as u32;
            let sy = (f64::from(dy) * f64::from(src_h) / f64::from(dst_h)) as u32;

            let src_idx = ((sy * src_w + sx) * 4) as usize;
            let dst_idx = ((dy * dst_w + dx) * 4) as usize;

            if src_idx + 3 < src.len() && dst_idx + 3 < dst.len() {
                dst[dst_idx] = src[src_idx];
                dst[dst_idx + 1] = src[src_idx + 1];
                dst[dst_idx + 2] = src[src_idx + 2];
                dst[dst_idx + 3] = src[src_idx + 3];
            }
        }
    }

    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thumbnail_format_extension() {
        assert_eq!(ThumbnailFormat::Jpeg.extension(), "jpg");
        assert_eq!(ThumbnailFormat::Png.extension(), "png");
        assert_eq!(ThumbnailFormat::Webp.extension(), "webp");
    }

    #[test]
    fn test_thumbnail_format_mime_type() {
        assert_eq!(ThumbnailFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(ThumbnailFormat::Png.mime_type(), "image/png");
        assert_eq!(ThumbnailFormat::Webp.mime_type(), "image/webp");
    }

    #[test]
    fn test_thumbnail_config_default_web() {
        let cfg = ThumbnailConfig::default_web();
        assert_eq!(cfg.width, 320);
        assert_eq!(cfg.height, 180);
        assert_eq!(cfg.format, ThumbnailFormat::Jpeg);
        assert_eq!(cfg.count, 10);
        assert!(cfg.is_valid());
    }

    #[test]
    fn test_thumbnail_config_sprite_sheet() {
        let cfg = ThumbnailConfig::sprite_sheet(20);
        assert_eq!(cfg.width, 160);
        assert_eq!(cfg.height, 90);
        assert_eq!(cfg.count, 20);
        assert!(cfg.is_valid());
    }

    #[test]
    fn test_thumbnail_pixel_count() {
        let thumb = Thumbnail::new(0, 160, 90, vec![0; 160 * 90 * 4]);
        assert_eq!(thumb.pixel_count(), 14400);
        assert_eq!(thumb.expected_byte_len(), 57600);
    }

    #[test]
    fn test_compute_timestamps_at_timestamps() {
        let strategy = ThumbnailStrategy::AtTimestamps(vec![1000, 2000, 3000]);
        let ts = compute_thumbnail_timestamps(5000, &strategy, 0.0);
        assert_eq!(ts, vec![1000, 2000, 3000]);
    }

    #[test]
    fn test_compute_timestamps_at_timestamps_filters_out_of_range() {
        let strategy = ThumbnailStrategy::AtTimestamps(vec![1000, 2000, 9999]);
        let ts = compute_thumbnail_timestamps(5000, &strategy, 0.0);
        assert_eq!(ts, vec![1000, 2000]);
    }

    #[test]
    fn test_compute_timestamps_zero_duration() {
        let ts = compute_thumbnail_timestamps(0, &ThumbnailStrategy::Uniform, 24.0);
        assert!(ts.is_empty());
    }

    #[test]
    fn test_compute_uniform_timestamps_count() {
        let ts = compute_uniform_timestamps(60_000, 5, 0.0);
        assert_eq!(ts.len(), 5);
        // First should be 0, last should be 60000
        assert_eq!(ts[0], 0);
        assert_eq!(ts[4], 60_000);
    }

    #[test]
    fn test_compute_uniform_timestamps_single() {
        let ts = compute_uniform_timestamps(10_000, 1, 0.0);
        assert_eq!(ts.len(), 1);
        assert_eq!(ts[0], 5000);
    }

    #[test]
    fn test_compute_fixed_interval_timestamps() {
        // 30 seconds → timestamps at 0, 10000, 20000, 30000
        let ts = compute_thumbnail_timestamps(30_000, &ThumbnailStrategy::FixedInterval, 0.0);
        assert_eq!(ts, vec![0, 10_000, 20_000, 30_000]);
    }

    #[test]
    fn test_scale_thumbnail_identity() {
        // 2x2 RGBA image (identity scale)
        let src = vec![
            255, 0, 0, 255, // pixel (0,0): red
            0, 255, 0, 255, // pixel (1,0): green
            0, 0, 255, 255, // pixel (0,1): blue
            255, 255, 0, 255, // pixel (1,1): yellow
        ];
        let dst = scale_thumbnail(&src, 2, 2, 2, 2);
        assert_eq!(dst, src);
    }

    #[test]
    fn test_scale_thumbnail_upscale() {
        // 1x1 → 2x2: all pixels should be the same
        let src = vec![100u8, 150, 200, 255];
        let dst = scale_thumbnail(&src, 1, 1, 2, 2);
        assert_eq!(dst.len(), 16);
        // All four pixels should replicate the source
        assert_eq!(&dst[0..4], &[100, 150, 200, 255]);
        assert_eq!(&dst[4..8], &[100, 150, 200, 255]);
    }

    #[test]
    fn test_scale_thumbnail_zero_dimensions() {
        let src = vec![255u8; 16];
        assert!(scale_thumbnail(&src, 0, 2, 4, 4).is_empty());
        assert!(scale_thumbnail(&src, 2, 2, 0, 4).is_empty());
    }

    #[test]
    fn test_scale_thumbnail_undersized_src() {
        // Supply less data than expected → empty result
        let src = vec![255u8; 4]; // only 1 pixel, but claiming 4x4
        let dst = scale_thumbnail(&src, 4, 4, 2, 2);
        assert!(dst.is_empty());
    }
}
