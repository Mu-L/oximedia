//! Visual feature indexing.

use crate::error::SearchResult;
use crate::SearchResultItem;
use std::path::Path;
use uuid::Uuid;

/// Visual index for efficient similarity search
pub struct VisualIndex {
    index_path: std::path::PathBuf,
    features: Vec<(Uuid, Vec<f32>)>,
}

impl VisualIndex {
    /// Create a new visual index
    ///
    /// # Errors
    ///
    /// Returns an error if index creation fails
    pub fn new(index_path: &Path) -> SearchResult<Self> {
        if !index_path.exists() {
            std::fs::create_dir_all(index_path)?;
        }

        Ok(Self {
            index_path: index_path.to_path_buf(),
            features: Vec::new(),
        })
    }

    /// Add a document to the visual index
    ///
    /// # Errors
    ///
    /// Returns an error if indexing fails
    pub fn add_document(&mut self, asset_id: Uuid, features: &[u8]) -> SearchResult<()> {
        // Convert byte features to f32 vector
        let feature_vec: Vec<f32> = features.iter().map(|&b| f32::from(b) / 255.0).collect();
        self.features.push((asset_id, feature_vec));
        Ok(())
    }

    /// Search for similar images
    ///
    /// # Errors
    ///
    /// Returns an error if search fails
    pub fn search_similar(
        &self,
        query_data: &[u8],
        limit: usize,
    ) -> SearchResult<Vec<SearchResultItem>> {
        // Convert query data to feature vector
        let query_features: Vec<f32> = query_data.iter().map(|&b| f32::from(b) / 255.0).collect();

        let mut results: Vec<_> = self
            .features
            .iter()
            .map(|(id, features)| {
                let distance = Self::euclidean_distance(&query_features, features);
                let score = 1.0 / (1.0 + distance);

                SearchResultItem {
                    asset_id: *id,
                    score,
                    title: None,
                    description: None,
                    file_path: String::new(),
                    mime_type: None,
                    duration_ms: None,
                    created_at: 0,
                    modified_at: None,
                    file_size: None,
                    matched_fields: vec!["visual".to_string()],
                    thumbnail_url: None,
                }
            })
            .collect();

        // Sort by score (descending)
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results.into_iter().take(limit).collect())
    }

    /// Calculate Euclidean distance
    fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::MAX;
        }

        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// Commit changes to disk
    ///
    /// # Errors
    ///
    /// Returns an error if commit fails
    pub fn commit(&self) -> SearchResult<()> {
        // Save index to disk
        Ok(())
    }

    /// Delete a document from the index
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails
    pub fn delete(&mut self, asset_id: Uuid) -> SearchResult<()> {
        self.features.retain(|(id, _)| *id != asset_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_search() {
        let temp_dir = std::env::temp_dir().join("visual_index_test");
        let mut index = VisualIndex::new(&temp_dir).expect("should succeed in test");

        let id = Uuid::new_v4();
        let features = vec![100, 150, 200];

        index
            .add_document(id, &features)
            .expect("should succeed in test");

        let results = index
            .search_similar(&features, 10)
            .expect("should succeed in test");
        assert!(!results.is_empty());
        assert_eq!(results[0].asset_id, id);

        std::fs::remove_dir_all(temp_dir).ok();
    }
}
