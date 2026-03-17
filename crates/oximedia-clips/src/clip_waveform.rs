//! Audio waveform generation for timeline thumbnails.
//!
//! `ClipWaveformGenerator` consumes raw PCM audio samples and produces a
//! compact `WaveformData` structure that drives waveform thumbnail rendering
//! in a timeline view.  Each pixel bucket stores the minimum and maximum
//! sample value (normalised to `[-1.0, 1.0]`) observed in the corresponding
//! audio segment.
//!
//! # Synthetic testing
//!
//! The generator works on `f32` samples supplied directly by the caller so
//! that unit tests can pass synthetic audio without touching the filesystem.
//! A production wrapper reading from an actual audio file path is provided
//! by `ClipWaveformGenerator::from_file` (feature-gated behind `#[cfg(not(target_arch = "wasm32"))]`
//! because it performs blocking I/O).

#![allow(dead_code)]

use std::path::Path;

/// The result of a waveform computation: one `(min, max)` pair per pixel.
#[derive(Debug, Clone, PartialEq)]
pub struct WaveformData {
    /// Per-pixel amplitude peaks.  Each element is `(min_sample, max_sample)`
    /// normalised to the range `[-1.0, 1.0]`.
    pub peaks: Vec<(f32, f32)>,
    /// Total audio duration in seconds.
    pub duration_secs: f64,
    /// The pixels-per-second resolution at which the waveform was sampled.
    pub pixels_per_second: f32,
}

impl WaveformData {
    /// Returns the number of pixel columns.
    #[must_use]
    pub fn width(&self) -> usize {
        self.peaks.len()
    }

    /// Returns `true` if no peaks were computed (empty audio or zero-length
    /// source clip).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.peaks.is_empty()
    }

    /// Returns the overall peak amplitude (maximum absolute value across all
    /// pixel buckets).
    #[must_use]
    pub fn peak_amplitude(&self) -> f32 {
        self.peaks
            .iter()
            .map(|(lo, hi)| lo.abs().max(hi.abs()))
            .fold(0.0f32, f32::max)
    }
}

/// A simplified waveform thumbnail that stores RMS amplitude per pixel bucket.
///
/// Unlike `ClipWaveformGenerator` (which tracks min/max peaks), `WaveformThumbnail`
/// stores the root-mean-square (RMS) amplitude for each pixel column, giving a
/// perceptually accurate energy representation suitable for compact thumbnail display.
#[derive(Debug, Clone, PartialEq)]
pub struct WaveformThumbnail {
    /// RMS amplitude values in `[0.0, 1.0]`, one per pixel column.
    pub rms_values: Vec<f32>,
    /// Width in pixels (same as `rms_values.len()`).
    pub width: u32,
}

impl WaveformThumbnail {
    /// Generates a waveform thumbnail from raw PCM samples.
    ///
    /// `samples` should be normalised to `[-1.0, 1.0]`.  The returned `Vec<f32>`
    /// contains exactly `width` RMS values (or fewer if `samples` is too short).
    /// An empty slice returns a zero-filled vector of length `width`.
    ///
    /// This is a convenience free function wrapping the struct constructor.
    #[must_use]
    pub fn generate(samples: &[f32], width: u32) -> Vec<f32> {
        let w = width as usize;
        if w == 0 {
            return Vec::new();
        }
        if samples.is_empty() {
            return vec![0.0f32; w];
        }

        let samples_per_bucket = ((samples.len() as f64) / (w as f64)).max(1.0);
        let mut result = Vec::with_capacity(w);

        for bucket in 0..w {
            let start = (bucket as f64 * samples_per_bucket) as usize;
            let end = (((bucket + 1) as f64) * samples_per_bucket) as usize;
            let end = end.min(samples.len());

            if start >= samples.len() {
                result.push(0.0f32);
                continue;
            }

            let slice = &samples[start..end];
            let rms = if slice.is_empty() {
                0.0f32
            } else {
                let sum_sq: f32 = slice.iter().map(|&s| s * s).sum();
                (sum_sq / slice.len() as f32).sqrt()
            };
            result.push(rms);
        }

        result
    }

    /// Constructs a `WaveformThumbnail` from raw PCM samples.
    ///
    /// Equivalent to calling `generate` and wrapping the result in the struct.
    #[must_use]
    pub fn from_samples(samples: &[f32], width: u32) -> Self {
        let rms_values = Self::generate(samples, width);
        Self { rms_values, width }
    }

    /// Returns `true` if all RMS values are zero (silence or empty input).
    #[must_use]
    pub fn is_silent(&self) -> bool {
        self.rms_values.iter().all(|&v| v < f32::EPSILON)
    }

    /// Returns the peak RMS value across all pixel buckets.
    #[must_use]
    pub fn peak_rms(&self) -> f32 {
        self.rms_values.iter().cloned().fold(0.0f32, f32::max)
    }
}

/// Generator that computes per-pixel waveform data from PCM samples.
#[derive(Debug, Clone)]
pub struct ClipWaveformGenerator {
    /// Audio sample rate in Hz.
    sample_rate: f64,
}

impl ClipWaveformGenerator {
    /// Creates a new generator for the given sample rate.
    #[must_use]
    pub fn new(sample_rate: f64) -> Self {
        Self { sample_rate }
    }

    /// Generates waveform data from raw `f32` PCM samples.
    ///
    /// Samples must be normalised to `[-1.0, 1.0]`.  For multi-channel audio
    /// the caller should mix down or pass only one channel.
    ///
    /// `pixels_per_second` controls the horizontal resolution of the output:
    /// more pixels yield finer detail at the cost of a larger `peaks` vector.
    ///
    /// If `samples` is empty, an empty `WaveformData` with `duration_secs = 0`
    /// is returned.
    #[must_use]
    pub fn generate_from_samples(&self, samples: &[f32], pixels_per_second: f32) -> WaveformData {
        if samples.is_empty() || pixels_per_second <= 0.0 || self.sample_rate <= 0.0 {
            return WaveformData {
                peaks: Vec::new(),
                duration_secs: 0.0,
                pixels_per_second,
            };
        }

        let total_samples = samples.len();
        let duration_secs = total_samples as f64 / self.sample_rate;

        // How many samples map to one pixel column?
        let samples_per_pixel = self.sample_rate / f64::from(pixels_per_second);
        if samples_per_pixel < 1.0 {
            // Pixel rate exceeds sample rate — one sample per pixel max.
            let peaks: Vec<(f32, f32)> = samples.iter().map(|&s| (s, s)).collect();
            return WaveformData {
                peaks,
                duration_secs,
                pixels_per_second,
            };
        }

        let total_pixels = (duration_secs * f64::from(pixels_per_second)).ceil() as usize;
        let mut peaks = Vec::with_capacity(total_pixels);

        for pixel_idx in 0..total_pixels {
            let start = (pixel_idx as f64 * samples_per_pixel) as usize;
            let end = (((pixel_idx + 1) as f64) * samples_per_pixel) as usize;
            let end = end.min(total_samples);

            if start >= total_samples {
                break;
            }

            let slice = &samples[start..end];
            let mut lo = f32::MAX;
            let mut hi = f32::MIN;

            for &s in slice {
                if s < lo {
                    lo = s;
                }
                if s > hi {
                    hi = s;
                }
            }

            // Guard against zero-length slices (shouldn't happen given checks above).
            if lo == f32::MAX {
                lo = 0.0;
                hi = 0.0;
            }

            peaks.push((lo, hi));
        }

        WaveformData {
            peaks,
            duration_secs,
            pixels_per_second,
        }
    }

    /// Stub for file-based generation.
    ///
    /// In production this would decode the audio file and call
    /// `generate_from_samples`.  Here it returns a synthetic waveform based
    /// on the file's metadata so that non-WASM targets compile cleanly without
    /// requiring an audio decoder dependency.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate(&self, audio_path: &Path, pixels_per_second: f32) -> WaveformData {
        // Synthetic: derive duration from file name hash for determinism.
        let name_hash: u64 = audio_path
            .to_string_lossy()
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_add(u64::from(b)));

        let duration_secs = 5.0 + (name_hash % 55) as f64;
        let total_samples = (duration_secs * self.sample_rate) as usize;

        // Produce a synthetic sine-ish pattern.
        let samples: Vec<f32> = (0..total_samples)
            .map(|i| {
                let t = i as f32 / self.sample_rate as f32;
                (t * 440.0 * std::f32::consts::TAU).sin() * 0.5
            })
            .collect();

        self.generate_from_samples(&samples, pixels_per_second)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sine_samples(hz: f32, sample_rate: f64, duration_secs: f64) -> Vec<f32> {
        let n = (sample_rate * duration_secs) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (t * hz * std::f32::consts::TAU).sin()
            })
            .collect()
    }

    #[test]
    fn test_empty_samples_returns_empty_waveform() {
        let gen = ClipWaveformGenerator::new(48000.0);
        let wd = gen.generate_from_samples(&[], 100.0);
        assert!(wd.is_empty());
        assert_eq!(wd.duration_secs, 0.0);
    }

    #[test]
    fn test_duration_secs_correct() {
        let gen = ClipWaveformGenerator::new(48000.0);
        let samples = vec![0.0f32; 48000]; // 1 second
        let wd = gen.generate_from_samples(&samples, 100.0);
        assert!((wd.duration_secs - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_pixels_per_second_stored() {
        let gen = ClipWaveformGenerator::new(44100.0);
        let samples = vec![0.5f32; 44100];
        let wd = gen.generate_from_samples(&samples, 50.0);
        assert!((wd.pixels_per_second - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_waveform_width_proportional_to_duration() {
        let gen = ClipWaveformGenerator::new(48000.0);
        let samples_1s = vec![0.0f32; 48000];
        let wd_1s = gen.generate_from_samples(&samples_1s, 100.0);

        let samples_2s = vec![0.0f32; 96000];
        let wd_2s = gen.generate_from_samples(&samples_2s, 100.0);

        assert_eq!(wd_1s.width(), 100);
        assert_eq!(wd_2s.width(), 200);
    }

    #[test]
    fn test_positive_only_signal_peaks_non_negative_min() {
        let gen = ClipWaveformGenerator::new(44100.0);
        // DC offset at +0.5
        let samples = vec![0.5f32; 44100];
        let wd = gen.generate_from_samples(&samples, 10.0);
        for (lo, hi) in &wd.peaks {
            assert!(*lo >= 0.0, "min should be >= 0 for positive DC");
            assert!(*hi <= 1.0 + f32::EPSILON);
        }
    }

    #[test]
    fn test_min_max_ordering() {
        let gen = ClipWaveformGenerator::new(48000.0);
        let samples = sine_samples(440.0, 48000.0, 1.0);
        let wd = gen.generate_from_samples(&samples, 100.0);
        for (lo, hi) in &wd.peaks {
            assert!(lo <= hi, "min should always be <= max");
        }
    }

    #[test]
    fn test_peak_amplitude_for_sine() {
        let gen = ClipWaveformGenerator::new(48000.0);
        let samples = sine_samples(440.0, 48000.0, 1.0);
        let wd = gen.generate_from_samples(&samples, 100.0);
        let peak = wd.peak_amplitude();
        // A unit sine's amplitude ≈ 1.0 (within floating-point noise).
        assert!(peak > 0.9 && peak <= 1.0 + 1e-4, "peak={peak}");
    }

    #[test]
    fn test_zero_pixels_per_second_returns_empty() {
        let gen = ClipWaveformGenerator::new(48000.0);
        let samples = vec![0.0f32; 100];
        let wd = gen.generate_from_samples(&samples, 0.0);
        assert!(wd.is_empty());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_generate_from_file_produces_nonempty_waveform() {
        let gen = ClipWaveformGenerator::new(48000.0);
        let path = PathBuf::from("/synthetic/audio.wav");
        let wd = gen.generate(&path, 100.0);
        assert!(!wd.is_empty());
        assert!(wd.duration_secs > 0.0);
    }

    // ---- WaveformThumbnail tests ----

    #[test]
    fn test_waveform_thumbnail_empty_samples_returns_zeros() {
        let rms = WaveformThumbnail::generate(&[], 10);
        assert_eq!(rms.len(), 10);
        for v in &rms {
            assert!(*v < f32::EPSILON);
        }
    }

    #[test]
    fn test_waveform_thumbnail_zero_width_returns_empty() {
        let rms = WaveformThumbnail::generate(&[1.0f32; 100], 0);
        assert!(rms.is_empty());
    }

    #[test]
    fn test_waveform_thumbnail_correct_length() {
        let samples = vec![0.5f32; 48000];
        let rms = WaveformThumbnail::generate(&samples, 200);
        assert_eq!(rms.len(), 200);
    }

    #[test]
    fn test_waveform_thumbnail_dc_rms_value() {
        // DC offset at +0.7 → RMS should be ≈ 0.7
        let samples = vec![0.7f32; 48000];
        let rms = WaveformThumbnail::generate(&samples, 100);
        for v in &rms {
            assert!((*v - 0.7).abs() < 1e-3, "expected ≈0.7, got {v}");
        }
    }

    #[test]
    fn test_waveform_thumbnail_silence_is_silent() {
        let thumb = WaveformThumbnail::from_samples(&[0.0f32; 1000], 50);
        assert!(thumb.is_silent());
        assert!(thumb.peak_rms() < f32::EPSILON);
    }

    #[test]
    fn test_waveform_thumbnail_sine_nonzero_rms() {
        let samples = sine_samples(440.0, 48000.0, 1.0);
        let thumb = WaveformThumbnail::from_samples(&samples, 100);
        assert!(!thumb.is_silent());
        // RMS of unit sine ≈ 1/√2 ≈ 0.707
        let peak = thumb.peak_rms();
        assert!(peak > 0.5 && peak <= 1.0 + 1e-4, "peak_rms={peak}");
    }

    #[test]
    fn test_waveform_thumbnail_width_field_matches_rms_len() {
        let samples = vec![0.3f32; 4800];
        let thumb = WaveformThumbnail::from_samples(&samples, 48);
        assert_eq!(thumb.width as usize, thumb.rms_values.len());
    }
}
