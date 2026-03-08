//! Calibration LUT generation and verification.
//!
//! This module provides tools for generating calibration LUTs from measurements
//! and verifying their accuracy.

pub mod generate;
pub mod measure;
pub mod verify;

pub use generate::LutGenerator;
pub use measure::LutMeasurement;
pub use verify::LutVerifier;
