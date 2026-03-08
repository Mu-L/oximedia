//! Streaming demuxing and muxing.
//!
//! This module provides streaming capabilities for both demuxing and muxing,
//! optimized for live streaming and network sources.

#![forbid(unsafe_code)]

pub mod demux;
pub mod mux;

pub use demux::{
    spawn_demuxer, PacketReceiver, ProgressiveBuffer, StreamingDemuxer, StreamingDemuxerConfig,
    StreamingState,
};
pub use mux::{
    spawn_muxer, LatencyMonitor, MuxingStats, PacketSender, StreamingMuxer, StreamingMuxerConfig,
};
