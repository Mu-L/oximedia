//! Noise analysis and profiling module.

pub mod classify;
pub mod profile;
pub mod snr;

pub use classify::{classify_noise, NoiseType};
pub use profile::{NoiseProfile, NoiseProfiler};
pub use snr::{compute_snr_db, signal_to_noise_ratio};
