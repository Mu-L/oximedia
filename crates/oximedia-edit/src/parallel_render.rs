//! Parallel / multi-threaded rendering for the timeline editor.
//!
//! Splits the total frame range into [`RenderChunk`]s and processes them
//! concurrently using `rayon` (on native targets) or sequentially (on WASM).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

use crate::error::EditResult;
use crate::render::RenderConfig;
use crate::timeline::Timeline;

// ─────────────────────────────────────────────────────────────────────────────
// RenderChunk
// ─────────────────────────────────────────────────────────────────────────────

/// A contiguous range of frames assigned to one render worker.
#[derive(Clone, Debug)]
pub struct RenderChunk {
    /// Zero-based chunk index.
    pub chunk_id: usize,
    /// First frame of the chunk (inclusive).
    pub frame_start: u64,
    /// Last frame of the chunk (exclusive).
    pub frame_end: u64,
    /// Optional path for writing the chunk output.
    pub output_path: Option<PathBuf>,
}

impl RenderChunk {
    /// Create a new render chunk without an output path.
    #[must_use]
    pub fn new(chunk_id: usize, frame_start: u64, frame_end: u64) -> Self {
        Self {
            chunk_id,
            frame_start,
            frame_end,
            output_path: None,
        }
    }

    /// Create a render chunk with an explicit output path.
    #[must_use]
    pub fn with_output(
        chunk_id: usize,
        frame_start: u64,
        frame_end: u64,
        output_path: PathBuf,
    ) -> Self {
        Self {
            chunk_id,
            frame_start,
            frame_end,
            output_path: Some(output_path),
        }
    }

    /// Number of frames in this chunk.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_end.saturating_sub(self.frame_start)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ParallelRenderConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the parallel renderer.
#[derive(Clone, Debug)]
pub struct ParallelRenderConfig {
    /// Number of worker threads (ignored on WASM).
    pub num_threads: usize,
    /// Target number of frames per chunk.
    pub chunk_size: u64,
    /// Per-frame render configuration.
    pub render_config: RenderConfig,
}

impl ParallelRenderConfig {
    /// Create a configuration with sensible defaults (4 threads, 30-frame chunks).
    #[must_use]
    pub fn new(render_config: RenderConfig) -> Self {
        Self {
            num_threads: 4,
            chunk_size: 30,
            render_config,
        }
    }

    /// Override the number of worker threads.
    #[must_use]
    pub fn with_threads(mut self, n: usize) -> Self {
        self.num_threads = n.max(1);
        self
    }

    /// Override the chunk size in frames.
    #[must_use]
    pub fn with_chunk_size(mut self, size: u64) -> Self {
        self.chunk_size = size.max(1);
        self
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ParallelRenderResult
// ─────────────────────────────────────────────────────────────────────────────

/// Result produced by rendering a single [`RenderChunk`].
#[derive(Clone, Debug)]
pub struct ParallelRenderResult {
    /// The chunk that was rendered.
    pub chunk: RenderChunk,
    /// Number of frames that were successfully rendered.
    pub frames_rendered: u64,
    /// Wall-clock duration of this chunk's render, in milliseconds.
    pub duration_ms: u64,
    /// Whether the chunk completed without errors.
    pub success: bool,
    /// Error message, populated when `success` is `false`.
    pub error: Option<String>,
}

impl ParallelRenderResult {
    /// Build a successful result.
    #[must_use]
    fn ok(chunk: RenderChunk, frames_rendered: u64, duration_ms: u64) -> Self {
        Self {
            frames_rendered,
            duration_ms,
            success: true,
            error: None,
            chunk,
        }
    }

    /// Build a failed result.
    #[must_use]
    fn err(chunk: RenderChunk, message: impl Into<String>) -> Self {
        Self {
            frames_rendered: 0,
            duration_ms: 0,
            success: false,
            error: Some(message.into()),
            chunk,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ParallelRenderer
// ─────────────────────────────────────────────────────────────────────────────

/// Splits a timeline into frame chunks and renders them in parallel.
///
/// On WASM targets rayon is unavailable, so chunks are processed sequentially
/// to keep the API surface identical across platforms.
pub struct ParallelRenderer {
    /// Render configuration.
    pub config: ParallelRenderConfig,
}

impl ParallelRenderer {
    /// Create a new parallel renderer.
    #[must_use]
    pub fn new(config: ParallelRenderConfig) -> Self {
        Self { config }
    }

    /// Split `total_frames` into a list of [`RenderChunk`]s.
    ///
    /// Each chunk covers at most `config.chunk_size` frames.  The final chunk
    /// may be smaller.
    #[must_use]
    pub fn split_chunks(&self, total_frames: u64) -> Vec<RenderChunk> {
        if total_frames == 0 {
            return Vec::new();
        }

        let chunk_size = self.config.chunk_size;
        let num_chunks = (total_frames + chunk_size - 1) / chunk_size; // ceil division

        (0..num_chunks)
            .map(|i| {
                let start = i * chunk_size;
                let end = (start + chunk_size).min(total_frames);
                RenderChunk::new(i as usize, start, end)
            })
            .collect()
    }

    /// Render `total_frames` from `timeline` in parallel.
    ///
    /// Returns a result per chunk.  Individual chunk failures do not abort the
    /// remaining chunks — errors are captured in [`ParallelRenderResult::error`].
    pub fn render_parallel(
        &self,
        total_frames: u64,
        timeline: &Arc<Timeline>,
    ) -> EditResult<Vec<ParallelRenderResult>> {
        let chunks = self.split_chunks(total_frames);

        #[cfg(not(target_arch = "wasm32"))]
        let results: Vec<ParallelRenderResult> = {
            // Configure rayon thread pool inline so we respect `num_threads`.
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(self.config.num_threads)
                .build()
                .unwrap_or_else(|_| {
                    rayon::ThreadPoolBuilder::new()
                        .build()
                        .expect("default rayon pool build should never fail")
                });

            pool.install(|| {
                chunks
                    .par_iter()
                    .map(|chunk| self.render_chunk(chunk, timeline))
                    .collect()
            })
        };

        #[cfg(target_arch = "wasm32")]
        let results: Vec<ParallelRenderResult> = chunks
            .iter()
            .map(|chunk| self.render_chunk(chunk, timeline))
            .collect();

        Ok(results)
    }

    /// Render a single [`RenderChunk`].
    ///
    /// The current implementation counts frames and records timings; actual
    /// pixel decoding is handled by the timeline's render pipeline.
    pub fn render_chunk(
        &self,
        chunk: &RenderChunk,
        _timeline: &Arc<Timeline>,
    ) -> ParallelRenderResult {
        let start_time = Instant::now();

        // Validate the chunk range
        if chunk.frame_end < chunk.frame_start {
            return ParallelRenderResult::err(
                chunk.clone(),
                format!(
                    "invalid chunk {}: frame_end {} < frame_start {}",
                    chunk.chunk_id, chunk.frame_end, chunk.frame_start
                ),
            );
        }

        let frames_rendered = chunk.frame_count();

        // In a full implementation this loop would call the async renderer for
        // each frame and write pixel data to `chunk.output_path`.  Here we
        // iterate over frame indices to represent the work without touching I/O.
        for _frame in chunk.frame_start..chunk.frame_end {
            // Placeholder: per-frame render work would go here.
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;
        ParallelRenderResult::ok(chunk.clone(), frames_rendered, duration_ms)
    }

    /// Total number of frames that would be rendered given `total_frames`.
    #[must_use]
    pub fn total_frames_for(&self, total_frames: u64) -> u64 {
        self.split_chunks(total_frames)
            .iter()
            .map(RenderChunk::frame_count)
            .sum()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Timeline;

    fn make_renderer() -> ParallelRenderer {
        let cfg = ParallelRenderConfig::new(RenderConfig::default())
            .with_threads(2)
            .with_chunk_size(10);
        ParallelRenderer::new(cfg)
    }

    #[test]
    fn test_split_chunks_exact_multiple() {
        let r = make_renderer();
        let chunks = r.split_chunks(30);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].frame_start, 0);
        assert_eq!(chunks[0].frame_end, 10);
        assert_eq!(chunks[1].frame_start, 10);
        assert_eq!(chunks[1].frame_end, 20);
        assert_eq!(chunks[2].frame_start, 20);
        assert_eq!(chunks[2].frame_end, 30);
    }

    #[test]
    fn test_split_chunks_non_multiple() {
        let r = make_renderer();
        let chunks = r.split_chunks(25);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[2].frame_end, 25);
        assert_eq!(chunks[2].frame_count(), 5);
    }

    #[test]
    fn test_split_chunks_zero_frames() {
        let r = make_renderer();
        let chunks = r.split_chunks(0);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_render_parallel_all_succeed() {
        let r = make_renderer();
        let timeline = Arc::new(Timeline::default());
        let results = r
            .render_parallel(30, &timeline)
            .expect("render_parallel ok");
        assert_eq!(results.len(), 3);
        for res in &results {
            assert!(
                res.success,
                "chunk {} failed: {:?}",
                res.chunk.chunk_id, res.error
            );
            assert_eq!(res.frames_rendered, 10);
        }
    }

    #[test]
    fn test_config_builder() {
        let cfg = ParallelRenderConfig::new(RenderConfig::default())
            .with_threads(8)
            .with_chunk_size(50);
        assert_eq!(cfg.num_threads, 8);
        assert_eq!(cfg.chunk_size, 50);
    }

    #[test]
    fn test_render_chunk_frame_count() {
        let r = make_renderer();
        let timeline = Arc::new(Timeline::default());
        let chunk = RenderChunk::new(0, 0, 15);
        let result = r.render_chunk(&chunk, &timeline);
        assert!(result.success);
        assert_eq!(result.frames_rendered, 15);
    }

    #[test]
    fn test_total_frames_for() {
        let r = make_renderer();
        assert_eq!(r.total_frames_for(30), 30);
        assert_eq!(r.total_frames_for(25), 25);
        assert_eq!(r.total_frames_for(0), 0);
    }
}
