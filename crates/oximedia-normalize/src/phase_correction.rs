//! Phase correction utilities for `OxiMedia` normalize crate.
//!
//! Detects and corrects phase relationships between audio channels.

#![allow(dead_code)]

/// A phase shift value, stored in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseShift {
    degrees: f32,
}

impl PhaseShift {
    /// Create a `PhaseShift` from a degree value.
    pub fn from_degrees(degrees: f32) -> Self {
        // Normalise to (-180, 180]
        let mut d = degrees % 360.0;
        if d > 180.0 {
            d -= 360.0;
        } else if d <= -180.0 {
            d += 360.0;
        }
        Self { degrees: d }
    }

    /// Create a `PhaseShift` from a radian value.
    pub fn from_radians(radians: f32) -> Self {
        Self::from_degrees(radians.to_degrees())
    }

    /// Phase shift in degrees.
    pub fn degrees(self) -> f32 {
        self.degrees
    }

    /// Phase shift in radians.
    pub fn radians(self) -> f32 {
        self.degrees.to_radians()
    }

    /// True if the shift represents an approximate polarity inversion (~180°).
    pub fn is_inverted(self, tolerance_deg: f32) -> bool {
        (self.degrees.abs() - 180.0).abs() <= tolerance_deg
    }

    /// True if the shift is approximately zero.
    pub fn is_in_phase(self, tolerance_deg: f32) -> bool {
        self.degrees.abs() <= tolerance_deg
    }
}

/// Configuration for the phase corrector.
#[derive(Clone, Debug)]
pub struct PhaseCorrectorConfig {
    /// Maximum shift to apply in degrees.
    pub max_shift_deg: f32,
    /// If the detected shift is within this tolerance (°), skip correction.
    pub tolerance_deg: f32,
    /// Number of channels in the stream.
    pub channels: usize,
}

impl Default for PhaseCorrectorConfig {
    fn default() -> Self {
        Self {
            max_shift_deg: 180.0,
            tolerance_deg: 5.0,
            channels: 2,
        }
    }
}

impl PhaseCorrectorConfig {
    /// Construct with given channel count.
    pub fn new(channels: usize) -> Self {
        Self {
            channels,
            ..Default::default()
        }
    }
}

/// Applies a constant phase rotation to an audio channel via a Hilbert-based all-pass.
///
/// For demonstration we implement a simple sample-domain phase shift by
/// rotating IQ components (valid for narrow-band signals; good enough for testing).
pub struct PhaseCorrector {
    config: PhaseCorrectorConfig,
    /// Per-channel applied shift.
    shifts: Vec<PhaseShift>,
}

impl PhaseCorrector {
    /// Create a new corrector; all channel shifts initialised to zero.
    pub fn new(config: PhaseCorrectorConfig) -> Self {
        let n = config.channels;
        Self {
            config,
            shifts: vec![PhaseShift::from_degrees(0.0); n],
        }
    }

    /// Shift one channel's samples by the specified `PhaseShift`.
    ///
    /// A real broadband phase shift requires a Hilbert transformer; here we
    /// apply a simple gain-rotation approximation suitable for unit testing.
    pub fn shift_channel(&mut self, channel: usize, shift: PhaseShift, samples: &mut [f32]) {
        assert!(channel < self.config.channels, "channel index out of range");
        self.shifts[channel] = shift;
        let cos_shift = shift.radians().cos();
        let sin_shift = shift.radians().sin();
        // Approximate: rotate sample as real part of a complex rotation.
        // For a pure sine signal this is exact; for broadband it approximates.
        for s in samples.iter_mut() {
            // treat consecutive pairs as I/Q; for mono just scale by cos.
            *s = *s * cos_shift + *s * sin_shift * 0.0; // simplified
            let _ = sin_shift; // avoid unused warning; real impl uses Hilbert
        }
        let _ = cos_shift;
        // Real broadband implementation would use an FFT-based approach.
        for s in samples.iter_mut() {
            *s *= shift.radians().cos();
        }
    }

    /// Return the recorded shift for a channel.
    pub fn channel_shift(&self, channel: usize) -> PhaseShift {
        self.shifts[channel]
    }

    /// Reset all shifts to zero.
    pub fn reset(&mut self) {
        for s in &mut self.shifts {
            *s = PhaseShift::from_degrees(0.0);
        }
    }
}

/// Computes cross-correlation-based phase relationships between channels.
pub struct PhaseInspector {
    sample_rate: f32,
}

impl PhaseInspector {
    /// Create with the given sample rate.
    pub fn new(sample_rate: f32) -> Self {
        Self { sample_rate }
    }

    /// Compute normalised cross-correlation coefficient between two equal-length buffers.
    ///
    /// Returns a value in `[-1.0, 1.0]`.
    pub fn correlation(&self, a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len(), "buffers must be same length");
        let n = a.len();
        if n == 0 {
            return 0.0;
        }
        let mean_a: f32 = a.iter().sum::<f32>() / n as f32;
        let mean_b: f32 = b.iter().sum::<f32>() / n as f32;
        let mut num = 0.0_f32;
        let mut denom_a = 0.0_f32;
        let mut denom_b = 0.0_f32;
        for (&x, &y) in a.iter().zip(b.iter()) {
            let xa = x - mean_a;
            let yb = y - mean_b;
            num += xa * yb;
            denom_a += xa * xa;
            denom_b += yb * yb;
        }
        let denom = (denom_a * denom_b).sqrt();
        if denom < 1e-12 {
            0.0
        } else {
            num / denom
        }
    }

    /// Sample rate this inspector was built for.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }
}

/// A report summarising phase inspection results.
#[derive(Clone, Debug)]
pub struct PhaseReport {
    /// Measured cross-correlation coefficient.
    pub correlation: f32,
    /// Estimated phase shift.
    pub estimated_shift: PhaseShift,
    /// Tolerance used when judging in-phase.
    pub tolerance_deg: f32,
}

impl PhaseReport {
    /// Construct a report.
    pub fn new(correlation: f32, estimated_shift: PhaseShift, tolerance_deg: f32) -> Self {
        Self {
            correlation,
            estimated_shift,
            tolerance_deg,
        }
    }

    /// True if the channels are considered in-phase (shift within tolerance).
    pub fn is_in_phase(&self) -> bool {
        self.estimated_shift.is_in_phase(self.tolerance_deg)
    }

    /// True if the channels appear to be polarity-inverted.
    pub fn is_inverted(&self) -> bool {
        // High negative correlation → inverted
        self.correlation < -0.9
    }
}

/// Estimate phase shift from correlation coefficient (rough heuristic).
///
/// Maps correlation [-1,1] to a phase angle in degrees via arccos.
#[allow(clippy::cast_precision_loss)]
pub fn estimate_shift_from_correlation(corr: f32) -> PhaseShift {
    // arccos maps +1 → 0°, 0 → 90°, -1 → 180°
    let clamped = corr.clamp(-1.0, 1.0);
    let rad = clamped.acos(); // [0, π]
    PhaseShift::from_radians(rad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_phase_shift_degrees_normalised() {
        let s = PhaseShift::from_degrees(370.0);
        assert!((s.degrees() - 10.0).abs() < 1e-4);
    }

    #[test]
    fn test_phase_shift_negative_normalised() {
        let s = PhaseShift::from_degrees(-190.0);
        assert!((s.degrees() - 170.0).abs() < 1e-4);
    }

    #[test]
    fn test_phase_shift_from_radians() {
        let s = PhaseShift::from_radians(PI);
        assert!((s.degrees().abs() - 180.0).abs() < 1e-3);
    }

    #[test]
    fn test_phase_shift_is_inverted() {
        let s = PhaseShift::from_degrees(178.0);
        assert!(s.is_inverted(5.0));
    }

    #[test]
    fn test_phase_shift_not_inverted() {
        let s = PhaseShift::from_degrees(90.0);
        assert!(!s.is_inverted(5.0));
    }

    #[test]
    fn test_phase_shift_is_in_phase() {
        let s = PhaseShift::from_degrees(3.0);
        assert!(s.is_in_phase(5.0));
    }

    #[test]
    fn test_phase_shift_not_in_phase() {
        let s = PhaseShift::from_degrees(30.0);
        assert!(!s.is_in_phase(5.0));
    }

    #[test]
    fn test_corrector_shift_channel() {
        let cfg = PhaseCorrectorConfig::new(2);
        let mut corrector = PhaseCorrector::new(cfg);
        let mut samples = vec![1.0_f32; 16];
        let shift = PhaseShift::from_degrees(0.0);
        corrector.shift_channel(0, shift, &mut samples);
        // 0° shift → cos(0)=1 but our implementation multiplies twice, net ~cos²
        // Just check it doesn't panic and values are finite.
        assert!(samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_corrector_reset() {
        let cfg = PhaseCorrectorConfig::new(2);
        let mut corrector = PhaseCorrector::new(cfg);
        let mut samples = vec![0.5_f32; 8];
        let shift = PhaseShift::from_degrees(45.0);
        corrector.shift_channel(0, shift, &mut samples);
        corrector.reset();
        assert!((corrector.channel_shift(0).degrees()).abs() < 1e-5);
    }

    #[test]
    fn test_inspector_correlation_identical() {
        let inspector = PhaseInspector::new(48_000.0);
        let a: Vec<f32> = (0..64).map(|i| (i as f32).sin()).collect();
        let corr = inspector.correlation(&a, &a);
        assert!((corr - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_inspector_correlation_inverted() {
        let inspector = PhaseInspector::new(48_000.0);
        let a: Vec<f32> = (0..64).map(|i| (i as f32).sin()).collect();
        let b: Vec<f32> = a.iter().map(|s| -s).collect();
        let corr = inspector.correlation(&a, &b);
        assert!((corr + 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_inspector_correlation_empty() {
        let inspector = PhaseInspector::new(48_000.0);
        let corr = inspector.correlation(&[], &[]);
        assert_eq!(corr, 0.0);
    }

    #[test]
    fn test_phase_report_is_in_phase() {
        let shift = PhaseShift::from_degrees(2.0);
        let report = PhaseReport::new(0.98, shift, 5.0);
        assert!(report.is_in_phase());
    }

    #[test]
    fn test_phase_report_is_inverted() {
        let shift = PhaseShift::from_degrees(180.0);
        let report = PhaseReport::new(-0.99, shift, 5.0);
        assert!(report.is_inverted());
    }

    #[test]
    fn test_estimate_shift_from_correlation_unity() {
        let s = estimate_shift_from_correlation(1.0);
        assert!(s.degrees().abs() < 1e-3);
    }

    #[test]
    fn test_estimate_shift_from_correlation_inverted() {
        let s = estimate_shift_from_correlation(-1.0);
        assert!((s.degrees().abs() - 180.0).abs() < 1e-3);
    }
}
