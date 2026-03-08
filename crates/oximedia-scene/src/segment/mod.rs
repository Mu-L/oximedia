//! Image segmentation.

pub mod foreground;
pub mod semantic;

pub use foreground::{ForegroundSegmenter, SegmentMask};
pub use semantic::{SemanticRegion, SemanticSegmenter};
