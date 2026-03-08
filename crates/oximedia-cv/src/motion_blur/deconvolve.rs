//! Deconvolution algorithms for motion blur removal.
//!
//! This module provides various deconvolution methods to remove or reduce motion blur.

use super::MotionPSF;
use crate::error::{CvError, CvResult};

/// Deconvolution method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeconvolutionMethod {
    /// Wiener deconvolution with noise estimation.
    Wiener,
    /// Richardson-Lucy iterative deconvolution.
    #[default]
    RichardsonLucy,
    /// Fast Fourier Transform-based deconvolution.
    FFT,
    /// Total Variation regularized deconvolution.
    TotalVariation,
}

/// Wiener deconvolution parameters.
#[derive(Debug, Clone)]
pub struct WienerParams {
    /// Noise-to-signal ratio (NSR).
    pub nsr: f32,
    /// Regularization parameter.
    pub regularization: f32,
}

impl Default for WienerParams {
    fn default() -> Self {
        Self {
            nsr: 0.01,
            regularization: 0.001,
        }
    }
}

impl WienerParams {
    /// Create new Wiener parameters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set noise-to-signal ratio.
    #[must_use]
    pub const fn with_nsr(mut self, nsr: f32) -> Self {
        self.nsr = nsr;
        self
    }

    /// Set regularization parameter.
    #[must_use]
    pub const fn with_regularization(mut self, reg: f32) -> Self {
        self.regularization = reg;
        self
    }
}

/// Richardson-Lucy deconvolution parameters.
#[derive(Debug, Clone)]
pub struct RichardsonLucyParams {
    /// Number of iterations.
    pub iterations: usize,
    /// Convergence threshold.
    pub threshold: f32,
    /// Enable acceleration.
    pub accelerated: bool,
}

impl Default for RichardsonLucyParams {
    fn default() -> Self {
        Self {
            iterations: 30,
            threshold: 0.001,
            accelerated: false,
        }
    }
}

impl RichardsonLucyParams {
    /// Create new Richardson-Lucy parameters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set number of iterations.
    #[must_use]
    pub const fn with_iterations(mut self, iterations: usize) -> Self {
        self.iterations = iterations;
        self
    }

    /// Set convergence threshold.
    #[must_use]
    pub const fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    /// Enable accelerated version.
    #[must_use]
    pub const fn with_accelerated(mut self, accelerated: bool) -> Self {
        self.accelerated = accelerated;
        self
    }
}

/// Deconvolution processor.
pub struct Deconvolver {
    method: DeconvolutionMethod,
    wiener_params: WienerParams,
    rl_params: RichardsonLucyParams,
}

impl Deconvolver {
    /// Create a new deconvolver with specified method.
    #[must_use]
    pub fn new(method: DeconvolutionMethod) -> Self {
        Self {
            method,
            wiener_params: WienerParams::default(),
            rl_params: RichardsonLucyParams::default(),
        }
    }

    /// Set Wiener parameters.
    #[must_use]
    pub fn with_wiener_params(mut self, params: WienerParams) -> Self {
        self.wiener_params = params;
        self
    }

    /// Set Richardson-Lucy parameters.
    #[must_use]
    pub fn with_rl_params(mut self, params: RichardsonLucyParams) -> Self {
        self.rl_params = params;
        self
    }

    /// Deconvolve an RGB image using the specified PSF.
    ///
    /// # Arguments
    ///
    /// * `image` - Blurred RGB image (width * height * 3)
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `psf` - Point spread function
    pub fn deconvolve(
        &self,
        image: &[u8],
        width: u32,
        height: u32,
        psf: &MotionPSF,
    ) -> CvResult<Vec<u8>> {
        if width == 0 || height == 0 {
            return Err(CvError::invalid_dimensions(width, height));
        }

        let expected_size = (width * height * 3) as usize;
        if image.len() != expected_size {
            return Err(CvError::insufficient_data(expected_size, image.len()));
        }

        // Process each channel separately
        let mut output = vec![0u8; image.len()];

        for c in 0..3 {
            let channel = extract_channel(image, width, height, c);
            let deconvolved = match self.method {
                DeconvolutionMethod::Wiener => {
                    self.wiener_deconvolve(&channel, width, height, psf)?
                }
                DeconvolutionMethod::RichardsonLucy => {
                    self.richardson_lucy_deconvolve(&channel, width, height, psf)?
                }
                DeconvolutionMethod::FFT => self.fft_deconvolve(&channel, width, height, psf)?,
                DeconvolutionMethod::TotalVariation => {
                    self.tv_deconvolve(&channel, width, height, psf)?
                }
            };

            insert_channel(&mut output, &deconvolved, width, height, c);
        }

        Ok(output)
    }

    /// Wiener deconvolution.
    fn wiener_deconvolve(
        &self,
        channel: &[f32],
        width: u32,
        height: u32,
        psf: &MotionPSF,
    ) -> CvResult<Vec<f32>> {
        let mut output = vec![0.0; channel.len()];

        let half_w = psf.width as i32 / 2;
        let half_h = psf.height as i32 / 2;

        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let mut sum = 0.0;
                let mut weight_sum = 0.0;

                for ky in 0..psf.height as i32 {
                    for kx in 0..psf.width as i32 {
                        let ix = x + kx - half_w;
                        let iy = y + ky - half_h;

                        if ix >= 0 && ix < width as i32 && iy >= 0 && iy < height as i32 {
                            let idx = (iy * width as i32 + ix) as usize;
                            let kernel_idx = ky as usize * psf.width + kx as usize;

                            if idx < channel.len() && kernel_idx < psf.kernel.len() {
                                let h = psf.kernel[kernel_idx];
                                // Wiener filter: H* / (|H|^2 + NSR)
                                let weight = h
                                    / (h * h
                                        + self.wiener_params.nsr
                                        + self.wiener_params.regularization);
                                sum += channel[idx] * weight;
                                weight_sum += weight.abs();
                            }
                        }
                    }
                }

                let out_idx = (y * width as i32 + x) as usize;
                if out_idx < output.len() {
                    output[out_idx] = if weight_sum > 0.0 {
                        (sum / weight_sum).clamp(0.0, 1.0)
                    } else {
                        channel[out_idx]
                    };
                }
            }
        }

        Ok(output)
    }

    /// Richardson-Lucy iterative deconvolution.
    fn richardson_lucy_deconvolve(
        &self,
        channel: &[f32],
        width: u32,
        height: u32,
        psf: &MotionPSF,
    ) -> CvResult<Vec<f32>> {
        // Initialize estimate with the blurred image
        let mut estimate = channel.to_vec();
        let mut previous_estimate = estimate.clone();

        // Create flipped PSF for correlation
        let psf_flipped = flip_psf(psf);

        for iter in 0..self.rl_params.iterations {
            // Convolve estimate with PSF
            let blurred_estimate = convolve_with_psf(&estimate, width, height, psf);

            // Compute ratio of observed to blurred estimate
            let mut ratio = vec![0.0; channel.len()];
            for i in 0..channel.len() {
                if blurred_estimate[i] > f32::EPSILON {
                    ratio[i] = channel[i] / blurred_estimate[i];
                } else {
                    ratio[i] = 1.0;
                }
            }

            // Correlate ratio with flipped PSF
            let correction = convolve_with_psf(&ratio, width, height, &psf_flipped);

            // Update estimate
            for i in 0..estimate.len() {
                estimate[i] *= correction[i];
                estimate[i] = estimate[i].clamp(0.0, 1.0);
            }

            // Check convergence
            if iter > 0 && self.check_convergence(&estimate, &previous_estimate) {
                break;
            }

            previous_estimate.clone_from(&estimate);
        }

        Ok(estimate)
    }

    /// FFT-based deconvolution (simplified spatial domain version).
    fn fft_deconvolve(
        &self,
        channel: &[f32],
        width: u32,
        height: u32,
        psf: &MotionPSF,
    ) -> CvResult<Vec<f32>> {
        // For simplicity, use Wiener-like filtering in spatial domain
        self.wiener_deconvolve(channel, width, height, psf)
    }

    /// Total Variation regularized deconvolution.
    fn tv_deconvolve(
        &self,
        channel: &[f32],
        width: u32,
        height: u32,
        psf: &MotionPSF,
    ) -> CvResult<Vec<f32>> {
        // Start with Richardson-Lucy
        let mut estimate = self.richardson_lucy_deconvolve(channel, width, height, psf)?;

        // Apply TV denoising iterations
        let lambda = 0.1;
        let tv_iterations = 10;

        for _iter in 0..tv_iterations {
            estimate = apply_tv_denoising(&estimate, width, height, lambda);
        }

        Ok(estimate)
    }

    /// Check convergence between iterations.
    fn check_convergence(&self, current: &[f32], previous: &[f32]) -> bool {
        let mut diff_sum = 0.0;
        let mut count = 0;

        for i in 0..current.len().min(previous.len()) {
            diff_sum += (current[i] - previous[i]).abs();
            count += 1;
        }

        if count == 0 {
            return true;
        }

        let mean_diff = diff_sum / count as f32;
        mean_diff < self.rl_params.threshold
    }
}

impl Default for Deconvolver {
    fn default() -> Self {
        Self::new(DeconvolutionMethod::RichardsonLucy)
    }
}

/// Extract a single channel from RGB image to float.
fn extract_channel(image: &[u8], width: u32, height: u32, channel: usize) -> Vec<f32> {
    let size = (width * height) as usize;
    let mut output = vec![0.0; size];

    for i in 0..size {
        if i * 3 + channel < image.len() {
            output[i] = image[i * 3 + channel] as f32 / 255.0;
        }
    }

    output
}

/// Insert a float channel back into RGB image.
fn insert_channel(image: &mut [u8], channel: &[f32], width: u32, height: u32, ch: usize) {
    let size = (width * height) as usize;

    for i in 0..size {
        if i < channel.len() && i * 3 + ch < image.len() {
            image[i * 3 + ch] = (channel[i] * 255.0).clamp(0.0, 255.0) as u8;
        }
    }
}

/// Convolve image with PSF.
fn convolve_with_psf(image: &[f32], width: u32, height: u32, psf: &MotionPSF) -> Vec<f32> {
    let mut output = vec![0.0; image.len()];

    let half_w = psf.width as i32 / 2;
    let half_h = psf.height as i32 / 2;

    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let mut sum = 0.0;

            for ky in 0..psf.height as i32 {
                for kx in 0..psf.width as i32 {
                    let ix = x + kx - half_w;
                    let iy = y + ky - half_h;

                    if ix >= 0 && ix < width as i32 && iy >= 0 && iy < height as i32 {
                        let img_idx = (iy * width as i32 + ix) as usize;
                        let kernel_idx = ky as usize * psf.width + kx as usize;

                        if img_idx < image.len() && kernel_idx < psf.kernel.len() {
                            sum += image[img_idx] * psf.kernel[kernel_idx];
                        }
                    }
                }
            }

            let out_idx = (y * width as i32 + x) as usize;
            if out_idx < output.len() {
                output[out_idx] = sum;
            }
        }
    }

    output
}

/// Flip PSF for correlation (reverse convolution).
fn flip_psf(psf: &MotionPSF) -> MotionPSF {
    let mut flipped = MotionPSF::new(psf.width, psf.height);

    for y in 0..psf.height {
        for x in 0..psf.width {
            let src_x = psf.width - 1 - x;
            let src_y = psf.height - 1 - y;
            flipped.set(x, y, psf.get(src_x, src_y));
        }
    }

    flipped
}

/// Apply Total Variation denoising.
fn apply_tv_denoising(image: &[f32], width: u32, height: u32, lambda: f32) -> Vec<f32> {
    let mut output = image.to_vec();

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = (y * width + x) as usize;
            if idx >= image.len() {
                continue;
            }

            // Compute gradients
            let idx_right = (y * width + x + 1) as usize;
            let idx_left = (y * width + x - 1) as usize;
            let idx_down = ((y + 1) * width + x) as usize;
            let idx_up = ((y - 1) * width + x) as usize;

            if idx_right < image.len()
                && idx_left < image.len()
                && idx_down < image.len()
                && idx_up < image.len()
            {
                let grad_x = image[idx_right] - image[idx_left];
                let grad_y = image[idx_down] - image[idx_up];

                // TV regularization
                let grad_mag = (grad_x * grad_x + grad_y * grad_y).sqrt().max(f32::EPSILON);
                let div = (grad_x + grad_y) / grad_mag;

                output[idx] = image[idx] + lambda * div;
                output[idx] = output[idx].clamp(0.0, 1.0);
            }
        }
    }

    output
}

/// Compute image gradient magnitude (for edge detection in deconvolution).
#[allow(dead_code)]
fn compute_gradient_magnitude(image: &[f32], width: u32, height: u32) -> Vec<f32> {
    let mut gradient = vec![0.0; image.len()];

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = (y * width + x) as usize;
            let idx_right = (y * width + x + 1) as usize;
            let idx_left = (y * width + x - 1) as usize;
            let idx_down = ((y + 1) * width + x) as usize;
            let idx_up = ((y - 1) * width + x) as usize;

            if idx < image.len()
                && idx_right < image.len()
                && idx_left < image.len()
                && idx_down < image.len()
                && idx_up < image.len()
            {
                let grad_x = (image[idx_right] - image[idx_left]) / 2.0;
                let grad_y = (image[idx_down] - image[idx_up]) / 2.0;
                gradient[idx] = (grad_x * grad_x + grad_y * grad_y).sqrt();
            }
        }
    }

    gradient
}

/// Edge-preserving smoothing for deconvolution.
#[allow(dead_code)]
fn bilateral_filter(
    image: &[f32],
    width: u32,
    height: u32,
    spatial_sigma: f32,
    range_sigma: f32,
) -> Vec<f32> {
    let mut output = vec![0.0; image.len()];
    let window_size = (spatial_sigma * 3.0).ceil() as i32;

    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let idx = (y * width as i32 + x) as usize;
            if idx >= image.len() {
                continue;
            }

            let center_val = image[idx];
            let mut sum = 0.0;
            let mut weight_sum = 0.0;

            for dy in -window_size..=window_size {
                for dx in -window_size..=window_size {
                    let nx = x + dx;
                    let ny = y + dy;

                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let nidx = (ny * width as i32 + nx) as usize;
                        if nidx < image.len() {
                            let neighbor_val = image[nidx];

                            // Spatial weight
                            let spatial_dist = (dx * dx + dy * dy) as f32;
                            let spatial_weight =
                                (-spatial_dist / (2.0 * spatial_sigma * spatial_sigma)).exp();

                            // Range weight
                            let range_dist = (center_val - neighbor_val).abs();
                            let range_weight = (-range_dist * range_dist
                                / (2.0 * range_sigma * range_sigma))
                                .exp();

                            let weight = spatial_weight * range_weight;
                            sum += neighbor_val * weight;
                            weight_sum += weight;
                        }
                    }
                }
            }

            if weight_sum > 0.0 {
                output[idx] = sum / weight_sum;
            } else {
                output[idx] = center_val;
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wiener_params_default() {
        let params = WienerParams::default();
        assert!((params.nsr - 0.01).abs() < 0.001);
    }

    #[test]
    fn test_rl_params_default() {
        let params = RichardsonLucyParams::default();
        assert_eq!(params.iterations, 30);
        assert!((params.threshold - 0.001).abs() < 0.0001);
    }

    #[test]
    fn test_deconvolver_new() {
        let deconv = Deconvolver::new(DeconvolutionMethod::Wiener);
        assert!(matches!(deconv.method, DeconvolutionMethod::Wiener));
    }

    #[test]
    fn test_extract_channel() {
        let image = vec![10u8, 20, 30, 40, 50, 60];
        let channel = extract_channel(&image, 2, 1, 0);
        assert_eq!(channel.len(), 2);
        assert!((channel[0] - 10.0 / 255.0).abs() < 0.001);
    }

    #[test]
    fn test_flip_psf() {
        let mut psf = MotionPSF::new(3, 3);
        psf.set(0, 0, 1.0);
        psf.set(2, 2, 2.0);

        let flipped = flip_psf(&psf);
        assert!((flipped.get(2, 2) - 1.0).abs() < 0.001);
        assert!((flipped.get(0, 0) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_tv_denoising() {
        let image = vec![0.5f32; 100];
        let denoised = apply_tv_denoising(&image, 10, 10, 0.1);
        assert_eq!(denoised.len(), 100);
    }
}
