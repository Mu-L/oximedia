//! Hum removal using notch filters.

use crate::error::RestoreResult;
use crate::hum::detector::HumFrequencies;

/// Notch filter for hum removal.
#[derive(Debug, Clone)]
pub struct NotchFilter {
    frequency: f32,
    sample_rate: u32,
    q_factor: f32,
    // Biquad filter state
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl NotchFilter {
    /// Create a new notch filter.
    ///
    /// # Arguments
    ///
    /// * `frequency` - Center frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    /// * `q_factor` - Quality factor (higher = narrower notch)
    #[must_use]
    pub fn new(frequency: f32, sample_rate: u32, q_factor: f32) -> Self {
        let mut filter = Self {
            frequency,
            sample_rate,
            q_factor,
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        };

        filter.compute_coefficients();
        filter
    }

    /// Compute biquad filter coefficients.
    fn compute_coefficients(&mut self) {
        use std::f32::consts::PI;

        #[allow(clippy::cast_precision_loss)]
        let omega = 2.0 * PI * self.frequency / self.sample_rate as f32;
        let alpha = omega.sin() / (2.0 * self.q_factor);
        let cos_omega = omega.cos();

        // Notch filter coefficients
        let a0 = 1.0 + alpha;
        self.b0 = 1.0 / a0;
        self.b1 = -2.0 * cos_omega / a0;
        self.b2 = 1.0 / a0;
        self.a1 = -2.0 * cos_omega / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    /// Process a single sample.
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }

    /// Process samples.
    pub fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        samples.iter().map(|&s| self.process_sample(s)).collect()
    }

    /// Reset filter state.
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// Hum remover using cascaded notch filters.
#[derive(Debug, Clone)]
pub struct HumRemover {
    filters: Vec<NotchFilter>,
}

impl HumRemover {
    /// Create a new hum remover.
    ///
    /// # Arguments
    ///
    /// * `hum` - Detected hum frequencies
    /// * `sample_rate` - Sample rate in Hz
    /// * `q_factor` - Quality factor for notch filters
    #[must_use]
    pub fn new(hum: &HumFrequencies, sample_rate: u32, q_factor: f32) -> Self {
        let mut filters = Vec::new();

        // Add filter for fundamental
        filters.push(NotchFilter::new(hum.fundamental, sample_rate, q_factor));

        // Add filters for harmonics
        for &harmonic in &hum.harmonics {
            filters.push(NotchFilter::new(harmonic, sample_rate, q_factor));
        }

        Self { filters }
    }

    /// Create hum remover for standard frequencies.
    ///
    /// # Arguments
    ///
    /// * `fundamental` - Fundamental frequency (50 or 60 Hz)
    /// * `sample_rate` - Sample rate in Hz
    /// * `num_harmonics` - Number of harmonics to remove
    /// * `q_factor` - Quality factor for notch filters
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn new_standard(
        fundamental: f32,
        sample_rate: u32,
        num_harmonics: usize,
        q_factor: f32,
    ) -> Self {
        let mut filters = Vec::new();

        for n in 1..=num_harmonics {
            let freq = fundamental * n as f32;
            filters.push(NotchFilter::new(freq, sample_rate, q_factor));
        }

        Self { filters }
    }

    /// Process samples to remove hum.
    pub fn process(&mut self, samples: &[f32]) -> RestoreResult<Vec<f32>> {
        let mut output = samples.to_vec();

        // Apply each notch filter in cascade
        for filter in &mut self.filters {
            output = filter.process(&output);
        }

        Ok(output)
    }

    /// Reset all filter states.
    pub fn reset(&mut self) {
        for filter in &mut self.filters {
            filter.reset();
        }
    }
}

/// Comb filter for harmonic hum removal.
#[derive(Debug, Clone)]
pub struct CombFilter {
    #[allow(dead_code)]
    fundamental: f32,
    #[allow(dead_code)]
    sample_rate: u32,
    depth: f32,
    buffer: Vec<f32>,
    write_pos: usize,
}

impl CombFilter {
    /// Create a new comb filter.
    ///
    /// # Arguments
    ///
    /// * `fundamental` - Fundamental frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    /// * `depth` - Filter depth (0.0 to 1.0)
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn new(fundamental: f32, sample_rate: u32, depth: f32) -> Self {
        let delay_samples = (sample_rate as f32 / fundamental) as usize;
        let buffer = vec![0.0; delay_samples];

        Self {
            fundamental,
            sample_rate,
            depth: depth.clamp(0.0, 1.0),
            buffer,
            write_pos: 0,
        }
    }

    /// Process a single sample.
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.write_pos];
        let output = input - self.depth * delayed;

        self.buffer[self.write_pos] = input;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();

        output
    }

    /// Process samples.
    pub fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        samples.iter().map(|&s| self.process_sample(s)).collect()
    }

    /// Reset filter state.
    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notch_filter() {
        let mut filter = NotchFilter::new(50.0, 44100, 10.0);

        let samples = vec![1.0; 100];
        let output = filter.process(&samples);

        assert_eq!(output.len(), samples.len());
    }

    #[test]
    fn test_hum_remover() {
        use crate::hum::detector::HumFrequencies;

        let hum = HumFrequencies {
            fundamental: 50.0,
            harmonics: vec![100.0, 150.0],
            confidence: 0.9,
        };

        let mut remover = HumRemover::new(&hum, 44100, 10.0);

        // Create test signal
        use std::f32::consts::PI;
        let samples: Vec<f32> = (0..1000)
            .map(|i| {
                let t = i as f32 / 44100.0;
                (2.0 * PI * 50.0 * t).sin()
            })
            .collect();

        let output = remover.process(&samples).expect("should succeed in test");
        assert_eq!(output.len(), samples.len());
    }

    #[test]
    fn test_hum_remover_standard() {
        let mut remover = HumRemover::new_standard(60.0, 44100, 5, 10.0);

        let samples = vec![0.0; 100];
        let output = remover.process(&samples).expect("should succeed in test");
        assert_eq!(output.len(), samples.len());
    }

    #[test]
    fn test_comb_filter() {
        let mut filter = CombFilter::new(50.0, 44100, 0.5);

        let samples = vec![1.0; 1000];
        let output = filter.process(&samples);

        assert_eq!(output.len(), samples.len());
    }

    #[test]
    fn test_reset() {
        let mut filter = NotchFilter::new(50.0, 44100, 10.0);
        let samples = vec![1.0; 100];
        let _ = filter.process(&samples);

        filter.reset();
        assert_eq!(filter.x1, 0.0);
        assert_eq!(filter.y1, 0.0);
    }
}
