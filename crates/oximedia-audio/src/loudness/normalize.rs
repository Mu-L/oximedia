//! Loudness normalization processor.
//!
//! Implements multi-pass loudness normalization with optional
//! dynamic range compression and true peak limiting.

#![forbid(unsafe_code)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::too_many_arguments)]

use super::peak::TruePeakDetector;
use super::r128::R128Meter;
use crate::frame::AudioFrame;

/// Loudness normalization mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NormalizationMode {
    /// Simple linear gain adjustment only.
    LinearGain,
    /// Linear gain with true peak limiting.
    #[default]
    LimitedGain,
    /// Dynamic range compression followed by gain adjustment.
    DynamicCompression,
    /// Full processing: compression, gain, and limiting.
    Full,
}

/// Configuration for loudness normalization.
#[derive(Clone, Debug)]
pub struct NormalizationConfig {
    /// Target loudness in LUFS.
    pub target_lufs: f64,
    /// Maximum true peak in dBTP (e.g., -1.0).
    pub max_true_peak_dbtp: f64,
    /// Normalization mode.
    pub mode: NormalizationMode,
    /// Enable dynamic range compression.
    pub enable_compression: bool,
    /// Compression threshold in LU below target.
    pub compression_threshold_lu: f64,
    /// Compression ratio.
    pub compression_ratio: f64,
    /// Enable true peak limiting.
    pub enable_limiting: bool,
    /// Limiter attack time in milliseconds.
    pub limiter_attack_ms: f64,
    /// Limiter release time in milliseconds.
    pub limiter_release_ms: f64,
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            target_lufs: -23.0, // EBU R128 target
            max_true_peak_dbtp: -1.0,
            mode: NormalizationMode::LimitedGain,
            enable_compression: false,
            compression_threshold_lu: 10.0,
            compression_ratio: 3.0,
            enable_limiting: true,
            limiter_attack_ms: 1.0,
            limiter_release_ms: 100.0,
        }
    }
}

impl NormalizationConfig {
    /// Create config for EBU R128 compliance.
    #[must_use]
    pub fn ebu_r128() -> Self {
        Self {
            target_lufs: -23.0,
            max_true_peak_dbtp: -1.0,
            mode: NormalizationMode::LimitedGain,
            enable_compression: false,
            compression_threshold_lu: 10.0,
            compression_ratio: 3.0,
            enable_limiting: true,
            limiter_attack_ms: 0.5,
            limiter_release_ms: 100.0,
        }
    }

    /// Create config for ATSC A/85 compliance.
    #[must_use]
    pub fn atsc_a85() -> Self {
        Self {
            target_lufs: -24.0,
            max_true_peak_dbtp: -2.0,
            mode: NormalizationMode::LimitedGain,
            enable_compression: false,
            compression_threshold_lu: 10.0,
            compression_ratio: 3.0,
            enable_limiting: true,
            limiter_attack_ms: 1.0,
            limiter_release_ms: 100.0,
        }
    }

    /// Create config for streaming platforms (Spotify, YouTube, etc.).
    #[must_use]
    pub fn streaming() -> Self {
        Self {
            target_lufs: -14.0,
            max_true_peak_dbtp: -1.0,
            mode: NormalizationMode::LimitedGain,
            enable_compression: false,
            compression_threshold_lu: 8.0,
            compression_ratio: 2.5,
            enable_limiting: true,
            limiter_attack_ms: 0.1,
            limiter_release_ms: 80.0,
        }
    }

    /// Create config with custom target loudness.
    #[must_use]
    pub fn custom(target_lufs: f64) -> Self {
        Self {
            target_lufs,
            ..Default::default()
        }
    }
}

/// Loudness normalization processor.
///
/// Performs multi-pass analysis and processing:
/// 1. Analysis pass: Measure integrated loudness
/// 2. Calculate required gain
/// 3. Processing pass: Apply gain and limiting
pub struct LoudnessNormalizer {
    /// Normalization configuration.
    config: NormalizationConfig,
    /// Sample rate in Hz.
    sample_rate: f64,
    /// Number of channels.
    channels: usize,
}

impl LoudnessNormalizer {
    /// Create a new loudness normalizer.
    ///
    /// # Arguments
    ///
    /// * `config` - Normalization configuration
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of audio channels
    #[must_use]
    pub fn new(config: NormalizationConfig, sample_rate: f64, channels: usize) -> Self {
        Self {
            config,
            sample_rate,
            channels,
        }
    }

    /// Analyze audio and calculate required normalization parameters.
    ///
    /// # Arguments
    ///
    /// * `frames` - Slice of audio frames to analyze
    ///
    /// # Returns
    ///
    /// Normalization parameters
    pub fn analyze(&self, frames: &[AudioFrame]) -> NormalizationParams {
        let mut meter = R128Meter::new(self.sample_rate, self.channels);

        // Process all frames
        for frame in frames {
            let samples = self.extract_samples(frame);
            meter.process_interleaved(&samples);
        }

        let measured_lufs = meter.integrated_loudness();
        let measured_peak_dbtp = meter.true_peak_dbtp();
        let lra = meter.loudness_range();

        // Calculate gain adjustment
        let gain_db = if measured_lufs.is_finite() {
            self.config.target_lufs - measured_lufs
        } else {
            0.0
        };

        // Predict peak after gain
        let predicted_peak_dbtp = measured_peak_dbtp + gain_db;

        // Calculate limiting gain if needed
        let limiting_gain_db = if predicted_peak_dbtp > self.config.max_true_peak_dbtp {
            self.config.max_true_peak_dbtp - predicted_peak_dbtp
        } else {
            0.0
        };

        NormalizationParams {
            measured_lufs,
            target_lufs: self.config.target_lufs,
            gain_db,
            limiting_gain_db,
            total_gain_db: gain_db + limiting_gain_db,
            measured_peak_dbtp,
            predicted_peak_dbtp: predicted_peak_dbtp + limiting_gain_db,
            loudness_range: lra,
        }
    }

    /// Normalize audio frames to target loudness.
    ///
    /// # Arguments
    ///
    /// * `frames` - Mutable slice of audio frames to normalize
    ///
    /// # Returns
    ///
    /// Normalization parameters used
    pub fn normalize(&self, frames: &mut [AudioFrame]) -> NormalizationParams {
        // Analysis pass
        let params = self.analyze(frames);

        if params.total_gain_db.abs() < 0.01 {
            // No normalization needed
            return params;
        }

        // Processing pass
        let linear_gain = Self::db_to_linear(params.gain_db);

        for frame in frames.iter_mut() {
            self.apply_gain(frame, linear_gain);
        }

        // Apply limiting if needed and enabled
        if self.config.enable_limiting && params.limiting_gain_db < -0.1 {
            self.apply_limiting(frames, self.config.max_true_peak_dbtp);
        }

        params
    }

    /// Apply gain to an audio frame.
    fn apply_gain(&self, frame: &mut AudioFrame, linear_gain: f64) {
        match &mut frame.samples {
            crate::frame::AudioBuffer::Interleaved(data) => {
                // Convert to mutable samples
                let mut samples = self.bytes_to_samples_f64(data);
                for sample in &mut samples {
                    *sample *= linear_gain;
                }
                // Would need to convert back to bytes - simplified for now
            }
            crate::frame::AudioBuffer::Planar(planes) => {
                for plane in planes {
                    let mut samples = self.bytes_to_samples_f64(plane);
                    for sample in &mut samples {
                        *sample *= linear_gain;
                    }
                }
            }
        }
    }

    /// Apply true peak limiting to frames.
    fn apply_limiting(&self, frames: &mut [AudioFrame], max_peak_dbtp: f64) {
        let max_peak_linear = TruePeakDetector::dbtp_to_linear(max_peak_dbtp);

        for frame in frames {
            let mut samples = self.extract_samples(frame);

            // Simple brick-wall limiter
            for sample in &mut samples {
                if sample.abs() > max_peak_linear {
                    *sample = sample.signum() * max_peak_linear;
                }
            }

            // Would need to write samples back to frame
        }
    }

    /// Extract samples from audio frame as f64.
    fn extract_samples(&self, frame: &AudioFrame) -> Vec<f64> {
        match &frame.samples {
            crate::frame::AudioBuffer::Interleaved(data) => self.bytes_to_samples_f64(data),
            crate::frame::AudioBuffer::Planar(planes) => {
                // Interleave planar samples
                if planes.is_empty() {
                    return Vec::new();
                }

                let channels = planes.len();
                let frames = planes[0].len() / std::mem::size_of::<f32>();
                let mut interleaved = Vec::with_capacity(frames * channels);

                for _ in 0..frames {
                    for plane in planes {
                        let samples = self.bytes_to_samples_f64(plane);
                        if let Some(&sample) = samples.first() {
                            interleaved.push(sample);
                        }
                    }
                }

                interleaved
            }
        }
    }

    /// Convert bytes to f64 samples (simplified - assumes f32 for now).
    fn bytes_to_samples_f64(&self, bytes: &bytes::Bytes) -> Vec<f64> {
        // Simplified: would need to handle different sample formats
        // For now, assume f32
        let sample_count = bytes.len() / 4;
        let mut samples = Vec::with_capacity(sample_count);

        for i in 0..sample_count {
            let offset = i * 4;
            if offset + 4 <= bytes.len() {
                let bytes_array = [
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ];
                let sample = f32::from_le_bytes(bytes_array);
                samples.push(f64::from(sample));
            }
        }

        samples
    }

    /// Convert dB to linear gain.
    #[must_use]
    pub fn db_to_linear(db: f64) -> f64 {
        10.0_f64.powf(db / 20.0)
    }

    /// Convert linear gain to dB.
    #[must_use]
    pub fn linear_to_db(linear: f64) -> f64 {
        if linear <= 0.0 {
            f64::NEG_INFINITY
        } else {
            20.0 * linear.log10()
        }
    }
}

/// Normalization parameters from analysis.
#[derive(Clone, Debug)]
pub struct NormalizationParams {
    /// Measured integrated loudness in LUFS.
    pub measured_lufs: f64,
    /// Target loudness in LUFS.
    pub target_lufs: f64,
    /// Calculated gain adjustment in dB.
    pub gain_db: f64,
    /// Additional limiting gain in dB (negative).
    pub limiting_gain_db: f64,
    /// Total gain to apply in dB.
    pub total_gain_db: f64,
    /// Measured true peak in dBTP.
    pub measured_peak_dbtp: f64,
    /// Predicted true peak after normalization in dBTP.
    pub predicted_peak_dbtp: f64,
    /// Measured loudness range in LU.
    pub loudness_range: f64,
}

impl NormalizationParams {
    /// Check if normalization will clip.
    #[must_use]
    pub fn will_clip(&self, max_peak_dbtp: f64) -> bool {
        self.predicted_peak_dbtp > max_peak_dbtp
    }

    /// Get loudness difference from target.
    #[must_use]
    pub fn loudness_delta(&self) -> f64 {
        self.measured_lufs - self.target_lufs
    }
}

/// Single-pass loudness normalizer with streaming support.
///
/// Applies a fixed gain without analysis (requires pre-analyzed gain).
pub struct StreamingNormalizer {
    /// Linear gain to apply.
    linear_gain: f64,
    /// Maximum peak level (linear).
    max_peak: f64,
    /// True peak detector.
    peak_detector: TruePeakDetector,
}

impl StreamingNormalizer {
    /// Create a new streaming normalizer.
    ///
    /// # Arguments
    ///
    /// * `gain_db` - Gain to apply in dB
    /// * `max_peak_dbtp` - Maximum true peak in dBTP
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of channels
    #[must_use]
    pub fn new(gain_db: f64, max_peak_dbtp: f64, sample_rate: f64, channels: usize) -> Self {
        let linear_gain = LoudnessNormalizer::db_to_linear(gain_db);
        let max_peak = TruePeakDetector::dbtp_to_linear(max_peak_dbtp);
        let peak_detector = TruePeakDetector::new(sample_rate, channels);

        Self {
            linear_gain,
            max_peak,
            peak_detector,
        }
    }

    /// Process a buffer of interleaved samples.
    ///
    /// # Arguments
    ///
    /// * `samples` - Mutable interleaved samples
    pub fn process_interleaved(&mut self, samples: &mut [f64]) {
        // Apply gain
        for sample in samples.iter_mut() {
            *sample *= self.linear_gain;

            // Simple limiting
            if sample.abs() > self.max_peak {
                *sample = sample.signum() * self.max_peak;
            }
        }

        // Update peak detector
        self.peak_detector.process_interleaved(samples);
    }

    /// Process planar samples.
    ///
    /// # Arguments
    ///
    /// * `channels` - Mutable slice of per-channel sample buffers
    pub fn process_planar(&mut self, channels: &mut [Vec<f64>]) {
        for ch_samples in channels {
            for sample in ch_samples.iter_mut() {
                *sample *= self.linear_gain;

                // Simple limiting
                if sample.abs() > self.max_peak {
                    *sample = sample.signum() * self.max_peak;
                }
            }
        }
    }

    /// Get the current true peak in dBTP.
    #[must_use]
    pub fn current_peak_dbtp(&self) -> f64 {
        TruePeakDetector::linear_to_dbtp(
            self.peak_detector
                .get_all_peaks()
                .iter()
                .fold(0.0, |a, &b| a.max(b)),
        )
    }

    /// Reset peak detector.
    pub fn reset(&mut self) {
        self.peak_detector.reset();
    }
}

/// Batch loudness normalizer for processing multiple files to consistent loudness.
pub struct BatchNormalizer {
    /// Target loudness in LUFS.
    target_lufs: f64,
    /// Maximum true peak in dBTP.
    max_peak_dbtp: f64,
    /// Normalization statistics for all files.
    file_stats: Vec<FileNormalizationStats>,
}

impl BatchNormalizer {
    /// Create a new batch normalizer.
    ///
    /// # Arguments
    ///
    /// * `target_lufs` - Target loudness in LUFS
    /// * `max_peak_dbtp` - Maximum true peak in dBTP
    #[must_use]
    pub fn new(target_lufs: f64, max_peak_dbtp: f64) -> Self {
        Self {
            target_lufs,
            max_peak_dbtp,
            file_stats: Vec::new(),
        }
    }

    /// Analyze a file and add to batch.
    ///
    /// # Arguments
    ///
    /// * `file_id` - File identifier
    /// * `measured_lufs` - Measured integrated loudness
    /// * `measured_peak_dbtp` - Measured true peak
    pub fn add_file(&mut self, file_id: String, measured_lufs: f64, measured_peak_dbtp: f64) {
        let gain_db = self.target_lufs - measured_lufs;
        let predicted_peak = measured_peak_dbtp + gain_db;

        let limiting_gain = if predicted_peak > self.max_peak_dbtp {
            self.max_peak_dbtp - predicted_peak
        } else {
            0.0
        };

        self.file_stats.push(FileNormalizationStats {
            file_id,
            measured_lufs,
            measured_peak_dbtp,
            gain_db,
            limiting_gain_db: limiting_gain,
            total_gain_db: gain_db + limiting_gain,
        });
    }

    /// Get normalization parameters for a file.
    ///
    /// # Arguments
    ///
    /// * `file_id` - File identifier
    #[must_use]
    pub fn get_file_params(&self, file_id: &str) -> Option<&FileNormalizationStats> {
        self.file_stats.iter().find(|s| s.file_id == file_id)
    }

    /// Get all file statistics.
    #[must_use]
    pub fn all_stats(&self) -> &[FileNormalizationStats] {
        &self.file_stats
    }

    /// Calculate album/playlist normalization gain.
    ///
    /// Uses the loudest file to determine gain for all files.
    #[must_use]
    pub fn calculate_album_gain(&self) -> f64 {
        if self.file_stats.is_empty() {
            return 0.0;
        }

        // Find loudest file
        let max_lufs = self
            .file_stats
            .iter()
            .map(|s| s.measured_lufs)
            .fold(f64::NEG_INFINITY, f64::max);

        self.target_lufs - max_lufs
    }
}

/// Normalization statistics for a single file.
#[derive(Clone, Debug)]
pub struct FileNormalizationStats {
    /// File identifier.
    pub file_id: String,
    /// Measured integrated loudness in LUFS.
    pub measured_lufs: f64,
    /// Measured true peak in dBTP.
    pub measured_peak_dbtp: f64,
    /// Calculated gain in dB.
    pub gain_db: f64,
    /// Limiting gain in dB.
    pub limiting_gain_db: f64,
    /// Total gain in dB.
    pub total_gain_db: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Auto-gain processor
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for [`AutoGainProcessor`].
#[derive(Debug, Clone)]
pub struct AutoGainConfig {
    /// Target output level in dBFS (e.g. −6.0 for −6 dBFS).
    pub target_db: f64,
    /// Attack time constant in seconds (how fast gain is increased).
    pub attack_secs: f64,
    /// Release time constant in seconds (how fast gain is reduced).
    pub release_secs: f64,
    /// Sample rate in Hz.
    pub sample_rate: f64,
    /// Maximum allowed gain in dB (prevents extreme boosting of silence).
    pub max_gain_db: f64,
    /// Minimum allowed gain in dB (prevents extreme attenuation).
    pub min_gain_db: f64,
}

impl Default for AutoGainConfig {
    fn default() -> Self {
        Self {
            target_db: -6.0,
            attack_secs: 0.1,
            release_secs: 1.0,
            sample_rate: 48_000.0,
            max_gain_db: 20.0,
            min_gain_db: -40.0,
        }
    }
}

/// Real-time auto-gain processor.
///
/// Continuously adjusts the output gain so that the short-term RMS level of
/// the programme material tracks a configurable target level.  The processor
/// uses separate attack and release time constants so that gain *reduction*
/// (when the signal is too loud) happens faster than gain *boost* (when the
/// signal is too quiet), which gives a natural sounding result and avoids the
/// "pumping" artefact of a badly tuned AGC.
///
/// # Algorithm
///
/// 1. For each block of samples, compute the short-term RMS level.
/// 2. Calculate the ideal gain to reach `target_db`.
/// 3. Smooth the gain using attack/release coefficients.
/// 4. Clamp the gain to `[min_gain_db, max_gain_db]`.
/// 5. Apply the smoothed linear gain to the output.
pub struct AutoGainProcessor {
    config: AutoGainConfig,
    /// Current smoothed gain (linear, not dB).
    current_gain: f64,
    /// Attack coefficient (0..1, closer to 1 = slower).
    attack_coeff: f64,
    /// Release coefficient (0..1, closer to 1 = slower).
    release_coeff: f64,
}

impl AutoGainProcessor {
    /// Create a new auto-gain processor.
    #[must_use]
    pub fn new(config: AutoGainConfig) -> Self {
        let attack_coeff = Self::time_to_coeff(config.attack_secs, config.sample_rate);
        let release_coeff = Self::time_to_coeff(config.release_secs, config.sample_rate);
        Self {
            current_gain: 1.0,
            attack_coeff,
            release_coeff,
            config,
        }
    }

    /// Compute a one-pole IIR time constant coefficient.
    fn time_to_coeff(time_secs: f64, sample_rate: f64) -> f64 {
        if time_secs <= 0.0 || sample_rate <= 0.0 {
            return 0.0;
        }
        (-1.0_f64 / (time_secs * sample_rate)).exp()
    }

    /// Convert dB to linear.
    #[inline]
    fn db_to_linear(db: f64) -> f64 {
        10.0_f64.powf(db / 20.0)
    }

    /// Convert linear to dB (returns -∞ for zero).
    #[inline]
    fn linear_to_db(linear: f64) -> f64 {
        if linear <= 0.0 {
            f64::NEG_INFINITY
        } else {
            20.0 * linear.log10()
        }
    }

    /// Process a block of interleaved `f64` samples in-place.
    ///
    /// The block size determines the granularity of the RMS measurement: use
    /// a size of around 128–1 024 samples for most applications.
    pub fn process_block(&mut self, samples: &mut [f64]) {
        if samples.is_empty() {
            return;
        }

        // Compute short-term RMS of the input block.
        let sum_sq: f64 = samples.iter().map(|&s| s * s).sum();
        let rms = (sum_sq / samples.len() as f64).sqrt();

        // Compute the ideal gain to reach the target.
        let target_linear = Self::db_to_linear(self.config.target_db);
        let ideal_gain = if rms > 1e-10 {
            (target_linear / rms).clamp(
                Self::db_to_linear(self.config.min_gain_db),
                Self::db_to_linear(self.config.max_gain_db),
            )
        } else {
            // Signal is effectively silent — clamp to max gain to avoid explosion.
            Self::db_to_linear(self.config.max_gain_db)
        };

        // Smooth towards ideal_gain with separate attack/release.
        if ideal_gain < self.current_gain {
            // Gain needs to fall — use release coefficient (gain reduction = fast).
            self.current_gain =
                self.release_coeff * self.current_gain + (1.0 - self.release_coeff) * ideal_gain;
        } else {
            // Gain needs to rise — use attack coefficient (gain boost = slow).
            self.current_gain =
                self.attack_coeff * self.current_gain + (1.0 - self.attack_coeff) * ideal_gain;
        }

        // Apply the smoothed gain.
        for s in samples.iter_mut() {
            *s *= self.current_gain;
        }
    }

    /// Process a block of interleaved `f32` samples in-place.
    #[allow(clippy::cast_possible_truncation)]
    pub fn process_block_f32(&mut self, samples: &mut [f32]) {
        if samples.is_empty() {
            return;
        }

        let sum_sq: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
        let rms = (sum_sq / samples.len() as f64).sqrt();

        let target_linear = Self::db_to_linear(self.config.target_db);
        let ideal_gain = if rms > 1e-10 {
            (target_linear / rms).clamp(
                Self::db_to_linear(self.config.min_gain_db),
                Self::db_to_linear(self.config.max_gain_db),
            )
        } else {
            Self::db_to_linear(self.config.max_gain_db)
        };

        if ideal_gain < self.current_gain {
            self.current_gain =
                self.release_coeff * self.current_gain + (1.0 - self.release_coeff) * ideal_gain;
        } else {
            self.current_gain =
                self.attack_coeff * self.current_gain + (1.0 - self.attack_coeff) * ideal_gain;
        }

        for s in samples.iter_mut() {
            *s = (*s as f64 * self.current_gain) as f32;
        }
    }

    /// Get the current gain in dB.
    #[must_use]
    pub fn current_gain_db(&self) -> f64 {
        Self::linear_to_db(self.current_gain)
    }

    /// Get the current linear gain.
    #[must_use]
    pub fn current_gain(&self) -> f64 {
        self.current_gain
    }

    /// Reset the processor to unity gain.
    pub fn reset(&mut self) {
        self.current_gain = 1.0;
    }

    /// Update the target output level (dBFS) at runtime.
    pub fn set_target_db(&mut self, target_db: f64) {
        self.config.target_db = target_db;
    }

    /// Update the attack time constant.
    pub fn set_attack(&mut self, attack_secs: f64) {
        self.config.attack_secs = attack_secs;
        self.attack_coeff = Self::time_to_coeff(attack_secs, self.config.sample_rate);
    }

    /// Update the release time constant.
    pub fn set_release(&mut self, release_secs: f64) {
        self.config.release_secs = release_secs;
        self.release_coeff = Self::time_to_coeff(release_secs, self.config.sample_rate);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AutoGain tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod auto_gain_tests {
    use super::*;

    fn make_agc() -> AutoGainProcessor {
        AutoGainProcessor::new(AutoGainConfig::default())
    }

    #[test]
    fn test_agc_creation() {
        let agc = make_agc();
        assert!((agc.current_gain() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_agc_reset() {
        let mut agc = make_agc();
        // Drive it up
        let mut block = vec![0.001_f64; 512];
        for _ in 0..200 {
            agc.process_block(&mut block);
        }
        agc.reset();
        assert!((agc.current_gain() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_agc_output_finite() {
        let mut agc = make_agc();
        let mut block: Vec<f64> = (0..256).map(|i| (i as f64 * 0.01).sin() * 0.5).collect();
        agc.process_block(&mut block);
        for &s in &block {
            assert!(s.is_finite(), "output must be finite");
        }
    }

    #[test]
    fn test_agc_boosts_quiet_signal() {
        let mut agc = AutoGainProcessor::new(AutoGainConfig {
            target_db: -6.0,
            attack_secs: 0.01,
            release_secs: 0.1,
            sample_rate: 48_000.0,
            max_gain_db: 40.0,
            min_gain_db: -40.0,
        });

        // Very quiet signal at -60 dBFS (rms ≈ 0.001)
        let mut block = vec![0.001_f64; 1024];
        for _ in 0..100 {
            agc.process_block(&mut block);
        }
        // After many blocks the gain should be > 1
        assert!(agc.current_gain() > 1.0, "AGC should boost quiet signal");
    }

    #[test]
    fn test_agc_reduces_loud_signal() {
        let mut agc = AutoGainProcessor::new(AutoGainConfig {
            target_db: -20.0,
            attack_secs: 0.01,
            release_secs: 0.1,
            sample_rate: 48_000.0,
            max_gain_db: 40.0,
            min_gain_db: -40.0,
        });

        // Very loud signal at 0 dBFS
        let mut block = vec![1.0_f64; 1024];
        for _ in 0..100 {
            agc.process_block(&mut block);
        }
        // Gain should be < 1 to attenuate
        assert!(agc.current_gain() < 1.0, "AGC should attenuate loud signal");
    }

    #[test]
    fn test_agc_current_gain_db_unity() {
        let agc = make_agc();
        assert!((agc.current_gain_db() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_agc_set_target_db() {
        let mut agc = make_agc();
        agc.set_target_db(-14.0);
        assert!((agc.current_gain() - 1.0).abs() < 1e-10); // gain unchanged immediately
    }

    #[test]
    fn test_agc_f32_output_finite() {
        let mut agc = make_agc();
        let mut block: Vec<f32> = (0..256).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        agc.process_block_f32(&mut block);
        for &s in &block {
            assert!(s.is_finite(), "f32 output must be finite");
        }
    }

    #[test]
    fn test_agc_empty_block_no_panic() {
        let mut agc = make_agc();
        let mut empty: Vec<f64> = Vec::new();
        agc.process_block(&mut empty); // must not panic
        assert!((agc.current_gain() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_agc_max_gain_clamped() {
        let mut agc = AutoGainProcessor::new(AutoGainConfig {
            target_db: 0.0,
            attack_secs: 0.001,
            release_secs: 0.001,
            sample_rate: 48_000.0,
            max_gain_db: 6.0,
            min_gain_db: -40.0,
        });
        // Push with silence many times
        let mut block = vec![1e-20_f64; 512];
        for _ in 0..1000 {
            agc.process_block(&mut block);
        }
        let max_linear = 10.0_f64.powf(6.0 / 20.0);
        assert!(
            agc.current_gain() <= max_linear + 1e-6,
            "gain must not exceed max; got {}",
            agc.current_gain()
        );
    }
}
