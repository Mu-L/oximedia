//! Performance heads-up display (HUD) for live game streaming.
//!
//! Collects and renders real-time performance metrics (FPS, frame-time,
//! CPU / GPU utilisation, bitrate, dropped frames) as a lightweight
//! text-based overlay that can be composited onto the output stream.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Metric sample
// ---------------------------------------------------------------------------

/// A single timestamped performance sample.
#[derive(Debug, Clone, Copy)]
pub struct PerfSample {
    /// Frames per second at the moment of capture.
    pub fps: f32,
    /// Frame time in milliseconds.
    pub frame_time_ms: f32,
    /// CPU utilisation as a fraction in `[0.0, 1.0]`.
    pub cpu_usage: f32,
    /// GPU utilisation as a fraction in `[0.0, 1.0]`.
    pub gpu_usage: f32,
    /// Encoding bitrate in kbps.
    pub bitrate_kbps: u32,
    /// Number of frames dropped since the last sample.
    pub dropped_frames: u32,
}

impl Default for PerfSample {
    fn default() -> Self {
        Self {
            fps: 0.0,
            frame_time_ms: 0.0,
            cpu_usage: 0.0,
            gpu_usage: 0.0,
            bitrate_kbps: 0,
            dropped_frames: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// HUD position / colour
// ---------------------------------------------------------------------------

/// Screen corner where the HUD is anchored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudPosition {
    /// Top-left corner.
    TopLeft,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-right corner.
    BottomRight,
}

/// Simple RGBA colour value for the HUD text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HudColor {
    /// Red channel 0-255.
    pub r: u8,
    /// Green channel 0-255.
    pub g: u8,
    /// Blue channel 0-255.
    pub b: u8,
    /// Alpha channel 0-255.
    pub a: u8,
}

impl HudColor {
    /// Create a new opaque colour.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create a colour with custom alpha.
    #[must_use]
    pub const fn with_alpha(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Pre-defined green colour for "good" status.
    #[must_use]
    pub const fn green() -> Self {
        Self::new(0, 255, 0)
    }

    /// Pre-defined yellow colour for "warning" status.
    #[must_use]
    pub const fn yellow() -> Self {
        Self::new(255, 255, 0)
    }

    /// Pre-defined red colour for "critical" status.
    #[must_use]
    pub const fn red() -> Self {
        Self::new(255, 0, 0)
    }
}

// ---------------------------------------------------------------------------
// HUD configuration
// ---------------------------------------------------------------------------

/// Configuration for the performance HUD overlay.
#[derive(Debug, Clone)]
pub struct PerfHudConfig {
    /// Where to place the HUD.
    pub position: HudPosition,
    /// Text colour.
    pub text_color: HudColor,
    /// Background colour (may be semi-transparent).
    pub bg_color: HudColor,
    /// Font size in points.
    pub font_size: u32,
    /// How many historical samples to keep for averaging.
    pub history_size: usize,
    /// Show FPS counter.
    pub show_fps: bool,
    /// Show frame-time graph.
    pub show_frame_time: bool,
    /// Show CPU usage.
    pub show_cpu: bool,
    /// Show GPU usage.
    pub show_gpu: bool,
    /// Show bitrate.
    pub show_bitrate: bool,
    /// Show dropped frames.
    pub show_dropped: bool,
    /// Update interval.
    pub update_interval: Duration,
}

impl Default for PerfHudConfig {
    fn default() -> Self {
        Self {
            position: HudPosition::TopLeft,
            text_color: HudColor::green(),
            bg_color: HudColor::with_alpha(0, 0, 0, 128),
            font_size: 14,
            history_size: 120,
            show_fps: true,
            show_frame_time: true,
            show_cpu: true,
            show_gpu: true,
            show_bitrate: true,
            show_dropped: true,
            update_interval: Duration::from_millis(250),
        }
    }
}

// ---------------------------------------------------------------------------
// PerfHud
// ---------------------------------------------------------------------------

/// The performance HUD collects samples and produces text lines for overlay.
pub struct PerfHud {
    config: PerfHudConfig,
    history: VecDeque<PerfSample>,
}

impl PerfHud {
    /// Create a new HUD with the given configuration.
    #[must_use]
    pub fn new(config: PerfHudConfig) -> Self {
        let cap = config.history_size;
        Self {
            config,
            history: VecDeque::with_capacity(cap),
        }
    }

    /// Record a new performance sample.
    pub fn push_sample(&mut self, sample: PerfSample) {
        if self.history.len() == self.config.history_size {
            self.history.pop_front();
        }
        self.history.push_back(sample);
    }

    /// Number of samples currently stored.
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.history.len()
    }

    /// Average FPS over the stored history.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_fps(&self) -> f32 {
        if self.history.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.history.iter().map(|s| s.fps).sum();
        sum / self.history.len() as f32
    }

    /// Average frame time in milliseconds over the history.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_frame_time_ms(&self) -> f32 {
        if self.history.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.history.iter().map(|s| s.frame_time_ms).sum();
        sum / self.history.len() as f32
    }

    /// Average CPU usage over the history.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_cpu(&self) -> f32 {
        if self.history.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.history.iter().map(|s| s.cpu_usage).sum();
        sum / self.history.len() as f32
    }

    /// Average GPU usage over the history.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_gpu(&self) -> f32 {
        if self.history.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.history.iter().map(|s| s.gpu_usage).sum();
        sum / self.history.len() as f32
    }

    /// Total dropped frames across all stored samples.
    #[must_use]
    pub fn total_dropped(&self) -> u64 {
        self.history
            .iter()
            .map(|s| u64::from(s.dropped_frames))
            .sum()
    }

    /// Determine colour for a given FPS value relative to a target.
    #[must_use]
    pub fn fps_color(&self, fps: f32, target_fps: f32) -> HudColor {
        if fps >= target_fps * 0.95 {
            HudColor::green()
        } else if fps >= target_fps * 0.75 {
            HudColor::yellow()
        } else {
            HudColor::red()
        }
    }

    /// Render the HUD as a vector of display lines.
    #[must_use]
    pub fn render_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.config.show_fps {
            lines.push(format!("FPS: {:.1}", self.avg_fps()));
        }
        if self.config.show_frame_time {
            lines.push(format!("Frame: {:.2} ms", self.avg_frame_time_ms()));
        }
        if self.config.show_cpu {
            lines.push(format!("CPU: {:.0}%", self.avg_cpu() * 100.0));
        }
        if self.config.show_gpu {
            lines.push(format!("GPU: {:.0}%", self.avg_gpu() * 100.0));
        }
        if self.config.show_bitrate {
            if let Some(last) = self.history.back() {
                lines.push(format!("Bitrate: {} kbps", last.bitrate_kbps));
            }
        }
        if self.config.show_dropped {
            lines.push(format!("Dropped: {}", self.total_dropped()));
        }
        lines
    }

    /// Clear all stored samples.
    pub fn clear(&mut self) {
        self.history.clear();
    }

    /// Get a reference to the current configuration.
    #[must_use]
    pub fn config(&self) -> &PerfHudConfig {
        &self.config
    }

    /// The 1st-percentile (worst-case) FPS across the history.
    #[must_use]
    pub fn percentile_1_fps(&self) -> f32 {
        if self.history.is_empty() {
            return 0.0;
        }
        let mut fps_vals: Vec<f32> = self.history.iter().map(|s| s.fps).collect();
        fps_vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        #[allow(clippy::cast_precision_loss)]
        let idx = ((fps_vals.len() as f32) * 0.01).ceil() as usize;
        let idx = idx.max(1).min(fps_vals.len()) - 1;
        fps_vals[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(fps: f32, cpu: f32, gpu: f32, dropped: u32) -> PerfSample {
        PerfSample {
            fps,
            frame_time_ms: if fps > 0.0 { 1000.0 / fps } else { 0.0 },
            cpu_usage: cpu,
            gpu_usage: gpu,
            bitrate_kbps: 6000,
            dropped_frames: dropped,
        }
    }

    #[test]
    fn test_hud_creation_default() {
        let hud = PerfHud::new(PerfHudConfig::default());
        assert_eq!(hud.sample_count(), 0);
    }

    #[test]
    fn test_push_sample() {
        let mut hud = PerfHud::new(PerfHudConfig::default());
        hud.push_sample(PerfSample::default());
        assert_eq!(hud.sample_count(), 1);
    }

    #[test]
    fn test_history_cap_eviction() {
        let cfg = PerfHudConfig {
            history_size: 3,
            ..PerfHudConfig::default()
        };
        let mut hud = PerfHud::new(cfg);
        for i in 0..5 {
            #[allow(clippy::cast_precision_loss)]
            hud.push_sample(sample(60.0 + i as f32, 0.5, 0.5, 0));
        }
        assert_eq!(hud.sample_count(), 3);
    }

    #[test]
    fn test_avg_fps_empty() {
        let hud = PerfHud::new(PerfHudConfig::default());
        assert!((hud.avg_fps() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_avg_fps_single() {
        let mut hud = PerfHud::new(PerfHudConfig::default());
        hud.push_sample(sample(60.0, 0.5, 0.5, 0));
        assert!((hud.avg_fps() - 60.0).abs() < 1e-3);
    }

    #[test]
    fn test_avg_fps_multiple() {
        let mut hud = PerfHud::new(PerfHudConfig::default());
        hud.push_sample(sample(50.0, 0.0, 0.0, 0));
        hud.push_sample(sample(70.0, 0.0, 0.0, 0));
        assert!((hud.avg_fps() - 60.0).abs() < 1e-3);
    }

    #[test]
    fn test_avg_frame_time() {
        let mut hud = PerfHud::new(PerfHudConfig::default());
        hud.push_sample(sample(60.0, 0.0, 0.0, 0));
        // 1000/60 ≈ 16.667
        assert!((hud.avg_frame_time_ms() - 16.6667).abs() < 0.1);
    }

    #[test]
    fn test_avg_cpu_gpu() {
        let mut hud = PerfHud::new(PerfHudConfig::default());
        hud.push_sample(sample(60.0, 0.4, 0.8, 0));
        hud.push_sample(sample(60.0, 0.6, 0.6, 0));
        assert!((hud.avg_cpu() - 0.5).abs() < 1e-5);
        assert!((hud.avg_gpu() - 0.7).abs() < 1e-5);
    }

    #[test]
    fn test_total_dropped() {
        let mut hud = PerfHud::new(PerfHudConfig::default());
        hud.push_sample(sample(60.0, 0.0, 0.0, 5));
        hud.push_sample(sample(60.0, 0.0, 0.0, 3));
        assert_eq!(hud.total_dropped(), 8);
    }

    #[test]
    fn test_fps_color_green() {
        let hud = PerfHud::new(PerfHudConfig::default());
        let c = hud.fps_color(59.0, 60.0);
        assert_eq!(c, HudColor::green());
    }

    #[test]
    fn test_fps_color_yellow() {
        let hud = PerfHud::new(PerfHudConfig::default());
        let c = hud.fps_color(50.0, 60.0);
        assert_eq!(c, HudColor::yellow());
    }

    #[test]
    fn test_fps_color_red() {
        let hud = PerfHud::new(PerfHudConfig::default());
        let c = hud.fps_color(30.0, 60.0);
        assert_eq!(c, HudColor::red());
    }

    #[test]
    fn test_render_lines_non_empty() {
        let mut hud = PerfHud::new(PerfHudConfig::default());
        hud.push_sample(sample(60.0, 0.5, 0.7, 0));
        let lines = hud.render_lines();
        assert!(!lines.is_empty());
        assert!(lines[0].contains("FPS"));
    }

    #[test]
    fn test_clear() {
        let mut hud = PerfHud::new(PerfHudConfig::default());
        hud.push_sample(PerfSample::default());
        hud.clear();
        assert_eq!(hud.sample_count(), 0);
    }

    #[test]
    fn test_percentile_1_fps() {
        let mut hud = PerfHud::new(PerfHudConfig::default());
        for i in 0..100 {
            #[allow(clippy::cast_precision_loss)]
            hud.push_sample(sample(30.0 + i as f32, 0.0, 0.0, 0));
        }
        let p1 = hud.percentile_1_fps();
        // lowest values are 30, 31, … — 1% of 100 = index 0
        assert!(p1 >= 30.0 && p1 <= 32.0);
    }

    #[test]
    fn test_hud_position_variants() {
        let positions = [
            HudPosition::TopLeft,
            HudPosition::TopRight,
            HudPosition::BottomLeft,
            HudPosition::BottomRight,
        ];
        for pos in positions {
            let cfg = PerfHudConfig {
                position: pos,
                ..PerfHudConfig::default()
            };
            let hud = PerfHud::new(cfg);
            assert_eq!(hud.config().position, pos);
        }
    }
}
