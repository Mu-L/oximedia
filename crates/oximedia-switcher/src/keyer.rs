//! Upstream and downstream keyer implementation for video switchers.
//!
//! Keyers allow compositing multiple video layers with transparency.

use crate::chroma::{ChromaKey, ChromaKeyParams};
use crate::luma::{LumaKey, LumaKeyParams};
use oximedia_codec::VideoFrame;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur with keyer operations.
#[derive(Error, Debug, Clone)]
pub enum KeyerError {
    #[error("Invalid keyer ID: {0}")]
    InvalidKeyerId(usize),

    #[error("Keyer {0} not found")]
    KeyerNotFound(usize),

    #[error("Invalid fill source: {0}")]
    InvalidFillSource(usize),

    #[error("Invalid key source: {0}")]
    InvalidKeySource(usize),

    #[error("Processing error: {0}")]
    ProcessingError(String),
}

/// Type of keyer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum KeyerType {
    /// Luma key (brightness-based)
    Luma,
    /// Chroma key (color-based)
    Chroma,
    /// Linear key (uses external matte)
    Linear,
    /// Pattern key (uses pattern generator)
    Pattern,
}

/// Key source configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeySource {
    /// Fill source (foreground video)
    pub fill: usize,
    /// Key source (matte/alpha channel)
    pub key: Option<usize>,
}

impl KeySource {
    /// Create a new key source.
    pub fn new(fill: usize) -> Self {
        Self { fill, key: None }
    }

    /// Create with both fill and key.
    pub fn with_key(fill: usize, key: usize) -> Self {
        Self {
            fill,
            key: Some(key),
        }
    }

    /// Set the fill source.
    pub fn set_fill(&mut self, fill: usize) {
        self.fill = fill;
    }

    /// Set the key source.
    pub fn set_key(&mut self, key: Option<usize>) {
        self.key = key;
    }
}

/// Keyer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyerConfig {
    /// Keyer ID
    pub id: usize,
    /// Keyer type
    pub keyer_type: KeyerType,
    /// Source configuration
    pub source: KeySource,
    /// Enabled state
    pub enabled: bool,
    /// On-air state
    pub on_air: bool,
    /// Tie (enables fill and key together)
    pub tie: bool,
    /// Pre-multiplied alpha
    pub pre_multiplied: bool,
}

impl KeyerConfig {
    /// Create a new keyer configuration.
    pub fn new(id: usize, keyer_type: KeyerType, fill: usize) -> Self {
        Self {
            id,
            keyer_type,
            source: KeySource::new(fill),
            enabled: true,
            on_air: false,
            tie: true,
            pre_multiplied: false,
        }
    }
}

/// Upstream keyer (USK) - part of the M/E row, affected by transitions.
pub struct UpstreamKeyer {
    config: KeyerConfig,
    luma_key: LumaKey,
    chroma_key: ChromaKey,
}

impl UpstreamKeyer {
    /// Create a new upstream keyer.
    pub fn new(id: usize, keyer_type: KeyerType, fill: usize) -> Self {
        Self {
            config: KeyerConfig::new(id, keyer_type, fill),
            luma_key: LumaKey::new(),
            chroma_key: ChromaKey::new_green(),
        }
    }

    /// Get the configuration.
    pub fn config(&self) -> &KeyerConfig {
        &self.config
    }

    /// Get mutable configuration.
    pub fn config_mut(&mut self) -> &mut KeyerConfig {
        &mut self.config
    }

    /// Set the keyer type.
    pub fn set_type(&mut self, keyer_type: KeyerType) {
        self.config.keyer_type = keyer_type;
    }

    /// Get the keyer type.
    pub fn keyer_type(&self) -> KeyerType {
        self.config.keyer_type
    }

    /// Enable or disable the keyer.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// Check if the keyer is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Set on-air state.
    pub fn set_on_air(&mut self, on_air: bool) {
        self.config.on_air = on_air;
    }

    /// Check if the keyer is on-air.
    pub fn is_on_air(&self) -> bool {
        self.config.on_air
    }

    /// Get the luma key processor.
    pub fn luma_key(&self) -> &LumaKey {
        &self.luma_key
    }

    /// Get mutable luma key processor.
    pub fn luma_key_mut(&mut self) -> &mut LumaKey {
        &mut self.luma_key
    }

    /// Get the chroma key processor.
    pub fn chroma_key(&self) -> &ChromaKey {
        &self.chroma_key
    }

    /// Get mutable chroma key processor.
    pub fn chroma_key_mut(&mut self) -> &mut ChromaKey {
        &mut self.chroma_key
    }

    /// Set luma key parameters.
    pub fn set_luma_params(&mut self, params: LumaKeyParams) {
        self.luma_key.set_params(params);
    }

    /// Set chroma key parameters.
    pub fn set_chroma_params(&mut self, params: ChromaKeyParams) {
        self.chroma_key.set_params(params);
    }

    /// Process video through the keyer.
    #[allow(dead_code)]
    pub fn process(
        &self,
        _fill: &VideoFrame,
        _key: Option<&VideoFrame>,
    ) -> Result<VideoFrame, KeyerError> {
        // In a real implementation, this would:
        // 1. Apply the appropriate key type (luma, chroma, linear, pattern)
        // 2. Generate alpha channel
        // 3. Composite fill over background
        // 4. Return processed frame

        // Placeholder
        Err(KeyerError::ProcessingError("Not implemented".to_string()))
    }
}

/// Downstream keyer (DSK) - applied after M/E processing, not affected by transitions.
pub struct DownstreamKeyer {
    config: KeyerConfig,
    #[allow(dead_code)]
    luma_key: LumaKey,
    /// Clip level (0.0 - 1.0)
    clip: f32,
    /// Gain (0.0 - 2.0)
    gain: f32,
    /// Invert key
    invert: bool,
}

impl DownstreamKeyer {
    /// Create a new downstream keyer.
    pub fn new(id: usize, fill: usize, key: usize) -> Self {
        let mut config = KeyerConfig::new(id, KeyerType::Linear, fill);
        config.source.set_key(Some(key));

        Self {
            config,
            luma_key: LumaKey::new(),
            clip: 0.5,
            gain: 1.0,
            invert: false,
        }
    }

    /// Get the configuration.
    pub fn config(&self) -> &KeyerConfig {
        &self.config
    }

    /// Get mutable configuration.
    pub fn config_mut(&mut self) -> &mut KeyerConfig {
        &mut self.config
    }

    /// Set on-air state.
    pub fn set_on_air(&mut self, on_air: bool) {
        self.config.on_air = on_air;
    }

    /// Check if the keyer is on-air.
    pub fn is_on_air(&self) -> bool {
        self.config.on_air
    }

    /// Set tie state.
    pub fn set_tie(&mut self, tie: bool) {
        self.config.tie = tie;
    }

    /// Check if tie is enabled.
    pub fn is_tie(&self) -> bool {
        self.config.tie
    }

    /// Set clip level.
    pub fn set_clip(&mut self, clip: f32) {
        self.clip = clip.clamp(0.0, 1.0);
    }

    /// Get clip level.
    pub fn clip(&self) -> f32 {
        self.clip
    }

    /// Set gain.
    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain.clamp(0.0, 2.0);
    }

    /// Get gain.
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// Set invert state.
    pub fn set_invert(&mut self, invert: bool) {
        self.invert = invert;
    }

    /// Check if invert is enabled.
    pub fn is_invert(&self) -> bool {
        self.invert
    }

    /// Auto-transition DSK on or off.
    pub fn auto_transition(&mut self, duration_frames: u32) -> Result<(), KeyerError> {
        // In a real implementation, this would:
        // 1. Start an automatic transition
        // 2. Fade the DSK on or off over duration_frames
        // 3. Update on_air state when complete

        // For now, just toggle immediately
        self.config.on_air = !self.config.on_air;
        let _ = duration_frames; // Suppress unused warning
        Ok(())
    }

    /// Process video through the DSK.
    #[allow(dead_code)]
    pub fn process(
        &self,
        _program: &VideoFrame,
        _fill: &VideoFrame,
        _key: &VideoFrame,
    ) -> Result<VideoFrame, KeyerError> {
        // In a real implementation, this would:
        // 1. Process the key signal with clip/gain/invert
        // 2. Use key as alpha to composite fill over program
        // 3. Return composited frame

        // Placeholder
        Err(KeyerError::ProcessingError("Not implemented".to_string()))
    }
}

/// Keyer manager for a switcher.
pub struct KeyerManager {
    upstream_keyers: Vec<UpstreamKeyer>,
    downstream_keyers: Vec<DownstreamKeyer>,
}

impl KeyerManager {
    /// Create a new keyer manager.
    pub fn new(num_upstream: usize, num_downstream: usize) -> Self {
        let upstream_keyers = (0..num_upstream)
            .map(|i| UpstreamKeyer::new(i, KeyerType::Luma, 0))
            .collect();

        let downstream_keyers = (0..num_downstream)
            .map(|i| DownstreamKeyer::new(i, 0, 0))
            .collect();

        Self {
            upstream_keyers,
            downstream_keyers,
        }
    }

    /// Get an upstream keyer.
    pub fn get_upstream(&self, id: usize) -> Result<&UpstreamKeyer, KeyerError> {
        self.upstream_keyers
            .get(id)
            .ok_or(KeyerError::KeyerNotFound(id))
    }

    /// Get a mutable upstream keyer.
    pub fn get_upstream_mut(&mut self, id: usize) -> Result<&mut UpstreamKeyer, KeyerError> {
        self.upstream_keyers
            .get_mut(id)
            .ok_or(KeyerError::KeyerNotFound(id))
    }

    /// Get a downstream keyer.
    pub fn get_downstream(&self, id: usize) -> Result<&DownstreamKeyer, KeyerError> {
        self.downstream_keyers
            .get(id)
            .ok_or(KeyerError::KeyerNotFound(id))
    }

    /// Get a mutable downstream keyer.
    pub fn get_downstream_mut(&mut self, id: usize) -> Result<&mut DownstreamKeyer, KeyerError> {
        self.downstream_keyers
            .get_mut(id)
            .ok_or(KeyerError::KeyerNotFound(id))
    }

    /// Get all upstream keyers.
    pub fn upstream_keyers(&self) -> &[UpstreamKeyer] {
        &self.upstream_keyers
    }

    /// Get all downstream keyers.
    pub fn downstream_keyers(&self) -> &[DownstreamKeyer] {
        &self.downstream_keyers
    }

    /// Get the number of upstream keyers.
    pub fn upstream_count(&self) -> usize {
        self.upstream_keyers.len()
    }

    /// Get the number of downstream keyers.
    pub fn downstream_count(&self) -> usize {
        self.downstream_keyers.len()
    }

    /// Get all on-air upstream keyer IDs.
    pub fn on_air_upstream(&self) -> Vec<usize> {
        self.upstream_keyers
            .iter()
            .filter(|k| k.is_on_air())
            .map(|k| k.config().id)
            .collect()
    }

    /// Get all on-air downstream keyer IDs.
    pub fn on_air_downstream(&self) -> Vec<usize> {
        self.downstream_keyers
            .iter()
            .filter(|k| k.is_on_air())
            .map(|k| k.config().id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_source() {
        let source = KeySource::new(1);
        assert_eq!(source.fill, 1);
        assert_eq!(source.key, None);

        let source_with_key = KeySource::with_key(1, 2);
        assert_eq!(source_with_key.fill, 1);
        assert_eq!(source_with_key.key, Some(2));
    }

    #[test]
    fn test_keyer_config() {
        let config = KeyerConfig::new(0, KeyerType::Luma, 1);
        assert_eq!(config.id, 0);
        assert_eq!(config.keyer_type, KeyerType::Luma);
        assert_eq!(config.source.fill, 1);
        assert!(config.enabled);
        assert!(!config.on_air);
        assert!(config.tie);
    }

    #[test]
    fn test_upstream_keyer_creation() {
        let usk = UpstreamKeyer::new(0, KeyerType::Chroma, 1);
        assert_eq!(usk.config().id, 0);
        assert_eq!(usk.keyer_type(), KeyerType::Chroma);
        assert!(usk.is_enabled());
        assert!(!usk.is_on_air());
    }

    #[test]
    fn test_upstream_keyer_on_air() {
        let mut usk = UpstreamKeyer::new(0, KeyerType::Luma, 1);
        assert!(!usk.is_on_air());

        usk.set_on_air(true);
        assert!(usk.is_on_air());

        usk.set_on_air(false);
        assert!(!usk.is_on_air());
    }

    #[test]
    fn test_downstream_keyer_creation() {
        let dsk = DownstreamKeyer::new(0, 1, 2);
        assert_eq!(dsk.config().id, 0);
        assert_eq!(dsk.config().source.fill, 1);
        assert_eq!(dsk.config().source.key, Some(2));
        assert!(!dsk.is_on_air());
    }

    #[test]
    fn test_downstream_keyer_clip_gain() {
        let mut dsk = DownstreamKeyer::new(0, 1, 2);

        assert_eq!(dsk.clip(), 0.5);
        assert_eq!(dsk.gain(), 1.0);

        dsk.set_clip(0.3);
        assert_eq!(dsk.clip(), 0.3);

        dsk.set_gain(1.5);
        assert_eq!(dsk.gain(), 1.5);

        // Test clamping
        dsk.set_clip(1.5);
        assert_eq!(dsk.clip(), 1.0);

        dsk.set_gain(3.0);
        assert_eq!(dsk.gain(), 2.0);
    }

    #[test]
    fn test_downstream_keyer_tie() {
        let mut dsk = DownstreamKeyer::new(0, 1, 2);
        assert!(dsk.is_tie());

        dsk.set_tie(false);
        assert!(!dsk.is_tie());
    }

    #[test]
    fn test_downstream_keyer_invert() {
        let mut dsk = DownstreamKeyer::new(0, 1, 2);
        assert!(!dsk.is_invert());

        dsk.set_invert(true);
        assert!(dsk.is_invert());
    }

    #[test]
    fn test_keyer_manager_creation() {
        let manager = KeyerManager::new(4, 2);
        assert_eq!(manager.upstream_count(), 4);
        assert_eq!(manager.downstream_count(), 2);
    }

    #[test]
    fn test_keyer_manager_get_upstream() {
        let mut manager = KeyerManager::new(4, 2);

        let usk = manager.get_upstream(0).expect("should succeed in test");
        assert_eq!(usk.config().id, 0);

        let usk_mut = manager.get_upstream_mut(1).expect("should succeed in test");
        usk_mut.set_on_air(true);
        assert!(manager
            .get_upstream(1)
            .expect("should succeed in test")
            .is_on_air());

        assert!(manager.get_upstream(10).is_err());
    }

    #[test]
    fn test_keyer_manager_get_downstream() {
        let mut manager = KeyerManager::new(4, 2);

        let dsk = manager.get_downstream(0).expect("should succeed in test");
        assert_eq!(dsk.config().id, 0);

        let dsk_mut = manager
            .get_downstream_mut(1)
            .expect("should succeed in test");
        dsk_mut.set_on_air(true);
        assert!(manager
            .get_downstream(1)
            .expect("should succeed in test")
            .is_on_air());

        assert!(manager.get_downstream(10).is_err());
    }

    #[test]
    fn test_keyer_manager_on_air_lists() {
        let mut manager = KeyerManager::new(4, 2);

        assert_eq!(manager.on_air_upstream().len(), 0);
        assert_eq!(manager.on_air_downstream().len(), 0);

        manager
            .get_upstream_mut(0)
            .expect("should succeed in test")
            .set_on_air(true);
        manager
            .get_upstream_mut(2)
            .expect("should succeed in test")
            .set_on_air(true);
        manager
            .get_downstream_mut(0)
            .expect("should succeed in test")
            .set_on_air(true);

        let on_air_usk = manager.on_air_upstream();
        assert_eq!(on_air_usk.len(), 2);
        assert!(on_air_usk.contains(&0));
        assert!(on_air_usk.contains(&2));

        let on_air_dsk = manager.on_air_downstream();
        assert_eq!(on_air_dsk.len(), 1);
        assert!(on_air_dsk.contains(&0));
    }

    #[test]
    fn test_keyer_type_variants() {
        assert_eq!(KeyerType::Luma, KeyerType::Luma);
        assert_ne!(KeyerType::Luma, KeyerType::Chroma);
        assert!(matches!(KeyerType::Linear, KeyerType::Linear));
        assert!(matches!(KeyerType::Pattern, KeyerType::Pattern));
    }

    #[test]
    fn test_upstream_keyer_type_change() {
        let mut usk = UpstreamKeyer::new(0, KeyerType::Luma, 1);
        assert_eq!(usk.keyer_type(), KeyerType::Luma);

        usk.set_type(KeyerType::Chroma);
        assert_eq!(usk.keyer_type(), KeyerType::Chroma);
    }

    #[test]
    fn test_auto_transition() {
        let mut dsk = DownstreamKeyer::new(0, 1, 2);
        assert!(!dsk.is_on_air());

        dsk.auto_transition(30).expect("should succeed in test");
        assert!(dsk.is_on_air());

        dsk.auto_transition(30).expect("should succeed in test");
        assert!(!dsk.is_on_air());
    }
}
