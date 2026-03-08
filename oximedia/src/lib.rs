//! # `OxiMedia`
//!
//! A patent-free multimedia framework for Rust.
//!
//! `OxiMedia` provides safe, async-first media processing with focus on
//! royalty-free codecs (AV1, VP9, Opus, FLAC) and clean room implementation.
//!
//! ## Features
//!
//! - **Patent-Free**: Only supports royalty-free codecs (Green List)
//! - **Memory Safe**: Pure Rust with zero unsafe code
//! - **Async-First**: Built on tokio for high concurrency
//! - **Zero-Copy**: Efficient buffer management throughout
//!
//! ## Example
//!
//! ```ignore
//! use oximedia::prelude::*;
//!
//! // Probe a media file
//! let format = probe_format(&data)?;
//! println!("Detected: {:?}", format);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

// Re-export core types
pub use oximedia_core::{
    CodecId, MediaType, OxiError, OxiResult, PixelFormat, Rational, SampleFormat, Timestamp,
};

// Re-export I/O types
pub use oximedia_io::{BitReader, FileSource, MediaSource, MemorySource};

// Re-export container types
pub use oximedia_container::{
    probe_format, CodecParams, ContainerFormat, Demuxer, Metadata, Packet, PacketFlags,
    ProbeResult, StreamInfo,
};

/// Prelude module for convenient imports.
///
/// Provides convenient re-exports for common usage.
///
/// ```ignore
/// use oximedia::prelude::*;
/// ```
pub mod prelude {

    pub use crate::{
        probe_format, CodecId, ContainerFormat, MediaType, OxiError, OxiResult, Packet,
        PacketFlags, PixelFormat, Rational, SampleFormat, Timestamp,
    };

    pub use oximedia_container::Demuxer;
    pub use oximedia_io::MediaSource;
}
