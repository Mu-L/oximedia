//! Advanced media search and indexing engine for `OxiMedia`.
//!
//! `oximedia-search` provides comprehensive search capabilities for media asset management,
//! including full-text search, visual similarity, audio fingerprinting, faceted search,
//! and advanced query processing.
//!
//! # Features
//!
//! - **Full-text Search**: Search metadata, transcripts, descriptions with fuzzy matching
//! - **Visual Search**: Find similar images and video frames using perceptual hashing
//! - **Audio Fingerprinting**: Identify and match audio content using patent-free algorithms
//! - **Faceted Search**: Filter by multiple criteria (codec, resolution, duration, etc.)
//! - **Boolean Queries**: Support for AND, OR, NOT operators
//! - **Range Queries**: Search by date ranges, duration ranges, numeric ranges
//! - **Reverse Search**: Find clips from sample frames or audio snippets
//! - **Color Search**: Search by dominant colors or color palettes
//! - **Face Search**: Find people in videos using face recognition
//! - **OCR Search**: Search text visible in video frames
//!
//! # Modules
//!
//! - [`index`]: Index building and management
//! - [`text`]: Full-text search implementation
//! - [`visual`]: Visual similarity search
//! - [`audio`]: Audio fingerprinting and matching
//! - [`facet`]: Faceted search and aggregation
//! - [`query`]: Query language parser and execution
//! - [`range`]: Range query support
//! - [`reverse`]: Reverse search (video/image/audio)
//! - [`color`]: Color-based search
//! - [`face`]: Face-based search
//! - [`ocr`]: OCR text search
//! - [`rank`]: Relevance scoring and boosting
//!
//! # Example
//!
//! ```
//! use oximedia_search::SearchQuery;
//!
//! // Build a query
//! let query = SearchQuery {
//!     text: Some("rainforest documentary".to_string()),
//!     visual: None,
//!     audio: None,
//!     filters: Default::default(),
//!     limit: 20,
//!     offset: 0,
//!     sort: Default::default(),
//! };
//! assert!(query.text.is_some());
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::missing_errors_doc)]
#![allow(dead_code)]

pub mod audio;
pub mod bool_query;
pub mod cache;
pub mod color;
pub mod duplicate_detection;
pub mod embedding_search;
pub mod error;
pub mod face;
pub mod facet;
pub mod facets;
pub mod geo_search;
pub mod hierarchical_facets;
pub mod index;
pub mod index_builder;
pub mod index_stats;
pub mod inv_index;
pub mod media_index;
pub mod ocr;
pub mod query;
pub mod query_parser;
pub mod range;
pub mod rank;
pub mod ranking;
pub mod related_content;
pub mod relevance_score;
pub mod result_cache;
pub mod reverse;
pub mod saved_search;
pub mod search_ab_test;
pub mod search_analytics;
pub mod search_cluster;
pub mod search_export;
pub mod search_federation;
pub mod search_filter;
pub mod search_pipeline;
pub mod search_ranking;
pub mod search_result;
pub mod search_rewrite;
pub mod search_suggest;
pub mod semantic;
pub mod spell_suggest;
pub mod temporal;
pub mod text;
pub mod visual;
pub mod vp_tree;

// Re-export commonly used items
pub use error::{SearchError, SearchResult};

use serde::{Deserialize, Serialize};
#[cfg(feature = "search-engine")]
use std::path::Path;
use uuid::Uuid;

/// Main search engine coordinating all search capabilities
#[cfg(feature = "search-engine")]
pub struct SearchEngine {
    /// Text search index
    text_index: text::search::TextSearchIndex,
    /// Visual search index
    visual_index: visual::index::VisualIndex,
    /// Audio fingerprint database
    audio_index: audio::fingerprint::AudioFingerprintIndex,
    /// Face index
    face_index: face::search::FaceIndex,
    /// OCR text index
    ocr_index: ocr::search::OcrIndex,
    /// Color index
    color_index: color::search::ColorIndex,
}

/// Unified search query supporting multiple search types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Text search query (optional)
    pub text: Option<String>,
    /// Visual similarity search (image/frame data)
    pub visual: Option<Vec<u8>>,
    /// Audio fingerprint for matching
    pub audio: Option<Vec<u8>>,
    /// Filters
    pub filters: SearchFilters,
    /// Result limit
    pub limit: usize,
    /// Result offset
    pub offset: usize,
    /// Sort options
    pub sort: SortOptions,
}

/// Search filters for narrowing results
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    /// MIME types to include
    pub mime_types: Vec<String>,
    /// File formats
    pub formats: Vec<String>,
    /// Video codecs
    pub codecs: Vec<String>,
    /// Resolution filters
    pub resolutions: Vec<String>,
    /// Duration range (in milliseconds)
    pub duration_range: Option<(i64, i64)>,
    /// Date range (unix timestamps)
    pub date_range: Option<(i64, i64)>,
    /// File size range (in bytes)
    pub file_size_range: Option<(i64, i64)>,
    /// Dominant colors
    pub colors: Vec<String>,
    /// Keywords
    pub keywords: Vec<String>,
    /// Categories
    pub categories: Vec<String>,
    /// Has faces
    pub has_faces: Option<bool>,
    /// Has OCR text
    pub has_ocr: Option<bool>,
    /// Specific face IDs to match
    pub face_ids: Vec<Uuid>,
    /// Codec-specific filters for detailed media property filtering
    pub codec_filters: Option<CodecFilters>,
}

/// Codec-specific filters for detailed media property filtering.
///
/// Allows narrowing search results by technical media attributes such as
/// bit depth, sample rate, frame rate, color space, and channel layout.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodecFilters {
    /// Audio/video bit depth range (e.g., 8, 10, 16, 24)
    pub bit_depth_range: Option<(u32, u32)>,
    /// Audio sample rate range in Hz (e.g., 44100..96000)
    pub sample_rate_range: Option<(u32, u32)>,
    /// Video frame rate range (e.g., 23.976..60.0)
    pub frame_rate_range: Option<(f64, f64)>,
    /// Color space filter (e.g., "bt709", "bt2020", "srgb", "p3")
    pub color_spaces: Vec<String>,
    /// Audio channel count range (e.g., 1..8)
    pub channel_count_range: Option<(u32, u32)>,
    /// Video scan type filter
    pub scan_type: Option<ScanType>,
    /// Chroma subsampling filter (e.g., "4:2:0", "4:2:2", "4:4:4")
    pub chroma_subsampling: Vec<String>,
    /// Bitrate range in bits per second
    pub bitrate_range: Option<(u64, u64)>,
}

/// Video scan type for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanType {
    /// Progressive scan
    Progressive,
    /// Interlaced scan
    Interlaced,
}

/// Sort options for search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortOptions {
    /// Sort field
    pub field: SortField,
    /// Sort order
    pub order: SortOrder,
}

/// Sort field
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SortField {
    /// Relevance score
    Relevance,
    /// Creation date
    CreatedAt,
    /// Modified date
    ModifiedAt,
    /// Duration
    Duration,
    /// File size
    FileSize,
    /// Title (alphabetical)
    Title,
}

/// Sort order
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SortOrder {
    /// Ascending
    Ascending,
    /// Descending
    Descending,
}

/// Search result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    /// Asset ID
    pub asset_id: Uuid,
    /// Relevance score
    pub score: f32,
    /// Title
    pub title: Option<String>,
    /// Description
    pub description: Option<String>,
    /// File path
    pub file_path: String,
    /// MIME type
    pub mime_type: Option<String>,
    /// Duration (milliseconds)
    pub duration_ms: Option<i64>,
    /// Created timestamp
    pub created_at: i64,
    /// Modified timestamp (unix seconds)
    pub modified_at: Option<i64>,
    /// File size in bytes
    pub file_size: Option<u64>,
    /// Matched fields (for highlighting)
    pub matched_fields: Vec<String>,
    /// Thumbnail URL
    pub thumbnail_url: Option<String>,
}

/// Search results with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    /// Result items
    pub results: Vec<SearchResultItem>,
    /// Total number of matches
    pub total: usize,
    /// Limit applied
    pub limit: usize,
    /// Offset applied
    pub offset: usize,
    /// Facets
    pub facets: facet::aggregation::Facets,
    /// Query execution time (milliseconds)
    pub query_time_ms: u64,
}

#[cfg(feature = "search-engine")]
impl SearchEngine {
    /// Create a new search engine
    ///
    /// # Errors
    ///
    /// Returns an error if index creation fails
    pub fn new(index_path: &Path) -> SearchResult<Self> {
        let text_index = text::search::TextSearchIndex::new(&index_path.join("text"))?;
        let visual_index = visual::index::VisualIndex::new(&index_path.join("visual"))?;
        let audio_index =
            audio::fingerprint::AudioFingerprintIndex::new(&index_path.join("audio"))?;
        let face_index = face::search::FaceIndex::new(&index_path.join("faces"))?;
        let ocr_index = ocr::search::OcrIndex::new(&index_path.join("ocr"))?;
        let color_index = color::search::ColorIndex::new(&index_path.join("colors"))?;

        Ok(Self {
            text_index,
            visual_index,
            audio_index,
            face_index,
            ocr_index,
            color_index,
        })
    }

    /// Execute a unified search query
    ///
    /// # Errors
    ///
    /// Returns an error if search execution fails
    pub fn search(&self, query: &SearchQuery) -> SearchResult<SearchResults> {
        let start = std::time::Instant::now();

        // Execute different search types and combine results
        let mut all_results = Vec::new();

        // Text search
        if let Some(ref text) = query.text {
            let text_results = self.text_index.search(text, query.limit)?;
            all_results.extend(text_results);
        }

        // Visual search
        if let Some(ref visual_data) = query.visual {
            let visual_results = self.visual_index.search_similar(visual_data, query.limit)?;
            all_results.extend(visual_results);
        }

        // Audio search
        if let Some(ref audio_data) = query.audio {
            let audio_results = self.audio_index.search_similar(audio_data, query.limit)?;
            all_results.extend(audio_results);
        }

        // Apply filters
        let filtered_results = self.apply_filters(all_results, &query.filters);

        // Sort results
        let sorted_results = self.sort_results(filtered_results, &query.sort);

        // Collect facets — aggregate over the full (pre-pagination) result set
        // so that every facet bucket reflects all matching documents, not just
        // the current page.
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let facets = facet::aggregation::aggregate_facets(&sorted_results, now_secs);

        // Paginate
        let total = sorted_results.len();
        let paginated: Vec<_> = sorted_results
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .collect();

        let query_time_ms = start.elapsed().as_millis() as u64;

        Ok(SearchResults {
            results: paginated,
            total,
            limit: query.limit,
            offset: query.offset,
            facets,
            query_time_ms,
        })
    }

    /// Apply filters to results
    fn apply_filters(
        &self,
        results: Vec<SearchResultItem>,
        filters: &SearchFilters,
    ) -> Vec<SearchResultItem> {
        results
            .into_iter()
            .filter(|item| {
                // MIME type filter
                if !filters.mime_types.is_empty() {
                    if let Some(ref mime) = item.mime_type {
                        if !filters.mime_types.contains(mime) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // Duration range filter
                if let Some((min, max)) = filters.duration_range {
                    if let Some(duration) = item.duration_ms {
                        if duration < min || duration > max {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // Date range filter
                if let Some((min, max)) = filters.date_range {
                    if item.created_at < min || item.created_at > max {
                        return false;
                    }
                }

                // File size range filter
                if let Some((min, max)) = filters.file_size_range {
                    if let Some(size) = item.file_size {
                        let size_i64 = size as i64;
                        if size_i64 < min || size_i64 > max {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Sort search results
    fn sort_results(
        &self,
        mut results: Vec<SearchResultItem>,
        sort: &SortOptions,
    ) -> Vec<SearchResultItem> {
        results.sort_by(|a, b| {
            let cmp = match sort.field {
                SortField::Relevance => b.score.total_cmp(&a.score),
                SortField::CreatedAt => b.created_at.cmp(&a.created_at),
                SortField::ModifiedAt => {
                    let a_mod = a.modified_at.unwrap_or(a.created_at);
                    let b_mod = b.modified_at.unwrap_or(b.created_at);
                    b_mod.cmp(&a_mod)
                }
                SortField::Duration => {
                    let a_dur = a.duration_ms.unwrap_or(i64::MIN);
                    let b_dur = b.duration_ms.unwrap_or(i64::MIN);
                    b_dur.cmp(&a_dur)
                }
                SortField::FileSize => {
                    let a_size = a.file_size.unwrap_or(0);
                    let b_size = b.file_size.unwrap_or(0);
                    b_size.cmp(&a_size)
                }
                SortField::Title => {
                    let a_title = a.title.as_deref().unwrap_or("");
                    let b_title = b.title.as_deref().unwrap_or("");
                    a_title.cmp(b_title)
                }
            };

            match sort.order {
                SortOrder::Ascending => cmp.reverse(),
                SortOrder::Descending => cmp,
            }
        });

        results
    }

    /// Index a new document
    ///
    /// # Errors
    ///
    /// Returns an error if indexing fails
    pub fn index_document(&mut self, doc: &index::builder::IndexDocument) -> SearchResult<()> {
        // Index in text index
        self.text_index.add_document(doc)?;

        // Index visual features if available
        if let Some(ref visual_features) = doc.visual_features {
            self.visual_index
                .add_document(doc.asset_id, &visual_features.phash)?;
        }

        // Index audio fingerprint if available
        if let Some(ref audio_fingerprint) = doc.audio_fingerprint {
            self.audio_index
                .add_document(doc.asset_id, audio_fingerprint)?;
        }

        // Index faces if available
        if let Some(ref faces) = doc.faces {
            self.face_index.add_faces(doc.asset_id, faces)?;
        }

        // Index OCR text if available
        if let Some(ref ocr_text) = doc.ocr_text {
            self.ocr_index.add_text(doc.asset_id, ocr_text)?;
        }

        // Index colors if available
        if let Some(ref colors) = doc.dominant_colors {
            self.color_index.add_colors(doc.asset_id, colors)?;
        }

        Ok(())
    }

    /// Commit all pending changes
    ///
    /// # Errors
    ///
    /// Returns an error if commit fails
    pub fn commit(&mut self) -> SearchResult<()> {
        self.text_index.commit()?;
        self.visual_index.commit()?;
        self.audio_index.commit()?;
        self.face_index.commit()?;
        self.ocr_index.commit()?;
        self.color_index.commit()?;
        Ok(())
    }

    /// Delete a document by ID
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails
    pub fn delete_document(&mut self, asset_id: Uuid) -> SearchResult<()> {
        self.text_index.delete(asset_id)?;
        self.visual_index.delete(asset_id)?;
        self.audio_index.delete(asset_id)?;
        self.face_index.delete(asset_id)?;
        self.ocr_index.delete(asset_id)?;
        self.color_index.delete(asset_id)?;
        Ok(())
    }
}

impl Default for SortOptions {
    fn default() -> Self {
        Self {
            field: SortField::Relevance,
            order: SortOrder::Descending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_item(
        title: Option<&str>,
        score: f32,
        created_at: i64,
        modified_at: Option<i64>,
        file_size: Option<u64>,
        duration_ms: Option<i64>,
        mime_type: Option<&str>,
    ) -> SearchResultItem {
        SearchResultItem {
            asset_id: Uuid::new_v4(),
            score,
            title: title.map(str::to_string),
            description: None,
            file_path: "/test/file.mp4".to_string(),
            mime_type: mime_type.map(str::to_string),
            duration_ms,
            created_at,
            modified_at,
            file_size,
            matched_fields: Vec::new(),
            thumbnail_url: None,
        }
    }

    #[test]
    fn test_search_query_default() {
        let filters = SearchFilters::default();
        assert!(filters.mime_types.is_empty());
        assert!(filters.duration_range.is_none());
        assert!(filters.codec_filters.is_none());
    }

    #[test]
    fn test_sort_options_default() {
        let sort = SortOptions::default();
        assert!(matches!(sort.field, SortField::Relevance));
        assert!(matches!(sort.order, SortOrder::Descending));
    }

    #[test]
    fn test_search_result_item_has_modified_at() {
        let item = make_test_item(Some("Test"), 1.0, 1000, Some(2000), None, None, None);
        assert_eq!(item.modified_at, Some(2000));
    }

    #[test]
    fn test_search_result_item_has_file_size() {
        let item = make_test_item(Some("Test"), 1.0, 1000, None, Some(1_048_576), None, None);
        assert_eq!(item.file_size, Some(1_048_576));
    }

    #[test]
    fn test_codec_filters_default() {
        let cf = CodecFilters::default();
        assert!(cf.bit_depth_range.is_none());
        assert!(cf.sample_rate_range.is_none());
        assert!(cf.frame_rate_range.is_none());
        assert!(cf.color_spaces.is_empty());
        assert!(cf.channel_count_range.is_none());
        assert!(cf.scan_type.is_none());
        assert!(cf.chroma_subsampling.is_empty());
        assert!(cf.bitrate_range.is_none());
    }

    #[test]
    fn test_codec_filters_with_values() {
        let cf = CodecFilters {
            bit_depth_range: Some((8, 16)),
            sample_rate_range: Some((44100, 96000)),
            frame_rate_range: Some((23.976, 60.0)),
            color_spaces: vec!["bt709".to_string(), "bt2020".to_string()],
            channel_count_range: Some((2, 8)),
            scan_type: Some(ScanType::Progressive),
            chroma_subsampling: vec!["4:2:0".to_string()],
            bitrate_range: Some((1_000_000, 50_000_000)),
        };
        assert_eq!(cf.bit_depth_range, Some((8, 16)));
        assert_eq!(cf.sample_rate_range, Some((44100, 96000)));
        assert!(cf.frame_rate_range.is_some());
        assert_eq!(cf.color_spaces.len(), 2);
        assert_eq!(cf.scan_type, Some(ScanType::Progressive));
        assert_eq!(cf.chroma_subsampling.len(), 1);
    }

    #[test]
    fn test_scan_type_equality() {
        assert_eq!(ScanType::Progressive, ScanType::Progressive);
        assert_ne!(ScanType::Progressive, ScanType::Interlaced);
    }

    #[test]
    fn test_search_filters_with_codec_filters() {
        let filters = SearchFilters {
            codec_filters: Some(CodecFilters {
                bit_depth_range: Some((10, 12)),
                frame_rate_range: Some((24.0, 30.0)),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(filters.codec_filters.is_some());
        let cf = filters.codec_filters.as_ref().expect("should exist");
        assert_eq!(cf.bit_depth_range, Some((10, 12)));
    }

    #[test]
    fn test_codec_filters_serialization() {
        let cf = CodecFilters {
            bit_depth_range: Some((8, 16)),
            scan_type: Some(ScanType::Interlaced),
            ..Default::default()
        };
        let json = serde_json::to_string(&cf).expect("should serialize");
        assert!(json.contains("bit_depth_range"));
        assert!(json.contains("Interlaced"));
    }

    #[test]
    fn test_search_result_item_serialization_with_new_fields() {
        let item = make_test_item(
            Some("Test Video"),
            0.95,
            1000,
            Some(2000),
            Some(5_000_000),
            Some(60_000),
            Some("video/mp4"),
        );
        let json = serde_json::to_string(&item).expect("should serialize");
        assert!(json.contains("modified_at"));
        assert!(json.contains("file_size"));
        assert!(json.contains("2000"));
        assert!(json.contains("5000000"));
    }
}
