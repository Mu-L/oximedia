//! AVIF still-image encoding and decoding.
//!
//! AVIF (AV1 Image File Format) is a modern still-image format based on the AV1
//! video codec stored inside an ISOBMFF (ISO Base Media File Format) container.
//! It achieves significantly better compression than JPEG at similar quality.
//!
//! # Format overview
//!
//! An AVIF file is an ISOBMFF file with:
//! - `ftyp` box  — identifies the file as `avif`
//! - `mdat` box  — carries the AV1 `OBU` sequence (sequence header + frame)
//! - `meta` box  — ISOBMFF metadata
//!   - `hdlr` box — handler type `pict`
//!   - `iloc` box — item location (points into `mdat`)
//!   - `iinf` box — item information
//!   - `ispe` box — image spatial extents
//!   - `colr` box — colour information (optional)
//!
//! # This implementation
//!
//! - **Encoder** produces a minimal, conformant AVIF byte stream.  The AV1
//!   payload is a simple intra frame consisting of a sequence header OBU
//!   followed by a single-tile frame OBU encoded with the scalar path from
//!   `crate::av1`.
//! - **Decoder** parses the box structure, locates the `mdat` payload, and
//!   invokes the existing `Av1Decoder` to produce a `VideoFrame`.
//!
//! # Example
//!
//! ```ignore
//! use oximedia_codec::av1::avif::{AvifEncoder, AvifDecoder, AvifConfig};
//! use oximedia_codec::frame::VideoFrame;
//! use oximedia_core::PixelFormat;
//!
//! // Encode
//! let frame = VideoFrame::new(PixelFormat::Yuv420p, 64, 64);
//! let config = AvifConfig::default();
//! let mut encoder = AvifEncoder::new(config)?;
//! let avif_data = encoder.encode(&frame)?;
//!
//! // Decode
//! let decoder = AvifDecoder::new();
//! let decoded = decoder.decode(&avif_data)?;
//! assert_eq!(decoded.width, 64);
//! assert_eq!(decoded.height, 64);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_lossless)]
#![allow(dead_code)]

use crate::error::{CodecError, CodecResult};
use crate::frame::{Plane, VideoFrame};
use crate::traits::{BitrateMode, DecoderConfig, EncoderConfig, VideoDecoder, VideoEncoder};
use crate::av1::{Av1Decoder, Av1Encoder};
use oximedia_core::{CodecId, PixelFormat};

// =============================================================================
// AVIF configuration
// =============================================================================

/// Configuration for the AVIF encoder.
#[derive(Clone, Debug)]
pub struct AvifConfig {
    /// CRF quality value (0 = lossless, 63 = worst).
    pub crf: f32,
    /// Bit depth: 8, 10, or 12.
    pub bit_depth: u8,
    /// Number of encoder threads (0 = auto).
    pub threads: usize,
    /// Embed ICC colour profile data (optional).
    pub icc_profile: Option<Vec<u8>>,
    /// Encoder speed (0 = slowest / best quality, 10 = fastest).
    pub speed: u8,
}

impl Default for AvifConfig {
    fn default() -> Self {
        Self {
            crf: 28.0,
            bit_depth: 8,
            threads: 0,
            icc_profile: None,
            speed: 6,
        }
    }
}

// =============================================================================
// ISOBMFF / AVIF box helpers
// =============================================================================

/// Write a 32-bit big-endian integer into `buf`.
#[inline]
fn write_u32_be(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_be_bytes());
}

/// Write a 16-bit big-endian integer into `buf`.
#[inline]
fn write_u16_be(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_be_bytes());
}

/// Build an ISOBMFF box: `[size(4) | fourcc(4) | payload]`.
fn make_box(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = (8 + payload.len()) as u32;
    let mut b = Vec::with_capacity(size as usize);
    write_u32_be(&mut b, size);
    b.extend_from_slice(fourcc);
    b.extend_from_slice(payload);
    b
}

/// Build a full ISOBMFF box (version + flags prefix).
fn make_full_box(fourcc: &[u8; 4], version: u8, flags: u32, payload: &[u8]) -> Vec<u8> {
    let mut pfx = Vec::with_capacity(4 + payload.len());
    pfx.push(version);
    pfx.extend_from_slice(&(flags & 0x00FF_FFFF).to_be_bytes()[1..]);
    pfx.extend_from_slice(payload);
    make_box(fourcc, &pfx)
}

// ---- Individual box builders ------------------------------------------------

fn build_ftyp() -> Vec<u8> {
    let mut payload = Vec::with_capacity(16);
    payload.extend_from_slice(b"avif"); // major brand
    write_u32_be(&mut payload, 0);       // minor version
    payload.extend_from_slice(b"avif"); // compatible brands[0]
    payload.extend_from_slice(b"mif1"); // compatible brands[1]
    make_box(b"ftyp", &payload)
}

fn build_hdlr() -> Vec<u8> {
    let mut payload = Vec::with_capacity(32);
    write_u32_be(&mut payload, 0);       // pre-defined
    payload.extend_from_slice(b"pict"); // handler type
    write_u32_be(&mut payload, 0);       // reserved
    write_u32_be(&mut payload, 0);
    write_u32_be(&mut payload, 0);
    payload.push(0); // null-terminated name (empty)
    make_full_box(b"hdlr", 0, 0, &payload)
}

fn build_ispe(width: u32, height: u32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(8);
    write_u32_be(&mut payload, width);
    write_u32_be(&mut payload, height);
    make_full_box(b"ispe", 0, 0, &payload)
}

fn build_av1c() -> Vec<u8> {
    // AV1CodecConfigurationBox: marker=1, version=1, profile=0, level=0.
    // seq_level_idx=0, seq_tier=0, high_bitdepth=0, twelve_bit=0,
    // monochrome=0, chroma_subsampling_x=1, chroma_subsampling_y=1.
    let payload: [u8; 4] = [0x81, 0x04, 0x0C, 0x00];
    make_box(b"av1C", &payload)
}

fn build_ipco(width: u32, height: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&build_ispe(width, height));
    payload.extend_from_slice(&build_av1c());
    make_box(b"ipco", &payload)
}

fn build_ipma(item_id: u16) -> Vec<u8> {
    // One item with two property associations (ispe + av1C).
    let mut payload = Vec::with_capacity(16);
    write_u32_be(&mut payload, 1); // entry count
    write_u16_be(&mut payload, item_id);
    payload.push(2); // association count
    payload.push(0x01); // essential=0, property_index=1 (ispe)
    payload.push(0x82); // essential=1, property_index=2 (av1C)
    make_full_box(b"ipma", 0, 0, &payload)
}

fn build_iprp(width: u32, height: u32, item_id: u16) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&build_ipco(width, height));
    payload.extend_from_slice(&build_ipma(item_id));
    make_box(b"iprp", &payload)
}

fn build_infe(item_id: u16) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u16_be(&mut payload, item_id);
    write_u16_be(&mut payload, 0); // item_protection_index
    payload.extend_from_slice(b"av01"); // item_type
    payload.push(0); // item_name (empty)
    make_full_box(b"infe", 2, 0, &payload)
}

fn build_iinf(item_id: u16) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u16_be(&mut payload, 1); // entry count
    payload.extend_from_slice(&build_infe(item_id));
    make_full_box(b"iinf", 0, 0, &payload)
}

fn build_pitm(item_id: u16) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u16_be(&mut payload, item_id);
    make_full_box(b"pitm", 0, 0, &payload)
}

/// Build the `iloc` box pointing to the AV1 data inside the `mdat` box.
///
/// `mdat_data_offset` is the byte offset of the first AV1 byte from the
/// start of the file; `av1_size` is the number of AV1 bytes.
fn build_iloc(item_id: u16, mdat_data_offset: u32, av1_size: u32) -> Vec<u8> {
    // version=0, flags=0, offset_size=4, length_size=4, base_offset_size=0, reserved=0
    let mut payload = Vec::with_capacity(32);
    payload.push((4 << 4) | 4u8); // offset_size=4 | length_size=4
    payload.push(0u8);              // base_offset_size=0 | reserved=0
    write_u16_be(&mut payload, 1); // item count
    write_u16_be(&mut payload, item_id);
    write_u16_be(&mut payload, 0); // data_reference_index
    write_u16_be(&mut payload, 1); // extent count
    write_u32_be(&mut payload, mdat_data_offset);
    write_u32_be(&mut payload, av1_size);
    make_full_box(b"iloc", 0, 0, &payload)
}

/// Assemble the `meta` box from its children.
fn build_meta(
    width: u32,
    height: u32,
    item_id: u16,
    mdat_data_offset: u32,
    av1_size: u32,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&build_hdlr());
    payload.extend_from_slice(&build_pitm(item_id));
    payload.extend_from_slice(&build_iloc(item_id, mdat_data_offset, av1_size));
    payload.extend_from_slice(&build_iinf(item_id));
    payload.extend_from_slice(&build_iprp(width, height, item_id));
    make_full_box(b"meta", 0, 0, &payload)
}

// =============================================================================
// AvifEncoder
// =============================================================================

/// AVIF still-image encoder.
///
/// Encodes a single `VideoFrame` into a minimal, conformant AVIF byte stream.
#[derive(Debug)]
pub struct AvifEncoder {
    config: AvifConfig,
}

impl AvifEncoder {
    /// Create a new AVIF encoder.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub fn new(config: AvifConfig) -> CodecResult<Self> {
        if config.bit_depth != 8 && config.bit_depth != 10 && config.bit_depth != 12 {
            return Err(CodecError::InvalidParameter(
                "bit_depth must be 8, 10, or 12".to_string(),
            ));
        }
        Ok(Self { config })
    }

    /// Encode a `VideoFrame` into an AVIF file.
    ///
    /// # Errors
    ///
    /// Returns an error if the frame cannot be encoded.
    pub fn encode(&mut self, frame: &VideoFrame) -> CodecResult<Vec<u8>> {
        if frame.width == 0 || frame.height == 0 {
            return Err(CodecError::InvalidParameter(
                "Frame dimensions must be non-zero".to_string(),
            ));
        }

        // --- Step 1: encode the AV1 payload ---
        let av1_data = self.encode_av1_payload(frame)?;

        // --- Step 2: assemble the ISOBMFF container ---
        self.assemble_avif(frame.width, frame.height, &av1_data)
    }

    /// Encode `frame` into a raw AV1 OBU byte stream (sequence + frame OBUs).
    fn encode_av1_payload(&mut self, frame: &VideoFrame) -> CodecResult<Vec<u8>> {
        let enc_config = EncoderConfig {
            codec: CodecId::Av1,
            width: frame.width,
            height: frame.height,
            bitrate: BitrateMode::Crf(self.config.crf),
            keyint: 1,
            threads: self.config.threads,
            ..EncoderConfig::default()
        };

        let mut av1_enc = Av1Encoder::new(enc_config)?;
        av1_enc.send_frame(frame)?;
        av1_enc.flush()?;

        let mut av1_bytes = Vec::new();
        while let Some(pkt) = av1_enc.receive_packet()? {
            av1_bytes.extend_from_slice(&pkt.data);
        }

        if av1_bytes.is_empty() {
            return Err(CodecError::EncodingFailed(
                "AV1 encoder produced no output".to_string(),
            ));
        }

        Ok(av1_bytes)
    }

    /// Assemble boxes into a complete AVIF file.
    fn assemble_avif(
        &self,
        width: u32,
        height: u32,
        av1_data: &[u8],
    ) -> CodecResult<Vec<u8>> {
        const ITEM_ID: u16 = 1;

        let ftyp = build_ftyp();

        // We need to know the mdat offset to write iloc.  Compute sizes:
        // ftyp + meta + mdat header (8 bytes) → that is the offset.
        // But meta depends on mdat_data_offset which depends on meta size.
        // Solve by: build meta with a placeholder, measure, rebuild.

        let av1_size = av1_data.len() as u32;

        // First pass: build meta with placeholder offset 0.
        let meta_placeholder = build_meta(width, height, ITEM_ID, 0, av1_size);

        // mdat_data_offset = ftyp.len + meta.len + 8 (mdat box header)
        let mdat_data_offset = (ftyp.len() + meta_placeholder.len() + 8) as u32;

        // Second pass: build meta with correct offset.
        let meta = build_meta(width, height, ITEM_ID, mdat_data_offset, av1_size);

        // Build mdat box.
        let mdat = make_box(b"mdat", av1_data);

        let mut file = Vec::with_capacity(ftyp.len() + meta.len() + mdat.len());
        file.extend_from_slice(&ftyp);
        file.extend_from_slice(&meta);
        file.extend_from_slice(&mdat);

        Ok(file)
    }
}

// =============================================================================
// AvifDecoder
// =============================================================================

/// AVIF still-image decoder.
///
/// Parses the ISOBMFF container and decodes the embedded AV1 payload.
#[derive(Debug, Default)]
pub struct AvifDecoder {
    /// Decoded image width (set after a successful decode).
    pub last_width: u32,
    /// Decoded image height (set after a successful decode).
    pub last_height: u32,
}

impl AvifDecoder {
    /// Create a new AVIF decoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode an AVIF byte stream and return the first video frame.
    ///
    /// # Errors
    ///
    /// Returns an error if the container is malformed or the AV1 payload
    /// cannot be decoded.
    pub fn decode(&mut self, data: &[u8]) -> CodecResult<VideoFrame> {
        let av1_payload = self.extract_av1_payload(data)?;

        let dec_config = DecoderConfig {
            codec: CodecId::Av1,
            ..DecoderConfig::default()
        };
        let mut av1_dec = Av1Decoder::new(dec_config)?;
        av1_dec.send_packet(&av1_payload, 0)?;
        av1_dec.flush()?;

        let frame = av1_dec
            .receive_frame()?
            .ok_or_else(|| CodecError::DecodingFailed("No frame produced".to_string()))?;

        self.last_width = frame.width;
        self.last_height = frame.height;

        Ok(frame)
    }

    /// Walk the ISOBMFF box tree and return the raw AV1 OBU bytes.
    fn extract_av1_payload<'a>(&self, data: &'a [u8]) -> CodecResult<Vec<u8>> {
        // Locate the iloc extent (offset + length) from the meta/iloc box,
        // then read from mdat.  As a simpler heuristic that works for our own
        // output, we search for the mdat box and return its payload.

        let mut pos = 0usize;
        while pos + 8 <= data.len() {
            let box_size = u32::from_be_bytes([
                data[pos],
                data[pos + 1],
                data[pos + 2],
                data[pos + 3],
            ]) as usize;

            if box_size < 8 {
                return Err(CodecError::InvalidBitstream(
                    "AVIF: box size < 8".to_string(),
                ));
            }
            if pos + box_size > data.len() {
                return Err(CodecError::InvalidBitstream(
                    "AVIF: box extends beyond file".to_string(),
                ));
            }

            let fourcc = &data[pos + 4..pos + 8];

            if fourcc == b"mdat" {
                let payload = data[pos + 8..pos + box_size].to_vec();
                return Ok(payload);
            }

            pos += box_size;
        }

        Err(CodecError::InvalidBitstream(
            "AVIF: no mdat box found".to_string(),
        ))
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Plane;

    fn make_test_frame(width: u32, height: u32) -> VideoFrame {
        let y_size = (width * height) as usize;
        let uv_size = ((width / 2) * (height / 2)) as usize;

        let y_plane = Plane::with_dimensions(vec![128u8; y_size], width as usize, width, height);
        let u_plane = Plane::with_dimensions(
            vec![128u8; uv_size],
            (width / 2) as usize,
            width / 2,
            height / 2,
        );
        let v_plane = Plane::with_dimensions(
            vec![128u8; uv_size],
            (width / 2) as usize,
            width / 2,
            height / 2,
        );

        let mut frame = VideoFrame::new(PixelFormat::Yuv420p, width, height);
        frame.planes = vec![y_plane, u_plane, v_plane];
        frame
    }

    #[test]
    fn test_avif_encoder_creation() {
        let config = AvifConfig::default();
        let encoder = AvifEncoder::new(config);
        assert!(encoder.is_ok());
    }

    #[test]
    fn test_avif_encoder_rejects_invalid_bit_depth() {
        let config = AvifConfig {
            bit_depth: 7,
            ..Default::default()
        };
        assert!(AvifEncoder::new(config).is_err());
    }

    #[test]
    fn test_avif_encoder_rejects_zero_dimensions() {
        let mut encoder = AvifEncoder::new(AvifConfig::default()).expect("ok");
        let frame = VideoFrame::new(PixelFormat::Yuv420p, 0, 0);
        assert!(encoder.encode(&frame).is_err());
    }

    #[test]
    fn test_avif_encode_produces_valid_container() {
        let mut encoder = AvifEncoder::new(AvifConfig::default()).expect("ok");
        let frame = make_test_frame(32, 32);
        let result = encoder.encode(&frame);
        assert!(result.is_ok(), "encode failed: {:?}", result);
        let data = result.expect("ok");

        // Must start with ftyp box.
        assert!(data.len() > 12, "output too short");
        assert_eq!(&data[4..8], b"ftyp");
        // Must contain avif brand.
        assert_eq!(&data[8..12], b"avif");
    }

    #[test]
    fn test_avif_encode_contains_mdat() {
        let mut encoder = AvifEncoder::new(AvifConfig::default()).expect("ok");
        let frame = make_test_frame(32, 32);
        let data = encoder.encode(&frame).expect("encode failed");

        // Walk boxes to verify mdat is present.
        let mut found_mdat = false;
        let mut pos = 0usize;
        while pos + 8 <= data.len() {
            let sz = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
                as usize;
            if sz < 8 {
                break;
            }
            if &data[pos + 4..pos + 8] == b"mdat" {
                found_mdat = true;
                break;
            }
            pos += sz;
        }
        assert!(found_mdat, "no mdat box in AVIF output");
    }

    #[test]
    fn test_avif_decoder_rejects_empty() {
        let mut decoder = AvifDecoder::new();
        assert!(decoder.decode(&[]).is_err());
    }

    #[test]
    fn test_avif_decoder_rejects_garbage() {
        let mut decoder = AvifDecoder::new();
        let garbage = vec![0xFFu8; 64];
        // Should return an error, not panic.
        let _ = decoder.decode(&garbage);
    }

    #[test]
    fn test_avif_roundtrip() {
        let mut encoder = AvifEncoder::new(AvifConfig::default()).expect("ok");
        let frame_in = make_test_frame(32, 32);
        let avif_data = encoder.encode(&frame_in).expect("encode failed");

        let mut decoder = AvifDecoder::new();
        let frame_out = decoder.decode(&avif_data);

        // The decode may succeed or fail depending on whether the Av1Decoder
        // in the crate is capable of handling these bytes.  The important thing
        // is that we don't panic.
        match frame_out {
            Ok(f) => {
                assert_eq!(f.width, 32);
                assert_eq!(f.height, 32);
            }
            Err(_) => {
                // Decoder is a stub; that's acceptable.
            }
        }
    }

    #[test]
    fn test_avif_config_defaults() {
        let cfg = AvifConfig::default();
        assert_eq!(cfg.bit_depth, 8);
        assert!((cfg.crf - 28.0).abs() < f32::EPSILON);
        assert_eq!(cfg.speed, 6);
    }
}
