//! Peak level metering with hold, history, and dBFS conversion.
#![allow(dead_code)]

/// Convert a linear amplitude value to dBFS.
#[allow(clippy::cast_precision_loss)]
fn linear_to_dbfs(linear: f64) -> f64 {
    if linear <= 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * linear.log10()
    }
}

/// A single peak level reading.
#[derive(Clone, Debug)]
pub struct PeakLevel {
    /// Peak value in linear scale (0.0 – 1.0+).
    pub linear: f64,
}

impl PeakLevel {
    /// Create a new `PeakLevel` from a linear amplitude.
    pub fn new(linear: f64) -> Self {
        Self { linear }
    }

    /// Convert to dBFS.
    pub fn to_dbfs(&self) -> f64 {
        linear_to_dbfs(self.linear)
    }

    /// Return `true` when the peak is at or above 0 dBFS.
    pub fn is_clipping(&self) -> bool {
        self.linear >= 1.0
    }
}

/// Peak meter with peak-hold functionality for a single channel.
#[derive(Debug)]
pub struct PeakMeter {
    /// Running peak since last reset.
    current_peak: f64,
    /// Held peak (reset with [`Self::reset_hold`]).
    held_peak: f64,
    /// Number of samples pushed.
    sample_count: u64,
}

impl PeakMeter {
    /// Create a new, zeroed peak meter.
    pub fn new() -> Self {
        Self {
            current_peak: 0.0,
            held_peak: 0.0,
            sample_count: 0,
        }
    }

    /// Push a single sample (absolute value is used).
    pub fn push_sample(&mut self, sample: f64) {
        let abs = sample.abs();
        if abs > self.current_peak {
            self.current_peak = abs;
        }
        if abs > self.held_peak {
            self.held_peak = abs;
        }
        self.sample_count += 1;
    }

    /// Push a slice of samples.
    pub fn push_slice(&mut self, samples: &[f64]) {
        for &s in samples {
            self.push_sample(s);
        }
    }

    /// Get current peak in dBFS.
    pub fn peak_dbfs(&self) -> f64 {
        linear_to_dbfs(self.current_peak)
    }

    /// Get the held peak in dBFS (highest peak seen since last [`Self::reset_hold`]).
    pub fn hold_peak(&self) -> f64 {
        linear_to_dbfs(self.held_peak)
    }

    /// Reset only the hold peak; the running peak is unchanged.
    pub fn reset_hold(&mut self) {
        self.held_peak = self.current_peak;
    }

    /// Reset both running and hold peaks to zero.
    pub fn reset(&mut self) {
        self.current_peak = 0.0;
        self.held_peak = 0.0;
        self.sample_count = 0;
    }

    /// Return the number of samples pushed since creation / last full reset.
    pub fn sample_count(&self) -> u64 {
        self.sample_count
    }

    /// Return current peak as a [`PeakLevel`].
    pub fn level(&self) -> PeakLevel {
        PeakLevel::new(self.current_peak)
    }
}

impl Default for PeakMeter {
    fn default() -> Self {
        Self::new()
    }
}

/// A timestamped entry in the peak history.
#[derive(Clone, Debug)]
pub struct PeakEntry {
    /// Peak linear amplitude.
    pub linear: f64,
    /// Monotonic sample index when this entry was recorded.
    pub sample_index: u64,
}

/// Rolling history of peak readings.
#[derive(Debug)]
pub struct PeakHistory {
    entries: Vec<PeakEntry>,
    capacity: usize,
}

impl PeakHistory {
    /// Create a history with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity: capacity.max(1),
        }
    }

    /// Record a new peak reading.
    pub fn record(&mut self, linear: f64, sample_index: u64) {
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(PeakEntry {
            linear,
            sample_index,
        });
    }

    /// Return the maximum peak (in linear) across all recorded entries.
    pub fn max_peak(&self) -> f64 {
        self.entries
            .iter()
            .map(|e| e.linear)
            .fold(0.0_f64, f64::max)
    }

    /// Return the maximum peak in dBFS.
    pub fn max_peak_dbfs(&self) -> f64 {
        linear_to_dbfs(self.max_peak())
    }

    /// Number of recorded entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` when no entries have been recorded yet.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Immutable access to the raw entries.
    pub fn entries(&self) -> &[PeakEntry] {
        &self.entries
    }

    /// Clear all recorded history.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PeakLevel ────────────────────────────────────────────────────────────

    #[test]
    fn peak_level_zero_is_neg_inf_dbfs() {
        let lvl = PeakLevel::new(0.0);
        assert!(lvl.to_dbfs().is_infinite() && lvl.to_dbfs() < 0.0);
    }

    #[test]
    fn peak_level_full_scale_is_zero_dbfs() {
        let lvl = PeakLevel::new(1.0);
        assert!((lvl.to_dbfs() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn peak_level_half_is_minus6dbfs() {
        let lvl = PeakLevel::new(0.5);
        // 20 * log10(0.5) ≈ -6.02 dBFS
        assert!((lvl.to_dbfs() - (-6.020_599_913_279_624)).abs() < 1e-6);
    }

    #[test]
    fn peak_level_clipping_at_or_above_one() {
        assert!(PeakLevel::new(1.0).is_clipping());
        assert!(PeakLevel::new(1.5).is_clipping());
        assert!(!PeakLevel::new(0.99).is_clipping());
    }

    // ── PeakMeter ────────────────────────────────────────────────────────────

    #[test]
    fn peak_meter_default_starts_empty() {
        let m = PeakMeter::default();
        assert_eq!(m.sample_count(), 0);
        assert!(m.peak_dbfs().is_infinite() && m.peak_dbfs() < 0.0);
    }

    #[test]
    fn peak_meter_push_sample_tracks_peak() {
        let mut m = PeakMeter::new();
        m.push_sample(0.3);
        m.push_sample(0.8);
        m.push_sample(0.1);
        assert!((m.peak_dbfs() - linear_to_dbfs(0.8)).abs() < 1e-9);
    }

    #[test]
    fn peak_meter_negative_sample_uses_absolute() {
        let mut m = PeakMeter::new();
        m.push_sample(-0.9);
        assert!((m.peak_dbfs() - linear_to_dbfs(0.9)).abs() < 1e-9);
    }

    #[test]
    fn peak_meter_hold_peak_is_max_ever_seen() {
        let mut m = PeakMeter::new();
        m.push_sample(0.5);
        assert!((m.hold_peak() - linear_to_dbfs(0.5)).abs() < 1e-9);
        m.reset_hold();
        // After reset_hold the held peak re-anchors to current
        assert!((m.hold_peak() - m.peak_dbfs()).abs() < 1e-9);
    }

    #[test]
    fn peak_meter_reset_clears_all() {
        let mut m = PeakMeter::new();
        m.push_sample(0.7);
        m.reset();
        assert_eq!(m.sample_count(), 0);
        assert!(m.peak_dbfs().is_infinite() && m.peak_dbfs() < 0.0);
    }

    #[test]
    fn peak_meter_push_slice() {
        let mut m = PeakMeter::new();
        m.push_slice(&[0.1, 0.4, 0.2]);
        assert_eq!(m.sample_count(), 3);
        assert!((m.peak_dbfs() - linear_to_dbfs(0.4)).abs() < 1e-9);
    }

    #[test]
    fn peak_meter_level_reflects_current_peak() {
        let mut m = PeakMeter::new();
        m.push_sample(0.6);
        let lvl = m.level();
        assert!((lvl.linear - 0.6).abs() < 1e-9);
    }

    // ── PeakHistory ──────────────────────────────────────────────────────────

    #[test]
    fn peak_history_empty_on_creation() {
        let h = PeakHistory::with_capacity(10);
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn peak_history_record_and_max() {
        let mut h = PeakHistory::with_capacity(5);
        h.record(0.3, 0);
        h.record(0.9, 1);
        h.record(0.5, 2);
        assert!((h.max_peak() - 0.9).abs() < 1e-9);
    }

    #[test]
    fn peak_history_evicts_oldest_when_full() {
        let mut h = PeakHistory::with_capacity(3);
        h.record(0.9, 0);
        h.record(0.2, 1);
        h.record(0.3, 2);
        h.record(0.4, 3); // should evict entry 0 (linear=0.9)
        assert_eq!(h.len(), 3);
        // 0.9 is gone; max should now be 0.4
        assert!((h.max_peak() - 0.4).abs() < 1e-9);
    }

    #[test]
    fn peak_history_clear_resets() {
        let mut h = PeakHistory::with_capacity(5);
        h.record(0.5, 0);
        h.clear();
        assert!(h.is_empty());
    }

    #[test]
    fn peak_history_max_peak_dbfs() {
        let mut h = PeakHistory::with_capacity(4);
        h.record(1.0, 0);
        assert!((h.max_peak_dbfs() - 0.0).abs() < 1e-9);
    }
}
