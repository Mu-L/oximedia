#![allow(dead_code)]
//! Render statistics collection and aggregation for the encoding farm.
//!
//! Tracks per-job and per-worker encoding metrics such as throughput
//! (frames/second), bitrate, quality scores, and wall-clock durations.
//! These statistics feed into the coordinator dashboard and are used by
//! the scheduler for capacity-planning decisions.

use std::collections::HashMap;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Per-task snapshot
// ---------------------------------------------------------------------------

/// Statistics for a single render task.
#[derive(Debug, Clone)]
pub struct TaskStats {
    /// Unique task identifier.
    pub task_id: String,
    /// Total frames rendered.
    pub frames_rendered: u64,
    /// Total bytes written to output.
    pub bytes_written: u64,
    /// Wall-clock elapsed time.
    pub wall_time: Duration,
    /// Average encoding speed in frames per second.
    pub fps: f64,
    /// Average output bitrate in kbps.
    pub bitrate_kbps: f64,
    /// Optional quality metric (e.g. VMAF).
    pub quality_score: Option<f64>,
}

/// Aggregated statistics across multiple tasks.
#[derive(Debug, Clone, Default)]
pub struct AggregateStats {
    /// Number of tasks included.
    pub task_count: u64,
    /// Total frames across all tasks.
    pub total_frames: u64,
    /// Total bytes across all tasks.
    pub total_bytes: u64,
    /// Total wall-clock time.
    pub total_wall_time: Duration,
    /// Mean fps.
    pub mean_fps: f64,
    /// Mean bitrate kbps.
    pub mean_bitrate_kbps: f64,
    /// Mean quality score (only over tasks that reported one).
    pub mean_quality: Option<f64>,
}

// ---------------------------------------------------------------------------
// Live tracker
// ---------------------------------------------------------------------------

/// A live tracker that accumulates task statistics during a render session.
#[derive(Debug)]
pub struct RenderStatsTracker {
    /// When the tracker was created.
    start: Instant,
    /// Per-task records.
    records: Vec<TaskStats>,
    /// Per-worker accumulated frame counts.
    worker_frames: HashMap<String, u64>,
}

impl RenderStatsTracker {
    /// Create a new tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            records: Vec::new(),
            worker_frames: HashMap::new(),
        }
    }

    /// Record a completed task.
    pub fn record_task(&mut self, stats: TaskStats) {
        self.records.push(stats);
    }

    /// Record a completed task and attribute its frames to a worker.
    pub fn record_task_for_worker(&mut self, worker_id: &str, stats: TaskStats) {
        *self.worker_frames.entry(worker_id.to_string()).or_insert(0) += stats.frames_rendered;
        self.records.push(stats);
    }

    /// Total tasks recorded so far.
    #[must_use]
    pub fn task_count(&self) -> usize {
        self.records.len()
    }

    /// Elapsed time since the tracker was created.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Total frames across all recorded tasks.
    #[must_use]
    pub fn total_frames(&self) -> u64 {
        self.records.iter().map(|r| r.frames_rendered).sum()
    }

    /// Total bytes written across all recorded tasks.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.records.iter().map(|r| r.bytes_written).sum()
    }

    /// Compute aggregate statistics.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn aggregate(&self) -> AggregateStats {
        if self.records.is_empty() {
            return AggregateStats::default();
        }
        let n = self.records.len() as f64;
        let total_frames: u64 = self.records.iter().map(|r| r.frames_rendered).sum();
        let total_bytes: u64 = self.records.iter().map(|r| r.bytes_written).sum();
        let total_wall: Duration = self.records.iter().map(|r| r.wall_time).sum();
        let mean_fps: f64 = self.records.iter().map(|r| r.fps).sum::<f64>() / n;
        let mean_br: f64 = self.records.iter().map(|r| r.bitrate_kbps).sum::<f64>() / n;
        let quality_records: Vec<f64> = self
            .records
            .iter()
            .filter_map(|r| r.quality_score)
            .collect();
        let mean_quality = if quality_records.is_empty() {
            None
        } else {
            Some(quality_records.iter().sum::<f64>() / quality_records.len() as f64)
        };

        AggregateStats {
            task_count: self.records.len() as u64,
            total_frames,
            total_bytes,
            total_wall_time: total_wall,
            mean_fps,
            mean_bitrate_kbps: mean_br,
            mean_quality,
        }
    }

    /// Return per-worker frame counts.
    #[must_use]
    pub fn worker_frame_counts(&self) -> &HashMap<String, u64> {
        &self.worker_frames
    }

    /// Clear all records.
    pub fn reset(&mut self) {
        self.records.clear();
        self.worker_frames.clear();
        self.start = Instant::now();
    }
}

impl Default for RenderStatsTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Throughput calculator
// ---------------------------------------------------------------------------

/// Calculate encoding throughput in frames per second.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn compute_fps(frames: u64, duration: Duration) -> f64 {
    let secs = duration.as_secs_f64();
    if secs <= 0.0 {
        return 0.0;
    }
    frames as f64 / secs
}

/// Calculate average bitrate in kbps.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn compute_bitrate_kbps(bytes: u64, duration: Duration) -> f64 {
    let secs = duration.as_secs_f64();
    if secs <= 0.0 {
        return 0.0;
    }
    (bytes as f64 * 8.0) / (secs * 1000.0)
}

/// Estimate remaining time given current progress.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn estimate_remaining(
    completed_frames: u64,
    total_frames: u64,
    elapsed: Duration,
) -> Option<Duration> {
    if completed_frames == 0 || total_frames == 0 {
        return None;
    }
    if completed_frames >= total_frames {
        return Some(Duration::ZERO);
    }
    let secs = elapsed.as_secs_f64();
    let fps = completed_frames as f64 / secs;
    if fps <= 0.0 {
        return None;
    }
    let remaining_frames = total_frames - completed_frames;
    let remaining_secs = remaining_frames as f64 / fps;
    Some(Duration::from_secs_f64(remaining_secs))
}

// ---------------------------------------------------------------------------
// Historical Render Time Prediction
// ---------------------------------------------------------------------------

/// A historical record of a completed render task, used for building
/// predictive models.
#[derive(Debug, Clone)]
pub struct HistoricalRecord {
    /// Job type identifier (e.g., "h264_1080p", "av1_4k").
    pub job_type: String,
    /// Worker identifier that processed the task.
    pub worker_id: String,
    /// Number of frames in the task.
    pub frame_count: u64,
    /// Total bytes of output.
    pub output_bytes: u64,
    /// Actual wall-clock duration.
    pub actual_duration: Duration,
    /// Timestamp when the record was created.
    pub recorded_at: Instant,
}

impl HistoricalRecord {
    /// Create a new historical record.
    #[must_use]
    pub fn new(
        job_type: impl Into<String>,
        worker_id: impl Into<String>,
        frame_count: u64,
        output_bytes: u64,
        actual_duration: Duration,
    ) -> Self {
        Self {
            job_type: job_type.into(),
            worker_id: worker_id.into(),
            frame_count,
            output_bytes,
            actual_duration,
            recorded_at: Instant::now(),
        }
    }

    /// Frames per second achieved by this record.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn fps(&self) -> f64 {
        let secs = self.actual_duration.as_secs_f64();
        if secs <= 0.0 {
            0.0
        } else {
            self.frame_count as f64 / secs
        }
    }

    /// Seconds per frame.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn seconds_per_frame(&self) -> f64 {
        if self.frame_count == 0 {
            0.0
        } else {
            self.actual_duration.as_secs_f64() / self.frame_count as f64
        }
    }
}

/// A prediction result with confidence interval.
#[derive(Debug, Clone)]
pub struct RenderPrediction {
    /// Estimated wall-clock duration.
    pub estimated_duration: Duration,
    /// Lower bound of the 90% confidence interval.
    pub lower_bound: Duration,
    /// Upper bound of the 90% confidence interval.
    pub upper_bound: Duration,
    /// Number of historical records used for the prediction.
    pub sample_count: usize,
    /// Confidence score from 0.0 (no data) to 1.0 (many samples, low variance).
    pub confidence: f64,
}

/// Capacity planning snapshot for a specific job type.
#[derive(Debug, Clone)]
pub struct CapacitySnapshot {
    /// Job type identifier.
    pub job_type: String,
    /// Average frames per second across all workers.
    pub avg_fps: f64,
    /// Fastest worker FPS.
    pub max_fps: f64,
    /// Slowest worker FPS.
    pub min_fps: f64,
    /// Number of distinct workers with history.
    pub worker_count: usize,
    /// Total historical samples.
    pub sample_count: usize,
    /// Estimated throughput: total frames per hour the farm can process
    /// given the historical average.
    pub estimated_frames_per_hour: f64,
}

/// Historical render time prediction engine.
///
/// Collects historical records of completed render tasks and uses them to
/// predict future render times with confidence intervals.  This feeds into
/// capacity planning by providing throughput estimates per job type and worker.
#[derive(Debug)]
pub struct RenderTimePredictor {
    /// All historical records.
    records: Vec<HistoricalRecord>,
    /// Maximum number of records to retain (oldest are pruned). 0 = unlimited.
    max_records: usize,
    /// Exponential smoothing factor for the EWMA predictor (0.0 to 1.0).
    /// Higher values weight recent observations more heavily.
    alpha: f64,
}

impl RenderTimePredictor {
    /// Create a new predictor with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            max_records: 10_000,
            alpha: 0.3,
        }
    }

    /// Set the maximum number of records to retain.
    #[must_use]
    pub fn with_max_records(mut self, max: usize) -> Self {
        self.max_records = max;
        self
    }

    /// Set the EWMA smoothing factor.
    #[must_use]
    pub fn with_alpha(mut self, alpha: f64) -> Self {
        self.alpha = alpha.clamp(0.01, 0.99);
        self
    }

    /// Add a historical record. Old records are pruned if `max_records` is exceeded.
    pub fn add_record(&mut self, record: HistoricalRecord) {
        self.records.push(record);
        if self.max_records > 0 && self.records.len() > self.max_records {
            // Remove oldest records
            let excess = self.records.len() - self.max_records;
            self.records.drain(..excess);
        }
    }

    /// Total number of historical records.
    #[must_use]
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Predict the render time for a task with the given job type and frame count.
    ///
    /// Uses EWMA-weighted historical seconds-per-frame for matching job types.
    /// Falls back to all records if no job-type-specific data exists.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn predict(
        &self,
        job_type: &str,
        frame_count: u64,
        worker_id: Option<&str>,
    ) -> Option<RenderPrediction> {
        // Collect relevant seconds-per-frame values
        let spf_values: Vec<f64> = self
            .records
            .iter()
            .filter(|r| r.job_type == job_type)
            .filter(|r| worker_id.map_or(true, |w| r.worker_id == w))
            .filter(|r| r.frame_count > 0)
            .map(|r| r.seconds_per_frame())
            .collect();

        if spf_values.is_empty() {
            // Try broader fallback: any job type on the same worker
            if worker_id.is_some() {
                return self.predict(job_type, frame_count, None);
            }
            // Try global average
            let global: Vec<f64> = self
                .records
                .iter()
                .filter(|r| r.frame_count > 0)
                .map(|r| r.seconds_per_frame())
                .collect();
            if global.is_empty() {
                return None;
            }
            return self.build_prediction(&global, frame_count);
        }

        self.build_prediction(&spf_values, frame_count)
    }

    /// Build a prediction from a set of seconds-per-frame samples.
    #[allow(clippy::cast_precision_loss)]
    fn build_prediction(&self, spf_values: &[f64], frame_count: u64) -> Option<RenderPrediction> {
        if spf_values.is_empty() {
            return None;
        }

        let n = spf_values.len();

        // Compute EWMA of seconds-per-frame (more recent records weighted more)
        let ewma_spf = self.ewma(spf_values);

        // Compute standard deviation for confidence intervals
        let mean = spf_values.iter().sum::<f64>() / n as f64;
        let variance = if n > 1 {
            spf_values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64
        } else {
            0.0
        };
        let std_dev = variance.sqrt();

        // 90% confidence interval: ~1.645 * std_dev for normal distribution
        let z = 1.645;
        let margin = z * std_dev;

        let estimated_secs = ewma_spf * frame_count as f64;
        let lower_secs = ((ewma_spf - margin) * frame_count as f64).max(0.0);
        let upper_secs = (ewma_spf + margin) * frame_count as f64;

        // Confidence: based on sample count and variance
        let confidence = if n == 0 {
            0.0
        } else {
            let sample_factor = (n as f64 / 50.0).min(1.0); // More samples = higher confidence
            let cv = if mean > 0.0 { std_dev / mean } else { 1.0 }; // Coefficient of variation
            let cv_factor = (1.0 - cv.min(1.0)).max(0.0); // Lower variance = higher confidence
            (sample_factor * 0.6 + cv_factor * 0.4).min(1.0)
        };

        Some(RenderPrediction {
            estimated_duration: Duration::from_secs_f64(estimated_secs),
            lower_bound: Duration::from_secs_f64(lower_secs),
            upper_bound: Duration::from_secs_f64(upper_secs),
            sample_count: n,
            confidence,
        })
    }

    /// Compute the exponentially weighted moving average of a series.
    fn ewma(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }
        if values.len() == 1 {
            return values[0];
        }
        let mut ewma = values[0];
        for &v in &values[1..] {
            ewma = self.alpha * v + (1.0 - self.alpha) * ewma;
        }
        ewma
    }

    /// Get a capacity snapshot for a specific job type.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn capacity_snapshot(&self, job_type: &str) -> Option<CapacitySnapshot> {
        let relevant: Vec<&HistoricalRecord> = self
            .records
            .iter()
            .filter(|r| r.job_type == job_type && r.frame_count > 0)
            .collect();

        if relevant.is_empty() {
            return None;
        }

        let fps_values: Vec<f64> = relevant.iter().map(|r| r.fps()).collect();
        let n = fps_values.len();
        let avg_fps = fps_values.iter().sum::<f64>() / n as f64;
        let max_fps = fps_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_fps = fps_values.iter().cloned().fold(f64::INFINITY, f64::min);

        let worker_ids: std::collections::HashSet<&str> =
            relevant.iter().map(|r| r.worker_id.as_str()).collect();

        Some(CapacitySnapshot {
            job_type: job_type.to_string(),
            avg_fps,
            max_fps,
            min_fps,
            worker_count: worker_ids.len(),
            sample_count: n,
            estimated_frames_per_hour: avg_fps * 3600.0,
        })
    }

    /// Get per-worker performance breakdown for a job type.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn worker_performance(&self, job_type: &str) -> Vec<(String, f64, usize)> {
        let mut per_worker: HashMap<String, Vec<f64>> = HashMap::new();
        for r in &self.records {
            if r.job_type == job_type && r.frame_count > 0 {
                per_worker
                    .entry(r.worker_id.clone())
                    .or_default()
                    .push(r.fps());
            }
        }
        let mut result: Vec<(String, f64, usize)> = per_worker
            .into_iter()
            .map(|(worker, fps_vals)| {
                let n = fps_vals.len();
                let avg = fps_vals.iter().sum::<f64>() / n as f64;
                (worker, avg, n)
            })
            .collect();
        // Sort by average FPS descending
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        result
    }

    /// Clear all historical records.
    pub fn clear(&mut self) {
        self.records.clear();
    }
}

impl Default for RenderTimePredictor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stats(id: &str, frames: u64, bytes: u64, secs: u64) -> TaskStats {
        let wall = Duration::from_secs(secs);
        #[allow(clippy::cast_precision_loss)]
        let fps = if secs > 0 {
            frames as f64 / secs as f64
        } else {
            0.0
        };
        #[allow(clippy::cast_precision_loss)]
        let br = if secs > 0 {
            (bytes as f64 * 8.0) / (secs as f64 * 1000.0)
        } else {
            0.0
        };
        TaskStats {
            task_id: id.to_string(),
            frames_rendered: frames,
            bytes_written: bytes,
            wall_time: wall,
            fps,
            bitrate_kbps: br,
            quality_score: None,
        }
    }

    #[test]
    fn test_tracker_new() {
        let t = RenderStatsTracker::new();
        assert_eq!(t.task_count(), 0);
        assert_eq!(t.total_frames(), 0);
    }

    #[test]
    fn test_record_task() {
        let mut t = RenderStatsTracker::new();
        t.record_task(make_stats("t1", 100, 5000, 10));
        assert_eq!(t.task_count(), 1);
        assert_eq!(t.total_frames(), 100);
    }

    #[test]
    fn test_record_for_worker() {
        let mut t = RenderStatsTracker::new();
        t.record_task_for_worker("w1", make_stats("t1", 200, 8000, 20));
        t.record_task_for_worker("w1", make_stats("t2", 300, 12000, 30));
        t.record_task_for_worker("w2", make_stats("t3", 100, 4000, 10));
        assert_eq!(
            *t.worker_frame_counts()
                .get("w1")
                .expect("failed to get value"),
            500
        );
        assert_eq!(
            *t.worker_frame_counts()
                .get("w2")
                .expect("failed to get value"),
            100
        );
    }

    #[test]
    fn test_aggregate_empty() {
        let t = RenderStatsTracker::new();
        let agg = t.aggregate();
        assert_eq!(agg.task_count, 0);
        assert_eq!(agg.total_frames, 0);
    }

    #[test]
    fn test_aggregate_multiple() {
        let mut t = RenderStatsTracker::new();
        t.record_task(make_stats("t1", 100, 5000, 10));
        t.record_task(make_stats("t2", 200, 10000, 20));
        let agg = t.aggregate();
        assert_eq!(agg.task_count, 2);
        assert_eq!(agg.total_frames, 300);
        assert_eq!(agg.total_bytes, 15000);
    }

    #[test]
    fn test_aggregate_quality() {
        let mut t = RenderStatsTracker::new();
        let mut s1 = make_stats("t1", 100, 5000, 10);
        s1.quality_score = Some(90.0);
        let mut s2 = make_stats("t2", 100, 5000, 10);
        s2.quality_score = Some(80.0);
        t.record_task(s1);
        t.record_task(s2);
        let agg = t.aggregate();
        assert!(
            (agg.mean_quality.expect("mean_quality should be valid") - 85.0).abs() < f64::EPSILON
        );
    }

    #[test]
    fn test_reset() {
        let mut t = RenderStatsTracker::new();
        t.record_task(make_stats("t1", 100, 5000, 10));
        t.reset();
        assert_eq!(t.task_count(), 0);
        assert!(t.worker_frame_counts().is_empty());
    }

    #[test]
    fn test_compute_fps() {
        let fps = compute_fps(300, Duration::from_secs(10));
        assert!((fps - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_fps_zero_duration() {
        let fps = compute_fps(100, Duration::ZERO);
        assert!((fps - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_bitrate_kbps() {
        // 125_000 bytes in 1 second = 1_000_000 bits/s = 1000 kbps
        let br = compute_bitrate_kbps(125_000, Duration::from_secs(1));
        assert!((br - 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_estimate_remaining() {
        let est = estimate_remaining(50, 100, Duration::from_secs(10));
        let remaining = est.expect("est should be valid");
        // 50 frames in 10s => 5fps => 50 remaining => 10s
        assert!((remaining.as_secs_f64() - 10.0).abs() < 0.1);
    }

    #[test]
    fn test_estimate_remaining_complete() {
        let est = estimate_remaining(100, 100, Duration::from_secs(10));
        assert_eq!(est.expect("est should be valid"), Duration::ZERO);
    }

    #[test]
    fn test_estimate_remaining_zero() {
        assert!(estimate_remaining(0, 100, Duration::from_secs(10)).is_none());
    }

    // ── Historical Render Time Prediction ──────────────────────────────────

    fn make_record(
        job_type: &str,
        worker: &str,
        frames: u64,
        bytes: u64,
        secs: u64,
    ) -> HistoricalRecord {
        HistoricalRecord {
            job_type: job_type.to_string(),
            worker_id: worker.to_string(),
            frame_count: frames,
            output_bytes: bytes,
            actual_duration: Duration::from_secs(secs),
            recorded_at: Instant::now(),
        }
    }

    #[test]
    fn test_predictor_empty_returns_none() {
        let pred = RenderTimePredictor::new();
        assert!(pred.predict("h264_1080p", 1000, None).is_none());
    }

    #[test]
    fn test_predictor_single_record() {
        let mut pred = RenderTimePredictor::new();
        // 1000 frames in 100 seconds = 10 fps = 0.1 sec/frame
        pred.add_record(make_record("h264_1080p", "w1", 1000, 100_000, 100));

        let result = pred.predict("h264_1080p", 2000, None);
        assert!(result.is_some());
        let p = result.expect("prediction should exist");
        // Should estimate ~200 seconds for 2000 frames
        assert!(
            (p.estimated_duration.as_secs_f64() - 200.0).abs() < 1.0,
            "expected ~200s, got {}",
            p.estimated_duration.as_secs_f64()
        );
        assert_eq!(p.sample_count, 1);
    }

    #[test]
    fn test_predictor_multiple_records_ewma() {
        let mut pred = RenderTimePredictor::new().with_alpha(0.5);
        // Add records with increasing speed
        pred.add_record(make_record("h264", "w1", 1000, 50_000, 200)); // 0.2 s/f
        pred.add_record(make_record("h264", "w1", 1000, 50_000, 100)); // 0.1 s/f
        pred.add_record(make_record("h264", "w1", 1000, 50_000, 100)); // 0.1 s/f

        let result = pred
            .predict("h264", 1000, None)
            .expect("should have prediction");
        // EWMA should weight recent records more heavily
        // The prediction should be closer to 0.1 than to 0.2
        assert!(result.estimated_duration.as_secs_f64() < 150.0);
        assert_eq!(result.sample_count, 3);
    }

    #[test]
    fn test_predictor_worker_specific() {
        let mut pred = RenderTimePredictor::new();
        pred.add_record(make_record("h264", "fast_worker", 1000, 50_000, 50));
        pred.add_record(make_record("h264", "slow_worker", 1000, 50_000, 200));

        let fast = pred
            .predict("h264", 1000, Some("fast_worker"))
            .expect("should have prediction");
        let slow = pred
            .predict("h264", 1000, Some("slow_worker"))
            .expect("should have prediction");

        assert!(fast.estimated_duration < slow.estimated_duration);
    }

    #[test]
    fn test_predictor_confidence_interval() {
        let mut pred = RenderTimePredictor::new();
        // Add varied records to get a meaningful CI
        for secs in [90, 110, 95, 105, 100, 92, 108, 97, 103, 99] {
            pred.add_record(make_record("h264", "w1", 1000, 50_000, secs));
        }

        let result = pred
            .predict("h264", 1000, None)
            .expect("should have prediction");
        assert!(result.lower_bound < result.estimated_duration);
        assert!(result.upper_bound > result.estimated_duration);
        assert!(result.confidence > 0.0);
        assert!(result.confidence <= 1.0);
    }

    #[test]
    fn test_predictor_confidence_increases_with_samples() {
        let mut pred = RenderTimePredictor::new();
        pred.add_record(make_record("h264", "w1", 1000, 50_000, 100));
        let c1 = pred
            .predict("h264", 1000, None)
            .expect("should have prediction")
            .confidence;

        for _ in 0..50 {
            pred.add_record(make_record("h264", "w1", 1000, 50_000, 100));
        }
        let c2 = pred
            .predict("h264", 1000, None)
            .expect("should have prediction")
            .confidence;

        assert!(c2 >= c1, "more samples should increase confidence");
    }

    #[test]
    fn test_predictor_capacity_snapshot() {
        let mut pred = RenderTimePredictor::new();
        pred.add_record(make_record("h264", "w1", 1000, 50_000, 100)); // 10 fps
        pred.add_record(make_record("h264", "w2", 1000, 50_000, 50)); // 20 fps
        pred.add_record(make_record("h264", "w1", 2000, 100_000, 200)); // 10 fps

        let snapshot = pred
            .capacity_snapshot("h264")
            .expect("should have snapshot");
        assert_eq!(snapshot.job_type, "h264");
        assert_eq!(snapshot.sample_count, 3);
        assert_eq!(snapshot.worker_count, 2);
        assert!(snapshot.max_fps >= snapshot.min_fps);
        assert!(snapshot.estimated_frames_per_hour > 0.0);
    }

    #[test]
    fn test_predictor_worker_performance() {
        let mut pred = RenderTimePredictor::new();
        pred.add_record(make_record("h264", "fast", 1000, 50_000, 50));
        pred.add_record(make_record("h264", "fast", 2000, 100_000, 100));
        pred.add_record(make_record("h264", "slow", 1000, 50_000, 200));

        let perf = pred.worker_performance("h264");
        assert_eq!(perf.len(), 2);
        // Fast worker should be first (sorted by avg FPS desc)
        assert_eq!(perf[0].0, "fast");
        assert!(perf[0].1 > perf[1].1);
    }

    #[test]
    fn test_predictor_max_records_pruning() {
        let mut pred = RenderTimePredictor::new().with_max_records(5);
        for i in 0..10 {
            pred.add_record(make_record("h264", "w1", 1000, 50_000, 100 + i));
        }
        assert_eq!(pred.record_count(), 5);
    }

    #[test]
    fn test_predictor_fallback_global() {
        let mut pred = RenderTimePredictor::new();
        pred.add_record(make_record("av1", "w1", 1000, 50_000, 100));

        // Requesting prediction for "h264" but only "av1" records exist.
        // Should fall back to global average.
        let result = pred.predict("h264", 1000, None);
        assert!(result.is_some());
    }

    #[test]
    fn test_predictor_clear() {
        let mut pred = RenderTimePredictor::new();
        pred.add_record(make_record("h264", "w1", 1000, 50_000, 100));
        pred.clear();
        assert_eq!(pred.record_count(), 0);
        assert!(pred.predict("h264", 1000, None).is_none());
    }

    #[test]
    fn test_historical_record_fps() {
        let r = make_record("h264", "w1", 300, 10_000, 10);
        assert!((r.fps() - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_historical_record_seconds_per_frame() {
        let r = make_record("h264", "w1", 100, 10_000, 10);
        assert!((r.seconds_per_frame() - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_capacity_snapshot_none_for_unknown_type() {
        let pred = RenderTimePredictor::new();
        assert!(pred.capacity_snapshot("unknown").is_none());
    }

    #[test]
    fn test_predictor_with_alpha() {
        let pred = RenderTimePredictor::new().with_alpha(0.8);
        // Alpha should be clamped
        assert!((pred.alpha - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_predictor_alpha_clamping() {
        let pred = RenderTimePredictor::new().with_alpha(2.0);
        assert!(pred.alpha <= 0.99);
        let pred2 = RenderTimePredictor::new().with_alpha(-1.0);
        assert!(pred2.alpha >= 0.01);
    }
}
