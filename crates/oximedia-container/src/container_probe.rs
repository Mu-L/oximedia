#![allow(dead_code)]
//! Higher-level container probing beyond magic-byte detection.
//!
//! Provides `ContainerProbeResult`, `ContainerInfo`, and `ContainerProber`
//! for interrogating container structure without a full demux pass.

/// Summary flags produced by probing a container's header region.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerProbeResult {
    /// Whether at least one video track was detected.
    pub video_present: bool,
    /// Whether at least one audio track was detected.
    pub audio_present: bool,
    /// Whether at least one subtitle track was detected.
    pub subtitle_present: bool,
    /// Confidence of the format detection in the range `[0.0, 1.0]`.
    pub confidence: f32,
    /// Raw format name string as reported by the container layer.
    pub format_label: String,
}

impl ContainerProbeResult {
    /// Creates a new probe result with default confidence of 1.0.
    #[must_use]
    pub fn new(format_label: impl Into<String>) -> Self {
        Self {
            video_present: false,
            audio_present: false,
            subtitle_present: false,
            confidence: 1.0,
            format_label: format_label.into(),
        }
    }

    /// Returns `true` when at least one video track was detected.
    #[must_use]
    pub fn has_video(&self) -> bool {
        self.video_present
    }

    /// Returns `true` when at least one audio track was detected.
    #[must_use]
    pub fn has_audio(&self) -> bool {
        self.audio_present
    }

    /// Returns `true` for multimedia containers that have both video and audio.
    #[must_use]
    pub fn is_av(&self) -> bool {
        self.video_present && self.audio_present
    }

    /// Returns `true` when confidence is at or above `threshold`.
    #[must_use]
    pub fn is_confident(&self, threshold: f32) -> bool {
        self.confidence >= threshold
    }
}

/// Detailed structural information about a container, produced after a
/// more thorough header scan than a simple magic-byte probe.
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    /// Short format name (e.g. `"matroska"`, `"mp4"`, `"ogg"`).
    format_name: String,
    /// Total number of tracks (all types).
    total_tracks: usize,
    /// Number of video tracks.
    video_count: usize,
    /// Number of audio tracks.
    audio_count: usize,
    /// Total container duration in milliseconds, if signalled.
    duration_ms: Option<u64>,
    /// Container file size in bytes, if known.
    file_size: Option<u64>,
}

impl ContainerInfo {
    /// Creates a new `ContainerInfo`.
    #[must_use]
    pub fn new(format_name: impl Into<String>) -> Self {
        Self {
            format_name: format_name.into(),
            total_tracks: 0,
            video_count: 0,
            audio_count: 0,
            duration_ms: None,
            file_size: None,
        }
    }

    /// Sets video and audio track counts, automatically deriving `total_tracks`.
    #[must_use]
    pub fn with_tracks(mut self, video: usize, audio: usize) -> Self {
        self.video_count = video;
        self.audio_count = audio;
        self.total_tracks = video + audio;
        self
    }

    /// Sets the duration.
    #[must_use]
    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Sets the file size.
    #[must_use]
    pub fn with_file_size(mut self, bytes: u64) -> Self {
        self.file_size = Some(bytes);
        self
    }

    /// Returns the short format name.
    #[must_use]
    pub fn format_name(&self) -> &str {
        &self.format_name
    }

    /// Returns the total track count (all types).
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.total_tracks
    }

    /// Returns the number of video tracks.
    #[must_use]
    pub fn video_count(&self) -> usize {
        self.video_count
    }

    /// Returns the number of audio tracks.
    #[must_use]
    pub fn audio_count(&self) -> usize {
        self.audio_count
    }

    /// Returns the duration in milliseconds, if known.
    #[must_use]
    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    /// Estimates the average bit rate in kbps from file size and duration.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn estimated_bitrate_kbps(&self) -> Option<f64> {
        match (self.file_size, self.duration_ms) {
            (Some(bytes), Some(ms)) if ms > 0 => Some((bytes as f64 * 8.0) / (ms as f64)),
            _ => None,
        }
    }
}

/// A thin prober that inspects raw bytes and fills a `ContainerInfo`.
#[derive(Debug, Default)]
pub struct ContainerProber {
    probed_count: usize,
}

impl ContainerProber {
    /// Creates a new `ContainerProber`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of containers probed so far.
    #[must_use]
    pub fn probed_count(&self) -> usize {
        self.probed_count
    }

    /// Inspects the first bytes of a container and returns a
    /// `ContainerProbeResult`.
    ///
    /// Detection is based on well-known magic sequences:
    /// - `[0x1A, 0x45, 0xDF, 0xA3]` → Matroska / `WebM`
    /// - `[0x66, 0x4C, 0x61, 0x43]` (`fLaC`) → FLAC
    /// - `[0x4F, 0x67, 0x67, 0x53]` (`OggS`) → Ogg
    /// - `[0x52, 0x49, 0x46, 0x46]` (`RIFF`) → WAV
    /// - `[0x00, 0x00, 0x00, _, 0x66, 0x74, 0x79, 0x70]` → MP4/ftyp
    pub fn probe_header(&mut self, header: &[u8]) -> ContainerProbeResult {
        self.probed_count += 1;

        if header.len() >= 4 && header[..4] == [0x1A, 0x45, 0xDF, 0xA3] {
            let mut r = ContainerProbeResult::new("matroska");
            r.video_present = true;
            r.audio_present = true;
            return r;
        }
        if header.len() >= 4 && &header[..4] == b"fLaC" {
            let mut r = ContainerProbeResult::new("flac");
            r.audio_present = true;
            return r;
        }
        if header.len() >= 4 && &header[..4] == b"OggS" {
            let mut r = ContainerProbeResult::new("ogg");
            r.audio_present = true;
            return r;
        }
        if header.len() >= 4 && &header[..4] == b"RIFF" {
            let mut r = ContainerProbeResult::new("wav");
            r.audio_present = true;
            return r;
        }
        // MP4: check bytes 4-7 for "ftyp"
        if header.len() >= 8 && &header[4..8] == b"ftyp" {
            let mut r = ContainerProbeResult::new("mp4");
            r.video_present = true;
            r.audio_present = true;
            return r;
        }

        let mut r = ContainerProbeResult::new("unknown");
        r.confidence = 0.0;
        r
    }
}

// ─── Enhanced multi-format container probing ──────────────────────────────────

/// Detailed information about one media stream found inside a container.
#[derive(Debug, Clone, Default)]
pub struct DetailedStreamInfo {
    /// Zero-based stream index.
    pub index: u32,
    /// Stream type: `"video"`, `"audio"`, `"subtitle"`, or `"data"`.
    pub stream_type: String,
    /// Short codec name (e.g. `"av1"`, `"opus"`, `"flac"`).
    pub codec: String,
    /// ISO 639-2 language tag, if present.
    pub language: Option<String>,
    /// Stream duration in milliseconds, if known.
    pub duration_ms: Option<u64>,
    /// Average bitrate in kbps, if estimable.
    pub bitrate_kbps: Option<u32>,
    // Video fields
    /// Frame width in pixels.
    pub width: Option<u32>,
    /// Frame height in pixels.
    pub height: Option<u32>,
    /// Frames per second.
    pub fps: Option<f32>,
    /// Pixel format string (e.g. `"yuv420p"`).
    pub pixel_format: Option<String>,
    // Audio fields
    /// Audio sample rate in Hz.
    pub sample_rate: Option<u32>,
    /// Number of audio channels.
    pub channels: Option<u8>,
    /// Sample format string (e.g. `"s16"`).
    pub sample_format: Option<String>,
}

/// Rich container information returned by [`MultiFormatProber`].
#[derive(Debug, Clone, Default)]
pub struct DetailedContainerInfo {
    /// Short format name (`"mp4"`, `"mkv"`, `"mpeg-ts"`, `"webm"`, `"ogg"`,
    /// `"wav"`, `"flac"`, `"unknown"`).
    pub format: String,
    /// Total duration in milliseconds, if signalled.
    pub duration_ms: Option<u64>,
    /// Overall bitrate in kbps, if estimable from file_size_bytes + duration_ms.
    pub bitrate_kbps: Option<u32>,
    /// Discovered streams.
    pub streams: Vec<DetailedStreamInfo>,
    /// Key/value metadata extracted from the container header.
    pub metadata: std::collections::HashMap<String, String>,
    /// Byte length of the input slice.
    pub file_size_bytes: u64,
}

/// A stateless multi-format container prober that inspects raw byte slices.
///
/// Compared to [`ContainerProber`] (magic-byte only), `MultiFormatProber`
/// performs a shallow parse of the container structure to discover stream
/// count, codec, dimensions, duration, and basic metadata — all without
/// decoding any compressed data.
///
/// # Supported formats
///
/// | Format | Detection | Duration | Streams |
/// |--------|-----------|----------|---------|
/// | MPEG-TS | ✓ | from PTS | from PMT |
/// | MP4/MOV | ✓ | mvhd | trak/hdlr |
/// | MKV/WebM | ✓ | EBML Segment/Info | TrackEntry |
/// | Ogg | ✓ | BOS codec | codec header |
/// | WAV | ✓ | fmt chunk | PCM params |
/// | FLAC | ✓ | STREAMINFO | sample params |
#[derive(Debug, Default)]
pub struct MultiFormatProber;

impl MultiFormatProber {
    /// Creates a new `MultiFormatProber`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Probes `data` and returns all available container information.
    #[must_use]
    pub fn probe(data: &[u8]) -> DetailedContainerInfo {
        let mut info = DetailedContainerInfo {
            file_size_bytes: data.len() as u64,
            ..Default::default()
        };

        if data.len() < 8 {
            info.format = "unknown".into();
            return info;
        }

        // Detect by magic bytes
        if data[0] == 0x47 && (data.len() < 376 || data[188] == 0x47) {
            // MPEG-TS: sync byte 0x47 at offset 0 (and 188 if data is long enough)
            Self::probe_mpegts(data, &mut info);
        } else if data[..4] == [0x1A, 0x45, 0xDF, 0xA3] {
            // Matroska / WebM
            Self::probe_mkv(data, &mut info);
        } else if data.len() >= 8 && &data[4..8] == b"ftyp" {
            // MP4 / MOV / CMAF
            Self::probe_mp4(data, &mut info);
        } else if &data[..4] == b"OggS" {
            // Ogg container
            Self::probe_ogg(data, &mut info);
        } else if &data[..4] == b"RIFF" {
            // WAV / RIFF
            Self::probe_wav(data, &mut info);
        } else if &data[..4] == b"fLaC" {
            // Native FLAC
            Self::probe_flac(data, &mut info);
        } else if data.len() >= 4 && &data[..4] == b"caff" {
            // CAF (Core Audio Format)
            Self::probe_caf(data, &mut info);
        } else if data.len() >= 8 && data[0..2] == [0x49, 0x49] && data[2..4] == [0x2A, 0x00] {
            // TIFF little-endian (DNG is a subset of TIFF)
            Self::probe_dng_tiff(data, &mut info);
        } else if data.len() >= 8 && data[0..2] == [0x4D, 0x4D] && data[2..4] == [0x00, 0x2A] {
            // TIFF big-endian
            Self::probe_dng_tiff(data, &mut info);
        } else if data.len() >= 16
            && data[0..4] == [0x06, 0x0E, 0x2B, 0x34]
            && data[4..8] == [0x02, 0x05, 0x01, 0x01]
        {
            // MXF (Material Exchange Format) - KLV key prefix
            Self::probe_mxf(data, &mut info);
        } else {
            info.format = "unknown".into();
        }

        // Estimate overall bitrate
        if let (Some(dur_ms), sz) = (info.duration_ms, info.file_size_bytes) {
            // bitrate kbps = bytes * 8 / ms
            if let Some(bitrate) = sz.saturating_mul(8).checked_div(dur_ms) {
                info.bitrate_kbps = Some(bitrate as u32);
            }
        }

        info
    }

    /// Returns only the stream list from `data`.
    #[must_use]
    pub fn probe_streams_only(data: &[u8]) -> Vec<DetailedStreamInfo> {
        Self::probe(data).streams
    }

    // ─── MPEG-TS ──────────────────────────────────────────────────────────

    fn probe_mpegts(data: &[u8], info: &mut DetailedContainerInfo) {
        use crate::container_probe::mpegts_probe::*;
        info.format = "mpeg-ts".into();

        let (streams, duration_ms) = scan_mpegts(data);
        info.streams = streams;
        info.duration_ms = duration_ms;
    }

    // ─── MP4 / MOV ────────────────────────────────────────────────────────

    fn probe_mp4(data: &[u8], info: &mut DetailedContainerInfo) {
        info.format = "mp4".into();

        // Walk top-level boxes looking for moov
        let mut offset = 0usize;
        while offset + 8 <= data.len() {
            let box_size = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            let fourcc = &data[offset + 4..offset + 8];

            if box_size < 8 || offset + box_size > data.len() {
                break;
            }

            if fourcc == b"moov" {
                parse_moov(&data[offset + 8..offset + box_size], info);
                break;
            }

            offset += box_size;
        }
    }

    // ─── MKV / WebM ───────────────────────────────────────────────────────

    fn probe_mkv(data: &[u8], info: &mut DetailedContainerInfo) {
        // Check if this is WebM (subset of Matroska)
        info.format = "mkv".into();
        parse_ebml_for_info(data, info);
    }

    // ─── Ogg ──────────────────────────────────────────────────────────────

    fn probe_ogg(data: &[u8], info: &mut DetailedContainerInfo) {
        info.format = "ogg".into();
        parse_ogg_bos(data, info);
    }

    // ─── WAV / RIFF ───────────────────────────────────────────────────────

    fn probe_wav(data: &[u8], info: &mut DetailedContainerInfo) {
        info.format = "wav".into();
        if data.len() >= 12 && &data[8..12] == b"WAVE" {
            parse_wav_chunks(data, info);
        }
    }

    // ─── FLAC ─────────────────────────────────────────────────────────────

    fn probe_flac(data: &[u8], info: &mut DetailedContainerInfo) {
        info.format = "flac".into();
        parse_flac_streaminfo(data, info);
    }

    // ─── CAF (Core Audio Format) ─────────────────────────────────────────

    fn probe_caf(data: &[u8], info: &mut DetailedContainerInfo) {
        info.format = "caf".into();
        if data.len() < 8 {
            return;
        }
        let version = u16::from_be_bytes([data[4], data[5]]);
        info.metadata
            .insert("caf_version".into(), format!("{version}"));

        let mut offset = 8usize;
        while offset + 12 <= data.len() {
            let chunk_type = &data[offset..offset + 4];
            let chunk_size = read_u64_be(data, offset + 4);

            if chunk_type == b"desc" && chunk_size >= 32 && offset + 44 <= data.len() {
                let desc = &data[offset + 12..];
                let sr = f64::from_be_bytes([
                    desc[0], desc[1], desc[2], desc[3], desc[4], desc[5], desc[6], desc[7],
                ]);
                let codec = String::from_utf8_lossy(&desc[8..12]).trim().to_string();
                let ch = if desc.len() >= 28 {
                    read_u32_be(desc, 24)
                } else {
                    0
                };
                let mut s = DetailedStreamInfo {
                    index: 0,
                    stream_type: "audio".into(),
                    codec,
                    ..Default::default()
                };
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                {
                    s.sample_rate = Some(sr as u32);
                    if ch > 0 && ch < 256 {
                        s.channels = Some(ch as u8);
                    }
                }
                info.streams.push(s);
            }

            let advance = 12 + chunk_size as usize;
            if advance == 0 {
                break;
            }
            match offset.checked_add(advance) {
                Some(new_offset) => offset = new_offset,
                None => break,
            }
        }
    }

    // ─── DNG / TIFF ──────────────────────────────────────────────────────

    fn probe_dng_tiff(data: &[u8], info: &mut DetailedContainerInfo) {
        let is_le = data[0] == 0x49;
        let ru16 = |off: usize| -> u16 {
            if off + 2 > data.len() {
                return 0;
            }
            if is_le {
                u16::from_le_bytes([data[off], data[off + 1]])
            } else {
                u16::from_be_bytes([data[off], data[off + 1]])
            }
        };
        let ru32 = |off: usize| -> u32 {
            if off + 4 > data.len() {
                return 0;
            }
            if is_le {
                u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
            } else {
                u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
            }
        };
        let ifd_offset = ru32(4) as usize;
        if ifd_offset + 2 > data.len() {
            info.format = "tiff".into();
            return;
        }

        let entry_count = ru16(ifd_offset) as usize;
        let (mut found_dng, mut width, mut height) = (false, 0u32, 0u32);
        for i in 0..entry_count {
            let off = ifd_offset + 2 + i * 12;
            if off + 12 > data.len() {
                break;
            }
            match ru16(off) {
                0xC612 => found_dng = true,
                0x0100 => width = ru32(off + 8),
                0x0101 => height = ru32(off + 8),
                _ => {}
            }
        }
        if found_dng {
            info.format = "dng".into();
            let mut s = DetailedStreamInfo {
                index: 0,
                stream_type: "video".into(),
                codec: "raw".into(),
                ..Default::default()
            };
            if width > 0 {
                s.width = Some(width);
            }
            if height > 0 {
                s.height = Some(height);
            }
            info.streams.push(s);
        } else {
            info.format = "tiff".into();
        }
    }

    // ─── MXF (Material Exchange Format) ──────────────────────────────────

    fn probe_mxf(data: &[u8], info: &mut DetailedContainerInfo) {
        info.format = "mxf".into();
        if data.len() >= 16 {
            let pt = data[13];
            let label = match pt {
                0x02 => "header_partition",
                0x03 => "body_partition",
                0x04 => "footer_partition",
                _ => "unknown_partition",
            };
            info.metadata
                .insert("mxf_partition_type".into(), label.into());
        }
        if data.len() >= 12 && data[8] == 0x0D && data[9] == 0x01 {
            info.metadata
                .insert("mxf_registry".into(), "smpte_rdd".into());
        }
        if data.len() >= 64 {
            info.streams.push(DetailedStreamInfo {
                index: 0,
                stream_type: "video".into(),
                codec: "mxf_essence".into(),
                ..Default::default()
            });
        }
    }
}

// ─── Container corruption detection ───────────────────────────────────────────

/// Result of a container integrity check.
#[derive(Debug, Clone, PartialEq)]
pub struct IntegrityCheckResult {
    /// Whether the container passes structural validation.
    pub valid: bool,
    /// List of issues found during validation.
    pub issues: Vec<String>,
    /// Overall integrity score (0.0 = completely corrupted, 1.0 = perfect).
    pub score: f64,
}

impl IntegrityCheckResult {
    /// Creates a new passing result.
    #[must_use]
    pub fn ok() -> Self {
        Self {
            valid: true,
            issues: Vec::new(),
            score: 1.0,
        }
    }

    /// Adds an issue and adjusts the score.
    pub fn add_issue(&mut self, issue: impl Into<String>, severity: f64) {
        self.issues.push(issue.into());
        self.score = (self.score - severity).max(0.0);
        if self.score < 0.5 {
            self.valid = false;
        }
    }
}

/// Checks the structural integrity of a container's byte stream.
#[must_use]
pub fn check_container_integrity(data: &[u8]) -> IntegrityCheckResult {
    let mut r = IntegrityCheckResult::ok();
    if data.is_empty() {
        r.add_issue("Container data is empty", 1.0);
        return r;
    }
    if data.len() < 8 {
        r.add_issue("Too short for any known format", 0.8);
        return r;
    }

    if &data[4..8] == b"ftyp" {
        validate_mp4_boxes(data, &mut r);
    } else if &data[..4] == b"fLaC" {
        validate_flac_structure(data, &mut r);
    } else if &data[..4] == b"RIFF" {
        validate_riff_structure(data, &mut r);
    }
    r
}

fn validate_mp4_boxes(data: &[u8], result: &mut IntegrityCheckResult) {
    let (mut offset, mut box_count, mut found_moov) = (0usize, 0u32, false);
    while offset + 8 <= data.len() {
        let size = read_u32_be(data, offset) as usize;
        if size < 8 {
            result.add_issue(format!("Bad MP4 box size at {offset}"), 0.3);
            break;
        }
        if offset + size > data.len() {
            result.add_issue(format!("MP4 box exceeds data at {offset}"), 0.2);
            break;
        }
        if &data[offset + 4..offset + 8] == b"moov" {
            found_moov = true;
        }
        box_count += 1;
        offset += size;
    }
    if box_count == 0 {
        result.add_issue("No valid MP4 boxes", 0.5);
    }
    if !found_moov && data.len() > 1024 {
        result.add_issue("MP4 missing moov", 0.3);
    }
}

fn validate_flac_structure(data: &[u8], result: &mut IntegrityCheckResult) {
    if data.len() < 42 {
        result.add_issue("FLAC too short for STREAMINFO", 0.4);
        return;
    }
    if data[4] & 0x7F != 0 {
        result.add_issue("First FLAC block not STREAMINFO", 0.3);
    }
}

fn validate_riff_structure(data: &[u8], result: &mut IntegrityCheckResult) {
    if data.len() < 12 {
        result.add_issue("RIFF too short", 0.5);
        return;
    }
    if &data[8..12] != b"WAVE" && &data[8..12] != b"AVI " {
        result.add_issue("RIFF form type not WAVE/AVI", 0.2);
    }
    let riff_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as u64;
    if riff_size + 8 > data.len() as u64 {
        result.add_issue(
            format!("RIFF size mismatch ({} vs {})", riff_size + 8, data.len()),
            0.15,
        );
    }
}

// ─── Format-specific parsers (private) ───────────────────────────────────────

/// MPEG-TS scanning helper (kept in a sub-module to avoid name collisions).
mod mpegts_probe {
    use super::DetailedStreamInfo;
    use crate::demux::mpegts_enhanced::TsDemuxer;

    /// Stream type byte → codec name mapping (patent-free only).
    fn stream_type_to_codec(st: u8) -> Option<&'static str> {
        match st {
            0x85 => Some("av1"),
            0x84 => Some("vp9"),
            0x83 => Some("vp8"),
            0x81 => Some("opus"),
            0x82 => Some("flac"),
            0x80 => Some("pcm"),
            0x06 => Some("private"),
            _ => None,
        }
    }

    fn stream_type_to_kind(st: u8) -> &'static str {
        match st {
            0x85 | 0x84 | 0x83 | 0x1B | 0x24 => "video",
            0x81 | 0x82 | 0x80 | 0x03 | 0x04 | 0x0F | 0x11 => "audio",
            _ => "data",
        }
    }

    /// Scans `data` for MPEG-TS packets, returning (streams, duration_ms).
    pub fn scan_mpegts(data: &[u8]) -> (Vec<DetailedStreamInfo>, Option<u64>) {
        let mut demux = TsDemuxer::new();
        // Only scan the first 2 MB to keep probe fast
        let scan_end = data.len().min(2 * 1024 * 1024);
        demux.feed(&data[..scan_end]);

        let si = demux.stream_info();
        let duration_ms = demux.duration_ms();

        let mut streams: Vec<DetailedStreamInfo> = Vec::new();
        let mut idx = 0u32;

        // Walk PMT streams
        for pmt in si.pmts.values() {
            for ps in &pmt.streams {
                let codec = stream_type_to_codec(ps.stream_type)
                    .unwrap_or("unknown")
                    .to_string();
                let kind = stream_type_to_kind(ps.stream_type).to_string();

                let pid_info = si.pids.get(&ps.elementary_pid);
                let mut s = DetailedStreamInfo {
                    index: idx,
                    stream_type: kind.clone(),
                    codec,
                    ..Default::default()
                };

                if let Some(pi) = pid_info {
                    if let (Some(f), Some(l)) = (pi.pts_first, pi.pts_last) {
                        if l > f {
                            s.duration_ms = Some((l - f) / 90);
                        }
                    }
                    if s.duration_ms.is_some() && pi.total_bytes > 0 {
                        let dur_s = s.duration_ms.unwrap_or(1) as u64;
                        if let Some(bitrate) = (pi.total_bytes * 8).checked_div(dur_s) {
                            s.bitrate_kbps = Some(bitrate as u32);
                        }
                    }
                }

                streams.push(s);
                idx += 1;
            }
        }

        (streams, duration_ms)
    }
}

// Format-specific parsers — extracted to `container_probe_parsers` module.
use crate::container_probe_parsers::{
    parse_ebml_for_info, parse_flac_streaminfo, parse_moov, parse_ogg_bos, parse_wav_chunks,
    read_u32_be, read_u64_be,
};

// ─── Unit tests ───────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    // 1. has_video – true
    #[test]
    fn test_has_video_true() {
        let mut r = ContainerProbeResult::new("mkv");
        r.video_present = true;
        assert!(r.has_video());
    }

    // 2. has_video – false
    #[test]
    fn test_has_video_false() {
        let r = ContainerProbeResult::new("flac");
        assert!(!r.has_video());
    }

    // 3. has_audio – true
    #[test]
    fn test_has_audio_true() {
        let mut r = ContainerProbeResult::new("ogg");
        r.audio_present = true;
        assert!(r.has_audio());
    }

    // 4. is_av – both present
    #[test]
    fn test_is_av_both() {
        let mut r = ContainerProbeResult::new("mp4");
        r.video_present = true;
        r.audio_present = true;
        assert!(r.is_av());
    }

    // 5. is_av – audio only
    #[test]
    fn test_is_av_audio_only() {
        let mut r = ContainerProbeResult::new("wav");
        r.audio_present = true;
        assert!(!r.is_av());
    }

    // 6. is_confident threshold
    #[test]
    fn test_is_confident() {
        let r = ContainerProbeResult::new("matroska");
        assert!(r.is_confident(0.9));
        assert!(!r.is_confident(1.1));
    }

    // 7. ContainerInfo format_name
    #[test]
    fn test_container_info_format_name() {
        let info = ContainerInfo::new("matroska");
        assert_eq!(info.format_name(), "matroska");
    }

    // 8. ContainerInfo track_count
    #[test]
    fn test_container_info_track_count() {
        let info = ContainerInfo::new("mp4").with_tracks(1, 2);
        assert_eq!(info.track_count(), 3);
    }

    // 9. ContainerInfo video_count
    #[test]
    fn test_container_info_video_count() {
        let info = ContainerInfo::new("mkv").with_tracks(2, 4);
        assert_eq!(info.video_count(), 2);
        assert_eq!(info.audio_count(), 4);
    }

    // 10. estimated_bitrate_kbps – computes correctly
    #[test]
    fn test_estimated_bitrate_kbps() {
        let info = ContainerInfo::new("mp4")
            .with_file_size(1_000_000)
            .with_duration_ms(1000);
        // 1 MB in 1 s = 8 Mbps = 8000 kbps
        let kbps = info
            .estimated_bitrate_kbps()
            .expect("operation should succeed");
        assert!((kbps - 8000.0).abs() < 1.0);
    }

    // 11. estimated_bitrate_kbps – None when no duration
    #[test]
    fn test_estimated_bitrate_kbps_no_duration() {
        let info = ContainerInfo::new("mkv").with_file_size(1_000_000);
        assert!(info.estimated_bitrate_kbps().is_none());
    }

    // 12. ContainerProber detects Matroska
    #[test]
    fn test_probe_matroska() {
        let mut p = ContainerProber::new();
        let magic = [0x1A, 0x45, 0xDF, 0xA3, 0x00, 0x00, 0x00, 0x00];
        let r = p.probe_header(&magic);
        assert_eq!(r.format_label, "matroska");
        assert!(r.has_video());
        assert!(r.has_audio());
    }

    // 13. ContainerProber detects FLAC
    #[test]
    fn test_probe_flac() {
        let mut p = ContainerProber::new();
        let r = p.probe_header(b"fLaC\x00\x00\x00\x22");
        assert_eq!(r.format_label, "flac");
        assert!(!r.has_video());
        assert!(r.has_audio());
    }

    // 14. ContainerProber detects MP4 via ftyp box
    #[test]
    fn test_probe_mp4() {
        let mut p = ContainerProber::new();
        // 4-byte box size + "ftyp"
        let header = b"\x00\x00\x00\x18ftyp\x69\x73\x6f\x6d";
        let r = p.probe_header(header);
        assert_eq!(r.format_label, "mp4");
        assert!(r.has_video());
        assert_eq!(p.probed_count(), 1);
    }

    // 15. ContainerProber unknown returns confidence 0
    #[test]
    fn test_probe_unknown() {
        let mut p = ContainerProber::new();
        let r = p.probe_header(b"\xFF\xFF\xFF\xFF");
        assert_eq!(r.format_label, "unknown");
        assert_eq!(r.confidence, 0.0);
    }

    // ─── MultiFormatProber tests ─────────────────────────────────────────

    // 16. Probe empty data → unknown format
    #[test]
    fn test_multiformat_probe_empty() {
        let info = MultiFormatProber::probe(&[]);
        assert_eq!(info.format, "unknown");
        assert!(info.streams.is_empty());
    }

    // 17. Probe short random bytes → unknown
    #[test]
    fn test_multiformat_probe_random() {
        let info = MultiFormatProber::probe(&[0xFF, 0xFE, 0xFD, 0xFC, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(info.format, "unknown");
    }

    // 18. Probe FLAC magic → format = "flac"
    #[test]
    fn test_multiformat_probe_flac_magic() {
        // fLaC + STREAMINFO block header (block_type=0, length=34) + 34 bytes of STREAMINFO
        let mut data = Vec::new();
        data.extend_from_slice(b"fLaC");
        // Block header: last=0, type=0, length=34
        data.push(0x00);
        data.push(0x00);
        data.push(0x00);
        data.push(0x22); // 34
                         // Minimal STREAMINFO (34 bytes): min_block=4096, max_block=4096, all zeros
        data.extend_from_slice(&[0u8; 10]);
        // sample_rate=44100 (0xAC44), channels=2, bits=16, total_samples=441000
        // Packed: sample_rate(20) | channels(3) | bits(5) → bytes 10-12
        // 44100 = 0xAC44 = 1010 1100 0100 0100
        // bits 19..0 of sample_rate in si[10..12] plus channel and bps
        // si[10] = 0b10101100 = 0xAC  (sample_rate bits 19..12)
        // si[11] = 0b01000100 = 0x44  (sample_rate bits 11..4)
        // si[12] = 0b0100_001_01111 → high nibble = sample_rate bits 3..0 (0100),
        //          then channels-1 (2-1=1 → 001), then bps-1 (16-1=15 → 01111) split over [12][13]
        // It is complex — use known-good bytes instead
        data.push(0xAC); // si[0]  sample_rate high
        data.push(0x44); // si[1]
                         // si[2]: high nibble = sample_rate low 4 bits (0x4 = 0100), then ch-1(1)=001, bps-1(15)=01111
                         // 0100 001 0 | 1111 xxxx  → si[2] = 0x42, si[3] = 0xF0
        data.push(0x42);
        data.push(0xF0);
        // total_samples and MD5 — just zeros
        data.extend_from_slice(&[0u8; 20]);

        let info = MultiFormatProber::probe(&data);
        assert_eq!(info.format, "flac");
        assert!(!info.streams.is_empty());
        assert_eq!(info.streams[0].codec, "flac");
        assert_eq!(info.streams[0].stream_type, "audio");
    }

    // 19. Probe RIFF/WAVE → format = "wav"
    #[test]
    fn test_multiformat_probe_wav() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        let total_size: u32 = 36;
        data.extend_from_slice(&total_size.to_le_bytes()); // file size - 8
        data.extend_from_slice(b"WAVE");
        data.extend_from_slice(b"fmt ");
        data.extend_from_slice(&16u32.to_le_bytes()); // chunk size
        data.extend_from_slice(&1u16.to_le_bytes()); // PCM format
        data.extend_from_slice(&2u16.to_le_bytes()); // channels
        data.extend_from_slice(&44100u32.to_le_bytes()); // sample rate
        data.extend_from_slice(&(44100 * 2 * 2u32).to_le_bytes()); // byte rate
        data.extend_from_slice(&4u16.to_le_bytes()); // block align
        data.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

        let info = MultiFormatProber::probe(&data);
        assert_eq!(info.format, "wav");
        assert!(!info.streams.is_empty());
        let s = &info.streams[0];
        assert_eq!(s.codec, "pcm");
        assert_eq!(s.sample_rate, Some(44100));
        assert_eq!(s.channels, Some(2));
    }

    // 20. Probe Ogg magic → format = "ogg"
    #[test]
    fn test_multiformat_probe_ogg() {
        // Minimal OggS page with OpusHead BOS
        let mut data = vec![0u8; 300];
        // OggS capture
        data[0..4].copy_from_slice(b"OggS");
        data[4] = 0; // version
        data[5] = 0x02; // header_type: BOS
                        // granule position (8 bytes)
        data[6..14].fill(0);
        // serial, sequence, checksum, n_segs
        data[14..18].fill(0); // serial
        data[18..22].fill(0); // sequence
        data[22..26].fill(0); // checksum
        data[26] = 1; // n_segs = 1
        data[27] = 19; // segment size = 19 (OpusHead)
                       // OpusHead payload
        data[28..36].copy_from_slice(b"OpusHead");
        data[36] = 1; // version
        data[37] = 2; // channels
        data[38..40].fill(0); // pre-skip
        data[40..44].copy_from_slice(&48000u32.to_le_bytes()); // sample rate
        data[44..46].fill(0); // output gain
        data[46] = 0; // channel mapping family

        let info = MultiFormatProber::probe(&data);
        assert_eq!(info.format, "ogg");
    }

    // 21. Probe MP4 ftyp magic → format = "mp4"
    #[test]
    fn test_multiformat_probe_mp4_magic() {
        let mut data = Vec::new();
        // ftyp box: size=20, "ftyp", "iso5", minor=0, compatible="iso5"
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"ftyp");
        data.extend_from_slice(b"iso5");
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(b"iso5");
        // No moov — still detects as mp4
        let info = MultiFormatProber::probe(&data);
        assert_eq!(info.format, "mp4");
    }

    // 22. Probe Matroska magic → format starts with "mkv" or "webm"
    #[test]
    fn test_multiformat_probe_mkv_magic() {
        // EBML header magic
        let data = [
            0x1A, 0x45, 0xDF, 0xA3, 0x84, 0x42, 0x82, 0x84, 0x77, 0x65, 0x62, 0x6D, 0x00,
        ];
        let info = MultiFormatProber::probe(&data);
        assert!(
            info.format == "mkv" || info.format == "webm",
            "got format: {}",
            info.format
        );
    }

    // 23. probe_streams_only delegates to probe correctly
    #[test]
    fn test_probe_streams_only() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&36u32.to_le_bytes());
        data.extend_from_slice(b"WAVE");
        data.extend_from_slice(b"fmt ");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes()); // mono
        data.extend_from_slice(&22050u32.to_le_bytes());
        data.extend_from_slice(&(22050u32 * 2).to_le_bytes());
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&16u16.to_le_bytes());

        let streams = MultiFormatProber::probe_streams_only(&data);
        assert!(!streams.is_empty());
        assert_eq!(streams[0].stream_type, "audio");
    }

    // 24. file_size_bytes is correctly reported
    #[test]
    fn test_multiformat_file_size() {
        let data = b"not a real container at all, just some bytes";
        let info = MultiFormatProber::probe(data);
        assert_eq!(info.file_size_bytes, data.len() as u64);
    }

    // 25. WAV with data chunk gives duration
    #[test]
    fn test_multiformat_wav_duration() {
        let mut data = Vec::new();
        // 44100 Hz, mono, 16-bit, 44100 samples = 1000 ms
        let pcm_bytes: u32 = 44100 * 2; // samples * 2 bytes/sample
        let total: u32 = 36 + pcm_bytes;
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&total.to_le_bytes());
        data.extend_from_slice(b"WAVE");
        data.extend_from_slice(b"fmt ");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes()); // PCM
        data.extend_from_slice(&1u16.to_le_bytes()); // mono
        data.extend_from_slice(&44100u32.to_le_bytes());
        data.extend_from_slice(&(44100u32 * 2).to_le_bytes());
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&16u16.to_le_bytes());
        data.extend_from_slice(b"data");
        data.extend_from_slice(&pcm_bytes.to_le_bytes());
        data.extend(vec![0u8; pcm_bytes as usize]);

        let info = MultiFormatProber::probe(&data);
        assert_eq!(info.format, "wav");
        assert_eq!(info.duration_ms, Some(1000));
    }

    // 26. DetailedStreamInfo default is empty
    #[test]
    fn test_detailed_stream_info_default() {
        let s = DetailedStreamInfo::default();
        assert!(s.codec.is_empty());
        assert!(s.stream_type.is_empty());
        assert!(s.duration_ms.is_none());
    }

    // 27. DetailedContainerInfo metadata map is empty by default
    #[test]
    fn test_detailed_container_info_metadata() {
        let info = DetailedContainerInfo::default();
        assert!(info.metadata.is_empty());
        assert!(info.streams.is_empty());
        assert_eq!(info.file_size_bytes, 0);
    }

    // ── CAF detection tests ──────────────────────────────────────────────────

    // 28. Probe CAF magic
    #[test]
    fn test_multiformat_probe_caf() {
        let mut data = Vec::new();
        data.extend_from_slice(b"caff");
        data.extend_from_slice(&1u16.to_be_bytes()); // version
        data.extend_from_slice(&0u16.to_be_bytes()); // flags
                                                     // desc chunk
        data.extend_from_slice(b"desc");
        data.extend_from_slice(&32u64.to_be_bytes()); // chunk size
                                                      // CAFAudioDescription
        data.extend_from_slice(&44100.0_f64.to_be_bytes()); // sample rate
        data.extend_from_slice(b"lpcm"); // format ID
        data.extend_from_slice(&0u32.to_be_bytes()); // format flags
        data.extend_from_slice(&4u32.to_be_bytes()); // bytes per packet
        data.extend_from_slice(&1u32.to_be_bytes()); // frames per packet
        data.extend_from_slice(&2u32.to_be_bytes()); // channels per frame
        data.extend_from_slice(&16u32.to_be_bytes()); // bits per channel

        let info = MultiFormatProber::probe(&data);
        assert_eq!(info.format, "caf");
        assert!(!info.streams.is_empty());
        assert_eq!(info.streams[0].stream_type, "audio");
        assert_eq!(info.streams[0].sample_rate, Some(44100));
        assert_eq!(info.streams[0].channels, Some(2));
    }

    // 29. CAF with short data
    #[test]
    fn test_caf_short_data() {
        let mut data = Vec::new();
        data.extend_from_slice(b"caff");
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes());
        // No desc chunk

        let info = MultiFormatProber::probe(&data);
        assert_eq!(info.format, "caf");
        assert!(info.streams.is_empty());
    }

    // ── DNG / TIFF detection tests ───────────────────────────────────────────

    // 30. TIFF LE detection without DNG tag
    #[test]
    fn test_probe_tiff_le() {
        let mut data = vec![0u8; 128];
        data[0] = 0x49; // 'I'
        data[1] = 0x49; // 'I'
        data[2] = 0x2A; // TIFF magic
        data[3] = 0x00;
        // IFD offset at byte 8
        data[4..8].copy_from_slice(&8u32.to_le_bytes());
        // IFD: entry count = 0
        data[8..10].copy_from_slice(&0u16.to_le_bytes());

        let info = MultiFormatProber::probe(&data);
        assert_eq!(info.format, "tiff");
    }

    // 31. DNG detection with DNGVersion tag
    #[test]
    fn test_probe_dng() {
        let mut data = vec![0u8; 128];
        data[0] = 0x49; // 'I'
        data[1] = 0x49; // 'I'
        data[2] = 0x2A;
        data[3] = 0x00;
        // IFD offset at byte 8
        data[4..8].copy_from_slice(&8u32.to_le_bytes());
        // IFD: 2 entries
        data[8..10].copy_from_slice(&2u16.to_le_bytes());
        // Entry 1: ImageWidth (0x0100) = 4000
        data[10..12].copy_from_slice(&0x0100u16.to_le_bytes());
        data[12..14].copy_from_slice(&3u16.to_le_bytes()); // type: SHORT
        data[14..18].copy_from_slice(&1u32.to_le_bytes()); // count
        data[18..22].copy_from_slice(&4000u32.to_le_bytes()); // value
                                                              // Entry 2: DNGVersion (0xC612) = 1
        data[22..24].copy_from_slice(&0xC612u16.to_le_bytes());
        data[24..26].copy_from_slice(&1u16.to_le_bytes()); // type: BYTE
        data[26..30].copy_from_slice(&4u32.to_le_bytes()); // count
        data[30..34].copy_from_slice(&1u32.to_le_bytes()); // value

        let info = MultiFormatProber::probe(&data);
        assert_eq!(info.format, "dng");
        assert!(!info.streams.is_empty());
        assert_eq!(info.streams[0].stream_type, "video");
        assert_eq!(info.streams[0].codec, "raw");
        assert_eq!(info.streams[0].width, Some(4000));
    }

    // 32. TIFF BE detection
    #[test]
    fn test_probe_tiff_be() {
        let mut data = vec![0u8; 64];
        data[0] = 0x4D; // 'M'
        data[1] = 0x4D; // 'M'
        data[2] = 0x00;
        data[3] = 0x2A;
        data[4..8].copy_from_slice(&8u32.to_be_bytes());
        data[8..10].copy_from_slice(&0u16.to_be_bytes());

        let info = MultiFormatProber::probe(&data);
        assert_eq!(info.format, "tiff");
    }

    // ── MXF detection tests ──────────────────────────────────────────────────

    // 33. MXF KLV header detection
    #[test]
    fn test_probe_mxf() {
        let mut data = vec![0u8; 128];
        // MXF header partition pack key
        data[0..4].copy_from_slice(&[0x06, 0x0E, 0x2B, 0x34]);
        data[4..8].copy_from_slice(&[0x02, 0x05, 0x01, 0x01]);
        data[8..12].copy_from_slice(&[0x0D, 0x01, 0x02, 0x01]);
        data[12..16].copy_from_slice(&[0x01, 0x02, 0x04, 0x00]); // header partition

        let info = MultiFormatProber::probe(&data);
        assert_eq!(info.format, "mxf");
        assert!(info.metadata.contains_key("mxf_partition_type"));
        assert_eq!(
            info.metadata.get("mxf_partition_type"),
            Some(&"header_partition".to_string())
        );
    }

    // 34. MXF with video stream
    #[test]
    fn test_probe_mxf_streams() {
        let mut data = vec![0u8; 128];
        data[0..4].copy_from_slice(&[0x06, 0x0E, 0x2B, 0x34]);
        data[4..8].copy_from_slice(&[0x02, 0x05, 0x01, 0x01]);
        data[8..12].copy_from_slice(&[0x0D, 0x01, 0x02, 0x01]);
        data[12..16].copy_from_slice(&[0x01, 0x03, 0x04, 0x00]); // body partition

        let info = MultiFormatProber::probe(&data);
        assert_eq!(info.format, "mxf");
        assert!(!info.streams.is_empty());
        assert_eq!(info.streams[0].codec, "mxf_essence");
    }

    // ── Container integrity tests ────────────────────────────────────────────

    // 35. Empty data
    #[test]
    fn test_integrity_empty() {
        let result = check_container_integrity(&[]);
        assert!(!result.valid);
        assert!(!result.issues.is_empty());
    }

    // 36. Too short data
    #[test]
    fn test_integrity_too_short() {
        let result = check_container_integrity(&[0x00, 0x01, 0x02]);
        assert!(!result.valid);
    }

    // 37. Valid MP4 structure
    #[test]
    fn test_integrity_valid_mp4() {
        let mut data = Vec::new();
        // ftyp box
        data.extend_from_slice(&20u32.to_be_bytes());
        data.extend_from_slice(b"ftyp");
        data.extend_from_slice(b"iso5");
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(b"iso5");

        let result = check_container_integrity(&data);
        assert!(result.valid);
        assert!(result.score > 0.5);
    }

    // 38. MP4 with bad box size
    #[test]
    fn test_integrity_mp4_bad_box() {
        let mut data = Vec::new();
        // ftyp box with size larger than data
        data.extend_from_slice(&200u32.to_be_bytes());
        data.extend_from_slice(b"ftyp");
        data.extend_from_slice(&[0u8; 12]);

        let result = check_container_integrity(&data);
        assert!(result.score < 1.0);
    }

    // 39. Valid FLAC structure
    #[test]
    fn test_integrity_valid_flac() {
        let mut data = vec![0u8; 50];
        data[0..4].copy_from_slice(b"fLaC");
        data[4] = 0x00; // STREAMINFO block type

        let result = check_container_integrity(&data);
        assert!(result.valid);
    }

    // 40. FLAC too short
    #[test]
    fn test_integrity_flac_short() {
        let mut data = vec![0u8; 20];
        data[0..4].copy_from_slice(b"fLaC");

        let result = check_container_integrity(&data);
        assert!(result.score < 1.0);
    }

    // 41. Valid RIFF/WAV
    #[test]
    fn test_integrity_valid_wav() {
        let data_size: u32 = 36;
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&data_size.to_le_bytes());
        data.extend_from_slice(b"WAVE");
        data.extend_from_slice(b"fmt ");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 16]); // fmt data
        data.extend_from_slice(b"data");
        data.extend_from_slice(&0u32.to_le_bytes());

        let result = check_container_integrity(&data);
        assert!(result.valid);
    }

    // 42. RIFF with size mismatch
    #[test]
    fn test_integrity_riff_size_mismatch() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&100_000u32.to_le_bytes()); // claims 100KB
        data.extend_from_slice(b"WAVE");
        data.extend_from_slice(&[0u8; 8]); // only 20 bytes total

        let result = check_container_integrity(&data);
        assert!(result.score < 1.0);
    }
}
