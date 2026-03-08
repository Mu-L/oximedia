//! Subtitle index for efficient timestamp-based lookup.
//!
//! Provides a binary-search-backed index over subtitle cues so callers
//! can quickly find all cues that are active at any given playback
//! position without iterating the entire subtitle list linearly.

#![allow(dead_code)]

/// A single entry stored in the index, pairing a timestamp range with
/// the position of the corresponding cue in the source list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// Cue start time in milliseconds.
    pub start_ms: i64,
    /// Cue end time in milliseconds.
    pub end_ms: i64,
    /// Position (zero-based) of this cue in the original subtitle list.
    pub cue_index: usize,
}

impl IndexEntry {
    /// Create a new `IndexEntry`.
    #[must_use]
    pub fn new(start_ms: i64, end_ms: i64, cue_index: usize) -> Self {
        Self {
            start_ms,
            end_ms,
            cue_index,
        }
    }

    /// Returns `true` if the given timestamp falls within `[start_ms, end_ms)`.
    #[must_use]
    pub fn is_active_at(&self, timestamp_ms: i64) -> bool {
        timestamp_ms >= self.start_ms && timestamp_ms < self.end_ms
    }
}

/// An index over a subtitle track that supports O(log n) lookups by
/// timestamp via binary search.
///
/// # Example
///
/// ```
/// use oximedia_subtitle::subtitle_index::{IndexEntry, SubtitleIndex};
///
/// let mut idx = SubtitleIndex::new();
/// idx.push(IndexEntry::new(0, 3000, 0));
/// idx.push(IndexEntry::new(3000, 6000, 1));
/// idx.build();
///
/// let active = idx.active_at(1500);
/// assert_eq!(active.len(), 1);
/// assert_eq!(active[0].cue_index, 0);
/// ```
#[derive(Debug, Default)]
pub struct SubtitleIndex {
    entries: Vec<IndexEntry>,
    /// Whether entries are currently sorted (index is "built").
    is_built: bool,
}

impl SubtitleIndex {
    /// Create an empty `SubtitleIndex`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            is_built: false,
        }
    }

    /// Construct an index directly from a slice of `(start_ms, end_ms)` pairs,
    /// assigning cue indices automatically.
    #[must_use]
    pub fn from_ranges(ranges: &[(i64, i64)]) -> Self {
        let mut idx = Self::new();
        for (i, &(start, end)) in ranges.iter().enumerate() {
            idx.entries.push(IndexEntry::new(start, end, i));
        }
        idx.build();
        idx
    }

    /// Append an entry.  The caller must call [`build`](Self::build) before
    /// querying if entries are not appended in sorted order.
    pub fn push(&mut self, entry: IndexEntry) {
        self.is_built = false;
        self.entries.push(entry);
    }

    /// Sort entries by `start_ms` so binary search can be used.
    pub fn build(&mut self) {
        self.entries.sort_by_key(|e| e.start_ms);
        self.is_built = true;
    }

    /// Return the number of indexed entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the index contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Find all cue entries whose time range contains `timestamp_ms`.
    ///
    /// Uses binary search to locate the candidate region and then expands
    /// left/right to collect all overlapping cues, so the operation is
    /// O(log n + k) where k is the number of matches.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if [`build`](Self::build) has not been called
    /// after the last [`push`](Self::push).
    #[must_use]
    pub fn active_at(&self, timestamp_ms: i64) -> Vec<&IndexEntry> {
        debug_assert!(self.is_built, "call build() before querying");

        if self.entries.is_empty() {
            return Vec::new();
        }

        // Binary search for the rightmost entry with start_ms <= timestamp_ms.
        let pivot = self.entries.partition_point(|e| e.start_ms <= timestamp_ms);

        let mut results = Vec::new();

        // Scan backwards from pivot to find all overlapping entries.
        // An entry overlaps if start_ms <= timestamp_ms AND end_ms > timestamp_ms.
        let scan_end = pivot;
        let mut i = scan_end;
        while i > 0 {
            i -= 1;
            let e = &self.entries[i];
            if e.start_ms > timestamp_ms {
                continue;
            }
            if e.end_ms > timestamp_ms {
                results.push(e);
            }
            // Entries before this can only have smaller start_ms; we still
            // need to check them all because overlapping cues are possible.
        }

        // Sort results by cue_index for deterministic output.
        results.sort_by_key(|e| e.cue_index);
        results
    }

    /// Return all entries whose time range intersects `[from_ms, to_ms)`.
    #[must_use]
    pub fn range_query(&self, from_ms: i64, to_ms: i64) -> Vec<&IndexEntry> {
        self.entries
            .iter()
            .filter(|e| e.start_ms < to_ms && e.end_ms > from_ms)
            .collect()
    }

    /// Return the entry with the earliest `start_ms`, or `None` if empty.
    #[must_use]
    pub fn first_entry(&self) -> Option<&IndexEntry> {
        self.entries.first()
    }

    /// Return the entry with the latest `start_ms`, or `None` if empty.
    #[must_use]
    pub fn last_entry(&self) -> Option<&IndexEntry> {
        self.entries.last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_index() -> SubtitleIndex {
        SubtitleIndex::from_ranges(&[
            (0, 2000),
            (2000, 4000),
            (4000, 6000),
            (5000, 7000), // overlaps with previous
            (8000, 10000),
        ])
    }

    #[test]
    fn test_new_is_empty() {
        let idx = SubtitleIndex::new();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
    }

    #[test]
    fn test_from_ranges_length() {
        let idx = sample_index();
        assert_eq!(idx.len(), 5);
    }

    #[test]
    fn test_active_at_first_cue() {
        let idx = sample_index();
        let hits = idx.active_at(1000);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].cue_index, 0);
    }

    #[test]
    fn test_active_at_second_cue() {
        let idx = sample_index();
        let hits = idx.active_at(3000);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].cue_index, 1);
    }

    #[test]
    fn test_active_at_overlap() {
        let idx = sample_index();
        // timestamp 5500 is inside cue 2 (4000-6000) AND cue 3 (5000-7000).
        let hits = idx.active_at(5500);
        assert_eq!(hits.len(), 2);
        let indices: Vec<usize> = hits.iter().map(|e| e.cue_index).collect();
        assert!(indices.contains(&2));
        assert!(indices.contains(&3));
    }

    #[test]
    fn test_active_at_gap() {
        let idx = sample_index();
        // timestamp 7500 is between cues.
        let hits = idx.active_at(7500);
        assert!(hits.is_empty());
    }

    #[test]
    fn test_active_at_before_start() {
        let idx = sample_index();
        // timestamp < 0 — nothing active.
        let hits = idx.active_at(-100);
        assert!(hits.is_empty());
    }

    #[test]
    fn test_active_at_end_exclusive() {
        let idx = sample_index();
        // End time is exclusive: at t=2000 the first cue should NOT be active.
        let hits = idx.active_at(2000);
        assert!(!hits.iter().any(|e| e.cue_index == 0));
    }

    #[test]
    fn test_range_query_basic() {
        let idx = sample_index();
        let hits = idx.range_query(3500, 5500);
        // Should include cues that start before 5500 and end after 3500.
        let indices: Vec<usize> = hits.iter().map(|e| e.cue_index).collect();
        assert!(indices.contains(&1)); // 2000-4000 ends at 4000 > 3500
        assert!(indices.contains(&2)); // 4000-6000
        assert!(indices.contains(&3)); // 5000-7000 starts at 5000 < 5500
    }

    #[test]
    fn test_range_query_no_match() {
        let idx = sample_index();
        let hits = idx.range_query(10000, 12000);
        assert!(hits.is_empty());
    }

    #[test]
    fn test_first_last_entry() {
        let idx = sample_index();
        assert_eq!(
            idx.first_entry().expect("should succeed in test").cue_index,
            0
        );
        // last entry by start_ms is cue 4 (8000-10000)
        assert_eq!(
            idx.last_entry().expect("should succeed in test").cue_index,
            4
        );
    }

    #[test]
    fn test_index_entry_is_active_at() {
        let e = IndexEntry::new(1000, 3000, 0);
        assert!(e.is_active_at(1000));
        assert!(e.is_active_at(2999));
        assert!(!e.is_active_at(3000));
        assert!(!e.is_active_at(999));
    }

    #[test]
    fn test_push_and_build() {
        let mut idx = SubtitleIndex::new();
        idx.push(IndexEntry::new(5000, 8000, 1));
        idx.push(IndexEntry::new(0, 3000, 0));
        idx.build();
        // After build, entries should be sorted.
        assert_eq!(
            idx.first_entry().expect("should succeed in test").start_ms,
            0
        );
        assert_eq!(
            idx.last_entry().expect("should succeed in test").start_ms,
            5000
        );
    }

    #[test]
    fn test_empty_index_active_at() {
        let idx = SubtitleIndex::new();
        // Build an empty index — should not panic.
        let mut idx2 = idx;
        idx2.build();
        assert!(idx2.active_at(0).is_empty());
    }
}
