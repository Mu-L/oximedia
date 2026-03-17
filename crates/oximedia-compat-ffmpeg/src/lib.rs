//! FFmpeg CLI argument compatibility layer for OxiMedia.
//!
//! Parses FFmpeg-style command-line arguments and translates them
//! to OxiMedia `TranscodeConfig` and `FilterGraph` operations.
//!
//! ## Quick Start
//!
//! ```rust
//! use oximedia_compat_ffmpeg::parse_and_translate;
//!
//! let args: Vec<String> = vec![
//!     "-i".into(), "input.mkv".into(),
//!     "-c:v".into(), "libaom-av1".into(),
//!     "-crf".into(), "28".into(),
//!     "-c:a".into(), "libopus".into(),
//!     "output.webm".into(),
//! ];
//!
//! let result = parse_and_translate(&args);
//! for diag in &result.diagnostics {
//!     eprintln!("{}", diag.format_ffmpeg_style("oximedia-ff"));
//! }
//! ```

pub mod arg_parser;
pub mod argument_builder;
pub mod codec_map;
pub mod codec_mapping;
pub mod diagnostics;
pub mod ffprobe;
pub mod filter_graph;
pub mod filter_lex;
pub mod real_world_tests;
pub mod stream_spec;
pub mod translator;

pub use arg_parser::{
    FfmpegArgs, GlobalOptions, InputSpec, MapSpec, OutputSpec, StreamOptions, StreamType,
};
pub use argument_builder::FfmpegArgumentBuilder;
pub use codec_map::{CodecCategory, CodecEntry, CodecMap};
pub use codec_mapping::{CodecMapper, CodecMapping, FormatMapping};
pub use diagnostics::{Diagnostic, DiagnosticKind, DiagnosticSink, TranslationError};
pub use filter_graph::{FilterChain, FilterGraph as AdvancedFilterGraph, FilterGraphNode};
pub use filter_lex::{
    parse_filter_graph, parse_filter_string, parse_filters, FilterGraph, FilterNode, ParsedFilter,
};
pub use stream_spec::{StreamIndex, StreamSpec, StreamType as SpecStreamType};
pub use translator::{
    parse_and_translate, MuxerAction, MuxerOption, TranscodeJob, TranslateResult,
};
