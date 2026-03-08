//! Chromatic adaptation transforms.
//!
//! This module provides tools for adapting colors to different illuminants.

pub mod adapt;

pub use adapt::{ChromaticAdaptation, ChromaticAdaptationMethod};
