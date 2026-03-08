//! Focus peaking and assist.

pub mod peaking;

pub use peaking::FocusPeaking;

use crate::scopes::focus::FocusAssist;

// Re-export from scopes module
pub use crate::scopes::focus::{PeakingColor, FocusMetrics};
