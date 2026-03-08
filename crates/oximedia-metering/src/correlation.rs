//! Stereo correlation and phase analysis for audio metering.

use std::collections::VecDeque;

/// Stereo correlation coefficient (Pearson correlation between L and R).
///
/// Range: -1.0 (out of phase) to +1.0 (in phase).
pub struct CorrelationMeter {
    window_size: usize,
    l_buffer: VecDeque<f32>,
    r_buffer: VecDeque<f32>,
    current_correlation: f32,
}

impl CorrelationMeter {
    /// Create a new correlation meter with the given window size.
    ///
    /// Default window size is 4096 samples.
    #[must_use]
    pub fn new(window_size: usize) -> Self {
        let size = if window_size == 0 { 4096 } else { window_size };
        Self {
            window_size: size,
            l_buffer: VecDeque::with_capacity(size),
            r_buffer: VecDeque::with_capacity(size),
            current_correlation: 0.0,
        }
    }

    /// Process a block of stereo samples.
    ///
    /// `l` and `r` must have the same length.
    pub fn process(&mut self, l: &[f32], r: &[f32]) {
        for (&lsample, &rsample) in l.iter().zip(r.iter()) {
            if self.l_buffer.len() >= self.window_size {
                self.l_buffer.pop_front();
                self.r_buffer.pop_front();
            }
            self.l_buffer.push_back(lsample);
            self.r_buffer.push_back(rsample);
        }
        self.update_correlation();
    }

    fn update_correlation(&mut self) {
        let n = self.l_buffer.len();
        if n < 2 {
            self.current_correlation = 0.0;
            return;
        }

        let n_f = n as f64;
        let mean_l: f64 = self.l_buffer.iter().map(|&x| f64::from(x)).sum::<f64>() / n_f;
        let mean_r: f64 = self.r_buffer.iter().map(|&x| f64::from(x)).sum::<f64>() / n_f;

        let mut cov = 0.0_f64;
        let mut var_l = 0.0_f64;
        let mut var_r = 0.0_f64;

        for (&lv, &rv) in self.l_buffer.iter().zip(self.r_buffer.iter()) {
            let dl = f64::from(lv) - mean_l;
            let dr = f64::from(rv) - mean_r;
            cov += dl * dr;
            var_l += dl * dl;
            var_r += dr * dr;
        }

        let denom = (var_l * var_r).sqrt();
        self.current_correlation = if denom > 1e-15 {
            (cov / denom).clamp(-1.0, 1.0) as f32
        } else {
            0.0
        };
    }

    /// Current Pearson correlation between L and R channels.
    #[must_use]
    pub fn correlation(&self) -> f32 {
        self.current_correlation
    }

    /// Returns `true` if the signal is mono-compatible (correlation > 0.0).
    #[must_use]
    pub fn is_mono_compatible(&self) -> bool {
        self.current_correlation > 0.0
    }

    /// Describe the phase relationship between L and R.
    #[must_use]
    pub fn phase_relationship(&self) -> PhaseRelationship {
        PhaseRelationship::from_correlation(self.current_correlation)
    }
}

/// Qualitative description of the phase relationship between stereo channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseRelationship {
    /// correlation > 0.5 – channels are substantially in phase.
    InPhase,
    /// 0.0 < correlation <= 0.5 – channels are somewhat in phase.
    NearInPhase,
    /// -0.1 < correlation <= 0.0 – channels are essentially uncorrelated.
    Uncorrelated,
    /// correlation <= -0.1 – channels are out of phase.
    OutOfPhase,
}

impl PhaseRelationship {
    /// Derive a `PhaseRelationship` from a correlation value.
    #[must_use]
    pub fn from_correlation(correlation: f32) -> Self {
        if correlation > 0.5 {
            Self::InPhase
        } else if correlation > 0.0 {
            Self::NearInPhase
        } else if correlation > -0.1 {
            Self::Uncorrelated
        } else {
            Self::OutOfPhase
        }
    }

    /// Human-readable name for this relationship.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::InPhase => "In Phase",
            Self::NearInPhase => "Near In Phase",
            Self::Uncorrelated => "Uncorrelated",
            Self::OutOfPhase => "Out of Phase",
        }
    }

    /// Returns `true` if this relationship is problematic for mono compatibility.
    ///
    /// `OutOfPhase` is problematic because summing L+R causes cancellation.
    #[must_use]
    pub fn is_problematic(self) -> bool {
        matches!(self, Self::OutOfPhase)
    }
}

/// A point in a goniometer (Lissajous) display for stereo imaging.
#[derive(Debug, Clone, Copy)]
pub struct GoniometerPoint {
    /// Mid channel: `M = (L + R) / sqrt(2)`.
    pub mid: f32,
    /// Side channel: `S = (L - R) / sqrt(2)`.
    pub side: f32,
}

impl GoniometerPoint {
    /// Create a goniometer point from left and right channel samples.
    #[must_use]
    pub fn from_lr(l: f32, r: f32) -> Self {
        let sqrt2 = std::f32::consts::SQRT_2;
        Self {
            mid: (l + r) / sqrt2,
            side: (l - r) / sqrt2,
        }
    }

    /// Magnitude of this point from the origin.
    #[must_use]
    pub fn magnitude(self) -> f32 {
        (self.mid * self.mid + self.side * self.side).sqrt()
    }

    /// Angle in degrees from the vertical (centre) axis.
    #[must_use]
    pub fn angle_deg(self) -> f32 {
        self.side.atan2(self.mid).to_degrees()
    }
}

/// Goniometer for stereo width visualization (Lissajous display).
pub struct Goniometer {
    history: VecDeque<GoniometerPoint>,
    max_history: usize,
}

impl Goniometer {
    /// Create a new goniometer with the given history size.
    #[must_use]
    pub fn new(max_history: usize) -> Self {
        let size = if max_history == 0 { 1 } else { max_history };
        Self {
            history: VecDeque::with_capacity(size),
            max_history: size,
        }
    }

    /// Process a block of stereo samples.
    pub fn process(&mut self, l: &[f32], r: &[f32]) {
        for (&lv, &rv) in l.iter().zip(r.iter()) {
            if self.history.len() >= self.max_history {
                self.history.pop_front();
            }
            self.history.push_back(GoniometerPoint::from_lr(lv, rv));
        }
    }

    /// Return the current history of goniometer points.
    #[must_use]
    pub fn points(&self) -> &VecDeque<GoniometerPoint> {
        &self.history
    }

    /// Stereo width estimate: `mean(|side|) / (mean(|mid|) + mean(|side|))`.
    ///
    /// Range \[0, 1\]: 0 = mono, approaching 1 = very wide.
    #[must_use]
    pub fn stereo_width(&self) -> f32 {
        let n = self.history.len();
        if n == 0 {
            return 0.0;
        }
        let mean_mid: f32 = self.history.iter().map(|p| p.mid.abs()).sum::<f32>() / n as f32;
        let mean_side: f32 = self.history.iter().map(|p| p.side.abs()).sum::<f32>() / n as f32;
        let total = mean_mid + mean_side;
        if total > 0.0 {
            mean_side / total
        } else {
            0.0
        }
    }

    /// Returns `true` if stereo width is greater than 0.5.
    #[must_use]
    pub fn is_wide(&self) -> bool {
        self.stereo_width() > 0.5
    }
}

/// A frequency band definition with RMS and peak levels.
#[derive(Debug, Clone)]
pub struct FrequencyBand {
    /// Human-readable name of this band.
    pub name: String,
    /// Lower frequency boundary in Hz.
    pub low_hz: f32,
    /// Upper frequency boundary in Hz.
    pub high_hz: f32,
    /// Smoothed RMS level in dBFS.
    pub rms_db: f32,
    /// Peak level in dBFS.
    pub peak_db: f32,
}

impl FrequencyBand {
    /// Create a new frequency band.
    fn make(name: &str, low_hz: f32, high_hz: f32) -> Self {
        Self {
            name: name.to_string(),
            low_hz,
            high_hz,
            rms_db: f32::NEG_INFINITY,
            peak_db: f32::NEG_INFINITY,
        }
    }

    /// Sub-bass band: 20–60 Hz.
    #[must_use]
    pub fn sub_bass() -> Self {
        Self::make("sub_bass", 20.0, 60.0)
    }

    /// Bass band: 60–250 Hz.
    #[must_use]
    pub fn bass() -> Self {
        Self::make("bass", 60.0, 250.0)
    }

    /// Low-mid band: 250–500 Hz.
    #[must_use]
    pub fn low_mid() -> Self {
        Self::make("low_mid", 250.0, 500.0)
    }

    /// Mid band: 500–2000 Hz.
    #[must_use]
    pub fn mid() -> Self {
        Self::make("mid", 500.0, 2000.0)
    }

    /// High-mid band: 2000–4000 Hz.
    #[must_use]
    pub fn high_mid() -> Self {
        Self::make("high_mid", 2000.0, 4000.0)
    }

    /// Presence band: 4000–6000 Hz.
    #[must_use]
    pub fn presence() -> Self {
        Self::make("presence", 4000.0, 6000.0)
    }

    /// Air band: 6000–20000 Hz.
    #[must_use]
    pub fn air() -> Self {
        Self::make("air", 6000.0, 20000.0)
    }
}

/// Multi-band energy meter using simple energy estimation per frequency band.
///
/// Note: This implementation uses a simplistic time-domain approach. For
/// production use, a proper filter bank or FFT-based approach is recommended.
#[derive(Debug, Clone)]
pub struct MultibandMeter {
    bands: Vec<FrequencyBand>,
    sample_rate: u32,
    // Per-band smoothed squared-sum accumulator
    smoothed_sq: Vec<f32>,
}

impl MultibandMeter {
    /// Create a standard 7-band meter (sub-bass, bass, low-mid, mid, high-mid, presence, air).
    #[must_use]
    pub fn standard(sample_rate: u32) -> Self {
        let bands = vec![
            FrequencyBand::sub_bass(),
            FrequencyBand::bass(),
            FrequencyBand::low_mid(),
            FrequencyBand::mid(),
            FrequencyBand::high_mid(),
            FrequencyBand::presence(),
            FrequencyBand::air(),
        ];
        let n = bands.len();
        Self {
            bands,
            sample_rate,
            smoothed_sq: vec![0.0; n],
        }
    }

    /// Process a mono block of samples, updating band levels.
    ///
    /// This uses a goertzel-style energy estimation per band centre frequency
    /// to avoid requiring a full FFT. For a realistic meter the caller should
    /// pass reasonably-sized blocks (e.g. 1024 samples).
    pub fn process_block(&mut self, samples: &[f32]) {
        let sr = self.sample_rate as f32;
        let alpha = 0.9_f32; // smoothing

        for (i, band) in self.bands.iter_mut().enumerate() {
            // Estimate band energy using Goertzel algorithm at band centre.
            let centre = (band.low_hz + band.high_hz) / 2.0;
            let energy = goertzel_energy(samples, centre, sr);

            // Smooth
            self.smoothed_sq[i] = alpha * self.smoothed_sq[i] + (1.0 - alpha) * energy;

            let rms = self.smoothed_sq[i].sqrt();
            band.rms_db = if rms > 1e-10 {
                20.0 * rms.log10()
            } else {
                f32::NEG_INFINITY
            };

            // Peak: max |sample|
            let peak_linear = samples.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
            band.peak_db = if peak_linear > 1e-10 {
                20.0 * peak_linear.log10()
            } else {
                f32::NEG_INFINITY
            };
        }
    }

    /// Return a slice of all frequency bands.
    #[must_use]
    pub fn bands(&self) -> &[FrequencyBand] {
        &self.bands
    }

    /// Look up a band by name.
    #[must_use]
    pub fn band_by_name(&self, name: &str) -> Option<&FrequencyBand> {
        self.bands.iter().find(|b| b.name == name)
    }
}

/// Goertzel algorithm: estimate power of a single frequency in a block.
fn goertzel_energy(samples: &[f32], freq: f32, sample_rate: f32) -> f32 {
    use std::f32::consts::PI;
    if samples.is_empty() || sample_rate <= 0.0 {
        return 0.0;
    }
    let k = (freq / sample_rate * samples.len() as f32).round() as usize;
    let omega = 2.0 * PI * k as f32 / samples.len() as f32;
    let coeff = 2.0 * omega.cos();
    let mut s_prev = 0.0_f32;
    let mut s_prev2 = 0.0_f32;

    for &sample in samples {
        let s = sample + coeff * s_prev - s_prev2;
        s_prev2 = s_prev;
        s_prev = s;
    }

    // Power = s_prev^2 + s_prev2^2 - coeff * s_prev * s_prev2
    let power = s_prev * s_prev + s_prev2 * s_prev2 - coeff * s_prev * s_prev2;
    power / samples.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_correlation_identical_signals() {
        let mut meter = CorrelationMeter::new(4096);
        let signal: Vec<f32> = (0..4096)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        meter.process(&signal, &signal);
        let corr = meter.correlation();
        assert!(
            corr > 0.99,
            "Identical signals should have correlation ~1.0, got {corr}"
        );
    }

    #[test]
    fn test_correlation_inverted_signals() {
        let mut meter = CorrelationMeter::new(4096);
        let signal: Vec<f32> = (0..4096)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let inverted: Vec<f32> = signal.iter().map(|&s| -s).collect();
        meter.process(&signal, &inverted);
        let corr = meter.correlation();
        assert!(
            corr < -0.99,
            "Inverted signals should have correlation ~-1.0, got {corr}"
        );
    }

    #[test]
    fn test_correlation_orthogonal() {
        let mut meter = CorrelationMeter::new(4096);
        // sin and cos are orthogonal over full period(s)
        let l: Vec<f32> = (0..4096)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();
        let r: Vec<f32> = (0..4096)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).cos())
            .collect();
        meter.process(&l, &r);
        let corr = meter.correlation();
        assert!(
            corr.abs() < 0.15,
            "Orthogonal signals should have near-zero correlation, got {corr}"
        );
    }

    #[test]
    fn test_phase_relationship_in_phase() {
        // Manually set via from_correlation
        let rel = PhaseRelationship::from_correlation(0.8);
        assert_eq!(rel, PhaseRelationship::InPhase);
    }

    #[test]
    fn test_phase_relationship_out_of_phase() {
        let rel = PhaseRelationship::from_correlation(-0.5);
        assert_eq!(rel, PhaseRelationship::OutOfPhase);
        assert!(rel.is_problematic());
    }

    #[test]
    fn test_goniometer_point_from_lr() {
        let pt = GoniometerPoint::from_lr(1.0, 0.0);
        let sqrt2_inv = 1.0 / std::f32::consts::SQRT_2;
        assert!(
            (pt.mid - sqrt2_inv).abs() < 1e-6,
            "mid should be 1/sqrt(2) ≈ 0.707, got {}",
            pt.mid
        );
        assert!(
            (pt.side - sqrt2_inv).abs() < 1e-6,
            "side should be 1/sqrt(2) ≈ 0.707, got {}",
            pt.side
        );
    }

    #[test]
    fn test_goniometer_stereo_width_mono() {
        let mut g = Goniometer::new(4096);
        // Mono signal: L == R → side = 0 → width ≈ 0
        let mono: Vec<f32> = vec![0.5; 4096];
        g.process(&mono, &mono);
        let w = g.stereo_width();
        assert!(w < 0.05, "Mono signal should have width ~0, got {w}");
    }

    #[test]
    fn test_multiband_meter_band_names() {
        let meter = MultibandMeter::standard(44100);
        assert!(
            meter.band_by_name("bass").is_some(),
            "Should have 'bass' band"
        );
    }

    #[test]
    fn test_multiband_meter_standard_has_7_bands() {
        let meter = MultibandMeter::standard(44100);
        assert_eq!(meter.bands().len(), 7, "Standard meter should have 7 bands");
    }

    #[test]
    fn test_frequency_band_sub_bass() {
        let band = FrequencyBand::sub_bass();
        assert_eq!(band.low_hz, 20.0);
        assert_eq!(band.high_hz, 60.0);
    }
}
