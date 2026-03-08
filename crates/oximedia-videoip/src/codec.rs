//! Video and audio codec wrapper for encoding/decoding.

use crate::error::VideoIpResult;
use crate::types::{AudioCodec, VideoCodec};
use bytes::Bytes;

/// Video frame data.
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Frame data.
    pub data: Bytes,
    /// Frame width.
    pub width: u32,
    /// Frame height.
    pub height: u32,
    /// Whether this is a keyframe.
    pub is_keyframe: bool,
    /// Presentation timestamp in microseconds.
    pub pts: u64,
}

impl VideoFrame {
    /// Creates a new video frame.
    #[must_use]
    pub const fn new(data: Bytes, width: u32, height: u32, is_keyframe: bool, pts: u64) -> Self {
        Self {
            data,
            width,
            height,
            is_keyframe,
            pts,
        }
    }
}

/// Audio samples data.
#[derive(Debug, Clone)]
pub struct AudioSamples {
    /// Audio data.
    pub data: Bytes,
    /// Number of samples per channel.
    pub sample_count: usize,
    /// Number of channels.
    pub channels: u8,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Presentation timestamp in microseconds.
    pub pts: u64,
}

impl AudioSamples {
    /// Creates new audio samples.
    #[must_use]
    pub const fn new(
        data: Bytes,
        sample_count: usize,
        channels: u8,
        sample_rate: u32,
        pts: u64,
    ) -> Self {
        Self {
            data,
            sample_count,
            channels,
            sample_rate,
            pts,
        }
    }
}

/// Video encoder interface.
pub trait VideoEncoder: Send + Sync {
    /// Encodes a video frame.
    ///
    /// # Errors
    ///
    /// Returns an error if encoding fails.
    fn encode(&mut self, frame: &VideoFrame) -> VideoIpResult<Bytes>;

    /// Flushes any buffered frames.
    ///
    /// # Errors
    ///
    /// Returns an error if flushing fails.
    fn flush(&mut self) -> VideoIpResult<Vec<Bytes>>;
}

/// Video decoder interface.
pub trait VideoDecoder: Send + Sync {
    /// Decodes video data.
    ///
    /// # Errors
    ///
    /// Returns an error if decoding fails.
    fn decode(&mut self, data: &[u8]) -> VideoIpResult<Option<VideoFrame>>;
}

/// Audio encoder interface.
pub trait AudioEncoder: Send + Sync {
    /// Encodes audio samples.
    ///
    /// # Errors
    ///
    /// Returns an error if encoding fails.
    fn encode(&mut self, samples: &AudioSamples) -> VideoIpResult<Bytes>;
}

/// Audio decoder interface.
pub trait AudioDecoder: Send + Sync {
    /// Decodes audio data.
    ///
    /// # Errors
    ///
    /// Returns an error if decoding fails.
    fn decode(&mut self, data: &[u8]) -> VideoIpResult<Option<AudioSamples>>;
}

/// Creates a video encoder for the specified codec.
///
/// # Errors
///
/// Returns an error if the codec is not supported.
pub fn create_video_encoder(
    codec: VideoCodec,
    width: u32,
    height: u32,
    _bitrate: Option<u64>,
) -> VideoIpResult<Box<dyn VideoEncoder>> {
    match codec {
        VideoCodec::Vp9 | VideoCodec::Av1 | VideoCodec::Vp8 => {
            // In a real implementation, this would create actual codec instances
            Ok(Box::new(DummyVideoEncoder::new(width, height)))
        }
        VideoCodec::V210 | VideoCodec::Uyvy | VideoCodec::Yuv420p | VideoCodec::Yuv420p10 => {
            // Uncompressed formats don't need encoding
            Ok(Box::new(PassthroughVideoEncoder))
        }
    }
}

/// Creates a video decoder for the specified codec.
///
/// # Errors
///
/// Returns an error if the codec is not supported.
pub fn create_video_decoder(codec: VideoCodec) -> VideoIpResult<Box<dyn VideoDecoder>> {
    match codec {
        VideoCodec::Vp9 | VideoCodec::Av1 | VideoCodec::Vp8 => {
            Ok(Box::new(DummyVideoDecoder::new()))
        }
        VideoCodec::V210 | VideoCodec::Uyvy | VideoCodec::Yuv420p | VideoCodec::Yuv420p10 => {
            Ok(Box::new(PassthroughVideoDecoder))
        }
    }
}

/// Creates an audio encoder for the specified codec.
///
/// # Errors
///
/// Returns an error if the codec is not supported.
pub fn create_audio_encoder(
    codec: AudioCodec,
    _sample_rate: u32,
    _channels: u8,
) -> VideoIpResult<Box<dyn AudioEncoder>> {
    match codec {
        AudioCodec::Opus => Ok(Box::new(DummyAudioEncoder)),
        AudioCodec::Pcm16 | AudioCodec::Pcm24 | AudioCodec::PcmF32 => {
            Ok(Box::new(PassthroughAudioEncoder))
        }
    }
}

/// Creates an audio decoder for the specified codec.
///
/// # Errors
///
/// Returns an error if the codec is not supported.
pub fn create_audio_decoder(codec: AudioCodec) -> VideoIpResult<Box<dyn AudioDecoder>> {
    match codec {
        AudioCodec::Opus => Ok(Box::new(DummyAudioDecoder)),
        AudioCodec::Pcm16 | AudioCodec::Pcm24 | AudioCodec::PcmF32 => {
            Ok(Box::new(PassthroughAudioDecoder))
        }
    }
}

// Dummy implementations for testing

#[allow(dead_code)]
struct DummyVideoEncoder {
    width: u32,
    height: u32,
}

impl DummyVideoEncoder {
    const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl VideoEncoder for DummyVideoEncoder {
    fn encode(&mut self, frame: &VideoFrame) -> VideoIpResult<Bytes> {
        // In a real implementation, this would use actual codec
        Ok(frame.data.clone())
    }

    fn flush(&mut self) -> VideoIpResult<Vec<Bytes>> {
        Ok(Vec::new())
    }
}

struct DummyVideoDecoder;

impl DummyVideoDecoder {
    const fn new() -> Self {
        Self
    }
}

impl VideoDecoder for DummyVideoDecoder {
    fn decode(&mut self, data: &[u8]) -> VideoIpResult<Option<VideoFrame>> {
        // In a real implementation, this would use actual codec
        Ok(Some(VideoFrame::new(
            Bytes::copy_from_slice(data),
            1920,
            1080,
            true,
            0,
        )))
    }
}

struct PassthroughVideoEncoder;

impl VideoEncoder for PassthroughVideoEncoder {
    fn encode(&mut self, frame: &VideoFrame) -> VideoIpResult<Bytes> {
        Ok(frame.data.clone())
    }

    fn flush(&mut self) -> VideoIpResult<Vec<Bytes>> {
        Ok(Vec::new())
    }
}

struct PassthroughVideoDecoder;

impl VideoDecoder for PassthroughVideoDecoder {
    fn decode(&mut self, data: &[u8]) -> VideoIpResult<Option<VideoFrame>> {
        Ok(Some(VideoFrame::new(
            Bytes::copy_from_slice(data),
            1920,
            1080,
            false,
            0,
        )))
    }
}

struct DummyAudioEncoder;

impl AudioEncoder for DummyAudioEncoder {
    fn encode(&mut self, samples: &AudioSamples) -> VideoIpResult<Bytes> {
        Ok(samples.data.clone())
    }
}

struct DummyAudioDecoder;

impl AudioDecoder for DummyAudioDecoder {
    fn decode(&mut self, data: &[u8]) -> VideoIpResult<Option<AudioSamples>> {
        Ok(Some(AudioSamples::new(
            Bytes::copy_from_slice(data),
            1024,
            2,
            48000,
            0,
        )))
    }
}

struct PassthroughAudioEncoder;

impl AudioEncoder for PassthroughAudioEncoder {
    fn encode(&mut self, samples: &AudioSamples) -> VideoIpResult<Bytes> {
        Ok(samples.data.clone())
    }
}

struct PassthroughAudioDecoder;

impl AudioDecoder for PassthroughAudioDecoder {
    fn decode(&mut self, data: &[u8]) -> VideoIpResult<Option<AudioSamples>> {
        Ok(Some(AudioSamples::new(
            Bytes::copy_from_slice(data),
            1024,
            2,
            48000,
            0,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_frame_creation() {
        let frame = VideoFrame::new(Bytes::from_static(b"data"), 1920, 1080, true, 12345);
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);
        assert!(frame.is_keyframe);
        assert_eq!(frame.pts, 12345);
    }

    #[test]
    fn test_audio_samples_creation() {
        let samples = AudioSamples::new(Bytes::from_static(b"data"), 1024, 2, 48000, 12345);
        assert_eq!(samples.sample_count, 1024);
        assert_eq!(samples.channels, 2);
        assert_eq!(samples.sample_rate, 48000);
        assert_eq!(samples.pts, 12345);
    }

    #[test]
    fn test_create_video_encoder() {
        let encoder = create_video_encoder(VideoCodec::Vp9, 1920, 1080, None);
        assert!(encoder.is_ok());
    }

    #[test]
    fn test_create_video_decoder() {
        let decoder = create_video_decoder(VideoCodec::Vp9);
        assert!(decoder.is_ok());
    }

    #[test]
    fn test_create_audio_encoder() {
        let encoder = create_audio_encoder(AudioCodec::Opus, 48000, 2);
        assert!(encoder.is_ok());
    }

    #[test]
    fn test_create_audio_decoder() {
        let decoder = create_audio_decoder(AudioCodec::Opus);
        assert!(decoder.is_ok());
    }

    #[test]
    fn test_video_encoder_encode() {
        let mut encoder = create_video_encoder(VideoCodec::Vp9, 1920, 1080, None)
            .expect("should succeed in test");
        let frame = VideoFrame::new(Bytes::from_static(b"test"), 1920, 1080, true, 0);
        let encoded = encoder.encode(&frame);
        assert!(encoded.is_ok());
    }

    #[test]
    fn test_video_decoder_decode() {
        let mut decoder = create_video_decoder(VideoCodec::Vp9).expect("should succeed in test");
        let decoded = decoder.decode(b"test");
        assert!(decoded.is_ok());
    }
}
