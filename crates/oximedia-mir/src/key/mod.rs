//! Key detection using Krumhansl-Schmuckler algorithm.

pub mod detect;
pub mod profile;

pub use detect::KeyDetector;
pub use profile::{KeyProfile, KEY_PROFILES};
