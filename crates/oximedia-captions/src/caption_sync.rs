//! Caption synchronisation utilities for `OxiMedia`.
//!
//! Aligns caption tracks to audio/video references using anchor points,
//! detects drift, and produces detailed sync reports.

#![allow(dead_code)]

/// A timing anchor that pins a caption timestamp to a reference timestamp.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncAnchor {
    /// Caption timestamp in milliseconds.
    pub caption_ms: i64,
    /// Reference (audio/video) timestamp in milliseconds.
    pub reference_ms: i64,
    /// Confidence of this anchor (0.0 – 1.0).
    pub confidence: f32,
    /// Optional human-readable label (e.g. "shot cut", "speech onset").
    pub label: Option<String>,
}

impl SyncAnchor {
    /// Creates a new `SyncAnchor`.
    #[must_use]
    pub fn new(caption_ms: i64, reference_ms: i64, confidence: f32) -> Self {
        Self {
            caption_ms,
            reference_ms,
            confidence: confidence.clamp(0.0, 1.0),
            label: None,
        }
    }

    /// Attaches a label to this anchor.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Returns the signed drift: `reference_ms - caption_ms`.
    ///
    /// A positive value means the caption is early; negative means it is late.
    #[must_use]
    pub fn drift_ms(&self) -> i64 {
        self.reference_ms - self.caption_ms
    }

    /// Returns `true` when the anchor has high confidence (≥ 0.8).
    #[must_use]
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }
}

/// Configuration for a caption synchronisation pass.
#[derive(Debug, Clone)]
pub struct CaptionSyncConfig {
    /// Maximum acceptable drift in milliseconds before a cue is flagged.
    pub max_drift_ms: i64,
    /// Tolerance used when deciding whether an anchor aligns well enough.
    pub tolerance_ms: i64,
    /// Whether to apply a linear correction across the whole track.
    pub apply_linear_correction: bool,
    /// Minimum anchor confidence required for an anchor to be used.
    pub min_anchor_confidence: f32,
}

impl Default for CaptionSyncConfig {
    fn default() -> Self {
        Self {
            max_drift_ms: 500,
            tolerance_ms: 80,
            apply_linear_correction: true,
            min_anchor_confidence: 0.5,
        }
    }
}

impl CaptionSyncConfig {
    /// Creates a new sync configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum drift threshold.
    #[must_use]
    pub fn with_max_drift(mut self, ms: i64) -> Self {
        self.max_drift_ms = ms;
        self
    }

    /// Returns the tolerance in milliseconds.
    #[must_use]
    pub fn tolerance_ms(&self) -> i64 {
        self.tolerance_ms
    }
}

/// A caption cue with mutable timing, used during synchronisation.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncableCue {
    /// Cue index.
    pub index: usize,
    /// Start time in milliseconds.
    pub start_ms: i64,
    /// End time in milliseconds.
    pub end_ms: i64,
    /// Caption text content.
    pub text: String,
}

impl SyncableCue {
    /// Creates a new `SyncableCue`.
    #[must_use]
    pub fn new(index: usize, start_ms: i64, end_ms: i64, text: impl Into<String>) -> Self {
        Self {
            index,
            start_ms,
            end_ms,
            text: text.into(),
        }
    }

    /// Returns the duration of the cue in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> i64 {
        self.end_ms - self.start_ms
    }

    /// Shifts the cue by `delta_ms` (positive = shift later).
    pub fn shift(&mut self, delta_ms: i64) {
        self.start_ms += delta_ms;
        self.end_ms += delta_ms;
    }
}

/// Aligns caption tracks to reference timing using anchor points.
#[derive(Debug, Default)]
pub struct CaptionSyncer;

impl CaptionSyncer {
    /// Creates a new `CaptionSyncer`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Aligns `cues` to reference timing using the provided `anchors` and `config`.
    ///
    /// Returns adjusted cues and a sync report.
    #[must_use]
    pub fn align(
        &self,
        mut cues: Vec<SyncableCue>,
        anchors: &[SyncAnchor],
        config: &CaptionSyncConfig,
    ) -> (Vec<SyncableCue>, SyncReport) {
        let valid_anchors: Vec<&SyncAnchor> = anchors
            .iter()
            .filter(|a| a.confidence >= config.min_anchor_confidence)
            .collect();

        if valid_anchors.is_empty() {
            let report = SyncReport::new(0, 0, 0, vec![], 0);
            return (cues, report);
        }

        // Compute global offset as the weighted mean drift of all valid anchors
        let (sum_w, sum_drift) = valid_anchors.iter().fold((0.0f64, 0.0f64), |(sw, sd), a| {
            let w = f64::from(a.confidence);
            (sw + w, sd + w * a.drift_ms() as f64)
        });
        let global_offset_ms = if sum_w > 0.0 {
            (sum_drift / sum_w).round() as i64
        } else {
            0
        };

        let mut drifts: Vec<i64> = Vec::new();
        let mut over_limit_cues: Vec<usize> = Vec::new();
        let mut corrected = 0usize;

        for cue in &mut cues {
            let effective_drift = global_offset_ms;
            drifts.push(effective_drift.abs());
            if effective_drift.abs() > config.tolerance_ms && config.apply_linear_correction {
                cue.shift(effective_drift);
                corrected += 1;
            }
            if effective_drift.abs() > config.max_drift_ms {
                over_limit_cues.push(cue.index);
            }
        }

        let max_drift = drifts.iter().copied().max().unwrap_or(0);
        let avg_drift = if drifts.is_empty() {
            0
        } else {
            (drifts.iter().sum::<i64>() as f64 / drifts.len() as f64).round() as i64
        };

        let report = SyncReport::new(
            max_drift,
            avg_drift,
            corrected,
            over_limit_cues,
            valid_anchors.len(),
        );
        (cues, report)
    }

    /// Returns the sync status description based on maximum drift.
    #[must_use]
    pub fn sync_status(&self, max_drift_ms: i64, config: &CaptionSyncConfig) -> SyncStatus {
        if max_drift_ms <= config.tolerance_ms {
            SyncStatus::Good
        } else if max_drift_ms <= config.max_drift_ms {
            SyncStatus::Acceptable
        } else {
            SyncStatus::OutOfSync
        }
    }
}

/// Describes the overall synchronisation quality of a caption track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
    /// All cues are within tolerance.
    Good,
    /// Some drift exists but is within the maximum threshold.
    Acceptable,
    /// Drift exceeds the configured threshold.
    OutOfSync,
}

impl SyncStatus {
    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Good => "good",
            Self::Acceptable => "acceptable",
            Self::OutOfSync => "out-of-sync",
        }
    }
}

/// Summary of a synchronisation pass.
#[derive(Debug, Clone)]
pub struct SyncReport {
    /// Maximum absolute drift observed across all cues in milliseconds.
    pub max_drift_ms: i64,
    /// Average absolute drift in milliseconds.
    pub avg_drift_ms: i64,
    /// Number of cues that were corrected.
    pub corrected_cues: usize,
    /// Indices of cues whose drift exceeded the maximum threshold.
    pub over_limit_cue_indices: Vec<usize>,
    /// Number of anchors used in the alignment.
    pub anchors_used: usize,
}

impl SyncReport {
    /// Creates a new `SyncReport`.
    #[must_use]
    pub fn new(
        max_drift_ms: i64,
        avg_drift_ms: i64,
        corrected_cues: usize,
        over_limit_cue_indices: Vec<usize>,
        anchors_used: usize,
    ) -> Self {
        Self {
            max_drift_ms,
            avg_drift_ms,
            corrected_cues,
            over_limit_cue_indices,
            anchors_used,
        }
    }

    /// Returns the maximum absolute drift in milliseconds.
    #[must_use]
    pub fn max_drift_ms(&self) -> i64 {
        self.max_drift_ms
    }

    /// Returns `true` when no cues exceeded the maximum drift threshold.
    #[must_use]
    pub fn all_within_limit(&self) -> bool {
        self.over_limit_cue_indices.is_empty()
    }

    /// Returns the number of cues that exceeded the drift limit.
    #[must_use]
    pub fn over_limit_count(&self) -> usize {
        self.over_limit_cue_indices.len()
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_cues() -> Vec<SyncableCue> {
        vec![
            SyncableCue::new(0, 0, 2000, "First cue"),
            SyncableCue::new(1, 2500, 5000, "Second cue"),
            SyncableCue::new(2, 5500, 8000, "Third cue"),
        ]
    }

    #[test]
    fn test_anchor_drift_positive() {
        let a = SyncAnchor::new(1000, 1200, 0.9);
        assert_eq!(a.drift_ms(), 200);
    }

    #[test]
    fn test_anchor_drift_negative() {
        let a = SyncAnchor::new(1000, 800, 0.9);
        assert_eq!(a.drift_ms(), -200);
    }

    #[test]
    fn test_anchor_high_confidence() {
        let high = SyncAnchor::new(0, 0, 0.9);
        let low = SyncAnchor::new(0, 0, 0.5);
        assert!(high.is_high_confidence());
        assert!(!low.is_high_confidence());
    }

    #[test]
    fn test_anchor_label() {
        let a = SyncAnchor::new(0, 0, 1.0).with_label("shot cut");
        assert_eq!(a.label.as_deref(), Some("shot cut"));
    }

    #[test]
    fn test_sync_config_tolerance() {
        let cfg = CaptionSyncConfig::new().with_max_drift(300);
        assert_eq!(cfg.max_drift_ms, 300);
        assert_eq!(cfg.tolerance_ms(), 80); // default
    }

    #[test]
    fn test_syncable_cue_duration() {
        let cue = SyncableCue::new(0, 1000, 4000, "text");
        assert_eq!(cue.duration_ms(), 3000);
    }

    #[test]
    fn test_syncable_cue_shift() {
        let mut cue = SyncableCue::new(0, 1000, 3000, "text");
        cue.shift(200);
        assert_eq!(cue.start_ms, 1200);
        assert_eq!(cue.end_ms, 3200);
    }

    #[test]
    fn test_syncer_no_anchors_returns_unchanged() {
        let cues = simple_cues();
        let config = CaptionSyncConfig::new();
        let (synced, report) = CaptionSyncer::new().align(cues.clone(), &[], &config);
        assert_eq!(synced.len(), cues.len());
        assert_eq!(report.anchors_used, 0);
    }

    #[test]
    fn test_syncer_applies_global_offset() {
        let cues = simple_cues();
        let anchors = vec![
            SyncAnchor::new(0, 200, 1.0),     // drift +200
            SyncAnchor::new(2500, 2700, 1.0), // drift +200
        ];
        let config = CaptionSyncConfig::new();
        let (synced, report) = CaptionSyncer::new().align(cues, &anchors, &config);
        // Offset of 200 ms applied
        assert_eq!(synced[0].start_ms, 200);
        assert_eq!(report.anchors_used, 2);
    }

    #[test]
    fn test_sync_report_all_within_limit() {
        let report = SyncReport::new(50, 25, 2, vec![], 3);
        assert!(report.all_within_limit());
        assert_eq!(report.over_limit_count(), 0);
    }

    #[test]
    fn test_sync_report_over_limit_count() {
        let report = SyncReport::new(600, 400, 0, vec![1, 3], 2);
        assert!(!report.all_within_limit());
        assert_eq!(report.over_limit_count(), 2);
    }

    #[test]
    fn test_sync_status_good() {
        let config = CaptionSyncConfig::new();
        let syncer = CaptionSyncer::new();
        assert_eq!(syncer.sync_status(40, &config), SyncStatus::Good);
    }

    #[test]
    fn test_sync_status_acceptable() {
        let config = CaptionSyncConfig::new();
        let syncer = CaptionSyncer::new();
        assert_eq!(syncer.sync_status(200, &config), SyncStatus::Acceptable);
    }

    #[test]
    fn test_sync_status_out_of_sync() {
        let config = CaptionSyncConfig::new();
        let syncer = CaptionSyncer::new();
        assert_eq!(syncer.sync_status(1000, &config), SyncStatus::OutOfSync);
    }

    #[test]
    fn test_sync_status_labels() {
        assert_eq!(SyncStatus::Good.label(), "good");
        assert_eq!(SyncStatus::Acceptable.label(), "acceptable");
        assert_eq!(SyncStatus::OutOfSync.label(), "out-of-sync");
    }
}
