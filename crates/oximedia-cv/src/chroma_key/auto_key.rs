//! Automatic key color detection.
//!
//! This module provides algorithms for automatically detecting the key color
//! from a video frame, eliminating the need for manual color selection.

use super::{Hsv, Rgb};
use crate::error::{CvError, CvResult};
use oximedia_codec::VideoFrame;
use oximedia_core::PixelFormat;
use std::collections::HashMap;

/// Automatic key color detector.
///
/// Analyzes video frames to identify the most suitable key color,
/// typically the most dominant saturated color in the background.
pub struct AutoKeyDetector {
    /// Minimum saturation for key color candidates (0.0-1.0).
    min_saturation: f32,
    /// Minimum value/brightness for key color candidates (0.0-1.0).
    min_value: f32,
    /// Hue bucket size in degrees for histogram analysis.
    hue_bucket_size: f32,
}

impl AutoKeyDetector {
    /// Create a new auto key detector with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            min_saturation: 0.3,
            min_value: 0.2,
            hue_bucket_size: 10.0,
        }
    }

    /// Set minimum saturation threshold for key color candidates.
    pub fn set_min_saturation(&mut self, saturation: f32) {
        self.min_saturation = saturation.clamp(0.0, 1.0);
    }

    /// Set minimum value/brightness threshold.
    pub fn set_min_value(&mut self, value: f32) {
        self.min_value = value.clamp(0.0, 1.0);
    }

    /// Set hue bucket size for histogram analysis.
    pub fn set_hue_bucket_size(&mut self, size: f32) {
        self.hue_bucket_size = size.clamp(1.0, 90.0);
    }

    /// Detect key color from a specific region in the frame.
    ///
    /// # Arguments
    ///
    /// * `frame` - The video frame to analyze
    /// * `x`, `y` - Top-left corner of sample region
    /// * `width`, `height` - Size of sample region
    ///
    /// # Errors
    ///
    /// Returns an error if the region is invalid or detection fails.
    pub fn detect_from_region(
        &self,
        frame: &VideoFrame,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> CvResult<Rgb> {
        // Validate region
        if x + width > frame.width || y + height > frame.height {
            return Err(CvError::invalid_roi(x, y, width, height));
        }

        // Convert region to RGB data
        let rgb_data = self.extract_region_rgb(frame, x, y, width, height)?;

        // Analyze colors in the region
        self.detect_from_rgb_data(&rgb_data, width as usize, height as usize)
    }

    /// Detect key color from entire frame.
    ///
    /// # Errors
    ///
    /// Returns an error if detection fails.
    pub fn detect_from_frame(&self, frame: &VideoFrame) -> CvResult<Rgb> {
        self.detect_from_region(frame, 0, 0, frame.width, frame.height)
    }

    /// Detect key color using edge detection strategy.
    ///
    /// This method focuses on the edges of the frame where the background
    /// (green/blue screen) is typically most visible.
    ///
    /// # Errors
    ///
    /// Returns an error if detection fails.
    #[allow(clippy::vec_init_then_push)]
    pub fn detect_from_edges(&self, frame: &VideoFrame) -> CvResult<Rgb> {
        let width = frame.width;
        let height = frame.height;
        let border_size = (width.min(height) / 10).max(20); // 10% border or min 20px

        // Sample from all four edges
        let mut samples = Vec::with_capacity(4);

        // Top edge
        samples.push(self.detect_from_region(frame, 0, 0, width, border_size)?);

        // Bottom edge
        samples.push(self.detect_from_region(
            frame,
            0,
            height - border_size,
            width,
            border_size,
        )?);

        // Left edge
        samples.push(self.detect_from_region(frame, 0, 0, border_size, height)?);

        // Right edge
        samples.push(self.detect_from_region(
            frame,
            width - border_size,
            0,
            border_size,
            height,
        )?);

        // Average the samples
        let avg_r = samples.iter().map(|c| c.r).sum::<f32>() / samples.len() as f32;
        let avg_g = samples.iter().map(|c| c.g).sum::<f32>() / samples.len() as f32;
        let avg_b = samples.iter().map(|c| c.b).sum::<f32>() / samples.len() as f32;

        Ok(Rgb::new(avg_r, avg_g, avg_b))
    }

    /// Detect key color from corner samples.
    ///
    /// Useful when the subject is centered and background is visible in corners.
    ///
    /// # Errors
    ///
    /// Returns an error if detection fails.
    #[allow(clippy::vec_init_then_push)]
    pub fn detect_from_corners(&self, frame: &VideoFrame) -> CvResult<Rgb> {
        let width = frame.width;
        let height = frame.height;
        let sample_size = (width.min(height) / 8).max(50); // 1/8 of smaller dimension

        // Sample from all four corners
        let mut samples = Vec::with_capacity(4);

        // Top-left
        samples.push(self.detect_from_region(frame, 0, 0, sample_size, sample_size)?);

        // Top-right
        samples.push(self.detect_from_region(
            frame,
            width - sample_size,
            0,
            sample_size,
            sample_size,
        )?);

        // Bottom-left
        samples.push(self.detect_from_region(
            frame,
            0,
            height - sample_size,
            sample_size,
            sample_size,
        )?);

        // Bottom-right
        samples.push(self.detect_from_region(
            frame,
            width - sample_size,
            height - sample_size,
            sample_size,
            sample_size,
        )?);

        // Return the most common color (mode)
        self.find_mode_color(&samples)
    }

    /// Extract RGB data from a specific region.
    fn extract_region_rgb(
        &self,
        frame: &VideoFrame,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> CvResult<Vec<f32>> {
        let region_size = (width * height) as usize;
        let mut rgb_data = vec![0.0f32; region_size * 3];

        match frame.format {
            PixelFormat::Rgb24 => {
                if frame.planes.is_empty() {
                    return Err(CvError::invalid_parameter("planes", "empty"));
                }
                let data = &frame.planes[0].data;
                let stride = frame.planes[0].stride;

                for row in 0..height as usize {
                    let src_y = y as usize + row;
                    let src_offset = src_y * stride + x as usize * 3;
                    let dst_offset = row * width as usize * 3;

                    for col in 0..width as usize {
                        let src_idx = src_offset + col * 3;
                        let dst_idx = dst_offset + col * 3;

                        rgb_data[dst_idx] = f32::from(data[src_idx]) / 255.0;
                        rgb_data[dst_idx + 1] = f32::from(data[src_idx + 1]) / 255.0;
                        rgb_data[dst_idx + 2] = f32::from(data[src_idx + 2]) / 255.0;
                    }
                }
            }
            PixelFormat::Rgba32 => {
                if frame.planes.is_empty() {
                    return Err(CvError::invalid_parameter("planes", "empty"));
                }
                let data = &frame.planes[0].data;
                let stride = frame.planes[0].stride;

                for row in 0..height as usize {
                    let src_y = y as usize + row;
                    let src_offset = src_y * stride + x as usize * 4;
                    let dst_offset = row * width as usize * 3;

                    for col in 0..width as usize {
                        let src_idx = src_offset + col * 4;
                        let dst_idx = dst_offset + col * 3;

                        rgb_data[dst_idx] = f32::from(data[src_idx]) / 255.0;
                        rgb_data[dst_idx + 1] = f32::from(data[src_idx + 1]) / 255.0;
                        rgb_data[dst_idx + 2] = f32::from(data[src_idx + 2]) / 255.0;
                    }
                }
            }
            _ => {
                return Err(CvError::unsupported_format(format!("{}", frame.format)));
            }
        }

        Ok(rgb_data)
    }

    /// Detect key color from RGB data using histogram analysis.
    fn detect_from_rgb_data(&self, rgb_data: &[f32], width: usize, height: usize) -> CvResult<Rgb> {
        let pixel_count = width * height;

        // Build HSV histogram
        let mut hue_histogram: HashMap<i32, ColorAccumulator> = HashMap::new();

        for i in 0..pixel_count {
            let r = rgb_data[i * 3];
            let g = rgb_data[i * 3 + 1];
            let b = rgb_data[i * 3 + 2];

            let pixel = Rgb::new(r, g, b);
            let hsv = pixel.to_hsv();

            // Filter by saturation and value thresholds
            if hsv.s >= self.min_saturation && hsv.v >= self.min_value {
                let hue_bucket = (hsv.h / self.hue_bucket_size) as i32;

                let accumulator = hue_histogram.entry(hue_bucket).or_insert(ColorAccumulator {
                    count: 0,
                    sum_h: 0.0,
                    sum_s: 0.0,
                    sum_v: 0.0,
                });

                accumulator.count += 1;
                accumulator.sum_h += hsv.h;
                accumulator.sum_s += hsv.s;
                accumulator.sum_v += hsv.v;
            }
        }

        // Find the most dominant hue bucket
        let dominant_bucket = hue_histogram
            .iter()
            .max_by_key(|(_, acc)| acc.count)
            .ok_or_else(|| CvError::detection_failed("No suitable key color found"))?;

        let accumulator = dominant_bucket.1;
        let avg_hue = accumulator.sum_h / accumulator.count as f32;
        let avg_sat = accumulator.sum_s / accumulator.count as f32;
        let avg_val = accumulator.sum_v / accumulator.count as f32;

        let key_hsv = Hsv::new(avg_hue, avg_sat, avg_val);
        Ok(key_hsv.to_rgb())
    }

    /// Find the mode (most common) color from samples.
    fn find_mode_color(&self, samples: &[Rgb]) -> CvResult<Rgb> {
        if samples.is_empty() {
            return Err(CvError::detection_failed("No color samples provided"));
        }

        // Convert to HSV for clustering
        let hsv_samples: Vec<Hsv> = samples.iter().map(super::Rgb::to_hsv).collect();

        // Group by hue bucket
        let mut hue_groups: HashMap<i32, Vec<Hsv>> = HashMap::new();

        for hsv in &hsv_samples {
            if hsv.s >= self.min_saturation && hsv.v >= self.min_value {
                let bucket = (hsv.h / self.hue_bucket_size) as i32;
                hue_groups.entry(bucket).or_default().push(*hsv);
            }
        }

        // Find largest group
        let largest_group = hue_groups
            .values()
            .max_by_key(|group| group.len())
            .ok_or_else(|| CvError::detection_failed("No suitable key color found"))?;

        // Average colors in largest group
        let avg_h = largest_group.iter().map(|hsv| hsv.h).sum::<f32>() / largest_group.len() as f32;
        let avg_s = largest_group.iter().map(|hsv| hsv.s).sum::<f32>() / largest_group.len() as f32;
        let avg_v = largest_group.iter().map(|hsv| hsv.v).sum::<f32>() / largest_group.len() as f32;

        let mode_hsv = Hsv::new(avg_h, avg_s, avg_v);
        Ok(mode_hsv.to_rgb())
    }
}

impl Default for AutoKeyDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulator for color histogram buckets.
#[derive(Debug, Clone, Copy)]
struct ColorAccumulator {
    count: usize,
    sum_h: f32,
    sum_s: f32,
    sum_v: f32,
}

/// Key color recommendation based on screen type detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenType {
    /// Green screen detected.
    GreenScreen,
    /// Blue screen detected.
    BlueScreen,
    /// Unknown or custom color.
    Unknown,
}

/// Screen type detector.
///
/// Analyzes detected key color to determine if it's a green screen,
/// blue screen, or custom color.
pub struct ScreenTypeDetector {
    /// Threshold for green screen detection (hue degrees).
    green_hue_center: f32,
    green_hue_tolerance: f32,
    /// Threshold for blue screen detection (hue degrees).
    blue_hue_center: f32,
    blue_hue_tolerance: f32,
}

impl ScreenTypeDetector {
    /// Create a new screen type detector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            green_hue_center: 120.0,   // Green is around 120 degrees
            green_hue_tolerance: 30.0, // ±30 degrees
            blue_hue_center: 240.0,    // Blue is around 240 degrees
            blue_hue_tolerance: 30.0,  // ±30 degrees
        }
    }

    /// Detect screen type from RGB color.
    #[must_use]
    pub fn detect(&self, color: &Rgb) -> ScreenType {
        let hsv = color.to_hsv();

        // Check if it's green screen
        let green_diff = (hsv.h - self.green_hue_center).abs();
        if green_diff <= self.green_hue_tolerance {
            return ScreenType::GreenScreen;
        }

        // Check if it's blue screen (handle wrap-around at 360)
        let blue_diff = (hsv.h - self.blue_hue_center).abs();
        let blue_diff_wrapped = (hsv.h - (self.blue_hue_center + 360.0)).abs();
        if blue_diff <= self.blue_hue_tolerance || blue_diff_wrapped <= self.blue_hue_tolerance {
            return ScreenType::BlueScreen;
        }

        ScreenType::Unknown
    }

    /// Get recommended configuration for detected screen type.
    #[must_use]
    pub fn recommend_config(&self, screen_type: ScreenType) -> (f32, f32) {
        match screen_type {
            ScreenType::GreenScreen => {
                // Green screens typically need slightly higher threshold
                (0.35, 0.15)
            }
            ScreenType::BlueScreen => {
                // Blue screens can use slightly lower threshold
                (0.30, 0.12)
            }
            ScreenType::Unknown => {
                // Conservative defaults for custom colors
                (0.30, 0.10)
            }
        }
    }
}

impl Default for ScreenTypeDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Multi-frame key color detector.
///
/// Analyzes multiple frames to find a consistent key color,
/// reducing the impact of motion and lighting variations.
pub struct MultiFrameDetector {
    auto_detector: AutoKeyDetector,
    samples: Vec<Rgb>,
    max_samples: usize,
}

impl MultiFrameDetector {
    /// Create a new multi-frame detector.
    ///
    /// # Arguments
    ///
    /// * `max_samples` - Maximum number of frames to analyze
    #[must_use]
    pub fn new(max_samples: usize) -> Self {
        Self {
            auto_detector: AutoKeyDetector::new(),
            samples: Vec::new(),
            max_samples: max_samples.max(1),
        }
    }

    /// Add a frame sample to the detector.
    ///
    /// # Errors
    ///
    /// Returns an error if key color detection fails for this frame.
    pub fn add_frame(&mut self, frame: &VideoFrame) -> CvResult<()> {
        let color = self.auto_detector.detect_from_edges(frame)?;

        self.samples.push(color);

        // Keep only the most recent samples
        if self.samples.len() > self.max_samples {
            self.samples.remove(0);
        }

        Ok(())
    }

    /// Get the consensus key color from all samples.
    ///
    /// # Errors
    ///
    /// Returns an error if no samples have been added.
    pub fn get_key_color(&self) -> CvResult<Rgb> {
        if self.samples.is_empty() {
            return Err(CvError::detection_failed("No frames have been sampled"));
        }

        // Find mode color
        self.auto_detector.find_mode_color(&self.samples)
    }

    /// Get the number of samples collected.
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Check if detector has collected enough samples.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.samples.len() >= self.max_samples / 2
    }

    /// Reset the detector, clearing all samples.
    pub fn reset(&mut self) {
        self.samples.clear();
    }
}
