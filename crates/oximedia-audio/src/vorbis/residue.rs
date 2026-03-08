//! Residue encoding for Vorbis.
//!
//! Residue encoding handles the spectral details after floor encoding.
//! Vorbis uses vector quantization (VQ) with codebooks to compress
//! the residue data efficiently.

#![forbid(unsafe_code)]

use super::bitpack::BitPacker;
use crate::AudioResult;

/// Residue encoder for Vorbis.
#[derive(Debug, Clone)]
pub struct ResidueEncoder {
    /// Quality level.
    quality: f32,
    /// Quantization step size.
    quant_step: f32,
}

impl ResidueEncoder {
    /// Create new residue encoder.
    ///
    /// # Arguments
    ///
    /// * `quality` - Quality level (-1.0 to 10.0)
    #[must_use]
    pub fn new(quality: f32) -> Self {
        let quant_step = Self::quality_to_quant_step(quality);

        Self {
            quality,
            quant_step,
        }
    }

    /// Convert quality to quantization step size.
    fn quality_to_quant_step(quality: f32) -> f32 {
        // Higher quality = smaller step size = finer quantization
        let q = quality.clamp(-1.0, 10.0);
        let normalized = (10.0 - q) / 11.0; // 0.0 (highest) to 1.0 (lowest)
        0.01 + normalized * 0.99 // Step size from 0.01 to 1.0
    }

    /// Encode residue coefficients.
    ///
    /// # Arguments
    ///
    /// * `packer` - Bitstream packer
    /// * `coeffs` - Residue coefficients (after floor division)
    ///
    /// # Errors
    ///
    /// Returns error if encoding fails.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn encode(&self, packer: &mut BitPacker, coeffs: &[f32]) -> AudioResult<()> {
        // Simplified residue encoding using scalar quantization
        // Real Vorbis uses VQ with codebooks

        let n = coeffs.len() / 2; // Only encode first N/2 coefficients

        // Encode using run-length encoding of quantized values
        let mut run_length = 0;
        let mut prev_value = 0i32;

        for &coeff in coeffs.iter().take(n) {
            let quantized = Self::quantize(coeff, self.quant_step);

            if quantized == prev_value {
                run_length += 1;
            } else {
                // Encode previous run
                if run_length > 0 {
                    self.encode_run(packer, run_length)?;
                }

                // Encode new value
                self.encode_value(packer, quantized)?;

                prev_value = quantized;
                run_length = 1;
            }
        }

        // Encode final run
        if run_length > 0 {
            self.encode_run(packer, run_length)?;
        }

        Ok(())
    }

    /// Quantize a coefficient value.
    #[allow(clippy::cast_possible_truncation)]
    fn quantize(value: f32, step: f32) -> i32 {
        (value / step).round() as i32
    }

    /// Dequantize a coefficient value.
    #[allow(clippy::cast_precision_loss)]
    fn dequantize(quantized: i32, step: f32) -> f32 {
        quantized as f32 * step
    }

    /// Encode a run length.
    #[allow(clippy::cast_possible_truncation)]
    fn encode_run(&self, packer: &mut BitPacker, run_length: usize) -> AudioResult<()> {
        // Simple run-length encoding
        if run_length < 4 {
            // Short run: encode directly
            packer.write_bits(0, 2); // Short run flag
            packer.write_bits(run_length as u32, 2);
        } else {
            // Long run: use more bits
            packer.write_bits(1, 2); // Long run flag
            packer.write_bits((run_length - 4) as u32, 8);
        }
        Ok(())
    }

    /// Encode a quantized value.
    #[allow(clippy::cast_sign_loss)]
    fn encode_value(&self, packer: &mut BitPacker, value: i32) -> AudioResult<()> {
        // Encode sign and magnitude separately
        if value == 0 {
            packer.write_bits(0, 1); // Zero flag
        } else {
            packer.write_bits(1, 1); // Non-zero flag
            packer.write_bits(if value < 0 { 1 } else { 0 }, 1); // Sign

            let magnitude = value.unsigned_abs();
            let bits = Self::magnitude_bits(magnitude);
            packer.write_bits(bits as u32, 4); // Bit count
            packer.write_bits(magnitude, bits as u8);
        }
        Ok(())
    }

    /// Determine number of bits needed for magnitude.
    fn magnitude_bits(magnitude: u32) -> usize {
        if magnitude == 0 {
            0
        } else {
            32 - magnitude.leading_zeros() as usize
        }
    }

    /// Compute rate-distortion optimization.
    ///
    /// Determines optimal quantization for each coefficient based on
    /// perceptual importance and bit budget.
    #[allow(dead_code)]
    pub fn rate_distortion_optimize(&self, coeffs: &[f32], _bit_budget: usize) -> Vec<i32> {
        // Simplified: just quantize all coefficients uniformly
        coeffs
            .iter()
            .map(|&c| Self::quantize(c, self.quant_step))
            .collect()
    }

    /// Compute noise allocation.
    ///
    /// Determines how much quantization noise to allow in each frequency band
    /// based on psychoacoustic masking.
    #[allow(dead_code)]
    pub fn compute_noise_allocation(&self, _masking: &[f32]) -> Vec<f32> {
        // Simplified: uniform allocation
        vec![self.quant_step; _masking.len()]
    }

    /// Get quality level.
    #[must_use]
    pub const fn quality(&self) -> f32 {
        self.quality
    }

    /// Get quantization step size.
    #[must_use]
    pub const fn quant_step(&self) -> f32 {
        self.quant_step
    }
}

/// Residue type 0: Interleaved vector quantization.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ResidueType0 {
    /// Begin frequency.
    begin: u32,
    /// End frequency.
    end: u32,
    /// Partition size.
    partition_size: u32,
    /// Number of classifications.
    classifications: u8,
    /// Classbook number.
    classbook: u8,
}

impl ResidueType0 {
    /// Create new residue type 0.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            begin: 0,
            end: 256,
            partition_size: 32,
            classifications: 4,
            classbook: 0,
        }
    }
}

impl Default for ResidueType0 {
    fn default() -> Self {
        Self::new()
    }
}

/// Residue type 1: Distinct vector quantization.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ResidueType1 {
    /// Begin frequency.
    begin: u32,
    /// End frequency.
    end: u32,
    /// Partition size.
    partition_size: u32,
    /// Number of classifications.
    classifications: u8,
    /// Classbook number.
    classbook: u8,
}

impl ResidueType1 {
    /// Create new residue type 1.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            begin: 0,
            end: 256,
            partition_size: 32,
            classifications: 4,
            classbook: 0,
        }
    }
}

impl Default for ResidueType1 {
    fn default() -> Self {
        Self::new()
    }
}

/// Residue type 2: Multidimensional vector quantization.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ResidueType2 {
    /// Begin frequency.
    begin: u32,
    /// End frequency.
    end: u32,
    /// Partition size.
    partition_size: u32,
    /// Number of classifications.
    classifications: u8,
    /// Classbook number.
    classbook: u8,
}

impl ResidueType2 {
    /// Create new residue type 2.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            begin: 0,
            end: 256,
            partition_size: 32,
            classifications: 4,
            classbook: 0,
        }
    }
}

impl Default for ResidueType2 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_residue_encoder_creation() {
        let encoder = ResidueEncoder::new(5.0);
        assert_eq!(encoder.quality(), 5.0);
        assert!(encoder.quant_step() > 0.0);
    }

    #[test]
    fn test_quality_to_quant_step() {
        let step_low = ResidueEncoder::quality_to_quant_step(-1.0);
        let step_mid = ResidueEncoder::quality_to_quant_step(5.0);
        let step_high = ResidueEncoder::quality_to_quant_step(10.0);

        // Higher quality should have smaller step
        assert!(step_high < step_mid);
        assert!(step_mid < step_low);
    }

    #[test]
    fn test_quantize() {
        let step = 0.1;
        assert_eq!(ResidueEncoder::quantize(0.5, step), 5);
        assert_eq!(ResidueEncoder::quantize(-0.5, step), -5);
        assert_eq!(ResidueEncoder::quantize(0.05, step), 1);
    }

    #[test]
    fn test_dequantize() {
        let step = 0.1;
        let value = 5;
        let dequant = ResidueEncoder::dequantize(value, step);
        assert!((dequant - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_magnitude_bits() {
        assert_eq!(ResidueEncoder::magnitude_bits(0), 0);
        assert_eq!(ResidueEncoder::magnitude_bits(1), 1);
        assert_eq!(ResidueEncoder::magnitude_bits(2), 2);
        assert_eq!(ResidueEncoder::magnitude_bits(7), 3);
        assert_eq!(ResidueEncoder::magnitude_bits(8), 4);
        assert_eq!(ResidueEncoder::magnitude_bits(255), 8);
    }

    #[test]
    fn test_encode_residue() {
        let encoder = ResidueEncoder::new(5.0);
        let mut packer = BitPacker::new();
        let coeffs = vec![0.1, 0.2, 0.3, 0.2, 0.1];

        assert!(encoder.encode(&mut packer, &coeffs).is_ok());
        assert!(packer.size() > 0);
    }

    #[test]
    fn test_residue_type0() {
        let residue = ResidueType0::new();
        assert_eq!(residue.begin, 0);
        assert_eq!(residue.end, 256);
    }

    #[test]
    fn test_residue_type1() {
        let residue = ResidueType1::new();
        assert_eq!(residue.partition_size, 32);
    }

    #[test]
    fn test_residue_type2() {
        let residue = ResidueType2::new();
        assert_eq!(residue.classifications, 4);
    }
}
