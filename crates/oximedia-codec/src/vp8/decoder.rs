//! VP8 decoder implementation.
//!
//! This module provides the main VP8 decoder that implements the
//! `VideoDecoder` trait from this crate.
//!
//! VP8 is a royalty-free video codec developed by Google as part of
//! the `WebM` project. This decoder is based on RFC 6386.
//!
//! # Honest status: bitstream parsing only
//!
//! Per `docs/codec_status.md`, VP8 is **parse-only** in this release:
//! frame headers are parsed (dimensions, frame type, reference-refresh
//! flags), but pixel reconstruction (entropy decode of coefficients,
//! intra/inter prediction, inverse DCT/WHT, loop filter) is **not
//! implemented** and is deferred to 0.2.0. Instead of fabricating output
//! (an earlier revision emitted constant-gray frames), the
//! decode-to-pixels path returns an honest
//! [`CodecError::UnsupportedFeature`] error. Header information parsed
//! before the error (e.g. [`Vp8Decoder::dimensions`]) remains available.

use crate::error::{CodecError, CodecResult};
use crate::frame::VideoFrame;
use crate::traits::{DecoderConfig, VideoDecoder};
use crate::vp8::frame_header::FrameHeader;
use oximedia_core::{CodecId, PixelFormat};

/// VP8 decoder (bitstream parsing only — see module docs).
///
/// A pure Rust VP8 bitstream parser based on RFC 6386. Frame headers are
/// parsed and stream properties (dimensions, output format) are exposed,
/// but pixel reconstruction is not implemented in this release:
/// [`Vp8Decoder::send_packet`] parses the frame header and then returns an
/// honest [`CodecError::UnsupportedFeature`] error rather than producing
/// fabricated pixel data.
///
/// # Examples
///
/// ```
/// use oximedia_codec::vp8::Vp8Decoder;
/// use oximedia_codec::traits::{DecoderConfig, VideoDecoder};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = DecoderConfig::default();
/// let mut decoder = Vp8Decoder::new(config)?;
///
/// // Decoder is ready to receive packets
/// assert!(decoder.dimensions().is_none());
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct Vp8Decoder {
    /// Decoder configuration.
    #[allow(dead_code)]
    config: DecoderConfig,
    /// Current frame width.
    width: Option<u32>,
    /// Current frame height.
    height: Option<u32>,
    /// Pending frame output queue (never populated until pixel
    /// reconstruction is implemented; kept for the `VideoDecoder`
    /// push/pull contract).
    output_queue: Vec<VideoFrame>,
    /// Whether the decoder is in flush mode.
    flushing: bool,
}

impl Vp8Decoder {
    /// Creates a new VP8 decoder with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Decoder configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_codec::vp8::Vp8Decoder;
    /// use oximedia_codec::traits::DecoderConfig;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = DecoderConfig::default();
    /// let decoder = Vp8Decoder::new(config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(config: DecoderConfig) -> CodecResult<Self> {
        Ok(Self {
            config,
            width: None,
            height: None,
            output_queue: Vec::new(),
            flushing: false,
        })
    }

    /// Parses a VP8 frame and reports the pixel-reconstruction gap honestly.
    ///
    /// The frame header is parsed and stream dimensions are updated for
    /// keyframes, so callers can still probe stream properties via
    /// [`Vp8Decoder::dimensions`] / [`Vp8Decoder::output_format`]. Actual
    /// pixel reconstruction is not implemented (deferred to 0.2.0), so this
    /// always ends in [`CodecError::UnsupportedFeature`] rather than
    /// emitting fabricated (constant-gray) frames.
    fn decode_frame(&mut self, data: &[u8], _pts: i64) -> CodecResult<()> {
        // Parse frame header
        let header = FrameHeader::parse(data)?;

        // Update dimensions for keyframes
        if header.is_keyframe() {
            self.width = Some(u32::from(header.width));
            self.height = Some(u32::from(header.height));
        }

        if self.width.is_none() || self.height.is_none() {
            return Err(CodecError::InvalidBitstream(
                "VP8: No keyframe received yet, cannot decode inter frame".to_string(),
            ));
        }

        // Header parsing succeeded, but decoding this frame to pixels would
        // require macroblock entropy decode, intra/inter prediction, the
        // inverse DCT/WHT, and the loop filter — none of which are
        // implemented yet. Fail honestly instead of returning gray frames.
        Err(CodecError::UnsupportedFeature(
            "VP8 pixel reconstruction not implemented: bitstream parsing only \
             (full decode deferred to 0.2.0; see docs/codec_status.md)"
                .to_string(),
        ))
    }
}

impl VideoDecoder for Vp8Decoder {
    fn codec(&self) -> CodecId {
        CodecId::Vp8
    }

    /// Sends a packet to the decoder.
    ///
    /// The frame header is parsed (updating [`Vp8Decoder::dimensions`] on
    /// keyframes), then an honest [`CodecError::UnsupportedFeature`] error
    /// is returned because pixel reconstruction is not implemented — see
    /// the module documentation.
    fn send_packet(&mut self, data: &[u8], pts: i64) -> CodecResult<()> {
        if self.flushing {
            return Err(CodecError::InvalidParameter(
                "VP8: Cannot send packet while flushing".to_string(),
            ));
        }

        self.decode_frame(data, pts)
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
        self.flushing = false;
        // Note: we keep width/height to allow continued decoding after seek
    }

    fn output_format(&self) -> Option<PixelFormat> {
        if self.width.is_some() && self.height.is_some() {
            Some(PixelFormat::Yuv420p)
        } else {
            None
        }
    }

    fn dimensions(&self) -> Option<(u32, u32)> {
        match (self.width, self.height) {
            (Some(w), Some(h)) => Some((w, h)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vp8_decoder_new() {
        let config = DecoderConfig::default();
        let decoder = Vp8Decoder::new(config).expect("should succeed");
        assert_eq!(decoder.codec(), CodecId::Vp8);
        assert!(decoder.output_format().is_none());
        assert!(decoder.dimensions().is_none());
    }

    #[test]
    fn test_decode_keyframe_parses_header_then_errors_honestly() {
        let config = DecoderConfig::default();
        let mut decoder = Vp8Decoder::new(config).expect("should succeed");

        // Valid VP8 keyframe header
        let keyframe = [
            0x10, // frame_type=0, version=0, show=1
            0x00, 0x00, // first_partition_size
            0x9D, 0x01, 0x2A, // sync code
            0x40, 0x01, // width=320
            0xF0, 0x00, // height=240
        ];

        // Pixel reconstruction is not implemented — send_packet must return
        // an honest UnsupportedFeature error, NOT queue a fake gray frame.
        let result = decoder.send_packet(&keyframe, 0);
        assert!(
            matches!(result, Err(CodecError::UnsupportedFeature(_))),
            "expected honest UnsupportedFeature error, got {result:?}"
        );

        // But header parsing still succeeded before the error.
        assert_eq!(decoder.dimensions(), Some((320, 240)));
        assert_eq!(decoder.output_format(), Some(PixelFormat::Yuv420p));

        // No fabricated frame may be emitted.
        let frame = decoder.receive_frame().expect("should succeed");
        assert!(frame.is_none(), "no fake gray frame may be output");
    }

    #[test]
    fn test_honest_error_message_names_the_gap() {
        let config = DecoderConfig::default();
        let mut decoder = Vp8Decoder::new(config).expect("should succeed");
        let keyframe = [0x10, 0x00, 0x00, 0x9D, 0x01, 0x2A, 0x40, 0x01, 0xF0, 0x00];

        let err = match decoder.send_packet(&keyframe, 0) {
            Err(e) => e,
            Ok(()) => panic!("send_packet must not pretend to decode pixels"),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("pixel reconstruction not implemented"),
            "error must state the limitation clearly, got: {msg}"
        );
    }

    #[test]
    fn test_inter_frame_without_keyframe() {
        let config = DecoderConfig::default();
        let mut decoder = Vp8Decoder::new(config).expect("should succeed");

        // Inter frame (without prior keyframe)
        let inter = [
            0x01, // frame_type=1 (inter)
            0x00, 0x00,
        ];

        // Should fail because no keyframe was received
        assert!(decoder.send_packet(&inter, 0).is_err());
    }

    #[test]
    fn test_flush() {
        let config = DecoderConfig::default();
        let mut decoder = Vp8Decoder::new(config).expect("should succeed");

        // Send a keyframe: header parses, then the honest error is returned.
        let keyframe = [0x10, 0x00, 0x00, 0x9D, 0x01, 0x2A, 0x40, 0x01, 0xF0, 0x00];
        assert!(matches!(
            decoder.send_packet(&keyframe, 0),
            Err(CodecError::UnsupportedFeature(_))
        ));

        // Flush
        decoder.flush().expect("should succeed");

        // Should return EOF when no more frames
        assert!(matches!(decoder.receive_frame(), Err(CodecError::Eof)));

        // Cannot send more packets while flushing
        assert!(decoder.send_packet(&keyframe, 0).is_err());
    }

    #[test]
    fn test_reset() {
        let config = DecoderConfig::default();
        let mut decoder = Vp8Decoder::new(config).expect("should succeed");

        // Send a keyframe (parses, then honest error)
        let keyframe = [0x10, 0x00, 0x00, 0x9D, 0x01, 0x2A, 0x40, 0x01, 0xF0, 0x00];
        assert!(matches!(
            decoder.send_packet(&keyframe, 0),
            Err(CodecError::UnsupportedFeature(_))
        ));

        // Flush and reset
        decoder.flush().expect("should succeed");
        decoder.reset();

        // After reset, packets are accepted for parsing again (and still
        // end in the honest reconstruction error, not an InvalidParameter
        // "flushing" error).
        assert!(matches!(
            decoder.send_packet(&keyframe, 0),
            Err(CodecError::UnsupportedFeature(_))
        ));
        // Dimensions survive reset (allows continued probing after seek).
        assert_eq!(decoder.dimensions(), Some((320, 240)));
    }

    #[test]
    fn test_no_frame_available() {
        let config = DecoderConfig::default();
        let mut decoder = Vp8Decoder::new(config).expect("should succeed");

        // No packets sent, no frames available
        let result = decoder.receive_frame().expect("should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn test_hidden_frame_header_parsed_before_honest_error() {
        let config = DecoderConfig::default();
        let mut decoder = Vp8Decoder::new(config).expect("should succeed");

        // Keyframe with show_frame=0 (hidden). Even hidden frames need
        // pixel reconstruction (they become references), so the honest
        // error applies here too.
        let hidden_keyframe = [
            0x00, // frame_type=0, version=0, show=0
            0x00, 0x00, 0x9D, 0x01, 0x2A, 0x40, 0x01, 0xF0, 0x00,
        ];

        assert!(matches!(
            decoder.send_packet(&hidden_keyframe, 0),
            Err(CodecError::UnsupportedFeature(_))
        ));

        // Dimensions should be updated by header parsing
        assert_eq!(decoder.dimensions(), Some((320, 240)));

        // And no frame should be output
        let frame = decoder.receive_frame().expect("should succeed");
        assert!(frame.is_none());
    }
}
