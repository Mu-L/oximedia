//! Media source abstractions for reading from various inputs.
//!
//! This module provides the [`MediaSource`] trait and implementations for
//! reading media data from files, memory buffers, and other sources.

mod file;
mod memory;
mod traits;

pub use file::FileSource;
pub use memory::MemorySource;
pub use traits::MediaSource;
