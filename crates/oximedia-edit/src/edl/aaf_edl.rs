//! AAF-style EDL format stub.

use super::{Edl, EdlError, EdlResult};

/// Parse an AAF-style EDL (stub — format not yet implemented).
pub fn parse(_content: &str) -> EdlResult<Edl> {
    Err(EdlError::UnsupportedFeature(
        "AAF-EDL parsing is not yet implemented".to_string(),
    ))
}

/// Write an EDL in AAF-style format (stub — format not yet implemented).
pub fn write(_edl: &Edl) -> EdlResult<String> {
    Err(EdlError::UnsupportedFeature(
        "AAF-EDL writing is not yet implemented".to_string(),
    ))
}
