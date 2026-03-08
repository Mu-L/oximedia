#![allow(dead_code)]
//! Timecode burn-in overlay for broadcast monitoring outputs.
//!
//! Renders timecode strings (HH:MM:SS:FF) into a simple bitmap that can be
//! composited onto monitoring video feeds.  Supports configurable position,
//! font size, and colour.

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Position on screen where the timecode overlay is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayPosition {
    /// Top-left corner.
    TopLeft,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-right corner.
    BottomRight,
    /// Centre of the frame.
    Center,
}

/// RGBA colour (0-255 per channel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    /// Red component.
    pub r: u8,
    /// Green component.
    pub g: u8,
    /// Blue component.
    pub b: u8,
    /// Alpha component (255 = fully opaque).
    pub a: u8,
}

impl Rgba {
    /// Opaque white.
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    /// Opaque black.
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    /// Semi-transparent black (for background boxes).
    pub const SHADOW: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 160,
    };

    /// Create a new RGBA colour.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// Configuration for the timecode overlay.
#[derive(Debug, Clone)]
pub struct TimecodeOverlayConfig {
    /// Where on the frame to place the overlay.
    pub position: OverlayPosition,
    /// Font size in pixels (height of a character cell).
    pub font_size: u32,
    /// Foreground (text) colour.
    pub fg_color: Rgba,
    /// Background box colour.
    pub bg_color: Rgba,
    /// Margin from the edge of the frame in pixels.
    pub margin: u32,
    /// Whether to draw a background box behind the text.
    pub draw_background: bool,
    /// Frame rate numerator (used for FF field).
    pub fps_num: u32,
    /// Frame rate denominator.
    pub fps_den: u32,
    /// Whether timecode uses drop-frame notation.
    pub drop_frame: bool,
}

impl Default for TimecodeOverlayConfig {
    fn default() -> Self {
        Self {
            position: OverlayPosition::TopLeft,
            font_size: 24,
            fg_color: Rgba::WHITE,
            bg_color: Rgba::SHADOW,
            margin: 16,
            draw_background: true,
            fps_num: 25,
            fps_den: 1,
            drop_frame: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Timecode formatting
// ---------------------------------------------------------------------------

/// A timecode value (hours, minutes, seconds, frames).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timecode {
    /// Hours (0-23).
    pub hours: u8,
    /// Minutes (0-59).
    pub minutes: u8,
    /// Seconds (0-59).
    pub seconds: u8,
    /// Frames (0 .. fps-1).
    pub frames: u8,
    /// Whether this is drop-frame timecode.
    pub drop_frame: bool,
}

impl Timecode {
    /// Create a timecode value.
    pub const fn new(hours: u8, minutes: u8, seconds: u8, frames: u8, drop_frame: bool) -> Self {
        Self {
            hours,
            minutes,
            seconds,
            frames,
            drop_frame,
        }
    }

    /// Format as `HH:MM:SS:FF` (or `HH:MM:SS;FF` for drop-frame).
    pub fn to_string_repr(&self) -> String {
        let sep = if self.drop_frame { ';' } else { ':' };
        format!(
            "{:02}:{:02}:{:02}{}{:02}",
            self.hours, self.minutes, self.seconds, sep, self.frames
        )
    }

    /// Convert a frame count to a timecode given the frames-per-second.
    #[allow(clippy::cast_precision_loss)]
    pub fn from_frame_count(total_frames: u64, fps: u32) -> Self {
        if fps == 0 {
            return Self::new(0, 0, 0, 0, false);
        }
        let fps_u64 = u64::from(fps);
        let total_seconds = total_frames / fps_u64;
        let frames = (total_frames % fps_u64) as u8;
        let hours = (total_seconds / 3600) as u8;
        let minutes = ((total_seconds % 3600) / 60) as u8;
        let seconds = (total_seconds % 60) as u8;
        Self::new(hours, minutes, seconds, frames, false)
    }

    /// Convert this timecode back to a total frame count.
    pub fn to_frame_count(&self, fps: u32) -> u64 {
        let fps_u64 = u64::from(fps);
        let s =
            u64::from(self.hours) * 3600 + u64::from(self.minutes) * 60 + u64::from(self.seconds);
        s * fps_u64 + u64::from(self.frames)
    }
}

// ---------------------------------------------------------------------------
// Overlay Renderer
// ---------------------------------------------------------------------------

/// Rendered overlay bitmap (RGBA pixel buffer).
#[derive(Debug, Clone)]
pub struct OverlayBitmap {
    /// Width of the bitmap in pixels.
    pub width: u32,
    /// Height of the bitmap in pixels.
    pub height: u32,
    /// RGBA pixel data (row-major, 4 bytes per pixel).
    pub pixels: Vec<u8>,
}

/// Renders timecode overlays as RGBA bitmaps.
#[derive(Debug)]
pub struct TimecodeOverlayRenderer {
    config: TimecodeOverlayConfig,
}

impl TimecodeOverlayRenderer {
    /// Create a renderer with the given configuration.
    pub fn new(config: TimecodeOverlayConfig) -> Self {
        Self { config }
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &TimecodeOverlayConfig {
        &self.config
    }

    /// Render a timecode string into a small RGBA bitmap.
    ///
    /// The bitmap is sized to fit the text plus optional background padding.
    pub fn render(&self, tc: &Timecode) -> OverlayBitmap {
        let text = tc.to_string_repr();
        let char_w = self.config.font_size / 2;
        let char_h = self.config.font_size;
        let text_w = char_w * text.len() as u32;
        let pad = if self.config.draw_background { 4 } else { 0 };
        let bmp_w = text_w + pad * 2;
        let bmp_h = char_h + pad * 2;
        let mut pixels = vec![0u8; (bmp_w * bmp_h * 4) as usize];

        // Draw background box if enabled
        if self.config.draw_background {
            let bg = self.config.bg_color;
            for y in 0..bmp_h {
                for x in 0..bmp_w {
                    let off = ((y * bmp_w + x) * 4) as usize;
                    pixels[off] = bg.r;
                    pixels[off + 1] = bg.g;
                    pixels[off + 2] = bg.b;
                    pixels[off + 3] = bg.a;
                }
            }
        }

        // Render each character as a filled rectangle (placeholder rasteriser).
        let fg = self.config.fg_color;
        for (ci, _ch) in text.chars().enumerate() {
            let cx = pad + ci as u32 * char_w;
            let cy = pad;
            // Fill a slightly inset rectangle for visual distinctiveness.
            let inset = 1u32;
            for dy in inset..(char_h.saturating_sub(inset)) {
                for dx in inset..(char_w.saturating_sub(inset)) {
                    let px = cx + dx;
                    let py = cy + dy;
                    if px < bmp_w && py < bmp_h {
                        let off = ((py * bmp_w + px) * 4) as usize;
                        pixels[off] = fg.r;
                        pixels[off + 1] = fg.g;
                        pixels[off + 2] = fg.b;
                        pixels[off + 3] = fg.a;
                    }
                }
            }
        }

        OverlayBitmap {
            width: bmp_w,
            height: bmp_h,
            pixels,
        }
    }

    /// Compute the (x, y) offset for compositing the overlay onto a frame
    /// of the given dimensions.
    pub fn compute_offset(&self, frame_w: u32, frame_h: u32, bmp: &OverlayBitmap) -> (u32, u32) {
        let m = self.config.margin;
        match self.config.position {
            OverlayPosition::TopLeft => (m, m),
            OverlayPosition::TopRight => (frame_w.saturating_sub(bmp.width + m), m),
            OverlayPosition::BottomLeft => (m, frame_h.saturating_sub(bmp.height + m)),
            OverlayPosition::BottomRight => (
                frame_w.saturating_sub(bmp.width + m),
                frame_h.saturating_sub(bmp.height + m),
            ),
            OverlayPosition::Center => (
                (frame_w.saturating_sub(bmp.width)) / 2,
                (frame_h.saturating_sub(bmp.height)) / 2,
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timecode_format_ndf() {
        let tc = Timecode::new(1, 2, 3, 4, false);
        assert_eq!(tc.to_string_repr(), "01:02:03:04");
    }

    #[test]
    fn test_timecode_format_df() {
        let tc = Timecode::new(10, 30, 59, 29, true);
        assert_eq!(tc.to_string_repr(), "10:30:59;29");
    }

    #[test]
    fn test_from_frame_count() {
        // 25 fps: 90_000 frames = 1 hour
        let tc = Timecode::from_frame_count(90_000, 25);
        assert_eq!(tc.hours, 1);
        assert_eq!(tc.minutes, 0);
        assert_eq!(tc.seconds, 0);
        assert_eq!(tc.frames, 0);
    }

    #[test]
    fn test_to_frame_count_roundtrip() {
        let tc = Timecode::new(0, 1, 30, 12, false);
        let count = tc.to_frame_count(25);
        let tc2 = Timecode::from_frame_count(count, 25);
        assert_eq!(tc.hours, tc2.hours);
        assert_eq!(tc.minutes, tc2.minutes);
        assert_eq!(tc.seconds, tc2.seconds);
        assert_eq!(tc.frames, tc2.frames);
    }

    #[test]
    fn test_from_frame_count_zero_fps() {
        let tc = Timecode::from_frame_count(100, 0);
        assert_eq!(tc.hours, 0);
        assert_eq!(tc.frames, 0);
    }

    #[test]
    fn test_overlay_default_config() {
        let cfg = TimecodeOverlayConfig::default();
        assert_eq!(cfg.position, OverlayPosition::TopLeft);
        assert_eq!(cfg.font_size, 24);
        assert!(cfg.draw_background);
    }

    #[test]
    fn test_render_bitmap_dimensions() {
        let r = TimecodeOverlayRenderer::new(TimecodeOverlayConfig::default());
        let tc = Timecode::new(0, 0, 0, 0, false);
        let bmp = r.render(&tc);
        // "00:00:00:00" = 11 chars, char_w=12, pad=4 => 11*12 + 8 = 140
        assert!(bmp.width > 0);
        assert!(bmp.height > 0);
        assert_eq!(bmp.pixels.len(), (bmp.width * bmp.height * 4) as usize);
    }

    #[test]
    fn test_render_no_background() {
        let cfg = TimecodeOverlayConfig {
            draw_background: false,
            ..TimecodeOverlayConfig::default()
        };
        let r = TimecodeOverlayRenderer::new(cfg);
        let tc = Timecode::new(12, 0, 0, 0, false);
        let bmp = r.render(&tc);
        assert!(bmp.width > 0);
    }

    #[test]
    fn test_compute_offset_top_left() {
        let r = TimecodeOverlayRenderer::new(TimecodeOverlayConfig::default());
        let bmp = OverlayBitmap {
            width: 100,
            height: 30,
            pixels: vec![],
        };
        let (x, y) = r.compute_offset(1920, 1080, &bmp);
        assert_eq!(x, 16); // margin
        assert_eq!(y, 16);
    }

    #[test]
    fn test_compute_offset_bottom_right() {
        let cfg = TimecodeOverlayConfig {
            position: OverlayPosition::BottomRight,
            margin: 10,
            ..TimecodeOverlayConfig::default()
        };
        let r = TimecodeOverlayRenderer::new(cfg);
        let bmp = OverlayBitmap {
            width: 100,
            height: 30,
            pixels: vec![],
        };
        let (x, y) = r.compute_offset(1920, 1080, &bmp);
        assert_eq!(x, 1920 - 100 - 10);
        assert_eq!(y, 1080 - 30 - 10);
    }

    #[test]
    fn test_compute_offset_center() {
        let cfg = TimecodeOverlayConfig {
            position: OverlayPosition::Center,
            ..TimecodeOverlayConfig::default()
        };
        let r = TimecodeOverlayRenderer::new(cfg);
        let bmp = OverlayBitmap {
            width: 100,
            height: 30,
            pixels: vec![],
        };
        let (x, y) = r.compute_offset(1920, 1080, &bmp);
        assert_eq!(x, (1920 - 100) / 2);
        assert_eq!(y, (1080 - 30) / 2);
    }

    #[test]
    fn test_rgba_constants() {
        assert_eq!(Rgba::WHITE, Rgba::new(255, 255, 255, 255));
        assert_eq!(Rgba::BLACK, Rgba::new(0, 0, 0, 255));
    }

    #[test]
    fn test_timecode_equality() {
        let a = Timecode::new(1, 2, 3, 4, false);
        let b = Timecode::new(1, 2, 3, 4, false);
        assert_eq!(a, b);
    }
}
