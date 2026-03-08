//! Reverb effects module.
//!
//! Provides multiple reverb algorithms for different applications:
//!
//! - **Freeverb** - Classic algorithmic reverb, efficient and versatile
//! - **Plate Reverb** - Smooth, dense plate reverb simulation
//! - **Convolution Reverb** - Realistic spaces using impulse responses

pub mod convolution;
pub mod freeverb;
pub mod plate;

// Re-exports
pub use convolution::ConvolutionReverb;
pub use freeverb::Freeverb;
pub use plate::PlateReverb;
