//! High-level tally system for broadcast switchers.
//!
//! Provides a unified tally management layer that tracks on-air and preview
//! states across all M/E buses and distributes tally signals to connected
//! tally lights and downstream devices.

#![allow(dead_code)]

use std::collections::HashMap;

/// Color emitted by a tally light on a camera or source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TallyLightColor {
    /// Source is live on program output (red).
    Red,
    /// Source is selected on preview bus (green).
    Green,
    /// Source is queued or in standby (amber).
    Amber,
    /// No active tally (off).
    Off,
}

/// Operational state of a single tally-enabled source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TallySourceState {
    /// Source is on air (program).
    Live,
    /// Source is on preview.
    Preview,
    /// Source is active in a keyer layer.
    KeyerLive,
    /// Source is not in use.
    Inactive,
}

impl TallySourceState {
    /// Return the tally light color that corresponds to this state.
    #[must_use]
    pub fn light_color(self) -> TallyLightColor {
        match self {
            Self::Live | Self::KeyerLive => TallyLightColor::Red,
            Self::Preview => TallyLightColor::Green,
            Self::Inactive => TallyLightColor::Off,
        }
    }

    /// Returns `true` if the source is currently on air.
    #[must_use]
    pub fn is_on_air(self) -> bool {
        matches!(self, Self::Live | Self::KeyerLive)
    }
}

/// Entry stored for each registered source.
#[derive(Debug, Clone)]
struct TallyEntry {
    label: String,
    state: TallySourceState,
}

/// Central tally management system.
///
/// Tracks the on-air and preview state for every registered source and
/// provides convenient queries such as "which sources are currently live?"
#[derive(Debug, Default)]
pub struct TallySystem {
    entries: HashMap<u32, TallyEntry>,
    /// Currently active program source IDs (per M/E row).
    program_sources: Vec<u32>,
    /// Currently active preview source IDs (per M/E row).
    preview_sources: Vec<u32>,
}

impl TallySystem {
    /// Create a new, empty tally system.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a source with the tally system.
    pub fn register_source(&mut self, id: u32, label: &str) {
        self.entries.insert(
            id,
            TallyEntry {
                label: label.to_string(),
                state: TallySourceState::Inactive,
            },
        );
    }

    /// Remove a source from the tally system.
    pub fn unregister_source(&mut self, id: u32) {
        self.entries.remove(&id);
    }

    /// Set a source as live (on program output).
    ///
    /// Also clears its preview state if it was previously on preview.
    pub fn set_live(&mut self, id: u32) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.state = TallySourceState::Live;
        }
        if !self.program_sources.contains(&id) {
            self.program_sources.push(id);
        }
        self.preview_sources.retain(|&x| x != id);
    }

    /// Set a source as on preview.
    pub fn set_preview(&mut self, id: u32) {
        if let Some(entry) = self.entries.get_mut(&id) {
            if entry.state != TallySourceState::Live {
                entry.state = TallySourceState::Preview;
            }
        }
        if !self.preview_sources.contains(&id) {
            self.preview_sources.push(id);
        }
    }

    /// Set a source as inactive (remove from all buses).
    pub fn set_inactive(&mut self, id: u32) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.state = TallySourceState::Inactive;
        }
        self.program_sources.retain(|&x| x != id);
        self.preview_sources.retain(|&x| x != id);
    }

    /// Mark a source as active in a keyer layer (still on-air).
    pub fn set_keyer_live(&mut self, id: u32) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.state = TallySourceState::KeyerLive;
        }
    }

    /// Query the current state of a source.
    #[must_use]
    pub fn state(&self, id: u32) -> TallySourceState {
        self.entries
            .get(&id)
            .map_or(TallySourceState::Inactive, |e| e.state)
    }

    /// Query the tally light color for a source.
    #[must_use]
    pub fn light_color(&self, id: u32) -> TallyLightColor {
        self.state(id).light_color()
    }

    /// Return all sources that are currently live (on-air).
    #[must_use]
    pub fn live_sources(&self) -> Vec<u32> {
        self.entries
            .iter()
            .filter(|(_, e)| e.state.is_on_air())
            .map(|(&id, _)| id)
            .collect()
    }

    /// Return all sources that are currently on preview.
    #[must_use]
    pub fn preview_sources(&self) -> Vec<u32> {
        self.entries
            .iter()
            .filter(|(_, e)| e.state == TallySourceState::Preview)
            .map(|(&id, _)| id)
            .collect()
    }

    /// Return the human-readable label for a source, if registered.
    #[must_use]
    pub fn label(&self, id: u32) -> Option<&str> {
        self.entries.get(&id).map(|e| e.label.as_str())
    }

    /// Return total number of registered sources.
    #[must_use]
    pub fn source_count(&self) -> usize {
        self.entries.len()
    }

    /// Clear all tally states (set every source to inactive).
    pub fn clear_all(&mut self) {
        for entry in self.entries.values_mut() {
            entry.state = TallySourceState::Inactive;
        }
        self.program_sources.clear();
        self.preview_sources.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_system() -> TallySystem {
        let mut s = TallySystem::new();
        s.register_source(1, "Camera 1");
        s.register_source(2, "Camera 2");
        s.register_source(3, "VT 1");
        s
    }

    #[test]
    fn test_new_system_is_empty() {
        let s = TallySystem::new();
        assert_eq!(s.source_count(), 0);
    }

    #[test]
    fn test_register_source() {
        let mut s = TallySystem::new();
        s.register_source(1, "Cam 1");
        assert_eq!(s.source_count(), 1);
        assert_eq!(s.state(1), TallySourceState::Inactive);
    }

    #[test]
    fn test_set_live() {
        let mut s = make_system();
        s.set_live(1);
        assert_eq!(s.state(1), TallySourceState::Live);
        assert_eq!(s.light_color(1), TallyLightColor::Red);
        assert!(s.state(1).is_on_air());
    }

    #[test]
    fn test_set_preview() {
        let mut s = make_system();
        s.set_preview(2);
        assert_eq!(s.state(2), TallySourceState::Preview);
        assert_eq!(s.light_color(2), TallyLightColor::Green);
        assert!(!s.state(2).is_on_air());
    }

    #[test]
    fn test_set_inactive() {
        let mut s = make_system();
        s.set_live(1);
        s.set_inactive(1);
        assert_eq!(s.state(1), TallySourceState::Inactive);
        assert_eq!(s.light_color(1), TallyLightColor::Off);
    }

    #[test]
    fn test_set_keyer_live() {
        let mut s = make_system();
        s.set_keyer_live(3);
        assert_eq!(s.state(3), TallySourceState::KeyerLive);
        assert!(s.state(3).is_on_air());
        assert_eq!(s.light_color(3), TallyLightColor::Red);
    }

    #[test]
    fn test_live_sources() {
        let mut s = make_system();
        s.set_live(1);
        s.set_keyer_live(3);
        let live = s.live_sources();
        assert!(live.contains(&1));
        assert!(live.contains(&3));
        assert!(!live.contains(&2));
    }

    #[test]
    fn test_preview_sources() {
        let mut s = make_system();
        s.set_preview(2);
        let pv = s.preview_sources();
        assert!(pv.contains(&2));
        assert!(!pv.contains(&1));
    }

    #[test]
    fn test_set_live_removes_from_preview() {
        let mut s = make_system();
        s.set_preview(1);
        assert_eq!(s.state(1), TallySourceState::Preview);
        s.set_live(1);
        assert_eq!(s.state(1), TallySourceState::Live);
        assert!(s.preview_sources().is_empty());
    }

    #[test]
    fn test_clear_all() {
        let mut s = make_system();
        s.set_live(1);
        s.set_preview(2);
        s.set_keyer_live(3);
        s.clear_all();
        assert_eq!(s.state(1), TallySourceState::Inactive);
        assert_eq!(s.state(2), TallySourceState::Inactive);
        assert_eq!(s.state(3), TallySourceState::Inactive);
        assert!(s.live_sources().is_empty());
        assert!(s.preview_sources().is_empty());
    }

    #[test]
    fn test_label() {
        let s = make_system();
        assert_eq!(s.label(1), Some("Camera 1"));
        assert_eq!(s.label(2), Some("Camera 2"));
        assert_eq!(s.label(99), None);
    }

    #[test]
    fn test_unregister_source() {
        let mut s = make_system();
        s.unregister_source(1);
        assert_eq!(s.source_count(), 2);
        assert_eq!(s.state(1), TallySourceState::Inactive);
    }

    #[test]
    fn test_unregistered_source_is_inactive() {
        let s = TallySystem::new();
        assert_eq!(s.state(42), TallySourceState::Inactive);
        assert_eq!(s.light_color(42), TallyLightColor::Off);
    }

    #[test]
    fn test_tally_light_color_variants() {
        assert_eq!(TallySourceState::Live.light_color(), TallyLightColor::Red);
        assert_eq!(
            TallySourceState::KeyerLive.light_color(),
            TallyLightColor::Red
        );
        assert_eq!(
            TallySourceState::Preview.light_color(),
            TallyLightColor::Green
        );
        assert_eq!(
            TallySourceState::Inactive.light_color(),
            TallyLightColor::Off
        );
    }
}
