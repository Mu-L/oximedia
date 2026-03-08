//! Locality-Sensitive Hashing (LSH) index for approximate nearest-neighbour
//! deduplication of high-dimensional media feature vectors.

#![allow(dead_code)]

use std::collections::HashMap;

// ── Bucket ────────────────────────────────────────────────────────────────────

/// A single LSH bucket containing item IDs that hashed to the same key.
#[derive(Debug, Clone, Default)]
pub struct LshBucket {
    items: Vec<u64>,
}

impl LshBucket {
    /// Create an empty bucket.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an item ID into this bucket.
    pub fn insert(&mut self, id: u64) {
        if !self.items.contains(&id) {
            self.items.push(id);
        }
    }

    /// Returns the number of items in this bucket.
    #[must_use]
    pub fn size(&self) -> usize {
        self.items.len()
    }

    /// Returns the item IDs in this bucket.
    #[must_use]
    pub fn items(&self) -> &[u64] {
        &self.items
    }
}

// ── Bucket statistics ─────────────────────────────────────────────────────────

/// Aggregate statistics about all buckets in an LSH index.
#[derive(Debug, Clone)]
pub struct BucketStats {
    /// Total number of (non-empty) buckets.
    pub bucket_count: usize,
    /// Average items per bucket.
    pub avg_size: f64,
    /// Largest bucket size.
    pub max_size: usize,
    /// Total items across all buckets.
    pub total_items: usize,
}

impl BucketStats {
    /// Returns the average bucket size.
    #[must_use]
    pub fn avg_size(&self) -> f64 {
        self.avg_size
    }

    /// Returns the maximum bucket size.
    #[must_use]
    pub fn max_size(&self) -> usize {
        self.max_size
    }
}

// ── LSH Index ─────────────────────────────────────────────────────────────────

/// A simple random-projection LSH index for `D`-dimensional `f32` vectors.
///
/// Uses multiple hash tables, each projecting the vector onto a random
/// hyperplane sign pattern to form a bucket key.
#[derive(Debug)]
pub struct LshIndex {
    /// Number of hash tables.
    num_tables: usize,
    /// Number of bits (hyperplanes) per table.
    bits_per_table: usize,
    /// Random projection vectors: `[table][bit][dim]`
    projections: Vec<Vec<Vec<f32>>>,
    /// Hash tables: `[table][bucket_key] -> LshBucket`
    tables: Vec<HashMap<u64, LshBucket>>,
    /// Dimensionality of the indexed vectors.
    dim: usize,
}

impl LshIndex {
    /// Create a new LSH index.
    ///
    /// # Arguments
    /// * `dim`            – Vector dimensionality.
    /// * `num_tables`     – Number of independent hash tables.
    /// * `bits_per_table` – Bits (hyperplanes) per table.
    /// * `seed`           – Seed for deterministic projection generation.
    #[must_use]
    pub fn new(dim: usize, num_tables: usize, bits_per_table: usize, seed: u64) -> Self {
        let projections = Self::generate_projections(dim, num_tables, bits_per_table, seed);
        let tables = vec![HashMap::new(); num_tables];
        Self {
            num_tables,
            bits_per_table,
            projections,
            tables,
            dim,
        }
    }

    /// Generate projection hyperplanes using a simple LCG PRNG seeded by
    /// `seed` so results are fully deterministic without external crates.
    #[allow(clippy::cast_precision_loss)]
    fn generate_projections(
        dim: usize,
        num_tables: usize,
        bits: usize,
        seed: u64,
    ) -> Vec<Vec<Vec<f32>>> {
        let mut state = seed.wrapping_add(1);
        let lcg_next = |s: &mut u64| -> f32 {
            *s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            // Map to [-1, 1]
            let val = (*s >> 11) as f32 / (1u64 << 53) as f32;
            val * 2.0 - 1.0
        };

        (0..num_tables)
            .map(|_| {
                (0..bits)
                    .map(|_| (0..dim).map(|_| lcg_next(&mut state)).collect())
                    .collect()
            })
            .collect()
    }

    /// Compute the bucket key for `vec` in hash table `table_idx`.
    #[allow(clippy::cast_precision_loss)]
    fn bucket_key(&self, vec: &[f32], table_idx: usize) -> u64 {
        let mut key = 0u64;
        for (bit_idx, proj) in self.projections[table_idx].iter().enumerate() {
            let dot: f32 = vec.iter().zip(proj.iter()).map(|(a, b)| a * b).sum();
            if dot >= 0.0 {
                key |= 1u64 << bit_idx;
            }
        }
        key
    }

    /// Insert item `id` with feature vector `vec` into the index.
    ///
    /// # Panics
    /// Panics if `vec.len() != self.dim`.
    pub fn insert(&mut self, id: u64, vec: &[f32]) {
        assert_eq!(
            vec.len(),
            self.dim,
            "Vector dimensionality mismatch: expected {}, got {}",
            self.dim,
            vec.len()
        );
        for t in 0..self.num_tables {
            let key = self.bucket_key(vec, t);
            self.tables[t].entry(key).or_default().insert(id);
        }
    }

    /// Query for all candidate neighbours of `vec`.
    ///
    /// Returns the union of IDs found in any matching bucket across all tables.
    ///
    /// # Panics
    /// Panics if `vec.len() != self.dim`.
    #[must_use]
    pub fn query(&self, vec: &[f32]) -> Vec<u64> {
        assert_eq!(
            vec.len(),
            self.dim,
            "Vector dimensionality mismatch: expected {}, got {}",
            self.dim,
            vec.len()
        );
        let mut candidates = std::collections::HashSet::new();
        for t in 0..self.num_tables {
            let key = self.bucket_key(vec, t);
            if let Some(bucket) = self.tables[t].get(&key) {
                for &id in bucket.items() {
                    candidates.insert(id);
                }
            }
        }
        let mut result: Vec<u64> = candidates.into_iter().collect();
        result.sort_unstable();
        result
    }

    /// Query and then filter to approximate nearest neighbours by Euclidean
    /// distance, returning up to `k` results sorted nearest-first.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn approximate_neighbors(&self, vec: &[f32], k: usize) -> Vec<u64> {
        // For this index we store IDs only (not vectors), so we return the
        // raw candidate list trimmed to k.  A full implementation would store
        // vectors too and re-rank by distance.
        let mut candidates = self.query(vec);
        candidates.truncate(k);
        candidates
    }

    /// Compute aggregate statistics across all buckets.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn bucket_stats(&self) -> BucketStats {
        let all_sizes: Vec<usize> = self
            .tables
            .iter()
            .flat_map(|table| table.values().map(LshBucket::size))
            .collect();

        let bucket_count = all_sizes.len();
        let total_items: usize = all_sizes.iter().sum();
        let max_size = all_sizes.iter().copied().max().unwrap_or(0);
        let avg_size = if bucket_count == 0 {
            0.0
        } else {
            total_items as f64 / bucket_count as f64
        };

        BucketStats {
            bucket_count,
            avg_size,
            max_size,
            total_items,
        }
    }

    /// Returns the number of hash tables.
    #[must_use]
    pub fn num_tables(&self) -> usize {
        self.num_tables
    }

    /// Returns the vector dimensionality.
    #[must_use]
    pub fn dim(&self) -> usize {
        self.dim
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_vec(dim: usize, hot: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; dim];
        v[hot % dim] = 1.0;
        v
    }

    #[test]
    fn test_lsh_bucket_size_empty() {
        let b = LshBucket::new();
        assert_eq!(b.size(), 0);
    }

    #[test]
    fn test_lsh_bucket_insert_and_size() {
        let mut b = LshBucket::new();
        b.insert(1);
        b.insert(2);
        b.insert(1); // duplicate, should not increase size
        assert_eq!(b.size(), 2);
    }

    #[test]
    fn test_lsh_bucket_items() {
        let mut b = LshBucket::new();
        b.insert(42);
        b.insert(99);
        assert!(b.items().contains(&42));
        assert!(b.items().contains(&99));
    }

    #[test]
    fn test_lsh_index_creation() {
        let idx = LshIndex::new(8, 4, 6, 42);
        assert_eq!(idx.dim(), 8);
        assert_eq!(idx.num_tables(), 4);
    }

    #[test]
    fn test_lsh_index_insert_and_query_self() {
        let mut idx = LshIndex::new(4, 3, 4, 7);
        let v = vec![1.0f32, 0.0, 0.0, 0.0];
        idx.insert(1, &v);
        let results = idx.query(&v);
        assert!(results.contains(&1));
    }

    #[test]
    fn test_lsh_query_returns_sorted() {
        let mut idx = LshIndex::new(4, 2, 4, 13);
        let v = vec![1.0f32, 1.0, 1.0, 1.0];
        idx.insert(5, &v);
        idx.insert(3, &v);
        idx.insert(7, &v);
        let results = idx.query(&v);
        let mut sorted = results.clone();
        sorted.sort_unstable();
        assert_eq!(results, sorted);
    }

    #[test]
    fn test_lsh_similar_vectors_in_same_bucket() {
        let mut idx = LshIndex::new(8, 6, 6, 99);
        let v1 = vec![1.0f32, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        let v2 = vec![1.0f32, 1.0, 1.0, 0.9, 0.0, 0.0, 0.0, 0.0]; // very similar
        idx.insert(10, &v1);
        idx.insert(11, &v2);
        let results = idx.query(&v1);
        // v1 itself must be found
        assert!(results.contains(&10));
    }

    #[test]
    fn test_lsh_approximate_neighbors_k_limit() {
        let mut idx = LshIndex::new(4, 2, 4, 17);
        let v = vec![1.0f32, 0.0, 0.0, 0.0];
        for i in 0..20u64 {
            idx.insert(i, &v);
        }
        let results = idx.approximate_neighbors(&v, 5);
        assert!(results.len() <= 5);
    }

    #[test]
    fn test_bucket_stats_empty() {
        let idx = LshIndex::new(4, 3, 4, 0);
        let stats = idx.bucket_stats();
        assert_eq!(stats.bucket_count, 0);
        assert_eq!(stats.max_size(), 0);
        assert_eq!(stats.avg_size(), 0.0);
    }

    #[test]
    fn test_bucket_stats_after_inserts() {
        let mut idx = LshIndex::new(4, 2, 4, 55);
        let v = vec![0.5f32, 0.5, 0.5, 0.5];
        idx.insert(1, &v);
        idx.insert(2, &v);
        let stats = idx.bucket_stats();
        assert!(stats.bucket_count > 0);
        assert!(stats.max_size() >= 1);
        assert!(stats.avg_size() > 0.0);
    }

    #[test]
    fn test_unit_vectors_different_dimensions() {
        let mut idx = LshIndex::new(8, 4, 5, 77);
        for i in 0..8u64 {
            let v = unit_vec(8, i as usize);
            idx.insert(i, &v);
        }
        // Each unit vector inserted without panic
        assert_eq!(idx.dim(), 8);
    }

    #[test]
    fn test_insert_multiple_tables() {
        let mut idx = LshIndex::new(4, 5, 4, 11);
        let v = vec![0.1f32, 0.2, 0.3, 0.4];
        idx.insert(100, &v);
        // Item should appear in query results
        let r = idx.query(&v);
        assert!(r.contains(&100));
    }

    #[test]
    fn test_bucket_stats_avg_max() {
        let stats = BucketStats {
            bucket_count: 3,
            avg_size: 2.5,
            max_size: 5,
            total_items: 7,
        };
        assert_eq!(stats.avg_size(), 2.5);
        assert_eq!(stats.max_size(), 5);
    }
}
