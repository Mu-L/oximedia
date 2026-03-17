//! MP4 muxer writer implementation.
//!
//! Generates ISOBMFF-compliant MP4 files with moov/mdat layout for
//! progressive MP4, or moov/moof+mdat for fragmented MP4.

#![forbid(unsafe_code)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

use oximedia_core::{CodecId, MediaType, OxiError, OxiResult, Rational};

use crate::mux::cmaf::{write_box, write_full_box, write_u32_be, write_u64_be};
use crate::{Packet, StreamInfo};

// ─── Constants ──────────────────────────────────────────────────────────────

/// Default timescale for the movie header (ticks per second).
const MOVIE_TIMESCALE: u32 = 1000;

/// Maximum supported tracks.
const MAX_TRACKS: usize = 16;

// ─── Configuration ──────────────────────────────────────────────────────────

/// MP4 muxing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mp4Mode {
    /// Progressive MP4: single moov + single mdat.
    /// All sample metadata is collected in memory and written to moov at finalize.
    Progressive,
    /// Fragmented MP4: moov (with mvex) + repeating moof+mdat fragments.
    Fragmented,
}

impl Default for Mp4Mode {
    fn default() -> Self {
        Self::Progressive
    }
}

/// Configuration for the MP4 muxer.
#[derive(Debug, Clone)]
pub struct Mp4Config {
    /// Muxing mode (progressive or fragmented).
    pub mode: Mp4Mode,
    /// Fragment duration in milliseconds (only for fragmented mode).
    pub fragment_duration_ms: u32,
    /// Major brand for ftyp box.
    pub major_brand: [u8; 4],
    /// Minor version for ftyp box.
    pub minor_version: u32,
    /// Compatible brands for ftyp box.
    pub compatible_brands: Vec<[u8; 4]>,
    /// Creation time (seconds since 1904-01-01).
    pub creation_time: u64,
    /// Modification time (seconds since 1904-01-01).
    pub modification_time: u64,
}

impl Default for Mp4Config {
    fn default() -> Self {
        Self {
            mode: Mp4Mode::Progressive,
            fragment_duration_ms: 2000,
            major_brand: *b"isom",
            minor_version: 0x200,
            compatible_brands: vec![*b"isom", *b"iso6", *b"mp41"],
            creation_time: 0,
            modification_time: 0,
        }
    }
}

impl Mp4Config {
    /// Creates a new `Mp4Config` with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the muxing mode.
    #[must_use]
    pub const fn with_mode(mut self, mode: Mp4Mode) -> Self {
        self.mode = mode;
        self
    }

    /// Sets the fragment duration in milliseconds.
    #[must_use]
    pub const fn with_fragment_duration_ms(mut self, ms: u32) -> Self {
        self.fragment_duration_ms = ms;
        self
    }

    /// Sets the major brand.
    #[must_use]
    pub const fn with_major_brand(mut self, brand: [u8; 4]) -> Self {
        self.major_brand = brand;
        self
    }

    /// Sets the minor version.
    #[must_use]
    pub const fn with_minor_version(mut self, version: u32) -> Self {
        self.minor_version = version;
        self
    }

    /// Adds a compatible brand.
    #[must_use]
    pub fn with_compatible_brand(mut self, brand: [u8; 4]) -> Self {
        self.compatible_brands.push(brand);
        self
    }
}

// ─── Sample Entry ───────────────────────────────────────────────────────────

/// Metadata for a single sample in a track.
#[derive(Debug, Clone)]
pub struct Mp4SampleEntry {
    /// Sample size in bytes.
    pub size: u32,
    /// Sample duration in the track's timescale.
    pub duration: u32,
    /// Composition time offset (PTS - DTS) in the track's timescale.
    pub composition_offset: i32,
    /// Whether this sample is a sync (key) sample.
    pub is_sync: bool,
    /// Chunk index this sample belongs to.
    pub chunk_index: u32,
}

// ─── Track State ────────────────────────────────────────────────────────────

/// Per-track state maintained during muxing.
#[derive(Debug, Clone)]
pub struct Mp4TrackState {
    /// Stream information.
    pub stream_info: StreamInfo,
    /// Track ID (1-based).
    pub track_id: u32,
    /// Timescale for this track.
    pub timescale: u32,
    /// Accumulated sample entries.
    pub samples: Vec<Mp4SampleEntry>,
    /// Raw mdat data for this track.
    pub mdat_data: Vec<u8>,
    /// Current chunk boundaries (sample indices).
    pub chunk_boundaries: Vec<u32>,
    /// Number of samples in the current chunk.
    pub current_chunk_samples: u32,
    /// Maximum samples per chunk before starting a new one.
    pub max_samples_per_chunk: u32,
    /// Total duration in timescale units.
    pub total_duration: u64,
    /// Handler type identifier.
    pub handler_type: [u8; 4],
    /// Handler name.
    pub handler_name: String,
}

impl Mp4TrackState {
    fn new(stream_info: StreamInfo, track_id: u32) -> Self {
        let (timescale, handler_type, handler_name) = match stream_info.media_type {
            MediaType::Video => {
                let ts = if stream_info.timebase.den != 0 {
                    // Use timebase denominator as timescale
                    stream_info.timebase.den as u32
                } else {
                    90000
                };
                (ts, *b"vide", "OxiMedia Video Handler".to_string())
            }
            MediaType::Audio => {
                let ts = stream_info.codec_params.sample_rate.unwrap_or(48000);
                (ts, *b"soun", "OxiMedia Audio Handler".to_string())
            }
            _ => (1000, *b"text", "OxiMedia Text Handler".to_string()),
        };

        Self {
            stream_info,
            track_id,
            timescale,
            samples: Vec::new(),
            mdat_data: Vec::new(),
            chunk_boundaries: vec![0],
            current_chunk_samples: 0,
            max_samples_per_chunk: 10,
            total_duration: 0,
            handler_type,
            handler_name,
        }
    }

    fn add_sample(&mut self, data: &[u8], duration: u32, comp_offset: i32, is_sync: bool) {
        let chunk_index = self.chunk_boundaries.len().saturating_sub(1) as u32;
        self.samples.push(Mp4SampleEntry {
            size: data.len() as u32,
            duration,
            composition_offset: comp_offset,
            is_sync,
            chunk_index,
        });
        self.mdat_data.extend_from_slice(data);
        self.total_duration += u64::from(duration);
        self.current_chunk_samples += 1;

        if self.current_chunk_samples >= self.max_samples_per_chunk {
            self.chunk_boundaries.push(self.samples.len() as u32);
            self.current_chunk_samples = 0;
        }
    }
}

// ─── MP4 Muxer ──────────────────────────────────────────────────────────────

/// MP4/ISOBMFF muxer.
///
/// Builds a valid ISOBMFF file in memory with proper box hierarchy.
/// Supports both progressive and fragmented modes.
#[derive(Debug)]
pub struct Mp4Muxer {
    /// Configuration.
    config: Mp4Config,
    /// Per-track states.
    tracks: Vec<Mp4TrackState>,
    /// Whether the header has been written.
    header_written: bool,
    /// Fragment sequence number (for fragmented mode).
    fragment_sequence: u32,
    /// Completed fragments (for fragmented mode).
    fragments: Vec<Vec<u8>>,
}

impl Mp4Muxer {
    /// Creates a new MP4 muxer.
    #[must_use]
    pub fn new(config: Mp4Config) -> Self {
        Self {
            config,
            tracks: Vec::new(),
            header_written: false,
            fragment_sequence: 1,
            fragments: Vec::new(),
        }
    }

    /// Adds a stream/track to the muxer. Returns the track index.
    ///
    /// # Errors
    ///
    /// Returns an error if the codec is not supported or the maximum number
    /// of tracks has been exceeded.
    pub fn add_stream(&mut self, info: StreamInfo) -> OxiResult<usize> {
        if self.header_written {
            return Err(OxiError::parse(
                0,
                "Cannot add streams after header is written",
            ));
        }
        if self.tracks.len() >= MAX_TRACKS {
            return Err(OxiError::parse(0, "Maximum number of tracks exceeded"));
        }

        // Validate codec is patent-free
        validate_codec(info.codec)?;

        let track_id = (self.tracks.len() + 1) as u32;
        self.tracks.push(Mp4TrackState::new(info, track_id));
        Ok(self.tracks.len() - 1)
    }

    /// Writes the file type box. Call after adding all streams.
    ///
    /// # Errors
    ///
    /// Returns an error if no streams have been added.
    pub fn write_header(&mut self) -> OxiResult<()> {
        if self.tracks.is_empty() {
            return Err(OxiError::parse(0, "No streams added"));
        }
        self.header_written = true;
        Ok(())
    }

    /// Writes a packet to the appropriate track.
    ///
    /// # Errors
    ///
    /// Returns an error if the stream index is invalid or the header
    /// has not been written.
    pub fn write_packet(&mut self, packet: &Packet) -> OxiResult<()> {
        if !self.header_written {
            return Err(OxiError::parse(0, "Header not written yet"));
        }

        let track_idx = packet.stream_index;
        if track_idx >= self.tracks.len() {
            return Err(OxiError::parse(
                0,
                format!("Invalid stream index {track_idx}"),
            ));
        }

        let track = &self.tracks[track_idx];
        let timescale = track.timescale;

        // Calculate sample duration from packet
        let duration = packet
            .duration()
            .map(|d| convert_duration(d, &track.stream_info.timebase, timescale))
            .unwrap_or(1);

        // Composition offset: PTS - DTS
        let comp_offset = if let Some(dts) = packet.dts() {
            let pts_ticks = convert_duration(packet.pts(), &track.stream_info.timebase, timescale);
            let dts_ticks = convert_duration(dts, &track.stream_info.timebase, timescale);
            (pts_ticks as i64 - dts_ticks as i64) as i32
        } else {
            0
        };

        let is_sync = packet.is_keyframe();

        let track = &mut self.tracks[track_idx];
        track.add_sample(&packet.data, duration, comp_offset, is_sync);

        Ok(())
    }

    /// Finalizes the MP4 file and returns the complete byte output.
    ///
    /// For progressive mode, this assembles ftyp + moov + mdat.
    /// For fragmented mode, this assembles ftyp + moov (with mvex) + fragments.
    ///
    /// # Errors
    ///
    /// Returns an error if finalization fails.
    pub fn finalize(&self) -> OxiResult<Vec<u8>> {
        if !self.header_written {
            return Err(OxiError::parse(0, "Header not written"));
        }

        let mut output = Vec::new();

        // 1. ftyp box
        output.extend(self.build_ftyp());

        match self.config.mode {
            Mp4Mode::Progressive => {
                // 2. Collect all mdat data with offsets
                let (mdat_box, chunk_offsets) = self.build_mdat_progressive();

                // 3. moov box (needs chunk offsets, which depend on ftyp + moov size)
                // We need to do a two-pass: first estimate moov size, then recalc offsets
                let ftyp_size = output.len() as u64;
                let moov_estimate = self.build_moov_progressive(&chunk_offsets, ftyp_size);
                let moov_size = moov_estimate.len() as u64;

                // Recalculate offsets with correct moov size
                let mdat_start = ftyp_size + moov_size + 8; // +8 for mdat header
                let adjusted_offsets = adjust_chunk_offsets(&chunk_offsets, mdat_start);

                let moov_final = self.build_moov_progressive(&adjusted_offsets, ftyp_size);

                output.extend(moov_final);
                output.extend(mdat_box);
            }
            Mp4Mode::Fragmented => {
                // moov with mvex
                let moov = self.build_moov_fragmented();
                output.extend(moov);

                // Append all cached fragments
                for frag in &self.fragments {
                    output.extend(frag);
                }

                // Build fragments from pending samples
                let frags = self.build_fragments_from_tracks();
                for frag in frags {
                    output.extend(frag);
                }
            }
        }

        Ok(output)
    }

    /// Returns the number of tracks.
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    /// Returns a reference to a track state by index.
    #[must_use]
    pub fn track(&self, index: usize) -> Option<&Mp4TrackState> {
        self.tracks.get(index)
    }

    /// Returns the total number of samples across all tracks.
    #[must_use]
    pub fn total_samples(&self) -> usize {
        self.tracks.iter().map(|t| t.samples.len()).sum()
    }

    // ─── Box builders ───────────────────────────────────────────────────

    fn build_ftyp(&self) -> Vec<u8> {
        let mut content = Vec::new();
        content.extend_from_slice(&self.config.major_brand);
        content.extend_from_slice(&write_u32_be(self.config.minor_version));
        for brand in &self.config.compatible_brands {
            content.extend_from_slice(brand);
        }
        write_box(b"ftyp", &content)
    }

    fn build_mdat_progressive(&self) -> (Vec<u8>, Vec<Vec<u64>>) {
        let mut mdat_payload = Vec::new();
        let mut all_chunk_offsets: Vec<Vec<u64>> = Vec::new();

        for track in &self.tracks {
            let mut track_offsets = Vec::new();
            let mut sample_idx: u32 = 0;
            let mut current_offset = mdat_payload.len() as u64;

            // Process each chunk
            for (chunk_idx, &chunk_start) in track.chunk_boundaries.iter().enumerate() {
                let chunk_end = track
                    .chunk_boundaries
                    .get(chunk_idx + 1)
                    .copied()
                    .unwrap_or(track.samples.len() as u32);

                track_offsets.push(current_offset);

                for si in chunk_start..chunk_end {
                    if let Some(sample) = track.samples.get(si as usize) {
                        let start = sample_data_offset(track, sample_idx);
                        let end = start + sample.size as usize;
                        if end <= track.mdat_data.len() {
                            mdat_payload.extend_from_slice(&track.mdat_data[start..end]);
                            current_offset += u64::from(sample.size);
                        }
                    }
                    sample_idx += 1;
                }
            }

            all_chunk_offsets.push(track_offsets);
        }

        let mdat = write_box(b"mdat", &mdat_payload);
        (mdat, all_chunk_offsets)
    }

    fn build_moov_progressive(&self, chunk_offsets: &[Vec<u64>], _ftyp_size: u64) -> Vec<u8> {
        let mut content = Vec::new();

        // mvhd
        content.extend(self.build_mvhd());

        // trak boxes
        for (i, track) in self.tracks.iter().enumerate() {
            let offsets = chunk_offsets.get(i).map_or(&[][..], |v| v.as_slice());
            content.extend(self.build_trak(track, offsets));
        }

        write_box(b"moov", &content)
    }

    fn build_moov_fragmented(&self) -> Vec<u8> {
        let mut content = Vec::new();
        content.extend(self.build_mvhd());

        // mvex with trex for each track
        let mut mvex_content = Vec::new();
        for track in &self.tracks {
            mvex_content.extend(build_trex(track.track_id));
        }
        content.extend(write_box(b"mvex", &mvex_content));

        // trak boxes (with empty stbl for fragmented)
        for track in &self.tracks {
            content.extend(self.build_trak(track, &[]));
        }

        write_box(b"moov", &content)
    }

    fn build_mvhd(&self) -> Vec<u8> {
        let mut c = Vec::new();
        // creation_time
        c.extend_from_slice(&write_u32_be(self.config.creation_time as u32));
        // modification_time
        c.extend_from_slice(&write_u32_be(self.config.modification_time as u32));
        // timescale
        c.extend_from_slice(&write_u32_be(MOVIE_TIMESCALE));
        // duration
        let max_dur = self
            .tracks
            .iter()
            .map(|t| {
                if t.timescale == 0 {
                    0
                } else {
                    t.total_duration * u64::from(MOVIE_TIMESCALE) / u64::from(t.timescale)
                }
            })
            .max()
            .unwrap_or(0);
        c.extend_from_slice(&write_u32_be(max_dur as u32));
        // rate (1.0 as fixed-point 16.16)
        c.extend_from_slice(&write_u32_be(0x0001_0000));
        // volume (1.0 as fixed-point 8.8)
        c.extend_from_slice(&[0x01, 0x00]);
        // reserved (10 bytes)
        c.extend_from_slice(&[0u8; 10]);
        // matrix (identity, 36 bytes)
        c.extend_from_slice(&IDENTITY_MATRIX);
        // pre_defined (24 bytes)
        c.extend_from_slice(&[0u8; 24]);
        // next_track_id
        c.extend_from_slice(&write_u32_be((self.tracks.len() + 1) as u32));

        write_full_box(b"mvhd", 0, 0, &c)
    }

    fn build_trak(&self, track: &Mp4TrackState, chunk_offsets: &[u64]) -> Vec<u8> {
        let mut content = Vec::new();
        content.extend(build_tkhd(track));
        content.extend(self.build_mdia(track, chunk_offsets));
        write_box(b"trak", &content)
    }

    fn build_mdia(&self, track: &Mp4TrackState, chunk_offsets: &[u64]) -> Vec<u8> {
        let mut content = Vec::new();
        content.extend(build_mdhd(track));
        content.extend(build_hdlr(track));
        content.extend(self.build_minf(track, chunk_offsets));
        write_box(b"mdia", &content)
    }

    fn build_minf(&self, track: &Mp4TrackState, chunk_offsets: &[u64]) -> Vec<u8> {
        let mut content = Vec::new();

        // vmhd or smhd
        match track.stream_info.media_type {
            MediaType::Video => {
                content.extend(build_vmhd());
            }
            MediaType::Audio => {
                content.extend(build_smhd());
            }
            _ => {
                // nmhd for other types
                content.extend(write_full_box(b"nmhd", 0, 0, &[]));
            }
        }

        // dinf + dref
        content.extend(build_dinf());

        // stbl
        content.extend(self.build_stbl(track, chunk_offsets));

        write_box(b"minf", &content)
    }

    fn build_stbl(&self, track: &Mp4TrackState, chunk_offsets: &[u64]) -> Vec<u8> {
        let mut content = Vec::new();

        // stsd (sample description)
        content.extend(build_stsd(track));

        // stts (time-to-sample)
        content.extend(build_stts(track));

        // ctts (composition time offsets, if needed)
        if track.samples.iter().any(|s| s.composition_offset != 0) {
            content.extend(build_ctts(track));
        }

        // stsc (sample-to-chunk)
        content.extend(build_stsc(track));

        // stsz (sample sizes)
        content.extend(build_stsz(track));

        // stco (chunk offsets)
        content.extend(build_stco(chunk_offsets));

        // stss (sync sample table, for video with non-all-sync)
        if track.stream_info.media_type == MediaType::Video
            && track.samples.iter().any(|s| !s.is_sync)
        {
            content.extend(build_stss(track));
        }

        write_box(b"stbl", &content)
    }

    fn build_fragments_from_tracks(&self) -> Vec<Vec<u8>> {
        let mut result = Vec::new();
        let mut seq = self.fragment_sequence;

        for track in &self.tracks {
            if track.samples.is_empty() {
                continue;
            }

            // Build one moof+mdat per track
            let mut moof_content = Vec::new();

            // mfhd
            let mut mfhd_c = Vec::new();
            mfhd_c.extend_from_slice(&write_u32_be(seq));
            moof_content.extend(write_full_box(b"mfhd", 0, 0, &mfhd_c));

            // traf
            let mut traf_content = Vec::new();

            // tfhd
            let mut tfhd_c = Vec::new();
            tfhd_c.extend_from_slice(&write_u32_be(track.track_id));
            traf_content.extend(write_full_box(b"tfhd", 0, 0x020000, &tfhd_c));

            // tfdt
            let mut tfdt_c = Vec::new();
            tfdt_c.extend_from_slice(&write_u64_be(0)); // base_media_decode_time
            traf_content.extend(write_full_box(b"tfdt", 1, 0, &tfdt_c));

            // trun
            let trun_data = build_trun_for_track(track);
            traf_content.extend(trun_data);

            moof_content.extend(write_box(b"traf", &traf_content));

            let moof = write_box(b"moof", &moof_content);
            let mdat = write_box(b"mdat", &track.mdat_data);

            let mut fragment = Vec::new();
            fragment.extend(moof);
            fragment.extend(mdat);
            result.push(fragment);

            seq += 1;
        }

        result
    }
}

// ─── Box builders (standalone functions) ────────────────────────────────────

/// Identity matrix for mvhd/tkhd (9 x u32 in fixed-point 16.16 / 2.30).
const IDENTITY_MATRIX: [u8; 36] = [
    0x00, 0x01, 0x00, 0x00, // 1.0
    0x00, 0x00, 0x00, 0x00, // 0.0
    0x00, 0x00, 0x00, 0x00, // 0.0
    0x00, 0x00, 0x00, 0x00, // 0.0
    0x00, 0x01, 0x00, 0x00, // 1.0
    0x00, 0x00, 0x00, 0x00, // 0.0
    0x00, 0x00, 0x00, 0x00, // 0.0
    0x00, 0x00, 0x00, 0x00, // 0.0
    0x40, 0x00, 0x00, 0x00, // 16384.0 (1.0 in 2.30)
];

fn build_tkhd(track: &Mp4TrackState) -> Vec<u8> {
    let mut c = Vec::new();
    // creation_time
    c.extend_from_slice(&write_u32_be(0));
    // modification_time
    c.extend_from_slice(&write_u32_be(0));
    // track_id
    c.extend_from_slice(&write_u32_be(track.track_id));
    // reserved
    c.extend_from_slice(&write_u32_be(0));
    // duration in movie timescale
    let dur_ms = if track.timescale == 0 {
        0
    } else {
        track.total_duration * u64::from(MOVIE_TIMESCALE) / u64::from(track.timescale)
    };
    c.extend_from_slice(&write_u32_be(dur_ms as u32));
    // reserved (8 bytes)
    c.extend_from_slice(&[0u8; 8]);
    // layer
    c.extend_from_slice(&[0u8; 2]);
    // alternate_group
    c.extend_from_slice(&[0u8; 2]);
    // volume (audio=0x0100, video=0)
    if track.stream_info.media_type == MediaType::Audio {
        c.extend_from_slice(&[0x01, 0x00]);
    } else {
        c.extend_from_slice(&[0x00, 0x00]);
    }
    // reserved
    c.extend_from_slice(&[0u8; 2]);
    // matrix
    c.extend_from_slice(&IDENTITY_MATRIX);
    // width (fixed-point 16.16)
    let w = track.stream_info.codec_params.width.unwrap_or(0);
    c.extend_from_slice(&write_u32_be(w << 16));
    // height (fixed-point 16.16)
    let h = track.stream_info.codec_params.height.unwrap_or(0);
    c.extend_from_slice(&write_u32_be(h << 16));

    // flags=3 (track_enabled | track_in_movie)
    write_full_box(b"tkhd", 0, 3, &c)
}

fn build_mdhd(track: &Mp4TrackState) -> Vec<u8> {
    let mut c = Vec::new();
    // creation_time
    c.extend_from_slice(&write_u32_be(0));
    // modification_time
    c.extend_from_slice(&write_u32_be(0));
    // timescale
    c.extend_from_slice(&write_u32_be(track.timescale));
    // duration
    c.extend_from_slice(&write_u32_be(track.total_duration as u32));
    // language (undetermined = 0x55C4)
    c.extend_from_slice(&[0x55, 0xC4]);
    // pre_defined
    c.extend_from_slice(&[0u8; 2]);

    write_full_box(b"mdhd", 0, 0, &c)
}

fn build_hdlr(track: &Mp4TrackState) -> Vec<u8> {
    let mut c = Vec::new();
    // pre_defined
    c.extend_from_slice(&write_u32_be(0));
    // handler_type
    c.extend_from_slice(&track.handler_type);
    // reserved (12 bytes)
    c.extend_from_slice(&[0u8; 12]);
    // name (null-terminated)
    c.extend_from_slice(track.handler_name.as_bytes());
    c.push(0);

    write_full_box(b"hdlr", 0, 0, &c)
}

fn build_vmhd() -> Vec<u8> {
    let mut c = Vec::new();
    // graphicsmode
    c.extend_from_slice(&[0u8; 2]);
    // opcolor (3 x u16)
    c.extend_from_slice(&[0u8; 6]);
    write_full_box(b"vmhd", 0, 1, &c)
}

fn build_smhd() -> Vec<u8> {
    let mut c = Vec::new();
    // balance
    c.extend_from_slice(&[0u8; 2]);
    // reserved
    c.extend_from_slice(&[0u8; 2]);
    write_full_box(b"smhd", 0, 0, &c)
}

fn build_dinf() -> Vec<u8> {
    // dref with one url entry (self-contained)
    let url_box = write_full_box(b"url ", 0, 1, &[]); // flag=1 means self-contained
    let mut dref_content = Vec::new();
    dref_content.extend_from_slice(&write_u32_be(1)); // entry_count
    dref_content.extend(url_box);
    let dref = write_full_box(b"dref", 0, 0, &dref_content);

    write_box(b"dinf", &dref)
}

fn build_stsd(track: &Mp4TrackState) -> Vec<u8> {
    let mut c = Vec::new();
    c.extend_from_slice(&write_u32_be(1)); // entry_count

    match track.stream_info.media_type {
        MediaType::Video => {
            c.extend(build_video_sample_entry(track));
        }
        MediaType::Audio => {
            c.extend(build_audio_sample_entry(track));
        }
        _ => {
            // Generic sample entry
            c.extend(build_generic_sample_entry(track));
        }
    }

    write_full_box(b"stsd", 0, 0, &c)
}

fn build_video_sample_entry(track: &Mp4TrackState) -> Vec<u8> {
    let fourcc = codec_to_fourcc(track.stream_info.codec);
    let mut c = Vec::new();

    // reserved (6 bytes)
    c.extend_from_slice(&[0u8; 6]);
    // data_reference_index
    c.extend_from_slice(&[0x00, 0x01]);
    // pre_defined + reserved (16 bytes)
    c.extend_from_slice(&[0u8; 16]);
    // width
    let w = track.stream_info.codec_params.width.unwrap_or(0) as u16;
    c.extend_from_slice(&w.to_be_bytes());
    // height
    let h = track.stream_info.codec_params.height.unwrap_or(0) as u16;
    c.extend_from_slice(&h.to_be_bytes());
    // horiz_resolution (72 dpi as fixed-point 16.16)
    c.extend_from_slice(&write_u32_be(0x0048_0000));
    // vert_resolution
    c.extend_from_slice(&write_u32_be(0x0048_0000));
    // reserved
    c.extend_from_slice(&write_u32_be(0));
    // frame_count
    c.extend_from_slice(&[0x00, 0x01]);
    // compressor_name (32 bytes, padded)
    let mut comp_name = [0u8; 32];
    let name = b"OxiMedia";
    let len = name.len().min(31);
    comp_name[0] = len as u8;
    comp_name[1..1 + len].copy_from_slice(&name[..len]);
    c.extend_from_slice(&comp_name);
    // depth
    c.extend_from_slice(&[0x00, 0x18]); // 24 bit
                                        // pre_defined
    c.extend_from_slice(&[0xFF, 0xFF]);

    // Codec-specific configuration box
    if let Some(extradata) = &track.stream_info.codec_params.extradata {
        let config_fourcc = codec_config_fourcc(track.stream_info.codec);
        c.extend(write_box(&config_fourcc, extradata));
    }

    write_box(&fourcc, &c)
}

fn build_audio_sample_entry(track: &Mp4TrackState) -> Vec<u8> {
    let fourcc = codec_to_fourcc(track.stream_info.codec);
    let mut c = Vec::new();

    // reserved (6 bytes)
    c.extend_from_slice(&[0u8; 6]);
    // data_reference_index
    c.extend_from_slice(&[0x00, 0x01]);
    // reserved (8 bytes)
    c.extend_from_slice(&[0u8; 8]);
    // channel_count
    let channels = track.stream_info.codec_params.channels.unwrap_or(2) as u16;
    c.extend_from_slice(&channels.to_be_bytes());
    // sample_size (16 bits)
    c.extend_from_slice(&[0x00, 0x10]);
    // pre_defined + reserved
    c.extend_from_slice(&[0u8; 4]);
    // sample_rate (fixed-point 16.16)
    let sr = track.stream_info.codec_params.sample_rate.unwrap_or(48000);
    c.extend_from_slice(&write_u32_be(sr << 16));

    // Codec-specific configuration box
    if let Some(extradata) = &track.stream_info.codec_params.extradata {
        let config_fourcc = codec_config_fourcc(track.stream_info.codec);
        c.extend(write_box(&config_fourcc, extradata));
    }

    write_box(&fourcc, &c)
}

fn build_generic_sample_entry(track: &Mp4TrackState) -> Vec<u8> {
    let fourcc = codec_to_fourcc(track.stream_info.codec);
    let mut c = Vec::new();
    c.extend_from_slice(&[0u8; 6]); // reserved
    c.extend_from_slice(&[0x00, 0x01]); // data_reference_index
    write_box(&fourcc, &c)
}

fn build_stts(track: &Mp4TrackState) -> Vec<u8> {
    // Run-length encode sample durations
    let mut entries: Vec<(u32, u32)> = Vec::new();

    for sample in &track.samples {
        if let Some(last) = entries.last_mut() {
            if last.1 == sample.duration {
                last.0 += 1;
                continue;
            }
        }
        entries.push((1, sample.duration));
    }

    let mut c = Vec::new();
    c.extend_from_slice(&write_u32_be(entries.len() as u32));
    for (count, delta) in &entries {
        c.extend_from_slice(&write_u32_be(*count));
        c.extend_from_slice(&write_u32_be(*delta));
    }

    write_full_box(b"stts", 0, 0, &c)
}

fn build_ctts(track: &Mp4TrackState) -> Vec<u8> {
    // Run-length encode composition offsets
    let mut entries: Vec<(u32, i32)> = Vec::new();

    for sample in &track.samples {
        if let Some(last) = entries.last_mut() {
            if last.1 == sample.composition_offset {
                last.0 += 1;
                continue;
            }
        }
        entries.push((1, sample.composition_offset));
    }

    let mut c = Vec::new();
    c.extend_from_slice(&write_u32_be(entries.len() as u32));
    for (count, offset) in &entries {
        c.extend_from_slice(&write_u32_be(*count));
        c.extend_from_slice(&((*offset) as u32).to_be_bytes());
    }

    // version=1 for signed offsets
    write_full_box(b"ctts", 1, 0, &c)
}

fn build_stsc(track: &Mp4TrackState) -> Vec<u8> {
    // Build sample-to-chunk table
    let mut entries: Vec<(u32, u32, u32)> = Vec::new(); // (first_chunk, samples_per_chunk, sdi)

    for (chunk_idx, &chunk_start) in track.chunk_boundaries.iter().enumerate() {
        let chunk_end = track
            .chunk_boundaries
            .get(chunk_idx + 1)
            .copied()
            .unwrap_or(track.samples.len() as u32);
        let samples_in_chunk = chunk_end.saturating_sub(chunk_start);

        if samples_in_chunk == 0 {
            continue;
        }

        let first_chunk = (chunk_idx + 1) as u32; // 1-based
        if let Some(last) = entries.last() {
            if last.1 == samples_in_chunk {
                continue; // Same pattern as previous
            }
        }
        entries.push((first_chunk, samples_in_chunk, 1));
    }

    if entries.is_empty() {
        entries.push((1, 1, 1)); // Fallback
    }

    let mut c = Vec::new();
    c.extend_from_slice(&write_u32_be(entries.len() as u32));
    for (first_chunk, spc, sdi) in &entries {
        c.extend_from_slice(&write_u32_be(*first_chunk));
        c.extend_from_slice(&write_u32_be(*spc));
        c.extend_from_slice(&write_u32_be(*sdi));
    }

    write_full_box(b"stsc", 0, 0, &c)
}

fn build_stsz(track: &Mp4TrackState) -> Vec<u8> {
    let mut c = Vec::new();

    // Check if all samples have the same size
    let first_size = track.samples.first().map_or(0, |s| s.size);
    let all_same = track.samples.iter().all(|s| s.size == first_size);

    if all_same && !track.samples.is_empty() {
        // sample_size (common for all)
        c.extend_from_slice(&write_u32_be(first_size));
        // sample_count
        c.extend_from_slice(&write_u32_be(track.samples.len() as u32));
    } else {
        // sample_size = 0 (variable)
        c.extend_from_slice(&write_u32_be(0));
        // sample_count
        c.extend_from_slice(&write_u32_be(track.samples.len() as u32));
        // entry_size[]
        for sample in &track.samples {
            c.extend_from_slice(&write_u32_be(sample.size));
        }
    }

    write_full_box(b"stsz", 0, 0, &c)
}

fn build_stco(chunk_offsets: &[u64]) -> Vec<u8> {
    // Use stco (32-bit) if all offsets fit, otherwise co64
    let use_64bit = chunk_offsets.iter().any(|&o| o > u64::from(u32::MAX));

    if use_64bit {
        let mut c = Vec::new();
        c.extend_from_slice(&write_u32_be(chunk_offsets.len() as u32));
        for &offset in chunk_offsets {
            c.extend_from_slice(&write_u64_be(offset));
        }
        write_full_box(b"co64", 0, 0, &c)
    } else {
        let mut c = Vec::new();
        c.extend_from_slice(&write_u32_be(chunk_offsets.len() as u32));
        for &offset in chunk_offsets {
            c.extend_from_slice(&write_u32_be(offset as u32));
        }
        write_full_box(b"stco", 0, 0, &c)
    }
}

fn build_stss(track: &Mp4TrackState) -> Vec<u8> {
    let sync_samples: Vec<u32> = track
        .samples
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_sync)
        .map(|(i, _)| (i + 1) as u32) // 1-based
        .collect();

    let mut c = Vec::new();
    c.extend_from_slice(&write_u32_be(sync_samples.len() as u32));
    for &idx in &sync_samples {
        c.extend_from_slice(&write_u32_be(idx));
    }

    write_full_box(b"stss", 0, 0, &c)
}

fn build_trex(track_id: u32) -> Vec<u8> {
    let mut c = Vec::new();
    c.extend_from_slice(&write_u32_be(track_id));
    c.extend_from_slice(&write_u32_be(1)); // default_sample_description_index
    c.extend_from_slice(&write_u32_be(0)); // default_sample_duration
    c.extend_from_slice(&write_u32_be(0)); // default_sample_size
    c.extend_from_slice(&write_u32_be(0)); // default_sample_flags
    write_full_box(b"trex", 0, 0, &c)
}

fn build_trun_for_track(track: &Mp4TrackState) -> Vec<u8> {
    // flags: 0x000301 = data_offset_present | duration_present | size_present
    let flags: u32 = 0x000301;

    let mut c = Vec::new();
    c.extend_from_slice(&write_u32_be(track.samples.len() as u32)); // sample_count
    c.extend_from_slice(&write_u32_be(0)); // data_offset (placeholder)

    for sample in &track.samples {
        c.extend_from_slice(&write_u32_be(sample.duration));
        c.extend_from_slice(&write_u32_be(sample.size));
    }

    write_full_box(b"trun", 0, flags, &c)
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn validate_codec(codec: CodecId) -> OxiResult<()> {
    match codec {
        CodecId::Av1
        | CodecId::Vp9
        | CodecId::Vp8
        | CodecId::Opus
        | CodecId::Flac
        | CodecId::Vorbis => Ok(()),
        _ => Err(OxiError::PatentViolation(format!(
            "Codec {:?} is not supported in MP4 muxer (patent-free codecs only)",
            codec
        ))),
    }
}

fn codec_to_fourcc(codec: CodecId) -> [u8; 4] {
    match codec {
        CodecId::Av1 => *b"av01",
        CodecId::Vp9 => *b"vp09",
        CodecId::Vp8 => *b"vp08",
        CodecId::Opus => *b"Opus",
        CodecId::Flac => *b"fLaC",
        CodecId::Vorbis => *b"vorb",
        _ => *b"unkn",
    }
}

fn codec_config_fourcc(codec: CodecId) -> [u8; 4] {
    match codec {
        CodecId::Av1 => *b"av1C",
        CodecId::Vp9 => *b"vpcC",
        CodecId::Vp8 => *b"vpcC",
        CodecId::Opus => *b"dOps",
        CodecId::Flac => *b"dfLa",
        _ => *b"conf",
    }
}

#[allow(clippy::cast_precision_loss)]
fn convert_duration(ticks: i64, timebase: &Rational, target_timescale: u32) -> u32 {
    if timebase.den == 0 || target_timescale == 0 {
        return 1;
    }
    let seconds = (ticks as f64 * timebase.num as f64) / timebase.den as f64;
    let result = seconds * target_timescale as f64;
    if result <= 0.0 {
        1
    } else {
        result as u32
    }
}

fn sample_data_offset(track: &Mp4TrackState, sample_idx: u32) -> usize {
    let mut offset = 0usize;
    for i in 0..sample_idx as usize {
        if let Some(s) = track.samples.get(i) {
            offset += s.size as usize;
        }
    }
    offset
}

fn adjust_chunk_offsets(offsets: &[Vec<u64>], mdat_data_start: u64) -> Vec<Vec<u64>> {
    offsets
        .iter()
        .map(|track_offsets| track_offsets.iter().map(|&o| o + mdat_data_start).collect())
        .collect()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PacketFlags;
    use bytes::Bytes;
    use oximedia_core::{CodecId, Rational, Timestamp};

    fn make_video_stream() -> StreamInfo {
        let mut info = StreamInfo::new(0, CodecId::Av1, Rational::new(1, 90000));
        info.codec_params = crate::CodecParams::video(1920, 1080);
        info
    }

    fn make_audio_stream() -> StreamInfo {
        let mut info = StreamInfo::new(1, CodecId::Opus, Rational::new(1, 48000));
        info.codec_params = crate::CodecParams::audio(48000, 2);
        info
    }

    fn make_video_packet(stream_index: usize, pts: i64, keyframe: bool) -> Packet {
        let mut ts = Timestamp::new(pts, Rational::new(1, 90000));
        ts.duration = Some(3000); // 1 frame at 30fps in 90kHz
        Packet::new(
            stream_index,
            Bytes::from(vec![0xAA; 100]),
            ts,
            if keyframe {
                PacketFlags::KEYFRAME
            } else {
                PacketFlags::empty()
            },
        )
    }

    fn make_audio_packet(stream_index: usize, pts: i64) -> Packet {
        let mut ts = Timestamp::new(pts, Rational::new(1, 48000));
        ts.duration = Some(960); // 20ms of audio at 48kHz
        Packet::new(
            stream_index,
            Bytes::from(vec![0xBB; 50]),
            ts,
            PacketFlags::KEYFRAME,
        )
    }

    // ── Config tests ────────────────────────────────────────────────────

    #[test]
    fn test_mp4_config_default() {
        let config = Mp4Config::new();
        assert_eq!(config.mode, Mp4Mode::Progressive);
        assert_eq!(config.major_brand, *b"isom");
        assert_eq!(config.fragment_duration_ms, 2000);
    }

    #[test]
    fn test_mp4_config_builder() {
        let config = Mp4Config::new()
            .with_mode(Mp4Mode::Fragmented)
            .with_fragment_duration_ms(4000)
            .with_major_brand(*b"av01")
            .with_minor_version(0x100)
            .with_compatible_brand(*b"dash");

        assert_eq!(config.mode, Mp4Mode::Fragmented);
        assert_eq!(config.fragment_duration_ms, 4000);
        assert_eq!(config.major_brand, *b"av01");
        assert_eq!(config.minor_version, 0x100);
        assert!(config.compatible_brands.contains(b"dash"));
    }

    // ── Muxer lifecycle tests ───────────────────────────────────────────

    #[test]
    fn test_add_stream_returns_index() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        let idx0 = muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        let idx1 = muxer
            .add_stream(make_audio_stream())
            .expect("should succeed");
        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);
        assert_eq!(muxer.track_count(), 2);
    }

    #[test]
    fn test_add_stream_after_header_fails() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");
        let result = muxer.add_stream(make_audio_stream());
        assert!(result.is_err());
    }

    #[test]
    fn test_write_header_no_streams_fails() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        let result = muxer.write_header();
        assert!(result.is_err());
    }

    #[test]
    fn test_write_packet_before_header_fails() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        let pkt = make_video_packet(0, 0, true);
        let result = muxer.write_packet(&pkt);
        assert!(result.is_err());
    }

    #[test]
    fn test_write_packet_invalid_index_fails() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");
        let pkt = make_video_packet(99, 0, true);
        let result = muxer.write_packet(&pkt);
        assert!(result.is_err());
    }

    #[test]
    fn test_patent_encumbered_codec_rejected() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        let info = StreamInfo::new(0, CodecId::Pcm, Rational::new(1, 44100));
        let result = muxer.add_stream(info);
        assert!(result.is_err());
    }

    // ── Progressive MP4 output tests ────────────────────────────────────

    #[test]
    fn test_progressive_ftyp_present() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");
        let pkt = make_video_packet(0, 0, true);
        muxer.write_packet(&pkt).expect("should succeed");

        let output = muxer.finalize().expect("should succeed");
        // ftyp box should be first
        assert_eq!(&output[4..8], b"ftyp");
        // major brand should be "isom"
        assert_eq!(&output[8..12], b"isom");
    }

    #[test]
    fn test_progressive_contains_moov_and_mdat() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");
        muxer
            .write_packet(&make_video_packet(0, 0, true))
            .expect("should succeed");
        muxer
            .write_packet(&make_video_packet(0, 3000, false))
            .expect("should succeed");

        let output = muxer.finalize().expect("should succeed");
        let has_moov = output.windows(4).any(|w| w == b"moov");
        let has_mdat = output.windows(4).any(|w| w == b"mdat");
        assert!(has_moov, "output must contain moov box");
        assert!(has_mdat, "output must contain mdat box");
    }

    #[test]
    fn test_progressive_contains_trak_boxes() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer
            .add_stream(make_audio_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");
        muxer
            .write_packet(&make_video_packet(0, 0, true))
            .expect("should succeed");
        muxer
            .write_packet(&make_audio_packet(1, 0))
            .expect("should succeed");

        let output = muxer.finalize().expect("should succeed");
        let trak_count = output.windows(4).filter(|w| *w == b"trak").count();
        assert_eq!(trak_count, 2, "output must contain 2 trak boxes");
    }

    #[test]
    fn test_progressive_contains_stbl_tables() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");

        for i in 0..5 {
            muxer
                .write_packet(&make_video_packet(0, i * 3000, i == 0))
                .expect("should succeed");
        }

        let output = muxer.finalize().expect("should succeed");

        // Check for essential sample table boxes
        let has_stsd = output.windows(4).any(|w| w == b"stsd");
        let has_stts = output.windows(4).any(|w| w == b"stts");
        let has_stsz = output.windows(4).any(|w| w == b"stsz");
        let has_stco = output.windows(4).any(|w| w == b"stco");
        let has_stss = output.windows(4).any(|w| w == b"stss");

        assert!(has_stsd, "must have stsd");
        assert!(has_stts, "must have stts");
        assert!(has_stsz, "must have stsz");
        assert!(has_stco, "must have stco");
        assert!(has_stss, "must have stss (video with non-sync frames)");
    }

    #[test]
    fn test_progressive_mdat_contains_sample_data() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");
        muxer
            .write_packet(&make_video_packet(0, 0, true))
            .expect("should succeed");

        let output = muxer.finalize().expect("should succeed");

        // Find mdat and verify it contains our 0xAA payload
        let mdat_pos = output
            .windows(4)
            .position(|w| w == b"mdat")
            .expect("mdat must exist");
        // mdat data starts 4 bytes after the fourcc
        let mdat_start = mdat_pos + 4;
        assert!(
            output[mdat_start..].windows(4).any(|w| w == [0xAA; 4]),
            "mdat must contain sample payload"
        );
    }

    #[test]
    fn test_total_samples_count() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer
            .add_stream(make_audio_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");

        muxer
            .write_packet(&make_video_packet(0, 0, true))
            .expect("should succeed");
        muxer
            .write_packet(&make_video_packet(0, 3000, false))
            .expect("should succeed");
        muxer
            .write_packet(&make_audio_packet(1, 0))
            .expect("should succeed");

        assert_eq!(muxer.total_samples(), 3);
    }

    // ── Fragmented MP4 tests ────────────────────────────────────────────

    #[test]
    fn test_fragmented_contains_mvex() {
        let config = Mp4Config::new().with_mode(Mp4Mode::Fragmented);
        let mut muxer = Mp4Muxer::new(config);
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");
        muxer
            .write_packet(&make_video_packet(0, 0, true))
            .expect("should succeed");

        let output = muxer.finalize().expect("should succeed");
        let has_mvex = output.windows(4).any(|w| w == b"mvex");
        let has_trex = output.windows(4).any(|w| w == b"trex");
        assert!(has_mvex, "fragmented must contain mvex");
        assert!(has_trex, "fragmented must contain trex");
    }

    #[test]
    fn test_fragmented_contains_moof_mdat() {
        let config = Mp4Config::new().with_mode(Mp4Mode::Fragmented);
        let mut muxer = Mp4Muxer::new(config);
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");
        muxer
            .write_packet(&make_video_packet(0, 0, true))
            .expect("should succeed");

        let output = muxer.finalize().expect("should succeed");
        let has_moof = output.windows(4).any(|w| w == b"moof");
        let has_mdat = output.windows(4).any(|w| w == b"mdat");
        assert!(has_moof, "fragmented must contain moof");
        assert!(has_mdat, "fragmented must contain mdat");
    }

    #[test]
    fn test_fragmented_contains_traf_trun() {
        let config = Mp4Config::new().with_mode(Mp4Mode::Fragmented);
        let mut muxer = Mp4Muxer::new(config);
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");
        muxer
            .write_packet(&make_video_packet(0, 0, true))
            .expect("should succeed");

        let output = muxer.finalize().expect("should succeed");
        let has_traf = output.windows(4).any(|w| w == b"traf");
        let has_trun = output.windows(4).any(|w| w == b"trun");
        assert!(has_traf, "fragmented must contain traf");
        assert!(has_trun, "fragmented must contain trun");
    }

    // ── Helper function tests ───────────────────────────────────────────

    #[test]
    fn test_validate_codec_accepted() {
        assert!(validate_codec(CodecId::Av1).is_ok());
        assert!(validate_codec(CodecId::Vp9).is_ok());
        assert!(validate_codec(CodecId::Opus).is_ok());
        assert!(validate_codec(CodecId::Flac).is_ok());
    }

    #[test]
    fn test_validate_codec_rejected() {
        assert!(validate_codec(CodecId::Pcm).is_err());
    }

    #[test]
    fn test_codec_to_fourcc() {
        assert_eq!(codec_to_fourcc(CodecId::Av1), *b"av01");
        assert_eq!(codec_to_fourcc(CodecId::Vp9), *b"vp09");
        assert_eq!(codec_to_fourcc(CodecId::Opus), *b"Opus");
    }

    #[test]
    fn test_convert_duration_basic() {
        let tb = Rational::new(1, 90000);
        let result = convert_duration(90000, &tb, 1000);
        assert_eq!(result, 1000); // 1 second
    }

    #[test]
    fn test_convert_duration_zero_target() {
        let tb = Rational::new(1, 90000);
        let result = convert_duration(100, &tb, 0);
        assert_eq!(result, 1); // fallback
    }

    #[test]
    fn test_sample_data_offset() {
        let mut track = Mp4TrackState::new(make_video_stream(), 1);
        track.add_sample(&[0xAA; 100], 3000, 0, true);
        track.add_sample(&[0xBB; 200], 3000, 0, false);
        track.add_sample(&[0xCC; 150], 3000, 0, false);

        assert_eq!(sample_data_offset(&track, 0), 0);
        assert_eq!(sample_data_offset(&track, 1), 100);
        assert_eq!(sample_data_offset(&track, 2), 300);
    }

    #[test]
    fn test_adjust_chunk_offsets() {
        let offsets = vec![vec![0, 100, 200], vec![0, 50]];
        let adjusted = adjust_chunk_offsets(&offsets, 1000);
        assert_eq!(adjusted[0], vec![1000, 1100, 1200]);
        assert_eq!(adjusted[1], vec![1000, 1050]);
    }

    #[test]
    fn test_build_stts_run_length() {
        let mut track = Mp4TrackState::new(make_video_stream(), 1);
        // 5 samples all with duration 3000
        for _ in 0..5 {
            track.add_sample(&[0u8; 10], 3000, 0, true);
        }
        let stts = build_stts(&track);
        // Should have entry_count=1 (all same duration)
        // fullbox header (12) + entry_count (4) + one entry (8) = 24
        assert_eq!(stts.len(), 24);
    }

    #[test]
    fn test_build_stsz_uniform() {
        let mut track = Mp4TrackState::new(make_video_stream(), 1);
        for _ in 0..3 {
            track.add_sample(&[0u8; 42], 3000, 0, true);
        }
        let stsz = build_stsz(&track);
        // With uniform sizes: fullbox header (12) + sample_size (4) + sample_count (4) = 20
        assert_eq!(stsz.len(), 20);
    }

    #[test]
    fn test_build_stsz_variable() {
        let mut track = Mp4TrackState::new(make_video_stream(), 1);
        track.add_sample(&[0u8; 100], 3000, 0, true);
        track.add_sample(&[0u8; 200], 3000, 0, false);
        let stsz = build_stsz(&track);
        // Variable: fullbox header (12) + sample_size=0 (4) + sample_count (4) + 2*4 = 28
        assert_eq!(stsz.len(), 28);
    }

    #[test]
    fn test_build_stss_sync_samples() {
        let mut track = Mp4TrackState::new(make_video_stream(), 1);
        track.add_sample(&[0u8; 10], 3000, 0, true);
        track.add_sample(&[0u8; 10], 3000, 0, false);
        track.add_sample(&[0u8; 10], 3000, 0, false);
        track.add_sample(&[0u8; 10], 3000, 0, true);
        let stss = build_stss(&track);
        // 2 sync samples: fullbox header (12) + entry_count (4) + 2*4 = 24
        assert_eq!(stss.len(), 24);
    }

    #[test]
    fn test_track_state_handler_types() {
        let video_track = Mp4TrackState::new(make_video_stream(), 1);
        assert_eq!(video_track.handler_type, *b"vide");

        let audio_track = Mp4TrackState::new(make_audio_stream(), 2);
        assert_eq!(audio_track.handler_type, *b"soun");
    }

    #[test]
    fn test_progressive_multi_packet_output_valid() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer
            .add_stream(make_audio_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");

        // Interleaved packets
        for i in 0..10 {
            muxer
                .write_packet(&make_video_packet(0, i * 3000, i == 0 || i == 5))
                .expect("should succeed");
            muxer
                .write_packet(&make_audio_packet(1, i * 960))
                .expect("should succeed");
        }

        let output = muxer.finalize().expect("should succeed");
        assert!(!output.is_empty());

        // Verify box structure: ftyp must come first
        assert_eq!(&output[4..8], b"ftyp");

        // Must contain all essential boxes
        let has_mvhd = output.windows(4).any(|w| w == b"mvhd");
        let has_tkhd = output.windows(4).any(|w| w == b"tkhd");
        let has_mdhd = output.windows(4).any(|w| w == b"mdhd");
        let has_hdlr = output.windows(4).any(|w| w == b"hdlr");
        assert!(has_mvhd);
        assert!(has_tkhd);
        assert!(has_mdhd);
        assert!(has_hdlr);
    }

    #[test]
    fn test_finalize_before_header_fails() {
        let muxer = Mp4Muxer::new(Mp4Config::new());
        assert!(muxer.finalize().is_err());
    }

    #[test]
    fn test_empty_tracks_finalize() {
        let mut muxer = Mp4Muxer::new(Mp4Config::new());
        muxer
            .add_stream(make_video_stream())
            .expect("should succeed");
        muxer.write_header().expect("should succeed");
        // No packets written
        let output = muxer.finalize().expect("should succeed");
        assert!(!output.is_empty());
        assert_eq!(&output[4..8], b"ftyp");
    }
}
