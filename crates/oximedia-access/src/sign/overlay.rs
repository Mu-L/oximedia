//! Sign language video overlay.

use crate::error::{AccessError, AccessResult};
use crate::sign::SignConfig;

/// Sign language overlay manager.
#[derive(Debug, Clone)]
pub struct SignLanguageOverlay {
    config: SignConfig,
}

impl SignLanguageOverlay {
    /// Create a new sign language overlay.
    #[must_use]
    pub const fn new(config: SignConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn default() -> Self {
        Self::new(SignConfig::default())
    }

    /// Apply sign language overlay to video frame.
    ///
    /// This would composite the sign language video onto the main video.
    pub fn apply(&self, _main_frame: &[u8], _sign_frame: &[u8]) -> AccessResult<Vec<u8>> {
        // In production, this would:
        // 1. Scale sign language video to configured size
        // 2. Position it according to config
        // 3. Apply border if configured
        // 4. Composite onto main frame with opacity

        Ok(vec![])
    }

    /// Get configuration.
    #[must_use]
    pub const fn config(&self) -> &SignConfig {
        &self.config
    }

    /// Update configuration.
    pub fn set_config(&mut self, config: SignConfig) {
        self.config = config;
    }

    /// Validate configuration.
    pub fn validate(&self) -> AccessResult<()> {
        if !(0.0..=1.0).contains(&self.config.opacity) {
            return Err(AccessError::SignLanguageFailed(
                "Opacity must be between 0.0 and 1.0".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_creation() {
        let overlay = SignLanguageOverlay::default();
        assert!((overlay.config().opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_validation() {
        let mut overlay = SignLanguageOverlay::default();
        assert!(overlay.validate().is_ok());

        overlay.config.opacity = 1.5;
        assert!(overlay.validate().is_err());
    }
}
