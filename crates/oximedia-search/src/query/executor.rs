//! Query execution engine.

use crate::error::SearchResult;
use crate::query::parser::ParsedQuery;
use crate::SearchResultItem;

/// Query executor
pub struct QueryExecutor;

impl QueryExecutor {
    /// Create a new query executor
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Execute a parsed query
    ///
    /// # Errors
    ///
    /// Returns an error if execution fails
    pub fn execute(&self, _query: &ParsedQuery) -> SearchResult<Vec<SearchResultItem>> {
        // Placeholder
        Ok(Vec::new())
    }
}

impl Default for QueryExecutor {
    fn default() -> Self {
        Self::new()
    }
}
