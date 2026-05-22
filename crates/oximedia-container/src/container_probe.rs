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

// ─── Detailed stream statistics ──────────────────────────────────────────────

/// Per-second bitrate window statistics for one stream.
///
/// Produced by [`probe_detailed`].  The histogram divides the stream into
/// 1-second windows and records the number of bits observed in each window.
#[derive(Debug, Clone)]
pub struct DetailedStreamStats {
    /// Zero-based stream index (matching the ordering in [`DetailedContainerInfo::streams`]).
    pub stream_index: usize,
    /// Short codec name (e.g. `"av1"`, `"opus"`, `"flac"`).
    pub codec_id: String,
    /// Stream duration in fractional seconds (0.0 if unknown).
    pub duration_s: f64,
    /// Bitrate window size in seconds (always 1.0 in current implementation).
    pub bitrate_window_s: f64,
    /// Number of bits per window, one entry per complete second.
    pub bitrate_histogram: Vec<u64>,
    /// Mean bitrate across all windows (bits per second).
    pub bitrate_mean: f64,
    /// Median (P50) bitrate (bits per second).
    pub bitrate_p50: f64,
    /// 95th-percentile bitrate (bits per second).
    pub bitrate_p95: f64,
    /// Peak bitrate across all windows (bits per second).
    pub bitrate_max: f64,
    /// Sorted list of inter-keyframe intervals in seconds.  `None` for
    /// audio/data streams where keyframes are not meaningful.
    pub keyframe_intervals_s: Option<Vec<f64>>,
    /// Mean inter-keyframe interval (seconds).  `None` when `keyframe_intervals_s` is `None`
    /// or empty (fewer than two keyframes observed).
    pub keyframe_interval_mean: Option<f64>,
    /// Median (P50) inter-keyframe interval (seconds).
    pub keyframe_interval_p50: Option<f64>,
    /// 95th-percentile inter-keyframe interval (seconds).
    pub keyframe_interval_p95: Option<f64>,
    /// Maximum inter-keyframe interval (seconds).
    pub keyframe_interval_max: Option<f64>,
}

/// Compute detailed per-second bitrate and keyframe-interval statistics from
/// a raw container byte slice.
///
/// # Algorithm
///
/// 1. Runs [`MultiFormatProber::probe`] to obtain stream metadata (codec,
///    duration, type).
/// 2. For **MPEG-TS** data: replays the byte slice through the enhanced
///    [`crate::demux::mpegts_enhanced::TsDemuxer`] at packet granularity, bucketing payload bytes into 1-second
///    windows and collecting PTS timestamps of PUSI-marked video packets as
///    keyframe proxies.
/// 3. For all other formats: falls back to a single-bucket histogram derived
///    from the aggregate bitrate reported by the prober, and sets
///    `keyframe_intervals_s = None`.
///
/// # Errors
///
/// Returns an error only if `data` is empty.
pub fn probe_detailed(data: &[u8]) -> oximedia_core::OxiResult<Vec<DetailedStreamStats>> {
    if data.is_empty() {
        return Err(oximedia_core::OxiError::Parse {
            offset: 0,
            message: "probe_detailed: empty data".into(),
        });
    }

    // Step 1 — Coarse probe for stream metadata.
    let base = MultiFormatProber::probe(data);

    // Step 2 — Format-specific per-packet analysis.
    if base.format == "mpeg-ts" {
        probe_detailed_mpegts(data, &base)
    } else {
        probe_detailed_fallback(data, &base)
    }
}

// ─── MPEG-TS detailed path ────────────────────────────────────────────────────

fn probe_detailed_mpegts(
    data: &[u8],
    base: &DetailedContainerInfo,
) -> oximedia_core::OxiResult<Vec<DetailedStreamStats>> {
    use crate::demux::mpegts_enhanced::TsDemuxer;

    const WINDOW_S: f64 = 1.0;
    const TS_CLOCK: f64 = 90_000.0; // 90 kHz PTS clock

    // Replay through the demuxer to collect per-PID per-packet data.
    let mut demux = TsDemuxer::new();
    let packets = demux.feed(data);
    let si = demux.stream_info();

    // Build a PID → stream-index map from PMT data.
    let mut pid_to_idx: std::collections::HashMap<u16, usize> = std::collections::HashMap::new();
    let mut pid_to_type: std::collections::HashMap<u16, &str> = std::collections::HashMap::new();
    {
        let mut stream_idx = 0usize;
        for pmt in si.pmts.values() {
            for ps in &pmt.streams {
                pid_to_idx.insert(ps.elementary_pid, stream_idx);
                pid_to_type.insert(ps.elementary_pid, stream_type_kind(ps.stream_type));
                stream_idx += 1;
            }
        }
    }

    // Per-stream accumulators: (bits_per_window, keyframe_pts_list).
    let num_streams = pid_to_idx.len();
    let mut window_bits: Vec<Vec<u64>> = vec![Vec::new(); num_streams];
    let mut kf_pts: Vec<Option<Vec<f64>>> = (0..num_streams)
        .map(|i| {
            // Determine whether this stream index is video.
            let pid = pid_to_idx
                .iter()
                .find(|(_, &v)| v == i)
                .map(|(&k, _)| k)
                .unwrap_or(u16::MAX);
            if pid_to_type.get(&pid).copied() == Some("video") {
                Some(Vec::new())
            } else {
                None
            }
        })
        .collect();

    // Walk every parsed TS packet and accumulate statistics.
    for pkt in &packets {
        let idx = match pid_to_idx.get(&pkt.pid) {
            Some(&i) => i,
            None => continue,
        };

        // Bits contributed by this TS packet (188 bytes × 8).
        let bits = 188u64 * 8;

        // Determine the 1-second window from PTS when available.
        let window_idx = if let Some(pts) = pkt.pts {
            // pts is in 90 kHz ticks; divide by 90000 to get seconds.
            (pts as f64 / TS_CLOCK / WINDOW_S) as usize
        } else {
            // No PTS on this packet — use current window count as a proxy.
            window_bits[idx].len()
        };

        // Grow the window vector if needed.
        if window_idx >= window_bits[idx].len() {
            window_bits[idx].resize(window_idx + 1, 0);
        }
        window_bits[idx][window_idx] += bits;

        // Keyframe proxy: PUSI on video streams.
        if pkt.payload_unit_start {
            if let Some(ref mut kf_list) = kf_pts[idx] {
                if let Some(pts) = pkt.pts {
                    kf_list.push(pts as f64 / TS_CLOCK);
                }
            }
        }
    }

    // Build output statistics per stream.
    let mut stats: Vec<DetailedStreamStats> = Vec::with_capacity(num_streams);

    for stream_idx in 0..num_streams {
        // Codec and duration from the base probe.
        let base_stream = base.streams.get(stream_idx);
        let codec_id = base_stream
            .map(|s| s.codec.clone())
            .unwrap_or_else(|| "unknown".into());
        let duration_s = base_stream
            .and_then(|s| s.duration_ms)
            .map(|ms| ms as f64 / 1000.0)
            .unwrap_or(0.0);

        let histogram = window_bits[stream_idx].clone();
        let (mean, p50, p95, max) = bitrate_percentiles(&histogram);

        // Keyframe interval computation.
        let (kf_intervals, kf_mean, kf_p50, kf_p95, kf_max) =
            compute_kf_interval_stats(kf_pts[stream_idx].as_deref());

        stats.push(DetailedStreamStats {
            stream_index: stream_idx,
            codec_id,
            duration_s,
            bitrate_window_s: WINDOW_S,
            bitrate_histogram: histogram,
            bitrate_mean: mean,
            bitrate_p50: p50,
            bitrate_p95: p95,
            bitrate_max: max,
            keyframe_intervals_s: kf_intervals,
            keyframe_interval_mean: kf_mean,
            keyframe_interval_p50: kf_p50,
            keyframe_interval_p95: kf_p95,
            keyframe_interval_max: kf_max,
        });
    }

    Ok(stats)
}

// ─── Fallback path (non-MPEG-TS) ─────────────────────────────────────────────

fn probe_detailed_fallback(
    _data: &[u8],
    base: &DetailedContainerInfo,
) -> oximedia_core::OxiResult<Vec<DetailedStreamStats>> {
    let mut stats = Vec::with_capacity(base.streams.len());

    for (idx, stream) in base.streams.iter().enumerate() {
        // Build a single-bucket histogram from overall average bitrate when available.
        let histogram = if let Some(kbps) = stream.bitrate_kbps {
            vec![u64::from(kbps) * 1000]
        } else if let (Some(kbps), Some(dur_ms)) = (base.bitrate_kbps, base.duration_ms) {
            // Distribute overall bitrate across duration.
            let n_windows = ((dur_ms as f64 / 1000.0).ceil() as usize).max(1);
            vec![u64::from(kbps) * 1000; n_windows]
        } else {
            Vec::new()
        };

        let (mean, p50, p95, max) = bitrate_percentiles(&histogram);
        let duration_s = stream
            .duration_ms
            .or(base.duration_ms)
            .map(|ms| ms as f64 / 1000.0)
            .unwrap_or(0.0);

        stats.push(DetailedStreamStats {
            stream_index: idx,
            codec_id: stream.codec.clone(),
            duration_s,
            bitrate_window_s: 1.0,
            bitrate_histogram: histogram,
            bitrate_mean: mean,
            bitrate_p50: p50,
            bitrate_p95: p95,
            bitrate_max: max,
            keyframe_intervals_s: None,
            keyframe_interval_mean: None,
            keyframe_interval_p50: None,
            keyframe_interval_p95: None,
            keyframe_interval_max: None,
        });
    }

    Ok(stats)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Maps an ISO 13818-1 stream-type byte to `"video"`, `"audio"`, or `"data"`.
fn stream_type_kind(st: u8) -> &'static str {
    match st {
        0x85 | 0x84 | 0x83 | 0x1B | 0x24 => "video",
        0x81 | 0x82 | 0x80 | 0x03 | 0x04 | 0x0F | 0x11 => "audio",
        _ => "data",
    }
}

/// Computes (mean, p50, p95, max) of a histogram of `u64` bit-count values.
///
/// All output values are in the same unit as the input (bits per window).
/// Returns `(0.0, 0.0, 0.0, 0.0)` if `histogram` is empty.
#[allow(clippy::cast_precision_loss)]
fn bitrate_percentiles(histogram: &[u64]) -> (f64, f64, f64, f64) {
    if histogram.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mean = histogram.iter().sum::<u64>() as f64 / histogram.len() as f64;
    let max = *histogram.iter().max().unwrap_or(&0) as f64;

    let mut sorted: Vec<f64> = histogram.iter().map(|&v| v as f64).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let p50 = percentile(&sorted, 0.50);
    let p95 = percentile(&sorted, 0.95);

    (mean, p50, p95, max)
}

/// Derives keyframe interval statistics from an optional list of keyframe timestamps.
///
/// Returns `(intervals, mean, p50, p95, max)` — all `None` when `kf_timestamps`
/// is `None` or has fewer than two entries.
#[allow(clippy::cast_precision_loss)]
fn compute_kf_interval_stats(
    kf_timestamps: Option<&[f64]>,
) -> (
    Option<Vec<f64>>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
) {
    let Some(ts) = kf_timestamps else {
        return (None, None, None, None, None);
    };
    if ts.len() < 2 {
        return (Some(Vec::new()), None, None, None, None);
    }

    let mut intervals: Vec<f64> = ts.windows(2).map(|w| w[1] - w[0]).collect();
    intervals.retain(|&v| v >= 0.0);
    intervals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    if intervals.is_empty() {
        return (Some(Vec::new()), None, None, None, None);
    }

    let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
    let max = intervals.last().copied();
    let p50 = percentile(&intervals, 0.50);
    let p95 = percentile(&intervals, 0.95);

    (Some(intervals), Some(mean), Some(p50), Some(p95), max)
}

/// Computes the `p`-th percentile (0.0–1.0) of a **pre-sorted** slice using
/// linear interpolation between neighbours.
///
/// Returns 0.0 for empty slices; returns `sorted[0]` for single-element slices.
#[allow(clippy::cast_precision_loss)]
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let idx = p * (sorted.len() - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    let frac = idx - lo as f64;
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}

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

    // ─── probe_detailed and percentile tests ─────────────────────────────────

    // 43. percentile on empty slice returns 0.0
    #[test]
    fn test_percentile_empty() {
        assert_eq!(percentile(&[], 0.5), 0.0);
    }

    // 44. percentile on single-element slice
    #[test]
    fn test_percentile_single() {
        assert_eq!(percentile(&[42.0], 0.0), 42.0);
        assert_eq!(percentile(&[42.0], 1.0), 42.0);
    }

    // 45. percentile on five-element slice
    #[test]
    fn test_percentile_five_elements() {
        let data = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(&data, 0.0), 1.0);
        assert_eq!(percentile(&data, 1.0), 5.0);
        let p50 = percentile(&data, 0.5);
        assert!((p50 - 3.0).abs() < 1e-9, "p50={p50}");
    }

    // 46. percentile linear interpolation
    #[test]
    fn test_percentile_interpolation() {
        let data = [0.0, 10.0];
        let p25 = percentile(&data, 0.25);
        assert!((p25 - 2.5).abs() < 1e-9, "p25={p25}");
        let p75 = percentile(&data, 0.75);
        assert!((p75 - 7.5).abs() < 1e-9, "p75={p75}");
    }

    // 47. probe_detailed returns Err on empty slice
    #[test]
    fn test_probe_detailed_empty_error() {
        let result = probe_detailed(&[]);
        assert!(result.is_err());
    }

    // 48. probe_detailed on WAV returns Ok with one stream
    #[test]
    fn test_probe_detailed_wav_single_stream() {
        // Minimal WAV: 44100 Hz, mono, 16-bit, 44100 samples = 1 second
        let pcm_bytes: u32 = 44100 * 2;
        let total: u32 = 36 + pcm_bytes;
        let mut data = Vec::new();
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

        let stats = probe_detailed(&data).expect("probe_detailed should succeed on WAV");
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].stream_index, 0);
        assert_eq!(stats[0].codec_id, "pcm");
        // Audio stream: keyframe intervals not applicable
        assert!(stats[0].keyframe_intervals_s.is_none());
        // bitrate_window_s should be 1.0
        assert!((stats[0].bitrate_window_s - 1.0).abs() < f64::EPSILON);
    }

    // 49. probe_detailed on unknown data returns Ok with empty stream list
    #[test]
    fn test_probe_detailed_unknown_format_empty_streams() {
        // Random bytes that won't match any format
        let data = [0xFF_u8; 64];
        let stats = probe_detailed(&data).expect("probe_detailed should succeed on unknown data");
        // MultiFormatProber returns format="unknown" with no streams
        assert!(stats.is_empty());
    }

    // 50. probe_detailed on FLAC populates codec_id = "flac"
    #[test]
    fn test_probe_detailed_flac_codec() {
        // Minimal FLAC: fLaC + STREAMINFO block
        let mut data = Vec::new();
        data.extend_from_slice(b"fLaC");
        // Block header: last=0, type=0, length=34
        data.push(0x00);
        data.push(0x00);
        data.push(0x00);
        data.push(0x22); // 34
                         // STREAMINFO (34 bytes): first 10 bytes zeros then sample_rate/channels/bps
        data.extend_from_slice(&[0u8; 10]);
        data.push(0xAC); // sample_rate high
        data.push(0x44);
        data.push(0x42);
        data.push(0xF0);
        data.extend_from_slice(&[0u8; 20]);

        let stats = probe_detailed(&data).expect("probe_detailed should succeed on FLAC");
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].codec_id, "flac");
    }

    // 51. DetailedStreamStats has expected fields and defaults
    #[test]
    fn test_detailed_stream_stats_fields() {
        let s = DetailedStreamStats {
            stream_index: 2,
            codec_id: "av1".into(),
            duration_s: 10.5,
            bitrate_window_s: 1.0,
            bitrate_histogram: vec![1_000_000, 2_000_000],
            bitrate_mean: 1_500_000.0,
            bitrate_p50: 1_500_000.0,
            bitrate_p95: 1_900_000.0,
            bitrate_max: 2_000_000.0,
            keyframe_intervals_s: Some(vec![2.0, 2.0, 2.0]),
            keyframe_interval_mean: Some(2.0),
            keyframe_interval_p50: Some(2.0),
            keyframe_interval_p95: Some(2.0),
            keyframe_interval_max: Some(2.0),
        };
        assert_eq!(s.stream_index, 2);
        assert_eq!(s.codec_id, "av1");
        assert!((s.duration_s - 10.5).abs() < f64::EPSILON);
        assert_eq!(s.bitrate_histogram.len(), 2);
        assert!(s.keyframe_intervals_s.is_some());
    }

    // 52. probe_detailed on MPEG-TS data with a synthetic packet sequence
    #[test]
    fn test_probe_detailed_mpegts_synthetic() {
        // Build a minimal MPEG-TS stream:
        // PAT + PMT (one video stream on PID 0x100) + a few data packets.
        let mut data = Vec::new();

        // Helper: build a 188-byte TS packet with given PID, PUSI, payload
        let make_ts_pkt = |pid: u16, pusi: bool, payload: &[u8]| -> [u8; 188] {
            let mut pkt = [0u8; 188];
            pkt[0] = 0x47;
            pkt[1] = (if pusi { 0x40 } else { 0x00 }) | ((pid >> 8) as u8 & 0x1F);
            pkt[2] = (pid & 0xFF) as u8;
            pkt[3] = 0x10; // payload only, no adaptation
            let copy_len = payload.len().min(184);
            pkt[4..4 + copy_len].copy_from_slice(&payload[..copy_len]);
            pkt
        };

        // PAT packet (PID 0x0000): points program 1 to PMT PID 0x0010
        let mut pat_payload = vec![0u8; 184];
        pat_payload[0] = 0x00; // pointer field
        pat_payload[1] = 0x00; // table_id = 0 (PAT)
        pat_payload[2] = 0xB0; // section_syntax + section_length high
        pat_payload[3] = 0x0D; // section_length = 13 (5 fixed + 4 CRC + 4 entry)
        pat_payload[4] = 0x00;
        pat_payload[5] = 0x01; // transport_stream_id = 1
        pat_payload[6] = 0xC1; // version + current_next
        pat_payload[7] = 0x00; // section_number
        pat_payload[8] = 0x00; // last_section_number
        pat_payload[9] = 0x00;
        pat_payload[10] = 0x01; // program_number = 1
        pat_payload[11] = 0xE0 | 0x00;
        pat_payload[12] = 0x10; // pmt_pid = 0x0010
                                // CRC (ignored by TsDemuxer)
        data.extend_from_slice(&make_ts_pkt(0x0000, true, &pat_payload));

        // PMT packet (PID 0x0010): video stream type 0x85 (AV1) on PID 0x0100
        let mut pmt_payload = vec![0u8; 184];
        pmt_payload[0] = 0x00; // pointer
        pmt_payload[1] = 0x02; // table_id = 2 (PMT)
        pmt_payload[2] = 0xB0;
        pmt_payload[3] = 0x12; // section_length = 18
        pmt_payload[4] = 0x00;
        pmt_payload[5] = 0x01; // program_number
        pmt_payload[6] = 0xC1;
        pmt_payload[7] = 0x00;
        pmt_payload[8] = 0x00;
        pmt_payload[9] = 0xE1; // PCR PID high
        pmt_payload[10] = 0x00; // PCR PID = 0x100
        pmt_payload[11] = 0xF0;
        pmt_payload[12] = 0x00; // program_info_length = 0
                                // Stream entry: type=0x85 (AV1 video), PID=0x0100
        pmt_payload[13] = 0x85;
        pmt_payload[14] = 0xE1;
        pmt_payload[15] = 0x00; // elementary PID = 0x100
        pmt_payload[16] = 0xF0;
        pmt_payload[17] = 0x00; // ES info length = 0
                                // CRC
        data.extend_from_slice(&make_ts_pkt(0x0010, true, &pmt_payload));

        // Two video packets with PTS (PID 0x0100)
        // PES header with PTS 90000 (= 1s)
        let mut pes1 = vec![0u8; 184];
        pes1[0] = 0x00;
        pes1[1] = 0x00;
        pes1[2] = 0x01; // start code
        pes1[3] = 0xE0; // stream_id = video
        pes1[4] = 0x00;
        pes1[5] = 0x00; // PES packet length = 0 (unbounded)
        pes1[6] = 0x80; // flags: PTS present
        pes1[7] = 0x80; // PTS_DTS_flags: PTS only
        pes1[8] = 0x05; // header_data_length
                        // PTS = 90000 (1 second): encode in 5 bytes
        let pts1: u64 = 90_000;
        pes1[9] = 0x21 | (((pts1 >> 29) & 0x0E) as u8);
        pes1[10] = ((pts1 >> 22) & 0xFF) as u8;
        pes1[11] = (((pts1 >> 14) & 0xFE) as u8) | 0x01;
        pes1[12] = ((pts1 >> 7) & 0xFF) as u8;
        pes1[13] = (((pts1 & 0x7F) << 1) as u8) | 0x01;
        data.extend_from_slice(&make_ts_pkt(0x0100, true, &pes1));

        // Second video packet PTS = 270000 (= 3s)
        let mut pes2 = pes1.clone();
        let pts2: u64 = 270_000;
        pes2[9] = 0x21 | (((pts2 >> 29) & 0x0E) as u8);
        pes2[10] = ((pts2 >> 22) & 0xFF) as u8;
        pes2[11] = (((pts2 >> 14) & 0xFE) as u8) | 0x01;
        pes2[12] = ((pts2 >> 7) & 0xFF) as u8;
        pes2[13] = (((pts2 & 0x7F) << 1) as u8) | 0x01;
        data.extend_from_slice(&make_ts_pkt(0x0100, true, &pes2));

        let stats = probe_detailed(&data).expect("probe_detailed should succeed on TS");
        // Should have at least one stream.
        assert!(!stats.is_empty(), "Expected at least one stream");

        // The video stream should have a non-empty histogram.
        let video_stat = stats.iter().find(|s| !s.bitrate_histogram.is_empty());
        assert!(
            video_stat.is_some(),
            "Expected at least one stream with non-empty bitrate histogram"
        );
    }
}
