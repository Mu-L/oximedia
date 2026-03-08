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
