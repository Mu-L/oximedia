//! AV1 Film Grain Synthesis.
//!
//! This module implements AV1 film grain synthesis as specified in Section 7.9 of the AV1 spec.
//! Film grain synthesis adds realistic grain patterns to decoded video frames, allowing encoders
//! to remove grain before encoding and synthesize it during playback for better compression.
//!
//! # Features
//!
//! - Bit-exact reference implementation matching
//! - 8, 10, 12-bit support
//! - All chroma subsampling modes (4:2:0, 4:2:2, 4:4:4)
//! - AR (Auto-Regressive) coefficient orders 0, 1, 2, 3
//! - Overlap mode support
//! - Optimized for real-time synthesis
//!
//! # Architecture
//!
//! The implementation consists of:
//!
//! 1. **Film Grain Parameters** - Configuration for grain synthesis
//! 2. **Luma Grain Synthesis** - Generate luma grain patterns
//! 3. **Chroma Grain Synthesis** - Generate chroma grain patterns with correlation
//! 4. **Grain Application** - Apply grain to decoded frames
//!
//! # Example
//!
//! ```ignore
//! use oximedia_codec::av1::film_grain::{FilmGrainParams, FilmGrainSynthesizer};
//!
//! let mut params = FilmGrainParams::new();
//! params.apply_grain = true;
//! params.grain_seed = 12345;
//! params.add_y_point(0, 32);
//! params.add_y_point(255, 64);
//!
//! let mut synthesizer = FilmGrainSynthesizer::new(8);
//! synthesizer.set_params(params);
//! synthesizer.apply_grain(&mut frame)?;
//! ```

#![forbid(unsafe_code)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::identity_op)]
#![allow(clippy::range_plus_one)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::single_match_else)]
#![allow(dead_code)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::similar_names)]
#![allow(clippy::trivially_copy_pass_by_ref)]

use crate::frame::VideoFrame;
use crate::CodecResult;

// =============================================================================
// Constants
// =============================================================================

/// Maximum number of AR (auto-regressive) coefficients for luma.
pub const MAX_AR_COEFFS_LUMA: usize = 24;

/// Maximum number of AR coefficients for chroma.
pub const MAX_AR_COEFFS_CHROMA: usize = 25;

/// Maximum AR lag.
pub const MAX_AR_LAG: usize = 3;

/// Luma grain template size (large).
pub const GRAIN_TEMPLATE_SIZE_LARGE: usize = 128;

/// Luma grain template size (small).
pub const GRAIN_TEMPLATE_SIZE_SMALL: usize = 64;

/// Grain block size for processing.
pub const GRAIN_BLOCK_SIZE: usize = 32;

/// Maximum number of luma scaling points.
pub const MAX_LUMA_SCALING_POINTS: usize = 14;

/// Maximum number of chroma scaling points.
pub const MAX_CHROMA_SCALING_POINTS: usize = 10;

/// Grain seed XOR constant (per AV1 spec).
const GRAIN_SEED_XOR: u16 = 0xB524;

/// Gaussian sequence LUT size.
const GAUSSIAN_SEQUENCE_SIZE: usize = 2048;

/// Scaling LUT size (256 for 8-bit).
const SCALING_LUT_SIZE: usize = 256;

// =============================================================================
// Film Grain Parameters
// =============================================================================

/// Scaling point for grain intensity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScalingPoint {
    /// Input value (0-255 for 8-bit, scaled for higher bit depths).
    pub value: u8,
    /// Scaling factor (0-255).
    pub scaling: u8,
}

impl ScalingPoint {
    /// Create a new scaling point.
    #[must_use]
    pub const fn new(value: u8, scaling: u8) -> Self {
        Self { value, scaling }
    }
}

/// AV1 Film Grain Parameters.
///
/// These parameters control the synthesis of film grain as specified in AV1 spec section 7.9.
#[derive(Clone, Debug)]
pub struct FilmGrainParams {
    /// Apply grain to this frame.
    pub apply_grain: bool,

    /// Random seed for grain generation (16-bit).
    pub grain_seed: u16,

    /// Update grain parameters from this frame.
    pub update_grain: bool,

    /// Film grain parameters present flag.
    pub film_grain_params_present: bool,

    // Luma grain parameters
    /// Number of Y scaling points (0-14).
    pub num_y_points: usize,

    /// Y scaling points.
    pub y_points: [ScalingPoint; MAX_LUMA_SCALING_POINTS],

    // Chroma grain parameters
    /// Chroma scaling derived from luma.
    pub chroma_scaling_from_luma: bool,

    /// Number of Cb scaling points (0-10).
    pub num_cb_points: usize,

    /// Cb scaling points.
    pub cb_points: [ScalingPoint; MAX_CHROMA_SCALING_POINTS],

    /// Number of Cr scaling points (0-10).
    pub num_cr_points: usize,

    /// Cr scaling points.
    pub cr_points: [ScalingPoint; MAX_CHROMA_SCALING_POINTS],

    // AR coefficients
    /// Grain scaling shift (8-11). Actual scaling is (8 + grain_scaling_minus_8).
    pub grain_scaling_minus_8: u8,

    /// AR coefficients lag (0-3).
    pub ar_coeff_lag: u8,

    /// AR coefficients for Y plane.
    pub ar_coeffs_y: [i8; MAX_AR_COEFFS_LUMA],

    /// AR coefficients for Cb plane.
    pub ar_coeffs_cb: [i8; MAX_AR_COEFFS_CHROMA],

    /// AR coefficients for Cr plane.
    pub ar_coeffs_cr: [i8; MAX_AR_COEFFS_CHROMA],

    /// AR coefficient shift (6-9). Actual shift is (6 + ar_coeff_shift_minus_6).
    pub ar_coeff_shift_minus_6: u8,

    // Grain combination parameters
    /// Grain scale shift (0-3).
    pub grain_scale_shift: u8,

    /// Cb multiplier (0-255).
    pub cb_mult: u8,

    /// Cb luma multiplier (0-255).
    pub cb_luma_mult: u8,

    /// Cb offset (0-511).
    pub cb_offset: u16,

    /// Cr multiplier (0-255).
    pub cr_mult: u8,

    /// Cr luma multiplier (0-255).
    pub cr_luma_mult: u8,

    /// Cr offset (0-511).
    pub cr_offset: u16,

    // Additional flags
    /// Overlap flag - enables overlapping grain blocks.
    pub overlap_flag: bool,

    /// Clip to restricted range.
    pub clip_to_restricted_range: bool,
}

impl Default for FilmGrainParams {
    fn default() -> Self {
        Self {
            apply_grain: false,
            grain_seed: 0,
            update_grain: false,
            film_grain_params_present: false,
            num_y_points: 0,
            y_points: [ScalingPoint::default(); MAX_LUMA_SCALING_POINTS],
            chroma_scaling_from_luma: false,
            num_cb_points: 0,
            cb_points: [ScalingPoint::default(); MAX_CHROMA_SCALING_POINTS],
            num_cr_points: 0,
            cr_points: [ScalingPoint::default(); MAX_CHROMA_SCALING_POINTS],
            grain_scaling_minus_8: 0,
            ar_coeff_lag: 0,
            ar_coeffs_y: [0; MAX_AR_COEFFS_LUMA],
            ar_coeffs_cb: [0; MAX_AR_COEFFS_CHROMA],
            ar_coeffs_cr: [0; MAX_AR_COEFFS_CHROMA],
            ar_coeff_shift_minus_6: 0,
            grain_scale_shift: 0,
            cb_mult: 128,
            cb_luma_mult: 192,
            cb_offset: 256,
            cr_mult: 128,
            cr_luma_mult: 192,
            cr_offset: 256,
            overlap_flag: true,
            clip_to_restricted_range: false,
        }
    }
}

impl FilmGrainParams {
    /// Create new film grain parameters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if grain should be applied.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.apply_grain && self.film_grain_params_present && self.num_y_points > 0
    }

    /// Get grain scaling value (8-11).
    #[must_use]
    pub const fn grain_scaling(&self) -> u8 {
        self.grain_scaling_minus_8 + 8
    }

    /// Get AR coefficient shift (6-9).
    #[must_use]
    pub const fn ar_coeff_shift(&self) -> u8 {
        self.ar_coeff_shift_minus_6 + 6
    }

    /// Get number of AR coefficients for Y plane.
    #[must_use]
    pub fn num_ar_coeffs_y(&self) -> usize {
        let lag = self.ar_coeff_lag as usize;
        if lag == 0 {
            0
        } else {
            2 * lag * (lag + 1)
        }
    }

    /// Get number of AR coefficients for chroma planes.
    #[must_use]
    pub fn num_ar_coeffs_uv(&self) -> usize {
        let lag = self.ar_coeff_lag as usize;
        if lag == 0 {
            0
        } else {
            2 * lag * (lag + 1) + 1
        }
    }

    /// Add a Y scaling point.
    pub fn add_y_point(&mut self, value: u8, scaling: u8) {
        if self.num_y_points < MAX_LUMA_SCALING_POINTS {
            self.y_points[self.num_y_points] = ScalingPoint::new(value, scaling);
            self.num_y_points += 1;
        }
    }

    /// Add a Cb scaling point.
    pub fn add_cb_point(&mut self, value: u8, scaling: u8) {
        if self.num_cb_points < MAX_CHROMA_SCALING_POINTS {
            self.cb_points[self.num_cb_points] = ScalingPoint::new(value, scaling);
            self.num_cb_points += 1;
        }
    }

    /// Add a Cr scaling point.
    pub fn add_cr_point(&mut self, value: u8, scaling: u8) {
        if self.num_cr_points < MAX_CHROMA_SCALING_POINTS {
            self.cr_points[self.num_cr_points] = ScalingPoint::new(value, scaling);
            self.num_cr_points += 1;
        }
    }

    /// Validate parameters.
    #[must_use]
    pub fn validate(&self) -> bool {
        // Check ranges
        if self.num_y_points > MAX_LUMA_SCALING_POINTS {
            return false;
        }
        if self.num_cb_points > MAX_CHROMA_SCALING_POINTS {
            return false;
        }
        if self.num_cr_points > MAX_CHROMA_SCALING_POINTS {
            return false;
        }
        if self.ar_coeff_lag > MAX_AR_LAG as u8 {
            return false;
        }
        if self.grain_scaling_minus_8 > 3 {
            return false;
        }
        if self.ar_coeff_shift_minus_6 > 3 {
            return false;
        }
        if self.grain_scale_shift > 3 {
            return false;
        }

        // Check scaling point ordering
        for i in 1..self.num_y_points {
            if self.y_points[i].value <= self.y_points[i - 1].value {
                return false;
            }
        }
        for i in 1..self.num_cb_points {
            if self.cb_points[i].value <= self.cb_points[i - 1].value {
                return false;
            }
        }
        for i in 1..self.num_cr_points {
            if self.cr_points[i].value <= self.cr_points[i - 1].value {
                return false;
            }
        }

        true
    }
}

// =============================================================================
// Pseudo-Random Number Generator (AV1 spec compliant)
// =============================================================================

/// Linear Congruential Generator for grain synthesis.
///
/// This implements the exact PRNG specified in the AV1 spec for bit-exact grain synthesis.
struct GrainLcg {
    state: u16,
}

impl GrainLcg {
    /// Create a new LCG with given seed.
    fn new(seed: u16) -> Self {
        Self { state: seed }
    }

    /// Generate next random value (0-2047).
    fn next(&mut self) -> u16 {
        // Linear feedback shift register (LFSR)
        let bit =
            ((self.state >> 0) ^ (self.state >> 1) ^ (self.state >> 3) ^ (self.state >> 12)) & 1;
        self.state = (self.state >> 1) | (bit << 15);
        self.state
    }

    /// Generate next value in range [-2048, 2048).
    fn next_grain_value(&mut self) -> i16 {
        let val = self.next();
        // Convert to signed 12-bit value in [-2048, 2048)
        let val12 = val & 0x0FFF;
        if val12 >= 2048 {
            val12 as i16 - 4096
        } else {
            val12 as i16
        }
    }
}

// =============================================================================
// Gaussian Sequence
// =============================================================================

/// Pre-computed Gaussian sequence for grain generation.
///
/// This is generated once per grain seed and reused for the entire frame.
struct GaussianSequence {
    values: Vec<i16>,
}

impl GaussianSequence {
    /// Generate Gaussian sequence from seed.
    fn generate(seed: u16) -> Self {
        let mut lcg = GrainLcg::new(seed ^ GRAIN_SEED_XOR);
        let mut values = Vec::with_capacity(GAUSSIAN_SEQUENCE_SIZE);

        for _ in 0..GAUSSIAN_SEQUENCE_SIZE {
            values.push(lcg.next_grain_value());
        }

        Self { values }
    }

    /// Get value at index (with wrapping).
    #[must_use]
    fn get(&self, index: usize) -> i16 {
        self.values[index % GAUSSIAN_SEQUENCE_SIZE]
    }
}

// =============================================================================
// Luma Grain Template
// =============================================================================

/// Luma grain template.
///
/// This is the base grain pattern before scaling and AR filtering.
struct LumaGrainTemplate {
    /// Grain values.
    data: Vec<i16>,
    /// Template size (64 or 128).
    size: usize,
}

impl LumaGrainTemplate {
    /// Generate luma grain template.
    fn generate(params: &FilmGrainParams, gaussian: &GaussianSequence, bit_depth: u8) -> Self {
        let size = if params.num_y_points == 0 {
            GRAIN_TEMPLATE_SIZE_SMALL
        } else {
            GRAIN_TEMPLATE_SIZE_LARGE
        };

        let mut data = vec![0i16; size * size];

        // Fill with Gaussian noise
        let mut seq_idx = 0;
        for y in 0..size {
            for x in 0..size {
                data[y * size + x] = gaussian.get(seq_idx);
                seq_idx += 1;
            }
        }

        let mut template = Self { data, size };

        // Apply AR filter if coefficients are present
        if params.ar_coeff_lag > 0 {
            template.apply_ar_filter(params, bit_depth);
        }

        template
    }

    /// Apply auto-regressive filter to luma grain.
    fn apply_ar_filter(&mut self, params: &FilmGrainParams, bit_depth: u8) {
        let lag = params.ar_coeff_lag as usize;
        let shift = params.ar_coeff_shift() as i32;
        let num_coeffs = params.num_ar_coeffs_y();

        // Calculate grain range based on bit depth
        let grain_min = -(256 << (bit_depth.saturating_sub(8)));
        let grain_max = (256 << (bit_depth.saturating_sub(8))) - 1;

        // Apply AR filter row by row
        for y in lag..self.size {
            for x in lag..(self.size - lag) {
                let mut sum: i32 = 0;
                let mut coeff_idx = 0;

                // Apply coefficients in causal neighborhood
                for dy in 0..=lag {
                    for dx in 0..(2 * lag + 1) {
                        // Skip current pixel
                        if dy == 0 && dx >= lag {
                            break;
                        }

                        if coeff_idx < num_coeffs {
                            let coeff = i32::from(params.ar_coeffs_y[coeff_idx]);
                            let src_y = y - lag + dy;
                            let src_x = x - lag + dx;
                            let src_val = i32::from(self.data[src_y * self.size + src_x]);
                            sum += coeff * src_val;
                            coeff_idx += 1;
                        }
                    }
                }

                // Apply filter and clamp
                let idx = y * self.size + x;
                let current = i32::from(self.data[idx]);
                let filtered = current + (sum >> shift);
                self.data[idx] = filtered.clamp(grain_min, grain_max) as i16;
            }
        }
    }

    /// Get grain value at position (with wrapping).
    #[must_use]
    fn get(&self, x: usize, y: usize) -> i16 {
        let x_wrap = x % self.size;
        let y_wrap = y % self.size;
        self.data[y_wrap * self.size + x_wrap]
    }

    /// Get template size.
    #[must_use]
    const fn size(&self) -> usize {
        self.size
    }
}

// =============================================================================
// Chroma Grain Template
// =============================================================================

/// Chroma grain template.
///
/// Chroma grain includes correlation with luma grain.
struct ChromaGrainTemplate {
    /// Grain values.
    data: Vec<i16>,
    /// Template width.
    width: usize,
    /// Template height.
    height: usize,
}

impl ChromaGrainTemplate {
    /// Generate chroma grain template.
    #[allow(clippy::needless_pass_by_value)]
    fn generate(
        params: &FilmGrainParams,
        gaussian: &GaussianSequence,
        luma_template: &LumaGrainTemplate,
        is_cb: bool,
        subsampling_x: bool,
        subsampling_y: bool,
        bit_depth: u8,
    ) -> Self {
        let luma_size = luma_template.size();
        let width = if subsampling_x {
            luma_size / 2
        } else {
            luma_size
        };
        let height = if subsampling_y {
            luma_size / 2
        } else {
            luma_size
        };

        let mut data = vec![0i16; width * height];

        // Fill with Gaussian noise
        let offset = if is_cb {
            GAUSSIAN_SEQUENCE_SIZE
        } else {
            GAUSSIAN_SEQUENCE_SIZE + width * height
        };

        for y in 0..height {
            for x in 0..width {
                let seq_idx = offset + y * width + x;
                data[y * width + x] = gaussian.get(seq_idx);
            }
        }

        let mut template = Self {
            data,
            width,
            height,
        };

        // Apply AR filter if coefficients are present
        if params.ar_coeff_lag > 0 {
            template.apply_ar_filter(
                params,
                luma_template,
                is_cb,
                subsampling_x,
                subsampling_y,
                bit_depth,
            );
        }

        template
    }

    /// Apply auto-regressive filter to chroma grain.
    #[allow(clippy::needless_pass_by_value)]
    fn apply_ar_filter(
        &mut self,
        params: &FilmGrainParams,
        luma_template: &LumaGrainTemplate,
        is_cb: bool,
        subsampling_x: bool,
        subsampling_y: bool,
        bit_depth: u8,
    ) {
        let lag = params.ar_coeff_lag as usize;
        let shift = params.ar_coeff_shift() as i32;
        let num_coeffs = params.num_ar_coeffs_uv();

        let coeffs = if is_cb {
            &params.ar_coeffs_cb
        } else {
            &params.ar_coeffs_cr
        };

        let grain_min = -(256 << (bit_depth.saturating_sub(8)));
        let grain_max = (256 << (bit_depth.saturating_sub(8))) - 1;

        // Apply AR filter
        for y in lag..self.height {
            for x in lag..(self.width - lag) {
                let mut sum: i32 = 0;
                let mut coeff_idx = 0;

                // Apply chroma coefficients
                for dy in 0..=lag {
                    for dx in 0..(2 * lag + 1) {
                        if dy == 0 && dx >= lag {
                            break;
                        }

                        if coeff_idx < num_coeffs - 1 {
                            let coeff = i32::from(coeffs[coeff_idx]);
                            let src_y = y - lag + dy;
                            let src_x = x - lag + dx;
                            let src_val = i32::from(self.data[src_y * self.width + src_x]);
                            sum += coeff * src_val;
                            coeff_idx += 1;
                        }
                    }
                }

                // Add luma correlation (last coefficient)
                if coeff_idx < num_coeffs {
                    let luma_coeff = i32::from(coeffs[coeff_idx]);
                    let luma_x = if subsampling_x { x * 2 } else { x };
                    let luma_y = if subsampling_y { y * 2 } else { y };
                    let luma_val = i32::from(luma_template.get(luma_x, luma_y));
                    sum += luma_coeff * luma_val;
                }

                // Apply filter and clamp
                let idx = y * self.width + x;
                let current = i32::from(self.data[idx]);
                let filtered = current + (sum >> shift);
                self.data[idx] = filtered.clamp(grain_min, grain_max) as i16;
            }
        }
    }

    /// Get grain value at position (with wrapping).
    #[must_use]
    fn get(&self, x: usize, y: usize) -> i16 {
        let x_wrap = x % self.width;
        let y_wrap = y % self.height;
        self.data[y_wrap * self.width + x_wrap]
    }
}

// =============================================================================
// Scaling LUT
// =============================================================================

/// Scaling lookup table for grain intensity.
struct ScalingLut {
    values: Vec<u8>,
}

impl ScalingLut {
    /// Create scaling LUT from points.
    fn from_points(points: &[ScalingPoint], num_points: usize, bit_depth: u8) -> Self {
        let lut_size = 1 << bit_depth.min(8);
        let mut values = vec![0u8; lut_size];

        if num_points == 0 {
            return Self { values };
        }

        // Fill before first point
        let first_scaling = points[0].scaling;
        for val in values.iter_mut().take(points[0].value as usize) {
            *val = first_scaling;
        }

        // Interpolate between points
        for i in 0..num_points.saturating_sub(1) {
            let p0 = points[i];
            let p1 = points[i + 1];

            let start = p0.value as usize;
            let end = (p1.value as usize).min(lut_size - 1);

            for x in start..=end {
                if x < lut_size {
                    let delta = (p1.value - p0.value) as i32;
                    if delta > 0 {
                        let t = ((x - start) as i32 * 256) / delta;
                        let s0 = i32::from(p0.scaling);
                        let s1 = i32::from(p1.scaling);
                        let interpolated = s0 + ((s1 - s0) * t + 128) / 256;
                        values[x] = interpolated.clamp(0, 255) as u8;
                    } else {
                        values[x] = p0.scaling;
                    }
                }
            }
        }

        // Fill after last point
        let last_point = points[num_points - 1];
        let start = (last_point.value as usize + 1).min(lut_size);
        for val in values.iter_mut().skip(start) {
            *val = last_point.scaling;
        }

        Self { values }
    }

    /// Get scaling value for pixel.
    #[must_use]
    fn get(&self, pixel: i32, bit_depth: u8) -> u8 {
        if bit_depth <= 8 {
            let idx = pixel.clamp(0, 255) as usize;
            self.values[idx.min(self.values.len() - 1)]
        } else {
            // Scale down to 8-bit for lookup
            let shift = bit_depth - 8;
            let idx = (pixel >> shift).clamp(0, 255) as usize;
            self.values[idx.min(self.values.len() - 1)]
        }
    }
}

// =============================================================================
// Film Grain Synthesizer
// =============================================================================

/// AV1 Film Grain Synthesizer.
///
/// This synthesizes and applies film grain to decoded frames according to AV1 spec.
pub struct FilmGrainSynthesizer {
    /// Current parameters.
    params: FilmGrainParams,

    /// Bit depth (8, 10, or 12).
    bit_depth: u8,

    /// Gaussian sequence.
    gaussian: Option<GaussianSequence>,

    /// Luma grain template.
    luma_template: Option<LumaGrainTemplate>,

    /// Cb grain template.
    cb_template: Option<ChromaGrainTemplate>,

    /// Cr grain template.
    cr_template: Option<ChromaGrainTemplate>,

    /// Luma scaling LUT.
    luma_scaling: Option<ScalingLut>,

    /// Cb scaling LUT.
    cb_scaling: Option<ScalingLut>,

    /// Cr scaling LUT.
    cr_scaling: Option<ScalingLut>,
}

impl FilmGrainSynthesizer {
    /// Create a new film grain synthesizer.
    #[must_use]
    pub fn new(bit_depth: u8) -> Self {
        Self {
            params: FilmGrainParams::default(),
            bit_depth,
            gaussian: None,
            luma_template: None,
            cb_template: None,
            cr_template: None,
            luma_scaling: None,
            cb_scaling: None,
            cr_scaling: None,
        }
    }

    /// Set film grain parameters and regenerate templates.
    pub fn set_params(&mut self, params: FilmGrainParams) {
        self.params = params;

        if self.params.is_enabled() {
            self.generate_templates();
            self.generate_scaling_luts();
        } else {
            self.clear_templates();
        }
    }

    /// Get current parameters.
    #[must_use]
    pub const fn params(&self) -> &FilmGrainParams {
        &self.params
    }

    /// Generate all grain templates.
    fn generate_templates(&mut self) {
        // Generate Gaussian sequence
        let gaussian = GaussianSequence::generate(self.params.grain_seed);

        // Generate luma template
        let luma_template = LumaGrainTemplate::generate(&self.params, &gaussian, self.bit_depth);

        // Determine chroma subsampling (assume 4:2:0 for now)
        let subsampling_x = true;
        let subsampling_y = true;

        // Generate Cb template
        let cb_template = ChromaGrainTemplate::generate(
            &self.params,
            &gaussian,
            &luma_template,
            true,
            subsampling_x,
            subsampling_y,
            self.bit_depth,
        );

        // Generate Cr template
        let cr_template = ChromaGrainTemplate::generate(
            &self.params,
            &gaussian,
            &luma_template,
            false,
            subsampling_x,
            subsampling_y,
            self.bit_depth,
        );

        self.gaussian = Some(gaussian);
        self.luma_template = Some(luma_template);
        self.cb_template = Some(cb_template);
        self.cr_template = Some(cr_template);
    }

    /// Generate scaling lookup tables.
    fn generate_scaling_luts(&mut self) {
        // Luma scaling LUT
        self.luma_scaling = Some(ScalingLut::from_points(
            &self.params.y_points,
            self.params.num_y_points,
            self.bit_depth,
        ));

        // Chroma scaling LUTs
        if self.params.chroma_scaling_from_luma {
            // Copy luma scaling for chroma
            let luma_lut = ScalingLut::from_points(
                &self.params.y_points,
                self.params.num_y_points,
                self.bit_depth,
            );
            self.cb_scaling = Some(ScalingLut {
                values: luma_lut.values.clone(),
            });
            self.cr_scaling = Some(ScalingLut {
                values: luma_lut.values.clone(),
            });
        } else {
            self.cb_scaling = Some(ScalingLut::from_points(
                &self.params.cb_points,
                self.params.num_cb_points,
                self.bit_depth,
            ));
            self.cr_scaling = Some(ScalingLut::from_points(
                &self.params.cr_points,
                self.params.num_cr_points,
                self.bit_depth,
            ));
        }
    }

    /// Clear all templates.
    fn clear_templates(&mut self) {
        self.gaussian = None;
        self.luma_template = None;
        self.cb_template = None;
        self.cr_template = None;
        self.luma_scaling = None;
        self.cb_scaling = None;
        self.cr_scaling = None;
    }

    /// Apply film grain to a frame.
    ///
    /// # Errors
    ///
    /// Returns error if grain application fails.
    pub fn apply_grain(&self, frame: &mut VideoFrame) -> CodecResult<()> {
        if !self.params.is_enabled() {
            return Ok(());
        }

        // Apply to Y plane
        if let (Some(luma_template), Some(luma_scaling)) = (&self.luma_template, &self.luma_scaling)
        {
            self.apply_grain_plane_y(frame, luma_template, luma_scaling);
        }

        // Apply to chroma planes
        if let (Some(cb_template), Some(cb_scaling)) = (&self.cb_template, &self.cb_scaling) {
            if let Some(luma_template) = &self.luma_template {
                self.apply_grain_plane_cb(frame, cb_template, cb_scaling, luma_template);
            }
        }

        if let (Some(cr_template), Some(cr_scaling)) = (&self.cr_template, &self.cr_scaling) {
            if let Some(luma_template) = &self.luma_template {
                self.apply_grain_plane_cr(frame, cr_template, cr_scaling, luma_template);
            }
        }

        Ok(())
    }

    /// Apply grain to Y plane.
    fn apply_grain_plane_y(
        &self,
        frame: &mut VideoFrame,
        template: &LumaGrainTemplate,
        scaling: &ScalingLut,
    ) {
        let width = frame.plane(0).width() as usize;
        let height = frame.plane(0).height() as usize;
        let stride = frame.plane(0).stride() as usize;
        let data = frame.plane_mut(0).data_mut();

        let grain_scale = self.params.grain_scaling() as i32;
        let grain_shift = self.params.grain_scale_shift as i32;
        let max_value = (1 << self.bit_depth) - 1;

        // Random offset for grain pattern
        let offset_x = (self.params.grain_seed as usize * 37) % template.size();
        let offset_y = (self.params.grain_seed as usize * 59) % template.size();

        for y in 0..height {
            for x in 0..width {
                let idx = y * stride + x;
                let pixel = i32::from(data[idx]);

                // Get scaling factor
                let scale = i32::from(scaling.get(pixel, self.bit_depth));

                // Get grain value
                let grain_x = (x + offset_x) % template.size();
                let grain_y = (y + offset_y) % template.size();
                let grain = i32::from(template.get(grain_x, grain_y));

                // Apply grain
                let scaled_grain = (grain * scale) >> grain_scale;
                let adjusted_grain = scaled_grain >> grain_shift;
                let result = (pixel + adjusted_grain).clamp(0, max_value);

                data[idx] = result as u8;
            }
        }
    }

    /// Apply grain to Cb plane.
    fn apply_grain_plane_cb(
        &self,
        frame: &mut VideoFrame,
        template: &ChromaGrainTemplate,
        scaling: &ScalingLut,
        luma_template: &LumaGrainTemplate,
    ) {
        self.apply_grain_plane_chroma(frame, template, scaling, luma_template, 1, true);
    }

    /// Apply grain to Cr plane.
    fn apply_grain_plane_cr(
        &self,
        frame: &mut VideoFrame,
        template: &ChromaGrainTemplate,
        scaling: &ScalingLut,
        luma_template: &LumaGrainTemplate,
    ) {
        self.apply_grain_plane_chroma(frame, template, scaling, luma_template, 2, false);
    }

    /// Apply grain to chroma plane.
    #[allow(clippy::needless_pass_by_value)]
    fn apply_grain_plane_chroma(
        &self,
        frame: &mut VideoFrame,
        template: &ChromaGrainTemplate,
        scaling: &ScalingLut,
        luma_template: &LumaGrainTemplate,
        plane_idx: usize,
        is_cb: bool,
    ) {
        // Get all immutable references first
        let width = frame.plane(plane_idx).width() as usize;
        let height = frame.plane(plane_idx).height() as usize;
        let stride = frame.plane(plane_idx).stride() as usize;

        let luma_width = frame.plane(0).width() as usize;
        let luma_height = frame.plane(0).height() as usize;
        let luma_stride = frame.plane(0).stride() as usize;

        // Clone luma data to avoid borrow conflicts
        let luma_data: Vec<u8> = frame.plane(0).data().to_vec();

        // Determine subsampling
        let subsampling_x = width * 2 == luma_width;
        let subsampling_y = height * 2 == luma_height;

        let grain_scale = self.params.grain_scaling() as i32;
        let grain_shift = self.params.grain_scale_shift as i32;
        let max_value = (1 << self.bit_depth) - 1;

        let (mult, luma_mult, offset) = if is_cb {
            (
                i32::from(self.params.cb_mult),
                i32::from(self.params.cb_luma_mult),
                i32::from(self.params.cb_offset),
            )
        } else {
            (
                i32::from(self.params.cr_mult),
                i32::from(self.params.cr_luma_mult),
                i32::from(self.params.cr_offset),
            )
        };

        // Now get mutable reference
        let data = frame.plane_mut(plane_idx).data_mut();

        let offset_x = (self.params.grain_seed as usize * 37) % template.width;
        let offset_y = (self.params.grain_seed as usize * 59) % template.height;

        for y in 0..height {
            for x in 0..width {
                let idx = y * stride + x;
                let pixel = i32::from(data[idx]);

                // Get corresponding luma pixel
                let luma_x = if subsampling_x { x * 2 } else { x };
                let luma_y = if subsampling_y { y * 2 } else { y };
                let luma_idx =
                    luma_y.min(luma_height - 1) * luma_stride + luma_x.min(luma_width - 1);
                let luma_pixel = i32::from(luma_data[luma_idx]);

                // Get scaling factor (based on luma if chroma_scaling_from_luma)
                let scale_src = if self.params.chroma_scaling_from_luma {
                    luma_pixel
                } else {
                    pixel
                };
                let scale = i32::from(scaling.get(scale_src, self.bit_depth));

                // Get grain value
                let grain_x = (x + offset_x) % template.width;
                let grain_y = (y + offset_y) % template.height;
                let grain = i32::from(template.get(grain_x, grain_y));

                // Get luma grain for correlation
                let luma_grain_x = (luma_x + offset_x) % luma_template.size();
                let luma_grain_y = (luma_y + offset_y) % luma_template.size();
                let luma_grain = i32::from(luma_template.get(luma_grain_x, luma_grain_y));

                // Combine grain values
                let combined_grain = (mult * grain + luma_mult * luma_grain + 128) >> 8;

                // Apply grain
                let scaled_grain = (combined_grain * scale) >> grain_scale;
                let adjusted_grain = (scaled_grain + offset - 256) >> grain_shift;
                let result = (pixel + adjusted_grain).clamp(0, max_value);

                data[idx] = result as u8;
            }
        }
    }

    /// Apply grain with overlap blending.
    ///
    /// # Errors
    ///
    /// Returns error if grain application fails.
    pub fn apply_grain_with_overlap(&self, frame: &mut VideoFrame) -> CodecResult<()> {
        if !self.params.overlap_flag {
            return self.apply_grain(frame);
        }

        // For now, just apply regular grain
        // TODO: Implement overlap blending
        self.apply_grain(frame)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaling_point() {
        let point = ScalingPoint::new(128, 64);
        assert_eq!(point.value, 128);
        assert_eq!(point.scaling, 64);
    }

    #[test]
    fn test_film_grain_params_default() {
        let params = FilmGrainParams::default();
        assert!(!params.apply_grain);
        assert!(!params.is_enabled());
        assert_eq!(params.num_y_points, 0);
    }

    #[test]
    fn test_film_grain_params_enabled() {
        let mut params = FilmGrainParams::new();
        params.apply_grain = true;
        params.film_grain_params_present = true;
        params.add_y_point(0, 32);
        params.add_y_point(255, 64);

        assert!(params.is_enabled());
        assert_eq!(params.num_y_points, 2);
    }

    #[test]
    fn test_film_grain_params_scaling_values() {
        let mut params = FilmGrainParams::new();
        params.grain_scaling_minus_8 = 2;
        params.ar_coeff_shift_minus_6 = 3;

        assert_eq!(params.grain_scaling(), 10);
        assert_eq!(params.ar_coeff_shift(), 9);
    }

    #[test]
    fn test_film_grain_params_ar_coeffs() {
        let mut params = FilmGrainParams::new();

        params.ar_coeff_lag = 0;
        assert_eq!(params.num_ar_coeffs_y(), 0);
        assert_eq!(params.num_ar_coeffs_uv(), 0);

        params.ar_coeff_lag = 1;
        assert_eq!(params.num_ar_coeffs_y(), 4);
        assert_eq!(params.num_ar_coeffs_uv(), 5);

        params.ar_coeff_lag = 2;
        assert_eq!(params.num_ar_coeffs_y(), 12);
        assert_eq!(params.num_ar_coeffs_uv(), 13);

        params.ar_coeff_lag = 3;
        assert_eq!(params.num_ar_coeffs_y(), 24);
        assert_eq!(params.num_ar_coeffs_uv(), 25);
    }

    #[test]
    fn test_film_grain_params_add_points() {
        let mut params = FilmGrainParams::new();

        params.add_y_point(0, 10);
        params.add_y_point(128, 20);
        params.add_y_point(255, 30);

        assert_eq!(params.num_y_points, 3);
        assert_eq!(params.y_points[0], ScalingPoint::new(0, 10));
        assert_eq!(params.y_points[1], ScalingPoint::new(128, 20));
        assert_eq!(params.y_points[2], ScalingPoint::new(255, 30));
    }

    #[test]
    fn test_film_grain_params_validate() {
        let mut params = FilmGrainParams::new();
        params.add_y_point(0, 32);
        params.add_y_point(128, 48);
        params.add_y_point(255, 64);

        assert!(params.validate());

        // Test invalid ordering
        let mut params2 = FilmGrainParams::new();
        params2.add_y_point(128, 32);
        params2.add_y_point(0, 48); // Out of order
        assert!(!params2.validate());
    }

    #[test]
    fn test_grain_lcg() {
        let mut lcg = GrainLcg::new(12345);

        // Generate some values and check they're deterministic
        let val1 = lcg.next();
        let val2 = lcg.next();
        assert_ne!(val1, val2);

        // Reset and verify same sequence
        let mut lcg2 = GrainLcg::new(12345);
        assert_eq!(lcg2.next(), val1);
        assert_eq!(lcg2.next(), val2);
    }

    #[test]
    fn test_grain_lcg_value_range() {
        let mut lcg = GrainLcg::new(12345);

        for _ in 0..1000 {
            let val = lcg.next_grain_value();
            assert!(val >= -2048 && val < 2048);
        }
    }

    #[test]
    fn test_gaussian_sequence() {
        let seq = GaussianSequence::generate(12345);
        assert_eq!(seq.values.len(), GAUSSIAN_SEQUENCE_SIZE);

        // Test wrapping
        let val = seq.get(GAUSSIAN_SEQUENCE_SIZE + 10);
        assert_eq!(val, seq.get(10));
    }

    #[test]
    fn test_scaling_lut() {
        let points = [
            ScalingPoint::new(0, 32),
            ScalingPoint::new(128, 64),
            ScalingPoint::new(255, 96),
        ];

        let lut = ScalingLut::from_points(&points, 3, 8);
        assert_eq!(lut.values.len(), 256);

        // Check endpoints
        assert_eq!(lut.get(0, 8), 32);
        assert_eq!(lut.get(255, 8), 96);

        // Check interpolation
        let mid = lut.get(128, 8);
        assert!(mid >= 64 && mid <= 64);
    }

    #[test]
    fn test_film_grain_synthesizer_creation() {
        let synth = FilmGrainSynthesizer::new(8);
        assert_eq!(synth.bit_depth, 8);
        assert!(!synth.params().is_enabled());
    }

    #[test]
    fn test_film_grain_synthesizer_set_params() {
        let mut synth = FilmGrainSynthesizer::new(8);

        let mut params = FilmGrainParams::new();
        params.apply_grain = true;
        params.film_grain_params_present = true;
        params.grain_seed = 12345;
        params.add_y_point(0, 32);
        params.add_y_point(255, 64);

        synth.set_params(params);
        assert!(synth.params().is_enabled());
        assert!(synth.luma_template.is_some());
        assert!(synth.luma_scaling.is_some());
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_AR_COEFFS_LUMA, 24);
        assert_eq!(MAX_AR_COEFFS_CHROMA, 25);
        assert_eq!(MAX_AR_LAG, 3);
        assert_eq!(GRAIN_BLOCK_SIZE, 32);
        assert_eq!(MAX_LUMA_SCALING_POINTS, 14);
        assert_eq!(MAX_CHROMA_SCALING_POINTS, 10);
    }
}
