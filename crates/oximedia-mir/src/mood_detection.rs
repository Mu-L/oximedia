//! Music mood and emotion detection.
//!
//! Implements a simple Russell-inspired valence/arousal model for classifying
//! music into emotional quadrants based on low-level audio features.

#![allow(dead_code)]

// ── ValenceLevel ─────────────────────────────────────────────────────────────

/// Emotional valence level (negative ↔ positive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValenceLevel {
    /// Strongly negative affect (sad, melancholic).
    VeryNegative,
    /// Mildly negative affect.
    Negative,
    /// Neutral / ambiguous valence.
    Neutral,
    /// Mildly positive affect.
    Positive,
    /// Strongly positive affect (joyful, happy).
    VeryPositive,
}

impl ValenceLevel {
    /// Numeric score in the range \[−1.0, 1.0\] representing this valence level.
    #[must_use]
    pub fn score(&self) -> f32 {
        match self {
            Self::VeryNegative => -1.0,
            Self::Negative => -0.5,
            Self::Neutral => 0.0,
            Self::Positive => 0.5,
            Self::VeryPositive => 1.0,
        }
    }
}

// ── ArousalLevel ──────────────────────────────────────────────────────────────

/// Emotional arousal level (calm ↔ energetic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArousalLevel {
    /// Extremely calm and slow.
    VeryCalm,
    /// Calm / relaxed.
    Calm,
    /// Moderate energy level.
    Moderate,
    /// High energy level.
    Energetic,
    /// Very high energy level.
    VeryEnergetic,
}

impl ArousalLevel {
    /// Approximate BPM range `(min, max)` that typically corresponds to this arousal level.
    #[must_use]
    pub fn bpm_range(&self) -> (f32, f32) {
        match self {
            Self::VeryCalm => (40.0, 70.0),
            Self::Calm => (70.0, 95.0),
            Self::Moderate => (95.0, 120.0),
            Self::Energetic => (120.0, 160.0),
            Self::VeryEnergetic => (160.0, 240.0),
        }
    }
}

// ── MoodVector ────────────────────────────────────────────────────────────────

/// A 2-D mood vector in the valence/arousal emotion space.
#[derive(Debug, Clone)]
pub struct MoodVector {
    /// Valence (negative ↔ positive).
    pub valence: ValenceLevel,
    /// Arousal (calm ↔ energetic).
    pub arousal: ArousalLevel,
    /// Classifier confidence in \[0.0, 1.0\].
    pub confidence: f32,
}

impl MoodVector {
    /// Returns `true` if the classifier confidence exceeds the threshold `t`.
    #[must_use]
    pub fn is_confident(&self, t: f32) -> bool {
        self.confidence > t
    }

    /// Returns the emotional quadrant label.
    ///
    /// * "happy"  – positive valence + high arousal
    /// * "calm"   – positive valence + low arousal
    /// * "angry"  – negative valence + high arousal
    /// * "sad"    – negative valence + low arousal
    #[must_use]
    pub fn quadrant(&self) -> &str {
        let positive_valence = matches!(
            self.valence,
            ValenceLevel::Positive | ValenceLevel::VeryPositive
        );
        let high_arousal = matches!(
            self.arousal,
            ArousalLevel::Energetic | ArousalLevel::VeryEnergetic
        );

        match (positive_valence, high_arousal) {
            (true, true) => "happy",
            (true, false) => "calm",
            (false, true) => "angry",
            (false, false) => "sad",
        }
    }
}

// ── MoodClassifier ────────────────────────────────────────────────────────────

/// Heuristic mood classifier based on low-level audio features.
#[derive(Debug, Clone)]
pub struct MoodClassifier {
    /// Estimated tempo in beats per minute.
    pub tempo_bpm: f32,
    /// Spectral centroid in Hz (brightness indicator).
    pub spectral_centroid: f32,
    /// RMS energy of the audio (loudness proxy).
    pub energy: f32,
}

impl MoodClassifier {
    /// Classify the mood based on tempo, spectral centroid, and energy.
    ///
    /// Heuristic rules:
    /// - Arousal is mapped from tempo (slow → calm, fast → energetic).
    /// - Valence is estimated from spectral centroid (dark → negative, bright → positive).
    /// - Confidence is proportional to how well the features agree with the heuristics.
    #[must_use]
    pub fn classify(&self) -> MoodVector {
        // --- Arousal from tempo ---
        let arousal = if self.tempo_bpm < 70.0 {
            ArousalLevel::VeryCalm
        } else if self.tempo_bpm < 95.0 {
            ArousalLevel::Calm
        } else if self.tempo_bpm < 120.0 {
            ArousalLevel::Moderate
        } else if self.tempo_bpm < 160.0 {
            ArousalLevel::Energetic
        } else {
            ArousalLevel::VeryEnergetic
        };

        // --- Valence from spectral centroid (bright ↔ positive) ---
        let valence = if self.spectral_centroid < 1000.0 {
            ValenceLevel::VeryNegative
        } else if self.spectral_centroid < 2000.0 {
            ValenceLevel::Negative
        } else if self.spectral_centroid < 3500.0 {
            ValenceLevel::Neutral
        } else if self.spectral_centroid < 5000.0 {
            ValenceLevel::Positive
        } else {
            ValenceLevel::VeryPositive
        };

        // --- Confidence: higher when energy is meaningful ---
        let confidence = (self.energy.abs().min(1.0) * 0.8 + 0.2).clamp(0.0, 1.0);

        MoodVector {
            valence,
            arousal,
            confidence,
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ValenceLevel ───────────────────────────────────────────────────────────

    #[test]
    fn test_valence_very_negative_score() {
        assert!((ValenceLevel::VeryNegative.score() - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn test_valence_very_positive_score() {
        assert!((ValenceLevel::VeryPositive.score() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_valence_neutral_score() {
        assert!((ValenceLevel::Neutral.score() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_valence_negative_score() {
        assert!((ValenceLevel::Negative.score() - (-0.5)).abs() < 1e-5);
    }

    #[test]
    fn test_valence_positive_score() {
        assert!((ValenceLevel::Positive.score() - 0.5).abs() < 1e-5);
    }

    // ── ArousalLevel ───────────────────────────────────────────────────────────

    #[test]
    fn test_arousal_very_calm_bpm_range() {
        let (lo, hi) = ArousalLevel::VeryCalm.bpm_range();
        assert!(lo < hi);
        assert!((lo - 40.0).abs() < 1e-5);
    }

    #[test]
    fn test_arousal_very_energetic_bpm_range() {
        let (lo, hi) = ArousalLevel::VeryEnergetic.bpm_range();
        assert!(lo >= 160.0);
        assert!(hi > lo);
    }

    #[test]
    fn test_arousal_moderate_bpm_range_includes_110() {
        let (lo, hi) = ArousalLevel::Moderate.bpm_range();
        assert!(110.0 > lo && 110.0 < hi);
    }

    // ── MoodVector ─────────────────────────────────────────────────────────────

    #[test]
    fn test_mood_vector_is_confident_above_threshold() {
        let mv = MoodVector {
            valence: ValenceLevel::Positive,
            arousal: ArousalLevel::Energetic,
            confidence: 0.85,
        };
        assert!(mv.is_confident(0.8));
    }

    #[test]
    fn test_mood_vector_not_confident_below_threshold() {
        let mv = MoodVector {
            valence: ValenceLevel::Neutral,
            arousal: ArousalLevel::Moderate,
            confidence: 0.4,
        };
        assert!(!mv.is_confident(0.6));
    }

    #[test]
    fn test_quadrant_happy() {
        let mv = MoodVector {
            valence: ValenceLevel::VeryPositive,
            arousal: ArousalLevel::VeryEnergetic,
            confidence: 0.9,
        };
        assert_eq!(mv.quadrant(), "happy");
    }

    #[test]
    fn test_quadrant_calm() {
        let mv = MoodVector {
            valence: ValenceLevel::Positive,
            arousal: ArousalLevel::Calm,
            confidence: 0.7,
        };
        assert_eq!(mv.quadrant(), "calm");
    }

    #[test]
    fn test_quadrant_angry() {
        let mv = MoodVector {
            valence: ValenceLevel::VeryNegative,
            arousal: ArousalLevel::Energetic,
            confidence: 0.75,
        };
        assert_eq!(mv.quadrant(), "angry");
    }

    #[test]
    fn test_quadrant_sad() {
        let mv = MoodVector {
            valence: ValenceLevel::Negative,
            arousal: ArousalLevel::VeryCalm,
            confidence: 0.65,
        };
        assert_eq!(mv.quadrant(), "sad");
    }

    // ── MoodClassifier ─────────────────────────────────────────────────────────

    #[test]
    fn test_classify_fast_bright_is_happy() {
        let clf = MoodClassifier {
            tempo_bpm: 140.0,
            spectral_centroid: 4500.0,
            energy: 0.8,
        };
        let mood = clf.classify();
        assert_eq!(mood.quadrant(), "happy");
    }

    #[test]
    fn test_classify_slow_dark_is_sad() {
        let clf = MoodClassifier {
            tempo_bpm: 55.0,
            spectral_centroid: 800.0,
            energy: 0.3,
        };
        let mood = clf.classify();
        assert_eq!(mood.quadrant(), "sad");
    }

    #[test]
    fn test_classify_confidence_nonnegative() {
        let clf = MoodClassifier {
            tempo_bpm: 120.0,
            spectral_centroid: 3000.0,
            energy: 0.5,
        };
        let mood = clf.classify();
        assert!(mood.confidence >= 0.0);
        assert!(mood.confidence <= 1.0);
    }
}
