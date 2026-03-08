//! Music Information Retrieval (MIR) system for `OxiMedia`.
//!
//! This crate provides comprehensive music analysis capabilities for audio content:
//!
//! # Tempo and Beat Analysis
//!
//! - **Tempo Detection** - BPM detection using autocorrelation and comb filtering
//! - **Beat Tracking** - Beat and downbeat detection with dynamic programming
//! - **Onset Detection** - Transient detection using spectral flux and HFC
//!
//! # Tonal Analysis
//!
//! - **Key Detection** - Musical key detection (Krumhansl-Schmuckler algorithm)
//! - **Chord Recognition** - Chord progression analysis using chroma features
//! - **Melody Extraction** - Dominant melody line extraction
//! - **Harmonic Analysis** - Harmonic-percussive separation
//!
//! # Structure Analysis
//!
//! - **Structural Segmentation** - Section boundary detection
//! - **Self-Similarity Analysis** - Pattern and repetition detection
//! - **Section Labeling** - Intro, verse, chorus, bridge identification
//!
//! # High-Level Features
//!
//! - **Genre Classification** - Genre detection from audio features
//! - **Mood Detection** - Valence and arousal estimation
//! - **Loudness Analysis** - Integrated loudness and dynamics
//!
//! # Low-Level Features
//!
//! - **Spectral Features** - Centroid, rolloff, flux, contrast
//! - **Rhythm Features** - Rhythm patterns and complexity
//! - **Pitch Features** - Pitch class profiles and chromagrams
//!
//! # Usage
//!
//! ```no_run
//! use oximedia_mir::{MirAnalyzer, MirConfig, FeatureSet};
//!
//! // Create analyzer with default configuration
//! let config = MirConfig::default();
//! let analyzer = MirAnalyzer::new(config);
//!
//! // Analyze audio samples (f32, mono or stereo)
//! let samples = vec![0.0_f32; 44100]; // 1 second of silence
//! let sample_rate = 44100.0;
//!
//! // Perform analysis
//! let result = analyzer.analyze(&samples, sample_rate)?;
//!
//! // Access results
//! if let Some(ref tempo) = result.tempo {
//!     println!("Tempo: {:.1} BPM (confidence: {:.2})", tempo.bpm, tempo.confidence);
//! }
//! if let Some(ref key) = result.key {
//!     println!("Key: {} (confidence: {:.2})", key.key, key.confidence);
//! }
//! if let Some(ref genre) = result.genre {
//!     println!("Genre: {} (confidence: {:.2})", genre.top_genre().0, genre.top_genre().1);
//! }
//!
//! # Ok::<(), oximedia_mir::MirError>(())
//! ```
//!
//! # Patent-Free Implementation
//!
//! All algorithms are implemented using patent-free methods:
//! - Autocorrelation-based tempo detection
//! - Chroma-based chord recognition
//! - Spectral-based onset detection
//! - Krumhansl-Schmuckler key detection
//!
//! # Real-Time Capable
//!
//! Many features support frame-by-frame processing for real-time applications.

#![warn(missing_docs)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::similar_names)]
#![allow(clippy::many_single_char_names)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::let_and_return)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::unused_self)]
#![allow(clippy::module_name_repetitions)]
#![allow(dead_code)]

pub mod audio_events;
pub mod audio_features;
pub mod beat;
pub mod beat_tracker;
pub mod chord;
pub mod chord_recognition;
pub mod chorus_detect;
pub mod cover_detect;
pub mod dynamic_range;
pub mod energy_contour;
pub mod fade_detect;
pub mod fingerprint;
pub mod genre;
pub mod genre_classify;
pub mod harmonic;
pub mod harmonic_analysis;
pub mod instrument_detection;
pub mod key;
pub mod key_detection;
pub mod loudness;
pub mod melody;
pub mod mir_feature;
pub mod mood;
pub mod mood_detection;
pub mod music_summary;
pub mod onset_strength;
pub mod pitch_key;
pub mod pitch_track;
pub mod playlist;
pub mod playlist_gen;
pub mod rhythm;
pub mod rhythm_pattern;
pub mod segmentation;
pub mod similarity;
pub mod source_separation;
pub mod spectral;
pub mod spectral_contrast;
pub mod spectral_features;
pub mod structure;
pub mod structure_analysis;
pub mod tempo;
pub mod tempo_map;
pub mod tuning_detect;
pub mod vocal_detect;

mod error;
mod types;
mod utils;

pub use error::{MirError, MirResult};
pub use types::{
    AnalysisResult, BeatResult, ChordResult, FeatureSet, GenreResult, HarmonicResult, KeyResult,
    LoudnessResult, MelodyResult, MoodResult, RhythmResult, SpectralResult, StructureResult,
    TempoResult,
};

use std::collections::HashMap;

/// Configuration for MIR analysis.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct MirConfig {
    /// Window size for frame-based analysis (samples).
    pub window_size: usize,

    /// Hop size for frame-based analysis (samples).
    pub hop_size: usize,

    /// Minimum tempo to detect (BPM).
    pub min_tempo: f32,

    /// Maximum tempo to detect (BPM).
    pub max_tempo: f32,

    /// Enable beat tracking.
    pub enable_beat_tracking: bool,

    /// Enable key detection.
    pub enable_key_detection: bool,

    /// Enable chord recognition.
    pub enable_chord_recognition: bool,

    /// Enable melody extraction.
    pub enable_melody_extraction: bool,

    /// Enable structure analysis.
    pub enable_structure_analysis: bool,

    /// Enable genre classification.
    pub enable_genre_classification: bool,

    /// Enable mood detection.
    pub enable_mood_detection: bool,

    /// Enable spectral features.
    pub enable_spectral_features: bool,

    /// Enable rhythm features.
    pub enable_rhythm_features: bool,

    /// Enable harmonic analysis.
    pub enable_harmonic_analysis: bool,
}

impl Default for MirConfig {
    fn default() -> Self {
        Self {
            window_size: 2048,
            hop_size: 512,
            min_tempo: 60.0,
            max_tempo: 200.0,
            enable_beat_tracking: true,
            enable_key_detection: true,
            enable_chord_recognition: true,
            enable_melody_extraction: true,
            enable_structure_analysis: true,
            enable_genre_classification: true,
            enable_mood_detection: true,
            enable_spectral_features: true,
            enable_rhythm_features: true,
            enable_harmonic_analysis: true,
        }
    }
}

/// Main MIR analyzer.
pub struct MirAnalyzer {
    config: MirConfig,
}

impl MirAnalyzer {
    /// Create a new MIR analyzer with the given configuration.
    #[must_use]
    pub fn new(config: MirConfig) -> Self {
        Self { config }
    }

    /// Analyze audio samples and extract all enabled features.
    ///
    /// # Arguments
    ///
    /// * `samples` - Audio samples (f32, mono or interleaved stereo)
    /// * `sample_rate` - Sample rate in Hz
    ///
    /// # Returns
    ///
    /// Complete analysis results including all enabled features.
    ///
    /// # Errors
    ///
    /// Returns error if analysis fails.
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_precision_loss)]
    pub fn analyze(&self, samples: &[f32], sample_rate: f32) -> MirResult<AnalysisResult> {
        // Convert to mono if stereo
        let mono = self.to_mono(samples);

        // Tempo and beat analysis
        let tempo = if self.config.enable_beat_tracking {
            let detector = tempo::TempoDetector::new(
                sample_rate,
                self.config.min_tempo,
                self.config.max_tempo,
            );
            Some(detector.detect(&mono)?)
        } else {
            None
        };

        let beat = if self.config.enable_beat_tracking {
            let tracker = beat::BeatTracker::new(sample_rate, self.config.hop_size);
            Some(tracker.track(&mono, tempo.as_ref())?)
        } else {
            None
        };

        // Key detection
        let key = if self.config.enable_key_detection {
            let detector = key::KeyDetector::new(sample_rate, self.config.window_size);
            Some(detector.detect(&mono)?)
        } else {
            None
        };

        // Chord recognition
        let chord = if self.config.enable_chord_recognition {
            let recognizer = chord::ChordRecognizer::new(
                sample_rate,
                self.config.window_size,
                self.config.hop_size,
            );
            Some(recognizer.recognize(&mono)?)
        } else {
            None
        };

        // Melody extraction
        let melody = if self.config.enable_melody_extraction {
            let extractor = melody::MelodyExtractor::new(
                sample_rate,
                self.config.window_size,
                self.config.hop_size,
            );
            Some(extractor.extract(&mono)?)
        } else {
            None
        };

        // Structure analysis
        let structure = if self.config.enable_structure_analysis {
            let analyzer = structure::StructureAnalyzer::new(
                sample_rate,
                self.config.window_size,
                self.config.hop_size,
            );
            Some(analyzer.analyze(&mono)?)
        } else {
            None
        };

        // Genre classification
        let genre = if self.config.enable_genre_classification {
            let classifier = genre::GenreClassifier::new(sample_rate);
            Some(classifier.classify(&mono)?)
        } else {
            None
        };

        // Mood detection
        let mood = if self.config.enable_mood_detection {
            let detector = mood::MoodDetector::new(sample_rate);
            Some(detector.detect(&mono)?)
        } else {
            None
        };

        // Spectral features
        let spectral = if self.config.enable_spectral_features {
            let analyzer = spectral::SpectralAnalyzer::new(
                sample_rate,
                self.config.window_size,
                self.config.hop_size,
            );
            Some(analyzer.analyze(&mono)?)
        } else {
            None
        };

        // Rhythm features
        let rhythm = if self.config.enable_rhythm_features {
            let analyzer = rhythm::RhythmAnalyzer::new(sample_rate, self.config.hop_size);
            Some(analyzer.analyze(&mono)?)
        } else {
            None
        };

        // Harmonic analysis
        let harmonic = if self.config.enable_harmonic_analysis {
            let analyzer = harmonic::HarmonicAnalyzer::new(
                sample_rate,
                self.config.window_size,
                self.config.hop_size,
            );
            Some(analyzer.analyze(&mono)?)
        } else {
            None
        };

        Ok(AnalysisResult {
            tempo,
            beat,
            key,
            chord,
            melody,
            structure,
            genre,
            mood,
            spectral,
            rhythm,
            harmonic,
            sample_rate,
            duration: mono.len() as f32 / sample_rate,
        })
    }

    /// Convert stereo to mono by averaging channels.
    fn to_mono(&self, samples: &[f32]) -> Vec<f32> {
        // Assume mono for now, could detect stereo and convert
        let _ = self; // Suppress unused_self warning
        samples.to_vec()
    }

    /// Extract specific feature set.
    ///
    /// # Errors
    ///
    /// Returns error if feature extraction fails.
    pub fn extract_features(
        &self,
        samples: &[f32],
        sample_rate: f32,
        features: FeatureSet,
    ) -> MirResult<HashMap<String, Vec<f32>>> {
        let mono = self.to_mono(samples);
        let mut result = HashMap::new();

        if features.contains(FeatureSet::SPECTRAL) {
            let analyzer = spectral::SpectralAnalyzer::new(
                sample_rate,
                self.config.window_size,
                self.config.hop_size,
            );
            let spectral = analyzer.analyze(&mono)?;
            result.insert("spectral_centroid".to_string(), spectral.centroid);
            result.insert("spectral_rolloff".to_string(), spectral.rolloff);
            result.insert("spectral_flux".to_string(), spectral.flux);
        }

        if features.contains(FeatureSet::RHYTHM) {
            let analyzer = rhythm::RhythmAnalyzer::new(sample_rate, self.config.hop_size);
            let rhythm = analyzer.analyze(&mono)?;
            result.insert("onset_strength".to_string(), rhythm.onset_strength);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mir_config_default() {
        let config = MirConfig::default();
        assert_eq!(config.window_size, 2048);
        assert_eq!(config.hop_size, 512);
        assert!(config.enable_beat_tracking);
    }

    #[test]
    fn test_mir_analyzer_creation() {
        let config = MirConfig::default();
        let _analyzer = MirAnalyzer::new(config);
    }

    #[test]
    fn test_analyze_silence() {
        let config = MirConfig {
            enable_beat_tracking: false, // Disable beat tracking for silence test
            enable_genre_classification: false,
            enable_structure_analysis: false,
            ..MirConfig::default()
        };
        let analyzer = MirAnalyzer::new(config);
        let samples = vec![0.0_f32; 44100]; // 1 second of silence
        let result = analyzer.analyze(&samples, 44100.0);
        assert!(result.is_ok());
    }
}
