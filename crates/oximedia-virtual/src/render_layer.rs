#![allow(dead_code)]
//! Render layer management for virtual production compositing.
//!
//! Provides `LayerType`, `RenderLayer`, and `LayerStack` for ordering
//! visual elements in a virtual production pipeline.

/// Category of a compositing layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerType {
    /// Furthest-back environment plate or generated background.
    Background,
    /// Subject or prop placed in front of the background.
    Foreground,
    /// Graphic or HUD element placed on top of everything else.
    Overlay,
    /// Alpha or luminance matte used to cut out areas.
    Matte,
}

impl LayerType {
    /// Returns `true` if this layer type typically contains transparency
    /// (i.e., it is not expected to fill the full frame).
    #[must_use]
    pub fn is_transparent(&self) -> bool {
        matches!(self, Self::Overlay | Self::Matte)
    }
}

/// A single compositing layer with a z-order depth value.
#[derive(Debug, Clone)]
pub struct RenderLayer {
    /// Human-readable name for the layer.
    pub name: String,
    /// Layer category.
    pub layer_type: LayerType,
    /// Z-order depth; lower values render first (behind higher values).
    depth: i32,
    /// Opacity in \[0.0, 1.0\].
    pub opacity: f32,
}

impl RenderLayer {
    /// Create a new `RenderLayer`.
    #[must_use]
    pub fn new(name: impl Into<String>, layer_type: LayerType, depth: i32) -> Self {
        Self {
            name: name.into(),
            layer_type,
            depth,
            opacity: 1.0,
        }
    }

    /// Returns the z-order depth of this layer.
    #[must_use]
    pub fn z_order(&self) -> i32 {
        self.depth
    }

    /// Set the opacity.
    #[must_use]
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
}

/// An ordered stack of `RenderLayer` values.
#[derive(Debug, Default)]
pub struct LayerStack {
    layers: Vec<RenderLayer>,
}

impl LayerStack {
    /// Create an empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a layer onto the stack and re-sort by ascending z-order.
    pub fn push(&mut self, layer: RenderLayer) {
        self.layers.push(layer);
        self.layers.sort_by_key(RenderLayer::z_order);
    }

    /// Remove the topmost layer (highest z-order) and return it.
    pub fn pop(&mut self) -> Option<RenderLayer> {
        self.layers.pop()
    }

    /// Return the first layer with the given `LayerType`, if any.
    #[must_use]
    pub fn find_by_type(&self, layer_type: LayerType) -> Option<&RenderLayer> {
        self.layers.iter().find(|l| l.layer_type == layer_type)
    }

    /// Number of layers in the stack.
    #[must_use]
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Returns `true` if the stack is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Return all layers ordered by ascending z-order.
    #[must_use]
    pub fn layers(&self) -> &[RenderLayer] {
        &self.layers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_background_not_transparent() {
        assert!(!LayerType::Background.is_transparent());
    }

    #[test]
    fn test_foreground_not_transparent() {
        assert!(!LayerType::Foreground.is_transparent());
    }

    #[test]
    fn test_overlay_is_transparent() {
        assert!(LayerType::Overlay.is_transparent());
    }

    #[test]
    fn test_matte_is_transparent() {
        assert!(LayerType::Matte.is_transparent());
    }

    #[test]
    fn test_render_layer_z_order() {
        let layer = RenderLayer::new("bg", LayerType::Background, 0);
        assert_eq!(layer.z_order(), 0);
        let layer2 = RenderLayer::new("fg", LayerType::Foreground, 10);
        assert_eq!(layer2.z_order(), 10);
    }

    #[test]
    fn test_render_layer_opacity_clamped() {
        let layer = RenderLayer::new("hud", LayerType::Overlay, 100).with_opacity(1.5);
        assert!((layer.opacity - 1.0).abs() < 1e-6);
        let layer2 = RenderLayer::new("hud2", LayerType::Overlay, 100).with_opacity(-0.5);
        assert!((layer2.opacity).abs() < 1e-6);
    }

    #[test]
    fn test_stack_push_orders_by_z() {
        let mut stack = LayerStack::new();
        stack.push(RenderLayer::new("overlay", LayerType::Overlay, 50));
        stack.push(RenderLayer::new("bg", LayerType::Background, 0));
        stack.push(RenderLayer::new("fg", LayerType::Foreground, 20));
        let orders: Vec<i32> = stack
            .layers()
            .iter()
            .map(super::RenderLayer::z_order)
            .collect();
        assert_eq!(orders, vec![0, 20, 50]);
    }

    #[test]
    fn test_stack_pop_returns_topmost() {
        let mut stack = LayerStack::new();
        stack.push(RenderLayer::new("bg", LayerType::Background, 0));
        stack.push(RenderLayer::new("overlay", LayerType::Overlay, 99));
        let popped = stack.pop().expect("should succeed in test");
        assert_eq!(popped.z_order(), 99);
    }

    #[test]
    fn test_stack_find_by_type() {
        let mut stack = LayerStack::new();
        stack.push(RenderLayer::new("bg", LayerType::Background, 0));
        stack.push(RenderLayer::new("matte", LayerType::Matte, 5));
        let found = stack.find_by_type(LayerType::Matte);
        assert!(found.is_some());
        assert_eq!(found.expect("should succeed in test").name, "matte");
    }

    #[test]
    fn test_stack_find_missing_type() {
        let mut stack = LayerStack::new();
        stack.push(RenderLayer::new("bg", LayerType::Background, 0));
        assert!(stack.find_by_type(LayerType::Overlay).is_none());
    }

    #[test]
    fn test_stack_len_and_is_empty() {
        let mut stack = LayerStack::new();
        assert!(stack.is_empty());
        stack.push(RenderLayer::new("bg", LayerType::Background, 0));
        assert_eq!(stack.len(), 1);
        assert!(!stack.is_empty());
    }

    #[test]
    fn test_stack_pop_empty() {
        let mut stack = LayerStack::new();
        assert!(stack.pop().is_none());
    }
}
