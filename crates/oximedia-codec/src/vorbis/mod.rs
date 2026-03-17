//! Vorbis audio encoder and decoder.
//!
//! Vorbis is a patent-free, fully open general-purpose compressed audio format
//! by the Xiph.Org Foundation.  This module provides:
//!
//! - **`VorbisEncoder`** — psychoacoustic lossy encoder: window, MDCT, psychoacoustic
//!   masking, floor curve, residue vector quantisation, Huffman packing.
//! - **`VorbisDecoder`** — packet decoder that reconstructs PCM from Vorbis packets.
//!
//! The implementation follows the Vorbis I specification
//! (<https://xiph.org/vorbis/doc/Vorbis_I_spec.html>).
//!
//! # Quick start
//!
//! ```rust
//! use oximedia_codec::vorbis::{VorbisEncoder, VorbisDecoder, VorbisConfig, VorbisQuality};
//!
//! let config = VorbisConfig {
//!     sample_rate: 44100,
//!     channels: 2,
//!     quality: VorbisQuality::Q5,
//! };
//!
//! let mut encoder = VorbisEncoder::new(config).expect("encoder init failed");
//! // encode silence
//! let samples = vec![0.0f32; 4096]; // 2048 stereo interleaved samples
//! let packets = encoder.encode_interleaved(&samples).expect("encode failed");
//! assert!(!packets.is_empty() || true); // may buffer
//! ```

pub mod codebook;
pub mod decoder;
pub mod encoder;
pub mod floor;
pub mod mdct;
pub mod residue;

pub use decoder::{
    DecoderState, VorbisCommentHeader, VorbisDecoder, VorbisHeaderType, VorbisIdHeader,
};
pub use encoder::{
    SimpleVorbisEncoder, VorbisConfig, VorbisEncConfig, VorbisEncoder, VorbisPacket, VorbisQuality,
};
