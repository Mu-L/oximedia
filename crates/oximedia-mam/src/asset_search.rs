//! Asset search engine for MAM.
//!
//! Provides a structured query model with multiple search fields and an
//! in-memory `AssetSearchEngine` that filters a collection of asset
//! descriptors.

#![allow(dead_code)]

use std::collections::HashMap;

/// Fields available for filtering assets.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SearchField {
    /// Asset title / name.
    Title,
    /// Creator or author.
    Creator,
    /// Media type / MIME type string.
    MediaType,
    /// Comma-separated list of tags.
    Tags,
    /// Project or production name.
    Project,
    /// Status label (e.g. `"approved"`, `"draft"`).
    Status,
    /// File extension (without leading dot).
    Extension,
    /// An arbitrary custom metadata key.
    Custom(String),
}

/// A single filter criterion.
#[derive(Clone, Debug)]
pub struct FilterClause {
    /// The field to match against.
    pub field: SearchField,
    /// Substring to search for (case-insensitive).
    pub value: String,
    /// When `true`, the value must NOT be present.
    pub negate: bool,
}

impl FilterClause {
    /// Positive (inclusive) filter clause.
    #[must_use]
    pub fn include(field: SearchField, value: impl Into<String>) -> Self {
        Self {
            field,
            value: value.into(),
            negate: false,
        }
    }

    /// Negative (exclusive) filter clause.
    #[must_use]
    pub fn exclude(field: SearchField, value: impl Into<String>) -> Self {
        Self {
            field,
            value: value.into(),
            negate: true,
        }
    }
}

/// A structured asset search query.
#[derive(Clone, Debug, Default)]
pub struct SearchQuery {
    /// Clauses that are ANDed together.
    pub clauses: Vec<FilterClause>,
    /// Maximum number of results to return (0 = unlimited).
    pub limit: usize,
    /// Number of results to skip before returning.
    pub offset: usize,
}

impl SearchQuery {
    /// Create an empty query.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an include clause for a field/value pair.
    pub fn with(mut self, field: SearchField, value: impl Into<String>) -> Self {
        self.clauses.push(FilterClause::include(field, value));
        self
    }

    /// Add an exclude clause for a field/value pair.
    pub fn without(mut self, field: SearchField, value: impl Into<String>) -> Self {
        self.clauses.push(FilterClause::exclude(field, value));
        self
    }

    /// Set the maximum number of results.
    #[must_use]
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = n;
        self
    }

    /// Set the result offset.
    #[must_use]
    pub fn offset(mut self, n: usize) -> Self {
        self.offset = n;
        self
    }
}

/// A lightweight descriptor for an asset stored in the engine.
#[derive(Clone, Debug)]
pub struct AssetDescriptor {
    /// Unique asset ID.
    pub id: u64,
    /// Asset title.
    pub title: String,
    /// Creator name.
    pub creator: String,
    /// Media type string (e.g. `"video/mp4"`).
    pub media_type: String,
    /// Tags associated with this asset.
    pub tags: Vec<String>,
    /// Project name.
    pub project: String,
    /// Workflow status.
    pub status: String,
    /// File extension.
    pub extension: String,
    /// Custom metadata key-value pairs.
    pub custom: HashMap<String, String>,
}

impl AssetDescriptor {
    /// Create a minimal asset descriptor.
    #[must_use]
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            creator: String::new(),
            media_type: String::new(),
            tags: Vec::new(),
            project: String::new(),
            status: String::new(),
            extension: String::new(),
            custom: HashMap::new(),
        }
    }

    /// Retrieve the string value for a given [`SearchField`].
    #[must_use]
    fn field_value(&self, field: &SearchField) -> String {
        match field {
            SearchField::Title => self.title.clone(),
            SearchField::Creator => self.creator.clone(),
            SearchField::MediaType => self.media_type.clone(),
            SearchField::Tags => self.tags.join(","),
            SearchField::Project => self.project.clone(),
            SearchField::Status => self.status.clone(),
            SearchField::Extension => self.extension.clone(),
            SearchField::Custom(key) => self.custom.get(key).cloned().unwrap_or_default(),
        }
    }

    /// Return `true` if all clauses match this asset.
    #[must_use]
    fn matches(&self, query: &SearchQuery) -> bool {
        for clause in &query.clauses {
            let field_val = self.field_value(&clause.field).to_ascii_lowercase();
            let needle = clause.value.to_ascii_lowercase();
            let found = field_val.contains(&needle);
            if clause.negate == found {
                // negate=true & found=true  => exclusion failed
                // negate=false & found=false => inclusion failed
                return false;
            }
        }
        true
    }
}

/// In-memory asset search engine.
///
/// # Example
/// ```
/// use oximedia_mam::asset_search::{AssetDescriptor, AssetSearchEngine, SearchQuery, SearchField};
///
/// let mut engine = AssetSearchEngine::new();
/// let mut asset = AssetDescriptor::new(1, "Holiday Reel");
/// asset.tags = vec!["holiday".to_string(), "promo".to_string()];
/// engine.add(asset);
///
/// let results = engine.filter(
///     &SearchQuery::new().with(SearchField::Tags, "holiday")
/// );
/// assert_eq!(results.len(), 1);
/// ```
#[derive(Default)]
pub struct AssetSearchEngine {
    assets: Vec<AssetDescriptor>,
}

impl AssetSearchEngine {
    /// Create an empty engine.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an asset to the engine.
    pub fn add(&mut self, asset: AssetDescriptor) {
        self.assets.push(asset);
    }

    /// Remove asset with the given ID.  Returns `true` if found.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.assets.len();
        self.assets.retain(|a| a.id != id);
        self.assets.len() < before
    }

    /// Return the number of indexed assets.
    #[must_use]
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    /// Return `true` when no assets are indexed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Execute a [`SearchQuery`] and return matching asset references.
    ///
    /// Applies `offset` and `limit` (0 = no limit) to the result slice.
    #[must_use]
    pub fn filter<'a>(&'a self, query: &SearchQuery) -> Vec<&'a AssetDescriptor> {
        let matched: Vec<&AssetDescriptor> =
            self.assets.iter().filter(|a| a.matches(query)).collect();

        let start = query.offset.min(matched.len());
        let slice = &matched[start..];
        if query.limit == 0 {
            slice.to_vec()
        } else {
            slice.iter().copied().take(query.limit).collect()
        }
    }

    /// Look up an asset by exact ID.
    #[must_use]
    pub fn find_by_id(&self, id: u64) -> Option<&AssetDescriptor> {
        self.assets.iter().find(|a| a.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> AssetSearchEngine {
        let mut engine = AssetSearchEngine::new();

        let mut a1 = AssetDescriptor::new(1, "Summer Campaign");
        a1.creator = "Alice".to_string();
        a1.media_type = "video/mp4".to_string();
        a1.tags = vec!["summer".to_string(), "promo".to_string()];
        a1.project = "Alpha".to_string();
        a1.status = "approved".to_string();
        a1.extension = "mp4".to_string();

        let mut a2 = AssetDescriptor::new(2, "Winter Promo");
        a2.creator = "Bob".to_string();
        a2.media_type = "video/mov".to_string();
        a2.tags = vec!["winter".to_string(), "promo".to_string()];
        a2.project = "Beta".to_string();
        a2.status = "draft".to_string();
        a2.extension = "mov".to_string();

        let mut a3 = AssetDescriptor::new(3, "Spring Shoot");
        a3.creator = "Alice".to_string();
        a3.media_type = "image/tiff".to_string();
        a3.tags = vec!["spring".to_string()];
        a3.project = "Alpha".to_string();
        a3.status = "approved".to_string();
        a3.extension = "tiff".to_string();

        engine.add(a1);
        engine.add(a2);
        engine.add(a3);
        engine
    }

    #[test]
    fn test_engine_len() {
        let engine = make_engine();
        assert_eq!(engine.len(), 3);
    }

    #[test]
    fn test_filter_by_title() {
        let engine = make_engine();
        let q = SearchQuery::new().with(SearchField::Title, "promo");
        assert_eq!(engine.filter(&q).len(), 1);
    }

    #[test]
    fn test_filter_by_creator() {
        let engine = make_engine();
        let q = SearchQuery::new().with(SearchField::Creator, "alice");
        assert_eq!(engine.filter(&q).len(), 2);
    }

    #[test]
    fn test_filter_by_tag() {
        let engine = make_engine();
        let q = SearchQuery::new().with(SearchField::Tags, "promo");
        assert_eq!(engine.filter(&q).len(), 2);
    }

    #[test]
    fn test_filter_negate() {
        let engine = make_engine();
        let q = SearchQuery::new().without(SearchField::Status, "draft");
        assert_eq!(engine.filter(&q).len(), 2);
    }

    #[test]
    fn test_filter_combined_clauses() {
        let engine = make_engine();
        let q = SearchQuery::new()
            .with(SearchField::Creator, "alice")
            .with(SearchField::Status, "approved");
        assert_eq!(engine.filter(&q).len(), 2);
    }

    #[test]
    fn test_filter_limit() {
        let engine = make_engine();
        let q = SearchQuery::new().limit(1);
        assert_eq!(engine.filter(&q).len(), 1);
    }

    #[test]
    fn test_filter_offset() {
        let engine = make_engine();
        let q = SearchQuery::new().offset(2);
        assert_eq!(engine.filter(&q).len(), 1);
    }

    #[test]
    fn test_filter_no_match() {
        let engine = make_engine();
        let q = SearchQuery::new().with(SearchField::Project, "Gamma");
        assert!(engine.filter(&q).is_empty());
    }

    #[test]
    fn test_filter_empty_query_returns_all() {
        let engine = make_engine();
        assert_eq!(engine.filter(&SearchQuery::new()).len(), 3);
    }

    #[test]
    fn test_find_by_id() {
        let engine = make_engine();
        assert_eq!(
            engine.find_by_id(2).expect("should succeed in test").title,
            "Winter Promo"
        );
    }

    #[test]
    fn test_find_by_id_missing() {
        let engine = make_engine();
        assert!(engine.find_by_id(99).is_none());
    }

    #[test]
    fn test_remove_asset() {
        let mut engine = make_engine();
        assert!(engine.remove(1));
        assert_eq!(engine.len(), 2);
        assert!(engine.find_by_id(1).is_none());
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut engine = make_engine();
        assert!(!engine.remove(999));
    }

    #[test]
    fn test_custom_field_search() {
        let mut engine = AssetSearchEngine::new();
        let mut a = AssetDescriptor::new(10, "Branded Content");
        a.custom
            .insert("client".to_string(), "Acme Corp".to_string());
        engine.add(a);
        let q = SearchQuery::new().with(SearchField::Custom("client".to_string()), "acme");
        assert_eq!(engine.filter(&q).len(), 1);
    }

    #[test]
    fn test_filter_extension() {
        let engine = make_engine();
        let q = SearchQuery::new().with(SearchField::Extension, "tiff");
        assert_eq!(engine.filter(&q).len(), 1);
    }
}
