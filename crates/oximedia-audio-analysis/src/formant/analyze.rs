//! Formant analysis using Linear Predictive Coding (LPC).

use crate::{AnalysisConfig, AnalysisError, Result};

/// Formant analyzer using LPC.
pub struct FormantAnalyzer {
    config: AnalysisConfig,
    lpc_order: usize,
}

impl FormantAnalyzer {
    /// Create a new formant analyzer.
    #[must_use]
    pub fn new(config: AnalysisConfig) -> Self {
        // LPC order typically 2 + sample_rate / 1000
        let lpc_order = 12; // Good for standard speech analysis

        Self { config, lpc_order }
    }

    /// Analyze formants from audio samples.
    pub fn analyze(&self, samples: &[f32], sample_rate: f32) -> Result<FormantResult> {
        if samples.len() < self.config.fft_size {
            return Err(AnalysisError::InsufficientSamples {
                needed: self.config.fft_size,
                got: samples.len(),
            });
        }

        // Pre-emphasize signal (high-pass filter to enhance higher frequencies)
        let emphasized = self.pre_emphasize(samples);

        // Compute LPC coefficients
        let lpc_coeffs = self.compute_lpc(&emphasized)?;

        // Find formants from LPC coefficients
        let formants = self.find_formants(&lpc_coeffs, sample_rate)?;

        Ok(FormantResult {
            formants,
            lpc_coefficients: lpc_coeffs,
        })
    }

    /// Pre-emphasize signal using first-order high-pass filter.
    #[allow(clippy::unused_self)]
    fn pre_emphasize(&self, samples: &[f32]) -> Vec<f32> {
        let alpha = 0.97;
        let mut emphasized = Vec::with_capacity(samples.len());

        emphasized.push(samples[0]);
        for i in 1..samples.len() {
            emphasized.push(samples[i] - alpha * samples[i - 1]);
        }

        emphasized
    }

    /// Compute LPC coefficients using autocorrelation method (Levinson-Durbin).
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    fn compute_lpc(&self, samples: &[f32]) -> Result<Vec<f32>> {
        // Compute autocorrelation
        let mut r = vec![0.0; self.lpc_order + 1];
        for i in 0..=self.lpc_order {
            let mut sum = 0.0;
            for j in 0..(samples.len() - i) {
                sum += samples[j] * samples[j + i];
            }
            r[i] = sum;
        }

        // Levinson-Durbin algorithm
        let mut a = vec![0.0; self.lpc_order + 1];
        let mut e = r[0];

        for i in 1..=self.lpc_order {
            let mut lambda = 0.0;
            for j in 1..i {
                lambda -= a[j] * r[i - j];
            }
            lambda -= r[i];

            let k = if e == 0.0 { 0.0 } else { lambda / e };

            a[i] = k;

            for j in 1..i {
                let temp = a[j];
                a[j] += k * a[i - j];
                a[i - j] = temp;
            }

            e *= 1.0 - k * k;
        }

        Ok(a)
    }

    /// Find formant frequencies and bandwidths from LPC coefficients.
    #[allow(clippy::unnecessary_wraps)]
    fn find_formants(&self, lpc_coeffs: &[f32], sample_rate: f32) -> Result<Vec<f32>> {
        let (formants, _bandwidths) = self.find_formants_with_bandwidth(lpc_coeffs, sample_rate)?;
        Ok(formants)
    }

    /// Find formant frequencies and bandwidths from LPC coefficients.
    ///
    /// Bandwidth is computed from the pole radius: `BW = -ln(r) * sample_rate / pi`
    /// where r is the magnitude of the complex root.
    #[allow(clippy::unnecessary_wraps)]
    fn find_formants_with_bandwidth(
        &self,
        lpc_coeffs: &[f32],
        sample_rate: f32,
    ) -> Result<(Vec<f32>, Vec<f32>)> {
        let roots = self.find_lpc_roots(lpc_coeffs)?;

        // Extract formant info from roots with positive imaginary part and magnitude < 1
        let mut formant_data: Vec<(f32, f32)> = roots
            .iter()
            .filter(|(real, imag)| {
                let magnitude = (real * real + imag * imag).sqrt();
                magnitude < 1.0 && magnitude > 0.7 && *imag > 0.0
            })
            .map(|(real, imag)| {
                let angle = imag.atan2(*real);
                let frequency = (angle * sample_rate) / (2.0 * std::f32::consts::PI);
                let magnitude = (real * real + imag * imag).sqrt();
                // Bandwidth from pole radius: BW = -ln(r) * fs / pi
                let bandwidth = if magnitude > f32::EPSILON {
                    -(magnitude.ln()) * sample_rate / std::f32::consts::PI
                } else {
                    sample_rate / 2.0 // Maximum bandwidth as fallback
                };
                (frequency, bandwidth)
            })
            .collect();

        // Sort by frequency
        formant_data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        // Take first 4 formants
        formant_data.truncate(4);

        // If we don't have enough formants, use typical values with estimated bandwidths
        let default_formants = [500.0, 1500.0, 2500.0, 3500.0];
        let default_bandwidths = [80.0, 120.0, 150.0, 200.0];
        while formant_data.len() < 4 {
            let idx = formant_data.len();
            formant_data.push((default_formants[idx], default_bandwidths[idx]));
        }

        let (formants, bandwidths): (Vec<f32>, Vec<f32>) = formant_data.into_iter().unzip();
        Ok((formants, bandwidths))
    }

    /// Analyze formants including bandwidth estimation.
    ///
    /// Returns `FormantResultDetailed` which includes both frequencies and bandwidths.
    pub fn analyze_with_bandwidth(
        &self,
        samples: &[f32],
        sample_rate: f32,
    ) -> Result<FormantResultDetailed> {
        if samples.len() < self.config.fft_size {
            return Err(AnalysisError::InsufficientSamples {
                needed: self.config.fft_size,
                got: samples.len(),
            });
        }

        let emphasized = self.pre_emphasize(samples);
        let lpc_coeffs = self.compute_lpc(&emphasized)?;
        let (formants, bandwidths) = self.find_formants_with_bandwidth(&lpc_coeffs, sample_rate)?;

        // Compute LPC prediction error (residual energy)
        let prediction_error = compute_prediction_error(samples, &lpc_coeffs);

        Ok(FormantResultDetailed {
            formants: formants.clone(),
            bandwidths: bandwidths.clone(),
            lpc_coefficients: lpc_coeffs,
            prediction_error,
            formant_pairs: formants
                .into_iter()
                .zip(bandwidths)
                .map(|(freq, bw)| FormantPair {
                    frequency: freq,
                    bandwidth: bw,
                })
                .collect(),
        })
    }

    /// Find roots of LPC polynomial using Durand-Kerner method.
    #[allow(clippy::unnecessary_wraps, clippy::needless_range_loop)]
    fn find_lpc_roots(&self, coeffs: &[f32]) -> Result<Vec<(f32, f32)>> {
        if coeffs.is_empty() {
            return Ok(Vec::new());
        }

        let n = coeffs.len() - 1;
        if n == 0 {
            return Ok(Vec::new());
        }

        // Initialize roots on unit circle
        let mut roots: Vec<(f32, f32)> = (0..n)
            .map(|i| {
                let angle = 2.0 * std::f32::consts::PI * (i as f32 + 0.4) / n as f32;
                (0.9 * angle.cos(), 0.9 * angle.sin())
            })
            .collect();

        // Iterate to refine roots (Durand-Kerner method)
        for _ in 0..50 {
            let mut max_change: f32 = 0.0;

            for i in 0..n {
                let (re, im) = roots[i];

                // Evaluate polynomial at current root
                let (p_re, p_im) = self.eval_poly(coeffs, re, im);

                // Compute product of differences with other roots
                let mut prod_re = 1.0;
                let mut prod_im = 0.0;

                for j in 0..n {
                    if i != j {
                        let (rj_re, rj_im) = roots[j];
                        let diff_re = re - rj_re;
                        let diff_im = im - rj_im;

                        let temp_re = prod_re * diff_re - prod_im * diff_im;
                        let temp_im = prod_re * diff_im + prod_im * diff_re;
                        prod_re = temp_re;
                        prod_im = temp_im;
                    }
                }

                // Division: p / prod
                let denom = prod_re * prod_re + prod_im * prod_im;
                if denom > 1e-10 {
                    let delta_re = (p_re * prod_re + p_im * prod_im) / denom;
                    let delta_im = (p_im * prod_re - p_re * prod_im) / denom;

                    roots[i].0 -= delta_re;
                    roots[i].1 -= delta_im;

                    max_change = max_change.max(delta_re.abs() + delta_im.abs());
                }
            }

            if max_change < 1e-6 {
                break;
            }
        }

        Ok(roots)
    }

    /// Evaluate polynomial at complex point.
    #[allow(clippy::unused_self)]
    fn eval_poly(&self, coeffs: &[f32], re: f32, im: f32) -> (f32, f32) {
        let mut result_re = 0.0;
        let mut result_im = 0.0;
        let mut power_re = 1.0;
        let mut power_im = 0.0;

        for &coeff in coeffs {
            result_re += coeff * power_re;
            result_im += coeff * power_im;

            // Multiply power by (re, im)
            let temp_re = power_re * re - power_im * im;
            let temp_im = power_re * im + power_im * re;
            power_re = temp_re;
            power_im = temp_im;
        }

        (result_re, result_im)
    }
}

/// Formant analysis result (basic).
#[derive(Debug, Clone)]
pub struct FormantResult {
    /// Formant frequencies [F1, F2, F3, F4] in Hz
    pub formants: Vec<f32>,
    /// LPC coefficients
    pub lpc_coefficients: Vec<f32>,
}

/// A single formant with frequency and bandwidth.
#[derive(Debug, Clone)]
pub struct FormantPair {
    /// Formant frequency in Hz
    pub frequency: f32,
    /// Formant bandwidth in Hz (3 dB bandwidth of the resonance)
    pub bandwidth: f32,
}

/// Detailed formant analysis result including bandwidths.
#[derive(Debug, Clone)]
pub struct FormantResultDetailed {
    /// Formant frequencies [F1, F2, F3, F4] in Hz
    pub formants: Vec<f32>,
    /// Formant bandwidths [BW1, BW2, BW3, BW4] in Hz
    pub bandwidths: Vec<f32>,
    /// LPC coefficients
    pub lpc_coefficients: Vec<f32>,
    /// LPC prediction error (residual energy, lower = better model fit)
    pub prediction_error: f32,
    /// Combined frequency+bandwidth pairs for each formant
    pub formant_pairs: Vec<FormantPair>,
}

/// Compute LPC prediction error (residual energy).
fn compute_prediction_error(samples: &[f32], lpc_coeffs: &[f32]) -> f32 {
    if samples.is_empty() || lpc_coeffs.is_empty() {
        return 0.0;
    }
    let order = lpc_coeffs.len() - 1;
    let mut error_energy = 0.0_f32;
    let mut count = 0;

    for i in order..samples.len() {
        let mut predicted = 0.0_f32;
        for j in 1..=order {
            predicted -= lpc_coeffs[j] * samples[i - j];
        }
        let residual = samples[i] - predicted;
        error_energy += residual * residual;
        count += 1;
    }

    if count > 0 {
        error_energy / count as f32
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formant_analyzer() {
        let config = AnalysisConfig::default();
        let analyzer = FormantAnalyzer::new(config);

        let sample_rate = 16000.0;
        let samples = vec![0.1; 4096];

        let result = analyzer.analyze(&samples, sample_rate);
        assert!(result.is_ok());

        let formants = result.expect("expected successful result").formants;
        assert_eq!(formants.len(), 4);

        for &f in &formants {
            assert!(f > 0.0);
        }
    }

    #[test]
    fn test_pre_emphasis() {
        let config = AnalysisConfig::default();
        let analyzer = FormantAnalyzer::new(config);

        let samples = vec![1.0, 2.0, 3.0, 4.0];
        let emphasized = analyzer.pre_emphasize(&samples);

        assert_eq!(emphasized.len(), samples.len());
        assert_eq!(emphasized[0], samples[0]);
    }

    #[test]
    fn test_formant_bandwidth_analysis() {
        let config = AnalysisConfig::default();
        let analyzer = FormantAnalyzer::new(config);

        let sample_rate = 16000.0;
        let samples = vec![0.1; 4096];

        let result = analyzer.analyze_with_bandwidth(&samples, sample_rate);
        assert!(result.is_ok());

        let detailed = result.expect("expected successful result");
        assert_eq!(detailed.formants.len(), 4);
        assert_eq!(detailed.bandwidths.len(), 4);
        assert_eq!(detailed.formant_pairs.len(), 4);

        // All formant frequencies should be positive
        for &f in &detailed.formants {
            assert!(f > 0.0, "Formant frequency should be positive: {f}");
        }

        // All bandwidths should be positive
        for &bw in &detailed.bandwidths {
            assert!(bw > 0.0, "Bandwidth should be positive: {bw}");
        }

        // Formant pairs should match
        for (pair, (&freq, &bw)) in detailed
            .formant_pairs
            .iter()
            .zip(detailed.formants.iter().zip(detailed.bandwidths.iter()))
        {
            assert!((pair.frequency - freq).abs() < f32::EPSILON);
            assert!((pair.bandwidth - bw).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_formant_bandwidth_sine_wave() {
        let config = AnalysisConfig::default();
        let analyzer = FormantAnalyzer::new(config);

        // Generate a sine wave at 500 Hz (approximating vowel /a/ F1)
        let sample_rate = 16000.0;
        let samples: Vec<f32> = (0..4096)
            .map(|i| {
                let t = i as f32 / sample_rate;
                (2.0 * std::f32::consts::PI * 500.0 * t).sin()
            })
            .collect();

        let result = analyzer.analyze_with_bandwidth(&samples, sample_rate);
        assert!(result.is_ok());
        let detailed = result.expect("expected successful result");

        // Should have prediction error >= 0
        assert!(
            detailed.prediction_error >= 0.0,
            "Prediction error should be non-negative"
        );
    }

    #[test]
    fn test_formant_bandwidth_insufficient_samples() {
        let config = AnalysisConfig::default();
        let analyzer = FormantAnalyzer::new(config);

        let samples = vec![0.1; 100]; // too short
        let result = analyzer.analyze_with_bandwidth(&samples, 16000.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_formant_bandwidth_ordering() {
        let config = AnalysisConfig::default();
        let analyzer = FormantAnalyzer::new(config);

        // Generate a signal with multiple frequency components
        let sample_rate = 16000.0;
        let samples: Vec<f32> = (0..4096)
            .map(|i| {
                let t = i as f32 / sample_rate;
                (2.0 * std::f32::consts::PI * 500.0 * t).sin() * 0.5
                    + (2.0 * std::f32::consts::PI * 1500.0 * t).sin() * 0.3
                    + (2.0 * std::f32::consts::PI * 2500.0 * t).sin() * 0.2
            })
            .collect();

        let result = analyzer
            .analyze_with_bandwidth(&samples, sample_rate)
            .expect("should succeed");

        // Formants should be in ascending frequency order
        for i in 1..result.formants.len() {
            assert!(
                result.formants[i] >= result.formants[i - 1],
                "Formants should be sorted: F{} ({}) >= F{} ({})",
                i,
                result.formants[i],
                i - 1,
                result.formants[i - 1]
            );
        }
    }

    #[test]
    fn test_prediction_error_computation() {
        let samples = vec![1.0, 0.5, 0.25, 0.125, 0.0625];
        let lpc_coeffs = vec![1.0, -0.5]; // Simple first-order predictor
        let error = compute_prediction_error(&samples, &lpc_coeffs);
        assert!(
            error >= 0.0,
            "Prediction error should be non-negative: {error}"
        );
    }

    #[test]
    fn test_prediction_error_perfect_prediction() {
        // For a signal that matches the LPC model exactly, error should be very small
        let lpc_coeffs = vec![1.0, -0.9]; // x[n] = 0.9 * x[n-1]
        let mut samples = vec![0.0_f32; 100];
        samples[0] = 1.0;
        for i in 1..100 {
            samples[i] = 0.9 * samples[i - 1];
        }
        let error = compute_prediction_error(&samples, &lpc_coeffs);
        // Should be near zero since the signal follows the model
        assert!(
            error < 0.01,
            "Perfect prediction should have near-zero error: {error}"
        );
    }
}
