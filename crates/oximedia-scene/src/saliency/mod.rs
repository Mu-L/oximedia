//! Saliency detection and attention prediction.
//!
//! Includes spatial saliency via center-surround differences, temporal saliency
//! for video (motion-weighted attention maps), and an optimized spectral saliency
//! implementation with pre-allocated FFT-like buffers.

pub mod attention;
pub mod detect;
pub mod temporal;

pub use attention::{AttentionMap, AttentionPredictor};
pub use detect::{SaliencyDetector, SaliencyMap};
pub use temporal::{TemporalSaliencyConfig, TemporalSaliencyDetector, TemporalSaliencyMap};
