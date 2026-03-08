#![allow(dead_code)]
//! Frame buffer management for broadcast playout.
//!
//! Provides a fixed-capacity ring buffer of video frames with pre-roll
//! support, frame-accurate indexing, and under-run / overflow detection.

use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Pixel format used in a frame buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8-bit YUV 4:2:2.
    Yuv422P8,
    /// 10-bit YUV 4:2:2.
    Yuv422P10,
    /// 8-bit RGBA.
    Rgba8,
    /// 10-bit RGBA.
    Rgba10,
}

impl PixelFormat {
    /// Bytes per pixel (approximate for planar formats).
    #[allow(clippy::cast_precision_loss)]
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            Self::Yuv422P8 => 2,
            Self::Yuv422P10 => 3,
            Self::Rgba8 => 4,
            Self::Rgba10 => 5,
        }
    }
}

/// Metadata attached to each buffered frame.
#[derive(Debug, Clone)]
pub struct FrameMeta {
    /// Monotonic frame index since playout start.
    pub frame_index: u64,
    /// Presentation timestamp in microseconds.
    pub pts_us: i64,
    /// Whether this frame is a keyframe.
    pub is_key: bool,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel format.
    pub format: PixelFormat,
}

/// A buffered video frame.
#[derive(Debug, Clone)]
pub struct BufferedFrame {
    /// Frame metadata.
    pub meta: FrameMeta,
    /// Raw pixel data (opaque byte vector).
    pub data: Vec<u8>,
}

/// Statistics reported by the frame buffer.
#[derive(Debug, Clone, Default)]
pub struct BufferStats {
    /// Total frames pushed into the buffer since creation.
    pub total_pushed: u64,
    /// Total frames popped (consumed).
    pub total_popped: u64,
    /// Number of times a push was rejected because the buffer was full.
    pub overflow_count: u64,
    /// Number of times a pop was attempted on an empty buffer.
    pub underrun_count: u64,
}

/// Configuration for the frame ring buffer.
#[derive(Debug, Clone)]
pub struct FrameBufferConfig {
    /// Maximum number of frames the buffer can hold.
    pub capacity: usize,
    /// Number of frames that must be buffered before playout may begin.
    pub pre_roll: usize,
}

impl Default for FrameBufferConfig {
    fn default() -> Self {
        Self {
            capacity: 30,
            pre_roll: 5,
        }
    }
}

// ---------------------------------------------------------------------------
// Frame Ring Buffer
// ---------------------------------------------------------------------------

/// A ring buffer of video frames with pre-roll gating and statistics.
#[derive(Debug)]
pub struct FrameBuffer {
    config: FrameBufferConfig,
    ring: VecDeque<BufferedFrame>,
    stats: BufferStats,
    pre_roll_met: bool,
}

impl FrameBuffer {
    /// Create a new frame buffer with the given configuration.
    pub fn new(config: FrameBufferConfig) -> Self {
        let cap = config.capacity;
        Self {
            config,
            ring: VecDeque::with_capacity(cap),
            stats: BufferStats::default(),
            pre_roll_met: false,
        }
    }

    /// Return the current fill level (number of buffered frames).
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// Check whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// Check whether the buffer is full.
    pub fn is_full(&self) -> bool {
        self.ring.len() >= self.config.capacity
    }

    /// Check whether the pre-roll requirement has been met.
    pub fn pre_roll_ready(&self) -> bool {
        self.pre_roll_met
    }

    /// Return a snapshot of the current buffer statistics.
    pub fn stats(&self) -> &BufferStats {
        &self.stats
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &FrameBufferConfig {
        &self.config
    }

    /// Push a frame into the buffer.
    ///
    /// Returns `true` if the frame was accepted, `false` if the buffer was
    /// full (overflow).
    pub fn push(&mut self, frame: BufferedFrame) -> bool {
        if self.is_full() {
            self.stats.overflow_count += 1;
            return false;
        }
        self.ring.push_back(frame);
        self.stats.total_pushed += 1;

        if !self.pre_roll_met && self.ring.len() >= self.config.pre_roll {
            self.pre_roll_met = true;
        }
        true
    }

    /// Pop the oldest frame from the buffer.
    ///
    /// Returns `None` and increments the under-run counter if the buffer is
    /// empty.
    pub fn pop(&mut self) -> Option<BufferedFrame> {
        if let Some(f) = self.ring.pop_front() {
            self.stats.total_popped += 1;
            Some(f)
        } else {
            self.stats.underrun_count += 1;
            None
        }
    }

    /// Peek at the next frame without removing it.
    pub fn peek(&self) -> Option<&BufferedFrame> {
        self.ring.front()
    }

    /// Flush all buffered frames and reset pre-roll state.
    pub fn flush(&mut self) {
        self.ring.clear();
        self.pre_roll_met = false;
    }

    /// Fill percentage as a value in [0.0, 1.0].
    #[allow(clippy::cast_precision_loss)]
    pub fn fill_ratio(&self) -> f64 {
        if self.config.capacity == 0 {
            return 0.0;
        }
        self.ring.len() as f64 / self.config.capacity as f64
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

fn make_frame(idx: u64) -> BufferedFrame {
    BufferedFrame {
        meta: FrameMeta {
            frame_index: idx,
            pts_us: (idx as i64) * 40_000,
            is_key: idx.is_multiple_of(10),
            width: 1920,
            height: 1080,
            format: PixelFormat::Yuv422P8,
        },
        data: vec![0u8; 64],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = FrameBufferConfig::default();
        assert_eq!(cfg.capacity, 30);
        assert_eq!(cfg.pre_roll, 5);
    }

    #[test]
    fn test_push_pop_single() {
        let mut buf = FrameBuffer::new(FrameBufferConfig::default());
        assert!(buf.is_empty());
        assert!(buf.push(make_frame(0)));
        assert_eq!(buf.len(), 1);
        let f = buf.pop().expect("should succeed in test");
        assert_eq!(f.meta.frame_index, 0);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_overflow() {
        let cfg = FrameBufferConfig {
            capacity: 3,
            pre_roll: 1,
        };
        let mut buf = FrameBuffer::new(cfg);
        assert!(buf.push(make_frame(0)));
        assert!(buf.push(make_frame(1)));
        assert!(buf.push(make_frame(2)));
        assert!(!buf.push(make_frame(3))); // overflow
        assert_eq!(buf.stats().overflow_count, 1);
    }

    #[test]
    fn test_underrun() {
        let mut buf = FrameBuffer::new(FrameBufferConfig::default());
        assert!(buf.pop().is_none());
        assert_eq!(buf.stats().underrun_count, 1);
    }

    #[test]
    fn test_pre_roll() {
        let cfg = FrameBufferConfig {
            capacity: 10,
            pre_roll: 3,
        };
        let mut buf = FrameBuffer::new(cfg);
        assert!(!buf.pre_roll_ready());
        buf.push(make_frame(0));
        buf.push(make_frame(1));
        assert!(!buf.pre_roll_ready());
        buf.push(make_frame(2));
        assert!(buf.pre_roll_ready());
    }

    #[test]
    fn test_flush_resets_pre_roll() {
        let cfg = FrameBufferConfig {
            capacity: 10,
            pre_roll: 2,
        };
        let mut buf = FrameBuffer::new(cfg);
        buf.push(make_frame(0));
        buf.push(make_frame(1));
        assert!(buf.pre_roll_ready());
        buf.flush();
        assert!(!buf.pre_roll_ready());
        assert!(buf.is_empty());
    }

    #[test]
    fn test_fifo_order() {
        let mut buf = FrameBuffer::new(FrameBufferConfig {
            capacity: 10,
            pre_roll: 1,
        });
        buf.push(make_frame(10));
        buf.push(make_frame(20));
        buf.push(make_frame(30));
        assert_eq!(
            buf.pop().expect("should succeed in test").meta.frame_index,
            10
        );
        assert_eq!(
            buf.pop().expect("should succeed in test").meta.frame_index,
            20
        );
        assert_eq!(
            buf.pop().expect("should succeed in test").meta.frame_index,
            30
        );
    }

    #[test]
    fn test_peek_does_not_remove() {
        let mut buf = FrameBuffer::new(FrameBufferConfig::default());
        buf.push(make_frame(5));
        assert_eq!(
            buf.peek().expect("should succeed in test").meta.frame_index,
            5
        );
        assert_eq!(buf.len(), 1);
    }

    #[test]
    fn test_fill_ratio() {
        let cfg = FrameBufferConfig {
            capacity: 4,
            pre_roll: 1,
        };
        let mut buf = FrameBuffer::new(cfg);
        assert!((buf.fill_ratio() - 0.0).abs() < f64::EPSILON);
        buf.push(make_frame(0));
        buf.push(make_frame(1));
        assert!((buf.fill_ratio() - 0.5).abs() < f64::EPSILON);
        buf.push(make_frame(2));
        buf.push(make_frame(3));
        assert!((buf.fill_ratio() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_is_full() {
        let cfg = FrameBufferConfig {
            capacity: 2,
            pre_roll: 1,
        };
        let mut buf = FrameBuffer::new(cfg);
        assert!(!buf.is_full());
        buf.push(make_frame(0));
        buf.push(make_frame(1));
        assert!(buf.is_full());
    }

    #[test]
    fn test_stats_counters() {
        let cfg = FrameBufferConfig {
            capacity: 5,
            pre_roll: 1,
        };
        let mut buf = FrameBuffer::new(cfg);
        buf.push(make_frame(0));
        buf.push(make_frame(1));
        buf.pop();
        assert_eq!(buf.stats().total_pushed, 2);
        assert_eq!(buf.stats().total_popped, 1);
    }

    #[test]
    fn test_pixel_format_bytes_per_pixel() {
        assert_eq!(PixelFormat::Yuv422P8.bytes_per_pixel(), 2);
        assert_eq!(PixelFormat::Rgba8.bytes_per_pixel(), 4);
        assert_eq!(PixelFormat::Rgba10.bytes_per_pixel(), 5);
    }

    #[test]
    fn test_fill_ratio_zero_capacity() {
        let cfg = FrameBufferConfig {
            capacity: 0,
            pre_roll: 0,
        };
        let buf = FrameBuffer::new(cfg);
        assert!((buf.fill_ratio() - 0.0).abs() < f64::EPSILON);
    }
}
