//! Video noise reduction filter.
//!
//! This filter provides comprehensive spatial and temporal noise reduction
//! for video frames using various advanced techniques.
//!
//! # Features
//!
//! ## Spatial Noise Reduction:
//! - Bilateral filtering (edge-preserving)
//! - Non-local means (NLM) denoising
//! - Gaussian filtering
//! - Median filtering
//! - Adaptive filtering based on local variance
//!
//! ## Temporal Noise Reduction:
//! - Motion-compensated temporal filtering
//! - Recursive temporal averaging
//! - 3D block matching (BM3D-inspired)
//! - Weighted temporal averaging
//!
//! ## Advanced Features:
//! - Separate chroma and luma processing
//! - Edge-preserving noise reduction
//! - Adaptive strength based on content
//! - Multi-frame temporal coherence
//! - Configurable strength, radius, and temporal depth
//!
//! # Example
//!
//! ```ignore
//! use oximedia_graph::filters::video::{DenoiseFilter, DenoiseConfig, DenoiseMethod};
//! use oximedia_graph::node::NodeId;
//!
//! // Create a denoise filter with bilateral filtering
//! let config = DenoiseConfig::new()
//!     .with_method(DenoiseMethod::Bilateral)
//!     .with_strength(0.8)
//!     .with_spatial_radius(5)
//!     .with_temporal_depth(3);
//!
//! let filter = DenoiseFilter::new(NodeId(0), "denoise", config);
//! ```

#![forbid(unsafe_code)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]
#![allow(clippy::many_single_char_names)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::unused_self)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::bool_to_int_with_if)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::too_many_arguments)]
#![allow(dead_code)]

use std::collections::VecDeque;

use crate::error::{GraphError, GraphResult};
use crate::frame::FilterFrame;
use crate::node::{Node, NodeId, NodeState, NodeType};
use crate::port::{InputPort, OutputPort, PortFormat, PortId, PortType, VideoPortFormat};
use oximedia_codec::{Plane, VideoFrame};

/// Noise reduction method.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DenoiseMethod {
    /// Bilateral filtering (edge-preserving spatial filter).
    #[default]
    Bilateral,
    /// Non-local means denoising.
    NonLocalMeans,
    /// Gaussian blur.
    Gaussian,
    /// Median filtering.
    Median,
    /// Adaptive filtering based on local variance.
    Adaptive,
    /// Temporal averaging.
    Temporal,
    /// Motion-compensated temporal filtering.
    MotionCompensated,
    /// 3D block matching (BM3D-inspired).
    BlockMatching3D,
    /// Combined spatial and temporal.
    Combined,
}

/// Temporal filter mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TemporalMode {
    /// Simple temporal averaging.
    #[default]
    Average,
    /// Recursive temporal filtering.
    Recursive,
    /// Motion-compensated.
    MotionCompensated,
    /// Weighted averaging based on similarity.
    WeightedAverage,
}

/// Motion estimation quality.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MotionQuality {
    /// Fast motion estimation (larger block size, limited search).
    Fast,
    /// Balanced quality and speed.
    #[default]
    Medium,
    /// High quality (smaller blocks, larger search).
    High,
}

/// Configuration for the denoise filter.
#[derive(Clone, Debug)]
pub struct DenoiseConfig {
    /// Primary denoising method.
    pub method: DenoiseMethod,
    /// Overall noise reduction strength (0.0-1.0).
    pub strength: f32,
    /// Luma (Y) plane strength multiplier.
    pub luma_strength: f32,
    /// Chroma (U/V) plane strength multiplier.
    pub chroma_strength: f32,
    /// Spatial filter radius (pixels).
    pub spatial_radius: u32,
    /// Temporal depth (number of frames to use).
    pub temporal_depth: usize,
    /// Temporal filter mode.
    pub temporal_mode: TemporalMode,
    /// Motion estimation quality.
    pub motion_quality: MotionQuality,
    /// Sigma for color/range in bilateral filter.
    pub sigma_color: f32,
    /// Sigma for space/distance in bilateral filter.
    pub sigma_space: f32,
    /// Search window size for NLM and motion estimation.
    pub search_window: u32,
    /// Patch size for NLM and block matching.
    pub patch_size: u32,
    /// NLM filtering strength parameter (h).
    pub nlm_h: f32,
    /// Enable edge preservation.
    pub preserve_edges: bool,
    /// Edge threshold for adaptive filtering.
    pub edge_threshold: f32,
    /// Enable adaptive strength based on noise level.
    pub adaptive_strength: bool,
    /// Recursive filter alpha (for temporal recursive mode).
    pub recursive_alpha: f32,
}

impl DenoiseConfig {
    /// Create a new denoise configuration with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            method: DenoiseMethod::Bilateral,
            strength: 0.7,
            luma_strength: 1.0,
            chroma_strength: 1.2,
            spatial_radius: 5,
            temporal_depth: 3,
            temporal_mode: TemporalMode::Average,
            motion_quality: MotionQuality::Medium,
            sigma_color: 50.0,
            sigma_space: 10.0,
            search_window: 21,
            patch_size: 7,
            nlm_h: 10.0,
            preserve_edges: true,
            edge_threshold: 30.0,
            adaptive_strength: true,
            recursive_alpha: 0.3,
        }
    }

    /// Set the denoising method.
    #[must_use]
    pub fn with_method(mut self, method: DenoiseMethod) -> Self {
        self.method = method;
        self
    }

    /// Set the overall strength.
    #[must_use]
    pub fn with_strength(mut self, strength: f32) -> Self {
        self.strength = strength.clamp(0.0, 1.0);
        self
    }

    /// Set luma strength multiplier.
    #[must_use]
    pub fn with_luma_strength(mut self, strength: f32) -> Self {
        self.luma_strength = strength.clamp(0.0, 2.0);
        self
    }

    /// Set chroma strength multiplier.
    #[must_use]
    pub fn with_chroma_strength(mut self, strength: f32) -> Self {
        self.chroma_strength = strength.clamp(0.0, 2.0);
        self
    }

    /// Set spatial filter radius.
    #[must_use]
    pub fn with_spatial_radius(mut self, radius: u32) -> Self {
        self.spatial_radius = radius.clamp(1, 31);
        self
    }

    /// Set temporal depth.
    #[must_use]
    pub fn with_temporal_depth(mut self, depth: usize) -> Self {
        self.temporal_depth = depth.clamp(1, 10);
        self
    }

    /// Set temporal mode.
    #[must_use]
    pub fn with_temporal_mode(mut self, mode: TemporalMode) -> Self {
        self.temporal_mode = mode;
        self
    }

    /// Set motion estimation quality.
    #[must_use]
    pub fn with_motion_quality(mut self, quality: MotionQuality) -> Self {
        self.motion_quality = quality;
        self
    }

    /// Set bilateral filter parameters.
    #[must_use]
    pub fn with_bilateral_params(mut self, sigma_color: f32, sigma_space: f32) -> Self {
        self.sigma_color = sigma_color;
        self.sigma_space = sigma_space;
        self
    }

    /// Set NLM parameters.
    #[must_use]
    pub fn with_nlm_params(mut self, h: f32, search_window: u32, patch_size: u32) -> Self {
        self.nlm_h = h;
        self.search_window = search_window;
        self.patch_size = patch_size;
        self
    }

    /// Enable/disable edge preservation.
    #[must_use]
    pub fn with_edge_preservation(mut self, enabled: bool) -> Self {
        self.preserve_edges = enabled;
        self
    }

    /// Set edge threshold.
    #[must_use]
    pub fn with_edge_threshold(mut self, threshold: f32) -> Self {
        self.edge_threshold = threshold;
        self
    }

    /// Enable/disable adaptive strength.
    #[must_use]
    pub fn with_adaptive_strength(mut self, enabled: bool) -> Self {
        self.adaptive_strength = enabled;
        self
    }

    /// Set recursive filter alpha.
    #[must_use]
    pub fn with_recursive_alpha(mut self, alpha: f32) -> Self {
        self.recursive_alpha = alpha.clamp(0.0, 1.0);
        self
    }
}

impl Default for DenoiseConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Video denoise filter.
///
/// Removes noise from video frames using spatial and/or temporal filtering
/// techniques. Supports various methods and can process luma and chroma
/// planes with different strengths.
pub struct DenoiseFilter {
    id: NodeId,
    name: String,
    state: NodeState,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    config: DenoiseConfig,
    /// Frame buffer for temporal processing.
    frame_buffer: VecDeque<VideoFrame>,
    /// Previous frame for recursive temporal filtering.
    prev_frame: Option<VideoFrame>,
    /// Noise statistics for adaptive processing.
    noise_stats: Option<NoiseStatistics>,
}

impl DenoiseFilter {
    /// Create a new denoise filter.
    #[must_use]
    pub fn new(id: NodeId, name: impl Into<String>, config: DenoiseConfig) -> Self {
        Self {
            id,
            name: name.into(),
            state: NodeState::Idle,
            inputs: vec![InputPort::new(PortId(0), "input", PortType::Video)
                .with_format(PortFormat::Video(VideoPortFormat::any()))],
            outputs: vec![OutputPort::new(PortId(0), "output", PortType::Video)
                .with_format(PortFormat::Video(VideoPortFormat::any()))],
            config,
            frame_buffer: VecDeque::new(),
            prev_frame: None,
            noise_stats: None,
        }
    }

    /// Get the current configuration.
    #[must_use]
    pub fn config(&self) -> &DenoiseConfig {
        &self.config
    }

    /// Update the configuration.
    pub fn set_config(&mut self, config: DenoiseConfig) {
        self.config = config;
        self.frame_buffer.clear();
        self.prev_frame = None;
        self.noise_stats = None;
    }

    /// Process a single frame.
    fn process_frame(&mut self, mut frame: VideoFrame) -> GraphResult<VideoFrame> {
        // Update temporal buffer
        if self.needs_temporal_processing() {
            self.frame_buffer.push_back(frame.clone());
            if self.frame_buffer.len() > self.config.temporal_depth * 2 + 1 {
                self.frame_buffer.pop_front();
            }
        }

        // Estimate noise if adaptive strength is enabled
        if self.config.adaptive_strength && self.noise_stats.is_none() {
            self.noise_stats = Some(NoiseStatistics::estimate(&frame));
        }

        // Process frame based on method
        match self.config.method {
            DenoiseMethod::Bilateral => {
                self.apply_bilateral(&mut frame)?;
            }
            DenoiseMethod::NonLocalMeans => {
                self.apply_nlm(&mut frame)?;
            }
            DenoiseMethod::Gaussian => {
                self.apply_gaussian(&mut frame)?;
            }
            DenoiseMethod::Median => {
                self.apply_median(&mut frame)?;
            }
            DenoiseMethod::Adaptive => {
                self.apply_adaptive(&mut frame)?;
            }
            DenoiseMethod::Temporal => {
                self.apply_temporal(&mut frame)?;
            }
            DenoiseMethod::MotionCompensated => {
                self.apply_motion_compensated(&mut frame)?;
            }
            DenoiseMethod::BlockMatching3D => {
                self.apply_bm3d(&mut frame)?;
            }
            DenoiseMethod::Combined => {
                self.apply_combined(&mut frame)?;
            }
        }

        // Update previous frame for recursive temporal
        if self.config.temporal_mode == TemporalMode::Recursive {
            self.prev_frame = Some(frame.clone());
        }

        Ok(frame)
    }

    /// Check if temporal processing is needed.
    fn needs_temporal_processing(&self) -> bool {
        matches!(
            self.config.method,
            DenoiseMethod::Temporal
                | DenoiseMethod::MotionCompensated
                | DenoiseMethod::BlockMatching3D
                | DenoiseMethod::Combined
        )
    }

    /// Apply bilateral filtering.
    fn apply_bilateral(&self, frame: &mut VideoFrame) -> GraphResult<()> {
        let (h_sub, v_sub) = frame.format.chroma_subsampling();

        for (plane_idx, plane) in frame.planes.iter_mut().enumerate() {
            let (width, height) = if plane_idx == 0 {
                (frame.width, frame.height)
            } else {
                (frame.width / h_sub, frame.height / v_sub)
            };

            let strength = self.get_plane_strength(plane_idx);
            let sigma_color = self.config.sigma_color * strength;
            let sigma_space = self.config.sigma_space;
            let radius = self.config.spatial_radius;

            self.bilateral_filter_plane(plane, width, height, radius, sigma_color, sigma_space)?;
        }

        Ok(())
    }

    /// Apply bilateral filter to a single plane.
    fn bilateral_filter_plane(
        &self,
        plane: &mut Plane,
        width: u32,
        height: u32,
        radius: u32,
        sigma_color: f32,
        sigma_space: f32,
    ) -> GraphResult<()> {
        let mut data = plane.data.to_vec();
        let original = data.clone();

        let color_coeff = -0.5 / (sigma_color * sigma_color);
        let space_coeff = -0.5 / (sigma_space * sigma_space);

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let center_val = original.get(idx).copied().unwrap_or(128) as f32;

                let mut sum = 0.0f32;
                let mut weight_sum = 0.0f32;

                let y_min = y.saturating_sub(radius);
                let y_max = (y + radius + 1).min(height);
                let x_min = x.saturating_sub(radius);
                let x_max = (x + radius + 1).min(width);

                for ny in y_min..y_max {
                    for nx in x_min..x_max {
                        let nidx = (ny * width + nx) as usize;
                        let neighbor_val = original.get(nidx).copied().unwrap_or(128) as f32;

                        let color_dist = neighbor_val - center_val;
                        let space_dist =
                            ((nx as i32 - x as i32).pow(2) + (ny as i32 - y as i32).pow(2)) as f32;

                        let color_weight = (color_dist * color_dist * color_coeff).exp();
                        let space_weight = (space_dist * space_coeff).exp();
                        let weight = color_weight * space_weight;

                        sum += neighbor_val * weight;
                        weight_sum += weight;
                    }
                }

                if weight_sum > 0.0 {
                    data[idx] = (sum / weight_sum).round().clamp(0.0, 255.0) as u8;
                }
            }
        }

        *plane = Plane::new(data, plane.stride);
        Ok(())
    }

    /// Apply non-local means denoising.
    fn apply_nlm(&self, frame: &mut VideoFrame) -> GraphResult<()> {
        let (h_sub, v_sub) = frame.format.chroma_subsampling();

        for (plane_idx, plane) in frame.planes.iter_mut().enumerate() {
            let (width, height) = if plane_idx == 0 {
                (frame.width, frame.height)
            } else {
                (frame.width / h_sub, frame.height / v_sub)
            };

            let strength = self.get_plane_strength(plane_idx);
            let h = self.config.nlm_h * strength;

            self.nlm_filter_plane(plane, width, height, h)?;
        }

        Ok(())
    }

    /// Apply NLM filter to a single plane.
    fn nlm_filter_plane(
        &self,
        plane: &mut Plane,
        width: u32,
        height: u32,
        h: f32,
    ) -> GraphResult<()> {
        let mut data = plane.data.to_vec();
        let original = data.clone();

        let search_radius = self.config.search_window / 2;
        let patch_radius = self.config.patch_size / 2;
        let h_sq = h * h;

        for y in patch_radius..(height - patch_radius) {
            for x in patch_radius..(width - patch_radius) {
                let mut sum = 0.0f32;
                let mut weight_sum = 0.0f32;

                let y_min = y.saturating_sub(search_radius);
                let y_max = (y + search_radius + 1).min(height - patch_radius);
                let x_min = x.saturating_sub(search_radius);
                let x_max = (x + search_radius + 1).min(width - patch_radius);

                for sy in y_min..y_max {
                    for sx in x_min..x_max {
                        let dist =
                            self.patch_distance(&original, x, y, sx, sy, width, patch_radius);

                        let weight = (-dist / h_sq).exp();
                        let sidx = (sy * width + sx) as usize;
                        sum += original.get(sidx).copied().unwrap_or(128) as f32 * weight;
                        weight_sum += weight;
                    }
                }

                let idx = (y * width + x) as usize;
                if weight_sum > 0.0 {
                    data[idx] = (sum / weight_sum).round().clamp(0.0, 255.0) as u8;
                }
            }
        }

        *plane = Plane::new(data, plane.stride);
        Ok(())
    }

    /// Compute patch distance.
    fn patch_distance(
        &self,
        data: &[u8],
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        width: u32,
        radius: u32,
    ) -> f32 {
        let mut dist = 0.0f32;
        let mut count = 0;

        for dy in 0..=radius * 2 {
            for dx in 0..=radius * 2 {
                let px1 = x1 + dx - radius;
                let py1 = y1 + dy - radius;
                let px2 = x2 + dx - radius;
                let py2 = y2 + dy - radius;

                let idx1 = (py1 * width + px1) as usize;
                let idx2 = (py2 * width + px2) as usize;

                let v1 = data.get(idx1).copied().unwrap_or(128) as f32;
                let v2 = data.get(idx2).copied().unwrap_or(128) as f32;

                let diff = v1 - v2;
                dist += diff * diff;
                count += 1;
            }
        }

        if count > 0 {
            dist / count as f32
        } else {
            0.0
        }
    }

    /// Apply Gaussian filtering.
    fn apply_gaussian(&self, frame: &mut VideoFrame) -> GraphResult<()> {
        let (h_sub, v_sub) = frame.format.chroma_subsampling();

        for (plane_idx, plane) in frame.planes.iter_mut().enumerate() {
            let (width, height) = if plane_idx == 0 {
                (frame.width, frame.height)
            } else {
                (frame.width / h_sub, frame.height / v_sub)
            };

            let strength = self.get_plane_strength(plane_idx);
            let sigma = self.config.sigma_space * strength;
            let radius = self.config.spatial_radius;

            self.gaussian_filter_plane(plane, width, height, radius, sigma)?;
        }

        Ok(())
    }

    /// Apply Gaussian filter to a single plane.
    fn gaussian_filter_plane(
        &self,
        plane: &mut Plane,
        width: u32,
        height: u32,
        radius: u32,
        sigma: f32,
    ) -> GraphResult<()> {
        let kernel = create_gaussian_kernel(radius as usize, sigma);
        let mut data = plane.data.to_vec();
        let original = data.clone();

        for y in 0..height {
            for x in 0..width {
                let value = self.apply_kernel(&original, x, y, width, height, &kernel, radius);
                let idx = (y * width + x) as usize;
                data[idx] = value;
            }
        }

        *plane = Plane::new(data, plane.stride);
        Ok(())
    }

    /// Apply convolution kernel.
    fn apply_kernel(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        kernel: &[f32],
        radius: u32,
    ) -> u8 {
        let mut sum = 0.0f32;
        let mut weight_sum = 0.0f32;
        let ksize = (radius * 2 + 1) as usize;

        for ky in 0..ksize {
            let py = y as i32 + ky as i32 - radius as i32;
            if py < 0 || py >= height as i32 {
                continue;
            }

            for kx in 0..ksize {
                let px = x as i32 + kx as i32 - radius as i32;
                if px < 0 || px >= width as i32 {
                    continue;
                }

                let idx = (py as u32 * width + px as u32) as usize;
                let weight = kernel[ky * ksize + kx];
                sum += data.get(idx).copied().unwrap_or(128) as f32 * weight;
                weight_sum += weight;
            }
        }

        if weight_sum > 0.0 {
            (sum / weight_sum).round().clamp(0.0, 255.0) as u8
        } else {
            128
        }
    }

    /// Apply median filtering.
    fn apply_median(&self, frame: &mut VideoFrame) -> GraphResult<()> {
        let (h_sub, v_sub) = frame.format.chroma_subsampling();

        for (plane_idx, plane) in frame.planes.iter_mut().enumerate() {
            let (width, height) = if plane_idx == 0 {
                (frame.width, frame.height)
            } else {
                (frame.width / h_sub, frame.height / v_sub)
            };

            let radius = self.config.spatial_radius;
            self.median_filter_plane(plane, width, height, radius)?;
        }

        Ok(())
    }

    /// Apply median filter to a single plane.
    fn median_filter_plane(
        &self,
        plane: &mut Plane,
        width: u32,
        height: u32,
        radius: u32,
    ) -> GraphResult<()> {
        let mut data = plane.data.to_vec();
        let original = data.clone();

        for y in 0..height {
            for x in 0..width {
                let mut values = Vec::new();

                let y_min = y.saturating_sub(radius);
                let y_max = (y + radius + 1).min(height);
                let x_min = x.saturating_sub(radius);
                let x_max = (x + radius + 1).min(width);

                for ny in y_min..y_max {
                    for nx in x_min..x_max {
                        let nidx = (ny * width + nx) as usize;
                        values.push(original.get(nidx).copied().unwrap_or(128));
                    }
                }

                values.sort_unstable();
                let median = if values.is_empty() {
                    128
                } else {
                    values[values.len() / 2]
                };

                let idx = (y * width + x) as usize;
                data[idx] = median;
            }
        }

        *plane = Plane::new(data, plane.stride);
        Ok(())
    }

    /// Apply adaptive filtering based on local variance.
    fn apply_adaptive(&self, frame: &mut VideoFrame) -> GraphResult<()> {
        let (h_sub, v_sub) = frame.format.chroma_subsampling();

        for (plane_idx, plane) in frame.planes.iter_mut().enumerate() {
            let (width, height) = if plane_idx == 0 {
                (frame.width, frame.height)
            } else {
                (frame.width / h_sub, frame.height / v_sub)
            };

            self.adaptive_filter_plane(plane, width, height)?;
        }

        Ok(())
    }

    /// Apply adaptive filter to a single plane.
    fn adaptive_filter_plane(&self, plane: &mut Plane, width: u32, height: u32) -> GraphResult<()> {
        let mut data = plane.data.to_vec();
        let original = data.clone();

        let radius = self.config.spatial_radius;

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let center_val = original.get(idx).copied().unwrap_or(128);

                let (_mean, variance) =
                    self.compute_local_statistics(&original, x, y, width, height, radius);

                let adaptive_strength = if variance < self.config.edge_threshold {
                    self.config.strength
                } else {
                    self.config.strength * 0.3
                };

                let filtered = self.bilateral_filter_pixel(
                    &original,
                    x,
                    y,
                    width,
                    height,
                    radius,
                    self.config.sigma_color,
                    self.config.sigma_space,
                );

                let blended = center_val as f32 * (1.0 - adaptive_strength)
                    + filtered as f32 * adaptive_strength;
                data[idx] = blended.round().clamp(0.0, 255.0) as u8;
            }
        }

        *plane = Plane::new(data, plane.stride);
        Ok(())
    }

    /// Compute local statistics (mean and variance).
    fn compute_local_statistics(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        radius: u32,
    ) -> (f32, f32) {
        let mut sum = 0.0f32;
        let mut sq_sum = 0.0f32;
        let mut count = 0;

        let y_min = y.saturating_sub(radius);
        let y_max = (y + radius + 1).min(height);
        let x_min = x.saturating_sub(radius);
        let x_max = (x + radius + 1).min(width);

        for ny in y_min..y_max {
            for nx in x_min..x_max {
                let nidx = (ny * width + nx) as usize;
                let val = data.get(nidx).copied().unwrap_or(128) as f32;
                sum += val;
                sq_sum += val * val;
                count += 1;
            }
        }

        if count > 0 {
            let mean = sum / count as f32;
            let variance = (sq_sum / count as f32) - (mean * mean);
            (mean, variance)
        } else {
            (128.0, 0.0)
        }
    }

    /// Bilateral filter for a single pixel.
    fn bilateral_filter_pixel(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        radius: u32,
        sigma_color: f32,
        sigma_space: f32,
    ) -> u8 {
        let center_val = data.get((y * width + x) as usize).copied().unwrap_or(128) as f32;

        let mut sum = 0.0f32;
        let mut weight_sum = 0.0f32;

        let color_coeff = -0.5 / (sigma_color * sigma_color);
        let space_coeff = -0.5 / (sigma_space * sigma_space);

        let y_min = y.saturating_sub(radius);
        let y_max = (y + radius + 1).min(height);
        let x_min = x.saturating_sub(radius);
        let x_max = (x + radius + 1).min(width);

        for ny in y_min..y_max {
            for nx in x_min..x_max {
                let nidx = (ny * width + nx) as usize;
                let neighbor_val = data.get(nidx).copied().unwrap_or(128) as f32;

                let color_dist = neighbor_val - center_val;
                let space_dist =
                    ((nx as i32 - x as i32).pow(2) + (ny as i32 - y as i32).pow(2)) as f32;

                let color_weight = (color_dist * color_dist * color_coeff).exp();
                let space_weight = (space_dist * space_coeff).exp();
                let weight = color_weight * space_weight;

                sum += neighbor_val * weight;
                weight_sum += weight;
            }
        }

        if weight_sum > 0.0 {
            (sum / weight_sum).round().clamp(0.0, 255.0) as u8
        } else {
            center_val as u8
        }
    }

    /// Apply temporal filtering.
    fn apply_temporal(&mut self, frame: &mut VideoFrame) -> GraphResult<()> {
        if self.frame_buffer.is_empty() {
            return Ok(());
        }

        match self.config.temporal_mode {
            TemporalMode::Average => self.temporal_average(frame),
            TemporalMode::Recursive => self.temporal_recursive(frame),
            TemporalMode::MotionCompensated => self.temporal_motion_compensated(frame),
            TemporalMode::WeightedAverage => self.temporal_weighted(frame),
        }
    }

    /// Simple temporal averaging.
    fn temporal_average(&self, frame: &mut VideoFrame) -> GraphResult<()> {
        let (h_sub, v_sub) = frame.format.chroma_subsampling();

        for (plane_idx, plane) in frame.planes.iter_mut().enumerate() {
            let (width, height) = if plane_idx == 0 {
                (frame.width, frame.height)
            } else {
                (frame.width / h_sub, frame.height / v_sub)
            };

            let mut data = plane.data.to_vec();

            for y in 0..height {
                for x in 0..width {
                    let idx = (y * width + x) as usize;
                    let mut sum = data.get(idx).copied().unwrap_or(128) as f32;
                    let mut count = 1;

                    for buffered_frame in &self.frame_buffer {
                        if let Some(buffered_plane) = buffered_frame.planes.get(plane_idx) {
                            sum += buffered_plane.data.get(idx).copied().unwrap_or(128) as f32;
                            count += 1;
                        }
                    }

                    data[idx] = (sum / count as f32).round().clamp(0.0, 255.0) as u8;
                }
            }

            *plane = Plane::new(data, plane.stride);
        }

        Ok(())
    }

    /// Recursive temporal filtering.
    fn temporal_recursive(&self, frame: &mut VideoFrame) -> GraphResult<()> {
        if let Some(ref prev) = self.prev_frame {
            let (h_sub, v_sub) = frame.format.chroma_subsampling();
            let alpha = self.config.recursive_alpha;

            for (plane_idx, plane) in frame.planes.iter_mut().enumerate() {
                let (width, height) = if plane_idx == 0 {
                    (frame.width, frame.height)
                } else {
                    (frame.width / h_sub, frame.height / v_sub)
                };

                let mut data = plane.data.to_vec();

                if let Some(prev_plane) = prev.planes.get(plane_idx) {
                    for idx in 0..(width * height) as usize {
                        let current = data.get(idx).copied().unwrap_or(128) as f32;
                        let previous = prev_plane.data.get(idx).copied().unwrap_or(128) as f32;

                        let filtered = current * alpha + previous * (1.0 - alpha);
                        data[idx] = filtered.round().clamp(0.0, 255.0) as u8;
                    }
                }

                *plane = Plane::new(data, plane.stride);
            }
        }

        Ok(())
    }

    /// Motion-compensated temporal filtering.
    fn temporal_motion_compensated(&self, frame: &mut VideoFrame) -> GraphResult<()> {
        if self.frame_buffer.is_empty() {
            return Ok(());
        }

        let (h_sub, v_sub) = frame.format.chroma_subsampling();

        for (plane_idx, plane) in frame.planes.iter_mut().enumerate() {
            let (width, height) = if plane_idx == 0 {
                (frame.width, frame.height)
            } else {
                (frame.width / h_sub, frame.height / v_sub)
            };

            let mut data = plane.data.to_vec();
            let current_data = data.clone();

            let block_size = match self.config.motion_quality {
                MotionQuality::Fast => 16,
                MotionQuality::Medium => 8,
                MotionQuality::High => 4,
            };

            for by in (0..height).step_by(block_size as usize) {
                for bx in (0..width).step_by(block_size as usize) {
                    let bw = block_size.min(width - bx);
                    let bh = block_size.min(height - by);

                    let mut sum_block = vec![0.0f32; (bw * bh) as usize];
                    let mut count = 1;

                    for py in 0..bh {
                        for px in 0..bw {
                            let idx = ((by + py) * width + (bx + px)) as usize;
                            sum_block[(py * bw + px) as usize] =
                                current_data.get(idx).copied().unwrap_or(128) as f32;
                        }
                    }

                    for buffered_frame in &self.frame_buffer {
                        if let Some(buffered_plane) = buffered_frame.planes.get(plane_idx) {
                            let (mv_x, mv_y) = self.estimate_motion(
                                &current_data,
                                &buffered_plane.data,
                                bx,
                                by,
                                bw,
                                bh,
                                width,
                                height,
                            );

                            for py in 0..bh {
                                for px in 0..bw {
                                    let src_x = (bx as i32 + px as i32 + mv_x)
                                        .clamp(0, width as i32 - 1)
                                        as u32;
                                    let src_y = (by as i32 + py as i32 + mv_y)
                                        .clamp(0, height as i32 - 1)
                                        as u32;
                                    let src_idx = (src_y * width + src_x) as usize;

                                    sum_block[(py * bw + px) as usize] +=
                                        buffered_plane.data.get(src_idx).copied().unwrap_or(128)
                                            as f32;
                                }
                            }
                            count += 1;
                        }
                    }

                    for py in 0..bh {
                        for px in 0..bw {
                            let idx = ((by + py) * width + (bx + px)) as usize;
                            let avg = sum_block[(py * bw + px) as usize] / count as f32;
                            data[idx] = avg.round().clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }

            *plane = Plane::new(data, plane.stride);
        }

        Ok(())
    }

    /// Estimate motion vector for a block.
    fn estimate_motion(
        &self,
        current: &[u8],
        reference: &[u8],
        bx: u32,
        by: u32,
        bw: u32,
        bh: u32,
        width: u32,
        height: u32,
    ) -> (i32, i32) {
        let search_range = match self.config.motion_quality {
            MotionQuality::Fast => 8,
            MotionQuality::Medium => 16,
            MotionQuality::High => 32,
        };

        let mut best_mv = (0i32, 0i32);
        let mut best_sad = f32::INFINITY;

        for mv_y in -search_range..=search_range {
            for mv_x in -search_range..=search_range {
                let ref_x = bx as i32 + mv_x;
                let ref_y = by as i32 + mv_y;

                if ref_x < 0
                    || ref_y < 0
                    || ref_x + bw as i32 > width as i32
                    || ref_y + bh as i32 > height as i32
                {
                    continue;
                }

                let sad = self.compute_sad(
                    current,
                    reference,
                    bx,
                    by,
                    ref_x as u32,
                    ref_y as u32,
                    bw,
                    bh,
                    width,
                );

                if sad < best_sad {
                    best_sad = sad;
                    best_mv = (mv_x, mv_y);
                }
            }
        }

        best_mv
    }

    /// Compute sum of absolute differences (SAD).
    fn compute_sad(
        &self,
        current: &[u8],
        reference: &[u8],
        cur_x: u32,
        cur_y: u32,
        ref_x: u32,
        ref_y: u32,
        bw: u32,
        bh: u32,
        width: u32,
    ) -> f32 {
        let mut sad = 0.0f32;

        for y in 0..bh {
            for x in 0..bw {
                let cur_idx = ((cur_y + y) * width + (cur_x + x)) as usize;
                let ref_idx = ((ref_y + y) * width + (ref_x + x)) as usize;

                let cur_val = current.get(cur_idx).copied().unwrap_or(128) as f32;
                let ref_val = reference.get(ref_idx).copied().unwrap_or(128) as f32;

                sad += (cur_val - ref_val).abs();
            }
        }

        sad
    }

    /// Weighted temporal averaging based on similarity.
    fn temporal_weighted(&self, frame: &mut VideoFrame) -> GraphResult<()> {
        if self.frame_buffer.is_empty() {
            return Ok(());
        }

        let (h_sub, v_sub) = frame.format.chroma_subsampling();

        for (plane_idx, plane) in frame.planes.iter_mut().enumerate() {
            let (width, height) = if plane_idx == 0 {
                (frame.width, frame.height)
            } else {
                (frame.width / h_sub, frame.height / v_sub)
            };

            let mut data = plane.data.to_vec();
            let current_data = data.clone();

            for y in 0..height {
                for x in 0..width {
                    let idx = (y * width + x) as usize;
                    let current_val = current_data.get(idx).copied().unwrap_or(128) as f32;

                    let mut sum = current_val;
                    let mut weight_sum = 1.0f32;

                    for buffered_frame in &self.frame_buffer {
                        if let Some(buffered_plane) = buffered_frame.planes.get(plane_idx) {
                            let buffered_val =
                                buffered_plane.data.get(idx).copied().unwrap_or(128) as f32;

                            let diff = (current_val - buffered_val).abs();
                            let weight = (-diff / (self.config.sigma_color * 0.5)).exp();

                            sum += buffered_val * weight;
                            weight_sum += weight;
                        }
                    }

                    data[idx] = (sum / weight_sum).round().clamp(0.0, 255.0) as u8;
                }
            }

            *plane = Plane::new(data, plane.stride);
        }

        Ok(())
    }

    /// Apply motion-compensated temporal filtering (same as in temporal mode).
    fn apply_motion_compensated(&mut self, frame: &mut VideoFrame) -> GraphResult<()> {
        self.temporal_motion_compensated(frame)
    }

    /// Apply BM3D-inspired denoising.
    fn apply_bm3d(&self, frame: &mut VideoFrame) -> GraphResult<()> {
        let (h_sub, v_sub) = frame.format.chroma_subsampling();

        for (plane_idx, plane) in frame.planes.iter_mut().enumerate() {
            let (width, height) = if plane_idx == 0 {
                (frame.width, frame.height)
            } else {
                (frame.width / h_sub, frame.height / v_sub)
            };

            self.bm3d_filter_plane(plane, width, height)?;
        }

        Ok(())
    }

    /// Apply BM3D filter to a single plane.
    fn bm3d_filter_plane(&self, plane: &mut Plane, width: u32, height: u32) -> GraphResult<()> {
        let mut data = plane.data.to_vec();
        let original = data.clone();

        let block_size = self.config.patch_size;
        let search_window = self.config.search_window;
        let max_similar_blocks = 16;

        for by in (0..height).step_by(block_size as usize) {
            for bx in (0..width).step_by(block_size as usize) {
                let bw = block_size.min(width - bx);
                let bh = block_size.min(height - by);

                let similar_blocks = self.find_similar_blocks(
                    &original,
                    bx,
                    by,
                    bw,
                    bh,
                    width,
                    height,
                    search_window,
                    max_similar_blocks,
                );

                for py in 0..bh {
                    for px in 0..bw {
                        let idx = ((by + py) * width + (bx + px)) as usize;
                        let mut sum = 0.0f32;
                        let mut weight_sum = 0.0f32;

                        for (block_x, block_y, weight) in &similar_blocks {
                            let src_idx = ((block_y + py) * width + (block_x + px)) as usize;
                            sum += original.get(src_idx).copied().unwrap_or(128) as f32 * weight;
                            weight_sum += weight;
                        }

                        if weight_sum > 0.0 {
                            data[idx] = (sum / weight_sum).round().clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        }

        *plane = Plane::new(data, plane.stride);
        Ok(())
    }

    /// Find similar blocks for BM3D.
    fn find_similar_blocks(
        &self,
        data: &[u8],
        bx: u32,
        by: u32,
        bw: u32,
        bh: u32,
        width: u32,
        height: u32,
        search_window: u32,
        max_blocks: usize,
    ) -> Vec<(u32, u32, f32)> {
        let mut candidates = Vec::new();

        let search_radius = search_window / 2;
        let y_min = by.saturating_sub(search_radius);
        let y_max = (by + search_radius).min(height - bh);
        let x_min = bx.saturating_sub(search_radius);
        let x_max = (bx + search_radius).min(width - bw);

        for sy in (y_min..=y_max).step_by(bw as usize) {
            for sx in (x_min..=x_max).step_by(bw as usize) {
                let dist = self.block_distance(data, bx, by, sx, sy, bw, bh, width);
                let weight = (-dist / (self.config.nlm_h * self.config.nlm_h)).exp();
                candidates.push((sx, sy, weight));
            }
        }

        candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(max_blocks);
        candidates
    }

    /// Compute block distance.
    fn block_distance(
        &self,
        data: &[u8],
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        bw: u32,
        bh: u32,
        width: u32,
    ) -> f32 {
        let mut dist = 0.0f32;
        let mut count = 0;

        for y in 0..bh {
            for x in 0..bw {
                let idx1 = ((y1 + y) * width + (x1 + x)) as usize;
                let idx2 = ((y2 + y) * width + (x2 + x)) as usize;

                let v1 = data.get(idx1).copied().unwrap_or(128) as f32;
                let v2 = data.get(idx2).copied().unwrap_or(128) as f32;

                let diff = v1 - v2;
                dist += diff * diff;
                count += 1;
            }
        }

        if count > 0 {
            dist / count as f32
        } else {
            0.0
        }
    }

    /// Apply combined spatial and temporal filtering.
    fn apply_combined(&mut self, frame: &mut VideoFrame) -> GraphResult<()> {
        self.apply_bilateral(frame)?;

        if !self.frame_buffer.is_empty() {
            self.apply_temporal(frame)?;
        }

        Ok(())
    }

    /// Get strength for a specific plane (luma vs chroma).
    fn get_plane_strength(&self, plane_idx: usize) -> f32 {
        let multiplier = if plane_idx == 0 {
            self.config.luma_strength
        } else {
            self.config.chroma_strength
        };
        self.config.strength * multiplier
    }
}

impl Node for DenoiseFilter {
    fn id(&self) -> NodeId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> NodeType {
        NodeType::Filter
    }

    fn state(&self) -> NodeState {
        self.state
    }

    fn set_state(&mut self, state: NodeState) -> GraphResult<()> {
        if !self.state.can_transition_to(state) {
            return Err(GraphError::InvalidStateTransition {
                node: self.id,
                from: self.state.to_string(),
                to: state.to_string(),
            });
        }
        self.state = state;
        Ok(())
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn process(&mut self, input: Option<FilterFrame>) -> GraphResult<Option<FilterFrame>> {
        match input {
            Some(FilterFrame::Video(frame)) => {
                let processed = self.process_frame(frame)?;
                Ok(Some(FilterFrame::Video(processed)))
            }
            Some(_) => Err(GraphError::PortTypeMismatch {
                expected: "Video".to_string(),
                actual: "Audio".to_string(),
            }),
            None => Ok(None),
        }
    }

    fn reset(&mut self) -> GraphResult<()> {
        self.frame_buffer.clear();
        self.prev_frame = None;
        self.noise_stats = None;
        self.set_state(NodeState::Idle)
    }
}

/// Create a Gaussian kernel for filtering.
fn create_gaussian_kernel(radius: usize, sigma: f32) -> Vec<f32> {
    let size = radius * 2 + 1;
    let mut kernel = vec![0.0f32; size * size];
    let mut sum = 0.0f32;

    let sigma_sq = sigma * sigma;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - radius as f32;
            let dy = y as f32 - radius as f32;
            let dist_sq = dx * dx + dy * dy;

            let val = (-dist_sq / (2.0 * sigma_sq)).exp();
            kernel[y * size + x] = val;
            sum += val;
        }
    }

    for val in &mut kernel {
        *val /= sum;
    }

    kernel
}

/// Noise statistics for adaptive processing.
#[derive(Debug, Clone)]
struct NoiseStatistics {
    /// Estimated noise standard deviation for luma.
    luma_noise_sigma: f32,
    /// Estimated noise standard deviation for chroma.
    chroma_noise_sigma: f32,
    /// Noise level (0.0 = clean, 1.0 = very noisy).
    noise_level: f32,
}

impl NoiseStatistics {
    /// Estimate noise statistics from a frame.
    fn estimate(frame: &VideoFrame) -> Self {
        let luma_noise_sigma = if let Some(plane) = frame.planes.first() {
            Self::estimate_plane_noise(&plane.data, frame.width, frame.height)
        } else {
            0.0
        };

        let chroma_noise_sigma = if frame.planes.len() > 1 {
            let (h_sub, v_sub) = frame.format.chroma_subsampling();
            let chroma_width = frame.width / h_sub;
            let chroma_height = frame.height / v_sub;

            let u_sigma = if let Some(plane) = frame.planes.get(1) {
                Self::estimate_plane_noise(&plane.data, chroma_width, chroma_height)
            } else {
                0.0
            };

            let v_sigma = if let Some(plane) = frame.planes.get(2) {
                Self::estimate_plane_noise(&plane.data, chroma_width, chroma_height)
            } else {
                0.0
            };

            (u_sigma + v_sigma) / 2.0
        } else {
            0.0
        };

        let noise_level = (luma_noise_sigma / 50.0).clamp(0.0, 1.0);

        Self {
            luma_noise_sigma,
            chroma_noise_sigma,
            noise_level,
        }
    }

    /// Estimate noise in a single plane using median absolute deviation.
    fn estimate_plane_noise(data: &[u8], width: u32, height: u32) -> f32 {
        let mut diffs = Vec::new();

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let idx = (y * width + x) as usize;
                let center = data.get(idx).copied().unwrap_or(128) as i32;

                let right = data.get(idx + 1).copied().unwrap_or(128) as i32;
                let bottom = data
                    .get((idx as u32 + width) as usize)
                    .copied()
                    .unwrap_or(128) as i32;

                diffs.push((center - right).abs());
                diffs.push((center - bottom).abs());
            }
        }

        if diffs.is_empty() {
            return 0.0;
        }

        diffs.sort_unstable();
        let median = diffs[diffs.len() / 2] as f32;

        median / 0.6745
    }
}

/// Denoise presets for common use cases.
pub mod presets {
    use super::{DenoiseConfig, DenoiseMethod, MotionQuality, TemporalMode};

    /// Light denoising for slightly noisy footage.
    #[must_use]
    pub fn light() -> DenoiseConfig {
        DenoiseConfig::new()
            .with_method(DenoiseMethod::Bilateral)
            .with_strength(0.4)
            .with_spatial_radius(3)
            .with_bilateral_params(30.0, 8.0)
    }

    /// Medium denoising for moderately noisy footage.
    #[must_use]
    pub fn medium() -> DenoiseConfig {
        DenoiseConfig::new()
            .with_method(DenoiseMethod::Combined)
            .with_strength(0.7)
            .with_spatial_radius(5)
            .with_temporal_depth(3)
            .with_bilateral_params(50.0, 10.0)
    }

    /// Strong denoising for very noisy footage.
    #[must_use]
    pub fn strong() -> DenoiseConfig {
        DenoiseConfig::new()
            .with_method(DenoiseMethod::NonLocalMeans)
            .with_strength(0.9)
            .with_spatial_radius(7)
            .with_nlm_params(15.0, 21, 7)
    }

    /// Temporal-only denoising.
    #[must_use]
    pub fn temporal() -> DenoiseConfig {
        DenoiseConfig::new()
            .with_method(DenoiseMethod::Temporal)
            .with_strength(0.7)
            .with_temporal_depth(5)
            .with_temporal_mode(TemporalMode::WeightedAverage)
    }

    /// Motion-compensated temporal denoising.
    #[must_use]
    pub fn motion_compensated() -> DenoiseConfig {
        DenoiseConfig::new()
            .with_method(DenoiseMethod::MotionCompensated)
            .with_strength(0.8)
            .with_temporal_depth(3)
            .with_motion_quality(MotionQuality::High)
    }

    /// BM3D-style denoising.
    #[must_use]
    pub fn bm3d() -> DenoiseConfig {
        DenoiseConfig::new()
            .with_method(DenoiseMethod::BlockMatching3D)
            .with_strength(0.8)
            .with_spatial_radius(8)
            .with_nlm_params(12.0, 21, 8)
    }

    /// Chroma-only denoising.
    #[must_use]
    pub fn chroma_only() -> DenoiseConfig {
        DenoiseConfig::new()
            .with_method(DenoiseMethod::Bilateral)
            .with_strength(0.8)
            .with_luma_strength(0.0)
            .with_chroma_strength(1.5)
            .with_spatial_radius(5)
    }

    /// Fast denoising (lower quality, faster).
    #[must_use]
    pub fn fast() -> DenoiseConfig {
        DenoiseConfig::new()
            .with_method(DenoiseMethod::Gaussian)
            .with_strength(0.6)
            .with_spatial_radius(3)
    }
}

/// Edge detection utilities for edge-preserving filtering.
pub mod edge {
    /// Detect edges using Sobel operator.
    #[must_use]
    pub fn detect_sobel(data: &[u8], width: u32, height: u32) -> Vec<f32> {
        let mut edges = vec![0.0f32; (width * height) as usize];

        let sobel_x = [-1, 0, 1, -2, 0, 2, -1, 0, 1];
        let sobel_y = [-1, -2, -1, 0, 0, 0, 1, 2, 1];

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let mut gx = 0.0f32;
                let mut gy = 0.0f32;

                for ky in 0..3 {
                    for kx in 0..3 {
                        let nx = x + kx - 1;
                        let ny = y + ky - 1;
                        let nidx = (ny * width + nx) as usize;
                        let val = data.get(nidx).copied().unwrap_or(128) as f32;

                        let kidx = (ky * 3 + kx) as usize;
                        gx += val * sobel_x[kidx] as f32;
                        gy += val * sobel_y[kidx] as f32;
                    }
                }

                let magnitude = (gx * gx + gy * gy).sqrt();
                edges[(y * width + x) as usize] = magnitude;
            }
        }

        edges
    }

    /// Detect edges using Laplacian operator.
    #[must_use]
    pub fn detect_laplacian(data: &[u8], width: u32, height: u32) -> Vec<f32> {
        let mut edges = vec![0.0f32; (width * height) as usize];

        let laplacian = [0, -1, 0, -1, 4, -1, 0, -1, 0];

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let mut sum = 0.0f32;

                for ky in 0..3 {
                    for kx in 0..3 {
                        let nx = x + kx - 1;
                        let ny = y + ky - 1;
                        let nidx = (ny * width + nx) as usize;
                        let val = data.get(nidx).copied().unwrap_or(128) as f32;

                        let kidx = (ky * 3 + kx) as usize;
                        sum += val * laplacian[kidx] as f32;
                    }
                }

                edges[(y * width + x) as usize] = sum.abs();
            }
        }

        edges
    }

    /// Compute edge map with threshold.
    #[must_use]
    pub fn edge_map(edges: &[f32], threshold: f32) -> Vec<bool> {
        edges.iter().map(|&e| e > threshold).collect()
    }

    /// Non-maximum suppression for edge thinning.
    pub fn non_maximum_suppression(edges: &mut [f32], width: u32, height: u32) {
        let original = edges.to_vec();

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let idx = (y * width + x) as usize;
                let val = original[idx];

                let neighbors = [
                    original.get((idx - 1) as usize).copied().unwrap_or(0.0),
                    original.get((idx + 1) as usize).copied().unwrap_or(0.0),
                    original
                        .get((idx as u32 - width) as usize)
                        .copied()
                        .unwrap_or(0.0),
                    original
                        .get((idx as u32 + width) as usize)
                        .copied()
                        .unwrap_or(0.0),
                ];

                if !neighbors.iter().all(|&n| val >= n) {
                    edges[idx] = 0.0;
                }
            }
        }
    }
}

/// Quality metrics for evaluating denoise results.
pub mod metrics {
    /// Compute PSNR between original and denoised frames.
    #[must_use]
    pub fn psnr(original: &[u8], denoised: &[u8]) -> f32 {
        if original.len() != denoised.len() || original.is_empty() {
            return 0.0;
        }

        let mut mse = 0.0f64;
        for (a, b) in original.iter().zip(denoised.iter()) {
            let diff = *a as f64 - *b as f64;
            mse += diff * diff;
        }

        mse /= original.len() as f64;

        if mse < 1e-10 {
            return f32::INFINITY;
        }

        let max_val = 255.0;
        (10.0 * (max_val * max_val / mse).log10()) as f32
    }

    /// Compute SNR (Signal-to-Noise Ratio).
    #[must_use]
    pub fn snr(original: &[u8], denoised: &[u8]) -> f32 {
        if original.len() != denoised.len() || original.is_empty() {
            return 0.0;
        }

        let mut signal_power = 0.0f64;
        let mut noise_power = 0.0f64;

        for (a, b) in original.iter().zip(denoised.iter()) {
            let signal = *a as f64;
            let noise = (*a as f64 - *b as f64).abs();

            signal_power += signal * signal;
            noise_power += noise * noise;
        }

        if noise_power < 1e-10 {
            return f32::INFINITY;
        }

        (10.0 * (signal_power / noise_power).log10()) as f32
    }

    /// Compute mean absolute error.
    #[must_use]
    pub fn mae(original: &[u8], denoised: &[u8]) -> f32 {
        if original.len() != denoised.len() || original.is_empty() {
            return 0.0;
        }

        let sum: f32 = original
            .iter()
            .zip(denoised.iter())
            .map(|(a, b)| (*a as f32 - *b as f32).abs())
            .sum();

        sum / original.len() as f32
    }

    /// Compute structural similarity index (SSIM).
    #[must_use]
    pub fn ssim(original: &[u8], denoised: &[u8], width: usize, height: usize) -> f32 {
        if original.len() != denoised.len() || original.is_empty() {
            return 0.0;
        }

        let c1 = (0.01 * 255.0) * (0.01 * 255.0);
        let c2 = (0.03 * 255.0) * (0.03 * 255.0);

        let window_size = 11usize;
        let half = window_size / 2;

        let mut ssim_sum = 0.0f64;
        let mut count = 0;

        for y in half..(height - half) {
            for x in half..(width - half) {
                let (mean_x, mean_y, var_x, var_y, cov_xy) =
                    compute_window_stats(original, denoised, x, y, width, window_size);

                let ssim_val = ((2.0 * mean_x * mean_y + c1) * (2.0 * cov_xy + c2))
                    / ((mean_x * mean_x + mean_y * mean_y + c1) * (var_x + var_y + c2));

                ssim_sum += ssim_val;
                count += 1;
            }
        }

        if count > 0 {
            (ssim_sum / count as f64) as f32
        } else {
            0.0
        }
    }

    fn compute_window_stats(
        img1: &[u8],
        img2: &[u8],
        cx: usize,
        cy: usize,
        width: usize,
        window_size: usize,
    ) -> (f64, f64, f64, f64, f64) {
        let half = window_size / 2;
        let mut sum_x = 0.0f64;
        let mut sum_y = 0.0f64;
        let mut sum_xx = 0.0f64;
        let mut sum_yy = 0.0f64;
        let mut sum_xy = 0.0f64;
        let mut count = 0;

        for dy in 0..window_size {
            let y = cy + dy - half;
            for dx in 0..window_size {
                let x = cx + dx - half;
                let idx = y * width + x;

                let val_x = img1.get(idx).copied().unwrap_or(0) as f64;
                let val_y = img2.get(idx).copied().unwrap_or(0) as f64;

                sum_x += val_x;
                sum_y += val_y;
                sum_xx += val_x * val_x;
                sum_yy += val_y * val_y;
                sum_xy += val_x * val_y;
                count += 1;
            }
        }

        let n = count as f64;
        let mean_x = sum_x / n;
        let mean_y = sum_y / n;
        let var_x = sum_xx / n - mean_x * mean_x;
        let var_y = sum_yy / n - mean_y * mean_y;
        let cov_xy = sum_xy / n - mean_x * mean_y;

        (mean_x, mean_y, var_x, var_y, cov_xy)
    }
}

/// Utility functions for noise analysis.
pub mod analysis {
    /// Estimate noise level in a frame.
    #[must_use]
    pub fn estimate_noise_level(data: &[u8], width: u32, height: u32) -> f32 {
        super::NoiseStatistics::estimate_plane_noise(data, width, height)
    }

    /// Compute local variance map.
    #[must_use]
    pub fn variance_map(data: &[u8], width: u32, height: u32, radius: u32) -> Vec<f32> {
        let mut variance = vec![0.0f32; (width * height) as usize];

        for y in 0..height {
            for x in 0..width {
                let (_, var) = compute_local_stats(data, x, y, width, height, radius);
                variance[(y * width + x) as usize] = var;
            }
        }

        variance
    }

    fn compute_local_stats(
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        radius: u32,
    ) -> (f32, f32) {
        let mut sum = 0.0f32;
        let mut sq_sum = 0.0f32;
        let mut count = 0;

        let y_min = y.saturating_sub(radius);
        let y_max = (y + radius + 1).min(height);
        let x_min = x.saturating_sub(radius);
        let x_max = (x + radius + 1).min(width);

        for ny in y_min..y_max {
            for nx in x_min..x_max {
                let nidx = (ny * width + nx) as usize;
                let val = data.get(nidx).copied().unwrap_or(128) as f32;
                sum += val;
                sq_sum += val * val;
                count += 1;
            }
        }

        if count > 0 {
            let mean = sum / count as f32;
            let variance = (sq_sum / count as f32) - (mean * mean);
            (mean, variance)
        } else {
            (128.0, 0.0)
        }
    }

    /// Classify noise type based on characteristics.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum NoiseType {
        /// Gaussian white noise.
        Gaussian,
        /// Salt and pepper (impulse) noise.
        Impulse,
        /// Film grain noise.
        FilmGrain,
        /// Compression artifacts.
        Compression,
        /// Unknown or mixed noise.
        Unknown,
    }

    /// Classify the type of noise in a frame.
    #[must_use]
    pub fn classify_noise(data: &[u8], width: u32, height: u32) -> NoiseType {
        let noise_sigma = estimate_noise_level(data, width, height);

        if noise_sigma < 5.0 {
            return NoiseType::Compression;
        }

        let mut histogram = [0u32; 256];
        for &val in data {
            histogram[val as usize] += 1;
        }

        let total = data.len() as f32;
        let extreme_ratio = (histogram[0] + histogram[255]) as f32 / total;

        if extreme_ratio > 0.05 {
            NoiseType::Impulse
        } else if noise_sigma > 20.0 {
            NoiseType::FilmGrain
        } else if noise_sigma > 5.0 && noise_sigma < 20.0 {
            NoiseType::Gaussian
        } else {
            NoiseType::Unknown
        }
    }

    /// Get recommended config for detected noise type.
    #[must_use]
    pub fn recommend_config(noise_type: NoiseType) -> super::DenoiseConfig {
        match noise_type {
            NoiseType::Gaussian => super::presets::medium(),
            NoiseType::Impulse => super::DenoiseConfig::new()
                .with_method(super::DenoiseMethod::Median)
                .with_strength(0.8)
                .with_spatial_radius(3),
            NoiseType::FilmGrain => super::presets::strong(),
            NoiseType::Compression => super::DenoiseConfig::new()
                .with_method(super::DenoiseMethod::Bilateral)
                .with_strength(0.5)
                .with_spatial_radius(3),
            NoiseType::Unknown => super::presets::medium(),
        }
    }
}

/// GPU acceleration hints for denoise operations.
pub mod gpu {
    use super::DenoiseMethod;

    /// GPU acceleration availability.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum GpuSupport {
        /// No GPU support.
        None,
        /// CUDA support.
        Cuda,
        /// OpenCL support.
        OpenCl,
        /// Vulkan support.
        Vulkan,
        /// Metal support (macOS/iOS).
        Metal,
    }

    /// GPU compute kernel hint.
    #[derive(Debug, Clone)]
    pub struct GpuKernelHint {
        /// Method being accelerated.
        pub method: DenoiseMethod,
        /// Recommended workgroup size.
        pub workgroup_size: (u32, u32),
        /// Memory requirements (bytes).
        pub memory_requirements: usize,
        /// Expected speedup factor.
        pub speedup_factor: f32,
    }

    impl GpuKernelHint {
        /// Get GPU kernel hint for a method.
        #[must_use]
        pub fn for_method(method: DenoiseMethod) -> Self {
            match method {
                DenoiseMethod::Bilateral => Self {
                    method,
                    workgroup_size: (16, 16),
                    memory_requirements: 1024 * 1024,
                    speedup_factor: 8.0,
                },
                DenoiseMethod::NonLocalMeans => Self {
                    method,
                    workgroup_size: (8, 8),
                    memory_requirements: 4 * 1024 * 1024,
                    speedup_factor: 15.0,
                },
                DenoiseMethod::Gaussian => Self {
                    method,
                    workgroup_size: (32, 8),
                    memory_requirements: 512 * 1024,
                    speedup_factor: 5.0,
                },
                DenoiseMethod::Median => Self {
                    method,
                    workgroup_size: (16, 16),
                    memory_requirements: 2 * 1024 * 1024,
                    speedup_factor: 6.0,
                },
                DenoiseMethod::Adaptive => Self {
                    method,
                    workgroup_size: (16, 16),
                    memory_requirements: 2 * 1024 * 1024,
                    speedup_factor: 7.0,
                },
                DenoiseMethod::MotionCompensated => Self {
                    method,
                    workgroup_size: (8, 8),
                    memory_requirements: 8 * 1024 * 1024,
                    speedup_factor: 20.0,
                },
                DenoiseMethod::BlockMatching3D => Self {
                    method,
                    workgroup_size: (8, 8),
                    memory_requirements: 16 * 1024 * 1024,
                    speedup_factor: 25.0,
                },
                _ => Self {
                    method,
                    workgroup_size: (16, 16),
                    memory_requirements: 1024 * 1024,
                    speedup_factor: 3.0,
                },
            }
        }

        /// Check if GPU is worth using for given resolution.
        #[must_use]
        pub fn is_worthwhile(&self, width: u32, height: u32) -> bool {
            let pixels = width * height;
            pixels > 1920 * 1080 / 4
        }
    }

    /// Detect available GPU support.
    #[must_use]
    #[allow(unexpected_cfgs)]
    pub fn detect_gpu_support() -> Vec<GpuSupport> {
        let mut support = Vec::new();

        if cfg!(feature = "cuda") {
            support.push(GpuSupport::Cuda);
        }

        if cfg!(feature = "opencl") {
            support.push(GpuSupport::OpenCl);
        }

        if cfg!(feature = "vulkan") {
            support.push(GpuSupport::Vulkan);
        }

        if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
            support.push(GpuSupport::Metal);
        }

        if support.is_empty() {
            support.push(GpuSupport::None);
        }

        support
    }
}

/// Benchmarking utilities for denoise performance.
pub mod bench {
    use super::{DenoiseConfig, DenoiseMethod};
    use std::time::Duration;

    /// Benchmark result for a denoise operation.
    #[derive(Debug, Clone)]
    pub struct BenchmarkResult {
        /// Method being benchmarked.
        pub method: DenoiseMethod,
        /// Frame resolution.
        pub resolution: (u32, u32),
        /// Processing time.
        pub duration: Duration,
        /// Frames per second.
        pub fps: f32,
        /// Megapixels per second.
        pub mpixels_per_sec: f32,
    }

    impl BenchmarkResult {
        /// Create a benchmark result.
        #[must_use]
        pub fn new(method: DenoiseMethod, width: u32, height: u32, duration: Duration) -> Self {
            let pixels = (width * height) as f32;
            let seconds = duration.as_secs_f32();
            let fps = if seconds > 0.0 { 1.0 / seconds } else { 0.0 };
            let mpixels_per_sec = if seconds > 0.0 {
                pixels / (seconds * 1_000_000.0)
            } else {
                0.0
            };

            Self {
                method,
                resolution: (width, height),
                duration,
                fps,
                mpixels_per_sec,
            }
        }

        /// Format result as a string.
        #[must_use]
        pub fn format(&self) -> String {
            format!(
                "{:?} @ {}x{}: {:.2}ms ({:.2} fps, {:.2} MP/s)",
                self.method,
                self.resolution.0,
                self.resolution.1,
                self.duration.as_secs_f64() * 1000.0,
                self.fps,
                self.mpixels_per_sec
            )
        }
    }

    /// Benchmark multiple denoise methods.
    #[must_use]
    pub fn compare_methods(width: u32, height: u32) -> Vec<BenchmarkResult> {
        let methods = [
            DenoiseMethod::Gaussian,
            DenoiseMethod::Bilateral,
            DenoiseMethod::Median,
            DenoiseMethod::NonLocalMeans,
            DenoiseMethod::Adaptive,
        ];

        methods
            .iter()
            .map(|&method| {
                let config = DenoiseConfig::new().with_method(method);
                estimate_performance(&config, width, height)
            })
            .collect()
    }

    /// Estimate performance for a config.
    #[must_use]
    pub fn estimate_performance(
        config: &DenoiseConfig,
        width: u32,
        height: u32,
    ) -> BenchmarkResult {
        let pixels = (width * height) as f32;

        let base_time_per_pixel = match config.method {
            DenoiseMethod::Gaussian => 0.5,
            DenoiseMethod::Bilateral => 2.0,
            DenoiseMethod::Median => 3.0,
            DenoiseMethod::NonLocalMeans => 10.0,
            DenoiseMethod::Adaptive => 4.0,
            DenoiseMethod::Temporal => 1.5,
            DenoiseMethod::MotionCompensated => 8.0,
            DenoiseMethod::BlockMatching3D => 15.0,
            DenoiseMethod::Combined => 5.0,
        };

        let radius_factor = (config.spatial_radius as f32 / 5.0).powi(2);
        let time_ns = pixels * base_time_per_pixel * radius_factor;
        let duration = Duration::from_nanos(time_ns as u64);

        BenchmarkResult::new(config.method, width, height, duration)
    }
}

/// Temporal coherence tracking for better temporal filtering.
pub mod temporal {

    /// Motion vector.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct MotionVector {
        /// Horizontal displacement.
        pub x: i32,
        /// Vertical displacement.
        pub y: i32,
        /// Confidence (0.0-1.0).
        pub confidence: f32,
    }

    impl MotionVector {
        /// Create a new motion vector.
        #[must_use]
        pub fn new(x: i32, y: i32, confidence: f32) -> Self {
            Self { x, y, confidence }
        }

        /// Get magnitude.
        #[must_use]
        pub fn magnitude(&self) -> f32 {
            ((self.x * self.x + self.y * self.y) as f32).sqrt()
        }

        /// Check if this is a zero vector.
        #[must_use]
        pub fn is_zero(&self) -> bool {
            self.x == 0 && self.y == 0
        }
    }

    /// Motion field for a frame.
    #[derive(Debug, Clone)]
    pub struct MotionField {
        /// Width in blocks.
        pub width: u32,
        /// Height in blocks.
        pub height: u32,
        /// Block size.
        pub block_size: u32,
        /// Motion vectors.
        pub vectors: Vec<MotionVector>,
    }

    impl MotionField {
        /// Create a new motion field.
        #[must_use]
        pub fn new(frame_width: u32, frame_height: u32, block_size: u32) -> Self {
            let width = frame_width.div_ceil(block_size);
            let height = frame_height.div_ceil(block_size);
            let vectors = vec![MotionVector::new(0, 0, 1.0); (width * height) as usize];

            Self {
                width,
                height,
                block_size,
                vectors,
            }
        }

        /// Get motion vector for a block.
        #[must_use]
        pub fn get(&self, bx: u32, by: u32) -> MotionVector {
            if bx >= self.width || by >= self.height {
                return MotionVector::new(0, 0, 0.0);
            }
            self.vectors[(by * self.width + bx) as usize]
        }

        /// Set motion vector for a block.
        pub fn set(&mut self, bx: u32, by: u32, mv: MotionVector) {
            if bx < self.width && by < self.height {
                self.vectors[(by * self.width + bx) as usize] = mv;
            }
        }

        /// Get average motion magnitude.
        #[must_use]
        pub fn average_magnitude(&self) -> f32 {
            let sum: f32 = self.vectors.iter().map(|mv| mv.magnitude()).sum();
            sum / self.vectors.len() as f32
        }

        /// Smooth motion field.
        pub fn smooth(&mut self, iterations: usize) {
            for _ in 0..iterations {
                let original = self.vectors.clone();

                for by in 0..self.height {
                    for bx in 0..self.width {
                        let mut sum_x = 0.0f32;
                        let mut sum_y = 0.0f32;
                        let mut weight_sum = 0.0f32;

                        for dy in -1i32..=1 {
                            for dx in -1i32..=1 {
                                let nx = bx as i32 + dx;
                                let ny = by as i32 + dy;

                                if nx >= 0
                                    && nx < self.width as i32
                                    && ny >= 0
                                    && ny < self.height as i32
                                {
                                    let idx = (ny as u32 * self.width + nx as u32) as usize;
                                    if let Some(mv) = original.get(idx) {
                                        let weight = mv.confidence;
                                        sum_x += mv.x as f32 * weight;
                                        sum_y += mv.y as f32 * weight;
                                        weight_sum += weight;
                                    }
                                }
                            }
                        }

                        if weight_sum > 0.0 {
                            let idx = (by * self.width + bx) as usize;
                            self.vectors[idx].x = (sum_x / weight_sum).round() as i32;
                            self.vectors[idx].y = (sum_y / weight_sum).round() as i32;
                        }
                    }
                }
            }
        }
    }

    /// Temporal coherence tracker.
    #[derive(Debug)]
    pub struct CoherenceTracker {
        /// Motion fields for recent frames.
        motion_history: Vec<MotionField>,
        /// Maximum history length.
        max_history: usize,
    }

    impl CoherenceTracker {
        /// Create a new coherence tracker.
        #[must_use]
        pub fn new(max_history: usize) -> Self {
            Self {
                motion_history: Vec::new(),
                max_history,
            }
        }

        /// Add a motion field.
        pub fn add_motion_field(&mut self, field: MotionField) {
            self.motion_history.push(field);
            if self.motion_history.len() > self.max_history {
                self.motion_history.remove(0);
            }
        }

        /// Get motion consistency at a location.
        #[must_use]
        pub fn get_consistency(&self, bx: u32, by: u32) -> f32 {
            if self.motion_history.len() < 2 {
                return 1.0;
            }

            let mut total_diff = 0.0f32;
            let mut count = 0;

            for i in 1..self.motion_history.len() {
                let prev_mv = self.motion_history[i - 1].get(bx, by);
                let curr_mv = self.motion_history[i].get(bx, by);

                let dx = (curr_mv.x - prev_mv.x) as f32;
                let dy = (curr_mv.y - prev_mv.y) as f32;
                let diff = (dx * dx + dy * dy).sqrt();

                total_diff += diff;
                count += 1;
            }

            if count > 0 {
                let avg_diff = total_diff / count as f32;
                (1.0 / (1.0 + avg_diff * 0.1)).clamp(0.0, 1.0)
            } else {
                1.0
            }
        }

        /// Clear history.
        pub fn clear(&mut self) {
            self.motion_history.clear();
        }
    }
}

/// Advanced filtering techniques.
pub mod advanced {

    /// Wiener filter for noise reduction.
    #[derive(Debug, Clone)]
    pub struct WienerFilter {
        /// Noise variance estimate.
        noise_variance: f32,
        /// Window size.
        window_size: u32,
    }

    impl WienerFilter {
        /// Create a new Wiener filter.
        #[must_use]
        pub fn new(noise_variance: f32, window_size: u32) -> Self {
            Self {
                noise_variance,
                window_size,
            }
        }

        /// Apply Wiener filter to data.
        pub fn apply(&self, data: &mut [u8], width: u32, height: u32) {
            let original = data.to_vec();
            let radius = self.window_size / 2;

            for y in 0..height {
                for x in 0..width {
                    let (mean, variance) =
                        compute_window_stats(&original, x, y, width, height, radius);

                    let idx = (y * width + x) as usize;
                    let pixel = original.get(idx).copied().unwrap_or(128) as f32;

                    let local_variance = variance.max(0.0);
                    let weight = if local_variance > self.noise_variance {
                        1.0 - self.noise_variance / local_variance
                    } else {
                        0.0
                    };

                    let filtered = mean + weight * (pixel - mean);
                    data[idx] = filtered.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }

    fn compute_window_stats(
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        radius: u32,
    ) -> (f32, f32) {
        let mut sum = 0.0f32;
        let mut sq_sum = 0.0f32;
        let mut count = 0;

        let y_min = y.saturating_sub(radius);
        let y_max = (y + radius + 1).min(height);
        let x_min = x.saturating_sub(radius);
        let x_max = (x + radius + 1).min(width);

        for ny in y_min..y_max {
            for nx in x_min..x_max {
                let nidx = (ny * width + nx) as usize;
                let val = data.get(nidx).copied().unwrap_or(128) as f32;
                sum += val;
                sq_sum += val * val;
                count += 1;
            }
        }

        if count > 0 {
            let mean = sum / count as f32;
            let variance = (sq_sum / count as f32) - (mean * mean);
            (mean, variance)
        } else {
            (128.0, 0.0)
        }
    }

    /// Anisotropic diffusion for edge-preserving smoothing.
    #[derive(Debug, Clone)]
    pub struct AnisotropicDiffusion {
        /// Number of iterations.
        iterations: u32,
        /// Diffusion coefficient.
        kappa: f32,
        /// Time step.
        delta_t: f32,
    }

    impl AnisotropicDiffusion {
        /// Create a new anisotropic diffusion filter.
        #[must_use]
        pub fn new(iterations: u32, kappa: f32) -> Self {
            Self {
                iterations,
                kappa,
                delta_t: 0.25,
            }
        }

        /// Apply anisotropic diffusion.
        pub fn apply(&self, data: &mut [u8], width: u32, height: u32) {
            let kappa_sq = self.kappa * self.kappa;

            for _ in 0..self.iterations {
                let original = data.to_vec();

                for y in 1..height - 1 {
                    for x in 1..width - 1 {
                        let idx = (y * width + x) as usize;
                        let center = original.get(idx).copied().unwrap_or(128) as f32;

                        let north = original
                            .get(((y - 1) * width + x) as usize)
                            .copied()
                            .unwrap_or(128) as f32;
                        let south = original
                            .get(((y + 1) * width + x) as usize)
                            .copied()
                            .unwrap_or(128) as f32;
                        let west = original
                            .get((y * width + (x - 1)) as usize)
                            .copied()
                            .unwrap_or(128) as f32;
                        let east = original
                            .get((y * width + (x + 1)) as usize)
                            .copied()
                            .unwrap_or(128) as f32;

                        let grad_n = north - center;
                        let grad_s = south - center;
                        let grad_w = west - center;
                        let grad_e = east - center;

                        let c_n = self.diffusion_coeff(grad_n, kappa_sq);
                        let c_s = self.diffusion_coeff(grad_s, kappa_sq);
                        let c_w = self.diffusion_coeff(grad_w, kappa_sq);
                        let c_e = self.diffusion_coeff(grad_e, kappa_sq);

                        let update = self.delta_t
                            * (c_n * grad_n + c_s * grad_s + c_w * grad_w + c_e * grad_e);

                        data[idx] = (center + update).round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }

        fn diffusion_coeff(&self, gradient: f32, kappa_sq: f32) -> f32 {
            let grad_sq = gradient * gradient;
            (-(grad_sq / kappa_sq)).exp()
        }
    }

    /// Total variation denoising.
    #[derive(Debug, Clone)]
    pub struct TotalVariationDenoising {
        /// Regularization parameter.
        lambda: f32,
        /// Number of iterations.
        iterations: u32,
    }

    impl TotalVariationDenoising {
        /// Create a new TV denoising filter.
        #[must_use]
        pub fn new(lambda: f32, iterations: u32) -> Self {
            Self { lambda, iterations }
        }

        /// Apply TV denoising.
        pub fn apply(&self, data: &mut [u8], width: u32, height: u32) {
            let noisy = data.iter().map(|&x| x as f32).collect::<Vec<_>>();
            let mut denoised = noisy.clone();

            for _ in 0..self.iterations {
                let prev = denoised.clone();

                for y in 1..height - 1 {
                    for x in 1..width - 1 {
                        let idx = (y * width + x) as usize;

                        let grad_x = prev
                            .get((y * width + (x + 1)) as usize)
                            .copied()
                            .unwrap_or(128.0)
                            - prev
                                .get((y * width + (x - 1)) as usize)
                                .copied()
                                .unwrap_or(128.0);

                        let grad_y = prev
                            .get(((y + 1) * width + x) as usize)
                            .copied()
                            .unwrap_or(128.0)
                            - prev
                                .get(((y - 1) * width + x) as usize)
                                .copied()
                                .unwrap_or(128.0);

                        let grad_mag = (grad_x * grad_x + grad_y * grad_y).sqrt() + 1e-8;

                        let div_x = (prev
                            .get((y * width + (x + 1)) as usize)
                            .copied()
                            .unwrap_or(128.0)
                            - 2.0 * prev[idx]
                            + prev
                                .get((y * width + (x - 1)) as usize)
                                .copied()
                                .unwrap_or(128.0))
                            / grad_mag;

                        let div_y = (prev
                            .get(((y + 1) * width + x) as usize)
                            .copied()
                            .unwrap_or(128.0)
                            - 2.0 * prev[idx]
                            + prev
                                .get(((y - 1) * width + x) as usize)
                                .copied()
                                .unwrap_or(128.0))
                            / grad_mag;

                        let divergence = div_x + div_y;
                        let data_term = noisy[idx] - denoised[idx];

                        denoised[idx] += 0.1 * (data_term / self.lambda + divergence);
                    }
                }
            }

            for (i, &val) in denoised.iter().enumerate() {
                data[i] = val.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Color space utilities for denoise operations.
pub mod color_space {
    /// Convert RGB to YCbCr.
    #[must_use]
    pub fn rgb_to_ycbcr(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        let rf = r as f32;
        let gf = g as f32;
        let bf = b as f32;

        let y = (0.299 * rf + 0.587 * gf + 0.114 * bf)
            .round()
            .clamp(0.0, 255.0) as u8;
        let cb = (128.0 - 0.168736 * rf - 0.331264 * gf + 0.5 * bf)
            .round()
            .clamp(0.0, 255.0) as u8;
        let cr = (128.0 + 0.5 * rf - 0.418688 * gf - 0.081312 * bf)
            .round()
            .clamp(0.0, 255.0) as u8;

        (y, cb, cr)
    }

    /// Convert YCbCr to RGB.
    #[must_use]
    pub fn ycbcr_to_rgb(y: u8, cb: u8, cr: u8) -> (u8, u8, u8) {
        let yf = y as f32;
        let cbf = cb as f32 - 128.0;
        let crf = cr as f32 - 128.0;

        let r = (yf + 1.402 * crf).round().clamp(0.0, 255.0) as u8;
        let g = (yf - 0.344136 * cbf - 0.714136 * crf)
            .round()
            .clamp(0.0, 255.0) as u8;
        let b = (yf + 1.772 * cbf).round().clamp(0.0, 255.0) as u8;

        (r, g, b)
    }

    /// Separate luma and chroma processing.
    pub fn process_luma_chroma<F>(data: &mut [u8], width: u32, height: u32, mut f: F)
    where
        F: FnMut(&mut [u8], &mut [u8], &mut [u8]),
    {
        if data.len() < (width * height * 3) as usize {
            return;
        }

        let pixel_count = (width * height) as usize;
        let mut y_plane = vec![0u8; pixel_count];
        let mut cb_plane = vec![0u8; pixel_count];
        let mut cr_plane = vec![0u8; pixel_count];

        for i in 0..pixel_count {
            let r = data[i * 3];
            let g = data[i * 3 + 1];
            let b = data[i * 3 + 2];

            let (y, cb, cr) = rgb_to_ycbcr(r, g, b);
            y_plane[i] = y;
            cb_plane[i] = cb;
            cr_plane[i] = cr;
        }

        f(&mut y_plane, &mut cb_plane, &mut cr_plane);

        for i in 0..pixel_count {
            let y = y_plane[i];
            let cb = cb_plane[i];
            let cr = cr_plane[i];

            let (r, g, b) = ycbcr_to_rgb(y, cb, cr);
            data[i * 3] = r;
            data[i * 3 + 1] = g;
            data[i * 3 + 2] = b;
        }
    }
}

/// Utility functions for denoise operations.
pub mod utils {
    /// Clamp a value to a range.
    #[must_use]
    pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
        value.max(min).min(max)
    }

    /// Linear interpolation.
    #[must_use]
    pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }

    /// Smoothstep interpolation.
    #[must_use]
    pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Compute Gaussian weight.
    #[must_use]
    pub fn gaussian_weight(distance: f32, sigma: f32) -> f32 {
        let sigma_sq = sigma * sigma;
        (-(distance * distance) / (2.0 * sigma_sq)).exp()
    }

    /// Compute bilateral weight.
    #[must_use]
    pub fn bilateral_weight(
        spatial_dist: f32,
        intensity_diff: f32,
        sigma_space: f32,
        sigma_color: f32,
    ) -> f32 {
        let space_weight = gaussian_weight(spatial_dist, sigma_space);
        let color_weight = gaussian_weight(intensity_diff, sigma_color);
        space_weight * color_weight
    }

    /// Fast approximation of exp().
    #[must_use]
    pub fn fast_exp(x: f32) -> f32 {
        if x < -10.0 {
            return 0.0;
        }
        if x > 10.0 {
            return 1.0;
        }
        x.exp()
    }

    /// Convert decibels to linear scale.
    #[must_use]
    pub fn db_to_linear(db: f32) -> f32 {
        10.0f32.powf(db / 20.0)
    }

    /// Convert linear scale to decibels.
    #[must_use]
    pub fn linear_to_db(linear: f32) -> f32 {
        20.0 * linear.log10()
    }
}
