//! `OxiMedia` I/O Layer
//!
//! This crate provides the I/O foundation for the `OxiMedia` framework:
//!
//! - **Source Module**: Abstractions for reading media from various sources
//!   - [`MediaSource`] - Unified async media source trait
//!   - [`FileSource`] - Local file access via tokio
//!   - [`MemorySource`] - In-memory buffer access
//!
//! - **Bits Module**: Bit-level reading utilities
//!   - [`BitReader`] - Bit-level reader for parsing binary formats
//!   - Exp-Golomb coding support for H.264-style variable-length integers
//!
//! # Example
//!
//! ```no_run
//! use oximedia_io::source::{FileSource, MediaSource};
//! use std::io::SeekFrom;
//!
//! #[tokio::main]
//! async fn main() -> oximedia_core::OxiResult<()> {
//!     let mut source = FileSource::open("video.webm").await?;
//!
//!     let mut buffer = [0u8; 1024];
//!     let bytes_read = source.read(&mut buffer).await?;
//!
//!     println!("Read {} bytes", bytes_read);
//!     Ok(())
//! }
//! ```

pub mod aligned_io;
pub mod async_io;
pub mod bits;
pub mod buffer_pool;
pub mod buffered_io;
pub mod checksum;
pub mod chunked_writer;
pub mod compression;
pub mod copy_engine;
pub mod file_metadata;
pub mod file_watch;
pub mod io_pipeline;
pub mod io_stats;
pub mod mmap;
pub mod progress_reader;
pub mod rate_limiter;
pub mod ring_buffer;
pub mod scatter_gather;
pub mod seekable;
pub mod source;
pub mod splice_pipe;
pub mod temp_files;
pub mod verify_io;
pub mod write_journal;

// Re-export commonly used types
pub use bits::BitReader;
#[cfg(not(target_arch = "wasm32"))]
pub use source::FileSource;
pub use source::{MediaSource, MemorySource};
