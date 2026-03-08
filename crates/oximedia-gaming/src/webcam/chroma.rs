//! Chroma key (green screen) removal.

/// Chroma key processor.
#[allow(dead_code)]
pub struct ChromaKey {
    config: ChromaKeyConfig,
}

/// Chroma key configuration.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ChromaKeyConfig {
    /// Key color (RGB)
    pub key_color: (u8, u8, u8),
    /// Similarity threshold (0.0 to 1.0)
    pub similarity: f32,
    /// Smoothness (0.0 to 1.0)
    pub smoothness: f32,
    /// Spill reduction (0.0 to 1.0)
    pub spill_reduction: f32,
}

impl ChromaKey {
    /// Create a new chroma key processor.
    #[must_use]
    pub fn new(config: ChromaKeyConfig) -> Self {
        Self { config }
    }
}

impl Default for ChromaKeyConfig {
    fn default() -> Self {
        Self {
            key_color: (0, 255, 0), // Green
            similarity: 0.4,
            smoothness: 0.08,
            spill_reduction: 0.1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chroma_key_creation() {
        let _chroma = ChromaKey::new(ChromaKeyConfig::default());
    }
}
