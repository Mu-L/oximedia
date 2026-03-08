//! Input device capture.

use crate::GamingResult;

/// Input capture for keyboard and mouse.
pub struct InputCapture {
    keyboard_enabled: bool,
    mouse_enabled: bool,
}

/// Input event.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Keyboard key press
    KeyPress(String),
    /// Keyboard key release
    KeyRelease(String),
    /// Mouse button press
    MousePress(u8),
    /// Mouse button release
    MouseRelease(u8),
    /// Mouse movement
    MouseMove {
        /// X coordinate
        x: i32,
        /// Y coordinate
        y: i32,
    },
}

/// Keyboard state.
#[derive(Debug, Clone, Default)]
pub struct KeyboardState {
    /// Pressed keys
    pub pressed_keys: Vec<String>,
}

/// Mouse state.
#[derive(Debug, Clone, Default)]
pub struct MouseState {
    /// Position
    pub position: (i32, i32),
    /// Pressed buttons
    pub pressed_buttons: Vec<u8>,
}

impl InputCapture {
    /// Create a new input capture.
    #[must_use]
    pub fn new() -> Self {
        Self {
            keyboard_enabled: true,
            mouse_enabled: true,
        }
    }

    /// Enable keyboard capture.
    pub fn enable_keyboard(&mut self) {
        self.keyboard_enabled = true;
    }

    /// Disable keyboard capture.
    pub fn disable_keyboard(&mut self) {
        self.keyboard_enabled = false;
    }

    /// Enable mouse capture.
    pub fn enable_mouse(&mut self) {
        self.mouse_enabled = true;
    }

    /// Disable mouse capture.
    pub fn disable_mouse(&mut self) {
        self.mouse_enabled = false;
    }

    /// Poll for input events.
    pub fn poll_events(&self) -> GamingResult<Vec<InputEvent>> {
        Ok(Vec::new())
    }
}

impl Default for InputCapture {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_capture_creation() {
        let capture = InputCapture::new();
        assert!(capture.keyboard_enabled);
        assert!(capture.mouse_enabled);
    }

    #[test]
    fn test_enable_disable() {
        let mut capture = InputCapture::new();
        capture.disable_keyboard();
        assert!(!capture.keyboard_enabled);
        capture.enable_keyboard();
        assert!(capture.keyboard_enabled);
    }
}
