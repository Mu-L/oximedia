#![allow(dead_code)]
//! Scheduled integrity scanning for archived media
//!
//! Provides periodic full-archive and incremental integrity scans. Detects
//! bit-rot, silent data corruption, and missing files. Reports per-file
//! status and aggregate health metrics.

use std::collections::HashMap;
use std::fmt;

/// Status of a single file after an integrity scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileIntegrity {
    /// File is intact.
    Ok,
    /// File has been modified since last scan.
    Modified,
    /// File is missing from storage.
    Missing,
    /// File is corrupted (checksum mismatch).
    Corrupted,
    /// File has not yet been scanned.
    Unknown,
}

impl fmt::Display for FileIntegrity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Ok => "OK",
            Self::Modified => "MODIFIED",
            Self::Missing => "MISSING",
            Self::Corrupted => "CORRUPTED",
            Self::Unknown => "UNKNOWN",
        };
        write!(f, "{s}")
    }
}

/// Policy for how often and what to scan.
#[derive(Debug, Clone)]
pub struct ScanPolicy {
    /// Interval between full scans in hours.
    pub full_scan_interval_hours: u32,
    /// Interval between incremental scans in hours.
    pub incremental_interval_hours: u32,
    /// Maximum number of files per incremental scan batch.
    pub incremental_batch_size: usize,
    /// Whether to halt on first corruption found.
    pub stop_on_first_error: bool,
    /// Number of parallel scan threads.
    pub parallelism: usize,
}

impl Default for ScanPolicy {
    fn default() -> Self {
        Self {
            full_scan_interval_hours: 168, // weekly
            incremental_interval_hours: 24,
            incremental_batch_size: 1000,
            stop_on_first_error: false,
            parallelism: 4,
        }
    }
}

/// Record of a single file's scan result.
#[derive(Debug, Clone)]
pub struct FileScanRecord {
    /// File path.
    pub path: String,
    /// Expected checksum.
    pub expected_checksum: String,
    /// Actual checksum computed during scan.
    pub actual_checksum: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Integrity status.
    pub status: FileIntegrity,
    /// Scan timestamp in epoch milliseconds.
    pub scanned_at_ms: u64,
}

impl FileScanRecord {
    /// Create a new scan record.
    pub fn new(
        path: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
        size_bytes: u64,
        scanned_at_ms: u64,
    ) -> Self {
        let expected_checksum = expected.into();
        let actual_checksum = actual.into();
        let status = if expected_checksum == actual_checksum {
            FileIntegrity::Ok
        } else {
            FileIntegrity::Corrupted
        };
        Self {
            path: path.into(),
            expected_checksum,
            actual_checksum,
            size_bytes,
            status,
            scanned_at_ms,
        }
    }

    /// Create a record for a missing file.
    pub fn missing(path: impl Into<String>, scanned_at_ms: u64) -> Self {
        Self {
            path: path.into(),
            expected_checksum: String::new(),
            actual_checksum: String::new(),
            size_bytes: 0,
            status: FileIntegrity::Missing,
            scanned_at_ms,
        }
    }
}

/// Aggregate health metrics from a scan.
#[derive(Debug, Clone)]
pub struct ScanHealthMetrics {
    /// Total files scanned.
    pub total_scanned: usize,
    /// Number of OK files.
    pub ok_count: usize,
    /// Number of corrupted files.
    pub corrupted_count: usize,
    /// Number of missing files.
    pub missing_count: usize,
    /// Number of modified files.
    pub modified_count: usize,
    /// Total bytes scanned.
    pub total_bytes_scanned: u64,
    /// Duration of the scan in milliseconds.
    pub duration_ms: u64,
}

impl ScanHealthMetrics {
    /// Compute the health score as a fraction from 0.0 to 1.0.
    #[allow(clippy::cast_precision_loss)]
    pub fn health_score(&self) -> f64 {
        if self.total_scanned == 0 {
            return 1.0;
        }
        self.ok_count as f64 / self.total_scanned as f64
    }

    /// Whether the archive is considered healthy (score >= threshold).
    #[allow(clippy::cast_precision_loss)]
    pub fn is_healthy(&self, threshold: f64) -> bool {
        self.health_score() >= threshold
    }
}

/// An integrity scan session that accumulates file scan records.
#[derive(Debug)]
pub struct IntegrityScan {
    /// Scan policy.
    policy: ScanPolicy,
    /// All file scan records.
    records: Vec<FileScanRecord>,
    /// Start time in epoch ms.
    start_ms: u64,
    /// End time in epoch ms (0 if not finished).
    end_ms: u64,
}

impl IntegrityScan {
    /// Create a new integrity scan session.
    pub fn new(policy: ScanPolicy, start_ms: u64) -> Self {
        Self {
            policy,
            records: Vec::new(),
            start_ms,
            end_ms: 0,
        }
    }

    /// Create with default policy.
    pub fn with_defaults(start_ms: u64) -> Self {
        Self::new(ScanPolicy::default(), start_ms)
    }

    /// Get the policy.
    pub fn policy(&self) -> &ScanPolicy {
        &self.policy
    }

    /// Add a file scan record.
    pub fn add_record(&mut self, record: FileScanRecord) {
        self.records.push(record);
    }

    /// Number of records collected so far.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Mark the scan as finished.
    pub fn finish(&mut self, end_ms: u64) {
        self.end_ms = end_ms;
    }

    /// Whether the scan has finished.
    pub fn is_finished(&self) -> bool {
        self.end_ms > 0
    }

    /// Get all records.
    pub fn records(&self) -> &[FileScanRecord] {
        &self.records
    }

    /// Get only corrupted records.
    pub fn corrupted(&self) -> Vec<&FileScanRecord> {
        self.records
            .iter()
            .filter(|r| r.status == FileIntegrity::Corrupted)
            .collect()
    }

    /// Get only missing records.
    pub fn missing(&self) -> Vec<&FileScanRecord> {
        self.records
            .iter()
            .filter(|r| r.status == FileIntegrity::Missing)
            .collect()
    }

    /// Compute aggregate health metrics.
    pub fn metrics(&self) -> ScanHealthMetrics {
        let mut ok = 0usize;
        let mut corrupted = 0usize;
        let mut missing = 0usize;
        let mut modified = 0usize;
        let mut total_bytes = 0u64;
        for r in &self.records {
            match r.status {
                FileIntegrity::Ok => ok += 1,
                FileIntegrity::Corrupted => corrupted += 1,
                FileIntegrity::Missing => missing += 1,
                FileIntegrity::Modified => modified += 1,
                FileIntegrity::Unknown => {}
            }
            total_bytes += r.size_bytes;
        }
        ScanHealthMetrics {
            total_scanned: self.records.len(),
            ok_count: ok,
            corrupted_count: corrupted,
            missing_count: missing,
            modified_count: modified,
            total_bytes_scanned: total_bytes,
            duration_ms: self.end_ms.saturating_sub(self.start_ms),
        }
    }

    /// Group records by integrity status.
    pub fn group_by_status(&self) -> HashMap<String, Vec<&FileScanRecord>> {
        let mut map: HashMap<String, Vec<&FileScanRecord>> = HashMap::new();
        for r in &self.records {
            map.entry(r.status.to_string()).or_default().push(r);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_integrity_display() {
        assert_eq!(FileIntegrity::Ok.to_string(), "OK");
        assert_eq!(FileIntegrity::Corrupted.to_string(), "CORRUPTED");
        assert_eq!(FileIntegrity::Missing.to_string(), "MISSING");
        assert_eq!(FileIntegrity::Modified.to_string(), "MODIFIED");
        assert_eq!(FileIntegrity::Unknown.to_string(), "UNKNOWN");
    }

    #[test]
    fn test_default_scan_policy() {
        let p = ScanPolicy::default();
        assert_eq!(p.full_scan_interval_hours, 168);
        assert_eq!(p.incremental_interval_hours, 24);
        assert!(!p.stop_on_first_error);
    }

    #[test]
    fn test_scan_record_ok() {
        let r = FileScanRecord::new("/a.mxf", "abc", "abc", 1024, 1000);
        assert_eq!(r.status, FileIntegrity::Ok);
        assert_eq!(r.size_bytes, 1024);
    }

    #[test]
    fn test_scan_record_corrupted() {
        let r = FileScanRecord::new("/a.mxf", "abc", "xyz", 1024, 1000);
        assert_eq!(r.status, FileIntegrity::Corrupted);
    }

    #[test]
    fn test_scan_record_missing() {
        let r = FileScanRecord::missing("/gone.mxf", 2000);
        assert_eq!(r.status, FileIntegrity::Missing);
        assert_eq!(r.size_bytes, 0);
    }

    #[test]
    fn test_new_scan_not_finished() {
        let scan = IntegrityScan::with_defaults(1000);
        assert!(!scan.is_finished());
        assert_eq!(scan.record_count(), 0);
    }

    #[test]
    fn test_scan_add_record_and_finish() {
        let mut scan = IntegrityScan::with_defaults(1000);
        scan.add_record(FileScanRecord::new("/a.mxf", "aa", "aa", 500, 1001));
        scan.finish(2000);
        assert!(scan.is_finished());
        assert_eq!(scan.record_count(), 1);
    }

    #[test]
    fn test_scan_corrupted_filter() {
        let mut scan = IntegrityScan::with_defaults(0);
        scan.add_record(FileScanRecord::new("/ok.mxf", "aa", "aa", 100, 1));
        scan.add_record(FileScanRecord::new("/bad.mxf", "aa", "bb", 200, 2));
        assert_eq!(scan.corrupted().len(), 1);
        assert_eq!(scan.corrupted()[0].path, "/bad.mxf");
    }

    #[test]
    fn test_scan_missing_filter() {
        let mut scan = IntegrityScan::with_defaults(0);
        scan.add_record(FileScanRecord::missing("/gone.mxf", 1));
        scan.add_record(FileScanRecord::new("/ok.mxf", "aa", "aa", 100, 2));
        assert_eq!(scan.missing().len(), 1);
    }

    #[test]
    fn test_metrics_all_ok() {
        let mut scan = IntegrityScan::with_defaults(100);
        scan.add_record(FileScanRecord::new("/a.mxf", "aa", "aa", 100, 101));
        scan.add_record(FileScanRecord::new("/b.mxf", "bb", "bb", 200, 102));
        scan.finish(200);
        let m = scan.metrics();
        assert_eq!(m.total_scanned, 2);
        assert_eq!(m.ok_count, 2);
        assert_eq!(m.corrupted_count, 0);
        assert!((m.health_score() - 1.0).abs() < f64::EPSILON);
        assert_eq!(m.duration_ms, 100);
        assert_eq!(m.total_bytes_scanned, 300);
    }

    #[test]
    fn test_metrics_with_corruption() {
        let mut scan = IntegrityScan::with_defaults(0);
        scan.add_record(FileScanRecord::new("/a.mxf", "aa", "aa", 100, 1));
        scan.add_record(FileScanRecord::new("/b.mxf", "bb", "xx", 200, 2));
        let m = scan.metrics();
        assert_eq!(m.ok_count, 1);
        assert_eq!(m.corrupted_count, 1);
        assert!((m.health_score() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_empty_scan_healthy() {
        let scan = IntegrityScan::with_defaults(0);
        let m = scan.metrics();
        assert!((m.health_score() - 1.0).abs() < f64::EPSILON);
        assert!(m.is_healthy(0.99));
    }

    #[test]
    fn test_is_healthy_threshold() {
        let mut scan = IntegrityScan::with_defaults(0);
        for i in 0..9 {
            scan.add_record(FileScanRecord::new(
                format!("/ok_{i}.mxf"),
                "aa",
                "aa",
                100,
                1,
            ));
        }
        scan.add_record(FileScanRecord::new("/bad.mxf", "aa", "xx", 100, 1));
        let m = scan.metrics();
        assert!(m.is_healthy(0.8));
        assert!(!m.is_healthy(0.95));
    }

    #[test]
    fn test_group_by_status() {
        let mut scan = IntegrityScan::with_defaults(0);
        scan.add_record(FileScanRecord::new("/ok.mxf", "aa", "aa", 100, 1));
        scan.add_record(FileScanRecord::new("/bad.mxf", "aa", "xx", 100, 2));
        scan.add_record(FileScanRecord::missing("/gone.mxf", 3));
        let groups = scan.group_by_status();
        assert_eq!(groups.get("OK").map(|v| v.len()), Some(1));
        assert_eq!(groups.get("CORRUPTED").map(|v| v.len()), Some(1));
        assert_eq!(groups.get("MISSING").map(|v| v.len()), Some(1));
    }
}
