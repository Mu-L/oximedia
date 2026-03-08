//! Fragmented MP4 (fMP4) support for DASH/HLS.
//!
//! Implements fragmented MP4 format used in adaptive streaming protocols
//! like MPEG-DASH and HLS.

#![forbid(unsafe_code)]

use oximedia_core::{OxiError, OxiResult, Rational, Timestamp};
use std::time::Duration;

use crate::{Packet, StreamInfo};

/// Configuration for fragmented MP4.
#[derive(Clone, Debug)]
pub struct FragmentedMp4Config {
    /// Fragment duration in milliseconds.
    pub fragment_duration_ms: u64,
    /// Enable separate initialization segment.
    pub separate_init_segment: bool,
    /// Enable self-initializing segments.
    pub self_initializing: bool,
    /// Fragment sequence number (increments for each fragment).
    pub sequence_number: u32,
    /// Enable single fragment per segment.
    pub single_fragment: bool,
}

impl Default for FragmentedMp4Config {
    fn default() -> Self {
        Self {
            fragment_duration_ms: 2000, // 2 seconds
            separate_init_segment: true,
            self_initializing: false,
            sequence_number: 1,
            single_fragment: true,
        }
    }
}

impl FragmentedMp4Config {
    /// Creates a new configuration with default values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fragment_duration_ms: 2000,
            separate_init_segment: true,
            self_initializing: false,
            sequence_number: 1,
            single_fragment: true,
        }
    }

    /// Sets the fragment duration.
    #[must_use]
    pub const fn with_fragment_duration(mut self, duration_ms: u64) -> Self {
        self.fragment_duration_ms = duration_ms;
        self
    }

    /// Enables separate initialization segment.
    #[must_use]
    pub const fn with_separate_init(mut self, enabled: bool) -> Self {
        self.separate_init_segment = enabled;
        self
    }

    /// Enables self-initializing segments.
    #[must_use]
    pub const fn with_self_initializing(mut self, enabled: bool) -> Self {
        self.self_initializing = enabled;
        self
    }

    /// Sets the starting sequence number.
    #[must_use]
    pub const fn with_sequence_number(mut self, number: u32) -> Self {
        self.sequence_number = number;
        self
    }
}

/// Type of MP4 fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentType {
    /// Initialization segment (ftyp + moov).
    Init,
    /// Media fragment (moof + mdat).
    Media,
}

/// An MP4 fragment.
#[derive(Debug, Clone)]
pub struct Mp4Fragment {
    /// Type of fragment.
    pub fragment_type: FragmentType,
    /// Sequence number.
    pub sequence: u32,
    /// Fragment data.
    pub data: Vec<u8>,
    /// Duration of the fragment in microseconds.
    pub duration_us: u64,
    /// Start timestamp.
    pub start_timestamp: Timestamp,
    /// Stream indices included in this fragment.
    pub stream_indices: Vec<usize>,
    /// Whether this fragment contains a keyframe.
    pub has_keyframe: bool,
}

impl Mp4Fragment {
    /// Creates a new MP4 fragment.
    #[must_use]
    pub fn new(fragment_type: FragmentType, sequence: u32) -> Self {
        Self {
            fragment_type,
            sequence,
            data: Vec::new(),
            duration_us: 0,
            start_timestamp: Timestamp::new(0, Rational::new(1, 1)),
            stream_indices: Vec::new(),
            has_keyframe: false,
        }
    }

    /// Returns the size of the fragment in bytes.
    #[must_use]
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Returns the duration as a `Duration`.
    #[must_use]
    pub const fn duration(&self) -> Duration {
        Duration::from_micros(self.duration_us)
    }

    /// Returns true if this is an initialization fragment.
    #[must_use]
    pub const fn is_init(&self) -> bool {
        matches!(self.fragment_type, FragmentType::Init)
    }

    /// Returns true if this is a media fragment.
    #[must_use]
    pub const fn is_media(&self) -> bool {
        matches!(self.fragment_type, FragmentType::Media)
    }
}

/// Builder for constructing fragmented MP4 streams.
#[derive(Debug)]
pub struct FragmentedMp4Builder {
    config: FragmentedMp4Config,
    streams: Vec<StreamInfo>,
    #[allow(dead_code)]
    current_fragment: Option<Mp4Fragment>,
    fragment_start_time: Option<i64>,
    packets_in_fragment: Vec<Packet>,
}

impl FragmentedMp4Builder {
    /// Creates a new builder with the given configuration.
    #[must_use]
    pub fn new(config: FragmentedMp4Config) -> Self {
        Self {
            config,
            streams: Vec::new(),
            current_fragment: None,
            fragment_start_time: None,
            packets_in_fragment: Vec::new(),
        }
    }

    /// Adds a stream to the builder.
    pub fn add_stream(&mut self, info: StreamInfo) -> usize {
        self.streams.push(info);
        self.streams.len() - 1
    }

    /// Returns the streams.
    #[must_use]
    pub fn streams(&self) -> &[StreamInfo] {
        &self.streams
    }

    /// Generates the initialization segment.
    ///
    /// # Errors
    ///
    /// Returns `Err` if no streams have been added.
    pub fn build_init_segment(&self) -> OxiResult<Mp4Fragment> {
        if self.streams.is_empty() {
            return Err(OxiError::InvalidData("No streams added".into()));
        }

        let mut fragment = Mp4Fragment::new(FragmentType::Init, 0);

        // In a real implementation, we would generate the actual ftyp and moov boxes here
        // For now, we just create a placeholder
        fragment.data = b"ftyp".to_vec();

        Ok(fragment)
    }

    /// Adds a packet to the current fragment.
    ///
    /// Returns `Some(fragment)` if the fragment is complete.
    ///
    /// # Errors
    ///
    /// Returns `Err` if fragment finalization fails.
    pub fn add_packet(&mut self, packet: Packet) -> OxiResult<Option<Mp4Fragment>> {
        // Initialize fragment start time if needed
        if self.fragment_start_time.is_none() {
            self.fragment_start_time = Some(packet.pts());
        }

        self.packets_in_fragment.push(packet);

        // Check if fragment is complete
        if self.should_close_fragment() {
            self.finalize_fragment()
        } else {
            Ok(None)
        }
    }

    /// Checks if the current fragment should be closed.
    fn should_close_fragment(&self) -> bool {
        if self.packets_in_fragment.is_empty() {
            return false;
        }

        // Close on keyframe after minimum duration
        if let Some(last_packet) = self.packets_in_fragment.last() {
            if let Some(start_time) = self.fragment_start_time {
                let duration_ms = (last_packet.pts() - start_time) / 1000;
                #[allow(clippy::cast_sign_loss)]
                {
                    if duration_ms as u64 >= self.config.fragment_duration_ms
                        && last_packet.is_keyframe()
                    {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Finalizes the current fragment.
    fn finalize_fragment(&mut self) -> OxiResult<Option<Mp4Fragment>> {
        if self.packets_in_fragment.is_empty() {
            return Ok(None);
        }

        let sequence = self.config.sequence_number;
        let mut fragment = Mp4Fragment::new(FragmentType::Media, sequence);

        // Calculate duration and collect stream indices
        let start_pts = self
            .fragment_start_time
            .ok_or_else(|| OxiError::InvalidData("No start time".into()))?;
        let end_pts = self
            .packets_in_fragment
            .last()
            .map_or(start_pts, super::super::packet::Packet::pts);

        #[allow(clippy::cast_sign_loss)]
        {
            fragment.duration_us = ((end_pts - start_pts) * 1000) as u64;
        }

        // Set start timestamp
        if let Some(first_packet) = self.packets_in_fragment.first() {
            fragment.start_timestamp = first_packet.timestamp;
        }

        // Collect unique stream indices
        let mut stream_indices: Vec<usize> = self
            .packets_in_fragment
            .iter()
            .map(|p| p.stream_index)
            .collect();
        stream_indices.sort_unstable();
        stream_indices.dedup();
        fragment.stream_indices = stream_indices;

        // Check for keyframe
        fragment.has_keyframe = self
            .packets_in_fragment
            .iter()
            .any(super::super::packet::Packet::is_keyframe);

        // In a real implementation, we would generate the actual moof and mdat boxes here
        fragment.data = b"moof".to_vec();

        // Clear state
        self.packets_in_fragment.clear();
        self.fragment_start_time = None;
        self.config.sequence_number += 1;

        Ok(Some(fragment))
    }

    /// Forces finalization of the current fragment.
    ///
    /// # Errors
    ///
    /// Returns `Err` if fragment finalization fails.
    pub fn flush(&mut self) -> OxiResult<Option<Mp4Fragment>> {
        self.finalize_fragment()
    }
}

/// Fragmented MP4 track information.
#[derive(Debug, Clone)]
pub struct FragmentedTrack {
    /// Track ID (1-based).
    pub track_id: u32,
    /// Stream index.
    pub stream_index: usize,
    /// Track information.
    pub stream_info: StreamInfo,
    /// Default sample duration.
    pub default_sample_duration: Option<u32>,
    /// Default sample size.
    pub default_sample_size: Option<u32>,
}

impl FragmentedTrack {
    /// Creates a new fragmented track.
    #[must_use]
    pub const fn new(track_id: u32, stream_index: usize, stream_info: StreamInfo) -> Self {
        Self {
            track_id,
            stream_index,
            stream_info,
            default_sample_duration: None,
            default_sample_size: None,
        }
    }

    /// Sets the default sample duration.
    #[must_use]
    pub const fn with_default_duration(mut self, duration: u32) -> Self {
        self.default_sample_duration = Some(duration);
        self
    }

    /// Sets the default sample size.
    #[must_use]
    pub const fn with_default_size(mut self, size: u32) -> Self {
        self.default_sample_size = Some(size);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oximedia_core::Rational;

    #[test]
    fn test_config_default() {
        let config = FragmentedMp4Config::default();
        assert_eq!(config.fragment_duration_ms, 2000);
        assert!(config.separate_init_segment);
        assert!(!config.self_initializing);
        assert_eq!(config.sequence_number, 1);
    }

    #[test]
    fn test_config_builder() {
        let config = FragmentedMp4Config::new()
            .with_fragment_duration(3000)
            .with_separate_init(false)
            .with_self_initializing(true)
            .with_sequence_number(10);

        assert_eq!(config.fragment_duration_ms, 3000);
        assert!(!config.separate_init_segment);
        assert!(config.self_initializing);
        assert_eq!(config.sequence_number, 10);
    }

    #[test]
    fn test_fragment_creation() {
        let fragment = Mp4Fragment::new(FragmentType::Init, 0);
        assert!(fragment.is_init());
        assert!(!fragment.is_media());
        assert_eq!(fragment.sequence, 0);
        assert_eq!(fragment.size(), 0);
    }

    #[test]
    fn test_builder() {
        let config = FragmentedMp4Config::default();
        let mut builder = FragmentedMp4Builder::new(config);

        // This would normally be a real StreamInfo
        let mut stream_info =
            StreamInfo::new(0, oximedia_core::CodecId::Opus, Rational::new(1, 48000));
        stream_info.codec_params = crate::stream::CodecParams::audio(48000, 2);

        let index = builder.add_stream(stream_info);
        assert_eq!(index, 0);
        assert_eq!(builder.streams().len(), 1);
    }

    #[test]
    fn test_fragmented_track() {
        let mut stream_info =
            StreamInfo::new(0, oximedia_core::CodecId::Opus, Rational::new(1, 48000));
        stream_info.codec_params = crate::stream::CodecParams::audio(48000, 2);

        let track = FragmentedTrack::new(1, 0, stream_info)
            .with_default_duration(960)
            .with_default_size(100);

        assert_eq!(track.track_id, 1);
        assert_eq!(track.stream_index, 0);
        assert_eq!(track.default_sample_duration, Some(960));
        assert_eq!(track.default_sample_size, Some(100));
    }
}
