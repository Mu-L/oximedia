//! Incremental streaming demuxer for WASM.
//!
//! This module provides a chunk-oriented demuxer that can accept data
//! progressively — suitable for network streaming scenarios in the browser
//! where bytes arrive in pieces (e.g. `ReadableStream` chunks) rather than
//! as a complete file upfront.
//!
//! # Design
//!
//! Unlike [`WasmDemuxer`](crate::demuxer::WasmDemuxer) which requires the
//! full file in memory at construction time, `WasmStreamingDemuxer` maintains
//! an internal growable ring buffer.  Callers push chunks with
//! [`append_data`](WasmStreamingDemuxer::append_data) and pull packets with
//! [`read_packet`](WasmStreamingDemuxer::read_packet).  The demuxer signals
//! "not enough data yet" by returning `null` from `read_packet`.
//!
//! # Honesty contract
//!
//! Like [`WasmDemuxer`](crate::demuxer::WasmDemuxer) (see its module docs
//! for the concrete real-parser path), this never fabricates stream or
//! packet data. Real per-container header/frame parsing is not wired in
//! for any format yet, so [`read_packet`](WasmStreamingDemuxer::read_packet)
//! returns `null` while there is genuinely not enough data buffered to
//! attempt a probe, and honestly throws once enough bytes have accumulated
//! to attempt one -- it never invents a stream list or wraps an arbitrary
//! byte window in a fake packet.
//!
//! # JavaScript Example
//!
//! ```javascript
//! import * as oximedia from 'oximedia-wasm';
//!
//! const sd = new oximedia.WasmStreamingDemuxer("webm");
//! try {
//!     for await (const chunk of response.body) {
//!         sd.append_data(chunk);
//!         let packet;
//!         while ((packet = sd.read_packet()) !== null) {
//!             console.log('Packet size:', packet.size());
//!         }
//!     }
//!     sd.flush();
//! } catch (e) {
//!     // Expected today: full per-container demux is not wired in yet
//!     // (see module docs), so `read_packet` throws once enough bytes
//!     // have accumulated to attempt a real probe.
//!     console.error('Full demux not yet available for this build:', e);
//! }
//! ```

use wasm_bindgen::prelude::*;

use crate::container::ContainerFormat;
use crate::types::{WasmPacket, WasmStreamInfo};

// ---------------------------------------------------------------------------
// Internal ring-buffer of chunks

/// Maximum number of bytes we buffer before forcing a read.
///
/// After this threshold, old data in the front of the buffer is
/// discarded up to the current read cursor so memory does not grow
/// without bound.
const MAX_BUFFER_BYTES: usize = 32 * 1024 * 1024; // 32 MiB

// ---------------------------------------------------------------------------

/// Incremental (streaming) demuxer WASM binding.
///
/// Accepts data chunks pushed from JavaScript and emits packets as soon as
/// enough data is available to complete a container frame boundary.
///
/// The demuxer operates fully synchronously — no async/await or threads.
///
/// # Supported format strings
///
/// | String | Container |
/// |--------|-----------|
/// | `"webm"` / `"matroska"` / `"mkv"` | Matroska / WebM |
/// | `"ogg"` | Ogg |
/// | `"flac"` | FLAC |
/// | `"wav"` | WAV |
/// | `"mp4"` / `"mov"` | MP4 / ISOBMFF |
#[wasm_bindgen]
pub struct WasmStreamingDemuxer {
    /// Accumulated bytes not yet consumed by packet reader.
    buffer: Vec<u8>,
    /// Byte offset of `buffer[0]` in the conceptual stream.
    buffer_start_offset: u64,
    /// Read cursor within `buffer` (relative index).
    read_cursor: usize,
    /// Detected/declared container format.
    format: ContainerFormat,
    /// Whether `probe()` has been completed (i.e. we have seen enough header bytes).
    probed: bool,
    /// Streams discovered during probing. Always empty today -- see the
    /// module-level "Honesty contract" docs.
    streams: Vec<WasmStreamInfo>,
    /// Packet sequence counter. Always `0` today since no packet is ever
    /// really demuxed yet -- see the module-level "Honesty contract" docs.
    packet_count: u64,
    /// Whether `flush()` has been called — no more data will arrive.
    flushed: bool,
}

#[wasm_bindgen]
impl WasmStreamingDemuxer {
    /// Create a streaming demuxer for the specified container format.
    ///
    /// # Arguments
    ///
    /// * `format_hint` - A case-insensitive string identifying the container
    ///   format (e.g. `"webm"`, `"ogg"`, `"wav"`).  The format is used to
    ///   guide packet parsing without requiring the entire file to be present.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if `format_hint` is not recognised.
    #[wasm_bindgen(constructor)]
    pub fn new(format_hint: &str) -> Result<WasmStreamingDemuxer, JsValue> {
        let format = parse_format_hint(format_hint).map_err(|e| crate::utils::js_err(&e))?;
        Ok(Self {
            buffer: Vec::new(),
            buffer_start_offset: 0,
            read_cursor: 0,
            format,
            probed: false,
            streams: Vec::new(),
            packet_count: 0,
            flushed: false,
        })
    }

    /// Append a chunk of raw bytes to the internal buffer.
    ///
    /// This method is cheap — it simply extends the buffer.  No parsing is
    /// performed until `read_packet()` is called.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer would exceed the 32 MiB safety limit
    /// after appending.
    pub fn append_data(&mut self, chunk: &[u8]) -> Result<(), JsValue> {
        self.append_data_inner(chunk)
            .map_err(|e| crate::utils::js_err(&e))
    }

    /// Attempt to read the next available packet from the buffer.
    ///
    /// Returns `null` if there is not yet enough data to form a complete
    /// packet — the caller should push more chunks and try again.
    ///
    /// Returns `null` after `flush()` has been called and the buffer is
    /// exhausted.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error on unrecoverable parse failures.
    pub fn read_packet(&mut self) -> Result<Option<WasmPacket>, JsValue> {
        self.read_packet_inner()
            .map_err(|e| crate::utils::js_err(&e))
    }

    /// Returns stream information discovered during header probing.
    ///
    /// The slice may be empty until enough header data has been received.
    pub fn streams(&self) -> Vec<WasmStreamInfo> {
        self.streams.clone()
    }

    /// Signal that no more data will be appended.
    ///
    /// After calling `flush()`, callers should drain remaining packets by
    /// calling `read_packet()` until it returns `null`.
    pub fn flush(&mut self) {
        self.flushed = true;
    }

    /// Returns the total number of bytes that have been consumed (read) so far.
    pub fn bytes_consumed(&self) -> u64 {
        self.buffer_start_offset + self.read_cursor as u64
    }

    /// Returns the number of bytes currently held in the buffer (including
    /// already-consumed but not yet evicted bytes).
    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns the number of packets emitted so far.
    pub fn packets_emitted(&self) -> u64 {
        self.packet_count
    }

    /// Returns `true` once `flush()` has been called and the buffer is fully
    /// drained.
    pub fn is_done(&self) -> bool {
        self.flushed && self.read_cursor >= self.buffer.len()
    }
}

// ---------------------------------------------------------------------------
// Private helpers

impl WasmStreamingDemuxer {
    /// Move already-consumed bytes out of the front of the buffer.
    fn compact_buffer(&mut self) {
        if self.read_cursor == 0 {
            return;
        }
        let compacted_amount = self.read_cursor;
        self.buffer.drain(..compacted_amount);
        self.buffer_start_offset += compacted_amount as u64;
        self.read_cursor = 0;
    }

    /// Inner impl of `append_data` — returns `String` error, no `JsValue`.
    fn append_data_inner(&mut self, chunk: &[u8]) -> Result<(), String> {
        self.compact_buffer();
        let pending_bytes = self.buffer.len() - self.read_cursor;
        if pending_bytes + chunk.len() > MAX_BUFFER_BYTES {
            return Err(
                "WasmStreamingDemuxer: buffer overflow — call read_packet() more frequently"
                    .to_string(),
            );
        }
        self.buffer.extend_from_slice(chunk);
        Ok(())
    }

    /// Inner impl of `read_packet` — returns `String` error, no `JsValue`.
    ///
    /// # Honesty contract
    ///
    /// Never fabricates a packet. [`try_probe`](Self::try_probe) never
    /// actually sets `probed = true` today (real per-container parsing is
    /// not wired in), so this deliberately has no code path that can wrap
    /// a raw byte window in a fake `Packet` -- the previous implementation
    /// did exactly that (a format-aware but content-blind byte-chunking
    /// heuristic with a synthetic incrementing PTS).
    fn read_packet_inner(&mut self) -> Result<Option<WasmPacket>, String> {
        if !self.probed {
            self.try_probe()?;
        }
        // `try_probe` above either returned `Err` (enough bytes to attempt
        // a probe, but real per-container header parsing is not wired in
        // yet) or left `probed == false` (not enough bytes buffered to
        // even attempt it, i.e. genuinely "keep streaming, ask again
        // later"). Either way `probed` can never be `true` here, so there
        // is nothing honest left to return except "still waiting".
        Ok(None)
    }

    /// Attempts to probe the container format from accumulated header
    /// bytes.
    ///
    /// # Honesty contract
    ///
    /// Never fabricates a stream list. The previous implementation
    /// invented a hardcoded stream set per format (e.g. always guessing a
    /// VP9 video + Opus audio pair for Matroska/WebM, regardless of the
    /// container's actual tracks), which is exactly the kind of fabricated
    /// success this WASM binding must not produce. Real per-container
    /// header parsing is not wired in yet (see
    /// [`WasmDemuxer`](crate::demuxer::WasmDemuxer)'s module docs for the
    /// concrete real-parser path), so once enough bytes have accumulated
    /// to plausibly attempt a probe, this honestly reports failure instead.
    ///
    /// TODO(0.2.x): wire real per-container header parsing once
    /// `oximedia-container`'s in-memory demuxers become a dependency of
    /// this crate.
    fn try_probe(&mut self) -> Result<(), String> {
        // Wait for enough bytes before even attempting -- with too little
        // data buffered we genuinely don't know anything yet, which is
        // different from "we know we can't parse this build."
        if self.buffer.len() < 32 {
            return Ok(());
        }

        Err(format!(
            "WasmStreamingDemuxer: header probe for {:?} not yet available in the WASM build — see TODO(0.2.x) in streaming_demuxer.rs",
            self.format
        ))
    }
}

/// Parse a user-supplied format hint string into a `ContainerFormat`.
///
/// Returns `Err(String)` (not `JsValue`) so it can be called from native tests.
fn parse_format_hint(hint: &str) -> Result<ContainerFormat, String> {
    match hint.to_lowercase().as_str() {
        "webm" | "matroska" | "mkv" => Ok(ContainerFormat::Matroska),
        "ogg" | "oga" | "ogv" => Ok(ContainerFormat::Ogg),
        "flac" => Ok(ContainerFormat::Flac),
        "wav" | "wave" => Ok(ContainerFormat::Wav),
        "mp4" | "mov" | "m4v" | "m4a" | "isobmff" => Ok(ContainerFormat::Mp4),
        other => Err(format!(
            "WasmStreamingDemuxer: unrecognised format hint '{other}'. \
             Supported values: webm, matroska, mkv, ogg, flac, wav, mp4, mov"
        )),
    }
}

// ---------------------------------------------------------------------------

impl WasmStreamingDemuxer {
    /// Construct a demuxer bypassing `JsValue` conversion — for native tests.
    #[cfg(test)]
    fn new_for_test(format_hint: &str) -> Self {
        let format = parse_format_hint(format_hint).expect("valid format hint in test");
        Self {
            buffer: Vec::new(),
            buffer_start_offset: 0,
            read_cursor: 0,
            format,
            probed: false,
            streams: Vec::new(),
            packet_count: 0,
            flushed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_format_hint_webm() {
        assert!(matches!(
            parse_format_hint("webm").expect("webm should be valid"),
            ContainerFormat::Matroska
        ));
        assert!(matches!(
            parse_format_hint("WebM").expect("WebM should be valid"),
            ContainerFormat::Matroska
        ));
        assert!(matches!(
            parse_format_hint("MKV").expect("MKV should be valid"),
            ContainerFormat::Matroska
        ));
    }

    #[test]
    fn test_parse_format_hint_ogg() {
        assert!(matches!(
            parse_format_hint("ogg").expect("ogg should be valid"),
            ContainerFormat::Ogg
        ));
    }

    #[test]
    fn test_parse_format_hint_unknown_fails() {
        assert!(parse_format_hint("avi").is_err());
        assert!(parse_format_hint("").is_err());
    }

    /// Below the 32-byte probe threshold there is genuinely not enough
    /// data yet -- this must be `Ok(None)` ("keep streaming"), not an
    /// error and not a fabricated packet.
    #[test]
    fn test_read_packet_before_probe_threshold_returns_none() {
        let mut sd = WasmStreamingDemuxer::new_for_test("wav");
        sd.append_data_inner(&vec![0u8; 10])
            .expect("append should succeed");
        let packet = sd
            .read_packet_inner()
            .expect("insufficient data should be Ok(None), not an error");
        assert!(packet.is_none());
        assert!(!sd.probed);
    }

    /// Once enough bytes have accumulated to attempt a real probe, the
    /// demuxer must honestly report that container parsing isn't wired in
    /// yet -- never fabricate a stream list. This is a regression test for
    /// the exact bug reported: the previous implementation always guessed
    /// a VP9 video + Opus audio pair for Matroska/WebM regardless of the
    /// actual track data, and wrapped raw byte windows in fake packets.
    #[test]
    fn test_read_packet_past_probe_threshold_is_honest_err() {
        let mut sd = WasmStreamingDemuxer::new_for_test("webm");
        sd.append_data_inner(&vec![0u8; 200])
            .expect("append should succeed");
        let result = sd.read_packet_inner();
        assert!(
            result.is_err(),
            "read_packet must not fabricate a packet once enough bytes exist to attempt a probe"
        );
        assert!(
            sd.streams().is_empty(),
            "no stream list should ever be fabricated"
        );
        assert!(
            !sd.probed,
            "probed must stay false -- no header was really parsed"
        );
        assert_eq!(sd.packets_emitted(), 0);
    }

    #[test]
    fn test_flush_does_not_fabricate_final_packet() {
        let mut sd = WasmStreamingDemuxer::new_for_test("flac");
        sd.append_data_inner(&vec![0u8; 100])
            .expect("append should succeed");
        let _ = sd.read_packet_inner(); // honestly errors; ignored here
        sd.append_data_inner(&vec![0u8; 10])
            .expect("append should succeed");
        sd.flush();
        // Even after flush(), leftover buffered bytes must never be
        // wrapped into a fabricated final packet.
        let result = sd.read_packet_inner();
        assert!(matches!(result, Err(_) | Ok(None)));
        assert_eq!(sd.packets_emitted(), 0);
    }

    #[test]
    fn test_streams_never_fabricated_for_any_format() {
        for hint in ["webm", "ogg", "flac", "wav", "mp4"] {
            let mut sd = WasmStreamingDemuxer::new_for_test(hint);
            sd.append_data_inner(&vec![0u8; 64])
                .expect("append should succeed");
            let _ = sd.read_packet_inner();
            assert!(
                sd.streams().is_empty(),
                "{hint}: stream list must not be fabricated"
            );
        }
    }

    /// Since nothing is really parsed, nothing should be silently marked
    /// "consumed" -- buffered bytes that were never actually demuxed must
    /// not be discarded by compaction either.
    #[test]
    fn test_no_bytes_silently_consumed_without_real_parsing() {
        let mut sd = WasmStreamingDemuxer::new_for_test("ogg");
        sd.append_data_inner(&vec![0u8; 512])
            .expect("append should succeed");
        let _ = sd.read_packet_inner();
        assert_eq!(sd.bytes_consumed(), 0);
        assert_eq!(sd.buffer_len(), 512);

        // A further append (which triggers compaction internally) must
        // not lose the unparsed bytes either.
        sd.append_data_inner(&vec![0u8; 16])
            .expect("append should succeed");
        assert_eq!(sd.buffer_len(), 528);
    }
}
