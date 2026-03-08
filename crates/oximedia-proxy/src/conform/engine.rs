//! Conforming engine for relinking proxies to originals.

use super::edl::EdlConformer;
use crate::{ProxyLinkManager, Result};
use std::path::Path;

/// Conforming engine for proxy-to-original workflows.
pub struct ConformEngine {
    link_manager: ProxyLinkManager,
}

impl ConformEngine {
    /// Create a new conform engine with the specified link database.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened.
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let link_manager = ProxyLinkManager::new(db_path).await?;
        Ok(Self { link_manager })
    }

    /// Conform from an EDL file.
    ///
    /// # Errors
    ///
    /// Returns an error if conforming fails.
    pub async fn conform_from_edl(
        &self,
        edl_path: impl AsRef<Path>,
        output: impl AsRef<Path>,
    ) -> Result<ConformResult> {
        let conformer = EdlConformer::new(&self.link_manager);
        conformer.conform(edl_path, output).await
    }

    /// Relink a single proxy file to its original.
    ///
    /// # Errors
    ///
    /// Returns an error if no link exists for the proxy.
    pub fn relink(&self, proxy_path: impl AsRef<Path>) -> Result<&Path> {
        self.link_manager.get_original(proxy_path)
    }

    /// Get the link manager.
    #[must_use]
    pub const fn link_manager(&self) -> &ProxyLinkManager {
        &self.link_manager
    }
}

/// Result of a conform operation.
#[derive(Debug, Clone)]
pub struct ConformResult {
    /// Output file path.
    pub output_path: std::path::PathBuf,

    /// Number of clips relinked.
    pub clips_relinked: usize,

    /// Number of clips that couldn't be relinked.
    pub clips_failed: usize,

    /// Total duration in seconds.
    pub total_duration: f64,

    /// Frame-accurate conforming was successful.
    pub frame_accurate: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_conform_engine_creation() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_conform.json");

        let engine = ConformEngine::new(&db_path).await;
        assert!(engine.is_ok());

        // Clean up
        let _ = std::fs::remove_file(db_path);
    }
}
