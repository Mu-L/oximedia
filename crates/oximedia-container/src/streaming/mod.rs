//! Streaming demuxing and muxing.
//!
//! This module provides streaming capabilities for both demuxing and muxing,
//! optimized for live streaming and network sources.

#![forbid(unsafe_code)]

pub mod demux;
pub mod mux;

#[cfg(not(target_arch = "wasm32"))]
pub use demux::{spawn_demuxer, PacketReceiver};
pub use demux::{ProgressiveBuffer, StreamingDemuxer, StreamingDemuxerConfig, StreamingState};
#[cfg(not(target_arch = "wasm32"))]
pub use mux::{spawn_muxer, PacketSender, StreamingMuxer};
pub use mux::{LatencyMonitor, MuxingStats, StreamingMuxerConfig};
