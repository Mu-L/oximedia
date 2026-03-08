//! Distortion effects module.
//!
//! Provides various distortion and saturation effects:
//!
//! - **Overdrive** - Soft clipping for warm saturation
//! - **Fuzz** - Hard clipping for aggressive distortion
//! - **Bit Crusher** - Digital degradation (bit depth and sample rate reduction)

pub mod bitcrusher;
pub mod fuzz;
pub mod overdrive;

// Re-exports
pub use bitcrusher::{BitCrusher, BitCrusherConfig};
pub use fuzz::{Fuzz, FuzzConfig};
pub use overdrive::{Overdrive, OverdriveConfig};
