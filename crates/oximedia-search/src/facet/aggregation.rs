//! Facet aggregation.

use serde::{Deserialize, Serialize};

/// Facet aggregations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Facets {
    /// MIME type facets
    pub mime_types: Vec<FacetCount>,
    /// Format facets
    pub formats: Vec<FacetCount>,
    /// Codec facets
    pub codecs: Vec<FacetCount>,
    /// Resolution facets
    pub resolutions: Vec<FacetCount>,
    /// Category facets
    pub categories: Vec<FacetCount>,
}

/// Facet count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetCount {
    /// Facet value
    pub value: String,
    /// Count
    pub count: usize,
}

impl Facets {
    /// Create new empty facets
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_facets_new() {
        let facets = Facets::new();
        assert!(facets.mime_types.is_empty());
    }
}
