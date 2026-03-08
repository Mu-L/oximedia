//! Seeking infrastructure for demuxers.
//!
//! This module provides types and utilities for seeking in media containers,
//! including seek flags and seek target specifications.

use bitflags::bitflags;

bitflags! {
    /// Flags controlling seek behavior.
    ///
    /// These flags allow fine-grained control over how a seek operation
    /// is performed and what position is targeted.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
    pub struct SeekFlags: u32 {
        /// Seek backward (to position <= target).
        ///
        /// Without this flag, seeks go forward (to position >= target).
        /// This is useful for finding the keyframe before a target position.
        const BACKWARD = 0x0001;

        /// Allow seeking to any frame, not just keyframes.
        ///
        /// By default, seeks target keyframes for clean decoding.
        /// Setting this flag allows seeking to any position, which may
        /// require decoding from the previous keyframe.
        const ANY = 0x0002;

        /// Seek to the nearest keyframe.
        ///
        /// This is the default behavior and ensures the seek position
        /// can be decoded immediately without reference frames.
        const KEYFRAME = 0x0004;

        /// Seek by bytes rather than time.
        ///
        /// When set, the seek target is interpreted as a byte offset
        /// in the file rather than a timestamp.
        const BYTE = 0x0008;

        /// Seek to exact position (frame-accurate).
        ///
        /// Attempts to seek to the exact target timestamp, which may
        /// require additional parsing and decoding.
        const FRAME_ACCURATE = 0x0010;
    }
}

/// Target for a seek operation.
///
/// Specifies where to seek and which stream to use as reference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeekTarget {
    /// Target timestamp in seconds, or byte offset if `SeekFlags::BYTE` is set.
    pub position: f64,

    /// Stream index to use for seeking, or `None` for the default stream.
    ///
    /// The default stream is typically the first video stream, or the
    /// first audio stream if there are no video streams.
    pub stream_index: Option<usize>,

    /// Seek flags controlling behavior.
    pub flags: SeekFlags,
}

impl SeekTarget {
    /// Creates a new seek target to a timestamp in seconds.
    ///
    /// # Arguments
    ///
    /// * `position` - Target timestamp in seconds
    #[must_use]
    pub const fn time(position: f64) -> Self {
        Self {
            position,
            stream_index: None,
            flags: SeekFlags::KEYFRAME,
        }
    }

    /// Creates a new seek target to a byte offset.
    ///
    /// # Arguments
    ///
    /// * `offset` - Target byte offset in the file
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn byte(offset: u64) -> Self {
        Self {
            position: offset as f64,
            stream_index: None,
            flags: SeekFlags::BYTE,
        }
    }

    /// Sets the stream index for this seek target.
    #[must_use]
    pub const fn with_stream(mut self, stream_index: usize) -> Self {
        self.stream_index = Some(stream_index);
        self
    }

    /// Sets the seek flags for this seek target.
    #[must_use]
    pub const fn with_flags(mut self, flags: SeekFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Adds additional flags to this seek target.
    #[must_use]
    pub const fn add_flags(mut self, flags: SeekFlags) -> Self {
        self.flags = SeekFlags::from_bits_truncate(self.flags.bits() | flags.bits());
        self
    }

    /// Returns true if this is a backward seek.
    #[must_use]
    pub const fn is_backward(&self) -> bool {
        self.flags.contains(SeekFlags::BACKWARD)
    }

    /// Returns true if this allows seeking to any frame.
    #[must_use]
    pub const fn is_any(&self) -> bool {
        self.flags.contains(SeekFlags::ANY)
    }

    /// Returns true if this seeks to a keyframe.
    #[must_use]
    pub const fn is_keyframe(&self) -> bool {
        self.flags.contains(SeekFlags::KEYFRAME)
    }

    /// Returns true if this is a byte-based seek.
    #[must_use]
    pub const fn is_byte(&self) -> bool {
        self.flags.contains(SeekFlags::BYTE)
    }

    /// Returns true if this is a frame-accurate seek.
    #[must_use]
    pub const fn is_frame_accurate(&self) -> bool {
        self.flags.contains(SeekFlags::FRAME_ACCURATE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seek_flags() {
        let flags = SeekFlags::BACKWARD | SeekFlags::KEYFRAME;
        assert!(flags.contains(SeekFlags::BACKWARD));
        assert!(flags.contains(SeekFlags::KEYFRAME));
        assert!(!flags.contains(SeekFlags::ANY));
    }

    #[test]
    fn test_seek_target_time() {
        let target = SeekTarget::time(10.5);
        assert_eq!(target.position, 10.5);
        assert!(target.is_keyframe());
        assert!(!target.is_byte());
        assert_eq!(target.stream_index, None);
    }

    #[test]
    fn test_seek_target_byte() {
        let target = SeekTarget::byte(1024);
        assert_eq!(target.position, 1024.0);
        assert!(target.is_byte());
        assert_eq!(target.stream_index, None);
    }

    #[test]
    fn test_seek_target_with_stream() {
        let target = SeekTarget::time(5.0).with_stream(1);
        assert_eq!(target.stream_index, Some(1));
        assert_eq!(target.position, 5.0);
    }

    #[test]
    fn test_seek_target_with_flags() {
        let target = SeekTarget::time(3.0)
            .with_flags(SeekFlags::BACKWARD)
            .add_flags(SeekFlags::ANY);

        assert!(target.is_backward());
        assert!(target.is_any());
    }

    #[test]
    fn test_seek_target_predicates() {
        let target =
            SeekTarget::time(1.0).add_flags(SeekFlags::BACKWARD | SeekFlags::FRAME_ACCURATE);

        assert!(target.is_backward());
        assert!(!target.is_any());
        assert!(target.is_keyframe());
        assert!(!target.is_byte());
        assert!(target.is_frame_accurate());
    }
}
