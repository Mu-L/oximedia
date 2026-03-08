//! AV1 CDEF (Constrained Directional Enhancement Filter) SIMD operations.
//!
//! CDEF is an in-loop filter that reduces ringing artifacts while preserving
//! edges and texture details.

use crate::simd::traits::SimdOps;
use crate::simd::types::{I16x8, U8x16};

/// AV1 CDEF SIMD operations.
pub struct CdefSimd<S> {
    simd: S,
}

impl<S: SimdOps> CdefSimd<S> {
    /// Create a new CDEF SIMD instance.
    #[inline]
    pub const fn new(simd: S) -> Self {
        Self { simd }
    }

    /// Apply CDEF filtering to an 8x8 block.
    ///
    /// # Arguments
    /// * `src` - Source pixels (with border for filtering)
    /// * `dst` - Destination buffer for filtered pixels
    /// * `src_stride` - Stride of source buffer
    /// * `dst_stride` - Stride of destination buffer
    /// * `pri_strength` - Primary filtering strength (0-15)
    /// * `sec_strength` - Secondary filtering strength (0-4)
    /// * `direction` - Filtering direction (0-7)
    /// * `damping` - Damping parameter (0-6)
    #[allow(clippy::too_many_arguments)]
    pub fn filter_block_8x8(
        &self,
        src: &[u8],
        dst: &mut [u8],
        src_stride: usize,
        dst_stride: usize,
        pri_strength: u8,
        sec_strength: u8,
        direction: u8,
        damping: u8,
    ) {
        for y in 0..8 {
            for x in 0..8 {
                let src_offset = y * src_stride + x;
                let dst_offset = y * dst_stride + x;

                if src.len() <= src_offset || dst.len() <= dst_offset {
                    continue;
                }

                let pixel = src[src_offset];
                let filtered = self.filter_pixel(
                    src,
                    src_stride,
                    x,
                    y,
                    pixel,
                    pri_strength,
                    sec_strength,
                    direction,
                    damping,
                );
                dst[dst_offset] = filtered;
            }
        }
    }

    /// Apply CDEF filtering to a 4x4 block.
    #[allow(clippy::too_many_arguments)]
    pub fn filter_block_4x4(
        &self,
        src: &[u8],
        dst: &mut [u8],
        src_stride: usize,
        dst_stride: usize,
        pri_strength: u8,
        sec_strength: u8,
        direction: u8,
        damping: u8,
    ) {
        for y in 0..4 {
            for x in 0..4 {
                let src_offset = y * src_stride + x;
                let dst_offset = y * dst_stride + x;

                if src.len() <= src_offset || dst.len() <= dst_offset {
                    continue;
                }

                let pixel = src[src_offset];
                let filtered = self.filter_pixel(
                    src,
                    src_stride,
                    x,
                    y,
                    pixel,
                    pri_strength,
                    sec_strength,
                    direction,
                    damping,
                );
                dst[dst_offset] = filtered;
            }
        }
    }

    /// Find the best CDEF direction for a block.
    ///
    /// Returns the direction index (0-7) that minimizes variance
    /// along the direction.
    pub fn find_direction(&self, src: &[u8], stride: usize, block_size: usize) -> u8 {
        let mut best_direction = 0u8;
        let mut best_variance = u32::MAX;

        // Try all 8 directions
        for dir in 0..8 {
            let variance = self.calculate_directional_variance(src, stride, block_size, dir);
            if variance < best_variance {
                best_variance = variance;
                best_direction = dir;
            }
        }

        best_direction
    }

    // ========================================================================
    // Internal Filtering Operations
    // ========================================================================

    /// Filter a single pixel using CDEF.
    #[allow(clippy::too_many_arguments)]
    fn filter_pixel(
        &self,
        src: &[u8],
        stride: usize,
        x: usize,
        y: usize,
        pixel: u8,
        pri_strength: u8,
        sec_strength: u8,
        direction: u8,
        damping: u8,
    ) -> u8 {
        if pri_strength == 0 && sec_strength == 0 {
            return pixel;
        }

        // Get directional offsets
        let (dx, dy) = self.get_direction_offset(direction);

        // Calculate primary tap positions
        let pri_taps = [
            (dx, dy),           // Primary direction
            (-dx, -dy),         // Opposite direction
            (dx * 2, dy * 2),   // Extended primary
            (-dx * 2, -dy * 2), // Extended opposite
        ];

        // Calculate secondary tap positions (perpendicular)
        let (sdx, sdy) = (-dy, dx);
        let sec_taps = [
            (sdx, sdy),
            (-sdx, -sdy),
            (sdx * 2, sdy * 2),
            (-sdx * 2, -sdy * 2),
        ];

        // Accumulate filtered value
        let mut sum = i32::from(pixel) << 7; // Scale by 128
        let mut total_weight = 128i32;

        // Apply primary taps
        for &(ox, oy) in &pri_taps {
            let weight =
                self.calculate_weight(src, stride, x, y, ox, oy, pixel, pri_strength, damping);
            sum += weight.0;
            total_weight += weight.1;
        }

        // Apply secondary taps
        for &(ox, oy) in &sec_taps {
            let weight =
                self.calculate_weight(src, stride, x, y, ox, oy, pixel, sec_strength, damping);
            sum += weight.0;
            total_weight += weight.1;
        }

        // Normalize and clamp
        let result = (sum + total_weight / 2) / total_weight;
        result.clamp(0, 255) as u8
    }

    /// Calculate filtering weight for a tap.
    #[allow(clippy::too_many_arguments)]
    fn calculate_weight(
        &self,
        src: &[u8],
        stride: usize,
        x: usize,
        y: usize,
        ox: i32,
        oy: i32,
        pixel: u8,
        strength: u8,
        damping: u8,
    ) -> (i32, i32) {
        let tx = x as i32 + ox;
        let ty = y as i32 + oy;

        if tx < 0 || ty < 0 {
            return (0, 0);
        }

        let offset = ty as usize * stride + tx as usize;
        if offset >= src.len() {
            return (0, 0);
        }

        let tap_pixel = src[offset];
        let diff = i32::from(tap_pixel) - i32::from(pixel);
        let abs_diff = diff.abs();

        // Calculate weight based on difference
        let threshold = 1 << damping;
        if abs_diff >= threshold {
            return (0, 0);
        }

        let weight = i32::from(strength) * (threshold - abs_diff) / threshold;
        let weighted_value = diff * weight;

        (weighted_value, weight)
    }

    /// Get direction offset (dx, dy) for a given direction index.
    fn get_direction_offset(&self, direction: u8) -> (i32, i32) {
        match direction % 8 {
            0 => (1, 0),   // Horizontal
            1 => (1, 1),   // Diagonal ↗
            2 => (0, 1),   // Vertical
            3 => (-1, 1),  // Diagonal ↖
            4 => (-1, 0),  // Horizontal ←
            5 => (-1, -1), // Diagonal ↙
            6 => (0, -1),  // Vertical ↑
            7 => (1, -1),  // Diagonal ↘
            _ => (1, 0),
        }
    }

    /// Calculate variance along a direction for direction finding.
    fn calculate_directional_variance(
        &self,
        src: &[u8],
        stride: usize,
        block_size: usize,
        direction: u8,
    ) -> u32 {
        let (dx, dy) = self.get_direction_offset(direction);
        let mut variance = 0u32;
        let mut count = 0u32;

        for y in 1..block_size.saturating_sub(1) {
            for x in 1..block_size.saturating_sub(1) {
                let offset = y * stride + x;
                if offset >= src.len() {
                    continue;
                }

                let pixel = src[offset];

                // Sample along direction
                let tx = x as i32 + dx;
                let ty = y as i32 + dy;

                if tx >= 0 && ty >= 0 {
                    let tap_offset = ty as usize * stride + tx as usize;
                    if tap_offset < src.len() {
                        let tap_pixel = src[tap_offset];
                        let diff = u32::from(pixel.abs_diff(tap_pixel));
                        variance += diff * diff;
                        count += 1;
                    }
                }
            }
        }

        if count > 0 {
            variance / count
        } else {
            u32::MAX
        }
    }

    /// SIMD-accelerated row filtering (process 8 pixels at once).
    #[allow(dead_code)]
    fn filter_row_simd(
        &self,
        src: &[u8],
        dst: &mut [u8],
        width: usize,
        pri_strength: u8,
        sec_strength: u8,
    ) {
        // Process 8 pixels at a time using SIMD
        let chunks = width / 8;
        for i in 0..chunks {
            let offset = i * 8;
            if offset + 8 > src.len() || offset + 8 > dst.len() {
                continue;
            }

            let mut pixels = U8x16::zero();
            for j in 0..8 {
                pixels[j] = src[offset + j];
            }

            // Convert to i16 for filtering
            let pixels_i16 = self.simd.widen_low_u8_to_i16(pixels);

            // Apply simple smoothing filter
            let strength_vec = I16x8::from_array([i16::from(pri_strength + sec_strength); 8]);
            let filtered = self.simd.add_i16x8(pixels_i16, strength_vec);

            // Convert back to u8
            for j in 0..8 {
                dst[offset + j] = filtered[j].clamp(0, 255) as u8;
            }
        }

        // Handle remaining pixels
        for i in (chunks * 8)..width.min(src.len()).min(dst.len()) {
            dst[i] = src[i];
        }
    }
}
