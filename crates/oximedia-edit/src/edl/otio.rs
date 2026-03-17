//! OpenTimelineIO JSON format stub.

use super::{Edl, EdlError, EdlResult};

/// Parse an OpenTimelineIO JSON string (stub — format not yet implemented).
pub fn parse(_content: &str) -> EdlResult<Edl> {
    Err(EdlError::UnsupportedFeature(
        "OpenTimelineIO parsing is not yet implemented".to_string(),
    ))
}

/// Write an EDL as OpenTimelineIO JSON (stub — format not yet implemented).
pub fn write(_edl: &Edl) -> EdlResult<String> {
    Err(EdlError::UnsupportedFeature(
        "OpenTimelineIO writing is not yet implemented".to_string(),
    ))
}
