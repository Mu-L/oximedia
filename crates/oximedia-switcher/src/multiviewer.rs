//! Multi-viewer layout and rendering for video switchers.
//!
//! Displays multiple video sources in a single composite output for monitoring.

use crate::tally::TallyState;
use oximedia_codec::VideoFrame;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur with multi-viewer operations.
#[derive(Error, Debug, Clone)]
pub enum MultiviewerError {
    #[error("Invalid window ID: {0}")]
    InvalidWindowId(usize),

    #[error("Window {0} not found")]
    WindowNotFound(usize),

    #[error("Invalid layout configuration")]
    InvalidLayout,

    #[error("Rendering error: {0}")]
    RenderingError(String),
}

/// Multi-viewer layout types.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MultiviewerLayout {
    /// 2x2 grid (4 windows)
    Grid2x2,
    /// 3x3 grid (9 windows)
    Grid3x3,
    /// 4x4 grid (16 windows)
    Grid4x4,
    /// Program left, 4 sources right
    ProgramLeft4x1,
    /// Program top, sources below
    ProgramTop,
    /// Custom layout
    Custom,
}

impl MultiviewerLayout {
    /// Get the number of windows for this layout.
    pub fn window_count(&self) -> usize {
        match self {
            MultiviewerLayout::Grid2x2 => 4,
            MultiviewerLayout::Grid3x3 => 9,
            MultiviewerLayout::Grid4x4 => 16,
            MultiviewerLayout::ProgramLeft4x1 => 5,
            MultiviewerLayout::ProgramTop => 5,
            MultiviewerLayout::Custom => 0, // Variable
        }
    }

    /// Get grid dimensions (columns, rows).
    pub fn grid_dimensions(&self) -> (usize, usize) {
        match self {
            MultiviewerLayout::Grid2x2 => (2, 2),
            MultiviewerLayout::Grid3x3 => (3, 3),
            MultiviewerLayout::Grid4x4 => (4, 4),
            MultiviewerLayout::ProgramLeft4x1 => (2, 2),
            MultiviewerLayout::ProgramTop => (2, 3),
            MultiviewerLayout::Custom => (0, 0),
        }
    }
}

/// Position and size of a window in normalized coordinates (0.0 - 1.0).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WindowRect {
    /// X position (0.0 = left)
    pub x: f32,
    /// Y position (0.0 = top)
    pub y: f32,
    /// Width
    pub width: f32,
    /// Height
    pub height: f32,
}

impl WindowRect {
    /// Create a new window rectangle.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0),
            width: width.clamp(0.0, 1.0),
            height: height.clamp(0.0, 1.0),
        }
    }

    /// Full screen rectangle.
    pub fn full_screen() -> Self {
        Self::new(0.0, 0.0, 1.0, 1.0)
    }
}

/// Multi-viewer window configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiviewerWindow {
    /// Window ID
    pub id: usize,
    /// Window label
    pub label: String,
    /// Input source ID
    pub source_id: usize,
    /// Window rectangle
    pub rect: WindowRect,
    /// Show tally border
    pub show_tally: bool,
    /// Show audio meters
    pub show_audio: bool,
    /// Show label overlay
    pub show_label: bool,
}

impl MultiviewerWindow {
    /// Create a new multiviewer window.
    pub fn new(id: usize, source_id: usize, rect: WindowRect) -> Self {
        Self {
            id,
            label: format!("Input {source_id}"),
            source_id,
            rect,
            show_tally: true,
            show_audio: true,
            show_label: true,
        }
    }

    /// Set the label.
    pub fn set_label(&mut self, label: String) {
        self.label = label;
    }

    /// Set the source.
    pub fn set_source(&mut self, source_id: usize) {
        self.source_id = source_id;
    }
}

/// Multi-viewer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiviewerConfig {
    /// Layout type
    pub layout: MultiviewerLayout,
    /// Windows
    pub windows: HashMap<usize, MultiviewerWindow>,
    /// Show borders between windows
    pub show_borders: bool,
    /// Border color (R, G, B)
    pub border_color: (u8, u8, u8),
    /// Border width in pixels
    pub border_width: u32,
}

impl MultiviewerConfig {
    /// Create a new multiviewer configuration.
    pub fn new(layout: MultiviewerLayout) -> Self {
        Self {
            layout,
            windows: HashMap::new(),
            show_borders: true,
            border_color: (64, 64, 64),
            border_width: 2,
        }
    }

    /// Create a 2x2 grid layout.
    pub fn grid_2x2() -> Self {
        let mut config = Self::new(MultiviewerLayout::Grid2x2);

        // Create 2x2 grid windows
        for row in 0..2 {
            for col in 0..2 {
                let id = row * 2 + col;
                let rect = WindowRect::new(col as f32 * 0.5, row as f32 * 0.5, 0.5, 0.5);
                let window = MultiviewerWindow::new(id, id, rect);
                config.windows.insert(id, window);
            }
        }

        config
    }

    /// Create a 4x4 grid layout.
    pub fn grid_4x4() -> Self {
        let mut config = Self::new(MultiviewerLayout::Grid4x4);

        // Create 4x4 grid windows
        for row in 0..4 {
            for col in 0..4 {
                let id = row * 4 + col;
                let rect = WindowRect::new(col as f32 * 0.25, row as f32 * 0.25, 0.25, 0.25);
                let window = MultiviewerWindow::new(id, id, rect);
                config.windows.insert(id, window);
            }
        }

        config
    }

    /// Add a window.
    pub fn add_window(&mut self, window: MultiviewerWindow) {
        self.windows.insert(window.id, window);
    }

    /// Remove a window.
    pub fn remove_window(&mut self, id: usize) {
        self.windows.remove(&id);
    }

    /// Get a window.
    pub fn get_window(&self, id: usize) -> Option<&MultiviewerWindow> {
        self.windows.get(&id)
    }

    /// Get a mutable window.
    pub fn get_window_mut(&mut self, id: usize) -> Option<&mut MultiviewerWindow> {
        self.windows.get_mut(&id)
    }

    /// Get the number of windows.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }
}

/// Multi-viewer renderer.
pub struct Multiviewer {
    config: MultiviewerConfig,
    tally_states: HashMap<usize, TallyState>,
    enabled: bool,
}

impl Multiviewer {
    /// Create a new multiviewer.
    pub fn new(config: MultiviewerConfig) -> Self {
        Self {
            config,
            tally_states: HashMap::new(),
            enabled: true,
        }
    }

    /// Create a 2x2 multiviewer.
    pub fn grid_2x2() -> Self {
        Self::new(MultiviewerConfig::grid_2x2())
    }

    /// Create a 4x4 multiviewer.
    pub fn grid_4x4() -> Self {
        Self::new(MultiviewerConfig::grid_4x4())
    }

    /// Get the configuration.
    pub fn config(&self) -> &MultiviewerConfig {
        &self.config
    }

    /// Get mutable configuration.
    pub fn config_mut(&mut self) -> &mut MultiviewerConfig {
        &mut self.config
    }

    /// Set the configuration.
    pub fn set_config(&mut self, config: MultiviewerConfig) {
        self.config = config;
    }

    /// Enable or disable the multiviewer.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if the multiviewer is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Update tally states.
    pub fn update_tally(&mut self, source_id: usize, state: TallyState) {
        self.tally_states.insert(source_id, state);
    }

    /// Get tally state for a source.
    pub fn get_tally(&self, source_id: usize) -> TallyState {
        self.tally_states
            .get(&source_id)
            .copied()
            .unwrap_or(TallyState::Idle)
    }

    /// Clear all tally states.
    pub fn clear_tally(&mut self) {
        self.tally_states.clear();
    }

    /// Render the multiviewer output.
    #[allow(dead_code)]
    pub fn render(
        &self,
        _inputs: &HashMap<usize, VideoFrame>,
    ) -> Result<VideoFrame, MultiviewerError> {
        // In a real implementation, this would:
        // 1. Create a blank canvas
        // 2. For each window:
        //    a. Get the input frame
        //    b. Scale and position it in the window rect
        //    c. Draw tally borders if enabled
        //    d. Draw labels if enabled
        //    e. Draw audio meters if enabled
        // 3. Draw borders between windows if enabled
        // 4. Return the composited frame

        // Placeholder
        Err(MultiviewerError::RenderingError(
            "Not implemented".to_string(),
        ))
    }

    /// Get tally border color for a state.
    pub fn tally_border_color(&self, state: TallyState) -> (u8, u8, u8) {
        match state {
            TallyState::Idle => (64, 64, 64),            // Gray
            TallyState::Program => (255, 0, 0),          // Red
            TallyState::Preview => (0, 255, 0),          // Green
            TallyState::ProgramPreview => (255, 165, 0), // Orange
        }
    }

    /// Calculate pixel coordinates for a window.
    pub fn window_pixel_rect(
        &self,
        window: &MultiviewerWindow,
        canvas_width: u32,
        canvas_height: u32,
    ) -> (u32, u32, u32, u32) {
        let x = (window.rect.x * canvas_width as f32) as u32;
        let y = (window.rect.y * canvas_height as f32) as u32;
        let width = (window.rect.width * canvas_width as f32) as u32;
        let height = (window.rect.height * canvas_height as f32) as u32;

        (x, y, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_window_count() {
        assert_eq!(MultiviewerLayout::Grid2x2.window_count(), 4);
        assert_eq!(MultiviewerLayout::Grid3x3.window_count(), 9);
        assert_eq!(MultiviewerLayout::Grid4x4.window_count(), 16);
    }

    #[test]
    fn test_layout_grid_dimensions() {
        assert_eq!(MultiviewerLayout::Grid2x2.grid_dimensions(), (2, 2));
        assert_eq!(MultiviewerLayout::Grid3x3.grid_dimensions(), (3, 3));
        assert_eq!(MultiviewerLayout::Grid4x4.grid_dimensions(), (4, 4));
    }

    #[test]
    fn test_window_rect() {
        let rect = WindowRect::new(0.25, 0.5, 0.5, 0.5);
        assert_eq!(rect.x, 0.25);
        assert_eq!(rect.y, 0.5);
        assert_eq!(rect.width, 0.5);
        assert_eq!(rect.height, 0.5);
    }

    #[test]
    fn test_window_rect_clamping() {
        let rect = WindowRect::new(-0.5, 1.5, 2.0, 0.5);
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 1.0);
        assert_eq!(rect.width, 1.0);
        assert_eq!(rect.height, 0.5);
    }

    #[test]
    fn test_window_rect_full_screen() {
        let rect = WindowRect::full_screen();
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.0);
        assert_eq!(rect.width, 1.0);
        assert_eq!(rect.height, 1.0);
    }

    #[test]
    fn test_multiviewer_window() {
        let rect = WindowRect::new(0.0, 0.0, 0.5, 0.5);
        let mut window = MultiviewerWindow::new(0, 1, rect);

        assert_eq!(window.id, 0);
        assert_eq!(window.source_id, 1);
        assert!(window.show_tally);
        assert!(window.show_audio);
        assert!(window.show_label);

        window.set_label("Camera 1".to_string());
        assert_eq!(window.label, "Camera 1");

        window.set_source(2);
        assert_eq!(window.source_id, 2);
    }

    #[test]
    fn test_multiviewer_config_2x2() {
        let config = MultiviewerConfig::grid_2x2();
        assert_eq!(config.layout, MultiviewerLayout::Grid2x2);
        assert_eq!(config.window_count(), 4);
        assert!(config.show_borders);
    }

    #[test]
    fn test_multiviewer_config_4x4() {
        let config = MultiviewerConfig::grid_4x4();
        assert_eq!(config.layout, MultiviewerLayout::Grid4x4);
        assert_eq!(config.window_count(), 16);
    }

    #[test]
    fn test_multiviewer_config_add_remove_window() {
        let mut config = MultiviewerConfig::new(MultiviewerLayout::Custom);
        assert_eq!(config.window_count(), 0);

        let rect = WindowRect::full_screen();
        let window = MultiviewerWindow::new(0, 0, rect);
        config.add_window(window);
        assert_eq!(config.window_count(), 1);

        config.remove_window(0);
        assert_eq!(config.window_count(), 0);
    }

    #[test]
    fn test_multiviewer_creation() {
        let multiviewer = Multiviewer::grid_2x2();
        assert!(multiviewer.is_enabled());
        assert_eq!(multiviewer.config().window_count(), 4);
    }

    #[test]
    fn test_multiviewer_enable_disable() {
        let mut multiviewer = Multiviewer::grid_2x2();
        assert!(multiviewer.is_enabled());

        multiviewer.set_enabled(false);
        assert!(!multiviewer.is_enabled());
    }

    #[test]
    fn test_multiviewer_tally() {
        let mut multiviewer = Multiviewer::grid_2x2();

        assert_eq!(multiviewer.get_tally(1), TallyState::Idle);

        multiviewer.update_tally(1, TallyState::Program);
        assert_eq!(multiviewer.get_tally(1), TallyState::Program);

        multiviewer.update_tally(2, TallyState::Preview);
        assert_eq!(multiviewer.get_tally(2), TallyState::Preview);

        multiviewer.clear_tally();
        assert_eq!(multiviewer.get_tally(1), TallyState::Idle);
    }

    #[test]
    fn test_tally_border_colors() {
        let multiviewer = Multiviewer::grid_2x2();

        assert_eq!(
            multiviewer.tally_border_color(TallyState::Idle),
            (64, 64, 64)
        );
        assert_eq!(
            multiviewer.tally_border_color(TallyState::Program),
            (255, 0, 0)
        );
        assert_eq!(
            multiviewer.tally_border_color(TallyState::Preview),
            (0, 255, 0)
        );
        assert_eq!(
            multiviewer.tally_border_color(TallyState::ProgramPreview),
            (255, 165, 0)
        );
    }

    #[test]
    fn test_window_pixel_rect() {
        let multiviewer = Multiviewer::grid_2x2();
        let rect = WindowRect::new(0.5, 0.5, 0.5, 0.5);
        let window = MultiviewerWindow::new(0, 0, rect);

        let (x, y, width, height) = multiviewer.window_pixel_rect(&window, 1920, 1080);

        assert_eq!(x, 960);
        assert_eq!(y, 540);
        assert_eq!(width, 960);
        assert_eq!(height, 540);
    }

    #[test]
    fn test_get_window() {
        let config = MultiviewerConfig::grid_2x2();
        assert!(config.get_window(0).is_some());
        assert!(config.get_window(3).is_some());
        assert!(config.get_window(4).is_none());
    }

    #[test]
    fn test_get_window_mut() {
        let mut config = MultiviewerConfig::grid_2x2();

        let window = config.get_window_mut(0).expect("should succeed in test");
        window.set_label("Modified".to_string());

        assert_eq!(
            config.get_window(0).expect("should succeed in test").label,
            "Modified"
        );
    }
}
