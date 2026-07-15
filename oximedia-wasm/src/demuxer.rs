//! Demuxer for WASM.
//!
//! This module provides a synchronous demuxer interface for extracting
//! packets from media containers in the browser.
//!
//! Unlike the async demuxers in the main library, this uses synchronous
//! operations on in-memory data, which is suitable for the WASM
//! single-threaded environment.
//!
//! # Honesty contract
//!
//! [`WasmDemuxer`] never fabricates stream or packet data. Full container
//! parsing (EBML/Ogg-page/MP4-box/etc.) is not yet wired into the WASM
//! build for any format -- see the `TODO(0.2.x)` marker on each
//! `parse_*_headers` method below for the concrete real parser each format
//! should eventually delegate to. In short: `oximedia-container`'s
//! per-format demuxers (`oximedia_container::demux::{matroska,ogg,flac,wav,mp4}`)
//! are generic over `oximedia_io::MediaSource`, and
//! `oximedia_io::source::MemorySource` is an in-memory, non-fs, non-thread
//! `Bytes`-backed implementation of that trait (`oximedia-container` and
//! `oximedia-io` already gate their `tokio` dependency off for
//! `wasm32` targets). `MediaSource` is an `async_trait`, but
//! `MemorySource`'s `read`/`seek` never actually suspend (they're plain
//! memory copies), so the resulting futures always resolve on the first
//! `poll` and could be driven synchronously with a small no-op-`Waker`
//! `block_on` helper (`std::task::Wake` + `Context::from_waker` +
//! `std::pin::pin!`; no `unsafe`, no executor crate needed) instead of a
//! real async runtime. This was not wired up in this pass because it
//! requires adding `oximedia-container` (and `oximedia-io`) as new
//! dependencies of this crate, which is outside this pass's dependency
//! budget. Until that lands, [`WasmDemuxer::probe`] honestly returns `Err`
//! for every format rather than inventing a plausible-looking stream. The
//! raw magic-byte format sniff ([`probe_format`](crate::probe::probe_format),
//! used internally by [`WasmDemuxer::probe`]) is real and unaffected.

use crate::container::ContainerFormat;
use bytes::Bytes;
use wasm_bindgen::prelude::*;

use crate::io::ByteSource;
use crate::types::{WasmPacket, WasmStreamInfo};
use crate::utils::to_js_error;

/// WASM-compatible demuxer.
///
/// Provides synchronous demuxing of media containers from in-memory data.
///
/// # Current status
///
/// Magic-byte format detection ([`probe_format`](crate::probe::probe_format))
/// is real. Full per-container header/packet parsing is not yet wired in
/// (see the module-level "Honesty contract" docs), so
/// [`probe`](Self::probe) currently returns `Err` for every supported
/// format instead of fabricating stream/packet data.
///
/// # JavaScript Example
///
/// ```javascript
/// import * as oximedia from 'oximedia-wasm';
///
/// // Load file data
/// const response = await fetch('video.webm');
/// const arrayBuffer = await response.arrayBuffer();
/// const data = new Uint8Array(arrayBuffer);
///
/// // Create demuxer
/// const demuxer = new oximedia.WasmDemuxer(data);
///
/// // Probe format. NOTE: this currently always throws -- full container
/// // parsing is not wired in yet (see module docs) -- so real callers
/// // must handle the rejection rather than assume success.
/// try {
///     const probe = demuxer.probe();
///     console.log('Format:', probe.format());
///
///     const streams = demuxer.streams();
///     console.log('Found', streams.length, 'streams');
///     for (const stream of streams) {
///         console.log(`Stream ${stream.index()}: ${stream.codec()}`);
///     }
///
///     let count = 0;
///     while (true) {
///         const packet = demuxer.read_packet();
///         if (!packet) break;
///         console.log(`Packet ${count++}: stream=${packet.stream_index()}, size=${packet.size()}`);
///     }
/// } catch (e) {
///     console.error('Full demux not yet available for this build:', e);
/// }
/// ```
#[wasm_bindgen]
pub struct WasmDemuxer {
    source: ByteSource,
    format: Option<ContainerFormat>,
    streams: Vec<WasmStreamInfo>,
    probed: bool,
}

#[wasm_bindgen]
impl WasmDemuxer {
    /// Creates a new demuxer from raw bytes.
    ///
    /// # Arguments
    ///
    /// * `data` - The complete media file data as a `Uint8Array`
    ///
    /// # Example
    ///
    /// ```javascript
    /// const data = new Uint8Array([...]);
    /// const demuxer = new oximedia.WasmDemuxer(data);
    /// ```
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(data: &[u8]) -> Self {
        let bytes = Bytes::copy_from_slice(data);
        Self {
            source: ByteSource::new(bytes),
            format: None,
            streams: Vec::new(),
            probed: false,
        }
    }

    /// Probes the format and parses container headers.
    ///
    /// This must be called before reading packets. It detects the container
    /// format and extracts stream information.
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception if the format cannot be detected or
    /// headers are invalid.
    ///
    /// # Example
    ///
    /// ```javascript
    /// const probe = demuxer.probe();
    /// console.log('Format:', probe.format());
    /// console.log('Confidence:', probe.confidence());
    /// ```
    pub fn probe(&mut self) -> Result<crate::probe::WasmProbeResult, JsValue> {
        // Read first bytes for format detection
        let mut header = [0u8; 32];
        let n = self.source.read(&mut header).map_err(to_js_error)?;

        // Probe format
        let result = crate::container::probe_format(&header[..n]).map_err(to_js_error)?;

        self.format = Some(result.format);

        // Reset to beginning
        self.source
            .seek(std::io::SeekFrom::Start(0))
            .map_err(to_js_error)?;

        // Parse headers based on format
        self.parse_headers()?;
        self.probed = true;

        Ok(crate::probe::WasmProbeResult::new_internal(
            result.format,
            result.confidence,
        ))
    }

    /// Returns information about all streams.
    ///
    /// This is only valid after `probe()` has been called.
    ///
    /// # Example
    ///
    /// ```javascript
    /// const streams = demuxer.streams();
    /// for (const stream of streams) {
    ///     console.log(`Stream ${stream.index()}: ${stream.codec()}`);
    /// }
    /// ```
    #[must_use]
    pub fn streams(&self) -> Vec<WasmStreamInfo> {
        self.streams.clone()
    }

    /// Reads the next packet from the container.
    ///
    /// Returns `null` when there are no more packets (EOF).
    ///
    /// # Errors
    ///
    /// Throws a JavaScript exception for parse failures or I/O errors.
    ///
    /// # Example
    ///
    /// ```javascript
    /// while (true) {
    ///     const packet = demuxer.read_packet();
    ///     if (!packet) break;
    ///     console.log('Packet size:', packet.size());
    /// }
    /// ```
    pub fn read_packet(&mut self) -> Result<Option<WasmPacket>, JsValue> {
        if !self.probed {
            return Err(crate::utils::js_err(
                "Must call probe() before reading packets",
            ));
        }

        // Check if we're at EOF
        if self.source.is_eof() {
            return Ok(None);
        }

        // Read packet based on format
        match self.format {
            Some(ContainerFormat::Matroska) => self.read_matroska_packet(),
            Some(ContainerFormat::Ogg) => self.read_ogg_packet(),
            Some(ContainerFormat::Flac) => self.read_flac_packet(),
            Some(ContainerFormat::Wav) => self.read_wav_packet(),
            Some(ContainerFormat::Mp4) => self.read_mp4_packet(),
            None => Err(crate::utils::js_err("No format detected")),
        }
    }

    /// Returns the total size of the media data in bytes.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.source.size()
    }

    /// Returns the current read position in bytes.
    #[must_use]
    pub fn position(&self) -> u64 {
        self.source.position()
    }

    /// Returns true if all packets have been read.
    #[must_use]
    pub fn is_eof(&self) -> bool {
        self.source.is_eof()
    }
}

// Private implementation methods
impl WasmDemuxer {
    /// Parse container headers to extract stream information.
    fn parse_headers(&mut self) -> Result<(), JsValue> {
        match self.format {
            Some(ContainerFormat::Matroska) => self.parse_matroska_headers(),
            Some(ContainerFormat::Ogg) => self.parse_ogg_headers(),
            Some(ContainerFormat::Flac) => self.parse_flac_headers(),
            Some(ContainerFormat::Wav) => self.parse_wav_headers(),
            Some(ContainerFormat::Mp4) => self.parse_mp4_headers(),
            None => Err(crate::utils::js_err("No format detected")),
        }
    }

    /// Parses Matroska/`WebM` headers.
    ///
    /// # Honesty contract
    ///
    /// Real EBML/segment/cluster parsing is not wired in yet. This
    /// deliberately returns `Err` instead of fabricating a plausible-looking
    /// stream (the previous implementation always guessed a single VP9
    /// video stream here, regardless of the container's actual tracks).
    ///
    /// TODO(0.2.x): wire in-memory `oximedia-container` demux for
    /// Matroska/WebM (`oximedia_container::demux::matroska::MatroskaDemuxer`)
    /// -- see the module-level docs for the concrete approach.
    fn parse_matroska_headers(&mut self) -> Result<(), JsValue> {
        Err(crate::utils::js_err(
            "demux for Matroska/WebM not yet available in the WASM build — see TODO(0.2.x) in demuxer.rs",
        ))
    }

    /// Parses Ogg headers.
    ///
    /// # Honesty contract
    /// See [`parse_matroska_headers`](Self::parse_matroska_headers).
    ///
    /// TODO(0.2.x): wire in-memory `oximedia-container` demux for Ogg
    /// (`oximedia_container::demux::ogg::OggDemuxer`).
    fn parse_ogg_headers(&mut self) -> Result<(), JsValue> {
        Err(crate::utils::js_err(
            "demux for Ogg not yet available in the WASM build — see TODO(0.2.x) in demuxer.rs",
        ))
    }

    /// Parses FLAC headers.
    ///
    /// # Honesty contract
    /// See [`parse_matroska_headers`](Self::parse_matroska_headers).
    ///
    /// TODO(0.2.x): wire in-memory `oximedia-container` demux for FLAC
    /// (`oximedia_container::demux::flac::FlacDemuxer`).
    fn parse_flac_headers(&mut self) -> Result<(), JsValue> {
        Err(crate::utils::js_err(
            "demux for FLAC not yet available in the WASM build — see TODO(0.2.x) in demuxer.rs",
        ))
    }

    /// Parses WAV headers.
    ///
    /// # Honesty contract
    /// See [`parse_matroska_headers`](Self::parse_matroska_headers).
    ///
    /// TODO(0.2.x): wire in-memory `oximedia-container` demux for WAV
    /// (`oximedia_container::demux::wav::WavDemuxer`).
    fn parse_wav_headers(&mut self) -> Result<(), JsValue> {
        Err(crate::utils::js_err(
            "demux for WAV not yet available in the WASM build — see TODO(0.2.x) in demuxer.rs",
        ))
    }

    /// Parses MP4 headers.
    ///
    /// # Honesty contract
    /// See [`parse_matroska_headers`](Self::parse_matroska_headers).
    ///
    /// TODO(0.2.x): wire in-memory `oximedia-container` demux for MP4
    /// (`oximedia_container::demux::mp4::Mp4Demuxer`; AV1/VP9 only, per
    /// that demuxer's patent-free restriction).
    fn parse_mp4_headers(&mut self) -> Result<(), JsValue> {
        Err(crate::utils::js_err(
            "demux for MP4 not yet available in the WASM build — see TODO(0.2.x) in demuxer.rs",
        ))
    }

    /// Reads a Matroska packet.
    ///
    /// # Honesty contract
    ///
    /// `probe()` always fails before `read_packet()` can reach this (see
    /// [`parse_matroska_headers`](Self::parse_matroska_headers)), since
    /// `probed` is only set `true` after a successful header parse. Kept as
    /// an explicit honest `Err` -- rather than deleted -- as defense in
    /// depth: it must never resurrect the previous behavior of wrapping a
    /// raw 1024-byte window of file bytes in a fake `Packet` if a future
    /// refactor changes the `probed` gating.
    fn read_matroska_packet(&mut self) -> Result<Option<WasmPacket>, JsValue> {
        self.read_unavailable_packet("Matroska/WebM")
    }

    /// Reads an Ogg packet. See [`read_matroska_packet`](Self::read_matroska_packet).
    fn read_ogg_packet(&mut self) -> Result<Option<WasmPacket>, JsValue> {
        self.read_unavailable_packet("Ogg")
    }

    /// Reads a FLAC packet. See [`read_matroska_packet`](Self::read_matroska_packet).
    fn read_flac_packet(&mut self) -> Result<Option<WasmPacket>, JsValue> {
        self.read_unavailable_packet("FLAC")
    }

    /// Reads a WAV packet. See [`read_matroska_packet`](Self::read_matroska_packet).
    fn read_wav_packet(&mut self) -> Result<Option<WasmPacket>, JsValue> {
        self.read_unavailable_packet("WAV")
    }

    /// Reads an MP4 packet. See [`read_matroska_packet`](Self::read_matroska_packet).
    fn read_mp4_packet(&mut self) -> Result<Option<WasmPacket>, JsValue> {
        self.read_unavailable_packet("MP4")
    }

    /// Honest "not yet available" packet read shared by all formats whose
    /// container-level packet demux is not wired to a real parser yet.
    ///
    /// Never fabricates packet data -- the previous implementation wrapped
    /// an arbitrary raw byte window read from the source in a fake
    /// `Packet` with `stream_index: 0` and `pts: 0`, regardless of actual
    /// container structure.
    fn read_unavailable_packet(&self, format_name: &str) -> Result<Option<WasmPacket>, JsValue> {
        Err(crate::utils::js_err(&format!(
            "demux for {format_name} not yet available in the WASM build — see TODO(0.2.x) in demuxer.rs"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demuxer_new() {
        let data = vec![0x1A, 0x45, 0xDF, 0xA3, 0, 0, 0, 0];
        let demuxer = WasmDemuxer::new(&data);
        assert_eq!(demuxer.size(), 8);
        assert_eq!(demuxer.position(), 0);
    }

    /// `probe()` must honestly fail rather than fabricate a Matroska
    /// stream: real EBML/cluster parsing is not wired in yet (see the
    /// module-level "Honesty contract" docs and the `TODO(0.2.x)` markers
    /// on each `parse_*_headers` method). This is a regression test for
    /// the exact bug reported: `probe()` used to succeed and silently
    /// invent a single VP9 video stream.
    #[test]
    fn test_demuxer_probe_matroska_is_honest_err() {
        let data = vec![0x1A, 0x45, 0xDF, 0xA3, 0, 0, 0, 0];
        let mut demuxer = WasmDemuxer::new(&data);
        let result = demuxer.probe();
        assert!(
            result.is_err(),
            "probe() must not fabricate a stream for a format that isn't really parsed"
        );
        assert!(
            demuxer.streams().is_empty(),
            "no stream list should ever be fabricated"
        );
    }

    /// The underlying magic-byte format sniff is real and must keep
    /// working even though the full per-container header parse above
    /// honestly fails -- these are two different layers.
    #[test]
    fn test_raw_format_sniff_still_detects_matroska() {
        let data = [0x1A, 0x45, 0xDF, 0xA3, 0, 0, 0, 0];
        let result = crate::container::probe_format(&data).expect("magic bytes should be detected");
        assert_eq!(result.format, ContainerFormat::Matroska);
        assert!(result.confidence > 0.9);
    }

    /// Every currently-supported format must honestly error from `probe()`
    /// -- none of them are wired to a real per-container parser yet. This
    /// guards against fabrication silently creeping back in for any one
    /// format while the others are fixed.
    #[test]
    fn test_demuxer_probe_is_honest_err_for_all_formats() {
        let cases: &[(&[u8], &str)] = &[
            (&[0x1A, 0x45, 0xDF, 0xA3, 0, 0, 0, 0], "Matroska"),
            (b"OggS\x00\x02\x00\x00\x00\x00\x00\x00", "Ogg"),
            (b"fLaC\x00\x00\x00\x22", "Flac"),
            (b"RIFF\x00\x00\x00\x00WAVEfmt ", "Wav"),
            (b"\x00\x00\x00\x18ftypisom\x00\x00\x02\x00", "Mp4"),
        ];

        for (data, label) in cases {
            let mut demuxer = WasmDemuxer::new(data);
            let result = demuxer.probe();
            assert!(
                result.is_err(),
                "{label}: probe() fabricated success instead of an honest error"
            );
            assert!(
                demuxer.streams().is_empty(),
                "{label}: no stream list should ever be fabricated"
            );
        }
    }

    #[test]
    fn test_read_packet_before_probe_still_errors() {
        let data = vec![0x1A, 0x45, 0xDF, 0xA3, 0, 0, 0, 0];
        let mut demuxer = WasmDemuxer::new(&data);
        let result = demuxer.read_packet();
        assert!(result.is_err());
    }
}
