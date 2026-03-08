//! Shot and transition detection algorithms.

pub mod cut;
pub mod dissolve;
pub mod fade;
pub mod wipe;

pub use cut::CutDetector;
pub use dissolve::DissolveDetector;
pub use fade::{FadeColor, FadeDetector, FadeDirection};
pub use wipe::{WipeDetector, WipeDirection};
