//! Combined matching strategies.

use crate::config::ConformConfig;
use crate::types::{ClipMatch, ClipReference, MatchMethod, MediaFile};
use dashmap::DashMap;
use rayon::prelude::*;
use std::sync::Arc;

use super::content::duration_match;
use super::filename::{exact_filename_match, fuzzy_filename_match, relink_proxy_match};
use super::timecode::timecode_match;

/// Match strategy for conforming clips to media files.
pub struct MatchStrategy {
    config: Arc<ConformConfig>,
}

impl MatchStrategy {
    /// Create a new match strategy with the given configuration.
    #[must_use]
    pub fn new(config: ConformConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Match a single clip against all available media files.
    #[must_use]
    pub fn match_clip(&self, clip: &ClipReference, all_media: &[MediaFile]) -> Vec<ClipMatch> {
        let mut all_matches = Vec::new();

        // Try exact filename matching first
        let mut matches = exact_filename_match(clip, all_media);
        all_matches.append(&mut matches);

        // Try fuzzy filename matching if enabled
        if self.config.fuzzy_matching {
            let mut matches = fuzzy_filename_match(clip, all_media, &self.config);
            all_matches.append(&mut matches);
        }

        // Try proxy relinking if enabled
        if self.config.auto_relink {
            let mut matches = relink_proxy_match(clip, all_media, &self.config);
            all_matches.append(&mut matches);
        }

        // Try timecode matching if enabled
        if self.config.timecode_matching {
            let mut matches = timecode_match(clip, all_media, &self.config);
            all_matches.append(&mut matches);
        }

        // Try duration matching if enabled
        if self.config.duration_matching {
            let mut matches = duration_match(clip, all_media, &self.config);
            all_matches.append(&mut matches);
        }

        // Filter by match threshold
        all_matches.retain(|m| m.score >= self.config.match_threshold);

        // Sort by score (highest first)
        all_matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Deduplicate by media file ID
        self.deduplicate_matches(all_matches)
    }

    /// Match multiple clips in parallel.
    #[must_use]
    pub fn match_clips(
        &self,
        clips: &[ClipReference],
        all_media: &[MediaFile],
    ) -> Vec<Vec<ClipMatch>> {
        clips
            .par_iter()
            .map(|clip| self.match_clip(clip, all_media))
            .collect()
    }

    /// Deduplicate matches by keeping the highest score for each media file.
    fn deduplicate_matches(&self, matches: Vec<ClipMatch>) -> Vec<ClipMatch> {
        let best_matches: DashMap<uuid::Uuid, ClipMatch> = DashMap::new();

        for m in matches {
            let media_id = m.media.id;
            best_matches
                .entry(media_id)
                .and_modify(|existing| {
                    if m.score > existing.score {
                        *existing = m.clone();
                    }
                })
                .or_insert(m);
        }

        let mut result: Vec<ClipMatch> = best_matches.into_iter().map(|(_, v)| v).collect();
        result.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        result
    }

    /// Get the best match for a clip (highest score).
    #[must_use]
    pub fn get_best_match(
        &self,
        clip: &ClipReference,
        all_media: &[MediaFile],
    ) -> Option<ClipMatch> {
        let matches = self.match_clip(clip, all_media);
        matches.into_iter().next()
    }

    /// Check if a match is ambiguous (multiple matches with similar scores).
    #[must_use]
    pub fn is_ambiguous(&self, matches: &[ClipMatch], tolerance: f64) -> bool {
        if matches.len() <= 1 {
            return false;
        }

        if let Some(best) = matches.first() {
            matches
                .iter()
                .skip(1)
                .any(|m| (best.score - m.score).abs() < tolerance)
        } else {
            false
        }
    }

    /// Combine multiple matches into a single match with combined score.
    #[must_use]
    pub fn combine_matches(&self, matches: Vec<ClipMatch>) -> Option<ClipMatch> {
        if matches.is_empty() {
            return None;
        }

        if matches.len() == 1 {
            return matches.into_iter().next();
        }

        // Check if all matches point to the same media file
        let first_media_id = matches[0].media.id;
        if matches.iter().all(|m| m.media.id == first_media_id) {
            let combined_score: f64 =
                matches.iter().map(|m| m.score).sum::<f64>() / matches.len() as f64;
            let methods: Vec<String> = matches.iter().map(|m| m.method.to_string()).collect();
            let details = format!("Combined match from: {}", methods.join(", "));

            Some(ClipMatch {
                clip: matches[0].clip.clone(),
                media: matches[0].media.clone(),
                score: combined_score,
                method: MatchMethod::Combined,
                details,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FrameRate, Timecode, TrackType};
    use std::path::PathBuf;

    fn create_test_clip(id: &str, source_file: &str) -> ClipReference {
        ClipReference {
            id: id.to_string(),
            source_file: Some(source_file.to_string()),
            source_in: Timecode::new(1, 0, 0, 0),
            source_out: Timecode::new(1, 0, 10, 0),
            record_in: Timecode::new(1, 0, 0, 0),
            record_out: Timecode::new(1, 0, 10, 0),
            track: TrackType::Video,
            fps: FrameRate::Fps25,
            metadata: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_match_strategy_exact() {
        let config = ConformConfig::default();
        let strategy = MatchStrategy::new(config);

        let clip = create_test_clip("clip1", "test.mov");
        let media = MediaFile::new(PathBuf::from("/path/test.mov"));

        let matches = strategy.match_clip(&clip, &[media]);
        assert!(!matches.is_empty());
    }

    #[test]
    fn test_match_clips_parallel() {
        let config = ConformConfig::default();
        let strategy = MatchStrategy::new(config);

        let clips = vec![
            create_test_clip("clip1", "test1.mov"),
            create_test_clip("clip2", "test2.mov"),
        ];

        let media = vec![
            MediaFile::new(PathBuf::from("/path/test1.mov")),
            MediaFile::new(PathBuf::from("/path/test2.mov")),
        ];

        let results = strategy.match_clips(&clips, &media);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_get_best_match() {
        let config = ConformConfig::default();
        let strategy = MatchStrategy::new(config);

        let clip = create_test_clip("clip1", "test.mov");
        let media = MediaFile::new(PathBuf::from("/path/test.mov"));

        let best = strategy.get_best_match(&clip, &[media]);
        assert!(best.is_some());
    }

    #[test]
    fn test_is_ambiguous() {
        let config = ConformConfig::default();
        let strategy = MatchStrategy::new(config);

        let clip = create_test_clip("clip1", "test.mov");
        let media1 = MediaFile::new(PathBuf::from("/path/test1.mov"));
        let media2 = MediaFile::new(PathBuf::from("/path/test2.mov"));

        let matches = vec![
            ClipMatch {
                clip: clip.clone(),
                media: media1,
                score: 0.9,
                method: MatchMethod::ExactFilename,
                details: String::new(),
            },
            ClipMatch {
                clip,
                media: media2,
                score: 0.89,
                method: MatchMethod::FuzzyFilename,
                details: String::new(),
            },
        ];

        assert!(strategy.is_ambiguous(&matches, 0.05));
        assert!(!strategy.is_ambiguous(&matches, 0.005));
    }
}
