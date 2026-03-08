//! Saliency detection and attention prediction.

pub mod attention;
pub mod detect;

pub use attention::{AttentionMap, AttentionPredictor};
pub use detect::{SaliencyDetector, SaliencyMap};
