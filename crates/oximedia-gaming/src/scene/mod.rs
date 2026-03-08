//! Scene management and transitions.

pub mod hotkey;
pub mod manager;
pub mod transition;

pub use hotkey::{Hotkey, HotkeyAction, HotkeyManager};
pub use manager::{Scene, SceneManager};
pub use transition::{SceneTransition, TransitionType};
