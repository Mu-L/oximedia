//! Routing automation with timecode support.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Timecode representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timecode {
    /// Hours (0-23)
    pub hours: u8,
    /// Minutes (0-59)
    pub minutes: u8,
    /// Seconds (0-59)
    pub seconds: u8,
    /// Frames (0-frame_rate-1)
    pub frames: u8,
    /// Frame rate
    pub frame_rate: FrameRate,
}

/// Frame rate for timecode
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FrameRate {
    /// 24 fps (film)
    Fps24,
    /// 25 fps (PAL)
    Fps25,
    /// 29.97 fps drop-frame (NTSC)
    Fps2997Df,
    /// 29.97 fps non-drop (NTSC)
    Fps2997Ndf,
    /// 30 fps
    Fps30,
    /// 50 fps
    Fps50,
    /// 59.94 fps
    Fps5994,
    /// 60 fps
    Fps60,
}

impl FrameRate {
    /// Get the numeric frame rate
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        match self {
            Self::Fps24 => 24,
            Self::Fps25 => 25,
            Self::Fps2997Df | Self::Fps2997Ndf => 30,
            Self::Fps30 => 30,
            Self::Fps50 => 50,
            Self::Fps5994 => 60,
            Self::Fps60 => 60,
        }
    }
}

impl Timecode {
    /// Create a new timecode
    #[must_use]
    pub const fn new(
        hours: u8,
        minutes: u8,
        seconds: u8,
        frames: u8,
        frame_rate: FrameRate,
    ) -> Self {
        Self {
            hours,
            minutes,
            seconds,
            frames,
            frame_rate,
        }
    }

    /// Create timecode from total frames
    #[must_use]
    pub fn from_frames(total_frames: u64, frame_rate: FrameRate) -> Self {
        let fps = u64::from(frame_rate.as_u8());
        let frames = (total_frames % fps) as u8;
        let total_seconds = total_frames / fps;
        let seconds = (total_seconds % 60) as u8;
        let total_minutes = total_seconds / 60;
        let minutes = (total_minutes % 60) as u8;
        let hours = (total_minutes / 60) as u8;

        Self {
            hours,
            minutes,
            seconds,
            frames,
            frame_rate,
        }
    }

    /// Convert to total frames
    #[must_use]
    pub fn to_frames(&self) -> u64 {
        let fps = u64::from(self.frame_rate.as_u8());
        u64::from(self.hours) * 3600 * fps
            + u64::from(self.minutes) * 60 * fps
            + u64::from(self.seconds) * fps
            + u64::from(self.frames)
    }
}

/// Routing automation action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutomationAction {
    /// Connect two points
    Connect {
        source: usize,
        destination: usize,
        gain_db: f32,
    },
    /// Disconnect two points
    Disconnect {
        /// Source index
        source: usize,
        /// Destination index
        destination: usize,
    },
    /// Set gain
    SetGain {
        /// Channel index
        channel: usize,
        /// Gain in dB
        gain_db: f32,
    },
    /// Mute channel
    Mute {
        /// Channel index
        channel: usize,
    },
    /// Unmute channel
    Unmute {
        /// Channel index
        channel: usize,
    },
    /// Load preset
    LoadPreset {
        /// Preset ID
        preset_id: u64,
    },
    /// Custom action
    Custom {
        /// Action type identifier
        action_type: String,
        /// Action parameters
        parameters: Vec<f32>,
    },
}

/// Automation event at a specific timecode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationEvent {
    /// Timecode when this event triggers
    pub timecode: Timecode,
    /// Action to perform
    pub action: AutomationAction,
    /// Event description
    pub description: String,
    /// Whether this event is enabled
    pub enabled: bool,
}

/// Automation timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationTimeline {
    /// Events indexed by timecode
    events: BTreeMap<Timecode, Vec<AutomationEvent>>,
    /// Timeline name
    pub name: String,
    /// Frame rate for this timeline
    pub frame_rate: FrameRate,
}

impl AutomationTimeline {
    /// Create a new automation timeline
    #[must_use]
    pub fn new(name: String, frame_rate: FrameRate) -> Self {
        Self {
            events: BTreeMap::new(),
            name,
            frame_rate,
        }
    }

    /// Add an event
    pub fn add_event(&mut self, event: AutomationEvent) {
        self.events.entry(event.timecode).or_default().push(event);
    }

    /// Remove events at a timecode
    pub fn remove_events_at(&mut self, timecode: Timecode) {
        self.events.remove(&timecode);
    }

    /// Get events at a specific timecode
    #[must_use]
    pub fn get_events_at(&self, timecode: Timecode) -> Option<&Vec<AutomationEvent>> {
        self.events.get(&timecode)
    }

    /// Get all events in a time range
    #[must_use]
    pub fn get_events_in_range(&self, start: Timecode, end: Timecode) -> Vec<&AutomationEvent> {
        self.events
            .range(start..=end)
            .flat_map(|(_, events)| events)
            .collect()
    }

    /// Get all enabled events
    #[must_use]
    pub fn get_enabled_events(&self) -> Vec<&AutomationEvent> {
        self.events
            .values()
            .flatten()
            .filter(|e| e.enabled)
            .collect()
    }

    /// Get total event count
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.values().map(Vec::len).sum()
    }

    /// Clear all events
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timecode_creation() {
        let tc = Timecode::new(1, 30, 45, 12, FrameRate::Fps25);
        assert_eq!(tc.hours, 1);
        assert_eq!(tc.minutes, 30);
        assert_eq!(tc.seconds, 45);
        assert_eq!(tc.frames, 12);
    }

    #[test]
    fn test_timecode_from_frames() {
        let tc = Timecode::from_frames(150, FrameRate::Fps25);
        assert_eq!(tc.seconds, 6);
        assert_eq!(tc.frames, 0);
    }

    #[test]
    fn test_timecode_to_frames() {
        let tc = Timecode::new(0, 0, 10, 0, FrameRate::Fps25);
        assert_eq!(tc.to_frames(), 250);
    }

    #[test]
    fn test_timeline_creation() {
        let timeline = AutomationTimeline::new("Show 1".to_string(), FrameRate::Fps25);
        assert_eq!(timeline.event_count(), 0);
    }

    #[test]
    fn test_add_event() {
        let mut timeline = AutomationTimeline::new("Test".to_string(), FrameRate::Fps25);

        let event = AutomationEvent {
            timecode: Timecode::new(0, 1, 0, 0, FrameRate::Fps25),
            action: AutomationAction::Mute { channel: 0 },
            description: "Mute channel 0".to_string(),
            enabled: true,
        };

        timeline.add_event(event);
        assert_eq!(timeline.event_count(), 1);
    }

    #[test]
    fn test_get_events_at() {
        let mut timeline = AutomationTimeline::new("Test".to_string(), FrameRate::Fps25);

        let tc = Timecode::new(0, 1, 0, 0, FrameRate::Fps25);
        let event = AutomationEvent {
            timecode: tc,
            action: AutomationAction::Mute { channel: 0 },
            description: "Test".to_string(),
            enabled: true,
        };

        timeline.add_event(event);

        let events = timeline.get_events_at(tc);
        assert!(events.is_some());
        assert_eq!(events.expect("should succeed in test").len(), 1);
    }

    #[test]
    fn test_get_events_in_range() {
        let mut timeline = AutomationTimeline::new("Test".to_string(), FrameRate::Fps25);

        let tc1 = Timecode::new(0, 1, 0, 0, FrameRate::Fps25);
        let tc2 = Timecode::new(0, 2, 0, 0, FrameRate::Fps25);
        let tc3 = Timecode::new(0, 3, 0, 0, FrameRate::Fps25);

        timeline.add_event(AutomationEvent {
            timecode: tc1,
            action: AutomationAction::Mute { channel: 0 },
            description: "Event 1".to_string(),
            enabled: true,
        });

        timeline.add_event(AutomationEvent {
            timecode: tc2,
            action: AutomationAction::Mute { channel: 1 },
            description: "Event 2".to_string(),
            enabled: true,
        });

        timeline.add_event(AutomationEvent {
            timecode: tc3,
            action: AutomationAction::Mute { channel: 2 },
            description: "Event 3".to_string(),
            enabled: true,
        });

        let events = timeline.get_events_in_range(tc1, tc2);
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_remove_events() {
        let mut timeline = AutomationTimeline::new("Test".to_string(), FrameRate::Fps25);

        let tc = Timecode::new(0, 1, 0, 0, FrameRate::Fps25);
        timeline.add_event(AutomationEvent {
            timecode: tc,
            action: AutomationAction::Mute { channel: 0 },
            description: "Test".to_string(),
            enabled: true,
        });

        assert_eq!(timeline.event_count(), 1);

        timeline.remove_events_at(tc);
        assert_eq!(timeline.event_count(), 0);
    }

    #[test]
    fn test_enabled_events() {
        let mut timeline = AutomationTimeline::new("Test".to_string(), FrameRate::Fps25);

        timeline.add_event(AutomationEvent {
            timecode: Timecode::new(0, 1, 0, 0, FrameRate::Fps25),
            action: AutomationAction::Mute { channel: 0 },
            description: "Enabled".to_string(),
            enabled: true,
        });

        timeline.add_event(AutomationEvent {
            timecode: Timecode::new(0, 2, 0, 0, FrameRate::Fps25),
            action: AutomationAction::Mute { channel: 1 },
            description: "Disabled".to_string(),
            enabled: false,
        });

        let enabled = timeline.get_enabled_events();
        assert_eq!(enabled.len(), 1);
    }
}
