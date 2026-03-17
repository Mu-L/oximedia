//! SIMD-optimized summing for mix bus computation.
//!
//! Provides vectorized sum-with-gain operations used by the matrix router.
//! On targets that do not support SIMD, falls back to scalar accumulation.
//!
//! All functions in this module are pure Rust (no intrinsics) but are
//! structured to auto-vectorize well with LLVM.

/// Sums `samples[i] * gains[i]` for paired slices.
///
/// Returns the accumulated sum. The slices must be the same length;
/// if they differ, only the shorter length is used.
pub fn sum_with_gains(samples: &[f32], gains: &[f32]) -> f32 {
    let len = samples.len().min(gains.len());
    // Process in chunks of 8 for autovectorization.
    let chunks = len / 8;
    let remainder = len % 8;

    let mut acc = [0.0_f32; 8];

    for chunk in 0..chunks {
        let base = chunk * 8;
        for j in 0..8 {
            acc[j] += samples[base + j] * gains[base + j];
        }
    }

    let mut total: f32 = acc.iter().sum();

    // Handle remainder
    let tail_base = chunks * 8;
    for i in 0..remainder {
        total += samples[tail_base + i] * gains[tail_base + i];
    }

    total
}

/// Sums `samples[indices[i]] * gains[i]` for sparse routing lookups.
///
/// This is the common pattern in crosspoint matrices where only some
/// inputs contribute to an output.
pub fn sparse_sum_with_gains(samples: &[f32], indices: &[usize], gains: &[f32]) -> f32 {
    let len = indices.len().min(gains.len());
    let mut sum = 0.0_f32;
    for i in 0..len {
        let idx = indices[i];
        if idx < samples.len() {
            sum += samples[idx] * gains[i];
        }
    }
    sum
}

/// Converts dB values to linear gains in-place.
///
/// `10^(db / 20)` for each element.
pub fn db_to_linear_batch(db_values: &[f32], out: &mut [f32]) {
    let len = db_values.len().min(out.len());
    for i in 0..len {
        out[i] = 10.0_f32.powf(db_values[i] / 20.0);
    }
}

/// Sums multiple input buffers with per-input gain, writing the result
/// into an output buffer (mix bus).
///
/// `output[s] = sum(input_buffers[i][s] * gains[i])` for each sample `s`.
pub fn mix_bus_sum(input_buffers: &[&[f32]], gains: &[f32], output: &mut [f32]) {
    let num_inputs = input_buffers.len().min(gains.len());

    // Zero the output
    for s in output.iter_mut() {
        *s = 0.0;
    }

    for i in 0..num_inputs {
        let gain = gains[i];
        let buf = input_buffers[i];
        let len = buf.len().min(output.len());
        for j in 0..len {
            output[j] += buf[j] * gain;
        }
    }
}

/// Scales a buffer in-place by a linear gain factor.
pub fn scale_buffer(buffer: &mut [f32], gain: f32) {
    for s in buffer.iter_mut() {
        *s *= gain;
    }
}

/// Adds `src` into `dst` (element-wise accumulate).
pub fn accumulate(dst: &mut [f32], src: &[f32]) {
    let len = dst.len().min(src.len());
    for i in 0..len {
        dst[i] += src[i];
    }
}

/// Peak absolute value of a buffer.
pub fn peak_abs(buffer: &[f32]) -> f32 {
    let mut peak = 0.0_f32;
    for &s in buffer {
        let abs = s.abs();
        if abs > peak {
            peak = abs;
        }
    }
    peak
}

/// RMS of a buffer.
pub fn rms(buffer: &[f32]) -> f32 {
    if buffer.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = buffer.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum_sq / buffer.len() as f64).sqrt() as f32
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sum_with_gains_basic() {
        let samples = [1.0_f32, 2.0, 3.0, 4.0];
        let gains = [1.0_f32, 0.5, 0.25, 0.125];
        let result = sum_with_gains(&samples, &gains);
        // 1*1 + 2*0.5 + 3*0.25 + 4*0.125 = 1 + 1 + 0.75 + 0.5 = 3.25
        assert!((result - 3.25).abs() < 1e-5);
    }

    #[test]
    fn test_sum_with_gains_empty() {
        assert!(sum_with_gains(&[], &[]).abs() < 1e-10);
    }

    #[test]
    fn test_sum_with_gains_mismatched_lengths() {
        let samples = [1.0_f32, 2.0, 3.0];
        let gains = [1.0_f32, 0.5];
        let result = sum_with_gains(&samples, &gains);
        // Only first 2 used: 1*1 + 2*0.5 = 2.0
        assert!((result - 2.0).abs() < 1e-5);
    }

    #[test]
    fn test_sum_with_gains_large() {
        // Test with 100 elements to exercise the chunked loop
        let samples: Vec<f32> = (0..100).map(|i| i as f32 * 0.01).collect();
        let gains: Vec<f32> = vec![1.0; 100];
        let result = sum_with_gains(&samples, &gains);
        let expected: f32 = samples.iter().sum();
        assert!((result - expected).abs() < 1e-3);
    }

    #[test]
    fn test_sparse_sum_basic() {
        let samples = [0.0_f32, 0.5, 0.0, 0.8];
        let indices = [1, 3];
        let gains = [1.0_f32, 0.5];
        let result = sparse_sum_with_gains(&samples, &indices, &gains);
        // 0.5*1.0 + 0.8*0.5 = 0.9
        assert!((result - 0.9).abs() < 1e-5);
    }

    #[test]
    fn test_sparse_sum_out_of_bounds_index() {
        let samples = [1.0_f32, 2.0];
        let indices = [0, 99]; // index 99 out of bounds
        let gains = [1.0, 1.0];
        let result = sparse_sum_with_gains(&samples, &indices, &gains);
        // Only index 0 valid: 1.0
        assert!((result - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_db_to_linear_batch() {
        let db = [0.0_f32, -20.0, -6.0];
        let mut out = [0.0_f32; 3];
        db_to_linear_batch(&db, &mut out);
        assert!((out[0] - 1.0).abs() < 1e-5);
        assert!((out[1] - 0.1).abs() < 0.01);
        assert!((out[2] - 0.5012).abs() < 0.01);
    }

    #[test]
    fn test_mix_bus_sum() {
        let buf_a = [1.0_f32, 0.5, 0.25];
        let buf_b = [0.5_f32, 0.25, 0.125];
        let gains = [1.0_f32, 0.5];
        let mut output = [0.0_f32; 3];
        mix_bus_sum(&[&buf_a, &buf_b], &gains, &mut output);
        // output[0] = 1.0*1.0 + 0.5*0.5 = 1.25
        assert!((output[0] - 1.25).abs() < 1e-5);
        // output[1] = 0.5*1.0 + 0.25*0.5 = 0.625
        assert!((output[1] - 0.625).abs() < 1e-5);
    }

    #[test]
    fn test_scale_buffer() {
        let mut buf = [1.0_f32, 2.0, 3.0];
        scale_buffer(&mut buf, 0.5);
        assert!((buf[0] - 0.5).abs() < 1e-5);
        assert!((buf[1] - 1.0).abs() < 1e-5);
        assert!((buf[2] - 1.5).abs() < 1e-5);
    }

    #[test]
    fn test_accumulate() {
        let mut dst = [1.0_f32, 2.0, 3.0];
        let src = [0.5_f32, 0.5, 0.5];
        accumulate(&mut dst, &src);
        assert!((dst[0] - 1.5).abs() < 1e-5);
        assert!((dst[1] - 2.5).abs() < 1e-5);
        assert!((dst[2] - 3.5).abs() < 1e-5);
    }

    #[test]
    fn test_peak_abs() {
        let buf = [0.5_f32, -0.8, 0.3, -0.1];
        assert!((peak_abs(&buf) - 0.8).abs() < 1e-5);
    }

    #[test]
    fn test_peak_abs_empty() {
        assert!(peak_abs(&[]).abs() < 1e-10);
    }

    #[test]
    fn test_rms_constant() {
        let buf = [0.5_f32; 100];
        assert!((rms(&buf) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn test_rms_empty() {
        assert!(rms(&[]).abs() < 1e-10);
    }

    #[test]
    fn test_rms_sine() {
        // RMS of a sine wave is amplitude / sqrt(2)
        let buf: Vec<f32> = (0..48000)
            .map(|i| {
                let t = i as f64 / 48000.0;
                (2.0 * std::f64::consts::PI * 1000.0 * t).sin() as f32
            })
            .collect();
        let r = rms(&buf);
        let expected = 1.0_f32 / 2.0_f32.sqrt();
        assert!((r - expected).abs() < 0.01);
    }
}
