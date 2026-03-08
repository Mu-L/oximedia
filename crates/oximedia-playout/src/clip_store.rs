//! Clip store for playout
//!
//! Manages a catalogue of `PlayoutClip` objects that can be searched by ID or
//! name and queried for aggregate duration.

#![allow(dead_code)]

/// A single clip available for playout
#[derive(Debug, Clone)]
pub struct PlayoutClip {
    /// Unique identifier
    pub id: u64,
    /// Human-readable name
    pub name: String,
    /// Absolute path to the media file
    pub path: String,
    /// Total length of the media in frames
    pub duration_frames: u64,
    /// In-point frame (first frame to use)
    pub in_point: u64,
    /// Out-point frame (last frame to use, inclusive)
    pub out_point: u64,
    /// Whether the clip has an audio track
    pub has_audio: bool,
    /// Whether the clip has a video track
    pub has_video: bool,
}

impl PlayoutClip {
    /// Create a new clip spanning the full duration (in=0, out=frames-1)
    ///
    /// `frames` must be at least 1.  If `frames` is 0, `out_point` is also
    /// set to 0 and `is_valid()` will return `false`.
    pub fn new(id: u64, name: &str, path: &str, frames: u64) -> Self {
        let out_point = if frames > 0 { frames - 1 } else { 0 };
        Self {
            id,
            name: name.to_string(),
            path: path.to_string(),
            duration_frames: frames,
            in_point: 0,
            out_point,
            has_audio: true,
            has_video: true,
        }
    }

    /// Effective duration: `out_point - in_point + 1` frames
    pub fn duration_frames(&self) -> u64 {
        if self.out_point >= self.in_point {
            self.out_point - self.in_point + 1
        } else {
            0
        }
    }

    /// A clip is valid when it has a non-empty path, non-zero duration, a valid
    /// in/out range, and carries at least one of audio or video.
    pub fn is_valid(&self) -> bool {
        !self.path.is_empty()
            && self.duration_frames > 0
            && self.out_point >= self.in_point
            && (self.has_audio || self.has_video)
    }
}

/// Catalogue of clips available for playout
#[derive(Debug, Default)]
pub struct ClipStore {
    clips: Vec<PlayoutClip>,
}

impl ClipStore {
    /// Create a new empty clip store
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a clip to the store
    pub fn add(&mut self, clip: PlayoutClip) {
        self.clips.push(clip);
    }

    /// Remove a clip by id; returns `true` if a clip was removed
    pub fn remove(&mut self, id: u64) -> bool {
        if let Some(pos) = self.clips.iter().position(|c| c.id == id) {
            self.clips.remove(pos);
            true
        } else {
            false
        }
    }

    /// Find a clip by id
    pub fn find(&self, id: u64) -> Option<&PlayoutClip> {
        self.clips.iter().find(|c| c.id == id)
    }

    /// Find the first clip whose name matches exactly
    pub fn find_by_name(&self, name: &str) -> Option<&PlayoutClip> {
        self.clips.iter().find(|c| c.name == name)
    }

    /// Sum of `duration_frames()` across all clips in the store
    pub fn total_duration_frames(&self) -> u64 {
        self.clips.iter().map(PlayoutClip::duration_frames).sum()
    }

    /// Number of clips in the store
    pub fn count(&self) -> usize {
        self.clips.len()
    }

    /// Iterate over all clips
    pub fn iter(&self) -> impl Iterator<Item = &PlayoutClip> {
        self.clips.iter()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Unit tests
// ──────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn make_clip(id: u64, frames: u64) -> PlayoutClip {
        PlayoutClip::new(id, &format!("clip_{id}"), "/media/clip.mxf", frames)
    }

    #[test]
    fn test_clip_new_full_range() {
        let c = make_clip(1, 100);
        assert_eq!(c.in_point, 0);
        assert_eq!(c.out_point, 99);
    }

    #[test]
    fn test_clip_duration_frames() {
        let c = make_clip(1, 100);
        assert_eq!(c.duration_frames(), 100);
    }

    #[test]
    fn test_clip_duration_custom_range() {
        let mut c = make_clip(1, 100);
        c.in_point = 10;
        c.out_point = 49;
        assert_eq!(c.duration_frames(), 40);
    }

    #[test]
    fn test_clip_is_valid() {
        let c = make_clip(1, 50);
        assert!(c.is_valid());
    }

    #[test]
    fn test_clip_zero_frames_invalid() {
        let c = make_clip(1, 0);
        assert!(!c.is_valid());
    }

    #[test]
    fn test_clip_no_tracks_invalid() {
        let mut c = make_clip(1, 50);
        c.has_audio = false;
        c.has_video = false;
        assert!(!c.is_valid());
    }

    #[test]
    fn test_store_add_and_count() {
        let mut store = ClipStore::new();
        store.add(make_clip(1, 100));
        store.add(make_clip(2, 200));
        assert_eq!(store.count(), 2);
    }

    #[test]
    fn test_store_remove_existing() {
        let mut store = ClipStore::new();
        store.add(make_clip(1, 100));
        assert!(store.remove(1));
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_store_remove_nonexistent_returns_false() {
        let mut store = ClipStore::new();
        assert!(!store.remove(99));
    }

    #[test]
    fn test_store_find_by_id() {
        let mut store = ClipStore::new();
        store.add(make_clip(7, 300));
        assert!(store.find(7).is_some());
        assert!(store.find(8).is_none());
    }

    #[test]
    fn test_store_find_by_name() {
        let mut store = ClipStore::new();
        store.add(make_clip(3, 100));
        assert!(store.find_by_name("clip_3").is_some());
        assert!(store.find_by_name("missing").is_none());
    }

    #[test]
    fn test_store_total_duration_frames() {
        let mut store = ClipStore::new();
        store.add(make_clip(1, 100));
        store.add(make_clip(2, 200));
        store.add(make_clip(3, 50));
        assert_eq!(store.total_duration_frames(), 350);
    }
}
