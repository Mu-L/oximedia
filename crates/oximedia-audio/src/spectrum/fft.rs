//! FFT implementation and window functions.

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::f64::consts::PI;
use std::sync::Arc;

/// Window function type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WindowFunction {
    /// Rectangular window (no windowing).
    Rectangle,
    /// Hann window (raised cosine).
    #[default]
    Hann,
    /// Hamming window.
    Hamming,
    /// Blackman window.
    Blackman,
    /// Blackman-Harris window.
    BlackmanHarris,
    /// Kaiser window.
    Kaiser(u32),
    /// Tukey window (tapered cosine).
    Tukey(u32),
    /// Bartlett window (triangular).
    Bartlett,
    /// Welch window.
    Welch,
    /// Flat-top window.
    FlatTop,
}

impl WindowFunction {
    /// Generate window coefficients.
    #[must_use]
    pub fn generate(&self, size: usize) -> Vec<f64> {
        match self {
            Self::Rectangle => vec![1.0; size],
            Self::Hann => Self::generate_hann(size),
            Self::Hamming => Self::generate_hamming(size),
            Self::Blackman => Self::generate_blackman(size),
            Self::BlackmanHarris => Self::generate_blackman_harris(size),
            Self::Kaiser(beta) => Self::generate_kaiser(size, f64::from(*beta)),
            Self::Tukey(alpha) => Self::generate_tukey(size, f64::from(*alpha) / 100.0),
            Self::Bartlett => Self::generate_bartlett(size),
            Self::Welch => Self::generate_welch(size),
            Self::FlatTop => Self::generate_flattop(size),
        }
    }

    /// Hann window: 0.5 * (1 - cos(2*pi*n/(N-1))).
    fn generate_hann(size: usize) -> Vec<f64> {
        if size == 0 {
            return Vec::new();
        }
        if size == 1 {
            return vec![1.0];
        }

        (0..size)
            .map(|i| {
                let n = i as f64;
                let n_max = (size - 1) as f64;
                0.5 * (1.0 - (2.0 * PI * n / n_max).cos())
            })
            .collect()
    }

    /// Hamming window: 0.54 - 0.46 * cos(2*pi*n/(N-1)).
    fn generate_hamming(size: usize) -> Vec<f64> {
        if size == 0 {
            return Vec::new();
        }
        if size == 1 {
            return vec![1.0];
        }

        (0..size)
            .map(|i| {
                let n = i as f64;
                let n_max = (size - 1) as f64;
                0.54 - 0.46 * (2.0 * PI * n / n_max).cos()
            })
            .collect()
    }

    /// Blackman window.
    fn generate_blackman(size: usize) -> Vec<f64> {
        if size == 0 {
            return Vec::new();
        }
        if size == 1 {
            return vec![1.0];
        }

        const A0: f64 = 0.42;
        const A1: f64 = 0.5;
        const A2: f64 = 0.08;

        (0..size)
            .map(|i| {
                let n = i as f64;
                let n_max = (size - 1) as f64;
                A0 - A1 * (2.0 * PI * n / n_max).cos() + A2 * (4.0 * PI * n / n_max).cos()
            })
            .collect()
    }

    /// Blackman-Harris window.
    fn generate_blackman_harris(size: usize) -> Vec<f64> {
        if size == 0 {
            return Vec::new();
        }
        if size == 1 {
            return vec![1.0];
        }

        const A0: f64 = 0.35875;
        const A1: f64 = 0.48829;
        const A2: f64 = 0.14128;
        const A3: f64 = 0.01168;

        (0..size)
            .map(|i| {
                let n = i as f64;
                let n_max = (size - 1) as f64;
                A0 - A1 * (2.0 * PI * n / n_max).cos() + A2 * (4.0 * PI * n / n_max).cos()
                    - A3 * (6.0 * PI * n / n_max).cos()
            })
            .collect()
    }

    /// Kaiser window.
    fn generate_kaiser(size: usize, beta: f64) -> Vec<f64> {
        if size == 0 {
            return Vec::new();
        }
        if size == 1 {
            return vec![1.0];
        }

        let i0_beta = Self::bessel_i0(beta);
        let n_max = (size - 1) as f64;

        (0..size)
            .map(|i| {
                let n = i as f64;
                let arg = beta * (1.0 - ((2.0 * n / n_max) - 1.0).powi(2)).sqrt();
                Self::bessel_i0(arg) / i0_beta
            })
            .collect()
    }

    /// Modified Bessel function of the first kind, order 0.
    fn bessel_i0(x: f64) -> f64 {
        let mut sum = 1.0;
        let mut term = 1.0;
        let x_half = x / 2.0;

        for k in 1..=50 {
            term *= (x_half / k as f64).powi(2);
            sum += term;
            if term < 1e-12 * sum {
                break;
            }
        }

        sum
    }

    /// Tukey window (tapered cosine).
    fn generate_tukey(size: usize, alpha: f64) -> Vec<f64> {
        if size == 0 {
            return Vec::new();
        }
        if size == 1 {
            return vec![1.0];
        }

        let alpha = alpha.clamp(0.0, 1.0);
        let n_max = (size - 1) as f64;

        (0..size)
            .map(|i| {
                let n = i as f64;
                let ratio = n / n_max;

                if ratio < alpha / 2.0 {
                    0.5 * (1.0 + (2.0 * PI * ratio / alpha - PI).cos())
                } else if ratio > 1.0 - alpha / 2.0 {
                    0.5 * (1.0 + (2.0 * PI * (1.0 - ratio) / alpha - PI).cos())
                } else {
                    1.0
                }
            })
            .collect()
    }

    /// Bartlett window (triangular).
    fn generate_bartlett(size: usize) -> Vec<f64> {
        if size == 0 {
            return Vec::new();
        }
        if size == 1 {
            return vec![1.0];
        }

        let n_max = (size - 1) as f64;

        (0..size)
            .map(|i| {
                let n = i as f64;
                1.0 - ((n - n_max / 2.0) / (n_max / 2.0)).abs()
            })
            .collect()
    }

    /// Welch window.
    fn generate_welch(size: usize) -> Vec<f64> {
        if size == 0 {
            return Vec::new();
        }
        if size == 1 {
            return vec![1.0];
        }

        let n_max = (size - 1) as f64;

        (0..size)
            .map(|i| {
                let n = i as f64;
                let ratio = (n - n_max / 2.0) / (n_max / 2.0);
                1.0 - ratio * ratio
            })
            .collect()
    }

    /// Flat-top window.
    fn generate_flattop(size: usize) -> Vec<f64> {
        if size == 0 {
            return Vec::new();
        }
        if size == 1 {
            return vec![1.0];
        }

        const A0: f64 = 0.21557895;
        const A1: f64 = 0.41663158;
        const A2: f64 = 0.277263158;
        const A3: f64 = 0.083578947;
        const A4: f64 = 0.006947368;

        (0..size)
            .map(|i| {
                let n = i as f64;
                let n_max = (size - 1) as f64;
                A0 - A1 * (2.0 * PI * n / n_max).cos() + A2 * (4.0 * PI * n / n_max).cos()
                    - A3 * (6.0 * PI * n / n_max).cos()
                    + A4 * (8.0 * PI * n / n_max).cos()
            })
            .collect()
    }

    /// Get the coherent gain of the window (sum of coefficients divided by size).
    #[must_use]
    pub fn coherent_gain(&self, size: usize) -> f64 {
        if size == 0 {
            return 0.0;
        }
        let window = self.generate(size);
        window.iter().sum::<f64>() / size as f64
    }

    /// Get the equivalent noise bandwidth factor.
    #[must_use]
    pub fn enbw(&self, size: usize) -> f64 {
        if size == 0 {
            return 0.0;
        }
        let window = self.generate(size);
        let sum_squares: f64 = window.iter().map(|&x| x * x).sum();
        let sum: f64 = window.iter().sum();
        size as f64 * sum_squares / (sum * sum)
    }
}

/// FFT processor.
pub struct FftProcessor {
    fft_size: usize,
    window: Vec<f64>,
    fft: Arc<dyn Fft<f64>>,
    scratch_buffer: Vec<Complex<f64>>,
}

impl FftProcessor {
    /// Create a new FFT processor.
    #[must_use]
    pub fn new(fft_size: usize, window_fn: WindowFunction) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        Self {
            fft_size,
            window: window_fn.generate(fft_size),
            fft,
            scratch_buffer: vec![Complex::new(0.0, 0.0); fft_size],
        }
    }

    /// Get FFT size.
    #[must_use]
    pub const fn size(&self) -> usize {
        self.fft_size
    }

    /// Process audio samples and return frequency domain representation.
    pub fn process(&mut self, samples: &[f64]) -> Vec<Complex<f64>> {
        // Apply zero-padding if needed
        let input_size = samples.len().min(self.fft_size);

        // Apply window and convert to complex
        let mut buffer: Vec<Complex<f64>> = (0..self.fft_size)
            .map(|i| {
                if i < input_size {
                    Complex::new(samples[i] * self.window[i], 0.0)
                } else {
                    Complex::new(0.0, 0.0)
                }
            })
            .collect();

        // Perform FFT
        self.fft.process(&mut buffer);

        buffer
    }

    /// Process and return magnitude spectrum.
    pub fn magnitude_spectrum(&mut self, samples: &[f64]) -> Vec<f64> {
        let spectrum = self.process(samples);
        spectrum.iter().map(|c| c.norm()).collect()
    }

    /// Process and return power spectrum (magnitude squared).
    pub fn power_spectrum(&mut self, samples: &[f64]) -> Vec<f64> {
        let spectrum = self.process(samples);
        spectrum.iter().map(|c| c.norm_sqr()).collect()
    }

    /// Process and return phase spectrum.
    pub fn phase_spectrum(&mut self, samples: &[f64]) -> Vec<f64> {
        let spectrum = self.process(samples);
        spectrum.iter().map(|c| c.arg()).collect()
    }

    /// Convert magnitude to decibels.
    #[must_use]
    pub fn to_db(magnitude: f64, reference: f64) -> f64 {
        if magnitude <= 0.0 {
            -100.0
        } else {
            20.0 * (magnitude / reference).log10()
        }
    }

    /// Convert power to decibels.
    #[must_use]
    pub fn power_to_db(power: f64, reference: f64) -> f64 {
        if power <= 0.0 {
            -100.0
        } else {
            10.0 * (power / reference).log10()
        }
    }

    /// Get frequency bin for a given index.
    #[must_use]
    pub fn bin_frequency(&self, bin: usize, sample_rate: f64) -> f64 {
        bin as f64 * sample_rate / self.fft_size as f64
    }

    /// Get bin index for a given frequency.
    #[must_use]
    pub fn frequency_to_bin(&self, frequency: f64, sample_rate: f64) -> usize {
        ((frequency * self.fft_size as f64) / sample_rate)
            .round()
            .max(0.0) as usize
    }

    /// Get the Nyquist bin index.
    #[must_use]
    pub const fn nyquist_bin(&self) -> usize {
        self.fft_size / 2
    }
}

/// Mel scale conversion utilities.
pub struct MelScale;

impl MelScale {
    /// Convert frequency (Hz) to mel scale.
    #[must_use]
    pub fn hz_to_mel(hz: f64) -> f64 {
        2595.0 * (1.0 + hz / 700.0).log10()
    }

    /// Convert mel scale to frequency (Hz).
    #[must_use]
    pub fn mel_to_hz(mel: f64) -> f64 {
        700.0 * (10.0_f64.powf(mel / 2595.0) - 1.0)
    }

    /// Create mel filterbank.
    #[must_use]
    pub fn create_filterbank(
        num_filters: usize,
        fft_size: usize,
        sample_rate: f64,
        min_freq: f64,
        max_freq: f64,
    ) -> Vec<Vec<f64>> {
        let min_mel = Self::hz_to_mel(min_freq);
        let max_mel = Self::hz_to_mel(max_freq);

        // Create equally spaced mel points
        let mel_points: Vec<f64> = (0..=num_filters + 1)
            .map(|i| min_mel + (max_mel - min_mel) * i as f64 / (num_filters + 1) as f64)
            .collect();

        // Convert back to Hz
        let hz_points: Vec<f64> = mel_points.iter().map(|&m| Self::mel_to_hz(m)).collect();

        // Convert to FFT bin numbers
        let bin_points: Vec<usize> = hz_points
            .iter()
            .map(|&f| ((fft_size + 1) as f64 * f / sample_rate).floor() as usize)
            .collect();

        // Create filterbank
        let mut filters = Vec::new();

        for i in 0..num_filters {
            let mut filter = vec![0.0; fft_size / 2 + 1];

            let start = bin_points[i];
            let center = bin_points[i + 1];
            let end = bin_points[i + 2];

            // Rising slope
            for k in start..center {
                if center > start {
                    filter[k] = (k - start) as f64 / (center - start) as f64;
                }
            }

            // Falling slope
            for k in center..end {
                if end > center {
                    filter[k] = (end - k) as f64 / (end - center) as f64;
                }
            }

            filters.push(filter);
        }

        filters
    }

    /// Apply mel filterbank to power spectrum.
    #[must_use]
    pub fn apply_filterbank(spectrum: &[f64], filterbank: &[Vec<f64>]) -> Vec<f64> {
        filterbank
            .iter()
            .map(|filter| {
                spectrum
                    .iter()
                    .zip(filter.iter())
                    .map(|(&s, &f)| s * f)
                    .sum()
            })
            .collect()
    }

    /// Compute Mel Frequency Cepstral Coefficients (MFCC).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn compute_mfcc(mel_spectrum: &[f64], num_coeffs: usize) -> Vec<f64> {
        let n = mel_spectrum.len();

        (0..num_coeffs)
            .map(|i| {
                mel_spectrum
                    .iter()
                    .enumerate()
                    .map(|(k, &val)| {
                        let log_val = if val > 0.0 { val.ln() } else { -100.0 };
                        log_val * (PI * i as f64 * (k as f64 + 0.5) / n as f64).cos()
                    })
                    .sum()
            })
            .collect()
    }
}

/// Overlap-add processing for streaming FFT.
pub struct OverlapAdd {
    fft_size: usize,
    hop_size: usize,
    buffer: Vec<f64>,
}

impl OverlapAdd {
    /// Create a new overlap-add processor.
    #[must_use]
    pub fn new(fft_size: usize, hop_size: usize) -> Self {
        Self {
            fft_size,
            hop_size,
            buffer: Vec::new(),
        }
    }

    /// Add samples to buffer and return available frames.
    pub fn push(&mut self, samples: &[f64]) -> Vec<Vec<f64>> {
        self.buffer.extend_from_slice(samples);

        let mut frames = Vec::new();
        while self.buffer.len() >= self.fft_size {
            frames.push(self.buffer[..self.fft_size].to_vec());
            self.buffer.drain(..self.hop_size);
        }

        frames
    }

    /// Get remaining buffered samples.
    #[must_use]
    pub fn remaining(&self) -> &[f64] {
        &self.buffer
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Get buffer length.
    #[must_use]
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }
}
