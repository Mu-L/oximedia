//! Segment-based encoding for HLS and DASH streaming.
//!
//! This module provides tools for planning segment boundaries,
//! tracking encoded segments, and generating HLS/DASH manifests.

/// Configuration for segment-based encoding.
#[derive(Debug, Clone)]
pub struct SegmentConfig {
    /// Target segment duration in seconds.
    pub duration_secs: f32,
    /// Keyframe interval in frames.
    pub keyframe_interval: u32,
    /// Force a keyframe at each segment boundary.
    pub force_key_at_segment: bool,
}

impl Default for SegmentConfig {
    fn default() -> Self {
        Self {
            duration_secs: 6.0,
            keyframe_interval: 60,
            force_key_at_segment: true,
        }
    }
}

impl SegmentConfig {
    /// Creates a new segment configuration.
    #[must_use]
    pub fn new(duration_secs: f32, keyframe_interval: u32, force_key_at_segment: bool) -> Self {
        Self {
            duration_secs,
            keyframe_interval,
            force_key_at_segment,
        }
    }
}

/// A boundary point between two segments.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentBoundary {
    /// Frame index where the segment starts.
    pub frame_idx: u64,
    /// Whether this frame is a keyframe.
    pub is_keyframe: bool,
    /// Timestamp in seconds.
    pub timestamp_secs: f64,
}

/// A complete segment plan for encoding.
#[derive(Debug, Clone)]
pub struct SegmentPlan {
    /// All segment boundaries.
    pub boundaries: Vec<SegmentBoundary>,
    /// Total number of frames.
    pub total_frames: u64,
    /// Total number of segments.
    pub segment_count: u32,
}

/// Plans segment boundaries for a given video.
#[derive(Debug, Clone, Default)]
pub struct SegmentPlanner;

impl SegmentPlanner {
    /// Creates a new segment planner.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Plans segment boundaries for `total_frames` frames at `fps` with the given config.
    #[must_use]
    pub fn plan(total_frames: u64, fps: f32, config: &SegmentConfig) -> SegmentPlan {
        let frames_per_segment = (config.duration_secs * fps).round() as u64;
        let frames_per_segment = frames_per_segment.max(1);

        let mut boundaries = Vec::new();
        let mut frame_idx = 0u64;
        let mut seg_count = 0u32;

        while frame_idx < total_frames {
            let is_keyframe = if config.force_key_at_segment {
                true
            } else {
                // Keyframe at regular intervals
                frame_idx % u64::from(config.keyframe_interval) == 0
            };

            let timestamp_secs = frame_idx as f64 / f64::from(fps);

            boundaries.push(SegmentBoundary {
                frame_idx,
                is_keyframe,
                timestamp_secs,
            });

            frame_idx += frames_per_segment;
            seg_count += 1;
        }

        SegmentPlan {
            boundaries,
            total_frames,
            segment_count: seg_count,
        }
    }
}

/// An encoded segment of media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedSegment {
    /// Segment index (0-based).
    pub index: u32,
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Actual bitrate in kilobits per second.
    pub bitrate_kbps: u32,
    /// Codec used for this segment.
    pub codec: String,
}

impl EncodedSegment {
    /// Creates a new encoded segment.
    #[must_use]
    pub fn new(
        index: u32,
        start_ms: u64,
        duration_ms: u64,
        size_bytes: u64,
        bitrate_kbps: u32,
        codec: impl Into<String>,
    ) -> Self {
        Self {
            index,
            start_ms,
            duration_ms,
            size_bytes,
            bitrate_kbps,
            codec: codec.into(),
        }
    }

    /// Returns the end time in milliseconds.
    #[must_use]
    pub fn end_ms(&self) -> u64 {
        self.start_ms + self.duration_ms
    }

    /// Returns the duration in seconds.
    #[must_use]
    pub fn duration_secs(&self) -> f64 {
        self.duration_ms as f64 / 1000.0
    }
}

/// Encoder that tracks encoded segments.
#[derive(Debug, Clone, Default)]
pub struct SegmentEncoder {
    /// All encoded segments.
    pub encoded_segments: Vec<EncodedSegment>,
}

impl SegmentEncoder {
    /// Creates a new segment encoder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a segment to the encoder's list.
    pub fn add_segment(&mut self, segment: EncodedSegment) {
        self.encoded_segments.push(segment);
    }

    /// Returns the total number of encoded segments.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.encoded_segments.len()
    }

    /// Returns the total encoded size in bytes.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.encoded_segments.iter().map(|s| s.size_bytes).sum()
    }

    /// Returns the average bitrate across all segments.
    #[must_use]
    pub fn average_bitrate_kbps(&self) -> Option<u32> {
        if self.encoded_segments.is_empty() {
            return None;
        }
        let sum: u64 = self
            .encoded_segments
            .iter()
            .map(|s| u64::from(s.bitrate_kbps))
            .sum();
        Some((sum / self.encoded_segments.len() as u64) as u32)
    }
}

/// Generates HLS and DASH manifests from encoded segments.
#[derive(Debug, Clone, Default)]
pub struct SegmentManifest;

impl SegmentManifest {
    /// Generates an HLS `.m3u8` manifest.
    #[must_use]
    pub fn generate_hls(segments: &[EncodedSegment], base_url: &str) -> String {
        let max_duration = segments
            .iter()
            .map(EncodedSegment::duration_secs)
            .fold(0.0_f64, f64::max);

        let mut manifest = format!(
            "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:{}\n#EXT-X-MEDIA-SEQUENCE:0\n",
            max_duration.ceil() as u64
        );

        for seg in segments {
            let duration = seg.duration_secs();
            manifest.push_str(&format!(
                "#EXTINF:{:.3},\n{}/segment_{:05}.ts\n",
                duration, base_url, seg.index
            ));
        }

        manifest.push_str("#EXT-X-ENDLIST\n");
        manifest
    }

    /// Generates a MPEG-DASH `manifest.mpd` manifest.
    #[must_use]
    pub fn generate_dash(segments: &[EncodedSegment], base_url: &str) -> String {
        let total_ms: u64 = segments.iter().map(|s| s.duration_ms).sum();
        let total_secs = total_ms as f64 / 1000.0;

        let avg_bitrate = if segments.is_empty() {
            0u32
        } else {
            let sum: u64 = segments.iter().map(|s| u64::from(s.bitrate_kbps)).sum();
            (sum / segments.len() as u64) as u32
        };

        let codec = segments.first().map_or("avc1", |s| s.codec.as_str());

        let mut mpd = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <MPD xmlns=\"urn:mpeg:dash:schema:mpd:2011\" \
             mediaPresentationDuration=\"PT{total_secs:.3}S\" \
             type=\"static\">\n  \
             <Period>\n    \
             <AdaptationSet mimeType=\"video/mp4\">\n      \
             <Representation id=\"0\" codecs=\"{codec}\" bandwidth=\"{}\">\n",
            avg_bitrate * 1000
        );

        mpd.push_str("        <SegmentList>\n");
        for seg in segments {
            mpd.push_str(&format!(
                "          <SegmentURL media=\"{}/segment_{:05}.mp4\"/>\n",
                base_url, seg.index
            ));
        }
        mpd.push_str(
            "        </SegmentList>\n      \
             </Representation>\n    \
             </AdaptationSet>\n  \
             </Period>\n\
             </MPD>\n",
        );

        mpd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_config_default() {
        let cfg = SegmentConfig::default();
        assert_eq!(cfg.duration_secs, 6.0);
        assert!(cfg.force_key_at_segment);
    }

    #[test]
    fn test_segment_config_new() {
        let cfg = SegmentConfig::new(4.0, 120, false);
        assert_eq!(cfg.duration_secs, 4.0);
        assert_eq!(cfg.keyframe_interval, 120);
        assert!(!cfg.force_key_at_segment);
    }

    #[test]
    fn test_segment_planner_basic() {
        let cfg = SegmentConfig::default(); // 6s segments
                                            // 180 frames at 30fps = 6 seconds total → 1 segment
        let plan = SegmentPlanner::plan(180, 30.0, &cfg);
        assert_eq!(plan.total_frames, 180);
        assert!(plan.segment_count >= 1);
    }

    #[test]
    fn test_segment_planner_multiple_segments() {
        let cfg = SegmentConfig::new(2.0, 30, true);
        // 60 frames at 30fps = 2s per segment → expect 1 segment start at 0
        let plan = SegmentPlanner::plan(60, 30.0, &cfg);
        assert!(!plan.boundaries.is_empty());
    }

    #[test]
    fn test_segment_planner_keyframe_at_boundary() {
        let cfg = SegmentConfig::new(2.0, 60, true);
        let plan = SegmentPlanner::plan(120, 30.0, &cfg);
        for b in &plan.boundaries {
            assert!(b.is_keyframe, "All boundaries should be keyframes");
        }
    }

    #[test]
    fn test_segment_boundary_timestamp() {
        let cfg = SegmentConfig::new(2.0, 60, true);
        let plan = SegmentPlanner::plan(120, 30.0, &cfg);
        assert!((plan.boundaries[0].timestamp_secs - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_encoded_segment_end_ms() {
        let seg = EncodedSegment::new(0, 0, 2000, 512_000, 2048, "h264");
        assert_eq!(seg.end_ms(), 2000);
        assert!((seg.duration_secs() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_segment_encoder_add_and_count() {
        let mut enc = SegmentEncoder::new();
        assert_eq!(enc.segment_count(), 0);
        enc.add_segment(EncodedSegment::new(0, 0, 2000, 1024, 4000, "h264"));
        enc.add_segment(EncodedSegment::new(1, 2000, 2000, 2048, 8000, "h264"));
        assert_eq!(enc.segment_count(), 2);
    }

    #[test]
    fn test_segment_encoder_total_bytes() {
        let mut enc = SegmentEncoder::new();
        enc.add_segment(EncodedSegment::new(0, 0, 2000, 1000, 4000, "h264"));
        enc.add_segment(EncodedSegment::new(1, 2000, 2000, 2000, 8000, "h264"));
        assert_eq!(enc.total_bytes(), 3000);
    }

    #[test]
    fn test_segment_encoder_average_bitrate() {
        let mut enc = SegmentEncoder::new();
        assert!(enc.average_bitrate_kbps().is_none());
        enc.add_segment(EncodedSegment::new(0, 0, 2000, 1000, 4000, "h264"));
        enc.add_segment(EncodedSegment::new(1, 2000, 2000, 2000, 6000, "h264"));
        assert_eq!(enc.average_bitrate_kbps(), Some(5000));
    }

    #[test]
    fn test_generate_hls_contains_extm3u() {
        let segments = vec![
            EncodedSegment::new(0, 0, 6000, 1000, 4000, "h264"),
            EncodedSegment::new(1, 6000, 6000, 1000, 4000, "h264"),
        ];
        let manifest = SegmentManifest::generate_hls(&segments, "https://cdn.example.com");
        assert!(manifest.contains("#EXTM3U"));
        assert!(manifest.contains("#EXT-X-ENDLIST"));
        assert!(manifest.contains("segment_00000.ts"));
        assert!(manifest.contains("segment_00001.ts"));
    }

    #[test]
    fn test_generate_dash_contains_mpd() {
        let segments = vec![EncodedSegment::new(0, 0, 6000, 1000, 4000, "avc1")];
        let manifest = SegmentManifest::generate_dash(&segments, "https://cdn.example.com");
        assert!(manifest.contains("<?xml"));
        assert!(manifest.contains("<MPD"));
        assert!(manifest.contains("segment_00000.mp4"));
    }

    #[test]
    fn test_generate_hls_empty() {
        let manifest = SegmentManifest::generate_hls(&[], "https://cdn.example.com");
        assert!(manifest.contains("#EXTM3U"));
        assert!(manifest.contains("#EXT-X-ENDLIST"));
    }

    #[test]
    fn test_generate_dash_empty() {
        let manifest = SegmentManifest::generate_dash(&[], "https://cdn.example.com");
        assert!(manifest.contains("<?xml"));
    }
}
