//! OCR text extraction.

use crate::error::SearchResult;

/// OCR text extractor
pub struct OcrExtractor;

impl OcrExtractor {
    /// Create a new OCR extractor
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Extract text from image
    ///
    /// # Errors
    ///
    /// Returns an error if extraction fails
    pub fn extract(&self, _image_data: &[u8]) -> SearchResult<String> {
        // Placeholder: would use Tesseract or similar
        Ok(String::new())
    }
}

impl Default for OcrExtractor {
    fn default() -> Self {
        Self::new()
    }
}
