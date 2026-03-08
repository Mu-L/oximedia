//! Stereo correlation analysis with qualitative classification.
//!
//! This module provides [`CorrelationAnalyzer`] — a sliding-window Pearson
//! correlation meter — and [`CorrelationReport`] which summarises the
//! relationship over a processed audio segment.
//!
//! The [`CorrelationValue`] enum maps the continuous [-1, +1] range into
//! five broadcast-meaningful categories.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

use std::collections::VecDeque;

/// Qualitative classification of a stereo correlation coefficient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrelationValue {
    /// Strong in-phase relationship (coefficient > 0.5).
    StrongPositive,
    /// Weak in-phase relationship (0.0 < coefficient ≤ 0.5).
    WeakPositive,
    /// No correlation (coefficient ≈ 0.0, within ±0.05).
    Zero,
    /// Weak out-of-phase relationship (-0.5 ≤ coefficient < 0.0).
    WeakNegative,
    /// Strong out-of-phase relationship (coefficient < -0.5).
    StrongNegative,
}

impl CorrelationValue {
    /// Classify a raw Pearson correlation coefficient.
    pub fn from_coefficient(c: f32) -> Self {
        if c > 0.5 {
            Self::StrongPositive
        } else if c > 0.05 {
            Self::WeakPositive
        } else if c >= -0.05 {
            Self::Zero
        } else if c >= -0.5 {
            Self::WeakNegative
        } else {
            Self::StrongNegative
        }
    }

    /// Return `true` if the correlation indicates potential phase cancellation
    /// when summed to mono.
    pub fn has_cancellation_risk(self) -> bool {
        matches!(self, Self::WeakNegative | Self::StrongNegative)
    }

    /// Return a short human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::StrongPositive => "Strong Positive",
            Self::WeakPositive => "Weak Positive",
            Self::Zero => "Zero",
            Self::WeakNegative => "Weak Negative",
            Self::StrongNegative => "Strong Negative",
        }
    }
}

impl std::fmt::Display for CorrelationValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Sliding-window stereo correlation analyser.
///
/// Uses a circular buffer to compute the Pearson correlation coefficient
/// between left and right channels over the most recent `window_size` samples.
pub struct CorrelationAnalyzer {
    window_size: usize,
    l_buf: VecDeque<f32>,
    r_buf: VecDeque<f32>,
    /// Running sum of L samples.
    sum_l: f64,
    /// Running sum of R samples.
    sum_r: f64,
    /// Running sum of L² samples.
    sum_l2: f64,
    /// Running sum of R² samples.
    sum_r2: f64,
    /// Running sum of L*R samples.
    sum_lr: f64,
    /// Correlation value from last compute.
    last_coefficient: f32,
    /// Total samples processed.
    total_samples: u64,
    /// Minimum coefficient seen.
    min_coefficient: f32,
    /// Maximum coefficient seen.
    max_coefficient: f32,
}

impl CorrelationAnalyzer {
    /// Create a new analyser.
    ///
    /// `window_size` is the number of stereo sample-pairs in the sliding window.
    /// A value of 0 defaults to 4096.
    pub fn new(window_size: usize) -> Self {
        let size = if window_size == 0 { 4096 } else { window_size };
        Self {
            window_size: size,
            l_buf: VecDeque::with_capacity(size),
            r_buf: VecDeque::with_capacity(size),
            sum_l: 0.0,
            sum_r: 0.0,
            sum_l2: 0.0,
            sum_r2: 0.0,
            sum_lr: 0.0,
            last_coefficient: 0.0,
            total_samples: 0,
            min_coefficient: f32::INFINITY,
            max_coefficient: f32::NEG_INFINITY,
        }
    }

    /// Process separate left and right channel slices (equal length).
    pub fn process(&mut self, left: &[f32], right: &[f32]) {
        let n = left.len().min(right.len());
        for i in 0..n {
            let l = left[i];
            let r = right[i];

            // Evict oldest sample when window is full
            if self.l_buf.len() >= self.window_size {
                let old_l = f64::from(self.l_buf.pop_front().unwrap_or(0.0));
                let old_r = f64::from(self.r_buf.pop_front().unwrap_or(0.0));
                self.sum_l -= old_l;
                self.sum_r -= old_r;
                self.sum_l2 -= old_l * old_l;
                self.sum_r2 -= old_r * old_r;
                self.sum_lr -= old_l * old_r;
            }

            let lf = f64::from(l);
            let rf = f64::from(r);
            self.l_buf.push_back(l);
            self.r_buf.push_back(r);
            self.sum_l += lf;
            self.sum_r += rf;
            self.sum_l2 += lf * lf;
            self.sum_r2 += rf * rf;
            self.sum_lr += lf * rf;
        }

        self.total_samples += n as u64;
        self.recompute_coefficient();
    }

    /// Process interleaved stereo samples (L, R, L, R, …).
    pub fn process_interleaved(&mut self, samples: &[f32]) {
        let pairs = samples.len() / 2;
        let mut left = Vec::with_capacity(pairs);
        let mut right = Vec::with_capacity(pairs);
        for chunk in samples.chunks_exact(2) {
            left.push(chunk[0]);
            right.push(chunk[1]);
        }
        self.process(&left, &right);
    }

    fn recompute_coefficient(&mut self) {
        let n = self.l_buf.len() as f64;
        if n < 2.0 {
            self.last_coefficient = 0.0;
            return;
        }
        let mean_l = self.sum_l / n;
        let mean_r = self.sum_r / n;
        let cov = self.sum_lr / n - mean_l * mean_r;
        let var_l = (self.sum_l2 / n - mean_l * mean_l).max(0.0);
        let var_r = (self.sum_r2 / n - mean_r * mean_r).max(0.0);
        let denom = var_l.sqrt() * var_r.sqrt();
        let coeff = if denom < 1e-12 { 0.0 } else { cov / denom };
        let clamped = coeff.clamp(-1.0, 1.0) as f32;

        self.last_coefficient = clamped;
        if clamped < self.min_coefficient {
            self.min_coefficient = clamped;
        }
        if clamped > self.max_coefficient {
            self.max_coefficient = clamped;
        }
    }

    /// Return the current Pearson correlation coefficient in [-1, +1].
    pub fn coefficient(&self) -> f32 {
        self.last_coefficient
    }

    /// Return the current qualitative [`CorrelationValue`].
    pub fn value(&self) -> CorrelationValue {
        CorrelationValue::from_coefficient(self.last_coefficient)
    }

    /// Return `true` if the current reading indicates phase cancellation risk.
    pub fn has_cancellation_risk(&self) -> bool {
        self.value().has_cancellation_risk()
    }

    /// Generate a [`CorrelationReport`] from all data processed so far.
    pub fn report(&self) -> CorrelationReport {
        let min = if self.min_coefficient.is_infinite() {
            0.0
        } else {
            self.min_coefficient
        };
        let max = if self.max_coefficient.is_infinite() {
            0.0
        } else {
            self.max_coefficient
        };
        CorrelationReport {
            current_coefficient: self.last_coefficient,
            current_value: CorrelationValue::from_coefficient(self.last_coefficient),
            min_coefficient: min,
            max_coefficient: max,
            total_samples: self.total_samples,
            window_size: self.window_size,
        }
    }

    /// Reset all state.
    pub fn reset(&mut self) {
        self.l_buf.clear();
        self.r_buf.clear();
        self.sum_l = 0.0;
        self.sum_r = 0.0;
        self.sum_l2 = 0.0;
        self.sum_r2 = 0.0;
        self.sum_lr = 0.0;
        self.last_coefficient = 0.0;
        self.total_samples = 0;
        self.min_coefficient = f32::INFINITY;
        self.max_coefficient = f32::NEG_INFINITY;
    }

    /// Return the total number of stereo sample-pairs processed.
    pub fn total_samples(&self) -> u64 {
        self.total_samples
    }
}

/// Summary report produced by [`CorrelationAnalyzer`].
#[derive(Debug, Clone)]
pub struct CorrelationReport {
    /// Coefficient at the time the report was generated.
    pub current_coefficient: f32,
    /// Qualitative value at report time.
    pub current_value: CorrelationValue,
    /// Minimum coefficient observed.
    pub min_coefficient: f32,
    /// Maximum coefficient observed.
    pub max_coefficient: f32,
    /// Total stereo sample-pairs processed.
    pub total_samples: u64,
    /// Window size used.
    pub window_size: usize,
}

impl CorrelationReport {
    /// Return `true` if the current coefficient indicates potential mono
    /// compatibility issues.
    pub fn has_mono_compatibility_risk(&self) -> bool {
        self.current_value.has_cancellation_risk()
    }

    /// Coefficient range (max - min) across the processed segment.
    pub fn coefficient_range(&self) -> f32 {
        self.max_coefficient - self.min_coefficient
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correlation_value_strong_positive() {
        assert_eq!(
            CorrelationValue::from_coefficient(0.9),
            CorrelationValue::StrongPositive
        );
    }

    #[test]
    fn correlation_value_weak_positive() {
        assert_eq!(
            CorrelationValue::from_coefficient(0.3),
            CorrelationValue::WeakPositive
        );
    }

    #[test]
    fn correlation_value_zero() {
        assert_eq!(
            CorrelationValue::from_coefficient(0.0),
            CorrelationValue::Zero
        );
        assert_eq!(
            CorrelationValue::from_coefficient(0.04),
            CorrelationValue::Zero
        );
    }

    #[test]
    fn correlation_value_weak_negative() {
        assert_eq!(
            CorrelationValue::from_coefficient(-0.3),
            CorrelationValue::WeakNegative
        );
    }

    #[test]
    fn correlation_value_strong_negative() {
        assert_eq!(
            CorrelationValue::from_coefficient(-0.9),
            CorrelationValue::StrongNegative
        );
    }

    #[test]
    fn cancellation_risk_for_negative_values() {
        assert!(CorrelationValue::WeakNegative.has_cancellation_risk());
        assert!(CorrelationValue::StrongNegative.has_cancellation_risk());
        assert!(!CorrelationValue::StrongPositive.has_cancellation_risk());
    }

    #[test]
    fn correlation_value_label() {
        assert_eq!(CorrelationValue::StrongPositive.label(), "Strong Positive");
        assert_eq!(CorrelationValue::Zero.label(), "Zero");
    }

    #[test]
    fn correlation_value_display() {
        assert_eq!(
            format!("{}", CorrelationValue::WeakNegative),
            "Weak Negative"
        );
    }

    #[test]
    fn analyzer_in_phase_signal_gives_positive_correlation() {
        let mut meter = CorrelationAnalyzer::new(1024);
        let signal: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.01).sin()).collect();
        meter.process(&signal, &signal);
        let coeff = meter.coefficient();
        assert!(coeff > 0.99, "expected near +1.0, got {}", coeff);
    }

    #[test]
    fn analyzer_out_of_phase_gives_negative_correlation() {
        let mut meter = CorrelationAnalyzer::new(1024);
        let signal: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.01).sin()).collect();
        let inverted: Vec<f32> = signal.iter().map(|&x| -x).collect();
        meter.process(&signal, &inverted);
        let coeff = meter.coefficient();
        assert!(coeff < -0.99, "expected near -1.0, got {}", coeff);
    }

    #[test]
    fn analyzer_processes_interleaved_correctly() {
        let mut meter = CorrelationAnalyzer::new(512);
        // identical L and R => coefficient should approach +1
        let samples: Vec<f32> = (0..1024)
            .map(|i| {
                let v = (i as f32 * 0.05).sin();
                v // both L and R get the same value via interleave below
            })
            .collect();
        let interleaved: Vec<f32> = samples.chunks(2).flat_map(|c| [c[0], c[0]]).collect();
        meter.process_interleaved(&interleaved);
        assert!(meter.coefficient() > 0.95);
    }

    #[test]
    fn analyzer_total_samples_increments() {
        let mut meter = CorrelationAnalyzer::new(64);
        let l = vec![0.1_f32; 32];
        let r = vec![0.1_f32; 32];
        meter.process(&l, &r);
        assert_eq!(meter.total_samples(), 32);
    }

    #[test]
    fn analyzer_reset_clears_state() {
        let mut meter = CorrelationAnalyzer::new(64);
        let sig: Vec<f32> = (0..64).map(|i| i as f32 * 0.01).collect();
        meter.process(&sig, &sig);
        meter.reset();
        assert_eq!(meter.total_samples(), 0);
        assert_eq!(meter.coefficient(), 0.0);
    }

    #[test]
    fn report_mono_compatibility_risk() {
        let mut meter = CorrelationAnalyzer::new(512);
        let l: Vec<f32> = (0..512).map(|i| (i as f32 * 0.02).sin()).collect();
        let r: Vec<f32> = l.iter().map(|&x| -x).collect();
        meter.process(&l, &r);
        let report = meter.report();
        assert!(report.has_mono_compatibility_risk());
    }

    #[test]
    fn report_coefficient_range_non_negative() {
        let mut meter = CorrelationAnalyzer::new(128);
        let sig: Vec<f32> = vec![0.5_f32; 128];
        meter.process(&sig, &sig);
        let report = meter.report();
        assert!(report.coefficient_range() >= 0.0);
    }

    #[test]
    fn report_window_size_matches() {
        let meter = CorrelationAnalyzer::new(2048);
        let report = meter.report();
        assert_eq!(report.window_size, 2048);
    }

    #[test]
    fn analyzer_default_window_size() {
        let meter = CorrelationAnalyzer::new(0);
        let report = meter.report();
        assert_eq!(report.window_size, 4096);
    }
}
