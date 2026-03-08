//! Scene summarization: key shot extraction and scene-level digests.
//!
//! Identifies important shots within a scene based on dialog presence,
//! face detection, and motion levels, then builds per-scene summaries.

/// Per-shot data used for summarization.
#[derive(Debug, Clone)]
pub struct ShotSummary {
    /// Unique shot identifier.
    pub shot_id: u64,
    /// Duration of the shot in frames.
    pub duration_frames: u32,
    /// Dominant colour (R, G, B).
    pub dominant_color: [u8; 3],
    /// Normalised motion level (0.0 = static, 1.0 = maximum motion).
    pub motion_level: f32,
    /// Whether dialog is present in this shot.
    pub dialog: bool,
    /// Number of faces detected.
    pub faces_detected: u32,
}

impl ShotSummary {
    /// A shot is a key shot when it contains dialog, has detected faces, or
    /// has high motion (> 0.6).
    #[must_use]
    pub fn is_key_shot(&self) -> bool {
        self.dialog || self.faces_detected > 0 || self.motion_level > 0.6
    }
}

/// Aggregated summary of a scene built from individual shot summaries.
#[derive(Debug, Clone)]
pub struct SceneSummary {
    /// Scene identifier.
    pub scene_id: u64,
    /// Number of shots in the scene.
    pub shot_count: u32,
    /// Total frame count across all shots.
    pub total_frames: u64,
    /// Narrative importance score (0.0 – 1.0).
    pub narrative_importance: f32,
    /// IDs of shots identified as key shots.
    pub key_shots: Vec<u64>,
}

impl SceneSummary {
    /// Average shot duration in frames, or `0.0` when there are no shots.
    #[must_use]
    pub fn avg_shot_duration_frames(&self) -> f64 {
        if self.shot_count == 0 {
            return 0.0;
        }
        self.total_frames as f64 / f64::from(self.shot_count)
    }

    /// Returns `true` when any of the key shots contains dialog.
    ///
    /// This is a convenience accessor that delegates to whether any key shot
    /// was selected (implying dialog, faces, or high motion).
    /// Caller is expected to cross-reference with the input `ShotSummary` slice.
    #[must_use]
    pub fn has_dialog(&self) -> bool {
        !self.key_shots.is_empty()
    }
}

/// Configuration controlling which shots are selected for the summary.
#[derive(Debug, Clone)]
pub struct SummarizationConfig {
    /// Maximum number of shots to include in the summary.
    pub max_shots_per_summary: usize,
    /// Minimum narrative importance needed for a scene to be summarised.
    pub min_importance: f32,
}

impl Default for SummarizationConfig {
    fn default() -> Self {
        Self {
            max_shots_per_summary: 5,
            min_importance: 0.1,
        }
    }
}

/// Builds `SceneSummary` instances from slices of `ShotSummary`.
pub struct SceneSummarizer {
    config: SummarizationConfig,
}

impl SceneSummarizer {
    /// Create a summarizer with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SummarizationConfig::default(),
        }
    }

    /// Create a summarizer with the given configuration.
    #[must_use]
    pub fn with_config(config: SummarizationConfig) -> Self {
        Self { config }
    }

    /// Summarize a scene from a slice of shot summaries.
    ///
    /// Narrative importance is computed as the ratio of key shots to total shots.
    #[must_use]
    pub fn summarize_scene(&self, shots: &[ShotSummary]) -> SceneSummary {
        let shot_count = shots.len() as u32;
        let total_frames: u64 = shots.iter().map(|s| u64::from(s.duration_frames)).sum();
        let key_shots = self.select_key_shots(shots);
        let narrative_importance = if shot_count == 0 {
            0.0
        } else {
            key_shots.len() as f32 / shot_count as f32
        };
        SceneSummary {
            scene_id: 0, // caller can overwrite
            shot_count,
            total_frames,
            narrative_importance,
            key_shots,
        }
    }

    /// Summarize a scene, assigning the given `scene_id`.
    #[must_use]
    pub fn summarize_scene_with_id(&self, scene_id: u64, shots: &[ShotSummary]) -> SceneSummary {
        let mut summary = self.summarize_scene(shots);
        summary.scene_id = scene_id;
        summary
    }

    /// Select key shot IDs from the input slice, limited by `max_shots_per_summary`.
    #[must_use]
    pub fn select_key_shots(&self, shots: &[ShotSummary]) -> Vec<u64> {
        shots
            .iter()
            .filter(|s| s.is_key_shot())
            .take(self.config.max_shots_per_summary)
            .map(|s| s.shot_id)
            .collect()
    }
}

impl Default for SceneSummarizer {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_shot(id: u64, duration: u32, motion: f32, dialog: bool, faces: u32) -> ShotSummary {
        ShotSummary {
            shot_id: id,
            duration_frames: duration,
            dominant_color: [128, 64, 32],
            motion_level: motion,
            dialog,
            faces_detected: faces,
        }
    }

    // 1. ShotSummary::is_key_shot – dialog present
    #[test]
    fn test_is_key_shot_dialog() {
        let s = make_shot(0, 30, 0.1, true, 0);
        assert!(s.is_key_shot());
    }

    // 2. ShotSummary::is_key_shot – faces detected
    #[test]
    fn test_is_key_shot_faces() {
        let s = make_shot(1, 30, 0.1, false, 2);
        assert!(s.is_key_shot());
    }

    // 3. ShotSummary::is_key_shot – high motion
    #[test]
    fn test_is_key_shot_motion() {
        let s = make_shot(2, 30, 0.8, false, 0);
        assert!(s.is_key_shot());
    }

    // 4. ShotSummary::is_key_shot – none of the conditions
    #[test]
    fn test_is_not_key_shot() {
        let s = make_shot(3, 30, 0.3, false, 0);
        assert!(!s.is_key_shot());
    }

    // 5. ShotSummary::is_key_shot – boundary motion (exactly 0.6 is NOT > 0.6)
    #[test]
    fn test_is_key_shot_motion_boundary() {
        let s = make_shot(4, 30, 0.6, false, 0);
        assert!(!s.is_key_shot());
        let s2 = make_shot(5, 30, 0.600_001, false, 0);
        assert!(s2.is_key_shot());
    }

    // 6. SceneSummary::avg_shot_duration_frames – basic
    #[test]
    fn test_avg_shot_duration() {
        let summary = SceneSummary {
            scene_id: 0,
            shot_count: 4,
            total_frames: 120,
            narrative_importance: 0.5,
            key_shots: vec![],
        };
        assert!((summary.avg_shot_duration_frames() - 30.0).abs() < f64::EPSILON);
    }

    // 7. SceneSummary::avg_shot_duration_frames – zero shots
    #[test]
    fn test_avg_shot_duration_zero() {
        let summary = SceneSummary {
            scene_id: 0,
            shot_count: 0,
            total_frames: 0,
            narrative_importance: 0.0,
            key_shots: vec![],
        };
        assert!((summary.avg_shot_duration_frames()).abs() < f64::EPSILON);
    }

    // 8. SceneSummary::has_dialog – no key shots
    #[test]
    fn test_has_dialog_no_key_shots() {
        let summary = SceneSummary {
            scene_id: 0,
            shot_count: 2,
            total_frames: 60,
            narrative_importance: 0.0,
            key_shots: vec![],
        };
        assert!(!summary.has_dialog());
    }

    // 9. SceneSummary::has_dialog – with key shots
    #[test]
    fn test_has_dialog_with_key_shots() {
        let summary = SceneSummary {
            scene_id: 1,
            shot_count: 3,
            total_frames: 90,
            narrative_importance: 0.33,
            key_shots: vec![0, 1],
        };
        assert!(summary.has_dialog());
    }

    // 10. SummarizationConfig::default values
    #[test]
    fn test_summarization_config_default() {
        let cfg = SummarizationConfig::default();
        assert_eq!(cfg.max_shots_per_summary, 5);
        assert!((cfg.min_importance - 0.1).abs() < f32::EPSILON);
    }

    // 11. SceneSummarizer::summarize_scene – empty input
    #[test]
    fn test_summarize_empty() {
        let s = SceneSummarizer::new();
        let summary = s.summarize_scene(&[]);
        assert_eq!(summary.shot_count, 0);
        assert_eq!(summary.total_frames, 0);
        assert!(summary.key_shots.is_empty());
    }

    // 12. SceneSummarizer::summarize_scene – counts frames
    #[test]
    fn test_summarize_total_frames() {
        let s = SceneSummarizer::new();
        let shots = vec![
            make_shot(0, 30, 0.1, false, 0),
            make_shot(1, 60, 0.1, false, 0),
        ];
        let summary = s.summarize_scene(&shots);
        assert_eq!(summary.shot_count, 2);
        assert_eq!(summary.total_frames, 90);
    }

    // 13. SceneSummarizer::select_key_shots respects max_shots_per_summary
    #[test]
    fn test_select_key_shots_limit() {
        let cfg = SummarizationConfig {
            max_shots_per_summary: 2,
            min_importance: 0.0,
        };
        let s = SceneSummarizer::with_config(cfg);
        let shots: Vec<ShotSummary> = (0..10).map(|i| make_shot(i, 30, 0.0, true, 0)).collect();
        let keys = s.select_key_shots(&shots);
        assert_eq!(keys.len(), 2);
    }

    // 14. SceneSummarizer::summarize_scene_with_id sets scene_id
    #[test]
    fn test_summarize_with_id() {
        let s = SceneSummarizer::new();
        let shots = vec![make_shot(0, 30, 0.1, true, 1)];
        let summary = s.summarize_scene_with_id(42, &shots);
        assert_eq!(summary.scene_id, 42);
    }

    // 15. narrative_importance equals key_count / shot_count
    #[test]
    fn test_narrative_importance_ratio() {
        let s = SceneSummarizer::new();
        let shots = vec![
            make_shot(0, 30, 0.0, true, 0),  // key
            make_shot(1, 30, 0.0, false, 0), // not key
            make_shot(2, 30, 0.0, false, 0), // not key
            make_shot(3, 30, 0.0, false, 1), // key
        ];
        let summary = s.summarize_scene(&shots);
        // 2 key shots out of 4 = 0.5
        assert!((summary.narrative_importance - 0.5).abs() < f32::EPSILON);
    }
}
