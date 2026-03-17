//! WebP codec support.
//!
//! Provides VP8 (lossy) and VP8L (lossless) WebP encoding, alpha channel
//! handling, RIFF container parsing/writing, and animated WebP support.

pub mod alpha;
pub mod animation;
pub mod encoder;
pub mod riff;
pub mod vp8l_decoder;
pub mod vp8l_encoder;
pub(crate) mod vp8l_pixel;

pub use alpha::{decode_alpha, encode_alpha, AlphaCompression, AlphaFilter, AlphaHeader};
pub use animation::{WebpAnimConfig, WebpAnimDecoder, WebpAnimEncoder, WebpAnimFrame};
pub use encoder::WebPLossyEncoder;
pub use riff::{ChunkType, RiffChunk, Vp8xFeatures, WebPContainer, WebPEncoding, WebPWriter};
pub use vp8l_decoder::{
    ColorTransformElement, DecodedImage, HuffmanCode, HuffmanTree, Transform, Vp8lBitReader,
    Vp8lDecoder, Vp8lHeader,
};
pub use vp8l_encoder::{Vp8lBitWriter, Vp8lEncoder};
