//! Item vector similarity primitives for content-based recommendation.
//!
//! Provides dense feature vectors and a similarity matrix to enable fast
//! nearest-neighbour lookups across a media catalogue.

#![allow(dead_code)]

use std::collections::HashMap;

/// A dense feature vector representing a media item's content attributes.
#[derive(Debug, Clone)]
pub struct ItemVector {
    /// Unique identifier for this item.
    pub id: String,
    /// Feature values; length defines the embedding dimension.
    pub values: Vec<f64>,
}

impl ItemVector {
    /// Create a new item vector.
    #[must_use]
    pub fn new(id: impl Into<String>, values: Vec<f64>) -> Self {
        Self {
            id: id.into(),
            values,
        }
    }

    /// Dot product with another vector of the same dimension.
    ///
    /// Truncates to the shorter length if dimensions differ.
    #[must_use]
    pub fn dot_product(&self, other: &Self) -> f64 {
        self.values
            .iter()
            .zip(other.values.iter())
            .map(|(a, b)| a * b)
            .sum()
    }

    /// Euclidean magnitude (L2 norm) of this vector.
    #[must_use]
    pub fn magnitude(&self) -> f64 {
        self.values.iter().map(|v| v * v).sum::<f64>().sqrt()
    }

    /// Cosine similarity with another vector in `[−1, 1]`.
    ///
    /// Returns `0.0` if either vector has zero magnitude.
    #[must_use]
    pub fn cosine_similarity(&self, other: &Self) -> f64 {
        let denom = self.magnitude() * other.magnitude();
        if denom < f64::EPSILON {
            return 0.0;
        }
        (self.dot_product(other) / denom).clamp(-1.0, 1.0)
    }

    /// Dimension (number of features) of this vector.
    #[must_use]
    pub fn dim(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` if the vector has no components.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Precomputed pairwise similarity scores between items.
///
/// Scores are stored symmetrically: inserting `(a, b, s)` also records
/// `(b, a, s)` so lookups work in both directions.
#[derive(Debug, Clone, Default)]
pub struct SimilarityMatrix {
    /// `scores[a][b] = similarity(a, b)`
    scores: HashMap<String, HashMap<String, f64>>,
}

impl SimilarityMatrix {
    /// Create an empty matrix.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert (or overwrite) the similarity score between two items.
    ///
    /// The score is stored in both directions.
    pub fn insert(&mut self, id_a: impl Into<String>, id_b: impl Into<String>, score: f64) {
        let a = id_a.into();
        let b = id_b.into();
        self.scores
            .entry(a.clone())
            .or_default()
            .insert(b.clone(), score);
        self.scores.entry(b).or_default().insert(a, score);
    }

    /// Retrieve the stored similarity between two items, if present.
    #[must_use]
    pub fn get(&self, id_a: &str, id_b: &str) -> Option<f64> {
        self.scores.get(id_a)?.get(id_b).copied()
    }

    /// Find the `top_k` most similar items to `query_id`, sorted descending.
    ///
    /// The query item itself is excluded from results.
    #[must_use]
    pub fn find_similar(&self, query_id: &str, top_k: usize) -> Vec<(String, f64)> {
        let Some(row) = self.scores.get(query_id) else {
            return Vec::new();
        };
        let mut pairs: Vec<(String, f64)> = row
            .iter()
            .filter(|(id, _)| id.as_str() != query_id)
            .map(|(id, &score)| (id.clone(), score))
            .collect();
        pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        pairs.truncate(top_k);
        pairs
    }

    /// Total number of unique item IDs tracked in the matrix.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.scores.len()
    }

    /// Build a `SimilarityMatrix` from a slice of `ItemVector`s using cosine similarity.
    #[must_use]
    pub fn from_vectors(vectors: &[ItemVector]) -> Self {
        let mut matrix = Self::new();
        for i in 0..vectors.len() {
            for j in (i + 1)..vectors.len() {
                let sim = vectors[i].cosine_similarity(&vectors[j]);
                matrix.insert(vectors[i].id.clone(), vectors[j].id.clone(), sim);
            }
        }
        matrix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec2(id: &str, x: f64, y: f64) -> ItemVector {
        ItemVector::new(id, vec![x, y])
    }

    #[test]
    fn test_dot_product_basic() {
        let a = vec2("a", 1.0, 2.0);
        let b = vec2("b", 3.0, 4.0);
        assert!((a.dot_product(&b) - 11.0).abs() < 1e-10);
    }

    #[test]
    fn test_dot_product_zero() {
        let a = vec2("a", 0.0, 0.0);
        let b = vec2("b", 1.0, 1.0);
        assert!((a.dot_product(&b) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_magnitude_unit_vector() {
        let v = vec2("v", 1.0, 0.0);
        assert!((v.magnitude() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_magnitude_general() {
        let v = vec2("v", 3.0, 4.0);
        assert!((v.magnitude() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec2("a", 1.0, 2.0);
        let b = vec2("b", 1.0, 2.0);
        assert!((a.cosine_similarity(&b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec2("a", 1.0, 0.0);
        let b = vec2("b", 0.0, 1.0);
        assert!(a.cosine_similarity(&b).abs() < 1e-10);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec2("a", 0.0, 0.0);
        let b = vec2("b", 1.0, 2.0);
        assert!((a.cosine_similarity(&b) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_item_vector_dim() {
        let v = ItemVector::new("x", vec![1.0, 2.0, 3.0]);
        assert_eq!(v.dim(), 3);
    }

    #[test]
    fn test_item_vector_is_empty() {
        let v = ItemVector::new("x", vec![]);
        assert!(v.is_empty());
    }

    #[test]
    fn test_similarity_matrix_insert_and_get() {
        let mut m = SimilarityMatrix::new();
        m.insert("a", "b", 0.8);
        assert!((m.get("a", "b").expect("should succeed in test") - 0.8).abs() < 1e-10);
        assert!((m.get("b", "a").expect("should succeed in test") - 0.8).abs() < 1e-10);
    }

    #[test]
    fn test_similarity_matrix_missing_returns_none() {
        let m = SimilarityMatrix::new();
        assert!(m.get("x", "y").is_none());
    }

    #[test]
    fn test_find_similar_ordering() {
        let mut m = SimilarityMatrix::new();
        m.insert("a", "b", 0.9);
        m.insert("a", "c", 0.5);
        m.insert("a", "d", 0.7);
        let results = m.find_similar("a", 3);
        assert_eq!(results[0].0, "b");
        assert_eq!(results[1].0, "d");
    }

    #[test]
    fn test_find_similar_top_k_limit() {
        let mut m = SimilarityMatrix::new();
        for i in 0..10_u32 {
            m.insert("q", format!("item{i}"), f64::from(i) * 0.1);
        }
        let results = m.find_similar("q", 3);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_find_similar_empty_for_unknown() {
        let m = SimilarityMatrix::new();
        assert!(m.find_similar("unknown", 5).is_empty());
    }

    #[test]
    fn test_item_count() {
        let mut m = SimilarityMatrix::new();
        m.insert("a", "b", 0.5);
        m.insert("a", "c", 0.6);
        assert_eq!(m.item_count(), 3);
    }

    #[test]
    fn test_from_vectors_builds_correct_similarity() {
        let vectors = vec![
            ItemVector::new("a", vec![1.0, 0.0]),
            ItemVector::new("b", vec![1.0, 0.0]),
            ItemVector::new("c", vec![0.0, 1.0]),
        ];
        let m = SimilarityMatrix::from_vectors(&vectors);
        let ab = m.get("a", "b").expect("should succeed in test");
        let ac = m.get("a", "c").expect("should succeed in test");
        assert!((ab - 1.0).abs() < 1e-9);
        assert!(ac.abs() < 1e-9);
    }
}
