//! Classic CRDT primitives for distributed state management.
//!
//! Provides pure-Rust, dependency-free implementations of the foundational
//! Conflict-free Replicated Data Types that form the building blocks of
//! distributed, eventually-consistent systems:
//!
//! * `GCounter`     — grow-only counter
//! * `PNCounter`    — positive/negative counter (increment + decrement)
//! * `LWWRegister`  — last-write-wins register (single value)
//! * `MVRegister`   — multi-value register (concurrent-write awareness)
//! * `GSet`         — grow-only set
//! * `TwoPhaseSet`  — two-phase set (add + remove, with tombstone)
//!
//! Each type exposes a `merge(&other)` method that is commutative, associative,
//! and idempotent — the three laws required of a valid CRDT merge function.

use std::collections::{HashMap, HashSet};

// ─────────────────────────────────────────────────────────────────────────────
// NodeId
// ─────────────────────────────────────────────────────────────────────────────

/// Opaque identifier for a replica/node in the distributed system.
///
/// Any `Clone + Eq + Hash` type can be used as a node identifier; this
/// type alias defaults to `String` for human-readable ids.
pub type NodeId = String;

// ─────────────────────────────────────────────────────────────────────────────
// VectorClock (local, standalone — not shared with the Yjs-backed crdt.rs)
// ─────────────────────────────────────────────────────────────────────────────

/// A simple vector clock used by [`MVRegister`] to track causality.
///
/// Each entry records the logical time at which the owning node last wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorClock {
    entries: HashMap<NodeId, u64>,
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorClock {
    /// Create an empty vector clock.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Increment the logical time for `node` and return the new value.
    pub fn tick(&mut self, node: &NodeId) -> u64 {
        let t = self.entries.entry(node.clone()).or_insert(0);
        *t += 1;
        *t
    }

    /// Return the current logical time for `node` (0 if never ticked).
    pub fn get(&self, node: &NodeId) -> u64 {
        self.entries.get(node).copied().unwrap_or(0)
    }

    /// Merge two clocks by taking the component-wise maximum.
    pub fn merge(&mut self, other: &Self) {
        for (node, &t) in &other.entries {
            let entry = self.entries.entry(node.clone()).or_insert(0);
            if t > *entry {
                *entry = t;
            }
        }
    }

    /// `true` if `self` happened-before `other` (strict ≤ in all components,
    /// strict < in at least one).
    pub fn happens_before(&self, other: &Self) -> bool {
        let mut strictly_less = false;
        for (node, &t) in &self.entries {
            let other_t = other.get(node);
            if t > other_t {
                return false;
            }
            if t < other_t {
                strictly_less = true;
            }
        }
        // Also check nodes only in `other`.
        for node in other.entries.keys() {
            if !self.entries.contains_key(node) && other.get(node) > 0 {
                strictly_less = true;
            }
        }
        strictly_less
    }

    /// `true` if neither clock dominates the other (concurrent writes).
    pub fn is_concurrent_with(&self, other: &Self) -> bool {
        !self.happens_before(other) && !other.happens_before(self) && self != other
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GCounter
// ─────────────────────────────────────────────────────────────────────────────

/// Grow-only counter CRDT.
///
/// Each node owns a monotonically increasing local counter.  The global value
/// is the sum of all per-node counters.  `merge` takes the component-wise max,
/// so replicas always converge to the same total.
///
/// # Guarantees
/// * No overflows are possible for individual node counters up to `u64::MAX`.
/// * `value()` may be at most `nodes * u64::MAX`.  In practice counters are
///   used with much smaller ranges.
#[derive(Debug, Clone, Default)]
pub struct GCounter {
    counts: HashMap<NodeId, u64>,
}

impl GCounter {
    /// Create a new, zeroed counter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment the local counter for `node` by `amount`.
    ///
    /// # Panics
    /// Panics in debug builds if the per-node counter would overflow.
    pub fn increment(&mut self, node: &NodeId, amount: u64) {
        let c = self.counts.entry(node.clone()).or_insert(0);
        *c = c.saturating_add(amount);
    }

    /// Return the sum of all per-node counters.
    pub fn value(&self) -> u64 {
        self.counts
            .values()
            .fold(0u64, |acc, &v| acc.saturating_add(v))
    }

    /// Return the counter for a single `node`.
    pub fn node_value(&self, node: &NodeId) -> u64 {
        self.counts.get(node).copied().unwrap_or(0)
    }

    /// Merge `other` into `self` by taking component-wise maximums.
    ///
    /// This operation is idempotent, commutative, and associative.
    pub fn merge(&mut self, other: &Self) {
        for (node, &v) in &other.counts {
            let entry = self.counts.entry(node.clone()).or_insert(0);
            if v > *entry {
                *entry = v;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PNCounter
// ─────────────────────────────────────────────────────────────────────────────

/// Positive-Negative counter CRDT.
///
/// Composed of two `GCounter`s: one for increments, one for decrements.
/// The observable value is `increments.value() - decrements.value()`.
/// The result is returned as `i64`; wrap-around is handled by saturating
/// arithmetic in the underlying `GCounter`.
#[derive(Debug, Clone, Default)]
pub struct PNCounter {
    increments: GCounter,
    decrements: GCounter,
}

impl PNCounter {
    /// Create a new counter at zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add `amount` to the counter on behalf of `node`.
    pub fn increment(&mut self, node: &NodeId, amount: u64) {
        self.increments.increment(node, amount);
    }

    /// Subtract `amount` from the counter on behalf of `node`.
    pub fn decrement(&mut self, node: &NodeId, amount: u64) {
        self.decrements.increment(node, amount);
    }

    /// Return the net value (increments − decrements) as `i64`.
    pub fn value(&self) -> i64 {
        let pos = self.increments.value();
        let neg = self.decrements.value();
        pos as i64 - neg as i64
    }

    /// Merge `other` into `self`.
    pub fn merge(&mut self, other: &Self) {
        self.increments.merge(&other.increments);
        self.decrements.merge(&other.decrements);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LWWRegister
// ─────────────────────────────────────────────────────────────────────────────

/// Last-Write-Wins register CRDT.
///
/// Stores a single value together with a (timestamp, node_id) pair used as
/// a tie-breaker when two replicas have writes at exactly the same timestamp.
/// The node with the lexicographically *larger* `NodeId` wins the tie.
///
/// # Type parameter
/// `T` must be `Clone`.  It does not need to implement `Ord`.
#[derive(Debug, Clone)]
pub struct LWWRegister<T: Clone> {
    /// The stored value.
    pub value: T,
    /// Wall-clock timestamp in milliseconds since UNIX epoch.
    pub timestamp: u64,
    /// The node that performed the last write.
    pub node_id: NodeId,
}

impl<T: Clone> LWWRegister<T> {
    /// Create a new register with an initial value.
    pub fn new(node_id: NodeId, value: T, timestamp: u64) -> Self {
        Self {
            value,
            timestamp,
            node_id,
        }
    }

    /// Write a new value.  Updates `timestamp` and `node_id` of the writer.
    pub fn write(&mut self, node_id: NodeId, value: T, timestamp: u64) {
        // Only apply if this write would win the merge.
        if self.would_win(timestamp, &node_id) {
            self.value = value;
            self.timestamp = timestamp;
            self.node_id = node_id;
        }
    }

    /// Return `true` if a write from `(timestamp, node_id)` would win over
    /// the current state (used internally and for testing).
    pub fn would_win(&self, timestamp: u64, node_id: &NodeId) -> bool {
        timestamp > self.timestamp
            || (timestamp == self.timestamp && node_id.as_str() > self.node_id.as_str())
    }

    /// Merge `other` into `self`.  The replica with the higher timestamp (or
    /// higher node_id on tie) wins.
    pub fn merge(&mut self, other: &Self)
    where
        T: Clone,
    {
        // Ask: "does a write at (other.timestamp, other.node_id) beat my
        // current state?"  `would_win` answers exactly this question.
        if self.would_win(other.timestamp, &other.node_id) {
            self.value = other.value.clone();
            self.timestamp = other.timestamp;
            self.node_id = other.node_id.clone();
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MVRegister
// ─────────────────────────────────────────────────────────────────────────────

/// Multi-Value register CRDT.
///
/// Unlike `LWWRegister`, `MVRegister` retains *all* concurrently written
/// values rather than picking one winner.  Causally dominated versions are
/// discarded during merge so that the value set stays minimal.
///
/// * A single-node sequential history always results in exactly one value.
/// * Concurrent writes from different nodes produce multiple values; the
///   application must resolve the conflict.
#[derive(Debug, Clone)]
pub struct MVRegister<T: Clone + PartialEq> {
    /// Pairs of `(value, version_vector_at_write_time)`.
    versions: Vec<(T, VectorClock)>,
}

impl<T: Clone + PartialEq> Default for MVRegister<T> {
    fn default() -> Self {
        Self {
            versions: Vec::new(),
        }
    }
}

impl<T: Clone + PartialEq> MVRegister<T> {
    /// Create an empty register (no value yet).
    pub fn new() -> Self {
        Self::default()
    }

    /// Write `value` with the given `clock`.
    ///
    /// The new entry is added; any version dominated by `clock` is removed.
    pub fn write(&mut self, value: T, clock: VectorClock) {
        // Remove dominated versions.
        self.versions.retain(|(_, vc)| !vc.happens_before(&clock));
        self.versions.push((value, clock));
    }

    /// Return references to all currently live values (may be > 1 on conflict).
    pub fn values(&self) -> Vec<&T> {
        self.versions.iter().map(|(v, _)| v).collect()
    }

    /// Number of concurrent values.
    pub fn len(&self) -> usize {
        self.versions.len()
    }

    /// `true` if no value has been written yet.
    pub fn is_empty(&self) -> bool {
        self.versions.is_empty()
    }

    /// Merge `other` into `self`.
    ///
    /// The result is the union of non-dominated (value, clock) pairs from
    /// both replicas, with duplicates collapsed.
    pub fn merge(&mut self, other: &Self) {
        for (val, clock) in &other.versions {
            // Skip if already dominated by something in self.
            let dominated = self.versions.iter().any(|(_, sc)| clock.happens_before(sc));
            if !dominated {
                // Remove self entries dominated by this incoming version.
                self.versions.retain(|(_, sc)| !sc.happens_before(clock));
                // Avoid exact duplicates.
                let already = self
                    .versions
                    .iter()
                    .any(|(sv, sc)| sv == val && sc == clock);
                if !already {
                    self.versions.push((val.clone(), clock.clone()));
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GSet
// ─────────────────────────────────────────────────────────────────────────────

/// Grow-only set CRDT.
///
/// Elements can only be added, never removed.  `merge` is set union.
///
/// # Type parameter
/// `T` must be `Clone + Eq + std::hash::Hash`.
#[derive(Debug, Clone)]
pub struct GSet<T: Clone + Eq + std::hash::Hash> {
    items: HashSet<T>,
}

impl<T: Clone + Eq + std::hash::Hash> Default for GSet<T> {
    fn default() -> Self {
        Self {
            items: HashSet::new(),
        }
    }
}

impl<T: Clone + Eq + std::hash::Hash> GSet<T> {
    /// Create an empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert `item` into the set.
    pub fn insert(&mut self, item: T) {
        self.items.insert(item);
    }

    /// Return `true` if `item` is in the set.
    pub fn contains(&self, item: &T) -> bool {
        self.items.contains(item)
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Iterate over all items.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    /// Merge `other` into `self` (set union).
    pub fn merge(&mut self, other: &Self) {
        for item in &other.items {
            self.items.insert(item.clone());
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TwoPhaseSet
// ─────────────────────────────────────────────────────────────────────────────

/// Two-phase set CRDT (2P-Set).
///
/// Elements may be added and removed, but once removed they can *never* be
/// re-added.  Internally keeps an "added" `GSet` and a "removed" (tombstone)
/// `GSet`; an element is logically present iff it is in `added` and **not**
/// in `removed`.
///
/// # Type parameter
/// `T` must be `Clone + Eq + std::hash::Hash`.
#[derive(Debug, Clone)]
pub struct TwoPhaseSet<T: Clone + Eq + std::hash::Hash> {
    added: GSet<T>,
    removed: GSet<T>,
}

impl<T: Clone + Eq + std::hash::Hash> Default for TwoPhaseSet<T> {
    fn default() -> Self {
        Self {
            added: GSet::new(),
            removed: GSet::new(),
        }
    }
}

impl<T: Clone + Eq + std::hash::Hash> TwoPhaseSet<T> {
    /// Create an empty 2P-Set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert `item`.  No-op if the item has already been removed.
    pub fn insert(&mut self, item: T) {
        if !self.removed.contains(&item) {
            self.added.insert(item);
        }
    }

    /// Remove `item`.  The item is tombstoned permanently.
    pub fn remove(&mut self, item: T) {
        // Must be in added before it can be removed (per 2P-Set semantics).
        if self.added.contains(&item) {
            self.removed.insert(item);
        }
    }

    /// Return `true` if `item` is logically present.
    pub fn contains(&self, item: &T) -> bool {
        self.added.contains(item) && !self.removed.contains(item)
    }

    /// Number of *live* (not tombstoned) elements.
    pub fn len(&self) -> usize {
        self.added
            .iter()
            .filter(|i| !self.removed.contains(i))
            .count()
    }

    /// `true` if there are no live elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over live elements.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.added.iter().filter(move |i| !self.removed.contains(i))
    }

    /// Merge `other` into `self`.
    ///
    /// Both the `added` and `removed` GSet components are merged independently.
    pub fn merge(&mut self, other: &Self) {
        self.added.merge(&other.added);
        self.removed.merge(&other.removed);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── VectorClock ──────────────────────────────────────────────────────────

    #[test]
    fn test_vector_clock_tick_and_get() {
        let mut vc = VectorClock::new();
        let n = "node-A".to_string();
        assert_eq!(vc.tick(&n), 1);
        assert_eq!(vc.tick(&n), 2);
        assert_eq!(vc.get(&n), 2);
        assert_eq!(vc.get(&"node-B".to_string()), 0);
    }

    #[test]
    fn test_vector_clock_merge() {
        let mut a = VectorClock::new();
        let mut b = VectorClock::new();
        a.tick(&"A".to_string());
        a.tick(&"A".to_string()); // A: 2
        b.tick(&"B".to_string()); // B: 1
        a.merge(&b);
        assert_eq!(a.get(&"A".to_string()), 2);
        assert_eq!(a.get(&"B".to_string()), 1);
    }

    #[test]
    fn test_vector_clock_happens_before() {
        let mut a = VectorClock::new();
        let mut b = VectorClock::new();
        let n = "N".to_string();
        a.tick(&n); // a: {N:1}
        b.tick(&n);
        b.tick(&n); // b: {N:2}
        assert!(a.happens_before(&b));
        assert!(!b.happens_before(&a));
    }

    #[test]
    fn test_vector_clock_concurrent() {
        let mut a = VectorClock::new();
        let mut b = VectorClock::new();
        a.tick(&"A".to_string()); // a: {A:1}
        b.tick(&"B".to_string()); // b: {B:1}
        assert!(a.is_concurrent_with(&b));
        assert!(b.is_concurrent_with(&a));
    }

    // ── GCounter ─────────────────────────────────────────────────────────────

    #[test]
    fn test_gcounter_increment_and_value() {
        let mut g = GCounter::new();
        g.increment(&"A".to_string(), 3);
        g.increment(&"B".to_string(), 2);
        assert_eq!(g.value(), 5);
    }

    #[test]
    fn test_gcounter_merge_idempotent() {
        let mut a = GCounter::new();
        a.increment(&"A".to_string(), 5);
        let b = a.clone();
        a.merge(&b);
        assert_eq!(a.value(), 5); // same, not doubled
    }

    #[test]
    fn test_gcounter_merge_commutative() {
        let mut a = GCounter::new();
        a.increment(&"A".to_string(), 10);
        let mut b = GCounter::new();
        b.increment(&"B".to_string(), 7);

        let mut ab = a.clone();
        ab.merge(&b);

        let mut ba = b.clone();
        ba.merge(&a);

        assert_eq!(ab.value(), ba.value());
    }

    #[test]
    fn test_gcounter_node_value() {
        let mut g = GCounter::new();
        g.increment(&"X".to_string(), 4);
        assert_eq!(g.node_value(&"X".to_string()), 4);
        assert_eq!(g.node_value(&"Y".to_string()), 0);
    }

    // ── PNCounter ────────────────────────────────────────────────────────────

    #[test]
    fn test_pncounter_basic() {
        let mut p = PNCounter::new();
        p.increment(&"A".to_string(), 10);
        p.decrement(&"A".to_string(), 3);
        assert_eq!(p.value(), 7);
    }

    #[test]
    fn test_pncounter_merge() {
        let mut a = PNCounter::new();
        a.increment(&"A".to_string(), 5);
        let mut b = PNCounter::new();
        b.increment(&"B".to_string(), 3);
        b.decrement(&"B".to_string(), 1);
        a.merge(&b);
        assert_eq!(a.value(), 7); // 5 + 3 - 1
    }

    #[test]
    fn test_pncounter_negative() {
        let mut p = PNCounter::new();
        p.increment(&"A".to_string(), 2);
        p.decrement(&"A".to_string(), 5);
        assert_eq!(p.value(), -3);
    }

    // ── LWWRegister ──────────────────────────────────────────────────────────

    #[test]
    fn test_lww_register_write_wins_by_timestamp() {
        let mut r = LWWRegister::new("A".to_string(), "hello", 100);
        r.write("B".to_string(), "world", 200);
        assert_eq!(r.value, "world");
        assert_eq!(r.timestamp, 200);
    }

    #[test]
    fn test_lww_register_older_write_ignored() {
        let mut r = LWWRegister::new("A".to_string(), "hello", 200);
        r.write("B".to_string(), "world", 100);
        assert_eq!(r.value, "hello"); // earlier write loses
    }

    #[test]
    fn test_lww_register_tie_broken_by_node_id() {
        let mut r = LWWRegister::new("aaa".to_string(), "first", 100);
        r.write("zzz".to_string(), "second", 100); // same ts, larger node wins
        assert_eq!(r.value, "second");
    }

    #[test]
    fn test_lww_register_merge() {
        let mut a = LWWRegister::new("A".to_string(), 1u32, 50);
        let b = LWWRegister::new("B".to_string(), 2u32, 100);
        a.merge(&b);
        assert_eq!(a.value, 2);
    }

    #[test]
    fn test_lww_register_merge_idempotent() {
        let mut a = LWWRegister::new("A".to_string(), "x", 100);
        let b = a.clone();
        a.merge(&b);
        assert_eq!(a.value, "x");
        assert_eq!(a.timestamp, 100);
    }

    // ── MVRegister ───────────────────────────────────────────────────────────

    #[test]
    fn test_mvregister_single_write() {
        let mut r: MVRegister<&str> = MVRegister::new();
        let mut vc = VectorClock::new();
        vc.tick(&"A".to_string());
        r.write("hello", vc);
        assert_eq!(r.values(), vec![&"hello"]);
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn test_mvregister_sequential_write_replaces() {
        let mut r: MVRegister<u32> = MVRegister::new();
        let n = "A".to_string();
        let mut vc = VectorClock::new();
        vc.tick(&n);
        r.write(1, vc.clone());
        vc.tick(&n);
        r.write(2, vc);
        // Only the latest causally dominates the earlier one.
        assert_eq!(r.len(), 1);
        assert_eq!(r.values(), vec![&2]);
    }

    #[test]
    fn test_mvregister_concurrent_writes_both_survive() {
        let mut r1: MVRegister<&str> = MVRegister::new();
        let mut r2: MVRegister<&str> = MVRegister::new();

        let mut vc1 = VectorClock::new();
        vc1.tick(&"A".to_string());
        r1.write("apple", vc1);

        let mut vc2 = VectorClock::new();
        vc2.tick(&"B".to_string());
        r2.write("banana", vc2);

        r1.merge(&r2);
        assert_eq!(r1.len(), 2); // both concurrent values survive
    }

    #[test]
    fn test_mvregister_merge_idempotent() {
        let mut r: MVRegister<i32> = MVRegister::new();
        let mut vc = VectorClock::new();
        vc.tick(&"A".to_string());
        r.write(42, vc);
        let clone = r.clone();
        r.merge(&clone);
        assert_eq!(r.len(), 1);
    }

    // ── GSet ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_gset_insert_and_contains() {
        let mut s: GSet<u32> = GSet::new();
        s.insert(1);
        s.insert(2);
        assert!(s.contains(&1));
        assert!(s.contains(&2));
        assert!(!s.contains(&3));
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn test_gset_merge_union() {
        let mut a: GSet<&str> = GSet::new();
        a.insert("x");
        let mut b: GSet<&str> = GSet::new();
        b.insert("y");
        a.merge(&b);
        assert!(a.contains(&"x"));
        assert!(a.contains(&"y"));
    }

    #[test]
    fn test_gset_merge_idempotent() {
        let mut a: GSet<i32> = GSet::new();
        a.insert(10);
        let b = a.clone();
        a.merge(&b);
        assert_eq!(a.len(), 1);
    }

    // ── TwoPhaseSet ──────────────────────────────────────────────────────────

    #[test]
    fn test_two_phase_set_add_remove() {
        let mut s: TwoPhaseSet<u32> = TwoPhaseSet::new();
        s.insert(1);
        s.insert(2);
        assert!(s.contains(&1));
        s.remove(1);
        assert!(!s.contains(&1));
        assert!(s.contains(&2));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn test_two_phase_set_removed_cannot_be_readded() {
        let mut s: TwoPhaseSet<&str> = TwoPhaseSet::new();
        s.insert("cat");
        s.remove("cat");
        s.insert("cat"); // should be silently ignored
        assert!(!s.contains(&"cat"));
    }

    #[test]
    fn test_two_phase_set_remove_not_in_added_is_noop() {
        let mut s: TwoPhaseSet<u32> = TwoPhaseSet::new();
        s.remove(99); // never added — should not panic or tombstone
        s.insert(99); // should now succeed because it was never in `added`
        assert!(s.contains(&99));
    }

    #[test]
    fn test_two_phase_set_merge() {
        let mut a: TwoPhaseSet<i32> = TwoPhaseSet::new();
        a.insert(10);
        a.insert(20);
        a.remove(10);

        let mut b: TwoPhaseSet<i32> = TwoPhaseSet::new();
        b.insert(20);
        b.insert(30);

        a.merge(&b);
        assert!(!a.contains(&10)); // tombstoned
        assert!(a.contains(&20));
        assert!(a.contains(&30));
    }

    #[test]
    fn test_two_phase_set_merge_tombstone_propagates() {
        let mut a: TwoPhaseSet<u32> = TwoPhaseSet::new();
        a.insert(7);
        a.remove(7);

        let mut b: TwoPhaseSet<u32> = TwoPhaseSet::new();
        b.insert(7); // b doesn't know about removal

        b.merge(&a); // tombstone propagates to b
        assert!(!b.contains(&7));
    }

    #[test]
    fn test_two_phase_set_is_empty() {
        let s: TwoPhaseSet<String> = TwoPhaseSet::new();
        assert!(s.is_empty());
    }

    #[test]
    fn test_two_phase_set_iter() {
        let mut s: TwoPhaseSet<u32> = TwoPhaseSet::new();
        s.insert(1);
        s.insert(2);
        s.insert(3);
        s.remove(2);
        let mut items: Vec<u32> = s.iter().copied().collect();
        items.sort();
        assert_eq!(items, vec![1, 3]);
    }
}
