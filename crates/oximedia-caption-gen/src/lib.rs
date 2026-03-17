//! # oximedia-caption-gen
//!
//! Advanced caption and subtitle generation for the OxiMedia Sovereign Media
//! Framework.
//!
//! This crate provides speech-to-caption alignment with frame-accurate timing,
//! greedy and optimal (Knuth-Plass DP) line-breaking algorithms, WCAG 2.1
//! accessibility compliance checking, and speaker diarization metadata with
//! crosstalk detection — all in pure Rust.
//!
//! ## Modules
//!
//! - [`alignment`] — Word timestamps, transcript segments, segment
//!   merging/splitting, frame alignment, and caption block construction.
//! - [`line_breaking`] — Greedy and optimal line-breaking, reading-speed
//!   helpers (CPS), and line-balance optimisation.
//! - [`wcag`] — WCAG 2.1 compliance checks (1.2.2, 1.2.4, 1.2.6), reading
//!   speed validation, minimum display duration, gap detection, and compliance
//!   scoring.
//! - [`diarization`] — Speaker metadata, turn merging, per-speaker statistics,
//!   crosstalk detection, voice activity ratio, and speaker-to-caption
//!   assignment.
//! - [`multilang`] — Multi-language subtitle support with ISO 639-1 validated
//!   language codes, SRT export, and cross-language timing merge.
//! - [`burn_in`] — Burned-in subtitle rendering onto raw RGBA video frames
//!   using a built-in 8×12 bitmap font.

pub mod alignment;
pub mod burn_in;
pub mod diarization;
pub mod line_breaking;
pub mod multilang;
pub mod wcag;

// ── Re-exports of key public types ──────────────────────────────────────────

pub use alignment::{
    align_to_frames, build_caption_blocks, merge_short_segments, split_long_segments,
    AlignmentError, CaptionBlock, CaptionPosition, TranscriptSegment, WordTimestamp,
};
pub use diarization::{
    assign_speakers_to_blocks, dominant_speaker, format_speaker_label, merge_consecutive_turns,
    speaker_stats, voice_activity_ratio, CrosstalkDetector, DiarizationResult, Speaker,
    SpeakerGender, SpeakerStats, SpeakerTurn,
};
pub use line_breaking::{
    compute_cps, greedy_break, optimal_break, reading_speed_ok, rebalance_lines, LineBalance,
    LineBreakAlgorithm, LineBreakConfig,
};
pub use wcag::{
    check_caption_coverage, check_cps, check_live_latency, check_min_duration, check_sign_language,
    compliance_score, run_all_checks, WcagChecker, WcagLevel, WcagViolation,
};

// ─── Error type ─────────────────────────────────────────────────────────────

/// Errors produced by caption generation operations.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CaptionGenError {
    /// A speech-to-caption alignment operation failed.
    #[error("alignment error: {0}")]
    Alignment(#[from] AlignmentError),

    /// A parameter value is invalid.
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),

    /// A timestamp is invalid (e.g. start >= end).
    #[error("invalid timestamp")]
    InvalidTimestamp,

    /// The transcript is empty and cannot be processed.
    #[error("empty transcript")]
    EmptyTranscript,

    /// Parsing of caption data or configuration failed.
    #[error("parse error: {0}")]
    ParseError(String),
}

pub use burn_in::{BurnInConfig, SubtitleBurnIn, SubtitlePosition};
pub use multilang::{CaptionEntry, LanguageCode, MultiLangCaption, MultiLangCaptionBuilder};
