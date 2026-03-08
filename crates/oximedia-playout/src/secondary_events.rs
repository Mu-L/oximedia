//! Secondary event triggers: ad break markers, chapter points, and data carousel.

#![allow(dead_code)]

use std::collections::VecDeque;

// ── Event types ───────────────────────────────────────────────────────────────

/// Category of a secondary event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecondaryEventKind {
    /// SCTE-35 splice insert (ad break in)
    AdBreakIn,
    /// SCTE-35 splice insert (ad break out / return to programme)
    AdBreakOut,
    /// Chapter or scene bookmark
    ChapterPoint,
    /// Data carousel packet (DVB carousel, HbbTV object, etc.)
    DataCarousel,
    /// Programme identification label (PIDs, EPG metadata)
    ProgramId,
    /// Thumbnail or poster frame trigger
    ThumbnailCapture,
    /// Custom / user-defined trigger
    Custom,
}

impl SecondaryEventKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::AdBreakIn => "ad-break-in",
            Self::AdBreakOut => "ad-break-out",
            Self::ChapterPoint => "chapter",
            Self::DataCarousel => "data-carousel",
            Self::ProgramId => "program-id",
            Self::ThumbnailCapture => "thumbnail",
            Self::Custom => "custom",
        }
    }

    pub fn is_ad_related(self) -> bool {
        matches!(self, Self::AdBreakIn | Self::AdBreakOut)
    }
}

// ── Secondary event ────────────────────────────────────────────────────────────

/// A secondary event anchored to a playout timeline position
#[derive(Debug, Clone)]
pub struct SecondaryEvent {
    /// Unique identifier
    pub id: String,
    pub kind: SecondaryEventKind,
    /// Position on the main playout timeline in milliseconds
    pub trigger_ms: u64,
    /// Optional duration (for ad breaks, chapter ranges, etc.)
    pub duration_ms: Option<u64>,
    /// Arbitrary payload (e.g. SCTE-35 cue bytes encoded as hex, HbbTV URL)
    pub payload: Option<String>,
    /// Whether this event has already been fired
    pub fired: bool,
}

impl SecondaryEvent {
    pub fn new(id: &str, kind: SecondaryEventKind, trigger_ms: u64) -> Self {
        Self {
            id: id.to_string(),
            kind,
            trigger_ms,
            duration_ms: None,
            payload: None,
            fired: false,
        }
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn with_payload(mut self, payload: &str) -> Self {
        self.payload = Some(payload.to_string());
        self
    }

    /// End position (trigger + duration) or None if no duration
    pub fn end_ms(&self) -> Option<u64> {
        self.duration_ms.map(|d| self.trigger_ms + d)
    }

    /// Mark this event as fired
    pub fn fire(&mut self) {
        self.fired = true;
    }
}

// ── Ad break model ────────────────────────────────────────────────────────────

/// An ad break defined by an in-cue and an out-cue
#[derive(Debug, Clone)]
pub struct AdBreak {
    pub id: String,
    pub break_in_ms: u64,
    pub break_out_ms: u64,
    /// Duration of the available break window in ms
    pub avail_duration_ms: u64,
    /// SCTE-35 segmentation upid (optional)
    pub upid: Option<String>,
}

impl AdBreak {
    pub fn new(id: &str, break_in_ms: u64, break_out_ms: u64) -> Self {
        let avail_duration_ms = break_out_ms.saturating_sub(break_in_ms);
        Self {
            id: id.to_string(),
            break_in_ms,
            break_out_ms,
            avail_duration_ms,
            upid: None,
        }
    }

    pub fn duration_ms(&self) -> u64 {
        self.avail_duration_ms
    }

    pub fn contains(&self, pos_ms: u64) -> bool {
        pos_ms >= self.break_in_ms && pos_ms < self.break_out_ms
    }
}

// ── Chapter point ─────────────────────────────────────────────────────────────

/// A chapter or scene marker on the timeline
#[derive(Debug, Clone)]
pub struct ChapterPoint {
    pub id: String,
    pub title: String,
    pub position_ms: u64,
    pub thumbnail_url: Option<String>,
}

impl ChapterPoint {
    pub fn new(id: &str, title: &str, position_ms: u64) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            position_ms,
            thumbnail_url: None,
        }
    }
}

// ── Data carousel ─────────────────────────────────────────────────────────────

/// Priority of a data carousel object
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CarouselPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// A single object within a data carousel
#[derive(Debug, Clone)]
pub struct CarouselObject {
    pub id: String,
    /// Module / object type tag
    pub tag: String,
    /// Raw payload bytes (simulated as `Vec<u8>`)
    pub data: Vec<u8>,
    pub priority: CarouselPriority,
    /// Repetition interval in ms
    pub repeat_interval_ms: u64,
}

impl CarouselObject {
    pub fn new(id: &str, tag: &str, data: Vec<u8>) -> Self {
        Self {
            id: id.to_string(),
            tag: tag.to_string(),
            data,
            priority: CarouselPriority::Normal,
            repeat_interval_ms: 5000,
        }
    }

    pub fn data_len(&self) -> usize {
        self.data.len()
    }
}

/// Simple data carousel scheduler — cycles through objects by priority
#[derive(Debug, Default)]
pub struct DataCarousel {
    queue: VecDeque<CarouselObject>,
    cycle_count: u64,
}

impl DataCarousel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an object to the carousel
    pub fn add(&mut self, obj: CarouselObject) {
        // Insert in priority order (highest first)
        let pos = self
            .queue
            .iter()
            .position(|o| o.priority < obj.priority)
            .unwrap_or(self.queue.len());
        self.queue.insert(pos, obj);
    }

    /// Retrieve the next object to transmit (round-robin)
    pub fn next(&mut self) -> Option<&CarouselObject> {
        if self.queue.is_empty() {
            return None;
        }
        let idx = (self.cycle_count as usize) % self.queue.len();
        self.cycle_count += 1;
        self.queue.get(idx)
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

// ── Secondary event timeline ──────────────────────────────────────────────────

/// Timeline of secondary events for a playout session
#[derive(Debug, Default)]
pub struct SecondaryEventTimeline {
    events: Vec<SecondaryEvent>,
}

impl SecondaryEventTimeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert an event, keeping the list sorted by trigger time
    pub fn insert(&mut self, event: SecondaryEvent) {
        let pos = self
            .events
            .partition_point(|e| e.trigger_ms <= event.trigger_ms);
        self.events.insert(pos, event);
    }

    /// Fire all events at or before `now_ms` that have not been fired yet.
    /// Returns the ids of fired events.
    pub fn fire_due(&mut self, now_ms: u64) -> Vec<String> {
        let mut fired = Vec::new();
        for event in &mut self.events {
            if !event.fired && event.trigger_ms <= now_ms {
                event.fire();
                fired.push(event.id.clone());
            }
        }
        fired
    }

    /// Return pending (not yet fired) events
    pub fn pending(&self) -> Vec<&SecondaryEvent> {
        self.events.iter().filter(|e| !e.fired).collect()
    }

    /// Return all events of a specific kind
    pub fn by_kind(&self, kind: SecondaryEventKind) -> Vec<&SecondaryEvent> {
        self.events.iter().filter(|e| e.kind == kind).collect()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secondary_event_kind_label() {
        assert_eq!(SecondaryEventKind::AdBreakIn.label(), "ad-break-in");
        assert_eq!(SecondaryEventKind::ChapterPoint.label(), "chapter");
    }

    #[test]
    fn test_secondary_event_kind_is_ad_related() {
        assert!(SecondaryEventKind::AdBreakIn.is_ad_related());
        assert!(!SecondaryEventKind::ChapterPoint.is_ad_related());
    }

    #[test]
    fn test_secondary_event_with_duration_end_ms() {
        let ev =
            SecondaryEvent::new("e1", SecondaryEventKind::AdBreakIn, 5000).with_duration(30_000);
        assert_eq!(ev.end_ms(), Some(35_000));
    }

    #[test]
    fn test_secondary_event_fire() {
        let mut ev = SecondaryEvent::new("e1", SecondaryEventKind::DataCarousel, 1000);
        assert!(!ev.fired);
        ev.fire();
        assert!(ev.fired);
    }

    #[test]
    fn test_ad_break_duration_and_contains() {
        let ab = AdBreak::new("b1", 10_000, 40_000);
        assert_eq!(ab.duration_ms(), 30_000);
        assert!(ab.contains(20_000));
        assert!(!ab.contains(5_000));
        assert!(!ab.contains(40_000));
    }

    #[test]
    fn test_chapter_point_creation() {
        let cp = ChapterPoint::new("c1", "Opening", 0);
        assert_eq!(cp.title, "Opening");
        assert_eq!(cp.position_ms, 0);
    }

    #[test]
    fn test_carousel_priority_ordering() {
        assert!(CarouselPriority::Critical > CarouselPriority::Low);
    }

    #[test]
    fn test_data_carousel_add_and_next() {
        let mut c = DataCarousel::new();
        c.add(CarouselObject::new("o1", "hbbtv", vec![1, 2, 3]));
        c.add(CarouselObject::new("o2", "ait", vec![4, 5]));
        assert_eq!(c.len(), 2);
        assert!(c.next().is_some());
    }

    #[test]
    fn test_data_carousel_priority_ordering() {
        let mut c = DataCarousel::new();
        c.add(CarouselObject {
            id: "low".to_string(),
            tag: "t".to_string(),
            data: vec![],
            priority: CarouselPriority::Low,
            repeat_interval_ms: 1000,
        });
        c.add(CarouselObject {
            id: "high".to_string(),
            tag: "t".to_string(),
            data: vec![],
            priority: CarouselPriority::High,
            repeat_interval_ms: 1000,
        });
        // First item in queue should be the high-priority one
        let first = c.queue.front().expect("should succeed in test");
        assert_eq!(first.priority, CarouselPriority::High);
    }

    #[test]
    fn test_secondary_event_timeline_insert_sorted() {
        let mut tl = SecondaryEventTimeline::new();
        tl.insert(SecondaryEvent::new(
            "e2",
            SecondaryEventKind::AdBreakOut,
            20_000,
        ));
        tl.insert(SecondaryEvent::new(
            "e1",
            SecondaryEventKind::AdBreakIn,
            10_000,
        ));
        assert_eq!(tl.events[0].id, "e1");
        assert_eq!(tl.events[1].id, "e2");
    }

    #[test]
    fn test_secondary_event_timeline_fire_due() {
        let mut tl = SecondaryEventTimeline::new();
        tl.insert(SecondaryEvent::new(
            "e1",
            SecondaryEventKind::AdBreakIn,
            5_000,
        ));
        tl.insert(SecondaryEvent::new(
            "e2",
            SecondaryEventKind::ChapterPoint,
            15_000,
        ));
        let fired = tl.fire_due(10_000);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0], "e1");
        assert_eq!(tl.pending().len(), 1);
    }

    #[test]
    fn test_secondary_event_timeline_by_kind() {
        let mut tl = SecondaryEventTimeline::new();
        tl.insert(SecondaryEvent::new("a1", SecondaryEventKind::AdBreakIn, 0));
        tl.insert(SecondaryEvent::new(
            "c1",
            SecondaryEventKind::ChapterPoint,
            0,
        ));
        tl.insert(SecondaryEvent::new(
            "c2",
            SecondaryEventKind::ChapterPoint,
            1000,
        ));
        assert_eq!(tl.by_kind(SecondaryEventKind::ChapterPoint).len(), 2);
    }

    #[test]
    fn test_secondary_event_payload() {
        let ev =
            SecondaryEvent::new("e1", SecondaryEventKind::Custom, 0).with_payload("0xDEADBEEF");
        assert_eq!(ev.payload.as_deref(), Some("0xDEADBEEF"));
    }

    #[test]
    fn test_carousel_object_data_len() {
        let obj = CarouselObject::new("o1", "tag", vec![0u8; 256]);
        assert_eq!(obj.data_len(), 256);
    }
}
