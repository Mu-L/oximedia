//! Caption search utilities for `OxiMedia`.
//!
//! Provides full-text and pattern-based search over caption tracks,
//! returning structured match results with optional surrounding context.

#![allow(dead_code)]

/// Controls how text matching is performed during caption search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptionSearchMode {
    /// Case-sensitive exact substring match.
    CaseSensitive,
    /// Case-insensitive substring match.
    CaseInsensitive,
    /// Regular-expression match (case-sensitive).
    Regex,
    /// Regular-expression match (case-insensitive).
    RegexIgnoreCase,
    /// Whole-word match (case-insensitive).
    WholeWord,
    /// Whole-word match (case-sensitive).
    WholeWordCaseSensitive,
}

impl CaptionSearchMode {
    /// Returns `true` when the mode performs case-sensitive comparison.
    #[must_use]
    pub fn is_case_sensitive(self) -> bool {
        matches!(
            self,
            Self::CaseSensitive | Self::Regex | Self::WholeWordCaseSensitive
        )
    }

    /// Returns `true` when the mode uses regular expressions.
    #[must_use]
    pub fn is_regex(self) -> bool {
        matches!(self, Self::Regex | Self::RegexIgnoreCase)
    }

    /// Human-readable label for the search mode.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::CaseSensitive => "case-sensitive",
            Self::CaseInsensitive => "case-insensitive",
            Self::Regex => "regex",
            Self::RegexIgnoreCase => "regex (ignore case)",
            Self::WholeWord => "whole-word",
            Self::WholeWordCaseSensitive => "whole-word (case-sensitive)",
        }
    }
}

/// A single match found during a caption search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptionMatch {
    /// Index of the cue that contains the match.
    pub cue_index: usize,
    /// Start time of the matching cue in milliseconds.
    pub start_ms: u64,
    /// End time of the matching cue in milliseconds.
    pub end_ms: u64,
    /// The matched substring.
    pub matched_text: String,
    /// Surrounding context snippet.
    pub context: String,
    /// Byte offset of the match within the cue text.
    pub byte_offset: usize,
}

impl CaptionMatch {
    /// Creates a new `CaptionMatch`.
    #[must_use]
    pub fn new(
        cue_index: usize,
        start_ms: u64,
        end_ms: u64,
        matched_text: impl Into<String>,
        context: impl Into<String>,
        byte_offset: usize,
    ) -> Self {
        Self {
            cue_index,
            start_ms,
            end_ms,
            matched_text: matched_text.into(),
            context: context.into(),
            byte_offset,
        }
    }

    /// Returns the number of characters in the surrounding context snippet.
    #[must_use]
    pub fn context_chars(&self) -> usize {
        self.context.chars().count()
    }

    /// Returns the duration of the matched cue in milliseconds.
    #[must_use]
    pub fn cue_duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }
}

/// Configuration for a caption search operation.
#[derive(Debug, Clone)]
pub struct CaptionSearchConfig {
    /// The pattern to search for.
    pub query: String,
    /// Search mode.
    pub mode: CaptionSearchMode,
    /// Number of characters of surrounding context to include in matches.
    pub context_chars: usize,
    /// Maximum number of results to return (0 = unlimited).
    pub max_results: usize,
}

impl CaptionSearchConfig {
    /// Creates a new configuration with sensible defaults.
    #[must_use]
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            mode: CaptionSearchMode::CaseInsensitive,
            context_chars: 40,
            max_results: 0,
        }
    }

    /// Sets the search mode.
    #[must_use]
    pub fn with_mode(mut self, mode: CaptionSearchMode) -> Self {
        self.mode = mode;
        self
    }

    /// Sets the surrounding context size.
    #[must_use]
    pub fn with_context(mut self, chars: usize) -> Self {
        self.context_chars = chars;
        self
    }
}

/// A simple caption cue used as input to the searcher.
#[derive(Debug, Clone)]
pub struct SearchableCue {
    /// Index of this cue in the track.
    pub index: usize,
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds.
    pub end_ms: u64,
    /// Plain-text content of the cue.
    pub text: String,
}

impl SearchableCue {
    /// Creates a new `SearchableCue`.
    #[must_use]
    pub fn new(index: usize, start_ms: u64, end_ms: u64, text: impl Into<String>) -> Self {
        Self {
            index,
            start_ms,
            end_ms,
            text: text.into(),
        }
    }
}

/// Searches caption tracks for a given query pattern.
#[derive(Debug, Default)]
pub struct CaptionSearcher;

impl CaptionSearcher {
    /// Creates a new `CaptionSearcher`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Searches `cues` using the provided `config` and returns all matches.
    #[must_use]
    pub fn search(
        &self,
        cues: &[SearchableCue],
        config: &CaptionSearchConfig,
    ) -> CaptionSearchResult {
        let mut matches = Vec::new();

        for cue in cues {
            let haystack = if config.mode.is_case_sensitive() {
                cue.text.clone()
            } else {
                cue.text.to_lowercase()
            };
            let needle = if config.mode.is_case_sensitive() {
                config.query.clone()
            } else {
                config.query.to_lowercase()
            };

            let mut search_start = 0usize;
            while let Some(pos) = haystack[search_start..].find(&needle) {
                let abs_pos = search_start + pos;
                let ctx =
                    self.extract_context(&cue.text, abs_pos, needle.len(), config.context_chars);
                matches.push(CaptionMatch::new(
                    cue.index,
                    cue.start_ms,
                    cue.end_ms,
                    &cue.text[abs_pos..abs_pos + needle.len()],
                    ctx,
                    abs_pos,
                ));
                search_start = abs_pos + needle.len().max(1);
                if config.max_results > 0 && matches.len() >= config.max_results {
                    return CaptionSearchResult::new(matches);
                }
            }
        }

        CaptionSearchResult::new(matches)
    }

    /// Extracts a context snippet around `offset` with `ctx_chars` on each side.
    fn extract_context(
        &self,
        text: &str,
        offset: usize,
        match_len: usize,
        ctx_chars: usize,
    ) -> String {
        let chars: Vec<char> = text.chars().collect();
        // Convert byte offset to char offset (approximate)
        let char_offset = text[..offset].chars().count();
        let start = char_offset.saturating_sub(ctx_chars);
        let end = (char_offset + match_len + ctx_chars).min(chars.len());
        chars[start..end].iter().collect()
    }
}

/// The result of a caption search operation.
#[derive(Debug, Clone, Default)]
pub struct CaptionSearchResult {
    /// All matches found.
    pub matches: Vec<CaptionMatch>,
}

impl CaptionSearchResult {
    /// Creates a new result from a list of matches.
    #[must_use]
    pub fn new(matches: Vec<CaptionMatch>) -> Self {
        Self { matches }
    }

    /// Returns the total number of matches.
    #[must_use]
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Returns `true` when no matches were found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// Returns the indices of all cues that contained at least one match.
    #[must_use]
    pub fn matching_cue_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self.matches.iter().map(|m| m.cue_index).collect();
        indices.dedup();
        indices
    }

    /// Returns matches that fall within a time window (inclusive).
    #[must_use]
    pub fn filter_by_time(&self, from_ms: u64, to_ms: u64) -> Vec<&CaptionMatch> {
        self.matches
            .iter()
            .filter(|m| m.start_ms >= from_ms && m.end_ms <= to_ms)
            .collect()
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cues() -> Vec<SearchableCue> {
        vec![
            SearchableCue::new(0, 0, 2000, "Hello world"),
            SearchableCue::new(1, 2000, 4000, "World of captions"),
            SearchableCue::new(2, 4000, 6000, "Goodbye world"),
            SearchableCue::new(3, 6000, 8000, "No match here"),
        ]
    }

    #[test]
    fn test_mode_case_sensitive() {
        assert!(CaptionSearchMode::CaseSensitive.is_case_sensitive());
        assert!(!CaptionSearchMode::CaseInsensitive.is_case_sensitive());
    }

    #[test]
    fn test_mode_regex_detection() {
        assert!(CaptionSearchMode::Regex.is_regex());
        assert!(CaptionSearchMode::RegexIgnoreCase.is_regex());
        assert!(!CaptionSearchMode::CaseSensitive.is_regex());
    }

    #[test]
    fn test_mode_labels() {
        assert_eq!(CaptionSearchMode::CaseSensitive.label(), "case-sensitive");
        assert_eq!(
            CaptionSearchMode::CaseInsensitive.label(),
            "case-insensitive"
        );
        assert_eq!(CaptionSearchMode::Regex.label(), "regex");
    }

    #[test]
    fn test_caption_match_context_chars() {
        let m = CaptionMatch::new(0, 0, 1000, "world", "Hello world", 6);
        assert_eq!(m.context_chars(), 11);
    }

    #[test]
    fn test_caption_match_cue_duration() {
        let m = CaptionMatch::new(0, 1000, 3500, "x", "x", 0);
        assert_eq!(m.cue_duration_ms(), 2500);
    }

    #[test]
    fn test_search_case_insensitive_finds_multiple() {
        let cues = sample_cues();
        let config = CaptionSearchConfig::new("world");
        let result = CaptionSearcher::new().search(&cues, &config);
        assert_eq!(result.match_count(), 3);
    }

    #[test]
    fn test_search_case_sensitive_misses_capitalised() {
        let cues = sample_cues();
        let config = CaptionSearchConfig::new("world").with_mode(CaptionSearchMode::CaseSensitive);
        let result = CaptionSearcher::new().search(&cues, &config);
        // "Hello world" and "Goodbye world" match; "World of captions" does not
        assert_eq!(result.match_count(), 2);
    }

    #[test]
    fn test_search_no_match() {
        let cues = sample_cues();
        let config = CaptionSearchConfig::new("zzz");
        let result = CaptionSearcher::new().search(&cues, &config);
        assert!(result.is_empty());
    }

    #[test]
    fn test_search_result_is_empty_when_empty() {
        let result = CaptionSearchResult::new(vec![]);
        assert!(result.is_empty());
        assert_eq!(result.match_count(), 0);
    }

    #[test]
    fn test_filter_by_time() {
        let cues = sample_cues();
        let config = CaptionSearchConfig::new("world");
        let result = CaptionSearcher::new().search(&cues, &config);
        let in_window = result.filter_by_time(0, 4000);
        assert_eq!(in_window.len(), 2);
    }

    #[test]
    fn test_matching_cue_indices() {
        let cues = sample_cues();
        let config = CaptionSearchConfig::new("world");
        let result = CaptionSearcher::new().search(&cues, &config);
        let indices = result.matching_cue_indices();
        assert!(indices.contains(&0));
        assert!(indices.contains(&1));
        assert!(indices.contains(&2));
        assert!(!indices.contains(&3));
    }

    #[test]
    fn test_max_results_limit() {
        let cues = sample_cues();
        let mut config = CaptionSearchConfig::new("world");
        config.max_results = 1;
        let result = CaptionSearcher::new().search(&cues, &config);
        assert_eq!(result.match_count(), 1);
    }

    #[test]
    fn test_config_with_context() {
        let cfg = CaptionSearchConfig::new("x").with_context(20);
        assert_eq!(cfg.context_chars, 20);
    }

    #[test]
    fn test_config_with_mode() {
        let cfg = CaptionSearchConfig::new("x").with_mode(CaptionSearchMode::Regex);
        assert_eq!(cfg.mode, CaptionSearchMode::Regex);
    }

    #[test]
    fn test_searchable_cue_constructor() {
        let cue = SearchableCue::new(5, 1000, 2000, "Test text");
        assert_eq!(cue.index, 5);
        assert_eq!(cue.start_ms, 1000);
        assert_eq!(cue.end_ms, 2000);
        assert_eq!(cue.text, "Test text");
    }
}
