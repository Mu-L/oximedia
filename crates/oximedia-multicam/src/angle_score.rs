#![allow(dead_code)]
//! Camera angle scoring for automated selection.
//!
//! Provides `ScoringMetric`, `AngleScore`, and `AngleScorer` to evaluate and
//! rank camera angles based on multiple perceptual criteria.

/// A single criterion used to evaluate a camera angle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScoringMetric {
    /// Sharpness / focus quality.
    Focus,
    /// Correct exposure (not blown or crushed).
    Exposure,
    /// Amount and quality of on-screen motion.
    Motion,
    /// Rule-of-thirds and framing quality.
    Composition,
}

impl ScoringMetric {
    /// Default weight for this metric in a composite score (0.0–1.0).
    #[must_use]
    pub fn weight(&self) -> f32 {
        match self {
            Self::Focus => 0.35,
            Self::Exposure => 0.25,
            Self::Motion => 0.20,
            Self::Composition => 0.20,
        }
    }
}

/// Per-metric score for a single camera angle.
#[derive(Debug, Clone)]
pub struct AngleScore {
    /// The angle index this score belongs to.
    pub angle_index: usize,
    /// Focus score in \[0.0, 1.0\].
    pub focus: f32,
    /// Exposure score in \[0.0, 1.0\].
    pub exposure: f32,
    /// Motion score in \[0.0, 1.0\].
    pub motion: f32,
    /// Composition score in \[0.0, 1.0\].
    pub composition: f32,
}

impl AngleScore {
    /// Create a new `AngleScore` with all metrics set to zero.
    #[must_use]
    pub fn new(angle_index: usize) -> Self {
        Self {
            angle_index,
            focus: 0.0,
            exposure: 0.0,
            motion: 0.0,
            composition: 0.0,
        }
    }

    /// Compute a weighted total score across all metrics.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn total_score(&self) -> f32 {
        self.focus * ScoringMetric::Focus.weight()
            + self.exposure * ScoringMetric::Exposure.weight()
            + self.motion * ScoringMetric::Motion.weight()
            + self.composition * ScoringMetric::Composition.weight()
    }
}

/// Accumulates per-metric data and produces `AngleScore` results.
#[derive(Debug, Default)]
pub struct AngleScorer {
    scores: Vec<AngleScore>,
}

impl AngleScorer {
    /// Create a new, empty scorer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a pre-built `AngleScore`.
    pub fn score_angle(&mut self, score: AngleScore) {
        self.scores.push(score);
    }

    /// Return the index of the angle with the highest total score, or `None`
    /// if no angles have been added.
    #[must_use]
    pub fn best_angle(&self) -> Option<usize> {
        self.scores
            .iter()
            .max_by(|a, b| {
                a.total_score()
                    .partial_cmp(&b.total_score())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|s| s.angle_index)
    }

    /// Return all stored scores.
    #[must_use]
    pub fn scores(&self) -> &[AngleScore] {
        &self.scores
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_weights_sum_to_one() {
        let sum = ScoringMetric::Focus.weight()
            + ScoringMetric::Exposure.weight()
            + ScoringMetric::Motion.weight()
            + ScoringMetric::Composition.weight();
        assert!(
            (sum - 1.0_f32).abs() < 1e-6,
            "weights should sum to 1.0, got {sum}"
        );
    }

    #[test]
    fn test_focus_weight() {
        assert!((ScoringMetric::Focus.weight() - 0.35).abs() < 1e-6);
    }

    #[test]
    fn test_angle_score_zero_total() {
        let s = AngleScore::new(0);
        assert!((s.total_score()).abs() < 1e-6);
    }

    #[test]
    fn test_angle_score_perfect() {
        let s = AngleScore {
            angle_index: 0,
            focus: 1.0,
            exposure: 1.0,
            motion: 1.0,
            composition: 1.0,
        };
        assert!((s.total_score() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_angle_score_partial() {
        let s = AngleScore {
            angle_index: 1,
            focus: 0.8,
            exposure: 0.6,
            motion: 0.5,
            composition: 0.7,
        };
        let expected = 0.8 * 0.35 + 0.6 * 0.25 + 0.5 * 0.20 + 0.7 * 0.20;
        assert!((s.total_score() - expected).abs() < 1e-5);
    }

    #[test]
    fn test_scorer_empty_best_angle() {
        let scorer = AngleScorer::new();
        assert!(scorer.best_angle().is_none());
    }

    #[test]
    fn test_scorer_single_angle() {
        let mut scorer = AngleScorer::new();
        scorer.score_angle(AngleScore {
            angle_index: 2,
            focus: 0.9,
            exposure: 0.9,
            motion: 0.9,
            composition: 0.9,
        });
        assert_eq!(scorer.best_angle(), Some(2));
    }

    #[test]
    fn test_scorer_best_of_two() {
        let mut scorer = AngleScorer::new();
        scorer.score_angle(AngleScore {
            angle_index: 0,
            focus: 0.5,
            exposure: 0.5,
            motion: 0.5,
            composition: 0.5,
        });
        scorer.score_angle(AngleScore {
            angle_index: 1,
            focus: 0.9,
            exposure: 0.9,
            motion: 0.9,
            composition: 0.9,
        });
        assert_eq!(scorer.best_angle(), Some(1));
    }

    #[test]
    fn test_scorer_best_of_three() {
        let mut scorer = AngleScorer::new();
        scorer.score_angle(AngleScore {
            angle_index: 0,
            focus: 0.3,
            exposure: 0.3,
            motion: 0.3,
            composition: 0.3,
        });
        scorer.score_angle(AngleScore {
            angle_index: 1,
            focus: 1.0,
            exposure: 1.0,
            motion: 1.0,
            composition: 1.0,
        });
        scorer.score_angle(AngleScore {
            angle_index: 2,
            focus: 0.7,
            exposure: 0.7,
            motion: 0.7,
            composition: 0.7,
        });
        assert_eq!(scorer.best_angle(), Some(1));
    }

    #[test]
    fn test_scorer_scores_accessor() {
        let mut scorer = AngleScorer::new();
        scorer.score_angle(AngleScore::new(0));
        scorer.score_angle(AngleScore::new(1));
        assert_eq!(scorer.scores().len(), 2);
    }

    #[test]
    fn test_angle_index_preserved() {
        let s = AngleScore::new(7);
        assert_eq!(s.angle_index, 7);
    }

    #[test]
    fn test_composition_metric_weight() {
        assert!((ScoringMetric::Composition.weight() - 0.20).abs() < 1e-6);
    }
}
