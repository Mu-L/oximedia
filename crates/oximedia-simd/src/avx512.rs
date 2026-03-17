//! AVX-512 SIMD implementations for high-throughput media processing.
//!
//! All `unsafe` functions in this module use `#[target_feature(enable = "...")]`
//! and must only be called after runtime detection via `is_x86_feature_detected!`.
//!
//! Safe public wrappers that perform runtime dispatch are provided at the
//! bottom of this file under the re-exported names used by `dispatch.rs`.

#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::too_many_lines)]

// ── BGRA → RGBA ─────────────────────────────────────────────────────────────

/// Scalar BGRA to RGBA conversion.
///
/// Swaps B and R channels for every 4-byte pixel.  The shorter of the two
/// slices determines the number of complete pixels processed.
pub fn bgra_to_rgba_scalar(src: &[u8], dst: &mut [u8]) {
    let pixels = src.len().min(dst.len()) / 4;
    for i in 0..pixels {
        let b = i * 4;
        dst[b] = src[b + 2]; // R ← B position
        dst[b + 1] = src[b + 1]; // G unchanged
        dst[b + 2] = src[b]; // B ← R position
        dst[b + 3] = src[b + 3]; // A unchanged
    }
}

/// AVX-512BW BGRA to RGBA conversion.
///
/// Processes 16 pixels (64 bytes) per loop iteration using a byte-shuffle
/// over a 512-bit register.  Remaining pixels that do not fill a full
/// 512-bit lane are handled by the scalar fallback.
///
/// # Safety
///
/// Caller must ensure that `avx512f` and `avx512bw` CPU features are
/// available (detected via `is_x86_feature_detected!`).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512bw")]
pub unsafe fn bgra_to_rgba_avx512(src: &[u8], dst: &mut [u8]) {
    use std::arch::x86_64::*;

    let pixels = src.len().min(dst.len()) / 4;
    let simd_pixels = pixels & !15; // round down to multiple of 16

    // Shuffle mask: for each 4-byte BGRA group swap indices 0↔2.
    // Pattern repeats 16 times to fill all 64 bytes of the ZMM register.
    // BGRA = [B,G,R,A] at offsets [0,1,2,3]; we want RGBA = [R,G,B,A].
    // So output byte k = input byte [2,1,0,3] within each group.
    let mut mask_arr = [0u8; 64];
    for group in 0..16usize {
        let base = group * 4;
        // Within each 16-byte lane, _mm512_shuffle_epi8 applies a 16-byte
        // shuffle pattern.  Each 16-byte lane holds 4 pixels.
        // Within the lane: pixel offsets 0,4,8,12.
        // We replicate the 4-byte [2,1,0,3] pattern across 16 bytes per lane
        // then tile 4 lanes to fill 64 bytes.
        mask_arr[base] = (base + 2) as u8;
        mask_arr[base + 1] = (base + 1) as u8;
        mask_arr[base + 2] = base as u8;
        mask_arr[base + 3] = (base + 3) as u8;
    }
    // _mm512_shuffle_epi8 wraps indices within each 16-byte lane, so we
    // must keep mask indices relative to the lane (mod 16).
    // Rewrite mask so each entry is lane-local (0..15).
    for group in 0..16usize {
        let base = group * 4;
        let lane_offset = (group % 4) * 4; // offset within the 16-byte lane
        mask_arr[base] = (lane_offset + 2) as u8;
        mask_arr[base + 1] = (lane_offset + 1) as u8;
        mask_arr[base + 2] = lane_offset as u8;
        mask_arr[base + 3] = (lane_offset + 3) as u8;
    }

    // SAFETY: avx512f,avx512bw ensured by caller and target_feature gate.
    let shuffle_mask = _mm512_loadu_si512(mask_arr.as_ptr() as *const __m512i);

    let mut i = 0usize;
    while i < simd_pixels {
        let src_ptr = src.as_ptr().add(i * 4) as *const __m512i;
        let dst_ptr = dst.as_mut_ptr().add(i * 4) as *mut __m512i;

        // SAFETY: bounds guaranteed by simd_pixels calculation and slice validity.
        let data = _mm512_loadu_si512(src_ptr);
        let shuffled = _mm512_shuffle_epi8(data, shuffle_mask);
        _mm512_storeu_si512(dst_ptr, shuffled);

        i += 16;
    }

    // Handle remaining pixels with scalar fallback.
    let rem_byte_start = simd_pixels * 4;
    bgra_to_rgba_scalar(&src[rem_byte_start..], &mut dst[rem_byte_start..]);
}

// ── Horizontal sum of f32 ────────────────────────────────────────────────────

/// Scalar horizontal sum of an f32 slice.
pub fn hsum_f32_scalar(data: &[f32]) -> f32 {
    data.iter().copied().fold(0.0f32, |acc, x| acc + x)
}

/// AVX-512F horizontal sum of an f32 slice.
///
/// Processes 16 f32 values per iteration (512 bits), reducing via
/// `_mm512_reduce_add_ps` which performs a tree reduction inside the ZMM
/// register.  Remaining elements are handled with the scalar fallback.
///
/// # Safety
///
/// Caller must ensure that `avx512f` CPU feature is available.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
pub unsafe fn hsum_f32_avx512(data: &[f32]) -> f32 {
    use std::arch::x86_64::*;

    let simd_len = data.len() & !15; // round down to multiple of 16
                                     // SAFETY: avx512f ensured by caller.
    let mut acc = _mm512_setzero_ps();

    let mut i = 0usize;
    while i < simd_len {
        // SAFETY: i + 16 <= data.len() guaranteed by simd_len.
        let chunk = _mm512_loadu_ps(data.as_ptr().add(i));
        acc = _mm512_add_ps(acc, chunk);
        i += 16;
    }

    // Reduce the 512-bit accumulator to a scalar.
    // SAFETY: avx512f available.
    let mut total = _mm512_reduce_add_ps(acc);

    // Scalar tail.
    for &v in &data[simd_len..] {
        total += v;
    }
    total
}

// ── Scale i16 samples by f32 gain ───────────────────────────────────────────

/// Scalar i16 sample scaling by a gain factor.
pub fn scale_i16_scalar(samples: &mut [i16], gain: f32) {
    for s in samples.iter_mut() {
        let scaled = f32::from(*s) * gain;
        *s = scaled.clamp(-32768.0, 32767.0) as i16;
    }
}

/// AVX-512F i16 sample scaling by a gain factor.
///
/// Strategy: load 16 i16 values (256 bits) into a `__m256i`, sign-extend to
/// 16 × i32 in a `__m512i`, convert to f32, multiply by gain, convert back
/// to i32 (saturating), then truncate to i16 via `_mm512_cvtepi32_epi16`.
/// Processes 16 i16 samples per iteration.
///
/// # Safety
///
/// Caller must ensure that `avx512f` CPU feature is available.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512bw")]
pub unsafe fn scale_i16_avx512(samples: &mut [i16], gain: f32) {
    use std::arch::x86_64::*;

    let simd_len = samples.len() & !15; // round down to multiple of 16
                                        // SAFETY: avx512f ensured by caller.
    let gain_vec = _mm512_set1_ps(gain);
    let max_val = _mm512_set1_ps(32767.0f32);
    let min_val = _mm512_set1_ps(-32768.0f32);

    let mut i = 0usize;
    while i < simd_len {
        let ptr = samples.as_mut_ptr().add(i);

        // SAFETY: bounds guaranteed by simd_len.
        // Load 16 × i16 → __m256i.
        let i16_vec = _mm256_loadu_epi16(ptr as *const i16);
        // Sign-extend 16 × i16 → 16 × i32 in __m512i.
        let i32_vec = _mm512_cvtepi16_epi32(i16_vec);
        // Convert 16 × i32 → 16 × f32.
        let f32_vec = _mm512_cvtepi32_ps(i32_vec);
        // Multiply by gain.
        let scaled = _mm512_mul_ps(f32_vec, gain_vec);
        // Clamp to i16 range.
        let clamped = _mm512_min_ps(_mm512_max_ps(scaled, min_val), max_val);
        // Convert back: f32 → i32 (truncate).
        let as_i32 = _mm512_cvtps_epi32(clamped);
        // Narrow i32 → i16 (lower 16 bits of each lane).
        let as_i16 = _mm512_cvtepi32_epi16(as_i32);
        // Store 16 × i16.
        _mm256_storeu_epi16(ptr, as_i16);

        i += 16;
    }

    // Scalar tail.
    scale_i16_scalar(&mut samples[simd_len..], gain);
}

// ── Dot product of two f32 slices ────────────────────────────────────────────

/// Scalar f32 dot product.
pub fn dot_product_f32_scalar(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// AVX-512F f32 dot product.
///
/// Uses FMA (`_mm512_fmadd_ps`) to accumulate 16 products per iteration,
/// then reduces with `_mm512_reduce_add_ps`.
///
/// # Safety
///
/// Caller must ensure that `avx512f` CPU feature is available.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
pub unsafe fn dot_product_f32_avx512(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::x86_64::*;

    let len = a.len().min(b.len());
    let simd_len = len & !15; // round down to multiple of 16

    // SAFETY: avx512f ensured by caller.
    let mut acc = _mm512_setzero_ps();

    let mut i = 0usize;
    while i < simd_len {
        // SAFETY: i + 16 <= simd_len <= len guaranteed.
        let va = _mm512_loadu_ps(a.as_ptr().add(i));
        let vb = _mm512_loadu_ps(b.as_ptr().add(i));
        // acc += va * vb
        acc = _mm512_fmadd_ps(va, vb, acc);
        i += 16;
    }

    // SAFETY: avx512f available.
    let mut total = _mm512_reduce_add_ps(acc);

    // Scalar tail.
    for j in simd_len..len {
        total += a[j] * b[j];
    }
    total
}

// ── Clamp f32 to [0.0, 1.0] ─────────────────────────────────────────────────

/// Scalar clamp of an f32 slice to [0.0, 1.0] in place.
pub fn clamp_f32_scalar(data: &mut [f32]) {
    for v in data.iter_mut() {
        *v = v.clamp(0.0, 1.0);
    }
}

/// AVX-512F clamp of an f32 slice to [0.0, 1.0] in place.
///
/// Uses `_mm512_max_ps` / `_mm512_min_ps` to clamp 16 values per iteration.
///
/// # Safety
///
/// Caller must ensure that `avx512f` CPU feature is available.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
pub unsafe fn clamp_f32_avx512(data: &mut [f32]) {
    use std::arch::x86_64::*;

    let simd_len = data.len() & !15; // round down to multiple of 16

    // SAFETY: avx512f ensured by caller.
    let zero = _mm512_setzero_ps();
    let one = _mm512_set1_ps(1.0f32);

    let mut i = 0usize;
    while i < simd_len {
        let ptr = data.as_mut_ptr().add(i);

        // SAFETY: i + 16 <= simd_len <= data.len() guaranteed.
        let v = _mm512_loadu_ps(ptr);
        let clamped = _mm512_min_ps(_mm512_max_ps(v, zero), one);
        _mm512_storeu_ps(ptr, clamped);

        i += 16;
    }

    // Scalar tail.
    clamp_f32_scalar(&mut data[simd_len..]);
}

// ── Safe runtime-dispatched public API ───────────────────────────────────────

/// Safe runtime-dispatched BGRA to RGBA conversion.
///
/// On x86-64 with AVX-512BW the hardware shuffle path is used; otherwise
/// falls back to a scalar byte-swap loop.
pub fn bgra_to_rgba(src: &[u8], dst: &mut [u8]) {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512bw") {
            // SAFETY: avx512f and avx512bw detected at runtime above.
            unsafe { bgra_to_rgba_avx512(src, dst) };
            return;
        }
    }
    bgra_to_rgba_scalar(src, dst);
}

/// Safe runtime-dispatched horizontal sum of an f32 slice.
pub fn hsum_f32(data: &[f32]) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512f") {
            // SAFETY: avx512f detected at runtime above.
            return unsafe { hsum_f32_avx512(data) };
        }
    }
    hsum_f32_scalar(data)
}

/// Safe runtime-dispatched i16 sample scaling by a gain factor.
pub fn scale_i16(samples: &mut [i16], gain: f32) {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512f") && is_x86_feature_detected!("avx512bw") {
            // SAFETY: avx512f and avx512bw detected at runtime above.
            unsafe { scale_i16_avx512(samples, gain) };
            return;
        }
    }
    scale_i16_scalar(samples, gain);
}

/// Safe runtime-dispatched f32 dot product.
pub fn dot_product_f32(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512f") {
            // SAFETY: avx512f detected at runtime above.
            return unsafe { dot_product_f32_avx512(a, b) };
        }
    }
    dot_product_f32_scalar(a, b)
}

/// Safe runtime-dispatched f32 clamp to [0.0, 1.0] in place.
pub fn clamp_f32(data: &mut [f32]) {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512f") {
            // SAFETY: avx512f detected at runtime above.
            unsafe { clamp_f32_avx512(data) };
            return;
        }
    }
    clamp_f32_scalar(data);
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: approx equality for f32
    fn approx_eq(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    // ── bgra_to_rgba ─────────────────────────────────────────────────────────

    #[test]
    fn test_bgra_to_rgba_scalar_basic() {
        // One pixel: BGRA = [10, 20, 30, 255] → RGBA = [30, 20, 10, 255]
        let src = [10u8, 20, 30, 255];
        let mut dst = [0u8; 4];
        bgra_to_rgba_scalar(&src, &mut dst);
        assert_eq!(dst, [30, 20, 10, 255]);
    }

    #[test]
    fn test_bgra_to_rgba_scalar_multiple_pixels() {
        let src = [10u8, 20, 30, 255, 100, 150, 200, 128];
        let mut dst = [0u8; 8];
        bgra_to_rgba_scalar(&src, &mut dst);
        assert_eq!(dst[0], 30);
        assert_eq!(dst[1], 20);
        assert_eq!(dst[2], 10);
        assert_eq!(dst[3], 255);
        assert_eq!(dst[4], 200);
        assert_eq!(dst[5], 150);
        assert_eq!(dst[6], 100);
        assert_eq!(dst[7], 128);
    }

    #[test]
    fn test_bgra_to_rgba_scalar_empty() {
        let src: [u8; 0] = [];
        let mut dst: [u8; 0] = [];
        bgra_to_rgba_scalar(&src, &mut dst); // must not panic
    }

    #[test]
    fn test_bgra_to_rgba_scalar_identity_when_b_eq_r() {
        // When B == R the result looks the same
        let src = [128u8, 64, 128, 200];
        let mut dst = [0u8; 4];
        bgra_to_rgba_scalar(&src, &mut dst);
        assert_eq!(dst, [128, 64, 128, 200]);
    }

    #[test]
    fn test_bgra_to_rgba_dispatch_matches_scalar() {
        // 20 pixels → exercises simd path + scalar tail (if AVX-512 available)
        // and scalar-only path otherwise. Both must produce same result.
        let src: Vec<u8> = (0..80).map(|i| i as u8).collect();
        let mut dst_dispatch = vec![0u8; 80];
        let mut dst_scalar = vec![0u8; 80];

        bgra_to_rgba(&src, &mut dst_dispatch);
        bgra_to_rgba_scalar(&src, &mut dst_scalar);

        assert_eq!(dst_dispatch, dst_scalar);
    }

    #[test]
    fn test_bgra_to_rgba_dispatch_17_pixels() {
        // 17 pixels = 68 bytes: tests unaligned tail handling
        let src: Vec<u8> = (0..68).map(|i| (i * 3) as u8).collect();
        let mut dst_dispatch = vec![0u8; 68];
        let mut dst_scalar = vec![0u8; 68];

        bgra_to_rgba(&src, &mut dst_dispatch);
        bgra_to_rgba_scalar(&src, &mut dst_scalar);

        assert_eq!(dst_dispatch, dst_scalar);
    }

    #[test]
    fn test_bgra_to_rgba_dispatch_single_pixel() {
        let src = [5u8, 10, 15, 20];
        let mut dst = [0u8; 4];
        bgra_to_rgba(&src, &mut dst);
        assert_eq!(dst, [15, 10, 5, 20]);
    }

    // ── hsum_f32 ─────────────────────────────────────────────────────────────

    #[test]
    fn test_hsum_f32_scalar_empty() {
        assert!(approx_eq(hsum_f32_scalar(&[]), 0.0, 1e-6));
    }

    #[test]
    fn test_hsum_f32_scalar_single() {
        assert!(approx_eq(hsum_f32_scalar(&[42.0]), 42.0, 1e-5));
    }

    #[test]
    fn test_hsum_f32_scalar_known() {
        let data = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        assert!(approx_eq(hsum_f32_scalar(&data), 36.0, 1e-4));
    }

    #[test]
    fn test_hsum_f32_dispatch_matches_scalar() {
        let data: Vec<f32> = (0..33).map(|i| i as f32 * 0.5).collect(); // 33 = non-multiple of 16
        let expected = hsum_f32_scalar(&data);
        let result = hsum_f32(&data);
        assert!(approx_eq(result, expected, 1e-3));
    }

    #[test]
    fn test_hsum_f32_dispatch_16_elements() {
        let data = [1.0f32; 16]; // exactly one SIMD lane
        let result = hsum_f32(&data);
        assert!(approx_eq(result, 16.0, 1e-4));
    }

    #[test]
    fn test_hsum_f32_dispatch_empty() {
        assert!(approx_eq(hsum_f32(&[]), 0.0, 1e-6));
    }

    // ── scale_i16 ────────────────────────────────────────────────────────────

    #[test]
    fn test_scale_i16_scalar_double() {
        let mut samples = [100i16, -200, 300, -400];
        scale_i16_scalar(&mut samples, 2.0);
        assert_eq!(samples, [200, -400, 600, -800]);
    }

    #[test]
    fn test_scale_i16_scalar_zero_gain() {
        let mut samples = [1000i16, -2000, 3000];
        scale_i16_scalar(&mut samples, 0.0);
        assert_eq!(samples, [0, 0, 0]);
    }

    #[test]
    fn test_scale_i16_scalar_saturation() {
        let mut samples = [32000i16];
        scale_i16_scalar(&mut samples, 2.0); // would overflow i16 → clamp to 32767
        assert_eq!(samples[0], 32767);
    }

    #[test]
    fn test_scale_i16_scalar_negative_saturation() {
        let mut samples = [-32000i16];
        scale_i16_scalar(&mut samples, 2.0); // would underflow → clamp to -32768
        assert_eq!(samples[0], -32768);
    }

    #[test]
    fn test_scale_i16_dispatch_matches_scalar() {
        let mut samples_dispatch: Vec<i16> = (0..33).map(|i| (i * 100) as i16).collect();
        let mut samples_scalar = samples_dispatch.clone();

        scale_i16(&mut samples_dispatch, 1.5);
        scale_i16_scalar(&mut samples_scalar, 1.5);

        assert_eq!(samples_dispatch, samples_scalar);
    }

    #[test]
    fn test_scale_i16_dispatch_empty() {
        let mut empty: [i16; 0] = [];
        scale_i16(&mut empty, 2.0); // must not panic
    }

    // ── dot_product_f32 ──────────────────────────────────────────────────────

    #[test]
    fn test_dot_product_f32_scalar_basic() {
        let a = [1.0f32, 2.0, 3.0];
        let b = [4.0f32, 5.0, 6.0];
        // 1*4 + 2*5 + 3*6 = 4+10+18 = 32
        assert!(approx_eq(dot_product_f32_scalar(&a, &b), 32.0, 1e-4));
    }

    #[test]
    fn test_dot_product_f32_scalar_empty() {
        assert!(approx_eq(dot_product_f32_scalar(&[], &[]), 0.0, 1e-6));
    }

    #[test]
    fn test_dot_product_f32_scalar_orthogonal() {
        let a = [1.0f32, 0.0, 0.0];
        let b = [0.0f32, 1.0, 0.0];
        assert!(approx_eq(dot_product_f32_scalar(&a, &b), 0.0, 1e-6));
    }

    #[test]
    fn test_dot_product_f32_dispatch_matches_scalar() {
        let a: Vec<f32> = (0..33).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..33).map(|i| (33 - i) as f32).collect();
        let expected = dot_product_f32_scalar(&a, &b);
        let result = dot_product_f32(&a, &b);
        assert!(approx_eq(result, expected, 0.1)); // some floating-point order difference allowed
    }

    #[test]
    fn test_dot_product_f32_dispatch_16_elements() {
        let a = [1.0f32; 16];
        let b = [2.0f32; 16];
        let result = dot_product_f32(&a, &b);
        assert!(approx_eq(result, 32.0, 1e-3)); // 16 * 1 * 2 = 32
    }

    #[test]
    fn test_dot_product_f32_dispatch_empty() {
        assert!(approx_eq(dot_product_f32(&[], &[]), 0.0, 1e-6));
    }

    // ── clamp_f32 ────────────────────────────────────────────────────────────

    #[test]
    fn test_clamp_f32_scalar_basic() {
        let mut data = [-1.0f32, 0.0, 0.5, 1.0, 2.0];
        clamp_f32_scalar(&mut data);
        assert_eq!(data, [0.0, 0.0, 0.5, 1.0, 1.0]);
    }

    #[test]
    fn test_clamp_f32_scalar_empty() {
        let mut data: [f32; 0] = [];
        clamp_f32_scalar(&mut data); // must not panic
    }

    #[test]
    fn test_clamp_f32_scalar_already_clamped() {
        let mut data = [0.0f32, 0.25, 0.5, 0.75, 1.0];
        let original = data;
        clamp_f32_scalar(&mut data);
        for (a, b) in data.iter().zip(original.iter()) {
            assert!(approx_eq(*a, *b, 1e-6));
        }
    }

    #[test]
    fn test_clamp_f32_dispatch_matches_scalar() {
        let raw: Vec<f32> = (0..33).map(|i| (i as f32 - 16.0) * 0.1).collect(); // mix of negative, 0-1, >1
        let mut data_dispatch = raw.clone();
        let mut data_scalar = raw.clone();

        clamp_f32(&mut data_dispatch);
        clamp_f32_scalar(&mut data_scalar);

        for (a, b) in data_dispatch.iter().zip(data_scalar.iter()) {
            assert!(approx_eq(*a, *b, 1e-6));
        }
    }

    #[test]
    fn test_clamp_f32_dispatch_16_elements() {
        // 16 values spanning [-2, 3] → all should land in [0, 1]
        let mut data: Vec<f32> = (0..16).map(|i| i as f32 * 0.4 - 2.0).collect();
        clamp_f32(&mut data);
        for &v in &data {
            assert!(v >= 0.0 && v <= 1.0, "value {v} out of [0,1]");
        }
    }

    #[test]
    fn test_clamp_f32_dispatch_empty() {
        let mut data: Vec<f32> = Vec::new();
        clamp_f32(&mut data); // must not panic
    }

    #[test]
    fn test_clamp_f32_dispatch_17_elements() {
        let mut data: Vec<f32> = (0..17).map(|i| i as f32 * 0.2 - 1.0).collect();
        let mut expected = data.clone();
        clamp_f32_scalar(&mut expected);
        clamp_f32(&mut data);
        for (a, b) in data.iter().zip(expected.iter()) {
            assert!(approx_eq(*a, *b, 1e-6));
        }
    }
}

// ── AVX-512 availability check ───────────────────────────────────────────────

/// Returns `true` when the executing CPU supports AVX-512F.
///
/// On x86-64 this performs a runtime CPUID check via the standard
/// `is_x86_feature_detected!` macro (cached by the OS/runtime).
/// On all other architectures this always returns `false`.
#[must_use]
pub fn avx512_available() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("avx512f")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

// ── RGBA → planar YUV 4:2:0 (AVX-512 dispatched) ────────────────────────────

/// Scalar RGBA → planar YUV 4:2:0 conversion.
///
/// BT.601 limited-range Q8 coefficients:
/// ```text
///   Y  = (  66·R + 129·G +  25·B + 128) >> 8 + 16
///   Cb = ( -38·R -  74·G + 112·B + 128) >> 8 + 128
///   Cr = ( 112·R -  94·G -  18·B + 128) >> 8 + 128
/// ```
/// Chroma is 2×2 sub-sampled (4:2:0) using the top-left pixel of each 2×2 block.
///
/// Returns a flat `Vec<u8>` with layout: Y plane (w×h bytes), U plane
/// ((w/2)×(h/2) bytes), V plane ((w/2)×(h/2) bytes).
fn rgba_to_yuv420_scalar(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let chroma_w = w / 2;
    let chroma_h = h / 2;
    let y_size = w * h;
    let uv_size = chroma_w * chroma_h;
    let mut out = vec![0u8; y_size + uv_size * 2];

    let (y_plane, rest) = out.split_at_mut(y_size);
    let (u_plane, v_plane) = rest.split_at_mut(uv_size);

    for row in 0..h {
        for col in 0..w {
            let src = (row * w + col) * 4;
            let r = rgba[src] as i32;
            let g = rgba[src + 1] as i32;
            let b = rgba[src + 2] as i32;

            let y = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
            y_plane[row * w + col] = y.clamp(16, 235) as u8;

            // Chroma at 2:1 sub-sampling — only top-left pixel of each 2×2 block.
            if row % 2 == 0 && col % 2 == 0 {
                let cu = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
                let cv = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
                let uv_idx = (row / 2) * chroma_w + col / 2;
                u_plane[uv_idx] = cu.clamp(16, 240) as u8;
                v_plane[uv_idx] = cv.clamp(16, 240) as u8;
            }
        }
    }

    out
}

/// AVX-512F RGBA → planar YUV 4:2:0 conversion (unsafe inner kernel).
///
/// Processes pixels in 16-wide groups when AVX-512F is available, filling the
/// Y plane with vectorised BT.601 arithmetic.  Chroma planes are filled
/// using the scalar helper (chroma computation is memory-bound anyway).
///
/// # Safety
///
/// Caller must ensure `avx512f` CPU feature is available.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn rgba_to_yuv420_avx512_inner(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    use std::arch::x86_64::*;

    let w = width as usize;
    let h = height as usize;
    let chroma_w = w / 2;
    let chroma_h = h / 2;
    let y_size = w * h;
    let uv_size = chroma_w * chroma_h;
    let mut out = vec![0u8; y_size + uv_size * 2];

    let (y_plane, rest) = out.split_at_mut(y_size);
    let (u_plane, v_plane) = rest.split_at_mut(uv_size);

    // BT.601 Y coefficients in Q8 (broadcast to 16-wide f32 vectors).
    // SAFETY: avx512f available per target_feature gate.
    let coeff_r = _mm512_set1_ps(66.0_f32);
    let coeff_g = _mm512_set1_ps(129.0_f32);
    let coeff_b = _mm512_set1_ps(25.0_f32);
    let bias = _mm512_set1_ps(128.0_f32);
    let shift = _mm512_set1_ps(1.0_f32 / 256.0_f32); // divide by 256
    let offset16 = _mm512_set1_ps(16.0_f32);
    let max235 = _mm512_set1_ps(235.0_f32);

    let pixels = w * h;
    let simd_len = pixels & !15; // multiple of 16

    let mut i = 0usize;
    while i < simd_len {
        // Gather R, G, B channels from the RGBA interleaved buffer.
        let mut r_arr = [0.0f32; 16];
        let mut g_arr = [0.0f32; 16];
        let mut b_arr = [0.0f32; 16];
        for k in 0..16usize {
            let base = (i + k) * 4;
            r_arr[k] = rgba[base] as f32;
            g_arr[k] = rgba[base + 1] as f32;
            b_arr[k] = rgba[base + 2] as f32;
        }

        // SAFETY: arrays are 16 elements, load unaligned.
        let vr = _mm512_loadu_ps(r_arr.as_ptr());
        let vg = _mm512_loadu_ps(g_arr.as_ptr());
        let vb = _mm512_loadu_ps(b_arr.as_ptr());

        // Y = (66*R + 129*G + 25*B + 128) / 256 + 16, clamped to [16, 235].
        let mut vy = _mm512_fmadd_ps(vr, coeff_r, bias);
        vy = _mm512_fmadd_ps(vg, coeff_g, vy);
        vy = _mm512_fmadd_ps(vb, coeff_b, vy);
        vy = _mm512_mul_ps(vy, shift);
        vy = _mm512_add_ps(vy, offset16);
        vy = _mm512_min_ps(vy, max235);
        vy = _mm512_max_ps(vy, offset16);

        // Convert f32 → u8 via i32.
        let yi32 = _mm512_cvtps_epi32(vy);
        let mut y_arr = [0i32; 16];
        _mm512_storeu_si512(y_arr.as_mut_ptr() as *mut __m512i, yi32);
        for k in 0..16usize {
            y_plane[i + k] = y_arr[k].clamp(16, 235) as u8;
        }

        i += 16;
    }

    // Scalar tail for remaining pixels in Y plane.
    for j in simd_len..pixels {
        let base = j * 4;
        let r = rgba[base] as i32;
        let g = rgba[base + 1] as i32;
        let b = rgba[base + 2] as i32;
        let y = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
        y_plane[j] = y.clamp(16, 235) as u8;
    }

    // Chroma planes via scalar (memory-bound, AVX-512 overhead not worth it).
    for row in (0..h).step_by(2) {
        for col in (0..w).step_by(2) {
            let src = (row * w + col) * 4;
            let r = rgba[src] as i32;
            let g = rgba[src + 1] as i32;
            let b = rgba[src + 2] as i32;
            let cu = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
            let cv = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
            let uv_idx = (row / 2) * chroma_w + col / 2;
            u_plane[uv_idx] = cu.clamp(16, 240) as u8;
            v_plane[uv_idx] = cv.clamp(16, 240) as u8;
        }
    }

    out
}

/// Safe runtime-dispatched RGBA → planar YUV 4:2:0 conversion.
///
/// On x86-64 with AVX-512F the vectorised inner kernel is used; otherwise
/// falls back to the pure-scalar implementation.
///
/// # Arguments
/// * `rgba`   – Packed RGBA input (`width * height * 4` bytes, R first).
/// * `width`  – Frame width in pixels (must be even, ≥ 2).
/// * `height` – Frame height in pixels (must be even, ≥ 2).
///
/// # Returns
/// Flat `Vec<u8>` with layout: Y plane, then Cb (U) plane, then Cr (V) plane.
///
/// # Panics
/// Panics if `rgba.len() < width * height * 4`, or if width/height are odd.
#[must_use]
pub fn rgba_to_yuv420_avx512(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    assert!(
        width % 2 == 0 && height % 2 == 0,
        "width and height must be even for 4:2:0 sub-sampling"
    );
    assert!(
        rgba.len() >= (width as usize) * (height as usize) * 4,
        "RGBA buffer too small"
    );

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512f") {
            // SAFETY: avx512f confirmed by runtime detection above.
            return unsafe { rgba_to_yuv420_avx512_inner(rgba, width, height) };
        }
    }

    rgba_to_yuv420_scalar(rgba, width, height)
}

// ── Named alias: dot_product_avx512 ──────────────────────────────────────────

/// Safe runtime-dispatched f32 dot product (named AVX-512 alias).
///
/// Identical to [`dot_product_f32`] but exposed under the name requested by
/// the extended AVX-512 API surface for explicitness.  On x86-64 with AVX-512F
/// the FMA inner kernel is used; otherwise falls back to the scalar path.
#[must_use]
pub fn dot_product_avx512(a: &[f32], b: &[f32]) -> f32 {
    dot_product_f32(a, b)
}

// ── Extended AVX-512 tests ────────────────────────────────────────────────────

#[cfg(test)]
mod avx512_extended_tests {
    use super::*;

    fn approx_eq(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    // ── avx512_available ─────────────────────────────────────────────────────

    #[test]
    fn test_avx512_available_returns_bool() {
        // Just verify it doesn't panic and returns a consistent value.
        let first = avx512_available();
        let second = avx512_available();
        assert_eq!(first, second);
    }

    #[cfg(not(target_arch = "x86_64"))]
    #[test]
    fn test_avx512_not_available_on_non_x86() {
        assert!(!avx512_available());
    }

    // ── dot_product_avx512 ───────────────────────────────────────────────────

    #[test]
    fn test_dot_product_avx512_empty() {
        assert!(approx_eq(dot_product_avx512(&[], &[]), 0.0, 1e-9));
    }

    #[test]
    fn test_dot_product_avx512_single() {
        assert!(approx_eq(dot_product_avx512(&[3.0], &[4.0]), 12.0, 1e-5));
    }

    #[test]
    fn test_dot_product_avx512_basic() {
        let a = [1.0f32, 2.0, 3.0];
        let b = [4.0f32, 5.0, 6.0];
        assert!(approx_eq(dot_product_avx512(&a, &b), 32.0, 1e-4));
    }

    #[test]
    fn test_dot_product_avx512_16_elements() {
        let a = [1.0f32; 16];
        let b = [2.0f32; 16];
        assert!(approx_eq(dot_product_avx512(&a, &b), 32.0, 1e-3));
    }

    #[test]
    fn test_dot_product_avx512_17_elements() {
        let a: Vec<f32> = (0..17).map(|i| i as f32).collect();
        let b = vec![1.0f32; 17];
        // Sum of 0..=16 = 136
        assert!(approx_eq(dot_product_avx512(&a, &b), 136.0, 0.2));
    }

    #[test]
    fn test_dot_product_avx512_matches_scalar() {
        let a: Vec<f32> = (0..33).map(|i| i as f32 * 0.5).collect();
        let b: Vec<f32> = (0..33).map(|i| (33 - i) as f32 * 0.1).collect();
        let scalar = dot_product_f32_scalar(&a, &b);
        let avx = dot_product_avx512(&a, &b);
        assert!(approx_eq(avx, scalar, 0.5));
    }

    #[test]
    fn test_dot_product_avx512_orthogonal() {
        let a = [1.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let b = [0.0f32, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert!(approx_eq(dot_product_avx512(&a, &b), 0.0, 1e-9));
    }

    // ── rgba_to_yuv420_avx512 ────────────────────────────────────────────────

    #[test]
    fn test_yuv420_basic_dimensions() {
        let width = 4u32;
        let height = 4u32;
        let rgba = vec![128u8; (width * height * 4) as usize];
        let yuv = rgba_to_yuv420_avx512(&rgba, width, height);
        let expected_size = (width * height) as usize          // Y
            + (width / 2 * height / 2) as usize    // U
            + (width / 2 * height / 2) as usize; // V
        assert_eq!(yuv.len(), expected_size);
    }

    #[test]
    fn test_yuv420_black_frame() {
        let width = 2u32;
        let height = 2u32;
        let rgba = vec![0u8; (width * height * 4) as usize];
        let yuv = rgba_to_yuv420_avx512(&rgba, width, height);
        // Black → Y = 16 (limited range), U ≈ 128, V ≈ 128
        assert_eq!(yuv[0], 16); // Y plane first pixel
                                // Chroma at Y=black: Cb = Cr = 128 expected
        let y_size = (width * height) as usize;
        let uv_size = (width / 2 * height / 2) as usize;
        assert_eq!(yuv[y_size], 128); // first U
        assert_eq!(yuv[y_size + uv_size], 128); // first V
    }

    #[test]
    fn test_yuv420_white_frame() {
        let width = 2u32;
        let height = 2u32;
        let rgba = vec![255u8; (width * height * 4) as usize];
        let yuv = rgba_to_yuv420_avx512(&rgba, width, height);
        // White → Y ≈ 235 (limited range max)
        assert_eq!(yuv[0], 235);
    }

    #[test]
    fn test_yuv420_y_plane_range() {
        let width = 4u32;
        let height = 4u32;
        let rgba: Vec<u8> = (0..(width * height) as usize)
            .flat_map(|i| {
                let v = ((i * 16) & 0xFF) as u8;
                [v, v / 2, v / 4, 255]
            })
            .collect();
        let yuv = rgba_to_yuv420_avx512(&rgba, width, height);
        let y_size = (width * height) as usize;
        for &y in &yuv[..y_size] {
            assert!(y >= 16 && y <= 235, "Y={y} out of BT.601 limited range");
        }
    }

    #[test]
    fn test_yuv420_avx512_matches_scalar() {
        let width = 4u32;
        let height = 4u32;
        let rgba: Vec<u8> = (0..(width * height * 4) as usize)
            .map(|i| (i * 7 % 256) as u8)
            .collect();
        let avx = rgba_to_yuv420_avx512(&rgba, width, height);
        let scalar = rgba_to_yuv420_scalar(&rgba, width, height);
        assert_eq!(avx.len(), scalar.len());
        // Y plane must match exactly.
        let y_size = (width * height) as usize;
        assert_eq!(&avx[..y_size], &scalar[..y_size]);
        // Chroma may differ by at most 1 due to float rounding in the AVX-512 path.
        for (a, b) in avx[y_size..].iter().zip(scalar[y_size..].iter()) {
            let diff = (*a as i32 - *b as i32).unsigned_abs();
            assert!(diff <= 1, "chroma mismatch a={a} b={b}");
        }
    }

    #[test]
    fn test_yuv420_chroma_subsampling() {
        // Only top-left pixel of each 2×2 block contributes to chroma.
        // Verify the U/V plane dimensions are correct.
        let width = 8u32;
        let height = 6u32;
        let rgba = vec![100u8; (width * height * 4) as usize];
        let yuv = rgba_to_yuv420_avx512(&rgba, width, height);
        let y_size = (width * height) as usize;
        let uv_size = (width / 2 * height / 2) as usize;
        assert_eq!(yuv.len(), y_size + 2 * uv_size);
    }
}
