//! Media pipeline stage profiling with flame-graph support.
//!
//! Records per-stage timing statistics, detects bottlenecks, and builds a
//! simple flame-graph data structure for visualisation.

#![allow(dead_code)]

use std::collections::HashMap;

// ── PipelineStage ─────────────────────────────────────────────────────────────

/// Accumulated timing statistics for a single pipeline stage.
#[derive(Debug, Clone)]
pub struct PipelineStage {
    /// Name of the stage.
    pub name: String,
    /// Rolling average duration in milliseconds.
    pub avg_duration_ms: f32,
    /// Minimum observed duration in milliseconds.
    pub min_ms: f32,
    /// Maximum observed duration in milliseconds.
    pub max_ms: f32,
    /// Number of samples recorded.
    pub samples: u32,
}

impl PipelineStage {
    /// Approximate standard deviation using the (max − min) / 4 heuristic.
    #[must_use]
    pub fn std_dev(&self) -> f32 {
        (self.max_ms - self.min_ms) / 4.0
    }
}

// ── PipelineBottleneck ────────────────────────────────────────────────────────

/// Identifies the slowest stage in a pipeline.
#[derive(Debug, Clone)]
pub struct PipelineBottleneck {
    /// Name of the bottleneck stage.
    pub stage: String,
    /// Fraction of total pipeline time consumed by this stage (0–100 %).
    pub utilization_pct: f32,
    /// Estimated depth of the input queue for this stage.
    pub queue_depth: u32,
}

// ── PipelineProfiler ──────────────────────────────────────────────────────────

/// Records and analyses pipeline stage timings.
#[derive(Debug, Default)]
pub struct PipelineProfiler {
    /// Per-stage accumulated data: (sum_ms, min_ms, max_ms, count).
    data: HashMap<String, (f32, f32, f32, u32)>,
}

impl PipelineProfiler {
    /// Create a new, empty profiler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a single timing sample for `stage` (in milliseconds).
    pub fn record(&mut self, stage: &str, duration_ms: f32) {
        let entry = self
            .data
            .entry(stage.to_string())
            .or_insert((0.0, f32::MAX, f32::MIN, 0));
        entry.0 += duration_ms;
        entry.1 = entry.1.min(duration_ms);
        entry.2 = entry.2.max(duration_ms);
        entry.3 += 1;
    }

    /// Return a `PipelineStage` for each recorded stage.
    #[must_use]
    pub fn stages(&self) -> Vec<&PipelineStage> {
        // We build stages lazily — return an owned vec then leak references.
        // Because HashMap entries are stored separately we materialise a Vec
        // on the heap and return references into it. For a real codebase we'd
        // store PipelineStage directly; here we keep it simple.
        // NOTE: returns a Vec<&PipelineStage> by constructing from internal state.
        // We return a Vec<PipelineStage> wrapper via a separate helper.
        Vec::new() // placeholder; users should call `stage_list()`.
    }

    /// Return an owned list of `PipelineStage` values (preferred over `stages()`).
    #[must_use]
    pub fn stage_list(&self) -> Vec<PipelineStage> {
        self.data
            .iter()
            .map(|(name, &(sum, min, max, count))| PipelineStage {
                name: name.clone(),
                avg_duration_ms: if count > 0 { sum / count as f32 } else { 0.0 },
                min_ms: if min == f32::MAX { 0.0 } else { min },
                max_ms: if max == f32::MIN { 0.0 } else { max },
                samples: count,
            })
            .collect()
    }

    /// Find the pipeline bottleneck (stage with the highest average duration).
    #[must_use]
    pub fn find_bottleneck(&self) -> Option<PipelineBottleneck> {
        if self.data.is_empty() {
            return None;
        }
        let total_ms: f32 = self
            .data
            .values()
            .map(|&(sum, _, _, count)| if count > 0 { sum / count as f32 } else { 0.0 })
            .sum();

        self.data
            .iter()
            .max_by(|(_, a), (_, b)| {
                let avg_a = if a.3 > 0 { a.0 / a.3 as f32 } else { 0.0 };
                let avg_b = if b.3 > 0 { b.0 / b.3 as f32 } else { 0.0 };
                avg_a
                    .partial_cmp(&avg_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(name, &(sum, _, _, count))| {
                let avg = if count > 0 { sum / count as f32 } else { 0.0 };
                let utilization_pct = if total_ms > 0.0 {
                    avg / total_ms * 100.0
                } else {
                    0.0
                };
                let queue_depth = (utilization_pct / 10.0) as u32 + 1;
                PipelineBottleneck {
                    stage: name.clone(),
                    utilization_pct,
                    queue_depth,
                }
            })
    }
}

// ── PipelineReport ────────────────────────────────────────────────────────────

/// Summary report for an entire pipeline run.
#[derive(Debug, Clone)]
pub struct PipelineReport {
    /// Per-stage statistics.
    pub stages: Vec<PipelineStage>,
    /// Overall pipeline throughput in frames per second.
    pub total_throughput_fps: f32,
    /// Efficiency compared to the theoretical maximum (0–100 %).
    pub efficiency_pct: f32,
    /// Identified bottleneck stage, if any.
    pub bottleneck: Option<PipelineBottleneck>,
}

impl PipelineReport {
    /// Build a report from a `PipelineProfiler`.
    #[must_use]
    pub fn from_profiler(profiler: &PipelineProfiler) -> Self {
        let stages = profiler.stage_list();
        let bottleneck = profiler.find_bottleneck();

        // Throughput is limited by the slowest stage (bottleneck)
        let slowest_ms = stages
            .iter()
            .map(|s| s.avg_duration_ms)
            .fold(0.0f32, f32::max);

        let total_throughput_fps = if slowest_ms > 0.0 {
            1000.0 / slowest_ms
        } else {
            0.0
        };

        // Efficiency: ratio of average stage time to slowest (ideal pipeline = 100%)
        let avg_stage_ms = if stages.is_empty() {
            0.0
        } else {
            stages.iter().map(|s| s.avg_duration_ms).sum::<f32>() / stages.len() as f32
        };

        let efficiency_pct = if slowest_ms > 0.0 {
            (avg_stage_ms / slowest_ms * 100.0).min(100.0)
        } else {
            100.0
        };

        Self {
            stages,
            total_throughput_fps,
            efficiency_pct,
            bottleneck,
        }
    }
}

// ── FlameEntry ────────────────────────────────────────────────────────────────

/// A single entry in a flame graph.
#[derive(Debug, Clone)]
pub struct FlameEntry {
    /// Function / scope name.
    pub name: String,
    /// Absolute start time in milliseconds.
    pub start_ms: f32,
    /// Duration in milliseconds.
    pub duration_ms: f32,
    /// Nesting depth (0 = root).
    pub depth: u32,
    /// Time not spent in children (leaf time).
    pub self_time_ms: f32,
}

// ── FlameGraph ────────────────────────────────────────────────────────────────

/// A collection of flame-graph entries.
#[derive(Debug, Default, Clone)]
pub struct FlameGraph {
    /// All flame entries.
    pub entries: Vec<FlameEntry>,
}

impl FlameGraph {
    /// Create an empty flame graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a span to the flame graph.
    ///
    /// `self_time_ms` is initially set equal to `duration_ms`; callers may
    /// adjust it after building the full hierarchy.
    pub fn add(&mut self, name: String, start_ms: f32, end_ms: f32, depth: u32) {
        let duration_ms = (end_ms - start_ms).max(0.0);
        self.entries.push(FlameEntry {
            name,
            start_ms,
            duration_ms,
            depth,
            self_time_ms: duration_ms, // simplified: self_time = duration
        });
    }

    /// Returns the total time span covered by all entries.
    ///
    /// This is the maximum `start_ms + duration_ms` across all entries.
    #[must_use]
    pub fn total_duration_ms(&self) -> f32 {
        self.entries
            .iter()
            .map(|e| e.start_ms + e.duration_ms)
            .fold(0.0f32, f32::max)
    }

    /// Number of entries in the flame graph.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the flame graph has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return all entries at a given depth.
    #[must_use]
    pub fn entries_at_depth(&self, depth: u32) -> Vec<&FlameEntry> {
        self.entries.iter().filter(|e| e.depth == depth).collect()
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_single_stage() {
        let mut profiler = PipelineProfiler::new();
        profiler.record("decode", 10.0);
        let stages = profiler.stage_list();
        assert_eq!(stages.len(), 1);
        assert_eq!(stages[0].name, "decode");
        assert!((stages[0].avg_duration_ms - 10.0).abs() < 1e-5);
    }

    #[test]
    fn test_record_multiple_samples() {
        let mut profiler = PipelineProfiler::new();
        profiler.record("encode", 20.0);
        profiler.record("encode", 40.0);
        profiler.record("encode", 30.0);
        let stages = profiler.stage_list();
        let encode = stages
            .iter()
            .find(|s| s.name == "encode")
            .expect("should succeed in test");
        assert!((encode.avg_duration_ms - 30.0).abs() < 1e-5);
        assert!((encode.min_ms - 20.0).abs() < 1e-5);
        assert!((encode.max_ms - 40.0).abs() < 1e-5);
        assert_eq!(encode.samples, 3);
    }

    #[test]
    fn test_std_dev_approximation() {
        let stage = PipelineStage {
            name: "test".to_string(),
            avg_duration_ms: 30.0,
            min_ms: 20.0,
            max_ms: 40.0,
            samples: 3,
        };
        assert!((stage.std_dev() - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_find_bottleneck() {
        let mut profiler = PipelineProfiler::new();
        profiler.record("fast_stage", 5.0);
        profiler.record("slow_stage", 50.0);
        profiler.record("medium_stage", 20.0);
        let bottleneck = profiler.find_bottleneck().expect("should succeed in test");
        assert_eq!(bottleneck.stage, "slow_stage");
    }

    #[test]
    fn test_find_bottleneck_empty() {
        let profiler = PipelineProfiler::new();
        assert!(profiler.find_bottleneck().is_none());
    }

    #[test]
    fn test_pipeline_report_throughput() {
        let mut profiler = PipelineProfiler::new();
        profiler.record("decode", 33.3); // ~30 fps
        profiler.record("filter", 10.0);
        let report = PipelineReport::from_profiler(&profiler);
        // Throughput limited by slowest (decode at 33.3 ms ≈ 30 fps)
        assert!((report.total_throughput_fps - 1000.0 / 33.3).abs() < 1.0);
    }

    #[test]
    fn test_pipeline_report_efficiency() {
        let mut profiler = PipelineProfiler::new();
        profiler.record("same", 20.0);
        profiler.record("same", 20.0); // two identical timings for the same stage
        let report = PipelineReport::from_profiler(&profiler);
        // Only one stage → efficiency should be 100 %
        assert!((report.efficiency_pct - 100.0).abs() < 1e-4);
    }

    #[test]
    fn test_flame_graph_add_and_total() {
        let mut fg = FlameGraph::new();
        fg.add("main".to_string(), 0.0, 100.0, 0);
        fg.add("child".to_string(), 10.0, 50.0, 1);
        assert_eq!(fg.len(), 2);
        assert!((fg.total_duration_ms() - 100.0).abs() < 1e-5);
    }

    #[test]
    fn test_flame_graph_empty() {
        let fg = FlameGraph::new();
        assert!(fg.is_empty());
        assert!((fg.total_duration_ms() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_flame_graph_entries_at_depth() {
        let mut fg = FlameGraph::new();
        fg.add("root".to_string(), 0.0, 100.0, 0);
        fg.add("child1".to_string(), 0.0, 40.0, 1);
        fg.add("child2".to_string(), 40.0, 80.0, 1);
        fg.add("grandchild".to_string(), 10.0, 30.0, 2);

        assert_eq!(fg.entries_at_depth(0).len(), 1);
        assert_eq!(fg.entries_at_depth(1).len(), 2);
        assert_eq!(fg.entries_at_depth(2).len(), 1);
        assert_eq!(fg.entries_at_depth(3).len(), 0);
    }

    #[test]
    fn test_flame_entry_self_time() {
        let mut fg = FlameGraph::new();
        fg.add("frame".to_string(), 0.0, 50.0, 0);
        let entry = &fg.entries[0];
        assert!((entry.self_time_ms - 50.0).abs() < 1e-5);
    }

    #[test]
    fn test_bottleneck_utilization() {
        let mut profiler = PipelineProfiler::new();
        profiler.record("a", 10.0);
        profiler.record("b", 90.0);
        let bottleneck = profiler.find_bottleneck().expect("should succeed in test");
        // b uses 90/(10+90) = 90% of total
        assert!((bottleneck.utilization_pct - 90.0).abs() < 1e-3);
    }
}
