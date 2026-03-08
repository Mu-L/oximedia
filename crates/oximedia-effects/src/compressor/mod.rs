//! Dynamics compression effects.
//!
//! Provides professional compressor, limiter, and expander with industry-standard
//! gain computer and level detector designs.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

/// Compressor configuration parameters.
#[derive(Debug, Clone)]
pub struct CompressorConfig {
    /// Threshold in dB above which compression begins.
    pub threshold_db: f32,
    /// Compression ratio (e.g. 4.0 = 4:1).
    pub ratio: f32,
    /// Attack time in milliseconds.
    pub attack_ms: f32,
    /// Release time in milliseconds.
    pub release_ms: f32,
    /// Knee width in dB (0 = hard knee).
    pub knee_db: f32,
    /// Makeup gain in dB applied after compression.
    pub makeup_gain_db: f32,
}

impl CompressorConfig {
    /// Standard general-purpose compressor (4:1 ratio, moderate attack/release).
    #[must_use]
    pub fn standard() -> Self {
        Self {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 6.0,
            makeup_gain_db: 3.0,
        }
    }

    /// Limiting configuration (100:1 ratio, very fast attack).
    #[must_use]
    pub fn limiting() -> Self {
        Self {
            threshold_db: -3.0,
            ratio: 100.0,
            attack_ms: 0.1,
            release_ms: 50.0,
            knee_db: 0.0,
            makeup_gain_db: 0.0,
        }
    }

    /// Vocal compressor preset (gentle 3:1 ratio).
    #[must_use]
    pub fn vocal() -> Self {
        Self {
            threshold_db: -20.0,
            ratio: 3.0,
            attack_ms: 5.0,
            release_ms: 80.0,
            knee_db: 8.0,
            makeup_gain_db: 4.0,
        }
    }
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self::standard()
    }
}

/// Peak level detector with attack/release envelopes.
pub struct LevelDetector {
    /// Current peak level.
    pub peak_level: f32,
}

impl LevelDetector {
    /// Create a new level detector.
    #[must_use]
    pub fn new() -> Self {
        Self { peak_level: 0.0 }
    }

    /// Process a single sample and return the envelope level.
    ///
    /// # Arguments
    ///
    /// * `x` - Input sample (absolute value used)
    /// * `attack` - Attack coefficient (0..1), computed as `1 - exp(-2.2 / (attack_ms * sr / 1000))`
    /// * `release` - Release coefficient (0..1)
    pub fn process(&mut self, x: f32, attack: f32, release: f32) -> f32 {
        let input_level = x.abs();
        if input_level > self.peak_level {
            self.peak_level += attack * (input_level - self.peak_level);
        } else {
            self.peak_level += release * (input_level - self.peak_level);
        }
        self.peak_level
    }

    /// Reset the detector state.
    pub fn reset(&mut self) {
        self.peak_level = 0.0;
    }
}

impl Default for LevelDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Gain computer state that implements the compression curve.
pub struct GainComputerState {
    /// Last computed gain reduction in dB.
    pub last_gain_reduction_db: f32,
}

impl GainComputerState {
    /// Create a new gain computer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            last_gain_reduction_db: 0.0,
        }
    }

    /// Compute gain reduction in dB for the given input level.
    ///
    /// Implements soft-knee compression curve from AES guidelines.
    pub fn compute_gain(&mut self, input_db: f32, config: &CompressorConfig) -> f32 {
        let threshold = config.threshold_db;
        let ratio = config.ratio;
        let knee = config.knee_db;
        let half_knee = knee / 2.0;

        let gain_reduction_db =
            if knee > 0.0 && input_db >= threshold - half_knee && input_db <= threshold + half_knee
            {
                // Soft knee region: smooth transition
                let knee_input = input_db - threshold + half_knee;
                let knee_factor = knee_input / knee;
                // Soft knee formula: gain_reduction = (1/R - 1) * (input - threshold + knee/2)^2 / (2*knee)
                (1.0 / ratio - 1.0) * (knee_factor * knee_input) / 2.0
            } else if input_db > threshold + half_knee {
                // Above threshold: apply ratio
                (input_db - threshold) * (1.0 / ratio - 1.0)
            } else {
                // Below threshold: no gain reduction
                0.0
            };

        self.last_gain_reduction_db = gain_reduction_db;
        gain_reduction_db
    }
}

impl Default for GainComputerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Gain reduction tracking for metering.
#[derive(Debug, Clone, Default)]
pub struct GainReduction {
    /// Peak gain reduction observed in dB (positive = reduction).
    pub peak_db: f32,
    /// RMS gain reduction in dB over a measurement window.
    pub rms_db: f32,
    /// Accumulator for RMS computation.
    accumulator: f32,
    /// Sample count for RMS window.
    sample_count: u32,
    /// Window size for RMS.
    window_size: u32,
}

impl GainReduction {
    /// Create a new gain reduction tracker.
    #[must_use]
    pub fn new(window_size: u32) -> Self {
        Self {
            window_size,
            ..Default::default()
        }
    }

    /// Update with a new gain reduction value (in dB, positive = reduction).
    pub fn update(&mut self, reduction_db: f32) {
        let abs_reduction = reduction_db.abs();
        if abs_reduction > self.peak_db {
            self.peak_db = abs_reduction;
        }
        self.accumulator += abs_reduction * abs_reduction;
        self.sample_count += 1;
        if self.sample_count >= self.window_size {
            self.rms_db = (self.accumulator / self.window_size as f32).sqrt();
            self.accumulator = 0.0;
            self.sample_count = 0;
        }
    }

    /// Reset peak reading.
    pub fn reset_peak(&mut self) {
        self.peak_db = 0.0;
    }
}

/// Professional dynamics compressor.
pub struct Compressor {
    config: CompressorConfig,
    detector: LevelDetector,
    gain_computer: GainComputerState,
    /// Current gain reduction in linear.
    gain_reduction_linear: f32,
    /// Gain reduction metering.
    pub gain_reduction: GainReduction,
    /// Smoothed gain reduction in dB (for smooth ballistics).
    smoothed_gr_db: f32,
}

impl Compressor {
    /// Create a new compressor with the given configuration and sample rate.
    #[must_use]
    pub fn new(config: CompressorConfig, _sample_rate: u32) -> Self {
        Self {
            config,
            detector: LevelDetector::new(),
            gain_computer: GainComputerState::new(),
            gain_reduction_linear: 1.0,
            gain_reduction: GainReduction::new(4800),
            smoothed_gr_db: 0.0,
        }
    }

    fn db_to_linear(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    fn linear_to_db(linear: f32) -> f32 {
        20.0 * linear.max(1e-10_f32).log10()
    }

    fn attack_coeff(attack_ms: f32, sample_rate: u32) -> f32 {
        let attack_samples = attack_ms * sample_rate as f32 / 1000.0;
        if attack_samples > 0.0 {
            1.0 - (-2.2_f32 / attack_samples).exp()
        } else {
            1.0
        }
    }

    fn release_coeff(release_ms: f32, sample_rate: u32) -> f32 {
        let release_samples = release_ms * sample_rate as f32 / 1000.0;
        if release_samples > 0.0 {
            1.0 - (-2.2_f32 / release_samples).exp()
        } else {
            1.0
        }
    }

    /// Process a buffer of samples and return compressed output.
    #[must_use]
    pub fn process(&mut self, samples: &[f32], sample_rate: u32) -> Vec<f32> {
        let attack = Self::attack_coeff(self.config.attack_ms, sample_rate);
        let release = Self::release_coeff(self.config.release_ms, sample_rate);
        let makeup = Self::db_to_linear(self.config.makeup_gain_db);

        // Ballistics: smooth the gain reduction itself
        let gr_attack = Self::attack_coeff(self.config.attack_ms, sample_rate);
        let gr_release = Self::release_coeff(self.config.release_ms, sample_rate);

        samples
            .iter()
            .map(|&x| {
                // Detect level
                let level = self.detector.process(x, attack, release);
                let level_db = Self::linear_to_db(level);

                // Compute gain reduction
                let gr_db = self.gain_computer.compute_gain(level_db, &self.config);

                // Smooth gain reduction (ballistics on gain signal)
                if gr_db < self.smoothed_gr_db {
                    // Attack: gain goes down (more reduction)
                    self.smoothed_gr_db += gr_attack * (gr_db - self.smoothed_gr_db);
                } else {
                    // Release: gain comes back up
                    self.smoothed_gr_db += gr_release * (gr_db - self.smoothed_gr_db);
                }

                self.gain_reduction_linear = Self::db_to_linear(self.smoothed_gr_db);
                self.gain_reduction.update(self.smoothed_gr_db);

                x * self.gain_reduction_linear * makeup
            })
            .collect()
    }

    /// Reset compressor state.
    pub fn reset(&mut self) {
        self.detector.reset();
        self.gain_reduction_linear = 1.0;
        self.smoothed_gr_db = 0.0;
    }

    /// Get current gain reduction in dB.
    #[must_use]
    pub fn current_gain_reduction_db(&self) -> f32 {
        -self.smoothed_gr_db
    }
}

/// Below-threshold expander / gate-like processor.
///
/// Reduces signal level when below the threshold, acting as a soft gate.
pub struct Expander {
    /// Threshold below which expansion is applied (dB).
    pub threshold_db: f32,
    /// Expansion ratio (> 1 = expand).
    pub ratio: f32,
    /// Attack time in milliseconds.
    pub attack_ms: f32,
    /// Release time in milliseconds.
    pub release_ms: f32,
    /// Knee width in dB.
    pub knee_db: f32,
    detector: LevelDetector,
    smoothed_gain_db: f32,
}

impl Expander {
    /// Create a new expander.
    #[must_use]
    pub fn new(
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        knee_db: f32,
    ) -> Self {
        Self {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            detector: LevelDetector::new(),
            smoothed_gain_db: 0.0,
        }
    }

    /// Default gate-like expander preset.
    #[must_use]
    pub fn gate() -> Self {
        Self::new(-40.0, 10.0, 1.0, 50.0, 4.0)
    }

    fn db_to_linear(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    fn linear_to_db(linear: f32) -> f32 {
        20.0 * linear.max(1e-10_f32).log10()
    }

    fn attack_coeff(attack_ms: f32, sample_rate: u32) -> f32 {
        let s = attack_ms * sample_rate as f32 / 1000.0;
        if s > 0.0 {
            1.0 - (-2.2_f32 / s).exp()
        } else {
            1.0
        }
    }

    fn release_coeff(release_ms: f32, sample_rate: u32) -> f32 {
        let s = release_ms * sample_rate as f32 / 1000.0;
        if s > 0.0 {
            1.0 - (-2.2_f32 / s).exp()
        } else {
            1.0
        }
    }

    fn compute_expansion_gain(&self, input_db: f32) -> f32 {
        let threshold = self.threshold_db;
        let ratio = self.ratio;
        let half_knee = self.knee_db / 2.0;

        if input_db < threshold - half_knee {
            // Below threshold: expand downward
            (threshold - input_db) * (1.0 - ratio)
        } else if input_db <= threshold + half_knee && self.knee_db > 0.0 {
            // Soft knee region
            let knee_input = input_db - threshold + half_knee;
            (1.0 - ratio) * (knee_input - self.knee_db) * (knee_input - self.knee_db)
                / (2.0 * self.knee_db)
        } else {
            0.0
        }
    }

    /// Process a buffer of samples.
    #[must_use]
    pub fn process(&mut self, samples: &[f32], sample_rate: u32) -> Vec<f32> {
        let attack = Self::attack_coeff(self.attack_ms, sample_rate);
        let release = Self::release_coeff(self.release_ms, sample_rate);

        samples
            .iter()
            .map(|&x| {
                let level = self.detector.process(x, attack, release);
                let level_db = Self::linear_to_db(level);
                let gain_db = self.compute_expansion_gain(level_db);

                // Smooth the gain
                if gain_db < self.smoothed_gain_db {
                    self.smoothed_gain_db += attack * (gain_db - self.smoothed_gain_db);
                } else {
                    self.smoothed_gain_db += release * (gain_db - self.smoothed_gain_db);
                }

                x * Self::db_to_linear(self.smoothed_gain_db)
            })
            .collect()
    }

    /// Reset expander state.
    pub fn reset(&mut self) {
        self.detector.reset();
        self.smoothed_gain_db = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressor_config_standard() {
        let config = CompressorConfig::standard();
        assert_eq!(config.ratio, 4.0);
        assert!(config.threshold_db < 0.0);
    }

    #[test]
    fn test_compressor_config_limiting() {
        let config = CompressorConfig::limiting();
        assert_eq!(config.ratio, 100.0);
        assert!(config.attack_ms < 1.0);
    }

    #[test]
    fn test_compressor_config_vocal() {
        let config = CompressorConfig::vocal();
        assert_eq!(config.ratio, 3.0);
    }

    #[test]
    fn test_level_detector_attack() {
        let mut det = LevelDetector::new();
        // After processing a loud signal, level should be greater than 0
        for _ in 0..100 {
            det.process(1.0, 0.1, 0.01);
        }
        assert!(det.peak_level > 0.0);
    }

    #[test]
    fn test_level_detector_release() {
        let mut det = LevelDetector::new();
        det.peak_level = 1.0;
        // After processing silence, level should decrease
        for _ in 0..100 {
            det.process(0.0, 0.1, 0.1);
        }
        assert!(det.peak_level < 0.5);
    }

    #[test]
    fn test_gain_computer_below_threshold() {
        let config = CompressorConfig {
            threshold_db: -10.0,
            knee_db: 0.0,
            ..CompressorConfig::standard()
        };
        let mut computer = GainComputerState::new();
        // Signal well below threshold: no gain reduction
        let gr = computer.compute_gain(-20.0, &config);
        assert!(
            gr >= -0.001,
            "Expected no reduction below threshold, got {gr}"
        );
    }

    #[test]
    fn test_gain_computer_above_threshold() {
        let config = CompressorConfig {
            threshold_db: -10.0,
            ratio: 4.0,
            knee_db: 0.0,
            ..CompressorConfig::standard()
        };
        let mut computer = GainComputerState::new();
        // 10dB above threshold with 4:1 ratio → 7.5dB of reduction
        let gr = computer.compute_gain(0.0, &config);
        // Gain reduction should be negative (attenuation)
        assert!(
            gr < 0.0,
            "Expected gain reduction above threshold, got {gr}"
        );
    }

    #[test]
    fn test_compressor_output_finite() {
        let config = CompressorConfig::standard();
        let mut comp = Compressor::new(config, 48000);
        let input: Vec<f32> = (0..512).map(|i| (i as f32 * 0.01).sin()).collect();
        let output = comp.process(&input, 48000);
        assert_eq!(output.len(), 512);
        assert!(output.iter().all(|&s| s.is_finite()));
    }

    #[test]
    fn test_compressor_reduces_loud_signal() {
        let config = CompressorConfig {
            threshold_db: -6.0,
            ratio: 10.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            knee_db: 0.0,
            makeup_gain_db: 0.0,
        };
        let mut comp = Compressor::new(config, 48000);
        // Loud constant signal
        let input = vec![0.9f32; 1024];
        let output = comp.process(&input, 48000);

        // After settling, output should be lower than input for a loud signal
        let in_rms: f32 = (input.iter().map(|&s| s * s).sum::<f32>() / input.len() as f32).sqrt();
        let out_rms: f32 =
            (output.iter().map(|&s| s * s).sum::<f32>() / output.len() as f32).sqrt();
        assert!(out_rms < in_rms, "Compressor should reduce loud signal");
    }

    #[test]
    fn test_compressor_limiter() {
        let config = CompressorConfig::limiting();
        let mut comp = Compressor::new(config, 48000);
        // Very loud signal
        let input = vec![0.99f32; 2048];
        let output = comp.process(&input, 48000);
        assert!(output.iter().all(|&s| s.is_finite()));
    }

    #[test]
    fn test_gain_reduction_tracking() {
        let mut gr = GainReduction::new(100);
        gr.update(3.0);
        gr.update(6.0);
        assert!(gr.peak_db >= 6.0);
        gr.reset_peak();
        assert_eq!(gr.peak_db, 0.0);
    }

    #[test]
    fn test_expander_output_finite() {
        let mut exp = Expander::gate();
        let input: Vec<f32> = (0..512).map(|i| (i as f32 * 0.01).sin() * 0.1).collect();
        let output = exp.process(&input, 48000);
        assert_eq!(output.len(), 512);
        assert!(output.iter().all(|&s| s.is_finite()));
    }

    #[test]
    fn test_expander_attenuates_below_threshold() {
        let mut exp = Expander::new(-10.0, 5.0, 1.0, 50.0, 0.0);
        // Quiet signal below threshold
        let input = vec![0.001f32; 1024];
        let output = exp.process(&input, 48000);
        let in_rms: f32 = (input.iter().map(|&s| s * s).sum::<f32>() / input.len() as f32).sqrt();
        let out_rms: f32 =
            (output.iter().map(|&s| s * s).sum::<f32>() / output.len() as f32).sqrt();
        assert!(
            out_rms <= in_rms + 1e-6,
            "Expander should attenuate or not increase quiet signals"
        );
    }

    #[test]
    fn test_compressor_reset() {
        let config = CompressorConfig::standard();
        let mut comp = Compressor::new(config, 48000);
        let _ = comp.process(&vec![0.9f32; 512], 48000);
        comp.reset();
        assert_eq!(comp.smoothed_gr_db, 0.0);
    }
}
