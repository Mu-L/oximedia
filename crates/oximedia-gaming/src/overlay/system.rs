//! Overlay management system.
//!
//! Provides a compositing overlay system that can render text, FPS counters,
//! performance metrics, and custom graphics onto captured frames in real time.

use crate::capture::screen::CapturedFrame;
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
    /// Layer content type
    pub content: OverlayContent,
}

/// What kind of content an overlay layer renders.
#[derive(Debug, Clone)]
pub enum OverlayContent {
    /// Static text at a position.
    Text(TextOverlay),
    /// FPS counter.
    FpsCounter(FpsCounterOverlay),
    /// Performance metrics panel.
    PerfPanel(PerfPanelOverlay),
    /// Solid colour rectangle (for backgrounds, debug, etc.).
    Rect(RectOverlay),
    /// Custom RGBA pixel data.
    Image(ImageOverlay),
}

/// Text overlay configuration.
#[derive(Debug, Clone)]
pub struct TextOverlay {
    /// Text to display.
    pub text: String,
    /// Position (x, y) from top-left.
    pub position: (u32, u32),
    /// Text colour (RGBA).
    pub color: [u8; 4],
    /// Font size in pixels (each glyph is rendered as size x size block).
    pub font_size: u32,
}

/// FPS counter overlay.
#[derive(Debug, Clone)]
pub struct FpsCounterOverlay {
    /// Position (x, y).
    pub position: (u32, u32),
    /// Colour.
    pub color: [u8; 4],
    /// Current FPS value (updated externally).
    pub current_fps: f32,
    /// Font size in pixels.
    pub font_size: u32,
}

/// Performance panel overlay showing multiple metrics.
#[derive(Debug, Clone)]
pub struct PerfPanelOverlay {
    /// Position (x, y).
    pub position: (u32, u32),
    /// Background colour (RGBA).
    pub bg_color: [u8; 4],
    /// Text colour.
    pub text_color: [u8; 4],
    /// Metric lines to display.
    pub lines: Vec<String>,
    /// Font size.
    pub font_size: u32,
}

/// Solid rectangle overlay.
#[derive(Debug, Clone)]
pub struct RectOverlay {
    /// Position (x, y).
    pub position: (u32, u32),
    /// Size (width, height).
    pub size: (u32, u32),
    /// Colour (RGBA).
    pub color: [u8; 4],
}

/// Custom image overlay (RGBA pixel data).
#[derive(Debug, Clone)]
pub struct ImageOverlay {
    /// Position (x, y).
    pub position: (u32, u32),
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// RGBA pixel data.
    pub data: Vec<u8>,
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

    /// Get a mutable reference to a named layer.
    pub fn get_layer_mut(&mut self, name: &str) -> Option<&mut OverlayLayer> {
        self.layers.iter_mut().find(|l| l.name == name)
    }

    /// Get an immutable reference to a named layer.
    #[must_use]
    pub fn get_layer(&self, name: &str) -> Option<&OverlayLayer> {
        self.layers.iter().find(|l| l.name == name)
    }

    /// Composite all visible layers onto a captured frame (in-place).
    ///
    /// Layers are composited in z-index order (lowest first). Each layer's
    /// opacity is respected via alpha blending.
    pub fn composite_onto(&self, frame: &mut CapturedFrame) {
        for layer in &self.layers {
            if !layer.visible {
                continue;
            }
            match &layer.content {
                OverlayContent::Text(t) => {
                    Self::render_text(frame, t, layer.opacity);
                }
                OverlayContent::FpsCounter(f) => {
                    let text = format!("FPS: {:.0}", f.current_fps);
                    let t = TextOverlay {
                        text,
                        position: f.position,
                        color: f.color,
                        font_size: f.font_size,
                    };
                    Self::render_text(frame, &t, layer.opacity);
                }
                OverlayContent::PerfPanel(p) => {
                    Self::render_perf_panel(frame, p, layer.opacity);
                }
                OverlayContent::Rect(r) => {
                    Self::render_rect(frame, r, layer.opacity);
                }
                OverlayContent::Image(img) => {
                    Self::render_image(frame, img, layer.opacity);
                }
            }
        }
    }

    /// Render text onto a frame using a simple block-glyph approach.
    ///
    /// Each character is rendered as a `font_size x font_size` block of the
    /// specified colour. This is intentionally simple — a real implementation
    /// would use a bitmap font atlas.
    fn render_text(frame: &mut CapturedFrame, overlay: &TextOverlay, opacity: f32) {
        let glyph_w = overlay.font_size.max(1);
        let glyph_h = overlay.font_size.max(1);
        let spacing = glyph_w + 1;

        for (ci, _ch) in overlay.text.chars().enumerate() {
            let gx = overlay.position.0 + (ci as u32) * spacing;
            let gy = overlay.position.1;

            for dy in 0..glyph_h {
                for dx in 0..glyph_w {
                    let px = gx + dx;
                    let py = gy + dy;
                    if px < frame.width && py < frame.height {
                        let idx = ((py * frame.width + px) * 4) as usize;
                        if idx + 3 < frame.data.len() {
                            Self::blend_pixel(
                                &mut frame.data[idx..idx + 4],
                                overlay.color,
                                opacity,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Render a performance panel with background and text lines.
    fn render_perf_panel(frame: &mut CapturedFrame, panel: &PerfPanelOverlay, opacity: f32) {
        let line_height = panel.font_size + 2;
        let panel_height = (panel.lines.len() as u32) * line_height + 4;
        let panel_width = panel
            .lines
            .iter()
            .map(|l| (l.len() as u32) * (panel.font_size + 1))
            .max()
            .unwrap_or(0)
            + 8;

        // Draw background
        let bg = RectOverlay {
            position: panel.position,
            size: (panel_width, panel_height),
            color: panel.bg_color,
        };
        Self::render_rect(frame, &bg, opacity);

        // Draw each text line
        for (i, line) in panel.lines.iter().enumerate() {
            let t = TextOverlay {
                text: line.clone(),
                position: (
                    panel.position.0 + 4,
                    panel.position.1 + 2 + (i as u32) * line_height,
                ),
                color: panel.text_color,
                font_size: panel.font_size,
            };
            Self::render_text(frame, &t, opacity);
        }
    }

    /// Render a solid rectangle.
    fn render_rect(frame: &mut CapturedFrame, rect: &RectOverlay, opacity: f32) {
        for dy in 0..rect.size.1 {
            for dx in 0..rect.size.0 {
                let px = rect.position.0 + dx;
                let py = rect.position.1 + dy;
                if px < frame.width && py < frame.height {
                    let idx = ((py * frame.width + px) * 4) as usize;
                    if idx + 3 < frame.data.len() {
                        Self::blend_pixel(&mut frame.data[idx..idx + 4], rect.color, opacity);
                    }
                }
            }
        }
    }

    /// Render a custom image overlay.
    fn render_image(frame: &mut CapturedFrame, img: &ImageOverlay, opacity: f32) {
        for dy in 0..img.height {
            for dx in 0..img.width {
                let src_idx = ((dy * img.width + dx) * 4) as usize;
                if src_idx + 3 >= img.data.len() {
                    continue;
                }
                let px = img.position.0 + dx;
                let py = img.position.1 + dy;
                if px < frame.width && py < frame.height {
                    let dst_idx = ((py * frame.width + px) * 4) as usize;
                    if dst_idx + 3 < frame.data.len() {
                        let src_color = [
                            img.data[src_idx],
                            img.data[src_idx + 1],
                            img.data[src_idx + 2],
                            img.data[src_idx + 3],
                        ];
                        Self::blend_pixel(
                            &mut frame.data[dst_idx..dst_idx + 4],
                            src_color,
                            opacity,
                        );
                    }
                }
            }
        }
    }

    /// Alpha-blend a source colour onto a destination pixel, respecting layer opacity.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn blend_pixel(dst: &mut [u8], src: [u8; 4], opacity: f32) {
        let sa = (src[3] as f32 / 255.0) * opacity;
        let da = 1.0 - sa;
        dst[0] = ((src[0] as f32 * sa) + (dst[0] as f32 * da)).min(255.0) as u8;
        dst[1] = ((src[1] as f32 * sa) + (dst[1] as f32 * da)).min(255.0) as u8;
        dst[2] = ((src[2] as f32 * sa) + (dst[2] as f32 * da)).min(255.0) as u8;
        dst[3] = 255; // keep alpha opaque
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

    fn make_frame(w: u32, h: u32) -> CapturedFrame {
        CapturedFrame {
            data: vec![0u8; (w as usize) * (h as usize) * 4],
            width: w,
            height: h,
            timestamp: std::time::Duration::ZERO,
            sequence: 0,
        }
    }

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
            content: OverlayContent::Rect(RectOverlay {
                position: (0, 0),
                size: (100, 100),
                color: [255, 0, 0, 128],
            }),
        };
        system.add_layer(layer);
        assert_eq!(system.layer_count(), 1);
    }

    #[test]
    fn test_remove_layer() {
        let mut system = OverlaySystem::new();
        system.add_layer(OverlayLayer {
            name: "a".into(),
            z_index: 1,
            visible: true,
            opacity: 1.0,
            content: OverlayContent::Rect(RectOverlay {
                position: (0, 0),
                size: (1, 1),
                color: [0; 4],
            }),
        });
        system.add_layer(OverlayLayer {
            name: "b".into(),
            z_index: 2,
            visible: true,
            opacity: 1.0,
            content: OverlayContent::Rect(RectOverlay {
                position: (0, 0),
                size: (1, 1),
                color: [0; 4],
            }),
        });
        assert_eq!(system.layer_count(), 2);
        system.remove_layer("a");
        assert_eq!(system.layer_count(), 1);
    }

    #[test]
    fn test_layer_visibility() {
        let mut system = OverlaySystem::new();
        system.add_layer(OverlayLayer {
            name: "test".into(),
            z_index: 0,
            visible: true,
            opacity: 1.0,
            content: OverlayContent::Rect(RectOverlay {
                position: (0, 0),
                size: (1, 1),
                color: [0; 4],
            }),
        });
        system
            .set_layer_visibility("test", false)
            .expect("set visibility");
        let layer = system.get_layer("test").expect("layer exists");
        assert!(!layer.visible);
    }

    #[test]
    fn test_z_order_sorting() {
        let mut system = OverlaySystem::new();
        for z in [30, 10, 20] {
            system.add_layer(OverlayLayer {
                name: format!("z{z}"),
                z_index: z,
                visible: true,
                opacity: 1.0,
                content: OverlayContent::Rect(RectOverlay {
                    position: (0, 0),
                    size: (1, 1),
                    color: [0; 4],
                }),
            });
        }
        assert_eq!(system.get_layer("z10").expect("z10").z_index, 10);
    }

    #[test]
    fn test_composite_rect_onto_frame() {
        let mut system = OverlaySystem::new();
        system.add_layer(OverlayLayer {
            name: "red".into(),
            z_index: 0,
            visible: true,
            opacity: 1.0,
            content: OverlayContent::Rect(RectOverlay {
                position: (0, 0),
                size: (4, 4),
                color: [255, 0, 0, 255],
            }),
        });

        let mut frame = make_frame(8, 8);
        system.composite_onto(&mut frame);

        // Top-left 4x4 should be red
        assert_eq!(frame.data[0], 255); // R
        assert_eq!(frame.data[1], 0); // G
        assert_eq!(frame.data[2], 0); // B
    }

    #[test]
    fn test_composite_text_overlay() {
        let mut system = OverlaySystem::new();
        system.add_layer(OverlayLayer {
            name: "text".into(),
            z_index: 0,
            visible: true,
            opacity: 1.0,
            content: OverlayContent::Text(TextOverlay {
                text: "Hi".into(),
                position: (0, 0),
                color: [0, 255, 0, 255],
                font_size: 2,
            }),
        });

        let mut frame = make_frame(32, 32);
        system.composite_onto(&mut frame);

        // First character glyph should be green
        assert_eq!(frame.data[0], 0);
        assert_eq!(frame.data[1], 255);
    }

    #[test]
    fn test_composite_fps_counter() {
        let mut system = OverlaySystem::new();
        system.add_layer(OverlayLayer {
            name: "fps".into(),
            z_index: 0,
            visible: true,
            opacity: 1.0,
            content: OverlayContent::FpsCounter(FpsCounterOverlay {
                position: (0, 0),
                color: [255, 255, 0, 255],
                current_fps: 60.0,
                font_size: 2,
            }),
        });

        let mut frame = make_frame(64, 32);
        system.composite_onto(&mut frame);

        // Should have rendered something (non-zero pixel)
        let has_overlay = frame.data.chunks(4).any(|p| p[0] > 0 || p[1] > 0);
        assert!(has_overlay);
    }

    #[test]
    fn test_composite_perf_panel() {
        let mut system = OverlaySystem::new();
        system.add_layer(OverlayLayer {
            name: "perf".into(),
            z_index: 0,
            visible: true,
            opacity: 1.0,
            content: OverlayContent::PerfPanel(PerfPanelOverlay {
                position: (0, 0),
                bg_color: [0, 0, 0, 200],
                text_color: [255, 255, 255, 255],
                lines: vec!["FPS: 60".into(), "CPU: 30%".into()],
                font_size: 2,
            }),
        });

        let mut frame = make_frame(64, 32);
        system.composite_onto(&mut frame);

        // Should have modified some pixels
        let modified = frame
            .data
            .chunks(4)
            .any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0);
        assert!(modified);
    }

    #[test]
    fn test_composite_image_overlay() {
        let mut system = OverlaySystem::new();
        // 2x2 magenta image
        let img_data = vec![
            255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255,
        ];
        system.add_layer(OverlayLayer {
            name: "img".into(),
            z_index: 0,
            visible: true,
            opacity: 1.0,
            content: OverlayContent::Image(ImageOverlay {
                position: (0, 0),
                width: 2,
                height: 2,
                data: img_data,
            }),
        });

        let mut frame = make_frame(8, 8);
        system.composite_onto(&mut frame);

        assert_eq!(frame.data[0], 255); // R
        assert_eq!(frame.data[1], 0); // G
        assert_eq!(frame.data[2], 255); // B
    }

    #[test]
    fn test_hidden_layer_not_composited() {
        let mut system = OverlaySystem::new();
        system.add_layer(OverlayLayer {
            name: "hidden".into(),
            z_index: 0,
            visible: false,
            opacity: 1.0,
            content: OverlayContent::Rect(RectOverlay {
                position: (0, 0),
                size: (4, 4),
                color: [255, 0, 0, 255],
            }),
        });

        let mut frame = make_frame(8, 8);
        let original = frame.data.clone();
        system.composite_onto(&mut frame);
        assert_eq!(frame.data, original);
    }

    #[test]
    fn test_half_opacity_blend() {
        let mut system = OverlaySystem::new();
        system.add_layer(OverlayLayer {
            name: "half".into(),
            z_index: 0,
            visible: true,
            opacity: 0.5,
            content: OverlayContent::Rect(RectOverlay {
                position: (0, 0),
                size: (1, 1),
                color: [200, 200, 200, 255],
            }),
        });

        let mut frame = make_frame(4, 4);
        // Set initial pixel to (100, 100, 100, 255)
        frame.data[0] = 100;
        frame.data[1] = 100;
        frame.data[2] = 100;
        frame.data[3] = 255;

        system.composite_onto(&mut frame);

        // Should be blended: (200*0.5 + 100*0.5) = 150
        assert_eq!(frame.data[0], 150);
        assert_eq!(frame.data[1], 150);
        assert_eq!(frame.data[2], 150);
    }

    #[test]
    fn test_get_layer_mut() {
        let mut system = OverlaySystem::new();
        system.add_layer(OverlayLayer {
            name: "mutable".into(),
            z_index: 0,
            visible: true,
            opacity: 1.0,
            content: OverlayContent::Rect(RectOverlay {
                position: (0, 0),
                size: (1, 1),
                color: [0; 4],
            }),
        });
        let layer = system.get_layer_mut("mutable").expect("exists");
        layer.opacity = 0.5;
        assert!((system.get_layer("mutable").expect("exists").opacity - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_overlay_out_of_bounds_safe() {
        let mut system = OverlaySystem::new();
        system.add_layer(OverlayLayer {
            name: "oob".into(),
            z_index: 0,
            visible: true,
            opacity: 1.0,
            content: OverlayContent::Rect(RectOverlay {
                position: (6, 6),
                size: (10, 10),
                color: [255, 255, 255, 255],
            }),
        });

        let mut frame = make_frame(8, 8);
        // Should not panic
        system.composite_onto(&mut frame);
    }

    #[test]
    fn test_multiple_layers_composite_order() {
        let mut system = OverlaySystem::new();
        // Lower z-index = rendered first
        system.add_layer(OverlayLayer {
            name: "bottom".into(),
            z_index: 0,
            visible: true,
            opacity: 1.0,
            content: OverlayContent::Rect(RectOverlay {
                position: (0, 0),
                size: (4, 4),
                color: [255, 0, 0, 255],
            }),
        });
        // Higher z-index = rendered on top
        system.add_layer(OverlayLayer {
            name: "top".into(),
            z_index: 10,
            visible: true,
            opacity: 1.0,
            content: OverlayContent::Rect(RectOverlay {
                position: (0, 0),
                size: (4, 4),
                color: [0, 0, 255, 255],
            }),
        });

        let mut frame = make_frame(8, 8);
        system.composite_onto(&mut frame);

        // Top layer (blue) should be on top
        assert_eq!(frame.data[0], 0);
        assert_eq!(frame.data[2], 255);
    }
}
