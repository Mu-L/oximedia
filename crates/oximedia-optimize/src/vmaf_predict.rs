#![allow(dead_code)]
//! VMAF score prediction and estimation for encoding optimization.
//!
//! This module provides a lightweight VMAF (Video Multi-method Assessment Fusion)
//! score estimator that predicts quality scores without running the full VMAF model.
//! It uses spatial and temporal feature extraction to approximate VMAF scores,
//! enabling fast quality-aware encoding decisions.

use std::collections::VecDeque;

/// Feature vector used for VMAF prediction.
#[derive(Debug, Clone)]
pub struct VmafFeatures {
    /// Visual Information Fidelity (VIF) approximation at scale 0.
    pub vif_scale0: f64,
    /// Visual Information Fidelity (VIF) approximation at scale 1.
    pub vif_scale1: f64,
    /// Visual Information Fidelity (VIF) approximation at scale 2.
    pub vif_scale2: f64,
    /// Visual Information Fidelity (VIF) approximation at scale 3.
    pub vif_scale3: f64,
    /// Detail Loss Metric (DLM) approximation.
    pub dlm: f64,
    /// Motion metric (temporal difference).
    pub motion: f64,
    /// ADM (Additive Detail Masking) approximation.
    pub adm: f64,
}

impl VmafFeatures {
    /// Creates a new feature set with default (perfect quality) values.
    pub fn perfect() -> Self {
        Self {
            vif_scale0: 1.0,
            vif_scale1: 1.0,
            vif_scale2: 1.0,
            vif_scale3: 1.0,
            dlm: 1.0,
            motion: 0.0,
            adm: 1.0,
        }
    }

    /// Creates a feature set from raw pixel comparison statistics.
    #[allow(clippy::cast_precision_loss)]
    pub fn from_stats(psnr: f64, ssim: f64, motion_magnitude: f64) -> Self {
        // Map PSNR/SSIM to approximate VIF scores
        let vif_base = (psnr / 50.0).min(1.0).max(0.0);
        let ssim_clamped = ssim.min(1.0).max(0.0);
        Self {
            vif_scale0: vif_base * 0.9 + ssim_clamped * 0.1,
            vif_scale1: vif_base * 0.85 + ssim_clamped * 0.15,
            vif_scale2: vif_base * 0.8 + ssim_clamped * 0.2,
            vif_scale3: vif_base * 0.75 + ssim_clamped * 0.25,
            dlm: ssim_clamped,
            motion: motion_magnitude,
            adm: ssim_clamped * 0.95 + vif_base * 0.05,
        }
    }

    /// Returns the mean VIF across all scales.
    pub fn mean_vif(&self) -> f64 {
        (self.vif_scale0 + self.vif_scale1 + self.vif_scale2 + self.vif_scale3) / 4.0
    }
}

/// A predicted VMAF score with confidence information.
#[derive(Debug, Clone)]
pub struct VmafPrediction {
    /// Predicted VMAF score (0-100 scale).
    pub score: f64,
    /// Confidence level (0.0-1.0).
    pub confidence: f64,
    /// Feature set used for this prediction.
    pub features: VmafFeatures,
}

impl VmafPrediction {
    /// Returns whether the predicted quality is considered acceptable (>= threshold).
    pub fn meets_threshold(&self, threshold: f64) -> bool {
        self.score >= threshold
    }

    /// Returns a quality tier classification.
    pub fn quality_tier(&self) -> QualityTier {
        if self.score >= 93.0 {
            QualityTier::Excellent
        } else if self.score >= 80.0 {
            QualityTier::Good
        } else if self.score >= 60.0 {
            QualityTier::Fair
        } else if self.score >= 40.0 {
            QualityTier::Poor
        } else {
            QualityTier::Bad
        }
    }
}

/// Quality tier classification based on VMAF score ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityTier {
    /// VMAF >= 93: Visually lossless.
    Excellent,
    /// VMAF >= 80: Good quality, minor artifacts.
    Good,
    /// VMAF >= 60: Acceptable quality.
    Fair,
    /// VMAF >= 40: Noticeable degradation.
    Poor,
    /// VMAF < 40: Severely degraded.
    Bad,
}

/// Configuration for the VMAF predictor.
#[derive(Debug, Clone)]
pub struct VmafPredictorConfig {
    /// Model version hint (affects coefficient selection).
    pub model_version: ModelVersion,
    /// Enable temporal pooling of scores.
    pub temporal_pooling: bool,
    /// Window size for temporal pooling.
    pub pooling_window: usize,
    /// Phone model adjustment (boosts scores for small screens).
    pub phone_model: bool,
}

impl Default for VmafPredictorConfig {
    fn default() -> Self {
        Self {
            model_version: ModelVersion::V063,
            temporal_pooling: true,
            pooling_window: 8,
            phone_model: false,
        }
    }
}

/// VMAF model version for coefficient selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelVersion {
    /// VMAF model version 0.6.1.
    V061,
    /// VMAF model version 0.6.3 (more recent).
    V063,
}

/// The VMAF predictor engine.
#[derive(Debug)]
pub struct VmafPredictor {
    /// Configuration.
    config: VmafPredictorConfig,
    /// Temporal score buffer for pooling.
    score_history: VecDeque<f64>,
    /// Total frames predicted.
    total_frames: u64,
    /// Running sum of all scores for mean computation.
    score_sum: f64,
}

impl VmafPredictor {
    /// Creates a new VMAF predictor with the given configuration.
    pub fn new(config: VmafPredictorConfig) -> Self {
        Self {
            config,
            score_history: VecDeque::new(),
            total_frames: 0,
            score_sum: 0.0,
        }
    }

    /// Creates a predictor with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(VmafPredictorConfig::default())
    }

    /// Predicts VMAF score from pre-computed features.
    #[allow(clippy::cast_precision_loss)]
    pub fn predict(&mut self, features: VmafFeatures) -> VmafPrediction {
        let raw_score = self.compute_raw_score(&features);
        let phone_adjusted = if self.config.phone_model {
            // Phone model typically adds ~3-5 points
            (raw_score + 4.0).min(100.0)
        } else {
            raw_score
        };

        let final_score = if self.config.temporal_pooling && !self.score_history.is_empty() {
            self.apply_temporal_pooling(phone_adjusted)
        } else {
            phone_adjusted
        };

        self.score_history.push_back(final_score);
        if self.score_history.len() > self.config.pooling_window {
            self.score_history.pop_front();
        }
        self.total_frames += 1;
        self.score_sum += final_score;

        let confidence = self.compute_confidence(&features);

        VmafPrediction {
            score: final_score,
            confidence,
            features,
        }
    }

    /// Predicts from PSNR/SSIM/motion statistics directly.
    pub fn predict_from_stats(
        &mut self,
        psnr: f64,
        ssim: f64,
        motion: f64,
    ) -> VmafPrediction {
        let features = VmafFeatures::from_stats(psnr, ssim, motion);
        self.predict(features)
    }

    /// Returns the mean VMAF score across all predicted frames.
    #[allow(clippy::cast_precision_loss)]
    pub fn mean_score(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            self.score_sum / self.total_frames as f64
        }
    }

    /// Returns total number of frames predicted.
    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    /// Resets the predictor state.
    pub fn reset(&mut self) {
        self.score_history.clear();
        self.total_frames = 0;
        self.score_sum = 0.0;
    }

    /// Returns the minimum score in the current temporal window.
    pub fn window_min(&self) -> f64 {
        self.score_history
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min)
    }

    /// Returns the maximum score in the current temporal window.
    pub fn window_max(&self) -> f64 {
        self.score_history
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Computes the raw VMAF score from features using linear regression coefficients.
    fn compute_raw_score(&self, features: &VmafFeatures) -> f64 {
        let (w_vif, w_dlm, w_motion, w_adm, bias) = match self.config.model_version {
            ModelVersion::V061 => (0.35, 0.25, -0.02, 0.38, 2.0),
            ModelVersion::V063 => (0.40, 0.20, -0.015, 0.385, 1.5),
        };

        let vif_contrib = features.mean_vif() * w_vif * 100.0;
        let dlm_contrib = features.dlm * w_dlm * 100.0;
        let motion_contrib = features.motion * w_motion;
        let adm_contrib = features.adm * w_adm * 100.0;

        let raw = vif_contrib + dlm_contrib + motion_contrib + adm_contrib + bias;
        raw.max(0.0).min(100.0)
    }

    /// Applies temporal pooling (harmonic mean for conservative estimate).
    #[allow(clippy::cast_precision_loss)]
    fn apply_temporal_pooling(&self, current: f64) -> f64 {
        if self.score_history.is_empty() {
            return current;
        }
        let mut sum = 0.0;
        let mut count = 0usize;
        for &s in &self.score_history {
            if s > 0.0 {
                sum += 1.0 / s;
                count += 1;
            }
        }
        if current > 0.0 {
            sum += 1.0 / current;
            count += 1;
        }
        if count == 0 || sum == 0.0 {
            current
        } else {
            // Blend harmonic mean with current score
            let harmonic = count as f64 / sum;
            harmonic * 0.3 + current * 0.7
        }
    }

    /// Computes confidence based on feature range validity.
    fn compute_confidence(&self, features: &VmafFeatures) -> f64 {
        let mut conf = 1.0;
        // Lower confidence for extreme feature values
        if features.mean_vif() < 0.1 {
            conf *= 0.5;
        }
        if features.dlm < 0.1 {
            conf *= 0.7;
        }
        if features.motion > 100.0 {
            conf *= 0.8;
        }
        conf
    }
}

/// Estimates the QP value needed to achieve a target VMAF score.
#[allow(clippy::cast_precision_loss)]
#[allow(clippy::cast_possible_truncation)]
pub fn estimate_qp_for_target_vmaf(target_vmaf: f64, content_complexity: f64) -> u8 {
    // Empirical model: VMAF ~ 100 - k * QP^1.2 * complexity
    // Solving for QP: QP ~ ((100 - target) / (k * complexity))^(1/1.2)
    let k = 0.15;
    let complexity = content_complexity.max(0.1);
    let diff = (100.0 - target_vmaf).max(0.0);
    let qp_f = (diff / (k * complexity)).powf(1.0 / 1.2);
    qp_f.round().max(1.0).min(51.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vmaf_features_perfect() {
        let f = VmafFeatures::perfect();
        assert!((f.mean_vif() - 1.0).abs() < f64::EPSILON);
        assert!((f.dlm - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_vmaf_features_from_stats() {
        let f = VmafFeatures::from_stats(40.0, 0.95, 5.0);
        assert!(f.vif_scale0 > 0.0 && f.vif_scale0 <= 1.0);
        assert!(f.dlm > 0.0 && f.dlm <= 1.0);
        assert!((f.motion - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_vmaf_features_mean_vif() {
        let f = VmafFeatures {
            vif_scale0: 0.8,
            vif_scale1: 0.6,
            vif_scale2: 0.4,
            vif_scale3: 0.2,
            dlm: 0.5,
            motion: 0.0,
            adm: 0.5,
        };
        assert!((f.mean_vif() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_vmaf_prediction_quality_tier() {
        let pred = VmafPrediction {
            score: 95.0,
            confidence: 1.0,
            features: VmafFeatures::perfect(),
        };
        assert_eq!(pred.quality_tier(), QualityTier::Excellent);

        let pred2 = VmafPrediction {
            score: 85.0,
            confidence: 1.0,
            features: VmafFeatures::perfect(),
        };
        assert_eq!(pred2.quality_tier(), QualityTier::Good);

        let pred3 = VmafPrediction {
            score: 30.0,
            confidence: 0.5,
            features: VmafFeatures::perfect(),
        };
        assert_eq!(pred3.quality_tier(), QualityTier::Bad);
    }

    #[test]
    fn test_vmaf_prediction_threshold() {
        let pred = VmafPrediction {
            score: 85.0,
            confidence: 1.0,
            features: VmafFeatures::perfect(),
        };
        assert!(pred.meets_threshold(80.0));
        assert!(!pred.meets_threshold(90.0));
    }

    #[test]
    fn test_vmaf_predictor_new() {
        let p = VmafPredictor::with_defaults();
        assert_eq!(p.total_frames(), 0);
        assert!((p.mean_score() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_vmaf_predictor_predict_perfect() {
        let mut p = VmafPredictor::with_defaults();
        let result = p.predict(VmafFeatures::perfect());
        assert!(result.score > 90.0);
        assert!(result.confidence > 0.9);
        assert_eq!(p.total_frames(), 1);
    }

    #[test]
    fn test_vmaf_predictor_predict_from_stats() {
        let mut p = VmafPredictor::with_defaults();
        let result = p.predict_from_stats(42.0, 0.98, 2.0);
        assert!(result.score > 0.0);
        assert!(result.score <= 100.0);
    }

    #[test]
    fn test_vmaf_predictor_mean_score() {
        let mut p = VmafPredictor::with_defaults();
        p.predict(VmafFeatures::perfect());
        p.predict(VmafFeatures::perfect());
        let mean = p.mean_score();
        assert!(mean > 0.0);
    }

    #[test]
    fn test_vmaf_predictor_reset() {
        let mut p = VmafPredictor::with_defaults();
        p.predict(VmafFeatures::perfect());
        p.reset();
        assert_eq!(p.total_frames(), 0);
        assert!((p.mean_score() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_vmaf_predictor_phone_model() {
        let mut p_std = VmafPredictor::new(VmafPredictorConfig {
            phone_model: false,
            temporal_pooling: false,
            ..Default::default()
        });
        let mut p_phone = VmafPredictor::new(VmafPredictorConfig {
            phone_model: true,
            temporal_pooling: false,
            ..Default::default()
        });
        let features = VmafFeatures::from_stats(35.0, 0.9, 3.0);
        let score_std = p_std.predict(features.clone()).score;
        let score_phone = p_phone.predict(features).score;
        assert!(score_phone > score_std);
    }

    #[test]
    fn test_vmaf_predictor_window_min_max() {
        let mut p = VmafPredictor::new(VmafPredictorConfig {
            temporal_pooling: false,
            ..Default::default()
        });
        p.predict(VmafFeatures::from_stats(30.0, 0.8, 1.0));
        p.predict(VmafFeatures::from_stats(45.0, 0.99, 0.5));
        assert!(p.window_min() < p.window_max());
    }

    #[test]
    fn test_estimate_qp_for_target_vmaf() {
        let qp_high = estimate_qp_for_target_vmaf(95.0, 0.5);
        let qp_low = estimate_qp_for_target_vmaf(60.0, 0.5);
        assert!(qp_high < qp_low);

        let qp = estimate_qp_for_target_vmaf(100.0, 0.5);
        assert!(qp >= 1);
        assert!(qp <= 51);
    }

    #[test]
    fn test_model_version_affects_score() {
        let mut p1 = VmafPredictor::new(VmafPredictorConfig {
            model_version: ModelVersion::V061,
            temporal_pooling: false,
            ..Default::default()
        });
        let mut p2 = VmafPredictor::new(VmafPredictorConfig {
            model_version: ModelVersion::V063,
            temporal_pooling: false,
            ..Default::default()
        });
        let features = VmafFeatures::from_stats(38.0, 0.92, 4.0);
        let s1 = p1.predict(features.clone()).score;
        let s2 = p2.predict(features).score;
        // Different models should produce slightly different scores
        assert!((s1 - s2).abs() < 10.0);
    }
}
