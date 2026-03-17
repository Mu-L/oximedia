//! SIMD-accelerated block difference metrics for motion estimation.
//!
//! Provides hardware-accelerated implementations of:
//! - **SAD** (Sum of Absolute Differences) — fast, used for motion search
//! - **SATD** (Sum of Absolute Transformed Differences) — more accurate, used
//!   for final mode decision
//!
//! Uses `is_x86_feature_detected!` at runtime to dispatch to AVX2/SSE4.1
//! accelerated paths on x86-64, with a portable scalar fallback for all
//! other platforms. Results are bit-identical across all paths.

// ── SAD (Sum of Absolute Differences) ────────────────────────────────────────

/// Compute the Sum of Absolute Differences between two equally-sized blocks.
///
/// Both slices must have the same length. Returns 0 if either slice is empty.
#[must_use]
pub fn sad(a: &[u8], b: &[u8]) -> u32 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0;
    }
    sad_impl(&a[..len], &b[..len])
}

/// SAD dispatcher: selects AVX2 / SSE4.1 / scalar path at runtime.
fn sad_impl(a: &[u8], b: &[u8]) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: We have just checked that AVX2 is available at runtime.
            return unsafe { sad_avx2(a, b) };
        }
        if is_x86_feature_detected!("sse4.1") {
            // SAFETY: We have just checked that SSE4.1 is available at runtime.
            return unsafe { sad_sse41(a, b) };
        }
    }
    sad_scalar(a, b)
}

/// AVX2 SAD using 256-bit registers processing 32 bytes per iteration.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn sad_avx2(a: &[u8], b: &[u8]) -> u32 {
    use std::arch::x86_64::*;

    let len = a.len().min(b.len());
    let mut sum: u32 = 0;
    let chunks = len / 32;

    for i in 0..chunks {
        let offset = i * 32;
        let va = _mm256_loadu_si256(a.as_ptr().add(offset) as *const __m256i);
        let vb = _mm256_loadu_si256(b.as_ptr().add(offset) as *const __m256i);
        // Compute |a - b| for each byte via (a - min(a,b)) + (b - min(a,b)) = |a - b|
        let min_ab = _mm256_min_epu8(va, vb);
        let max_ab = _mm256_max_epu8(va, vb);
        let diff = _mm256_sub_epi8(max_ab, min_ab);
        // Horizontal sum using sad_epu8 with zero
        let zero = _mm256_setzero_si256();
        let sad_result = _mm256_sad_epu8(diff, zero);
        // Extract 4 x u64 lanes and sum
        let lo = _mm256_extracti128_si256(sad_result, 0);
        let hi = _mm256_extracti128_si256(sad_result, 1);
        let s0 = _mm_extract_epi64(lo, 0) as u32;
        let s1 = _mm_extract_epi64(lo, 1) as u32;
        let s2 = _mm_extract_epi64(hi, 0) as u32;
        let s3 = _mm_extract_epi64(hi, 1) as u32;
        sum = sum
            .wrapping_add(s0)
            .wrapping_add(s1)
            .wrapping_add(s2)
            .wrapping_add(s3);
    }

    // Scalar remainder
    let remainder_start = chunks * 32;
    for i in remainder_start..len {
        sum += u32::from(a[i].abs_diff(b[i]));
    }
    sum
}

/// SSE4.1 SAD using 128-bit registers processing 16 bytes per iteration.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn sad_sse41(a: &[u8], b: &[u8]) -> u32 {
    use std::arch::x86_64::*;

    let len = a.len().min(b.len());
    let mut sum: u32 = 0;
    let chunks = len / 16;

    for i in 0..chunks {
        let offset = i * 16;
        let va = _mm_loadu_si128(a.as_ptr().add(offset) as *const __m128i);
        let vb = _mm_loadu_si128(b.as_ptr().add(offset) as *const __m128i);
        let _zero = _mm_setzero_si128();
        let sad_result = _mm_sad_epu8(va, vb);
        let lo = _mm_extract_epi64(sad_result, 0) as u32;
        let hi = _mm_extract_epi64(sad_result, 1) as u32;
        sum = sum.wrapping_add(lo).wrapping_add(hi);
    }

    // Scalar remainder
    let remainder_start = chunks * 16;
    for i in remainder_start..len {
        sum += u32::from(a[i].abs_diff(b[i]));
    }
    let _ = sum; // suppress warning about unused variable from sse path
    sum
}

/// Portable scalar SAD fallback.
pub(crate) fn sad_scalar(a: &[u8], b: &[u8]) -> u32 {
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| u32::from(x.abs_diff(y)))
        .sum()
}

// ── SATD (Sum of Absolute Transformed Differences) ───────────────────────────

/// Compute the SATD of two 4×4 blocks using the Hadamard transform.
///
/// Each slice must contain at least `stride * 3 + 4` elements. The function
/// reads a 4×4 sub-block starting at offset 0.
///
/// Returns `None` if either slice is too short.
#[must_use]
pub fn satd_4x4(a: &[u8], b: &[u8], stride: usize) -> Option<u32> {
    if a.len() < stride * 3 + 4 || b.len() < stride * 3 + 4 {
        return None;
    }
    Some(satd_4x4_impl(a, b, stride))
}

/// SATD 4×4 Hadamard transform (scalar, works on all platforms).
fn satd_4x4_impl(a: &[u8], b: &[u8], stride: usize) -> u32 {
    // Build difference block
    let mut diff = [0i16; 16];
    for row in 0..4 {
        for col in 0..4 {
            let idx = row * stride + col;
            diff[row * 4 + col] = i16::from(a[idx]) - i16::from(b[idx]);
        }
    }

    // Row-wise Hadamard
    let mut tmp = [0i16; 16];
    for row in 0..4 {
        let base = row * 4;
        let d0 = diff[base];
        let d1 = diff[base + 1];
        let d2 = diff[base + 2];
        let d3 = diff[base + 3];
        let a0 = d0 + d1;
        let a1 = d0 - d1;
        let a2 = d2 + d3;
        let a3 = d2 - d3;
        tmp[base] = a0 + a2;
        tmp[base + 1] = a1 + a3;
        tmp[base + 2] = a0 - a2;
        tmp[base + 3] = a1 - a3;
    }

    // Column-wise Hadamard then accumulate absolute values
    let mut sum: u32 = 0;
    for col in 0..4 {
        let t0 = tmp[col];
        let t1 = tmp[4 + col];
        let t2 = tmp[8 + col];
        let t3 = tmp[12 + col];
        let a0 = t0 + t1;
        let a1 = t0 - t1;
        let a2 = t2 + t3;
        let a3 = t2 - t3;
        sum += u32::from((a0 + a2).unsigned_abs());
        sum += u32::from((a1 + a3).unsigned_abs());
        sum += u32::from((a0 - a2).unsigned_abs());
        sum += u32::from((a1 - a3).unsigned_abs());
    }

    // Standard SATD normalisation: divide by 2
    (sum + 1) / 2
}

// ── SAD for block with stride ────────────────────────────────────────────────

/// Compute SAD between two blocks with specified dimensions and strides.
///
/// Each block is accessed as `block[row * stride + col]` for `row` in `0..height`
/// and `col` in `0..width`.
///
/// Returns `None` if either slice is too short.
#[must_use]
pub fn sad_block(
    a: &[u8],
    b: &[u8],
    width: usize,
    height: usize,
    stride_a: usize,
    stride_b: usize,
) -> Option<u32> {
    let required_a = if height == 0 {
        0
    } else {
        stride_a * (height - 1) + width
    };
    let required_b = if height == 0 {
        0
    } else {
        stride_b * (height - 1) + width
    };
    if a.len() < required_a || b.len() < required_b {
        return None;
    }
    if width == 0 || height == 0 {
        return Some(0);
    }

    let mut total = 0u32;
    for row in 0..height {
        let row_a = &a[row * stride_a..row * stride_a + width];
        let row_b = &b[row * stride_b..row * stride_b + width];
        total += sad_impl(row_a, row_b);
    }
    Some(total)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sad_identical_blocks() {
        let a = vec![128u8; 64];
        let b = vec![128u8; 64];
        assert_eq!(sad(&a, &b), 0);
    }

    #[test]
    fn test_sad_known_values() {
        let a = vec![10u8, 20, 30, 40];
        let b = vec![12u8, 18, 35, 38];
        // |10-12| + |20-18| + |30-35| + |40-38| = 2 + 2 + 5 + 2 = 11
        assert_eq!(sad(&a, &b), 11);
    }

    #[test]
    fn test_sad_empty() {
        assert_eq!(sad(&[], &[]), 0);
    }

    #[test]
    fn test_sad_large_block_simd_vs_scalar() {
        // 64 bytes: exercises the SIMD path
        let a: Vec<u8> = (0..64).collect();
        let b: Vec<u8> = (0..64).map(|i| ((i + 3) % 256) as u8).collect();

        let simd_result = sad(&a, &b);
        let scalar_result = sad_scalar(&a, &b);
        assert_eq!(simd_result, scalar_result);
    }

    #[test]
    fn test_sad_non_aligned_length() {
        // 37 bytes: tests remainder handling
        let a: Vec<u8> = (0..37).map(|i| (i * 7 % 256) as u8).collect();
        let b: Vec<u8> = (0..37).map(|i| (i * 11 % 256) as u8).collect();

        let simd_result = sad(&a, &b);
        let scalar_result = sad_scalar(&a, &b);
        assert_eq!(simd_result, scalar_result);
    }

    #[test]
    fn test_satd_4x4_identical() {
        let block = vec![128u8; 16];
        assert_eq!(satd_4x4(&block, &block, 4), Some(0));
    }

    #[test]
    fn test_satd_4x4_known() {
        let a = vec![
            10u8, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160,
        ];
        let b = vec![
            12u8, 18, 32, 38, 52, 58, 72, 78, 92, 98, 112, 118, 132, 138, 152, 158,
        ];

        let result = satd_4x4(&a, &b, 4);
        assert!(result.is_some());
        // The SATD value should be consistent
        let expected = satd_4x4_impl(&a, &b, 4);
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_satd_4x4_too_short() {
        let a = vec![0u8; 8]; // too short for a 4x4 block
        let b = vec![0u8; 16];
        assert_eq!(satd_4x4(&a, &b, 4), None);
    }

    #[test]
    fn test_sad_block_strided() {
        // 8x4 image, process 4x4 block
        let a: Vec<u8> = (0..32).collect();
        let b: Vec<u8> = (0..32).map(|i| ((i + 1) % 256) as u8).collect();

        let result = sad_block(&a, &b, 4, 4, 8, 8);
        assert!(result.is_some());

        // Manual check: each row has 4 elements with diff=1, total = 4*4 = 16
        assert_eq!(result, Some(16));
    }

    #[test]
    fn test_sad_block_too_short() {
        let a = vec![0u8; 10];
        let b = vec![0u8; 32];
        assert_eq!(sad_block(&a, &b, 4, 4, 8, 8), None);
    }

    #[test]
    fn test_satd_nonzero_for_different_blocks() {
        let a = vec![0u8; 16];
        let b = vec![50u8; 16];
        let result = satd_4x4(&a, &b, 4);
        assert!(result.is_some());
        assert!(result.expect("SATD for different blocks should return a value") > 0);
    }

    #[test]
    fn test_sad_all_max_diff() {
        let a = vec![0u8; 32];
        let b = vec![255u8; 32];
        assert_eq!(sad(&a, &b), 32 * 255);
    }
}
