//! CEA-608 closed caption format (Line 21, NTSC)

use crate::error::{CaptionError, Result};
use crate::formats::{FormatParser, FormatWriter};
use crate::types::CaptionTrack;

/// CEA-608 format parser
pub struct Cea608Parser;

impl FormatParser for Cea608Parser {
    fn parse(&self, _data: &[u8]) -> Result<CaptionTrack> {
        // CEA-608 is a complex binary format with specific encoding
        // Full implementation would require CEA-608 decoder
        Err(CaptionError::FeatureNotEnabled(
            "CEA-608 parsing requires 'cea' feature".to_string(),
        ))
    }
}

/// CEA-608 format writer
pub struct Cea608Writer;

impl FormatWriter for Cea608Writer {
    fn write(&self, _track: &CaptionTrack) -> Result<Vec<u8>> {
        Err(CaptionError::FeatureNotEnabled(
            "CEA-608 writing requires 'cea' feature".to_string(),
        ))
    }
}

/// CEA-608 control codes
#[allow(dead_code)]
pub mod control_codes {
    /// Resume caption loading
    pub const RCL: u16 = 0x9420;
    /// Resume direct captioning
    pub const RDC: u16 = 0x9429;
    /// Erase displayed memory
    pub const EDM: u16 = 0x942C;
    /// Carriage return
    pub const CR: u16 = 0x942D;
    /// Erase non-displayed memory
    pub const ENM: u16 = 0x942E;
    /// End of caption
    pub const EOC: u16 = 0x942F;
}
