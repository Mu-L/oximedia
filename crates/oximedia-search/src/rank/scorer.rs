//! Relevance scoring using BM25 and TF-IDF.

use crate::SearchResultItem;

/// Relevance scorer
pub struct RelevanceScorer {
    /// BM25 k1 parameter
    k1: f32,
    /// BM25 b parameter
    b: f32,
}

impl RelevanceScorer {
    /// Create a new relevance scorer
    #[must_use]
    pub const fn new() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }

    /// Score search results using BM25
    pub fn score_bm25(&self, results: &mut [SearchResultItem], _avg_doc_length: f32) {
        // Placeholder BM25 scoring
        for result in results {
            result.score *= self.k1;
        }
    }

    /// Apply TF-IDF scoring
    pub fn score_tfidf(&self, results: &mut [SearchResultItem]) {
        // Placeholder TF-IDF scoring
        for result in results {
            result.score *= 1.0 + self.b;
        }
    }
}

impl Default for RelevanceScorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_scorer() {
        let scorer = RelevanceScorer::new();
        let mut results = vec![SearchResultItem {
            asset_id: Uuid::new_v4(),
            score: 1.0,
            title: None,
            description: None,
            file_path: String::new(),
            mime_type: None,
            duration_ms: None,
            created_at: 0,
            matched_fields: vec![],
            thumbnail_url: None,
        }];

        scorer.score_bm25(&mut results, 100.0);
        assert!(results[0].score > 1.0);
    }
}
