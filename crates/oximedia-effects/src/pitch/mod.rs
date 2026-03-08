//! Pitch and time manipulation effects.
//!
//! Provides:
//!
//! - **Pitch Shifter** - Change pitch without affecting tempo
//! - **Time Stretcher** - Change tempo without affecting pitch
//! - **Auto-tune** - Pitch correction to musical scale

pub mod autotune;
pub mod shifter;
pub mod stretch;

// Re-exports
pub use autotune::{AutoTune, AutoTuneConfig, Scale};
pub use shifter::{
    AdvancedPitchShifter, FormantPreserver, PitchAlgorithm, PitchShifter, PitchShifterConfig,
};
pub use stretch::{TimeStretchConfig, TimeStretcher};
