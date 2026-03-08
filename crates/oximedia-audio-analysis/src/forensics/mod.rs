//! Audio forensics module for authenticity verification.

pub mod authenticity;
pub mod compression;
pub mod edit;
pub mod noise;

pub use authenticity::{AuthenticityResult, AuthenticityVerifier};
pub use compression::{detect_compression_history, CompressionHistory};
pub use edit::{EditDetector, EditResult};
pub use noise::{NoiseAnalyzer, NoiseConsistency};
