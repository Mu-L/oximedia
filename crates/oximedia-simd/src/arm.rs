//! ARM NEON assembly FFI wrappers and safety guarantees
//!
//! This module provides safe Rust wrappers around hand-written NEON
//! assembly implementations for `AArch64`.
#![allow(clippy::too_many_arguments)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]

use crate::{BlockSize, DctSize, InterpolationFilter, Result, SimdError};

// External NEON assembly functions
extern "C" {
    // NEON DCT functions
    fn neon_forward_dct_4x4(input: *const i16, output: *mut i16);
    fn neon_forward_dct_8x8(input: *const i16, output: *mut i16);
    fn neon_forward_dct_16x16(input: *const i16, output: *mut i16);

    fn neon_inverse_dct_4x4(input: *const i16, output: *mut i16);
    fn neon_inverse_dct_8x8(input: *const i16, output: *mut i16);
    fn neon_inverse_dct_16x16(input: *const i16, output: *mut i16);

    // NEON interpolation functions
    fn neon_interpolate_bilinear(
        src: *const u8,
        dst: *mut u8,
        src_stride: i32,
        dst_stride: i32,
        width: i32,
        height: i32,
        dx: i32,
        dy: i32,
    );

    fn neon_interpolate_bicubic(
        src: *const u8,
        dst: *mut u8,
        src_stride: i32,
        dst_stride: i32,
        width: i32,
        height: i32,
        dx: i32,
        dy: i32,
    );

    fn neon_interpolate_8tap(
        src: *const u8,
        dst: *mut u8,
        src_stride: i32,
        dst_stride: i32,
        width: i32,
        height: i32,
        dx: i32,
        dy: i32,
    );

    // NEON SAD functions
    fn neon_sad_16x16(src1: *const u8, src2: *const u8, stride1: i32, stride2: i32) -> u32;
    fn neon_sad_32x32(src1: *const u8, src2: *const u8, stride1: i32, stride2: i32) -> u32;
    fn neon_sad_64x64(src1: *const u8, src2: *const u8, stride1: i32, stride2: i32) -> u32;
}

/// Check alignment for NEON operations (16-byte)
fn check_neon_alignment(ptr: *const u8) -> Result<()> {
    if !(ptr as usize).is_multiple_of(16) {
        return Err(SimdError::InvalidAlignment);
    }
    Ok(())
}

/// Safe wrapper for NEON forward DCT
pub fn forward_dct_neon(input: &[i16], output: &mut [i16], size: DctSize) -> Result<()> {
    match size {
        DctSize::Dct4x4 => {
            if input.len() < 16 || output.len() < 16 {
                return Err(SimdError::InvalidBufferSize);
            }
            unsafe {
                neon_forward_dct_4x4(input.as_ptr(), output.as_mut_ptr());
            }
        }
        DctSize::Dct8x8 => {
            if input.len() < 64 || output.len() < 64 {
                return Err(SimdError::InvalidBufferSize);
            }
            unsafe {
                neon_forward_dct_8x8(input.as_ptr(), output.as_mut_ptr());
            }
        }
        DctSize::Dct16x16 => {
            if input.len() < 256 || output.len() < 256 {
                return Err(SimdError::InvalidBufferSize);
            }
            unsafe {
                neon_forward_dct_16x16(input.as_ptr(), output.as_mut_ptr());
            }
        }
        DctSize::Dct32x32 => {
            // NEON implementation for 32x32 would use the 16x16 in a hierarchical way
            // or fall back to scalar
            return Err(SimdError::UnsupportedOperation);
        }
    }

    Ok(())
}

/// Safe wrapper for NEON inverse DCT
pub fn inverse_dct_neon(input: &[i16], output: &mut [i16], size: DctSize) -> Result<()> {
    match size {
        DctSize::Dct4x4 => {
            if input.len() < 16 || output.len() < 16 {
                return Err(SimdError::InvalidBufferSize);
            }
            unsafe {
                neon_inverse_dct_4x4(input.as_ptr(), output.as_mut_ptr());
            }
        }
        DctSize::Dct8x8 => {
            if input.len() < 64 || output.len() < 64 {
                return Err(SimdError::InvalidBufferSize);
            }
            unsafe {
                neon_inverse_dct_8x8(input.as_ptr(), output.as_mut_ptr());
            }
        }
        DctSize::Dct16x16 => {
            if input.len() < 256 || output.len() < 256 {
                return Err(SimdError::InvalidBufferSize);
            }
            unsafe {
                neon_inverse_dct_16x16(input.as_ptr(), output.as_mut_ptr());
            }
        }
        DctSize::Dct32x32 => {
            return Err(SimdError::UnsupportedOperation);
        }
    }

    Ok(())
}

/// Safe wrapper for NEON interpolation
pub fn interpolate_neon(
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
    // Validate buffer sizes
    if src.len() < (height + 8) * src_stride {
        return Err(SimdError::InvalidBufferSize);
    }
    if dst.len() < height * dst_stride {
        return Err(SimdError::InvalidBufferSize);
    }

    unsafe {
        match filter {
            InterpolationFilter::Bilinear => {
                neon_interpolate_bilinear(
                    src.as_ptr(),
                    dst.as_mut_ptr(),
                    src_stride as i32,
                    dst_stride as i32,
                    width as i32,
                    height as i32,
                    dx,
                    dy,
                );
            }
            InterpolationFilter::Bicubic => {
                neon_interpolate_bicubic(
                    src.as_ptr(),
                    dst.as_mut_ptr(),
                    src_stride as i32,
                    dst_stride as i32,
                    width as i32,
                    height as i32,
                    dx,
                    dy,
                );
            }
            InterpolationFilter::EightTap => {
                neon_interpolate_8tap(
                    src.as_ptr(),
                    dst.as_mut_ptr(),
                    src_stride as i32,
                    dst_stride as i32,
                    width as i32,
                    height as i32,
                    dx,
                    dy,
                );
            }
        }
    }

    Ok(())
}

/// Safe wrapper for NEON SAD
pub fn sad_neon(
    src1: &[u8],
    src2: &[u8],
    stride1: usize,
    stride2: usize,
    size: BlockSize,
) -> Result<u32> {
    let (_width, height) = match size {
        BlockSize::Block16x16 => (16, 16),
        BlockSize::Block32x32 => (32, 32),
        BlockSize::Block64x64 => (64, 64),
    };

    if src1.len() < height * stride1 || src2.len() < height * stride2 {
        return Err(SimdError::InvalidBufferSize);
    }

    let result = unsafe {
        match size {
            BlockSize::Block16x16 => {
                neon_sad_16x16(src1.as_ptr(), src2.as_ptr(), stride1 as i32, stride2 as i32)
            }
            BlockSize::Block32x32 => {
                neon_sad_32x32(src1.as_ptr(), src2.as_ptr(), stride1 as i32, stride2 as i32)
            }
            BlockSize::Block64x64 => {
                neon_sad_64x64(src1.as_ptr(), src2.as_ptr(), stride1 as i32, stride2 as i32)
            }
        }
    };

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neon_alignment() {
        let _aligned: Vec<u8> = vec![0; 64];
        // Alignment tests
    }

    #[test]
    fn test_buffer_validation() {
        let small_buf = [0i16; 8];
        let mut out_buf = [0i16; 8];
        let result = forward_dct_neon(&small_buf, &mut out_buf, DctSize::Dct8x8);
        assert!(result.is_err());
    }
}
