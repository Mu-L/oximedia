//! Diversity enforcement for recommendations.

use crate::error::RecommendResult;
use crate::{DiversitySettings, Recommendation};
use std::collections::{HashMap, HashSet};

/// Diversity enforcer
pub struct DiversityEnforcer {
    /// Maximum items per category
    max_per_category: usize,
}

impl DiversityEnforcer {
    /// Create a new diversity enforcer
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_per_category: 3,
        }
    }

    /// Enforce diversity on recommendations
    ///
    /// # Errors
    ///
    /// Returns an error if enforcement fails
    pub fn enforce_diversity(
        &self,
        recommendations: Vec<Recommendation>,
        settings: &DiversitySettings,
    ) -> RecommendResult<Vec<Recommendation>> {
        if !settings.enabled {
            return Ok(recommendations);
        }

        let mut diverse_recommendations = Vec::new();
        let mut category_counts: HashMap<String, usize> = HashMap::new();

        for rec in recommendations {
            let categories = &rec.metadata.categories;

            // Check if adding this recommendation would violate diversity constraints
            let mut can_add = true;
            for category in categories {
                let count = category_counts.get(category).unwrap_or(&0);
                if *count >= self.max_per_category {
                    can_add = false;
                    break;
                }
            }

            if can_add {
                // Update category counts
                for category in categories {
                    *category_counts.entry(category.clone()).or_insert(0) += 1;
                }
                diverse_recommendations.push(rec);
            }
        }

        // Re-rank after diversity enforcement
        for (idx, rec) in diverse_recommendations.iter_mut().enumerate() {
            rec.rank = idx + 1;
        }

        Ok(diverse_recommendations)
    }

    /// Calculate diversity score
    #[must_use]
    pub fn calculate_diversity_score(recommendations: &[Recommendation]) -> f32 {
        if recommendations.is_empty() {
            return 0.0;
        }

        let mut all_categories = HashSet::new();
        let mut total_categories = 0;

        for rec in recommendations {
            for category in &rec.metadata.categories {
                all_categories.insert(category.clone());
                total_categories += 1;
            }
        }

        if total_categories == 0 {
            return 0.0;
        }

        all_categories.len() as f32 / total_categories as f32
    }
}

impl Default for DiversityEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

/// Maximum Marginal Relevance (MMR) for diversity
pub struct MaximumMarginalRelevance {
    /// Lambda parameter (0-1) for relevance vs diversity tradeoff
    lambda: f32,
}

impl MaximumMarginalRelevance {
    /// Create a new MMR calculator
    #[must_use]
    pub fn new(lambda: f32) -> Self {
        Self {
            lambda: lambda.clamp(0.0, 1.0),
        }
    }

    /// Calculate MMR score
    #[must_use]
    pub fn calculate_score(&self, relevance: f32, max_similarity: f32) -> f32 {
        self.lambda * relevance - (1.0 - self.lambda) * max_similarity
    }
}

impl Default for MaximumMarginalRelevance {
    fn default() -> Self {
        Self::new(0.7) // Favor relevance slightly over diversity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diversity_enforcer() {
        let enforcer = DiversityEnforcer::new();
        assert_eq!(enforcer.max_per_category, 3);
    }

    #[test]
    fn test_mmr() {
        let mmr = MaximumMarginalRelevance::new(0.7);
        let score = mmr.calculate_score(0.9, 0.5);
        assert!(score > 0.0);
    }
}
