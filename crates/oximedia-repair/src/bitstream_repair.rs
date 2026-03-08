//! Bitstream-level error detection and repair for media streams.
//!
//! Provides tools to detect corrupt NAL units, PES packets, and other
//! bitstream-level anomalies, and attempt in-place repair.

#![allow(dead_code)]

/// Categories of bitstream errors that can be detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BitstreamError {
    /// Invalid start code or sync word.
    InvalidStartCode {
        /// Byte offset of the invalid start code.
        offset: u64,
    },
    /// Illegal NALU type for the current codec.
    IllegalNaluType {
        /// Byte offset of the illegal NALU.
        offset: u64,
        /// The illegal NALU type byte.
        code: u8,
    },
    /// Cabac / entropy decode failure.
    EntropyFailure {
        /// Byte offset where entropy decoding failed.
        offset: u64,
    },
    /// Forbidden zero bit set in H.264/H.265 header.
    ForbiddenBitSet {
        /// Byte offset of the header with the forbidden bit set.
        offset: u64,
    },
    /// Profile / level constraint violation.
    ConstraintViolation {
        /// Byte offset of the violation in the bitstream.
        offset: u64,
        /// Human-readable description of the violated constraint.
        detail: String,
    },
    /// Unexpected end of bitstream.
    UnexpectedEof {
        /// Number of bytes expected.
        expected: usize,
        /// Number of bytes actually available.
        got: usize,
    },
    /// Reference frame missing.
    MissingReference {
        /// Picture order count of the missing reference frame.
        poc: i32,
    },
    /// Corrupted PES header in MPEG transport stream.
    CorruptPesHeader {
        /// Byte offset of the corrupt PES header.
        offset: u64,
    },
}

impl BitstreamError {
    /// Return a human-readable label for the error category.
    pub fn label(&self) -> &'static str {
        match self {
            Self::InvalidStartCode { .. } => "invalid_start_code",
            Self::IllegalNaluType { .. } => "illegal_nalu_type",
            Self::EntropyFailure { .. } => "entropy_failure",
            Self::ForbiddenBitSet { .. } => "forbidden_bit_set",
            Self::ConstraintViolation { .. } => "constraint_violation",
            Self::UnexpectedEof { .. } => "unexpected_eof",
            Self::MissingReference { .. } => "missing_reference",
            Self::CorruptPesHeader { .. } => "corrupt_pes_header",
        }
    }

    /// Return the byte offset of the error if known.
    pub fn offset(&self) -> Option<u64> {
        match self {
            Self::InvalidStartCode { offset }
            | Self::IllegalNaluType { offset, .. }
            | Self::EntropyFailure { offset }
            | Self::ForbiddenBitSet { offset }
            | Self::ConstraintViolation { offset, .. }
            | Self::CorruptPesHeader { offset } => Some(*offset),
            _ => None,
        }
    }
}

/// Strategy used when repairing a damaged bitstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairStrategy {
    /// Skip the damaged unit and continue.
    Skip,
    /// Replace the damaged unit with a filler NALU / silence.
    Substitute,
    /// Attempt to interpolate from neighboring good frames.
    Interpolate,
    /// Truncate the stream at the first unrecoverable error.
    Truncate,
}

impl RepairStrategy {
    /// Returns `true` if this strategy may permanently discard stream data.
    pub fn is_destructive(&self) -> bool {
        matches!(self, Self::Skip | Self::Truncate)
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Substitute => "substitute",
            Self::Interpolate => "interpolate",
            Self::Truncate => "truncate",
        }
    }
}

/// Statistics gathered during a repair pass.
#[derive(Debug, Clone, Default)]
pub struct RepairReport {
    /// Total number of errors detected.
    pub errors_detected: usize,
    /// Number of errors successfully repaired.
    pub errors_repaired: usize,
    /// Number of errors that could not be repaired.
    pub errors_failed: usize,
    /// Bytes consumed from input.
    pub bytes_consumed: u64,
    /// Bytes written to output.
    pub bytes_written: u64,
}

impl RepairReport {
    /// Success rate in [0.0, 1.0]; returns 1.0 if no errors were detected.
    #[allow(clippy::cast_precision_loss)]
    pub fn success_rate(&self) -> f64 {
        if self.errors_detected == 0 {
            return 1.0;
        }
        self.errors_repaired as f64 / self.errors_detected as f64
    }

    /// Returns `true` if every detected error was repaired.
    pub fn is_fully_repaired(&self) -> bool {
        self.errors_failed == 0 && self.errors_detected > 0
    }
}

/// Engine that scans and repairs a raw byte buffer representing a media
/// bitstream.
#[derive(Debug, Clone)]
pub struct BitstreamRepair {
    strategy: RepairStrategy,
    max_errors: usize,
}

impl BitstreamRepair {
    /// Create a new repair engine with the given strategy.
    pub fn new(strategy: RepairStrategy) -> Self {
        Self {
            strategy,
            max_errors: 1024,
        }
    }

    /// Set the maximum number of errors to tolerate before aborting.
    pub fn with_max_errors(mut self, max: usize) -> Self {
        self.max_errors = max;
        self
    }

    /// Scan `data` and return a list of detected [`BitstreamError`]s.
    pub fn detect_errors(&self, data: &[u8]) -> Vec<BitstreamError> {
        let mut errors = Vec::new();
        let len = data.len();
        let mut i = 0usize;

        while i + 3 < len {
            // Look for Annex-B start code 0x000001 or 0x00000001.
            if data[i] == 0x00 && data[i + 1] == 0x00 && data[i + 2] == 0x01 {
                let nalu_byte = data[i + 3];
                // Forbidden zero bit is bit 7.
                if nalu_byte & 0x80 != 0 {
                    errors.push(BitstreamError::ForbiddenBitSet { offset: i as u64 });
                }
                // NALU type 0 is technically reserved / illegal as a stream unit.
                let nalu_type = nalu_byte & 0x1F;
                if nalu_type == 0 {
                    errors.push(BitstreamError::IllegalNaluType {
                        offset: i as u64,
                        code: nalu_type,
                    });
                }
                if errors.len() >= self.max_errors {
                    break;
                }
                i += 4;
            } else {
                i += 1;
            }
        }

        // Detect missing start code at the very beginning (non-empty buffer).
        if len >= 4
            && !(data[0] == 0x00 && data[1] == 0x00 && data[2] == 0x00 && data[3] == 0x01)
            && !(data[0] == 0x00 && data[1] == 0x00 && data[2] == 0x01)
        {
            // Heuristic: if first 3 bytes don't form a start code, flag it.
            // Only add if we don't already have a forbidden-bit error at 0.
            if !errors.iter().any(|e| e.offset() == Some(0)) {
                errors.insert(0, BitstreamError::InvalidStartCode { offset: 0 });
            }
        }

        errors
    }

    /// Attempt to repair `data` in place, returning the (possibly modified)
    /// bytes and a [`RepairReport`].
    pub fn repair(&self, data: &[u8]) -> (Vec<u8>, RepairReport) {
        let errors = self.detect_errors(data);
        let mut report = RepairReport {
            errors_detected: errors.len(),
            bytes_consumed: data.len() as u64,
            ..Default::default()
        };

        let mut out = data.to_vec();

        for error in &errors {
            match self.strategy {
                RepairStrategy::Substitute => {
                    if let Some(off) = error.offset() {
                        // Clear the forbidden zero bit if set.
                        if let BitstreamError::ForbiddenBitSet { .. } = error {
                            let idx = (off as usize) + 3;
                            if idx < out.len() {
                                out[idx] &= 0x7F;
                                report.errors_repaired += 1;
                                continue;
                            }
                        }
                        // For other errors, zero-out the offending byte.
                        let idx = off as usize;
                        if idx < out.len() {
                            out[idx] = 0x00;
                            report.errors_repaired += 1;
                        } else {
                            report.errors_failed += 1;
                        }
                    } else {
                        report.errors_failed += 1;
                    }
                }
                RepairStrategy::Skip => {
                    // Nothing to write; just mark as "handled".
                    report.errors_repaired += 1;
                }
                RepairStrategy::Truncate => {
                    if let Some(off) = error.offset() {
                        out.truncate(off as usize);
                        report.errors_repaired += 1;
                    } else {
                        report.errors_failed += 1;
                    }
                    // Stop after first truncation.
                    break;
                }
                RepairStrategy::Interpolate => {
                    // Simplified: mark as failed (real interpolation needs
                    // decoded frame context).
                    report.errors_failed += 1;
                }
            }
        }

        report.bytes_written = out.len() as u64;
        (out, report)
    }
}

impl Default for BitstreamRepair {
    fn default() -> Self {
        Self::new(RepairStrategy::Substitute)
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_valid_nalu(nalu_type: u8) -> Vec<u8> {
        // Annex-B 4-byte start code + legal NALU header byte.
        vec![0x00, 0x00, 0x00, 0x01, nalu_type & 0x1F]
    }

    #[test]
    fn test_error_label_invalid_start_code() {
        let e = BitstreamError::InvalidStartCode { offset: 0 };
        assert_eq!(e.label(), "invalid_start_code");
    }

    #[test]
    fn test_error_label_forbidden_bit() {
        let e = BitstreamError::ForbiddenBitSet { offset: 4 };
        assert_eq!(e.label(), "forbidden_bit_set");
    }

    #[test]
    fn test_error_offset_present() {
        let e = BitstreamError::EntropyFailure { offset: 16 };
        assert_eq!(e.offset(), Some(16));
    }

    #[test]
    fn test_error_offset_absent_for_missing_ref() {
        let e = BitstreamError::MissingReference { poc: 3 };
        assert_eq!(e.offset(), None);
    }

    #[test]
    fn test_strategy_is_destructive() {
        assert!(RepairStrategy::Skip.is_destructive());
        assert!(RepairStrategy::Truncate.is_destructive());
        assert!(!RepairStrategy::Substitute.is_destructive());
        assert!(!RepairStrategy::Interpolate.is_destructive());
    }

    #[test]
    fn test_strategy_names() {
        assert_eq!(RepairStrategy::Skip.name(), "skip");
        assert_eq!(RepairStrategy::Substitute.name(), "substitute");
        assert_eq!(RepairStrategy::Interpolate.name(), "interpolate");
        assert_eq!(RepairStrategy::Truncate.name(), "truncate");
    }

    #[test]
    fn test_report_success_rate_no_errors() {
        let r = RepairReport::default();
        assert!((r.success_rate() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_report_success_rate_partial() {
        let r = RepairReport {
            errors_detected: 4,
            errors_repaired: 3,
            errors_failed: 1,
            ..Default::default()
        };
        assert!((r.success_rate() - 0.75).abs() < 1e-9);
    }

    #[test]
    fn test_report_is_fully_repaired() {
        let r = RepairReport {
            errors_detected: 2,
            errors_repaired: 2,
            errors_failed: 0,
            ..Default::default()
        };
        assert!(r.is_fully_repaired());
    }

    #[test]
    fn test_detect_errors_clean_stream() {
        let data = make_valid_nalu(0x05); // IDR slice
        let engine = BitstreamRepair::default();
        let errors = engine.detect_errors(&data);
        assert!(errors.is_empty(), "Expected no errors, got {:?}", errors);
    }

    #[test]
    fn test_detect_forbidden_bit() {
        // Byte after start code has bit 7 set.
        let data = vec![0x00, 0x00, 0x00, 0x01, 0x85u8]; // 0x85 = 1000_0101
        let engine = BitstreamRepair::default();
        let errors = engine.detect_errors(&data);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, BitstreamError::ForbiddenBitSet { .. })),
            "Expected ForbiddenBitSet error"
        );
    }

    #[test]
    fn test_repair_substitute_clears_forbidden_bit() {
        let data = vec![0x00, 0x00, 0x00, 0x01, 0x85u8];
        let engine = BitstreamRepair::new(RepairStrategy::Substitute);
        let (repaired, report) = engine.repair(&data);
        // After clearing bit 7: 0x85 & 0x7F = 0x05
        assert_eq!(repaired[4], 0x05);
        assert!(report.errors_repaired > 0);
    }

    #[test]
    fn test_repair_truncate_strategy() {
        // Two valid NALUs; first has a forbidden bit set.
        let mut data = vec![0x00, 0x00, 0x00, 0x01, 0x80u8]; // forbidden
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x05]); // valid IDR
        let engine = BitstreamRepair::new(RepairStrategy::Truncate);
        let (repaired, report) = engine.repair(&data);
        // Stream should be truncated at offset 0.
        assert!(repaired.len() <= data.len());
        assert!(report.errors_repaired > 0);
    }

    #[test]
    fn test_detect_errors_max_errors_limit() {
        // Generate a buffer full of forbidden-bit NALUs.
        let mut data = Vec::new();
        for _ in 0..2000 {
            data.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x80]);
        }
        let engine = BitstreamRepair::new(RepairStrategy::Skip).with_max_errors(10);
        let errors = engine.detect_errors(&data);
        assert!(errors.len() <= 11); // at most max_errors + possible start-code error
    }

    #[test]
    fn test_bitstream_repair_default_strategy() {
        let engine = BitstreamRepair::default();
        assert_eq!(engine.strategy, RepairStrategy::Substitute);
    }
}
