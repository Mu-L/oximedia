//! Composite quality scoring across multiple perceptual dimensions.
//!
//! Provides a weighted multi-dimension quality assessment framework
//! with grade classification from overall numeric scores.

#![allow(dead_code)]

/// A named quality dimension with an associated weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityDimension {
    /// Spatial sharpness and resolution quality.
    Sharpness,
    /// Noise and grain level (inverse: low noise = high quality).
    Noise,
    /// Compression artifact severity (inverse).
    Artifacts,
    /// Color accuracy and saturation quality.
    Color,
    /// Temporal stability and motion smoothness.
    Temporal,
}

impl QualityDimension {
    /// Return the default weight (0.0–1.0) for this dimension.
    ///
    /// Weights express relative importance across dimensions.
    #[must_use]
    pub fn weight(self) -> f32 {
        match self {
            Self::Sharpness => 0.30,
            Self::Noise => 0.20,
            Self::Artifacts => 0.25,
            Self::Color => 0.15,
            Self::Temporal => 0.10,
        }
    }
}

/// A score (0.0–100.0) for a single quality dimension.
#[derive(Debug, Clone, Copy)]
pub struct DimensionScore {
    /// The dimension this score belongs to.
    pub dimension: QualityDimension,
    /// Raw score in range 0.0–100.0.
    pub raw: f32,
}

impl DimensionScore {
    /// Create a new `DimensionScore`, clamping `raw` to 0.0–100.0.
    #[must_use]
    pub fn new(dimension: QualityDimension, raw: f32) -> Self {
        Self {
            dimension,
            raw: raw.clamp(0.0, 100.0),
        }
    }

    /// Compute the weighted contribution of this score.
    ///
    /// `weighted = raw * dimension.weight()`
    #[must_use]
    pub fn weighted(self) -> f32 {
        self.raw * self.dimension.weight()
    }
}

/// A quality scorer that accumulates dimension scores and computes a final score.
#[derive(Debug, Clone, Default)]
pub struct QualityScorer {
    scores: Vec<DimensionScore>,
}

impl QualityScorer {
    /// Create an empty `QualityScorer`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a dimension score (replaces any existing score for the same dimension).
    pub fn add_dimension(&mut self, score: DimensionScore) {
        self.scores.retain(|s| s.dimension != score.dimension);
        self.scores.push(score);
    }

    /// Compute the overall quality score (0.0–100.0).
    ///
    /// Uses weighted sum: `Σ(raw_i * weight_i) / Σ(weight_i)`.
    /// Returns 0.0 if no dimensions have been added.
    #[must_use]
    pub fn overall_score(&self) -> f32 {
        if self.scores.is_empty() {
            return 0.0;
        }
        let weight_sum: f32 = self.scores.iter().map(|s| s.dimension.weight()).sum();
        if weight_sum < f32::EPSILON {
            return 0.0;
        }
        let weighted_sum: f32 = self.scores.iter().map(|s| s.weighted()).sum();
        (weighted_sum / weight_sum).clamp(0.0, 100.0)
    }

    /// Return all recorded dimension scores.
    #[must_use]
    pub fn scores(&self) -> &[DimensionScore] {
        &self.scores
    }
}

/// A human-readable quality grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityGrade {
    /// Excellent: score ≥ 90.
    Excellent,
    /// Good: score ≥ 75.
    Good,
    /// Fair: score ≥ 55.
    Fair,
    /// Poor: score ≥ 30.
    Poor,
    /// Failing: score < 30.
    Failing,
}

impl QualityGrade {
    /// Classify a numeric score (0.0–100.0) into a `QualityGrade`.
    #[must_use]
    pub fn from_score(score: f32) -> Self {
        match score as u32 {
            90..=100 => Self::Excellent,
            75..=89 => Self::Good,
            55..=74 => Self::Fair,
            30..=54 => Self::Poor,
            _ => Self::Failing,
        }
    }

    /// Return a short label string for this grade.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Excellent => "Excellent",
            Self::Good => "Good",
            Self::Fair => "Fair",
            Self::Poor => "Poor",
            Self::Failing => "Failing",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimension_weight_sums_near_one() {
        let sum = QualityDimension::Sharpness.weight()
            + QualityDimension::Noise.weight()
            + QualityDimension::Artifacts.weight()
            + QualityDimension::Color.weight()
            + QualityDimension::Temporal.weight();
        assert!((sum - 1.0).abs() < 1e-5, "weights sum = {sum}");
    }

    #[test]
    fn test_dimension_score_clamps_above_100() {
        let ds = DimensionScore::new(QualityDimension::Sharpness, 150.0);
        assert!((ds.raw - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_dimension_score_clamps_below_0() {
        let ds = DimensionScore::new(QualityDimension::Noise, -10.0);
        assert!(ds.raw.abs() < f32::EPSILON);
    }

    #[test]
    fn test_dimension_score_weighted() {
        let ds = DimensionScore::new(QualityDimension::Sharpness, 100.0);
        assert!((ds.weighted() - 30.0).abs() < 1e-5);
    }

    #[test]
    fn test_quality_scorer_empty_overall() {
        let scorer = QualityScorer::new();
        assert!(scorer.overall_score().abs() < f32::EPSILON);
    }

    #[test]
    fn test_quality_scorer_single_dimension() {
        let mut scorer = QualityScorer::new();
        scorer.add_dimension(DimensionScore::new(QualityDimension::Sharpness, 80.0));
        // Only one dimension: weighted/weight_sum = 80*0.30/0.30 = 80.0
        assert!((scorer.overall_score() - 80.0).abs() < 1e-4);
    }

    #[test]
    fn test_quality_scorer_replace_existing_dimension() {
        let mut scorer = QualityScorer::new();
        scorer.add_dimension(DimensionScore::new(QualityDimension::Sharpness, 50.0));
        scorer.add_dimension(DimensionScore::new(QualityDimension::Sharpness, 90.0));
        assert_eq!(scorer.scores().len(), 1);
        assert!((scorer.scores()[0].raw - 90.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_quality_scorer_all_dimensions_perfect() {
        let mut scorer = QualityScorer::new();
        for dim in [
            QualityDimension::Sharpness,
            QualityDimension::Noise,
            QualityDimension::Artifacts,
            QualityDimension::Color,
            QualityDimension::Temporal,
        ] {
            scorer.add_dimension(DimensionScore::new(dim, 100.0));
        }
        assert!((scorer.overall_score() - 100.0).abs() < 1e-4);
    }

    #[test]
    fn test_quality_grade_excellent() {
        assert_eq!(QualityGrade::from_score(95.0), QualityGrade::Excellent);
    }

    #[test]
    fn test_quality_grade_good() {
        assert_eq!(QualityGrade::from_score(80.0), QualityGrade::Good);
    }

    #[test]
    fn test_quality_grade_fair() {
        assert_eq!(QualityGrade::from_score(60.0), QualityGrade::Fair);
    }

    #[test]
    fn test_quality_grade_poor() {
        assert_eq!(QualityGrade::from_score(40.0), QualityGrade::Poor);
    }

    #[test]
    fn test_quality_grade_failing() {
        assert_eq!(QualityGrade::from_score(10.0), QualityGrade::Failing);
    }

    #[test]
    fn test_quality_grade_label() {
        assert_eq!(QualityGrade::Excellent.label(), "Excellent");
        assert_eq!(QualityGrade::Failing.label(), "Failing");
    }

    #[test]
    fn test_quality_scorer_scores_slice_length() {
        let mut scorer = QualityScorer::new();
        scorer.add_dimension(DimensionScore::new(QualityDimension::Color, 70.0));
        scorer.add_dimension(DimensionScore::new(QualityDimension::Noise, 60.0));
        assert_eq!(scorer.scores().len(), 2);
    }
}
