//! WASM smoke tests for the demux/probe/hash boundary.
//!
//! These exercise the real, non-fabricated surface this crate promises to
//! JavaScript callers:
//!
//! - Real magic-byte format detection (`probe_format`).
//! - `WasmDemuxer`'s honesty contract: `probe()` must return an honest
//!   `Err` for every container format rather than fabricating a plausible
//!   stream/packet, since full per-container parsing is not wired in yet
//!   (see `src/demuxer.rs` module docs).
//! - `WasmStreamingDemuxer`'s equivalent honesty contract for the
//!   chunk-oriented API (see `src/streaming_demuxer.rs` module docs).
//! - Real, deterministic content hashing (`probe_hash`) that reflects the
//!   actual input bytes rather than returning a constant.
//!
//! # Running
//!
//! This file is gated on `target_arch = "wasm32"` (see below), so it is a
//! no-op on the native host target -- plain `cargo test`/`cargo nextest`
//! neither compiles nor runs it. Native-target coverage for pure-logic
//! helpers lives in the `#[cfg(test)]` modules inside `src/*.rs` instead
//! (e.g. `src/demuxer.rs`, `src/streaming_demuxer.rs`, `src/probe.rs`).
//!
//! To actually compile and run *this* file, you need the
//! `wasm32-unknown-unknown` target and a JS runtime:
//!
//! ```bash
//! # Compile-only check (no JS runtime required):
//! cargo check -p oximedia-wasm --tests --target wasm32-unknown-unknown
//!
//! # Actually execute the tests, via Node.js (no browser needed):
//! wasm-pack test --node oximedia-wasm
//!
//! # ...or in a real browser engine:
//! wasm-pack test --headless --chrome oximedia-wasm
//! ```
//!
//! No `wasm_bindgen_test_configure!` call is made below, which keeps these
//! tests Node-compatible (calling `run_in_browser` would require `--chrome`
//! / `--firefox` / `--safari` instead of `--node`). None of the assertions
//! here need DOM/browser-only APIs.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::wasm_bindgen_test;

/// Minimal-but-real Ogg page header (`OggS` capture pattern + version byte
/// + header-type byte + zeroed granule-position/serial/sequence/CRC
/// fields). Real magic bytes recognized by the byte-level sniff, not a
/// fabricated full container -- there is no valid Ogg packet payload
/// following it, which is precisely why the demuxer must honestly fail to
/// extract streams/packets from it rather than pretend to succeed.
fn tiny_ogg_bytes() -> Vec<u8> {
    let mut data = b"OggS".to_vec();
    data.push(0x00); // stream_structure_version
    data.push(0x02); // header_type_flag: beginning-of-stream
    data.extend_from_slice(&[0u8; 20]); // granule pos + serial + seq + CRC (zeroed)
    data
}

/// Minimal valid FLAC stream marker + `STREAMINFO` block size field.
fn tiny_flac_bytes() -> Vec<u8> {
    b"fLaC\x00\x00\x00\x22".to_vec()
}

#[wasm_bindgen_test]
fn probe_format_detects_real_magic_bytes() {
    let data = tiny_ogg_bytes();
    let result = oximedia_wasm::probe_format(&data).expect("Ogg magic bytes should be detected");
    assert_eq!(result.format(), "Ogg");
    assert!(result.confidence() > 0.9);
}

#[wasm_bindgen_test]
fn probe_format_rejects_garbage() {
    let data = [0xFFu8; 16];
    assert!(
        oximedia_wasm::probe_format(&data).is_err(),
        "unrecognized bytes must not be reported as a detected format"
    );
}

#[wasm_bindgen_test]
fn demuxer_probe_is_honest_not_fabricated() {
    // The demuxer must never invent a stream/packet it did not actually
    // parse. Until real container parsing is wired in (see the
    // `TODO(0.2.x)` markers in `src/demuxer.rs`), `probe()` must fail
    // loudly instead of returning a fake single-stream guess.
    let data = tiny_flac_bytes();
    let mut demuxer = oximedia_wasm::WasmDemuxer::new(&data);
    let result = demuxer.probe();
    assert!(
        result.is_err(),
        "demuxer.probe() fabricated success instead of honestly erroring"
    );
    assert!(
        demuxer.streams().is_empty(),
        "no stream list should ever be fabricated"
    );
}

#[wasm_bindgen_test]
fn demuxer_read_packet_before_probe_errors() {
    let data = tiny_flac_bytes();
    let mut demuxer = oximedia_wasm::WasmDemuxer::new(&data);
    assert!(demuxer.read_packet().is_err());
}

#[wasm_bindgen_test]
fn streaming_demuxer_never_fabricates_a_stream_or_packet() {
    let mut sd = oximedia_wasm::WasmStreamingDemuxer::new("webm")
        .expect("'webm' is a recognized format hint");

    // Below the internal probe threshold: genuinely "not enough data yet",
    // must be `null` (`Ok(None)`), not an error and not a fabricated
    // packet.
    sd.append_data(&[0u8; 8])
        .expect("append_data should accept a small chunk");
    let early = sd
        .read_packet()
        .expect("insufficient data should be Ok(None), not an error");
    assert!(early.is_none());

    // Past the probe threshold: must honestly error instead of inventing
    // a VP9+Opus stream pair the way the old implementation did.
    sd.append_data(&[0u8; 200])
        .expect("append_data should accept a larger chunk");
    let result = sd.read_packet();
    assert!(
        result.is_err(),
        "streaming demuxer fabricated a packet instead of honestly erroring"
    );
    assert!(
        sd.streams().is_empty(),
        "no stream list should ever be fabricated"
    );
}

#[wasm_bindgen_test]
fn probe_hash_is_stable_and_reflects_real_bytes() {
    let data = tiny_ogg_bytes();
    let h1 = oximedia_wasm::probe_hash(&data);
    let h2 = oximedia_wasm::probe_hash(&data);
    assert_eq!(h1.crc32(), h2.crc32(), "hash must be deterministic");
    assert_eq!(h1.fnv1a64(), h2.fnv1a64(), "hash must be deterministic");
    assert_eq!(h1.byte_length(), data.len());

    let other = oximedia_wasm::probe_hash(b"completely different payload");
    assert_ne!(
        h1.crc32(),
        other.crc32(),
        "hash must reflect actual input bytes, not a constant"
    );
}

#[wasm_bindgen_test]
fn probe_hash_matches_known_crc32_check_value() {
    // Standard CRC-32/ISO-HDLC check value for the ASCII string
    // "123456789" -- a real, externally-verifiable reference value, not
    // just a self-consistency check.
    let h = oximedia_wasm::probe_hash(b"123456789");
    assert_eq!(h.crc32(), "cbf43926");
}
