//! Genre classification from audio features.

pub mod classify;
pub mod features;

pub use classify::GenreClassifier;
pub use features::GenreFeatures;
