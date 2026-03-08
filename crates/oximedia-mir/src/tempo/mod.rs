//! Tempo detection and BPM estimation.

pub mod detect;
pub mod estimate;

pub use detect::TempoDetector;
pub use estimate::BpmEstimator;
