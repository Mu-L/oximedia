#![allow(dead_code)]
//! Autocomplete and search suggestion engine for `oximedia-search`.
//!
//! Provides prefix-based and context-aware query suggestions to help
//! users find media assets faster by completing partial queries.

use std::collections::HashMap;

/// The kind of suggestion being offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestType {
    /// A query the user has previously typed.
    HistoryCompletion,
    /// A popular query from the global corpus.
    PopularQuery,
    /// A term derived from indexed media metadata (title, tag, etc.).
    MetadataCompletion,
    /// A spelling-corrected alternative.
    SpellCorrection,
}

/// A single autocomplete suggestion.
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// The full suggested query string.
    pub text: String,
    /// How the suggestion was generated.
    pub suggest_type: SuggestType,
    /// Confidence / relevance score in `[0.0, 1.0]`.
    pub score: f32,
    /// Number of times this query has been observed (for ranking).
    pub frequency: u32,
}

impl Suggestion {
    /// Create a new suggestion.
    pub fn new(
        text: impl Into<String>,
        suggest_type: SuggestType,
        score: f32,
        frequency: u32,
    ) -> Self {
        Self {
            text: text.into(),
            suggest_type,
            score: score.clamp(0.0, 1.0),
            frequency,
        }
    }

    /// Returns `true` if this suggestion came from query history.
    #[must_use]
    pub fn is_history(&self) -> bool {
        self.suggest_type == SuggestType::HistoryCompletion
    }

    /// Returns `true` if this suggestion was generated from metadata.
    #[must_use]
    pub fn is_metadata(&self) -> bool {
        self.suggest_type == SuggestType::MetadataCompletion
    }
}

/// Configuration for the `SearchSuggestor`.
#[derive(Debug, Clone)]
pub struct SuggestorConfig {
    /// Maximum number of suggestions to return.
    pub max_results: usize,
    /// Minimum prefix length before suggestions are generated.
    pub min_prefix_len: usize,
    /// Whether to include history-based suggestions.
    pub include_history: bool,
    /// Whether to include metadata-derived suggestions.
    pub include_metadata: bool,
}

impl Default for SuggestorConfig {
    fn default() -> Self {
        Self {
            max_results: 8,
            min_prefix_len: 2,
            include_history: true,
            include_metadata: true,
        }
    }
}

/// Autocomplete and query suggestion engine.
///
/// Combines personal query history, global popular queries, and indexed
/// metadata terms to produce ranked suggestions for a given prefix.
#[derive(Debug)]
pub struct SearchSuggestor {
    /// User query history: query -> occurrence count.
    history: HashMap<String, u32>,
    /// Global popular queries: query -> occurrence count.
    popular: HashMap<String, u32>,
    /// Metadata-derived completion terms.
    metadata_terms: Vec<String>,
    /// Configuration.
    config: SuggestorConfig,
}

impl SearchSuggestor {
    /// Create a new, empty suggestor with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
            popular: HashMap::new(),
            metadata_terms: Vec::new(),
            config: SuggestorConfig::default(),
        }
    }

    /// Create a suggestor with a custom configuration.
    #[must_use]
    pub fn with_config(config: SuggestorConfig) -> Self {
        Self {
            history: HashMap::new(),
            popular: HashMap::new(),
            metadata_terms: Vec::new(),
            config,
        }
    }

    /// Record a user query in the personal history.
    pub fn record_query(&mut self, query: impl Into<String>) {
        *self.history.entry(query.into()).or_insert(0) += 1;
    }

    /// Add or update a global popular query.
    pub fn add_popular(&mut self, query: impl Into<String>, count: u32) {
        self.popular.insert(query.into(), count);
    }

    /// Add a metadata-derived completion term (e.g. a title or tag).
    pub fn add_metadata_term(&mut self, term: impl Into<String>) {
        self.metadata_terms.push(term.into());
    }

    /// Return up to `config.max_results` suggestions for the given prefix.
    ///
    /// Suggestions are sorted by score descending, then alphabetically.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn suggest(&self, prefix: &str) -> Vec<Suggestion> {
        let prefix_lower = prefix.to_lowercase();

        if prefix_lower.len() < self.config.min_prefix_len {
            return Vec::new();
        }

        let mut suggestions: Vec<Suggestion> = Vec::new();

        // History-based completions.
        if self.config.include_history {
            for (query, &freq) in &self.history {
                if query.to_lowercase().starts_with(&prefix_lower) {
                    let score = Self::score_completion(query.len(), freq, 1.0);
                    suggestions.push(Suggestion::new(
                        query.clone(),
                        SuggestType::HistoryCompletion,
                        score,
                        freq,
                    ));
                }
            }
        }

        // Popular-query completions.
        for (query, &freq) in &self.popular {
            if query.to_lowercase().starts_with(&prefix_lower) {
                let score = Self::score_completion(query.len(), freq, 0.85);
                suggestions.push(Suggestion::new(
                    query.clone(),
                    SuggestType::PopularQuery,
                    score,
                    freq,
                ));
            }
        }

        // Metadata-term completions.
        if self.config.include_metadata {
            for term in &self.metadata_terms {
                if term.to_lowercase().starts_with(&prefix_lower) {
                    let score = Self::score_completion(term.len(), 1, 0.7);
                    suggestions.push(Suggestion::new(
                        term.clone(),
                        SuggestType::MetadataCompletion,
                        score,
                        1,
                    ));
                }
            }
        }

        // Sort: score descending, then text ascending.
        suggestions.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.text.cmp(&b.text))
        });
        suggestions.dedup_by(|a, b| a.text.eq_ignore_ascii_case(&b.text));
        suggestions.truncate(self.config.max_results);
        suggestions
    }

    /// Score a completion candidate.
    ///
    /// Higher frequency and shorter completions (closer to the prefix) rank higher.
    #[allow(clippy::cast_precision_loss)]
    fn score_completion(query_len: usize, frequency: u32, base: f32) -> f32 {
        let length_factor = 1.0_f32 / (1.0 + (query_len as f32) * 0.05);
        let freq_factor = (frequency as f32).ln_1p() / 10.0;
        (base + freq_factor + length_factor).clamp(0.0, 1.0)
    }

    /// Returns the number of history entries.
    #[must_use]
    pub fn history_size(&self) -> usize {
        self.history.len()
    }

    /// Clear personal query history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

impl Default for SearchSuggestor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_suggestor() -> SearchSuggestor {
        let mut s = SearchSuggestor::new();
        s.record_query("video editor");
        s.record_query("video encoder");
        s.record_query("video encoder");
        s.add_popular("video stream", 500);
        s.add_popular("audio codec", 300);
        s.add_metadata_term("Video Production 2024");
        s
    }

    #[test]
    fn test_suggest_returns_vec() {
        let s = base_suggestor();
        let results = s.suggest("vid");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_suggest_respects_min_prefix_len() {
        let s = base_suggestor();
        let results = s.suggest("v"); // len 1, below default min of 2
        assert!(results.is_empty());
    }

    #[test]
    fn test_suggest_prefix_filtering() {
        let s = base_suggestor();
        let results = s.suggest("aud");
        for r in &results {
            assert!(r.text.to_lowercase().starts_with("aud"));
        }
    }

    #[test]
    fn test_suggest_history_type() {
        let s = base_suggestor();
        let results = s.suggest("video e");
        let has_history = results
            .iter()
            .any(|r| r.suggest_type == SuggestType::HistoryCompletion);
        assert!(has_history);
    }

    #[test]
    fn test_suggest_popular_type() {
        let s = base_suggestor();
        let results = s.suggest("video s");
        let has_popular = results
            .iter()
            .any(|r| r.suggest_type == SuggestType::PopularQuery);
        assert!(has_popular);
    }

    #[test]
    fn test_suggest_metadata_type() {
        let s = base_suggestor();
        let results = s.suggest("Video P");
        let has_meta = results
            .iter()
            .any(|r| r.suggest_type == SuggestType::MetadataCompletion);
        assert!(has_meta);
    }

    #[test]
    fn test_suggest_max_results_respected() {
        let mut s = SearchSuggestor::with_config(SuggestorConfig {
            max_results: 3,
            min_prefix_len: 1,
            include_history: true,
            include_metadata: true,
        });
        for i in 0..20 {
            s.add_popular(format!("video clip {}", i), 100);
        }
        let results = s.suggest("v");
        assert!(results.len() <= 3);
    }

    #[test]
    fn test_suggest_sorted_by_score_desc() {
        let s = base_suggestor();
        let results = s.suggest("video");
        for w in results.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
    }

    #[test]
    fn test_record_query_increments_history() {
        let mut s = SearchSuggestor::new();
        s.record_query("codec");
        s.record_query("codec");
        assert_eq!(s.history_size(), 1);
        assert_eq!(*s.history.get("codec").expect("should succeed in test"), 2);
    }

    #[test]
    fn test_clear_history() {
        let mut s = SearchSuggestor::new();
        s.record_query("something");
        s.clear_history();
        assert_eq!(s.history_size(), 0);
    }

    #[test]
    fn test_suggestion_is_history() {
        let sug = Suggestion::new("video editor", SuggestType::HistoryCompletion, 0.9, 5);
        assert!(sug.is_history());
        assert!(!sug.is_metadata());
    }

    #[test]
    fn test_suggestion_is_metadata() {
        let sug = Suggestion::new("4K Footage", SuggestType::MetadataCompletion, 0.7, 1);
        assert!(sug.is_metadata());
        assert!(!sug.is_history());
    }

    #[test]
    fn test_suggestion_score_clamped() {
        let sug = Suggestion::new("x", SuggestType::PopularQuery, 5.0, 1);
        assert!(sug.score <= 1.0);
    }

    #[test]
    fn test_no_metadata_when_disabled() {
        let mut s = SearchSuggestor::with_config(SuggestorConfig {
            max_results: 10,
            min_prefix_len: 1,
            include_history: false,
            include_metadata: false,
        });
        s.add_metadata_term("special clip");
        s.add_popular("special", 100);
        let results = s.suggest("spec");
        let has_meta = results
            .iter()
            .any(|r| r.suggest_type == SuggestType::MetadataCompletion);
        assert!(!has_meta);
    }

    #[test]
    fn test_empty_prefix_returns_nothing_when_below_min() {
        let s = SearchSuggestor::new();
        assert!(s.suggest("").is_empty());
    }

    #[test]
    fn test_no_results_for_unmatched_prefix() {
        let s = base_suggestor();
        let results = s.suggest("zzzzzz");
        assert!(results.is_empty());
    }
}
