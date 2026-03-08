//! Pixel mapping for LED walls in virtual production.
//!
//! Provides tools for mapping rendered pixels to physical LED panel coordinates,
//! including HDR remapping, gamma correction, and per-panel calibration offsets.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

/// An RGB pixel with floating-point components in linear light [0.0, ∞).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearPixel {
    /// Red channel.
    pub r: f32,
    /// Green channel.
    pub g: f32,
    /// Blue channel.
    pub b: f32,
}

impl LinearPixel {
    /// Create a new linear pixel.
    #[must_use]
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    /// Clamp all channels to [0.0, `max_nits`].
    #[must_use]
    pub fn clamp_nits(self, max_nits: f32) -> Self {
        Self {
            r: self.r.clamp(0.0, max_nits),
            g: self.g.clamp(0.0, max_nits),
            b: self.b.clamp(0.0, max_nits),
        }
    }

    /// Scale all channels by a factor.
    #[must_use]
    pub fn scale(self, factor: f32) -> Self {
        Self {
            r: self.r * factor,
            g: self.g * factor,
            b: self.b * factor,
        }
    }

    /// Convert to 8-bit sRGB (gamma-corrected) output.
    #[must_use]
    pub fn to_srgb8(self) -> [u8; 3] {
        let apply_gamma = |c: f32| -> u8 {
            let clamped = c.clamp(0.0, 1.0);
            let gamma = if clamped <= 0.003_130_8 {
                clamped * 12.92
            } else {
                1.055 * clamped.powf(1.0 / 2.4) - 0.055
            };
            (gamma * 255.0 + 0.5) as u8
        };
        [
            apply_gamma(self.r),
            apply_gamma(self.g),
            apply_gamma(self.b),
        ]
    }

    /// Convert to 10-bit output (values 0–1023).
    #[must_use]
    pub fn to_10bit(self) -> [u16; 3] {
        let conv = |c: f32| -> u16 { (c.clamp(0.0, 1.0) * 1023.0 + 0.5) as u16 };
        [conv(self.r), conv(self.g), conv(self.b)]
    }
}

/// A per-panel calibration offset that adjusts pixel values.
#[derive(Debug, Clone, Copy)]
pub struct PanelCalibrationOffset {
    /// Additive gain offset for red [−1.0, 1.0].
    pub r_gain: f32,
    /// Additive gain offset for green [−1.0, 1.0].
    pub g_gain: f32,
    /// Additive gain offset for blue [−1.0, 1.0].
    pub b_gain: f32,
    /// Brightness multiplier (1.0 = no change).
    pub brightness: f32,
}

impl PanelCalibrationOffset {
    /// Create a new calibration offset.
    #[must_use]
    pub fn new(r_gain: f32, g_gain: f32, b_gain: f32, brightness: f32) -> Self {
        Self {
            r_gain,
            g_gain,
            b_gain,
            brightness: brightness.max(0.0),
        }
    }

    /// Identity calibration (no change).
    #[must_use]
    pub fn identity() -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0)
    }

    /// Apply calibration to a pixel.
    #[must_use]
    pub fn apply(&self, pixel: LinearPixel) -> LinearPixel {
        LinearPixel {
            r: ((pixel.r + self.r_gain) * self.brightness).max(0.0),
            g: ((pixel.g + self.g_gain) * self.brightness).max(0.0),
            b: ((pixel.b + self.b_gain) * self.brightness).max(0.0),
        }
    }
}

impl Default for PanelCalibrationOffset {
    fn default() -> Self {
        Self::identity()
    }
}

/// HDR tone-mapping mode for mapping scene-linear HDR to LED output range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToneMappingMode {
    /// No tone mapping — clip values above peak nits.
    Clip,
    /// Simple linear scale to fit peak nits.
    #[default]
    LinearScale,
    /// Reinhard global tone mapping.
    Reinhard,
    /// ACES filmic tone mapping approximation.
    AcesFilmic,
}

/// Applies HDR tone mapping from scene-linear input to LED output range.
#[allow(dead_code)]
pub struct HdrToneMapper {
    /// Peak nits of the LED wall.
    peak_nits: f32,
    /// Tone mapping mode.
    mode: ToneMappingMode,
}

impl HdrToneMapper {
    /// Create a new tone mapper.
    #[must_use]
    pub fn new(peak_nits: f32, mode: ToneMappingMode) -> Self {
        Self { peak_nits, mode }
    }

    /// Map a scene-linear pixel to LED output range [0.0, 1.0].
    #[must_use]
    pub fn map(&self, pixel: LinearPixel) -> LinearPixel {
        match self.mode {
            ToneMappingMode::Clip => pixel.clamp_nits(self.peak_nits).scale(1.0 / self.peak_nits),
            ToneMappingMode::LinearScale => pixel.scale(1.0 / self.peak_nits).clamp_nits(1.0),
            ToneMappingMode::Reinhard => self.reinhard(pixel),
            ToneMappingMode::AcesFilmic => self.aces(pixel),
        }
    }

    fn reinhard(&self, pixel: LinearPixel) -> LinearPixel {
        let scale = 1.0 / self.peak_nits;
        let map = |c: f32| -> f32 {
            let x = c * scale;
            x / (1.0 + x)
        };
        LinearPixel::new(map(pixel.r), map(pixel.g), map(pixel.b))
    }

    fn aces(&self, pixel: LinearPixel) -> LinearPixel {
        // Narkowicz 2015 ACES approximation
        let scale = 1.0 / self.peak_nits;
        let map = |c: f32| -> f32 {
            let x = c * scale;
            let a = 2.51_f32;
            let b = 0.03_f32;
            let c_const = 2.43_f32;
            let d = 0.59_f32;
            let e = 0.14_f32;
            ((x * (a * x + b)) / (x * (c_const * x + d) + e)).clamp(0.0, 1.0)
        };
        LinearPixel::new(map(pixel.r), map(pixel.g), map(pixel.b))
    }

    /// Peak nits configured for this mapper.
    #[must_use]
    pub fn peak_nits(&self) -> f32 {
        self.peak_nits
    }

    /// Tone mapping mode.
    #[must_use]
    pub fn mode(&self) -> ToneMappingMode {
        self.mode
    }
}

/// Maps a 2D buffer of pixels to panel-local pixel buffers.
#[allow(dead_code)]
pub struct PixelMapper {
    /// Total wall width in pixels.
    wall_width: u32,
    /// Total wall height in pixels.
    wall_height: u32,
    /// Width of each panel tile.
    tile_width: u32,
    /// Height of each panel tile.
    tile_height: u32,
}

impl PixelMapper {
    /// Create a new pixel mapper.
    #[must_use]
    pub fn new(wall_width: u32, wall_height: u32, tile_width: u32, tile_height: u32) -> Self {
        Self {
            wall_width,
            wall_height,
            tile_width,
            tile_height,
        }
    }

    /// Number of panel columns.
    #[must_use]
    pub fn panel_cols(&self) -> u32 {
        if self.tile_width == 0 {
            return 0;
        }
        self.wall_width.div_ceil(self.tile_width)
    }

    /// Number of panel rows.
    #[must_use]
    pub fn panel_rows(&self) -> u32 {
        if self.tile_height == 0 {
            return 0;
        }
        self.wall_height.div_ceil(self.tile_height)
    }

    /// Convert a global pixel coordinate to (col, row, `local_x`, `local_y`).
    #[must_use]
    pub fn global_to_panel(&self, gx: u32, gy: u32) -> Option<(u32, u32, u32, u32)> {
        if self.tile_width == 0 || self.tile_height == 0 {
            return None;
        }
        if gx >= self.wall_width || gy >= self.wall_height {
            return None;
        }
        let col = gx / self.tile_width;
        let row = gy / self.tile_height;
        let local_x = gx % self.tile_width;
        let local_y = gy % self.tile_height;
        Some((col, row, local_x, local_y))
    }

    /// Convert (col, row, `local_x`, `local_y`) back to global coordinates.
    #[must_use]
    pub fn panel_to_global(&self, col: u32, row: u32, local_x: u32, local_y: u32) -> (u32, u32) {
        (
            col * self.tile_width + local_x,
            row * self.tile_height + local_y,
        )
    }

    /// Wall resolution.
    #[must_use]
    pub fn resolution(&self) -> (u32, u32) {
        (self.wall_width, self.wall_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_pixel_clamp_nits() {
        let px = LinearPixel::new(2000.0, 500.0, -10.0);
        let clamped = px.clamp_nits(1000.0);
        assert_eq!(clamped.r, 1000.0);
        assert_eq!(clamped.g, 500.0);
        assert_eq!(clamped.b, 0.0);
    }

    #[test]
    fn test_linear_pixel_scale() {
        let px = LinearPixel::new(1.0, 0.5, 0.25);
        let scaled = px.scale(2.0);
        assert!((scaled.r - 2.0).abs() < 1e-6);
        assert!((scaled.g - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_linear_pixel_to_srgb8_black() {
        let px = LinearPixel::new(0.0, 0.0, 0.0);
        assert_eq!(px.to_srgb8(), [0, 0, 0]);
    }

    #[test]
    fn test_linear_pixel_to_srgb8_white() {
        let px = LinearPixel::new(1.0, 1.0, 1.0);
        assert_eq!(px.to_srgb8(), [255, 255, 255]);
    }

    #[test]
    fn test_linear_pixel_to_10bit_white() {
        let px = LinearPixel::new(1.0, 1.0, 1.0);
        assert_eq!(px.to_10bit(), [1023, 1023, 1023]);
    }

    #[test]
    fn test_linear_pixel_to_10bit_black() {
        let px = LinearPixel::new(0.0, 0.0, 0.0);
        assert_eq!(px.to_10bit(), [0, 0, 0]);
    }

    #[test]
    fn test_calibration_identity() {
        let cal = PanelCalibrationOffset::identity();
        let px = LinearPixel::new(0.5, 0.5, 0.5);
        let out = cal.apply(px);
        assert!((out.r - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_calibration_brightness() {
        let cal = PanelCalibrationOffset::new(0.0, 0.0, 0.0, 0.5);
        let px = LinearPixel::new(1.0, 1.0, 1.0);
        let out = cal.apply(px);
        assert!((out.r - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_calibration_gain_offset() {
        let cal = PanelCalibrationOffset::new(0.1, -0.1, 0.0, 1.0);
        let px = LinearPixel::new(0.5, 0.5, 0.5);
        let out = cal.apply(px);
        assert!((out.r - 0.6).abs() < 1e-5);
        assert!((out.g - 0.4).abs() < 1e-5);
    }

    #[test]
    fn test_calibration_no_negative_output() {
        let cal = PanelCalibrationOffset::new(-1.0, -1.0, -1.0, 1.0);
        let px = LinearPixel::new(0.0, 0.0, 0.0);
        let out = cal.apply(px);
        assert!(out.r >= 0.0);
        assert!(out.g >= 0.0);
        assert!(out.b >= 0.0);
    }

    #[test]
    fn test_tone_mapping_clip() {
        let mapper = HdrToneMapper::new(1000.0, ToneMappingMode::Clip);
        let px = LinearPixel::new(500.0, 1500.0, 0.0);
        let out = mapper.map(px);
        assert!((out.r - 0.5).abs() < 1e-5);
        assert!((out.g - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_tone_mapping_linear_scale() {
        let mapper = HdrToneMapper::new(1000.0, ToneMappingMode::LinearScale);
        let px = LinearPixel::new(500.0, 1000.0, 0.0);
        let out = mapper.map(px);
        assert!((out.r - 0.5).abs() < 1e-5);
        assert!((out.g - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_tone_mapping_reinhard_positive() {
        let mapper = HdrToneMapper::new(1000.0, ToneMappingMode::Reinhard);
        let px = LinearPixel::new(500.0, 500.0, 500.0);
        let out = mapper.map(px);
        // Reinhard: x/(1+x) where x = 500/1000 = 0.5 → 0.5/1.5 ≈ 0.333
        assert!(out.r > 0.0 && out.r < 1.0);
    }

    #[test]
    fn test_tone_mapping_aces_positive() {
        let mapper = HdrToneMapper::new(1000.0, ToneMappingMode::AcesFilmic);
        let px = LinearPixel::new(100.0, 500.0, 1000.0);
        let out = mapper.map(px);
        assert!(out.r >= 0.0 && out.r <= 1.0);
        assert!(out.g >= 0.0 && out.g <= 1.0);
        assert!(out.b >= 0.0 && out.b <= 1.0);
    }

    #[test]
    fn test_pixel_mapper_panel_cols_rows() {
        let mapper = PixelMapper::new(1920, 1080, 256, 128);
        // ceil(1920/256) = 8, ceil(1080/128) = 9 (since 1080 = 8*128 + 56)
        assert_eq!(mapper.panel_cols(), 8);
        assert_eq!(mapper.panel_rows(), 9);
    }

    #[test]
    fn test_pixel_mapper_global_to_panel() {
        let mapper = PixelMapper::new(512, 256, 256, 128);
        let result = mapper.global_to_panel(300, 150);
        assert!(result.is_some());
        let (col, row, lx, ly) = result.expect("should succeed in test");
        assert_eq!(col, 1);
        assert_eq!(row, 1);
        assert_eq!(lx, 300 - 256);
        assert_eq!(ly, 150 - 128);
    }

    #[test]
    fn test_pixel_mapper_global_to_panel_out_of_bounds() {
        let mapper = PixelMapper::new(512, 256, 256, 128);
        assert!(mapper.global_to_panel(512, 0).is_none());
        assert!(mapper.global_to_panel(0, 256).is_none());
    }

    #[test]
    fn test_pixel_mapper_panel_to_global_roundtrip() {
        let mapper = PixelMapper::new(1024, 512, 256, 128);
        let (gx, gy) = mapper.panel_to_global(2, 1, 50, 30);
        let result = mapper.global_to_panel(gx, gy);
        assert!(result.is_some());
        let (col, row, lx, ly) = result.expect("should succeed in test");
        assert_eq!((col, row, lx, ly), (2, 1, 50, 30));
    }
}
