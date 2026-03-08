//! Tone mapping operators for HDR to SDR conversion.

/// Tone mapping operator selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToneMappingOperator {
    /// Reinhard tone mapping (simple global operator)
    Reinhard,
    /// Modified Reinhard with white point
    ReinhardExtended,
    /// Hable (Uncharted 2) filmic tone mapping
    Hable,
    /// ACES filmic tone mapping
    Aces,
    /// Simple linear clamp
    Linear,
    /// Custom curve
    Custom,
}

/// Tone mapper for converting HDR to SDR.
#[derive(Clone, Debug)]
pub struct ToneMapper {
    operator: ToneMappingOperator,
    peak_input_nits: f64,
    peak_output_nits: f64,
    white_point: f64,
}

impl ToneMapper {
    /// Creates a new tone mapper.
    ///
    /// # Arguments
    ///
    /// * `operator` - Tone mapping operator to use
    /// * `peak_input_nits` - Peak luminance of input HDR content
    /// * `peak_output_nits` - Peak luminance of output display (typically 100 for SDR)
    #[must_use]
    pub const fn new(
        operator: ToneMappingOperator,
        peak_input_nits: f64,
        peak_output_nits: f64,
    ) -> Self {
        Self {
            operator,
            peak_input_nits,
            peak_output_nits,
            white_point: 1.0,
        }
    }

    /// Sets the white point for extended Reinhard.
    pub fn set_white_point(&mut self, white_point: f64) {
        self.white_point = white_point;
    }

    /// Applies tone mapping to linear HDR RGB.
    #[must_use]
    pub fn apply(&self, hdr_rgb: [f64; 3]) -> [f64; 3] {
        // Normalize to [0, 1] range where 1.0 = peak_output_nits
        let scale = self.peak_output_nits / self.peak_input_nits;
        let normalized = [hdr_rgb[0] * scale, hdr_rgb[1] * scale, hdr_rgb[2] * scale];

        match self.operator {
            ToneMappingOperator::Reinhard => apply_reinhard(normalized),
            ToneMappingOperator::ReinhardExtended => {
                apply_reinhard_extended(normalized, self.white_point)
            }
            ToneMappingOperator::Hable => apply_hable(normalized),
            ToneMappingOperator::Aces => apply_aces(normalized),
            ToneMappingOperator::Linear => apply_linear(normalized),
            ToneMappingOperator::Custom => normalized, // No-op, user provides custom
        }
    }
}

impl Default for ToneMapper {
    fn default() -> Self {
        Self::new(ToneMappingOperator::Aces, 1000.0, 100.0)
    }
}

/// Reinhard tone mapping operator.
///
/// Formula: `L_out` = `L_in` / (1 + `L_in`)
///
/// Simple and efficient, but can make highlights look flat.
#[must_use]
fn apply_reinhard(rgb: [f64; 3]) -> [f64; 3] {
    [
        rgb[0] / (1.0 + rgb[0]),
        rgb[1] / (1.0 + rgb[1]),
        rgb[2] / (1.0 + rgb[2]),
    ]
}

/// Extended Reinhard tone mapping with white point.
///
/// Formula: `L_out` = `L_in` * (1 + `L_in/L_white^2`) / (1 + `L_in`)
#[must_use]
fn apply_reinhard_extended(rgb: [f64; 3], white_point: f64) -> [f64; 3] {
    let white_sq = white_point * white_point;
    let map = |v: f64| (v * (1.0 + v / white_sq)) / (1.0 + v);

    [map(rgb[0]), map(rgb[1]), map(rgb[2])]
}

/// Hable (Uncharted 2) filmic tone mapping.
///
/// Popular filmic curve with good highlight handling.
#[must_use]
fn apply_hable(rgb: [f64; 3]) -> [f64; 3] {
    const EXPOSURE_BIAS: f64 = 2.0;
    let curr = hable_partial(
        rgb[0] * EXPOSURE_BIAS,
        rgb[1] * EXPOSURE_BIAS,
        rgb[2] * EXPOSURE_BIAS,
    );
    let white = hable_partial(11.2, 11.2, 11.2);

    [curr[0] / white[0], curr[1] / white[1], curr[2] / white[2]]
}

#[must_use]
fn hable_partial(r: f64, g: f64, b: f64) -> [f64; 3] {
    const A: f64 = 0.15;
    const B: f64 = 0.50;
    const C: f64 = 0.10;
    const D: f64 = 0.20;
    const E: f64 = 0.02;
    const F: f64 = 0.30;

    let map = |x: f64| ((x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F)) - E / F;

    [map(r), map(g), map(b)]
}

/// ACES filmic tone mapping.
///
/// Industry-standard filmic curve from ACES.
#[must_use]
fn apply_aces(rgb: [f64; 3]) -> [f64; 3] {
    let map = |x: f64| {
        const A: f64 = 2.51;
        const B: f64 = 0.03;
        const C: f64 = 2.43;
        const D: f64 = 0.59;
        const E: f64 = 0.14;

        ((x * (A * x + B)) / (x * (C * x + D) + E)).clamp(0.0, 1.0)
    };

    [map(rgb[0]), map(rgb[1]), map(rgb[2])]
}

/// Linear tone mapping (simple clamp).
#[must_use]
fn apply_linear(rgb: [f64; 3]) -> [f64; 3] {
    [
        rgb[0].clamp(0.0, 1.0),
        rgb[1].clamp(0.0, 1.0),
        rgb[2].clamp(0.0, 1.0),
    ]
}

/// Applies a smooth knee function for soft clipping.
///
/// This is useful for highlight protection and smooth rolloff.
#[must_use]
#[allow(dead_code)]
pub fn apply_soft_knee(rgb: [f64; 3], knee_start: f64, knee_end: f64) -> [f64; 3] {
    let apply_channel = |v: f64| -> f64 {
        if v < knee_start {
            v
        } else if v < knee_end {
            // Smooth knee using cosine
            let t = (v - knee_start) / (knee_end - knee_start);
            let smooth_t = (1.0 - (t * std::f64::consts::PI).cos()) / 2.0;
            knee_start + (knee_end - knee_start) * smooth_t
        } else {
            knee_end
        }
    };

    [
        apply_channel(rgb[0]),
        apply_channel(rgb[1]),
        apply_channel(rgb[2]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reinhard_black_and_white() {
        let black = apply_reinhard([0.0, 0.0, 0.0]);
        assert_eq!(black, [0.0, 0.0, 0.0]);

        // High values should approach 1.0
        let bright = apply_reinhard([100.0, 100.0, 100.0]);
        assert!(bright[0] > 0.99);
    }

    #[test]
    fn test_tone_mapper() {
        let mapper = ToneMapper::new(ToneMappingOperator::Aces, 1000.0, 100.0);
        let hdr = [10.0, 5.0, 2.0]; // Bright HDR values
        let sdr = mapper.apply(hdr);

        // Should be mapped to [0, 1]
        assert!(sdr[0] >= 0.0 && sdr[0] <= 1.0);
        assert!(sdr[1] >= 0.0 && sdr[1] <= 1.0);
        assert!(sdr[2] >= 0.0 && sdr[2] <= 1.0);

        // Brighter values should map to brighter outputs
        assert!(sdr[0] > sdr[1]);
        assert!(sdr[1] > sdr[2]);
    }

    #[test]
    fn test_hable_tone_mapping() {
        let rgb = [0.5, 1.0, 2.0];
        let result = apply_hable(rgb);

        // All values should be in [0, 1]
        assert!(result[0] >= 0.0 && result[0] <= 1.0);
        assert!(result[1] >= 0.0 && result[1] <= 1.0);
        assert!(result[2] >= 0.0 && result[2] <= 1.0);
    }

    #[test]
    fn test_aces_tone_mapping() {
        let rgb = [0.5, 1.0, 2.0];
        let result = apply_aces(rgb);

        // All values should be in [0, 1]
        assert!(result[0] >= 0.0 && result[0] <= 1.0);
        assert!(result[1] >= 0.0 && result[1] <= 1.0);
        assert!(result[2] >= 0.0 && result[2] <= 1.0);

        // Should preserve order
        assert!(result[0] < result[1]);
        assert!(result[1] < result[2]);
    }

    #[test]
    fn test_linear_tone_mapping() {
        let rgb = [0.5, 1.5, -0.5];
        let result = apply_linear(rgb);

        assert_eq!(result, [0.5, 1.0, 0.0]);
    }
}
