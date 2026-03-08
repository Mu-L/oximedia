//! CPL (Composition Playlist) structural parsing helpers.
//!
//! Provides lightweight, allocation-friendly types for representing the
//! segment/sequence hierarchy of an IMF Composition Playlist (SMPTE ST 2067-3)
//! without requiring a full XML round-trip.

#![allow(dead_code)]

/// A single resource reference within a CPL sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CplResource {
    /// IETF RFC 4122 UUID identifying the track-file asset.
    pub track_file_id: String,
    /// Edit-unit offset within the referenced track file.
    pub source_duration: u64,
    /// Number of edit units from this resource to include.
    pub entry_point: u64,
    /// Intrinsic duration of the resource in edit units.
    pub intrinsic_duration: u64,
    /// Repeat count (usually 1).
    pub repeat_count: u32,
}

impl CplResource {
    /// Create a minimal [`CplResource`] pointing at `track_file_id` for
    /// `duration` edit units starting from the beginning.
    #[must_use]
    pub fn simple(track_file_id: impl Into<String>, duration: u64) -> Self {
        Self {
            track_file_id: track_file_id.into(),
            source_duration: duration,
            entry_point: 0,
            intrinsic_duration: duration,
            repeat_count: 1,
        }
    }

    /// Effective duration contributed by this resource.
    #[must_use]
    pub fn effective_duration(&self) -> u64 {
        self.source_duration * u64::from(self.repeat_count)
    }
}

/// A CPL sequence groups resources of the same type (video, audio, subtitle …).
#[derive(Debug, Clone)]
pub struct CplSequence {
    /// UUID of this sequence.
    pub id: String,
    /// UUID of the virtual track to which this sequence belongs.
    pub track_id: String,
    /// Ordered list of resource references.
    pub resources: Vec<CplResource>,
}

impl CplSequence {
    /// Create a new empty [`CplSequence`].
    #[must_use]
    pub fn new(id: impl Into<String>, track_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            track_id: track_id.into(),
            resources: Vec::new(),
        }
    }

    /// Total edit-unit duration of this sequence.
    #[must_use]
    pub fn total_duration(&self) -> u64 {
        self.resources
            .iter()
            .map(CplResource::effective_duration)
            .sum()
    }

    /// Append a resource to the sequence.
    pub fn add_resource(&mut self, resource: CplResource) {
        self.resources.push(resource);
    }
}

/// A CPL segment groups simultaneously playing sequences.
///
/// Each segment corresponds to one `<Segment>` element in the CPL XML.
#[derive(Debug, Clone)]
pub struct CplSegment {
    /// UUID of this segment.
    pub id: String,
    /// Human-readable annotation label (optional in SMPTE ST 2067-3).
    pub annotation: Option<String>,
    /// All sequences within this segment.
    pub sequences: Vec<CplSequence>,
}

impl CplSegment {
    /// Create a new [`CplSegment`] with no sequences.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            annotation: None,
            sequences: Vec::new(),
        }
    }

    /// Append a sequence to this segment.
    pub fn add_sequence(&mut self, seq: CplSequence) {
        self.sequences.push(seq);
    }

    /// Maximum edit-unit duration across all sequences in this segment.
    ///
    /// Per SMPTE ST 2067-3 all sequences in a segment must have the same
    /// duration; this method returns the maximum as a guard against malformed
    /// data.
    #[must_use]
    pub fn duration(&self) -> u64 {
        self.sequences
            .iter()
            .map(CplSequence::total_duration)
            .max()
            .unwrap_or(0)
    }
}

/// In-memory representation of a Composition Playlist.
///
/// This is a pure Rust data type suitable for building CPLs programmatically
/// or as an intermediate representation after XML parsing.
#[derive(Debug, Clone)]
pub struct CompositionPlaylist {
    /// CPL UUID.
    pub id: String,
    /// Human-readable title.
    pub content_title: String,
    /// Edit rate as a `(numerator, denominator)` fraction.
    pub edit_rate: (u32, u32),
    /// Ordered list of segments.
    segments: Vec<CplSegment>,
}

impl CompositionPlaylist {
    /// Create a new empty [`CompositionPlaylist`].
    ///
    /// # Arguments
    /// * `id`            – CPL UUID string.
    /// * `content_title` – Human-readable title.
    /// * `edit_rate`     – `(numerator, denominator)` e.g. `(24, 1)`.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        content_title: impl Into<String>,
        edit_rate: (u32, u32),
    ) -> Self {
        Self {
            id: id.into(),
            content_title: content_title.into(),
            edit_rate,
            segments: Vec::new(),
        }
    }

    /// Append a segment to the composition.
    pub fn add_segment(&mut self, segment: CplSegment) {
        self.segments.push(segment);
    }

    /// Ordered slice of segments in this composition.
    #[must_use]
    pub fn segments(&self) -> &[CplSegment] {
        &self.segments
    }

    /// Total edit-unit duration (sum of all segment durations).
    #[must_use]
    pub fn total_duration(&self) -> u64 {
        self.segments.iter().map(CplSegment::duration).sum()
    }

    /// Total duration in seconds.
    #[must_use]
    pub fn total_duration_secs(&self) -> f64 {
        let (num, den) = self.edit_rate;
        if num == 0 {
            return 0.0;
        }
        self.total_duration() as f64 * den as f64 / num as f64
    }

    /// Number of segments in the composition.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Returns `true` when the composition has no segments.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_resource(dur: u64) -> CplResource {
        CplResource::simple("urn:uuid:test-tf-id", dur)
    }

    fn make_sequence(dur: u64) -> CplSequence {
        let mut seq = CplSequence::new("seq-1", "track-1");
        seq.add_resource(make_resource(dur));
        seq
    }

    fn make_segment(dur: u64) -> CplSegment {
        let mut seg = CplSegment::new("seg-1");
        seg.add_sequence(make_sequence(dur));
        seg
    }

    // ── CplResource ───────────────────────────────────────────────────────

    #[test]
    fn test_resource_simple_construction() {
        let r = make_resource(100);
        assert_eq!(r.intrinsic_duration, 100);
        assert_eq!(r.entry_point, 0);
        assert_eq!(r.repeat_count, 1);
    }

    #[test]
    fn test_resource_effective_duration_single() {
        let r = make_resource(50);
        assert_eq!(r.effective_duration(), 50);
    }

    #[test]
    fn test_resource_effective_duration_repeat() {
        let mut r = make_resource(50);
        r.repeat_count = 3;
        assert_eq!(r.effective_duration(), 150);
    }

    // ── CplSequence ───────────────────────────────────────────────────────

    #[test]
    fn test_sequence_empty_duration() {
        let seq = CplSequence::new("id", "track");
        assert_eq!(seq.total_duration(), 0);
    }

    #[test]
    fn test_sequence_single_resource() {
        let seq = make_sequence(240);
        assert_eq!(seq.total_duration(), 240);
    }

    #[test]
    fn test_sequence_multiple_resources() {
        let mut seq = CplSequence::new("s", "t");
        seq.add_resource(make_resource(100));
        seq.add_resource(make_resource(200));
        assert_eq!(seq.total_duration(), 300);
    }

    // ── CplSegment ────────────────────────────────────────────────────────

    #[test]
    fn test_segment_empty_duration() {
        let seg = CplSegment::new("seg");
        assert_eq!(seg.duration(), 0);
    }

    #[test]
    fn test_segment_duration() {
        let seg = make_segment(480);
        assert_eq!(seg.duration(), 480);
    }

    #[test]
    fn test_segment_annotation_optional() {
        let mut seg = CplSegment::new("seg");
        assert!(seg.annotation.is_none());
        seg.annotation = Some("Act 1".to_string());
        assert_eq!(seg.annotation.as_deref(), Some("Act 1"));
    }

    // ── CompositionPlaylist ───────────────────────────────────────────────

    #[test]
    fn test_cpl_empty() {
        let cpl = CompositionPlaylist::new("cpl-id", "My Film", (24, 1));
        assert!(cpl.is_empty());
        assert_eq!(cpl.total_duration(), 0);
        assert_eq!(cpl.segment_count(), 0);
    }

    #[test]
    fn test_cpl_add_segment() {
        let mut cpl = CompositionPlaylist::new("cpl-id", "My Film", (24, 1));
        cpl.add_segment(make_segment(2400)); // 100 s at 24 fps
        assert_eq!(cpl.segment_count(), 1);
        assert_eq!(cpl.total_duration(), 2400);
    }

    #[test]
    fn test_cpl_total_duration_secs() {
        let mut cpl = CompositionPlaylist::new("id", "Title", (24, 1));
        cpl.add_segment(make_segment(2400));
        let secs = cpl.total_duration_secs();
        assert!((secs - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_cpl_segments_slice() {
        let mut cpl = CompositionPlaylist::new("id", "Title", (24, 1));
        cpl.add_segment(make_segment(100));
        cpl.add_segment(make_segment(200));
        assert_eq!(cpl.segments().len(), 2);
    }

    #[test]
    fn test_cpl_zero_edit_rate_denominator() {
        // Edge case: zero numerator should not panic.
        let cpl = CompositionPlaylist::new("id", "Title", (0, 1));
        assert_eq!(cpl.total_duration_secs(), 0.0);
    }

    #[test]
    fn test_cpl_content_title() {
        let cpl = CompositionPlaylist::new("id", "Feature Film 2025", (25, 1));
        assert_eq!(cpl.content_title, "Feature Film 2025");
    }

    #[test]
    fn test_cpl_edit_rate_stored() {
        let cpl = CompositionPlaylist::new("id", "Title", (30000, 1001));
        assert_eq!(cpl.edit_rate, (30000, 1001));
    }
}
