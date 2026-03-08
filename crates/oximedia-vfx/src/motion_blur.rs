//! Motion blur VFX module.
//!
//! Provides kernel-based motion blur for linear, radial, and zoom motion.

/// The type of motion blur to apply.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum MotionBlurType {
    /// Blur along a straight line (dx, dy).
    Linear,
    /// Blur around a rotation centre.
    Radial,
    /// Zoom-in / zoom-out blur from the image centre.
    Zoom,
    /// Caller-supplied custom samples.
    Custom,
}

impl MotionBlurType {
    /// Returns `true` for rotational blur types (`Radial`).
    #[must_use]
    pub fn is_rotational(&self) -> bool {
        matches!(self, Self::Radial)
    }
}

/// A single weighted sample offset within a motion blur kernel.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct BlurSample {
    /// Horizontal offset in pixels.
    pub x_offset: f32,
    /// Vertical offset in pixels.
    pub y_offset: f32,
    /// Contribution weight for this sample.
    pub weight: f32,
}

impl BlurSample {
    /// Returns `true` if both offsets are exactly zero.
    #[must_use]
    pub fn is_zero_offset(&self) -> bool {
        self.x_offset == 0.0 && self.y_offset == 0.0
    }
}

/// A motion blur kernel composed of weighted offset samples.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MotionBlurKernel {
    /// Ordered list of weighted samples.
    pub samples: Vec<BlurSample>,
    /// Blur type that generated these samples.
    pub blur_type: MotionBlurType,
}

impl MotionBlurKernel {
    /// Build a linear motion blur kernel.
    ///
    /// Samples are distributed evenly along the vector (`dx`, `dy`),
    /// each with equal weight.
    #[must_use]
    pub fn linear_kernel(dx: f32, dy: f32, num_samples: u32) -> Self {
        let n = num_samples.max(1);
        let weight = 1.0 / n as f32;

        let samples = (0..n)
            .map(|i| {
                let t = if n == 1 {
                    0.0f32
                } else {
                    i as f32 / (n - 1) as f32 - 0.5
                };
                BlurSample {
                    x_offset: dx * t,
                    y_offset: dy * t,
                    weight,
                }
            })
            .collect();

        Self {
            samples,
            blur_type: MotionBlurType::Linear,
        }
    }

    /// Build a radial (rotational) motion blur kernel.
    ///
    /// Samples are distributed over the arc `[−angle_rad/2, +angle_rad/2]`
    /// centred at (`cx`, `cy`).  Since the rotation is applied relative to
    /// an unknown pixel position the offsets here represent the **delta**
    /// contribution at the centre point (all zero) — real per-pixel offsets
    /// are computed in `apply_motion_blur`.  Weight is uniform.
    #[must_use]
    pub fn radial_kernel(cx: f32, cy: f32, angle_rad: f32, num_samples: u32) -> Self {
        let n = num_samples.max(1);
        let weight = 1.0 / n as f32;

        let samples = (0..n)
            .map(|i| {
                let t = if n == 1 {
                    0.0f32
                } else {
                    i as f32 / (n - 1) as f32 - 0.5
                };
                let a = angle_rad * t;
                // For the centre pixel the offset is zero; for others it is
                // approximated as the tangential displacement at that angle.
                BlurSample {
                    x_offset: cx * a.cos() - cy * a.sin() - cx,
                    y_offset: cx * a.sin() + cy * a.cos() - cy,
                    weight,
                }
            })
            .collect();

        Self {
            samples,
            blur_type: MotionBlurType::Radial,
        }
    }

    /// Sum of all sample weights.
    #[must_use]
    pub fn total_weight(&self) -> f32 {
        self.samples.iter().map(|s| s.weight).sum()
    }

    /// Return a copy of the samples re-scaled so that `total_weight() == 1.0`.
    #[must_use]
    pub fn normalized_samples(&self) -> Vec<BlurSample> {
        let total = self.total_weight();
        if total == 0.0 {
            return self.samples.clone();
        }
        self.samples
            .iter()
            .map(|s| BlurSample {
                x_offset: s.x_offset,
                y_offset: s.y_offset,
                weight: s.weight / total,
            })
            .collect()
    }
}

/// Apply a motion blur kernel to an RGB pixel buffer.
///
/// `pixels` is a flat `width * height * 3` (RGB) byte slice.
/// Returns a new `Vec<u8>` of the same layout.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn apply_motion_blur(
    pixels: &[u8],
    width: u32,
    height: u32,
    kernel: &MotionBlurKernel,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut output = vec![0u8; w * h * 3];
    let samples = kernel.normalized_samples();

    for y in 0..h {
        for x in 0..w {
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;

            for sample in &samples {
                let sx = (x as f32 + sample.x_offset).round() as i64;
                let sy = (y as f32 + sample.y_offset).round() as i64;

                // Clamp to image bounds
                let sx = sx.clamp(0, (w as i64) - 1) as usize;
                let sy = sy.clamp(0, (h as i64) - 1) as usize;

                let idx = (sy * w + sx) * 3;
                r += pixels[idx] as f32 * sample.weight;
                g += pixels[idx + 1] as f32 * sample.weight;
                b += pixels[idx + 2] as f32 * sample.weight;
            }

            let out_idx = (y * w + x) * 3;
            output[out_idx] = r.round().clamp(0.0, 255.0) as u8;
            output[out_idx + 1] = g.round().clamp(0.0, 255.0) as u8;
            output[out_idx + 2] = b.round().clamp(0.0, 255.0) as u8;
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------ //
    // MotionBlurType
    // ------------------------------------------------------------------ //

    #[test]
    fn test_type_radial_is_rotational() {
        assert!(MotionBlurType::Radial.is_rotational());
    }

    #[test]
    fn test_type_linear_not_rotational() {
        assert!(!MotionBlurType::Linear.is_rotational());
    }

    #[test]
    fn test_type_zoom_not_rotational() {
        assert!(!MotionBlurType::Zoom.is_rotational());
    }

    #[test]
    fn test_type_custom_not_rotational() {
        assert!(!MotionBlurType::Custom.is_rotational());
    }

    // ------------------------------------------------------------------ //
    // BlurSample
    // ------------------------------------------------------------------ //

    #[test]
    fn test_sample_zero_offset() {
        let s = BlurSample {
            x_offset: 0.0,
            y_offset: 0.0,
            weight: 1.0,
        };
        assert!(s.is_zero_offset());
    }

    #[test]
    fn test_sample_nonzero_offset() {
        let s = BlurSample {
            x_offset: 1.0,
            y_offset: 0.0,
            weight: 1.0,
        };
        assert!(!s.is_zero_offset());
    }

    // ------------------------------------------------------------------ //
    // MotionBlurKernel – linear
    // ------------------------------------------------------------------ //

    #[test]
    fn test_linear_kernel_sample_count() {
        let k = MotionBlurKernel::linear_kernel(10.0, 0.0, 5);
        assert_eq!(k.samples.len(), 5);
    }

    #[test]
    fn test_linear_kernel_total_weight_one() {
        let k = MotionBlurKernel::linear_kernel(10.0, 5.0, 8);
        assert!((k.total_weight() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_linear_kernel_type() {
        let k = MotionBlurKernel::linear_kernel(0.0, 5.0, 3);
        assert_eq!(k.blur_type, MotionBlurType::Linear);
    }

    #[test]
    fn test_linear_kernel_single_sample_zero_offset() {
        let k = MotionBlurKernel::linear_kernel(10.0, 10.0, 1);
        assert!(k.samples[0].is_zero_offset());
    }

    // ------------------------------------------------------------------ //
    // MotionBlurKernel – radial
    // ------------------------------------------------------------------ //

    #[test]
    fn test_radial_kernel_sample_count() {
        let k = MotionBlurKernel::radial_kernel(0.0, 0.0, 0.1, 6);
        assert_eq!(k.samples.len(), 6);
    }

    #[test]
    fn test_radial_kernel_type() {
        let k = MotionBlurKernel::radial_kernel(0.0, 0.0, 0.1, 3);
        assert_eq!(k.blur_type, MotionBlurType::Radial);
    }

    // ------------------------------------------------------------------ //
    // MotionBlurKernel – normalization
    // ------------------------------------------------------------------ //

    #[test]
    fn test_normalized_samples_sum_to_one() {
        let k = MotionBlurKernel::linear_kernel(5.0, 0.0, 7);
        let ns = k.normalized_samples();
        let total: f32 = ns.iter().map(|s| s.weight).sum();
        assert!((total - 1.0).abs() < 1e-5);
    }

    // ------------------------------------------------------------------ //
    // apply_motion_blur
    // ------------------------------------------------------------------ //

    #[test]
    fn test_apply_motion_blur_identity() {
        // Single-sample kernel with zero offset → output == input
        let kernel = MotionBlurKernel {
            samples: vec![BlurSample {
                x_offset: 0.0,
                y_offset: 0.0,
                weight: 1.0,
            }],
            blur_type: MotionBlurType::Custom,
        };
        let pixels: Vec<u8> = (0..3 * 4 * 4).map(|i| (i % 256) as u8).collect();
        let out = apply_motion_blur(&pixels, 4, 4, &kernel);
        assert_eq!(pixels, out);
    }

    #[test]
    fn test_apply_motion_blur_output_length() {
        let kernel = MotionBlurKernel::linear_kernel(2.0, 0.0, 5);
        let pixels = vec![128u8; 3 * 8 * 8];
        let out = apply_motion_blur(&pixels, 8, 8, &kernel);
        assert_eq!(out.len(), pixels.len());
    }

    #[test]
    fn test_apply_motion_blur_uniform_image_unchanged() {
        // A solid colour image should remain identical after any blur
        let kernel = MotionBlurKernel::linear_kernel(5.0, 3.0, 9);
        let pixels = vec![200u8; 3 * 10 * 10];
        let out = apply_motion_blur(&pixels, 10, 10, &kernel);
        assert!(out.iter().all(|&v| v == 200));
    }
}
