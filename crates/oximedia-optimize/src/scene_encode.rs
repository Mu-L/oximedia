#![allow(dead_code)]
//! Scene-aware encoding optimization.
//!
//! This module provides scene-level encoding strategies that adjust encoding
//! parameters based on scene characteristics (complexity, motion, texture).
//! It operates at a higher level than per-frame optimization, making decisions
//! about GOP structure, QP offsets, and bitrate allocation on a scene-by-scene
//! basis.

use std::collections::VecDeque;

/// Scene type classification for encoding decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SceneType {
    /// Static or near-static scene (e.g., title card, credits).
    Static,
    /// Talking head or slow-paced dialogue scene.
    Dialogue,
    /// Moderate action with some motion.
    Moderate,
    /// Fast action scene with high motion and complexity.
    Action,
    /// Scene with lots of fine detail (e.g., foliage, crowds).
    HighDetail,
    /// Dark or low-light scene.
    DarkScene,
    /// Scene transition / crossfade.
    Transition,
}

/// Metrics describing a scene's visual characteristics.
#[derive(Debug, Clone)]
pub struct SceneMetrics {
    /// Scene index in the stream.
    pub scene_index: u32,
    /// Frame index where the scene starts.
    pub start_frame: u64,
    /// Frame index where the scene ends (exclusive).
    pub end_frame: u64,
    /// Average spatial complexity (0.0 - 1.0).
    pub spatial_complexity: f64,
    /// Average temporal complexity / motion (0.0 - 1.0).
    pub temporal_complexity: f64,
    /// Average luminance (0.0 - 1.0).
    pub avg_luminance: f64,
    /// Luminance variance.
    pub luminance_variance: f64,
    /// Texture density (0.0 - 1.0).
    pub texture_density: f64,
    /// Classified scene type.
    pub scene_type: SceneType,
}

impl SceneMetrics {
    /// Creates new scene metrics.
    #[must_use]
    pub fn new(scene_index: u32, start_frame: u64, end_frame: u64) -> Self {
        Self {
            scene_index,
            start_frame,
            end_frame,
            spatial_complexity: 0.5,
            temporal_complexity: 0.5,
            avg_luminance: 0.5,
            luminance_variance: 0.1,
            texture_density: 0.5,
            scene_type: SceneType::Moderate,
        }
    }

    /// Returns the number of frames in this scene.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.end_frame.saturating_sub(self.start_frame)
    }

    /// Returns a combined complexity score (0.0 - 1.0).
    #[must_use]
    pub fn combined_complexity(&self) -> f64 {
        (self.spatial_complexity * 0.4
            + self.temporal_complexity * 0.4
            + self.texture_density * 0.2)
            .clamp(0.0, 1.0)
    }

    /// Returns true if this is a dark scene.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.avg_luminance < 0.2
    }
}

/// Encoding parameters for a scene.
#[derive(Debug, Clone)]
pub struct SceneEncodeParams {
    /// QP offset relative to base QP (can be negative for higher quality).
    pub qp_offset: f64,
    /// Bitrate allocation weight (1.0 = normal, >1.0 = more bits).
    pub bitrate_weight: f64,
    /// Recommended GOP size in frames.
    pub gop_size: u32,
    /// Whether to force a keyframe at scene start.
    pub force_keyframe: bool,
    /// Minimum QP allowed.
    pub min_qp: f64,
    /// Maximum QP allowed.
    pub max_qp: f64,
    /// B-frame count for this scene.
    pub b_frames: u32,
    /// Whether to enable adaptive quantization.
    pub enable_aq: bool,
    /// AQ strength (0.0 - 2.0).
    pub aq_strength: f64,
}

impl Default for SceneEncodeParams {
    fn default() -> Self {
        Self {
            qp_offset: 0.0,
            bitrate_weight: 1.0,
            gop_size: 250,
            force_keyframe: true,
            min_qp: 0.0,
            max_qp: 51.0,
            b_frames: 3,
            enable_aq: true,
            aq_strength: 1.0,
        }
    }
}

impl SceneEncodeParams {
    /// Creates new default scene encoding parameters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the effective QP given a base QP.
    #[must_use]
    pub fn effective_qp(&self, base_qp: f64) -> f64 {
        (base_qp + self.qp_offset).clamp(self.min_qp, self.max_qp)
    }
}

/// Scene-based encoding optimizer.
#[derive(Debug)]
pub struct SceneEncoder {
    /// Base QP for the encoding session.
    base_qp: f64,
    /// Target bitrate in bps.
    target_bitrate_bps: u64,
    /// Frame rate.
    frame_rate: f64,
    /// Scene parameters generated so far.
    scene_params: Vec<SceneEncodeParams>,
    /// Scene metrics history.
    scene_history: VecDeque<SceneMetrics>,
    /// Maximum scene history length.
    max_history: usize,
}

impl SceneEncoder {
    /// Creates a new scene encoder with target settings.
    #[must_use]
    pub fn new(base_qp: f64, target_bitrate_bps: u64, frame_rate: f64) -> Self {
        Self {
            base_qp,
            target_bitrate_bps,
            frame_rate,
            scene_params: Vec::new(),
            scene_history: VecDeque::new(),
            max_history: 100,
        }
    }

    /// Returns the base QP.
    #[must_use]
    pub fn base_qp(&self) -> f64 {
        self.base_qp
    }

    /// Returns the target bitrate.
    #[must_use]
    pub fn target_bitrate_bps(&self) -> u64 {
        self.target_bitrate_bps
    }

    /// Generates encoding parameters for a scene based on its metrics.
    #[must_use]
    pub fn generate_params(&self, metrics: &SceneMetrics) -> SceneEncodeParams {
        let mut params = SceneEncodeParams::default();

        // Determine QP offset based on scene type and complexity
        params.qp_offset = self.compute_qp_offset(metrics);
        params.bitrate_weight = self.compute_bitrate_weight(metrics);
        params.gop_size = self.compute_gop_size(metrics);
        params.b_frames = self.compute_b_frames(metrics);
        params.aq_strength = self.compute_aq_strength(metrics);

        // Always force keyframe at scene boundaries
        params.force_keyframe = true;

        params
    }

    /// Processes a scene: records metrics and generates params.
    pub fn process_scene(&mut self, metrics: SceneMetrics) -> SceneEncodeParams {
        let params = self.generate_params(&metrics);
        self.scene_params.push(params.clone());
        self.scene_history.push_back(metrics);
        if self.scene_history.len() > self.max_history {
            self.scene_history.pop_front();
        }
        params
    }

    /// Returns scene parameters generated so far.
    #[must_use]
    pub fn scene_params(&self) -> &[SceneEncodeParams] {
        &self.scene_params
    }

    /// Returns the number of scenes processed.
    #[must_use]
    pub fn scenes_processed(&self) -> usize {
        self.scene_params.len()
    }

    /// Computes the QP offset for a scene.
    fn compute_qp_offset(&self, metrics: &SceneMetrics) -> f64 {
        match metrics.scene_type {
            SceneType::Static => -4.0,    // boost quality for static
            SceneType::Dialogue => -2.0,  // slightly boost dialogue
            SceneType::Moderate => 0.0,   // neutral
            SceneType::Action => 2.0,     // relax for action
            SceneType::HighDetail => 1.0, // slight relax for heavy detail
            SceneType::DarkScene => -3.0, // boost dark scenes (artifacts visible)
            SceneType::Transition => 3.0, // relax during transitions
        }
    }

    /// Computes the bitrate weight for a scene.
    fn compute_bitrate_weight(&self, metrics: &SceneMetrics) -> f64 {
        let complexity = metrics.combined_complexity();
        // Map complexity [0,1] to weight [0.6, 1.8]
        0.6 + complexity * 1.2
    }

    /// Computes GOP size based on temporal characteristics.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    fn compute_gop_size(&self, metrics: &SceneMetrics) -> u32 {
        let frame_count = metrics.frame_count();
        // GOP should not exceed scene length
        let max_gop = frame_count.min(300) as u32;

        match metrics.scene_type {
            SceneType::Static => max_gop.min(300),
            SceneType::Dialogue => max_gop.min(250),
            SceneType::Moderate => max_gop.min(200),
            SceneType::Action => max_gop.min(120),
            SceneType::HighDetail => max_gop.min(150),
            SceneType::DarkScene => max_gop.min(250),
            SceneType::Transition => max_gop.min(60),
        }
    }

    /// Computes B-frame count for a scene.
    fn compute_b_frames(&self, metrics: &SceneMetrics) -> u32 {
        match metrics.scene_type {
            SceneType::Static => 5,
            SceneType::Dialogue => 4,
            SceneType::Moderate => 3,
            SceneType::Action => 2,
            SceneType::HighDetail => 3,
            SceneType::DarkScene => 4,
            SceneType::Transition => 1,
        }
    }

    /// Computes adaptive quantization strength.
    fn compute_aq_strength(&self, metrics: &SceneMetrics) -> f64 {
        if metrics.is_dark() {
            // Stronger AQ for dark scenes to reduce banding
            1.5
        } else if metrics.spatial_complexity > 0.7 {
            1.2
        } else {
            1.0
        }
    }

    /// Returns the average complexity across all processed scenes.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_scene_complexity(&self) -> f64 {
        if self.scene_history.is_empty() {
            return 0.0;
        }
        let total: f64 = self
            .scene_history
            .iter()
            .map(|m| m.combined_complexity())
            .sum();
        total / self.scene_history.len() as f64
    }
}

/// Bitrate distribution plan across scenes.
#[derive(Debug, Clone)]
pub struct SceneBitratePlan {
    /// Scene index to allocated bitrate in bps.
    allocations: Vec<SceneBitrateAlloc>,
}

/// A single scene's bitrate allocation.
#[derive(Debug, Clone)]
pub struct SceneBitrateAlloc {
    /// Scene index.
    pub scene_index: u32,
    /// Allocated bitrate in bps.
    pub allocated_bps: u64,
    /// Frame count in the scene.
    pub frame_count: u64,
    /// Allocated bits for the entire scene.
    pub total_bits: u64,
}

impl SceneBitratePlan {
    /// Creates a new bitrate plan.
    #[must_use]
    pub fn new() -> Self {
        Self {
            allocations: Vec::new(),
        }
    }

    /// Distributes a total bitrate budget across scenes weighted by encode params.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn distribute(
        scenes: &[SceneMetrics],
        params: &[SceneEncodeParams],
        total_bitrate_bps: u64,
        fps: f64,
    ) -> Self {
        if scenes.is_empty() || params.is_empty() || fps <= 0.0 {
            return Self::new();
        }

        let len = scenes.len().min(params.len());

        // Compute weighted frame counts
        let weighted_total: f64 = (0..len)
            .map(|i| scenes[i].frame_count() as f64 * params[i].bitrate_weight)
            .sum();

        if weighted_total <= 0.0 {
            return Self::new();
        }

        let total_frames: u64 = scenes[..len].iter().map(|s| s.frame_count()).sum();
        let total_bits = (total_bitrate_bps as f64 * total_frames as f64 / fps) as u64;

        let mut allocations = Vec::with_capacity(len);
        for i in 0..len {
            let weight = scenes[i].frame_count() as f64 * params[i].bitrate_weight / weighted_total;
            let scene_bits = (total_bits as f64 * weight) as u64;
            let scene_bps = if scenes[i].frame_count() > 0 {
                (scene_bits as f64 * fps / scenes[i].frame_count() as f64) as u64
            } else {
                0
            };

            allocations.push(SceneBitrateAlloc {
                scene_index: scenes[i].scene_index,
                allocated_bps: scene_bps,
                frame_count: scenes[i].frame_count(),
                total_bits: scene_bits,
            });
        }

        Self { allocations }
    }

    /// Returns the allocations.
    #[must_use]
    pub fn allocations(&self) -> &[SceneBitrateAlloc] {
        &self.allocations
    }

    /// Returns the number of scene allocations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.allocations.len()
    }

    /// Returns true if no allocations exist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.allocations.is_empty()
    }

    /// Returns total allocated bits.
    #[must_use]
    pub fn total_bits(&self) -> u64 {
        self.allocations.iter().map(|a| a.total_bits).sum()
    }
}

impl Default for SceneBitratePlan {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_scene(index: u32, start: u64, end: u64, st: SceneType) -> SceneMetrics {
        let mut m = SceneMetrics::new(index, start, end);
        m.scene_type = st;
        m.spatial_complexity = match st {
            SceneType::Static => 0.1,
            SceneType::Dialogue => 0.3,
            SceneType::Moderate => 0.5,
            SceneType::Action => 0.8,
            SceneType::HighDetail => 0.9,
            SceneType::DarkScene => 0.4,
            SceneType::Transition => 0.6,
        };
        m.temporal_complexity = match st {
            SceneType::Static => 0.05,
            SceneType::Dialogue => 0.2,
            SceneType::Moderate => 0.5,
            SceneType::Action => 0.9,
            SceneType::HighDetail => 0.4,
            SceneType::DarkScene => 0.3,
            SceneType::Transition => 0.7,
        };
        if st == SceneType::DarkScene {
            m.avg_luminance = 0.1;
        }
        m
    }

    #[test]
    fn test_scene_metrics_frame_count() {
        let m = SceneMetrics::new(0, 10, 50);
        assert_eq!(m.frame_count(), 40);
    }

    #[test]
    fn test_scene_metrics_combined_complexity() {
        let mut m = SceneMetrics::new(0, 0, 100);
        m.spatial_complexity = 0.8;
        m.temporal_complexity = 0.6;
        m.texture_density = 0.5;
        let cc = m.combined_complexity();
        // 0.8*0.4 + 0.6*0.4 + 0.5*0.2 = 0.32 + 0.24 + 0.10 = 0.66
        assert!((cc - 0.66).abs() < 1e-6);
    }

    #[test]
    fn test_scene_metrics_is_dark() {
        let mut m = SceneMetrics::new(0, 0, 100);
        m.avg_luminance = 0.1;
        assert!(m.is_dark());
        m.avg_luminance = 0.5;
        assert!(!m.is_dark());
    }

    #[test]
    fn test_scene_encode_params_default() {
        let p = SceneEncodeParams::default();
        assert!((p.qp_offset - 0.0).abs() < 1e-9);
        assert!((p.bitrate_weight - 1.0).abs() < 1e-9);
        assert_eq!(p.gop_size, 250);
    }

    #[test]
    fn test_effective_qp() {
        let p = SceneEncodeParams {
            qp_offset: -3.0,
            min_qp: 10.0,
            max_qp: 45.0,
            ..SceneEncodeParams::default()
        };
        assert!((p.effective_qp(28.0) - 25.0).abs() < 1e-9);
        // Clamp to min
        assert!((p.effective_qp(11.0) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn test_scene_encoder_new() {
        let enc = SceneEncoder::new(26.0, 5_000_000, 30.0);
        assert!((enc.base_qp() - 26.0).abs() < 1e-9);
        assert_eq!(enc.target_bitrate_bps(), 5_000_000);
    }

    #[test]
    fn test_generate_params_static() {
        let enc = SceneEncoder::new(26.0, 5_000_000, 30.0);
        let scene = make_scene(0, 0, 300, SceneType::Static);
        let params = enc.generate_params(&scene);
        assert!(params.qp_offset < 0.0); // Should boost quality
        assert!(params.gop_size <= 300);
    }

    #[test]
    fn test_generate_params_action() {
        let enc = SceneEncoder::new(26.0, 5_000_000, 30.0);
        let scene = make_scene(0, 0, 200, SceneType::Action);
        let params = enc.generate_params(&scene);
        assert!(params.qp_offset > 0.0); // Should relax
        assert!(params.b_frames <= 3);
    }

    #[test]
    fn test_generate_params_dark() {
        let enc = SceneEncoder::new(26.0, 5_000_000, 30.0);
        let scene = make_scene(0, 0, 200, SceneType::DarkScene);
        let params = enc.generate_params(&scene);
        assert!(params.aq_strength > 1.0); // Stronger AQ for dark
    }

    #[test]
    fn test_process_scene() {
        let mut enc = SceneEncoder::new(26.0, 5_000_000, 30.0);
        let scene = make_scene(0, 0, 100, SceneType::Moderate);
        let _params = enc.process_scene(scene);
        assert_eq!(enc.scenes_processed(), 1);
    }

    #[test]
    fn test_avg_scene_complexity() {
        let mut enc = SceneEncoder::new(26.0, 5_000_000, 30.0);
        enc.process_scene(make_scene(0, 0, 100, SceneType::Static));
        enc.process_scene(make_scene(1, 100, 200, SceneType::Action));
        let avg = enc.avg_scene_complexity();
        assert!(avg > 0.0 && avg < 1.0);
    }

    #[test]
    fn test_bitrate_plan_distribute() {
        let scenes = vec![
            make_scene(0, 0, 100, SceneType::Static),
            make_scene(1, 100, 300, SceneType::Action),
        ];
        let enc = SceneEncoder::new(26.0, 5_000_000, 30.0);
        let params: Vec<_> = scenes.iter().map(|s| enc.generate_params(s)).collect();

        let plan = SceneBitratePlan::distribute(&scenes, &params, 5_000_000, 30.0);
        assert_eq!(plan.len(), 2);
        assert!(!plan.is_empty());
        assert!(plan.total_bits() > 0);
    }

    #[test]
    fn test_bitrate_plan_empty() {
        let plan = SceneBitratePlan::distribute(&[], &[], 5_000_000, 30.0);
        assert!(plan.is_empty());
        assert_eq!(plan.total_bits(), 0);
    }

    #[test]
    fn test_bitrate_plan_action_gets_more_bits() {
        let scenes = vec![
            make_scene(0, 0, 100, SceneType::Static),
            make_scene(1, 100, 200, SceneType::Action),
        ];
        let enc = SceneEncoder::new(26.0, 5_000_000, 30.0);
        let params: Vec<_> = scenes.iter().map(|s| enc.generate_params(s)).collect();

        let plan = SceneBitratePlan::distribute(&scenes, &params, 5_000_000, 30.0);
        let allocs = plan.allocations();
        // Action scene should get higher bitrate due to higher weight
        assert!(allocs[1].allocated_bps > allocs[0].allocated_bps);
    }
}
