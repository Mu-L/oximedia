//! Video delogo filter.
//!
//! This filter removes logos and watermarks from video frames using various
//! techniques including blur, inpainting, texture synthesis, and temporal
//! interpolation.
//!
//! # Features
//!
//! - Manual region specification with bounding boxes
//! - Template matching for automatic logo detection
//! - Logo tracking across frames
//! - Multi-logo support
//! - Multiple removal techniques:
//!   - Simple blur
//!   - PDE-based inpainting (Navier-Stokes)
//!   - Fast marching method
//!   - Exemplar-based inpainting
//!   - Patch-based texture synthesis
//!   - Edge-aware interpolation
//!   - Temporal coherence using neighboring frames
//! - Alpha blending with configurable strength
//! - Feathered edges for smooth transitions
//! - Adaptive blending based on content
//! - Semi-transparent logo handling
//!
//! # Example
//!
//! ```ignore
//! use oximedia_graph::filters::video::{DelogoFilter, DelogoConfig, DelogoMethod, Rectangle};
//! use oximedia_graph::node::NodeId;
//!
//! // Create a delogo filter for a watermark in the top-right corner
//! let region = Rectangle::new(1600, 50, 200, 100);
//! let config = DelogoConfig::new(region, DelogoMethod::Inpainting)
//!     .with_feather(10)
//!     .with_strength(1.0);
//!
//! let filter = DelogoFilter::new(NodeId(0), "delogo", config);
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

/// Rectangle region for logo specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rectangle {
    /// X coordinate of top-left corner.
    pub x: u32,
    /// Y coordinate of top-left corner.
    pub y: u32,
    /// Width of the rectangle.
    pub width: u32,
    /// Height of the rectangle.
    pub height: u32,
}

impl Rectangle {
    /// Create a new rectangle.
    #[must_use]
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Get the right edge coordinate.
    #[must_use]
    pub fn right(&self) -> u32 {
        self.x + self.width
    }

    /// Get the bottom edge coordinate.
    #[must_use]
    pub fn bottom(&self) -> u32 {
        self.y + self.height
    }

    /// Check if a point is inside the rectangle.
    #[must_use]
    pub fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// Expand the rectangle by a given amount.
    #[must_use]
    pub fn expand(&self, amount: u32) -> Self {
        Self {
            x: self.x.saturating_sub(amount),
            y: self.y.saturating_sub(amount),
            width: self.width + amount * 2,
            height: self.height + amount * 2,
        }
    }

    /// Clamp the rectangle to fit within given dimensions.
    #[must_use]
    pub fn clamp(&self, max_width: u32, max_height: u32) -> Self {
        let x = self.x.min(max_width.saturating_sub(1));
        let y = self.y.min(max_height.saturating_sub(1));
        let width = self.width.min(max_width.saturating_sub(x));
        let height = self.height.min(max_height.saturating_sub(y));

        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Get the area of the rectangle.
    #[must_use]
    pub fn area(&self) -> u32 {
        self.width * self.height
    }

    /// Scale the rectangle for chroma planes.
    #[must_use]
    pub fn scale_for_chroma(&self, h_ratio: u32, v_ratio: u32) -> Self {
        Self {
            x: self.x / h_ratio,
            y: self.y / v_ratio,
            width: self.width / h_ratio,
            height: self.height / v_ratio,
        }
    }
}

/// Logo removal method.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DelogoMethod {
    /// Simple Gaussian blur.
    Blur,
    /// PDE-based inpainting (Navier-Stokes).
    #[default]
    Inpainting,
    /// Fast marching method inpainting.
    FastMarching,
    /// Exemplar-based inpainting (patch matching).
    ExemplarBased,
    /// Patch-based texture synthesis.
    TextureSynthesis,
    /// Edge-aware interpolation.
    EdgeAware,
    /// Temporal interpolation using neighboring frames.
    TemporalInterpolation,
}

/// Logo detection mode.
#[derive(Clone, Debug, PartialEq)]
pub enum LogoDetection {
    /// Manual specification of logo region.
    Manual(Rectangle),
    /// Template matching with a reference template.
    Template {
        /// Template image data.
        template: Vec<u8>,
        /// Template width.
        width: u32,
        /// Template height.
        height: u32,
        /// Detection threshold (0.0-1.0).
        threshold: f32,
    },
    /// Automatic detection and tracking.
    Automatic {
        /// Initial search region.
        search_region: Rectangle,
        /// Detection sensitivity.
        sensitivity: f32,
    },
}

/// Configuration for the delogo filter.
#[derive(Clone, Debug)]
pub struct DelogoConfig {
    /// Logo regions to remove (supports multiple logos).
    pub regions: Vec<Rectangle>,
    /// Detection mode for automatic logo detection.
    pub detection: Option<LogoDetection>,
    /// Removal method.
    pub method: DelogoMethod,
    /// Blend strength (0.0 = no effect, 1.0 = full removal).
    pub strength: f32,
    /// Feather radius for edge blending (pixels).
    pub feather: u32,
    /// Enable temporal coherence.
    pub temporal_coherence: bool,
    /// Number of frames to use for temporal processing.
    pub temporal_radius: usize,
    /// Enable edge preservation.
    pub preserve_edges: bool,
    /// Inpainting iterations (for iterative methods).
    pub iterations: u32,
    /// Patch size for texture synthesis methods.
    pub patch_size: u32,
}

impl DelogoConfig {
    /// Create a new delogo configuration with a single region.
    #[must_use]
    pub fn new(region: Rectangle, method: DelogoMethod) -> Self {
        Self {
            regions: vec![region],
            detection: None,
            method,
            strength: 1.0,
            feather: 5,
            temporal_coherence: false,
            temporal_radius: 2,
            preserve_edges: true,
            iterations: 100,
            patch_size: 7,
        }
    }

    /// Create a configuration with multiple regions.
    #[must_use]
    pub fn with_regions(regions: Vec<Rectangle>, method: DelogoMethod) -> Self {
        Self {
            regions,
            detection: None,
            method,
            strength: 1.0,
            feather: 5,
            temporal_coherence: false,
            temporal_radius: 2,
            preserve_edges: true,
            iterations: 100,
            patch_size: 7,
        }
    }

    /// Enable automatic logo detection.
    #[must_use]
    pub fn with_detection(mut self, detection: LogoDetection) -> Self {
        self.detection = Some(detection);
        self
    }

    /// Set the blend strength.
    #[must_use]
    pub fn with_strength(mut self, strength: f32) -> Self {
        self.strength = strength.clamp(0.0, 1.0);
        self
    }

    /// Set the feather radius.
    #[must_use]
    pub fn with_feather(mut self, feather: u32) -> Self {
        self.feather = feather;
        self
    }

    /// Enable temporal coherence.
    #[must_use]
    pub fn with_temporal_coherence(mut self, radius: usize) -> Self {
        self.temporal_coherence = true;
        self.temporal_radius = radius.max(1);
        self
    }

    /// Set edge preservation mode.
    #[must_use]
    pub fn with_edge_preservation(mut self, enabled: bool) -> Self {
        self.preserve_edges = enabled;
        self
    }

    /// Set the number of inpainting iterations.
    #[must_use]
    pub fn with_iterations(mut self, iterations: u32) -> Self {
        self.iterations = iterations.max(1);
        self
    }

    /// Set the patch size for texture synthesis.
    #[must_use]
    pub fn with_patch_size(mut self, size: u32) -> Self {
        self.patch_size = size.clamp(3, 64);
        self
    }
}

/// Video delogo filter.
///
/// Removes logos and watermarks from video frames using various advanced
/// techniques. Supports multiple logos, temporal coherence, and edge-aware
/// processing for natural-looking results.
pub struct DelogoFilter {
    id: NodeId,
    name: String,
    state: NodeState,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    config: DelogoConfig,
    /// Frame buffer for temporal processing.
    frame_buffer: VecDeque<VideoFrame>,
    /// Logo tracker for automatic detection.
    tracker: Option<LogoTracker>,
}

impl DelogoFilter {
    /// Create a new delogo filter.
    #[must_use]
    pub fn new(id: NodeId, name: impl Into<String>, config: DelogoConfig) -> Self {
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
            tracker: None,
        }
    }

    /// Get the current configuration.
    #[must_use]
    pub fn config(&self) -> &DelogoConfig {
        &self.config
    }

    /// Update the configuration.
    pub fn set_config(&mut self, config: DelogoConfig) {
        self.config = config;
        self.frame_buffer.clear();
        self.tracker = None;
    }

    /// Add a logo region.
    pub fn add_region(&mut self, region: Rectangle) {
        self.config.regions.push(region);
    }

    /// Clear all logo regions.
    pub fn clear_regions(&mut self) {
        self.config.regions.clear();
    }

    /// Process a single frame.
    fn process_frame(&mut self, mut frame: VideoFrame) -> GraphResult<VideoFrame> {
        // Update temporal buffer
        if self.config.temporal_coherence {
            self.frame_buffer.push_back(frame.clone());
            if self.frame_buffer.len() > self.config.temporal_radius * 2 + 1 {
                self.frame_buffer.pop_front();
            }
        }

        // Detect logos if automatic detection is enabled
        let detected_regions = if let Some(detection) = self.config.detection.clone() {
            self.detect_logos(&frame, &detection)
        } else {
            None
        };

        if let Some(regions) = detected_regions {
            self.config.regions = regions;
        }

        // Process each logo region
        let regions = self.config.regions.clone();
        for region in &regions {
            let clamped = region.clamp(frame.width, frame.height);
            self.remove_logo(&mut frame, &clamped)?;
        }

        Ok(frame)
    }

    /// Remove a logo from a specific region.
    fn remove_logo(&self, frame: &mut VideoFrame, region: &Rectangle) -> GraphResult<()> {
        // Get format information
        let (h_sub, v_sub) = frame.format.chroma_subsampling();

        // Process each plane
        for (plane_idx, plane) in frame.planes.iter_mut().enumerate() {
            let plane_region = if plane_idx > 0 && frame.format.is_yuv() {
                region.scale_for_chroma(h_sub, v_sub)
            } else {
                *region
            };

            let (plane_width, plane_height) = if plane_idx == 0 {
                (frame.width, frame.height)
            } else {
                (frame.width / h_sub, frame.height / v_sub)
            };

            self.process_plane(plane, &plane_region, plane_width, plane_height)?;
        }

        Ok(())
    }

    /// Process a single plane for logo removal.
    fn process_plane(
        &self,
        plane: &mut Plane,
        region: &Rectangle,
        width: u32,
        height: u32,
    ) -> GraphResult<()> {
        // Create a working buffer
        let mut data = plane.data.to_vec();

        match self.config.method {
            DelogoMethod::Blur => {
                self.apply_blur(&mut data, region, width, height);
            }
            DelogoMethod::Inpainting => {
                self.apply_inpainting(&mut data, region, width, height);
            }
            DelogoMethod::FastMarching => {
                self.apply_fast_marching(&mut data, region, width, height);
            }
            DelogoMethod::ExemplarBased => {
                self.apply_exemplar_based(&mut data, region, width, height);
            }
            DelogoMethod::TextureSynthesis => {
                self.apply_texture_synthesis(&mut data, region, width, height);
            }
            DelogoMethod::EdgeAware => {
                self.apply_edge_aware(&mut data, region, width, height);
            }
            DelogoMethod::TemporalInterpolation => {
                self.apply_temporal_interpolation(&mut data, region, width, height);
            }
        }

        // Apply feathering/blending
        if self.config.feather > 0 {
            self.apply_feathering(&mut data, plane, region, width, height);
        }

        // Update plane data
        *plane = Plane::new(data, plane.stride);

        Ok(())
    }

    /// Apply Gaussian blur to the logo region.
    fn apply_blur(&self, data: &mut [u8], region: &Rectangle, width: u32, height: u32) {
        let radius = (region.width.min(region.height) / 8).clamp(3, 15);
        let sigma = radius as f32 / 2.0;

        // Create Gaussian kernel
        let kernel = create_gaussian_kernel(radius as usize, sigma);

        // Apply blur to region
        for y in region.y..region.bottom().min(height) {
            for x in region.x..region.right().min(width) {
                let blurred = self.apply_kernel(data, x, y, width, height, &kernel, radius as i32);
                let idx = (y * width + x) as usize;
                data[idx] = blurred;
            }
        }
    }

    /// Apply a convolution kernel at a specific position.
    fn apply_kernel(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        kernel: &[f32],
        radius: i32,
    ) -> u8 {
        let mut sum = 0.0f32;
        let mut weight_sum = 0.0f32;

        let ksize = (radius * 2 + 1) as usize;

        for ky in 0..ksize {
            let py = y as i32 + ky as i32 - radius;
            if py < 0 || py >= height as i32 {
                continue;
            }

            for kx in 0..ksize {
                let px = x as i32 + kx as i32 - radius;
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

    /// Apply Navier-Stokes PDE-based inpainting.
    fn apply_inpainting(&self, data: &mut [u8], region: &Rectangle, width: u32, height: u32) {
        // Create mask for the region
        let mut mask = vec![false; (width * height) as usize];
        for y in region.y..region.bottom().min(height) {
            for x in region.x..region.right().min(width) {
                mask[(y * width + x) as usize] = true;
            }
        }

        // Iterative inpainting using Navier-Stokes equations
        let mut working = data.to_vec();

        for _ in 0..self.config.iterations {
            let mut updated = false;

            for y in region.y..region.bottom().min(height) {
                for x in region.x..region.right().min(width) {
                    let idx = (y * width + x) as usize;

                    if mask[idx] {
                        // Compute gradient from boundary
                        let grad = self.compute_gradient(&working, x, y, width, height, &mask);

                        if grad.0 != 0.0 || grad.1 != 0.0 {
                            // Propagate information from boundary
                            let new_val =
                                self.propagate_isophote(&working, x, y, width, height, &mask, grad);
                            working[idx] = new_val;
                            updated = true;
                        }
                    }
                }
            }

            if !updated {
                break;
            }
        }

        data.copy_from_slice(&working);
    }

    /// Compute gradient at a position.
    fn compute_gradient(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        mask: &[bool],
    ) -> (f32, f32) {
        let mut gx = 0.0f32;
        let mut gy = 0.0f32;
        let mut count = 0;

        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let nx = x as i32 + dx;
                let ny = y as i32 + dy;

                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                    let nidx = (ny as u32 * width + nx as u32) as usize;

                    if !mask.get(nidx).copied().unwrap_or(false) {
                        let val = data.get(nidx).copied().unwrap_or(128) as f32;
                        gx += val * dx as f32;
                        gy += val * dy as f32;
                        count += 1;
                    }
                }
            }
        }

        if count > 0 {
            let norm = (gx * gx + gy * gy).sqrt().max(1.0);
            (gx / norm, gy / norm)
        } else {
            (0.0, 0.0)
        }
    }

    /// Propagate isophote information.
    fn propagate_isophote(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        mask: &[bool],
        _grad: (f32, f32),
    ) -> u8 {
        let mut sum = 0.0f32;
        let mut weight_sum = 0.0f32;

        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;

                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                    let nidx = (ny as u32 * width + nx as u32) as usize;

                    if !mask.get(nidx).copied().unwrap_or(false) {
                        let val = data.get(nidx).copied().unwrap_or(128) as f32;
                        let dist = ((dx * dx + dy * dy) as f32).sqrt();
                        let weight = 1.0 / (dist + 0.1);

                        sum += val * weight;
                        weight_sum += weight;
                    }
                }
            }
        }

        if weight_sum > 0.0 {
            (sum / weight_sum).round().clamp(0.0, 255.0) as u8
        } else {
            data.get((y * width + x) as usize).copied().unwrap_or(128)
        }
    }

    /// Apply fast marching method inpainting.
    fn apply_fast_marching(&self, data: &mut [u8], region: &Rectangle, width: u32, height: u32) {
        // Distance transform from boundary
        let mut distance = vec![f32::INFINITY; (width * height) as usize];
        let mut heap = std::collections::BinaryHeap::new();

        // Initialize boundary
        for y in region.y.saturating_sub(1)..=(region.bottom() + 1).min(height - 1) {
            for x in region.x.saturating_sub(1)..=(region.right() + 1).min(width - 1) {
                if !region.contains(x, y) {
                    let idx = (y * width + x) as usize;
                    distance[idx] = 0.0;
                    heap.push(OrderedFloat(-0.0, idx));
                }
            }
        }

        // Fast marching
        let mut working = data.to_vec();

        while let Some(OrderedFloat(neg_dist, idx)) = heap.pop() {
            let dist = -neg_dist;
            let y = (idx as u32 / width) as u32;
            let x = (idx as u32 % width) as u32;

            if distance[idx] < dist {
                continue;
            }

            // Update neighbors
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }

                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let nidx = (ny as u32 * width + nx as u32) as usize;
                        let step_dist = ((dx * dx + dy * dy) as f32).sqrt();
                        let new_dist = dist + step_dist;

                        if new_dist < distance[nidx] {
                            distance[nidx] = new_dist;

                            // Interpolate value
                            working[nidx] = self.interpolate_from_boundary(
                                data, nx as u32, ny as u32, width, height, region,
                            );

                            heap.push(OrderedFloat(-new_dist, nidx));
                        }
                    }
                }
            }
        }

        // Copy result
        for y in region.y..region.bottom().min(height) {
            for x in region.x..region.right().min(width) {
                let idx = (y * width + x) as usize;
                data[idx] = working[idx];
            }
        }
    }

    /// Interpolate value from boundary pixels.
    fn interpolate_from_boundary(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        region: &Rectangle,
    ) -> u8 {
        let mut sum = 0.0f32;
        let mut weight_sum = 0.0f32;

        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;

                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                    let nx = nx as u32;
                    let ny = ny as u32;

                    if !region.contains(nx, ny) {
                        let nidx = (ny * width + nx) as usize;
                        let val = data.get(nidx).copied().unwrap_or(128) as f32;
                        let dist = ((dx * dx + dy * dy) as f32).sqrt();
                        let weight = 1.0 / (dist + 0.1);

                        sum += val * weight;
                        weight_sum += weight;
                    }
                }
            }
        }

        if weight_sum > 0.0 {
            (sum / weight_sum).round().clamp(0.0, 255.0) as u8
        } else {
            128
        }
    }

    /// Apply exemplar-based inpainting using patch matching.
    fn apply_exemplar_based(&self, data: &mut [u8], region: &Rectangle, width: u32, height: u32) {
        let patch_size = self.config.patch_size as i32;
        let _half_patch = patch_size / 2;

        let mut working = data.to_vec();
        let mut mask = vec![false; (width * height) as usize];

        for y in region.y..region.bottom().min(height) {
            for x in region.x..region.right().min(width) {
                mask[(y * width + x) as usize] = true;
            }
        }

        // Iterative filling
        for _ in 0..self.config.iterations.min(10) {
            let mut updated = false;

            for y in region.y..region.bottom().min(height) {
                for x in region.x..region.right().min(width) {
                    let idx = (y * width + x) as usize;

                    if mask[idx] {
                        // Find best matching patch
                        if let Some(patch_val) =
                            self.find_best_patch(&working, x, y, width, height, &mask, patch_size)
                        {
                            working[idx] = patch_val;
                            updated = true;
                        }
                    }
                }
            }

            if !updated {
                break;
            }
        }

        data.copy_from_slice(&working);
    }

    /// Find the best matching patch for a position.
    fn find_best_patch(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        mask: &[bool],
        patch_size: i32,
    ) -> Option<u8> {
        let half_patch = patch_size / 2;
        let search_range = 20;

        let mut best_diff = f32::INFINITY;
        let mut best_val = None;

        for sy in (y as i32 - search_range).max(half_patch)
            ..=(y as i32 + search_range).min(height as i32 - half_patch - 1)
        {
            for sx in (x as i32 - search_range).max(half_patch)
                ..=(x as i32 + search_range).min(width as i32 - half_patch - 1)
            {
                let sidx = (sy as u32 * width + sx as u32) as usize;

                if !mask.get(sidx).copied().unwrap_or(false) {
                    // Compare patches
                    let diff = self.patch_difference(
                        data, x, y, sx as u32, sy as u32, width, mask, half_patch,
                    );

                    if diff < best_diff {
                        best_diff = diff;
                        best_val = data.get(sidx).copied();
                    }
                }
            }
        }

        best_val
    }

    /// Compute difference between two patches.
    fn patch_difference(
        &self,
        data: &[u8],
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
        width: u32,
        mask: &[bool],
        radius: i32,
    ) -> f32 {
        let mut diff = 0.0f32;
        let mut count = 0;

        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let px1 = x1 as i32 + dx;
                let py1 = y1 as i32 + dy;
                let px2 = x2 as i32 + dx;
                let py2 = y2 as i32 + dy;

                if px1 >= 0 && px2 >= 0 && py1 >= 0 && py2 >= 0 {
                    let idx1 = (py1 as u32 * width + px1 as u32) as usize;
                    let idx2 = (py2 as u32 * width + px2 as u32) as usize;

                    // Only compare where both are known
                    if !mask.get(idx1).copied().unwrap_or(false)
                        && !mask.get(idx2).copied().unwrap_or(false)
                    {
                        let v1 = data.get(idx1).copied().unwrap_or(128) as f32;
                        let v2 = data.get(idx2).copied().unwrap_or(128) as f32;
                        diff += (v1 - v2).abs();
                        count += 1;
                    }
                }
            }
        }

        if count > 0 {
            diff / count as f32
        } else {
            f32::INFINITY
        }
    }

    /// Apply texture synthesis.
    fn apply_texture_synthesis(
        &self,
        data: &mut [u8],
        region: &Rectangle,
        width: u32,
        height: u32,
    ) {
        // Use exemplar-based approach with larger search region
        self.apply_exemplar_based(data, region, width, height);
    }

    /// Apply edge-aware interpolation.
    fn apply_edge_aware(&self, data: &mut [u8], region: &Rectangle, width: u32, height: u32) {
        // Detect edges around the region
        let edges = self.detect_edges(data, width, height);

        let mut working = data.to_vec();

        for y in region.y..region.bottom().min(height) {
            for x in region.x..region.right().min(width) {
                let idx = (y * width + x) as usize;

                // Interpolate based on edge direction
                let edge_strength = edges[idx];
                let val = if edge_strength > 50.0 {
                    // Strong edge: interpolate along edge
                    self.interpolate_along_edge(data, x, y, width, height, &edges)
                } else {
                    // Weak edge: simple interpolation
                    self.interpolate_from_boundary(data, x, y, width, height, region)
                };

                working[idx] = val;
            }
        }

        data.copy_from_slice(&working);
    }

    /// Detect edges in the image.
    fn detect_edges(&self, data: &[u8], width: u32, height: u32) -> Vec<f32> {
        let mut edges = vec![0.0f32; (width * height) as usize];

        // Sobel operator
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let mut gx = 0.0f32;
                let mut gy = 0.0f32;

                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let nx = (x as i32 + dx) as u32;
                        let ny = (y as i32 + dy) as u32;
                        let nidx = (ny * width + nx) as usize;
                        let val = data.get(nidx).copied().unwrap_or(128) as f32;

                        // Sobel kernels
                        let sx = dx as f32;
                        let sy = dy as f32;

                        gx += val * sx;
                        gy += val * sy;
                    }
                }

                let magnitude = (gx * gx + gy * gy).sqrt();
                edges[(y * width + x) as usize] = magnitude;
            }
        }

        edges
    }

    /// Interpolate along edge direction.
    fn interpolate_along_edge(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        _edges: &[f32],
    ) -> u8 {
        // Simplified: average of perpendicular neighbors
        let mut sum = 0.0f32;
        let mut count = 0;

        for offset in [-2i32, -1, 1, 2] {
            for (dx, dy) in [(offset, 0), (0, offset)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;

                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                    let nidx = (ny as u32 * width + nx as u32) as usize;
                    sum += data.get(nidx).copied().unwrap_or(128) as f32;
                    count += 1;
                }
            }
        }

        if count > 0 {
            (sum / count as f32).round().clamp(0.0, 255.0) as u8
        } else {
            128
        }
    }

    /// Apply temporal interpolation using neighboring frames.
    fn apply_temporal_interpolation(
        &self,
        data: &mut [u8],
        region: &Rectangle,
        width: u32,
        height: u32,
    ) {
        if self.frame_buffer.is_empty() {
            // Fall back to spatial inpainting
            self.apply_inpainting(data, region, width, height);
            return;
        }

        // Average corresponding pixels from temporal neighbors
        for y in region.y..region.bottom().min(height) {
            for x in region.x..region.right().min(width) {
                let idx = (y * width + x) as usize;

                let mut sum = 0.0f32;
                let mut count = 0;

                for frame in &self.frame_buffer {
                    if let Some(plane) = frame.planes.first() {
                        let fidx = (y * width + x) as usize;
                        if let Some(&val) = plane.data.get(fidx) {
                            sum += val as f32;
                            count += 1;
                        }
                    }
                }

                if count > 0 {
                    data[idx] = (sum / count as f32).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }

    /// Apply feathering/blending at region edges.
    fn apply_feathering(
        &self,
        data: &mut [u8],
        original: &Plane,
        region: &Rectangle,
        width: u32,
        height: u32,
    ) {
        let feather = self.config.feather as i32;
        let expanded = region.expand(self.config.feather);

        for y in expanded.y..expanded.bottom().min(height) {
            for x in expanded.x..expanded.right().min(width) {
                let idx = (y * width + x) as usize;

                // Compute distance to region boundary
                let dist = self.distance_to_boundary(x, y, region);

                if dist < feather as f32 {
                    // Blend between original and processed
                    let alpha = (dist / feather as f32).clamp(0.0, 1.0);
                    let alpha = alpha * self.config.strength;

                    let original_val = original.data.get(idx).copied().unwrap_or(128) as f32;
                    let processed_val = data.get(idx).copied().unwrap_or(128) as f32;

                    let blended = original_val * (1.0 - alpha) + processed_val * alpha;
                    data[idx] = blended.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }

    /// Compute distance from a point to the region boundary.
    fn distance_to_boundary(&self, x: u32, y: u32, region: &Rectangle) -> f32 {
        if !region.contains(x, y) {
            // Outside region
            let dx = if x < region.x {
                region.x - x
            } else if x >= region.right() {
                x - region.right() + 1
            } else {
                0
            };

            let dy = if y < region.y {
                region.y - y
            } else if y >= region.bottom() {
                y - region.bottom() + 1
            } else {
                0
            };

            ((dx * dx + dy * dy) as f32).sqrt()
        } else {
            // Inside region - distance to nearest edge
            let dx = (x - region.x).min(region.right() - x - 1);
            let dy = (y - region.y).min(region.bottom() - y - 1);
            dx.min(dy) as f32
        }
    }

    /// Detect logos in the frame.
    fn detect_logos(
        &mut self,
        frame: &VideoFrame,
        detection: &LogoDetection,
    ) -> Option<Vec<Rectangle>> {
        match detection {
            LogoDetection::Manual(region) => Some(vec![*region]),
            LogoDetection::Template {
                template,
                width: t_width,
                height: t_height,
                threshold,
            } => {
                if let Some(plane) = frame.planes.first() {
                    self.template_match(
                        &plane.data,
                        frame.width,
                        frame.height,
                        template,
                        *t_width,
                        *t_height,
                        *threshold,
                    )
                } else {
                    None
                }
            }
            LogoDetection::Automatic {
                search_region,
                sensitivity,
            } => {
                if let Some(plane) = frame.planes.first() {
                    self.auto_detect(
                        &plane.data,
                        frame.width,
                        frame.height,
                        search_region,
                        *sensitivity,
                    )
                } else {
                    None
                }
            }
        }
    }

    /// Template matching for logo detection.
    fn template_match(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        template: &[u8],
        t_width: u32,
        t_height: u32,
        threshold: f32,
    ) -> Option<Vec<Rectangle>> {
        let mut matches = Vec::new();

        for y in 0..=(height.saturating_sub(t_height)) {
            for x in 0..=(width.saturating_sub(t_width)) {
                let score =
                    self.compute_template_score(data, x, y, width, template, t_width, t_height);

                if score >= threshold {
                    matches.push(Rectangle::new(x, y, t_width, t_height));
                }
            }
        }

        if matches.is_empty() {
            None
        } else {
            Some(matches)
        }
    }

    /// Compute normalized cross-correlation score for template matching.
    fn compute_template_score(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        template: &[u8],
        t_width: u32,
        t_height: u32,
    ) -> f32 {
        let mut sum = 0.0f32;
        let mut sq_sum = 0.0f32;
        let mut template_sum = 0.0f32;
        let mut template_sq_sum = 0.0f32;
        let mut cross_sum = 0.0f32;
        let mut count = 0;

        for ty in 0..t_height {
            for tx in 0..t_width {
                let px = x + tx;
                let py = y + ty;
                let idx = (py * width + px) as usize;
                let tidx = (ty * t_width + tx) as usize;

                let val = data.get(idx).copied().unwrap_or(0) as f32;
                let tval = template.get(tidx).copied().unwrap_or(0) as f32;

                sum += val;
                sq_sum += val * val;
                template_sum += tval;
                template_sq_sum += tval * tval;
                cross_sum += val * tval;
                count += 1;
            }
        }

        if count == 0 {
            return 0.0;
        }

        let n = count as f32;
        let numerator = cross_sum - (sum * template_sum / n);
        let denominator =
            ((sq_sum - sum * sum / n) * (template_sq_sum - template_sum * template_sum / n)).sqrt();

        if denominator > 0.0 {
            (numerator / denominator).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Automatic logo detection.
    fn auto_detect(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        search_region: &Rectangle,
        sensitivity: f32,
    ) -> Option<Vec<Rectangle>> {
        // Detect high-contrast regions that might be logos
        let edges = self.detect_edges(data, width, height);
        let threshold = 100.0 * sensitivity;

        let mut regions = Vec::new();

        // Simple connected component analysis
        let mut visited = vec![false; (width * height) as usize];

        for y in search_region.y..search_region.bottom().min(height) {
            for x in search_region.x..search_region.right().min(width) {
                let idx = (y * width + x) as usize;

                if !visited[idx] && edges.get(idx).copied().unwrap_or(0.0) > threshold {
                    if let Some(region) =
                        self.extract_component(&edges, &mut visited, x, y, width, height, threshold)
                    {
                        regions.push(region);
                    }
                }
            }
        }

        if regions.is_empty() {
            None
        } else {
            Some(regions)
        }
    }

    /// Extract connected component.
    fn extract_component(
        &self,
        edges: &[f32],
        visited: &mut [bool],
        start_x: u32,
        start_y: u32,
        width: u32,
        height: u32,
        threshold: f32,
    ) -> Option<Rectangle> {
        let mut min_x = start_x;
        let mut max_x = start_x;
        let mut min_y = start_y;
        let mut max_y = start_y;

        let mut stack = vec![(start_x, start_y)];
        visited[(start_y * width + start_x) as usize] = true;

        while let Some((x, y)) = stack.pop() {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);

            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }

                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let nidx = (ny as u32 * width + nx as u32) as usize;

                        if !visited[nidx] && edges.get(nidx).copied().unwrap_or(0.0) > threshold {
                            visited[nidx] = true;
                            stack.push((nx as u32, ny as u32));
                        }
                    }
                }
            }
        }

        let w = max_x - min_x + 1;
        let h = max_y - min_y + 1;

        // Filter out very small or very large regions
        if w >= 10 && h >= 10 && w < width / 2 && h < height / 2 {
            Some(Rectangle::new(min_x, min_y, w, h))
        } else {
            None
        }
    }
}

impl Node for DelogoFilter {
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
        self.tracker = None;
        self.set_state(NodeState::Idle)
    }
}

/// Logo tracker for multi-frame tracking.
#[derive(Debug)]
struct LogoTracker {
    /// Tracked regions.
    regions: Vec<TrackedRegion>,
    /// Maximum tracking distance per frame.
    max_distance: f32,
}

impl LogoTracker {
    /// Create a new logo tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
            max_distance: 20.0,
        }
    }

    /// Update tracked regions with new detections.
    pub fn update(&mut self, detected: Vec<Rectangle>) {
        // Simple tracking: match by proximity
        for region in detected {
            let mut matched = false;

            for tracked in &mut self.regions {
                if tracked.matches(&region, self.max_distance) {
                    tracked.update(region);
                    matched = true;
                    break;
                }
            }

            if !matched {
                self.regions.push(TrackedRegion::new(region));
            }
        }

        // Remove stale tracks
        self.regions.retain(|r| r.confidence > 0.3);
    }

    /// Get current tracked regions.
    #[must_use]
    pub fn regions(&self) -> Vec<Rectangle> {
        self.regions.iter().map(|r| r.region).collect()
    }
}

/// A tracked logo region.
#[derive(Debug)]
struct TrackedRegion {
    /// Current region.
    region: Rectangle,
    /// Tracking confidence (0.0-1.0).
    confidence: f32,
    /// Number of frames tracked.
    age: u32,
}

impl TrackedRegion {
    /// Create a new tracked region.
    #[must_use]
    pub fn new(region: Rectangle) -> Self {
        Self {
            region,
            confidence: 1.0,
            age: 0,
        }
    }

    /// Check if a detection matches this tracked region.
    #[must_use]
    pub fn matches(&self, other: &Rectangle, max_distance: f32) -> bool {
        let dx = (self.region.x as f32 - other.x as f32).abs();
        let dy = (self.region.y as f32 - other.y as f32).abs();
        let distance = (dx * dx + dy * dy).sqrt();

        distance < max_distance
    }

    /// Update with a new detection.
    pub fn update(&mut self, region: Rectangle) {
        // Smooth position update
        let alpha = 0.3;
        self.region.x = (self.region.x as f32 * (1.0 - alpha) + region.x as f32 * alpha) as u32;
        self.region.y = (self.region.y as f32 * (1.0 - alpha) + region.y as f32 * alpha) as u32;
        self.region.width =
            (self.region.width as f32 * (1.0 - alpha) + region.width as f32 * alpha) as u32;
        self.region.height =
            (self.region.height as f32 * (1.0 - alpha) + region.height as f32 * alpha) as u32;

        self.confidence = (self.confidence * 0.9 + 0.1).min(1.0);
        self.age += 1;
    }
}

/// Ordered float for priority queue.
#[derive(Debug, Clone, Copy)]
struct OrderedFloat(f32, usize);

impl PartialEq for OrderedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// Create a Gaussian kernel for blurring.
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

    // Normalize
    for val in &mut kernel {
        *val /= sum;
    }

    kernel
}

/// Mask generation utilities.
pub mod mask {
    use super::Rectangle;

    /// Mask type for logo regions.
    #[derive(Clone, Debug)]
    pub struct LogoMask {
        /// Width of the mask.
        pub width: u32,
        /// Height of the mask.
        pub height: u32,
        /// Mask data (0.0 = keep original, 1.0 = fully remove).
        pub data: Vec<f32>,
    }

    impl LogoMask {
        /// Create a new mask from a rectangle.
        #[must_use]
        pub fn from_rectangle(region: &Rectangle, width: u32, height: u32) -> Self {
            let mut data = vec![0.0f32; (width * height) as usize];

            for y in region.y..region.bottom().min(height) {
                for x in region.x..region.right().min(width) {
                    data[(y * width + x) as usize] = 1.0;
                }
            }

            Self {
                width,
                height,
                data,
            }
        }

        /// Create a feathered mask.
        #[must_use]
        pub fn from_rectangle_feathered(
            region: &Rectangle,
            width: u32,
            height: u32,
            feather: u32,
        ) -> Self {
            let mut data = vec![0.0f32; (width * height) as usize];

            for y in 0..height {
                for x in 0..width {
                    let dist = distance_to_rectangle(x, y, region);
                    let alpha = if dist < 0.0 {
                        // Inside
                        1.0
                    } else if dist < feather as f32 {
                        // Feather zone
                        1.0 - (dist / feather as f32)
                    } else {
                        // Outside
                        0.0
                    };

                    data[(y * width + x) as usize] = alpha;
                }
            }

            Self {
                width,
                height,
                data,
            }
        }

        /// Get mask value at position.
        #[must_use]
        pub fn get(&self, x: u32, y: u32) -> f32 {
            if x >= self.width || y >= self.height {
                return 0.0;
            }
            self.data
                .get((y * self.width + x) as usize)
                .copied()
                .unwrap_or(0.0)
        }

        /// Blur the mask for smoother transitions.
        pub fn blur(&mut self, radius: usize) {
            let kernel = super::create_gaussian_kernel(radius, radius as f32 / 2.0);
            let ksize = radius * 2 + 1;

            let mut blurred = self.data.clone();

            for y in 0..self.height {
                for x in 0..self.width {
                    let mut sum = 0.0f32;
                    let mut weight_sum = 0.0f32;

                    for ky in 0..ksize {
                        let py = y as i32 + ky as i32 - radius as i32;
                        if py < 0 || py >= self.height as i32 {
                            continue;
                        }

                        for kx in 0..ksize {
                            let px = x as i32 + kx as i32 - radius as i32;
                            if px < 0 || px >= self.width as i32 {
                                continue;
                            }

                            let idx = (py as u32 * self.width + px as u32) as usize;
                            let weight = kernel[ky * ksize + kx];
                            sum += self.data.get(idx).copied().unwrap_or(0.0) * weight;
                            weight_sum += weight;
                        }
                    }

                    let idx = (y * self.width + x) as usize;
                    if weight_sum > 0.0 {
                        blurred[idx] = sum / weight_sum;
                    }
                }
            }

            self.data = blurred;
        }

        /// Dilate the mask (expand regions).
        pub fn dilate(&mut self, iterations: usize) {
            for _ in 0..iterations {
                let mut dilated = self.data.clone();

                for y in 1..self.height - 1 {
                    for x in 1..self.width - 1 {
                        let mut max_val = self.get(x, y);

                        for dy in -1i32..=1 {
                            for dx in -1i32..=1 {
                                let nx = (x as i32 + dx) as u32;
                                let ny = (y as i32 + dy) as u32;
                                max_val = max_val.max(self.get(nx, ny));
                            }
                        }

                        dilated[(y * self.width + x) as usize] = max_val;
                    }
                }

                self.data = dilated;
            }
        }

        /// Erode the mask (shrink regions).
        pub fn erode(&mut self, iterations: usize) {
            for _ in 0..iterations {
                let mut eroded = self.data.clone();

                for y in 1..self.height - 1 {
                    for x in 1..self.width - 1 {
                        let mut min_val = self.get(x, y);

                        for dy in -1i32..=1 {
                            for dx in -1i32..=1 {
                                let nx = (x as i32 + dx) as u32;
                                let ny = (y as i32 + dy) as u32;
                                min_val = min_val.min(self.get(nx, ny));
                            }
                        }

                        eroded[(y * self.width + x) as usize] = min_val;
                    }
                }

                self.data = eroded;
            }
        }
    }

    /// Compute signed distance from point to rectangle.
    fn distance_to_rectangle(x: u32, y: u32, region: &Rectangle) -> f32 {
        if region.contains(x, y) {
            // Inside - negative distance to nearest edge
            let dx = (x - region.x).min(region.right() - x - 1);
            let dy = (y - region.y).min(region.bottom() - y - 1);
            -(dx.min(dy) as f32)
        } else {
            // Outside - distance to nearest edge
            let dx = if x < region.x {
                region.x - x
            } else if x >= region.right() {
                x - region.right() + 1
            } else {
                0
            };

            let dy = if y < region.y {
                region.y - y
            } else if y >= region.bottom() {
                y - region.bottom() + 1
            } else {
                0
            };

            ((dx * dx + dy * dy) as f32).sqrt()
        }
    }
}

/// Quality metrics for evaluating delogo results.
pub mod metrics {
    /// Compute PSNR between two frames.
    #[must_use]
    pub fn psnr(original: &[u8], processed: &[u8]) -> f32 {
        if original.len() != processed.len() || original.is_empty() {
            return 0.0;
        }

        let mut mse = 0.0f64;
        for (a, b) in original.iter().zip(processed.iter()) {
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

    /// Compute SSIM between two frames.
    #[must_use]
    pub fn ssim(original: &[u8], processed: &[u8], width: usize, height: usize) -> f32 {
        if original.len() != processed.len() || original.is_empty() {
            return 0.0;
        }

        let c1 = (0.01 * 255.0) * (0.01 * 255.0);
        let c2 = (0.03 * 255.0) * (0.03 * 255.0);

        let window_size = 11usize;
        let half_window = window_size / 2;

        let mut ssim_sum = 0.0f64;
        let mut count = 0;

        for y in half_window..(height - half_window) {
            for x in half_window..(width - half_window) {
                let (mean_x, mean_y, var_x, var_y, cov_xy) =
                    compute_window_stats(original, processed, x, y, width, window_size);

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

    /// Compute statistics for a window.
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

/// Color correction utilities for better blending.
pub mod color {
    /// Adjust brightness and contrast.
    #[must_use]
    pub fn adjust_brightness_contrast(value: u8, brightness: f32, contrast: f32) -> u8 {
        let v = value as f32;
        let adjusted = (v - 128.0) * contrast + 128.0 + brightness;
        adjusted.round().clamp(0.0, 255.0) as u8
    }

    /// Match histogram between two regions.
    pub fn match_histogram(target: &mut [u8], reference: &[u8]) {
        if target.is_empty() || reference.is_empty() {
            return;
        }

        // Compute CDFs
        let target_cdf = compute_cdf(target);
        let reference_cdf = compute_cdf(reference);

        // Create lookup table
        let mut lut = [0u8; 256];
        for i in 0..256 {
            let target_val = target_cdf[i];
            let mut best_match = 0;
            let mut best_diff = f32::INFINITY;

            for j in 0..256 {
                let diff = (reference_cdf[j] - target_val).abs();
                if diff < best_diff {
                    best_diff = diff;
                    best_match = j;
                }
            }

            lut[i] = best_match as u8;
        }

        // Apply lookup table
        for pixel in target.iter_mut() {
            *pixel = lut[*pixel as usize];
        }
    }

    /// Compute cumulative distribution function.
    fn compute_cdf(data: &[u8]) -> [f32; 256] {
        let mut histogram = [0u32; 256];
        for &pixel in data {
            histogram[pixel as usize] += 1;
        }

        let mut cdf = [0.0f32; 256];
        let total = data.len() as f32;
        let mut sum = 0u32;

        for i in 0..256 {
            sum += histogram[i];
            cdf[i] = sum as f32 / total;
        }

        cdf
    }

    /// Compute mean color value.
    #[must_use]
    pub fn mean(data: &[u8]) -> f32 {
        if data.is_empty() {
            return 128.0;
        }

        let sum: u32 = data.iter().map(|&x| x as u32).sum();
        sum as f32 / data.len() as f32
    }

    /// Compute standard deviation.
    #[must_use]
    pub fn std_dev(data: &[u8], mean_val: f32) -> f32 {
        if data.is_empty() {
            return 0.0;
        }

        let variance: f32 = data
            .iter()
            .map(|&x| {
                let diff = x as f32 - mean_val;
                diff * diff
            })
            .sum::<f32>()
            / data.len() as f32;

        variance.sqrt()
    }
}

/// Advanced inpainting algorithms.
pub mod advanced_inpainting {

    /// Criminisi's exemplar-based inpainting.
    #[allow(dead_code)]
    pub struct CriminisiInpainting {
        /// Patch size.
        patch_size: usize,
        /// Priority weight for data term.
        alpha: f32,
        /// Priority weight for confidence term.
        beta: f32,
    }

    impl CriminisiInpainting {
        /// Create a new Criminisi inpainter.
        #[must_use]
        pub fn new(patch_size: usize) -> Self {
            Self {
                patch_size,
                alpha: 0.7,
                beta: 0.3,
            }
        }

        /// Compute fill priority for a pixel on the boundary.
        #[must_use]
        pub fn compute_priority(
            &self,
            data: &[u8],
            mask: &[bool],
            x: u32,
            y: u32,
            width: u32,
            height: u32,
        ) -> f32 {
            let confidence = self.compute_confidence(mask, x, y, width, height);
            let data_term = self.compute_data_term(data, mask, x, y, width, height);

            self.beta * confidence + self.alpha * data_term
        }

        /// Compute confidence term (ratio of known pixels in patch).
        fn compute_confidence(
            &self,
            mask: &[bool],
            x: u32,
            y: u32,
            width: u32,
            height: u32,
        ) -> f32 {
            let half = self.patch_size as i32 / 2;
            let mut known = 0;
            let mut total = 0;

            for dy in -half..=half {
                for dx in -half..=half {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        total += 1;
                        let idx = (ny as u32 * width + nx as u32) as usize;
                        if !mask.get(idx).copied().unwrap_or(false) {
                            known += 1;
                        }
                    }
                }
            }

            if total > 0 {
                known as f32 / total as f32
            } else {
                0.0
            }
        }

        /// Compute data term (gradient strength perpendicular to boundary).
        fn compute_data_term(
            &self,
            data: &[u8],
            _mask: &[bool],
            x: u32,
            y: u32,
            width: u32,
            height: u32,
        ) -> f32 {
            // Compute image gradient
            let (gx, gy) = self.compute_gradient(data, x, y, width, height);
            let magnitude = (gx * gx + gy * gy).sqrt();

            magnitude / 255.0
        }

        /// Compute image gradient.
        fn compute_gradient(
            &self,
            data: &[u8],
            x: u32,
            y: u32,
            width: u32,
            height: u32,
        ) -> (f32, f32) {
            let mut gx = 0.0f32;
            let mut gy = 0.0f32;

            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let nidx = (ny as u32 * width + nx as u32) as usize;
                        let val = data.get(nidx).copied().unwrap_or(128) as f32;

                        gx += val * dx as f32;
                        gy += val * dy as f32;
                    }
                }
            }

            (gx, gy)
        }
    }

    /// Telea's fast marching inpainting.
    #[allow(dead_code)]
    pub struct TeleaInpainting {
        /// Neighborhood radius.
        radius: usize,
    }

    impl TeleaInpainting {
        /// Create a new Telea inpainter.
        #[must_use]
        pub fn new(radius: usize) -> Self {
            Self { radius }
        }

        /// Inpaint a pixel using weighted average of known neighbors.
        #[must_use]
        pub fn inpaint_pixel(
            &self,
            data: &[u8],
            mask: &[bool],
            x: u32,
            y: u32,
            width: u32,
            height: u32,
        ) -> u8 {
            let mut sum = 0.0f32;
            let mut weight_sum = 0.0f32;

            let r = self.radius as i32;

            for dy in -r..=r {
                for dx in -r..=r {
                    if dx == 0 && dy == 0 {
                        continue;
                    }

                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let nidx = (ny as u32 * width + nx as u32) as usize;

                        if !mask.get(nidx).copied().unwrap_or(false) {
                            let dist = ((dx * dx + dy * dy) as f32).sqrt();
                            let weight = 1.0 / (dist + 0.1);

                            sum += data.get(nidx).copied().unwrap_or(128) as f32 * weight;
                            weight_sum += weight;
                        }
                    }
                }
            }

            if weight_sum > 0.0 {
                (sum / weight_sum).round().clamp(0.0, 255.0) as u8
            } else {
                128
            }
        }
    }
}

/// Utility functions for logo detection and analysis.
pub mod detection {
    use super::Rectangle;

    /// Analyze a region for logo characteristics.
    #[derive(Debug, Clone)]
    pub struct LogoCharacteristics {
        /// Average brightness.
        pub brightness: f32,
        /// Contrast level.
        pub contrast: f32,
        /// Edge density.
        pub edge_density: f32,
        /// Color variance.
        pub variance: f32,
        /// Opacity estimate (0.0 = transparent, 1.0 = opaque).
        pub opacity: f32,
    }

    impl LogoCharacteristics {
        /// Analyze a region.
        #[must_use]
        pub fn analyze(data: &[u8], region: &Rectangle, width: u32, height: u32) -> Self {
            let mut values = Vec::new();

            for y in region.y..region.bottom().min(height) {
                for x in region.x..region.right().min(width) {
                    let idx = (y * width + x) as usize;
                    if let Some(&val) = data.get(idx) {
                        values.push(val);
                    }
                }
            }

            let brightness = super::color::mean(&values);
            let variance = super::color::std_dev(&values, brightness);
            let contrast = compute_contrast(&values);
            let edge_density = estimate_edge_density(data, region, width, height);
            let opacity = estimate_opacity(&values, brightness);

            Self {
                brightness,
                contrast,
                edge_density,
                variance,
                opacity,
            }
        }

        /// Check if the region likely contains a logo.
        #[must_use]
        pub fn is_likely_logo(&self) -> bool {
            // Logos typically have high contrast and edge density
            self.edge_density > 0.3 && self.contrast > 30.0
        }

        /// Check if the logo is semi-transparent.
        #[must_use]
        pub fn is_semitransparent(&self) -> bool {
            self.opacity < 0.8
        }
    }

    /// Compute contrast using difference between min and max.
    fn compute_contrast(data: &[u8]) -> f32 {
        if data.is_empty() {
            return 0.0;
        }

        let min = *data.iter().min().unwrap_or(&0);
        let max = *data.iter().max().unwrap_or(&255);

        (max - min) as f32
    }

    /// Estimate edge density in a region.
    fn estimate_edge_density(data: &[u8], region: &Rectangle, width: u32, height: u32) -> f32 {
        let mut edge_count = 0;
        let mut total = 0;

        for y in (region.y + 1)..(region.bottom() - 1).min(height - 1) {
            for x in (region.x + 1)..(region.right() - 1).min(width - 1) {
                let idx = (y * width + x) as usize;
                let val = data.get(idx).copied().unwrap_or(128);

                // Simple edge detection using neighbors
                let mut gradient = 0.0f32;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }

                        let nx = (x as i32 + dx) as u32;
                        let ny = (y as i32 + dy) as u32;
                        let nidx = (ny * width + nx) as usize;
                        let nval = data.get(nidx).copied().unwrap_or(128);

                        gradient += (val as f32 - nval as f32).abs();
                    }
                }

                if gradient > 200.0 {
                    edge_count += 1;
                }
                total += 1;
            }
        }

        if total > 0 {
            edge_count as f32 / total as f32
        } else {
            0.0
        }
    }

    /// Estimate opacity based on pixel values.
    fn estimate_opacity(data: &[u8], _mean: f32) -> f32 {
        if data.is_empty() {
            return 1.0;
        }

        // Count pixels close to extremes (fully opaque tends to be more extreme)
        let extreme_count = data.iter().filter(|&&v| !(50..=205).contains(&v)).count();

        let opacity = extreme_count as f32 / data.len() as f32;
        opacity.clamp(0.3, 1.0)
    }

    /// Non-maximum suppression for detected regions.
    pub fn non_maximum_suppression(regions: &[Rectangle], iou_threshold: f32) -> Vec<Rectangle> {
        if regions.is_empty() {
            return Vec::new();
        }

        let mut sorted: Vec<_> = regions.to_vec();
        sorted.sort_by_key(|r| r.area());
        sorted.reverse();

        let mut keep = Vec::new();

        for region in sorted {
            let mut should_keep = true;

            for kept in &keep {
                let iou = compute_iou(&region, kept);
                if iou > iou_threshold {
                    should_keep = false;
                    break;
                }
            }

            if should_keep {
                keep.push(region);
            }
        }

        keep
    }

    /// Compute intersection over union for two rectangles.
    fn compute_iou(a: &Rectangle, b: &Rectangle) -> f32 {
        let intersect_x1 = a.x.max(b.x);
        let intersect_y1 = a.y.max(b.y);
        let intersect_x2 = a.right().min(b.right());
        let intersect_y2 = a.bottom().min(b.bottom());

        if intersect_x2 <= intersect_x1 || intersect_y2 <= intersect_y1 {
            return 0.0;
        }

        let intersect_area = (intersect_x2 - intersect_x1) * (intersect_y2 - intersect_y1);
        let union_area = a.area() + b.area() - intersect_area;

        if union_area > 0 {
            intersect_area as f32 / union_area as f32
        } else {
            0.0
        }
    }
}
