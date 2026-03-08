//! 2D convolution kernels and filters for image processing.
//!
//! Provides kernel definitions (sharpen, edge-detect, blur, etc.) and a
//! `ConvolutionFilter` that applies them to pixel buffers.

#![allow(dead_code)]

/// Standard kernel sizes for convolution operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelSize {
    /// 3×3 kernel.
    K3x3,
    /// 5×5 kernel.
    K5x5,
    /// 7×7 kernel.
    K7x7,
}

impl KernelSize {
    /// Returns the side length of the kernel.
    #[must_use]
    pub const fn side(self) -> usize {
        match self {
            Self::K3x3 => 3,
            Self::K5x5 => 5,
            Self::K7x7 => 7,
        }
    }

    /// Returns the total number of elements in the kernel.
    #[must_use]
    pub const fn area(self) -> usize {
        let s = self.side();
        s * s
    }
}

/// A 2-dimensional convolution kernel stored in row-major order.
#[derive(Debug, Clone)]
pub struct Kernel2d {
    /// The kernel size category.
    pub size: KernelSize,
    /// Kernel weights in row-major order (length == size.area()).
    pub weights: Vec<f32>,
    /// Divisor applied after accumulation (normalisation factor).
    pub divisor: f32,
}

impl Kernel2d {
    /// Creates a new kernel, panics if `weights.len() != size.area()`.
    #[must_use]
    pub fn new(size: KernelSize, weights: Vec<f32>, divisor: f32) -> Self {
        assert_eq!(
            weights.len(),
            size.area(),
            "weights length must equal kernel area"
        );
        Self {
            size,
            weights,
            divisor,
        }
    }

    /// Applies this kernel to a single pixel at `(cx, cy)` in a row-major
    /// float buffer of dimensions `width × height`.  Out-of-bounds positions
    /// are treated as 0.0 (zero-padding).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn apply_to_pixel(
        &self,
        buf: &[f32],
        width: usize,
        height: usize,
        cx: usize,
        cy: usize,
    ) -> f32 {
        let half = (self.size.side() / 2) as isize;
        let mut acc = 0.0_f32;
        let mut wi = 0usize;
        for ky in -half..=half {
            for kx in -half..=half {
                let sx = cx as isize + kx;
                let sy = cy as isize + ky;
                let val = if sx >= 0 && sy >= 0 && (sx as usize) < width && (sy as usize) < height {
                    buf[sy as usize * width + sx as usize]
                } else {
                    0.0
                };
                acc += val * self.weights[wi];
                wi += 1;
            }
        }
        if self.divisor.abs() > f32::EPSILON {
            acc / self.divisor
        } else {
            acc
        }
    }

    /// Returns the sum of all kernel weights (useful for validation).
    #[must_use]
    pub fn weight_sum(&self) -> f32 {
        self.weights.iter().sum()
    }
}

/// Returns a 3×3 unsharp-masking / sharpening kernel.
#[must_use]
pub fn sharpen_kernel() -> Kernel2d {
    Kernel2d::new(
        KernelSize::K3x3,
        vec![0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0],
        1.0,
    )
}

/// Returns a 3×3 Laplacian edge-detection kernel.
#[must_use]
pub fn edge_detect_kernel() -> Kernel2d {
    Kernel2d::new(
        KernelSize::K3x3,
        vec![-1.0, -1.0, -1.0, -1.0, 8.0, -1.0, -1.0, -1.0, -1.0],
        1.0,
    )
}

/// Returns a 3×3 box-blur kernel.
#[must_use]
pub fn box_blur_kernel() -> Kernel2d {
    Kernel2d::new(KernelSize::K3x3, vec![1.0; 9], 9.0)
}

/// Returns a 3×3 Gaussian blur kernel (approximate σ ≈ 0.85).
#[must_use]
pub fn gaussian_kernel_3x3() -> Kernel2d {
    Kernel2d::new(
        KernelSize::K3x3,
        vec![1.0, 2.0, 1.0, 2.0, 4.0, 2.0, 1.0, 2.0, 1.0],
        16.0,
    )
}

/// Border handling policy when the kernel extends past image edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderMode {
    /// Pixels outside the image are treated as 0.0.
    Zero,
    /// Pixels outside the image clamp to the nearest edge pixel.
    Clamp,
    /// Pixels outside the image are reflected (mirror padding).
    Reflect,
}

/// Applies a `Kernel2d` to a full single-channel floating-point image.
#[derive(Debug, Clone)]
pub struct ConvolutionFilter {
    kernel: Kernel2d,
    border: BorderMode,
}

impl ConvolutionFilter {
    /// Creates a new filter with the given kernel and border mode.
    #[must_use]
    pub fn new(kernel: Kernel2d, border: BorderMode) -> Self {
        Self { kernel, border }
    }

    /// Returns a reference to the underlying kernel.
    #[must_use]
    pub fn kernel(&self) -> &Kernel2d {
        &self.kernel
    }

    /// Returns the border mode.
    #[must_use]
    pub fn border_mode(&self) -> BorderMode {
        self.border
    }

    /// Convolves `input` (row-major, single channel) and writes the result into
    /// `output`.  Both slices must have length `width * height`.
    ///
    /// # Panics
    ///
    /// Panics if `input.len() != width * height` or `output.len() != width * height`.
    #[allow(clippy::cast_precision_loss)]
    pub fn convolve(&self, input: &[f32], output: &mut [f32], width: usize, height: usize) {
        assert_eq!(input.len(), width * height);
        assert_eq!(output.len(), width * height);
        let half = (self.kernel.size.side() / 2) as isize;
        for cy in 0..height {
            for cx in 0..width {
                let mut acc = 0.0_f32;
                let mut wi = 0usize;
                for ky in -half..=half {
                    for kx in -half..=half {
                        let sx = cx as isize + kx;
                        let sy = cy as isize + ky;
                        let val = self.sample(input, width, height, sx, sy);
                        acc += val * self.kernel.weights[wi];
                        wi += 1;
                    }
                }
                if self.kernel.divisor.abs() > f32::EPSILON {
                    acc /= self.kernel.divisor;
                }
                output[cy * width + cx] = acc;
            }
        }
    }

    /// Samples `buf` at integer coordinates `(sx, sy)` applying the border mode.
    #[allow(clippy::cast_precision_loss)]
    fn sample(&self, buf: &[f32], width: usize, height: usize, sx: isize, sy: isize) -> f32 {
        let w = width as isize;
        let h = height as isize;
        let (ex, ey) = match self.border {
            BorderMode::Zero => {
                if sx < 0 || sy < 0 || sx >= w || sy >= h {
                    return 0.0;
                }
                (sx, sy)
            }
            BorderMode::Clamp => (sx.clamp(0, w - 1), sy.clamp(0, h - 1)),
            BorderMode::Reflect => {
                let rx = reflect_coord(sx, w);
                let ry = reflect_coord(sy, h);
                (rx, ry)
            }
        };
        buf[ey as usize * width + ex as usize]
    }
}

fn reflect_coord(v: isize, size: isize) -> isize {
    if size <= 0 {
        return 0;
    }
    let mut x = v;
    while x < 0 {
        x = -x - 1;
    }
    while x >= size {
        x = 2 * size - x - 1;
    }
    x
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_size_side() {
        assert_eq!(KernelSize::K3x3.side(), 3);
        assert_eq!(KernelSize::K5x5.side(), 5);
        assert_eq!(KernelSize::K7x7.side(), 7);
    }

    #[test]
    fn kernel_size_area() {
        assert_eq!(KernelSize::K3x3.area(), 9);
        assert_eq!(KernelSize::K5x5.area(), 25);
        assert_eq!(KernelSize::K7x7.area(), 49);
    }

    #[test]
    fn kernel2d_weight_sum_identity() {
        // Identity kernel — sum of weights == 1 when divisor is 1
        let k = Kernel2d::new(
            KernelSize::K3x3,
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
            1.0,
        );
        assert!((k.weight_sum() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn sharpen_kernel_center() {
        let k = sharpen_kernel();
        // centre weight should be 5
        assert!((k.weights[4] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn edge_detect_kernel_sum_zero() {
        let k = edge_detect_kernel();
        assert!(k.weight_sum().abs() < 1e-5);
    }

    #[test]
    fn box_blur_kernel_normalised() {
        let k = box_blur_kernel();
        let norm_sum: f32 = k.weights.iter().sum::<f32>() / k.divisor;
        assert!((norm_sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn gaussian_kernel_normalised() {
        let k = gaussian_kernel_3x3();
        let norm_sum: f32 = k.weights.iter().sum::<f32>() / k.divisor;
        assert!((norm_sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn apply_to_pixel_identity() {
        let k = Kernel2d::new(
            KernelSize::K3x3,
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
            1.0,
        );
        let buf = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9];
        let result = k.apply_to_pixel(&buf, 3, 3, 1, 1);
        assert!((result - 0.5).abs() < 1e-6);
    }

    #[test]
    fn apply_to_pixel_border_zero() {
        let k = Kernel2d::new(KernelSize::K3x3, vec![1.0; 9], 1.0);
        let buf = vec![1.0_f32; 9];
        // At corner (0,0): 4 of 9 neighbours are out of bounds → sum == 4/1
        let result = k.apply_to_pixel(&buf, 3, 3, 0, 0);
        assert!((result - 4.0).abs() < 1e-5);
    }

    #[test]
    fn convolve_identity_filter() {
        let identity = Kernel2d::new(
            KernelSize::K3x3,
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
            1.0,
        );
        let filter = ConvolutionFilter::new(identity, BorderMode::Zero);
        let input: Vec<f32> = (0..9).map(|i| i as f32).collect();
        let mut output = vec![0.0_f32; 9];
        filter.convolve(&input, &mut output, 3, 3);
        // Interior pixel (1,1) → index 4 → should be 4.0
        assert!((output[4] - 4.0).abs() < 1e-5);
    }

    #[test]
    fn convolve_uniform_blur() {
        let k = box_blur_kernel();
        let filter = ConvolutionFilter::new(k, BorderMode::Zero);
        let input = vec![1.0_f32; 25]; // 5x5 all-ones
        let mut output = vec![0.0_f32; 25];
        filter.convolve(&input, &mut output, 5, 5);
        // Interior pixel (2,2) has all 9 neighbours present → output == 1.0
        assert!((output[2 * 5 + 2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn border_mode_reflect_coord_positive() {
        // reflect_coord(5, 4) → should wrap back inside [0,3]
        let v = reflect_coord(5, 4);
        assert!((0..4).contains(&v), "got {v}");
    }

    #[test]
    fn convolve_clamp_border() {
        let k = box_blur_kernel();
        let filter = ConvolutionFilter::new(k, BorderMode::Clamp);
        let input: Vec<f32> = (0..9_u32).map(|i| i as f32).collect();
        let mut output = vec![0.0_f32; 9];
        filter.convolve(&input, &mut output, 3, 3);
        // All outputs must be finite
        assert!(output.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn kernel_new_panics_on_wrong_len() {
        let result =
            std::panic::catch_unwind(|| Kernel2d::new(KernelSize::K3x3, vec![1.0; 4], 1.0));
        assert!(result.is_err());
    }

    #[test]
    fn border_mode_copy() {
        let m = BorderMode::Zero;
        let m2 = m;
        assert_eq!(m, m2);
    }
}
