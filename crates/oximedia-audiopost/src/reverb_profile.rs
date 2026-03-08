#![allow(dead_code)]
//! Reverb profile management for audio post-production.
//!
//! Provides configurable reverb profiles modeling different acoustic spaces
//! (rooms, halls, plates, etc.) with early reflections, late reverb tail,
//! frequency-dependent decay, and mix control.

use std::collections::HashMap;

/// Type of reverb algorithm or acoustic model.
#[derive(Debug, Clone, PartialEq)]
pub enum ReverbType {
    /// Small room reverb.
    Room,
    /// Large hall reverb.
    Hall,
    /// Metal plate reverb simulation.
    Plate,
    /// Spring reverb simulation.
    Spring,
    /// Cathedral / church reverb.
    Cathedral,
    /// Ambient / diffuse reverb.
    Ambient,
    /// Chamber reverb (recording studio live room).
    Chamber,
    /// Custom convolution-based reverb.
    Convolution,
}

/// Early reflection definition.
#[derive(Debug, Clone)]
pub struct EarlyReflection {
    /// Delay in milliseconds from the direct sound.
    pub delay_ms: f64,
    /// Gain (linear, 0.0..1.0).
    pub gain: f64,
    /// Pan position (-1.0 = left, 0.0 = center, 1.0 = right).
    pub pan: f64,
}

impl EarlyReflection {
    /// Create a new early reflection.
    pub fn new(delay_ms: f64, gain: f64, pan: f64) -> Self {
        Self {
            delay_ms: delay_ms.max(0.0),
            gain: gain.clamp(0.0, 1.0),
            pan: pan.clamp(-1.0, 1.0),
        }
    }

    /// Return the delay as a sample count at the given sample rate.
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    pub fn delay_samples(&self, sample_rate: u32) -> usize {
        ((self.delay_ms / 1000.0) * sample_rate as f64) as usize
    }
}

/// Frequency band decay modifier.
#[derive(Debug, Clone)]
pub struct BandDecay {
    /// Center frequency in Hz.
    pub frequency_hz: f64,
    /// Decay multiplier (1.0 = normal, <1.0 = faster decay, >1.0 = slower).
    pub decay_factor: f64,
}

impl BandDecay {
    /// Create a new band decay entry.
    pub fn new(frequency_hz: f64, decay_factor: f64) -> Self {
        Self {
            frequency_hz: frequency_hz.max(20.0),
            decay_factor: decay_factor.max(0.0),
        }
    }
}

/// A complete reverb profile configuration.
#[derive(Debug, Clone)]
pub struct ReverbProfile {
    /// Profile name.
    pub name: String,
    /// Type of reverb.
    pub reverb_type: ReverbType,
    /// Pre-delay in milliseconds.
    pub pre_delay_ms: f64,
    /// RT60 decay time in seconds.
    pub decay_time_s: f64,
    /// Diffusion (0.0..1.0, controls echo density).
    pub diffusion: f64,
    /// Damping (0.0..1.0, high-frequency rolloff in the tail).
    pub damping: f64,
    /// Room size factor (0.0..1.0).
    pub room_size: f64,
    /// Early reflections.
    pub early_reflections: Vec<EarlyReflection>,
    /// Frequency-dependent decay modifiers.
    pub band_decays: Vec<BandDecay>,
    /// Dry/wet mix (0.0 = fully dry, 1.0 = fully wet).
    pub mix: f64,
    /// Output gain in dB.
    pub output_gain_db: f64,
    /// Stereo width (0.0 = mono, 1.0 = full stereo).
    pub stereo_width: f64,
    /// Metadata tags.
    pub tags: HashMap<String, String>,
}

impl ReverbProfile {
    /// Create a new reverb profile with default parameters.
    pub fn new(name: impl Into<String>, reverb_type: ReverbType) -> Self {
        Self {
            name: name.into(),
            reverb_type,
            pre_delay_ms: 10.0,
            decay_time_s: 1.5,
            diffusion: 0.7,
            damping: 0.5,
            room_size: 0.5,
            early_reflections: Vec::new(),
            band_decays: Vec::new(),
            mix: 0.3,
            output_gain_db: 0.0,
            stereo_width: 1.0,
            tags: HashMap::new(),
        }
    }

    /// Set pre-delay in milliseconds.
    pub fn with_pre_delay(mut self, ms: f64) -> Self {
        self.pre_delay_ms = ms.max(0.0);
        self
    }

    /// Set RT60 decay time in seconds.
    pub fn with_decay_time(mut self, seconds: f64) -> Self {
        self.decay_time_s = seconds.max(0.01);
        self
    }

    /// Set diffusion.
    pub fn with_diffusion(mut self, diffusion: f64) -> Self {
        self.diffusion = diffusion.clamp(0.0, 1.0);
        self
    }

    /// Set damping.
    pub fn with_damping(mut self, damping: f64) -> Self {
        self.damping = damping.clamp(0.0, 1.0);
        self
    }

    /// Set room size.
    pub fn with_room_size(mut self, size: f64) -> Self {
        self.room_size = size.clamp(0.0, 1.0);
        self
    }

    /// Set dry/wet mix.
    pub fn with_mix(mut self, mix: f64) -> Self {
        self.mix = mix.clamp(0.0, 1.0);
        self
    }

    /// Set output gain in dB.
    pub fn with_output_gain(mut self, db: f64) -> Self {
        self.output_gain_db = db;
        self
    }

    /// Set stereo width.
    pub fn with_stereo_width(mut self, width: f64) -> Self {
        self.stereo_width = width.clamp(0.0, 1.0);
        self
    }

    /// Add an early reflection.
    pub fn add_early_reflection(mut self, reflection: EarlyReflection) -> Self {
        self.early_reflections.push(reflection);
        self
    }

    /// Add a frequency band decay modifier.
    pub fn add_band_decay(mut self, band: BandDecay) -> Self {
        self.band_decays.push(band);
        self
    }

    /// Add a metadata tag.
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Compute the output gain as a linear multiplier.
    #[allow(clippy::cast_precision_loss)]
    pub fn output_gain_linear(&self) -> f64 {
        10.0_f64.powf(self.output_gain_db / 20.0)
    }

    /// Estimate the tail length in samples at the given sample rate.
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    pub fn tail_length_samples(&self, sample_rate: u32) -> usize {
        let total_s = (self.pre_delay_ms / 1000.0) + self.decay_time_s;
        (total_s * sample_rate as f64) as usize
    }

    /// Create a preset for a small room.
    pub fn small_room() -> Self {
        Self::new("Small Room", ReverbType::Room)
            .with_pre_delay(5.0)
            .with_decay_time(0.4)
            .with_diffusion(0.6)
            .with_damping(0.7)
            .with_room_size(0.2)
            .with_mix(0.2)
    }

    /// Create a preset for a large hall.
    pub fn large_hall() -> Self {
        Self::new("Large Hall", ReverbType::Hall)
            .with_pre_delay(25.0)
            .with_decay_time(3.0)
            .with_diffusion(0.85)
            .with_damping(0.3)
            .with_room_size(0.9)
            .with_mix(0.35)
    }

    /// Create a preset for a plate reverb.
    pub fn plate() -> Self {
        Self::new("Plate", ReverbType::Plate)
            .with_pre_delay(0.0)
            .with_decay_time(1.8)
            .with_diffusion(0.95)
            .with_damping(0.4)
            .with_room_size(0.5)
            .with_mix(0.25)
    }

    /// Create a preset for a cathedral.
    pub fn cathedral() -> Self {
        Self::new("Cathedral", ReverbType::Cathedral)
            .with_pre_delay(40.0)
            .with_decay_time(5.0)
            .with_diffusion(0.9)
            .with_damping(0.2)
            .with_room_size(1.0)
            .with_mix(0.4)
    }
}

/// Library of named reverb profiles.
#[derive(Debug)]
pub struct ReverbProfileLibrary {
    /// Profiles indexed by name.
    profiles: HashMap<String, ReverbProfile>,
}

impl Default for ReverbProfileLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl ReverbProfileLibrary {
    /// Create an empty library.
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
        }
    }

    /// Create a library pre-loaded with factory presets.
    pub fn with_factory_presets() -> Self {
        let mut lib = Self::new();
        lib.add(ReverbProfile::small_room());
        lib.add(ReverbProfile::large_hall());
        lib.add(ReverbProfile::plate());
        lib.add(ReverbProfile::cathedral());
        lib
    }

    /// Add a profile to the library.
    pub fn add(&mut self, profile: ReverbProfile) {
        self.profiles.insert(profile.name.clone(), profile);
    }

    /// Get a profile by name.
    pub fn get(&self, name: &str) -> Option<&ReverbProfile> {
        self.profiles.get(name)
    }

    /// Remove a profile by name.
    pub fn remove(&mut self, name: &str) -> Option<ReverbProfile> {
        self.profiles.remove(name)
    }

    /// List all profile names.
    pub fn names(&self) -> Vec<&str> {
        self.profiles.keys().map(|s| s.as_str()).collect()
    }

    /// Return the number of profiles.
    pub fn count(&self) -> usize {
        self.profiles.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_profile_defaults() {
        let p = ReverbProfile::new("test", ReverbType::Room);
        assert_eq!(p.name, "test");
        assert_eq!(p.reverb_type, ReverbType::Room);
        assert!((p.mix - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_with_pre_delay() {
        let p = ReverbProfile::new("t", ReverbType::Hall).with_pre_delay(20.0);
        assert!((p.pre_delay_ms - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_with_decay_time() {
        let p = ReverbProfile::new("t", ReverbType::Hall).with_decay_time(2.5);
        assert!((p.decay_time_s - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_clamping() {
        let p = ReverbProfile::new("t", ReverbType::Room)
            .with_diffusion(1.5)
            .with_damping(-0.5)
            .with_mix(2.0);
        assert!((p.diffusion - 1.0).abs() < f64::EPSILON);
        assert!((p.damping - 0.0).abs() < f64::EPSILON);
        assert!((p.mix - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_output_gain_linear() {
        let p = ReverbProfile::new("t", ReverbType::Room).with_output_gain(0.0);
        assert!((p.output_gain_linear() - 1.0).abs() < 1e-10);

        let p2 = ReverbProfile::new("t", ReverbType::Room).with_output_gain(-20.0);
        assert!((p2.output_gain_linear() - 0.1).abs() < 1e-10);
    }

    #[test]
    fn test_tail_length_samples() {
        let p = ReverbProfile::new("t", ReverbType::Room)
            .with_pre_delay(0.0)
            .with_decay_time(1.0);
        assert_eq!(p.tail_length_samples(48000), 48000);
    }

    #[test]
    fn test_early_reflection() {
        let er = EarlyReflection::new(15.0, 0.8, -0.3);
        assert!((er.delay_ms - 15.0).abs() < f64::EPSILON);
        assert!((er.gain - 0.8).abs() < f64::EPSILON);
        assert!((er.pan - (-0.3)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_early_reflection_delay_samples() {
        let er = EarlyReflection::new(10.0, 0.5, 0.0);
        assert_eq!(er.delay_samples(48000), 480);
    }

    #[test]
    fn test_early_reflection_clamping() {
        let er = EarlyReflection::new(-5.0, 1.5, 2.0);
        assert!(er.delay_ms >= 0.0);
        assert!((er.gain - 1.0).abs() < f64::EPSILON);
        assert!((er.pan - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_band_decay() {
        let bd = BandDecay::new(4000.0, 0.8);
        assert!((bd.frequency_hz - 4000.0).abs() < f64::EPSILON);
        assert!((bd.decay_factor - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_small_room_preset() {
        let p = ReverbProfile::small_room();
        assert_eq!(p.name, "Small Room");
        assert_eq!(p.reverb_type, ReverbType::Room);
        assert!(p.decay_time_s < 1.0);
    }

    #[test]
    fn test_large_hall_preset() {
        let p = ReverbProfile::large_hall();
        assert_eq!(p.name, "Large Hall");
        assert!(p.decay_time_s >= 2.0);
    }

    #[test]
    fn test_cathedral_preset() {
        let p = ReverbProfile::cathedral();
        assert_eq!(p.reverb_type, ReverbType::Cathedral);
        assert!(p.decay_time_s >= 4.0);
    }

    #[test]
    fn test_plate_preset() {
        let p = ReverbProfile::plate();
        assert_eq!(p.reverb_type, ReverbType::Plate);
    }

    #[test]
    fn test_library_basic() {
        let mut lib = ReverbProfileLibrary::new();
        lib.add(ReverbProfile::new("my-verb", ReverbType::Ambient));
        assert_eq!(lib.count(), 1);
        assert!(lib.get("my-verb").is_some());
        assert!(lib.get("unknown").is_none());
    }

    #[test]
    fn test_library_factory_presets() {
        let lib = ReverbProfileLibrary::with_factory_presets();
        assert_eq!(lib.count(), 4);
        assert!(lib.get("Small Room").is_some());
        assert!(lib.get("Large Hall").is_some());
        assert!(lib.get("Plate").is_some());
        assert!(lib.get("Cathedral").is_some());
    }

    #[test]
    fn test_library_remove() {
        let mut lib = ReverbProfileLibrary::with_factory_presets();
        let removed = lib.remove("Plate");
        assert!(removed.is_some());
        assert_eq!(lib.count(), 3);
    }

    #[test]
    fn test_profile_with_tags() {
        let p = ReverbProfile::new("tagged", ReverbType::Chamber).with_tag("genre", "film");
        assert_eq!(p.tags.get("genre").map(|s| s.as_str()), Some("film"));
    }

    #[test]
    fn test_default_library() {
        let lib = ReverbProfileLibrary::default();
        assert_eq!(lib.count(), 0);
    }

    #[test]
    fn test_add_early_reflection_and_band_decay() {
        let p = ReverbProfile::new("t", ReverbType::Room)
            .add_early_reflection(EarlyReflection::new(5.0, 0.9, 0.0))
            .add_early_reflection(EarlyReflection::new(12.0, 0.6, 0.5))
            .add_band_decay(BandDecay::new(8000.0, 0.5));
        assert_eq!(p.early_reflections.len(), 2);
        assert_eq!(p.band_decays.len(), 1);
    }
}
