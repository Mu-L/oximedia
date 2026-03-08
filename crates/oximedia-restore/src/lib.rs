//! Professional audio restoration tools for `OxiMedia`.
//!
//! `oximedia-restore` provides comprehensive audio restoration capabilities for
//! recovering and enhancing degraded audio recordings.
//!
//! # Features
//!
//! - **Click/Pop Removal** - Remove vinyl clicks and digital glitches
//! - **Hum Removal** - Remove 50Hz/60Hz hum and harmonics
//! - **Noise Reduction** - Spectral subtraction, gating, and Wiener filtering
//! - **Declipping** - Restore clipped audio peaks
//! - **Dehiss** - Remove tape hiss and background noise
//! - **Decrackle** - Remove crackle from old recordings
//! - **Azimuth Correction** - Correct tape azimuth errors
//! - **Wow/Flutter Removal** - Remove tape speed variations
//! - **DC Offset Removal** - Remove DC bias
//! - **Phase Correction** - Correct phase issues
//!
//! # Example
//!
//! ```
//! use oximedia_restore::presets::VinylRestoration;
//! use oximedia_restore::RestoreChain;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a restoration chain for vinyl
//! let mut chain = RestoreChain::new();
//! chain.add_preset(VinylRestoration::default());
//!
//! // Process samples
//! let samples = vec![0.0; 44100]; // 1 second at 44.1 kHz
//! let restored = chain.process(&samples, 44100)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Restoration Presets
//!
//! Pre-configured restoration chains for common scenarios:
//!
//! - **Vinyl Restoration** - Click removal, decrackle, hum removal
//! - **Tape Restoration** - Azimuth, wow/flutter, hiss removal
//! - **Broadcast Cleanup** - Declipping, noise reduction, DC removal
//! - **Archival** - Full restoration chain for preservation

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod azimuth;
pub mod click;
pub mod clip;
pub mod crackle;
pub mod dc;
pub mod declip;
pub mod error;
pub mod flutter_repair;
pub mod hiss;
pub mod hum;
pub mod noise;
pub mod phase;
pub mod room_correction;
pub mod utils;
pub mod wow;

pub mod banding_reduce;
pub mod color_bleed;
pub mod color_restore;
pub mod deband;
pub mod deflicker;
pub mod dropout_fix;
pub mod film_grain;
pub mod grain_add;
pub mod grain_restore;
pub mod noise_profile_match;
pub mod pitch_correct;
pub mod presets;
pub mod restore_plan;
pub mod restore_report;
pub mod scan_line;
pub mod spectral_repair;
pub mod stereo_field_repair;
pub mod telecine_detect;
pub mod upscale;
pub mod vintage;

// Re-exports
pub use error::{RestoreError, RestoreResult};

use azimuth::AzimuthCorrector;
use click::{ClickDetector, ClickRemover};
use clip::{BasicDeclipper, ClipDetector};
use crackle::{CrackleDetector, CrackleRemover};
use dc::DcRemover;
use hiss::HissRemover;
use hum::HumRemover;
use noise::{NoiseGate, SpectralSubtraction, WienerFilter};
use phase::PhaseCorrector;
use wow::WowFlutterCorrector;

/// Restoration step in a processing chain.
#[derive(Debug)]
pub enum RestorationStep {
    /// Remove DC offset.
    DcRemoval(DcRemover),
    /// Detect and remove clicks/pops.
    ClickRemoval {
        /// Click detector.
        detector: ClickDetector,
        /// Click remover.
        remover: ClickRemover,
    },
    /// Remove hum and harmonics.
    HumRemoval(HumRemover),
    /// Spectral noise reduction.
    NoiseReduction(SpectralSubtraction),
    /// Wiener filtering.
    WienerFilter(WienerFilter),
    /// Noise gate.
    NoiseGate(NoiseGate),
    /// Declipping.
    Declipping {
        /// Clipping detector.
        detector: ClipDetector,
        /// Declipper.
        declipper: BasicDeclipper,
    },
    /// Hiss removal.
    HissRemoval(HissRemover),
    /// Crackle removal.
    CrackleRemoval {
        /// Crackle detector.
        detector: CrackleDetector,
        /// Crackle remover.
        remover: CrackleRemover,
    },
    /// Azimuth correction (stereo only).
    AzimuthCorrection(AzimuthCorrector),
    /// Wow/flutter correction.
    WowFlutterCorrection(WowFlutterCorrector),
    /// Phase correction (stereo only).
    PhaseCorrection(PhaseCorrector),
}

/// Audio restoration processing chain.
#[derive(Debug)]
pub struct RestoreChain {
    steps: Vec<RestorationStep>,
}

impl RestoreChain {
    /// Create a new empty restoration chain.
    #[must_use]
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Add a restoration step to the chain.
    pub fn add_step(&mut self, step: RestorationStep) {
        self.steps.push(step);
    }

    /// Add a preset to the chain.
    pub fn add_preset(&mut self, preset: impl Into<Vec<RestorationStep>>) {
        self.steps.extend(preset.into());
    }

    /// Process mono audio samples.
    pub fn process(&mut self, samples: &[f32], sample_rate: u32) -> RestoreResult<Vec<f32>> {
        let mut output = samples.to_vec();

        for step in &mut self.steps {
            output = match step {
                RestorationStep::DcRemoval(remover) => remover.process(&output)?,
                RestorationStep::ClickRemoval { detector, remover } => {
                    let clicks = detector.detect(&output)?;
                    remover.remove(&output, &clicks)?
                }
                RestorationStep::HumRemoval(remover) => remover.process(&output)?,
                RestorationStep::NoiseReduction(reducer) => reducer.process(&output)?,
                RestorationStep::WienerFilter(filter) => filter.process(&output)?,
                RestorationStep::NoiseGate(gate) => gate.process(&output)?,
                RestorationStep::Declipping {
                    detector,
                    declipper,
                } => {
                    let regions = detector.detect(&output)?;
                    declipper.restore(&output, &regions)?
                }
                RestorationStep::HissRemoval(remover) => remover.process(&output, sample_rate)?,
                RestorationStep::CrackleRemoval { detector, remover } => {
                    let crackles = detector.detect(&output)?;
                    remover.remove(&output, &crackles)?
                }
                RestorationStep::WowFlutterCorrection(corrector) => corrector.correct(&output)?,
                // Stereo-only steps are skipped for mono
                RestorationStep::AzimuthCorrection(_) | RestorationStep::PhaseCorrection(_) => {
                    output
                }
            };
        }

        Ok(output)
    }

    /// Process stereo audio samples.
    pub fn process_stereo(
        &mut self,
        left: &[f32],
        right: &[f32],
        sample_rate: u32,
    ) -> RestoreResult<(Vec<f32>, Vec<f32>)> {
        let mut out_left = left.to_vec();
        let mut out_right = right.to_vec();

        for step in &mut self.steps {
            match step {
                RestorationStep::DcRemoval(remover) => {
                    out_left = remover.process(&out_left)?;
                    // Need separate instance for right channel
                    let mut remover_r = remover.clone();
                    out_right = remover_r.process(&out_right)?;
                }
                RestorationStep::ClickRemoval { detector, remover } => {
                    let clicks_l = detector.detect(&out_left)?;
                    out_left = remover.remove(&out_left, &clicks_l)?;

                    let clicks_r = detector.detect(&out_right)?;
                    out_right = remover.remove(&out_right, &clicks_r)?;
                }
                RestorationStep::HumRemoval(remover) => {
                    out_left = remover.process(&out_left)?;
                    let mut remover_r = remover.clone();
                    out_right = remover_r.process(&out_right)?;
                }
                RestorationStep::NoiseReduction(reducer) => {
                    out_left = reducer.process(&out_left)?;
                    // Note: In practice, you'd want separate instances
                    out_right = reducer.process(&out_right)?;
                }
                RestorationStep::WienerFilter(filter) => {
                    out_left = filter.process(&out_left)?;
                    out_right = filter.process(&out_right)?;
                }
                RestorationStep::NoiseGate(gate) => {
                    out_left = gate.process(&out_left)?;
                    let mut gate_r = gate.clone();
                    out_right = gate_r.process(&out_right)?;
                }
                RestorationStep::Declipping {
                    detector,
                    declipper,
                } => {
                    let regions_l = detector.detect(&out_left)?;
                    out_left = declipper.restore(&out_left, &regions_l)?;

                    let regions_r = detector.detect(&out_right)?;
                    out_right = declipper.restore(&out_right, &regions_r)?;
                }
                RestorationStep::HissRemoval(remover) => {
                    out_left = remover.process(&out_left, sample_rate)?;
                    out_right = remover.process(&out_right, sample_rate)?;
                }
                RestorationStep::CrackleRemoval { detector, remover } => {
                    let crackles_l = detector.detect(&out_left)?;
                    out_left = remover.remove(&out_left, &crackles_l)?;

                    let crackles_r = detector.detect(&out_right)?;
                    out_right = remover.remove(&out_right, &crackles_r)?;
                }
                RestorationStep::AzimuthCorrection(corrector) => {
                    (out_left, out_right) = corrector.correct(&out_left, &out_right)?;
                }
                RestorationStep::WowFlutterCorrection(corrector) => {
                    out_left = corrector.correct(&out_left)?;
                    out_right = corrector.correct(&out_right)?;
                }
                RestorationStep::PhaseCorrection(corrector) => {
                    (out_left, out_right) = corrector.correct(&out_left, &out_right)?;
                }
            }
        }

        Ok((out_left, out_right))
    }

    /// Clear all steps from the chain.
    pub fn clear(&mut self) {
        self.steps.clear();
    }

    /// Get number of steps in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Check if chain is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

impl Default for RestoreChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restore_chain() {
        let mut chain = RestoreChain::new();
        assert!(chain.is_empty());

        chain.add_step(RestorationStep::DcRemoval(DcRemover::new(10.0, 44100)));
        assert_eq!(chain.len(), 1);

        let samples = vec![0.5; 1000];
        let result = chain
            .process(&samples, 44100)
            .expect("should succeed in test");
        assert_eq!(result.len(), samples.len());
    }

    #[test]
    fn test_stereo_processing() {
        let mut chain = RestoreChain::new();
        chain.add_step(RestorationStep::DcRemoval(DcRemover::new(10.0, 44100)));

        let left = vec![0.5; 1000];
        let right = vec![0.5; 1000];

        let (out_l, out_r) = chain
            .process_stereo(&left, &right, 44100)
            .expect("should succeed in test");
        assert_eq!(out_l.len(), left.len());
        assert_eq!(out_r.len(), right.len());
    }

    #[test]
    fn test_clear() {
        let mut chain = RestoreChain::new();
        chain.add_step(RestorationStep::DcRemoval(DcRemover::new(10.0, 44100)));
        assert!(!chain.is_empty());

        chain.clear();
        assert!(chain.is_empty());
    }
}
