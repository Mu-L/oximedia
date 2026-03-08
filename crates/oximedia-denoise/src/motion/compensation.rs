//! Motion compensation for frame alignment.
//!
//! Motion compensation uses estimated motion vectors to align frames,
//! enabling better temporal filtering and prediction.

use crate::{DenoiseError, DenoiseResult};
use oximedia_codec::VideoFrame;
use rayon::prelude::*;

/// Apply motion compensation to align a frame using motion vectors.
///
/// # Arguments
/// * `reference` - Reference frame to warp
/// * `motion_vectors` - Motion vectors for each block
/// * `block_size` - Size of blocks used in motion estimation
///
/// # Returns
/// Motion-compensated frame
pub fn motion_compensate(
    reference: &VideoFrame,
    motion_vectors: &[(i16, i16)],
    block_size: usize,
) -> DenoiseResult<VideoFrame> {
    if reference.planes.is_empty() {
        return Err(DenoiseError::ProcessingError(
            "Frame has no planes".to_string(),
        ));
    }

    let mut output = reference.clone();

    // Process each plane
    for (plane_idx, plane) in output.planes.iter_mut().enumerate() {
        let reference_plane = &reference.planes[plane_idx];
        let (width, height) = reference.plane_dimensions(plane_idx);

        let mut compensated = plane.data.clone();

        motion_compensate_plane(
            reference_plane.data.as_ref(),
            &mut compensated,
            motion_vectors,
            width as usize,
            height as usize,
            plane.stride,
            block_size,
        )?;

        // Update plane data
        plane.data = compensated;
    }

    Ok(output)
}

/// Apply motion compensation to a single plane.
#[allow(clippy::too_many_arguments)]
fn motion_compensate_plane(
    reference: &[u8],
    output: &mut [u8],
    motion_vectors: &[(i16, i16)],
    width: usize,
    height: usize,
    stride: usize,
    block_size: usize,
) -> DenoiseResult<()> {
    let num_blocks_x = width.div_ceil(block_size);

    for by in 0..(height / block_size) {
        for bx in 0..(width / block_size) {
            let block_idx = by * num_blocks_x + bx;

            if block_idx >= motion_vectors.len() {
                continue;
            }

            let (mv_x, mv_y) = motion_vectors[block_idx];

            // Copy block with motion compensation
            for y in 0..block_size {
                let py = by * block_size + y;
                if py >= height {
                    break;
                }

                for x in 0..block_size {
                    let px = bx * block_size + x;
                    if px >= width {
                        break;
                    }

                    // Calculate source position with motion vector
                    let src_x = (px as i32 + i32::from(mv_x)).clamp(0, (width - 1) as i32) as usize;
                    let src_y =
                        (py as i32 + i32::from(mv_y)).clamp(0, (height - 1) as i32) as usize;

                    output[py * stride + px] = reference[src_y * stride + src_x];
                }
            }
        }
    }

    Ok(())
}

/// Bidirectional motion compensation using two reference frames.
pub fn bidirectional_motion_compensate(
    reference1: &VideoFrame,
    reference2: &VideoFrame,
    motion_vectors1: &[(i16, i16)],
    motion_vectors2: &[(i16, i16)],
    block_size: usize,
) -> DenoiseResult<VideoFrame> {
    // Compensate both references
    let comp1 = motion_compensate(reference1, motion_vectors1, block_size)?;
    let comp2 = motion_compensate(reference2, motion_vectors2, block_size)?;

    // Average the two compensated frames
    let mut output = comp1.clone();

    output
        .planes
        .par_iter_mut()
        .enumerate()
        .for_each(|(plane_idx, plane)| {
            let plane1 = &comp1.planes[plane_idx];
            let plane2 = &comp2.planes[plane_idx];
            let (width, height) = comp1.plane_dimensions(plane_idx);

            let mut data = plane.data.clone();

            for y in 0..(height as usize) {
                for x in 0..(width as usize) {
                    let idx = y * plane.stride + x;
                    let val1 = u16::from(plane1.data[idx]);
                    let val2 = u16::from(plane2.data[idx]);
                    data[idx] = ((val1 + val2) / 2) as u8;
                }
            }

            plane.data = data;
        });

    Ok(output)
}

/// Weighted motion compensation with confidence values.
pub fn weighted_motion_compensate(
    reference: &VideoFrame,
    motion_vectors: &[(i16, i16)],
    weights: &[f32],
    block_size: usize,
) -> DenoiseResult<VideoFrame> {
    if reference.planes.is_empty() {
        return Err(DenoiseError::ProcessingError(
            "Frame has no planes".to_string(),
        ));
    }

    let mut output = reference.clone();

    for (plane_idx, plane) in output.planes.iter_mut().enumerate() {
        let reference_plane = &reference.planes[plane_idx];
        let (width, height) = reference.plane_dimensions(plane_idx);

        let mut compensated = plane.data.clone();

        weighted_motion_compensate_plane(
            reference_plane.data.as_ref(),
            &mut compensated,
            motion_vectors,
            weights,
            width as usize,
            height as usize,
            plane.stride,
            block_size,
        )?;

        plane.data = compensated;
    }

    Ok(output)
}

/// Apply weighted motion compensation to a single plane.
#[allow(clippy::too_many_arguments)]
fn weighted_motion_compensate_plane(
    reference: &[u8],
    output: &mut [u8],
    motion_vectors: &[(i16, i16)],
    weights: &[f32],
    width: usize,
    height: usize,
    stride: usize,
    block_size: usize,
) -> DenoiseResult<()> {
    let num_blocks_x = width.div_ceil(block_size);

    for by in 0..(height / block_size) {
        for bx in 0..(width / block_size) {
            let block_idx = by * num_blocks_x + bx;

            if block_idx >= motion_vectors.len() {
                continue;
            }

            let (mv_x, mv_y) = motion_vectors[block_idx];
            let weight = if block_idx < weights.len() {
                weights[block_idx].clamp(0.0, 1.0)
            } else {
                1.0
            };

            // Copy block with weighted motion compensation
            for y in 0..block_size {
                let py = by * block_size + y;
                if py >= height {
                    break;
                }

                for x in 0..block_size {
                    let px = bx * block_size + x;
                    if px >= width {
                        break;
                    }

                    let original = f32::from(reference[py * stride + px]);

                    let src_x = (px as i32 + i32::from(mv_x)).clamp(0, (width - 1) as i32) as usize;
                    let src_y =
                        (py as i32 + i32::from(mv_y)).clamp(0, (height - 1) as i32) as usize;
                    let compensated = f32::from(reference[src_y * stride + src_x]);

                    // Blend based on weight
                    let blended = (1.0 - weight) * original + weight * compensated;
                    output[py * stride + px] = blended.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oximedia_core::PixelFormat;

    #[test]
    fn test_motion_compensate() {
        let mut reference = VideoFrame::new(PixelFormat::Yuv420p, 64, 64);
        reference.allocate();

        let motion_vectors = vec![(0, 0); 16]; // 4x4 blocks for 64x64 with block_size=16

        let result = motion_compensate(&reference, &motion_vectors, 16);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bidirectional_motion_compensate() {
        let mut ref1 = VideoFrame::new(PixelFormat::Yuv420p, 64, 64);
        ref1.allocate();

        let mut ref2 = VideoFrame::new(PixelFormat::Yuv420p, 64, 64);
        ref2.allocate();

        let mv1 = vec![(1, 0); 16];
        let mv2 = vec![(-1, 0); 16];

        let result = bidirectional_motion_compensate(&ref1, &ref2, &mv1, &mv2, 16);
        assert!(result.is_ok());
    }

    #[test]
    fn test_weighted_motion_compensate() {
        let mut reference = VideoFrame::new(PixelFormat::Yuv420p, 64, 64);
        reference.allocate();

        let motion_vectors = vec![(2, 2); 16];
        let weights = vec![0.5; 16];

        let result = weighted_motion_compensate(&reference, &motion_vectors, &weights, 16);
        assert!(result.is_ok());
    }
}
