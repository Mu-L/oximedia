//! Query builder for clip searches.

use crate::logging::Rating;

/// Query builder for advanced clip searches.
#[derive(Debug, Clone, Default)]
pub struct QueryBuilder {
    conditions: Vec<String>,
    params: Vec<String>,
}

impl QueryBuilder {
    /// Creates a new query builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            conditions: Vec::new(),
            params: Vec::new(),
        }
    }

    /// Adds a name condition.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.conditions.push("name LIKE ?".to_string());
        self.params.push(format!("%{}%", name.into()));
        self
    }

    /// Adds a rating condition.
    #[must_use]
    pub fn with_rating(mut self, rating: Rating) -> Self {
        self.conditions.push("rating = ?".to_string());
        self.params.push(rating.to_value().to_string());
        self
    }

    /// Adds a minimum rating condition.
    #[must_use]
    pub fn with_min_rating(mut self, rating: Rating) -> Self {
        self.conditions.push("rating >= ?".to_string());
        self.params.push(rating.to_value().to_string());
        self
    }

    /// Adds a favorite condition.
    #[must_use]
    pub fn with_favorite(mut self, is_favorite: bool) -> Self {
        self.conditions
            .push(format!("is_favorite = {}", i64::from(is_favorite)));
        self
    }

    /// Adds a rejected condition.
    #[must_use]
    pub fn with_rejected(mut self, is_rejected: bool) -> Self {
        self.conditions
            .push(format!("is_rejected = {}", i64::from(is_rejected)));
        self
    }

    /// Builds the SQL query.
    #[must_use]
    pub fn build(&self) -> String {
        let mut query = String::from("SELECT * FROM clips");

        if !self.conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&self.conditions.join(" AND "));
        }

        query.push_str(" ORDER BY created_at DESC");
        query
    }

    /// Returns the query parameters.
    #[must_use]
    pub fn params(&self) -> &[String] {
        &self.params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_builder() {
        let query = QueryBuilder::new()
            .with_name("test")
            .with_favorite(true)
            .build();

        assert!(query.contains("WHERE"));
        assert!(query.contains("name LIKE ?"));
        assert!(query.contains("is_favorite = 1"));
    }

    #[test]
    fn test_query_builder_params() {
        let builder = QueryBuilder::new().with_name("test");
        let params = builder.params();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], "%test%");
    }

    #[test]
    fn test_query_builder_empty() {
        let query = QueryBuilder::new().build();
        assert!(query.contains("SELECT * FROM clips"));
        assert!(query.contains("ORDER BY created_at DESC"));
        assert!(!query.contains("WHERE"));
    }
}
