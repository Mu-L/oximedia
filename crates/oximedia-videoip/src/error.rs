//! Error types for video-over-IP protocol.

use std::io;

/// Result type for video-over-IP operations.
pub type VideoIpResult<T> = Result<T, VideoIpError>;

/// Errors that can occur in video-over-IP operations.
#[derive(Debug, thiserror::Error)]
pub enum VideoIpError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Invalid video configuration.
    #[error("Invalid video configuration: {0}")]
    InvalidVideoConfig(String),

    /// Invalid audio configuration.
    #[error("Invalid audio configuration: {0}")]
    InvalidAudioConfig(String),

    /// Invalid packet format.
    #[error("Invalid packet: {0}")]
    InvalidPacket(String),

    /// Packet too large.
    #[error("Packet too large: {size} bytes (max {max})")]
    PacketTooLarge {
        /// The packet size.
        size: usize,
        /// The maximum allowed size.
        max: usize,
    },

    /// Discovery error.
    #[error("Discovery error: {0}")]
    Discovery(String),

    /// Service not found.
    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    /// Codec error.
    #[error("Codec error: {0}")]
    Codec(String),

    /// FEC error.
    #[error("FEC error: {0}")]
    Fec(String),

    /// Transport error.
    #[error("Transport error: {0}")]
    Transport(String),

    /// Buffer overflow.
    #[error("Buffer overflow")]
    BufferOverflow,

    /// Timeout.
    #[error("Operation timed out")]
    Timeout,

    /// Invalid state.
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Metadata error.
    #[error("Metadata error: {0}")]
    Metadata(String),

    /// PTZ control error.
    #[error("PTZ control error: {0}")]
    Ptz(String),

    /// `OxiMedia` core error.
    #[error("Core error: {0}")]
    Core(#[from] oximedia_core::error::OxiError),
}
