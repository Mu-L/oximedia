//! Audio/video synchronisation analysis.
//!
//! Measures A/V sync drift over time and classifies sync health,
//! providing sample-by-sample tracking and reporting.

#![allow(dead_code)]

/// A single A/V sync measurement at a point in time.
#[derive(Debug, Clone, Copy)]
pub struct SyncMeasure {
    /// Presentation timestamp in milliseconds.
    pub pts_ms: f64,
    /// Measured audio-video offset in milliseconds.
    /// Positive = audio leads video; negative = audio lags video.
    pub offset_ms: f64,
}

impl SyncMeasure {
    /// Create a new `SyncMeasure`.
    #[must_use]
    pub fn new(pts_ms: f64, offset_ms: f64) -> Self {
        Self { pts_ms, offset_ms }
    }

    /// Return the absolute drift magnitude in milliseconds.
    #[must_use]
    pub fn drift_ms(&self) -> f64 {
        self.offset_ms.abs()
    }

    /// Return `true` if audio leads video (positive offset).
    #[must_use]
    pub fn audio_leads(&self) -> bool {
        self.offset_ms > 0.0
    }
}

/// Classification of A/V sync health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
    /// Sync drift is within acceptable tolerance (≤ 40 ms).
    Ok,
    /// Sync drift is noticeable but not severe (40–80 ms).
    Warning,
    /// Sync drift exceeds acceptable limits (> 80 ms).
    Critical,
}

impl SyncStatus {
    /// Returns `true` if the status is `Ok`.
    #[must_use]
    pub fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    /// Classify a drift value (in ms) into a `SyncStatus`.
    #[must_use]
    pub fn from_drift(drift_ms: f64) -> Self {
        if drift_ms <= 40.0 {
            Self::Ok
        } else if drift_ms <= 80.0 {
            Self::Warning
        } else {
            Self::Critical
        }
    }

    /// Return a short label for this status.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Warning => "Warning",
            Self::Critical => "Critical",
        }
    }
}

/// Accumulates A/V sync measurements and tracks drift.
#[derive(Debug, Clone, Default)]
pub struct AvSyncAnalyzer {
    samples: Vec<SyncMeasure>,
}

impl AvSyncAnalyzer {
    /// Create a new, empty `AvSyncAnalyzer`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a sync measurement sample.
    pub fn add_sample(&mut self, sample: SyncMeasure) {
        self.samples.push(sample);
    }

    /// Return the most recent drift in milliseconds.
    ///
    /// Returns `0.0` if no samples have been recorded.
    #[must_use]
    pub fn current_drift_ms(&self) -> f64 {
        self.samples.last().map_or(0.0, SyncMeasure::drift_ms)
    }

    /// Return the current sync status based on the latest sample.
    #[must_use]
    pub fn current_status(&self) -> SyncStatus {
        SyncStatus::from_drift(self.current_drift_ms())
    }

    /// Return the average drift across all samples, or `0.0` if empty.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_drift_ms(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.samples.iter().map(SyncMeasure::drift_ms).sum();
        sum / self.samples.len() as f64
    }

    /// Build a `SyncReport` from the current state.
    #[must_use]
    pub fn build_report(&self) -> SyncReport {
        let max_drift = self
            .samples
            .iter()
            .map(SyncMeasure::drift_ms)
            .fold(0.0_f64, f64::max);
        SyncReport {
            sample_count: self.samples.len(),
            avg_drift_ms: self.avg_drift_ms(),
            max_drift_ms: max_drift,
            status: SyncStatus::from_drift(max_drift),
        }
    }

    /// Return all recorded samples as a slice.
    #[must_use]
    pub fn samples(&self) -> &[SyncMeasure] {
        &self.samples
    }
}

/// Summary report of A/V sync analysis results.
#[derive(Debug, Clone, Copy)]
pub struct SyncReport {
    /// Number of samples collected.
    pub sample_count: usize,
    /// Average drift in milliseconds.
    pub avg_drift_ms: f64,
    /// Maximum drift observed in milliseconds.
    pub max_drift_ms: f64,
    /// Overall sync health classification.
    pub status: SyncStatus,
}

impl SyncReport {
    /// Return the maximum drift in milliseconds.
    #[must_use]
    pub fn max_drift_ms(&self) -> f64 {
        self.max_drift_ms
    }

    /// Return `true` if the overall sync status is `Ok`.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.status.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_measure_drift_ms_positive() {
        let m = SyncMeasure::new(0.0, 30.0);
        assert!((m.drift_ms() - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sync_measure_drift_ms_negative() {
        let m = SyncMeasure::new(0.0, -50.0);
        assert!((m.drift_ms() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sync_measure_audio_leads() {
        let m = SyncMeasure::new(0.0, 10.0);
        assert!(m.audio_leads());
    }

    #[test]
    fn test_sync_measure_audio_lags() {
        let m = SyncMeasure::new(0.0, -10.0);
        assert!(!m.audio_leads());
    }

    #[test]
    fn test_sync_status_from_drift_ok() {
        assert_eq!(SyncStatus::from_drift(20.0), SyncStatus::Ok);
        assert_eq!(SyncStatus::from_drift(40.0), SyncStatus::Ok);
    }

    #[test]
    fn test_sync_status_from_drift_warning() {
        assert_eq!(SyncStatus::from_drift(41.0), SyncStatus::Warning);
        assert_eq!(SyncStatus::from_drift(80.0), SyncStatus::Warning);
    }

    #[test]
    fn test_sync_status_from_drift_critical() {
        assert_eq!(SyncStatus::from_drift(81.0), SyncStatus::Critical);
    }

    #[test]
    fn test_sync_status_is_ok() {
        assert!(SyncStatus::Ok.is_ok());
        assert!(!SyncStatus::Warning.is_ok());
        assert!(!SyncStatus::Critical.is_ok());
    }

    #[test]
    fn test_sync_status_label() {
        assert_eq!(SyncStatus::Ok.label(), "OK");
        assert_eq!(SyncStatus::Warning.label(), "Warning");
        assert_eq!(SyncStatus::Critical.label(), "Critical");
    }

    #[test]
    fn test_av_sync_analyzer_empty() {
        let analyzer = AvSyncAnalyzer::new();
        assert!((analyzer.current_drift_ms()).abs() < f64::EPSILON);
        assert_eq!(analyzer.current_status(), SyncStatus::Ok);
    }

    #[test]
    fn test_av_sync_analyzer_add_sample() {
        let mut analyzer = AvSyncAnalyzer::new();
        analyzer.add_sample(SyncMeasure::new(100.0, 90.0));
        assert!((analyzer.current_drift_ms() - 90.0).abs() < f64::EPSILON);
        assert_eq!(analyzer.current_status(), SyncStatus::Critical);
    }

    #[test]
    fn test_av_sync_analyzer_avg_drift() {
        let mut analyzer = AvSyncAnalyzer::new();
        analyzer.add_sample(SyncMeasure::new(0.0, 20.0));
        analyzer.add_sample(SyncMeasure::new(1000.0, 60.0));
        assert!((analyzer.avg_drift_ms() - 40.0).abs() < 1e-9);
    }

    #[test]
    fn test_sync_report_max_drift() {
        let mut analyzer = AvSyncAnalyzer::new();
        analyzer.add_sample(SyncMeasure::new(0.0, 10.0));
        analyzer.add_sample(SyncMeasure::new(1000.0, 85.0));
        analyzer.add_sample(SyncMeasure::new(2000.0, 30.0));
        let report = analyzer.build_report();
        assert!((report.max_drift_ms() - 85.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sync_report_is_healthy() {
        let mut analyzer = AvSyncAnalyzer::new();
        analyzer.add_sample(SyncMeasure::new(0.0, 5.0));
        let report = analyzer.build_report();
        assert!(report.is_healthy());
    }

    #[test]
    fn test_sync_report_not_healthy_when_critical() {
        let mut analyzer = AvSyncAnalyzer::new();
        analyzer.add_sample(SyncMeasure::new(0.0, 200.0));
        let report = analyzer.build_report();
        assert!(!report.is_healthy());
    }

    #[test]
    fn test_av_sync_samples_slice() {
        let mut analyzer = AvSyncAnalyzer::new();
        analyzer.add_sample(SyncMeasure::new(0.0, 5.0));
        analyzer.add_sample(SyncMeasure::new(100.0, 10.0));
        assert_eq!(analyzer.samples().len(), 2);
    }
}
