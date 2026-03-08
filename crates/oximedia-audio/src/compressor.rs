//! Dynamic range compressor for audio signals.
//!
//! Implements a full-featured compressor with attack/release envelope,
//! ratio, knee, and gain reduction tracking.

/// Compression ratio modes.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KneeMode {
    /// Hard knee: abrupt transition at threshold.
    Hard,
    /// Soft knee: gradual transition around threshold.
    Soft(f32),
}

/// Dynamic range compressor configuration.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CompressorConfig {
    /// Threshold in dBFS above which compression begins.
    pub threshold_db: f32,
    /// Compression ratio (e.g. 4.0 = 4:1).
    pub ratio: f32,
    /// Attack time in seconds.
    pub attack_secs: f32,
    /// Release time in seconds.
    pub release_secs: f32,
    /// Make-up gain in dB applied after compression.
    pub makeup_gain_db: f32,
    /// Knee mode (hard or soft with width in dB).
    pub knee: KneeMode,
    /// Sample rate in Hz.
    pub sample_rate: f32,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_secs: 0.01,
            release_secs: 0.1,
            makeup_gain_db: 0.0,
            knee: KneeMode::Hard,
            sample_rate: 48_000.0,
        }
    }
}

/// Dynamic range compressor state.
#[allow(dead_code)]
pub struct Compressor {
    config: CompressorConfig,
    /// Current envelope follower level (linear).
    envelope: f32,
    /// Attack coefficient.
    attack_coeff: f32,
    /// Release coefficient.
    release_coeff: f32,
    /// Last computed gain reduction in dB (positive = reduction).
    last_gain_reduction_db: f32,
}

impl Compressor {
    /// Create a new compressor from the given configuration.
    #[allow(dead_code)]
    pub fn new(config: CompressorConfig) -> Self {
        let attack_coeff = Self::time_to_coeff(config.attack_secs, config.sample_rate);
        let release_coeff = Self::time_to_coeff(config.release_secs, config.sample_rate);
        Self {
            config,
            envelope: 0.0,
            attack_coeff,
            release_coeff,
            last_gain_reduction_db: 0.0,
        }
    }

    /// Convert a time constant (seconds) to a one-pole IIR coefficient.
    #[allow(dead_code)]
    fn time_to_coeff(time_secs: f32, sample_rate: f32) -> f32 {
        if time_secs <= 0.0 || sample_rate <= 0.0 {
            return 0.0;
        }
        (-1.0_f32 / (time_secs * sample_rate)).exp()
    }

    /// Compute gain reduction in dB for an input level in dBFS.
    #[allow(dead_code)]
    fn compute_gain_reduction_db(&self, input_db: f32) -> f32 {
        let threshold = self.config.threshold_db;
        let ratio = self.config.ratio;

        match self.config.knee {
            KneeMode::Hard => {
                if input_db <= threshold {
                    0.0
                } else {
                    (input_db - threshold) * (1.0 - 1.0 / ratio)
                }
            }
            KneeMode::Soft(knee_width) => {
                let half_knee = knee_width / 2.0;
                if input_db < threshold - half_knee {
                    0.0
                } else if input_db > threshold + half_knee {
                    (input_db - threshold) * (1.0 - 1.0 / ratio)
                } else {
                    // Smooth interpolation within knee
                    let x = input_db - (threshold - half_knee);
                    let gain = x * x / (2.0 * knee_width);
                    gain * (1.0 - 1.0 / ratio)
                }
            }
        }
    }

    /// Process a single audio sample, returning the compressed output.
    #[allow(dead_code)]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let abs_input = input.abs();

        // Envelope follower
        if abs_input > self.envelope {
            self.envelope =
                self.attack_coeff * self.envelope + (1.0 - self.attack_coeff) * abs_input;
        } else {
            self.envelope =
                self.release_coeff * self.envelope + (1.0 - self.release_coeff) * abs_input;
        }

        // Compute gain reduction
        let level_db = if self.envelope > 1e-10 {
            20.0 * self.envelope.log10()
        } else {
            -120.0
        };

        let gr_db = self.compute_gain_reduction_db(level_db);
        self.last_gain_reduction_db = gr_db;

        // Apply gain: reduction + makeup
        let total_gain_db = -gr_db + self.config.makeup_gain_db;
        let gain_linear = 10.0_f32.powf(total_gain_db / 20.0);

        input * gain_linear
    }

    /// Process a buffer of samples in-place.
    #[allow(dead_code)]
    pub fn process_buffer(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s = self.process_sample(*s);
        }
    }

    /// Returns the last computed gain reduction in dB (positive = gain reduction applied).
    #[allow(dead_code)]
    pub fn gain_reduction_db(&self) -> f32 {
        self.last_gain_reduction_db
    }

    /// Reset the compressor state.
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.envelope = 0.0;
        self.last_gain_reduction_db = 0.0;
    }

    /// Update the attack time.
    #[allow(dead_code)]
    pub fn set_attack(&mut self, attack_secs: f32) {
        self.config.attack_secs = attack_secs;
        self.attack_coeff = Self::time_to_coeff(attack_secs, self.config.sample_rate);
    }

    /// Update the release time.
    #[allow(dead_code)]
    pub fn set_release(&mut self, release_secs: f32) {
        self.config.release_secs = release_secs;
        self.release_coeff = Self::time_to_coeff(release_secs, self.config.sample_rate);
    }

    /// Update the threshold.
    #[allow(dead_code)]
    pub fn set_threshold(&mut self, threshold_db: f32) {
        self.config.threshold_db = threshold_db;
    }

    /// Update the ratio.
    #[allow(dead_code)]
    pub fn set_ratio(&mut self, ratio: f32) {
        self.config.ratio = ratio.max(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_compressor() -> Compressor {
        Compressor::new(CompressorConfig::default())
    }

    #[test]
    fn test_compressor_creation() {
        let c = make_compressor();
        assert_eq!(c.config.threshold_db, -20.0);
        assert_eq!(c.config.ratio, 4.0);
    }

    #[test]
    fn test_silence_passes_through() {
        let mut c = make_compressor();
        let out = c.process_sample(0.0);
        assert_eq!(out, 0.0);
    }

    #[test]
    fn test_envelope_rises_on_signal() {
        let mut c = make_compressor();
        for _ in 0..100 {
            c.process_sample(1.0);
        }
        assert!(c.envelope > 0.0);
    }

    #[test]
    fn test_gain_reduction_hard_knee_below_threshold() {
        let c = make_compressor();
        let gr = c.compute_gain_reduction_db(-30.0);
        assert_eq!(gr, 0.0, "no reduction below threshold");
    }

    #[test]
    fn test_gain_reduction_hard_knee_above_threshold() {
        let c = make_compressor();
        // input_db = -10, threshold = -20, ratio = 4
        // reduction = 10 * (1 - 1/4) = 7.5 dB
        let gr = c.compute_gain_reduction_db(-10.0);
        assert!((gr - 7.5).abs() < 1e-4);
    }

    #[test]
    fn test_soft_knee_within_knee() {
        let config = CompressorConfig {
            knee: KneeMode::Soft(10.0),
            ..CompressorConfig::default()
        };
        let c = Compressor::new(config);
        // At threshold (boundary), reduction should be positive
        let gr = c.compute_gain_reduction_db(-20.0);
        assert!(gr >= 0.0);
    }

    #[test]
    fn test_process_buffer_modifies_samples() {
        let mut c = make_compressor();
        let mut buf = vec![0.5_f32; 1000];
        c.process_buffer(&mut buf);
        // Samples should still be valid (not NaN/inf)
        for s in &buf {
            assert!(s.is_finite());
        }
    }

    #[test]
    fn test_reset_clears_envelope() {
        let mut c = make_compressor();
        for _ in 0..500 {
            c.process_sample(1.0);
        }
        assert!(c.envelope > 0.0);
        c.reset();
        assert_eq!(c.envelope, 0.0);
    }

    #[test]
    fn test_time_to_coeff_zero_time() {
        let coeff = Compressor::time_to_coeff(0.0, 48_000.0);
        assert_eq!(coeff, 0.0);
    }

    #[test]
    fn test_time_to_coeff_positive() {
        let coeff = Compressor::time_to_coeff(0.01, 48_000.0);
        assert!(coeff > 0.0 && coeff < 1.0);
    }

    #[test]
    fn test_set_attack_updates_coeff() {
        let mut c = make_compressor();
        let old_coeff = c.attack_coeff;
        c.set_attack(0.001);
        assert_ne!(c.attack_coeff, old_coeff);
    }

    #[test]
    fn test_set_ratio_clamps_to_one() {
        let mut c = make_compressor();
        c.set_ratio(0.5);
        assert_eq!(c.config.ratio, 1.0);
    }

    #[test]
    fn test_makeup_gain_increases_output() {
        let config = CompressorConfig {
            makeup_gain_db: 6.0,
            threshold_db: 0.0, // no compression
            ..CompressorConfig::default()
        };
        let mut c = Compressor::new(config);
        // Run many samples to let envelope stabilize
        for _ in 0..10000 {
            c.process_sample(0.1);
        }
        let out = c.process_sample(0.1);
        // 6 dB makeup ~ factor of 2
        assert!(out > 0.1);
    }

    #[test]
    fn test_gain_reduction_accessor() {
        let mut c = make_compressor();
        for _ in 0..1000 {
            c.process_sample(1.0);
        }
        // After signal above threshold, gain reduction should be positive
        let gr = c.gain_reduction_db();
        assert!(gr >= 0.0);
    }
}
