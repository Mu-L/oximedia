//! # `OxiMedia` Routing
//!
//! Professional audio routing and patching system for `OxiMedia`.
//!
//! ## Features
//!
//! - **Crosspoint Matrix**: Full any-to-any audio routing
//! - **Virtual Patch Bay**: Input/output management with flexible patching
//! - **Channel Mapping**: Complex channel mapping and remapping (e.g., 5.1 to stereo)
//! - **Signal Flow**: Signal flow graph visualization and validation
//! - **Audio Embedding**: Audio embedding/de-embedding for SDI
//! - **Format Conversion**: Sample rate, bit depth, and channel count conversion
//! - **Gain Staging**: Per-channel gain control with metering
//! - **Monitoring**: AFL/PFL/Solo monitoring systems
//! - **Preset Management**: Save/load routing configurations
//! - **MADI Support**: 64-channel MADI routing
//! - **Dante Integration**: Dante audio-over-IP metadata (respects Audinate IP)
//! - **Automation**: Time-based routing changes with timecode
//!
//! ## Usage
//!
//! ```
//! use oximedia_routing::matrix::CrosspointMatrix;
//!
//! // Create a 16x8 crosspoint matrix
//! let mut matrix = CrosspointMatrix::new(16, 8);
//!
//! // Connect input 0 to output 0 with -6 dB gain
//! matrix.connect(0, 0, Some(-6.0)).expect("should succeed in test");
//!
//! // Check if connected
//! assert!(matrix.is_connected(0, 0));
//! ```
//!
//! ## Routing Scenarios
//!
//! ### Live Production
//! Route microphones to mixer, mixer to multitrack recorder
//!
//! ### Post-Production
//! Route editor to monitors, create submixes for stems
//!
//! ### Broadcast
//! Route studios to transmission, implement backup routing
//!
//! ### Multitrack Recording
//! Route 64+ channels from stage to recording system

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(
    clippy::similar_names,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::too_many_arguments,
    clippy::struct_excessive_bools,
    clippy::missing_errors_doc,
    clippy::type_complexity,
    clippy::match_like_matches_macro,
    clippy::match_same_arms,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    missing_docs
)]

// Matrix routing modules
pub mod matrix;

// Virtual patch bay
pub mod patch;

// Channel management
pub mod channel;

// Signal flow
pub mod flow;

// Audio embedding/de-embedding
pub mod embed;

// Format conversion
pub mod convert;

// Gain staging
pub mod gain;

// Monitoring
pub mod monitor;

// Preset management
pub mod preset;

// MADI support
pub mod madi;

// Dante support
pub mod dante;

// NMOS IS-04/IS-05
pub mod nmos;

#[cfg(feature = "nmos-http")]
pub use nmos::http::NmosHttpServer;

#[cfg(feature = "nmos-discovery")]
pub use nmos::{NmosDiscovery, NmosDiscoveryBuilder, NmosDiscoveryError, NmosRegistryInfo};

// Automation
pub mod automation;

// Video routing matrix
pub mod matrix_router;

// Signal path analysis
pub mod signal_path;

// IP media routing (ST 2110)
pub mod ip_router;

// Network path selection
pub mod path_selector;

// Crosspoint routing matrix
pub mod crosspoint_matrix;

// Automatic failover routing
pub mod failover_route;

// Route table with longest-prefix-match
pub mod route_table;

// Signal presence and health monitoring
pub mod signal_monitor;

// Policy-driven routing decisions
pub mod routing_policy;

// Bandwidth budgeting
pub mod bandwidth_budget;

// Route optimization
pub mod route_optimizer;

// Link aggregation
pub mod link_aggregation;

// Latency calculation and budgeting
pub mod latency_calc;

// Named routing presets for rapid recall
pub mod route_preset;

// Route audit trail
pub mod route_audit;

// Network topology mapping
pub mod topology_map;

// Redundancy group management
pub mod redundancy_group;

// Traffic shaping and QoS
pub mod traffic_shaper;

// AES67 audio-over-IP interoperability
pub mod aes67;

// Hardware GPI/O-triggered routing changes
pub mod gpio_trigger;

// Level meter insertion at arbitrary signal path points
pub mod metering_bridge;

// Save/restore complete routing state with atomic rollback
pub mod routing_snapshot;

// Test signal generator (sine, pink noise, sweep)
pub mod signal_generator;

// Mix-minus routing for broadcast IFB feeds
pub mod mix_minus;

// Sparse crosspoint matrix for large matrices (256×256+)
pub mod sparse_matrix;

/// Re-export commonly used types for convenience
pub mod prelude {
    pub use crate::automation::{AutomationTimeline, Timecode};
    pub use crate::channel::{ChannelLayout, ChannelRemapper};
    pub use crate::convert::{BitDepthConverter, ChannelCountConverter, SampleRateConverter};
    pub use crate::embed::{AudioDeembedder, AudioEmbedder};
    pub use crate::flow::{SignalFlowGraph, ValidationResult};
    pub use crate::gain::{GainStage, MultiChannelGainStage};
    pub use crate::madi::MadiInterface;
    pub use crate::matrix::{ConnectionManager, CrosspointMatrix, RoutingPathSolver};
    pub use crate::monitor::{AflMonitor, PflMonitor, SoloManager};
    pub use crate::patch::{PatchBay, PatchInput, PatchOutput};
    pub use crate::preset::{PresetManager, RoutingPreset};
}

#[cfg(test)]
mod tests {
    use super::prelude::*;

    #[test]
    fn test_basic_routing() {
        let mut matrix = CrosspointMatrix::new(4, 4);
        matrix.connect(0, 0, None).expect("should succeed in test");
        assert!(matrix.is_connected(0, 0));
    }

    #[test]
    fn test_patch_bay() {
        use crate::patch::{DestinationType, SourceType};

        let mut bay = PatchBay::new();

        let input = bay
            .input_manager_mut()
            .add_input("Mic 1".to_string(), SourceType::Microphone);
        let output = bay
            .output_manager_mut()
            .add_output("Monitor".to_string(), DestinationType::Monitor);

        bay.patch(input, output, None)
            .expect("should succeed in test");
        assert!(bay.is_patched(input, output));
    }

    #[test]
    fn test_channel_mapping() {
        let remapper = ChannelRemapper::downmix_51_to_stereo();
        assert_eq!(remapper.input_layout, ChannelLayout::Surround51);
        assert_eq!(remapper.output_layout, ChannelLayout::Stereo);
    }

    #[test]
    fn test_signal_flow() {
        use crate::flow::FlowEdge;

        let mut graph = SignalFlowGraph::new();

        let input = graph.add_input("Source".to_string(), 2);
        let output = graph.add_output("Destination".to_string(), 2);

        graph
            .connect(input, output, FlowEdge::default())
            .expect("should succeed in test");

        let result = graph.validate();
        assert!(result.is_valid);
    }

    #[test]
    fn test_gain_staging() {
        let mut stage = GainStage::new();
        stage.set_gain(-6.0);
        assert!((stage.gain_db - (-6.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_monitoring() {
        let mut solo = SoloManager::new();
        solo.solo(0);
        assert!(solo.is_soloed(0));
        assert!(!solo.is_soloed(1));
    }

    #[test]
    fn test_preset_management() {
        let mut manager = PresetManager::new();
        let preset = RoutingPreset::new("Test".to_string(), "Test preset".to_string());

        let id = manager.add_preset(preset);
        assert!(manager.get_preset(id).is_some());
    }

    #[test]
    fn test_madi_interface() {
        use crate::madi::FrameMode;

        let mut interface = MadiInterface::new("MADI 1".to_string());
        assert_eq!(interface.max_channels(), 64);

        interface.set_frame_mode(FrameMode::Frame96k);
        assert_eq!(interface.max_channels(), 32);
    }

    #[test]
    fn test_automation() {
        use crate::automation::{AutomationAction, AutomationEvent, FrameRate};

        let mut timeline = AutomationTimeline::new("Show".to_string(), FrameRate::Fps25);

        let event = AutomationEvent {
            timecode: Timecode::new(0, 1, 0, 0, FrameRate::Fps25),
            action: AutomationAction::Mute { channel: 0 },
            description: "Mute channel 0".to_string(),
            enabled: true,
        };

        timeline.add_event(event);
        assert_eq!(timeline.event_count(), 1);
    }
}
