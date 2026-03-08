//! Transcoding performance metrics collection and reporting.
//!
//! Provides `FrameMetric`, `MetricsSummary`, and `TranscodeMetricsCollector`
//! for recording and analysing per-frame and aggregate encode statistics.

#![allow(dead_code)]

/// Per-frame encoding metric captured during transcoding.
#[derive(Debug, Clone)]
pub struct FrameMetric {
    /// Frame index (0-based).
    pub frame_index: u64,
    /// Encode time for this frame in microseconds.
    pub encode_us: u64,
    /// Compressed size of this frame in bytes.
    pub compressed_bytes: u64,
    /// PSNR value for this frame (dB), if computed.
    pub psnr_db: Option<f64>,
}

impl FrameMetric {
    /// Creates a new frame metric.
    #[must_use]
    pub fn new(frame_index: u64, encode_us: u64, compressed_bytes: u64) -> Self {
        Self {
            frame_index,
            encode_us,
            compressed_bytes,
            psnr_db: None,
        }
    }

    /// Attaches a PSNR measurement.
    #[must_use]
    pub fn with_psnr(mut self, psnr_db: f64) -> Self {
        self.psnr_db = Some(psnr_db);
        self
    }

    /// Returns the instantaneous bitrate for this frame given a frame rate.
    ///
    /// `fps` is frames-per-second as a float.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn instantaneous_bitrate_bps(&self, fps: f64) -> f64 {
        self.compressed_bytes as f64 * 8.0 * fps
    }
}

/// Summary statistics over a collection of frame metrics.
#[derive(Debug, Clone)]
pub struct MetricsSummary {
    /// Total number of frames.
    pub frame_count: u64,
    /// Mean encode time per frame in microseconds.
    pub mean_encode_us: f64,
    /// Peak encode time in microseconds.
    pub peak_encode_us: u64,
    /// Total compressed bytes.
    pub total_bytes: u64,
    /// Mean PSNR in dB (None if not measured).
    pub mean_psnr_db: Option<f64>,
    /// Minimum PSNR in dB (None if not measured).
    pub min_psnr_db: Option<f64>,
}

impl MetricsSummary {
    /// Returns the mean bitrate in bits-per-second given input fps.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn mean_bitrate_bps(&self, fps: f64) -> f64 {
        if self.frame_count == 0 || fps <= 0.0 {
            return 0.0;
        }
        let total_bits = self.total_bytes as f64 * 8.0;
        let duration_secs = self.frame_count as f64 / fps;
        total_bits / duration_secs
    }

    /// Returns the encode throughput in frames per second.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn encode_fps(&self) -> f64 {
        if self.mean_encode_us <= 0.0 {
            return 0.0;
        }
        1_000_000.0 / self.mean_encode_us
    }
}

/// Collects frame-level metrics during a transcode session.
#[derive(Debug, Default)]
pub struct TranscodeMetricsCollector {
    metrics: Vec<FrameMetric>,
}

impl TranscodeMetricsCollector {
    /// Creates a new, empty collector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a collector with pre-allocated capacity.
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            metrics: Vec::with_capacity(cap),
        }
    }

    /// Records a frame metric.
    pub fn record(&mut self, metric: FrameMetric) {
        self.metrics.push(metric);
    }

    /// Returns the number of recorded frame metrics.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.metrics.len()
    }

    /// Returns `true` if no metrics have been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.metrics.is_empty()
    }

    /// Computes and returns a summary over all recorded metrics.
    #[allow(clippy::cast_precision_loss)]
    pub fn summarise(&self) -> MetricsSummary {
        let count = self.metrics.len() as u64;
        if count == 0 {
            return MetricsSummary {
                frame_count: 0,
                mean_encode_us: 0.0,
                peak_encode_us: 0,
                total_bytes: 0,
                mean_psnr_db: None,
                min_psnr_db: None,
            };
        }

        let total_encode_us: u64 = self.metrics.iter().map(|m| m.encode_us).sum();
        let peak_encode_us = self.metrics.iter().map(|m| m.encode_us).max().unwrap_or(0);
        let total_bytes: u64 = self.metrics.iter().map(|m| m.compressed_bytes).sum();

        let psnr_values: Vec<f64> = self.metrics.iter().filter_map(|m| m.psnr_db).collect();

        let mean_psnr_db = if psnr_values.is_empty() {
            None
        } else {
            Some(psnr_values.iter().sum::<f64>() / psnr_values.len() as f64)
        };

        let min_psnr_db = psnr_values.iter().copied().reduce(f64::min);

        MetricsSummary {
            frame_count: count,
            mean_encode_us: total_encode_us as f64 / count as f64,
            peak_encode_us,
            total_bytes,
            mean_psnr_db,
            min_psnr_db,
        }
    }

    /// Returns the worst (lowest) PSNR frame, if PSNR data is available.
    #[must_use]
    pub fn worst_psnr_frame(&self) -> Option<&FrameMetric> {
        self.metrics
            .iter()
            .filter(|m| m.psnr_db.is_some())
            .min_by(|a, b| {
                a.psnr_db
                    .expect("invariant: filter ensures psnr_db is Some")
                    .partial_cmp(
                        &b.psnr_db
                            .expect("invariant: filter ensures psnr_db is Some"),
                    )
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Returns the slowest (highest encode time) frame metric.
    #[must_use]
    pub fn slowest_frame(&self) -> Option<&FrameMetric> {
        self.metrics.iter().max_by_key(|m| m.encode_us)
    }

    /// Clears all recorded metrics.
    pub fn clear(&mut self) {
        self.metrics.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metric(index: u64, encode_us: u64, bytes: u64) -> FrameMetric {
        FrameMetric::new(index, encode_us, bytes)
    }

    #[test]
    fn test_frame_metric_creation() {
        let m = make_metric(0, 5000, 10_000);
        assert_eq!(m.frame_index, 0);
        assert_eq!(m.encode_us, 5000);
        assert_eq!(m.compressed_bytes, 10_000);
        assert!(m.psnr_db.is_none());
    }

    #[test]
    fn test_frame_metric_with_psnr() {
        let m = make_metric(0, 5000, 10_000).with_psnr(42.5);
        assert_eq!(m.psnr_db, Some(42.5));
    }

    #[test]
    fn test_instantaneous_bitrate() {
        let m = make_metric(0, 5000, 1000); // 1000 bytes at 30fps
        let bps = m.instantaneous_bitrate_bps(30.0);
        assert!((bps - 240_000.0).abs() < 0.01);
    }

    #[test]
    fn test_collector_empty() {
        let c = TranscodeMetricsCollector::new();
        assert!(c.is_empty());
        assert_eq!(c.frame_count(), 0);
    }

    #[test]
    fn test_collector_record_increments_count() {
        let mut c = TranscodeMetricsCollector::new();
        c.record(make_metric(0, 1000, 500));
        c.record(make_metric(1, 2000, 600));
        assert_eq!(c.frame_count(), 2);
        assert!(!c.is_empty());
    }

    #[test]
    fn test_summarise_empty() {
        let c = TranscodeMetricsCollector::new();
        let s = c.summarise();
        assert_eq!(s.frame_count, 0);
        assert_eq!(s.total_bytes, 0);
        assert!(s.mean_psnr_db.is_none());
    }

    #[test]
    fn test_summarise_mean_encode_us() {
        let mut c = TranscodeMetricsCollector::new();
        c.record(make_metric(0, 1000, 100));
        c.record(make_metric(1, 3000, 100));
        let s = c.summarise();
        assert!((s.mean_encode_us - 2000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_summarise_peak_encode_us() {
        let mut c = TranscodeMetricsCollector::new();
        c.record(make_metric(0, 1000, 100));
        c.record(make_metric(1, 5000, 200));
        let s = c.summarise();
        assert_eq!(s.peak_encode_us, 5000);
    }

    #[test]
    fn test_summarise_total_bytes() {
        let mut c = TranscodeMetricsCollector::new();
        c.record(make_metric(0, 1000, 400));
        c.record(make_metric(1, 1000, 600));
        let s = c.summarise();
        assert_eq!(s.total_bytes, 1000);
    }

    #[test]
    fn test_summarise_psnr() {
        let mut c = TranscodeMetricsCollector::new();
        c.record(make_metric(0, 100, 100).with_psnr(40.0));
        c.record(make_metric(1, 100, 100).with_psnr(44.0));
        let s = c.summarise();
        assert!((s.mean_psnr_db.expect("should succeed in test") - 42.0).abs() < 0.001);
        assert!((s.min_psnr_db.expect("should succeed in test") - 40.0).abs() < 0.001);
    }

    #[test]
    fn test_mean_bitrate_bps() {
        let mut c = TranscodeMetricsCollector::new();
        // 30 frames, each 1000 bytes at 30 fps → 1s → 240kbps
        for i in 0..30 {
            c.record(make_metric(i, 1000, 1000));
        }
        let s = c.summarise();
        let bps = s.mean_bitrate_bps(30.0);
        assert!((bps - 240_000.0).abs() < 1.0);
    }

    #[test]
    fn test_encode_fps() {
        let mut c = TranscodeMetricsCollector::new();
        // 33333 µs ≈ 30 fps
        c.record(make_metric(0, 33_333, 100));
        c.record(make_metric(1, 33_333, 100));
        let s = c.summarise();
        let fps = s.encode_fps();
        assert!((fps - 30.0).abs() < 0.1);
    }

    #[test]
    fn test_slowest_frame() {
        let mut c = TranscodeMetricsCollector::new();
        c.record(make_metric(0, 1000, 100));
        c.record(make_metric(1, 9000, 200));
        c.record(make_metric(2, 500, 50));
        let sf = c.slowest_frame().expect("should succeed in test");
        assert_eq!(sf.frame_index, 1);
    }

    #[test]
    fn test_worst_psnr_frame() {
        let mut c = TranscodeMetricsCollector::new();
        c.record(make_metric(0, 100, 100).with_psnr(45.0));
        c.record(make_metric(1, 100, 100).with_psnr(35.0));
        c.record(make_metric(2, 100, 100).with_psnr(50.0));
        let worst = c.worst_psnr_frame().expect("should succeed in test");
        assert_eq!(worst.frame_index, 1);
    }

    #[test]
    fn test_clear() {
        let mut c = TranscodeMetricsCollector::new();
        c.record(make_metric(0, 100, 100));
        c.clear();
        assert!(c.is_empty());
    }
}
