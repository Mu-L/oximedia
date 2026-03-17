//! Professional video processing operations for OxiMedia.
//!
//! Provides block-based motion estimation and compensation, frame rate
//! conversion with intermediate frame generation, video deinterlacing,
//! scene change detection, 3:2 pulldown cadence detection, perceptual
//! video fingerprinting, and temporal noise reduction.

#![warn(missing_docs, rust_2018_idioms, unreachable_pub, unsafe_code)]

pub mod cadence_convert;
pub mod deinterlace;
pub mod duplicate_frame_detect;
pub mod field_order_detect;
pub mod film_grain_synthesis;
pub mod frame_interpolation;
pub mod hdr_tonemapping;
pub mod motion_compensation;
pub mod parallel_motion_search;
pub mod pulldown_detect;
pub mod quality_metrics;
pub mod scene_detection;
pub mod shot_boundary_classifier;
pub mod slow_motion;
pub mod stabilization;
pub mod subpixel_refiner;
pub mod super_resolution;
pub mod temporal_denoise;
pub mod video_fingerprint;
