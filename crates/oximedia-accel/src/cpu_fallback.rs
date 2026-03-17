//! CPU fallback implementations for hardware acceleration operations.

use crate::error::{AccelError, AccelResult};
use crate::traits::{HardwareAccel, ScaleFilter};
use oximedia_core::PixelFormat;
use rayon::prelude::*;

/// CPU-based fallback acceleration backend.
///
/// This backend uses multi-threaded CPU implementations via rayon
/// when GPU acceleration is unavailable.
pub struct CpuAccel;

impl CpuAccel {
    /// Creates a new CPU acceleration backend.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for CpuAccel {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareAccel for CpuAccel {
    fn scale_image(
        &self,
        input: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
        format: PixelFormat,
        filter: ScaleFilter,
    ) -> AccelResult<Vec<u8>> {
        let channels = match format {
            PixelFormat::Rgb24 => 3,
            PixelFormat::Rgba32 => 4,
            PixelFormat::Gray8 => 1,
            _ => {
                return Err(AccelError::InvalidFormat(format!(
                    "Unsupported format: {format:?}"
                )))
            }
        };

        let expected_size = (src_width * src_height * channels) as usize;
        if input.len() != expected_size {
            return Err(AccelError::BufferSizeMismatch {
                expected: expected_size,
                actual: input.len(),
            });
        }

        let output_size = (dst_width * dst_height * channels) as usize;
        let mut output = vec![0u8; output_size];

        match filter {
            ScaleFilter::Nearest => {
                scale_nearest(
                    input,
                    &mut output,
                    src_width,
                    src_height,
                    dst_width,
                    dst_height,
                    channels,
                );
            }
            ScaleFilter::Bilinear => {
                scale_bilinear(
                    input,
                    &mut output,
                    src_width,
                    src_height,
                    dst_width,
                    dst_height,
                    channels,
                );
            }
            ScaleFilter::Bicubic => {
                scale_bicubic(
                    input,
                    &mut output,
                    src_width,
                    src_height,
                    dst_width,
                    dst_height,
                    channels,
                );
            }
            ScaleFilter::Lanczos => {
                scale_lanczos(
                    input,
                    &mut output,
                    src_width,
                    src_height,
                    dst_width,
                    dst_height,
                    channels,
                );
            }
        }

        Ok(output)
    }

    fn convert_color(
        &self,
        input: &[u8],
        width: u32,
        height: u32,
        src_format: PixelFormat,
        dst_format: PixelFormat,
    ) -> AccelResult<Vec<u8>> {
        match (src_format, dst_format) {
            (PixelFormat::Rgb24, PixelFormat::Yuv420p) => rgb_to_yuv420p(input, width, height),
            (PixelFormat::Yuv420p, PixelFormat::Rgb24) => yuv420p_to_rgb(input, width, height),
            _ => Err(AccelError::Unsupported(format!(
                "Color conversion from {src_format:?} to {dst_format:?} not implemented"
            ))),
        }
    }

    fn motion_estimation(
        &self,
        reference: &[u8],
        current: &[u8],
        width: u32,
        height: u32,
        block_size: u32,
    ) -> AccelResult<Vec<(i16, i16)>> {
        let expected_size = (width * height) as usize;
        if reference.len() != expected_size || current.len() != expected_size {
            return Err(AccelError::BufferSizeMismatch {
                expected: expected_size,
                actual: reference.len().min(current.len()),
            });
        }

        Ok(block_motion_estimation(
            reference, current, width, height, block_size,
        ))
    }
}

/// Nearest neighbor scaling (CPU implementation).
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
fn scale_nearest(
    input: &[u8],
    output: &mut [u8],
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    channels: u32,
) {
    output
        .par_chunks_exact_mut((dst_width * channels) as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let src_y = (y as u32 * src_height) / dst_height;
            for x in 0..dst_width {
                let src_x = (x * src_width) / dst_width;
                let src_idx = ((src_y * src_width + src_x) * channels) as usize;
                let dst_idx = (x * channels) as usize;

                let channels_usize = channels as usize;
                row[dst_idx..dst_idx + channels_usize]
                    .copy_from_slice(&input[src_idx..src_idx + channels_usize]);
            }
        });
}

/// Bilinear scaling (CPU implementation).
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_precision_loss)]
fn scale_bilinear(
    input: &[u8],
    output: &mut [u8],
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    channels: u32,
) {
    let x_ratio = (src_width - 1) as f32 / dst_width as f32;
    let y_ratio = (src_height - 1) as f32 / dst_height as f32;

    output
        .par_chunks_exact_mut((dst_width * channels) as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let src_y = y as f32 * y_ratio;
            let y1 = src_y.floor() as u32;
            let y2 = (y1 + 1).min(src_height - 1);
            let y_frac = src_y - y1 as f32;

            for x in 0..dst_width {
                let src_x = x as f32 * x_ratio;
                let x1 = src_x.floor() as u32;
                let x2 = (x1 + 1).min(src_width - 1);
                let x_frac = src_x - x1 as f32;

                let dst_idx = (x * channels) as usize;

                for c in 0..channels as usize {
                    let p11 = f32::from(input[((y1 * src_width + x1) * channels) as usize + c]);
                    let p12 = f32::from(input[((y2 * src_width + x1) * channels) as usize + c]);
                    let p21 = f32::from(input[((y1 * src_width + x2) * channels) as usize + c]);
                    let p22 = f32::from(input[((y2 * src_width + x2) * channels) as usize + c]);

                    let p1 = p11 * (1.0 - x_frac) + p21 * x_frac;
                    let p2 = p12 * (1.0 - x_frac) + p22 * x_frac;
                    let result = p1 * (1.0 - y_frac) + p2 * y_frac;

                    row[dst_idx + c] = result.clamp(0.0, 255.0) as u8;
                }
            }
        });
}

/// Cubic interpolation kernel (Catmull-Rom): weight function for bicubic.
///
/// Uses the standard cubic convolution with a = -0.5 (Catmull-Rom spline).
#[inline]
fn cubic_weight(t: f32) -> f32 {
    let t_abs = t.abs();
    if t_abs <= 1.0 {
        (1.5 * t_abs - 2.5) * t_abs * t_abs + 1.0
    } else if t_abs <= 2.0 {
        ((-0.5 * t_abs + 2.5) * t_abs - 4.0) * t_abs + 2.0
    } else {
        0.0
    }
}

/// Bicubic scaling (CPU implementation) using Catmull-Rom spline.
fn scale_bicubic(
    input: &[u8],
    output: &mut [u8],
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    channels: u32,
) {
    let x_ratio = src_width as f32 / dst_width as f32;
    let y_ratio = src_height as f32 / dst_height as f32;

    output
        .par_chunks_exact_mut((dst_width * channels) as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let src_y = (y as f32 + 0.5) * y_ratio - 0.5;
            let y0 = src_y.floor() as i32;

            for x in 0..dst_width {
                let src_x = (x as f32 + 0.5) * x_ratio - 0.5;
                let x0 = src_x.floor() as i32;
                let dst_idx = (x * channels) as usize;

                for c in 0..channels as usize {
                    let mut sum = 0.0f32;
                    let mut weight_sum = 0.0f32;

                    for ky in -1i32..=2 {
                        let sy = (y0 + ky).clamp(0, src_height as i32 - 1) as u32;
                        let wy = cubic_weight(src_y - (y0 + ky) as f32);

                        for kx in -1i32..=2 {
                            let sx = (x0 + kx).clamp(0, src_width as i32 - 1) as u32;
                            let wx = cubic_weight(src_x - (x0 + kx) as f32);
                            let w = wx * wy;

                            let src_idx = ((sy * src_width + sx) * channels) as usize + c;
                            sum += f32::from(input[src_idx]) * w;
                            weight_sum += w;
                        }
                    }

                    let value = if weight_sum.abs() > 1e-6 {
                        sum / weight_sum
                    } else {
                        sum
                    };
                    row[dst_idx + c] = value.clamp(0.0, 255.0) as u8;
                }
            }
        });
}

/// Lanczos kernel function: sinc(x) * sinc(x/a) windowed.
///
/// Uses a = 3 (Lanczos-3) for high-quality resampling.
#[inline]
fn lanczos_kernel(x: f32, a: f32) -> f32 {
    if x.abs() < 1e-6 {
        return 1.0;
    }
    if x.abs() >= a {
        return 0.0;
    }
    let pi_x = std::f32::consts::PI * x;
    let pi_x_over_a = pi_x / a;
    (pi_x.sin() / pi_x) * (pi_x_over_a.sin() / pi_x_over_a)
}

/// Lanczos-3 scaling (CPU implementation).
///
/// Lanczos-3 uses a 6-tap (radius 3) kernel for the highest quality
/// resampling among standard filters.
fn scale_lanczos(
    input: &[u8],
    output: &mut [u8],
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    channels: u32,
) {
    let a = 3.0f32; // Lanczos-3
    let x_ratio = src_width as f32 / dst_width as f32;
    let y_ratio = src_height as f32 / dst_height as f32;

    output
        .par_chunks_exact_mut((dst_width * channels) as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let src_y = (y as f32 + 0.5) * y_ratio - 0.5;
            let y0 = src_y.floor() as i32;

            for x in 0..dst_width {
                let src_x = (x as f32 + 0.5) * x_ratio - 0.5;
                let x0 = src_x.floor() as i32;
                let dst_idx = (x * channels) as usize;

                for c in 0..channels as usize {
                    let mut sum = 0.0f32;
                    let mut weight_sum = 0.0f32;
                    let radius = a as i32;

                    for ky in (1 - radius)..=radius {
                        let sy = (y0 + ky).clamp(0, src_height as i32 - 1) as u32;
                        let wy = lanczos_kernel(src_y - (y0 + ky) as f32, a);

                        for kx in (1 - radius)..=radius {
                            let sx = (x0 + kx).clamp(0, src_width as i32 - 1) as u32;
                            let wx = lanczos_kernel(src_x - (x0 + kx) as f32, a);
                            let w = wx * wy;

                            let src_idx = ((sy * src_width + sx) * channels) as usize + c;
                            sum += f32::from(input[src_idx]) * w;
                            weight_sum += w;
                        }
                    }

                    let value = if weight_sum.abs() > 1e-6 {
                        sum / weight_sum
                    } else {
                        sum
                    };
                    row[dst_idx + c] = value.clamp(0.0, 255.0) as u8;
                }
            }
        });
}

/// RGB to `YUV420p` conversion (CPU implementation).
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_precision_loss)]
fn rgb_to_yuv420p(input: &[u8], width: u32, height: u32) -> AccelResult<Vec<u8>> {
    let expected_size = (width * height * 3) as usize;
    if input.len() != expected_size {
        return Err(AccelError::BufferSizeMismatch {
            expected: expected_size,
            actual: input.len(),
        });
    }

    let y_size = (width * height) as usize;
    let uv_size = (width * height / 4) as usize;
    let mut output = vec![0u8; y_size + uv_size * 2];

    let (y_plane, uv_planes) = output.split_at_mut(y_size);
    let (u_plane, v_plane) = uv_planes.split_at_mut(uv_size);

    // Convert Y plane
    y_plane.par_iter_mut().enumerate().for_each(|(i, y)| {
        let rgb_idx = i * 3;
        let r = f32::from(input[rgb_idx]);
        let g = f32::from(input[rgb_idx + 1]);
        let b = f32::from(input[rgb_idx + 2]);

        *y = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 255.0) as u8;
    });

    // Convert U and V planes (subsampled 2x2)
    u_plane
        .par_iter_mut()
        .zip(v_plane.par_iter_mut())
        .enumerate()
        .for_each(|(i, (u, v))| {
            let uv_x = (i as u32 % (width / 2)) * 2;
            let uv_y = (i as u32 / (width / 2)) * 2;
            let rgb_idx = ((uv_y * width + uv_x) * 3) as usize;

            let r = f32::from(input[rgb_idx]);
            let g = f32::from(input[rgb_idx + 1]);
            let b = f32::from(input[rgb_idx + 2]);

            *u = (-0.169 * r - 0.331 * g + 0.500 * b + 128.0).clamp(0.0, 255.0) as u8;
            *v = (0.500 * r - 0.419 * g - 0.081 * b + 128.0).clamp(0.0, 255.0) as u8;
        });

    Ok(output)
}

/// `YUV420p` to RGB conversion (CPU implementation).
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_precision_loss)]
fn yuv420p_to_rgb(input: &[u8], width: u32, height: u32) -> AccelResult<Vec<u8>> {
    let y_size = (width * height) as usize;
    let uv_size = (width * height / 4) as usize;
    let expected_size = y_size + uv_size * 2;

    if input.len() != expected_size {
        return Err(AccelError::BufferSizeMismatch {
            expected: expected_size,
            actual: input.len(),
        });
    }

    let y_plane = &input[..y_size];
    let u_plane = &input[y_size..y_size + uv_size];
    let v_plane = &input[y_size + uv_size..];

    let output_size = (width * height * 3) as usize;
    let mut output = vec![0u8; output_size];

    output
        .par_chunks_exact_mut(3)
        .enumerate()
        .for_each(|(i, pixel)| {
            let pixel_x = i as u32 % width;
            let pixel_y = i as u32 / width;
            let uv_idx = ((pixel_y / 2) * (width / 2) + (pixel_x / 2)) as usize;

            let y_val = f32::from(y_plane[i]);
            let u_val = f32::from(u_plane[uv_idx]) - 128.0;
            let v_val = f32::from(v_plane[uv_idx]) - 128.0;

            let red = (y_val + 1.402 * v_val).clamp(0.0, 255.0) as u8;
            let green = (y_val - 0.344 * u_val - 0.714 * v_val).clamp(0.0, 255.0) as u8;
            let blue = (y_val + 1.772 * u_val).clamp(0.0, 255.0) as u8;

            pixel[0] = red;
            pixel[1] = green;
            pixel[2] = blue;
        });

    Ok(output)
}

/// Block-based motion estimation (CPU implementation).
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_possible_wrap)]
fn block_motion_estimation(
    reference: &[u8],
    current: &[u8],
    width: u32,
    height: u32,
    block_size: u32,
) -> Vec<(i16, i16)> {
    let blocks_wide = width.div_ceil(block_size);
    let blocks_high = height.div_ceil(block_size);
    let search_range = 8i32;

    let motion_vectors: Vec<(i16, i16)> = (0..blocks_high)
        .into_par_iter()
        .flat_map(|block_y| {
            (0..blocks_wide).into_par_iter().map(move |block_x| {
                let cur_x = block_x * block_size;
                let cur_y = block_y * block_size;

                let mut best_sad = u32::MAX;
                let mut best_delta_x = 0i16;
                let mut best_delta_y = 0i16;

                for dy in -search_range..=search_range {
                    for dx in -search_range..=search_range {
                        let ref_x = cur_x as i32 + dx;
                        let ref_y = cur_y as i32 + dy;

                        if ref_x < 0
                            || ref_y < 0
                            || ref_x + block_size as i32 > width as i32
                            || ref_y + block_size as i32 > height as i32
                        {
                            continue;
                        }

                        let mut sad = 0u32;
                        for by in 0..block_size {
                            for bx in 0..block_size {
                                #[allow(clippy::cast_sign_loss)]
                                let rx = (ref_x + bx as i32) as u32;
                                #[allow(clippy::cast_sign_loss)]
                                let ry = (ref_y + by as i32) as u32;
                                let cx = cur_x + bx;
                                let cy = cur_y + by;

                                if rx >= width || ry >= height || cx >= width || cy >= height {
                                    continue;
                                }

                                let ref_idx = (ry * width + rx) as usize;
                                let cur_idx = (cy * width + cx) as usize;

                                sad += u32::from(reference[ref_idx].abs_diff(current[cur_idx]));
                            }
                        }

                        if sad < best_sad {
                            best_sad = sad;
                            best_delta_x = dx as i16;
                            best_delta_y = dy as i16;
                        }
                    }
                }

                (best_delta_x, best_delta_y)
            })
        })
        .collect();

    motion_vectors
}
