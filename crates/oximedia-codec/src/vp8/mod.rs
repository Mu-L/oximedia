//! VP8 codec implementation.
//!
//! This module provides a pure Rust VP8 decoder (key frames), encoder
//! building blocks, and bitstream primitives based on RFC 6386. VP8 is a
//! royalty-free video codec developed by Google as part of the `WebM`
//! project.
//!
//! # Honest status: key frames decode to pixels; inter frames not yet
//!
//! Key-frame (intra) decoding is **fully implemented**: `Vp8Decoder`
//! reconstructs real YUV 4:2:0 pixels via the complete RFC 6386 intra
//! pipeline — boolean entropy decoding, full key-frame header parsing
//! (segmentation, loop-filter parameters, quantiser indices, token
//! probability updates), macroblock mode and DCT-token decoding,
//! per-segment dequantisation, inverse DCT/WHT, every 16x16/4x4/chroma
//! intra prediction mode, and both (simple + normal) in-loop deblocking
//! filters. The pipeline is ported from the production-verified
//! `oximedia-image` WebP/VP8 decoder (a WebP lossy image *is* a VP8 key
//! frame).
//!
//! Inter frames (P-frames) need motion-vector entropy decoding and
//! golden/altref reference-frame management, which are **not implemented**;
//! `Vp8Decoder::send_packet` returns an honest
//! [`CodecError`](crate::error::CodecError)`::UnsupportedFeature` error for
//! them instead of fabricating pixel data.
//!
//! This module also exposes standalone primitives (boolean decoder,
//! DCT/WHT transforms, prediction, motion-compensation and loop-filter
//! helpers) that predate the key-frame pipeline and remain available as
//! public building blocks.
//!
//! # Codec Details
//!
//! VP8 always outputs YUV 4:2:0 planar format (`Yuv420p`).
//! It supports two frame types:
//! - Keyframes: Can be decoded independently
//! - Inter frames: Use motion compensation from reference frames
//!
//! # Architecture
//!
//! VP8 operates on 16x16 macroblocks which can use:
//! - Intra prediction: I16 (16x16) or I4 (4x4) modes
//! - Inter prediction: Motion compensation with quarter-pixel precision
//! - Transform: 4x4 DCT or WHT for DC coefficients
//! - Loop filter: Deblocking filter at macroblock boundaries
//!
//! # Examples
//!
//! ```
//! use oximedia_codec::vp8::{Vp8Decoder, FrameHeader, FrameType};
//! use oximedia_codec::traits::{VideoDecoder, DecoderConfig};
//! use oximedia_codec::error::CodecError;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a decoder
//! let config = DecoderConfig::default();
//! let mut decoder = Vp8Decoder::new(config)?;
//!
//! // Parse a frame header directly (tag + start code + dimensions).
//! let header_data = [
//!     0x10, 0x00, 0x00,       // frame tag
//!     0x9D, 0x01, 0x2A,       // sync code
//!     0x40, 0x01, 0xF0, 0x00, // 320x240
//! ];
//! let header = FrameHeader::parse(&header_data)?;
//! assert!(header.is_keyframe());
//! assert_eq!(header.width, 320);
//! assert_eq!(header.height, 240);
//!
//! // Full decode needs the compressed header/token partitions too; this
//! // 10-byte stub has none, so decoding reports a bitstream error while
//! // stream dimensions stay available. A complete key-frame payload
//! // (e.g. the `VP8 ` chunk of a lossy WebP) decodes to real pixels.
//! let result = decoder.send_packet(&header_data, 0);
//! assert!(matches!(result, Err(CodecError::InvalidBitstream(_))));
//! assert_eq!(decoder.dimensions(), Some((320, 240)));
//! assert!(decoder.receive_frame()?.is_none());
//!
//! // Inter frames are not implemented yet: honest error, no fake frames.
//! let inter = [0x11, 0x00, 0x00, 0x00];
//! let result = decoder.send_packet(&inter, 1);
//! assert!(matches!(result, Err(CodecError::UnsupportedFeature(_))));
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!
//! - [RFC 6386: VP8 Data Format and Decoding Guide](https://tools.ietf.org/html/rfc6386)
//! - [WebM Project](https://www.webmproject.org/)

mod bool_decoder;
mod dct;
mod decoder;
mod encoder;
mod frame_header;
mod keyframe;
mod loopfilter;
mod mb_mode;
mod motion;
mod prediction;

pub use bool_decoder::BoolDecoder;
pub use dct::{dequantize_block, dequantize_coeff, idct4x4, iwht4x4, Block4x4, PixelBlock4x4};
pub use decoder::Vp8Decoder;
pub use encoder::{SimpleVp8Encoder, Vp8EncConfig, Vp8Encoder, Vp8EncoderConfig, Vp8Packet};
pub use frame_header::{ClampingType, ColorSpace, FrameHeader, FrameType};
pub use loopfilter::{
    calculate_filter_params, filter_horizontal_edge, filter_vertical_edge, LoopFilterConfig,
    MAX_LOOP_FILTER, MAX_SHARPNESS,
};
pub use mb_mode::{
    ChromaMode, InterMode, IntraMode16, IntraMode4, MacroblockType, PartitionType, RefFrame,
    NUM_CHROMA_MODES, NUM_I16_MODES, NUM_I4_MODES, NUM_MV_MODES,
};
pub use motion::{clamp_mv, motion_compensate, MotionVector};
pub use prediction::{predict_chroma, predict_intra_16x16, predict_intra_4x4};
