// Copyright 2025 OxiMedia Contributors
// Licensed under the Apache License, Version 2.0

//! Spec-compliant ALAC frame encoder (uncompressed/escape elements).
//!
//! The workspace's ALAC encoder round-trips against its own decoder but its
//! compressed elements are rejected by reference decoders (FFmpeg), so the
//! transcode pipeline uses this minimal, verified frame layer instead:
//! every element is written in ALAC's *escape* (uncompressed) form, which
//! is bit-exact lossless and decodable by every conformant ALAC decoder.
//!
//! Element layout (Apple `alac.c` / FFmpeg `alacdec.c`):
//!
//! ```text
//! [3b element tag: SCE=0 mono / CPE=3 pair]
//! [4b element instance tag = 0]
//! [12b unused = 0]
//! [1b partial-frame flag]
//! [2b bytes shifted = 0]
//! [1b escape = 1 (uncompressed)]
//! [32b sample count, only when partial]
//! [samples: sample-major, channel-interleaved, bit-depth signed bits]
//! ```
//!
//! The frame ends with the `END` tag (7) and zero padding to a byte
//! boundary.
//!
// TODO(0.2.x): add the compressed element form (adaptive Rice + LPC) so
// ALAC output shrinks below PCM size; until then output is lossless but
// stored ≈1:1.

use crate::flac_bitstream::BitWriter;
use crate::{Result, TranscodeError};

/// ALAC syntax element tags (AAC-style enumeration: SCE=0, CPE=1, END=7 —
/// note CPE is 1 here, unlike some other MPEG bitstreams).
const ID_SCE: u64 = 0; // single channel element
const ID_CPE: u64 = 1; // channel pair element
const ID_END: u64 = 7;

/// A spec-compliant ALAC frame encoder for 16-bit interleaved PCM.
pub struct AlacStreamEncoder {
    sample_rate: u32,
    channels: u16,
    frame_length: u32,
}

impl AlacStreamEncoder {
    /// Bits per sample handled by this encoder.
    const BPS: u32 = 16;

    /// Creates an encoder with the given frames-per-packet.
    ///
    /// # Errors
    ///
    /// Returns [`TranscodeError::InvalidInput`] for out-of-range parameters.
    pub fn new(sample_rate: u32, channels: u16, frame_length: u32) -> Result<Self> {
        if !(1..=8).contains(&channels) {
            return Err(TranscodeError::InvalidInput(format!(
                "ALAC supports 1-8 channels, got {channels}"
            )));
        }
        if sample_rate == 0 || frame_length == 0 {
            return Err(TranscodeError::InvalidInput(
                "ALAC sample rate and frame length must be non-zero".into(),
            ));
        }
        Ok(Self {
            sample_rate,
            channels,
            frame_length,
        })
    }

    /// The 24-byte `ALACSpecificConfig` magic cookie for this stream.
    #[must_use]
    pub fn magic_cookie(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(24);
        out.extend_from_slice(&self.frame_length.to_be_bytes());
        out.push(0); // compatible version
        out.push(Self::BPS as u8); // bit depth
        out.push(40); // pb (rice history mult) — defaults per Apple
        out.push(10); // mb (rice initial history)
        out.push(14); // kb (rice param limit)
        out.push(self.channels as u8);
        out.extend_from_slice(&255u16.to_be_bytes()); // maxRun
        out.extend_from_slice(&0u32.to_be_bytes()); // maxFrameBytes (unknown)
        out.extend_from_slice(&0u32.to_be_bytes()); // avgBitRate (unknown)
        out.extend_from_slice(&self.sample_rate.to_be_bytes());
        out
    }

    /// Encode one block of interleaved i16 samples into a complete ALAC
    /// frame. Block sizes 1..=`frame_length` per channel.
    ///
    /// # Errors
    ///
    /// Returns [`TranscodeError::CodecError`] on empty/ragged/oversized
    /// blocks.
    pub fn encode_block(&self, interleaved: &[i16]) -> Result<Vec<u8>> {
        let ch = usize::from(self.channels);
        if interleaved.is_empty() || interleaved.len() % ch != 0 {
            return Err(TranscodeError::CodecError(format!(
                "ALAC block of {} samples is not a multiple of {ch} channels",
                interleaved.len()
            )));
        }
        let num_samples = interleaved.len() / ch;
        if num_samples > self.frame_length as usize {
            return Err(TranscodeError::CodecError(format!(
                "ALAC block of {num_samples} samples exceeds the {}-sample frame length",
                self.frame_length
            )));
        }
        let partial = num_samples != self.frame_length as usize;

        let mut bw = BitWriter::new();
        // Group channels into stereo pairs with a trailing mono element.
        let mut c = 0usize;
        while c < ch {
            let pair = c + 1 < ch;
            bw.write_bits(if pair { ID_CPE } else { ID_SCE }, 3);
            bw.write_bits(0, 4); // element instance tag
            bw.write_bits(0, 12); // unused, must be zero
            bw.write_bits(u64::from(partial), 1);
            bw.write_bits(0, 2); // bytes shifted
            bw.write_bits(1, 1); // escape: uncompressed
            if partial {
                bw.write_bits(num_samples as u64, 32);
            }
            let width = if pair { 2 } else { 1 };
            for s in 0..num_samples {
                for k in 0..width {
                    let sample = interleaved[s * ch + c + k];
                    bw.write_bits(sample as u64, Self::BPS);
                }
            }
            c += width;
        }
        bw.write_bits(ID_END, 3);
        Ok(bw.into_bytes())
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_cookie_layout() {
        let enc = AlacStreamEncoder::new(48_000, 2, 4_096).expect("encoder");
        let cookie = enc.magic_cookie();
        assert_eq!(cookie.len(), 24);
        assert_eq!(&cookie[0..4], &4096u32.to_be_bytes());
        assert_eq!(cookie[5], 16, "bit depth");
        assert_eq!(cookie[9], 2, "channels");
        assert_eq!(&cookie[20..24], &48_000u32.to_be_bytes());
    }

    #[test]
    fn test_full_block_size_is_deterministic() {
        // Full 4096-sample stereo block, escape coding:
        // per pair element: 3+4+12+1+2+1 = 23 bits header, then
        // 4096*2*16 bits samples; plus END(3) and padding.
        let enc = AlacStreamEncoder::new(44_100, 2, 4_096).expect("encoder");
        let block = vec![0i16; 4_096 * 2];
        let frame = enc.encode_block(&block).expect("encode");
        let bits: usize = 23 + 4_096 * 2 * 16 + 3;
        assert_eq!(frame.len(), bits.div_ceil(8));
    }

    #[test]
    fn test_partial_block_has_length_field() {
        let enc = AlacStreamEncoder::new(44_100, 1, 4_096).expect("encoder");
        let frame_full = enc.encode_block(&vec![0i16; 4_096]).expect("full");
        let frame_short = enc.encode_block(&vec![0i16; 100]).expect("short");
        // Short block: 23 + 32 + 100*16 + 3 bits.
        let bits: usize = 23 + 32 + 100 * 16 + 3;
        assert_eq!(frame_short.len(), bits.div_ceil(8));
        assert!(frame_full.len() > frame_short.len());
    }

    #[test]
    fn test_rejects_bad_blocks() {
        let enc = AlacStreamEncoder::new(44_100, 2, 4_096).expect("encoder");
        assert!(enc.encode_block(&[]).is_err());
        assert!(enc.encode_block(&[1i16, 2, 3]).is_err());
        assert!(enc.encode_block(&vec![0i16; (4_096 + 1) * 2]).is_err());
    }
}
