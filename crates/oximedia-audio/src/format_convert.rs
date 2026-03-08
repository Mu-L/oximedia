//! PCM sample-format conversion utilities.
#![allow(dead_code)]

/// PCM sample bit-depth descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleDepth {
    /// 8-bit unsigned integer.
    U8,
    /// 16-bit signed integer (little-endian).
    S16,
    /// 24-bit signed integer packed in 3 bytes (little-endian).
    S24,
    /// 32-bit signed integer.
    S32,
    /// 32-bit IEEE 754 floating point.
    F32,
    /// 64-bit IEEE 754 floating point.
    F64,
}

impl SampleDepth {
    /// Number of bits per sample.
    #[must_use]
    pub fn bit_count(&self) -> u32 {
        match self {
            SampleDepth::U8 => 8,
            SampleDepth::S16 => 16,
            SampleDepth::S24 => 24,
            SampleDepth::S32 => 32,
            SampleDepth::F32 => 32,
            SampleDepth::F64 => 64,
        }
    }

    /// Number of bytes per sample (rounded up to a whole byte).
    #[must_use]
    pub fn byte_size(&self) -> usize {
        match self {
            SampleDepth::U8 => 1,
            SampleDepth::S16 => 2,
            SampleDepth::S24 => 3,
            SampleDepth::S32 => 4,
            SampleDepth::F32 => 4,
            SampleDepth::F64 => 8,
        }
    }

    /// Returns `true` when this format uses floating-point samples.
    #[must_use]
    pub fn is_float(&self) -> bool {
        matches!(self, SampleDepth::F32 | SampleDepth::F64)
    }
}

/// Stateless sample-format conversion functions.
pub struct FormatConvert;

impl FormatConvert {
    /// Convert a 24-bit signed integer (stored as the lower 3 bytes of a `u32`,
    /// little-endian byte order in `buf`) to normalised `f32` in `[-1, 1]`.
    ///
    /// # Panics
    /// Panics (in debug) when `buf.len() < 3`.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn s24_to_f32(buf: &[u8]) -> f32 {
        debug_assert!(buf.len() >= 3, "s24_to_f32: need at least 3 bytes");
        // Reconstruct the 24-bit two's-complement value.
        let raw = (buf[0] as i32) | ((buf[1] as i32) << 8) | ((buf[2] as i32) << 16);
        // Sign-extend from 24 to 32 bits.
        let signed = if raw & 0x80_0000 != 0 {
            raw | !0xFF_FFFF
        } else {
            raw
        };
        signed as f32 / 8_388_607.0 // 2^23 - 1
    }

    /// Convert normalised `f32` in `[-1, 1]` to a 16-bit signed integer.
    ///
    /// Values outside the normalised range are clamped.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn f32_to_s16(sample: f32) -> i16 {
        let clamped = sample.clamp(-1.0, 1.0);
        (clamped * 32_767.0) as i16
    }

    /// Convert a slice of normalised `f32` to 16-bit signed integers.
    #[must_use]
    pub fn f32_slice_to_s16(samples: &[f32]) -> Vec<i16> {
        samples.iter().map(|&s| Self::f32_to_s16(s)).collect()
    }

    /// Convert a slice of 16-bit signed integers to normalised `f32`.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn s16_slice_to_f32(samples: &[i16]) -> Vec<f32> {
        samples.iter().map(|&s| s as f32 / 32_768.0).collect()
    }
}

/// Stateful format converter that remembers source and destination formats.
#[derive(Debug, Clone)]
pub struct FormatConverter {
    /// Source sample depth.
    pub src: SampleDepth,
    /// Destination sample depth.
    pub dst: SampleDepth,
}

impl FormatConverter {
    /// Create a new converter.
    #[must_use]
    pub fn new(src: SampleDepth, dst: SampleDepth) -> Self {
        Self { src, dst }
    }

    /// Convert a raw byte buffer from `src` format to a `Vec<f32>`.
    ///
    /// Currently supports F32→F32 (identity) and S16→F32 and S24→F32 paths.
    /// Returns an empty `Vec` for unsupported conversions.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn convert_buffer(&self, input: &[u8]) -> Vec<f32> {
        match self.src {
            SampleDepth::F32 => {
                // Re-interpret bytes as f32 samples.
                input
                    .chunks_exact(4)
                    .map(|c| {
                        let arr = [c[0], c[1], c[2], c[3]];
                        f32::from_le_bytes(arr)
                    })
                    .collect()
            }
            SampleDepth::S16 => input
                .chunks_exact(2)
                .map(|c| {
                    let v = i16::from_le_bytes([c[0], c[1]]);
                    v as f32 / 32_768.0
                })
                .collect(),
            SampleDepth::S24 => input
                .chunks_exact(3)
                .map(|c| FormatConvert::s24_to_f32(c))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Byte size of the output buffer given `n_samples` samples.
    #[must_use]
    pub fn output_byte_size(&self, n_samples: usize) -> usize {
        n_samples * self.dst.byte_size()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- SampleDepth ---

    #[test]
    fn test_bit_count_u8() {
        assert_eq!(SampleDepth::U8.bit_count(), 8);
    }

    #[test]
    fn test_bit_count_s24() {
        assert_eq!(SampleDepth::S24.bit_count(), 24);
    }

    #[test]
    fn test_bit_count_f64() {
        assert_eq!(SampleDepth::F64.bit_count(), 64);
    }

    #[test]
    fn test_byte_size_s16() {
        assert_eq!(SampleDepth::S16.byte_size(), 2);
    }

    #[test]
    fn test_byte_size_s24() {
        assert_eq!(SampleDepth::S24.byte_size(), 3);
    }

    #[test]
    fn test_is_float_f32() {
        assert!(SampleDepth::F32.is_float());
    }

    #[test]
    fn test_is_float_s16_false() {
        assert!(!SampleDepth::S16.is_float());
    }

    // --- FormatConvert ---

    #[test]
    fn test_s24_to_f32_positive() {
        // 0x7FFFFF = 8_388_607 ≈ max positive
        let buf = [0xFF_u8, 0xFF, 0x7F];
        let v = FormatConvert::s24_to_f32(&buf);
        assert!((v - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_s24_to_f32_zero() {
        let buf = [0x00_u8, 0x00, 0x00];
        let v = FormatConvert::s24_to_f32(&buf);
        assert!(v.abs() < 1e-10);
    }

    #[test]
    fn test_f32_to_s16_positive() {
        let v = FormatConvert::f32_to_s16(1.0);
        assert_eq!(v, 32_767);
    }

    #[test]
    fn test_f32_to_s16_negative() {
        let v = FormatConvert::f32_to_s16(-1.0);
        assert_eq!(v, -32_767);
    }

    #[test]
    fn test_f32_to_s16_zero() {
        let v = FormatConvert::f32_to_s16(0.0);
        assert_eq!(v, 0);
    }

    #[test]
    fn test_f32_to_s16_clamped() {
        let v = FormatConvert::f32_to_s16(2.0);
        assert_eq!(v, 32_767);
    }

    #[test]
    fn test_roundtrip_s16() {
        let originals: Vec<f32> = vec![0.0, 0.5, -0.5, 0.9, -0.9];
        let s16 = FormatConvert::f32_slice_to_s16(&originals);
        let back = FormatConvert::s16_slice_to_f32(&s16);
        for (orig, rt) in originals.iter().zip(back.iter()) {
            // tolerance ~1/32768
            assert!((orig - rt).abs() < 0.0001, "orig={orig} rt={rt}");
        }
    }

    // --- FormatConverter ---

    #[test]
    fn test_output_byte_size() {
        let conv = FormatConverter::new(SampleDepth::S16, SampleDepth::F32);
        assert_eq!(conv.output_byte_size(100), 400);
    }

    #[test]
    fn test_convert_buffer_f32_identity() {
        let samples: Vec<f32> = vec![0.5, -0.5, 0.0];
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let conv = FormatConverter::new(SampleDepth::F32, SampleDepth::F32);
        let out = conv.convert_buffer(&bytes);
        assert_eq!(out.len(), 3);
        assert!((out[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_convert_buffer_s16_to_f32() {
        let samples: Vec<i16> = vec![16384, -16384]; // ≈ 0.5, -0.5
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let conv = FormatConverter::new(SampleDepth::S16, SampleDepth::F32);
        let out = conv.convert_buffer(&bytes);
        assert_eq!(out.len(), 2);
        assert!((out[0] - 0.5).abs() < 0.001);
        assert!((out[1] + 0.5).abs() < 0.001);
    }
}
