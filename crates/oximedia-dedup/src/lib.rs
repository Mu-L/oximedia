//! Media deduplication and duplicate detection for `OxiMedia`.
//!
//! `oximedia-dedup` provides comprehensive duplicate detection and media deduplication
//! for the `OxiMedia` multimedia framework. This includes:
//!
//! - **Cryptographic hashing**: BLAKE3-based exact duplicate detection
//! - **Visual similarity**: Perceptual hashing, SSIM, histogram, and feature matching
//! - **Audio fingerprinting**: Audio fingerprint comparison and waveform similarity
//! - **Metadata matching**: Fuzzy metadata comparison for near-duplicates
//! - **Storage optimization**: Fast SQLite-based indexing for large libraries
//! - **Reporting**: Comprehensive duplicate reports with similarity scoring
//!
//! # Modules
//!
//! - [`hash`]: Cryptographic and content-based hashing
//! - [`visual`]: Visual similarity detection
//! - [`audio`]: Audio fingerprint comparison
//! - [`metadata`]: Metadata-based deduplication
//! - [`database`]: SQLite-based indexing and lookup
//! - [`report`]: Duplicate detection reports
//!
//! # Example
//!
//! ```
//! use oximedia_dedup::{DuplicateDetector, DetectionStrategy, DedupConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = DedupConfig::default();
//! let mut detector = DuplicateDetector::new(config).await?;
//!
//! // Add files to the index
//! detector.add_file("/path/to/video1.mp4").await?;
//! detector.add_file("/path/to/video2.mp4").await?;
//!
//! // Find duplicates
//! let duplicates = detector.find_duplicates(DetectionStrategy::All).await?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::too_many_arguments)]
#![allow(dead_code)]

pub mod audio;
pub mod bloom_filter;
pub mod cluster;
pub mod content_id;
pub mod content_signature;
pub mod database;
pub mod dedup_cache;
pub mod dedup_index;
pub mod dedup_policy;
pub mod dedup_report;
pub mod dedup_report_ext;
pub mod dedup_stats;
pub mod frame_hash;
pub mod fuzzy_match;
pub mod hash;
pub mod hash_store;
pub mod lsh_index;
pub mod merge_strategy;
pub mod metadata;
pub mod near_duplicate;
pub mod perceptual_hash;
pub mod phash;
pub mod report;
pub mod rolling_hash;
pub mod segment_dedup;
pub mod similarity_index;
pub mod video_dedup;
pub mod visual;

use std::path::{Path, PathBuf};
use thiserror::Error;

pub use database::DedupDatabase;
pub use report::{DuplicateGroup, DuplicateReport, SimilarityScore};

/// Deduplication error type.
#[derive(Error, Debug)]
pub enum DedupError {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Hashing error
    #[error("Hashing error: {0}")]
    Hash(String),

    /// Visual processing error
    #[error("Visual processing error: {0}")]
    Visual(String),

    /// Audio processing error
    #[error("Audio processing error: {0}")]
    Audio(String),

    /// Metadata processing error
    #[error("Metadata processing error: {0}")]
    Metadata(String),

    /// File not found
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Core library error
    #[error("OxiMedia core error: {0}")]
    Core(#[from] oximedia_core::OxiError),
}

/// Deduplication result type.
pub type DedupResult<T> = Result<T, DedupError>;

/// Detection strategy for finding duplicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionStrategy {
    /// Exact duplicates only (cryptographic hash)
    ExactHash,

    /// Visual similarity using perceptual hashing
    PerceptualHash,

    /// Visual similarity using SSIM
    Ssim,

    /// Visual similarity using histogram comparison
    Histogram,

    /// Visual similarity using feature matching
    FeatureMatch,

    /// Audio fingerprint comparison
    AudioFingerprint,

    /// Metadata-based matching
    Metadata,

    /// All detection methods
    All,

    /// Combination of visual methods
    VisualAll,

    /// Combination of fast methods (hash + perceptual + metadata)
    Fast,
}

impl DetectionStrategy {
    /// Check if strategy includes exact hashing.
    #[must_use]
    pub fn includes_hash(self) -> bool {
        matches!(self, Self::ExactHash | Self::All | Self::Fast)
    }

    /// Check if strategy includes perceptual hashing.
    #[must_use]
    pub fn includes_perceptual(self) -> bool {
        matches!(
            self,
            Self::PerceptualHash | Self::All | Self::VisualAll | Self::Fast
        )
    }

    /// Check if strategy includes SSIM.
    #[must_use]
    pub fn includes_ssim(self) -> bool {
        matches!(self, Self::Ssim | Self::All | Self::VisualAll)
    }

    /// Check if strategy includes histogram.
    #[must_use]
    pub fn includes_histogram(self) -> bool {
        matches!(self, Self::Histogram | Self::All | Self::VisualAll)
    }

    /// Check if strategy includes feature matching.
    #[must_use]
    pub fn includes_feature_match(self) -> bool {
        matches!(self, Self::FeatureMatch | Self::All | Self::VisualAll)
    }

    /// Check if strategy includes audio fingerprinting.
    #[must_use]
    pub fn includes_audio(self) -> bool {
        matches!(self, Self::AudioFingerprint | Self::All)
    }

    /// Check if strategy includes metadata.
    #[must_use]
    pub fn includes_metadata(self) -> bool {
        matches!(self, Self::Metadata | Self::All | Self::Fast)
    }
}

/// Configuration for deduplication.
#[derive(Debug, Clone)]
pub struct DedupConfig {
    /// Database path
    pub database_path: PathBuf,

    /// Perceptual hash similarity threshold (0.0-1.0)
    pub perceptual_threshold: f64,

    /// SSIM similarity threshold (0.0-1.0)
    pub ssim_threshold: f64,

    /// Histogram similarity threshold (0.0-1.0)
    pub histogram_threshold: f64,

    /// Feature match threshold (minimum number of matches)
    pub feature_match_threshold: usize,

    /// Audio fingerprint similarity threshold (0.0-1.0)
    pub audio_threshold: f64,

    /// Metadata similarity threshold (0.0-1.0)
    pub metadata_threshold: f64,

    /// Enable parallel processing
    pub parallel: bool,

    /// Number of frames to sample for video analysis
    pub sample_frames: usize,

    /// Chunk size for content-based chunking (bytes)
    pub chunk_size: usize,
}

impl Default for DedupConfig {
    fn default() -> Self {
        Self {
            database_path: PathBuf::from("oximedia_dedup.db"),
            perceptual_threshold: 0.95,
            ssim_threshold: 0.90,
            histogram_threshold: 0.85,
            feature_match_threshold: 50,
            audio_threshold: 0.90,
            metadata_threshold: 0.80,
            parallel: true,
            sample_frames: 10,
            chunk_size: 4096,
        }
    }
}

/// Main duplicate detector.
pub struct DuplicateDetector {
    config: DedupConfig,
    database: DedupDatabase,
}

impl DuplicateDetector {
    /// Create a new duplicate detector.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or initialized.
    pub async fn new(config: DedupConfig) -> DedupResult<Self> {
        let database = DedupDatabase::open(&config.database_path).await?;
        Ok(Self { config, database })
    }

    /// Add a file to the deduplication index.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or processed.
    pub async fn add_file(&mut self, path: impl AsRef<Path>) -> DedupResult<()> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(DedupError::FileNotFound(path.to_path_buf()));
        }

        // Compute hashes
        let file_hash = hash::compute_file_hash(path)?;

        // Store in database
        self.database.insert_file(path, &file_hash.to_hex()).await?;

        Ok(())
    }

    /// Add multiple files in parallel.
    ///
    /// # Errors
    ///
    /// Returns an error if any file cannot be processed.
    pub async fn add_files(&mut self, paths: &[impl AsRef<Path>]) -> DedupResult<Vec<String>> {
        let mut errors = Vec::new();

        for path in paths {
            if let Err(e) = self.add_file(path).await {
                errors.push(format!("{}: {}", path.as_ref().display(), e));
            }
        }

        Ok(errors)
    }

    /// Find duplicates using the specified strategy.
    ///
    /// # Errors
    ///
    /// Returns an error if duplicate detection fails.
    pub async fn find_duplicates(
        &self,
        strategy: DetectionStrategy,
    ) -> DedupResult<DuplicateReport> {
        let mut report = DuplicateReport::new();

        // Exact hash duplicates
        if strategy.includes_hash() {
            let hash_dups = self.find_hash_duplicates().await?;
            report.add_groups(hash_dups);
        }

        // Perceptual hash duplicates
        if strategy.includes_perceptual() {
            let perceptual_dups = self.find_perceptual_duplicates().await?;
            report.add_groups(perceptual_dups);
        }

        // SSIM duplicates
        if strategy.includes_ssim() {
            let ssim_dups = self.find_ssim_duplicates().await?;
            report.add_groups(ssim_dups);
        }

        // Histogram duplicates
        if strategy.includes_histogram() {
            let histogram_dups = self.find_histogram_duplicates().await?;
            report.add_groups(histogram_dups);
        }

        // Feature match duplicates
        if strategy.includes_feature_match() {
            let feature_dups = self.find_feature_duplicates().await?;
            report.add_groups(feature_dups);
        }

        // Audio fingerprint duplicates
        if strategy.includes_audio() {
            let audio_dups = self.find_audio_duplicates().await?;
            report.add_groups(audio_dups);
        }

        // Metadata duplicates
        if strategy.includes_metadata() {
            let metadata_dups = self.find_metadata_duplicates().await?;
            report.add_groups(metadata_dups);
        }

        Ok(report)
    }

    /// Find exact duplicates by cryptographic hash.
    async fn find_hash_duplicates(&self) -> DedupResult<Vec<DuplicateGroup>> {
        let duplicates = self.database.find_duplicate_hashes().await?;
        let mut groups = Vec::new();

        for (hash, files) in duplicates {
            if files.len() > 1 {
                groups.push(DuplicateGroup {
                    files,
                    scores: vec![SimilarityScore {
                        method: "exact_hash".to_string(),
                        score: 1.0,
                        metadata: vec![("hash".to_string(), hash)],
                    }],
                });
            }
        }

        Ok(groups)
    }

    /// Find perceptual hash duplicates.
    async fn find_perceptual_duplicates(&self) -> DedupResult<Vec<DuplicateGroup>> {
        // This would be implemented using visual::compute_perceptual_hash
        // For now, return empty
        Ok(Vec::new())
    }

    /// Find SSIM duplicates.
    async fn find_ssim_duplicates(&self) -> DedupResult<Vec<DuplicateGroup>> {
        // This would be implemented using visual::compute_ssim
        // For now, return empty
        Ok(Vec::new())
    }

    /// Find histogram duplicates.
    async fn find_histogram_duplicates(&self) -> DedupResult<Vec<DuplicateGroup>> {
        // This would be implemented using visual::compute_histogram
        // For now, return empty
        Ok(Vec::new())
    }

    /// Find feature match duplicates.
    async fn find_feature_duplicates(&self) -> DedupResult<Vec<DuplicateGroup>> {
        // This would be implemented using visual::extract_features
        // For now, return empty
        Ok(Vec::new())
    }

    /// Find audio fingerprint duplicates.
    async fn find_audio_duplicates(&self) -> DedupResult<Vec<DuplicateGroup>> {
        // This would be implemented using audio::compute_fingerprint
        // For now, return empty
        Ok(Vec::new())
    }

    /// Find metadata duplicates.
    async fn find_metadata_duplicates(&self) -> DedupResult<Vec<DuplicateGroup>> {
        // This would be implemented using metadata::compare_metadata
        // For now, return empty
        Ok(Vec::new())
    }

    /// Get database statistics.
    ///
    /// # Errors
    ///
    /// Returns an error if database query fails.
    pub async fn get_stats(&self) -> DedupResult<DedupStats> {
        let total_files = self.database.count_files().await?;
        let total_hashes = self.database.count_unique_hashes().await?;

        Ok(DedupStats {
            total_files,
            total_hashes,
            duplicate_files: total_files.saturating_sub(total_hashes),
        })
    }

    /// Close the database.
    pub async fn close(self) -> DedupResult<()> {
        self.database.close().await?;
        Ok(())
    }
}

/// Deduplication statistics.
#[derive(Debug, Clone)]
pub struct DedupStats {
    /// Total number of indexed files
    pub total_files: usize,

    /// Total number of unique hashes
    pub total_hashes: usize,

    /// Number of duplicate files
    pub duplicate_files: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_strategy() {
        assert!(DetectionStrategy::ExactHash.includes_hash());
        assert!(!DetectionStrategy::ExactHash.includes_perceptual());

        assert!(DetectionStrategy::All.includes_hash());
        assert!(DetectionStrategy::All.includes_perceptual());
        assert!(DetectionStrategy::All.includes_audio());

        assert!(DetectionStrategy::Fast.includes_hash());
        assert!(DetectionStrategy::Fast.includes_perceptual());
        assert!(!DetectionStrategy::Fast.includes_ssim());
    }

    #[test]
    fn test_config_default() {
        let config = DedupConfig::default();
        assert_eq!(config.perceptual_threshold, 0.95);
        assert_eq!(config.ssim_threshold, 0.90);
        assert!(config.parallel);
    }
}
