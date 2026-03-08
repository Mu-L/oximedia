//! # `OxiMedia` WebAssembly Bindings
//!
//! This crate provides WebAssembly bindings for `OxiMedia`, enabling
//! patent-free multimedia processing in the browser.
//!
//! ## Features
//!
//! - **Format Probing**: Detect container formats from raw bytes
//! - **Container Demuxing**: Extract packets from `WebM`, Matroska, Ogg, FLAC, WAV
//! - **Zero-Copy**: Efficient buffer management using JavaScript `ArrayBuffer`
//! - **Browser-Native**: No file system, network-only I/O
//!
//! ## JavaScript API
//!
//! ```javascript
//! import * as oximedia from 'oximedia-wasm';
//!
//! // Probe format from bytes
//! const data = new Uint8Array([0x1A, 0x45, 0xDF, 0xA3, ...]);
//! const result = oximedia.probe_format(data);
//! console.log('Format:', result.format, 'Confidence:', result.confidence);
//!
//! // Demux a file
//! const demuxer = new oximedia.Demuxer(data);
//! await demuxer.probe();
//! const streams = demuxer.streams();
//! console.log('Streams:', streams);
//!
//! while (true) {
//!     const packet = await demuxer.read_packet();
//!     if (!packet) break;
//!     console.log('Packet:', packet.stream_index, packet.size);
//! }
//! ```
//!
//! ## Building
//!
//! ```bash
//! wasm-pack build --target web oximedia-wasm
//! ```

#![warn(missing_docs)]

use wasm_bindgen::prelude::*;

mod container;
mod demuxer;
mod io;
mod probe;
mod types;
mod utils;

// Re-export main types
pub use demuxer::WasmDemuxer;
pub use probe::{probe_format, WasmProbeResult};
pub use types::{WasmCodecParams, WasmMetadata, WasmPacket, WasmStreamInfo};

/// Initialize the WASM module.
///
/// This should be called once when the module is loaded.
/// It sets up panic hooks for better error messages in the browser console.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Get the version of `OxiMedia` WASM.
///
/// Returns the version string in semver format.
///
/// # Example
///
/// ```javascript
/// import * as oximedia from 'oximedia-wasm';
/// console.log('OxiMedia version:', oximedia.version());
/// ```
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
