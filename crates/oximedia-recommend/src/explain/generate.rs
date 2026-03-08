//! Explanation generation.

use crate::error::RecommendResult;
use crate::Recommendation;

/// Generate explanation for a recommendation
///
/// # Errors
///
/// Returns an error if explanation generation fails
pub fn generate_explanation(recommendation: &Recommendation) -> RecommendResult<String> {
    let builder = super::reason::ExplanationBuilder::new();

    if recommendation.reasons.is_empty() {
        return Ok(String::from("Recommended for you"));
    }

    Ok(builder.combine_reasons(&recommendation.reasons))
}

/// Detailed explanation generator
pub struct DetailedExplanationGenerator {
    /// Include technical details
    include_technical: bool,
}

impl DetailedExplanationGenerator {
    /// Create a new detailed explanation generator
    #[must_use]
    pub fn new(include_technical: bool) -> Self {
        Self { include_technical }
    }

    /// Generate detailed explanation
    #[must_use]
    pub fn generate(&self, recommendation: &Recommendation) -> String {
        let mut explanation = String::new();

        // Add title
        explanation.push_str(&format!(
            "\"{}\" recommended because:\n\n",
            recommendation.metadata.title
        ));

        // Add reasons
        for (idx, reason) in recommendation.reasons.iter().enumerate() {
            let builder = super::reason::ExplanationBuilder::new();
            let reason_text = builder.build_explanation(reason);
            explanation.push_str(&format!("{}. {}\n", idx + 1, reason_text));
        }

        // Add technical details if requested
        if self.include_technical {
            explanation.push_str(&format!("\nRelevance Score: {:.2}\n", recommendation.score));
            explanation.push_str(&format!("Rank: #{}\n", recommendation.rank));
        }

        explanation
    }

    /// Generate concise explanation
    #[must_use]
    pub fn generate_concise(&self, recommendation: &Recommendation) -> String {
        if recommendation.reasons.is_empty() {
            return String::from("Recommended for you");
        }

        let builder = super::reason::ExplanationBuilder::new();
        let primary_reason = builder.build_explanation(&recommendation.reasons[0]);

        if recommendation.reasons.len() > 1 {
            format!(
                "{} and {} more reasons",
                primary_reason,
                recommendation.reasons.len() - 1
            )
        } else {
            primary_reason
        }
    }
}

impl Default for DetailedExplanationGenerator {
    fn default() -> Self {
        Self::new(false)
    }
}

/// Explanation style
#[derive(Debug, Clone, Copy)]
pub enum ExplanationStyle {
    /// Brief, single-line explanation
    Brief,
    /// Detailed, multi-line explanation
    Detailed,
    /// Technical explanation with scores
    Technical,
}

/// Configurable explanation generator
pub struct ExplanationGenerator {
    /// Style of explanations
    style: ExplanationStyle,
    /// Detailed generator
    detailed_generator: DetailedExplanationGenerator,
}

impl ExplanationGenerator {
    /// Create a new explanation generator
    #[must_use]
    pub fn new(style: ExplanationStyle) -> Self {
        let include_technical = matches!(style, ExplanationStyle::Technical);

        Self {
            style,
            detailed_generator: DetailedExplanationGenerator::new(include_technical),
        }
    }

    /// Generate explanation based on configured style
    #[must_use]
    pub fn generate(&self, recommendation: &Recommendation) -> String {
        match self.style {
            ExplanationStyle::Brief => self.detailed_generator.generate_concise(recommendation),
            ExplanationStyle::Detailed | ExplanationStyle::Technical => {
                self.detailed_generator.generate(recommendation)
            }
        }
    }
}

impl Default for ExplanationGenerator {
    fn default() -> Self {
        Self::new(ExplanationStyle::Brief)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContentMetadata, RecommendationReason};
    use uuid::Uuid;

    fn create_test_recommendation() -> Recommendation {
        Recommendation {
            content_id: Uuid::new_v4(),
            score: 0.85,
            rank: 1,
            reasons: vec![
                RecommendationReason::SimilarToLiked {
                    content_id: Uuid::new_v4(),
                    similarity: 0.9,
                },
                RecommendationReason::Trending {
                    trending_score: 0.8,
                },
            ],
            metadata: ContentMetadata {
                title: String::from("Test Content"),
                description: None,
                categories: vec![String::from("Action")],
                duration_ms: Some(7200000),
                thumbnail_url: None,
                created_at: 0,
                avg_rating: Some(4.5),
                view_count: 1000,
            },
            explanation: None,
        }
    }

    #[test]
    fn test_generate_explanation() {
        let rec = create_test_recommendation();
        let explanation = generate_explanation(&rec);

        assert!(explanation.is_ok());
        assert!(explanation
            .expect("should succeed in test")
            .contains("Similar to"));
    }

    #[test]
    fn test_detailed_explanation() {
        let generator = DetailedExplanationGenerator::new(false);
        let rec = create_test_recommendation();
        let explanation = generator.generate(&rec);

        assert!(explanation.contains("Test Content"));
        assert!(explanation.contains("recommended because"));
    }

    #[test]
    fn test_concise_explanation() {
        let generator = DetailedExplanationGenerator::new(false);
        let rec = create_test_recommendation();
        let explanation = generator.generate_concise(&rec);

        assert!(explanation.contains("Similar to"));
    }

    #[test]
    fn test_explanation_generator_styles() {
        let rec = create_test_recommendation();

        let brief_gen = ExplanationGenerator::new(ExplanationStyle::Brief);
        let brief = brief_gen.generate(&rec);
        assert!(!brief.is_empty());

        let detailed_gen = ExplanationGenerator::new(ExplanationStyle::Detailed);
        let detailed = detailed_gen.generate(&rec);
        assert!(detailed.len() > brief.len());
    }
}
