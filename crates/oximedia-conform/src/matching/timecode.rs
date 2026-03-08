//! Timecode-based matching strategies.

use crate::config::ConformConfig;
use crate::types::{ClipMatch, ClipReference, MatchMethod, MediaFile, Timecode};

/// Match media files by source timecode.
#[must_use]
pub fn timecode_match(
    clip: &ClipReference,
    candidates: &[MediaFile],
    config: &ConformConfig,
) -> Vec<ClipMatch> {
    let mut matches = Vec::new();

    for media in candidates {
        if let Some(media_tc_start) = media.timecode_start {
            // Check if the media file's timecode range covers the clip's source range
            if let Some(media_duration) = media.duration {
                let media_frames = (media_duration * clip.fps.as_f64()) as u64;
                let media_tc_end = Timecode::from_frames(
                    media_tc_start.to_frames(clip.fps) + media_frames,
                    clip.fps,
                );

                let clip_start_frames = clip.source_in.to_frames(clip.fps);
                let clip_end_frames = clip.source_out.to_frames(clip.fps);
                let media_start_frames = media_tc_start.to_frames(clip.fps);
                let media_end_frames = media_tc_end.to_frames(clip.fps);

                // Check if ranges overlap or contain
                if media_start_frames <= clip_start_frames && clip_end_frames <= media_end_frames {
                    // Perfect containment
                    matches.push(ClipMatch {
                        clip: clip.clone(),
                        media: media.clone(),
                        score: 1.0,
                        method: MatchMethod::Timecode,
                        details: format!(
                            "Timecode match: clip [{}-{}] within media [{}-{}]",
                            clip.source_in, clip.source_out, media_tc_start, media_tc_end
                        ),
                    });
                } else if ranges_overlap(
                    media_start_frames,
                    media_end_frames,
                    clip_start_frames,
                    clip_end_frames,
                ) {
                    // Partial overlap
                    let overlap_score = calculate_overlap_score(
                        media_start_frames,
                        media_end_frames,
                        clip_start_frames,
                        clip_end_frames,
                    );

                    if overlap_score >= config.match_threshold {
                        matches.push(ClipMatch {
                            clip: clip.clone(),
                            media: media.clone(),
                            score: overlap_score,
                            method: MatchMethod::Timecode,
                            details: format!(
                                "Partial timecode overlap: {overlap_score:.2} [{}-{}] vs [{}-{}]",
                                media_tc_start, media_tc_end, clip.source_in, clip.source_out
                            ),
                        });
                    }
                }
            }
        }
    }

    matches
}

/// Check if timecode offset matches.
#[must_use]
pub fn timecode_offset_match(
    clip: &ClipReference,
    candidates: &[MediaFile],
    tolerance_frames: u64,
) -> Vec<ClipMatch> {
    let mut matches = Vec::new();

    for media in candidates {
        if let Some(media_tc_start) = media.timecode_start {
            let media_start_frames = media_tc_start.to_frames(clip.fps);
            let clip_start_frames = clip.source_in.to_frames(clip.fps);

            let offset = media_start_frames.abs_diff(clip_start_frames);

            if offset <= tolerance_frames {
                let score = 1.0 - (offset as f64 / tolerance_frames as f64);
                matches.push(ClipMatch {
                    clip: clip.clone(),
                    media: media.clone(),
                    score,
                    method: MatchMethod::Timecode,
                    details: format!(
                        "Timecode offset match: offset {offset} frames (tolerance: {tolerance_frames})"
                    ),
                });
            }
        }
    }

    matches
}

/// Check if two frame ranges overlap.
fn ranges_overlap(start1: u64, end1: u64, start2: u64, end2: u64) -> bool {
    start1 <= end2 && start2 <= end1
}

/// Calculate overlap score between two ranges.
fn calculate_overlap_score(start1: u64, end1: u64, start2: u64, end2: u64) -> f64 {
    if !ranges_overlap(start1, end1, start2, end2) {
        return 0.0;
    }

    let overlap_start = start1.max(start2);
    let overlap_end = end1.min(end2);
    let overlap_length = overlap_end - overlap_start;

    let range1_length = end1 - start1;
    let range2_length = end2 - start2;
    let min_length = range1_length.min(range2_length);

    if min_length == 0 {
        0.0
    } else {
        overlap_length as f64 / min_length as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FrameRate, TrackType};
    use std::path::PathBuf;

    fn create_test_clip(source_in: Timecode, source_out: Timecode) -> ClipReference {
        ClipReference {
            id: "test".to_string(),
            source_file: Some("test.mov".to_string()),
            source_in,
            source_out,
            record_in: Timecode::new(1, 0, 0, 0),
            record_out: Timecode::new(1, 0, 10, 0),
            track: TrackType::Video,
            fps: FrameRate::Fps25,
            metadata: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_timecode_match_perfect_containment() {
        let clip = create_test_clip(Timecode::new(1, 0, 5, 0), Timecode::new(1, 0, 10, 0));

        let mut media = MediaFile::new(PathBuf::from("/path/test.mov"));
        media.timecode_start = Some(Timecode::new(1, 0, 0, 0));
        media.duration = Some(20.0); // 20 seconds at 25fps = 500 frames
        media.fps = Some(FrameRate::Fps25);

        let config = ConformConfig::default();
        let matches = timecode_match(&clip, &[media], &config);
        assert_eq!(matches.len(), 1);
        assert!((matches[0].score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ranges_overlap() {
        assert!(ranges_overlap(0, 10, 5, 15));
        assert!(ranges_overlap(5, 15, 0, 10));
        assert!(!ranges_overlap(0, 10, 11, 20));
    }

    #[test]
    fn test_calculate_overlap_score() {
        // Complete overlap
        let score = calculate_overlap_score(0, 10, 0, 10);
        assert!((score - 1.0).abs() < f64::EPSILON);

        // 50% overlap
        let score = calculate_overlap_score(0, 10, 5, 15);
        assert!((score - 0.5).abs() < 0.01);

        // No overlap
        let score = calculate_overlap_score(0, 10, 11, 20);
        assert!(score.abs() < f64::EPSILON);
    }

    #[test]
    fn test_timecode_offset_match() {
        let clip = create_test_clip(Timecode::new(1, 0, 0, 5), Timecode::new(1, 0, 10, 0));

        let mut media = MediaFile::new(PathBuf::from("/path/test.mov"));
        media.timecode_start = Some(Timecode::new(1, 0, 0, 0));
        media.fps = Some(FrameRate::Fps25);

        let matches = timecode_offset_match(&clip, &[media], 10);
        assert_eq!(matches.len(), 1);
    }
}
