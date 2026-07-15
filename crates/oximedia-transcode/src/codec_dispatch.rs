// Copyright 2025 OxiMedia Contributors
// Licensed under the Apache License, Version 2.0

//! Codec encoder factory for the intra-frame video codecs with real
//! implementations in `oximedia-codec`.
//!
//! This module provides [`make_video_encoder`], which constructs a boxed
//! [`oximedia_codec::VideoEncoder`] for a given [`CodecId`].  Dispatched
//! codecs:
//!
//! | `CodecId`         | Encoder              | Feature gate | Input           |
//! |-------------------|----------------------|--------------|-----------------|
//! | `CodecId::Mjpeg`  | `MjpegEncoder`       | `mjpeg`      | 8-bit YUV/RGB   |
//! | `CodecId::Apv`    | `ApvEncoder`         | `apv`        | 8-bit YUV/RGB   |
//! | `CodecId::Mpeg2`  | `Mpeg2Encoder`       | `mpeg2`      | 8-bit YUV 4:2:0 |
//! | `CodecId::Ffv1`   | `Ffv1Encoder`        | `ffv1`       | 8-bit YUV 4:2:0 |
//! | `CodecId::ProRes` | `ProResEncoder`      | `prores`     | 10-bit 4:2:2 LE |
//!
//! `quality` interpretation is codec-specific — see [`VideoEncoderParams`].
//! AV1/VP9/VP8 are intentionally not dispatched: their encode paths do not
//! produce real output yet, and callers get a descriptive `Unsupported`
//! error instead of a fabricated file.

use crate::{Result, TranscodeError};
use oximedia_codec::traits::VideoEncoder;
#[cfg(feature = "mjpeg")]
use oximedia_codec::CodecError;
use oximedia_core::CodecId;

/// Parameters used to instantiate an intra-frame video encoder.
#[derive(Debug, Clone)]
pub struct VideoEncoderParams {
    /// Frame width in pixels (must be > 0).
    pub width: u32,
    /// Frame height in pixels (must be > 0).
    pub height: u32,
    /// Quality/QP value.  Interpretation depends on the codec:
    /// - MJPEG: JPEG quality 1-100 (higher = better).
    /// - APV: quantisation parameter 0-63 (lower = better).
    pub quality: u8,
}

impl VideoEncoderParams {
    /// Create a new parameter set.
    ///
    /// # Errors
    ///
    /// Returns [`TranscodeError::InvalidInput`] if width or height is zero.
    pub fn new(width: u32, height: u32, quality: u8) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(TranscodeError::InvalidInput(
                "width and height must be non-zero".into(),
            ));
        }
        Ok(Self {
            width,
            height,
            quality,
        })
    }
}

/// Build a boxed [`VideoEncoder`] for the specified codec.
///
/// # Errors
///
/// - [`TranscodeError::Unsupported`] if `codec_id` has no real encoder
///   (AV1/VP9/VP8) or its feature is not compiled in.
/// - [`TranscodeError::CodecError`] if the underlying encoder rejects the
///   parameters.
pub fn make_video_encoder(
    codec_id: CodecId,
    params: &VideoEncoderParams,
) -> Result<Box<dyn VideoEncoder>> {
    match codec_id {
        CodecId::Mjpeg => make_mjpeg_encoder(params),
        CodecId::Apv => make_apv_encoder(params),
        CodecId::Mpeg2 => make_mpeg2_encoder(params),
        CodecId::Ffv1 => make_ffv1_encoder(params),
        CodecId::ProRes => make_prores_encoder(params),
        other => Err(TranscodeError::Unsupported(format!(
            "codec {other:?} has no real encoder in this build; \
             transcoding to it is not yet supported"
        ))),
    }
}

// ─── MJPEG ───────────────────────────────────────────────────────────────────

#[cfg(feature = "mjpeg")]
fn make_mjpeg_encoder(params: &VideoEncoderParams) -> Result<Box<dyn VideoEncoder>> {
    use oximedia_codec::{MjpegConfig, MjpegEncoder};

    let config = MjpegConfig::new(params.width, params.height)
        .map_err(|e| TranscodeError::CodecError(e.to_string()))?
        .with_quality(params.quality);

    let encoder = MjpegEncoder::new(config)
        .map_err(|e: CodecError| TranscodeError::CodecError(e.to_string()))?;

    Ok(Box::new(encoder))
}

#[cfg(not(feature = "mjpeg"))]
fn make_mjpeg_encoder(_params: &VideoEncoderParams) -> Result<Box<dyn VideoEncoder>> {
    Err(TranscodeError::Unsupported(
        "MJPEG support requires the `mjpeg` feature of oximedia-codec".into(),
    ))
}

// ─── APV ─────────────────────────────────────────────────────────────────────

#[cfg(feature = "apv")]
fn make_apv_encoder(params: &VideoEncoderParams) -> Result<Box<dyn VideoEncoder>> {
    use oximedia_codec::{ApvConfig, ApvEncoder};

    let config = ApvConfig::new(params.width, params.height)
        .map_err(|e| TranscodeError::CodecError(e.to_string()))?
        .with_qp(params.quality);

    let encoder = ApvEncoder::new(config).map_err(|e| TranscodeError::CodecError(e.to_string()))?;

    Ok(Box::new(encoder))
}

#[cfg(not(feature = "apv"))]
fn make_apv_encoder(_params: &VideoEncoderParams) -> Result<Box<dyn VideoEncoder>> {
    Err(TranscodeError::Unsupported(
        "APV support requires the `apv` feature of oximedia-codec".into(),
    ))
}

// ─── MPEG-2 ──────────────────────────────────────────────────────────────────

#[cfg(feature = "mpeg2")]
fn make_mpeg2_encoder(params: &VideoEncoderParams) -> Result<Box<dyn VideoEncoder>> {
    use oximedia_codec::mpeg2::{Mpeg2Encoder, Mpeg2EncoderConfig};

    // MPEG-2 qscale range is 1..=31 (lower = better quality).
    let qscale = params.quality.clamp(1, 31);
    let config = Mpeg2EncoderConfig::yuv420p(params.width, params.height, qscale);
    let encoder =
        Mpeg2Encoder::new(config).map_err(|e| TranscodeError::CodecError(e.to_string()))?;
    Ok(Box::new(encoder))
}

#[cfg(not(feature = "mpeg2"))]
fn make_mpeg2_encoder(_params: &VideoEncoderParams) -> Result<Box<dyn VideoEncoder>> {
    Err(TranscodeError::Unsupported(
        "MPEG-2 support requires the `mpeg2` feature of oximedia-codec".into(),
    ))
}

// ─── FFV1 ────────────────────────────────────────────────────────────────────

#[cfg(feature = "ffv1")]
fn make_ffv1_encoder(params: &VideoEncoderParams) -> Result<Box<dyn VideoEncoder>> {
    use oximedia_codec::traits::EncoderConfig;
    use oximedia_codec::Ffv1Encoder;
    use oximedia_core::PixelFormat;

    // FFV1 is lossless — `quality` has no effect. All-intra (keyint = 1)
    // keeps every frame independently decodable.
    let config = EncoderConfig {
        codec: CodecId::Ffv1,
        width: params.width,
        height: params.height,
        pixel_format: PixelFormat::Yuv420p,
        keyint: 1,
        ..EncoderConfig::default()
    };
    let encoder =
        Ffv1Encoder::new(config).map_err(|e| TranscodeError::CodecError(e.to_string()))?;
    Ok(Box::new(encoder))
}

#[cfg(not(feature = "ffv1"))]
fn make_ffv1_encoder(_params: &VideoEncoderParams) -> Result<Box<dyn VideoEncoder>> {
    Err(TranscodeError::Unsupported(
        "FFV1 support requires the `ffv1` feature of oximedia-codec".into(),
    ))
}

// ─── ProRes ──────────────────────────────────────────────────────────────────

#[cfg(feature = "prores")]
fn make_prores_encoder(params: &VideoEncoderParams) -> Result<Box<dyn VideoEncoder>> {
    use oximedia_codec::{ProResEncoder, ProResEncoderConfig, ProResProfile};

    // The real ProRes encoder requires 16-pixel-aligned dimensions and
    // 10-bit 4:2:2 (`Yuv422p10le`) input frames.
    if params.width % 16 != 0 || params.height % 16 != 0 {
        return Err(TranscodeError::CodecError(format!(
            "ProRes requires dimensions that are multiples of 16, got {}x{}",
            params.width, params.height
        )));
    }
    let config = ProResEncoderConfig::new(ProResProfile::Standard, params.width, params.height);
    let encoder =
        ProResEncoder::new(config).map_err(|e| TranscodeError::CodecError(e.to_string()))?;
    Ok(Box::new(encoder))
}

#[cfg(not(feature = "prores"))]
fn make_prores_encoder(_params: &VideoEncoderParams) -> Result<Box<dyn VideoEncoder>> {
    Err(TranscodeError::Unsupported(
        "ProRes support requires the `prores` feature of oximedia-codec".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_params_new_valid() {
        let p = VideoEncoderParams::new(1920, 1080, 85);
        assert!(p.is_ok());
        let p = p.expect("valid params");
        assert_eq!(p.width, 1920);
        assert_eq!(p.height, 1080);
        assert_eq!(p.quality, 85);
    }

    #[test]
    fn test_params_zero_width() {
        assert!(VideoEncoderParams::new(0, 1080, 85).is_err());
    }

    #[test]
    fn test_params_zero_height() {
        assert!(VideoEncoderParams::new(1920, 0, 85).is_err());
    }

    #[test]
    fn test_unsupported_codec() {
        let p = VideoEncoderParams::new(320, 240, 30).expect("valid");
        let result = make_video_encoder(CodecId::Vp9, &p);
        assert!(result.is_err());
        // Extract the error without requiring Debug on the Ok variant.
        if let Err(e) = result {
            assert!(matches!(e, TranscodeError::Unsupported(_)));
        }
    }

    #[cfg(feature = "mjpeg")]
    #[test]
    fn test_make_mjpeg_encoder() {
        let p = VideoEncoderParams::new(320, 240, 85).expect("valid");
        let enc = make_video_encoder(CodecId::Mjpeg, &p);
        assert!(enc.is_ok(), "MJPEG encoder should build");
        let enc = enc.expect("ok");
        assert_eq!(enc.codec(), CodecId::Mjpeg);
    }

    #[cfg(feature = "apv")]
    #[test]
    fn test_make_apv_encoder() {
        let p = VideoEncoderParams::new(320, 240, 22).expect("valid");
        let enc = make_video_encoder(CodecId::Apv, &p);
        assert!(enc.is_ok(), "APV encoder should build");
        let enc = enc.expect("ok");
        assert_eq!(enc.codec(), CodecId::Apv);
    }

    #[cfg(not(feature = "mjpeg"))]
    #[test]
    fn test_mjpeg_disabled() {
        let p = VideoEncoderParams::new(320, 240, 85).expect("valid");
        let result = make_video_encoder(CodecId::Mjpeg, &p);
        assert!(matches!(result, Err(TranscodeError::Unsupported(_))));
    }

    #[cfg(not(feature = "apv"))]
    #[test]
    fn test_apv_disabled() {
        let p = VideoEncoderParams::new(320, 240, 22).expect("valid");
        let result = make_video_encoder(CodecId::Apv, &p);
        assert!(matches!(result, Err(TranscodeError::Unsupported(_))));
    }
}
