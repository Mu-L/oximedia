//! Frequency domain image processing using 2D Discrete Fourier Transform.
//!
//! Provides DFT-based analysis and filtering for single-channel (grayscale)
//! images.  The 2D DFT is computed via row-column decomposition: a 1D DFT is
//! applied to every row, then every column of the intermediate result.  This
//! gives an O(N² · M²) algorithm — correct and transparent for moderate image
//! sizes; for very large images a fast-convolution approach would be preferable
//! but is outside the scope of this module.
//!
//! # Coordinate convention
//!
//! Frequency indices run from `0` to `width-1` (resp. `height-1`).  Index `k`
//! corresponds to a normalised frequency of `k / N` (i.e. cycles per pixel).
//! The DC component sits at `(0, 0)`.  The `magnitude_spectrum` and
//! `phase_spectrum` methods shift the DC to the centre of the output image, as
//! is conventional in visualisation tools.

#![allow(dead_code)]

use std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The result of a 2D Discrete Fourier Transform applied to a grayscale image.
///
/// Coefficients are stored in row-major order with the same layout as the
/// input image: index `y * width + x` holds the frequency component at
/// horizontal index `x` and vertical index `y`.
#[derive(Debug, Clone)]
pub struct FrequencyDomain {
    /// Width of the original image (and the frequency grid).
    pub width: u32,
    /// Height of the original image (and the frequency grid).
    pub height: u32,
    /// Real parts of the DFT coefficients.
    pub real: Vec<f32>,
    /// Imaginary parts of the DFT coefficients.
    pub imag: Vec<f32>,
}

/// A frequency-domain filter specification.
#[derive(Debug, Clone)]
pub struct FrequencyFilter {
    /// Shape of the filter response.
    pub filter_type: FilterType,
    /// Primary cutoff frequency, normalised to `[0.0, 0.5]` (Nyquist = 0.5).
    pub cutoff_frequency: f32,
    /// Filter order — controls the steepness of the roll-off for Butterworth
    /// responses.  For ideal (brick-wall) filters this field is ignored.
    pub order: u32,
}

/// Shape of a frequency-domain filter's gain function.
#[derive(Debug, Clone)]
pub enum FilterType {
    /// Pass frequencies below `cutoff_frequency` (blurring effect).
    LowPass,
    /// Pass frequencies above `cutoff_frequency` (edge-enhancement effect).
    HighPass,
    /// Pass frequencies between `low` and `high`.
    BandPass {
        /// Lower cutoff (inclusive edge).
        low: f32,
        /// Upper cutoff (inclusive edge).
        high: f32,
    },
    /// Reject frequencies between `low` and `high`.
    BandStop {
        /// Lower cutoff of the stop band.
        low: f32,
        /// Upper cutoff of the stop band.
        high: f32,
    },
    /// Suppress specific frequency pairs.  Each tuple is `(fx, fy)` in
    /// normalised units.  Any coefficient within 0.02 of a listed centre is
    /// zeroed.
    Notch(Vec<(f32, f32)>),
}

// ---------------------------------------------------------------------------
// Core implementation
// ---------------------------------------------------------------------------

impl FrequencyDomain {
    // -----------------------------------------------------------------------
    // Construction / transforms
    // -----------------------------------------------------------------------

    /// Compute the 2D DFT of a row-major grayscale image.
    ///
    /// `pixels` must have length `width * height`.  Values are expected in an
    /// arbitrary finite range (typically `[0.0, 1.0]`); scaling is handled
    /// internally.
    ///
    /// # Panics
    ///
    /// Panics if `pixels.len() != (width * height) as usize`.
    #[must_use]
    pub fn from_gray(pixels: &[f32], width: u32, height: u32) -> Self {
        let w = width as usize;
        let h = height as usize;
        assert_eq!(pixels.len(), w * h, "pixel buffer length mismatch");

        // --- Step 1: 1D DFT over every row -----------------------------------
        // After this step `re_rows[y*w + x]` and `im_rows[y*w + x]` hold the
        // row-wise DFT coefficient at column-frequency `x` for row `y`.
        let mut re_rows = vec![0.0_f32; w * h];
        let mut im_rows = vec![0.0_f32; w * h];

        for y in 0..h {
            let row: Vec<f32> = pixels[y * w..(y + 1) * w].to_vec();
            let (re, im) = dft_row(&row);
            re_rows[y * w..(y + 1) * w].copy_from_slice(&re);
            im_rows[y * w..(y + 1) * w].copy_from_slice(&im);
        }

        // --- Step 2: 1D DFT over every column --------------------------------
        // We extract each column from the row-wise result and DFT it.
        let mut real = vec![0.0_f32; w * h];
        let mut imag = vec![0.0_f32; w * h];

        for x in 0..w {
            // Gather the complex column values.
            let col_re: Vec<f32> = (0..h).map(|y| re_rows[y * w + x]).collect();
            let col_im: Vec<f32> = (0..h).map(|y| im_rows[y * w + x]).collect();

            let (re2, im2) = dft_complex_row(&col_re, &col_im);

            for y in 0..h {
                real[y * w + x] = re2[y];
                imag[y * w + x] = im2[y];
            }
        }

        Self {
            width,
            height,
            real,
            imag,
        }
    }

    /// Reconstruct a spatial-domain grayscale image from DFT coefficients via
    /// the inverse DFT.
    ///
    /// The output is normalised so that values match the original input range.
    /// The returned slice has length `width * height`.
    #[must_use]
    pub fn to_gray(&self) -> Vec<f32> {
        let w = self.width as usize;
        let h = self.height as usize;

        // --- IDFT over columns ----------------------------------------------
        let mut re_cols = vec![0.0_f32; w * h];
        let mut im_cols = vec![0.0_f32; w * h];

        for x in 0..w {
            let col_re: Vec<f32> = (0..h).map(|y| self.real[y * w + x]).collect();
            let col_im: Vec<f32> = (0..h).map(|y| self.imag[y * w + x]).collect();

            let (re2, im2) = idft_complex_row(&col_re, &col_im);

            for y in 0..h {
                re_cols[y * w + x] = re2[y];
                im_cols[y * w + x] = im2[y];
            }
        }

        // --- IDFT over rows -------------------------------------------------
        let mut output = vec![0.0_f32; w * h];

        for y in 0..h {
            let row_re: Vec<f32> = re_cols[y * w..(y + 1) * w].to_vec();
            let row_im: Vec<f32> = im_cols[y * w..(y + 1) * w].to_vec();

            let (re2, _im2) = idft_complex_row(&row_re, &row_im);

            output[y * w..(y + 1) * w].copy_from_slice(&re2);
        }

        output
    }

    // -----------------------------------------------------------------------
    // Spectrum helpers
    // -----------------------------------------------------------------------

    /// Compute the magnitude spectrum `sqrt(re² + im²)`, shifted so that DC
    /// sits at the image centre.
    #[must_use]
    pub fn magnitude_spectrum(&self) -> Vec<f32> {
        let w = self.width as usize;
        let h = self.height as usize;
        let mag: Vec<f32> = self
            .real
            .iter()
            .zip(self.imag.iter())
            .map(|(&r, &i)| (r * r + i * i).sqrt())
            .collect();
        fftshift(&mag, w, h)
    }

    /// Compute the phase spectrum `atan2(im, re)` in radians, shifted so that
    /// DC sits at the image centre.
    #[must_use]
    pub fn phase_spectrum(&self) -> Vec<f32> {
        let w = self.width as usize;
        let h = self.height as usize;
        let phase: Vec<f32> = self
            .imag
            .iter()
            .zip(self.real.iter())
            .map(|(&i, &r)| i.atan2(r))
            .collect();
        fftshift(&phase, w, h)
    }

    /// Power spectral density (linear, un-normalised): `re² + im²`.
    ///
    /// The result is *not* shifted to centre; use `magnitude_spectrum` if you
    /// want a centred display-ready spectrum.
    #[must_use]
    pub fn power_spectrum_density(&self) -> Vec<f32> {
        self.real
            .iter()
            .zip(self.imag.iter())
            .map(|(&r, &i)| r * r + i * i)
            .collect()
    }

    /// Return the normalised frequency `(fx, fy)` of the DFT coefficient with
    /// the largest magnitude, excluding the DC component at `(0, 0)`.
    ///
    /// Frequencies are in `[0.0, 1.0)` before Nyquist folding; indices `k`
    /// larger than `N/2` represent negative frequencies.
    ///
    /// Returns `(0.0, 0.0)` if the image is empty or all non-DC power is zero.
    #[must_use]
    pub fn dominant_frequency(&self) -> (f32, f32) {
        let w = self.width as usize;
        let h = self.height as usize;
        if w == 0 || h == 0 {
            return (0.0, 0.0);
        }

        let mut best_mag = 0.0_f32;
        let mut best_x = 0usize;
        let mut best_y = 0usize;

        for y in 0..h {
            for x in 0..w {
                // Skip DC
                if x == 0 && y == 0 {
                    continue;
                }
                let idx = y * w + x;
                let r = self.real[idx];
                let i = self.imag[idx];
                let mag = r * r + i * i;
                if mag > best_mag {
                    best_mag = mag;
                    best_x = x;
                    best_y = y;
                }
            }
        }

        if best_mag == 0.0 {
            return (0.0, 0.0);
        }

        let fx = best_x as f32 / w as f32;
        let fy = best_y as f32 / h as f32;
        (fx, fy)
    }

    // -----------------------------------------------------------------------
    // Filtering
    // -----------------------------------------------------------------------

    /// Apply a frequency-domain filter in-place.
    ///
    /// For each DFT coefficient at normalised frequency `(fx, fy)` (in
    /// `[0.0, 1.0)`), a gain is computed according to `filter.filter_type` and
    /// both the real and imaginary parts are multiplied by that gain.
    ///
    /// The radius used for LowPass/HighPass/BandPass/BandStop is the Euclidean
    /// distance `sqrt(fx² + fy²)`, where frequencies above Nyquist are folded
    /// into `[0, 0.5]` first.
    pub fn apply_filter(&mut self, filter: &FrequencyFilter) {
        let w = self.width as usize;
        let h = self.height as usize;

        for y in 0..h {
            for x in 0..w {
                // Fold negative frequencies into [0, 0.5]
                let raw_fx = x as f32 / w as f32;
                let raw_fy = y as f32 / h as f32;
                let fx = if raw_fx > 0.5 { 1.0 - raw_fx } else { raw_fx };
                let fy = if raw_fy > 0.5 { 1.0 - raw_fy } else { raw_fy };
                let radius = (fx * fx + fy * fy).sqrt();

                let gain = compute_gain(
                    &filter.filter_type,
                    radius,
                    filter.cutoff_frequency,
                    filter.order,
                    fx,
                    fy,
                );

                let idx = y * w + x;
                self.real[idx] *= gain;
                self.imag[idx] *= gain;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Free functions — 1D DFT / IDFT
// ---------------------------------------------------------------------------

/// Compute the 1D DFT of a real-valued sequence.
///
/// Returns `(real, imag)` each of length `samples.len()`.
///
/// Uses the direct O(N²) summation formula:
/// ```text
/// X[k] = Σ_{n=0}^{N-1}  x[n] · exp(-j 2π k n / N)
///       = Σ x[n] · (cos(2π k n / N)  -  j·sin(2π k n / N))
/// ```
#[must_use]
pub fn dft_row(samples: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let n = samples.len();
    if n == 0 {
        return (vec![], vec![]);
    }
    let mut re = vec![0.0_f32; n];
    let mut im = vec![0.0_f32; n];
    let n_f = n as f32;
    for k in 0..n {
        let mut sum_re = 0.0_f32;
        let mut sum_im = 0.0_f32;
        for (t, &sample) in samples.iter().enumerate() {
            let angle = 2.0 * PI * k as f32 * t as f32 / n_f;
            sum_re += sample * angle.cos();
            sum_im -= sample * angle.sin();
        }
        re[k] = sum_re;
        im[k] = sum_im;
    }
    (re, im)
}

/// 1D DFT of a complex-valued sequence `(re_in, im_in)`.
///
/// Returns `(re_out, im_out)` each of length `re_in.len()`.
fn dft_complex_row(re_in: &[f32], im_in: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let n = re_in.len();
    if n == 0 {
        return (vec![], vec![]);
    }
    let mut re_out = vec![0.0_f32; n];
    let mut im_out = vec![0.0_f32; n];
    let n_f = n as f32;
    for k in 0..n {
        let mut sum_re = 0.0_f32;
        let mut sum_im = 0.0_f32;
        for t in 0..n {
            let angle = 2.0 * PI * k as f32 * t as f32 / n_f;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            // (a + jb)(cos - j·sin) = a·cos + b·sin + j(b·cos - a·sin)
            sum_re += re_in[t] * cos_a + im_in[t] * sin_a;
            sum_im += im_in[t] * cos_a - re_in[t] * sin_a;
        }
        re_out[k] = sum_re;
        im_out[k] = sum_im;
    }
    (re_out, im_out)
}

/// 1D inverse DFT of a complex-valued sequence.
///
/// Uses the conjugate DFT approach: `IDFT{X} = conj(DFT{conj(X)}) / N`.
fn idft_complex_row(re_in: &[f32], im_in: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let n = re_in.len();
    if n == 0 {
        return (vec![], vec![]);
    }
    // Conjugate the input
    let conj_im: Vec<f32> = im_in.iter().map(|&v| -v).collect();
    // Forward DFT of conjugated input
    let (re_tmp, im_tmp) = dft_complex_row(re_in, &conj_im);
    // Conjugate and scale
    let n_f = n as f32;
    let re_out: Vec<f32> = re_tmp.iter().map(|&v| v / n_f).collect();
    let im_out: Vec<f32> = im_tmp.iter().map(|&v| -v / n_f).collect();
    (re_out, im_out)
}

// ---------------------------------------------------------------------------
// Gain computation
// ---------------------------------------------------------------------------

/// Compute the filter gain for a single frequency component.
///
/// `radius` is the Euclidean distance from DC in normalised frequency units
/// `[0, 0.5√2]`.  `fx` and `fy` are the signed normalised frequencies used
/// for the Notch filter.
fn compute_gain(
    filter_type: &FilterType,
    radius: f32,
    cutoff: f32,
    order: u32,
    fx: f32,
    fy: f32,
) -> f32 {
    match filter_type {
        FilterType::LowPass => butterworth_low(radius, cutoff, order),
        FilterType::HighPass => 1.0 - butterworth_low(radius, cutoff, order),
        FilterType::BandPass { low, high } => {
            // High-pass at `low` multiplied by low-pass at `high`
            let hp = 1.0 - butterworth_low(radius, *low, order);
            let lp = butterworth_low(radius, *high, order);
            hp * lp
        }
        FilterType::BandStop { low, high } => {
            // Complement of BandPass
            let hp = 1.0 - butterworth_low(radius, *low, order);
            let lp = butterworth_low(radius, *high, order);
            1.0 - hp * lp
        }
        FilterType::Notch(centers) => {
            for &(cx, cy) in centers {
                let dx = fx - cx;
                let dy = fy - cy;
                if dx * dx + dy * dy < 0.02 * 0.02 {
                    return 0.0;
                }
                // Also suppress the symmetric conjugate frequency
                let dx2 = fx - (1.0 - cx);
                let dy2 = fy - (1.0 - cy);
                if dx2 * dx2 + dy2 * dy2 < 0.02 * 0.02 {
                    return 0.0;
                }
            }
            1.0
        }
    }
}

/// Butterworth low-pass gain.
///
/// When `order == 0` an ideal brick-wall filter is used instead
/// (`gain = 1 if radius < cutoff else 0`).
fn butterworth_low(radius: f32, cutoff: f32, order: u32) -> f32 {
    if cutoff <= 0.0 {
        return 0.0;
    }
    if order == 0 {
        // Ideal filter
        return if radius < cutoff { 1.0 } else { 0.0 };
    }
    let ratio = radius / cutoff;
    let exponent = 2.0 * order as f32;
    1.0 / (1.0 + ratio.powf(exponent)).sqrt()
}

// ---------------------------------------------------------------------------
// FFT-shift helper
// ---------------------------------------------------------------------------

/// Shift the zero-frequency (DC) component to the centre of the image.
///
/// This is equivalent to MATLAB's `fftshift` and NumPy's `np.fft.fftshift` for
/// a 2D array: we swap the four quadrants diagonally.
fn fftshift(data: &[f32], width: usize, height: usize) -> Vec<f32> {
    let mut shifted = vec![0.0_f32; width * height];
    let half_w = width / 2;
    let half_h = height / 2;
    for y in 0..height {
        for x in 0..width {
            let new_x = (x + half_w) % width;
            let new_y = (y + half_h) % height;
            shifted[new_y * width + new_x] = data[y * width + x];
        }
    }
    shifted
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create a flat (DC-only) image
    fn flat_image(w: u32, h: u32, value: f32) -> Vec<f32> {
        vec![value; (w * h) as usize]
    }

    // -----------------------------------------------------------------------

    #[test]
    fn test_dft_row_empty() {
        let (re, im) = dft_row(&[]);
        assert!(re.is_empty());
        assert!(im.is_empty());
    }

    #[test]
    fn test_dft_row_dc_only() {
        // A constant sequence → all energy in DC bin (k=0), rest ~0
        let samples = vec![1.0_f32; 8];
        let (re, im) = dft_row(&samples);
        // DC bin: sum = N = 8
        assert!((re[0] - 8.0).abs() < 1e-4, "DC re: {}", re[0]);
        assert!(im[0].abs() < 1e-4, "DC im: {}", im[0]);
        // All other bins near zero
        for k in 1..8 {
            assert!(re[k].abs() < 1e-4, "re[{}] = {}", k, re[k]);
            assert!(im[k].abs() < 1e-4, "im[{}] = {}", k, im[k]);
        }
    }

    #[test]
    fn test_from_gray_to_gray_roundtrip_flat() {
        let w = 4u32;
        let h = 4u32;
        let pixels = flat_image(w, h, 0.5);
        let fd = FrequencyDomain::from_gray(&pixels, w, h);
        let reconstructed = fd.to_gray();
        for (orig, rec) in pixels.iter().zip(reconstructed.iter()) {
            assert!((orig - rec).abs() < 1e-3, "mismatch: {} vs {}", orig, rec);
        }
    }

    #[test]
    fn test_from_gray_to_gray_roundtrip_gradient() {
        let w = 8u32;
        let h = 8u32;
        let pixels: Vec<f32> = (0..(w * h) as usize)
            .map(|i| i as f32 / (w * h) as f32)
            .collect();
        let fd = FrequencyDomain::from_gray(&pixels, w, h);
        let reconstructed = fd.to_gray();
        for (orig, rec) in pixels.iter().zip(reconstructed.iter()) {
            assert!((orig - rec).abs() < 1e-3, "mismatch: {} vs {}", orig, rec);
        }
    }

    #[test]
    fn test_magnitude_spectrum_length() {
        let w = 6u32;
        let h = 4u32;
        let fd = FrequencyDomain::from_gray(&flat_image(w, h, 1.0), w, h);
        let mag = fd.magnitude_spectrum();
        assert_eq!(mag.len(), (w * h) as usize);
    }

    #[test]
    fn test_phase_spectrum_length() {
        let w = 4u32;
        let h = 4u32;
        let fd = FrequencyDomain::from_gray(&flat_image(w, h, 0.3), w, h);
        let phase = fd.phase_spectrum();
        assert_eq!(phase.len(), (w * h) as usize);
    }

    #[test]
    fn test_power_spectrum_density_non_negative() {
        let w = 4u32;
        let h = 4u32;
        let pixels: Vec<f32> = (0..16).map(|i| i as f32 * 0.05).collect();
        let fd = FrequencyDomain::from_gray(&pixels, w, h);
        let psd = fd.power_spectrum_density();
        assert!(psd.iter().all(|&v| v >= 0.0));
    }

    #[test]
    fn test_lowpass_filter_preserves_dc() {
        let w = 4u32;
        let h = 4u32;
        let pixels = flat_image(w, h, 0.5);
        let mut fd = FrequencyDomain::from_gray(&pixels, w, h);
        let filter = FrequencyFilter {
            filter_type: FilterType::LowPass,
            cutoff_frequency: 0.3,
            order: 2,
        };
        fd.apply_filter(&filter);
        // DC should remain intact (radius = 0 < cutoff)
        assert!(fd.real[0].abs() > 0.01, "DC should survive lowpass");
    }

    #[test]
    fn test_highpass_filter_zeros_dc() {
        let w = 4u32;
        let h = 4u32;
        let pixels = flat_image(w, h, 1.0);
        let mut fd = FrequencyDomain::from_gray(&pixels, w, h);
        let filter = FrequencyFilter {
            filter_type: FilterType::HighPass,
            cutoff_frequency: 0.1,
            order: 4,
        };
        fd.apply_filter(&filter);
        // DC (radius = 0) → gain = 1 - butterworth(0) = 1 - 1 = 0
        assert!(
            fd.real[0].abs() < 1e-3,
            "DC should be zeroed by highpass: {}",
            fd.real[0]
        );
    }

    #[test]
    fn test_bandpass_filter_dimensions() {
        let w = 4u32;
        let h = 4u32;
        let pixels: Vec<f32> = (0..16).map(|i| (i as f32).sin()).collect();
        let mut fd = FrequencyDomain::from_gray(&pixels, w, h);
        let filter = FrequencyFilter {
            filter_type: FilterType::BandPass {
                low: 0.1,
                high: 0.4,
            },
            cutoff_frequency: 0.25,
            order: 2,
        };
        fd.apply_filter(&filter);
        // Just verify size is preserved
        assert_eq!(fd.real.len(), (w * h) as usize);
    }

    #[test]
    fn test_notch_filter_suppresses_target() {
        let w = 8u32;
        let h = 8u32;
        // Start with non-trivial image
        let pixels: Vec<f32> = (0..(w * h) as usize)
            .map(|i| ((i as f32) * 0.3).sin())
            .collect();
        let mut fd = FrequencyDomain::from_gray(&pixels, w, h);
        // Target the frequency at normalised (0.125, 0.0) — index (1, 0) in 8-wide
        let filter = FrequencyFilter {
            filter_type: FilterType::Notch(vec![(0.125, 0.0)]),
            cutoff_frequency: 0.0,
            order: 0,
        };
        fd.apply_filter(&filter);
        // The coefficient at (x=1, y=0) should now be 0
        assert!(
            fd.real[1].abs() < 1e-6 && fd.imag[1].abs() < 1e-6,
            "notch should zero target coefficient"
        );
    }

    #[test]
    fn test_dominant_frequency_not_dc() {
        let w = 8u32;
        let h = 1u32;
        // Single sinusoid at frequency 2/8 = 0.25 cycles/pixel
        let pixels: Vec<f32> = (0..8)
            .map(|x| (2.0 * PI * 2.0 * x as f32 / 8.0).cos())
            .collect();
        let fd = FrequencyDomain::from_gray(&pixels, w, h);
        let (fx, _fy) = fd.dominant_frequency();
        // Dominant should be near 0.25 (index 2 out of 8)
        assert!(
            (fx - 0.25).abs() < 0.05,
            "expected dominant near 0.25, got {}",
            fx
        );
    }

    #[test]
    fn test_fftshift_symmetry() {
        // Use an even-dimension grid so that two fftshift calls are a perfect
        // round-trip (each quadrant swap is self-inverse for even sizes).
        let data: Vec<f32> = (0..16).map(|i| i as f32).collect(); // 4×4
        let shifted = fftshift(&data, 4, 4);
        let double_shifted = fftshift(&shifted, 4, 4);
        for (a, b) in data.iter().zip(double_shifted.iter()) {
            assert!((a - b).abs() < 1e-6, "double-shift mismatch: {a} vs {b}");
        }
    }

    #[test]
    fn test_butterworth_low_ideal() {
        // order == 0 → ideal brick-wall
        assert_eq!(butterworth_low(0.1, 0.3, 0), 1.0);
        assert_eq!(butterworth_low(0.4, 0.3, 0), 0.0);
    }

    #[test]
    fn test_butterworth_low_rolloff() {
        // At cutoff the gain is exactly 1/sqrt(2) ≈ 0.707 for any order > 0
        let gain = butterworth_low(0.3, 0.3, 4);
        assert!(
            (gain - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5,
            "gain at cutoff should be 1/√2, got {}",
            gain
        );
    }
}
