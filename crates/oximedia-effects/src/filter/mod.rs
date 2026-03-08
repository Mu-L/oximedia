//! Filter effects module.
//!
//! Provides various filter types:
//!
//! - **State-Variable Filter** - Multi-mode filter (LP, HP, BP, Notch)
//! - **Moog Ladder Filter** - Classic 4-pole low-pass with resonance

pub mod moog;
pub mod state_variable;

// Re-exports
pub use moog::{MoogConfig, MoogFilter};
pub use state_variable::{FilterMode, StateVariableConfig, StateVariableFilter};
