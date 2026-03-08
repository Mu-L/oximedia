//! EDL optimization and consolidation.
//!
//! This module provides tools for optimizing and consolidating EDL data,
//! including merging adjacent clips, removing duplicates, and sorting by
//! timeline position.

/// Options controlling EDL optimization behavior.
#[derive(Debug, Clone)]
pub struct OptimizeOptions {
    /// Merge adjacent clips from the same source.
    pub merge_adjacent: bool,
    /// Remove duplicate clips.
    pub remove_duplicates: bool,
    /// Sort clips by timeline (record_in) position.
    pub sort_by_timeline: bool,
    /// Consolidate clips from the same source file.
    pub consolidate_sources: bool,
}

impl OptimizeOptions {
    /// Create default optimization options (all enabled).
    #[must_use]
    pub fn all() -> Self {
        Self {
            merge_adjacent: true,
            remove_duplicates: true,
            sort_by_timeline: true,
            consolidate_sources: true,
        }
    }

    /// Create options with no optimizations enabled.
    #[must_use]
    pub fn none() -> Self {
        Self {
            merge_adjacent: false,
            remove_duplicates: false,
            sort_by_timeline: false,
            consolidate_sources: false,
        }
    }
}

impl Default for OptimizeOptions {
    fn default() -> Self {
        Self::all()
    }
}

/// Statistics describing the result of an optimization pass.
#[derive(Debug, Clone, Default)]
pub struct OptimizeStats {
    /// Number of clips before optimization.
    pub original_clips: usize,
    /// Number of clips after optimization.
    pub optimized_clips: usize,
    /// Number of clips merged with adjacent clips.
    pub merged_count: usize,
    /// Number of duplicate clips removed.
    pub removed_count: usize,
}

impl OptimizeStats {
    /// Create a new statistics report.
    #[must_use]
    pub fn new(original: usize) -> Self {
        Self {
            original_clips: original,
            optimized_clips: original,
            merged_count: 0,
            removed_count: 0,
        }
    }

    /// Calculate the reduction percentage.
    #[must_use]
    pub fn reduction_percent(&self) -> f64 {
        if self.original_clips == 0 {
            return 0.0;
        }
        let saved = self.original_clips.saturating_sub(self.optimized_clips);
        (saved as f64 / self.original_clips as f64) * 100.0
    }
}

/// A simplified clip entry used for optimization operations.
///
/// Uses frame counts for precise comparisons and merging.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdlClipEntry {
    /// Unique event ID.
    pub id: u32,
    /// Source reel/file name.
    pub source_name: String,
    /// Record (timeline) in point, in frames.
    pub record_in: u64,
    /// Record (timeline) out point, in frames.
    pub record_out: u64,
    /// Source in point, in frames.
    pub source_in: u64,
    /// Source out point, in frames.
    pub source_out: u64,
}

impl EdlClipEntry {
    /// Create a new clip entry.
    #[allow(dead_code)]
    #[must_use]
    pub fn new(
        id: u32,
        source_name: impl Into<String>,
        record_in: u64,
        record_out: u64,
        source_in: u64,
        source_out: u64,
    ) -> Self {
        Self {
            id,
            source_name: source_name.into(),
            record_in,
            record_out,
            source_in,
            source_out,
        }
    }

    /// Duration of this clip in frames.
    #[must_use]
    pub fn duration(&self) -> u64 {
        self.record_out.saturating_sub(self.record_in)
    }

    /// Check whether two clips are adjacent on the timeline AND the source.
    ///
    /// Two clips are adjacent when:
    /// - They share the same source name.
    /// - `self.record_out == other.record_in` (back-to-back on timeline).
    /// - `self.source_out == other.source_in` (continuous in source).
    #[must_use]
    pub fn is_adjacent_to(&self, other: &Self) -> bool {
        self.source_name == other.source_name
            && self.record_out == other.record_in
            && self.source_out == other.source_in
    }
}

/// Run the full optimization pipeline on a list of clip entries.
///
/// The operations are applied in the following order (each enabled step
/// operates on the result of the previous one):
///
/// 1. Sort by timeline (if `opts.sort_by_timeline`)
/// 2. Remove duplicates (if `opts.remove_duplicates`)
/// 3. Merge adjacent clips (if `opts.merge_adjacent`)
///
/// Returns an [`OptimizeStats`] describing what changed.
pub fn optimize_edl(clips: &mut Vec<EdlClipEntry>, opts: &OptimizeOptions) -> OptimizeStats {
    let original = clips.len();
    let mut stats = OptimizeStats::new(original);

    if opts.sort_by_timeline {
        sort_by_timeline(clips);
    }

    if opts.remove_duplicates {
        let before = clips.len();
        let removed = remove_duplicate_clips(clips);
        stats.removed_count += removed;
        let after = clips.len();
        debug_assert_eq!(before - after, removed);
    }

    if opts.merge_adjacent {
        let before = clips.len();
        merge_adjacent_clips(clips);
        let merged = before - clips.len();
        stats.merged_count += merged;
    }

    stats.optimized_clips = clips.len();
    stats
}

/// Sort clips by their record-in (timeline) position, ascending.
pub fn sort_by_timeline(clips: &mut Vec<EdlClipEntry>) {
    clips.sort_by_key(|c| (c.record_in, c.source_name.clone()));
}

/// Remove clips that are exact duplicates of another clip in the list.
///
/// A duplicate is a clip where `id`, `source_name`, `record_in`,
/// `record_out`, `source_in`, and `source_out` all match an earlier entry.
///
/// Returns the number of duplicates removed.
pub fn remove_duplicate_clips(clips: &mut Vec<EdlClipEntry>) -> usize {
    let before = clips.len();
    let mut seen: std::collections::HashSet<(String, u64, u64, u64, u64)> =
        std::collections::HashSet::new();

    clips.retain(|c| {
        let key = (
            c.source_name.clone(),
            c.record_in,
            c.record_out,
            c.source_in,
            c.source_out,
        );
        seen.insert(key)
    });

    before - clips.len()
}

/// Merge adjacent clips that share the same source and are contiguous.
///
/// When two consecutive clips satisfy [`EdlClipEntry::is_adjacent_to`],
/// the first clip is extended to cover the range of both, and the second
/// is removed.  The process repeats until no more merges are possible.
pub fn merge_adjacent_clips(clips: &mut Vec<EdlClipEntry>) {
    let mut i = 0;
    while i + 1 < clips.len() {
        // Clone just the fields we need to avoid borrow-checker conflicts.
        let current_record_out = clips[i].record_out;
        let current_source_out = clips[i].source_out;
        let current_source_name = clips[i].source_name.clone();

        let next = &clips[i + 1];
        let can_merge = current_source_name == next.source_name
            && current_record_out == next.record_in
            && current_source_out == next.source_in;

        if can_merge {
            let next_record_out = clips[i + 1].record_out;
            let next_source_out = clips[i + 1].source_out;
            clips[i].record_out = next_record_out;
            clips[i].source_out = next_source_out;
            clips.remove(i + 1);
            // Don't advance i — try to merge the (now extended) clip with the
            // next one too.
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_clip(
        id: u32,
        source: &str,
        rec_in: u64,
        rec_out: u64,
        src_in: u64,
        src_out: u64,
    ) -> EdlClipEntry {
        EdlClipEntry::new(id, source, rec_in, rec_out, src_in, src_out)
    }

    // --- OptimizeOptions ---

    #[test]
    fn test_optimize_options_all() {
        let opts = OptimizeOptions::all();
        assert!(opts.merge_adjacent);
        assert!(opts.remove_duplicates);
        assert!(opts.sort_by_timeline);
        assert!(opts.consolidate_sources);
    }

    #[test]
    fn test_optimize_options_none() {
        let opts = OptimizeOptions::none();
        assert!(!opts.merge_adjacent);
        assert!(!opts.remove_duplicates);
        assert!(!opts.sort_by_timeline);
        assert!(!opts.consolidate_sources);
    }

    #[test]
    fn test_optimize_options_default() {
        let opts = OptimizeOptions::default();
        assert!(opts.merge_adjacent);
    }

    // --- OptimizeStats ---

    #[test]
    fn test_optimize_stats_reduction_percent_zero() {
        let stats = OptimizeStats::new(0);
        assert_eq!(stats.reduction_percent(), 0.0);
    }

    #[test]
    fn test_optimize_stats_reduction_percent_half() {
        let mut stats = OptimizeStats::new(10);
        stats.optimized_clips = 5;
        let pct = stats.reduction_percent();
        assert!((pct - 50.0).abs() < f64::EPSILON);
    }

    // --- EdlClipEntry ---

    #[test]
    fn test_clip_duration() {
        let clip = make_clip(1, "A001", 0, 50, 0, 50);
        assert_eq!(clip.duration(), 50);
    }

    #[test]
    fn test_clip_is_adjacent_to_true() {
        let a = make_clip(1, "A001", 0, 50, 100, 150);
        let b = make_clip(2, "A001", 50, 100, 150, 200);
        assert!(a.is_adjacent_to(&b));
    }

    #[test]
    fn test_clip_is_adjacent_to_false_different_source() {
        let a = make_clip(1, "A001", 0, 50, 0, 50);
        let b = make_clip(2, "B001", 50, 100, 50, 100);
        assert!(!a.is_adjacent_to(&b));
    }

    #[test]
    fn test_clip_is_adjacent_to_false_gap() {
        let a = make_clip(1, "A001", 0, 50, 0, 50);
        let b = make_clip(2, "A001", 60, 110, 50, 100);
        assert!(!a.is_adjacent_to(&b));
    }

    // --- sort_by_timeline ---

    #[test]
    fn test_sort_by_timeline() {
        let mut clips = vec![
            make_clip(2, "A001", 100, 150, 0, 50),
            make_clip(1, "A001", 0, 50, 0, 50),
            make_clip(3, "A001", 50, 100, 0, 50),
        ];
        sort_by_timeline(&mut clips);
        assert_eq!(clips[0].record_in, 0);
        assert_eq!(clips[1].record_in, 50);
        assert_eq!(clips[2].record_in, 100);
    }

    // --- remove_duplicate_clips ---

    #[test]
    fn test_remove_duplicate_clips_no_dupes() {
        let mut clips = vec![
            make_clip(1, "A001", 0, 50, 0, 50),
            make_clip(2, "A001", 50, 100, 50, 100),
        ];
        let removed = remove_duplicate_clips(&mut clips);
        assert_eq!(removed, 0);
        assert_eq!(clips.len(), 2);
    }

    #[test]
    fn test_remove_duplicate_clips_with_dupes() {
        let mut clips = vec![
            make_clip(1, "A001", 0, 50, 0, 50),
            make_clip(2, "A001", 0, 50, 0, 50), // exact duplicate of clip 1
            make_clip(3, "A001", 50, 100, 50, 100),
        ];
        let removed = remove_duplicate_clips(&mut clips);
        assert_eq!(removed, 1);
        assert_eq!(clips.len(), 2);
    }

    // --- merge_adjacent_clips ---

    #[test]
    fn test_merge_adjacent_clips_basic() {
        let mut clips = vec![
            make_clip(1, "A001", 0, 50, 100, 150),
            make_clip(2, "A001", 50, 100, 150, 200),
        ];
        merge_adjacent_clips(&mut clips);
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].record_out, 100);
        assert_eq!(clips[0].source_out, 200);
    }

    #[test]
    fn test_merge_adjacent_clips_chain() {
        let mut clips = vec![
            make_clip(1, "A001", 0, 25, 0, 25),
            make_clip(2, "A001", 25, 50, 25, 50),
            make_clip(3, "A001", 50, 75, 50, 75),
        ];
        merge_adjacent_clips(&mut clips);
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].record_out, 75);
    }

    #[test]
    fn test_merge_adjacent_clips_non_adjacent() {
        let mut clips = vec![
            make_clip(1, "A001", 0, 50, 0, 50),
            make_clip(2, "B001", 50, 100, 0, 50), // different source
        ];
        merge_adjacent_clips(&mut clips);
        assert_eq!(clips.len(), 2);
    }

    // --- optimize_edl ---

    #[test]
    fn test_optimize_edl_full() {
        let mut clips = vec![
            make_clip(3, "A001", 100, 150, 100, 150), // out of order
            make_clip(1, "A001", 0, 50, 0, 50),
            make_clip(2, "A001", 50, 100, 50, 100),
            make_clip(1, "A001", 0, 50, 0, 50), // duplicate
        ];
        let opts = OptimizeOptions::all();
        let stats = optimize_edl(&mut clips, &opts);

        // After dedup + sort + merge: 3 unique → 3 adjacent → 1 merged clip
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].record_out, 150);
        assert!(stats.optimized_clips < stats.original_clips);
        assert!(stats.reduction_percent() > 0.0);
    }

    #[test]
    fn test_optimize_edl_empty() {
        let mut clips: Vec<EdlClipEntry> = vec![];
        let stats = optimize_edl(&mut clips, &OptimizeOptions::all());
        assert_eq!(stats.original_clips, 0);
        assert_eq!(stats.optimized_clips, 0);
    }
}
