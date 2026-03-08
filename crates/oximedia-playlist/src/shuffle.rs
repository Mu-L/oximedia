//! Playlist shuffling: Fisher-Yates shuffle, weighted random selection,
//! and smart shuffle that avoids recently played repeats.

#![allow(dead_code)]

use std::collections::VecDeque;

// ── Fisher-Yates in-place shuffle ────────────────────────────────────────────

/// Shuffle a mutable slice in place using the Fisher-Yates algorithm.
/// Uses a deterministic seed for reproducibility in tests via a simple LCG.
pub fn fisher_yates<T>(items: &mut [T], seed: u64) {
    let n = items.len();
    if n < 2 {
        return;
    }
    let mut rng = LcgRng::new(seed);
    for i in (1..n).rev() {
        let j = rng.next_u64() as usize % (i + 1);
        items.swap(i, j);
    }
}

/// Minimal linear-congruential RNG for deterministic shuffling (no external deps).
#[derive(Debug, Clone)]
struct LcgRng {
    state: u64,
}

impl LcgRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(1),
        }
    }

    fn next_u64(&mut self) -> u64 {
        // Knuth multiplicative LCG.
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.state
    }
}

// ── WeightedShuffler ─────────────────────────────────────────────────────────

/// An item with an associated weight for weighted random selection.
#[derive(Debug, Clone)]
pub struct WeightedItem<T> {
    /// The item value.
    pub item: T,
    /// Relative weight (must be > 0.0).
    pub weight: f64,
}

impl<T> WeightedItem<T> {
    /// Create a new weighted item.
    #[must_use]
    pub fn new(item: T, weight: f64) -> Self {
        Self {
            item,
            weight: weight.max(f64::EPSILON),
        }
    }
}

/// Produce a weighted shuffle of items (higher weight → more likely to appear early).
/// Uses a reservoir-sampling-inspired technique: sort by `u^(1/w)` with a LCG key.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn weighted_shuffle<T: Clone>(items: &[WeightedItem<T>], seed: u64) -> Vec<T> {
    if items.is_empty() {
        return Vec::new();
    }
    let mut rng = LcgRng::new(seed);
    let mut keyed: Vec<(f64, usize)> = items
        .iter()
        .enumerate()
        .map(|(i, wi)| {
            // u in (0, 1).
            let u = (rng.next_u64() as f64 / u64::MAX as f64).clamp(1e-10, 1.0 - 1e-10);
            let key = u.ln() / wi.weight;
            (key, i)
        })
        .collect();
    // Sort descending by key (highest key = appears first).
    keyed.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    keyed
        .into_iter()
        .map(|(_, i)| items[i].item.clone())
        .collect()
}

// ── SmartShuffler ─────────────────────────────────────────────────────────────

/// A stateful shuffler that avoids replaying recently played tracks.
///
/// Internally maintains a queue of all track IDs in shuffled order, and a
/// "recent" window that prevents the same track from appearing until at least
/// `min_gap` other tracks have played.
#[derive(Debug)]
pub struct SmartShuffler {
    /// All available track IDs.
    track_ids: Vec<u64>,
    /// Current shuffle queue.
    queue: VecDeque<u64>,
    /// Recently played track IDs (oldest first).
    recent: VecDeque<u64>,
    /// Minimum number of distinct tracks between repeats.
    min_gap: usize,
    /// Seed for reproducibility.
    seed: u64,
    /// Counter incremented each re-shuffle to change the seed.
    generation: u64,
}

impl SmartShuffler {
    /// Create a new smart shuffler.
    ///
    /// * `track_ids` – all available tracks.
    /// * `min_gap`   – minimum distance between repeats (capped at track count − 1).
    /// * `seed`      – initial RNG seed.
    #[must_use]
    pub fn new(track_ids: Vec<u64>, min_gap: usize, seed: u64) -> Self {
        let cap = if track_ids.len() > 1 {
            track_ids.len() - 1
        } else {
            0
        };
        let min_gap = min_gap.min(cap);
        let mut s = Self {
            track_ids,
            queue: VecDeque::new(),
            recent: VecDeque::new(),
            min_gap,
            seed,
            generation: 0,
        };
        s.refill();
        s
    }

    /// Returns the next track ID, re-shuffling if the queue is exhausted.
    #[must_use]
    pub fn next(&mut self) -> Option<u64> {
        if self.track_ids.is_empty() {
            return None;
        }
        if self.queue.is_empty() {
            self.refill();
        }
        // Skip tracks in the recent window.
        loop {
            let candidate = *self.queue.front()?;
            if !self.recent.contains(&candidate) || self.recent.len() < self.min_gap {
                self.queue.pop_front();
                self.record_played(candidate);
                return Some(candidate);
            }
            // Move the blocked candidate to the back.
            self.queue.pop_front();
            self.queue.push_back(candidate);
        }
    }

    /// Record a track as played (updates the recent window).
    fn record_played(&mut self, id: u64) {
        self.recent.push_back(id);
        if self.recent.len() > self.min_gap {
            self.recent.pop_front();
        }
    }

    /// Refill the queue with a fresh shuffle of all tracks.
    fn refill(&mut self) {
        let mut ids = self.track_ids.clone();
        fisher_yates(&mut ids, self.seed.wrapping_add(self.generation));
        self.generation += 1;
        self.queue = ids.into();
    }

    /// Number of tracks remaining in the current shuffle pass.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.queue.len()
    }

    /// Total number of available tracks.
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.track_ids.len()
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fisher_yates_same_elements() {
        let mut items = vec![1, 2, 3, 4, 5];
        let original = items.clone();
        fisher_yates(&mut items, 42);
        let mut sorted = items.clone();
        sorted.sort();
        assert_eq!(sorted, original);
    }

    #[test]
    fn test_fisher_yates_deterministic() {
        let mut a = vec![1, 2, 3, 4, 5];
        let mut b = a.clone();
        fisher_yates(&mut a, 99);
        fisher_yates(&mut b, 99);
        assert_eq!(a, b);
    }

    #[test]
    fn test_fisher_yates_different_seeds() {
        let mut a = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let mut b = a.clone();
        fisher_yates(&mut a, 1);
        fisher_yates(&mut b, 2);
        // Different seeds should (almost always) produce different orders.
        // With 8 elements the probability of collision is 1/8! ≈ 0.002%.
        assert_ne!(a, b);
    }

    #[test]
    fn test_fisher_yates_empty() {
        let mut items: Vec<i32> = Vec::new();
        fisher_yates(&mut items, 1);
        assert!(items.is_empty());
    }

    #[test]
    fn test_fisher_yates_single() {
        let mut items = vec![42];
        fisher_yates(&mut items, 1);
        assert_eq!(items, vec![42]);
    }

    #[test]
    fn test_weighted_shuffle_preserves_elements() {
        let items = vec![
            WeightedItem::new(1u32, 1.0),
            WeightedItem::new(2u32, 2.0),
            WeightedItem::new(3u32, 0.5),
        ];
        let mut result = weighted_shuffle(&items, 7);
        result.sort();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_weighted_shuffle_empty() {
        let items: Vec<WeightedItem<u32>> = Vec::new();
        assert!(weighted_shuffle(&items, 1).is_empty());
    }

    #[test]
    fn test_weighted_shuffle_deterministic() {
        let items = vec![
            WeightedItem::new("a", 1.0),
            WeightedItem::new("b", 10.0),
            WeightedItem::new("c", 0.1),
        ];
        let r1 = weighted_shuffle(&items, 42);
        let r2 = weighted_shuffle(&items, 42);
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_weighted_shuffle_single() {
        let items = vec![WeightedItem::new(99u32, 5.0)];
        let result = weighted_shuffle(&items, 1);
        assert_eq!(result, vec![99]);
    }

    #[test]
    fn test_smart_shuffler_basic() {
        let ids: Vec<u64> = (1..=5).collect();
        let mut shuffler = SmartShuffler::new(ids.clone(), 2, 42);
        let mut seen = Vec::new();
        for _ in 0..5 {
            seen.push(shuffler.next().expect("should succeed in test"));
        }
        let mut sorted = seen.clone();
        sorted.sort();
        assert_eq!(sorted, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_smart_shuffler_empty() {
        let mut shuffler = SmartShuffler::new(vec![], 1, 1);
        assert!(shuffler.next().is_none());
    }

    #[test]
    fn test_smart_shuffler_single_track() {
        let mut shuffler = SmartShuffler::new(vec![42u64], 0, 1);
        assert_eq!(shuffler.next(), Some(42));
        assert_eq!(shuffler.next(), Some(42));
    }

    #[test]
    fn test_smart_shuffler_track_count() {
        let ids: Vec<u64> = (0..10).collect();
        let shuffler = SmartShuffler::new(ids, 3, 7);
        assert_eq!(shuffler.track_count(), 10);
    }

    #[test]
    fn test_smart_shuffler_continues_past_first_pass() {
        let ids: Vec<u64> = (1..=4).collect();
        let mut shuffler = SmartShuffler::new(ids, 1, 13);
        for _ in 0..8 {
            assert!(shuffler.next().is_some());
        }
    }

    #[test]
    fn test_lcg_rng_produces_different_values() {
        let mut rng = LcgRng::new(1);
        let a = rng.next_u64();
        let b = rng.next_u64();
        assert_ne!(a, b);
    }
}
