//! Vocoder effect module.
//!
//! Provides channel vocoder for imposing spectral characteristics
//! of one signal onto another.

pub mod channel;

// Re-exports
pub use channel::{Vocoder, VocoderConfig};
