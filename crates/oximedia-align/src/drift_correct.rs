//! Clock drift correction using PLL-based techniques.
//!
//! Provides phase-locked loop (PLL) based clock drift correction,
//! drift rate estimation, and sample-level adjustment for A/V synchronization.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::too_many_arguments)]

/// PLL (Phase-Locked Loop) state for drift correction
#[derive(Debug, Clone)]
pub struct PllState {
    /// Current phase error (in samples)
    pub phase_error: f64,
    /// Current frequency error (fractional)
    pub freq_error: f64,
    /// Loop filter state
    pub filter_state: f64,
    /// Proportional gain
    pub kp: f64,
    /// Integral gain
    pub ki: f64,
    /// Current correction (samples per input sample)
    pub correction: f64,
}

impl PllState {
    /// Create a new PLL state with given gains
    #[must_use]
    pub fn new(kp: f64, ki: f64) -> Self {
        Self {
            phase_error: 0.0,
            freq_error: 0.0,
            filter_state: 0.0,
            kp,
            ki,
            correction: 0.0,
        }
    }

    /// Update PLL with a new phase measurement and return correction
    pub fn update(&mut self, measured_phase: f64, expected_phase: f64) -> f64 {
        self.phase_error = measured_phase - expected_phase;
        self.filter_state += self.ki * self.phase_error;
        self.correction = self.kp * self.phase_error + self.filter_state;
        self.freq_error = self.correction;
        self.correction
    }

    /// Reset PLL state
    pub fn reset(&mut self) {
        self.phase_error = 0.0;
        self.freq_error = 0.0;
        self.filter_state = 0.0;
        self.correction = 0.0;
    }

    /// Returns true if PLL is locked (error below threshold)
    #[must_use]
    pub fn is_locked(&self, threshold: f64) -> bool {
        self.phase_error.abs() < threshold && self.freq_error.abs() < threshold * 0.01
    }
}

impl Default for PllState {
    fn default() -> Self {
        Self::new(0.1, 0.001)
    }
}

/// Drift rate estimator using linear regression over a window of samples
#[derive(Debug, Clone)]
pub struct DriftRateEstimator {
    /// Window of (time, offset) measurements
    measurements: Vec<(f64, f64)>,
    /// Maximum number of measurements to keep
    max_window: usize,
    /// Current estimated drift rate (parts per million)
    estimated_ppm: f64,
}

impl DriftRateEstimator {
    /// Create a new drift rate estimator
    #[must_use]
    pub fn new(max_window: usize) -> Self {
        Self {
            measurements: Vec::new(),
            max_window,
            estimated_ppm: 0.0,
        }
    }

    /// Add a new measurement (time in seconds, offset in samples)
    pub fn add_measurement(&mut self, time: f64, offset: f64) {
        self.measurements.push((time, offset));
        if self.measurements.len() > self.max_window {
            self.measurements.remove(0);
        }
        if self.measurements.len() >= 2 {
            self.estimated_ppm = self.compute_drift_ppm();
        }
    }

    /// Compute drift rate in PPM using linear regression
    fn compute_drift_ppm(&self) -> f64 {
        let n = self.measurements.len() as f64;
        let sum_t: f64 = self.measurements.iter().map(|(t, _)| t).sum();
        let sum_o: f64 = self.measurements.iter().map(|(_, o)| o).sum();
        let sum_t2: f64 = self.measurements.iter().map(|(t, _)| t * t).sum();
        let sum_to: f64 = self.measurements.iter().map(|(t, o)| t * o).sum();

        let denom = n * sum_t2 - sum_t * sum_t;
        if denom.abs() < f64::EPSILON {
            return 0.0;
        }
        let slope = (n * sum_to - sum_t * sum_o) / denom;
        // Convert slope (samples/second) to PPM assuming 48000 Hz
        slope / 48.0 // slope / (sample_rate / 1_000_000)
    }

    /// Get current drift rate estimate in PPM
    #[must_use]
    pub fn drift_ppm(&self) -> f64 {
        self.estimated_ppm
    }

    /// Get drift rate in samples per second (at given sample rate)
    #[must_use]
    pub fn drift_samples_per_second(&self, sample_rate: u32) -> f64 {
        self.estimated_ppm * f64::from(sample_rate) / 1_000_000.0
    }

    /// Clear all measurements
    pub fn clear(&mut self) {
        self.measurements.clear();
        self.estimated_ppm = 0.0;
    }

    /// Number of measurements held
    #[must_use]
    pub fn len(&self) -> usize {
        self.measurements.len()
    }

    /// Returns true if no measurements
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.measurements.is_empty()
    }
}

/// Sample adjustment result after drift correction
#[derive(Debug, Clone, Copy)]
pub struct SampleAdjustment {
    /// Number of samples to insert (positive) or drop (negative)
    pub delta: i64,
    /// Fractional part of the adjustment
    pub fractional: f64,
    /// Cumulative drift in samples
    pub cumulative_drift: f64,
}

impl SampleAdjustment {
    /// Create a new sample adjustment
    #[must_use]
    pub fn new(delta: i64, fractional: f64, cumulative_drift: f64) -> Self {
        Self {
            delta,
            fractional,
            cumulative_drift,
        }
    }

    /// Returns true if any adjustment is needed
    #[must_use]
    pub fn needs_adjustment(&self) -> bool {
        self.delta != 0
    }
}

/// Drift corrector combining PLL and sample adjustment
#[derive(Debug, Clone)]
pub struct DriftCorrector {
    /// PLL state
    pll: PllState,
    /// Drift rate estimator
    estimator: DriftRateEstimator,
    /// Accumulated fractional sample error
    accumulator: f64,
    /// Total samples processed
    total_samples: u64,
    /// Sample rate
    sample_rate: u32,
}

impl DriftCorrector {
    /// Create a new drift corrector
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            pll: PllState::new(0.05, 0.0005),
            estimator: DriftRateEstimator::new(100),
            accumulator: 0.0,
            total_samples: 0,
            sample_rate,
        }
    }

    /// Process a block of samples and return the required adjustment
    pub fn process_block(&mut self, block_size: usize, measured_offset: f64) -> SampleAdjustment {
        let time = self.total_samples as f64 / f64::from(self.sample_rate);
        self.estimator.add_measurement(time, measured_offset);

        let drift_rate = self.estimator.drift_samples_per_second(self.sample_rate);
        let block_drift = drift_rate * block_size as f64 / f64::from(self.sample_rate);

        self.accumulator += block_drift;
        let delta = self.accumulator.trunc() as i64;
        self.accumulator -= delta as f64;

        self.total_samples += block_size as u64;

        SampleAdjustment::new(delta, self.accumulator, measured_offset + block_drift)
    }

    /// Update PLL with external reference
    pub fn update_pll(&mut self, measured: f64, expected: f64) -> f64 {
        self.pll.update(measured, expected)
    }

    /// Reset all state
    pub fn reset(&mut self) {
        self.pll.reset();
        self.estimator.clear();
        self.accumulator = 0.0;
        self.total_samples = 0;
    }

    /// Get current drift estimate in PPM
    #[must_use]
    pub fn drift_ppm(&self) -> f64 {
        self.estimator.drift_ppm()
    }

    /// Check if PLL is locked
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.pll.is_locked(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pll_state_creation() {
        let pll = PllState::new(0.1, 0.001);
        assert_eq!(pll.kp, 0.1);
        assert_eq!(pll.ki, 0.001);
        assert_eq!(pll.phase_error, 0.0);
        assert_eq!(pll.correction, 0.0);
    }

    #[test]
    fn test_pll_default() {
        let pll = PllState::default();
        assert_eq!(pll.kp, 0.1);
        assert_eq!(pll.ki, 0.001);
    }

    #[test]
    fn test_pll_update_convergence() {
        let mut pll = PllState::new(0.5, 0.01);
        let mut phase = 10.0_f64;
        for _ in 0..50 {
            let correction = pll.update(phase, 0.0);
            phase -= correction;
        }
        assert!(phase.abs() < 1.0, "PLL should converge: {phase}");
    }

    #[test]
    fn test_pll_reset() {
        let mut pll = PllState::new(0.1, 0.001);
        pll.update(5.0, 0.0);
        assert_ne!(pll.phase_error, 0.0);
        pll.reset();
        assert_eq!(pll.phase_error, 0.0);
        assert_eq!(pll.filter_state, 0.0);
        assert_eq!(pll.correction, 0.0);
    }

    #[test]
    fn test_pll_lock_detection() {
        let pll = PllState::new(0.1, 0.001);
        // Fresh PLL with zero errors should be locked
        assert!(pll.is_locked(1.0));
    }

    #[test]
    fn test_drift_rate_estimator_creation() {
        let est = DriftRateEstimator::new(50);
        assert_eq!(est.max_window, 50);
        assert_eq!(est.drift_ppm(), 0.0);
        assert!(est.is_empty());
    }

    #[test]
    fn test_drift_rate_single_measurement() {
        let mut est = DriftRateEstimator::new(100);
        est.add_measurement(1.0, 5.0);
        assert_eq!(est.len(), 1);
        // Need at least 2 for regression
        assert_eq!(est.drift_ppm(), 0.0);
    }

    #[test]
    fn test_drift_rate_two_measurements() {
        let mut est = DriftRateEstimator::new(100);
        est.add_measurement(0.0, 0.0);
        est.add_measurement(1.0, 48.0); // 48 samples/sec drift
        assert_ne!(est.drift_ppm(), 0.0);
    }

    #[test]
    fn test_drift_rate_window_limit() {
        let mut est = DriftRateEstimator::new(5);
        for i in 0..10 {
            est.add_measurement(i as f64, i as f64 * 2.0);
        }
        assert_eq!(est.len(), 5);
    }

    #[test]
    fn test_drift_rate_clear() {
        let mut est = DriftRateEstimator::new(100);
        est.add_measurement(0.0, 0.0);
        est.add_measurement(1.0, 10.0);
        est.clear();
        assert!(est.is_empty());
        assert_eq!(est.drift_ppm(), 0.0);
    }

    #[test]
    fn test_sample_adjustment_creation() {
        let adj = SampleAdjustment::new(2, 0.3, 15.7);
        assert_eq!(adj.delta, 2);
        assert!((adj.fractional - 0.3).abs() < f64::EPSILON);
        assert!(adj.needs_adjustment());
    }

    #[test]
    fn test_sample_adjustment_no_adjustment() {
        let adj = SampleAdjustment::new(0, 0.5, 0.5);
        assert!(!adj.needs_adjustment());
    }

    #[test]
    fn test_drift_corrector_creation() {
        let dc = DriftCorrector::new(48000);
        assert_eq!(dc.sample_rate, 48000);
        assert_eq!(dc.total_samples, 0);
    }

    #[test]
    fn test_drift_corrector_process_block() {
        let mut dc = DriftCorrector::new(48000);
        // Add some measurements first
        dc.estimator.add_measurement(0.0, 0.0);
        dc.estimator.add_measurement(1.0, 48.0);
        let adj = dc.process_block(480, 48.0);
        // Should produce some adjustment given drift
        assert_eq!(adj.delta.abs() <= 1, true); // small block, small delta
    }

    #[test]
    fn test_drift_corrector_reset() {
        let mut dc = DriftCorrector::new(48000);
        dc.estimator.add_measurement(0.0, 0.0);
        dc.estimator.add_measurement(1.0, 100.0);
        dc.total_samples = 1000;
        dc.reset();
        assert_eq!(dc.total_samples, 0);
        assert_eq!(dc.accumulator, 0.0);
        assert!(dc.estimator.is_empty());
    }

    #[test]
    fn test_drift_corrector_pll_update() {
        let mut dc = DriftCorrector::new(48000);
        let correction = dc.update_pll(10.0, 0.0);
        assert!(correction.abs() > 0.0);
    }
}
