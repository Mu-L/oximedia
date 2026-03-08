#![allow(dead_code)]
//! Search query analytics: tracking popular and zero-result queries.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single recorded search event.
#[derive(Debug, Clone)]
pub struct QueryEvent {
    /// The search query string.
    pub query: String,
    /// Unix timestamp (seconds) when the query was executed.
    pub timestamp_secs: u64,
    /// Number of results returned.
    pub results: usize,
    /// Query execution duration in milliseconds.
    pub duration_ms: u64,
}

impl QueryEvent {
    /// Create a new `QueryEvent` with the current timestamp.
    pub fn new(query: impl Into<String>, results: usize, duration_ms: u64) -> Self {
        let timestamp_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            query: query.into(),
            timestamp_secs,
            results,
            duration_ms,
        }
    }

    /// Create a `QueryEvent` with an explicit timestamp (useful for tests).
    pub fn with_timestamp(
        query: impl Into<String>,
        results: usize,
        duration_ms: u64,
        timestamp_secs: u64,
    ) -> Self {
        Self {
            query: query.into(),
            timestamp_secs,
            results,
            duration_ms,
        }
    }

    /// Returns the number of results returned for this query.
    #[must_use]
    pub fn result_count(&self) -> usize {
        self.results
    }

    /// Returns `true` if the query returned no results.
    #[must_use]
    pub fn is_zero_result(&self) -> bool {
        self.results == 0
    }
}

/// Aggregated analytics over recorded search events.
#[derive(Debug, Default)]
pub struct SearchAnalytics {
    events: Vec<QueryEvent>,
}

impl SearchAnalytics {
    /// Create a new, empty analytics tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a search event.
    pub fn record_query(&mut self, event: QueryEvent) {
        self.events.push(event);
    }

    /// Returns total number of recorded queries.
    #[must_use]
    pub fn total_queries(&self) -> usize {
        self.events.len()
    }

    /// Returns the `n` most popular query strings by occurrence count.
    #[must_use]
    pub fn popular_queries(&self, n: usize) -> Vec<(String, usize)> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for e in &self.events {
            *counts.entry(e.query.as_str()).or_insert(0) += 1;
        }
        let mut sorted: Vec<(String, usize)> = counts
            .into_iter()
            .map(|(q, c)| (q.to_string(), c))
            .collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        sorted.truncate(n);
        sorted
    }

    /// Returns the `n` most common queries that returned zero results.
    #[must_use]
    pub fn zero_result_queries(&self, n: usize) -> Vec<(String, usize)> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for e in &self.events {
            if e.is_zero_result() {
                *counts.entry(e.query.as_str()).or_insert(0) += 1;
            }
        }
        let mut sorted: Vec<(String, usize)> = counts
            .into_iter()
            .map(|(q, c)| (q.to_string(), c))
            .collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        sorted.truncate(n);
        sorted
    }

    /// Returns the average result count across all recorded events.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn avg_result_count(&self) -> f64 {
        if self.events.is_empty() {
            return 0.0;
        }
        let total: usize = self.events.iter().map(|e| e.results).sum();
        total as f64 / self.events.len() as f64
    }

    /// Returns the average query duration in milliseconds.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn avg_duration_ms(&self) -> f64 {
        if self.events.is_empty() {
            return 0.0;
        }
        let total: u64 = self.events.iter().map(|e| e.duration_ms).sum();
        total as f64 / self.events.len() as f64
    }

    /// Clear all recorded events.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(query: &str, results: usize) -> QueryEvent {
        QueryEvent::with_timestamp(query, results, 10, 1_000_000)
    }

    #[test]
    fn test_query_event_result_count() {
        let e = make_event("video", 42);
        assert_eq!(e.result_count(), 42);
    }

    #[test]
    fn test_query_event_is_zero_result_true() {
        let e = make_event("xyzqrs", 0);
        assert!(e.is_zero_result());
    }

    #[test]
    fn test_query_event_is_zero_result_false() {
        let e = make_event("video", 5);
        assert!(!e.is_zero_result());
    }

    #[test]
    fn test_analytics_empty() {
        let a = SearchAnalytics::new();
        assert_eq!(a.total_queries(), 0);
    }

    #[test]
    fn test_record_query_increments_count() {
        let mut a = SearchAnalytics::new();
        a.record_query(make_event("video", 10));
        assert_eq!(a.total_queries(), 1);
    }

    #[test]
    fn test_popular_queries_ordering() {
        let mut a = SearchAnalytics::new();
        a.record_query(make_event("audio", 5));
        a.record_query(make_event("video", 5));
        a.record_query(make_event("video", 3));
        let popular = a.popular_queries(2);
        assert_eq!(popular[0].0, "video");
        assert_eq!(popular[0].1, 2);
    }

    #[test]
    fn test_popular_queries_limited() {
        let mut a = SearchAnalytics::new();
        for i in 0..10 {
            a.record_query(make_event(&format!("q{}", i), 1));
        }
        assert_eq!(a.popular_queries(3).len(), 3);
    }

    #[test]
    fn test_zero_result_queries() {
        let mut a = SearchAnalytics::new();
        a.record_query(make_event("missing", 0));
        a.record_query(make_event("missing", 0));
        a.record_query(make_event("video", 10));
        let zero = a.zero_result_queries(5);
        assert_eq!(zero.len(), 1);
        assert_eq!(zero[0].0, "missing");
        assert_eq!(zero[0].1, 2);
    }

    #[test]
    fn test_avg_result_count() {
        let mut a = SearchAnalytics::new();
        a.record_query(make_event("a", 10));
        a.record_query(make_event("b", 20));
        assert!((a.avg_result_count() - 15.0).abs() < 1e-9);
    }

    #[test]
    fn test_avg_result_count_empty() {
        let a = SearchAnalytics::new();
        assert_eq!(a.avg_result_count(), 0.0);
    }

    #[test]
    fn test_avg_duration_ms() {
        let mut a = SearchAnalytics::new();
        a.record_query(QueryEvent::with_timestamp("a", 1, 100, 0));
        a.record_query(QueryEvent::with_timestamp("b", 1, 200, 0));
        assert!((a.avg_duration_ms() - 150.0).abs() < 1e-9);
    }

    #[test]
    fn test_clear() {
        let mut a = SearchAnalytics::new();
        a.record_query(make_event("x", 1));
        a.clear();
        assert_eq!(a.total_queries(), 0);
    }

    #[test]
    fn test_popular_queries_empty() {
        let a = SearchAnalytics::new();
        assert!(a.popular_queries(5).is_empty());
    }

    #[test]
    fn test_zero_result_queries_none() {
        let mut a = SearchAnalytics::new();
        a.record_query(make_event("video", 10));
        assert!(a.zero_result_queries(5).is_empty());
    }
}
