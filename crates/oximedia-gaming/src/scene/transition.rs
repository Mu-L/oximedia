//! Scene transitions.

use std::time::Duration;

/// Scene transition effect.
pub struct SceneTransition {
    /// Transition type
    pub transition_type: TransitionType,
    /// Duration
    pub duration: Duration,
}

/// Transition type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionType {
    /// Instant cut
    Cut,
    /// Fade to black
    Fade,
    /// Slide from left
    SlideLeft,
    /// Slide from right
    SlideRight,
    /// Swipe
    Swipe,
}

impl SceneTransition {
    /// Create a new transition.
    #[must_use]
    pub fn new(transition_type: TransitionType, duration: Duration) -> Self {
        Self {
            transition_type,
            duration,
        }
    }
}

impl Default for SceneTransition {
    fn default() -> Self {
        Self {
            transition_type: TransitionType::Fade,
            duration: Duration::from_millis(300),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_creation() {
        let transition = SceneTransition::new(TransitionType::Fade, Duration::from_millis(500));
        assert_eq!(transition.transition_type, TransitionType::Fade);
    }
}
