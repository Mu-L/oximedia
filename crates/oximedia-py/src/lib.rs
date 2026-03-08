#![allow(unexpected_cfgs)]
//! Python bindings for `OxiMedia` using `PyO3`.
//!
//! This crate provides Python bindings for the `OxiMedia` multimedia framework,
//! enabling decoding, encoding, and muxing/demuxing of royalty-free media formats.
//!
//! # Supported Codecs
//!
//! ## Video
//! - AV1 (decode & encode)
//! - VP9 (decode)
//! - VP8 (decode)
//!
//! ## Audio
//! - Opus (decode)
//!
//! ## Containers
//! - Matroska/WebM (demux & mux)
//! - Ogg (demux & mux)
//! - FLAC (demux)
//! - WAV (demux)
//!
//! # Example Usage (Python)
//!
//! ```python
//! import oximedia
//!
//! # Decode video
//! decoder = oximedia.Av1Decoder()
//! decoder.send_packet(packet_data, pts=0)
//! frame = decoder.receive_frame()
//!
//! # Encode video
//! config = oximedia.EncoderConfig(
//!     width=1920,
//!     height=1080,
//!     framerate=(30, 1),
//!     crf=28.0
//! )
//! encoder = oximedia.Av1Encoder(config)
//! encoder.send_frame(frame)
//! packet = encoder.receive_packet()
//!
//! # Demux container
//! demuxer = oximedia.MatroskaDemuxer("video.mkv")
//! demuxer.probe()
//! streams = demuxer.streams()
//! packet = demuxer.read_packet()
//! ```

#![allow(clippy::used_underscore_binding)]
#![allow(clippy::borrow_deref_ref)]

mod audio;
pub mod batch;
pub mod batch_bindings;
pub mod codec_info;
mod container;
mod error;
/// Structured error types with categories, severity, and batch collection.
pub mod error_types;
/// Filter graph node descriptions for Python-side graph building.
pub mod filter_bindings;
mod filters;
/// Media format information, container capabilities, and codec queries.
pub mod format_info;
/// Media content hashing and fingerprinting for Python bindings.
pub mod media_hash;
pub mod pipeline_bindings;
pub mod pipeline_builder;
mod probe;
/// Progress tracking for long-running Python operations.
pub mod progress_tracker;
/// Structured configuration sections and fluent builder for Python bindings.
pub mod py_config;
/// Typed error codes and Python-boundary error converter.
pub mod py_error;
/// Typed metadata fields and Python-interop converter.
pub mod py_metadata;
/// Streaming media reader utilities for Python bindings.
pub mod stream_reader;
pub mod timeline;
pub mod transcode_options;
mod types;
mod video;
pub mod video_bindings;
pub mod video_meta;

use pyo3::prelude::*;

/// `OxiMedia` Python module - Royalty-free multimedia processing library.
///
/// Provides video/audio encoding, decoding, and container muxing/demuxing
/// for patent-free codecs like AV1, VP9, VP8, Opus, and Vorbis.
#[pymodule]
fn oximedia(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Error types
    m.add("OxiMediaError", m.py().get_type::<error::OxiMediaError>())?;

    // Core types
    m.add_class::<types::PixelFormat>()?;
    m.add_class::<types::SampleFormat>()?;
    m.add_class::<types::ChannelLayout>()?;
    m.add_class::<types::VideoFrame>()?;
    m.add_class::<types::AudioFrame>()?;
    m.add_class::<types::EncoderConfig>()?;
    m.add_class::<types::EncoderPreset>()?;
    m.add_class::<types::Rational>()?;

    // Video codecs
    m.add_class::<video::Av1Decoder>()?;
    m.add_class::<video::Av1Encoder>()?;
    m.add_class::<video::Vp9Decoder>()?;
    m.add_class::<video::Vp8Decoder>()?;

    // Audio codecs
    m.add_class::<audio::OpusDecoder>()?;
    m.add_class::<audio::VorbisDecoder>()?;
    m.add_class::<audio::FlacDecoder>()?;
    m.add_class::<audio::OpusEncoderConfig>()?;
    m.add_class::<audio::OpusEncoder>()?;

    // Filter graph configuration types
    m.add_class::<filters::PyScaleConfig>()?;
    m.add_class::<filters::PyCropConfig>()?;
    m.add_class::<filters::PyVolumeConfig>()?;
    m.add_class::<filters::PyNormalizeConfig>()?;

    // Container
    m.add_class::<container::Packet>()?;
    m.add_class::<container::StreamInfo>()?;
    m.add_class::<container::MatroskaDemuxer>()?;
    m.add_class::<container::OggDemuxer>()?;
    m.add_class::<container::MatroskaMuxer>()?;
    m.add_class::<container::OggMuxer>()?;

    // Probe / media-info types
    m.add_class::<probe::PyVideoInfo>()?;
    m.add_class::<probe::PyAudioInfo>()?;
    m.add_class::<probe::PyStreamInfo>()?;
    m.add_class::<probe::PyMediaInfo>()?;

    Ok(())
}
