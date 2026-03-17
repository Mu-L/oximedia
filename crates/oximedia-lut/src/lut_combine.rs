//! LUT combination and composition utilities.
//!
//! Provides:
//! * Sequential application of multiple 3-D LUTs.
//! * Composition of a 1-D pre-curve with a 3-D body LUT.
//! * Identity detection so trivial LUTs can be skipped.

use crate::Rgb;

// ---------------------------------------------------------------------------
// Identity detection
// ---------------------------------------------------------------------------

/// Tolerance used when checking whether a LUT is the identity.
const IDENTITY_EPSILON: f64 = 1e-6;

/// Return `true` if the 3-D `lut` (stored `[r][g][b]`, size³ entries) is
/// effectively an identity transform.
#[allow(dead_code)]
#[must_use]
pub fn is_identity_lut3d(lut: &[Rgb], size: usize) -> bool {
    if lut.len() != size * size * size || size < 2 {
        return false;
    }
    let scale = (size - 1) as f64;
    for r in 0..size {
        for g in 0..size {
            for b in 0..size {
                let expected = [r as f64 / scale, g as f64 / scale, b as f64 / scale];
                let entry = &lut[r * size * size + g * size + b];
                for ch in 0..3 {
                    if (entry[ch] - expected[ch]).abs() > IDENTITY_EPSILON {
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// Return `true` if the 1-D `curve` (per-channel, each channel has `size`
/// entries) is effectively an identity.
///
/// `curve[ch][i]` should equal `i as f64 / (size - 1) as f64` for all i.
#[allow(dead_code)]
#[must_use]
pub fn is_identity_lut1d(curve: &[[f64; 3]], size: usize) -> bool {
    if curve.len() != size || size < 2 {
        return false;
    }
    let scale = (size - 1) as f64;
    for (i, entry) in curve.iter().enumerate() {
        let expected = i as f64 / scale;
        for ch in 0..3 {
            if (entry[ch] - expected).abs() > IDENTITY_EPSILON {
                return false;
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// 1-D curve application
// ---------------------------------------------------------------------------

/// Apply a 1-D per-channel curve to a colour value.
///
/// `curve` – slice of length `size` where each element is `[r, g, b]`
/// representing the curve output at that normalised input position.
#[allow(dead_code)]
#[must_use]
pub fn apply_curve(curve: &[[f64; 3]], input: &Rgb) -> Rgb {
    let size = curve.len();
    assert!(size >= 2, "curve must have at least 2 entries");
    let scale = (size - 1) as f64;
    let mut out = [0.0f64; 3];
    for ch in 0..3 {
        let v = input[ch].clamp(0.0, 1.0) * scale;
        let lo = v.floor() as usize;
        let hi = (lo + 1).min(size - 1);
        let frac = v - lo as f64;
        out[ch] = curve[lo][ch] * (1.0 - frac) + curve[hi][ch] * frac;
    }
    out
}

// ---------------------------------------------------------------------------
// 3-D LUT application (trilinear)
// ---------------------------------------------------------------------------

/// Apply a 3-D LUT to a colour value using trilinear interpolation.
///
/// `lut` – flat slice of length `size³`, stored `[r][g][b]`.
#[allow(dead_code)]
#[must_use]
pub fn apply_lut3d(lut: &[Rgb], size: usize, input: &Rgb) -> Rgb {
    assert!(size >= 2, "size must be >= 2");
    assert_eq!(lut.len(), size * size * size, "LUT length mismatch");

    let scale = (size - 1) as f64;
    let r = input[0].clamp(0.0, 1.0) * scale;
    let g = input[1].clamp(0.0, 1.0) * scale;
    let b = input[2].clamp(0.0, 1.0) * scale;

    let r0 = r.floor() as usize;
    let g0 = g.floor() as usize;
    let b0 = b.floor() as usize;
    let r1 = (r0 + 1).min(size - 1);
    let g1 = (g0 + 1).min(size - 1);
    let b1 = (b0 + 1).min(size - 1);
    let dr = r - r0 as f64;
    let dg = g - g0 as f64;
    let db = b - b0 as f64;

    let idx = |ri: usize, gi: usize, bi: usize| -> Rgb { lut[ri * size * size + gi * size + bi] };

    let mut out = [0.0f64; 3];
    for ch in 0..3 {
        let c000 = idx(r0, g0, b0)[ch];
        let c100 = idx(r1, g0, b0)[ch];
        let c010 = idx(r0, g1, b0)[ch];
        let c110 = idx(r1, g1, b0)[ch];
        let c001 = idx(r0, g0, b1)[ch];
        let c101 = idx(r1, g0, b1)[ch];
        let c011 = idx(r0, g1, b1)[ch];
        let c111 = idx(r1, g1, b1)[ch];

        out[ch] = c000 * (1.0 - dr) * (1.0 - dg) * (1.0 - db)
            + c100 * dr * (1.0 - dg) * (1.0 - db)
            + c010 * (1.0 - dr) * dg * (1.0 - db)
            + c110 * dr * dg * (1.0 - db)
            + c001 * (1.0 - dr) * (1.0 - dg) * db
            + c101 * dr * (1.0 - dg) * db
            + c011 * (1.0 - dr) * dg * db
            + c111 * dr * dg * db;
    }
    out
}

// ---------------------------------------------------------------------------
// Composition
// ---------------------------------------------------------------------------

/// Apply a 1-D pre-curve followed by a 3-D LUT.
///
/// This is the standard way to compose a 1D tone / log curve with a 3D
/// creative LUT in a single pass.
#[allow(dead_code)]
#[must_use]
pub fn apply_1d_then_3d(curve: &[[f64; 3]], lut: &[Rgb], lut_size: usize, input: &Rgb) -> Rgb {
    let after_curve = apply_curve(curve, input);
    apply_lut3d(lut, lut_size, &after_curve)
}

/// Apply two 3-D LUTs sequentially: `first` is applied, then `second`.
#[allow(dead_code)]
#[must_use]
pub fn apply_sequential(
    first: &[Rgb],
    first_size: usize,
    second: &[Rgb],
    second_size: usize,
    input: &Rgb,
) -> Rgb {
    let intermediate = apply_lut3d(first, first_size, input);
    apply_lut3d(second, second_size, &intermediate)
}

/// Bake two sequential 3-D LUTs into one combined LUT.
///
/// Samples `first` at each lattice point of the output, then applies `second`
/// to produce the combined output. The resulting LUT has `out_size³` entries.
#[allow(dead_code)]
#[must_use]
pub fn bake_sequential(
    first: &[Rgb],
    first_size: usize,
    second: &[Rgb],
    second_size: usize,
    out_size: usize,
) -> Vec<Rgb> {
    let scale = (out_size - 1) as f64;
    let mut out = Vec::with_capacity(out_size * out_size * out_size);
    for r in 0..out_size {
        for g in 0..out_size {
            for b in 0..out_size {
                let input: Rgb = [r as f64 / scale, g as f64 / scale, b as f64 / scale];
                let combined = apply_sequential(first, first_size, second, second_size, &input);
                out.push(combined);
            }
        }
    }
    out
}

/// Bake a 1-D curve followed by a 3-D LUT into a new single 3-D LUT.
#[allow(dead_code)]
#[must_use]
pub fn bake_1d_then_3d(
    curve: &[[f64; 3]],
    lut: &[Rgb],
    lut_size: usize,
    out_size: usize,
) -> Vec<Rgb> {
    let scale = (out_size - 1) as f64;
    let mut out = Vec::with_capacity(out_size * out_size * out_size);
    for r in 0..out_size {
        for g in 0..out_size {
            for b in 0..out_size {
                let input: Rgb = [r as f64 / scale, g as f64 / scale, b as f64 / scale];
                out.push(apply_1d_then_3d(curve, lut, lut_size, &input));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// LUT Inversion
// ---------------------------------------------------------------------------

/// Invert a 1-D per-channel curve analytically.
///
/// For each output value, finds the input value that produced it via binary
/// search with linear interpolation. Works best when the curve is monotonic.
///
/// `curve` – slice of length `size` where `curve[i] = [r, g, b]`.
/// Returns a new curve of the same size representing the inverse.
#[allow(dead_code)]
#[must_use]
pub fn invert_curve(curve: &[[f64; 3]], out_size: usize) -> Vec<[f64; 3]> {
    let size = curve.len();
    if size < 2 || out_size < 2 {
        return vec![[0.0; 3]; out_size];
    }

    let out_scale = (out_size - 1) as f64;
    let in_scale = (size - 1) as f64;

    let mut result = vec![[0.0; 3]; out_size];

    for ch in 0..3 {
        // Extract single-channel values
        let channel_vals: Vec<f64> = curve.iter().map(|c| c[ch]).collect();

        // Determine if the channel is increasing or decreasing
        let is_increasing = channel_vals.last().copied().unwrap_or(0.0)
            >= channel_vals.first().copied().unwrap_or(0.0);

        for i in 0..out_size {
            let target = i as f64 / out_scale;

            // Binary search for the input index that produces `target`
            let found = if is_increasing {
                binary_search_increasing(&channel_vals, target)
            } else {
                binary_search_decreasing(&channel_vals, target)
            };

            result[i][ch] = found / in_scale;
        }
    }
    result
}

/// Binary search in a monotonically increasing channel, returning the
/// fractional index where `channel[idx] == target`.
fn binary_search_increasing(channel: &[f64], target: f64) -> f64 {
    let n = channel.len();
    if n < 2 {
        return 0.0;
    }

    // Clamp to range
    let first = channel[0];
    let last = channel[n - 1];
    if target <= first {
        return 0.0;
    }
    if target >= last {
        return (n - 1) as f64;
    }

    let mut lo = 0usize;
    let mut hi = n - 1;

    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if channel[mid] <= target {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    let denom = channel[hi] - channel[lo];
    if denom.abs() < 1e-15 {
        lo as f64
    } else {
        lo as f64 + (target - channel[lo]) / denom
    }
}

/// Binary search in a monotonically decreasing channel.
fn binary_search_decreasing(channel: &[f64], target: f64) -> f64 {
    let n = channel.len();
    if n < 2 {
        return 0.0;
    }

    let first = channel[0];
    let last = channel[n - 1];
    if target >= first {
        return 0.0;
    }
    if target <= last {
        return (n - 1) as f64;
    }

    let mut lo = 0usize;
    let mut hi = n - 1;

    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if channel[mid] >= target {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    let denom = channel[hi] - channel[lo];
    if denom.abs() < 1e-15 {
        lo as f64
    } else {
        lo as f64 + (target - channel[lo]) / denom
    }
}

/// Invert a 3-D LUT using iterative Newton-Raphson refinement.
///
/// For each lattice point in the *output* LUT, uses Newton-like iterations
/// to find the input that produces the desired output in the *forward* LUT.
///
/// `forward_lut` – flat slice of length `size³`, stored `[r][g][b]`.
/// `size` – lattice dimension of the forward LUT.
/// `out_size` – desired output lattice dimension.
/// `max_iterations` – maximum Newton iterations per lattice point.
/// `tolerance` – convergence tolerance (Euclidean distance in RGB).
///
/// Returns the inverted LUT as `out_size³` RGB entries.
#[allow(dead_code)]
pub fn invert_lut3d(
    forward_lut: &[Rgb],
    size: usize,
    out_size: usize,
    max_iterations: usize,
    tolerance: f64,
) -> Vec<Rgb> {
    let out_scale = (out_size - 1) as f64;
    let mut result = Vec::with_capacity(out_size * out_size * out_size);

    for ri in 0..out_size {
        for gi in 0..out_size {
            for bi in 0..out_size {
                let target = [
                    ri as f64 / out_scale,
                    gi as f64 / out_scale,
                    bi as f64 / out_scale,
                ];
                let inverse =
                    newton_invert_pixel(forward_lut, size, &target, max_iterations, tolerance);
                result.push(inverse);
            }
        }
    }
    result
}

/// Compute a good initial guess for LUT inversion by finding the nearest
/// lattice output point and returning its corresponding lattice input.
///
/// This avoids the poor-convergence problem that arises from starting at
/// `target` when the LUT is highly nonlinear (e.g. gamma-squared curves
/// produce very flat Jacobians near zero).
fn initial_guess_from_nearest_lattice(lut: &[Rgb], size: usize, target: &Rgb) -> Rgb {
    let scale = (size - 1) as f64;
    let mut best_dist = f64::MAX;
    let mut best_input = *target;

    for ri in 0..size {
        for gi in 0..size {
            for bi in 0..size {
                let lut_out = lut[ri * size * size + gi * size + bi];
                let d = (lut_out[0] - target[0]).powi(2)
                    + (lut_out[1] - target[1]).powi(2)
                    + (lut_out[2] - target[2]).powi(2);
                if d < best_dist {
                    best_dist = d;
                    best_input = [ri as f64 / scale, gi as f64 / scale, bi as f64 / scale];
                }
            }
        }
    }
    best_input
}

/// Newton-Raphson iteration to find the input that maps to `target` via the
/// forward LUT (using trilinear interpolation).
fn newton_invert_pixel(
    lut: &[Rgb],
    size: usize,
    target: &Rgb,
    max_iterations: usize,
    tolerance: f64,
) -> Rgb {
    // Use a nearest-lattice-point initial guess to avoid poor convergence on
    // highly nonlinear LUTs (e.g. gamma-squared, where starting at the target
    // yields a near-zero Jacobian and divergent Newton steps).
    let mut guess = initial_guess_from_nearest_lattice(lut, size, target);
    // Adaptive finite-difference step size.
    let step = (1.0 / (size - 1) as f64).max(1e-4);

    for _ in 0..max_iterations {
        let current = apply_lut3d(lut, size, &guess);
        let err = [
            current[0] - target[0],
            current[1] - target[1],
            current[2] - target[2],
        ];

        let dist = (err[0] * err[0] + err[1] * err[1] + err[2] * err[2]).sqrt();
        if dist < tolerance {
            break;
        }

        // Estimate Jacobian numerically (3x3)
        let mut jacobian = [[0.0f64; 3]; 3];
        for col in 0..3 {
            let mut offset = guess;
            offset[col] = (offset[col] + step).min(1.0);
            let perturbed = apply_lut3d(lut, size, &offset);
            let delta = offset[col] - guess[col];
            if delta.abs() > 1e-15 {
                for row in 0..3 {
                    jacobian[row][col] = (perturbed[row] - current[row]) / delta;
                }
            }
        }

        // Solve J * dx = -err using Cramer's rule for 3x3
        if let Some(dx) = solve_3x3(&jacobian, &[-err[0], -err[1], -err[2]]) {
            for ch in 0..3 {
                guess[ch] = (guess[ch] + dx[ch]).clamp(0.0, 1.0);
            }
        } else {
            // Jacobian is singular; fallback to gradient descent step
            for ch in 0..3 {
                guess[ch] = (guess[ch] - err[ch] * 0.5).clamp(0.0, 1.0);
            }
        }
    }
    guess
}

/// Solve a 3x3 linear system `A * x = b` using Cramer's rule.
/// Returns `None` if the determinant is near zero.
fn solve_3x3(a: &[[f64; 3]; 3], b: &[f64; 3]) -> Option<[f64; 3]> {
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);

    if det.abs() < 1e-12 {
        return None;
    }

    let inv_det = 1.0 / det;

    let x0 = (b[0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (b[1] * a[2][2] - a[1][2] * b[2])
        + a[0][2] * (b[1] * a[2][1] - a[1][1] * b[2]))
        * inv_det;

    let x1 = (a[0][0] * (b[1] * a[2][2] - a[1][2] * b[2])
        - b[0] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * b[2] - b[1] * a[2][0]))
        * inv_det;

    let x2 = (a[0][0] * (a[1][1] * b[2] - b[1] * a[2][1])
        - a[0][1] * (a[1][0] * b[2] - b[1] * a[2][0])
        + b[0] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]))
        * inv_det;

    Some([x0, x1, x2])
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_lut3d(size: usize) -> Vec<Rgb> {
        let scale = (size - 1) as f64;
        let mut lut = Vec::with_capacity(size * size * size);
        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    lut.push([r as f64 / scale, g as f64 / scale, b as f64 / scale]);
                }
            }
        }
        lut
    }

    fn identity_curve(size: usize) -> Vec<[f64; 3]> {
        let scale = (size - 1) as f64;
        (0..size)
            .map(|i| {
                let v = i as f64 / scale;
                [v, v, v]
            })
            .collect()
    }

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn rgb_approx(a: &Rgb, b: &Rgb) -> bool {
        approx_eq(a[0], b[0]) && approx_eq(a[1], b[1]) && approx_eq(a[2], b[2])
    }

    #[test]
    fn test_is_identity_lut3d_true() {
        let lut = identity_lut3d(3);
        assert!(is_identity_lut3d(&lut, 3));
    }

    #[test]
    fn test_is_identity_lut3d_false() {
        let mut lut = identity_lut3d(3);
        lut[0] = [0.1, 0.1, 0.1]; // Corrupt first entry.
        assert!(!is_identity_lut3d(&lut, 3));
    }

    #[test]
    fn test_is_identity_curve_true() {
        let curve = identity_curve(33);
        assert!(is_identity_lut1d(&curve, 33));
    }

    #[test]
    fn test_is_identity_curve_false() {
        let mut curve = identity_curve(33);
        curve[5] = [0.9, 0.9, 0.9];
        assert!(!is_identity_lut1d(&curve, 33));
    }

    #[test]
    fn test_apply_curve_identity() {
        let curve = identity_curve(33);
        let inp = [0.4, 0.6, 0.2];
        let out = apply_curve(&curve, &inp);
        assert!(rgb_approx(&out, &inp));
    }

    #[test]
    fn test_apply_lut3d_identity() {
        let lut = identity_lut3d(5);
        let inp = [0.25, 0.5, 0.75];
        let out = apply_lut3d(&lut, 5, &inp);
        assert!(rgb_approx(&out, &inp));
    }

    #[test]
    fn test_apply_lut3d_clamps_input() {
        let lut = identity_lut3d(3);
        let out = apply_lut3d(&lut, 3, &[-0.5, 0.5, 1.5]);
        assert!(out[0] >= 0.0 && out[0] <= 1.0 + 1e-9);
        assert!(out[2] >= 0.0 && out[2] <= 1.0 + 1e-9);
    }

    #[test]
    fn test_apply_1d_then_3d_identity() {
        let curve = identity_curve(33);
        let lut = identity_lut3d(5);
        let inp = [0.3, 0.7, 0.1];
        let out = apply_1d_then_3d(&curve, &lut, 5, &inp);
        // Both identity → output == input.
        for ch in 0..3 {
            assert!((out[ch] - inp[ch]).abs() < 0.01);
        }
    }

    #[test]
    fn test_apply_sequential_identity_identity() {
        let lut = identity_lut3d(3);
        let inp = [0.2, 0.5, 0.8];
        let out = apply_sequential(&lut, 3, &lut, 3, &inp);
        assert!(rgb_approx(&out, &inp));
    }

    #[test]
    fn test_bake_sequential_identity() {
        let lut = identity_lut3d(3);
        let baked = bake_sequential(&lut, 3, &lut, 3, 3);
        assert!(is_identity_lut3d(&baked, 3));
    }

    #[test]
    fn test_bake_1d_then_3d_identity() {
        let curve = identity_curve(33);
        let lut = identity_lut3d(3);
        let baked = bake_1d_then_3d(&curve, &lut, 3, 3);
        assert!(is_identity_lut3d(&baked, 3));
    }

    #[test]
    fn test_apply_curve_clamping() {
        let curve = identity_curve(33);
        // Over-range input should be clamped.
        let out = apply_curve(&curve, &[2.0, -1.0, 0.5]);
        assert!(out[0] <= 1.0 + 1e-9);
        assert!(out[1] >= -1e-9);
    }

    #[test]
    fn test_bake_sequential_produces_correct_size() {
        let lut = identity_lut3d(3);
        let baked = bake_sequential(&lut, 3, &lut, 3, 5);
        assert_eq!(baked.len(), 5 * 5 * 5);
    }

    #[test]
    fn test_apply_curve_endpoint_accuracy() {
        let curve = identity_curve(33);
        let black = apply_curve(&curve, &[0.0, 0.0, 0.0]);
        let white = apply_curve(&curve, &[1.0, 1.0, 1.0]);
        assert!(rgb_approx(&black, &[0.0, 0.0, 0.0]));
        assert!(rgb_approx(&white, &[1.0, 1.0, 1.0]));
    }

    // -------------------------------------------------------------------
    // Inversion tests
    // -------------------------------------------------------------------

    #[test]
    fn test_invert_identity_curve() {
        let curve = identity_curve(33);
        let inv = invert_curve(&curve, 33);
        // Inverse of identity should be identity
        for (i, entry) in inv.iter().enumerate() {
            let expected = i as f64 / 32.0;
            for ch in 0..3 {
                assert!(
                    (entry[ch] - expected).abs() < 1e-6,
                    "ch={ch} i={i}: expected {expected}, got {}",
                    entry[ch]
                );
            }
        }
    }

    #[test]
    fn test_invert_gamma_curve_roundtrip() {
        // Build a gamma 2.2 curve
        let size = 256;
        let scale = (size - 1) as f64;
        let forward: Vec<[f64; 3]> = (0..size)
            .map(|i| {
                let v = (i as f64 / scale).powf(2.2);
                [v, v, v]
            })
            .collect();

        let inv = invert_curve(&forward, size);

        // Applying forward then inverse should get back near the original
        for i in (0..size).step_by(16) {
            let t = i as f64 / scale;
            let encoded = apply_curve(&forward, &[t, t, t]);
            let decoded = apply_curve(&inv, &encoded);
            assert!(
                (decoded[0] - t).abs() < 0.02,
                "Roundtrip failed at t={t}: encoded={}, decoded={}",
                encoded[0],
                decoded[0]
            );
        }
    }

    #[test]
    fn test_invert_curve_small_size() {
        let inv = invert_curve(&[], 0);
        assert!(inv.is_empty());
    }

    #[test]
    fn test_invert_lut3d_identity() {
        let lut = identity_lut3d(5);
        let inv = invert_lut3d(&lut, 5, 5, 50, 1e-6);

        // Inverse of identity should be identity
        let scale = 4.0;
        for r in 0..5 {
            for g in 0..5 {
                for b in 0..5 {
                    let idx = r * 25 + g * 5 + b;
                    let expected = [r as f64 / scale, g as f64 / scale, b as f64 / scale];
                    for ch in 0..3 {
                        assert!(
                            (inv[idx][ch] - expected[ch]).abs() < 1e-3,
                            "r={r} g={g} b={b} ch={ch}: expected {}, got {}",
                            expected[ch],
                            inv[idx][ch]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_invert_lut3d_gamma_roundtrip() {
        // Build a simple gamma-like 3D LUT (size=5 forward LUT)
        let size = 5;
        // Use a higher resolution for the inverse LUT so that lookup at
        // non-lattice encoded values (e.g. 0.0625) can be accurately
        // interpolated.  A 17-point inverse gives lattice spacing of 1/16,
        // which resolves 0.0625 exactly.
        let inv_size = 17;
        let scale = (size - 1) as f64;
        let mut lut = Vec::with_capacity(size * size * size);
        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let rf = (r as f64 / scale).powf(2.0);
                    let gf = (g as f64 / scale).powf(2.0);
                    let bf = (b as f64 / scale).powf(2.0);
                    lut.push([rf, gf, bf]);
                }
            }
        }

        let inv = invert_lut3d(&lut, size, inv_size, 200, 1e-6);

        // Forward then inverse should approximate identity within 0.1
        let test_points: Vec<Rgb> = vec![
            [0.0, 0.0, 0.0],
            [0.25, 0.25, 0.25],
            [0.5, 0.5, 0.5],
            [1.0, 1.0, 1.0],
        ];
        for pt in &test_points {
            let encoded = apply_lut3d(&lut, size, pt);
            let decoded = apply_lut3d(&inv, inv_size, &encoded);
            for ch in 0..3 {
                assert!(
                    (decoded[ch] - pt[ch]).abs() < 0.1,
                    "Roundtrip failed at {:?} ch={ch}: decoded={}",
                    pt,
                    decoded[ch]
                );
            }
        }
    }

    #[test]
    fn test_solve_3x3_identity() {
        let a = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let b = [1.0, 2.0, 3.0];
        let x = solve_3x3(&a, &b);
        assert!(x.is_some());
        let x = x.expect("solve should succeed for identity");
        assert!((x[0] - 1.0).abs() < 1e-10);
        assert!((x[1] - 2.0).abs() < 1e-10);
        assert!((x[2] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_solve_3x3_singular() {
        let a = [[1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let b = [1.0, 2.0, 3.0];
        assert!(solve_3x3(&a, &b).is_none());
    }
}
