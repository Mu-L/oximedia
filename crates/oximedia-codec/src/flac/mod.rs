//! FLAC (Free Lossless Audio Codec) encoder and decoder.
//!
//! FLAC achieves lossless compression by:
//!
//! 1. **LPC analysis** — fit a linear predictor to each audio subframe.
//! 2. **Residual coding** — encode the prediction residuals using Rice coding.
//! 3. **Frame assembly** — pack subframes into frames with a CRC-16.
//!
//! The decoder reverses this: Rice decode → LPC synthesis → reconstruct PCM.
//!
//! # Reference
//!
//! FLAC format specification: <https://xiph.org/flac/format.html>
//!
//! # Example
//!
//! ```rust
//! use oximedia_codec::flac::{FlacEncoder, FlacDecoder, FlacConfig};
//!
//! let config = FlacConfig { sample_rate: 44100, channels: 2, bits_per_sample: 16 };
//! let mut encoder = FlacEncoder::new(config);
//!
//! // Encode two channels of silence (interleaved i32)
//! let pcm = vec![0i32; 4096]; // 2048 stereo frames
//! let (header, frames) = encoder.encode(&pcm).expect("encode");
//! assert!(header.starts_with(b"fLaC"));
//! ```

pub mod decoder;
pub mod encoder;
pub mod lpc;
pub mod rice;

pub use decoder::{FlacDecoder, FlacStreamInfo, FlacVorbisComment};
pub use encoder::{FlacConfig, FlacEncoder, FlacFrame};
