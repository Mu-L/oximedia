//! Near-duplicate detection using a Bloom filter.
//!
//! Provides:
//! - [`BloomFilter`]: space-efficient probabilistic set membership structure
//! - [`NearDuplicateDetector`]: wraps a Bloom filter for streaming deduplication

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

// ---------------------------------------------------------------------------
// Bloom filter
// ---------------------------------------------------------------------------

/// A counting-free Bloom filter backed by a `Vec<u64>` bit array.
///
/// Insert items and test membership with a configurable false-positive rate.
/// Deletions are not supported (standard Bloom filter design).
pub struct BloomFilter {
    /// Bit storage (each `u64` holds 64 bits).
    pub bits: Vec<u64>,
    /// Number of independent hash functions.
    pub num_hashes: u32,
    /// Total number of bits (length of the logical bit array).
    pub bit_count: u64,
}

impl BloomFilter {
    /// Create a new Bloom filter sized for `capacity` items at a given `false_positive_rate`.
    ///
    /// Uses the standard formulas:
    /// - `m = -n * ln(p) / (ln(2))^2`  (number of bits)
    /// - `k = (m / n) * ln(2)`          (number of hash functions)
    #[must_use]
    pub fn new(capacity: usize, false_positive_rate: f32) -> Self {
        let capacity = capacity.max(1);
        let fpr = false_positive_rate.clamp(1e-9, 1.0 - f32::EPSILON);

        let ln2 = std::f64::consts::LN_2;
        let n = capacity as f64;
        let p = fpr as f64;

        let m = (-n * p.ln() / (ln2 * ln2)).ceil() as u64;
        let m = m.max(64); // at least 64 bits
        let k = ((m as f64 / n) * ln2).round() as u32;
        let k = k.clamp(1, 30);

        let words = ((m + 63) / 64) as usize;

        Self {
            bits: vec![0u64; words],
            num_hashes: k,
            bit_count: m,
        }
    }

    /// Insert `item` into the filter.
    pub fn insert(&mut self, item: &[u8]) {
        for i in 0..self.num_hashes {
            let idx = self.hash_index(item, i);
            let word = (idx / 64) as usize;
            let bit = idx % 64;
            self.bits[word] |= 1u64 << bit;
        }
    }

    /// Returns `true` if `item` *may* be in the set (possible false positive).
    /// Returns `false` if `item` is *definitely not* in the set.
    #[must_use]
    pub fn contains(&self, item: &[u8]) -> bool {
        for i in 0..self.num_hashes {
            let idx = self.hash_index(item, i);
            let word = (idx / 64) as usize;
            let bit = idx % 64;
            if self.bits[word] & (1u64 << bit) == 0 {
                return false;
            }
        }
        true
    }

    /// Estimate the number of items inserted using the formula:
    /// `n̂ = -m / k * ln(1 - X / m)`
    /// where `X` is the number of set bits.
    #[must_use]
    pub fn estimated_count(&self) -> usize {
        let set_bits: u64 = self.bits.iter().map(|w| w.count_ones() as u64).sum();
        if set_bits == 0 {
            return 0;
        }
        if set_bits >= self.bit_count {
            // Filter saturated
            return usize::MAX;
        }
        let m = self.bit_count as f64;
        let k = self.num_hashes as f64;
        let x = set_bits as f64;
        let estimate = -(m / k) * (1.0 - x / m).ln();
        estimate.round() as usize
    }

    /// Clear all bits (reset the filter).
    pub fn clear(&mut self) {
        for w in &mut self.bits {
            *w = 0;
        }
    }

    /// Compute bit index for the i-th hash of `item` using FNV-like mixing.
    fn hash_index(&self, item: &[u8], i: u32) -> u64 {
        // Two independent hashes via FNV-1a, then Kirsch-Mitzenmacher combination
        let h1 = fnv1a_64(item);
        let h2 = fnv1a_64_seeded(item, i as u64 ^ 0x9e37_79b9_7f4a_7c15);
        h1.wrapping_add((i as u64).wrapping_mul(h2)) % self.bit_count
    }
}

/// FNV-1a 64-bit hash.
fn fnv1a_64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// FNV-1a 64-bit hash with an additional seed mixed in.
fn fnv1a_64_seeded(data: &[u8], seed: u64) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ seed.wrapping_mul(0x0000_0100_0000_01b3);
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------------------
// NearDuplicateDetector
// ---------------------------------------------------------------------------

/// Streaming near-duplicate detector backed by a Bloom filter.
///
/// Items (fingerprints) are inserted; if the Bloom filter reports a hit the
/// item is considered a near-duplicate.  Because the underlying structure is a
/// Bloom filter the detector never produces false negatives but may produce
/// false positives at a configurable rate.
pub struct NearDuplicateDetector {
    /// Similarity/membership threshold concept – stored for reference only.
    pub threshold: f32,
    /// The underlying Bloom filter.
    pub seen: BloomFilter,
}

impl NearDuplicateDetector {
    /// Create a detector with the given `capacity` (expected number of unique items).
    ///
    /// Uses a false-positive rate of 1%.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            threshold: 0.99,
            seen: BloomFilter::new(capacity, 0.01),
        }
    }

    /// Create with a custom false-positive rate.
    #[must_use]
    pub fn with_fpr(capacity: usize, false_positive_rate: f32) -> Self {
        Self {
            threshold: 1.0 - false_positive_rate,
            seen: BloomFilter::new(capacity, false_positive_rate),
        }
    }

    /// Add `fingerprint` and return `true` if it appears to be a near-duplicate
    /// (i.e., it was already seen before).
    pub fn add_and_check(&mut self, fingerprint: &[u8]) -> bool {
        let duplicate = self.seen.contains(fingerprint);
        self.seen.insert(fingerprint);
        duplicate
    }

    /// Reset the detector (clear all seen fingerprints).
    pub fn reset(&mut self) {
        self.seen.clear();
    }

    /// Estimated number of unique items seen so far.
    #[must_use]
    pub fn estimated_unique_count(&self) -> usize {
        self.seen.estimated_count()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- BloomFilter construction ----

    #[test]
    fn test_bloom_new_non_empty() {
        let bf = BloomFilter::new(1000, 0.01);
        assert!(!bf.bits.is_empty());
        assert!(bf.num_hashes >= 1);
        assert!(bf.bit_count >= 64);
    }

    #[test]
    fn test_bloom_larger_capacity_more_bits() {
        let bf_small = BloomFilter::new(100, 0.01);
        let bf_large = BloomFilter::new(10_000, 0.01);
        assert!(bf_large.bit_count > bf_small.bit_count);
    }

    #[test]
    fn test_bloom_initially_empty() {
        let bf = BloomFilter::new(100, 0.01);
        assert!(!bf.contains(b"hello"));
        assert!(!bf.contains(b"world"));
    }

    #[test]
    fn test_bloom_insert_and_contains() {
        let mut bf = BloomFilter::new(100, 0.01);
        bf.insert(b"oximedia");
        assert!(bf.contains(b"oximedia"));
    }

    #[test]
    fn test_bloom_multiple_inserts() {
        let mut bf = BloomFilter::new(200, 0.01);
        let items: Vec<&[u8]> = vec![b"alpha", b"beta", b"gamma", b"delta", b"epsilon"];
        for item in &items {
            bf.insert(item);
        }
        for item in &items {
            assert!(bf.contains(item), "Item should be present after insert");
        }
    }

    #[test]
    fn test_bloom_clear() {
        let mut bf = BloomFilter::new(100, 0.01);
        bf.insert(b"test_item");
        assert!(bf.contains(b"test_item"));
        bf.clear();
        // After clear, the filter should no longer report the item as present
        assert!(!bf.contains(b"test_item"));
    }

    #[test]
    fn test_bloom_estimated_count_zero_initially() {
        let bf = BloomFilter::new(100, 0.01);
        assert_eq!(bf.estimated_count(), 0);
    }

    #[test]
    fn test_bloom_estimated_count_after_inserts() {
        let mut bf = BloomFilter::new(1000, 0.01);
        for i in 0..100u64 {
            bf.insert(&i.to_le_bytes());
        }
        // Estimate should be in a reasonable range (not exact due to probabilistic nature)
        let est = bf.estimated_count();
        assert!(est > 0, "Estimated count should be positive");
    }

    #[test]
    fn test_bloom_different_fpr_different_hashes() {
        let bf_strict = BloomFilter::new(100, 0.0001);
        let bf_loose = BloomFilter::new(100, 0.1);
        // Stricter FPR → more hash functions
        assert!(bf_strict.num_hashes >= bf_loose.num_hashes);
    }

    // ---- NearDuplicateDetector tests ----

    #[test]
    fn test_near_dup_new() {
        let det = NearDuplicateDetector::new(500);
        assert!(det.seen.bit_count >= 64);
    }

    #[test]
    fn test_near_dup_first_occurrence_not_duplicate() {
        let mut det = NearDuplicateDetector::new(100);
        let result = det.add_and_check(b"unique_fingerprint");
        assert!(!result, "First occurrence should not be a duplicate");
    }

    #[test]
    fn test_near_dup_second_occurrence_is_duplicate() {
        let mut det = NearDuplicateDetector::new(100);
        det.add_and_check(b"duplicate_fingerprint");
        let result = det.add_and_check(b"duplicate_fingerprint");
        assert!(result, "Second occurrence should be detected as duplicate");
    }

    #[test]
    fn test_near_dup_reset_clears_state() {
        let mut det = NearDuplicateDetector::new(100);
        det.add_and_check(b"item_to_reset");
        det.reset();
        // After reset, the same item should not appear as a duplicate
        let result = det.add_and_check(b"item_to_reset");
        assert!(!result, "After reset, item should not be a duplicate");
    }

    #[test]
    fn test_near_dup_multiple_unique_items() {
        let mut det = NearDuplicateDetector::new(1000);
        let items: Vec<Vec<u8>> = (0u64..50).map(|i| i.to_le_bytes().to_vec()).collect();
        for item in &items {
            // First occurrence of each unique item should not be flagged
            let is_dup = det.add_and_check(item);
            // Could theoretically be a false positive, but with 1000 capacity it should not
            assert!(!is_dup, "Unique item should not be flagged as duplicate");
        }
    }

    #[test]
    fn test_near_dup_estimated_unique_count() {
        let mut det = NearDuplicateDetector::new(1000);
        for i in 0u64..20 {
            det.add_and_check(&i.to_le_bytes());
        }
        let est = det.estimated_unique_count();
        assert!(est > 0);
    }
}
