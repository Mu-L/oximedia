//! CSRT (Discriminative Correlation Filter with Channel and Spatial Reliability) tracker.
//!
//! CSRT uses spatial reliability maps and channel weights to improve
//! tracking robustness in challenging scenarios.
//!
//! # Example
//!
//! ```
//! use oximedia_cv::tracking::csrt::CsrtTracker;
//! use oximedia_cv::detect::BoundingBox;
//!
//! let bbox = BoundingBox::new(50.0, 50.0, 100.0, 100.0);
//! let tracker = CsrtTracker::new(bbox);
//! ```

use crate::detect::BoundingBox;
use crate::error::{CvError, CvResult};
use std::f64::consts::PI;

/// CSRT tracker configuration.
#[derive(Debug, Clone)]
pub struct CsrtTracker {
    /// Current bounding box.
    bbox: BoundingBox,
    /// Filter coefficients for each channel.
    filters: Vec<Vec<f64>>,
    /// Spatial reliability map.
    reliability_map: Vec<f64>,
    /// Channel weights.
    channel_weights: Vec<f64>,
    /// Template size.
    template_size: (usize, usize),
    /// Learning rate.
    learning_rate: f64,
    /// Number of feature channels.
    num_channels: usize,
    /// Padding factor.
    padding: f64,
    /// Scale estimation window.
    scale_window: Vec<f64>,
    /// Current confidence.
    confidence: f64,
    /// Background suppression factor.
    background_ratio: f64,
}

impl CsrtTracker {
    /// Create a new CSRT tracker.
    ///
    /// # Arguments
    ///
    /// * `bbox` - Initial bounding box
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_cv::tracking::csrt::CsrtTracker;
    /// use oximedia_cv::detect::BoundingBox;
    ///
    /// let bbox = BoundingBox::new(100.0, 100.0, 50.0, 50.0);
    /// let tracker = CsrtTracker::new(bbox);
    /// ```
    #[must_use]
    pub fn new(bbox: BoundingBox) -> Self {
        let template_size = (64, 64);
        let num_channels = 9; // Gray + HOG-like features

        Self {
            bbox,
            filters: vec![Vec::new(); num_channels],
            reliability_map: vec![1.0; template_size.0 * template_size.1],
            channel_weights: vec![1.0; num_channels],
            template_size,
            learning_rate: 0.025,
            num_channels,
            padding: 2.0,
            scale_window: create_scale_window(),
            confidence: 1.0,
            background_ratio: 0.3,
        }
    }

    /// Set learning rate.
    #[must_use]
    pub const fn with_learning_rate(mut self, rate: f64) -> Self {
        self.learning_rate = rate;
        self
    }

    /// Set background ratio for context modeling.
    #[must_use]
    pub const fn with_background_ratio(mut self, ratio: f64) -> Self {
        self.background_ratio = ratio;
        self
    }

    /// Initialize the tracker with the first frame.
    ///
    /// # Errors
    ///
    /// Returns an error if frame dimensions are invalid.
    pub fn initialize(&mut self, frame: &[u8], width: u32, height: u32) -> CvResult<()> {
        if width == 0 || height == 0 {
            return Err(CvError::invalid_dimensions(width, height));
        }

        // Extract multi-channel features
        let patch = self.get_padded_patch(frame, width, height)?;
        let features = extract_multichannel_features(&patch, self.template_size);

        // Create target labels
        let labels = create_segmentation_mask(self.template_size, self.background_ratio);

        // Initialize filters for each channel
        for ch in 0..self.num_channels {
            let channel_start = ch * self.template_size.0 * self.template_size.1;
            let channel_end = channel_start + self.template_size.0 * self.template_size.1;

            if channel_end <= features.len() {
                let channel_features = &features[channel_start..channel_end];
                let filter = train_channel_filter(
                    channel_features,
                    &labels,
                    &self.reliability_map,
                    self.template_size,
                );
                self.filters[ch] = filter;
            }
        }

        // Initialize reliability map (uniform at start)
        self.update_reliability_map(&features, &labels);

        // Initialize channel weights (uniform at start)
        self.update_channel_weights(&features, &labels);

        Ok(())
    }

    /// Update tracker with a new frame.
    ///
    /// # Errors
    ///
    /// Returns an error if tracking fails or dimensions are invalid.
    #[allow(clippy::too_many_lines)]
    pub fn update(&mut self, frame: &[u8], width: u32, height: u32) -> CvResult<BoundingBox> {
        if self.filters[0].is_empty() {
            return Err(CvError::tracking_error("Tracker not initialized"));
        }

        // Extract features at current location
        let patch = self.get_padded_patch(frame, width, height)?;
        let features = extract_multichannel_features(&patch, self.template_size);

        // Detect using multi-channel filters
        let response = self.detect_multichannel(&features);

        // Find peak with subpixel refinement
        let (peak_y, peak_x, max_response) = find_peak_subpixel(&response, self.template_size);

        // Update confidence
        self.confidence = (max_response / 5.0).clamp(0.0, 1.0);

        // Compute displacement
        let (tw, th) = self.template_size;
        let dy = peak_y - th as f64 / 2.0;
        let dx = peak_x - tw as f64 / 2.0;

        // Scale displacement by padding
        let cell_size = self.bbox.width as f64 * self.padding / tw as f64;
        let actual_dx = dx * cell_size;
        let actual_dy = dy * cell_size;

        // Update position
        self.bbox.x += actual_dx as f32;
        self.bbox.y += actual_dy as f32;

        // Scale estimation
        if self.confidence > 0.5 {
            let best_scale = self.estimate_scale(frame, width, height)?;
            self.bbox.width *= best_scale as f32;
            self.bbox.height *= best_scale as f32;
        }

        // Clamp to image bounds
        self.bbox = self.bbox.clamp(width as f32, height as f32);

        // Update model
        if self.confidence > 0.6 {
            let new_patch = self.get_padded_patch(frame, width, height)?;
            let new_features = extract_multichannel_features(&new_patch, self.template_size);
            let labels = create_segmentation_mask(self.template_size, self.background_ratio);

            // Update reliability map
            self.update_reliability_map(&new_features, &labels);

            // Update channel weights
            self.update_channel_weights(&new_features, &labels);

            // Update filters
            let lr = self.learning_rate;
            for ch in 0..self.num_channels {
                let channel_start = ch * self.template_size.0 * self.template_size.1;
                let channel_end = channel_start + self.template_size.0 * self.template_size.1;

                if channel_end <= new_features.len() {
                    let channel_features = &new_features[channel_start..channel_end];
                    let new_filter = train_channel_filter(
                        channel_features,
                        &labels,
                        &self.reliability_map,
                        self.template_size,
                    );

                    // Blend with old filter
                    for i in 0..self.filters[ch].len().min(new_filter.len()) {
                        self.filters[ch][i] = lr * new_filter[i] + (1.0 - lr) * self.filters[ch][i];
                    }
                }
            }
        }

        Ok(self.bbox)
    }

    /// Get current bounding box.
    #[must_use]
    pub const fn bbox(&self) -> &BoundingBox {
        &self.bbox
    }

    /// Get current confidence.
    #[must_use]
    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Reset tracker with new bounding box.
    pub fn reset(&mut self, bbox: BoundingBox) {
        self.bbox = bbox;
        for filter in &mut self.filters {
            filter.clear();
        }
        self.confidence = 1.0;
    }

    /// Get padded patch around current bbox.
    fn get_padded_patch(&self, frame: &[u8], width: u32, height: u32) -> CvResult<Vec<f64>> {
        let padded_w = (self.bbox.width * self.padding as f32) as usize;
        let padded_h = (self.bbox.height * self.padding as f32) as usize;

        let cx = self.bbox.x + self.bbox.width / 2.0;
        let cy = self.bbox.y + self.bbox.height / 2.0;

        let x0 = (cx - padded_w as f32 / 2.0).max(0.0) as usize;
        let y0 = (cy - padded_h as f32 / 2.0).max(0.0) as usize;
        let x1 = (cx + padded_w as f32 / 2.0).min(width as f32) as usize;
        let y1 = (cy + padded_h as f32 / 2.0).min(height as f32) as usize;

        if x1 <= x0 || y1 <= y0 {
            return Err(CvError::tracking_error("Invalid padded region"));
        }

        let (tw, th) = self.template_size;
        let mut patch = vec![0.0; tw * th];

        for y in 0..th {
            for x in 0..tw {
                let src_x = x0 + (x * (x1 - x0)) / tw;
                let src_y = y0 + (y * (y1 - y0)) / th;

                if src_x < width as usize && src_y < height as usize {
                    let idx = src_y * width as usize + src_x;
                    if idx < frame.len() {
                        patch[y * tw + x] = frame[idx] as f64;
                    }
                }
            }
        }

        Ok(patch)
    }

    /// Detect using multi-channel filters with channel weights.
    fn detect_multichannel(&self, features: &[f64]) -> Vec<f64> {
        let (w, h) = self.template_size;
        let mut response = vec![0.0; w * h];

        for ch in 0..self.num_channels {
            let channel_start = ch * w * h;
            let channel_end = channel_start + w * h;

            if channel_end <= features.len() && !self.filters[ch].is_empty() {
                let channel_features = &features[channel_start..channel_end];
                let channel_response =
                    correlate_with_filter(channel_features, &self.filters[ch], self.template_size);

                // Add weighted response
                let weight = self.channel_weights[ch];
                for i in 0..response.len().min(channel_response.len()) {
                    response[i] += weight * channel_response[i];
                }
            }
        }

        response
    }

    /// Estimate scale using scale pyramid.
    fn estimate_scale(&self, frame: &[u8], width: u32, height: u32) -> CvResult<f64> {
        let mut best_score = f64::NEG_INFINITY;
        let mut best_scale = 1.0;

        for (i, &scale) in self.scale_window.iter().enumerate() {
            let test_bbox = BoundingBox::new(
                self.bbox.x,
                self.bbox.y,
                self.bbox.width * scale as f32,
                self.bbox.height * scale as f32,
            );

            // Create temporary tracker for this scale
            let original_bbox = self.bbox;
            let mut temp_tracker = self.clone();
            temp_tracker.bbox = test_bbox;

            if let Ok(patch) = temp_tracker.get_padded_patch(frame, width, height) {
                let features = extract_multichannel_features(&patch, self.template_size);
                let response = temp_tracker.detect_multichannel(&features);

                let max_response = response.iter().copied().fold(f64::NEG_INFINITY, f64::max);

                // Weight by Gaussian scale window
                let scale_weight = gaussian_1d(i as f64, self.scale_window.len() as f64 / 2.0, 1.0);
                let score = max_response * scale_weight;

                if score > best_score {
                    best_score = score;
                    best_scale = scale;
                }
            }

            temp_tracker.bbox = original_bbox;
        }

        Ok(best_scale)
    }

    /// Update spatial reliability map.
    fn update_reliability_map(&mut self, features: &[f64], labels: &[f64]) {
        let (w, h) = self.template_size;
        let n = w * h;

        for i in 0..n.min(self.reliability_map.len()) {
            let mut reliability = 0.0;

            // Compute local consistency across channels
            for ch in 0..self.num_channels {
                let idx = ch * n + i;
                if idx < features.len() {
                    let feature_val = features[idx];
                    let label_val = labels[i];
                    reliability += (feature_val - label_val).abs();
                }
            }

            // Invert: high consistency = high reliability
            self.reliability_map[i] = (-reliability / self.num_channels as f64).exp();
        }
    }

    /// Update channel weights based on discrimination power.
    fn update_channel_weights(&mut self, features: &[f64], labels: &[f64]) {
        let (w, h) = self.template_size;
        let n = w * h;

        for ch in 0..self.num_channels {
            let channel_start = ch * n;
            let channel_end = channel_start + n;

            if channel_end <= features.len() {
                let channel_features = &features[channel_start..channel_end];

                // Compute discrimination: separation between foreground and background
                let mut fg_sum = 0.0;
                let mut bg_sum = 0.0;
                let mut fg_count = 0.0;
                let mut bg_count = 0.0;

                for i in 0..n.min(labels.len()) {
                    if labels[i] > 0.5 {
                        fg_sum += channel_features[i];
                        fg_count += 1.0;
                    } else {
                        bg_sum += channel_features[i];
                        bg_count += 1.0;
                    }
                }

                let fg_mean = if fg_count > 0.0 {
                    fg_sum / fg_count
                } else {
                    0.0
                };
                let bg_mean = if bg_count > 0.0 {
                    bg_sum / bg_count
                } else {
                    0.0
                };

                // Weight based on separation
                self.channel_weights[ch] = (fg_mean - bg_mean).abs();
            }
        }

        // Normalize weights
        let sum: f64 = self.channel_weights.iter().sum();
        if sum > 1e-6 {
            for weight in &mut self.channel_weights {
                *weight /= sum;
            }
        }
    }
}

/// Extract multi-channel features (HOG-like).
fn extract_multichannel_features(patch: &[f64], size: (usize, usize)) -> Vec<f64> {
    let (w, h) = size;
    let num_channels = 9;
    let mut features = vec![0.0; w * h * num_channels];

    // Channel 0: Grayscale
    for i in 0..(w * h).min(patch.len()) {
        features[i] = patch[i];
    }

    // Compute gradients for remaining channels
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let idx = y * w + x;

            let gx = patch[idx + 1] - patch[idx - 1];
            let gy = patch[idx + w] - patch[idx - w];
            let magnitude = (gx * gx + gy * gy).sqrt();
            let angle = gy.atan2(gx);

            // HOG-like orientation bins (8 bins)
            let bin_size = PI / 4.0;
            let bin = ((angle + PI) / bin_size) as usize % 8;

            // Distribute magnitude across channels
            features[w * h + idx] = gx; // Grad X
            features[2 * w * h + idx] = gy; // Grad Y
            features[(3 + bin) * w * h + idx] = magnitude; // Orientation bins
        }
    }

    // Normalize each channel
    for ch in 0..num_channels {
        let start = ch * w * h;
        let end = start + w * h;
        normalize_channel(&mut features[start..end]);
    }

    features
}

/// Normalize a single channel.
fn normalize_channel(channel: &mut [f64]) {
    let n = channel.len() as f64;
    let mean = channel.iter().sum::<f64>() / n;
    let variance = channel
        .iter()
        .map(|&x| (x - mean) * (x - mean))
        .sum::<f64>()
        / n;
    let std = (variance + 1e-5).sqrt();

    for val in channel {
        *val = (*val - mean) / std;
    }
}

/// Create segmentation mask (foreground/background labels).
fn create_segmentation_mask(size: (usize, usize), bg_ratio: f64) -> Vec<f64> {
    let (w, h) = size;
    let mut mask = vec![0.0; w * h];

    let fg_w = (w as f64 * (1.0 - bg_ratio)) as usize;
    let fg_h = (h as f64 * (1.0 - bg_ratio)) as usize;
    let x0 = (w - fg_w) / 2;
    let y0 = (h - fg_h) / 2;

    for y in y0..(y0 + fg_h).min(h) {
        for x in x0..(x0 + fg_w).min(w) {
            mask[y * w + x] = 1.0;
        }
    }

    mask
}

/// Train filter for a single channel with spatial reliability.
fn train_channel_filter(
    features: &[f64],
    labels: &[f64],
    reliability: &[f64],
    size: (usize, usize),
) -> Vec<f64> {
    let (w, h) = size;
    let n = w * h;

    // Apply reliability weighting to features
    let mut weighted_features = vec![0.0; n];
    for i in 0..n.min(features.len()).min(reliability.len()) {
        weighted_features[i] = features[i] * reliability[i].sqrt();
    }

    // Simplified filter training using direct correlation
    let mut filter = vec![0.0; n];

    for i in 0..n.min(labels.len()) {
        filter[i] = weighted_features[i] * labels[i];
    }

    filter
}

/// Correlate features with filter.
fn correlate_with_filter(features: &[f64], filter: &[f64], size: (usize, usize)) -> Vec<f64> {
    let (w, h) = size;
    let mut response = vec![0.0; w * h];

    for i in 0..(w * h).min(features.len()).min(filter.len()) {
        response[i] = features[i] * filter[i];
    }

    response
}

/// Find peak with subpixel refinement.
fn find_peak_subpixel(response: &[f64], size: (usize, usize)) -> (f64, f64, f64) {
    let (w, _h) = size;
    let mut max_idx = 0;
    let mut max_val = f64::NEG_INFINITY;

    for (i, &val) in response.iter().enumerate() {
        if val > max_val {
            max_val = val;
            max_idx = i;
        }
    }

    let peak_x = max_idx % w;
    let peak_y = max_idx / w;

    // Subpixel refinement using parabola fitting
    let (refined_x, refined_y) =
        if peak_x > 0 && peak_x < w - 1 && peak_y > 0 && peak_y < response.len() / w - 1 {
            let x_left = response[max_idx - 1];
            let x_right = response[max_idx + 1];
            let dx = 0.5 * (x_right - x_left) / (2.0 * max_val - x_left - x_right + 1e-10);

            let y_up = response[max_idx - w];
            let y_down = response[max_idx + w];
            let dy = 0.5 * (y_down - y_up) / (2.0 * max_val - y_up - y_down + 1e-10);

            (peak_x as f64 + dx, peak_y as f64 + dy)
        } else {
            (peak_x as f64, peak_y as f64)
        };

    (refined_y, refined_x, max_val)
}

/// Create scale window for scale estimation.
fn create_scale_window() -> Vec<f64> {
    vec![0.96, 0.98, 1.0, 1.02, 1.04]
}

/// Gaussian function for 1D.
fn gaussian_1d(x: f64, mean: f64, sigma: f64) -> f64 {
    let diff = x - mean;
    (-0.5 * diff * diff / (sigma * sigma)).exp()
}
