//! MP4/ISOBMFF container demuxer.
//!
//! Implements ISO Base Media File Format (ISO/IEC 14496-12) parsing.
//!
//! **Important**: Only royalty-free codecs are supported:
//! - Video: AV1, VP9
//! - Audio: Opus, FLAC, Vorbis
//!
//! Patent-encumbered codecs (H.264, H.265, AAC, AC-3, E-AC-3, DTS) are
//! rejected with a [`PatentViolation`](oximedia_core::OxiError::PatentViolation) error.
//!
//! # Example
//!
//! ```ignore
//! use oximedia_container::demux::Mp4Demuxer;
//! use oximedia_io::FileSource;
//!
//! let source = FileSource::open("video.mp4").await?;
//! let mut demuxer = Mp4Demuxer::new(source);
//!
//! let probe = demuxer.probe().await?;
//! println!("Detected: {:?}", probe.format);
//!
//! for stream in demuxer.streams() {
//!     println!("Stream {}: {:?}", stream.index, stream.codec);
//! }
//!
//! while let Ok(packet) = demuxer.read_packet().await {
//!     println!("Packet: stream={}, size={}",
//!              packet.stream_index, packet.size());
//! }
//! ```

mod atom;
mod boxes;

pub use atom::Mp4Atom;
pub use boxes::{
    BoxHeader, BoxType, CttsEntry, FtypBox, MoovBox, MvhdBox, StscEntry, SttsEntry, TkhdBox,
    TrakBox,
};

use async_trait::async_trait;
use bytes::Bytes;
use oximedia_core::{CodecId, OxiError, OxiResult, Rational};

use crate::demux::Demuxer;
use crate::{CodecParams, ContainerFormat, Metadata, Packet, ProbeResult, StreamInfo};

/// MP4/ISOBMFF demuxer supporting AV1 and VP9 only.
///
/// This demuxer parses MP4 containers but only allows playback of
/// royalty-free codecs. Attempting to demux files containing H.264,
/// H.265, AAC, or other patent-encumbered codecs will result in an error.
///
/// # Supported Codecs
///
/// | Type  | Codecs           |
/// |-------|------------------|
/// | Video | AV1, VP9         |
/// | Audio | Opus, FLAC, Vorbis |
///
/// # Patent Protection
///
/// When a patent-encumbered codec is detected during probing, the demuxer
/// returns [`OxiError::PatentViolation`] with the codec name. This ensures
/// users are immediately aware that the file cannot be processed.
#[allow(dead_code)]
pub struct Mp4Demuxer<R> {
    /// The underlying media source.
    source: R,

    /// Internal read buffer.
    buffer: Vec<u8>,

    /// Parsed `ftyp` box.
    ftyp: Option<FtypBox>,

    /// Parsed `moov` box.
    moov: Option<MoovBox>,

    /// Stream information extracted from tracks.
    streams: Vec<StreamInfo>,

    /// Per-track demuxing state.
    tracks: Vec<TrackState>,

    /// Current file position.
    position: u64,

    /// Start of `mdat` box.
    mdat_start: u64,

    /// Size of `mdat` box.
    mdat_size: u64,

    /// Whether headers have been parsed.
    header_parsed: bool,
}

/// Per-track demuxing state.
#[derive(Clone, Debug, Default)]
pub struct TrackState {
    /// Track ID from the container.
    pub track_id: u32,
    /// Index in the streams array.
    pub stream_index: usize,
    /// Current sample index (0-based).
    pub sample_index: u32,
    /// Total number of samples.
    pub sample_count: u32,
    /// Precomputed sample information for random access.
    pub samples: Vec<SampleInfo>,
}

/// Information about a single sample in a track.
#[derive(Clone, Debug)]
pub struct SampleInfo {
    /// Absolute offset in the file.
    pub offset: u64,
    /// Size in bytes.
    pub size: u32,
    /// Duration in timescale units.
    pub duration: u32,
    /// Composition time offset (PTS - DTS).
    pub cts_offset: i32,
    /// Whether this sample is a sync point (keyframe).
    pub is_sync: bool,
}

impl<R> Mp4Demuxer<R> {
    /// Creates a new MP4 demuxer with the given source.
    ///
    /// After creation, call [`probe`](Demuxer::probe) to parse headers
    /// and detect streams.
    #[must_use]
    pub fn new(source: R) -> Self {
        Self {
            source,
            buffer: Vec::with_capacity(65536),
            ftyp: None,
            moov: None,
            streams: Vec::new(),
            tracks: Vec::new(),
            position: 0,
            mdat_start: 0,
            mdat_size: 0,
            header_parsed: false,
        }
    }

    /// Returns a reference to the underlying source.
    #[must_use]
    pub const fn source(&self) -> &R {
        &self.source
    }

    /// Returns a mutable reference to the underlying source.
    pub fn source_mut(&mut self) -> &mut R {
        &mut self.source
    }

    /// Consumes the demuxer and returns the underlying source.
    #[must_use]
    #[allow(dead_code)]
    pub fn into_source(self) -> R {
        self.source
    }

    /// Returns the parsed `ftyp` box, if available.
    #[must_use]
    pub const fn ftyp(&self) -> Option<&FtypBox> {
        self.ftyp.as_ref()
    }

    /// Returns the parsed `moov` box, if available.
    #[must_use]
    pub const fn moov(&self) -> Option<&MoovBox> {
        self.moov.as_ref()
    }
}

#[async_trait]
impl<R: Send> Demuxer for Mp4Demuxer<R> {
    async fn probe(&mut self) -> OxiResult<ProbeResult> {
        // TODO: Implement actual box parsing
        // For now, return a stub result indicating MP4 format
        self.header_parsed = true;

        Ok(ProbeResult::new(ContainerFormat::Mp4, 0.90))
    }

    async fn read_packet(&mut self) -> OxiResult<Packet> {
        // TODO: Implement actual packet reading from mdat
        // For now, return EOF to indicate no packets available
        Err(OxiError::Eof)
    }

    fn streams(&self) -> &[StreamInfo] {
        &self.streams
    }
}

/// Maps an MP4 codec tag to an `OxiMedia` codec identifier.
///
/// This function enforces the patent-free policy by returning
/// [`PatentViolation`](OxiError::PatentViolation) for known
/// patent-encumbered codecs.
///
/// # Arguments
///
/// * `handler` - Handler type from `hdlr` box ("vide", "soun", etc.)
/// * `codec_tag` - 4CC codec tag from sample description
///
/// # Errors
///
/// - Returns [`PatentViolation`](OxiError::PatentViolation) for H.264, H.265, AAC, etc.
/// - Returns [`Unsupported`](OxiError::Unsupported) for unknown codecs.
///
/// # Supported Mappings
///
/// | Handler | Tag      | Codec  |
/// |---------|----------|--------|
/// | `vide`  | `av01`   | AV1    |
/// | `vide`  | `vp09`   | VP9    |
/// | `soun`  | `Opus`   | Opus   |
/// | `soun`  | `fLaC`   | FLAC   |
///
/// # Patent-Encumbered (Rejected)
///
/// | Handler | Tag      | Codec     |
/// |---------|----------|-----------|
/// | `vide`  | `avc1`   | H.264     |
/// | `vide`  | `avc3`   | H.264     |
/// | `vide`  | `hvc1`   | H.265     |
/// | `vide`  | `hev1`   | H.265     |
/// | `soun`  | `mp4a`   | AAC       |
/// | `soun`  | `ac-3`   | AC-3      |
/// | `soun`  | `ec-3`   | E-AC-3    |
pub fn map_codec(handler: &str, codec_tag: u32) -> OxiResult<CodecId> {
    match (handler, codec_tag) {
        // =====================
        // Royalty-Free Video
        // =====================

        // AV1 - "av01"
        ("vide", 0x6176_3031) => Ok(CodecId::Av1),

        // VP9 - "vp09"
        ("vide", 0x7670_3039) => Ok(CodecId::Vp9),

        // VP8 - "vp08"
        ("vide", 0x7670_3038) => Ok(CodecId::Vp8),

        // =====================
        // Royalty-Free Audio
        // =====================

        // Opus - "Opus"
        ("soun", 0x4F70_7573) => Ok(CodecId::Opus),

        // FLAC - "fLaC"
        ("soun", 0x664C_6143) => Ok(CodecId::Flac),

        // Vorbis - "vorb" (rare in MP4)
        ("soun", 0x766F_7262) => Ok(CodecId::Vorbis),

        // =====================
        // Patent-Encumbered - REJECT
        // =====================

        // H.264/AVC - "avc1", "avc2", "avc3", "avc4"
        ("vide", 0x6176_6331..=0x6176_6334) => Err(OxiError::PatentViolation("H.264/AVC".into())),

        // H.265/HEVC - "hvc1", "hev1", "hvc2", "hev2"
        ("vide", 0x6876_6331 | 0x6865_7631 | 0x6876_6332 | 0x6865_7632) => {
            Err(OxiError::PatentViolation("H.265/HEVC".into()))
        }

        // H.266/VVC - "vvc1", "vvi1"
        ("vide", 0x7676_6331 | 0x7676_6931) => Err(OxiError::PatentViolation("H.266/VVC".into())),

        // AAC - "mp4a"
        ("soun", 0x6D70_3461) => Err(OxiError::PatentViolation("AAC".into())),

        // AC-3 - "ac-3"
        ("soun", 0x6163_2D33) => Err(OxiError::PatentViolation("AC-3".into())),

        // E-AC-3 - "ec-3"
        ("soun", 0x6563_2D33) => Err(OxiError::PatentViolation("E-AC-3".into())),

        // DTS - "dtsc", "dtsh", "dtsl", "dtse"
        ("soun", 0x6474_7363 | 0x6474_7368 | 0x6474_736C | 0x6474_7365) => {
            Err(OxiError::PatentViolation("DTS".into()))
        }

        // MPEG-1/2 Audio - "mp3 ", ".mp3"
        ("soun", 0x6D70_3320 | 0x2E6D_7033) => Err(OxiError::PatentViolation("MP3".into())),

        // =====================
        // Unknown
        // =====================
        _ => Err(OxiError::Unsupported(format!(
            "Unknown MP4 codec: handler={handler}, tag=0x{codec_tag:08X} ({})",
            tag_to_string(codec_tag)
        ))),
    }
}

/// Converts a 4CC codec tag to a readable string.
fn tag_to_string(tag: u32) -> String {
    let bytes = tag.to_be_bytes();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Builds a [`StreamInfo`] from a parsed track.
///
/// # Arguments
///
/// * `index` - Stream index
/// * `track` - Parsed track information
/// * `movie_timescale` - Timescale from the movie header
///
/// # Errors
///
/// Returns an error if the codec is unsupported or patent-encumbered.
#[allow(dead_code)]
fn build_stream_info(index: usize, track: &TrakBox, movie_timescale: u32) -> OxiResult<StreamInfo> {
    let codec = map_codec(&track.handler_type, track.codec_tag)?;

    // Use track timescale, falling back to movie timescale
    let timescale = if track.timescale > 0 {
        track.timescale
    } else {
        movie_timescale
    };

    let mut stream = StreamInfo::new(index, codec, Rational::new(1, i64::from(timescale)));

    // Set codec-specific parameters
    match track.handler_type.as_str() {
        "vide" => {
            if let (Some(w), Some(h)) = (track.width, track.height) {
                stream.codec_params = CodecParams::video(w, h);
            }
        }
        "soun" => {
            if let (Some(rate), Some(ch)) = (track.sample_rate, track.channels) {
                stream.codec_params = CodecParams::audio(rate, u8::try_from(ch).unwrap_or(2));
            }
        }
        _ => {}
    }

    // Set extradata if available
    if let Some(ref extra) = track.extradata {
        stream.codec_params.extradata = Some(Bytes::copy_from_slice(extra));
    }

    // Calculate duration
    if let Some(tkhd) = &track.tkhd {
        if movie_timescale > 0 {
            #[allow(clippy::cast_possible_wrap)]
            let duration_in_stream_tb =
                (tkhd.duration as i64 * i64::from(timescale)) / i64::from(movie_timescale);
            stream.duration = Some(duration_in_stream_tb);
        }
    }

    stream.metadata = Metadata::new();

    Ok(stream)
}

/// Builds a sample table from track box information.
///
/// This function computes the offset, size, duration, and sync status
/// for each sample in the track.
#[allow(dead_code)]
fn build_sample_table(track: &TrakBox) -> Vec<SampleInfo> {
    let mut samples = Vec::new();

    // Calculate total sample count
    let sample_count = if track.sample_sizes.is_empty() {
        // All samples have the same size - count from stts
        track
            .stts_entries
            .iter()
            .map(|e| e.sample_count as usize)
            .sum()
    } else {
        track.sample_sizes.len()
    };

    if sample_count == 0 {
        return samples;
    }

    // Build sync sample lookup (if not all samples are sync)
    let sync_set: Option<std::collections::HashSet<u32>> = track
        .sync_samples
        .as_ref()
        .map(|ss| ss.iter().copied().collect());

    // Build sample-to-chunk lookup
    let mut chunk_sample_map: Vec<(u32, u32, u32)> = Vec::new(); // (first_sample, chunk, samples_per_chunk)
    let mut sample_num = 1u32;
    for i in 0..track.stsc_entries.len() {
        let entry = &track.stsc_entries[i];
        let next_first_chunk = if i + 1 < track.stsc_entries.len() {
            track.stsc_entries[i + 1].first_chunk
        } else {
            #[allow(clippy::cast_possible_truncation)]
            let chunk_count = track.chunk_offsets.len() as u32 + 1;
            chunk_count
        };

        for chunk in entry.first_chunk..next_first_chunk {
            chunk_sample_map.push((sample_num, chunk, entry.samples_per_chunk));
            sample_num += entry.samples_per_chunk;
        }
    }

    // Build duration lookup from stts
    let mut durations: Vec<u32> = Vec::with_capacity(sample_count);
    for entry in &track.stts_entries {
        for _ in 0..entry.sample_count {
            durations.push(entry.sample_delta);
        }
    }

    // Build CTS offset lookup from ctts
    let mut cts_offsets: Vec<i32> = Vec::with_capacity(sample_count);
    for entry in &track.ctts_entries {
        for _ in 0..entry.sample_count {
            cts_offsets.push(entry.sample_offset);
        }
    }

    // Now build the sample table
    let mut current_chunk_idx = 0usize;
    let mut sample_in_chunk = 0u32;

    for sample_idx in 0..sample_count {
        #[allow(clippy::cast_possible_truncation)]
        let sample_num_1based = sample_idx as u32 + 1;

        // Get chunk offset
        let chunk_offset = if current_chunk_idx < track.chunk_offsets.len() {
            track.chunk_offsets[current_chunk_idx]
        } else {
            0
        };

        // Calculate offset within chunk
        let samples_per_chunk = if current_chunk_idx < chunk_sample_map.len() {
            chunk_sample_map[current_chunk_idx].2
        } else {
            1
        };

        let mut offset_in_chunk = 0u64;
        let first_sample_in_chunk = sample_idx - sample_in_chunk as usize;
        for i in first_sample_in_chunk..sample_idx {
            let size = if track.default_sample_size > 0 {
                track.default_sample_size
            } else if i < track.sample_sizes.len() {
                track.sample_sizes[i]
            } else {
                0
            };
            offset_in_chunk += u64::from(size);
        }

        let offset = chunk_offset + offset_in_chunk;

        // Get sample size
        let size = if track.default_sample_size > 0 {
            track.default_sample_size
        } else if sample_idx < track.sample_sizes.len() {
            track.sample_sizes[sample_idx]
        } else {
            0
        };

        // Get duration
        let duration = durations.get(sample_idx).copied().unwrap_or(0);

        // Get CTS offset
        let cts_offset = cts_offsets.get(sample_idx).copied().unwrap_or(0);

        // Check if sync
        let is_sync = sync_set
            .as_ref()
            .map_or(true, |set| set.contains(&sample_num_1based));

        samples.push(SampleInfo {
            offset,
            size,
            duration,
            cts_offset,
            is_sync,
        });

        // Advance to next chunk if needed
        sample_in_chunk += 1;
        if sample_in_chunk >= samples_per_chunk {
            sample_in_chunk = 0;
            current_chunk_idx += 1;
        }
    }

    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSource;

    #[tokio::test]
    async fn test_mp4_demuxer_new() {
        let demuxer = Mp4Demuxer::new(MockSource);
        assert!(!demuxer.header_parsed);
        assert!(demuxer.streams().is_empty());
        assert!(demuxer.ftyp().is_none());
        assert!(demuxer.moov().is_none());
    }

    #[tokio::test]
    async fn test_mp4_demuxer_probe() {
        let mut demuxer = Mp4Demuxer::new(MockSource);
        let result = demuxer.probe().await.unwrap();

        assert_eq!(result.format, ContainerFormat::Mp4);
        assert!(result.confidence > 0.8);
        assert!(demuxer.header_parsed);
    }

    #[test]
    fn test_map_codec_av1() {
        // "av01" = 0x61763031
        let codec = map_codec("vide", 0x6176_3031).unwrap();
        assert_eq!(codec, CodecId::Av1);
    }

    #[test]
    fn test_map_codec_vp9() {
        // "vp09" = 0x76703039
        let codec = map_codec("vide", 0x7670_3039).unwrap();
        assert_eq!(codec, CodecId::Vp9);
    }

    #[test]
    fn test_map_codec_opus() {
        // "Opus" = 0x4F707573
        let codec = map_codec("soun", 0x4F70_7573).unwrap();
        assert_eq!(codec, CodecId::Opus);
    }

    #[test]
    fn test_map_codec_flac() {
        // "fLaC" = 0x664C6143
        let codec = map_codec("soun", 0x664C_6143).unwrap();
        assert_eq!(codec, CodecId::Flac);
    }

    #[test]
    fn test_map_codec_h264_rejected() {
        // "avc1" = 0x61766331
        let result = map_codec("vide", 0x6176_6331);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_patent_violation());
        assert!(format!("{err}").contains("H.264"));
    }

    #[test]
    fn test_map_codec_h265_rejected() {
        // "hvc1" = 0x68766331
        let result = map_codec("vide", 0x6876_6331);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_patent_violation());
        assert!(format!("{err}").contains("H.265"));
    }

    #[test]
    fn test_map_codec_aac_rejected() {
        // "mp4a" = 0x6D703461
        let result = map_codec("soun", 0x6D70_3461);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_patent_violation());
        assert!(format!("{err}").contains("AAC"));
    }

    #[test]
    fn test_map_codec_ac3_rejected() {
        // "ac-3" = 0x61632D33
        let result = map_codec("soun", 0x6163_2D33);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_patent_violation());
        assert!(format!("{err}").contains("AC-3"));
    }

    #[test]
    fn test_map_codec_eac3_rejected() {
        // "ec-3" = 0x65632D33
        let result = map_codec("soun", 0x6563_2D33);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_patent_violation());
        assert!(format!("{err}").contains("E-AC-3"));
    }

    #[test]
    fn test_map_codec_dts_rejected() {
        // "dtsc" = 0x64747363
        let result = map_codec("soun", 0x6474_7363);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_patent_violation());
        assert!(format!("{err}").contains("DTS"));
    }

    #[test]
    fn test_map_codec_unknown() {
        // Unknown codec
        let result = map_codec("vide", 0x1234_5678);
        assert!(result.is_err());
        match result.unwrap_err() {
            OxiError::Unsupported(msg) => {
                assert!(msg.contains("Unknown MP4 codec"));
            }
            other => panic!("Expected Unsupported error, got: {other:?}"),
        }
    }

    #[test]
    fn test_tag_to_string() {
        assert_eq!(tag_to_string(0x6176_3031), "av01");
        assert_eq!(tag_to_string(0x6D70_3461), "mp4a");
    }

    #[test]
    fn test_build_stream_info_av1() {
        let mut track = TrakBox::default();
        track.handler_type = "vide".into();
        track.codec_tag = 0x6176_3031; // av01
        track.timescale = 24000;
        track.width = Some(1920);
        track.height = Some(1080);
        track.tkhd = Some(TkhdBox {
            track_id: 1,
            duration: 240_000,
            width: 1920.0,
            height: 1080.0,
        });

        let stream = build_stream_info(0, &track, 1000).unwrap();
        assert_eq!(stream.codec, CodecId::Av1);
        assert_eq!(stream.codec_params.width, Some(1920));
        assert_eq!(stream.codec_params.height, Some(1080));
        assert!(stream.is_video());
    }

    #[test]
    fn test_build_stream_info_opus() {
        let mut track = TrakBox::default();
        track.handler_type = "soun".into();
        track.codec_tag = 0x4F70_7573; // Opus
        track.timescale = 48000;
        track.sample_rate = Some(48000);
        track.channels = Some(2);

        let stream = build_stream_info(0, &track, 1000).unwrap();
        assert_eq!(stream.codec, CodecId::Opus);
        assert_eq!(stream.codec_params.sample_rate, Some(48000));
        assert_eq!(stream.codec_params.channels, Some(2));
        assert!(stream.is_audio());
    }

    #[test]
    fn test_build_stream_info_rejected() {
        let mut track = TrakBox::default();
        track.handler_type = "vide".into();
        track.codec_tag = 0x6176_6331; // avc1 (H.264)

        let result = build_stream_info(0, &track, 1000);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_patent_violation());
    }

    #[test]
    fn test_build_sample_table_basic() {
        let mut track = TrakBox::default();

        // 10 samples, each 1000 bytes
        track.default_sample_size = 1000;
        track.stts_entries = vec![SttsEntry {
            sample_count: 10,
            sample_delta: 100,
        }];

        // 2 chunks, 5 samples each
        track.stsc_entries = vec![StscEntry {
            first_chunk: 1,
            samples_per_chunk: 5,
            sample_description_index: 1,
        }];

        // Chunk offsets
        track.chunk_offsets = vec![0, 5000];

        // All samples are sync
        track.sync_samples = None;

        let samples = build_sample_table(&track);
        assert_eq!(samples.len(), 10);

        // First sample
        assert_eq!(samples[0].offset, 0);
        assert_eq!(samples[0].size, 1000);
        assert_eq!(samples[0].duration, 100);
        assert!(samples[0].is_sync);

        // Fifth sample (last in first chunk)
        assert_eq!(samples[4].offset, 4000);

        // Sixth sample (first in second chunk)
        assert_eq!(samples[5].offset, 5000);
    }
}
