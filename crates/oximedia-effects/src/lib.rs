//! Professional audio effects suite for `OxiMedia`.
//!
//! This crate provides production-quality implementations of professional audio effects
//! used in music production, post-production, and broadcast applications.
//!
//! # Effect Categories
//!
//! ## Reverb
//! - **Freeverb** - Algorithmic reverb based on Schroeder reverb architecture
//! - **Plate Reverb** - Simulation of mechanical plate reverb
//! - **Convolution Reverb** - Impulse response-based reverb for realistic spaces
//!
//! ## Delay/Echo
//! - **Delay** - Simple delay with feedback
//! - **Multi-tap Delay** - Multiple delay taps with independent controls
//! - **Ping-pong Delay** - Stereo ping-pong delay effect
//!
//! ## Modulation
//! - **Chorus** - Multi-voice chorus effect
//! - **Flanger** - Flanging with feedback
//! - **Phaser** - All-pass filter cascade phasing
//! - **Tremolo** - Amplitude modulation
//! - **Vibrato** - Frequency modulation
//! - **Ring Modulator** - Ring modulation effect
//!
//! ## Distortion
//! - **Overdrive** - Soft clipping overdrive
//! - **Fuzz** - Hard clipping fuzz distortion
//! - **Bit Crusher** - Bit depth and sample rate reduction
//!
//! ## Dynamics
//! - **Gate** - Noise gate with threshold and hysteresis
//! - **Expander** - Upward and downward expansion
//!
//! ## Filters
//! - **Biquad** - Second-order IIR filters (low-pass, high-pass, band-pass, notch, shelving)
//! - **State Variable Filter** - Multi-mode state-variable filter
//! - **Moog Ladder** - Classic Moog ladder filter simulation
//!
//! ## Pitch/Time
//! - **Pitch Shifter** - Time-domain and frequency-domain pitch shifting
//! - **Time Stretch** - Tempo change without pitch change
//! - **Harmonizer** - Pitch shifting with formant preservation
//!
//! ## Vocoding/Correction
//! - **Vocoder** - Channel vocoder
//! - **Auto-tune** - Basic pitch correction
//!
//! # Architecture
//!
//! All effects implement the `AudioEffect` trait, which provides a unified interface
//! for real-time audio processing with support for both mono and stereo operation.
//!
//! Effects are designed to be:
//! - **Real-time capable** - Low latency, no allocations in process loops
//! - **Sample-accurate** - Parameter changes are smoothed to avoid artifacts
//! - **Efficient** - Optimized for CPU efficiency
//! - **Safe** - No unsafe code, enforced by `#![forbid(unsafe_code)]`
//!
//! # Example
//!
//! ```ignore
//! use oximedia_effects::{AudioEffect, reverb::Freeverb, ReverbConfig};
//!
//! let config = ReverbConfig::default()
//!     .with_room_size(0.8)
//!     .with_damping(0.5)
//!     .with_wet(0.3);
//!
//! let mut reverb = Freeverb::new(config, 48000.0);
//!
//! // Process stereo audio
//! let mut left = vec![0.0; 1024];
//! let mut right = vec![0.0; 1024];
//! reverb.process_stereo(&mut left, &mut right);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod auto_pan;
pub mod barrel_lens;
pub mod blend;
pub mod chorus;
pub mod color_grade;
pub mod composite;
pub mod compressor;
pub mod compressor_look;
pub mod deesser;
pub mod delay;
pub mod delay_line;
pub mod distort;
pub mod distortion;
pub mod ducking;
pub mod dynamics;
pub mod eq;
pub mod filter;
pub mod filter_bank;
pub mod flanger;
pub mod glitch;
pub mod keying;
pub mod luma_key;
pub mod modulation;
pub mod pitch;
pub mod reverb;
pub mod reverb_hall;
pub mod ring_mod;
pub mod room_reverb;
pub mod saturation;
pub mod spatial_audio;
pub mod stereo_widener;
pub mod tape_echo;
pub mod time_stretch;
pub mod transient_shaper;
pub mod tremolo;
pub mod utils;
pub mod vibrato;
pub mod video;
pub mod vocoder;
pub mod warp;
pub mod waveshaper;

use thiserror::Error;

/// Error types for audio effects.
#[derive(Debug, Error)]
pub enum EffectError {
    /// Invalid parameter value.
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Invalid sample rate.
    #[error("Invalid sample rate: {0}")]
    InvalidSampleRate(f32),

    /// Buffer size mismatch.
    #[error("Buffer size mismatch: expected {expected}, got {actual}")]
    BufferSizeMismatch {
        /// Expected buffer size.
        expected: usize,
        /// Actual buffer size.
        actual: usize,
    },

    /// Insufficient buffer size.
    #[error("Insufficient buffer size: need at least {required}, got {actual}")]
    InsufficientBuffer {
        /// Required buffer size.
        required: usize,
        /// Actual buffer size.
        actual: usize,
    },

    /// Effect not initialized.
    #[error("Effect not initialized")]
    NotInitialized,

    /// Processing error.
    #[error("Processing error: {0}")]
    ProcessingError(String),
}

/// Result type for effect operations.
pub type Result<T> = std::result::Result<T, EffectError>;

/// Core trait for audio effects.
///
/// All effects implement this trait to provide a unified interface for
/// real-time audio processing.
pub trait AudioEffect {
    /// Process a single mono sample.
    fn process_sample(&mut self, input: f32) -> f32;

    /// Process a buffer of mono samples in-place.
    fn process(&mut self, buffer: &mut [f32]) {
        for sample in buffer {
            *sample = self.process_sample(*sample);
        }
    }

    /// Process stereo samples (left and right channels).
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len());
        for i in 0..len {
            let (l, r) = self.process_sample_stereo(left[i], right[i]);
            left[i] = l;
            right[i] = r;
        }
    }

    /// Process a single stereo sample pair.
    fn process_sample_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        (self.process_sample(left), self.process_sample(right))
    }

    /// Reset the effect state (clear buffers, reset LFOs, etc.).
    fn reset(&mut self);

    /// Get the latency introduced by this effect in samples.
    fn latency_samples(&self) -> usize {
        0
    }

    /// Set the sample rate (if the effect supports it).
    fn set_sample_rate(&mut self, _sample_rate: f32) {}
}

/// Configuration for reverb effects.
#[derive(Debug, Clone)]
pub struct ReverbConfig {
    /// Room size (0.0 - 1.0).
    pub room_size: f32,
    /// Damping/high-frequency absorption (0.0 - 1.0).
    pub damping: f32,
    /// Wet signal level (0.0 - 1.0).
    pub wet: f32,
    /// Dry signal level (0.0 - 1.0).
    pub dry: f32,
    /// Stereo width (0.0 - 1.0).
    pub width: f32,
    /// Pre-delay in milliseconds.
    pub predelay_ms: f32,
}

impl Default for ReverbConfig {
    fn default() -> Self {
        Self {
            room_size: 0.5,
            damping: 0.5,
            wet: 0.33,
            dry: 0.67,
            width: 1.0,
            predelay_ms: 0.0,
        }
    }
}

impl ReverbConfig {
    /// Create a new reverb configuration with custom parameters.
    #[must_use]
    pub fn new(room_size: f32, damping: f32, wet: f32) -> Self {
        Self {
            room_size: room_size.clamp(0.0, 1.0),
            damping: damping.clamp(0.0, 1.0),
            wet: wet.clamp(0.0, 1.0),
            dry: (1.0 - wet).clamp(0.0, 1.0),
            width: 1.0,
            predelay_ms: 0.0,
        }
    }

    /// Set room size.
    #[must_use]
    pub fn with_room_size(mut self, room_size: f32) -> Self {
        self.room_size = room_size.clamp(0.0, 1.0);
        self
    }

    /// Set damping.
    #[must_use]
    pub fn with_damping(mut self, damping: f32) -> Self {
        self.damping = damping.clamp(0.0, 1.0);
        self
    }

    /// Set wet level.
    #[must_use]
    pub fn with_wet(mut self, wet: f32) -> Self {
        self.wet = wet.clamp(0.0, 1.0);
        self
    }

    /// Set dry level.
    #[must_use]
    pub fn with_dry(mut self, dry: f32) -> Self {
        self.dry = dry.clamp(0.0, 1.0);
        self
    }

    /// Set stereo width.
    #[must_use]
    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width.clamp(0.0, 1.0);
        self
    }

    /// Set pre-delay in milliseconds.
    #[must_use]
    pub fn with_predelay(mut self, predelay_ms: f32) -> Self {
        self.predelay_ms = predelay_ms.max(0.0);
        self
    }

    /// Small room preset.
    #[must_use]
    pub fn small_room() -> Self {
        Self::new(0.3, 0.4, 0.2)
    }

    /// Medium room preset.
    #[must_use]
    pub fn medium_room() -> Self {
        Self::new(0.5, 0.5, 0.3)
    }

    /// Large hall preset.
    #[must_use]
    pub fn hall() -> Self {
        Self::new(0.8, 0.6, 0.4).with_predelay(20.0)
    }

    /// Cathedral preset.
    #[must_use]
    pub fn cathedral() -> Self {
        Self::new(0.95, 0.7, 0.5).with_predelay(40.0)
    }

    /// Chamber preset.
    #[must_use]
    pub fn chamber() -> Self {
        Self::new(0.6, 0.4, 0.35).with_predelay(10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverb_config_defaults() {
        let config = ReverbConfig::default();
        assert_eq!(config.room_size, 0.5);
        assert_eq!(config.damping, 0.5);
        assert_eq!(config.wet, 0.33);
    }

    #[test]
    fn test_reverb_config_builder() {
        let config = ReverbConfig::default()
            .with_room_size(0.8)
            .with_damping(0.6)
            .with_wet(0.4);
        assert_eq!(config.room_size, 0.8);
        assert_eq!(config.damping, 0.6);
        assert_eq!(config.wet, 0.4);
    }

    #[test]
    fn test_reverb_config_clamping() {
        let config = ReverbConfig::new(1.5, -0.5, 2.0);
        assert_eq!(config.room_size, 1.0);
        assert_eq!(config.damping, 0.0);
        assert_eq!(config.wet, 1.0);
    }

    #[test]
    fn test_reverb_presets() {
        let small = ReverbConfig::small_room();
        assert!(small.room_size < 0.5);

        let hall = ReverbConfig::hall();
        assert!(hall.room_size > 0.7);
        assert!(hall.predelay_ms > 0.0);
    }
}
