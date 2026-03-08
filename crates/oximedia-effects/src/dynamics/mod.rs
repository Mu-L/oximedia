//! Dynamics processing effects.
//!
//! Provides dynamics control:
//!
//! - **Gate** - Noise gate with hysteresis
//! - **Expander** - Upward and downward expansion

pub mod expander;
pub mod gate;

// Re-exports
pub use expander::{Expander, ExpanderConfig, ExpanderType};
pub use gate::{Gate, GateConfig};
