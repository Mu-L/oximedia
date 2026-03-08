//! Camera calibration and profiling.
//!
//! This module provides tools for calibrating cameras using `ColorChecker` targets
//! and generating camera profiles for color-accurate image reproduction.

pub mod calibrate;
pub mod colorchecker;
pub mod profile;

pub use calibrate::CameraCalibrator;
pub use colorchecker::{ColorChecker, ColorCheckerType, PatchColor};
pub use profile::{CameraProfile, ProfileQuality};
