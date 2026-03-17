//! Linear Predictive Coding (LPC) for FLAC.
//!
//! FLAC uses LPC analysis to predict each audio sample from its `p` predecessors.
//! The prediction residuals are then Rice-coded.
//!
//! This module provides:
//!
//! - `autocorrelate` — compute autocorrelation lags.
//! - `levinson_durbin` — fit LPC coefficients via the Levinson-Durbin recursion.
//! - `predict` — apply LPC predictor to a signal.
//! - `compute_residuals` — subtract LPC prediction from signal.
//! - `restore_signal` — inverse: reconstruct signal from residuals + LPC state.

#![forbid(unsafe_code)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_lossless)]

/// Maximum supported LPC order.
pub const MAX_LPC_ORDER: usize = 32;

/// Compute the autocorrelation of `signal` at lags 0..=`order`.
///
/// Returns a vector of `order + 1` autocorrelation values.
#[must_use]
pub fn autocorrelate(signal: &[f64], order: usize) -> Vec<f64> {
    let n = signal.len();
    let mut ac = vec![0.0f64; order + 1];
    for lag in 0..=order {
        for i in lag..n {
            ac[lag] += signal[i] * signal[i - lag];
        }
    }
    ac
}

/// Fit LPC coefficients of order `p` using the Levinson-Durbin recursion.
///
/// Returns `(coeffs, error)` where `coeffs` has length `p` and `error` is the
/// residual prediction power.  Returns an empty vector if `ac[0]` is zero.
pub fn levinson_durbin(ac: &[f64], order: usize) -> (Vec<f64>, f64) {
    let p = order.min(ac.len().saturating_sub(1));
    if p == 0 || ac[0] == 0.0 {
        return (Vec::new(), 0.0);
    }

    let mut a = vec![0.0f64; p + 1];
    let mut err = ac[0];
    let mut km;

    for m in 1..=p {
        // Reflection coefficient
        let mut lambda = 0.0f64;
        for j in 1..m {
            lambda += a[j] * ac[m - j];
        }
        lambda = (ac[m] - lambda) / err;

        km = lambda;
        a[m] = km;

        // Update coefficients
        let half = m / 2;
        for j in 1..=half {
            let aj = a[j];
            let amj = a[m - j];
            a[j] = aj + km * amj;
            a[m - j] = amj + km * aj;
        }
        if m % 2 == 1 {
            a[(m + 1) / 2] *= 1.0 + km;
        }

        err *= 1.0 - km * km;
        if err <= 0.0 {
            err = 0.0;
            break;
        }
    }

    let coeffs = a[1..=p].to_vec();
    (coeffs, err)
}

/// Apply LPC predictor to `signal` using `coeffs`.
///
/// Returns predicted values starting at index `p = coeffs.len()`.
#[must_use]
pub fn predict(signal: &[i32], coeffs: &[f64]) -> Vec<i32> {
    let p = coeffs.len();
    let n = signal.len();
    if p == 0 || n <= p {
        return vec![0i32; n.saturating_sub(p)];
    }

    (p..n)
        .map(|i| {
            let pred: f64 = coeffs
                .iter()
                .enumerate()
                .map(|(j, &c)| c * f64::from(signal[i - 1 - j]))
                .sum();
            pred.round() as i32
        })
        .collect()
}

/// Compute LPC residuals: `residual[i] = signal[p+i] − prediction[i]`.
#[must_use]
pub fn compute_residuals(signal: &[i32], coeffs: &[f64]) -> Vec<i32> {
    let p = coeffs.len();
    let preds = predict(signal, coeffs);
    preds
        .iter()
        .enumerate()
        .map(|(i, &pred)| signal[p + i].wrapping_sub(pred))
        .collect()
}

/// Restore signal from residuals and LPC warmup samples.
///
/// `warmup` must be the first `p = coeffs.len()` original samples.
#[must_use]
pub fn restore_signal(warmup: &[i32], residuals: &[i32], coeffs: &[f64]) -> Vec<i32> {
    let p = coeffs.len();
    let mut out: Vec<i32> = warmup.to_vec();

    for (i, &r) in residuals.iter().enumerate() {
        let base = out.len().saturating_sub(1);
        let pred: f64 = coeffs
            .iter()
            .enumerate()
            .map(|(j, &c)| {
                let idx = base - j;
                c * f64::from(out[idx])
            })
            .sum();
        let _ = i;
        out.push((pred.round() as i32).wrapping_add(r));
    }

    out
}

/// Quantise floating-point LPC coefficients to fixed-point integer coefficients.
///
/// Returns `(int_coeffs, shift)` where `int_coeffs[i] = round(coeffs[i] * 2^shift)`.
/// `shift` is chosen to maximise precision while fitting into `bits`-bit signed integers.
#[must_use]
pub fn quantise_coeffs(coeffs: &[f64], bits: u8) -> (Vec<i32>, u8) {
    if coeffs.is_empty() {
        return (Vec::new(), 0);
    }
    let max_abs = coeffs.iter().cloned().fold(0.0f64, |a, v| a.max(v.abs()));
    if max_abs < 1e-10 {
        return (vec![0i32; coeffs.len()], 0);
    }

    let max_val = (1i64 << (bits - 1)) - 1;
    let scale = max_val as f64 / max_abs;
    let shift = scale.log2().floor() as u8;
    let actual_scale = (1i64 << shift) as f64;

    let int_coeffs: Vec<i32> = coeffs
        .iter()
        .map(|&c| (c * actual_scale).round() as i32)
        .collect();

    (int_coeffs, shift)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autocorrelate_dc_signal() {
        let dc = vec![1.0f64; 100];
        let ac = autocorrelate(&dc, 4);
        // For a DC signal, all lags have the same autocorrelation as lag 0.
        assert_eq!(ac.len(), 5);
        assert!(ac[0] > 0.0);
        assert!(ac[1] > 0.0);
    }

    #[test]
    fn test_autocorrelate_zero_signal() {
        let zero = vec![0.0f64; 50];
        let ac = autocorrelate(&zero, 3);
        for v in &ac {
            assert_eq!(*v, 0.0);
        }
    }

    #[test]
    fn test_levinson_durbin_order1() {
        // Sine wave → LPC should fit a 2-pole predictor
        let n = 64;
        let signal: Vec<f64> = (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * i as f64 / 16.0).sin())
            .collect();
        let ac = autocorrelate(&signal, 2);
        let (coeffs, err) = levinson_durbin(&ac, 2);
        assert_eq!(coeffs.len(), 2);
        assert!(err >= 0.0, "Residual error should be non-negative");
    }

    #[test]
    fn test_levinson_durbin_empty_ac() {
        let (coeffs, _) = levinson_durbin(&[], 2);
        assert!(coeffs.is_empty());
    }

    #[test]
    fn test_predict_simple() {
        let signal: Vec<i32> = vec![1, 2, 3, 4, 5];
        let coeffs = vec![1.0f64]; // order-1 predictor: predict[i] = signal[i-1]
        let preds = predict(&signal, &coeffs);
        assert_eq!(preds, vec![1, 2, 3, 4]); // pred[0]=signal[0]=1, ...
    }

    #[test]
    fn test_compute_residuals_lossless() {
        let signal: Vec<i32> = (0..32).map(|i| i * 3).collect();
        let coeffs = vec![1.0f64]; // naive predictor
        let residuals = compute_residuals(&signal, &coeffs);
        let warmup = &signal[..1];
        let restored = restore_signal(warmup, &residuals, &coeffs);
        assert_eq!(&restored, &signal, "Restore signal should be lossless");
    }

    #[test]
    fn test_restore_signal_dc() {
        // DC signal: each sample == 1000
        let signal: Vec<i32> = vec![1000i32; 16];
        let coeffs = vec![1.0f64];
        let residuals = compute_residuals(&signal, &coeffs);
        // Residuals should all be 0 (perfect DC prediction)
        let warmup = &signal[..1];
        let restored = restore_signal(warmup, &residuals, &coeffs);
        assert_eq!(&restored, &signal);
    }

    #[test]
    fn test_quantise_coeffs_basic() {
        let coeffs = vec![0.5f64, -0.25];
        let (int_c, shift) = quantise_coeffs(&coeffs, 12);
        assert_eq!(int_c.len(), 2);
        assert!(
            shift > 0,
            "Shift should be positive for non-trivial coefficients"
        );
    }

    #[test]
    fn test_quantise_coeffs_zero() {
        let coeffs = vec![0.0f64; 4];
        let (int_c, shift) = quantise_coeffs(&coeffs, 12);
        assert!(int_c.iter().all(|&v| v == 0));
        assert_eq!(shift, 0);
    }
}
