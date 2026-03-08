//! SCTE-35 marker generation for commercial breaks.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// SCTE-35 command type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Scte35Command {
    /// Splice insert command.
    SpliceInsert {
        /// Unique splice event ID.
        event_id: u32,
        /// Whether this is an immediate splice.
        immediate: bool,
        /// Pre-roll time before the splice point.
        pre_roll: Option<Duration>,
        /// Duration of the splice.
        duration: Option<Duration>,
    },

    /// Time signal command.
    TimeSignal {
        /// PTS time value.
        pts_time: u64,
    },

    /// Splice schedule command.
    SpliceSchedule {
        /// Splice event ID.
        event_id: u32,
        /// Scheduled splice time.
        splice_time: Duration,
    },

    /// Splice null (cancel).
    SpliceNull,
}

/// SCTE-35 marker for signaling commercial breaks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scte35Marker {
    /// Command to execute.
    pub command: Scte35Command,

    /// Descriptor tags (optional metadata).
    pub descriptors: Vec<Scte35Descriptor>,

    /// Tier value for authorization.
    pub tier: u16,

    /// Whether this is a network segmentation.
    pub is_network_segment: bool,
}

/// SCTE-35 descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scte35Descriptor {
    /// Descriptor tag.
    pub tag: u8,

    /// Descriptor data.
    pub data: Vec<u8>,
}

impl Scte35Marker {
    /// Creates a simple splice insert marker.
    #[must_use]
    pub fn splice_insert(event_id: u32, duration: Option<Duration>) -> Self {
        Self {
            command: Scte35Command::SpliceInsert {
                event_id,
                immediate: false,
                pre_roll: Some(Duration::from_secs(2)),
                duration,
            },
            descriptors: Vec::new(),
            tier: 0,
            is_network_segment: false,
        }
    }

    /// Creates a time signal marker.
    #[must_use]
    pub const fn time_signal(pts_time: u64) -> Self {
        Self {
            command: Scte35Command::TimeSignal { pts_time },
            descriptors: Vec::new(),
            tier: 0,
            is_network_segment: false,
        }
    }

    /// Creates an immediate splice marker.
    #[must_use]
    pub fn immediate_splice(event_id: u32, duration: Option<Duration>) -> Self {
        Self {
            command: Scte35Command::SpliceInsert {
                event_id,
                immediate: true,
                pre_roll: None,
                duration,
            },
            descriptors: Vec::new(),
            tier: 0,
            is_network_segment: false,
        }
    }

    /// Adds a descriptor to this marker.
    pub fn add_descriptor(&mut self, descriptor: Scte35Descriptor) {
        self.descriptors.push(descriptor);
    }

    /// Sets the tier value.
    #[must_use]
    pub const fn with_tier(mut self, tier: u16) -> Self {
        self.tier = tier;
        self
    }

    /// Marks this as a network segment.
    #[must_use]
    pub const fn as_network_segment(mut self) -> Self {
        self.is_network_segment = true;
        self
    }

    /// Encodes this marker to SCTE-35 binary format.
    ///
    /// Note: This is a simplified placeholder. Real implementation would
    /// follow the full SCTE-35 specification.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        // Placeholder for SCTE-35 encoding
        // Real implementation would generate proper SCTE-35 binary data
        vec![0xFC, 0x30, 0x00] // SCTE-35 header start
    }
}

impl Scte35Descriptor {
    /// Creates a segmentation descriptor.
    #[must_use]
    pub fn segmentation(segmentation_event_id: u32, type_id: u8) -> Self {
        let mut data = Vec::new();
        data.extend_from_slice(&segmentation_event_id.to_be_bytes());
        data.push(type_id);

        Self {
            tag: 0x02, // Segmentation descriptor tag
            data,
        }
    }
}

/// Segmentation type IDs for SCTE-35.
#[allow(dead_code)]
pub mod segmentation_types {
    /// Program start.
    pub const PROGRAM_START: u8 = 0x10;
    /// Program end.
    pub const PROGRAM_END: u8 = 0x11;
    /// Chapter start.
    pub const CHAPTER_START: u8 = 0x20;
    /// Chapter end.
    pub const CHAPTER_END: u8 = 0x21;
    /// Provider ad start.
    pub const PROVIDER_AD_START: u8 = 0x30;
    /// Provider ad end.
    pub const PROVIDER_AD_END: u8 = 0x31;
    /// Distributor ad start.
    pub const DISTRIBUTOR_AD_START: u8 = 0x32;
    /// Distributor ad end.
    pub const DISTRIBUTOR_AD_END: u8 = 0x33;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_splice_insert() {
        let marker = Scte35Marker::splice_insert(123, Some(Duration::from_secs(120)));

        match &marker.command {
            Scte35Command::SpliceInsert {
                event_id, duration, ..
            } => {
                assert_eq!(*event_id, 123);
                assert_eq!(*duration, Some(Duration::from_secs(120)));
            }
            _ => panic!("Expected SpliceInsert command"),
        }
    }

    #[test]
    fn test_immediate_splice() {
        let marker = Scte35Marker::immediate_splice(456, None);

        match &marker.command {
            Scte35Command::SpliceInsert { immediate, .. } => {
                assert!(immediate);
            }
            _ => panic!("Expected SpliceInsert command"),
        }
    }

    #[test]
    fn test_segmentation_descriptor() {
        let descriptor = Scte35Descriptor::segmentation(100, segmentation_types::PROVIDER_AD_START);
        assert_eq!(descriptor.tag, 0x02);
        assert!(!descriptor.data.is_empty());
    }

    #[test]
    fn test_marker_encoding() {
        let marker = Scte35Marker::splice_insert(1, Some(Duration::from_secs(30)));
        let encoded = marker.encode();
        assert!(!encoded.is_empty());
    }
}
