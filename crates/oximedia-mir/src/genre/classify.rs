//! Genre classification using audio features.

use crate::genre::features::GenreFeatures;
use crate::types::GenreResult;
use crate::MirResult;
use std::collections::HashMap;

/// Genre classifier.
pub struct GenreClassifier {
    sample_rate: f32,
}

impl GenreClassifier {
    /// Create a new genre classifier.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self { sample_rate }
    }

    /// Classify genre from audio signal.
    ///
    /// # Errors
    ///
    /// Returns error if classification fails.
    pub fn classify(&self, signal: &[f32]) -> MirResult<GenreResult> {
        // Extract features
        let feature_extractor = GenreFeatures::new(self.sample_rate);
        let features = feature_extractor.extract(signal)?;

        // Simple rule-based classification (in practice, would use ML model)
        let mut genre_scores = HashMap::new();

        // Electronic: high spectral centroid, low zero crossing
        let electronic_score =
            features.spectral_centroid * 0.6 + (1.0 - features.zero_crossing_rate) * 0.4;
        genre_scores.insert("electronic".to_string(), electronic_score);

        // Rock: high energy, moderate tempo
        let rock_score =
            features.energy * 0.5 + self.tempo_score(features.tempo, 120.0, 160.0) * 0.5;
        genre_scores.insert("rock".to_string(), rock_score);

        // Classical: low energy variation, wide spectral range
        let classical_score =
            (1.0 - features.energy_variance) * 0.6 + features.spectral_bandwidth * 0.4;
        genre_scores.insert("classical".to_string(), classical_score);

        // Jazz: moderate tempo, high harmonic complexity
        let jazz_score = self.tempo_score(features.tempo, 100.0, 140.0) * 0.4
            + features.harmonic_complexity * 0.6;
        genre_scores.insert("jazz".to_string(), jazz_score);

        // Hip-hop: strong beats, low tempo
        let hiphop_score =
            features.beat_strength * 0.6 + self.tempo_score(features.tempo, 80.0, 110.0) * 0.4;
        genre_scores.insert("hip-hop".to_string(), hiphop_score);

        // Pop: moderate everything, strong beats
        let pop_score =
            features.beat_strength * 0.5 + self.tempo_score(features.tempo, 100.0, 130.0) * 0.5;
        genre_scores.insert("pop".to_string(), pop_score);

        // Find top genre
        let (top_genre, top_confidence) = genre_scores
            .iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map_or(("unknown".to_string(), 0.0), |(g, &c)| (g.clone(), c));

        Ok(GenreResult {
            genres: genre_scores,
            top_genre_name: top_genre,
            top_genre_confidence: top_confidence,
        })
    }

    /// Score based on tempo range.
    fn tempo_score(&self, tempo: f32, min_bpm: f32, max_bpm: f32) -> f32 {
        if tempo >= min_bpm && tempo <= max_bpm {
            1.0 - ((tempo - (min_bpm + max_bpm) / 2.0).abs() / ((max_bpm - min_bpm) / 2.0))
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genre_classifier_creation() {
        let classifier = GenreClassifier::new(44100.0);
        assert_eq!(classifier.sample_rate, 44100.0);
    }

    #[test]
    fn test_tempo_score() {
        let classifier = GenreClassifier::new(44100.0);
        assert_eq!(classifier.tempo_score(120.0, 100.0, 140.0), 1.0);
        assert!(classifier.tempo_score(200.0, 100.0, 140.0) < 0.1);
    }
}
