//! Deduplication reporting: statistics, summaries, and formatted reports.

#![allow(dead_code)]

use std::collections::HashMap;

// ── DedupStat ────────────────────────────────────────────────────────────────

/// A single deduplication statistic value.
#[derive(Debug, Clone, PartialEq)]
pub enum DedupStat {
    /// A count of items.
    Count(u64),
    /// A size in bytes.
    Bytes(u64),
    /// A ratio (0.0 – 1.0).
    Ratio(f64),
    /// A plain label.
    Label(String),
}

impl DedupStat {
    /// Returns a human-readable name describing this statistic's type.
    #[must_use]
    pub fn stat_name(&self) -> &'static str {
        match self {
            Self::Count(_) => "count",
            Self::Bytes(_) => "bytes",
            Self::Ratio(_) => "ratio",
            Self::Label(_) => "label",
        }
    }

    /// Returns the inner u64 for a `Count` or `Bytes` variant, or `None`.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Count(v) | Self::Bytes(v) => Some(*v),
            _ => None,
        }
    }

    /// Returns the inner f64 for a `Ratio` variant, or `None`.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Ratio(v) => Some(*v),
            _ => None,
        }
    }
}

// ── DedupSummary ──────────────────────────────────────────────────────────────

/// High-level summary computed from deduplication statistics.
#[derive(Debug, Clone)]
pub struct DedupSummary {
    /// Total files examined.
    pub total_files: u64,
    /// Number of unique files.
    pub unique_files: u64,
    /// Number of duplicate files (total - unique).
    pub duplicate_files: u64,
    /// Total size of all files in bytes.
    pub total_bytes: u64,
    /// Bytes that can be reclaimed by removing duplicates.
    pub reclaimable_bytes: u64,
}

impl DedupSummary {
    /// Returns the number of bytes that would be saved by removing duplicates.
    #[must_use]
    pub fn space_saved_bytes(&self) -> u64 {
        self.reclaimable_bytes
    }

    /// Returns the fraction of storage consumed by duplicates (0.0 – 1.0).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn duplication_ratio(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        self.reclaimable_bytes as f64 / self.total_bytes as f64
    }

    /// Returns the number of duplicate files.
    #[must_use]
    pub fn duplicate_count(&self) -> u64 {
        self.duplicate_files
    }

    /// Returns a human-readable description.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn description(&self) -> String {
        format!(
            "{} / {} files are duplicates ({:.1}% duplication); {} bytes reclaimable",
            self.duplicate_files,
            self.total_files,
            self.duplication_ratio() * 100.0,
            self.reclaimable_bytes,
        )
    }
}

impl Default for DedupSummary {
    fn default() -> Self {
        Self {
            total_files: 0,
            unique_files: 0,
            duplicate_files: 0,
            total_bytes: 0,
            reclaimable_bytes: 0,
        }
    }
}

// ── DedupReport ───────────────────────────────────────────────────────────────

/// A complete deduplication report aggregating named statistics.
#[derive(Debug, Default)]
pub struct DedupReport {
    stats: HashMap<String, DedupStat>,
}

impl DedupReport {
    /// Create an empty report.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a named statistic to the report (overwrites if name exists).
    pub fn add_stat(&mut self, name: impl Into<String>, stat: DedupStat) {
        self.stats.insert(name.into(), stat);
    }

    /// Retrieve a statistic by name.
    #[must_use]
    pub fn get_stat(&self, name: &str) -> Option<&DedupStat> {
        self.stats.get(name)
    }

    /// Returns the total number of statistics stored.
    #[must_use]
    pub fn stat_count(&self) -> usize {
        self.stats.len()
    }

    /// Generate a `DedupSummary` from the standard named statistics.
    ///
    /// Expects the following keys to be present (as `Count` or `Bytes`):
    /// - `"total_files"` – `Count`
    /// - `"unique_files"` – `Count`
    /// - `"total_bytes"` – `Bytes`
    /// - `"reclaimable_bytes"` – `Bytes`
    #[must_use]
    pub fn generate_summary(&self) -> DedupSummary {
        let total_files = self
            .stats
            .get("total_files")
            .and_then(DedupStat::as_u64)
            .unwrap_or(0);
        let unique_files = self
            .stats
            .get("unique_files")
            .and_then(DedupStat::as_u64)
            .unwrap_or(0);
        let total_bytes = self
            .stats
            .get("total_bytes")
            .and_then(DedupStat::as_u64)
            .unwrap_or(0);
        let reclaimable_bytes = self
            .stats
            .get("reclaimable_bytes")
            .and_then(DedupStat::as_u64)
            .unwrap_or(0);

        DedupSummary {
            total_files,
            unique_files,
            duplicate_files: total_files.saturating_sub(unique_files),
            total_bytes,
            reclaimable_bytes,
        }
    }

    /// Return all stat names, sorted alphabetically.
    #[must_use]
    pub fn stat_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.stats.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup_stat_name_count() {
        let s = DedupStat::Count(42);
        assert_eq!(s.stat_name(), "count");
    }

    #[test]
    fn test_dedup_stat_name_bytes() {
        let s = DedupStat::Bytes(1024);
        assert_eq!(s.stat_name(), "bytes");
    }

    #[test]
    fn test_dedup_stat_name_ratio() {
        let s = DedupStat::Ratio(0.5);
        assert_eq!(s.stat_name(), "ratio");
    }

    #[test]
    fn test_dedup_stat_name_label() {
        let s = DedupStat::Label("hello".to_string());
        assert_eq!(s.stat_name(), "label");
    }

    #[test]
    fn test_dedup_stat_as_u64() {
        assert_eq!(DedupStat::Count(99).as_u64(), Some(99));
        assert_eq!(DedupStat::Bytes(512).as_u64(), Some(512));
        assert_eq!(DedupStat::Ratio(0.5).as_u64(), None);
    }

    #[test]
    fn test_dedup_stat_as_f64() {
        assert_eq!(DedupStat::Ratio(0.75).as_f64(), Some(0.75));
        assert_eq!(DedupStat::Count(10).as_f64(), None);
    }

    #[test]
    fn test_summary_space_saved_bytes() {
        let s = DedupSummary {
            total_files: 100,
            unique_files: 60,
            duplicate_files: 40,
            total_bytes: 10_000,
            reclaimable_bytes: 4_000,
        };
        assert_eq!(s.space_saved_bytes(), 4_000);
    }

    #[test]
    fn test_summary_duplication_ratio() {
        let s = DedupSummary {
            total_files: 100,
            unique_files: 50,
            duplicate_files: 50,
            total_bytes: 1_000,
            reclaimable_bytes: 500,
        };
        assert!((s.duplication_ratio() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_summary_duplication_ratio_zero_bytes() {
        let s = DedupSummary::default();
        assert_eq!(s.duplication_ratio(), 0.0);
    }

    #[test]
    fn test_report_add_and_get_stat() {
        let mut r = DedupReport::new();
        r.add_stat("total_files", DedupStat::Count(200));
        assert_eq!(r.get_stat("total_files"), Some(&DedupStat::Count(200)));
        assert_eq!(r.get_stat("nonexistent"), None);
    }

    #[test]
    fn test_report_stat_count() {
        let mut r = DedupReport::new();
        r.add_stat("a", DedupStat::Count(1));
        r.add_stat("b", DedupStat::Bytes(2));
        assert_eq!(r.stat_count(), 2);
    }

    #[test]
    fn test_report_generate_summary() {
        let mut r = DedupReport::new();
        r.add_stat("total_files", DedupStat::Count(100));
        r.add_stat("unique_files", DedupStat::Count(70));
        r.add_stat("total_bytes", DedupStat::Bytes(10_000));
        r.add_stat("reclaimable_bytes", DedupStat::Bytes(3_000));
        let s = r.generate_summary();
        assert_eq!(s.total_files, 100);
        assert_eq!(s.unique_files, 70);
        assert_eq!(s.duplicate_files, 30);
        assert_eq!(s.space_saved_bytes(), 3_000);
    }

    #[test]
    fn test_report_stat_names_sorted() {
        let mut r = DedupReport::new();
        r.add_stat("zebra", DedupStat::Count(1));
        r.add_stat("apple", DedupStat::Count(2));
        r.add_stat("mango", DedupStat::Count(3));
        let names = r.stat_names();
        assert_eq!(names, vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn test_report_overwrite_stat() {
        let mut r = DedupReport::new();
        r.add_stat("x", DedupStat::Count(1));
        r.add_stat("x", DedupStat::Count(99));
        assert_eq!(r.get_stat("x"), Some(&DedupStat::Count(99)));
        assert_eq!(r.stat_count(), 1);
    }

    #[test]
    fn test_summary_description_contains_percentage() {
        let s = DedupSummary {
            total_files: 10,
            unique_files: 8,
            duplicate_files: 2,
            total_bytes: 1000,
            reclaimable_bytes: 200,
        };
        let desc = s.description();
        assert!(desc.contains('%'));
        assert!(desc.contains("200"));
    }
}
