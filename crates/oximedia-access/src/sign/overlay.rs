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
    /// This would composite the sign language video onto the main video:
    /// scale the sign-language frame to the configured [`SignSize`],
    /// position it per [`SignPosition`], apply the configured
    /// [`SignBorder`], and alpha-blend it onto `_main_frame` at
    /// `config.opacity`.
    ///
    /// # Honesty note
    ///
    /// This is **not implemented**. An earlier revision returned
    /// `Ok(vec![])` — an empty frame reported as success — regardless of
    /// input, which silently discarded both frames. Beyond not being
    /// implemented, real compositing is not even possible with this
    /// signature today: `apply` receives only raw `&[u8]` byte slices with
    /// no width, height, stride, or pixel-format, so there is no geometry
    /// to scale/position/blend against. No compositor is reusable here
    /// either — `oximedia-graph`'s blend/alignment math
    /// (`filters::video::overlay::{BlendMode, Alignment}`) exists but is a
    /// private module not exported from that crate, and this crate does
    /// not otherwise depend on a pixel compositor. This fails honestly
    /// instead of fabricating an empty "success".
    ///
    /// # Errors
    ///
    /// Always returns [`AccessError::SignLanguageFailed`]: real PiP
    /// compositing is not implemented.
    // TODO(0.2.x): real PiP compositing. Requires extending this API with
    // frame geometry (e.g. accept `width`/`height`/`PixelFormat`, or a
    // `VideoFrame`-like type, instead of raw `&[u8]`) so scale/position/
    // alpha-blend math has something to work with; then either export and
    // reuse oximedia-graph's overlay blend primitives, or implement
    // equivalent scale+alpha-blend logic directly in this crate.
    pub fn apply(&self, _main_frame: &[u8], _sign_frame: &[u8]) -> AccessResult<Vec<u8>> {
        Err(AccessError::SignLanguageFailed(
            "PiP compositing is not implemented (and this API does not yet carry frame \
             width/height/pixel-format, which real scale+position+alpha-blend compositing \
             requires)"
                .to_string(),
        ))
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

    #[test]
    fn test_apply_is_honest_err_not_fabricated_empty_frame() {
        // CHANGED: apply() previously returned `Ok(vec![])` — an empty
        // frame reported as a successful composite — for any input. Real
        // compositing is not implemented (and the current signature lacks
        // the frame geometry a real implementation would need), so it must
        // report that honestly via `Err` instead.
        let overlay = SignLanguageOverlay::default();
        let main_frame = vec![0u8; 64];
        let sign_frame = vec![255u8; 32];

        let result = overlay.apply(&main_frame, &sign_frame);

        assert!(
            result.is_err(),
            "apply() must not fabricate an empty-but-Ok composited frame"
        );
        assert!(matches!(
            result.unwrap_err(),
            AccessError::SignLanguageFailed(_)
        ));
    }
}
