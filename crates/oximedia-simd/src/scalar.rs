//! Scalar fallback implementations for SIMD operations
//!
//! These implementations are used when SIMD is not available or for testing.
#![allow(clippy::too_many_arguments)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]

use crate::{DctSize, InterpolationFilter, Result};

/// Scalar forward DCT implementation
#[allow(clippy::unnecessary_wraps)]
pub fn forward_dct_scalar(input: &[i16], output: &mut [i16], size: DctSize) -> Result<()> {
    match size {
        DctSize::Dct4x4 => forward_dct_4x4_scalar(input, output),
        DctSize::Dct8x8 => forward_dct_8x8_scalar(input, output),
        DctSize::Dct16x16 => forward_dct_16x16_scalar(input, output),
        DctSize::Dct32x32 => forward_dct_32x32_scalar(input, output),
    }
}

/// Scalar inverse DCT implementation
#[allow(clippy::unnecessary_wraps)]
pub fn inverse_dct_scalar(input: &[i16], output: &mut [i16], size: DctSize) -> Result<()> {
    match size {
        DctSize::Dct4x4 => inverse_dct_4x4_scalar(input, output),
        DctSize::Dct8x8 => inverse_dct_8x8_scalar(input, output),
        DctSize::Dct16x16 => inverse_dct_16x16_scalar(input, output),
        DctSize::Dct32x32 => inverse_dct_32x32_scalar(input, output),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn forward_dct_4x4_scalar(input: &[i16], output: &mut [i16]) -> Result<()> {
    // Simple 4x4 DCT using separable 1D transforms
    let mut temp = [0i16; 16];

    // Transform rows
    for i in 0..4 {
        let i0 = i32::from(input[i * 4]);
        let i1 = i32::from(input[i * 4 + 1]);
        let i2 = i32::from(input[i * 4 + 2]);
        let i3 = i32::from(input[i * 4 + 3]);

        temp[i * 4] = ((i0 + i3 + i1 + i2) >> 1) as i16;
        temp[i * 4 + 1] = ((i0 - i3 + (i1 - i2) * 2) >> 1) as i16;
        temp[i * 4 + 2] = ((i0 + i3 - i1 - i2) >> 1) as i16;
        temp[i * 4 + 3] = ((i0 - i3 - (i1 - i2) * 2) >> 1) as i16;
    }

    // Transform columns
    for i in 0..4 {
        let t0 = i32::from(temp[i]);
        let t1 = i32::from(temp[4 + i]);
        let t2 = i32::from(temp[2 * 4 + i]);
        let t3 = i32::from(temp[3 * 4 + i]);

        output[i] = ((t0 + t3 + t1 + t2) >> 1) as i16;
        output[4 + i] = ((t0 - t3 + (t1 - t2) * 2) >> 1) as i16;
        output[2 * 4 + i] = ((t0 + t3 - t1 - t2) >> 1) as i16;
        output[3 * 4 + i] = ((t0 - t3 - (t1 - t2) * 2) >> 1) as i16;
    }

    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn inverse_dct_4x4_scalar(input: &[i16], output: &mut [i16]) -> Result<()> {
    // Simple 4x4 IDCT using separable 1D transforms
    let mut temp = [0i16; 16];

    // Transform rows
    for i in 0..4 {
        let i0 = i32::from(input[i * 4]);
        let i1 = i32::from(input[i * 4 + 1]);
        let i2 = i32::from(input[i * 4 + 2]);
        let i3 = i32::from(input[i * 4 + 3]);

        temp[i * 4] = (i0 + i1 + i2) as i16;
        temp[i * 4 + 1] = (i0 + i1 - i2 - i3 * 2) as i16;
        temp[i * 4 + 2] = (i0 - i1 - i2 + i3) as i16;
        temp[i * 4 + 3] = (i0 - i1 + i2 - i3) as i16;
    }

    // Transform columns
    for i in 0..4 {
        let t0 = i32::from(temp[i]);
        let t1 = i32::from(temp[4 + i]);
        let t2 = i32::from(temp[2 * 4 + i]);
        let t3 = i32::from(temp[3 * 4 + i]);

        output[i] = ((t0 + t1 + t2 + 32) >> 6) as i16;
        output[4 + i] = ((t0 + t1 - t2 - t3 * 2 + 32) >> 6) as i16;
        output[2 * 4 + i] = ((t0 - t1 - t2 + t3 + 32) >> 6) as i16;
        output[3 * 4 + i] = ((t0 - t1 + t2 - t3 + 32) >> 6) as i16;
    }

    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn forward_dct_8x8_scalar(input: &[i16], output: &mut [i16]) -> Result<()> {
    // Simplified 8x8 DCT (not a complete implementation)
    // In a real codec, this would use the full DCT-II transform
    let mut temp = [0i16; 64];

    // Transform rows
    for i in 0..8 {
        for j in 0..8 {
            let mut sum = 0i32;
            for k in 0..8 {
                sum += i32::from(input[i * 8 + k]);
            }
            temp[i * 8 + j] = (sum >> 3) as i16;
        }
    }

    // Transform columns
    for j in 0..8 {
        for i in 0..8 {
            let mut sum = 0i32;
            for k in 0..8 {
                sum += i32::from(temp[k * 8 + j]);
            }
            output[i * 8 + j] = (sum >> 3) as i16;
        }
    }

    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn inverse_dct_8x8_scalar(input: &[i16], output: &mut [i16]) -> Result<()> {
    // Simplified 8x8 IDCT (not a complete implementation)
    forward_dct_8x8_scalar(input, output) // Placeholder
}

#[allow(clippy::unnecessary_wraps)]
fn forward_dct_16x16_scalar(input: &[i16], output: &mut [i16]) -> Result<()> {
    // Simplified 16x16 DCT
    let mut temp = [0i16; 256];

    for i in 0..16 {
        for j in 0..16 {
            let mut sum = 0i32;
            for k in 0..16 {
                sum += i32::from(input[i * 16 + k]);
            }
            temp[i * 16 + j] = (sum >> 4) as i16;
        }
    }

    for j in 0..16 {
        for i in 0..16 {
            let mut sum = 0i32;
            for k in 0..16 {
                sum += i32::from(temp[k * 16 + j]);
            }
            output[i * 16 + j] = (sum >> 4) as i16;
        }
    }

    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn inverse_dct_16x16_scalar(input: &[i16], output: &mut [i16]) -> Result<()> {
    forward_dct_16x16_scalar(input, output) // Placeholder
}

#[allow(clippy::unnecessary_wraps)]
fn forward_dct_32x32_scalar(input: &[i16], output: &mut [i16]) -> Result<()> {
    // Simplified 32x32 DCT
    let mut temp = [0i16; 1024];

    for i in 0..32 {
        for j in 0..32 {
            let mut sum = 0i32;
            for k in 0..32 {
                sum += i32::from(input[i * 32 + k]);
            }
            temp[i * 32 + j] = (sum >> 5) as i16;
        }
    }

    for j in 0..32 {
        for i in 0..32 {
            let mut sum = 0i32;
            for k in 0..32 {
                sum += i32::from(temp[k * 32 + j]);
            }
            output[i * 32 + j] = (sum >> 5) as i16;
        }
    }

    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn inverse_dct_32x32_scalar(input: &[i16], output: &mut [i16]) -> Result<()> {
    forward_dct_32x32_scalar(input, output) // Placeholder
}

/// Scalar interpolation implementation
pub fn interpolate_scalar(
    src: &[u8],
    dst: &mut [u8],
    src_stride: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
    dx: i32,
    dy: i32,
    filter: InterpolationFilter,
) -> Result<()> {
    match filter {
        InterpolationFilter::Bilinear => {
            interpolate_bilinear_scalar(src, dst, src_stride, dst_stride, width, height, dx, dy)
        }
        InterpolationFilter::Bicubic => {
            interpolate_bicubic_scalar(src, dst, src_stride, dst_stride, width, height, dx, dy)
        }
        InterpolationFilter::EightTap => {
            interpolate_8tap_scalar(src, dst, src_stride, dst_stride, width, height, dx, dy)
        }
    }
}

#[allow(clippy::unnecessary_wraps)]
fn interpolate_bilinear_scalar(
    src: &[u8],
    dst: &mut [u8],
    src_stride: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
    dx: i32,
    dy: i32,
) -> Result<()> {
    let fx = dx & 15;
    let fy = dy & 15;

    for y in 0..height {
        for x in 0..width {
            let src_idx = y * src_stride + x;
            let p00 = i32::from(src[src_idx]);
            let p01 = i32::from(src[src_idx + 1]);
            let p10 = i32::from(src[src_idx + src_stride]);
            let p11 = i32::from(src[src_idx + src_stride + 1]);

            let v0 = p00 * (16 - fx) + p01 * fx;
            let v1 = p10 * (16 - fx) + p11 * fx;
            let v = (v0 * (16 - fy) + v1 * fy + 128) >> 8;

            #[allow(clippy::cast_sign_loss)]
            {
                dst[y * dst_stride + x] = v.clamp(0, 255) as u8;
            }
        }
    }

    Ok(())
}

fn interpolate_bicubic_scalar(
    src: &[u8],
    dst: &mut [u8],
    src_stride: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
    dx: i32,
    dy: i32,
) -> Result<()> {
    // Simplified bicubic (using bilinear as fallback)
    interpolate_bilinear_scalar(src, dst, src_stride, dst_stride, width, height, dx, dy)
}

fn interpolate_8tap_scalar(
    src: &[u8],
    dst: &mut [u8],
    src_stride: usize,
    dst_stride: usize,
    width: usize,
    height: usize,
    dx: i32,
    dy: i32,
) -> Result<()> {
    // Simplified 8-tap (using bilinear as fallback)
    interpolate_bilinear_scalar(src, dst, src_stride, dst_stride, width, height, dx, dy)
}

/// Scalar SAD implementation
#[allow(clippy::unnecessary_wraps)]
pub fn sad_scalar(
    src1: &[u8],
    src2: &[u8],
    stride1: usize,
    stride2: usize,
    width: usize,
    height: usize,
) -> Result<u32> {
    let mut sad = 0u32;

    for y in 0..height {
        for x in 0..width {
            let p1 = i32::from(src1[y * stride1 + x]);
            let p2 = i32::from(src2[y * stride2 + x]);
            sad += (p1 - p2).unsigned_abs();
        }
    }

    Ok(sad)
}
