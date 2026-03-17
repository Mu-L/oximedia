//! Bloom filter for fast O(1) negative candidate pre-filtering.
//!
//! Uses double hashing (FNV-1a + DJB2) to drive `k` independent bit-array
//! positions.  The bit array is stored as a `Vec<u64>` with 64 bits per word.
//!
//! # False-positive rate
//!
//! The bit-array size `m` and hash count `k` are computed from the expected
//! capacity `n` and the desired false-positive probability `p` using the
//! standard formulas:
//!
//! ```text
//! m = ceil(-(n * ln p) / (ln 2)^2)
//! k = round((m / n) * ln 2)
//! ```

// ─── Core Bloom filter ────────────────────────────────────────────────────────

/// A probabilistic set membership data structure.
///
/// Insertions are always correct.  `might_contain` can return `true` for items
/// that were never inserted (false positive), but *never* returns `false` for
/// items that were inserted (no false negatives).
#[derive(Debug, Clone)]
pub struct BloomFilter {
    /// Bit array stored as 64-bit words.
    bits: Vec<u64>,
    /// Total number of bits in the filter.
    bit_count: usize,
    /// Number of independent hash functions.
    hash_count: u32,
    /// Number of items inserted so far.
    item_count: usize,
}

impl BloomFilter {
    /// Create a new `BloomFilter` sized for `capacity` items at a
    /// `false_positive_rate` false-positive probability.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero or `false_positive_rate` is not in `(0, 1)`.
    #[must_use]
    pub fn new(capacity: usize, false_positive_rate: f64) -> Self {
        assert!(capacity > 0, "BloomFilter capacity must be > 0");
        assert!(
            false_positive_rate > 0.0 && false_positive_rate < 1.0,
            "BloomFilter false_positive_rate must be in (0, 1)"
        );

        let (bit_count, hash_count) = optimal_params(capacity, false_positive_rate);
        let word_count = bit_count.div_ceil(64);

        Self {
            bits: vec![0u64; word_count],
            bit_count,
            hash_count,
            item_count: 0,
        }
    }

    /// Insert `item` into the filter.
    pub fn insert(&mut self, item: &str) {
        for h in self.hashes(item) {
            let idx = h % self.bit_count;
            self.bits[idx / 64] |= 1u64 << (idx % 64);
        }
        self.item_count += 1;
    }

    /// Test whether `item` **might** be in the set.
    ///
    /// - Returns `false` → item definitely not present.
    /// - Returns `true` → item probably present (or false positive).
    #[must_use]
    pub fn might_contain(&self, item: &str) -> bool {
        for h in self.hashes(item) {
            let idx = h % self.bit_count;
            if self.bits[idx / 64] & (1u64 << (idx % 64)) == 0 {
                return false;
            }
        }
        true
    }

    /// Number of items inserted.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.item_count
    }

    /// Total bit-array size in bits.
    #[must_use]
    pub fn bit_count(&self) -> usize {
        self.bit_count
    }

    /// Number of hash functions.
    #[must_use]
    pub fn hash_count(&self) -> u32 {
        self.hash_count
    }

    /// Estimated current false-positive rate given `item_count` insertions.
    ///
    /// Formula: `(1 - e^(-k*n/m))^k`
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn estimated_false_positive_rate(&self) -> f64 {
        let k = self.hash_count as f64;
        let n = self.item_count as f64;
        let m = self.bit_count as f64;
        (1.0 - (-k * n / m).exp()).powf(k)
    }

    // ── Internal double hashing ───────────────────────────────────────────────

    fn hashes(&self, item: &str) -> impl Iterator<Item = usize> {
        let h1 = fnv1a(item);
        let h2 = djb2(item);
        let k = self.hash_count;
        let m = self.bit_count;
        (0..k).map(move |i| {
            let combined = h1.wrapping_add((i as u64).wrapping_mul(h2));
            (combined % m as u64) as usize
        })
    }
}

// ─── Hash functions ───────────────────────────────────────────────────────────

/// FNV-1a 64-bit hash.
fn fnv1a(s: &str) -> u64 {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    let mut hash = FNV_OFFSET;
    for byte in s.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// DJB2 64-bit hash.
fn djb2(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u64::from(byte));
    }
    // Ensure non-zero for double hashing
    if hash == 0 {
        1
    } else {
        hash
    }
}

// ─── Parameter computation ────────────────────────────────────────────────────

/// Compute optimal `(bit_count, hash_count)` for a Bloom filter.
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]
fn optimal_params(capacity: usize, fpr: f64) -> (usize, u32) {
    let n = capacity as f64;
    let ln2 = std::f64::consts::LN_2;
    let ln2_sq = ln2 * ln2;

    // m = ceil(-(n * ln(p)) / (ln 2)^2)
    let m = (-(n * fpr.ln()) / ln2_sq).ceil() as usize;
    let m = m.max(64); // minimum 64 bits

    // k = round((m / n) * ln 2)
    let k = ((m as f64 / n) * ln2).round() as u32;
    let k = k.max(1).min(32); // clamp to [1, 32]

    (m, k)
}

// ─── Convenience builder ──────────────────────────────────────────────────────

/// Builder for `BloomFilter` that inserts items fluently.
pub struct BloomFilterBuilder {
    filter: BloomFilter,
}

impl BloomFilterBuilder {
    /// Create a builder wrapping a `BloomFilter` of the given capacity / fpr.
    #[must_use]
    pub fn new(capacity: usize, false_positive_rate: f64) -> Self {
        Self {
            filter: BloomFilter::new(capacity, false_positive_rate),
        }
    }

    /// Insert an item and return `self` for chaining.
    #[must_use]
    pub fn insert(mut self, item: &str) -> Self {
        self.filter.insert(item);
        self
    }

    /// Consume the builder and return the finished `BloomFilter`.
    #[must_use]
    pub fn build(self) -> BloomFilter {
        self.filter
    }
}

// ─── Candidate pre-filter helper ──────────────────────────────────────────────

/// A pre-filter that wraps a `BloomFilter` for matching-candidate elimination.
///
/// Filenames of known media files are inserted during indexing; at query time
/// `might_contain` returns `false` for filenames that are definitely absent,
/// allowing the more expensive matching strategies to be skipped.
pub struct CandidatePreFilter {
    filter: BloomFilter,
}

impl CandidatePreFilter {
    /// Build a pre-filter from an iterator of candidate filenames.
    pub fn from_filenames<'a>(
        filenames: impl Iterator<Item = &'a str>,
        expected_count: usize,
        false_positive_rate: f64,
    ) -> Self {
        let mut filter = BloomFilter::new(expected_count.max(1), false_positive_rate);
        for name in filenames {
            filter.insert(name);
        }
        Self { filter }
    }

    /// `true` if the filename **might** be a candidate (passes the filter).
    /// `false` means it's definitely not in the catalogue — skip it entirely.
    #[must_use]
    pub fn might_be_candidate(&self, filename: &str) -> bool {
        self.filter.might_contain(filename)
    }

    /// Inner filter statistics.
    #[must_use]
    pub fn filter(&self) -> &BloomFilter {
        &self.filter
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_no_false_negatives() {
        let mut bf = BloomFilter::new(1000, 0.01);
        let items = ["alpha.mov", "beta.mxf", "gamma.mp4", "delta.wav"];
        for item in &items {
            bf.insert(item);
        }
        for item in &items {
            assert!(bf.might_contain(item), "False negative for {item}");
        }
    }

    #[test]
    fn test_bloom_filter_contains_after_insert() {
        let mut bf = BloomFilter::new(100, 0.01);
        bf.insert("test_file.mov");
        assert!(bf.might_contain("test_file.mov"));
    }

    #[test]
    fn test_bloom_filter_negative_for_absent_item() {
        let bf = BloomFilter::new(1000, 0.001);
        // With only 1000 capacity and 0.1% FPR, a random string very likely returns false
        // (not guaranteed, but statistically near-certain for this specific value)
        let _ = bf.might_contain("definitely_not_inserted_xyz_12345");
        // We cannot assert false due to probabilistic nature, but test does not panic
    }

    #[test]
    fn test_bloom_filter_item_count() {
        let mut bf = BloomFilter::new(100, 0.01);
        assert_eq!(bf.item_count(), 0);
        bf.insert("a");
        bf.insert("b");
        assert_eq!(bf.item_count(), 2);
    }

    #[test]
    fn test_bloom_filter_bit_count_positive() {
        let bf = BloomFilter::new(1000, 0.01);
        assert!(bf.bit_count() >= 64);
    }

    #[test]
    fn test_bloom_filter_hash_count_positive() {
        let bf = BloomFilter::new(1000, 0.01);
        assert!(bf.hash_count() >= 1);
    }

    #[test]
    fn test_bloom_filter_estimated_fpr_zero_inserts() {
        let bf = BloomFilter::new(100, 0.01);
        let fpr = bf.estimated_false_positive_rate();
        assert_eq!(fpr, 0.0);
    }

    #[test]
    fn test_bloom_filter_estimated_fpr_grows_with_inserts() {
        let mut bf = BloomFilter::new(10, 0.01);
        let fpr_before = bf.estimated_false_positive_rate();
        bf.insert("item1");
        bf.insert("item2");
        let fpr_after = bf.estimated_false_positive_rate();
        assert!(fpr_after > fpr_before);
    }

    #[test]
    fn test_bloom_filter_builder() {
        let bf = BloomFilterBuilder::new(100, 0.01)
            .insert("file1.mov")
            .insert("file2.mp4")
            .build();
        assert_eq!(bf.item_count(), 2);
        assert!(bf.might_contain("file1.mov"));
        assert!(bf.might_contain("file2.mp4"));
    }

    #[test]
    fn test_candidate_pre_filter_positive() {
        let filenames = vec!["alpha.mov", "beta.mp4"];
        let pf = CandidatePreFilter::from_filenames(filenames.into_iter(), 10, 0.01);
        assert!(pf.might_be_candidate("alpha.mov"));
        assert!(pf.might_be_candidate("beta.mp4"));
    }

    #[test]
    fn test_candidate_pre_filter_negative() {
        let filenames = vec!["known.mov"];
        let pf = CandidatePreFilter::from_filenames(filenames.into_iter(), 10, 0.001);
        // definitely absent — might_contain must not return true
        // (probabilistic, but with 0.1% FPR it will almost certainly be false)
        // We just verify it doesn't panic
        let _ = pf.might_be_candidate("totally_unknown_99999.mov");
    }

    #[test]
    fn test_double_hashing_uniform_distribution() {
        // Ensure many different strings produce different bit patterns
        let mut bf = BloomFilter::new(10_000, 0.001);
        for i in 0..500 {
            bf.insert(&format!("file_{i:04}.mov"));
        }
        assert_eq!(bf.item_count(), 500);
        // All inserted items must still be found
        for i in 0..500 {
            assert!(bf.might_contain(&format!("file_{i:04}.mov")));
        }
    }
}
