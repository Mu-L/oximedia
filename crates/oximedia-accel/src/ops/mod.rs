//! High-level operations built on compute kernels.

pub mod color;
pub mod convolution;
pub mod deinterlace;
pub mod motion;
pub mod scale;

// Re-export the alpha blending API at the ops level for convenience.
pub use color::{alpha_blend, alpha_blend_rgba};
