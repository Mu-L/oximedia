//! MAM full-text search index.
//!
//! A lightweight, in-process inverted index for searching asset metadata
//! without requiring an external search engine.

/// A single searchable field attached to an asset document.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct IndexedField {
    /// Field name (e.g. "title", "description", "tags").
    pub name: String,
    /// Field value as a string.
    pub value: String,
    /// Relative importance multiplier (higher = more relevant).
    pub weight: f32,
    /// Whether this field participates in full-text searches.
    pub searchable: bool,
}

impl IndexedField {
    /// Create a new indexed field.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        value: impl Into<String>,
        weight: f32,
        searchable: bool,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            weight,
            searchable,
        }
    }

    /// Returns `true` when the field's value contains the query (case-insensitive).
    #[must_use]
    pub fn matches(&self, query: &str) -> bool {
        if !self.searchable {
            return false;
        }
        self.value.to_lowercase().contains(&query.to_lowercase())
    }
}

/// A document representing one asset with all its indexed metadata.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AssetDocument {
    /// Unique asset identifier.
    pub id: String,
    /// All fields attached to this document.
    pub fields: Vec<IndexedField>,
}

impl AssetDocument {
    /// Create a new, empty asset document.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            fields: Vec::new(),
        }
    }

    /// Append a field to this document.
    pub fn add_field(&mut self, field: IndexedField) {
        self.fields.push(field);
    }

    /// Retrieve the string value of the first field with `name`, if present.
    #[must_use]
    pub fn field_value(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.value.as_str())
    }

    /// Compute a relevance score for `query` across all searchable fields.
    ///
    /// Score is the sum of weights of matching fields.
    #[must_use]
    pub fn search_score(&self, query: &str) -> f32 {
        self.fields
            .iter()
            .filter(|f| f.matches(query))
            .map(|f| f.weight)
            .sum()
    }
}

/// In-process search index over a collection of asset documents.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct SearchIndex {
    /// All indexed documents.
    pub documents: Vec<AssetDocument>,
}

impl SearchIndex {
    /// Create a new, empty search index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a document to the index.
    pub fn add(&mut self, doc: AssetDocument) {
        self.documents.push(doc);
    }

    /// Remove the document with `id` from the index.
    ///
    /// Returns `true` if a document was found and removed.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.documents.len();
        self.documents.retain(|d| d.id != id);
        self.documents.len() < before
    }

    /// Search all documents for `query` and return `(doc, score)` pairs sorted
    /// by descending relevance score.  Documents with a score of `0.0` are excluded.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<(&AssetDocument, f32)> {
        let mut results: Vec<(&AssetDocument, f32)> = self
            .documents
            .iter()
            .filter_map(|doc| {
                let score = doc.search_score(query);
                if score > 0.0 {
                    Some((doc, score))
                } else {
                    None
                }
            })
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Return the top `n` documents matching `query`.
    #[must_use]
    pub fn top_results(&self, query: &str, n: usize) -> Vec<&AssetDocument> {
        self.search(query)
            .into_iter()
            .take(n)
            .map(|(doc, _)| doc)
            .collect()
    }

    /// Returns the number of documents in the index.
    #[must_use]
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Returns `true` when the index contains no documents.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }
}

/// A structured search query with keyword terms and field-value filters.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SearchQuery {
    /// Free-text terms that must all appear in at least one field.
    pub terms: Vec<String>,
    /// Exact field filters as `(field_name, expected_value)` pairs.
    pub filters: Vec<(String, String)>,
}

impl SearchQuery {
    /// Parse a query string into terms and `field:value` filters.
    ///
    /// Tokens that contain a `:` are treated as field filters; all others
    /// are treated as free-text terms.
    #[must_use]
    pub fn parse(input: &str) -> Self {
        let mut terms = Vec::new();
        let mut filters = Vec::new();
        for token in input.split_whitespace() {
            if let Some((field, value)) = token.split_once(':') {
                filters.push((field.to_string(), value.to_string()));
            } else {
                terms.push(token.to_lowercase());
            }
        }
        Self { terms, filters }
    }

    /// Returns `true` when `doc` satisfies all terms and filters in this query.
    #[must_use]
    pub fn matches_document(&self, doc: &AssetDocument) -> bool {
        // All free-text terms must match at least one searchable field.
        let terms_ok = self
            .terms
            .iter()
            .all(|term| doc.fields.iter().any(|f| f.matches(term)));

        // All filters must match exactly.
        let filters_ok = self.filters.iter().all(|(field, expected)| {
            doc.field_value(field)
                .map(|v| v.to_lowercase() == expected.to_lowercase())
                .unwrap_or(false)
        });

        terms_ok && filters_ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(id: &str, title: &str, tags: &str) -> AssetDocument {
        let mut doc = AssetDocument::new(id);
        doc.add_field(IndexedField::new("title", title, 2.0, true));
        doc.add_field(IndexedField::new("tags", tags, 1.0, true));
        doc.add_field(IndexedField::new("id_field", id, 0.5, false));
        doc
    }

    fn populated_index() -> SearchIndex {
        let mut idx = SearchIndex::new();
        idx.add(make_doc("a1", "Breaking News London", "news uk"));
        idx.add(make_doc("a2", "Sports Highlights", "sports football"));
        idx.add(make_doc(
            "a3",
            "Corporate Announcement",
            "corporate business news",
        ));
        idx
    }

    // --- IndexedField tests ---

    #[test]
    fn test_field_matches_case_insensitive() {
        let f = IndexedField::new("title", "Breaking News", 1.0, true);
        assert!(f.matches("news"));
        assert!(f.matches("BREAKING"));
    }

    #[test]
    fn test_field_not_searchable_never_matches() {
        let f = IndexedField::new("internal", "secret", 1.0, false);
        assert!(!f.matches("secret"));
    }

    #[test]
    fn test_field_no_match() {
        let f = IndexedField::new("title", "Sports", 1.0, true);
        assert!(!f.matches("weather"));
    }

    // --- AssetDocument tests ---

    #[test]
    fn test_document_field_value_found() {
        let doc = make_doc("x", "My Title", "tag1");
        assert_eq!(doc.field_value("title"), Some("My Title"));
    }

    #[test]
    fn test_document_field_value_missing() {
        let doc = make_doc("x", "My Title", "tag1");
        assert!(doc.field_value("nonexistent").is_none());
    }

    #[test]
    fn test_document_search_score_positive() {
        let doc = make_doc("x", "Football Highlights", "sports");
        let score = doc.search_score("football");
        assert!(score > 0.0);
    }

    #[test]
    fn test_document_search_score_zero_no_match() {
        let doc = make_doc("x", "Football Highlights", "sports");
        let score = doc.search_score("cooking");
        assert!((score - 0.0).abs() < f32::EPSILON);
    }

    // --- SearchIndex tests ---

    #[test]
    fn test_index_add_and_len() {
        let mut idx = SearchIndex::new();
        idx.add(make_doc("d1", "Title One", "tag"));
        idx.add(make_doc("d2", "Title Two", "tag"));
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn test_index_remove_existing() {
        let mut idx = SearchIndex::new();
        idx.add(make_doc("d1", "Title", "tag"));
        assert!(idx.remove("d1"));
        assert!(idx.is_empty());
    }

    #[test]
    fn test_index_remove_nonexistent() {
        let mut idx = SearchIndex::new();
        assert!(!idx.remove("ghost"));
    }

    #[test]
    fn test_index_search_returns_relevant() {
        let idx = populated_index();
        let results = idx.search("news");
        assert!(!results.is_empty());
        assert!(results.iter().any(|(d, _)| d.id == "a1" || d.id == "a3"));
    }

    #[test]
    fn test_index_search_sorted_by_score() {
        let idx = populated_index();
        let results = idx.search("news");
        if results.len() >= 2 {
            assert!(results[0].1 >= results[1].1);
        }
    }

    #[test]
    fn test_index_top_results_limit() {
        let idx = populated_index();
        let top = idx.top_results("news", 1);
        assert_eq!(top.len(), 1);
    }

    // --- SearchQuery tests ---

    #[test]
    fn test_query_parse_terms() {
        let q = SearchQuery::parse("football highlights");
        assert_eq!(q.terms, vec!["football", "highlights"]);
        assert!(q.filters.is_empty());
    }

    #[test]
    fn test_query_parse_filters() {
        let q = SearchQuery::parse("type:video");
        assert_eq!(q.filters, vec![("type".to_string(), "video".to_string())]);
        assert!(q.terms.is_empty());
    }

    #[test]
    fn test_query_matches_document_terms() {
        let q = SearchQuery::parse("sports");
        let doc = make_doc("y", "Sports Reel", "sport events");
        assert!(q.matches_document(&doc));
    }

    #[test]
    fn test_query_no_match_missing_term() {
        let q = SearchQuery::parse("cooking");
        let doc = make_doc("y", "Sports Reel", "sport events");
        assert!(!q.matches_document(&doc));
    }
}
