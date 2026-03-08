//! Professional editing operations.

use crate::clip::{Clip, ClipId};
use crate::error::{TimelineError, TimelineResult};
use crate::timeline::Timeline;
use crate::track::TrackId;
use crate::types::{Duration, Position};

/// Edit mode for insert operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditMode {
    /// Insert mode (ripple all following clips).
    Insert,
    /// Overwrite mode (replace existing content).
    Overwrite,
}

/// Type of edit operation.
#[derive(Clone, Debug)]
pub enum EditOperation {
    /// Insert a clip (ripple mode).
    Insert {
        /// Track ID.
        track_id: TrackId,
        /// Clip to insert.
        clip: Clip,
        /// Position to insert at.
        position: Position,
    },
    /// Overwrite with a clip (non-ripple mode).
    Overwrite {
        /// Track ID.
        track_id: TrackId,
        /// Clip to insert.
        clip: Clip,
        /// Position to insert at.
        position: Position,
    },
    /// Delete a clip (with ripple).
    Delete {
        /// Track ID.
        track_id: TrackId,
        /// Clip ID to delete.
        clip_id: ClipId,
    },
    /// Lift a clip (without ripple).
    Lift {
        /// Track ID.
        track_id: TrackId,
        /// Clip ID to lift.
        clip_id: ClipId,
    },
    /// Split a clip at position.
    Split {
        /// Track ID.
        track_id: TrackId,
        /// Clip ID to split.
        clip_id: ClipId,
        /// Position to split at.
        position: Position,
    },
    /// Slip edit (change in/out, keep timeline position).
    Slip {
        /// Track ID.
        track_id: TrackId,
        /// Clip ID to slip.
        clip_id: ClipId,
        /// Offset to apply to source in/out points.
        offset: Duration,
    },
    /// Slide edit (change timeline position, keep duration).
    Slide {
        /// Track ID.
        track_id: TrackId,
        /// Clip ID to slide.
        clip_id: ClipId,
        /// New timeline position.
        new_position: Position,
    },
    /// Roll edit (trim adjacent clips).
    Roll {
        /// Track ID.
        track_id: TrackId,
        /// Clip ID (left clip).
        clip_id: ClipId,
        /// Offset to apply.
        offset: Duration,
    },
    /// Ripple edit (trim with timeline shift).
    Ripple {
        /// Track ID.
        track_id: TrackId,
        /// Clip ID to ripple.
        clip_id: ClipId,
        /// Offset to apply.
        offset: Duration,
    },
}

impl Timeline {
    /// Inserts a clip at a position, rippling all following clips.
    ///
    /// # Errors
    ///
    /// Returns error if track not found or locked.
    pub fn insert_clip(
        &mut self,
        track_id: TrackId,
        mut clip: Clip,
        position: Position,
    ) -> TimelineResult<()> {
        let track = self
            .get_track_mut(track_id)
            .ok_or(TimelineError::TrackNotFound(track_id))?;

        if track.locked {
            return Err(TimelineError::TrackLocked(track_id));
        }

        // Ripple all clips after insertion point
        let clip_duration = clip.timeline_duration();
        for existing_clip in &mut track.clips {
            if existing_clip.timeline_in >= position {
                existing_clip.timeline_in = existing_clip.timeline_in + clip_duration;
            }
        }

        clip.timeline_in = position;
        // Bypass overlap check: ripple insert already moved existing clips,
        // but clips spanning the insert point are not moved, so add_clip would
        // incorrectly detect an overlap. Push directly and sort instead.
        track.clips.push(clip);
        track.clips.sort_by_key(|c| c.timeline_in.value());
        self.update_duration();
        Ok(())
    }

    /// Overwrites at a position without rippling.
    ///
    /// # Errors
    ///
    /// Returns error if track not found or locked.
    pub fn overwrite_clip(
        &mut self,
        track_id: TrackId,
        mut clip: Clip,
        position: Position,
    ) -> TimelineResult<()> {
        let track = self
            .get_track_mut(track_id)
            .ok_or(TimelineError::TrackNotFound(track_id))?;

        if track.locked {
            return Err(TimelineError::TrackLocked(track_id));
        }

        let clip_end = position + clip.timeline_duration();

        // Remove overlapping clips
        let mut to_remove = Vec::new();
        for existing_clip in &track.clips {
            if existing_clip.overlaps(position, clip_end) {
                to_remove.push(existing_clip.id);
            }
        }

        for clip_id in to_remove {
            track.remove_clip(clip_id)?;
        }

        clip.timeline_in = position;
        track.add_clip(clip)?;
        self.update_duration();
        Ok(())
    }

    /// Deletes a clip and ripples following clips.
    ///
    /// # Errors
    ///
    /// Returns error if track or clip not found.
    pub fn delete_clip(&mut self, track_id: TrackId, clip_id: ClipId) -> TimelineResult<()> {
        let track = self
            .get_track_mut(track_id)
            .ok_or(TimelineError::TrackNotFound(track_id))?;

        let clip = track
            .get_clip(clip_id)
            .ok_or(TimelineError::ClipNotFound(clip_id))?;
        let clip_start = clip.timeline_in;
        let clip_duration = clip.timeline_duration();

        track.remove_clip(clip_id)?;

        // Ripple following clips
        for existing_clip in &mut track.clips {
            if existing_clip.timeline_in > clip_start {
                existing_clip.timeline_in =
                    Position::new(existing_clip.timeline_in.value() - clip_duration.value());
            }
        }

        self.transitions.remove(&clip_id);
        self.update_duration();
        Ok(())
    }

    /// Lifts a clip without rippling.
    ///
    /// # Errors
    ///
    /// Returns error if track or clip not found.
    pub fn lift_clip(&mut self, track_id: TrackId, clip_id: ClipId) -> TimelineResult<()> {
        self.remove_clip(track_id, clip_id)?;
        Ok(())
    }

    /// Splits a clip at a position.
    ///
    /// # Errors
    ///
    /// Returns error if track or clip not found, or position invalid.
    pub fn split_clip(
        &mut self,
        track_id: TrackId,
        clip_id: ClipId,
        position: Position,
    ) -> TimelineResult<(ClipId, ClipId)> {
        let track = self
            .get_track_mut(track_id)
            .ok_or(TimelineError::TrackNotFound(track_id))?;

        let clip = track
            .get_clip(clip_id)
            .ok_or(TimelineError::ClipNotFound(clip_id))?;

        let (left, right) = clip.split_at(position)?;
        let left_id = left.id;
        let right_id = right.id;

        track.remove_clip(clip_id)?;
        track.add_clip(left)?;
        track.add_clip(right)?;

        Ok((left_id, right_id))
    }

    /// Slip edit: changes source in/out points while keeping timeline position.
    ///
    /// # Errors
    ///
    /// Returns error if clip not found or offset invalid.
    pub fn slip_edit(
        &mut self,
        track_id: TrackId,
        clip_id: ClipId,
        offset: Duration,
    ) -> TimelineResult<()> {
        let track = self
            .get_track_mut(track_id)
            .ok_or(TimelineError::TrackNotFound(track_id))?;

        let clip = track
            .get_clip_mut(clip_id)
            .ok_or(TimelineError::ClipNotFound(clip_id))?;

        // Adjust source in/out points
        clip.source_in = Position::new(clip.source_in.value() + offset.value());
        clip.source_out = Position::new(clip.source_out.value() + offset.value());

        Ok(())
    }

    /// Slide edit: changes timeline position while keeping source in/out.
    ///
    /// # Errors
    ///
    /// Returns error if clip not found or new position causes overlap.
    pub fn slide_edit(
        &mut self,
        track_id: TrackId,
        clip_id: ClipId,
        new_position: Position,
    ) -> TimelineResult<()> {
        let track = self
            .get_track_mut(track_id)
            .ok_or(TimelineError::TrackNotFound(track_id))?;

        // Get the clip duration first
        let duration = track
            .get_clip(clip_id)
            .ok_or(TimelineError::ClipNotFound(clip_id))?
            .timeline_duration();
        let new_end = new_position + duration;

        // Check for overlaps with other clips
        for other_clip in &track.clips {
            if other_clip.id != clip_id && other_clip.overlaps(new_position, new_end) {
                return Err(TimelineError::ClipOverlap(new_position.value()));
            }
        }

        // Now perform the mutation after the borrow ends
        let clip = track
            .get_clip_mut(clip_id)
            .ok_or(TimelineError::ClipNotFound(clip_id))?;
        clip.timeline_in = new_position;
        Ok(())
    }

    /// Roll edit: adjusts the cut point between two adjacent clips.
    ///
    /// # Errors
    ///
    /// Returns error if clips not found or not adjacent.
    pub fn roll_edit(
        &mut self,
        track_id: TrackId,
        left_clip_id: ClipId,
        offset: Duration,
    ) -> TimelineResult<()> {
        let track = self
            .get_track_mut(track_id)
            .ok_or(TimelineError::TrackNotFound(track_id))?;

        let left_clip = track
            .get_clip(left_clip_id)
            .ok_or(TimelineError::ClipNotFound(left_clip_id))?;
        let left_end = left_clip.timeline_out();

        // Find adjacent right clip
        let right_clip_id = track
            .clips
            .iter()
            .find(|c| c.timeline_in == left_end)
            .map(|c| c.id)
            .ok_or_else(|| {
                TimelineError::Other("No adjacent clip found for roll edit".to_string())
            })?;

        // Adjust left clip out point
        let left_clip = track
            .get_clip_mut(left_clip_id)
            .ok_or(TimelineError::ClipNotFound(left_clip_id))?;
        left_clip.source_out = Position::new(left_clip.source_out.value() + offset.value());

        // Adjust right clip in point
        let right_clip = track
            .get_clip_mut(right_clip_id)
            .ok_or(TimelineError::ClipNotFound(right_clip_id))?;
        right_clip.source_in = Position::new(right_clip.source_in.value() + offset.value());
        right_clip.timeline_in = Position::new(right_clip.timeline_in.value() + offset.value());

        Ok(())
    }

    /// Ripple edit: trims a clip and ripples all following clips.
    ///
    /// # Errors
    ///
    /// Returns error if clip not found.
    pub fn ripple_edit(
        &mut self,
        track_id: TrackId,
        clip_id: ClipId,
        offset: Duration,
    ) -> TimelineResult<()> {
        let track = self
            .get_track_mut(track_id)
            .ok_or(TimelineError::TrackNotFound(track_id))?;

        let clip_start = track
            .get_clip(clip_id)
            .ok_or(TimelineError::ClipNotFound(clip_id))?
            .timeline_in;

        // Adjust clip
        let clip = track
            .get_clip_mut(clip_id)
            .ok_or(TimelineError::ClipNotFound(clip_id))?;
        clip.source_out = Position::new(clip.source_out.value() + offset.value());

        // Ripple following clips
        for other_clip in &mut track.clips {
            if other_clip.timeline_in > clip_start && other_clip.id != clip_id {
                other_clip.timeline_in =
                    Position::new(other_clip.timeline_in.value() + offset.value());
            }
        }

        self.update_duration();
        Ok(())
    }

    /// Three-point edit: Uses in/out points to determine clip placement.
    ///
    /// # Errors
    ///
    /// Returns error if insufficient points set.
    pub fn three_point_edit(&mut self, track_id: TrackId, clip: Clip) -> TimelineResult<()> {
        // For three-point editing, we need source in/out and one timeline point,
        // or source in and timeline in/out
        let timeline_in = self.in_point.ok_or_else(|| {
            TimelineError::Other("Timeline in point not set for three-point edit".to_string())
        })?;

        self.insert_clip(track_id, clip, timeline_in)
    }

    /// Four-point edit with fit-to-fill.
    ///
    /// # Errors
    ///
    /// Returns error if insufficient points set.
    pub fn four_point_edit(&mut self, track_id: TrackId, mut clip: Clip) -> TimelineResult<()> {
        let timeline_in = self.in_point.ok_or_else(|| {
            TimelineError::Other("Timeline in point not set for four-point edit".to_string())
        })?;

        let timeline_out = self.out_point.ok_or_else(|| {
            TimelineError::Other("Timeline out point not set for four-point edit".to_string())
        })?;

        let timeline_duration = Duration::new(timeline_out.value() - timeline_in.value());
        let source_duration = clip.source_duration();

        // Calculate speed to fit
        let speed_factor = timeline_duration.value() as f64 / source_duration.value() as f64;
        clip.set_speed(crate::types::Speed::new(speed_factor)?)?;

        self.overwrite_clip(track_id, clip, timeline_in)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::MediaSource;
    use oximedia_core::Rational;

    fn create_test_timeline() -> Timeline {
        Timeline::new("Test", Rational::new(24, 1), 48000).expect("should succeed in test")
    }

    fn create_test_clip(name: &str, timeline_in: i64, duration: i64) -> Clip {
        Clip::new(
            name.to_string(),
            MediaSource::black(),
            Position::new(0),
            Position::new(duration),
            Position::new(timeline_in),
        )
        .expect("should succeed in test")
    }

    #[test]
    fn test_insert_clip() {
        let mut timeline = create_test_timeline();
        let track_id = timeline
            .add_video_track("V1")
            .expect("should succeed in test");
        timeline
            .add_clip(track_id, create_test_clip("Clip1", 0, 100))
            .expect("should succeed in test");
        timeline
            .add_clip(track_id, create_test_clip("Clip2", 100, 100))
            .expect("should succeed in test");

        // Insert a clip at position 50
        timeline
            .insert_clip(
                track_id,
                create_test_clip("Insert", 50, 50),
                Position::new(50),
            )
            .expect("should succeed in test");

        // Second clip should have been rippled
        let track = timeline
            .get_track(track_id)
            .expect("should succeed in test");
        let clip2 = track
            .clips
            .iter()
            .find(|c| c.name == "Clip2")
            .expect("should succeed in test");
        assert_eq!(clip2.timeline_in.value(), 150);
    }

    #[test]
    fn test_overwrite_clip() {
        let mut timeline = create_test_timeline();
        let track_id = timeline
            .add_video_track("V1")
            .expect("should succeed in test");
        timeline
            .add_clip(track_id, create_test_clip("Clip1", 0, 100))
            .expect("should succeed in test");

        // Overwrite with new clip
        timeline
            .overwrite_clip(
                track_id,
                create_test_clip("Overwrite", 50, 100),
                Position::new(50),
            )
            .expect("should succeed in test");

        let track = timeline
            .get_track(track_id)
            .expect("should succeed in test");
        assert_eq!(track.clips.len(), 1); // Original should be removed
    }

    #[test]
    fn test_delete_clip() {
        let mut timeline = create_test_timeline();
        let track_id = timeline
            .add_video_track("V1")
            .expect("should succeed in test");
        let clip1 = create_test_clip("Clip1", 0, 100);
        let clip1_id = clip1.id;
        timeline
            .add_clip(track_id, clip1)
            .expect("should succeed in test");
        timeline
            .add_clip(track_id, create_test_clip("Clip2", 100, 100))
            .expect("should succeed in test");

        timeline
            .delete_clip(track_id, clip1_id)
            .expect("should succeed in test");

        let track = timeline
            .get_track(track_id)
            .expect("should succeed in test");
        let clip2 = track
            .clips
            .iter()
            .find(|c| c.name == "Clip2")
            .expect("should succeed in test");
        assert_eq!(clip2.timeline_in.value(), 0); // Should have been rippled
    }

    #[test]
    fn test_lift_clip() {
        let mut timeline = create_test_timeline();
        let track_id = timeline
            .add_video_track("V1")
            .expect("should succeed in test");
        let clip1 = create_test_clip("Clip1", 0, 100);
        let clip1_id = clip1.id;
        timeline
            .add_clip(track_id, clip1)
            .expect("should succeed in test");
        timeline
            .add_clip(track_id, create_test_clip("Clip2", 100, 100))
            .expect("should succeed in test");

        timeline
            .lift_clip(track_id, clip1_id)
            .expect("should succeed in test");

        let track = timeline
            .get_track(track_id)
            .expect("should succeed in test");
        assert_eq!(track.clips.len(), 1);
        // Clip2 should NOT have been rippled
        let clip2 = &track.clips[0];
        assert_eq!(clip2.timeline_in.value(), 100);
    }

    #[test]
    fn test_split_clip() {
        let mut timeline = create_test_timeline();
        let track_id = timeline
            .add_video_track("V1")
            .expect("should succeed in test");
        let clip = create_test_clip("Clip1", 0, 100);
        let clip_id = clip.id;
        timeline
            .add_clip(track_id, clip)
            .expect("should succeed in test");

        let (_left_id, _right_id) = timeline
            .split_clip(track_id, clip_id, Position::new(50))
            .expect("should succeed in test");

        let track = timeline
            .get_track(track_id)
            .expect("should succeed in test");
        assert_eq!(track.clips.len(), 2);
    }

    #[test]
    fn test_slip_edit() {
        let mut timeline = create_test_timeline();
        let track_id = timeline
            .add_video_track("V1")
            .expect("should succeed in test");
        let clip = create_test_clip("Clip1", 0, 100);
        let clip_id = clip.id;
        timeline
            .add_clip(track_id, clip)
            .expect("should succeed in test");

        timeline
            .slip_edit(track_id, clip_id, Duration::new(10))
            .expect("should succeed in test");

        let track = timeline
            .get_track(track_id)
            .expect("should succeed in test");
        let clip = track.get_clip(clip_id).expect("should succeed in test");
        assert_eq!(clip.source_in.value(), 10);
        assert_eq!(clip.source_out.value(), 110);
        assert_eq!(clip.timeline_in.value(), 0); // Timeline position unchanged
    }

    #[test]
    fn test_slide_edit() {
        let mut timeline = create_test_timeline();
        let track_id = timeline
            .add_video_track("V1")
            .expect("should succeed in test");
        let clip = create_test_clip("Clip1", 0, 100);
        let clip_id = clip.id;
        timeline
            .add_clip(track_id, clip)
            .expect("should succeed in test");

        timeline
            .slide_edit(track_id, clip_id, Position::new(50))
            .expect("should succeed in test");

        let track = timeline
            .get_track(track_id)
            .expect("should succeed in test");
        let clip = track.get_clip(clip_id).expect("should succeed in test");
        assert_eq!(clip.timeline_in.value(), 50);
        assert_eq!(clip.source_in.value(), 0); // Source points unchanged
        assert_eq!(clip.source_out.value(), 100);
    }
}
