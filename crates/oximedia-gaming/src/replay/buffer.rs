//! Replay buffer for instant replay.
//!
//! Implements a fixed-capacity ring buffer that stores recent encoded frames
//! up to the configured duration. When the buffer is full, the oldest frames
//! are evicted to make room for new ones.

use crate::{GamingError, GamingResult};
use std::collections::VecDeque;
use std::time::Duration;

/// A single frame stored in the replay buffer.
#[derive(Debug, Clone)]
pub struct ReplayFrame {
    /// Encoded frame data.
    pub data: Vec<u8>,
    /// Presentation timestamp relative to buffer start.
    pub timestamp: Duration,
    /// Whether this is a keyframe.
    pub is_keyframe: bool,
    /// Frame sequence number.
    pub sequence: u64,
}

/// Replay buffer for storing recent frames in a ring-buffer arrangement.
pub struct ReplayBuffer {
    config: ReplayConfig,
    enabled: bool,
    /// Ring buffer of frames.
    frames: VecDeque<ReplayFrame>,
    /// Maximum number of frames based on duration and estimated framerate.
    max_frames: usize,
    /// Total bytes currently stored.
    total_bytes: usize,
    /// Maximum bytes allowed (derived from bitrate * duration).
    max_bytes: usize,
    /// Next sequence number.
    next_sequence: u64,
}

/// Replay buffer configuration.
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    /// Buffer duration in seconds
    pub duration: u32,
    /// Video bitrate in kbps
    pub bitrate: u32,
    /// Audio enabled
    pub audio_enabled: bool,
    /// Target framerate (used for capacity estimation)
    pub framerate: u32,
}

impl ReplayBuffer {
    /// Create a new replay buffer.
    ///
    /// # Errors
    ///
    /// Returns error if duration is outside the 5-300 second range.
    pub fn new(config: ReplayConfig) -> GamingResult<Self> {
        if config.duration < 5 || config.duration > 300 {
            return Err(GamingError::ReplayBufferError(
                "Duration must be between 5 and 300 seconds".to_string(),
            ));
        }

        let max_frames = (config.framerate as usize) * (config.duration as usize);
        // max_bytes = bitrate_kbps * 1000 / 8 * duration_s
        let max_bytes = (config.bitrate as usize) * 1000 / 8 * (config.duration as usize);

        Ok(Self {
            config,
            enabled: false,
            frames: VecDeque::with_capacity(max_frames.min(8192)),
            max_frames,
            total_bytes: 0,
            max_bytes,
            next_sequence: 0,
        })
    }

    /// Enable replay buffer.
    pub fn enable(&mut self) -> GamingResult<()> {
        self.enabled = true;
        Ok(())
    }

    /// Disable replay buffer and clear stored frames.
    pub fn disable(&mut self) {
        self.enabled = false;
        self.clear();
    }

    /// Check if replay buffer is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get buffer duration configuration.
    #[must_use]
    pub fn duration(&self) -> Duration {
        Duration::from_secs(u64::from(self.config.duration))
    }

    /// Push a new frame into the replay buffer.
    ///
    /// If the buffer is at capacity (by frame count or byte budget), the oldest
    /// frames are evicted until there is room. This method is a no-op if the
    /// buffer is not enabled.
    ///
    /// # Errors
    ///
    /// Returns error if a single frame exceeds the entire buffer byte budget.
    pub fn push_frame(
        &mut self,
        data: Vec<u8>,
        timestamp: Duration,
        is_keyframe: bool,
    ) -> GamingResult<()> {
        if !self.enabled {
            return Ok(());
        }

        let frame_size = data.len();

        if frame_size > self.max_bytes {
            return Err(GamingError::ReplayBufferError(format!(
                "Single frame ({} bytes) exceeds total buffer capacity ({} bytes)",
                frame_size, self.max_bytes
            )));
        }

        // Evict oldest frames if we exceed frame count limit
        while self.frames.len() >= self.max_frames {
            if let Some(old) = self.frames.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(old.data.len());
            }
        }

        // Evict oldest frames if we exceed byte budget
        while self.total_bytes + frame_size > self.max_bytes {
            if let Some(old) = self.frames.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(old.data.len());
            } else {
                break;
            }
        }

        let seq = self.next_sequence;
        self.next_sequence += 1;
        self.total_bytes += frame_size;

        self.frames.push_back(ReplayFrame {
            data,
            timestamp,
            is_keyframe,
            sequence: seq,
        });

        Ok(())
    }

    /// Get the number of frames currently in the buffer.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Get the total bytes stored in the buffer.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Get the actual time span covered by the buffer.
    #[must_use]
    pub fn buffered_duration(&self) -> Duration {
        if self.frames.len() < 2 {
            return Duration::ZERO;
        }

        let oldest = &self.frames[0];
        let newest = self
            .frames
            .back()
            .map(|f| f.timestamp)
            .unwrap_or(Duration::ZERO);

        newest.saturating_sub(oldest.timestamp)
    }

    /// Extract all buffered frames as a snapshot for saving/export.
    ///
    /// The buffer is left intact. The returned frames start from the nearest
    /// keyframe to ensure the replay is decodable.
    #[must_use]
    pub fn snapshot(&self) -> Vec<ReplayFrame> {
        // Find the first keyframe in the buffer
        let start_idx = self.frames.iter().position(|f| f.is_keyframe).unwrap_or(0);

        self.frames.iter().skip(start_idx).cloned().collect()
    }

    /// Extract the last `duration` seconds of replay data.
    ///
    /// Returns frames starting from the nearest keyframe at or before the
    /// requested time window.
    #[must_use]
    pub fn snapshot_last(&self, duration: Duration) -> Vec<ReplayFrame> {
        if self.frames.is_empty() {
            return Vec::new();
        }

        let newest_ts = self
            .frames
            .back()
            .map(|f| f.timestamp)
            .unwrap_or(Duration::ZERO);
        let cutoff = newest_ts.saturating_sub(duration);

        // Find frames within the time window
        let first_in_window = self
            .frames
            .iter()
            .position(|f| f.timestamp >= cutoff)
            .unwrap_or(0);

        // Walk backwards from first_in_window to find nearest keyframe
        let mut start = first_in_window;
        for i in (0..=first_in_window).rev() {
            if self.frames[i].is_keyframe {
                start = i;
                break;
            }
        }

        self.frames.iter().skip(start).cloned().collect()
    }

    /// Clear all buffered frames.
    pub fn clear(&mut self) {
        self.frames.clear();
        self.total_bytes = 0;
    }
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            duration: 30,
            bitrate: 10000,
            audio_enabled: true,
            framerate: 60,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_buffer_creation() {
        let config = ReplayConfig::default();
        let buffer = ReplayBuffer::new(config).expect("valid replay buffer");
        assert!(!buffer.is_enabled());
        assert_eq!(buffer.frame_count(), 0);
    }

    #[test]
    fn test_enable_disable() {
        let mut buffer = ReplayBuffer::new(ReplayConfig::default()).expect("valid replay buffer");
        buffer.enable().expect("enable should succeed");
        assert!(buffer.is_enabled());
        buffer.disable();
        assert!(!buffer.is_enabled());
    }

    #[test]
    fn test_invalid_duration() {
        let config = ReplayConfig {
            duration: 1,
            ..ReplayConfig::default()
        };
        let result = ReplayBuffer::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_push_frame_when_disabled_is_noop() {
        let mut buffer = ReplayBuffer::new(ReplayConfig::default()).expect("valid");
        // Not enabled
        buffer
            .push_frame(vec![0u8; 100], Duration::from_millis(0), true)
            .expect("push should be noop");
        assert_eq!(buffer.frame_count(), 0);
    }

    #[test]
    fn test_push_and_count_frames() {
        let mut buffer = ReplayBuffer::new(ReplayConfig::default()).expect("valid");
        buffer.enable().expect("enable");

        for i in 0..10 {
            buffer
                .push_frame(vec![0u8; 1000], Duration::from_millis(i * 16), i % 30 == 0)
                .expect("push should succeed");
        }

        assert_eq!(buffer.frame_count(), 10);
        assert_eq!(buffer.total_bytes(), 10 * 1000);
    }

    #[test]
    fn test_eviction_by_frame_count() {
        let config = ReplayConfig {
            duration: 5,
            bitrate: 100000, // large byte budget
            framerate: 2,    // 2fps * 5s = 10 frames max
            ..ReplayConfig::default()
        };
        let mut buffer = ReplayBuffer::new(config).expect("valid");
        buffer.enable().expect("enable");

        for i in 0..20 {
            buffer
                .push_frame(vec![0u8; 100], Duration::from_millis(i * 500), i % 5 == 0)
                .expect("push");
        }

        assert_eq!(buffer.frame_count(), 10);
        // Oldest remaining should be sequence 10
        assert_eq!(buffer.frames[0].sequence, 10);
    }

    #[test]
    fn test_eviction_by_byte_budget() {
        let config = ReplayConfig {
            duration: 5,
            bitrate: 8, // 8 kbps * 5s = 5000 bytes budget
            framerate: 1000,
            ..ReplayConfig::default()
        };
        let mut buffer = ReplayBuffer::new(config).expect("valid");
        buffer.enable().expect("enable");

        // Each frame is 2000 bytes, budget is 5000
        for i in 0..5 {
            buffer
                .push_frame(vec![0u8; 2000], Duration::from_millis(i * 100), true)
                .expect("push");
        }

        // Should have evicted down to fit within 5000 bytes
        assert!(buffer.total_bytes() <= 5000);
        assert!(buffer.frame_count() <= 2);
    }

    #[test]
    fn test_snapshot_starts_from_keyframe() {
        let mut buffer = ReplayBuffer::new(ReplayConfig::default()).expect("valid");
        buffer.enable().expect("enable");

        // Push: non-key, non-key, key, non-key
        buffer
            .push_frame(vec![1], Duration::from_millis(0), false)
            .expect("push");
        buffer
            .push_frame(vec![2], Duration::from_millis(16), false)
            .expect("push");
        buffer
            .push_frame(vec![3], Duration::from_millis(32), true)
            .expect("push");
        buffer
            .push_frame(vec![4], Duration::from_millis(48), false)
            .expect("push");

        let snap = buffer.snapshot();
        // Should start from the keyframe at index 2
        assert_eq!(snap.len(), 2);
        assert_eq!(snap[0].data, vec![3]);
        assert!(snap[0].is_keyframe);
    }

    #[test]
    fn test_snapshot_last_duration() {
        let mut buffer = ReplayBuffer::new(ReplayConfig::default()).expect("valid");
        buffer.enable().expect("enable");

        // 10 frames at 100ms intervals, keyframe every 5
        for i in 0..10u64 {
            buffer
                .push_frame(vec![i as u8], Duration::from_millis(i * 100), i % 5 == 0)
                .expect("push");
        }

        // Get last 300ms (frames at 700, 800, 900)
        let snap = buffer.snapshot_last(Duration::from_millis(300));
        // Should include from the keyframe at 500ms onwards
        assert!(snap.len() >= 3);
        assert!(snap[0].is_keyframe);
    }

    #[test]
    fn test_buffered_duration() {
        let mut buffer = ReplayBuffer::new(ReplayConfig::default()).expect("valid");
        buffer.enable().expect("enable");

        assert_eq!(buffer.buffered_duration(), Duration::ZERO);

        buffer
            .push_frame(vec![0], Duration::from_millis(100), true)
            .expect("push");
        buffer
            .push_frame(vec![0], Duration::from_millis(500), false)
            .expect("push");

        assert_eq!(buffer.buffered_duration(), Duration::from_millis(400));
    }

    #[test]
    fn test_clear() {
        let mut buffer = ReplayBuffer::new(ReplayConfig::default()).expect("valid");
        buffer.enable().expect("enable");
        buffer
            .push_frame(vec![0; 1000], Duration::ZERO, true)
            .expect("push");
        assert_eq!(buffer.frame_count(), 1);

        buffer.clear();
        assert_eq!(buffer.frame_count(), 0);
        assert_eq!(buffer.total_bytes(), 0);
    }
}
