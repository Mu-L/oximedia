//! FCPXML (Final Cut Pro XML) format stub.

use super::{Edl, EdlError, EdlResult};

/// Parse an FCPXML string (stub — format not yet implemented).
pub fn parse(_content: &str) -> EdlResult<Edl> {
    Err(EdlError::UnsupportedFeature(
        "FCPXML parsing is not yet implemented".to_string(),
    ))
}

/// Write an EDL as FCPXML (stub — format not yet implemented).
pub fn write(_edl: &Edl) -> EdlResult<String> {
    Err(EdlError::UnsupportedFeature(
        "FCPXML writing is not yet implemented".to_string(),
    ))
}
