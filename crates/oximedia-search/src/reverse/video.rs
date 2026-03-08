//! Reverse video search.

use crate::error::SearchResult;
use uuid::Uuid;

/// Reverse video search result
#[derive(Debug, Clone)]
pub struct ReverseVideoResult {
    /// Asset ID
    pub asset_id: Uuid,
    /// Confidence score
    pub confidence: f32,
    /// Timestamp in source video (ms)
    pub timestamp_ms: i64,
}

/// Reverse video search engine
pub struct ReverseVideoSearch;

impl ReverseVideoSearch {
    /// Create a new reverse video search
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Find source video from a sample frame
    ///
    /// # Errors
    ///
    /// Returns an error if search fails
    pub fn search_frame(&self, _frame_data: &[u8]) -> SearchResult<Vec<ReverseVideoResult>> {
        // Placeholder
        Ok(Vec::new())
    }
}

impl Default for ReverseVideoSearch {
    fn default() -> Self {
        Self::new()
    }
}
