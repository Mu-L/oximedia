//! Text animation effects.

use crate::{EffectParams, Frame, VfxResult, VideoEffect};
use serde::{Deserialize, Serialize};

use super::render::{TextConfig, TextRenderer};

/// Text animation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationType {
    /// Typewriter effect.
    Typewriter,
    /// Fade in.
    FadeIn,
    /// Fade out.
    FadeOut,
    /// Slide in from left.
    SlideInLeft,
    /// Slide in from right.
    SlideInRight,
    /// Scale in.
    ScaleIn,
    /// Bounce in.
    BounceIn,
}

/// Text animation effect.
pub struct TextAnimation {
    animation_type: AnimationType,
    duration: f32,
    renderer: TextRenderer,
    start_time: f64,
}

impl TextAnimation {
    /// Create a new text animation.
    ///
    /// # Errors
    ///
    /// Returns an error if text renderer creation fails.
    pub fn new(animation_type: AnimationType, config: TextConfig) -> VfxResult<Self> {
        Ok(Self {
            animation_type,
            duration: 2.0,
            renderer: TextRenderer::new(config)?,
            start_time: 0.0,
        })
    }

    /// Set animation duration in seconds.
    #[must_use]
    pub const fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration;
        self
    }

    fn get_progress(&self, time: f64) -> f32 {
        let elapsed = (time - self.start_time) as f32;
        (elapsed / self.duration).clamp(0.0, 1.0)
    }

    fn apply_animation(&mut self, progress: f32) {
        match self.animation_type {
            AnimationType::Typewriter => {
                let full_text = self.renderer.config().text.clone();
                let char_count = (full_text.len() as f32 * progress) as usize;
                self.renderer.config_mut().text = full_text.chars().take(char_count).collect();
            }
            AnimationType::FadeIn => {
                let alpha = (progress * 255.0) as u8;
                self.renderer.config_mut().color.a = alpha;
            }
            AnimationType::FadeOut => {
                let alpha = ((1.0 - progress) * 255.0) as u8;
                self.renderer.config_mut().color.a = alpha;
            }
            AnimationType::SlideInLeft => {
                self.renderer.config_mut().x = progress;
            }
            AnimationType::SlideInRight => {
                self.renderer.config_mut().x = 1.0 - progress;
            }
            AnimationType::ScaleIn => {
                self.renderer.config_mut().font_size *= progress.max(0.01);
            }
            AnimationType::BounceIn => {
                let bounce = if progress < 0.5 {
                    2.0 * progress * progress
                } else {
                    1.0 - (-2.0 * progress + 2.0).powi(2) / 2.0
                };
                self.renderer.config_mut().font_size *= bounce.max(0.01);
            }
        }
    }
}

impl VideoEffect for TextAnimation {
    fn name(&self) -> &'static str {
        "Text Animation"
    }

    fn description(&self) -> &'static str {
        "Animated text with various effects"
    }

    fn apply(&mut self, input: &Frame, output: &mut Frame, params: &EffectParams) -> VfxResult<()> {
        let progress = self.get_progress(params.time);
        self.apply_animation(progress);
        self.renderer.apply(input, output, params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_animation() {
        let config = TextConfig::new("Animated").with_font_size(32.0);
        let mut animation = TextAnimation::new(AnimationType::FadeIn, config)
            .expect("should succeed in test")
            .with_duration(1.0);

        let input = Frame::new(200, 100).expect("should succeed in test");
        let mut output = Frame::new(200, 100).expect("should succeed in test");
        let params = EffectParams::new().with_time(0.5);
        animation
            .apply(&input, &mut output, &params)
            .expect("should succeed in test");
    }
}
