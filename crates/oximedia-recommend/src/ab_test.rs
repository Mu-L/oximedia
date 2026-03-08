//! A/B testing framework for recommendation algorithms.
//!
//! This module provides the ability to run controlled experiments
//! comparing different recommendation variants (algorithms, parameters,
//! models) and determine statistically significant winners.

#![allow(dead_code)]

use std::collections::HashMap;
use uuid::Uuid;

/// Configuration for a single variant in an A/B test.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VariantConfig {
    /// Unique identifier for this variant
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Traffic allocation fraction (0.0-1.0)
    pub traffic_fraction: f64,
    /// Variant-specific parameters
    pub params: HashMap<String, String>,
    /// Whether this is the control group
    pub is_control: bool,
}

impl VariantConfig {
    /// Creates a new variant config.
    #[must_use]
    pub fn new(id: &str, name: &str, traffic_fraction: f64) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            traffic_fraction,
            params: HashMap::new(),
            is_control: false,
        }
    }

    /// Marks this variant as the control group.
    #[must_use]
    pub fn as_control(mut self) -> Self {
        self.is_control = true;
        self
    }

    /// Adds a parameter to the variant.
    #[must_use]
    pub fn with_param(mut self, key: &str, value: &str) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }
}

/// Metric observation for a variant.
#[derive(Debug, Clone)]
struct Observation {
    /// Value observed (e.g., click-through rate, watch time)
    value: f64,
    /// Timestamp of the observation
    _timestamp: i64,
}

/// Tracks metrics and outcomes for an A/B test.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AbTestResult {
    /// Variant ID
    pub variant_id: String,
    /// Number of impressions (users exposed)
    pub impressions: u64,
    /// Number of conversions (clicks, watches, etc.)
    pub conversions: u64,
    /// Sum of metric values
    pub metric_sum: f64,
    /// Sum of squared metric values (for variance calculation)
    pub metric_sum_sq: f64,
}

impl AbTestResult {
    /// Creates a new empty result for a variant.
    #[must_use]
    pub fn new(variant_id: &str) -> Self {
        Self {
            variant_id: variant_id.to_string(),
            impressions: 0,
            conversions: 0,
            metric_sum: 0.0,
            metric_sum_sq: 0.0,
        }
    }

    /// Records an impression (user was shown this variant).
    pub fn record_impression(&mut self) {
        self.impressions += 1;
    }

    /// Records a conversion with a metric value.
    pub fn record_conversion(&mut self, value: f64) {
        self.conversions += 1;
        self.metric_sum += value;
        self.metric_sum_sq += value * value;
    }

    /// Computes the conversion rate.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn conversion_rate(&self) -> f64 {
        if self.impressions == 0 {
            return 0.0;
        }
        self.conversions as f64 / self.impressions as f64
    }

    /// Computes the mean metric value among conversions.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn mean_metric(&self) -> f64 {
        if self.conversions == 0 {
            return 0.0;
        }
        self.metric_sum / self.conversions as f64
    }

    /// Computes the variance of the metric values.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn metric_variance(&self) -> f64 {
        if self.conversions < 2 {
            return 0.0;
        }
        let n = self.conversions as f64;
        let mean = self.mean_metric();
        (self.metric_sum_sq / n) - (mean * mean)
    }

    /// Computes the standard error of the mean metric.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn standard_error(&self) -> f64 {
        if self.conversions < 2 {
            return 0.0;
        }
        (self.metric_variance() / self.conversions as f64).sqrt()
    }
}

/// Status of an A/B test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AbTestStatus {
    /// Test is configured but not yet running
    Draft,
    /// Test is actively collecting data
    Running,
    /// Test is paused
    Paused,
    /// Test has concluded
    Completed,
}

/// An A/B test comparing recommendation variants.
#[derive(Debug)]
pub struct AbTest {
    /// Unique test identifier
    pub id: Uuid,
    /// Human-readable name
    pub name: String,
    /// Test status
    pub status: AbTestStatus,
    /// Variants being tested
    variants: Vec<VariantConfig>,
    /// Results per variant
    results: HashMap<String, AbTestResult>,
    /// User-to-variant assignment (sticky)
    assignments: HashMap<Uuid, String>,
    /// Minimum observations per variant to declare a winner
    min_observations: u64,
    /// Significance level (e.g., 0.05 for 95% confidence)
    significance_level: f64,
}

impl AbTest {
    /// Creates a new A/B test.
    #[must_use]
    pub fn new(name: &str, variants: Vec<VariantConfig>) -> Self {
        let mut results = HashMap::new();
        for v in &variants {
            results.insert(v.id.clone(), AbTestResult::new(&v.id));
        }
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            status: AbTestStatus::Draft,
            variants,
            results,
            assignments: HashMap::new(),
            min_observations: 100,
            significance_level: 0.05,
        }
    }

    /// Sets the minimum observations needed per variant.
    #[must_use]
    pub fn with_min_observations(mut self, min: u64) -> Self {
        self.min_observations = min;
        self
    }

    /// Sets the significance level.
    #[must_use]
    pub fn with_significance_level(mut self, level: f64) -> Self {
        self.significance_level = level;
        self
    }

    /// Starts the test.
    pub fn start(&mut self) {
        self.status = AbTestStatus::Running;
    }

    /// Pauses the test.
    pub fn pause(&mut self) {
        self.status = AbTestStatus::Paused;
    }

    /// Completes the test.
    pub fn complete(&mut self) {
        self.status = AbTestStatus::Completed;
    }

    /// Assigns a user to a variant (sticky assignment).
    ///
    /// Uses deterministic hashing so the same user always gets
    /// the same variant for this test.
    #[must_use]
    pub fn assign_variant(&mut self, user_id: Uuid) -> &str {
        if let Some(variant_id) = self.assignments.get(&user_id) {
            // Find the variant and return a reference to variants vec
            for v in &self.variants {
                if v.id == *variant_id {
                    return &v.id;
                }
            }
        }

        // Deterministic assignment based on user_id hash
        let hash = Self::hash_user(user_id);
        let mut cumulative = 0.0_f64;
        let mut assigned_idx = 0;
        for (i, v) in self.variants.iter().enumerate() {
            cumulative += v.traffic_fraction;
            if hash < cumulative {
                assigned_idx = i;
                break;
            }
            if i == self.variants.len() - 1 {
                assigned_idx = i;
            }
        }

        let variant_id = self.variants[assigned_idx].id.clone();
        self.assignments.insert(user_id, variant_id);
        &self.variants[assigned_idx].id
    }

    /// Records an impression for a variant.
    pub fn record_impression(&mut self, variant_id: &str) {
        if let Some(result) = self.results.get_mut(variant_id) {
            result.record_impression();
        }
    }

    /// Records a conversion for a variant.
    pub fn record_conversion(&mut self, variant_id: &str, metric_value: f64) {
        if let Some(result) = self.results.get_mut(variant_id) {
            result.record_conversion(metric_value);
        }
    }

    /// Gets results for a specific variant.
    #[must_use]
    pub fn get_result(&self, variant_id: &str) -> Option<&AbTestResult> {
        self.results.get(variant_id)
    }

    /// Returns all variant results.
    #[must_use]
    pub fn all_results(&self) -> &HashMap<String, AbTestResult> {
        &self.results
    }

    /// Determines the winner among variants, if any.
    ///
    /// Returns `None` if not enough data or no statistically significant winner.
    /// Returns `Some(variant_id)` of the best-performing variant.
    #[must_use]
    pub fn winner(&self) -> Option<String> {
        // Check minimum observations
        for result in self.results.values() {
            if result.impressions < self.min_observations {
                return None;
            }
        }

        // Find the control variant
        let control = self.variants.iter().find(|v| v.is_control)?;
        let control_result = self.results.get(&control.id)?;
        let control_rate = control_result.conversion_rate();

        let mut best_variant: Option<String> = None;
        let mut best_lift = 0.0_f64;

        for v in &self.variants {
            if v.is_control {
                continue;
            }
            let result = self.results.get(&v.id)?;
            let variant_rate = result.conversion_rate();
            let lift = variant_rate - control_rate;

            // Simple z-test for proportions
            if self.is_significant(control_result, result) && lift > best_lift {
                best_lift = lift;
                best_variant = Some(v.id.clone());
            }
        }

        // If no treatment beats control significantly, control wins if it has data
        if best_variant.is_none() && control_result.impressions >= self.min_observations {
            return Some(control.id.clone());
        }

        best_variant
    }

    /// Performs a z-test for two proportions.
    #[allow(clippy::cast_precision_loss)]
    fn is_significant(&self, control: &AbTestResult, treatment: &AbTestResult) -> bool {
        let n1 = control.impressions as f64;
        let n2 = treatment.impressions as f64;
        if n1 == 0.0 || n2 == 0.0 {
            return false;
        }
        let p1 = control.conversion_rate();
        let p2 = treatment.conversion_rate();
        let p_pool = (control.conversions as f64 + treatment.conversions as f64) / (n1 + n2);

        if p_pool <= 0.0 || p_pool >= 1.0 {
            return false;
        }

        let se = (p_pool * (1.0 - p_pool) * (1.0 / n1 + 1.0 / n2)).sqrt();
        if se == 0.0 {
            return false;
        }

        let z = (p2 - p1).abs() / se;

        // z > 1.96 for ~95% confidence (two-tailed)
        let z_threshold = match () {
            () if self.significance_level <= 0.01 => 2.576,
            () if self.significance_level <= 0.05 => 1.960,
            () if self.significance_level <= 0.10 => 1.645,
            () => 1.282,
        };

        z > z_threshold
    }

    /// Simple hash of user ID to a value in [0, 1).
    #[allow(clippy::cast_precision_loss)]
    fn hash_user(user_id: Uuid) -> f64 {
        let bytes = user_id.as_bytes();
        let mut hash: u64 = 0;
        for &b in bytes {
            hash = hash.wrapping_mul(31).wrapping_add(u64::from(b));
        }
        (hash % 10000) as f64 / 10000.0
    }

    /// Returns the number of variants.
    #[must_use]
    pub fn variant_count(&self) -> usize {
        self.variants.len()
    }

    /// Returns total impressions across all variants.
    #[must_use]
    pub fn total_impressions(&self) -> u64 {
        self.results.values().map(|r| r.impressions).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_variants() -> Vec<VariantConfig> {
        vec![
            VariantConfig::new("control", "Control", 0.5).as_control(),
            VariantConfig::new("treatment", "Treatment A", 0.5),
        ]
    }

    #[test]
    fn test_variant_config_creation() {
        let v = VariantConfig::new("v1", "Variant 1", 0.5);
        assert_eq!(v.id, "v1");
        assert!((v.traffic_fraction - 0.5).abs() < f64::EPSILON);
        assert!(!v.is_control);
    }

    #[test]
    fn test_variant_config_as_control() {
        let v = VariantConfig::new("ctrl", "Control", 0.5).as_control();
        assert!(v.is_control);
    }

    #[test]
    fn test_variant_config_with_param() {
        let v = VariantConfig::new("v1", "V1", 0.5)
            .with_param("model", "v2")
            .with_param("threshold", "0.7");
        assert_eq!(v.params.len(), 2);
        assert_eq!(v.params.get("model").expect("should succeed in test"), "v2");
    }

    #[test]
    fn test_ab_test_result_empty() {
        let r = AbTestResult::new("v1");
        assert_eq!(r.impressions, 0);
        assert!((r.conversion_rate() - 0.0).abs() < f64::EPSILON);
        assert!((r.mean_metric() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ab_test_result_conversion_rate() {
        let mut r = AbTestResult::new("v1");
        for _ in 0..100 {
            r.record_impression();
        }
        for _ in 0..25 {
            r.record_conversion(1.0);
        }
        assert!((r.conversion_rate() - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ab_test_result_mean_metric() {
        let mut r = AbTestResult::new("v1");
        r.record_conversion(2.0);
        r.record_conversion(4.0);
        r.record_conversion(6.0);
        assert!((r.mean_metric() - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ab_test_result_variance() {
        let mut r = AbTestResult::new("v1");
        r.record_conversion(10.0);
        r.record_conversion(10.0);
        r.record_conversion(10.0);
        assert!(r.metric_variance().abs() < f64::EPSILON);
    }

    #[test]
    fn test_ab_test_creation() {
        let test = AbTest::new("Test 1", make_variants());
        assert_eq!(test.variant_count(), 2);
        assert_eq!(test.status, AbTestStatus::Draft);
    }

    #[test]
    fn test_ab_test_lifecycle() {
        let mut test = AbTest::new("Test 1", make_variants());
        assert_eq!(test.status, AbTestStatus::Draft);
        test.start();
        assert_eq!(test.status, AbTestStatus::Running);
        test.pause();
        assert_eq!(test.status, AbTestStatus::Paused);
        test.complete();
        assert_eq!(test.status, AbTestStatus::Completed);
    }

    #[test]
    fn test_ab_test_assign_variant_sticky() {
        let mut test = AbTest::new("Test 1", make_variants());
        let u = Uuid::new_v4();
        let v1 = test.assign_variant(u).to_string();
        let v2 = test.assign_variant(u).to_string();
        assert_eq!(v1, v2, "same user should get same variant");
    }

    #[test]
    fn test_ab_test_record_and_get_result() {
        let mut test = AbTest::new("Test 1", make_variants());
        test.record_impression("control");
        test.record_impression("control");
        test.record_conversion("control", 1.0);
        let r = test.get_result("control").expect("should succeed in test");
        assert_eq!(r.impressions, 2);
        assert_eq!(r.conversions, 1);
    }

    #[test]
    fn test_ab_test_winner_insufficient_data() {
        let test = AbTest::new("Test 1", make_variants()).with_min_observations(100);
        assert!(test.winner().is_none());
    }

    #[test]
    fn test_ab_test_winner_significant_treatment() {
        let mut test = AbTest::new("Test 1", make_variants()).with_min_observations(10);
        // Control: 10% conversion
        for _ in 0..200 {
            test.record_impression("control");
        }
        for _ in 0..20 {
            test.record_conversion("control", 1.0);
        }
        // Treatment: 30% conversion (clearly better)
        for _ in 0..200 {
            test.record_impression("treatment");
        }
        for _ in 0..60 {
            test.record_conversion("treatment", 1.0);
        }
        let w = test.winner();
        assert_eq!(w, Some("treatment".to_string()));
    }

    #[test]
    fn test_total_impressions() {
        let mut test = AbTest::new("Test 1", make_variants());
        test.record_impression("control");
        test.record_impression("control");
        test.record_impression("treatment");
        assert_eq!(test.total_impressions(), 3);
    }

    #[test]
    fn test_standard_error() {
        let mut r = AbTestResult::new("v1");
        r.record_conversion(10.0);
        r.record_conversion(20.0);
        r.record_conversion(30.0);
        assert!(r.standard_error() > 0.0);
    }
}
