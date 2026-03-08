//! Phase correlation for sub-pixel image alignment.
//!
//! Phase correlation is a frequency-domain technique that finds the translation
//! between two signals (or images) by analysing the peak of their cross-power
//! spectrum.  Parabolic interpolation around the peak delivers sub-pixel
//! (or sub-sample) accuracy without requiring an iterative solver.

use std::f64::consts::PI;

// ── 1-D DFT ───────────────────────────────────────────────────────────────────

/// Compute the 1-D Discrete Fourier Transform of `signal`.
///
/// Returns a `Vec` of `(real, imag)` complex pairs.
/// Uses the naïve O(n²) algorithm – suitable for short signals used in alignment.
#[must_use]
pub fn dft_1d(signal: &[f64]) -> Vec<(f64, f64)> {
    let n = signal.len();
    if n == 0 {
        return Vec::new();
    }
    (0..n)
        .map(|k| {
            let (mut re, mut im) = (0.0_f64, 0.0_f64);
            for (j, &xj) in signal.iter().enumerate() {
                let angle = -2.0 * PI * k as f64 * j as f64 / n as f64;
                re += xj * angle.cos();
                im += xj * angle.sin();
            }
            (re, im)
        })
        .collect()
}

/// Compute the 1-D Inverse Discrete Fourier Transform.
///
/// Input is a slice of `(real, imag)` complex pairs.
/// Returns the real part of the reconstructed signal (imaginary part is discarded).
#[must_use]
pub fn idft_1d(spectrum: &[(f64, f64)]) -> Vec<f64> {
    let n = spectrum.len();
    if n == 0 {
        return Vec::new();
    }
    (0..n)
        .map(|j| {
            let (mut re, mut _im) = (0.0_f64, 0.0_f64);
            for (k, &(sk_re, sk_im)) in spectrum.iter().enumerate() {
                let angle = 2.0 * PI * k as f64 * j as f64 / n as f64;
                re += sk_re * angle.cos() - sk_im * angle.sin();
                _im += sk_re * angle.sin() + sk_im * angle.cos();
            }
            re / n as f64
        })
        .collect()
}

// ── Cross-power spectrum ───────────────────────────────────────────────────────

/// Compute the normalised cross-power spectrum of two spectra.
///
/// The result is `conj(A) * B / |conj(A) * B|` element-wise.
/// Elements whose magnitude rounds to zero are left as `(0, 0)`.
#[must_use]
pub fn cross_power_spectrum(a: &[(f64, f64)], b: &[(f64, f64)]) -> Vec<(f64, f64)> {
    a.iter()
        .zip(b.iter())
        .map(|(&(ar, ai), &(br, bi))| {
            // conj(a) * b
            let prod_r = ar * br + ai * bi; // (ar - i*ai)(br + i*bi) real part
            let prod_i = ar * bi - ai * br; // imaginary part

            let mag = (prod_r * prod_r + prod_i * prod_i).sqrt();
            if mag < f64::EPSILON {
                (0.0, 0.0)
            } else {
                (prod_r / mag, prod_i / mag)
            }
        })
        .collect()
}

// ── Peak detection ─────────────────────────────────────────────────────────────

/// Find the index of the maximum value in a real-valued signal.
///
/// Returns 0 for an empty slice.
#[must_use]
pub fn find_peak_index(signal: &[f64]) -> usize {
    signal
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map_or(0, |(i, _)| i)
}

/// Refine a peak position using parabolic interpolation.
///
/// Given a discrete `signal` and the index of its peak `peak_idx`, fits a
/// parabola through the three samples centred on the peak and returns the
/// sub-sample offset from `peak_idx`.
///
/// The returned value is in [-0.5, 0.5].  Add it to `peak_idx as f64` to
/// obtain the sub-sample peak location.
///
/// Falls back to `peak_idx as f64` when the neighbours are unavailable or
/// the denominator is numerically zero.
#[must_use]
pub fn interpolate_peak(signal: &[f64], peak_idx: usize) -> f64 {
    let n = signal.len();
    if n < 3 || peak_idx == 0 || peak_idx >= n - 1 {
        return peak_idx as f64;
    }

    let y_m1 = signal[peak_idx - 1];
    let y_0 = signal[peak_idx];
    let y_p1 = signal[peak_idx + 1];

    let denom = 2.0 * (2.0 * y_0 - y_m1 - y_p1);
    if denom.abs() < f64::EPSILON {
        return peak_idx as f64;
    }

    let delta = (y_m1 - y_p1) / denom;
    peak_idx as f64 + delta
}

// ── 1-D phase correlation ──────────────────────────────────────────────────────

/// Estimate the sub-sample offset between two 1-D signals using phase correlation.
///
/// Returns the fractional sample offset `d` such that `b[i] ≈ a[i - d]`.
/// Positive `d` means `b` is shifted to the right relative to `a`.
#[must_use]
pub fn phase_correlate_1d(a: &[f64], b: &[f64]) -> f64 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let fa = dft_1d(a);
    let fb = dft_1d(b);
    let cps = cross_power_spectrum(&fa, &fb);
    let corr = idft_1d(&cps);

    let peak_idx = find_peak_index(&corr);
    let sub_peak = interpolate_peak(&corr, peak_idx);

    let n = a.len() as f64;

    // Map the peak index from [0, n) to a signed offset in (-n/2, n/2]
    if sub_peak > n / 2.0 {
        sub_peak - n
    } else {
        sub_peak
    }
}

// ── 2-D phase correlation ──────────────────────────────────────────────────────

/// Estimate the sub-pixel 2-D translation between two images using phase
/// correlation.
///
/// `a` and `b` are row-major grayscale images with `width × height` elements.
///
/// Returns `(dx, dy)` such that `b` is shifted right by `dx` pixels and down
/// by `dy` pixels relative to `a`.
#[must_use]
pub fn phase_correlate_2d(a: &[f64], b: &[f64], width: usize, height: usize) -> (f64, f64) {
    if a.len() != width * height || b.len() != width * height {
        return (0.0, 0.0);
    }

    // Correlate row projections for horizontal shift
    let a_row_sum: Vec<f64> = (0..width)
        .map(|x| (0..height).map(|y| a[y * width + x]).sum())
        .collect();
    let b_row_sum: Vec<f64> = (0..width)
        .map(|x| (0..height).map(|y| b[y * width + x]).sum())
        .collect();

    // Correlate column projections for vertical shift
    let a_col_sum: Vec<f64> = (0..height)
        .map(|y| (0..width).map(|x| a[y * width + x]).sum())
        .collect();
    let b_col_sum: Vec<f64> = (0..height)
        .map(|y| (0..width).map(|x| b[y * width + x]).sum())
        .collect();

    let dx = phase_correlate_1d(&a_row_sum, &b_row_sum);
    let dy = phase_correlate_1d(&a_col_sum, &b_col_sum);

    (dx, dy)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── dft_1d / idft_1d round-trip ───────────────────────────────────────────

    #[test]
    fn test_dft_empty() {
        assert!(dft_1d(&[]).is_empty());
    }

    #[test]
    fn test_idft_empty() {
        assert!(idft_1d(&[]).is_empty());
    }

    #[test]
    fn test_dft_idft_round_trip() {
        let signal = vec![1.0, 2.0, 3.0, 4.0];
        let spectrum = dft_1d(&signal);
        let recovered = idft_1d(&spectrum);
        assert_eq!(recovered.len(), signal.len());
        for (a, b) in signal.iter().zip(recovered.iter()) {
            assert!((a - b).abs() < 1e-9, "{a} ≠ {b}");
        }
    }

    #[test]
    fn test_dft_dc_component() {
        // DFT of a constant signal: DC bin = N * value, all others = 0
        let signal = vec![2.0_f64; 4];
        let spectrum = dft_1d(&signal);
        assert!((spectrum[0].0 - 8.0).abs() < 1e-9); // DC real = N * 2
        assert!(spectrum[0].1.abs() < 1e-9); // DC imaginary ≈ 0
        assert!(spectrum[1].0.abs() < 1e-9);
        assert!(spectrum[2].0.abs() < 1e-9);
    }

    // ── cross_power_spectrum ──────────────────────────────────────────────────

    #[test]
    fn test_cross_power_spectrum_identical() {
        let s = vec![(1.0_f64, 0.0_f64), (0.5, 0.5)];
        let cps = cross_power_spectrum(&s, &s);
        // conj(a) * a / |…| should have magnitude 1
        for (re, im) in &cps {
            let mag = (re * re + im * im).sqrt();
            assert!((mag - 1.0).abs() < 1e-9 || mag.abs() < 1e-9);
        }
    }

    #[test]
    fn test_cross_power_spectrum_zero_element() {
        let a = vec![(0.0_f64, 0.0_f64)];
        let b = vec![(1.0_f64, 0.0_f64)];
        let cps = cross_power_spectrum(&a, &b);
        assert_eq!(cps[0], (0.0, 0.0));
    }

    // ── find_peak_index ───────────────────────────────────────────────────────

    #[test]
    fn test_find_peak_index_basic() {
        let s = vec![0.1, 0.5, 0.9, 0.3];
        assert_eq!(find_peak_index(&s), 2);
    }

    #[test]
    fn test_find_peak_index_empty() {
        assert_eq!(find_peak_index(&[]), 0);
    }

    // ── interpolate_peak ──────────────────────────────────────────────────────

    #[test]
    fn test_interpolate_peak_boundary_returns_index() {
        let s = vec![0.1, 0.5, 0.9, 0.3, 0.1];
        // At index 0 or n-1 there are no neighbours; at idx 4 should fall back
        let result = interpolate_peak(&s, 4);
        assert_eq!(result, 4.0);
    }

    #[test]
    fn test_interpolate_peak_symmetric_returns_exact() {
        // Symmetric parabola: peak exactly at centre → no sub-pixel shift
        let s = vec![0.25_f64, 0.75, 1.0, 0.75, 0.25];
        let peak = find_peak_index(&s);
        let refined = interpolate_peak(&s, peak);
        assert!((refined - 2.0).abs() < 1e-9);
    }

    // ── phase_correlate_1d ────────────────────────────────────────────────────

    #[test]
    fn test_phase_correlate_1d_identical_signals() {
        let signal = vec![1.0, 2.0, 3.0, 4.0, 3.0, 2.0, 1.0, 0.5];
        let offset = phase_correlate_1d(&signal, &signal);
        assert!(offset.abs() < 0.5, "Offset for identical signals: {offset}");
    }

    #[test]
    fn test_phase_correlate_1d_empty() {
        assert_eq!(phase_correlate_1d(&[], &[]), 0.0);
    }

    #[test]
    fn test_phase_correlate_1d_mismatched_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(phase_correlate_1d(&a, &b), 0.0);
    }

    // ── phase_correlate_2d ────────────────────────────────────────────────────

    #[test]
    fn test_phase_correlate_2d_identical_images() {
        let img = vec![
            10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0, 110.0, 120.0, 100.0, 90.0,
            80.0, 70.0,
        ];
        let (dx, dy) = phase_correlate_2d(&img, &img, 4, 4);
        assert!(
            dx.abs() < 0.5 && dy.abs() < 0.5,
            "dx={dx}, dy={dy} for identical images"
        );
    }

    #[test]
    fn test_phase_correlate_2d_mismatched_size() {
        let a = vec![1.0; 16];
        let b = vec![1.0; 9];
        let (dx, dy) = phase_correlate_2d(&a, &b, 4, 4);
        assert_eq!((dx, dy), (0.0, 0.0));
    }
}
