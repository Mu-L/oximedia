// Copyright 2025 OxiMedia Contributors
// Licensed under the Apache License, Version 2.0

//! Frame-level transcode orchestration: real decode → filter → encode.
//!
//! This module is what runs when a [`crate::Pipeline`] job requests a codec
//! change (or a filter that requires re-encoding). It builds the concrete
//! [`FrameDecoder`] / [`FilterGraph`] / [`FrameEncoder`] chain from the
//! adapters in [`crate::audio_adapters`] / [`crate::frame_adapters`], wires
//! it into a [`MultiTrackExecutor`] with the right container muxer, and
//! executes end-to-end.
//!
//! # Support matrix (everything else returns a descriptive error — output
//! is never fabricated)
//!
//! | Input          | Target codec       | Output container        |
//! |----------------|--------------------|-------------------------|
//! | WAV, FLAC      | FLAC               | `.flac`, `.mka`/`.mkv`  |
//! | WAV, FLAC      | PCM (s16le)        | `.wav`, `.mka`/`.mkv`   |
//! | WAV, FLAC      | ALAC               | `.caf`                  |
//! | Y4M (4:2:0)    | MJPEG              | `.mkv` (`V_MJPEG`)      |
//! | Y4M (4:2:0)    | APV                | `.mkv` (VFW/`apv1`)     |
//! | Y4M (4:2:0)    | MPEG-2 (intra)     | `.m2v`/`.mpg` (raw ES)  |
//! | Y4M (4:2:0)    | rawvideo           | `.y4m`                  |
//!
//! AV1/VP9/VP8/Opus/Vorbis/AAC/MP3 encode, FFV1/ProRes muxing, and
//! re-encoding from Matroska/Ogg inputs are not wired yet; each failure
//! names the codec and the reason.

#![allow(clippy::module_name_repetitions)]

use std::path::Path;

use crate::audio_adapters::{FlacFrameEncoder, PcmBufferFrameDecoder, PcmFrameEncoder};
use crate::frame_adapters::{
    CodecVideoFrameEncoder, FpsResamplingDecoder, RawVideoFrameEncoder, Y4mFrameDecoder,
};
use crate::multi_track::{MultiTrackExecutor, MultiTrackStats, PerTrack};
use crate::pipeline_context::{FilterGraph, Frame, FrameDecoder};
use crate::raw_sinks::{CafAlacFileMuxer, FlacFileMuxer, RawEsFileMuxer, Y4mFileMuxer};
use crate::{
    make_video_encoder, PipelineConfig, RateControlMode, Result, TranscodeError, VideoEncoderParams,
};

use oximedia_container::demux::wav::FmtChunk;
use oximedia_container::{
    demux::{Demuxer, WavDemuxer},
    mux::{MatroskaMuxer, MuxerConfig, WavFormatConfig, WavMuxer},
    ContainerFormat, Muxer, StreamInfo,
};
use oximedia_core::{CodecId, Rational};
use oximedia_io::FileSource;
use tracing::info;

// ─── Stats ────────────────────────────────────────────────────────────────────

/// Byte/frame totals from a frame-level execution, in the same shape the
/// packet-level remux loop reports.
#[derive(Debug, Clone, Default)]
pub(crate) struct FrameLevelStats {
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub video_frames: u64,
    pub audio_frames: u64,
}

// ─── Target codecs ────────────────────────────────────────────────────────────

/// Audio codec targets with real encoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioTarget {
    Copy,
    Flac,
    Pcm,
    Alac,
}

/// Video codec targets with real encoders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VideoTarget {
    Copy,
    Mjpeg,
    Apv,
    Mpeg2,
    Ffv1,
    ProRes,
    Raw,
}

fn is_copy_name(name: &str) -> bool {
    matches!(name, "copy" | "stream-copy" | "stream_copy")
}

/// Resolve an audio codec name, or explain exactly why it cannot be used.
fn parse_audio_target(name: Option<&str>) -> Result<AudioTarget> {
    let Some(name) = name else {
        return Ok(AudioTarget::Copy);
    };
    let lc = name.to_lowercase();
    match lc.as_str() {
        s if is_copy_name(s) => Ok(AudioTarget::Copy),
        "flac" => Ok(AudioTarget::Flac),
        "pcm" | "pcm_s16le" | "wav" => Ok(AudioTarget::Pcm),
        "alac" => Ok(AudioTarget::Alac),
        // TODO(0.2.x): re-enable Opus once a reference-verified encoder
        // exists — both current workspace Opus encoders fail external
        // decoder verification (see audio_adapters.rs).
        "opus" | "libopus" => Err(TranscodeError::Unsupported(
            "Opus encoding is not yet supported for transcode (no encoder \
             in this build passes reference-decoder verification); \
             supported audio codecs: flac, pcm, alac"
                .into(),
        )),
        "vorbis" | "libvorbis" => Err(TranscodeError::Unsupported(
            "Vorbis encoding is not yet supported for transcode; \
             supported audio codecs: flac, pcm, alac"
                .into(),
        )),
        "aac" | "libfdk_aac" => Err(TranscodeError::Unsupported(
            "AAC is patent-encumbered and not supported by OxiMedia; \
             supported audio codecs: flac, pcm, alac"
                .into(),
        )),
        "mp3" | "libmp3lame" | "lame" => Err(TranscodeError::Unsupported(
            "MP3 encoding is not supported by OxiMedia; \
             supported audio codecs: flac, pcm, alac"
                .into(),
        )),
        other => Err(TranscodeError::Unsupported(format!(
            "unknown audio codec '{other}'; supported: flac, pcm, alac"
        ))),
    }
}

/// Resolve a video codec name, or explain exactly why it cannot be used.
fn parse_video_target(name: Option<&str>) -> Result<VideoTarget> {
    let Some(name) = name else {
        return Ok(VideoTarget::Copy);
    };
    let lc = name.to_lowercase();
    match lc.as_str() {
        s if is_copy_name(s) => Ok(VideoTarget::Copy),
        "mjpeg" | "motion-jpeg" | "motion_jpeg" => Ok(VideoTarget::Mjpeg),
        "apv" => Ok(VideoTarget::Apv),
        "mpeg2" | "mpeg2video" => Ok(VideoTarget::Mpeg2),
        "ffv1" => Ok(VideoTarget::Ffv1),
        "prores" => Ok(VideoTarget::ProRes),
        "rawvideo" | "raw" | "yuv420p" => Ok(VideoTarget::Raw),
        "av1" | "libaom-av1" => Err(TranscodeError::Unsupported(
            "AV1 encoding does not produce real output yet (planned for 0.2.x); \
             supported video codecs: mjpeg, apv, mpeg2, rawvideo"
                .into(),
        )),
        "vp9" | "libvpx-vp9" => Err(TranscodeError::Unsupported(
            "VP9 encoding is not yet supported for transcode; \
             supported video codecs: mjpeg, apv, mpeg2, rawvideo"
                .into(),
        )),
        "vp8" | "libvpx" => Err(TranscodeError::Unsupported(
            "VP8 encoding is not yet supported for transcode; \
             supported video codecs: mjpeg, apv, mpeg2, rawvideo"
                .into(),
        )),
        other => Err(TranscodeError::Unsupported(format!(
            "unknown video codec '{other}'; supported: mjpeg, apv, mpeg2, rawvideo"
        ))),
    }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

/// Execute a frame-level (decode → filter → encode) transcode job.
///
/// `normalization_gain_db` is folded into the audio filter gain alongside
/// any explicit `-af volume` request from the config.
pub(crate) async fn execute_frame_level(
    config: &PipelineConfig,
    normalization_gain_db: f64,
) -> Result<FrameLevelStats> {
    let audio_target = parse_audio_target(config.audio_codec.as_deref())?;
    let video_target = parse_video_target(config.video_codec.as_deref())?;

    let in_format = crate::pipeline::detect_format(&config.input).await?;

    match in_format {
        ContainerFormat::Wav | ContainerFormat::Flac => {
            if video_target != VideoTarget::Copy {
                return Err(TranscodeError::InvalidInput(format!(
                    "video codec requested but '{}' contains no video stream",
                    config.input.display()
                )));
            }
            if config.video_scale.is_some() || config.output_fps.is_some() {
                return Err(TranscodeError::InvalidInput(
                    "video filters (-vf/--scale/-r) require a video stream".into(),
                ));
            }
            execute_audio_job(config, in_format, audio_target, normalization_gain_db).await
        }
        ContainerFormat::Y4m => {
            if audio_target != AudioTarget::Copy {
                return Err(TranscodeError::InvalidInput(
                    "audio codec requested but Y4M input carries no audio stream".into(),
                ));
            }
            execute_video_job(config, video_target).await
        }
        // TODO(0.2.x): wire Matroska/Ogg in-container decoders (blocked on
        // MatroskaDemuxer codec-id mappings for V_MJPEG et al. in
        // oximedia-container) so MKV(MJPEG) and similar inputs can be
        // re-encoded, not just stream-copied.
        other => Err(TranscodeError::Unsupported(format!(
            "re-encoding from {other:?} input is not yet supported \
             (in-container decoders are not wired); supported transcode \
             inputs: WAV/FLAC (audio), Y4M (video). Stream copy (no codec \
             change) still works for this input."
        ))),
    }
}

// ─── Trim wrapper (-ss / -t) ─────────────────────────────────────────────────

/// Applies `-ss`/`-t` at the frame level by PTS: frames before the start
/// point are dropped, frames at/after the stop point end the stream.
struct TrimDecoder {
    inner: Box<dyn FrameDecoder>,
    start_ms: i64,
    stop_ms: Option<i64>,
    done: bool,
}

impl TrimDecoder {
    fn wrap(
        inner: Box<dyn FrameDecoder>,
        start_secs: Option<f64>,
        duration_secs: Option<f64>,
    ) -> Box<dyn FrameDecoder> {
        let start = start_secs.unwrap_or(0.0).max(0.0);
        if start == 0.0 && duration_secs.is_none() {
            return inner;
        }
        let stop_ms = duration_secs.map(|d| ((start + d) * 1_000.0) as i64);
        Box::new(Self {
            inner,
            start_ms: (start * 1_000.0) as i64,
            stop_ms,
            done: false,
        })
    }
}

impl FrameDecoder for TrimDecoder {
    fn decode_next(&mut self) -> Option<Frame> {
        if self.done {
            return None;
        }
        loop {
            let mut frame = self.inner.decode_next()?;
            if let Some(stop) = self.stop_ms {
                if frame.pts_ms >= stop {
                    self.done = true;
                    return None;
                }
            }
            if frame.pts_ms < self.start_ms {
                continue;
            }
            // Re-base timestamps so output starts at zero.
            frame.pts_ms -= self.start_ms;
            return Some(frame);
        }
    }

    fn eof(&self) -> bool {
        self.done || self.inner.eof()
    }
}

// ─── Audio jobs ───────────────────────────────────────────────────────────────

/// Decoded interleaved 16-bit PCM plus stream parameters.
struct DecodedPcm {
    data: Vec<u8>,
    sample_rate: u32,
    channels: u16,
}

/// Convert a raw WAV data payload to interleaved i16 LE using the `fmt `
/// chunk description. 16-bit integer PCM passes through byte-exact.
fn wav_payload_to_i16(raw: &[u8], fmt: &FmtChunk) -> Result<Vec<u8>> {
    let bits = fmt.bits_per_sample;
    if fmt.is_integer_pcm() {
        match bits {
            16 => return Ok(raw.to_vec()),
            8 => {
                let mut out = Vec::with_capacity(raw.len() * 2);
                for &b in raw {
                    let v = (i16::from(b) - 128) << 8;
                    out.extend_from_slice(&v.to_le_bytes());
                }
                return Ok(out);
            }
            24 => {
                let mut out = Vec::with_capacity(raw.len() / 3 * 2);
                for c in raw.chunks_exact(3) {
                    // Take the top 16 bits of the 24-bit LE sample.
                    out.extend_from_slice(&[c[1], c[2]]);
                }
                return Ok(out);
            }
            32 => {
                let mut out = Vec::with_capacity(raw.len() / 2);
                for c in raw.chunks_exact(4) {
                    out.extend_from_slice(&[c[2], c[3]]);
                }
                return Ok(out);
            }
            other => {
                return Err(TranscodeError::Unsupported(format!(
                    "{other}-bit integer WAV input is not supported (8/16/24/32 are)"
                )))
            }
        }
    }
    if fmt.is_float() {
        match bits {
            32 => {
                let mut out = Vec::with_capacity(raw.len() / 2);
                for c in raw.chunks_exact(4) {
                    let s = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                    let v = (s * 32_767.0).round().clamp(-32_768.0, 32_767.0) as i16;
                    out.extend_from_slice(&v.to_le_bytes());
                }
                return Ok(out);
            }
            64 => {
                let mut out = Vec::with_capacity(raw.len() / 4);
                for c in raw.chunks_exact(8) {
                    let s = f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
                    let v = (s * 32_767.0).round().clamp(-32_768.0, 32_767.0) as i16;
                    out.extend_from_slice(&v.to_le_bytes());
                }
                return Ok(out);
            }
            other => {
                return Err(TranscodeError::Unsupported(format!(
                    "{other}-bit float WAV input is not supported (32/64 are)"
                )))
            }
        }
    }
    Err(TranscodeError::Unsupported(format!(
        "WAV format {:?} is not supported for re-encoding (integer/float PCM are)",
        fmt.format
    )))
}

/// Fully demux + decode a WAV file to interleaved i16 PCM.
async fn load_wav_pcm(path: &Path) -> Result<DecodedPcm> {
    let source = FileSource::open(path)
        .await
        .map_err(|e| TranscodeError::IoError(e.to_string()))?;
    let mut demuxer = WavDemuxer::new(source);
    demuxer
        .probe()
        .await
        .map_err(|e| TranscodeError::ContainerError(format!("WAV probe failed: {e}")))?;
    let fmt = demuxer
        .format_info()
        .cloned()
        .ok_or_else(|| TranscodeError::ContainerError("WAV file carries no fmt chunk".into()))?;

    let mut raw = Vec::new();
    loop {
        match demuxer.read_packet().await {
            Ok(pkt) => raw.extend_from_slice(&pkt.data),
            Err(e) if e.is_eof() => break,
            Err(e) => {
                return Err(TranscodeError::ContainerError(format!(
                    "WAV read failed: {e}"
                )))
            }
        }
    }

    let data = wav_payload_to_i16(&raw, &fmt)?;
    Ok(DecodedPcm {
        data,
        sample_rate: fmt.sample_rate,
        channels: fmt.channels,
    })
}

/// Fully decode a FLAC file to interleaved i16 PCM via the spec-compliant
/// decoder in [`crate::flac_decode`] (the workspace's other FLAC decoders
/// reject real-world encoder output).
async fn load_flac_pcm(path: &Path) -> Result<DecodedPcm> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| TranscodeError::IoError(format!("cannot read '{}': {e}", path.display())))?;

    let (params, samples) = crate::flac_decode::decode_flac_to_i16(&bytes)?;
    let mut pcm = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        pcm.extend_from_slice(&s.to_le_bytes());
    }
    Ok(DecodedPcm {
        data: pcm,
        sample_rate: params.sample_rate,
        channels: params.channels,
    })
}

/// Run one audio track through the executor with the given muxer.
async fn run_audio_track<M: Muxer>(
    muxer: M,
    decoder: Box<dyn FrameDecoder>,
    filters: FilterGraph,
    encoder: Box<dyn crate::pipeline_context::FrameEncoder>,
    stream: StreamInfo,
) -> Result<MultiTrackStats> {
    let mut executor = MultiTrackExecutor::new(muxer);
    executor.add_track(PerTrack::new_typed(0, decoder, filters, encoder, true));
    executor.execute(&[stream]).await
}

/// Execute an audio-only frame-level job: WAV/FLAC input → real re-encode.
async fn execute_audio_job(
    config: &PipelineConfig,
    in_format: ContainerFormat,
    target: AudioTarget,
    normalization_gain_db: f64,
) -> Result<FrameLevelStats> {
    // 1. Decode the input to interleaved i16 PCM.
    let pcm = match in_format {
        ContainerFormat::Wav => load_wav_pcm(&config.input).await?,
        ContainerFormat::Flac => load_flac_pcm(&config.input).await?,
        other => {
            return Err(TranscodeError::Unsupported(format!(
                "audio re-encode from {other:?} is not supported"
            )))
        }
    };
    let bytes_in = pcm.data.len() as u64;
    let (sample_rate, channels) = (pcm.sample_rate, pcm.channels);

    // Resolve a bare "copy" to a real target so filter-only jobs
    // (e.g. `-af volume=…` on a WAV) still re-encode honestly.
    let target = if target == AudioTarget::Copy {
        match audio_target_from_extension(&config.output) {
            Some(t) => t,
            None => {
                return Err(TranscodeError::InvalidInput(
                    "audio filters require re-encoding: specify an audio codec \
                     (flac, pcm, alac, opus) or use a .flac/.wav/.caf/.ogg output"
                        .into(),
                ))
            }
        }
    } else {
        target
    };

    // 2. Build decoder (+ trim) and the gain filter graph.
    let base = Box::new(PcmBufferFrameDecoder::new(pcm.data, sample_rate, channels)?);
    let decoder = TrimDecoder::wrap(base, config.start_time_secs, config.duration_secs);

    let gain_db = normalization_gain_db + config.audio_gain_db.unwrap_or(0.0);
    let mut filters = FilterGraph::new();
    if gain_db.abs() > 0.001 {
        info!("Applying audio gain of {gain_db:.2} dB");
        filters = filters.add_audio_gain_db(gain_db);
    }

    let ext = output_extension(&config.output);
    let timebase = Rational::new(1, 1_000);
    let mux_config = MuxerConfig::new().with_writing_app("OxiMedia-Transcode");

    // 3. Dispatch on (target codec, output container).
    let stats = match target {
        AudioTarget::Flac => {
            let encoder = FlacFrameEncoder::new(sample_rate, channels)?;
            let counter = encoder.sample_counter();
            let encoder = Box::new(encoder);
            let mut stream = StreamInfo::new(0, CodecId::Flac, timebase);
            stream.codec_params = oximedia_container::CodecParams::audio(
                sample_rate,
                u8::try_from(channels).unwrap_or(u8::MAX),
            );
            match ext.as_str() {
                "flac" => {
                    // Own sink (not the container FlacMuxer): STREAMINFO
                    // total-samples must be exact or reference decoders
                    // report a stream/header mismatch.
                    let muxer =
                        FlacFileMuxer::new(config.output.clone(), sample_rate, channels, counter);
                    run_audio_track(muxer, decoder, filters, encoder, stream).await?
                }
                "mka" | "mkv" => {
                    let sink = FileSource::create(&config.output)
                        .await
                        .map_err(|e| TranscodeError::IoError(e.to_string()))?;
                    let muxer = MatroskaMuxer::new(sink, mux_config);
                    run_audio_track(muxer, decoder, filters, encoder, stream).await?
                }
                other => {
                    return Err(TranscodeError::InvalidOutput(format!(
                        "FLAC audio cannot be written to a .{other} file; \
                         use .flac or .mka"
                    )))
                }
            }
        }
        AudioTarget::Pcm => {
            let encoder = Box::new(PcmFrameEncoder::new());
            let mut stream = StreamInfo::new(0, CodecId::Pcm, timebase);
            stream.codec_params = oximedia_container::CodecParams::audio(
                sample_rate,
                u8::try_from(channels).unwrap_or(u8::MAX),
            );
            match ext.as_str() {
                "wav" => {
                    let sink = FileSource::create(&config.output)
                        .await
                        .map_err(|e| TranscodeError::IoError(e.to_string()))?;
                    let wav_config = WavFormatConfig::pcm(sample_rate, channels, 16);
                    let muxer = WavMuxer::with_format(sink, mux_config, wav_config);
                    run_audio_track(muxer, decoder, filters, encoder, stream).await?
                }
                "mka" | "mkv" => {
                    let sink = FileSource::create(&config.output)
                        .await
                        .map_err(|e| TranscodeError::IoError(e.to_string()))?;
                    let muxer = MatroskaMuxer::new(sink, mux_config);
                    run_audio_track(muxer, decoder, filters, encoder, stream).await?
                }
                other => {
                    return Err(TranscodeError::InvalidOutput(format!(
                        "PCM audio cannot be written to a .{other} file; use .wav or .mka"
                    )))
                }
            }
        }
        AudioTarget::Alac => {
            run_alac_job(config, decoder, filters, sample_rate, channels, &ext).await?
        }
        AudioTarget::Copy => {
            // Both entry paths resolve `Copy` before this match; reaching
            // this arm would be an internal logic error — fail, don't panic.
            return Err(TranscodeError::PipelineError(
                "internal: audio copy target reached the encode dispatch".into(),
            ));
        }
    };

    info!(
        "Frame-level audio transcode complete: {} frames, {} bytes out",
        stats.total_encoded_frames, stats.total_encoded_bytes
    );

    Ok(FrameLevelStats {
        bytes_in,
        bytes_out: stats.total_encoded_bytes,
        video_frames: 0,
        audio_frames: stats.total_encoded_frames,
    })
}

/// Pick the natural audio target for an output extension.
fn audio_target_from_extension(path: &Path) -> Option<AudioTarget> {
    match output_extension(path).as_str() {
        "flac" => Some(AudioTarget::Flac),
        "wav" => Some(AudioTarget::Pcm),
        "caf" => Some(AudioTarget::Alac),
        _ => None,
    }
}

fn output_extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default()
}

async fn run_alac_job(
    config: &PipelineConfig,
    decoder: Box<dyn FrameDecoder>,
    filters: FilterGraph,
    sample_rate: u32,
    channels: u16,
    ext: &str,
) -> Result<MultiTrackStats> {
    use crate::audio_adapters::AlacFrameEncoder;

    if ext != "caf" {
        return Err(TranscodeError::InvalidOutput(format!(
            "ALAC output is written as a CAF file; use a .caf extension \
             (got .{ext})"
        )));
    }
    let encoder = AlacFrameEncoder::new(sample_rate, channels)?;
    let cookie = encoder.magic_cookie().to_vec();
    let mut muxer = CafAlacFileMuxer::new(config.output.clone(), sample_rate, channels, cookie);
    muxer.set_sample_counter(encoder.sample_counter());

    let mut stream = StreamInfo::new(0, CodecId::Alac, Rational::new(1, 1_000));
    stream.codec_params = oximedia_container::CodecParams::audio(
        sample_rate,
        u8::try_from(channels).unwrap_or(u8::MAX),
    );
    run_audio_track(muxer, decoder, filters, Box::new(encoder), stream).await
}

// ─── Video jobs ───────────────────────────────────────────────────────────────

/// Per-codec default quality when no CRF was given (matches each codec's
/// natural scale: MJPEG quality, APV qp, MPEG-2 qscale).
fn default_quality_for(target: VideoTarget) -> u8 {
    match target {
        VideoTarget::Mjpeg => 85,
        VideoTarget::Apv => 22,
        VideoTarget::Mpeg2 => 6,
        _ => 85,
    }
}

/// Resolve the requested output dimensions against the source, preserving
/// aspect for `-1` axes and rounding to even (YUV 4:2:0 requirement).
fn resolve_scale(src_w: u32, src_h: u32, spec: Option<&crate::ScaleSpec>) -> Result<(u32, u32)> {
    let Some(spec) = spec else {
        return Ok((src_w, src_h));
    };
    let even = |v: u32| (v.max(2) / 2) * 2;
    let (w, h) = match (spec.width, spec.height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => {
            let h = (u64::from(w) * u64::from(src_h) / u64::from(src_w.max(1))) as u32;
            (w, h)
        }
        (None, Some(h)) => {
            let w = (u64::from(h) * u64::from(src_w) / u64::from(src_h.max(1))) as u32;
            (w, h)
        }
        (None, None) => (src_w, src_h),
    };
    if w == 0 || h == 0 {
        return Err(TranscodeError::InvalidInput(format!(
            "scale target {w}x{h} is invalid"
        )));
    }
    Ok((even(w), even(h)))
}

/// Run one video track through the executor with the given muxer.
async fn run_video_track<M: Muxer>(
    muxer: M,
    decoder: Box<dyn FrameDecoder>,
    filters: FilterGraph,
    encoder: Box<dyn crate::pipeline_context::FrameEncoder>,
    stream: StreamInfo,
) -> Result<MultiTrackStats> {
    let mut executor = MultiTrackExecutor::new(muxer);
    executor.add_track(PerTrack::new_typed(0, decoder, filters, encoder, false));
    executor.execute(&[stream]).await
}

/// Execute a video frame-level job: Y4M input → real re-encode.
async fn execute_video_job(
    config: &PipelineConfig,
    target: VideoTarget,
) -> Result<FrameLevelStats> {
    let y4m = Y4mFrameDecoder::open(&config.input)?;
    let (src_w, src_h) = y4m.dimensions();
    let src_fps = y4m.fps();

    // Resolve `copy` for filter-only jobs (e.g. `-vf scale` on Y4M → Y4M).
    let target = if target == VideoTarget::Copy {
        if output_extension(&config.output) == "y4m" {
            VideoTarget::Raw
        } else {
            return Err(TranscodeError::InvalidInput(
                "video filters require re-encoding: specify a video codec \
                 (mjpeg, apv, mpeg2, rawvideo)"
                    .into(),
            ));
        }
    } else {
        target
    };

    let (out_w, out_h) = resolve_scale(src_w, src_h, config.video_scale.as_ref())?;
    let out_fps = config.output_fps.unwrap_or(src_fps);

    // Decoder chain: Y4M → (fps resample) → (trim).
    let mut decoder: Box<dyn FrameDecoder> = Box::new(y4m);
    if config.output_fps.is_some() && config.output_fps != Some(src_fps) {
        info!(
            "Frame-rate conversion: {}/{} → {}/{}",
            src_fps.0, src_fps.1, out_fps.0, out_fps.1
        );
        decoder = Box::new(FpsResamplingDecoder::new(decoder, src_fps, out_fps));
    }
    decoder = TrimDecoder::wrap(decoder, config.start_time_secs, config.duration_secs);

    let mut filters = FilterGraph::new();
    if (out_w, out_h) != (src_w, src_h) {
        info!("Scaling video {src_w}x{src_h} → {out_w}x{out_h}");
        filters = filters.add_video_scale(out_w, out_h);
    }

    let quality = config
        .quality
        .as_ref()
        .and_then(|q| {
            if let RateControlMode::Crf(v) = q.rate_control {
                Some(v)
            } else {
                None
            }
        })
        .unwrap_or_else(|| default_quality_for(target));

    let ext = output_extension(&config.output);
    let timebase = Rational::new(1, 1_000);
    let mux_config = MuxerConfig::new().with_writing_app("OxiMedia-Transcode");
    let bytes_in = u64::from(src_w) * u64::from(src_h) * 3 / 2; // per-frame estimate

    let stats = match target {
        VideoTarget::Mjpeg | VideoTarget::Apv => {
            if !matches!(ext.as_str(), "mkv" | "webm") {
                return Err(TranscodeError::InvalidOutput(format!(
                    "{target:?} video is written to Matroska; use a .mkv output (got .{ext})"
                )));
            }
            let codec_id = if target == VideoTarget::Mjpeg {
                CodecId::Mjpeg
            } else {
                CodecId::Apv
            };
            let params = VideoEncoderParams::new(out_w, out_h, quality)?;
            let inner = make_video_encoder(codec_id, &params)?;
            let encoder = Box::new(CodecVideoFrameEncoder::new(inner, out_w, out_h));
            let mut stream = StreamInfo::new(0, codec_id, timebase);
            stream.codec_params = oximedia_container::CodecParams::video(out_w, out_h);
            let sink = FileSource::create(&config.output)
                .await
                .map_err(|e| TranscodeError::IoError(e.to_string()))?;
            let muxer = MatroskaMuxer::new(sink, mux_config);
            run_video_track(muxer, decoder, filters, encoder, stream).await?
        }
        VideoTarget::Mpeg2 => {
            if !matches!(ext.as_str(), "m2v" | "mpg" | "mpeg" | "mpv") {
                return Err(TranscodeError::InvalidOutput(format!(
                    "MPEG-2 video is written as a raw elementary stream; \
                     use a .m2v or .mpg output (got .{ext}). Matroska muxing \
                     of MPEG-2 is not supported yet."
                )));
            }
            let params = VideoEncoderParams::new(out_w, out_h, quality)?;
            let inner = make_video_encoder(CodecId::Mpeg2, &params)?;
            let encoder = Box::new(CodecVideoFrameEncoder::new(inner, out_w, out_h));
            let mut stream = StreamInfo::new(0, CodecId::Mpeg2, timebase);
            stream.codec_params = oximedia_container::CodecParams::video(out_w, out_h);
            let muxer = RawEsFileMuxer::new(config.output.clone());
            run_video_track(muxer, decoder, filters, encoder, stream).await?
        }
        VideoTarget::Raw => {
            if ext != "y4m" {
                return Err(TranscodeError::InvalidOutput(format!(
                    "rawvideo output is written as Y4M; use a .y4m output (got .{ext})"
                )));
            }
            let encoder = Box::new(RawVideoFrameEncoder::new());
            let mut stream = StreamInfo::new(0, CodecId::RawVideo, timebase);
            stream.codec_params = oximedia_container::CodecParams::video(out_w, out_h);
            let muxer = Y4mFileMuxer::new(config.output.clone(), out_w, out_h, out_fps);
            run_video_track(muxer, decoder, filters, encoder, stream).await?
        }
        VideoTarget::Ffv1 => {
            // TODO(0.2.x): FFV1 encode works (see make_video_encoder), but no
            // container in oximedia-container writes the V_FFV1 codec id +
            // required CodecPrivate extradata yet.
            return Err(TranscodeError::Unsupported(
                "FFV1 transcode is not yet supported: the Matroska muxer \
                 cannot carry FFV1 (V_FFV1 + CodecPrivate) yet; \
                 supported video codecs: mjpeg, apv, mpeg2, rawvideo"
                    .into(),
            ));
        }
        VideoTarget::ProRes => {
            // TODO(0.2.x): ProRes needs a 10-bit 4:2:2 frame path (the
            // pipeline is 8-bit 4:2:0) and a container that can label it.
            return Err(TranscodeError::Unsupported(
                "ProRes transcode is not yet supported: it requires a 10-bit \
                 4:2:2 pipeline; supported video codecs: mjpeg, apv, mpeg2, rawvideo"
                    .into(),
            ));
        }
        VideoTarget::Copy => {
            // Resolved to a concrete target before this match; reaching
            // this arm would be an internal logic error — fail, don't panic.
            return Err(TranscodeError::PipelineError(
                "internal: video copy target reached the encode dispatch".into(),
            ));
        }
    };

    info!(
        "Frame-level video transcode complete: {} frames, {} bytes out",
        stats.total_encoded_frames, stats.total_encoded_bytes
    );

    Ok(FrameLevelStats {
        bytes_in: bytes_in.saturating_mul(stats.total_encoded_frames),
        bytes_out: stats.total_encoded_bytes,
        video_frames: stats.total_encoded_frames,
        audio_frames: 0,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_audio_target_names() {
        assert_eq!(
            parse_audio_target(Some("flac")).expect("flac"),
            AudioTarget::Flac
        );
        assert_eq!(
            parse_audio_target(Some("pcm_s16le")).expect("pcm"),
            AudioTarget::Pcm
        );
        assert_eq!(
            parse_audio_target(Some("ALAC")).expect("alac"),
            AudioTarget::Alac
        );
        assert_eq!(
            parse_audio_target(Some("copy")).expect("copy"),
            AudioTarget::Copy
        );
        assert_eq!(parse_audio_target(None).expect("none"), AudioTarget::Copy);
    }

    #[test]
    fn test_parse_audio_target_unsupported_named() {
        let msg = parse_audio_target(Some("vorbis"))
            .expect_err("vorbis must be rejected")
            .to_string();
        assert!(msg.contains("Vorbis"), "must name the codec: {msg}");
        let msg = parse_audio_target(Some("aac"))
            .expect_err("aac must be rejected")
            .to_string();
        assert!(msg.contains("AAC"), "must name the codec: {msg}");
    }

    #[test]
    fn test_parse_video_target_unsupported_named() {
        for (name, needle) in [("vp9", "VP9"), ("av1", "AV1"), ("vp8", "VP8")] {
            let msg = parse_video_target(Some(name))
                .expect_err("must be rejected")
                .to_string();
            assert!(
                msg.contains(needle),
                "error for {name} must name the codec: {msg}"
            );
            assert!(
                msg.contains("mjpeg"),
                "error must list supported codecs: {msg}"
            );
            assert!(
                !msg.contains("MultiTrackExecutor"),
                "error must not leak internals: {msg}"
            );
        }
    }

    #[test]
    fn test_resolve_scale_aspect_and_even() {
        // Full spec.
        let spec = crate::ScaleSpec {
            width: Some(320),
            height: Some(240),
        };
        assert_eq!(
            resolve_scale(640, 480, Some(&spec)).expect("scale"),
            (320, 240)
        );
        // -1 height keeps aspect (and rounds to even).
        let spec = crate::ScaleSpec {
            width: Some(320),
            height: None,
        };
        assert_eq!(
            resolve_scale(640, 480, Some(&spec)).expect("scale"),
            (320, 240)
        );
        // Odd targets round down to even.
        let spec = crate::ScaleSpec {
            width: Some(321),
            height: Some(241),
        };
        assert_eq!(
            resolve_scale(640, 480, Some(&spec)).expect("scale"),
            (320, 240)
        );
        // No spec = source dims.
        assert_eq!(resolve_scale(640, 480, None).expect("none"), (640, 480));
    }

    #[test]
    fn test_wav_payload_conversions() {
        fn fmt(bits: u16, float: bool) -> FmtChunk {
            FmtChunk {
                format: if float {
                    oximedia_container::demux::wav::WavFormat::IeeeFloat
                } else {
                    oximedia_container::demux::wav::WavFormat::Pcm
                },
                channels: 1,
                sample_rate: 48_000,
                byte_rate: 0,
                block_align: 0,
                bits_per_sample: bits,
                extension: None,
            }
        }

        // 16-bit passthrough is byte-exact.
        let raw = vec![0x34, 0x12, 0xCC, 0xED];
        assert_eq!(
            wav_payload_to_i16(&raw, &fmt(16, false)).expect("16-bit"),
            raw
        );

        // f32 1.0 → 32767.
        let raw = 1.0f32.to_le_bytes().to_vec();
        let out = wav_payload_to_i16(&raw, &fmt(32, true)).expect("f32");
        assert_eq!(i16::from_le_bytes([out[0], out[1]]), 32_767);

        // 24-bit → top 16 bits.
        let raw = vec![0xFF, 0x34, 0x12];
        let out = wav_payload_to_i16(&raw, &fmt(24, false)).expect("24-bit");
        assert_eq!(i16::from_le_bytes([out[0], out[1]]), 0x1234);

        // 12-bit → honest error.
        assert!(wav_payload_to_i16(&[0, 0], &fmt(12, false)).is_err());
    }

    #[test]
    fn test_trim_decoder_by_pts() {
        struct Seq(std::collections::VecDeque<Frame>);
        impl FrameDecoder for Seq {
            fn decode_next(&mut self) -> Option<Frame> {
                self.0.pop_front()
            }
            fn eof(&self) -> bool {
                self.0.is_empty()
            }
        }

        // Frames at 0,100,…,900 ms; trim -ss 0.25 -t 0.3 → 300..549 ms
        // window (frames 300,400,500), re-based to start at ~50 ms.
        let frames: std::collections::VecDeque<Frame> = (0..10)
            .map(|i| Frame::audio(vec![i as u8; 4], i64::from(i) * 100))
            .collect();
        let mut dec = TrimDecoder::wrap(Box::new(Seq(frames)), Some(0.25), Some(0.3));
        let mut kept = Vec::new();
        while let Some(f) = dec.decode_next() {
            kept.push(f);
        }
        assert_eq!(kept.len(), 3, "expected frames at 300/400/500 ms");
        assert_eq!(kept[0].data[0], 3);
        assert_eq!(kept[0].pts_ms, 50, "PTS must be re-based to the seek point");
        assert!(dec.eof());
    }
}
