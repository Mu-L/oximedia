#![allow(dead_code)]
//! Automatic gap detection and filler content insertion for broadcast playlists.
//!
//! When a broadcast schedule has gaps between programs, this module identifies
//! them and selects appropriate filler content (promos, bumpers, interstitials)
//! to maintain continuous playout.

use std::collections::HashMap;
use std::time::Duration;

/// Represents a time slot in the playlist timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeSlot {
    /// Start offset from playlist origin in milliseconds.
    pub start_ms: u64,
    /// End offset from playlist origin in milliseconds.
    pub end_ms: u64,
    /// Label for this time slot.
    pub label: String,
}

impl TimeSlot {
    /// Create a new time slot.
    pub fn new(start_ms: u64, end_ms: u64, label: &str) -> Self {
        Self {
            start_ms,
            end_ms,
            label: label.to_string(),
        }
    }

    /// Duration of this time slot.
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }
}

/// Category of filler content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FillerCategory {
    /// Station promotional content.
    Promo,
    /// Short bumper between segments.
    Bumper,
    /// Public service announcement.
    Psa,
    /// Music video or performance clip.
    MusicClip,
    /// Animated station ident.
    StationIdent,
    /// Generic loop content.
    Loop,
}

impl FillerCategory {
    /// Human-readable name for the category.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Promo => "Promo",
            Self::Bumper => "Bumper",
            Self::Psa => "PSA",
            Self::MusicClip => "Music Clip",
            Self::StationIdent => "Station Ident",
            Self::Loop => "Loop",
        }
    }
}

/// A piece of filler content that can be inserted into gaps.
#[derive(Debug, Clone)]
pub struct FillerItem {
    /// Unique identifier for this filler.
    pub id: String,
    /// Duration of the filler in milliseconds.
    pub duration_ms: u64,
    /// Category of filler.
    pub category: FillerCategory,
    /// Priority (higher = preferred).
    pub priority: u32,
    /// Number of times this filler has been used recently.
    pub recent_play_count: u32,
    /// Whether this filler is currently enabled.
    pub enabled: bool,
}

impl FillerItem {
    /// Create a new filler item.
    pub fn new(id: &str, duration_ms: u64, category: FillerCategory) -> Self {
        Self {
            id: id.to_string(),
            duration_ms,
            category,
            priority: 1,
            recent_play_count: 0,
            enabled: true,
        }
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }
}

/// A detected gap in the playlist timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    /// Start of the gap in milliseconds from playlist origin.
    pub start_ms: u64,
    /// End of the gap in milliseconds from playlist origin.
    pub end_ms: u64,
}

impl Gap {
    /// Create a new gap.
    pub fn new(start_ms: u64, end_ms: u64) -> Self {
        Self { start_ms, end_ms }
    }

    /// Duration of the gap in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }

    /// Convert the gap duration to a `std::time::Duration`.
    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.duration_ms())
    }
}

/// Strategy for selecting filler content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillStrategy {
    /// Fill with the single best-fit item (closest to gap duration without exceeding).
    BestFit,
    /// Fill with multiple items to cover as much of the gap as possible.
    PackFull,
    /// Prefer variety by avoiding recently played fillers.
    RotateVariety,
    /// Always use a looping filler (trimmed to fit).
    LoopFill,
}

/// Result of a fill operation.
#[derive(Debug, Clone)]
pub struct FillResult {
    /// Items selected to fill the gap.
    pub items: Vec<FillerItem>,
    /// Total duration covered in milliseconds.
    pub covered_ms: u64,
    /// Remaining unfilled duration in milliseconds.
    pub remaining_ms: u64,
}

impl FillResult {
    /// Whether the gap was completely filled.
    pub fn is_fully_covered(&self) -> bool {
        self.remaining_ms == 0
    }

    /// Coverage ratio from 0.0 to 1.0.
    #[allow(clippy::cast_precision_loss)]
    pub fn coverage_ratio(&self, gap_ms: u64) -> f64 {
        if gap_ms == 0 {
            return 1.0;
        }
        self.covered_ms as f64 / gap_ms as f64
    }
}

/// Engine for detecting gaps and filling them with appropriate content.
pub struct GapFiller {
    /// Available filler content keyed by category.
    fillers: HashMap<FillerCategory, Vec<FillerItem>>,
    /// Fill strategy.
    strategy: FillStrategy,
    /// Minimum gap duration (ms) worth filling.
    min_gap_ms: u64,
}

impl GapFiller {
    /// Create a new gap filler with the given strategy.
    pub fn new(strategy: FillStrategy) -> Self {
        Self {
            fillers: HashMap::new(),
            strategy,
            min_gap_ms: 1000,
        }
    }

    /// Set the minimum gap threshold in milliseconds.
    pub fn set_min_gap_ms(&mut self, min_ms: u64) {
        self.min_gap_ms = min_ms;
    }

    /// Add a filler item to the pool.
    pub fn add_filler(&mut self, item: FillerItem) {
        self.fillers.entry(item.category).or_default().push(item);
    }

    /// Total number of filler items across all categories.
    pub fn filler_count(&self) -> usize {
        self.fillers.values().map(std::vec::Vec::len).sum()
    }

    /// Detect gaps between sorted time slots.
    pub fn detect_gaps(&self, slots: &[TimeSlot]) -> Vec<Gap> {
        if slots.is_empty() {
            return Vec::new();
        }
        let mut gaps = Vec::new();
        for i in 1..slots.len() {
            let prev_end = slots[i - 1].end_ms;
            let cur_start = slots[i].start_ms;
            if cur_start > prev_end {
                let gap_dur = cur_start - prev_end;
                if gap_dur >= self.min_gap_ms {
                    gaps.push(Gap::new(prev_end, cur_start));
                }
            }
        }
        gaps
    }

    /// Fill a single gap with filler content.
    pub fn fill_gap(&self, gap: &Gap) -> FillResult {
        let gap_ms = gap.duration_ms();
        let all_fillers = self.all_enabled_fillers_sorted();

        match self.strategy {
            FillStrategy::BestFit => self.best_fit(&all_fillers, gap_ms),
            FillStrategy::PackFull => self.pack_full(&all_fillers, gap_ms),
            FillStrategy::RotateVariety => self.rotate_variety(&all_fillers, gap_ms),
            FillStrategy::LoopFill => self.loop_fill(&all_fillers, gap_ms),
        }
    }

    /// Get all enabled fillers sorted by priority descending, then duration descending.
    fn all_enabled_fillers_sorted(&self) -> Vec<&FillerItem> {
        let mut all: Vec<&FillerItem> = self
            .fillers
            .values()
            .flat_map(|v| v.iter())
            .filter(|f| f.enabled)
            .collect();
        all.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| b.duration_ms.cmp(&a.duration_ms))
        });
        all
    }

    /// Best-fit: pick the single largest item that fits.
    fn best_fit(&self, fillers: &[&FillerItem], gap_ms: u64) -> FillResult {
        let mut best: Option<&FillerItem> = None;
        for f in fillers {
            if f.duration_ms <= gap_ms {
                match best {
                    Some(b) if f.duration_ms > b.duration_ms => best = Some(f),
                    None => best = Some(f),
                    _ => {}
                }
            }
        }
        match best {
            Some(item) => FillResult {
                covered_ms: item.duration_ms,
                remaining_ms: gap_ms - item.duration_ms,
                items: vec![item.clone()],
            },
            None => FillResult {
                covered_ms: 0,
                remaining_ms: gap_ms,
                items: Vec::new(),
            },
        }
    }

    /// Pack-full: greedily pack as many items as possible.
    fn pack_full(&self, fillers: &[&FillerItem], gap_ms: u64) -> FillResult {
        let mut remaining = gap_ms;
        let mut selected = Vec::new();
        for f in fillers {
            if f.duration_ms <= remaining {
                remaining -= f.duration_ms;
                selected.push((*f).clone());
                if remaining == 0 {
                    break;
                }
            }
        }
        FillResult {
            covered_ms: gap_ms - remaining,
            remaining_ms: remaining,
            items: selected,
        }
    }

    /// Rotate-variety: prefer items with lowest recent play count.
    fn rotate_variety(&self, fillers: &[&FillerItem], gap_ms: u64) -> FillResult {
        let mut sorted: Vec<&FillerItem> = fillers.to_vec();
        sorted.sort_by(|a, b| a.recent_play_count.cmp(&b.recent_play_count));
        self.pack_full(&sorted, gap_ms)
    }

    /// Loop-fill: pick the best single item and conceptually loop it.
    fn loop_fill(&self, fillers: &[&FillerItem], gap_ms: u64) -> FillResult {
        // Pick the highest-priority loop-category filler (or any if none is loop).
        let loop_item = fillers
            .iter()
            .find(|f| f.category == FillerCategory::Loop)
            .or_else(|| fillers.first());
        match loop_item {
            Some(item) => {
                let repeats = if item.duration_ms > 0 {
                    gap_ms.div_ceil(item.duration_ms)
                } else {
                    0
                };
                let covered = gap_ms; // looped content covers the whole gap
                let mut items = Vec::new();
                for _ in 0..repeats {
                    items.push((*item).clone());
                }
                FillResult {
                    covered_ms: covered,
                    remaining_ms: 0,
                    items,
                }
            }
            None => FillResult {
                covered_ms: 0,
                remaining_ms: gap_ms,
                items: Vec::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fillers() -> Vec<FillerItem> {
        vec![
            FillerItem::new("promo_01", 15_000, FillerCategory::Promo).with_priority(3),
            FillerItem::new("bumper_01", 5_000, FillerCategory::Bumper).with_priority(2),
            FillerItem::new("psa_01", 30_000, FillerCategory::Psa).with_priority(1),
            FillerItem::new("loop_01", 10_000, FillerCategory::Loop).with_priority(1),
            FillerItem::new("ident_01", 3_000, FillerCategory::StationIdent).with_priority(4),
        ]
    }

    fn make_filler(strategy: FillStrategy) -> GapFiller {
        let mut gf = GapFiller::new(strategy);
        for f in sample_fillers() {
            gf.add_filler(f);
        }
        gf
    }

    #[test]
    fn test_time_slot_duration() {
        let slot = TimeSlot::new(1000, 5000, "seg1");
        assert_eq!(slot.duration_ms(), 4000);
    }

    #[test]
    fn test_time_slot_zero_duration() {
        let slot = TimeSlot::new(3000, 3000, "empty");
        assert_eq!(slot.duration_ms(), 0);
    }

    #[test]
    fn test_gap_duration() {
        let gap = Gap::new(10_000, 25_000);
        assert_eq!(gap.duration_ms(), 15_000);
        assert_eq!(gap.duration(), Duration::from_millis(15_000));
    }

    #[test]
    fn test_detect_gaps_no_slots() {
        let gf = GapFiller::new(FillStrategy::BestFit);
        let gaps = gf.detect_gaps(&[]);
        assert!(gaps.is_empty());
    }

    #[test]
    fn test_detect_gaps_contiguous() {
        let gf = GapFiller::new(FillStrategy::BestFit);
        let slots = vec![
            TimeSlot::new(0, 5000, "a"),
            TimeSlot::new(5000, 10_000, "b"),
        ];
        let gaps = gf.detect_gaps(&slots);
        assert!(gaps.is_empty());
    }

    #[test]
    fn test_detect_gaps_with_gap() {
        let mut gf = GapFiller::new(FillStrategy::BestFit);
        gf.set_min_gap_ms(500);
        let slots = vec![
            TimeSlot::new(0, 5000, "a"),
            TimeSlot::new(8000, 12_000, "b"),
        ];
        let gaps = gf.detect_gaps(&slots);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].start_ms, 5000);
        assert_eq!(gaps[0].end_ms, 8000);
    }

    #[test]
    fn test_detect_gaps_below_threshold() {
        let mut gf = GapFiller::new(FillStrategy::BestFit);
        gf.set_min_gap_ms(5000);
        let slots = vec![
            TimeSlot::new(0, 5000, "a"),
            TimeSlot::new(7000, 12_000, "b"),
        ];
        // Gap is 2000ms which is below 5000ms threshold
        let gaps = gf.detect_gaps(&slots);
        assert!(gaps.is_empty());
    }

    #[test]
    fn test_best_fit_exact() {
        let gf = make_filler(FillStrategy::BestFit);
        let gap = Gap::new(0, 15_000);
        let result = gf.fill_gap(&gap);
        assert_eq!(result.covered_ms, 15_000);
        assert_eq!(result.remaining_ms, 0);
        assert!(result.is_fully_covered());
    }

    #[test]
    fn test_best_fit_no_fit() {
        let mut gf = GapFiller::new(FillStrategy::BestFit);
        gf.add_filler(FillerItem::new("big", 60_000, FillerCategory::Promo));
        let gap = Gap::new(0, 5_000);
        let result = gf.fill_gap(&gap);
        assert_eq!(result.covered_ms, 0);
        assert!(!result.is_fully_covered());
    }

    #[test]
    fn test_pack_full() {
        let gf = make_filler(FillStrategy::PackFull);
        let gap = Gap::new(0, 20_000);
        let result = gf.fill_gap(&gap);
        assert!(result.covered_ms > 0);
        assert!(result.items.len() >= 1);
    }

    #[test]
    fn test_loop_fill_covers_fully() {
        let gf = make_filler(FillStrategy::LoopFill);
        let gap = Gap::new(0, 50_000);
        let result = gf.fill_gap(&gap);
        assert!(result.is_fully_covered());
        assert_eq!(result.remaining_ms, 0);
    }

    #[test]
    fn test_filler_count() {
        let gf = make_filler(FillStrategy::BestFit);
        assert_eq!(gf.filler_count(), 5);
    }

    #[test]
    fn test_fill_result_coverage_ratio() {
        let result = FillResult {
            items: Vec::new(),
            covered_ms: 7500,
            remaining_ms: 2500,
        };
        let ratio = result.coverage_ratio(10_000);
        assert!((ratio - 0.75).abs() < 1e-9);
    }

    #[test]
    fn test_fill_result_coverage_ratio_zero_gap() {
        let result = FillResult {
            items: Vec::new(),
            covered_ms: 0,
            remaining_ms: 0,
        };
        let ratio = result.coverage_ratio(0);
        assert!((ratio - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_filler_category_display() {
        assert_eq!(FillerCategory::Promo.display_name(), "Promo");
        assert_eq!(FillerCategory::Psa.display_name(), "PSA");
        assert_eq!(FillerCategory::StationIdent.display_name(), "Station Ident");
    }

    #[test]
    fn test_rotate_variety_prefers_least_played() {
        let mut gf = GapFiller::new(FillStrategy::RotateVariety);
        let mut item_a = FillerItem::new("a", 5_000, FillerCategory::Promo);
        item_a.recent_play_count = 10;
        let mut item_b = FillerItem::new("b", 5_000, FillerCategory::Bumper);
        item_b.recent_play_count = 0;
        gf.add_filler(item_a);
        gf.add_filler(item_b);
        let gap = Gap::new(0, 5_000);
        let result = gf.fill_gap(&gap);
        // Should pick item_b first (lower play count)
        assert_eq!(result.items[0].id, "b");
    }
}
