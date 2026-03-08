//! Frame types for passing through the filter graph.
//!
//! This module provides wrapper types for video and audio frames that can be
//! passed between nodes in the filter graph.

use std::sync::Arc;

use oximedia_audio::{AudioFrame, ChannelLayout};
use oximedia_codec::VideoFrame;
use oximedia_core::{PixelFormat, SampleFormat, Timestamp};

/// A frame that can be passed through the filter graph.
#[derive(Clone, Debug)]
pub enum FilterFrame {
    /// Video frame.
    Video(VideoFrame),
    /// Audio frame.
    Audio(AudioFrame),
}

impl FilterFrame {
    /// Get the timestamp of the frame.
    #[must_use]
    pub fn timestamp(&self) -> &Timestamp {
        match self {
            Self::Video(f) => &f.timestamp,
            Self::Audio(f) => &f.timestamp,
        }
    }

    /// Check if this is a video frame.
    #[must_use]
    pub fn is_video(&self) -> bool {
        matches!(self, Self::Video(_))
    }

    /// Check if this is an audio frame.
    #[must_use]
    pub fn is_audio(&self) -> bool {
        matches!(self, Self::Audio(_))
    }

    /// Get as video frame if applicable.
    #[must_use]
    pub fn as_video(&self) -> Option<&VideoFrame> {
        match self {
            Self::Video(f) => Some(f),
            Self::Audio(_) => None,
        }
    }

    /// Get as audio frame if applicable.
    #[must_use]
    pub fn as_audio(&self) -> Option<&AudioFrame> {
        match self {
            Self::Video(_) => None,
            Self::Audio(f) => Some(f),
        }
    }

    /// Get mutable video frame if applicable.
    pub fn as_video_mut(&mut self) -> Option<&mut VideoFrame> {
        match self {
            Self::Video(f) => Some(f),
            Self::Audio(_) => None,
        }
    }

    /// Get mutable audio frame if applicable.
    pub fn as_audio_mut(&mut self) -> Option<&mut AudioFrame> {
        match self {
            Self::Video(_) => None,
            Self::Audio(f) => Some(f),
        }
    }
}

impl From<VideoFrame> for FilterFrame {
    fn from(frame: VideoFrame) -> Self {
        Self::Video(frame)
    }
}

impl From<AudioFrame> for FilterFrame {
    fn from(frame: AudioFrame) -> Self {
        Self::Audio(frame)
    }
}

/// Reference-counted frame for zero-copy passing.
///
/// When a frame needs to be shared between multiple consumers without copying,
/// use `FrameRef` to wrap it in an `Arc`.
#[derive(Clone, Debug)]
pub struct FrameRef {
    inner: Arc<FilterFrame>,
}

impl FrameRef {
    /// Create a new frame reference.
    pub fn new(frame: FilterFrame) -> Self {
        Self {
            inner: Arc::new(frame),
        }
    }

    /// Get a reference to the inner frame.
    #[must_use]
    pub fn frame(&self) -> &FilterFrame {
        &self.inner
    }

    /// Try to get exclusive access to the frame.
    ///
    /// Returns `Some` if this is the only reference, `None` otherwise.
    pub fn try_unwrap(self) -> Option<FilterFrame> {
        Arc::try_unwrap(self.inner).ok()
    }

    /// Get the reference count.
    #[must_use]
    pub fn ref_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    /// Make a copy of the frame if needed for mutation.
    ///
    /// If this is the only reference, returns the frame directly.
    /// Otherwise, clones the frame.
    #[must_use]
    pub fn make_mut(self) -> FilterFrame {
        match Arc::try_unwrap(self.inner) {
            Ok(frame) => frame,
            Err(arc) => (*arc).clone(),
        }
    }
}

impl From<FilterFrame> for FrameRef {
    fn from(frame: FilterFrame) -> Self {
        Self::new(frame)
    }
}

impl From<VideoFrame> for FrameRef {
    fn from(frame: VideoFrame) -> Self {
        Self::new(FilterFrame::Video(frame))
    }
}

impl From<AudioFrame> for FrameRef {
    fn from(frame: AudioFrame) -> Self {
        Self::new(FilterFrame::Audio(frame))
    }
}

/// Frame pool for reusing frame allocations.
///
/// This is a placeholder for integration with `oximedia-core`'s buffer pool.
#[allow(dead_code)]
pub struct FramePool {
    /// Maximum number of frames to keep in the pool.
    capacity: usize,
    /// Pooled video frames.
    video_frames: Vec<VideoFrame>,
    /// Pooled audio frames.
    audio_frames: Vec<AudioFrame>,
}

impl FramePool {
    /// Create a new frame pool with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            video_frames: Vec::with_capacity(capacity),
            audio_frames: Vec::with_capacity(capacity),
        }
    }

    /// Get a video frame from the pool or create a new one.
    #[must_use]
    pub fn get_video_frame(&mut self, format: PixelFormat, width: u32, height: u32) -> VideoFrame {
        // Try to find a matching frame in the pool
        if let Some(pos) = self
            .video_frames
            .iter()
            .position(|f| f.format == format && f.width == width && f.height == height)
        {
            return self.video_frames.swap_remove(pos);
        }

        // Create a new frame
        let mut frame = VideoFrame::new(format, width, height);
        frame.allocate();
        frame
    }

    /// Return a video frame to the pool.
    pub fn return_video_frame(&mut self, frame: VideoFrame) {
        if self.video_frames.len() < self.capacity {
            self.video_frames.push(frame);
        }
    }

    /// Get an audio frame from the pool or create a new one.
    #[must_use]
    pub fn get_audio_frame(
        &mut self,
        format: SampleFormat,
        sample_rate: u32,
        channels: ChannelLayout,
    ) -> AudioFrame {
        // Try to find a matching frame in the pool
        if let Some(pos) = self.audio_frames.iter().position(|f| {
            f.format == format && f.sample_rate == sample_rate && f.channels == channels
        }) {
            return self.audio_frames.swap_remove(pos);
        }

        // Create a new frame
        AudioFrame::new(format, sample_rate, channels)
    }

    /// Return an audio frame to the pool.
    pub fn return_audio_frame(&mut self, frame: AudioFrame) {
        if self.audio_frames.len() < self.capacity {
            self.audio_frames.push(frame);
        }
    }

    /// Clear all pooled frames.
    pub fn clear(&mut self) {
        self.video_frames.clear();
        self.audio_frames.clear();
    }

    /// Get the number of video frames in the pool.
    #[must_use]
    pub fn video_frame_count(&self) -> usize {
        self.video_frames.len()
    }

    /// Get the number of audio frames in the pool.
    #[must_use]
    pub fn audio_frame_count(&self) -> usize {
        self.audio_frames.len()
    }
}

impl Default for FramePool {
    fn default() -> Self {
        Self::new(16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_frame_video() {
        let video = VideoFrame::new(PixelFormat::Yuv420p, 1920, 1080);
        let frame = FilterFrame::Video(video);

        assert!(frame.is_video());
        assert!(!frame.is_audio());
        assert!(frame.as_video().is_some());
        assert!(frame.as_audio().is_none());
    }

    #[test]
    fn test_filter_frame_audio() {
        let audio = AudioFrame::new(SampleFormat::F32, 48000, ChannelLayout::Stereo);
        let frame = FilterFrame::Audio(audio);

        assert!(!frame.is_video());
        assert!(frame.is_audio());
        assert!(frame.as_video().is_none());
        assert!(frame.as_audio().is_some());
    }

    #[test]
    fn test_frame_ref() {
        let video = VideoFrame::new(PixelFormat::Yuv420p, 1920, 1080);
        let frame = FilterFrame::Video(video);
        let frame_ref = FrameRef::new(frame);

        assert_eq!(frame_ref.ref_count(), 1);

        let frame_ref2 = frame_ref.clone();
        assert_eq!(frame_ref.ref_count(), 2);
        assert_eq!(frame_ref2.ref_count(), 2);
    }

    #[test]
    fn test_frame_ref_try_unwrap() {
        let video = VideoFrame::new(PixelFormat::Yuv420p, 1920, 1080);
        let frame = FilterFrame::Video(video);
        let frame_ref = FrameRef::new(frame);

        // Should succeed with single reference
        let unwrapped = frame_ref.try_unwrap();
        assert!(unwrapped.is_some());
    }

    #[test]
    fn test_frame_ref_make_mut() {
        let video = VideoFrame::new(PixelFormat::Yuv420p, 1920, 1080);
        let frame = FilterFrame::Video(video);
        let frame_ref = FrameRef::new(frame);
        let frame_ref2 = frame_ref.clone();

        // Should clone since there are multiple references
        let owned = frame_ref.make_mut();
        assert!(owned.is_video());

        // frame_ref2 should still be valid
        assert!(frame_ref2.frame().is_video());
    }

    #[test]
    fn test_frame_pool() {
        let mut pool = FramePool::new(4);

        // Get a new frame
        let frame = pool.get_video_frame(PixelFormat::Yuv420p, 1920, 1080);
        assert_eq!(frame.width, 1920);
        assert_eq!(frame.height, 1080);

        // Return it to the pool
        pool.return_video_frame(frame);
        assert_eq!(pool.video_frame_count(), 1);

        // Get it back (should be the same allocation)
        let frame2 = pool.get_video_frame(PixelFormat::Yuv420p, 1920, 1080);
        assert_eq!(frame2.width, 1920);
        assert_eq!(pool.video_frame_count(), 0);
    }

    #[test]
    fn test_filter_frame_from() {
        let video = VideoFrame::new(PixelFormat::Yuv420p, 1920, 1080);
        let frame: FilterFrame = video.into();
        assert!(frame.is_video());

        let audio = AudioFrame::new(SampleFormat::F32, 48000, ChannelLayout::Stereo);
        let frame: FilterFrame = audio.into();
        assert!(frame.is_audio());
    }

    #[test]
    fn test_frame_timestamp() {
        use oximedia_core::Rational;

        let mut video = VideoFrame::new(PixelFormat::Yuv420p, 1920, 1080);
        video.timestamp = Timestamp::new(1000, Rational::new(1, 1000));
        let frame = FilterFrame::Video(video);

        assert_eq!(frame.timestamp().pts, 1000);
    }
}
