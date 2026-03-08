//! In-camera VFX compositor
//!
//! Real-time compositor for blending foreground elements, virtual backgrounds,
//! and LED wall content with depth-based compositing.

use super::{
    background::BackgroundRenderer, depth::DepthProcessor, foreground::ForegroundProcessor,
    BlendMode, CompositeLayer,
};
use crate::{Result, VirtualProductionError};
use serde::{Deserialize, Serialize};

/// Compositor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositorConfig {
    /// Output resolution (width, height)
    pub resolution: (usize, usize),
    /// Enable depth compositing
    pub depth_compositing: bool,
    /// Enable motion blur
    pub motion_blur: bool,
    /// Quality level (0.0 - 1.0)
    pub quality: f32,
}

impl Default for CompositorConfig {
    fn default() -> Self {
        Self {
            resolution: (1920, 1080),
            depth_compositing: true,
            motion_blur: false,
            quality: 1.0,
        }
    }
}

/// Composite frame data
#[derive(Debug, Clone)]
pub struct CompositeFrame {
    /// RGB pixel data
    pub pixels: Vec<u8>,
    /// Width in pixels
    pub width: usize,
    /// Height in pixels
    pub height: usize,
    /// Frame timestamp in nanoseconds
    pub timestamp_ns: u64,
}

impl CompositeFrame {
    /// Create new composite frame
    #[must_use]
    pub fn new(width: usize, height: usize, timestamp_ns: u64) -> Self {
        Self {
            pixels: vec![0; width * height * 3],
            width,
            height,
            timestamp_ns,
        }
    }

    /// Get pixel at position
    #[must_use]
    pub fn get_pixel(&self, x: usize, y: usize) -> Option<[u8; 3]> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let idx = (y * self.width + x) * 3;
        Some([self.pixels[idx], self.pixels[idx + 1], self.pixels[idx + 2]])
    }

    /// Set pixel at position
    pub fn set_pixel(&mut self, x: usize, y: usize, rgb: [u8; 3]) {
        if x >= self.width || y >= self.height {
            return;
        }

        let idx = (y * self.width + x) * 3;
        self.pixels[idx] = rgb[0];
        self.pixels[idx + 1] = rgb[1];
        self.pixels[idx + 2] = rgb[2];
    }
}

/// ICVFX compositor
pub struct IcvfxCompositor {
    config: CompositorConfig,
    #[allow(dead_code)]
    foreground_processor: ForegroundProcessor,
    #[allow(dead_code)]
    background_renderer: BackgroundRenderer,
    depth_processor: Option<DepthProcessor>,
    layers: Vec<CompositeLayer>,
}

impl IcvfxCompositor {
    /// Create new compositor
    pub fn new(config: CompositorConfig) -> Result<Self> {
        let foreground_processor = ForegroundProcessor::new()?;
        let background_renderer = BackgroundRenderer::new()?;
        let depth_processor = if config.depth_compositing {
            Some(DepthProcessor::new()?)
        } else {
            None
        };

        Ok(Self {
            config,
            foreground_processor,
            background_renderer,
            depth_processor,
            layers: Vec::new(),
        })
    }

    /// Add composite layer
    pub fn add_layer(&mut self, layer: CompositeLayer) {
        self.layers.push(layer);
    }

    /// Composite frame
    pub fn composite(
        &mut self,
        foreground: &[u8],
        background: &[u8],
        depth: Option<&[f32]>,
        timestamp_ns: u64,
    ) -> Result<CompositeFrame> {
        let (width, height) = self.config.resolution;

        // Validate input sizes
        let expected_size = width * height * 3;
        if foreground.len() != expected_size || background.len() != expected_size {
            return Err(VirtualProductionError::Compositing(format!(
                "Invalid input size: expected {}, got foreground: {}, background: {}",
                expected_size,
                foreground.len(),
                background.len()
            )));
        }

        let mut output = CompositeFrame::new(width, height, timestamp_ns);

        // Process depth if available
        let depth_map =
            if let (Some(depth_data), Some(processor)) = (depth, &mut self.depth_processor) {
                Some(processor.process(depth_data, width, height)?)
            } else {
                None
            };

        // Composite pixel by pixel
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 3;

                // Get foreground and background pixels
                let fg = [
                    f32::from(foreground[idx]) / 255.0,
                    f32::from(foreground[idx + 1]) / 255.0,
                    f32::from(foreground[idx + 2]) / 255.0,
                ];

                let bg = [
                    f32::from(background[idx]) / 255.0,
                    f32::from(background[idx + 1]) / 255.0,
                    f32::from(background[idx + 2]) / 255.0,
                ];

                // Determine blend alpha from depth
                let alpha = if let Some(ref depth_map) = depth_map {
                    depth_map[y * width + x]
                } else {
                    0.5 // Default blend
                };

                // Blend foreground and background
                let blended = BlendMode::Normal.blend(bg, fg, alpha);

                // Convert back to u8
                let result = [
                    (blended[0] * 255.0) as u8,
                    (blended[1] * 255.0) as u8,
                    (blended[2] * 255.0) as u8,
                ];

                output.set_pixel(x, y, result);
            }
        }

        Ok(output)
    }

    /// Get configuration
    #[must_use]
    pub fn config(&self) -> &CompositorConfig {
        &self.config
    }

    /// Get number of layers
    #[must_use]
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Clear all layers
    pub fn clear_layers(&mut self) {
        self.layers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composite_frame() {
        let frame = CompositeFrame::new(100, 100, 0);
        assert_eq!(frame.width, 100);
        assert_eq!(frame.height, 100);
        assert_eq!(frame.pixels.len(), 100 * 100 * 3);
    }

    #[test]
    fn test_composite_frame_pixel() {
        let mut frame = CompositeFrame::new(100, 100, 0);
        frame.set_pixel(50, 50, [255, 128, 64]);

        let pixel = frame.get_pixel(50, 50);
        assert_eq!(pixel, Some([255, 128, 64]));
    }

    #[test]
    fn test_compositor_creation() {
        let config = CompositorConfig::default();
        let compositor = IcvfxCompositor::new(config);
        assert!(compositor.is_ok());
    }

    #[test]
    fn test_compositor_layers() {
        let config = CompositorConfig::default();
        let mut compositor = IcvfxCompositor::new(config).expect("should succeed in test");

        compositor.add_layer(CompositeLayer::new("Layer 1".to_string()));
        compositor.add_layer(CompositeLayer::new("Layer 2".to_string()));

        assert_eq!(compositor.layer_count(), 2);

        compositor.clear_layers();
        assert_eq!(compositor.layer_count(), 0);
    }

    #[test]
    fn test_composite_simple() {
        let config = CompositorConfig {
            resolution: (10, 10),
            depth_compositing: false,
            motion_blur: false,
            quality: 1.0,
        };

        let mut compositor = IcvfxCompositor::new(config).expect("should succeed in test");

        let foreground = vec![255u8; 10 * 10 * 3];
        let background = vec![0u8; 10 * 10 * 3];

        let result = compositor.composite(&foreground, &background, None, 0);
        assert!(result.is_ok());
    }
}
