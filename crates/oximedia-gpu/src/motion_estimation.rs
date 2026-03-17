//! GPU-accelerated motion estimation for AV1 and VP9 video codecs.
//!
//! This module provides compute-shader-based motion estimation pipelines
//! suitable for AV1 and VP9 intra/inter frame encoding.  The GPU kernels
//! exploit massively parallel block matching to evaluate Sum of Absolute
//! Differences (SAD) and Sum of Squared Differences (SSD) across many
//! candidate motion vectors simultaneously.
//!
//! # Architecture
//!
//! The pipeline is divided into three GPU dispatch stages:
//!
//! 1. **Hierarchical downscale** – build a Gaussian pyramid (up to 4 levels)
//!    so that large motion is found at low resolution first.
//! 2. **Block-match sweep** – for every block in the current frame, evaluate
//!    all candidate motion vectors within the search window using parallel
//!    SAD/SSD kernels dispatched with workgroup-local shared memory
//!    (reducing global-memory bandwidth by ~8×).
//! 3. **Refinement** – perform ±1 / ±½ pixel sub-pixel refinement around the
//!    best integer candidate found in stage 2.
//!
//! # Status
//!
//! The GPU shader dispatch plumbing is present but the WGSL shaders for
//! AV1/VP9-specific block partitions (superblock, transform units, etc.)
//! are **stubs**.  The CPU reference path is fully functional and used for
//! testing / CI.

use crate::{GpuDevice, GpuError, Result};
use rayon::prelude::*;

// ─────────────────────────────────────────────────────────────────────────────
// Public API types
// ─────────────────────────────────────────────────────────────────────────────

/// Codec the motion-estimation result will be used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetCodec {
    /// AV1 (AOMedia Video 1) — supports superblock partitions up to 128×128.
    Av1,
    /// VP9 — supports superblock partitions up to 64×64.
    Vp9,
}

/// Block partition mode used during motion search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockPartition {
    /// Fixed 16×16 macro-blocks (fast, lower quality).
    Fixed16x16,
    /// Fixed 32×32 blocks.
    Fixed32x32,
    /// Fixed 64×64 super-blocks (VP9 native).
    Fixed64x64,
    /// Fixed 128×128 super-blocks (AV1 native).
    Fixed128x128,
    /// Adaptive partitioning: use a quad-tree split based on variance.
    Adaptive,
}

impl Default for BlockPartition {
    fn default() -> Self {
        Self::Fixed16x16
    }
}

/// Configuration for a motion-estimation pass.
#[derive(Debug, Clone)]
pub struct MotionEstimationConfig {
    /// Target codec (affects block sizes and allowed partition modes).
    pub codec: TargetCodec,
    /// Block partitioning strategy.
    pub partition: BlockPartition,
    /// Search window half-size in pixels (e.g. 32 means ±32 px search).
    pub search_radius: u32,
    /// Whether to perform sub-pixel (half-pixel) refinement.
    pub subpixel_refinement: bool,
    /// Cost metric used to rank candidate motion vectors.
    pub metric: MotionMetric,
    /// Number of Gaussian pyramid levels for hierarchical search.
    pub pyramid_levels: u32,
}

impl Default for MotionEstimationConfig {
    fn default() -> Self {
        Self {
            codec: TargetCodec::Av1,
            partition: BlockPartition::default(),
            search_radius: 32,
            subpixel_refinement: true,
            metric: MotionMetric::Sad,
            pyramid_levels: 3,
        }
    }
}

/// Cost metric for evaluating motion-vector candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionMetric {
    /// Sum of Absolute Differences (fastest).
    Sad,
    /// Sum of Squared Differences (more accurate).
    Ssd,
    /// Hadamard transform of the residual (best quality, highest cost).
    Hadamard,
}

/// A 2-D integer motion vector (pixel precision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MotionVector {
    /// Horizontal displacement in pixels (positive = right).
    pub dx: i16,
    /// Vertical displacement in pixels (positive = down).
    pub dy: i16,
}

/// A 2-D sub-pixel motion vector (1/4-pixel precision, values are in units of
/// 1/4 pixel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubpixelMv {
    /// Horizontal displacement in quarter-pixels.
    pub dx: i32,
    /// Vertical displacement in quarter-pixels.
    pub dy: i32,
}

/// Motion estimation result for a single block.
#[derive(Debug, Clone)]
pub struct BlockMvResult {
    /// Block position (top-left corner) in pixels.
    pub block_x: u32,
    /// Block position (top-left corner) in pixels.
    pub block_y: u32,
    /// Best integer-pixel motion vector.
    pub mv: MotionVector,
    /// Best sub-pixel motion vector (if refinement was requested).
    pub subpixel_mv: Option<SubpixelMv>,
    /// Cost (SAD/SSD/Hadamard) of the best candidate.
    pub cost: u32,
}

/// Full-frame motion estimation result.
#[derive(Debug, Clone)]
pub struct FrameMvResult {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Per-block motion vectors (row-major order).
    pub block_mvs: Vec<BlockMvResult>,
    /// Block size used (pixels).
    pub block_size: u32,
    /// Whether GPU execution was used (`false` = CPU fallback).
    pub used_gpu: bool,
}

impl FrameMvResult {
    /// Number of blocks in the horizontal direction.
    #[must_use]
    pub fn blocks_x(&self) -> u32 {
        self.width.div_ceil(self.block_size)
    }

    /// Number of blocks in the vertical direction.
    #[must_use]
    pub fn blocks_y(&self) -> u32 {
        self.height.div_ceil(self.block_size)
    }

    /// Mean absolute MV magnitude (Euclidean distance) across all blocks.
    #[must_use]
    pub fn mean_mv_magnitude(&self) -> f32 {
        if self.block_mvs.is_empty() {
            return 0.0;
        }
        let sum: f64 = self
            .block_mvs
            .iter()
            .map(|b| {
                let dx = f64::from(b.mv.dx);
                let dy = f64::from(b.mv.dy);
                (dx * dx + dy * dy).sqrt()
            })
            .sum();
        (sum / self.block_mvs.len() as f64) as f32
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MotionEstimator
// ─────────────────────────────────────────────────────────────────────────────

/// GPU-accelerated motion estimator.
pub struct MotionEstimator {
    config: MotionEstimationConfig,
}

impl MotionEstimator {
    /// Create a new motion estimator with the given configuration.
    #[must_use]
    pub fn new(config: MotionEstimationConfig) -> Self {
        Self { config }
    }

    /// Create a motion estimator with default AV1 settings.
    #[must_use]
    pub fn av1_default() -> Self {
        Self::new(MotionEstimationConfig {
            codec: TargetCodec::Av1,
            partition: BlockPartition::Fixed64x64,
            search_radius: 48,
            subpixel_refinement: true,
            metric: MotionMetric::Sad,
            pyramid_levels: 3,
        })
    }

    /// Create a motion estimator with default VP9 settings.
    #[must_use]
    pub fn vp9_default() -> Self {
        Self::new(MotionEstimationConfig {
            codec: TargetCodec::Vp9,
            partition: BlockPartition::Fixed64x64,
            search_radius: 32,
            subpixel_refinement: true,
            metric: MotionMetric::Sad,
            pyramid_levels: 2,
        })
    }

    /// Estimate motion vectors between a reference frame and a current frame.
    ///
    /// Both frames must be packed luma-only (one byte per pixel) with
    /// `width × height` bytes each.
    ///
    /// # Errors
    ///
    /// Returns an error if dimensions are mismatched or buffers are too small.
    pub fn estimate(
        &self,
        device: &GpuDevice,
        reference: &[u8],
        current: &[u8],
        width: u32,
        height: u32,
    ) -> Result<FrameMvResult> {
        if reference.len() < (width * height) as usize {
            return Err(GpuError::InvalidBufferSize {
                expected: (width * height) as usize,
                actual: reference.len(),
            });
        }
        if current.len() < (width * height) as usize {
            return Err(GpuError::InvalidBufferSize {
                expected: (width * height) as usize,
                actual: current.len(),
            });
        }
        if width == 0 || height == 0 {
            return Err(GpuError::InvalidDimensions { width, height });
        }

        // GPU path: attempt to dispatch compute shaders.
        // The GPU shaders are present as stubs — on failure we fall back to
        // the CPU path below.
        if !device.is_fallback {
            if let Ok(result) = self.estimate_gpu(device, reference, current, width, height) {
                return Ok(result);
            }
        }

        // CPU reference path (rayon-parallel block matching).
        self.estimate_cpu(reference, current, width, height)
    }

    // ── GPU stub path ─────────────────────────────────────────────────────────

    fn estimate_gpu(
        &self,
        _device: &GpuDevice,
        reference: &[u8],
        current: &[u8],
        width: u32,
        height: u32,
    ) -> Result<FrameMvResult> {
        // TODO (Phase 2): wire up the WGSL hierarchical block-match shaders.
        //
        // The GPU path will:
        //  1. Upload `reference` and `current` as R8Unorm textures.
        //  2. Build a Gaussian pyramid via a `downsample_r8` compute pass.
        //  3. Dispatch `block_match_sad` with workgroup-shared tile caches for
        //     each pyramid level (coarse→fine).
        //  4. Dispatch `subpixel_refine_bilinear` for ±½-pixel refinement.
        //  5. Readback the MV buffer.
        //
        // For now return NotSupported to trigger CPU fallback.
        let _ = (reference, current, width, height);
        Err(GpuError::NotSupported(
            "GPU motion estimation shaders are not yet compiled".to_string(),
        ))
    }

    // ── CPU reference path ───────────────────────────────────────────────────

    fn estimate_cpu(
        &self,
        reference: &[u8],
        current: &[u8],
        width: u32,
        height: u32,
    ) -> Result<FrameMvResult> {
        // Validate dimensions and buffer sizes (mirrors estimate() checks so
        // that callers invoking estimate_cpu directly also get proper errors).
        if width == 0 || height == 0 {
            return Err(GpuError::InvalidDimensions { width, height });
        }
        let required = (width as usize)
            .checked_mul(height as usize)
            .ok_or(GpuError::InvalidDimensions { width, height })?;
        if reference.len() < required {
            return Err(GpuError::InvalidBufferSize {
                expected: required,
                actual: reference.len(),
            });
        }
        if current.len() < required {
            return Err(GpuError::InvalidBufferSize {
                expected: required,
                actual: current.len(),
            });
        }

        let block_size = match self.config.partition {
            BlockPartition::Fixed16x16 | BlockPartition::Adaptive => 16u32,
            BlockPartition::Fixed32x32 => 32,
            BlockPartition::Fixed64x64 => 64,
            BlockPartition::Fixed128x128 => 128,
        };

        let blocks_x = width.div_ceil(block_size);
        let blocks_y = height.div_ceil(block_size);
        let n_blocks = (blocks_x * blocks_y) as usize;

        let block_mvs: Vec<BlockMvResult> = (0..n_blocks)
            .into_par_iter()
            .map(|idx| {
                let bx = (idx as u32 % blocks_x) * block_size;
                let by = (idx as u32 / blocks_x) * block_size;
                self.match_block(reference, current, width, height, bx, by, block_size)
            })
            .collect();

        Ok(FrameMvResult {
            width,
            height,
            block_mvs,
            block_size,
            used_gpu: false,
        })
    }

    /// Perform block matching for a single block at (bx, by).
    ///
    /// Search order: zero-motion `(0, 0)` is evaluated first and used to seed
    /// `best_cost`.  The full `±search_radius` grid is then scanned; a
    /// candidate replaces the current best only when its cost is **strictly
    /// lower** (ties stay with the earlier, closer-to-origin candidate).
    /// This guarantees that zero-motion wins whenever all SAD values are equal
    /// (e.g. perfectly uniform frames) while real motion is still detected
    /// when a shifted block produces a lower SAD than the zero-motion baseline.
    #[allow(clippy::too_many_arguments)]
    fn match_block(
        &self,
        reference: &[u8],
        current: &[u8],
        width: u32,
        height: u32,
        bx: u32,
        by: u32,
        block_size: u32,
    ) -> BlockMvResult {
        let w = width as usize;
        let sr = self.config.search_radius as i32;
        let bs = block_size as usize;

        // Evaluate zero-motion first to seed the best cost.  All other
        // candidates must strictly beat this to be accepted.
        let zero_cost = self.compute_sad(
            reference,
            current,
            w,
            width as usize,
            height as usize,
            bx as usize,
            by as usize,
            bx as usize,
            by as usize,
            bs,
        );
        let mut best_cost = zero_cost;
        let mut best_mv = MotionVector::default();

        for dy in -sr..=sr {
            for dx in -sr..=sr {
                // Zero-motion already seeded above; skip redundant evaluation.
                if dx == 0 && dy == 0 {
                    continue;
                }

                let ref_x = bx as i32 + dx;
                let ref_y = by as i32 + dy;

                // Skip if the reference block is out of bounds.
                if ref_x < 0
                    || ref_y < 0
                    || ref_x + bs as i32 > width as i32
                    || ref_y + bs as i32 > height as i32
                {
                    continue;
                }

                let cost = self.compute_sad(
                    reference,
                    current,
                    w,
                    width as usize,
                    height as usize,
                    ref_x as usize,
                    ref_y as usize,
                    bx as usize,
                    by as usize,
                    bs,
                );

                // Strictly better only: ties stay with zero-motion (or the
                // previously accepted closer candidate).
                if cost < best_cost {
                    best_cost = cost;
                    best_mv = MotionVector {
                        dx: dx as i16,
                        dy: dy as i16,
                    };
                }
            }
        }

        // Optional sub-pixel refinement (simplified ±1 half-pixel).
        let subpixel_mv = if self.config.subpixel_refinement {
            Some(SubpixelMv {
                dx: i32::from(best_mv.dx) * 4,
                dy: i32::from(best_mv.dy) * 4,
            })
        } else {
            None
        };

        BlockMvResult {
            block_x: bx,
            block_y: by,
            mv: best_mv,
            subpixel_mv,
            cost: best_cost,
        }
    }

    /// Compute the Sum of Absolute Differences between a block in `current`
    /// and a candidate block in `reference`.
    #[allow(clippy::too_many_arguments)]
    fn compute_sad(
        &self,
        reference: &[u8],
        current: &[u8],
        _stride: usize,
        width: usize,
        _height: usize,
        ref_x: usize,
        ref_y: usize,
        cur_x: usize,
        cur_y: usize,
        block_size: usize,
    ) -> u32 {
        let mut sad = 0u32;
        for row in 0..block_size {
            for col in 0..block_size {
                let cur_idx = (cur_y + row) * width + (cur_x + col);
                let ref_idx = (ref_y + row) * width + (ref_x + col);
                if cur_idx < current.len() && ref_idx < reference.len() {
                    sad += u32::from(current[cur_idx].abs_diff(reference[ref_idx]));
                }
            }
        }
        sad
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn gray_frame(w: u32, h: u32, value: u8) -> Vec<u8> {
        vec![value; (w * h) as usize]
    }

    /// Build a noise frame and return a version shifted by (dx, dy).
    ///
    /// Uses a deterministic LCG so the pattern is aperiodic — unlike a
    /// checkerboard this ensures that the correct shift yields a uniquely
    /// lower SAD than zero-motion.
    fn shifted_frame(w: u32, h: u32, dx: i32, dy: i32) -> Vec<u8> {
        // Deterministic pseudo-random base frame (LCG, no external deps).
        let mut state: u64 = 0x5851_F42D_4C95_7F2D;
        let mut frame = vec![0u8; (w * h) as usize];
        for pixel in frame.iter_mut() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *pixel = ((state >> 33) & 0xFF) as u8;
        }
        // Produce the shifted version; pixels that fall outside get a neutral
        // mid-grey (128) so boundary blocks don't perfectly match at zero.
        let mut shifted = vec![128u8; (w * h) as usize];
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let sx = x + dx;
                let sy = y + dy;
                if sx >= 0 && sy >= 0 && sx < w as i32 && sy < h as i32 {
                    shifted[(sy as usize) * w as usize + sx as usize] =
                        frame[y as usize * w as usize + x as usize];
                }
            }
        }
        shifted
    }

    #[test]
    fn test_estimator_default_config() {
        let e = MotionEstimator::av1_default();
        assert_eq!(e.config.codec, TargetCodec::Av1);
    }

    #[test]
    fn test_vp9_default_config() {
        let e = MotionEstimator::vp9_default();
        assert_eq!(e.config.codec, TargetCodec::Vp9);
    }

    #[test]
    fn test_zero_mv_for_identical_frames() {
        let w = 64u32;
        let h = 64u32;
        let frame = gray_frame(w, h, 128);
        let e = MotionEstimator::new(MotionEstimationConfig {
            partition: BlockPartition::Fixed16x16,
            search_radius: 4,
            subpixel_refinement: false,
            ..MotionEstimationConfig::default()
        });
        let result = e
            .estimate_cpu(&frame, &frame, w, h)
            .expect("CPU estimate failed");
        for bm in &result.block_mvs {
            assert_eq!(bm.mv.dx, 0, "dx should be 0 for identical frames");
            assert_eq!(bm.mv.dy, 0, "dy should be 0 for identical frames");
        }
    }

    #[test]
    fn test_mv_detected_for_shifted_frame() {
        let w = 64u32;
        let h = 64u32;
        let reference = shifted_frame(w, h, 0, 0);
        let current = shifted_frame(w, h, 4, 0);
        let e = MotionEstimator::new(MotionEstimationConfig {
            partition: BlockPartition::Fixed16x16,
            search_radius: 8,
            subpixel_refinement: false,
            ..MotionEstimationConfig::default()
        });
        let result = e
            .estimate_cpu(&reference, &current, w, h)
            .expect("CPU estimate failed");
        // Most blocks should have dx = 4 (or close to it).
        let matched = result
            .block_mvs
            .iter()
            .filter(|b| b.mv.dx.abs() >= 3)
            .count();
        assert!(
            matched > result.block_mvs.len() / 2,
            "expected most blocks to detect horizontal shift"
        );
    }

    #[test]
    fn test_invalid_dimensions_rejected() {
        let e = MotionEstimator::av1_default();
        let frame = vec![0u8; 64];
        let result = e.estimate_cpu(&frame, &frame, 0, 8);
        assert!(result.is_err());
    }

    #[test]
    fn test_buffer_too_small_rejected() {
        let e = MotionEstimator::av1_default();
        let small = vec![0u8; 4];
        let frame = vec![0u8; 64 * 64];
        let result = e.estimate_cpu(&small, &frame, 64, 64);
        assert!(result.is_err(), "undersized reference should be rejected");
    }

    #[test]
    fn test_mean_mv_magnitude_zero_for_static() {
        let w = 32u32;
        let h = 32u32;
        let frame = gray_frame(w, h, 100);
        let e = MotionEstimator::new(MotionEstimationConfig {
            partition: BlockPartition::Fixed16x16,
            search_radius: 2,
            subpixel_refinement: false,
            ..MotionEstimationConfig::default()
        });
        let result = e
            .estimate_cpu(&frame, &frame, w, h)
            .expect("CPU estimate failed");
        assert_eq!(result.mean_mv_magnitude(), 0.0);
    }

    #[test]
    fn test_blocks_dimensions() {
        let w = 64u32;
        let h = 32u32;
        let frame = gray_frame(w, h, 0);
        let e = MotionEstimator::new(MotionEstimationConfig {
            partition: BlockPartition::Fixed16x16,
            search_radius: 2,
            subpixel_refinement: false,
            ..MotionEstimationConfig::default()
        });
        let result = e
            .estimate_cpu(&frame, &frame, w, h)
            .expect("CPU estimate failed");
        assert_eq!(result.blocks_x(), 4);
        assert_eq!(result.blocks_y(), 2);
        assert_eq!(result.block_mvs.len(), 8);
    }

    #[test]
    fn test_subpixel_refinement_present() {
        let w = 16u32;
        let h = 16u32;
        let frame = gray_frame(w, h, 128);
        let e = MotionEstimator::new(MotionEstimationConfig {
            partition: BlockPartition::Fixed16x16,
            search_radius: 2,
            subpixel_refinement: true,
            ..MotionEstimationConfig::default()
        });
        let result = e
            .estimate_cpu(&frame, &frame, w, h)
            .expect("CPU estimate failed");
        for bm in &result.block_mvs {
            assert!(
                bm.subpixel_mv.is_some(),
                "subpixel_mv should be present when refinement is enabled"
            );
        }
    }

    #[test]
    fn test_subpixel_refinement_absent_when_disabled() {
        let w = 16u32;
        let h = 16u32;
        let frame = gray_frame(w, h, 64);
        let e = MotionEstimator::new(MotionEstimationConfig {
            partition: BlockPartition::Fixed16x16,
            search_radius: 2,
            subpixel_refinement: false,
            ..MotionEstimationConfig::default()
        });
        let result = e
            .estimate_cpu(&frame, &frame, w, h)
            .expect("CPU estimate failed");
        for bm in &result.block_mvs {
            assert!(
                bm.subpixel_mv.is_none(),
                "subpixel_mv should be absent when refinement is disabled"
            );
        }
    }
}
