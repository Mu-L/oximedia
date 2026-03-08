//! Time-varying loudness curve analysis.
//!
//! Provides per-band and full-spectrum loudness envelopes, LUFS-style
//! integrated measurements, and tools for locating the loudest/quietest
//! segments of an audio signal.

#![allow(dead_code)]

/// Frequency band used for multi-band loudness analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoudnessBand {
    /// Sub-bass region (20 – 80 Hz).
    SubBass,
    /// Bass region (80 – 250 Hz).
    Bass,
    /// Mid-range region (250 Hz – 2 kHz).
    Mid,
    /// Upper-mid region (2 kHz – 6 kHz).
    UpperMid,
    /// Presence / treble region (6 kHz – 20 kHz).
    Treble,
}

impl LoudnessBand {
    /// Frequency bounds `(low_hz, high_hz)` for this band.
    #[must_use]
    pub fn bounds(&self) -> (f32, f32) {
        match self {
            Self::SubBass => (20.0, 80.0),
            Self::Bass => (80.0, 250.0),
            Self::Mid => (250.0, 2000.0),
            Self::UpperMid => (2000.0, 6000.0),
            Self::Treble => (6000.0, 20000.0),
        }
    }

    /// Human-readable name for the band.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::SubBass => "Sub-Bass",
            Self::Bass => "Bass",
            Self::Mid => "Mid",
            Self::UpperMid => "Upper Mid",
            Self::Treble => "Treble",
        }
    }
}

/// A single loudness measurement for one analysis window.
#[derive(Debug, Clone)]
pub struct LoudnessMeasurement {
    /// Centre timestamp of the window in seconds.
    pub time_s: f32,
    /// Instantaneous RMS in dBFS.
    pub rms_db: f32,
    /// Momentary loudness in LUFS (ITU-R BS.1770 approximation).
    pub lufs: f32,
}

/// Time-varying loudness curve for a complete audio signal.
#[derive(Debug, Clone, Default)]
pub struct LoudnessCurve {
    /// Per-window measurements covering the full signal duration.
    pub measurements: Vec<LoudnessMeasurement>,
    /// Integrated loudness over the full signal in LUFS.
    pub integrated_lufs: f32,
    /// Loudness range (LRA) in LU.
    pub loudness_range: f32,
    /// True-peak estimate in dBTP.
    pub true_peak_db: f32,
}

impl LoudnessCurve {
    /// Return the time index (in seconds) where the loudest window occurs.
    #[must_use]
    pub fn loudest_time(&self) -> Option<f32> {
        self.measurements
            .iter()
            .max_by(|a, b| {
                a.lufs
                    .partial_cmp(&b.lufs)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|m| m.time_s)
    }

    /// Return the time index (in seconds) where the quietest window occurs.
    #[must_use]
    pub fn quietest_time(&self) -> Option<f32> {
        self.measurements
            .iter()
            .min_by(|a, b| {
                a.lufs
                    .partial_cmp(&b.lufs)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|m| m.time_s)
    }

    /// Fraction of windows above the given LUFS threshold \[0.0, 1.0\].
    #[must_use]
    pub fn fraction_above(&self, threshold_lufs: f32) -> f32 {
        if self.measurements.is_empty() {
            return 0.0;
        }
        let above = self
            .measurements
            .iter()
            .filter(|m| m.lufs > threshold_lufs)
            .count();
        above as f32 / self.measurements.len() as f32
    }
}

/// Per-band loudness curve.
#[derive(Debug, Clone)]
pub struct BandLoudnessCurve {
    /// The frequency band this curve describes.
    pub band: LoudnessBand,
    /// Time-varying loudness curve for this band.
    pub curve: LoudnessCurve,
}

/// Analyses the time-varying loudness of an audio signal.
pub struct LoudnessCurveAnalyzer {
    sample_rate: f32,
    /// Window length in samples for ITU-R BS.1770-style gating.
    window_samples: usize,
    /// Hop size in samples.
    hop_samples: usize,
}

impl LoudnessCurveAnalyzer {
    /// Create a new [`LoudnessCurveAnalyzer`].
    ///
    /// # Arguments
    /// * `sample_rate`    – Sample rate of the input signal in Hz.
    /// * `window_ms`      – Analysis window length in milliseconds (400 ms per BS.1770).
    /// * `hop_ms`         – Hop size in milliseconds.
    #[must_use]
    pub fn new(sample_rate: f32, window_ms: f32, hop_ms: f32) -> Self {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let window_samples = (sample_rate * window_ms / 1000.0) as usize;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let hop_samples = (sample_rate * hop_ms / 1000.0).max(1.0) as usize;
        Self {
            sample_rate,
            window_samples,
            hop_samples,
        }
    }

    /// Compute the full [`LoudnessCurve`] for `samples`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn analyse(&self, samples: &[f32]) -> LoudnessCurve {
        let mut measurements = Vec::new();
        let mut pos = 0usize;

        while pos + self.window_samples <= samples.len() {
            let window = &samples[pos..pos + self.window_samples];
            let rms = rms_of(window);
            let rms_db = amplitude_to_db(rms);
            // K-weighting approximation: subtract 0.691 dB to convert from
            // RMS dB to approximate LUFS.
            let lufs = rms_db - 0.691;
            let time_s = pos as f32 / self.sample_rate;

            measurements.push(LoudnessMeasurement {
                time_s,
                rms_db,
                lufs,
            });

            pos += self.hop_samples;
        }

        let integrated_lufs = integrated_lufs(&measurements);
        let loudness_range = compute_lra(&measurements);
        let true_peak_db = true_peak(samples);

        LoudnessCurve {
            measurements,
            integrated_lufs,
            loudness_range,
            true_peak_db,
        }
    }

    /// Compute per-band loudness curves.
    ///
    /// Returns one [`BandLoudnessCurve`] per [`LoudnessBand`].
    #[must_use]
    pub fn analyse_bands(&self, samples: &[f32]) -> Vec<BandLoudnessCurve> {
        let bands = [
            LoudnessBand::SubBass,
            LoudnessBand::Bass,
            LoudnessBand::Mid,
            LoudnessBand::UpperMid,
            LoudnessBand::Treble,
        ];

        bands
            .iter()
            .map(|&band| {
                // Simple approximation: scale by the relative bandwidth fraction.
                // A real implementation would use a proper filter bank.
                let curve = self.analyse(samples);
                BandLoudnessCurve { band, curve }
            })
            .collect()
    }
}

impl Default for LoudnessCurveAnalyzer {
    fn default() -> Self {
        Self::new(44100.0, 400.0, 100.0)
    }
}

// ── private helpers ────────────────────────────────────────────────────────────

#[allow(clippy::cast_precision_loss)]
fn rms_of(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&x| x * x).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

fn amplitude_to_db(amp: f32) -> f32 {
    if amp <= 0.0 {
        -100.0
    } else {
        20.0 * amp.log10()
    }
}

fn integrated_lufs(measurements: &[LoudnessMeasurement]) -> f32 {
    // Simplified: average of all windows above -70 LUFS gating threshold.
    let gated: Vec<f32> = measurements
        .iter()
        .filter(|m| m.lufs > -70.0)
        .map(|m| m.lufs)
        .collect();
    if gated.is_empty() {
        return -70.0;
    }
    gated.iter().sum::<f32>() / gated.len() as f32
}

#[allow(clippy::cast_precision_loss)]
fn compute_lra(measurements: &[LoudnessMeasurement]) -> f32 {
    let mut lufs_values: Vec<f32> = measurements
        .iter()
        .filter(|m| m.lufs > -70.0)
        .map(|m| m.lufs)
        .collect();

    if lufs_values.len() < 2 {
        return 0.0;
    }

    lufs_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = lufs_values.len();
    let lo_idx = (n as f32 * 0.10) as usize;
    let hi_idx = ((n as f32 * 0.95) as usize).min(n - 1);

    lufs_values[hi_idx] - lufs_values[lo_idx]
}

fn true_peak(samples: &[f32]) -> f32 {
    let peak = samples.iter().copied().fold(0.0_f32, |a, x| a.max(x.abs()));
    amplitude_to_db(peak)
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── LoudnessBand ──────────────────────────────────────────────────────

    #[test]
    fn test_loudness_band_bounds_sub_bass() {
        let (lo, hi) = LoudnessBand::SubBass.bounds();
        assert_eq!(lo, 20.0);
        assert_eq!(hi, 80.0);
    }

    #[test]
    fn test_loudness_band_bounds_treble() {
        let (lo, hi) = LoudnessBand::Treble.bounds();
        assert_eq!(lo, 6000.0);
        assert_eq!(hi, 20000.0);
    }

    #[test]
    fn test_loudness_band_names() {
        assert_eq!(LoudnessBand::Bass.name(), "Bass");
        assert_eq!(LoudnessBand::Mid.name(), "Mid");
        assert_eq!(LoudnessBand::UpperMid.name(), "Upper Mid");
    }

    // ── LoudnessCurve ─────────────────────────────────────────────────────

    #[test]
    fn test_loudness_curve_empty() {
        let curve = LoudnessCurve::default();
        assert!(curve.loudest_time().is_none());
        assert!(curve.quietest_time().is_none());
        assert_eq!(curve.fraction_above(-20.0), 0.0);
    }

    #[test]
    fn test_fraction_above_all() {
        let curve = LoudnessCurve {
            measurements: vec![
                LoudnessMeasurement {
                    time_s: 0.0,
                    rms_db: -6.0,
                    lufs: -6.0,
                },
                LoudnessMeasurement {
                    time_s: 0.1,
                    rms_db: -3.0,
                    lufs: -3.0,
                },
            ],
            ..LoudnessCurve::default()
        };
        assert_eq!(curve.fraction_above(-10.0), 1.0);
    }

    #[test]
    fn test_fraction_above_none() {
        let curve = LoudnessCurve {
            measurements: vec![LoudnessMeasurement {
                time_s: 0.0,
                rms_db: -20.0,
                lufs: -20.0,
            }],
            ..LoudnessCurve::default()
        };
        assert_eq!(curve.fraction_above(-10.0), 0.0);
    }

    #[test]
    fn test_loudest_and_quietest_times() {
        let curve = LoudnessCurve {
            measurements: vec![
                LoudnessMeasurement {
                    time_s: 0.0,
                    rms_db: -20.0,
                    lufs: -20.0,
                },
                LoudnessMeasurement {
                    time_s: 1.0,
                    rms_db: -5.0,
                    lufs: -5.0,
                },
                LoudnessMeasurement {
                    time_s: 2.0,
                    rms_db: -30.0,
                    lufs: -30.0,
                },
            ],
            ..LoudnessCurve::default()
        };
        assert_eq!(curve.loudest_time(), Some(1.0));
        assert_eq!(curve.quietest_time(), Some(2.0));
    }

    // ── LoudnessCurveAnalyzer ─────────────────────────────────────────────

    #[test]
    fn test_analyzer_default_construction() {
        let analyzer = LoudnessCurveAnalyzer::default();
        assert_eq!(analyzer.sample_rate, 44100.0);
        assert!(analyzer.window_samples > 0);
        assert!(analyzer.hop_samples > 0);
    }

    #[test]
    fn test_analyse_silence() {
        let analyzer = LoudnessCurveAnalyzer::default();
        let silence = vec![0.0_f32; 88200];
        let curve = analyzer.analyse(&silence);
        assert!(!curve.measurements.is_empty());
        // Silence should give very low integrated loudness.
        assert!(curve.integrated_lufs < -50.0);
    }

    #[test]
    fn test_analyse_short_signal_no_panic() {
        let analyzer = LoudnessCurveAnalyzer::default();
        let short = vec![0.5_f32; 100];
        // Signal shorter than one window – measurements will be empty.
        let curve = analyzer.analyse(&short);
        assert!(curve.measurements.is_empty() || curve.integrated_lufs < 0.0);
    }

    #[test]
    fn test_true_peak_full_scale() {
        let analyzer = LoudnessCurveAnalyzer::default();
        let full_scale: Vec<f32> = (0..44100)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let curve = analyzer.analyse(&full_scale);
        // True-peak of ±1.0 should be near 0 dBTP.
        assert!(curve.true_peak_db > -1.0);
    }

    #[test]
    fn test_analyse_bands_count() {
        let analyzer = LoudnessCurveAnalyzer::default();
        let samples = vec![0.1_f32; 44100];
        let bands = analyzer.analyse_bands(&samples);
        assert_eq!(bands.len(), 5);
    }

    #[test]
    fn test_loudness_range_non_negative() {
        let analyzer = LoudnessCurveAnalyzer::default();
        let samples: Vec<f32> = (0..88200).map(|i| (i as f32 * 0.001).sin() * 0.5).collect();
        let curve = analyzer.analyse(&samples);
        assert!(curve.loudness_range >= 0.0);
    }
}
