//! Batch ingest pipeline for media assets
//!
//! Provides utilities for planning, validating and executing batch ingest
//! operations across large sets of media files.

#![allow(dead_code)]

use std::collections::HashMap;

/// A single item to be ingested in a batch operation
#[derive(Debug, Clone)]
pub struct IngestItem {
    /// Source file path
    pub path: String,
    /// Destination path or folder within the MAM
    pub destination: String,
    /// Additional metadata to attach on ingest
    pub metadata: HashMap<String, String>,
    /// Processing priority (0 = lowest, 255 = highest)
    pub priority: u8,
}

impl IngestItem {
    /// Create a new ingest item
    #[must_use]
    pub fn new(path: impl Into<String>, destination: impl Into<String>, priority: u8) -> Self {
        Self {
            path: path.into(),
            destination: destination.into(),
            metadata: HashMap::new(),
            priority,
        }
    }

    /// Builder-style method to add metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Outcome of a single ingest attempt
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestStatus {
    /// File was ingested successfully
    Success,
    /// Ingest encountered a fatal error
    Failed,
    /// File was intentionally skipped (e.g. unsupported format)
    Skipped,
    /// File was already present in the MAM (duplicate detected)
    Duplicate,
}

impl IngestStatus {
    /// Returns `true` if the status indicates a successful ingest
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    /// Human-readable label
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Duplicate => "duplicate",
        }
    }
}

/// Result of a single ingest item being processed
#[derive(Debug, Clone)]
pub struct IngestResult {
    /// The item that was processed
    pub item: IngestItem,
    /// Final status of the ingest attempt
    pub status: IngestStatus,
    /// Error message if the ingest failed
    pub error: Option<String>,
    /// Time taken to process the item in milliseconds
    pub duration_ms: u64,
    /// Asset ID assigned by the MAM on success
    pub asset_id: Option<String>,
}

impl IngestResult {
    /// Create a successful result
    #[must_use]
    pub fn success(item: IngestItem, duration_ms: u64, asset_id: String) -> Self {
        Self {
            item,
            status: IngestStatus::Success,
            error: None,
            duration_ms,
            asset_id: Some(asset_id),
        }
    }

    /// Create a failed result
    #[must_use]
    pub fn failed(item: IngestItem, duration_ms: u64, error: impl Into<String>) -> Self {
        Self {
            item,
            status: IngestStatus::Failed,
            error: Some(error.into()),
            duration_ms,
            asset_id: None,
        }
    }

    /// Create a skipped result
    #[must_use]
    pub fn skipped(item: IngestItem, reason: impl Into<String>) -> Self {
        Self {
            item,
            status: IngestStatus::Skipped,
            error: Some(reason.into()),
            duration_ms: 0,
            asset_id: None,
        }
    }

    /// Create a duplicate result
    #[must_use]
    pub fn duplicate(item: IngestItem, existing_asset_id: impl Into<String>) -> Self {
        Self {
            item,
            status: IngestStatus::Duplicate,
            error: None,
            duration_ms: 0,
            asset_id: Some(existing_asset_id.into()),
        }
    }
}

/// Validates individual ingest items before they are processed
pub struct IngestValidator;

impl IngestValidator {
    /// Validate that a path string is syntactically acceptable.
    ///
    /// Returns `Ok(())` if valid, or `Err(reason)` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error string if the path is empty or contains
    /// characters that are not allowed in a file path.
    pub fn validate_path(path: &str) -> Result<(), String> {
        if path.is_empty() {
            return Err("Path must not be empty".to_string());
        }

        // Reject null bytes
        if path.contains('\0') {
            return Err("Path contains null bytes".to_string());
        }

        // Reject path traversal attempts
        if path.contains("..") {
            return Err("Path contains directory traversal sequence (..)".to_string());
        }

        Ok(())
    }

    /// Check whether a checksum is already present in the list of existing checksums.
    ///
    /// Returns `true` if this is a duplicate.
    #[must_use]
    pub fn check_duplicate(checksum: &str, existing: &[String]) -> bool {
        existing.iter().any(|e| e == checksum)
    }
}

/// A plan for a batch ingest operation
#[derive(Debug, Clone)]
pub struct BatchIngestPlan {
    /// Items to be ingested
    pub items: Vec<IngestItem>,
    /// Total estimated size of all source files in bytes
    pub total_size_bytes: u64,
    /// Estimated total duration in seconds
    pub estimated_duration_secs: f64,
}

impl BatchIngestPlan {
    /// Create a batch ingest plan
    #[must_use]
    pub fn new(
        items: Vec<IngestItem>,
        total_size_bytes: u64,
        estimated_duration_secs: f64,
    ) -> Self {
        Self {
            items,
            total_size_bytes,
            estimated_duration_secs,
        }
    }

    /// Return the number of items in the plan
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Estimate throughput in MB/s
    #[must_use]
    pub fn estimated_throughput_mbps(&self) -> f64 {
        if self.estimated_duration_secs <= 0.0 {
            return 0.0;
        }
        (self.total_size_bytes as f64 / 1_048_576.0) / self.estimated_duration_secs
    }
}

/// Summary report for a completed batch ingest
#[derive(Debug, Clone)]
pub struct BatchIngestReport {
    /// Individual results for every item in the batch
    pub results: Vec<IngestResult>,
    /// Number of successfully ingested items
    pub success_count: u32,
    /// Number of items that failed
    pub failed_count: u32,
    /// Total number of new assets created in the MAM
    pub total_assets_created: u32,
}

impl BatchIngestReport {
    /// Build a report from a list of results
    #[must_use]
    pub fn from_results(results: Vec<IngestResult>) -> Self {
        let success_count = results.iter().filter(|r| r.status.is_success()).count() as u32;
        let failed_count = results
            .iter()
            .filter(|r| r.status == IngestStatus::Failed)
            .count() as u32;
        let total_assets_created = results
            .iter()
            .filter(|r| r.status.is_success() && r.asset_id.is_some())
            .count() as u32;

        Self {
            results,
            success_count,
            failed_count,
            total_assets_created,
        }
    }

    /// Returns the fraction of items that succeeded (0.0 – 1.0)
    #[must_use]
    pub fn success_rate(&self) -> f32 {
        let total = self.results.len();
        if total == 0 {
            return 1.0;
        }
        self.success_count as f32 / total as f32
    }

    /// Returns the number of skipped items
    #[must_use]
    pub fn skipped_count(&self) -> u32 {
        self.results
            .iter()
            .filter(|r| r.status == IngestStatus::Skipped)
            .count() as u32
    }

    /// Returns the number of duplicate items
    #[must_use]
    pub fn duplicate_count(&self) -> u32 {
        self.results
            .iter()
            .filter(|r| r.status == IngestStatus::Duplicate)
            .count() as u32
    }
}

/// Priority queue for ingest items (higher priority value = processed sooner)
#[derive(Debug, Default)]
pub struct IngestQueue {
    /// Items stored in the queue (not sorted; popping performs a linear scan)
    items: Vec<IngestItem>,
}

impl IngestQueue {
    /// Create a new empty queue
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an item onto the queue
    pub fn push(&mut self, item: IngestItem) {
        self.items.push(item);
    }

    /// Pop the highest-priority item from the queue.
    ///
    /// When multiple items share the same priority, the one inserted
    /// earliest is returned first (stable FIFO within a priority level).
    #[must_use]
    pub fn pop(&mut self) -> Option<IngestItem> {
        if self.items.is_empty() {
            return None;
        }

        // Find the index of the highest-priority item (stable: first occurrence wins on tie)
        let idx = self
            .items
            .iter()
            .enumerate()
            .max_by(|(ia, a), (ib, b)| {
                a.priority.cmp(&b.priority).then_with(|| ib.cmp(ia)) // earlier index wins on tie
            })
            .map(|(i, _)| i)?;

        Some(self.items.remove(idx))
    }

    /// Return the number of items currently in the queue
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Return `true` if the queue is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Peek at the highest-priority item without removing it
    #[must_use]
    pub fn peek(&self) -> Option<&IngestItem> {
        self.items.iter().max_by(|a, b| a.priority.cmp(&b.priority))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &str, priority: u8) -> IngestItem {
        IngestItem::new(path, "/dest", priority)
    }

    #[test]
    fn test_ingest_status_is_success() {
        assert!(IngestStatus::Success.is_success());
        assert!(!IngestStatus::Failed.is_success());
        assert!(!IngestStatus::Skipped.is_success());
        assert!(!IngestStatus::Duplicate.is_success());
    }

    #[test]
    fn test_ingest_status_labels() {
        assert_eq!(IngestStatus::Success.label(), "success");
        assert_eq!(IngestStatus::Failed.label(), "failed");
        assert_eq!(IngestStatus::Skipped.label(), "skipped");
        assert_eq!(IngestStatus::Duplicate.label(), "duplicate");
    }

    #[test]
    fn test_ingest_result_success() {
        let r = IngestResult::success(item("a.mp4", 5), 200, "asset-001".to_string());
        assert!(r.status.is_success());
        assert_eq!(r.asset_id.as_deref(), Some("asset-001"));
        assert!(r.error.is_none());
    }

    #[test]
    fn test_ingest_result_failed() {
        let r = IngestResult::failed(item("b.mp4", 5), 100, "disk full");
        assert_eq!(r.status, IngestStatus::Failed);
        assert!(r.error.is_some());
    }

    #[test]
    fn test_ingest_result_skipped() {
        let r = IngestResult::skipped(item("c.xyz", 5), "unsupported format");
        assert_eq!(r.status, IngestStatus::Skipped);
    }

    #[test]
    fn test_ingest_result_duplicate() {
        let r = IngestResult::duplicate(item("d.mp4", 5), "existing-123");
        assert_eq!(r.status, IngestStatus::Duplicate);
        assert_eq!(r.asset_id.as_deref(), Some("existing-123"));
    }

    #[test]
    fn test_validator_validate_path_ok() {
        assert!(IngestValidator::validate_path("/media/foo/bar.mp4").is_ok());
    }

    #[test]
    fn test_validator_validate_path_empty() {
        assert!(IngestValidator::validate_path("").is_err());
    }

    #[test]
    fn test_validator_validate_path_traversal() {
        assert!(IngestValidator::validate_path("/media/../secret").is_err());
    }

    #[test]
    fn test_validator_check_duplicate_found() {
        let existing = vec!["abc123".to_string(), "def456".to_string()];
        assert!(IngestValidator::check_duplicate("abc123", &existing));
    }

    #[test]
    fn test_validator_check_duplicate_not_found() {
        let existing = vec!["abc123".to_string()];
        assert!(!IngestValidator::check_duplicate("xyz999", &existing));
    }

    #[test]
    fn test_batch_ingest_report_success_rate() {
        let results = vec![
            IngestResult::success(item("a.mp4", 5), 100, "a1".to_string()),
            IngestResult::success(item("b.mp4", 5), 100, "a2".to_string()),
            IngestResult::failed(item("c.mp4", 5), 50, "error"),
        ];
        let report = BatchIngestReport::from_results(results);
        assert_eq!(report.success_count, 2);
        assert_eq!(report.failed_count, 1);
        assert!((report.success_rate() - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_batch_ingest_report_empty() {
        let report = BatchIngestReport::from_results(vec![]);
        assert_eq!(report.success_rate(), 1.0);
    }

    #[test]
    fn test_ingest_queue_priority_ordering() {
        let mut q = IngestQueue::new();
        q.push(item("low.mp4", 1));
        q.push(item("high.mp4", 10));
        q.push(item("mid.mp4", 5));

        let first = q.pop().expect("should succeed in test");
        assert_eq!(first.path, "high.mp4");
        let second = q.pop().expect("should succeed in test");
        assert_eq!(second.path, "mid.mp4");
        let third = q.pop().expect("should succeed in test");
        assert_eq!(third.path, "low.mp4");
        assert!(q.pop().is_none());
    }

    #[test]
    fn test_ingest_queue_len_and_empty() {
        let mut q = IngestQueue::new();
        assert!(q.is_empty());
        q.push(item("a.mp4", 5));
        assert_eq!(q.len(), 1);
        let _ = q.pop();
        assert!(q.is_empty());
    }
}
