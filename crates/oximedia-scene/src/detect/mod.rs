//! Object, face, logo, and text detection.
//!
//! This module provides various detection algorithms using patent-free methods:
//!
//! - **Object detection**: HOG-based detection for common objects
//! - **Face detection**: Haar cascade-based face detection
//! - **Logo detection**: Template matching and feature-based detection
//! - **Text detection**: Text region detection using connected components

pub mod face;
pub mod logo;
pub mod object;
pub mod text;

pub use face::{FaceDetection, FaceDetector};
pub use logo::{LogoDetection, LogoDetector};
pub use object::{ObjectDetection, ObjectDetector, ObjectType};
pub use text::{TextDetection, TextDetector};
