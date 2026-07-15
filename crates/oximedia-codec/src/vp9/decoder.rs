//! VP9 decoder implementation.
//!
//! Keyframes (and the show-existing-frame mechanism) decode to real pixels
//! via the bit-exact intra pipeline in [`crate::vp9::kf`]; inter frames
//! return an honest [`CodecError::UnsupportedFeature`] until motion
//! compensation lands — never a fabricated frame.

#![forbid(unsafe_code)]
#![allow(clippy::cast_possible_truncation)]

use crate::error::{CodecError, CodecResult};
use crate::frame::{Plane, VideoFrame};
use crate::traits::{DecoderConfig, VideoDecoder};
use crate::vp9::kf;
use crate::vp9::superframe::Superframe;
use crate::vp9::uncompressed::UncompressedHeader;
use oximedia_core::{CodecId, PixelFormat, Rational, Timestamp};

/// VP9 decoder.
///
/// Scope: keyframe / intra-only reconstruction is implemented for 8-bit
/// 4:2:0 (profile 0) and verified bit-exact against libvpx; inter frames
/// and other profiles fail honestly.
#[derive(Debug)]
pub struct Vp9Decoder {
    #[allow(dead_code)]
    config: DecoderConfig,
    width: u32,
    height: u32,
    output_format: PixelFormat,
    output_queue: Vec<VideoFrame>,
    ref_frames: [Option<VideoFrame>; 8],
    flushing: bool,
    /// Frame counter.
    frame_count: u64,
}

impl Vp9Decoder {
    /// Creates a new VP9 decoder.
    ///
    /// # Errors
    ///
    /// Returns error if configuration is invalid.
    pub fn new(config: DecoderConfig) -> CodecResult<Self> {
        Ok(Self {
            config,
            width: 0,
            height: 0,
            output_format: PixelFormat::Yuv420p,
            output_queue: Vec::new(),
            ref_frames: Default::default(),
            flushing: false,
            frame_count: 0,
        })
    }

    /// Decodes a single (non-superframe) VP9 frame payload.
    fn decode_frame(&mut self, data: &[u8], pts: i64) -> CodecResult<()> {
        let header = UncompressedHeader::parse(data)?;

        if header.show_existing_frame {
            // Re-display a previously decoded reference frame.
            let idx = header.frame_to_show as usize;
            if let Some(ref frame) = self.ref_frames[idx] {
                let mut output = frame.clone();
                output.timestamp = Timestamp::new(pts, Rational::new(1, 1000));
                self.output_queue.push(output);
                return Ok(());
            }
            return Err(CodecError::InvalidBitstream(format!(
                "VP9 show_existing_frame: reference slot {idx} holds no decoded frame"
            )));
        }

        // Real header-parse results: update stream properties.
        if header.width > 0 && header.height > 0 {
            self.width = header.width;
            self.height = header.height;
        }
        self.output_format = match (header.bit_depth, header.subsampling_x, header.subsampling_y) {
            (8, true, true) => PixelFormat::Yuv420p,
            (8, true, false) => PixelFormat::Yuv422p,
            (8, false, false) => PixelFormat::Yuv444p,
            (10, true, true) => PixelFormat::Yuv420p10le,
            (12, true, true) => PixelFormat::Yuv420p12le,
            _ => PixelFormat::Yuv420p,
        };

        if !header.is_keyframe() {
            if header.is_intra_only() {
                // TODO(0.2.x): decode intra-only frames. Their pixel path is
                // the same as keyframes (already implemented in `kf`), but
                // they read `frame_context_idx` contexts that preceding
                // inter frames may have adapted — requires the inter decode
                // (or at least full context adaptation tracking) first.
                return Err(CodecError::UnsupportedFeature(
                    "VP9 intra-only frame decode requires inter-frame context \
                     tracking (not yet implemented); keyframes decode"
                        .to_string(),
                ));
            }
            // TODO(0.2.x): VP9 inter-frame decode — motion-vector /
            // ref-frame syntax (vp9_decodemv.c inter path), eighth-pel
            // motion compensation with the four interp filter sets, compound
            // prediction, and backward probability adaptation
            // (vp9_adapt_coef_probs et al) for !frame_parallel streams.
            return Err(CodecError::UnsupportedFeature(
                "VP9 inter-frame decode not yet implemented; keyframes decode".to_string(),
            ));
        }

        let decoded = kf::decode_keyframe(&header, data)?;
        let mut frame = Self::planes_to_video_frame(&header, &decoded);
        frame.timestamp = Timestamp::new(pts, Rational::new(1, 1000));
        self.frame_count += 1;

        // Keyframes refresh all reference slots (refresh_frame_flags 0xFF).
        for (i, slot) in self.ref_frames.iter_mut().enumerate() {
            if header.refresh_frame_flags & (1 << i) != 0 {
                *slot = Some(frame.clone());
            }
        }

        if header.show_frame {
            self.output_queue.push(frame);
        }
        Ok(())
    }

    /// Crops the MI-aligned reconstruction planes to display size and
    /// assembles a [`VideoFrame`].
    fn planes_to_video_frame(
        header: &UncompressedHeader,
        decoded: &kf::DecodedIntraFrame,
    ) -> VideoFrame {
        let w = decoded.width;
        let h = decoded.height;
        let cw = w.div_ceil(2);
        let ch = h.div_ceil(2);

        let mut frame = VideoFrame::new(PixelFormat::Yuv420p, header.width, header.height);
        let dims = [(w, h), (cw, ch), (cw, ch)];
        for (plane, &(pw, ph)) in decoded.planes.iter().zip(dims.iter()) {
            let mut data = Vec::with_capacity(pw * ph);
            for row in 0..ph {
                let start = row * plane.stride;
                data.extend_from_slice(&plane.data[start..start + pw]);
            }
            frame
                .planes
                .push(Plane::with_dimensions(data, pw, pw as u32, ph as u32));
        }
        frame
    }

    /// Returns the number of pending output frames.
    #[must_use]
    pub fn pending_frames(&self) -> usize {
        self.output_queue.len()
    }

    /// Returns true if the decoder has been flushed.
    #[must_use]
    pub fn is_flushing(&self) -> bool {
        self.flushing
    }
}

impl VideoDecoder for Vp9Decoder {
    fn codec(&self) -> CodecId {
        CodecId::Vp9
    }

    #[allow(clippy::cast_possible_wrap)]
    fn send_packet(&mut self, data: &[u8], pts: i64) -> CodecResult<()> {
        if self.flushing {
            return Err(CodecError::InvalidParameter(
                "Cannot send packet while flushing".into(),
            ));
        }

        let superframe = Superframe::parse(data)?;

        for (i, frame_data) in superframe.frames.iter().enumerate() {
            let frame_pts = pts + i as i64;
            self.decode_frame(frame_data, frame_pts)?;
        }

        Ok(())
    }

    fn receive_frame(&mut self) -> CodecResult<Option<VideoFrame>> {
        if self.output_queue.is_empty() {
            if self.flushing {
                return Err(CodecError::Eof);
            }
            return Ok(None);
        }

        Ok(Some(self.output_queue.remove(0)))
    }

    fn flush(&mut self) -> CodecResult<()> {
        self.flushing = true;
        Ok(())
    }

    fn reset(&mut self) {
        self.output_queue.clear();
        self.ref_frames = Default::default();
        self.flushing = false;
    }

    fn output_format(&self) -> Option<PixelFormat> {
        if self.width > 0 && self.height > 0 {
            Some(self.output_format)
        } else {
            None
        }
    }

    fn dimensions(&self) -> Option<(u32, u32)> {
        if self.width > 0 && self.height > 0 {
            Some((self.width, self.height))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vp9_decoder_new() {
        let config = DecoderConfig::default();
        let decoder = Vp9Decoder::new(config).expect("should succeed");
        assert_eq!(decoder.codec(), CodecId::Vp9);
        assert_eq!(decoder.pending_frames(), 0);
        assert!(!decoder.is_flushing());
    }

    #[test]
    fn test_decoder_initial_state() {
        let config = DecoderConfig::default();
        let decoder = Vp9Decoder::new(config).expect("should succeed");
        assert!(decoder.output_format().is_none());
        assert!(decoder.dimensions().is_none());
    }

    #[test]
    fn test_flush() {
        let config = DecoderConfig::default();
        let mut decoder = Vp9Decoder::new(config).expect("should succeed");
        assert!(!decoder.is_flushing());
        decoder.flush().expect("should succeed");
        assert!(decoder.is_flushing());
    }

    #[test]
    fn test_reset() {
        let config = DecoderConfig::default();
        let mut decoder = Vp9Decoder::new(config).expect("should succeed");
        decoder.flush().expect("should succeed");
        assert!(decoder.is_flushing());
        decoder.reset();
        assert!(!decoder.is_flushing());
    }

    #[test]
    fn test_receive_no_frame() {
        let config = DecoderConfig::default();
        let mut decoder = Vp9Decoder::new(config).expect("should succeed");
        let frame = decoder.receive_frame().expect("should succeed");
        assert!(frame.is_none());
    }

    #[test]
    fn test_send_while_flushing() {
        let config = DecoderConfig::default();
        let mut decoder = Vp9Decoder::new(config).expect("should succeed");
        decoder.flush().expect("should succeed");
        let result = decoder.send_packet(&[0x80], 0);
        assert!(result.is_err());
    }

    /// Real libvpx-vp9 keyframe (76x42, crf 24) — must decode to real
    /// pixels through the public `VideoDecoder` API and match the
    /// libvpx/ffmpeg reference decode bit-exactly.
    #[test]
    fn test_decode_real_keyframe_bit_exact() {
        const IVF_FRAME: &[u8] = include_bytes!("kf/testdata/kf76x42.frame0.bin");
        const REF_YUV: &[u8] = include_bytes!("kf/testdata/ref76x42.yuv");

        let config = DecoderConfig::default();
        let mut decoder = Vp9Decoder::new(config).expect("should succeed");
        decoder.send_packet(IVF_FRAME, 0).expect("keyframe decodes");

        assert_eq!(decoder.dimensions(), Some((76, 42)));
        assert_eq!(decoder.output_format(), Some(PixelFormat::Yuv420p));

        let frame = decoder
            .receive_frame()
            .expect("should succeed")
            .expect("one frame output");
        assert_eq!(frame.planes.len(), 3);
        assert_eq!(frame.planes[0].data.len(), 76 * 42);
        assert_eq!(frame.planes[1].data.len(), 38 * 21);
        assert_eq!(frame.planes[2].data.len(), 38 * 21);

        let expected_y = &REF_YUV[..76 * 42];
        let expected_u = &REF_YUV[76 * 42..76 * 42 + 38 * 21];
        let expected_v = &REF_YUV[76 * 42 + 38 * 21..];
        assert_eq!(frame.planes[0].data.as_slice(), expected_y, "Y plane");
        assert_eq!(frame.planes[1].data.as_slice(), expected_u, "U plane");
        assert_eq!(frame.planes[2].data.as_slice(), expected_v, "V plane");
    }

    /// A real libvpx-vp9 INTER frame (frame 1 of a 3-frame encode) must
    /// return an honest `UnsupportedFeature` error, never fabricated
    /// pixels.
    #[test]
    fn test_inter_frame_returns_honest_unsupported_error() {
        const INTER_FRAME: &[u8] = include_bytes!("kf/testdata/seq76x42.frame1.bin");

        let config = DecoderConfig::default();
        let mut decoder = Vp9Decoder::new(config).expect("should succeed");
        let result = decoder.send_packet(INTER_FRAME, 0);
        match result {
            Err(CodecError::UnsupportedFeature(msg)) => {
                assert!(
                    msg.contains("inter-frame decode not yet implemented"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected honest UnsupportedFeature, got {other:?}"),
        }
        let frame = decoder.receive_frame().expect("should succeed");
        assert!(frame.is_none(), "no fabricated frame may be output");
    }

    /// A truncated keyframe header must fail parsing — and must never
    /// produce a fabricated blank frame (regression guard for the old
    /// silent-blank-frame bug).
    #[test]
    fn test_truncated_keyframe_errors_without_blank_frame() {
        // 9 bytes: valid start of a 64x64 keyframe header, truncated before
        // the loop-filter/quant/tile sections.
        const TRUNCATED: [u8; 9] = [0x82, 0x49, 0x83, 0x42, 0x20, 0x03, 0xF0, 0x03, 0xF0];

        let config = DecoderConfig::default();
        let mut decoder = Vp9Decoder::new(config).expect("should succeed");
        assert!(decoder.send_packet(&TRUNCATED, 0).is_err());
        let frame = decoder.receive_frame().expect("should succeed");
        assert!(frame.is_none(), "no fabricated blank frame may be output");
    }
}
