//! Overlay management system.

use crate::GamingResult;

/// Overlay system for managing multiple overlay layers.
pub struct OverlaySystem {
    layers: Vec<OverlayLayer>,
}

/// Overlay layer.
#[derive(Debug, Clone)]
pub struct OverlayLayer {
    /// Layer name
    pub name: String,
    /// Z-index (higher = on top)
    pub z_index: i32,
    /// Visible
    pub visible: bool,
    /// Opacity (0.0 to 1.0)
    pub opacity: f32,
}

impl OverlaySystem {
    /// Create a new overlay system.
    #[must_use]
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Add a layer.
    pub fn add_layer(&mut self, layer: OverlayLayer) {
        self.layers.push(layer);
        self.layers.sort_by_key(|l| l.z_index);
    }

    /// Remove a layer.
    pub fn remove_layer(&mut self, name: &str) {
        self.layers.retain(|l| l.name != name);
    }

    /// Show/hide a layer.
    pub fn set_layer_visibility(&mut self, name: &str, visible: bool) -> GamingResult<()> {
        if let Some(layer) = self.layers.iter_mut().find(|l| l.name == name) {
            layer.visible = visible;
        }
        Ok(())
    }

    /// Get number of layers.
    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }
}

impl Default for OverlaySystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_system_creation() {
        let system = OverlaySystem::new();
        assert_eq!(system.layer_count(), 0);
    }

    #[test]
    fn test_add_layer() {
        let mut system = OverlaySystem::new();
        let layer = OverlayLayer {
            name: "Chat".to_string(),
            z_index: 10,
            visible: true,
            opacity: 1.0,
        };
        system.add_layer(layer);
        assert_eq!(system.layer_count(), 1);
    }
}
