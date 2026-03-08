#![allow(dead_code)]
//! Electro-optical / opto-electronic transfer functions for common colour spaces.

/// All supported transfer functions (EOTFs / OETFs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferFunction {
    /// Identity (no encoding).
    Linear,
    /// BT.1886 / traditional CRT gamma 2.2.
    Gamma22,
    /// sYCC / AdobeRGB gamma 2.4 approximation.
    Gamma24,
    /// IEC 61966-2-1 sRGB piecewise transfer function.
    Srgb,
    /// SMPTE ST 2084 Perceptual Quantizer (HDR10 / Dolby Vision).
    Pq,
    /// ITU-R BT.2100 Hybrid Log-Gamma.
    Hlg,
}

impl TransferFunction {
    /// Encode a normalised linear light value into a non-linear signal value.
    ///
    /// `linear` is in [0, 1] (relative scene luminance).
    /// The returned value is the electrical/code signal, clamped to [0, 1] for SDR
    /// functions.  PQ returns values in [0, 1] where 1 = 10 000 cd/m².
    pub fn encode(&self, linear: f64) -> f64 {
        let v = linear.max(0.0);
        match self {
            TransferFunction::Linear => v,
            TransferFunction::Gamma22 => v.powf(1.0 / 2.2).min(1.0),
            TransferFunction::Gamma24 => v.powf(1.0 / 2.4).min(1.0),
            TransferFunction::Srgb => srgb_encode(v),
            TransferFunction::Pq => pq_encode(v),
            TransferFunction::Hlg => hlg_encode(v),
        }
    }

    /// Decode a non-linear signal value back to normalised linear light.
    ///
    /// `nonlinear` should be in [0, 1].
    pub fn decode(&self, nonlinear: f64) -> f64 {
        let v = nonlinear.clamp(0.0, 1.0);
        match self {
            TransferFunction::Linear => v,
            TransferFunction::Gamma22 => v.powf(2.2),
            TransferFunction::Gamma24 => v.powf(2.4),
            TransferFunction::Srgb => srgb_decode(v),
            TransferFunction::Pq => pq_decode(v),
            TransferFunction::Hlg => hlg_decode(v),
        }
    }

    /// Return `true` if this is an HDR transfer function.
    pub fn is_hdr(&self) -> bool {
        matches!(self, TransferFunction::Pq | TransferFunction::Hlg)
    }

    /// Return a human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            TransferFunction::Linear => "Linear",
            TransferFunction::Gamma22 => "Gamma 2.2",
            TransferFunction::Gamma24 => "Gamma 2.4",
            TransferFunction::Srgb => "sRGB",
            TransferFunction::Pq => "PQ (ST 2084)",
            TransferFunction::Hlg => "HLG (BT.2100)",
        }
    }
}

// ── sRGB ──────────────────────────────────────────────────────────────────

fn srgb_encode(linear: f64) -> f64 {
    if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
    .min(1.0)
}

fn srgb_decode(nonlinear: f64) -> f64 {
    if nonlinear <= 0.040_45 {
        nonlinear / 12.92
    } else {
        ((nonlinear + 0.055) / 1.055).powf(2.4)
    }
}

// ── PQ (SMPTE ST 2084) ────────────────────────────────────────────────────
// Normalised so that `linear = 1.0` maps to 10 000 cd/m².

const PQ_M1: f64 = 2610.0 / 16384.0;
const PQ_M2: f64 = 2523.0 / 4096.0 * 128.0;
const PQ_C1: f64 = 3424.0 / 4096.0;
const PQ_C2: f64 = 2413.0 / 4096.0 * 32.0;
const PQ_C3: f64 = 2392.0 / 4096.0 * 32.0;

fn pq_encode(linear: f64) -> f64 {
    let y = linear.powf(PQ_M1);
    let num = PQ_C1 + PQ_C2 * y;
    let den = 1.0 + PQ_C3 * y;
    (num / den).powf(PQ_M2)
}

fn pq_decode(nonlinear: f64) -> f64 {
    let e = nonlinear.powf(1.0 / PQ_M2);
    let num = (e - PQ_C1).max(0.0);
    let den = PQ_C2 - PQ_C3 * e;
    if den <= 0.0 {
        return 0.0;
    }
    (num / den).powf(1.0 / PQ_M1)
}

// ── HLG (BT.2100) ─────────────────────────────────────────────────────────

const HLG_A: f64 = 0.178_832_77;
const HLG_B: f64 = 0.284_668_92;
const HLG_C: f64 = 0.559_910_73;

fn hlg_encode(linear: f64) -> f64 {
    if linear <= 1.0 / 12.0 {
        (3.0 * linear).sqrt()
    } else {
        HLG_A * (12.0 * linear - HLG_B).ln() + HLG_C
    }
}

fn hlg_decode(nonlinear: f64) -> f64 {
    if nonlinear <= 0.5 {
        (nonlinear * nonlinear) / 3.0
    } else {
        ((nonlinear - HLG_C).exp() / HLG_A + HLG_B) / 12.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_encode_decode_identity() {
        let v = 0.42;
        assert!((TransferFunction::Linear.encode(v) - v).abs() < 1e-10);
        assert!((TransferFunction::Linear.decode(v) - v).abs() < 1e-10);
    }

    #[test]
    fn test_gamma22_encode_mid() {
        let enc = TransferFunction::Gamma22.encode(0.5);
        let expected = 0.5_f64.powf(1.0 / 2.2);
        assert!((enc - expected).abs() < 1e-10);
    }

    #[test]
    fn test_gamma22_roundtrip() {
        let v = 0.3;
        let enc = TransferFunction::Gamma22.encode(v);
        let dec = TransferFunction::Gamma22.decode(enc);
        assert!((dec - v).abs() < 1e-9);
    }

    #[test]
    fn test_gamma24_roundtrip() {
        let v = 0.7;
        let enc = TransferFunction::Gamma24.encode(v);
        let dec = TransferFunction::Gamma24.decode(enc);
        assert!((dec - v).abs() < 1e-9);
    }

    #[test]
    fn test_srgb_encode_low() {
        // Linear value in sRGB linear region
        let enc = TransferFunction::Srgb.encode(0.001);
        assert!((enc - 0.001 * 12.92).abs() < 1e-9);
    }

    #[test]
    fn test_srgb_roundtrip() {
        for v in [0.0, 0.01, 0.18, 0.5, 0.9, 1.0] {
            let enc = TransferFunction::Srgb.encode(v);
            let dec = TransferFunction::Srgb.decode(enc);
            assert!((dec - v).abs() < 1e-9, "sRGB roundtrip failed for v={v}");
        }
    }

    #[test]
    fn test_pq_encode_one_maps_to_one() {
        // linear=1.0 corresponds to 10 000 cd/m², PQ code = 1.0
        let enc = TransferFunction::Pq.encode(1.0);
        assert!((enc - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_pq_encode_zero() {
        let enc = TransferFunction::Pq.encode(0.0);
        assert!(enc >= 0.0 && enc < 0.01);
    }

    #[test]
    fn test_pq_roundtrip() {
        let v = 0.01; // ~100 cd/m² normalised
        let enc = TransferFunction::Pq.encode(v);
        let dec = TransferFunction::Pq.decode(enc);
        assert!((dec - v).abs() < 1e-8, "PQ roundtrip failed: got {dec}");
    }

    #[test]
    fn test_hlg_roundtrip() {
        let v = 0.08;
        let enc = TransferFunction::Hlg.encode(v);
        let dec = TransferFunction::Hlg.decode(enc);
        assert!((dec - v).abs() < 1e-9, "HLG roundtrip failed: got {dec}");
    }

    #[test]
    fn test_is_hdr_true() {
        assert!(TransferFunction::Pq.is_hdr());
        assert!(TransferFunction::Hlg.is_hdr());
    }

    #[test]
    fn test_is_hdr_false() {
        assert!(!TransferFunction::Linear.is_hdr());
        assert!(!TransferFunction::Gamma22.is_hdr());
        assert!(!TransferFunction::Srgb.is_hdr());
        assert!(!TransferFunction::Gamma24.is_hdr());
    }

    #[test]
    fn test_names_non_empty() {
        for tf in [
            TransferFunction::Linear,
            TransferFunction::Gamma22,
            TransferFunction::Gamma24,
            TransferFunction::Srgb,
            TransferFunction::Pq,
            TransferFunction::Hlg,
        ] {
            assert!(!tf.name().is_empty());
        }
    }

    #[test]
    fn test_encode_clamps_negative() {
        // Negative input should be treated as 0
        let enc = TransferFunction::Srgb.encode(-0.5);
        assert!(enc >= 0.0);
    }
}
