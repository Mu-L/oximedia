//! Memory allocation utilities for `OxiMedia`.
//!
//! This module provides memory management utilities optimized for
//! multimedia processing:
//!
//! - [`BufferPool`] - Pool of reusable buffers for zero-copy operations

mod buffer_pool;

pub use buffer_pool::BufferPool;
