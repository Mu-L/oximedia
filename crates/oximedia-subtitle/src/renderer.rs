//! Main subtitle renderer.

use crate::font::Font;
use crate::overlay::overlay_subtitle;
use crate::style::{Position, SubtitleStyle};
use crate::text::TextLayoutEngine;
use crate::{Subtitle, SubtitleError, SubtitleResult};
use oximedia_codec::VideoFrame;
use oximedia_core::Timestamp;

/// Main subtitle renderer.
pub struct SubtitleRenderer {
    layout_engine: TextLayoutEngine,
    default_style: SubtitleStyle,
}

impl SubtitleRenderer {
    /// Create a new subtitle renderer.
    #[must_use]
    pub fn new(font: Font, style: SubtitleStyle) -> Self {
        Self {
            layout_engine: TextLayoutEngine::new(font),
            default_style: style,
        }
    }

    /// Render a subtitle onto a video frame at the given timestamp.
    ///
    /// # Errors
    ///
    /// Returns error if rendering fails or frame format is unsupported.
    pub fn render_subtitle(
        &mut self,
        subtitle: &Subtitle,
        frame: &mut VideoFrame,
        timestamp: Timestamp,
    ) -> SubtitleResult<()> {
        // Check if subtitle is active
        let timestamp_ms = (timestamp.to_seconds() * 1000.0) as i64;
        if !subtitle.is_active(timestamp_ms) {
            return Ok(());
        }

        // Get effective style
        let style = subtitle.style.as_ref().unwrap_or(&self.default_style);

        // Layout text
        let max_width = if style.max_width > 0 {
            style.max_width
        } else {
            frame
                .width
                .saturating_sub(style.margin_left + style.margin_right)
        };

        let layout = self
            .layout_engine
            .layout(&subtitle.text, style, max_width)?;

        if layout.is_empty() {
            return Ok(());
        }

        // Calculate position
        let position = subtitle.position.as_ref().unwrap_or(&style.position);
        let (x, y) = self.calculate_position(frame, &layout, position, style);

        // Apply animations
        let (color, outline_color) = self.apply_animations(
            subtitle,
            timestamp_ms,
            style.primary_color,
            style.outline.as_ref().map(|o| o.color),
        );

        // Render background box if enabled
        if let Some(bg_color) = style.background_color {
            self.render_background(frame, &layout, x, y, bg_color, style.background_padding)?;
        }

        // Get outline width
        let outline_width = style.outline.as_ref().map(|o| o.width).unwrap_or(0.0);

        // Overlay subtitle onto frame
        overlay_subtitle(frame, &layout, x, y, color, outline_color, outline_width)?;

        Ok(())
    }

    /// Render multiple subtitles (for overlapping subtitles).
    ///
    /// # Errors
    ///
    /// Returns error if any subtitle rendering fails.
    pub fn render_subtitles(
        &mut self,
        subtitles: &[Subtitle],
        frame: &mut VideoFrame,
        timestamp: Timestamp,
    ) -> SubtitleResult<()> {
        let timestamp_ms = (timestamp.to_seconds() * 1000.0) as i64;

        // Filter active subtitles
        let active: Vec<_> = subtitles
            .iter()
            .filter(|s| s.is_active(timestamp_ms))
            .collect();

        // Render each active subtitle
        for subtitle in active {
            self.render_subtitle(subtitle, frame, timestamp)?;
        }

        Ok(())
    }

    /// Calculate absolute position from relative position and alignment.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_precision_loss)]
    fn calculate_position(
        &self,
        frame: &VideoFrame,
        layout: &crate::text::TextLayout,
        position: &Position,
        style: &SubtitleStyle,
    ) -> (i32, i32) {
        let frame_width = frame.width as f32;
        let frame_height = frame.height as f32;

        // Calculate base position from relative coordinates
        let mut x = frame_width * position.x;
        let mut y = frame_height * position.y;

        // Apply alignment
        match position.alignment {
            crate::style::Alignment::Left => {
                x += style.margin_left as f32;
            }
            crate::style::Alignment::Center => {
                x -= layout.width / 2.0;
            }
            crate::style::Alignment::Right => {
                x -= layout.width + style.margin_right as f32;
            }
        }

        // Apply vertical alignment
        match position.vertical_alignment {
            crate::style::VerticalAlignment::Top => {
                y += style.margin_top as f32;
            }
            crate::style::VerticalAlignment::Middle => {
                y -= layout.height / 2.0;
            }
            crate::style::VerticalAlignment::Bottom => {
                y -= layout.height + style.margin_bottom as f32;
            }
        }

        (x as i32, y as i32)
    }

    /// Apply animation effects to colors.
    #[allow(clippy::cast_precision_loss)]
    fn apply_animations(
        &self,
        subtitle: &Subtitle,
        timestamp_ms: i64,
        mut primary_color: crate::style::Color,
        outline_color: Option<crate::style::Color>,
    ) -> (crate::style::Color, Option<crate::style::Color>) {
        use crate::style::Animation;

        let elapsed = timestamp_ms - subtitle.start_time;
        let duration = subtitle.duration();

        for animation in &subtitle.animations {
            match animation {
                Animation::FadeIn(fade_duration) => {
                    if elapsed < *fade_duration {
                        let progress = elapsed as f32 / *fade_duration as f32;
                        let alpha = (f32::from(primary_color.a) * progress) as u8;
                        primary_color = primary_color.with_alpha(alpha);
                    }
                }
                Animation::FadeOut(fade_duration) => {
                    let fade_start = duration - fade_duration;
                    if elapsed > fade_start {
                        let fade_elapsed = elapsed - fade_start;
                        let progress = fade_elapsed as f32 / *fade_duration as f32;
                        let alpha = (f32::from(primary_color.a) * (1.0 - progress)) as u8;
                        primary_color = primary_color.with_alpha(alpha);
                    }
                }
                _ => {
                    // Other animations would be applied to position/scale
                    // Not implemented here for simplicity
                }
            }
        }

        (primary_color, outline_color)
    }

    /// Render background box.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    fn render_background(
        &self,
        frame: &mut VideoFrame,
        layout: &crate::text::TextLayout,
        x: i32,
        y: i32,
        color: crate::style::Color,
        padding: f32,
    ) -> SubtitleResult<()> {
        if frame.planes.is_empty() {
            return Err(SubtitleError::InvalidFrameFormat(
                "Frame has no planes".to_string(),
            ));
        }

        let x1 = (x as f32 - padding).max(0.0) as usize;
        let y1 = (y as f32 - padding).max(0.0) as usize;
        let x2 = ((x as f32 + layout.width + padding) as usize).min(frame.width as usize);
        let y2 = ((y as f32 + layout.height + padding) as usize).min(frame.height as usize);

        let mut plane_data = frame.planes[0].data.to_vec();
        let stride = frame.planes[0].stride;

        let bytes_per_pixel = match frame.format {
            oximedia_core::PixelFormat::Rgb24 => 3,
            oximedia_core::PixelFormat::Rgba32 => 4,
            _ => return Ok(()), // Skip background for other formats
        };

        // Fill rectangle
        for py in y1..y2 {
            for px in x1..x2 {
                let idx = py * stride + px * bytes_per_pixel;
                if idx + bytes_per_pixel <= plane_data.len() {
                    let alpha = f32::from(color.a) / 255.0;
                    let inv_alpha = 1.0 - alpha;

                    plane_data[idx] =
                        (f32::from(color.r) * alpha + f32::from(plane_data[idx]) * inv_alpha) as u8;
                    plane_data[idx + 1] = (f32::from(color.g) * alpha
                        + f32::from(plane_data[idx + 1]) * inv_alpha)
                        as u8;
                    plane_data[idx + 2] = (f32::from(color.b) * alpha
                        + f32::from(plane_data[idx + 2]) * inv_alpha)
                        as u8;
                }
            }
        }

        frame.planes[0].data = plane_data;

        Ok(())
    }

    /// Get the default style.
    #[must_use]
    pub fn style(&self) -> &SubtitleStyle {
        &self.default_style
    }

    /// Set the default style.
    pub fn set_style(&mut self, style: SubtitleStyle) {
        self.default_style = style;
    }
}
