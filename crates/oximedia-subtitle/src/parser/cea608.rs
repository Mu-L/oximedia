//! CEA-608 and CEA-708 closed caption support.
//!
//! CEA-608 is the legacy analog closed caption standard for NTSC video.
//! CEA-708 is the digital closed caption standard for ATSC/DVB.
//!
//! This implementation provides basic CEA-608 decoding for compatibility.

use crate::style::{Alignment, Color, Position};
use crate::{Subtitle, SubtitleError, SubtitleResult, SubtitleStyle};

/// CEA-608 display mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cea608Mode {
    /// Pop-on mode - captions appear all at once.
    PopOn,
    /// Roll-up mode - captions scroll upward.
    RollUp2,
    /// Roll-up 3 lines.
    RollUp3,
    /// Roll-up 4 lines.
    RollUp4,
    /// Paint-on mode - characters appear as received.
    PaintOn,
}

/// CEA-608 decoder state.
pub struct Cea608Decoder {
    mode: Cea608Mode,
    buffer: String,
    display: String,
    row: u8,
    column: u8,
    style: Cea608Style,
}

/// CEA-608 text style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cea608Style {
    /// White text.
    White,
    /// Green text.
    Green,
    /// Blue text.
    Blue,
    /// Cyan text.
    Cyan,
    /// Red text.
    Red,
    /// Yellow text.
    Yellow,
    /// Magenta text.
    Magenta,
    /// Italic white text.
    Italic,
}

impl Cea608Style {
    /// Get color for this style.
    #[must_use]
    pub const fn color(&self) -> Color {
        match self {
            Self::White | Self::Italic => Color::white(),
            Self::Green => Color::rgb(0, 255, 0),
            Self::Blue => Color::rgb(0, 0, 255),
            Self::Cyan => Color::rgb(0, 255, 255),
            Self::Red => Color::rgb(255, 0, 0),
            Self::Yellow => Color::rgb(255, 255, 0),
            Self::Magenta => Color::rgb(255, 0, 255),
        }
    }
}

impl Default for Cea608Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Cea608Decoder {
    /// Create a new CEA-608 decoder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: Cea608Mode::PopOn,
            buffer: String::new(),
            display: String::new(),
            row: 15,
            column: 0,
            style: Cea608Style::White,
        }
    }

    /// Decode a pair of CEA-608 bytes.
    ///
    /// # Errors
    ///
    /// Returns error if the data is invalid.
    pub fn decode_pair(&mut self, byte1: u8, byte2: u8) -> SubtitleResult<Option<Subtitle>> {
        // Remove parity bits
        let b1 = byte1 & 0x7F;
        let b2 = byte2 & 0x7F;

        // Check for control codes
        if (0x10..=0x1F).contains(&b1) {
            return self.decode_control(b1, b2);
        }

        // Check for special characters
        if b1 == 0x11 || b1 == 0x12 || b1 == 0x13 {
            return self.decode_special(b1, b2);
        }

        // Regular characters
        if (0x20..=0x7F).contains(&b1) {
            self.add_char(b1 as char);
        }
        if (0x20..=0x7F).contains(&b2) {
            self.add_char(b2 as char);
        }

        Ok(None)
    }

    /// Decode control code.
    fn decode_control(&mut self, b1: u8, b2: u8) -> SubtitleResult<Option<Subtitle>> {
        match (b1, b2) {
            // Resume caption loading
            (0x14, 0x20) => {
                self.mode = Cea608Mode::PopOn;
            }
            // Backspace
            (0x14, 0x21) => {
                self.buffer.pop();
            }
            // End of caption
            (0x14, 0x2F) => {
                return Ok(self.end_of_caption());
            }
            // Carriage return
            (0x14, 0x2D) => {
                self.buffer.push('\n');
            }
            // Erase displayed memory
            (0x14, 0x2C) => {
                self.display.clear();
            }
            // Roll-up modes
            (0x14, 0x25) => {
                self.mode = Cea608Mode::RollUp2;
            }
            (0x14, 0x26) => {
                self.mode = Cea608Mode::RollUp3;
            }
            (0x14, 0x27) => {
                self.mode = Cea608Mode::RollUp4;
            }
            _ => {
                // Other control codes (positioning, etc.)
            }
        }

        Ok(None)
    }

    /// Decode special character.
    fn decode_special(&mut self, _b1: u8, _b2: u8) -> SubtitleResult<Option<Subtitle>> {
        // Special characters mapping would go here
        // For now, just ignore them
        Ok(None)
    }

    /// Add a character to the buffer.
    fn add_char(&mut self, c: char) {
        self.buffer.push(c);
        self.column += 1;
    }

    /// End of caption - swap buffers and create subtitle.
    fn end_of_caption(&mut self) -> Option<Subtitle> {
        if self.buffer.is_empty() {
            return None;
        }

        std::mem::swap(&mut self.buffer, &mut self.display);
        self.buffer.clear();

        // Create subtitle (timing would come from external source)
        let mut subtitle = Subtitle::new(0, 3000, self.display.clone());

        // Apply style
        let mut style = SubtitleStyle::default();
        style.primary_color = self.style.color();
        style.position = Position::bottom_center();
        subtitle.style = Some(style);

        Some(subtitle)
    }

    /// Reset decoder state.
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.display.clear();
        self.row = 15;
        self.column = 0;
        self.style = Cea608Style::White;
    }

    /// Get current display text.
    #[must_use]
    pub fn display(&self) -> &str {
        &self.display
    }
}

/// CEA-708 decoder (basic implementation).
pub struct Cea708Decoder {
    current_window: u8,
    windows: [String; 8],
}

impl Default for Cea708Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Cea708Decoder {
    /// Create a new CEA-708 decoder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_window: 0,
            windows: Default::default(),
        }
    }

    /// Decode CEA-708 service block.
    ///
    /// # Errors
    ///
    /// Returns error if the data is invalid.
    pub fn decode_service_block(&mut self, _data: &[u8]) -> SubtitleResult<Option<Vec<Subtitle>>> {
        // Full CEA-708 decoding is complex and would be implemented here
        // This is a placeholder for the basic structure
        Ok(None)
    }

    /// Reset decoder state.
    pub fn reset(&mut self) {
        self.current_window = 0;
        for window in &mut self.windows {
            window.clear();
        }
    }

    /// Get text from a window.
    #[must_use]
    pub fn window_text(&self, window: u8) -> &str {
        self.windows
            .get(window as usize)
            .map(String::as_str)
            .unwrap_or("")
    }
}

/// Extract CEA-608 data from user data.
///
/// # Errors
///
/// Returns error if the data is malformed.
pub fn extract_cea608_from_user_data(user_data: &[u8]) -> SubtitleResult<Vec<(u8, u8)>> {
    let mut pairs = Vec::new();

    // Look for CEA-608 data in ATSC A/53 format
    // This is a simplified implementation
    let mut i = 0;
    while i + 2 < user_data.len() {
        // Check for CEA-608 marker
        if user_data[i] == 0xCC {
            let byte1 = user_data[i + 1];
            let byte2 = user_data[i + 2];
            pairs.push((byte1, byte2));
            i += 3;
        } else {
            i += 1;
        }
    }

    Ok(pairs)
}

/// Extract CEA-708 data from user data.
///
/// # Errors
///
/// Returns error if the data is malformed.
pub fn extract_cea708_from_user_data(user_data: &[u8]) -> SubtitleResult<Vec<u8>> {
    // CEA-708 data extraction would be implemented here
    // This is a placeholder
    Ok(user_data.to_vec())
}
