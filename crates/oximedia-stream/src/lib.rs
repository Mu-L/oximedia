//! `oximedia-stream` — Adaptive streaming pipeline, segment lifecycle management,
//! and stream health monitoring for the OxiMedia framework.
//!
//! # Modules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`adaptive_pipeline`] | Quality ladder, BOLA-inspired ABR switching |
//! | [`segment_manager`] | Segment state machine, prefetch/eviction |
//! | [`stream_health`] | QoE scoring, issue detection, history |
//! | [`scte35`] | SCTE-35 splice information encoding/parsing/scheduling |
//! | [`multi_cdn`] | Multi-CDN failover routing with EWMA latency tracking |
//! | [`manifest_builder`] | HLS master/media playlist and DASH MPD generation |
//! | [`stream_packager`] | Media unit accumulation and segment packaging |
//! | [`ll_hls`] | Low-Latency HLS with partial segments (RFC 8216bis) |
//! | [`ll_dash`] | Low-Latency DASH with CMAF chunked transfer encoding |
//! | [`drm_signaling`] | DRM system signaling (Widevine, FairPlay, PlayReady) |
//! | [`thumbnail_track`] | I-frame-only playlists and trick-play manifests |
//! | [`multi_audio`] | Multiple audio track variants and language management |
//! | [`subtitle_track`] | WebVTT subtitle segment packaging and manifest integration |
//! | [`stream_analytics`] | Viewer-side playback metrics and QoE aggregation |
//! | [`dvr_recorder`] | DVR sliding-window recorder with VOD playlist generation |

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(dead_code)]

pub mod adaptive_pipeline;
pub mod cmaf;
pub mod drm_signaling;
pub mod ll_dash;
pub mod ll_hls;
pub mod manifest_builder;
pub mod multi_audio;
pub mod multi_cdn;
pub mod scte35;
pub mod segment_manager;
pub mod stream_analytics;
pub mod stream_health;
pub mod stream_packager;
pub mod subtitle_track;
pub mod thumbnail_track;

// ─── Crate-level error type ───────────────────────────────────────────────────

/// Top-level error type for `oximedia-stream`.
#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    /// A binary parsing operation encountered invalid or truncated data.
    #[error("parse error: {0}")]
    ParseError(String),

    /// A CDN routing operation could not find an eligible provider.
    #[error("routing error: {0}")]
    RoutingError(String),

    /// An I/O error occurred while reading or writing segment data.
    #[error("I/O error: {0}")]
    IoError(String),

    /// A generic stream-processing error.
    #[error("stream error: {0}")]
    Generic(String),
}

// ─── Re-exports ───────────────────────────────────────────────────────────────

// adaptive_pipeline
pub use adaptive_pipeline::{
    AbrAlgorithm, AdaptivePipeline, BandwidthEstimator, QualityLadder, QualitySwitch, QualityTier,
    SwitchReason,
};

// segment_manager
pub use segment_manager::{MediaSegment, PrefetchConfig, SegmentManager, SegmentState};

// stream_health
pub use stream_health::{
    HealthIssue, QoeConfig, QoeScore, StreamHealthMonitor, StreamHealthReport,
};

// scte35
pub use scte35::{
    encode_bandwidth_reservation, encode_splice_insert, encode_splice_null, parse_splice_info,
    BreakDuration, ScheduledCommand, SpliceCommand, SpliceCommandType, SpliceDescriptor,
    SpliceInfoSection, SpliceInsert, SpliceScheduler, TimeSignal,
};

// multi_cdn
pub use multi_cdn::{CdnProvider, FailoverPolicy, MultiCdnRouter, RoutingStrategy};

// manifest_builder
pub use manifest_builder::{
    build_dash_mpd, build_master_playlist, build_media_playlist, DashMpd, DashRepresentation,
    HlsManifest, HlsSegment, ManifestFormat, SegmentTemplate, StreamVariant,
};

// stream_packager
pub use stream_packager::{
    pack_segment, FileSegmentWriter, MediaUnit, PackagedSegment, PackagerConfig, SegmentPackager,
    SegmentWriter, StreamType,
};

// cmaf
pub use cmaf::{CmafChunk, CmafMuxer};

// ll_hls
pub use ll_hls::{
    BlockingReloadRequest, HintType, LlHlsConfig, LlHlsPlaylist, LlHlsPlaylistState, LlHlsSegment,
    PartialSegment, PreloadHint,
};

// ll_dash
pub use ll_dash::{LlDashChunk, LlDashConfig, LlDashSegment, LlDashTimeline};

// drm_signaling
pub use drm_signaling::{DrmManifestBuilder, DrmSignal, DrmSystem};

// thumbnail_track
pub use thumbnail_track::{ImageFormat, ThumbnailSegment, ThumbnailTrack, ThumbnailTrackBuilder};

// multi_audio
pub use multi_audio::{AudioCodecId, AudioTrack, AudioTrackManager};

// subtitle_track
pub use subtitle_track::{
    SubtitleCue, SubtitlePackager, SubtitleSegment, SubtitleTrack, SubtitleTrackManager,
};

// stream_analytics
pub use stream_analytics::{PlaybackEvent, PlaybackStats, StreamAnalytics};

// dvr_recorder
pub mod dvr_recorder;
pub use dvr_recorder::{DvrConfig, DvrRecorder, DvrSegment};

// throughput_abr
pub mod throughput_abr;
pub use throughput_abr::{ThroughputAbr, ThroughputMeasurement};
