#![allow(dead_code)]
//! LUT chain management – compose multiple LUTs into an ordered pipeline.
//!
//! Provides:
//! * [`LutChainEntry`] – a single stage (1-D or 3-D LUT) with identity detection.
//! * [`LutChain`]      – an ordered sequence of entries with pipeline application.
//! * [`LutChainValidator`] – structural validation of a chain before use.

use crate::Rgb;

// ---------------------------------------------------------------------------
// LutChainEntry
// ---------------------------------------------------------------------------

/// The kind of LUT stored in a chain entry.
#[derive(Debug, Clone, PartialEq)]
pub enum LutEntryKind {
    /// 1-D per-channel curve.  Each channel has `size` values in `[0, 1]`.
    Lut1d {
        /// Number of entries per channel.
        size: usize,
        /// Interleaved R/G/B values: `[r0, g0, b0, r1, g1, b1, …]`.
        data: Vec<f64>,
    },
    /// 3-D lattice LUT with `size³` RGB entries.
    Lut3d {
        /// Number of divisions per axis.
        size: usize,
        /// Flat lattice, row-major `[r][g][b]`.
        data: Vec<Rgb>,
    },
}

/// A single stage in a [`LutChain`].
#[derive(Debug, Clone)]
pub struct LutChainEntry {
    /// Human-readable label (e.g. filename or step name).
    pub label: String,
    /// The LUT data for this stage.
    pub kind: LutEntryKind,
}

impl LutChainEntry {
    /// Create a 1-D chain entry.
    #[must_use]
    pub fn new_1d(label: impl Into<String>, size: usize, data: Vec<f64>) -> Self {
        Self {
            label: label.into(),
            kind: LutEntryKind::Lut1d { size, data },
        }
    }

    /// Create a 3-D chain entry.
    #[must_use]
    pub fn new_3d(label: impl Into<String>, size: usize, data: Vec<Rgb>) -> Self {
        Self {
            label: label.into(),
            kind: LutEntryKind::Lut3d { size, data },
        }
    }

    /// Returns `true` when this entry performs no colour transformation.
    ///
    /// A 1-D entry is identity when every value equals its normalised position.
    /// A 3-D entry is identity when every lattice point equals its lattice index.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        const EPS: f64 = 1e-6;
        match &self.kind {
            LutEntryKind::Lut1d { size, data } => {
                if *size == 0 || data.len() != size * 3 {
                    return false;
                }
                let scale = (*size - 1) as f64;
                for i in 0..*size {
                    let expected = i as f64 / scale;
                    if (data[i * 3] - expected).abs() > EPS
                        || (data[i * 3 + 1] - expected).abs() > EPS
                        || (data[i * 3 + 2] - expected).abs() > EPS
                    {
                        return false;
                    }
                }
                true
            }
            LutEntryKind::Lut3d { size, data } => {
                if *size < 2 || data.len() != size * size * size {
                    return false;
                }
                let scale = (*size - 1) as f64;
                for r in 0..*size {
                    for g in 0..*size {
                        for b in 0..*size {
                            let idx = r * size * size + g * size + b;
                            let exp = [r as f64 / scale, g as f64 / scale, b as f64 / scale];
                            if (data[idx][0] - exp[0]).abs() > EPS
                                || (data[idx][1] - exp[1]).abs() > EPS
                                || (data[idx][2] - exp[2]).abs() > EPS
                            {
                                return false;
                            }
                        }
                    }
                }
                true
            }
        }
    }

    /// Apply this entry to a single RGB pixel using trilinear interpolation for
    /// 3-D and linear interpolation for 1-D.
    #[must_use]
    pub fn apply_rgb(&self, pixel: Rgb) -> Rgb {
        match &self.kind {
            LutEntryKind::Lut1d { size, data } => apply_1d(pixel, *size, data),
            LutEntryKind::Lut3d { size, data } => apply_3d_trilinear(pixel, *size, data),
        }
    }
}

// ---------------------------------------------------------------------------
// LutChain
// ---------------------------------------------------------------------------

/// An ordered sequence of LUT stages applied left-to-right.
#[derive(Debug, Clone, Default)]
pub struct LutChain {
    entries: Vec<LutChainEntry>,
}

impl LutChain {
    /// Create an empty chain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an entry to the end of the chain.
    pub fn push(&mut self, entry: LutChainEntry) {
        self.entries.push(entry);
    }

    /// Number of stages in the chain.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.entries.len()
    }

    /// Apply every stage in sequence to `pixel`.
    #[must_use]
    pub fn apply_rgb(&self, mut pixel: Rgb) -> Rgb {
        for entry in &self.entries {
            pixel = entry.apply_rgb(pixel);
        }
        pixel
    }

    /// Returns a reference to the entry at `index`, or `None`.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&LutChainEntry> {
        self.entries.get(index)
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &LutChainEntry> {
        self.entries.iter()
    }

    /// Returns `true` when every entry in the chain is an identity.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.entries.iter().all(LutChainEntry::is_identity)
    }
}

// ---------------------------------------------------------------------------
// LutChainValidator
// ---------------------------------------------------------------------------

/// Validation error returned by [`LutChainValidator::validate`].
#[derive(Debug, Clone, PartialEq)]
pub enum ChainValidationError {
    /// Chain is empty.
    Empty,
    /// Entry at `index` has an invalid data length.
    InvalidDataLength {
        /// Position of the invalid entry in the chain.
        index: usize,
        /// Label of the invalid entry.
        label: String,
    },
    /// Entry at `index` has a size that is too small.
    SizeTooSmall {
        /// Position of the undersized entry in the chain.
        index: usize,
        /// Label of the undersized entry.
        label: String,
    },
}

impl std::fmt::Display for ChainValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "LUT chain is empty"),
            Self::InvalidDataLength { index, label } => {
                write!(f, "Entry {index} '{label}': data length mismatch")
            }
            Self::SizeTooSmall { index, label } => {
                write!(f, "Entry {index} '{label}': size must be >= 2")
            }
        }
    }
}

/// Validates a [`LutChain`] before use.
pub struct LutChainValidator;

impl LutChainValidator {
    /// Validate all entries in `chain`.
    ///
    /// Returns `Ok(())` when every entry has consistent dimensions, or the
    /// first [`ChainValidationError`] encountered.
    pub fn validate(chain: &LutChain) -> Result<(), ChainValidationError> {
        if chain.depth() == 0 {
            return Err(ChainValidationError::Empty);
        }
        for (i, entry) in chain.iter().enumerate() {
            match &entry.kind {
                LutEntryKind::Lut1d { size, data } => {
                    if *size < 2 {
                        return Err(ChainValidationError::SizeTooSmall {
                            index: i,
                            label: entry.label.clone(),
                        });
                    }
                    if data.len() != size * 3 {
                        return Err(ChainValidationError::InvalidDataLength {
                            index: i,
                            label: entry.label.clone(),
                        });
                    }
                }
                LutEntryKind::Lut3d { size, data } => {
                    if *size < 2 {
                        return Err(ChainValidationError::SizeTooSmall {
                            index: i,
                            label: entry.label.clone(),
                        });
                    }
                    if data.len() != size * size * size {
                        return Err(ChainValidationError::InvalidDataLength {
                            index: i,
                            label: entry.label.clone(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Linear interpolation along each channel of a 1-D LUT.
fn apply_1d(pixel: Rgb, size: usize, data: &[f64]) -> Rgb {
    if size < 2 || data.len() < size * 3 {
        return pixel;
    }
    let scale = (size - 1) as f64;
    let mut out = [0.0f64; 3];
    for ch in 0..3 {
        let v = pixel[ch].clamp(0.0, 1.0) * scale;
        let lo = v.floor() as usize;
        let hi = (lo + 1).min(size - 1);
        let t = v - lo as f64;
        out[ch] = data[lo * 3 + ch] * (1.0 - t) + data[hi * 3 + ch] * t;
    }
    out
}

/// Trilinear interpolation on a 3-D LUT (size³ entries, row-major `[r][g][b]`).
fn apply_3d_trilinear(pixel: Rgb, size: usize, data: &[Rgb]) -> Rgb {
    if size < 2 || data.len() < size * size * size {
        return pixel;
    }
    let scale = (size - 1) as f64;
    let rv = pixel[0].clamp(0.0, 1.0) * scale;
    let gv = pixel[1].clamp(0.0, 1.0) * scale;
    let bv = pixel[2].clamp(0.0, 1.0) * scale;

    let r0 = rv.floor() as usize;
    let g0 = gv.floor() as usize;
    let b0 = bv.floor() as usize;
    let r1 = (r0 + 1).min(size - 1);
    let g1 = (g0 + 1).min(size - 1);
    let b1 = (b0 + 1).min(size - 1);

    let tr = rv - r0 as f64;
    let tg = gv - g0 as f64;
    let tb = bv - b0 as f64;

    macro_rules! idx {
        ($r:expr, $g:expr, $b:expr) => {
            $r * size * size + $g * size + $b
        };
    }

    let c000 = data[idx!(r0, g0, b0)];
    let c001 = data[idx!(r0, g0, b1)];
    let c010 = data[idx!(r0, g1, b0)];
    let c011 = data[idx!(r0, g1, b1)];
    let c100 = data[idx!(r1, g0, b0)];
    let c101 = data[idx!(r1, g0, b1)];
    let c110 = data[idx!(r1, g1, b0)];
    let c111 = data[idx!(r1, g1, b1)];

    let mut out = [0.0f64; 3];
    for ch in 0..3 {
        out[ch] = c000[ch] * (1.0 - tr) * (1.0 - tg) * (1.0 - tb)
            + c001[ch] * (1.0 - tr) * (1.0 - tg) * tb
            + c010[ch] * (1.0 - tr) * tg * (1.0 - tb)
            + c011[ch] * (1.0 - tr) * tg * tb
            + c100[ch] * tr * (1.0 - tg) * (1.0 - tb)
            + c101[ch] * tr * (1.0 - tg) * tb
            + c110[ch] * tr * tg * (1.0 - tb)
            + c111[ch] * tr * tg * tb;
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_identity_1d(size: usize) -> LutChainEntry {
        let scale = (size - 1) as f64;
        let data: Vec<f64> = (0..size)
            .flat_map(|i| {
                let v = i as f64 / scale;
                [v, v, v]
            })
            .collect();
        LutChainEntry::new_1d("id1d", size, data)
    }

    fn make_identity_3d(size: usize) -> LutChainEntry {
        let scale = (size - 1) as f64;
        let mut data = Vec::with_capacity(size * size * size);
        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    data.push([r as f64 / scale, g as f64 / scale, b as f64 / scale]);
                }
            }
        }
        LutChainEntry::new_3d("id3d", size, data)
    }

    #[test]
    fn test_identity_1d_detected() {
        let e = make_identity_1d(17);
        assert!(e.is_identity());
    }

    #[test]
    fn test_non_identity_1d() {
        let mut e = make_identity_1d(5);
        if let LutEntryKind::Lut1d { data, .. } = &mut e.kind {
            data[3] = 0.999; // shift red channel of index 1
        }
        assert!(!e.is_identity());
    }

    #[test]
    fn test_identity_3d_detected() {
        let e = make_identity_3d(5);
        assert!(e.is_identity());
    }

    #[test]
    fn test_non_identity_3d() {
        let mut e = make_identity_3d(5);
        if let LutEntryKind::Lut3d { data, .. } = &mut e.kind {
            data[1][0] += 0.1;
        }
        assert!(!e.is_identity());
    }

    #[test]
    fn test_apply_1d_identity_passthrough() {
        let e = make_identity_1d(33);
        let pixel = [0.2, 0.5, 0.8];
        let out = e.apply_rgb(pixel);
        assert!((out[0] - 0.2).abs() < 1e-4);
        assert!((out[1] - 0.5).abs() < 1e-4);
        assert!((out[2] - 0.8).abs() < 1e-4);
    }

    #[test]
    fn test_apply_3d_identity_passthrough() {
        let e = make_identity_3d(17);
        let pixel = [0.3, 0.6, 0.9];
        let out = e.apply_rgb(pixel);
        assert!((out[0] - 0.3).abs() < 1e-4);
        assert!((out[1] - 0.6).abs() < 1e-4);
        assert!((out[2] - 0.9).abs() < 1e-4);
    }

    #[test]
    fn test_chain_depth() {
        let mut chain = LutChain::new();
        assert_eq!(chain.depth(), 0);
        chain.push(make_identity_1d(17));
        assert_eq!(chain.depth(), 1);
        chain.push(make_identity_3d(17));
        assert_eq!(chain.depth(), 2);
    }

    #[test]
    fn test_chain_apply_two_identities() {
        let mut chain = LutChain::new();
        chain.push(make_identity_1d(17));
        chain.push(make_identity_3d(17));
        let pixel = [0.4, 0.5, 0.6];
        let out = chain.apply_rgb(pixel);
        assert!((out[0] - 0.4).abs() < 1e-3);
        assert!((out[1] - 0.5).abs() < 1e-3);
        assert!((out[2] - 0.6).abs() < 1e-3);
    }

    #[test]
    fn test_chain_is_identity_all() {
        let mut chain = LutChain::new();
        chain.push(make_identity_1d(9));
        chain.push(make_identity_3d(9));
        assert!(chain.is_identity());
    }

    #[test]
    fn test_chain_is_not_identity_when_one_differs() {
        let mut chain = LutChain::new();
        chain.push(make_identity_1d(9));
        let mut e = make_identity_3d(9);
        if let LutEntryKind::Lut3d { data, .. } = &mut e.kind {
            data[0][0] = 0.5;
        }
        chain.push(e);
        assert!(!chain.is_identity());
    }

    #[test]
    fn test_validator_empty_chain() {
        let chain = LutChain::new();
        assert_eq!(
            LutChainValidator::validate(&chain),
            Err(ChainValidationError::Empty)
        );
    }

    #[test]
    fn test_validator_valid_chain() {
        let mut chain = LutChain::new();
        chain.push(make_identity_3d(17));
        assert!(LutChainValidator::validate(&chain).is_ok());
    }

    #[test]
    fn test_validator_size_too_small() {
        let entry = LutChainEntry::new_3d("bad", 1, vec![[0.0, 0.0, 0.0]]);
        let mut chain = LutChain::new();
        chain.push(entry);
        assert!(matches!(
            LutChainValidator::validate(&chain),
            Err(ChainValidationError::SizeTooSmall { .. })
        ));
    }

    #[test]
    fn test_validator_data_length_mismatch_3d() {
        let entry = LutChainEntry::new_3d("bad", 3, vec![[0.0, 0.0, 0.0]; 5]);
        let mut chain = LutChain::new();
        chain.push(entry);
        assert!(matches!(
            LutChainValidator::validate(&chain),
            Err(ChainValidationError::InvalidDataLength { .. })
        ));
    }

    #[test]
    fn test_get_entry() {
        let mut chain = LutChain::new();
        chain.push(make_identity_1d(5));
        assert!(chain.get(0).is_some());
        assert!(chain.get(1).is_none());
    }

    #[test]
    fn test_entry_label() {
        let e = LutChainEntry::new_1d("my_lut", 5, vec![0.0; 15]);
        assert_eq!(e.label, "my_lut");
    }
}
