//! Clock drift compensation.

use std::time::{Duration, Instant};

/// Drift estimator.
pub struct DriftEstimator {
    /// Reference timestamp
    reference_time: Option<Instant>,
    /// Reference offset
    reference_offset: Option<i64>,
    /// Current drift estimate (ppb - parts per billion)
    drift_ppb: f64,
    /// Alpha for exponential smoothing
    alpha: f64,
}

impl DriftEstimator {
    /// Create a new drift estimator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            reference_time: None,
            reference_offset: None,
            drift_ppb: 0.0,
            alpha: 0.1, // Smoothing factor
        }
    }

    /// Update drift estimate with new offset measurement.
    pub fn update(&mut self, offset_ns: i64, now: Instant) {
        if let (Some(ref_time), Some(ref_offset)) = (self.reference_time, self.reference_offset) {
            let elapsed = now.duration_since(ref_time).as_secs_f64();

            if elapsed > 0.0 {
                // Calculate drift: (change in offset) / elapsed time
                let offset_change = offset_ns - ref_offset;
                let drift_estimate = (offset_change as f64 / elapsed) * 1e9; // Convert to ppb

                // Exponential smoothing
                self.drift_ppb = self.alpha * drift_estimate + (1.0 - self.alpha) * self.drift_ppb;

                // Update reference
                self.reference_time = Some(now);
                self.reference_offset = Some(offset_ns);
            }
        } else {
            // First measurement
            self.reference_time = Some(now);
            self.reference_offset = Some(offset_ns);
        }
    }

    /// Get current drift estimate (ppb).
    #[must_use]
    pub fn drift_ppb(&self) -> f64 {
        self.drift_ppb
    }

    /// Predict offset after a given duration based on current drift.
    #[must_use]
    pub fn predict_offset(&self, current_offset: i64, duration: Duration) -> i64 {
        let predicted_change = (self.drift_ppb * duration.as_secs_f64()) / 1e9;
        current_offset + predicted_change as i64
    }

    /// Reset the estimator.
    pub fn reset(&mut self) {
        self.reference_time = None;
        self.reference_offset = None;
        self.drift_ppb = 0.0;
    }
}

impl Default for DriftEstimator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drift_estimator() {
        let mut estimator = DriftEstimator::new();
        let now = Instant::now();

        estimator.update(1000, now);
        estimator.update(2000, now + Duration::from_secs(1));

        // Drift should be approximately 1000 ppb
        let drift = estimator.drift_ppb();
        assert!(drift > 0.0);
    }

    #[test]
    fn test_drift_prediction() {
        let estimator = DriftEstimator::new();
        // With zero drift, prediction should be same as current offset
        let predicted = estimator.predict_offset(1000, Duration::from_secs(1));
        assert_eq!(predicted, 1000);
    }
}
