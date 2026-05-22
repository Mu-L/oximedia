// Copyright 2025 OxiMedia Contributors
// Licensed under the Apache License, Version 2.0

//! Error-path integration tests for oximedia-convert subcommands.
//!
//! These tests verify that each subcommand returns the correct error variant
//! for invalid inputs — missing files, out-of-range timestamps, empty
//! collections, etc. — without requiring real media fixtures.

use oximedia_convert::{
    audio::extract::{AudioExtractor, AudioFormat},
    frame::extract::FrameExtractor,
    split::{chapter::ChapterSplitter, time::TimeSplitter},
    subtitle::{
        convert::{SubtitleConverter, SubtitleFormat},
        extract::SubtitleExtractor,
    },
    thumbnail::generate::ThumbnailGenerator,
    video::{extract::VideoExtractor, mute::VideoMuter},
    ConversionError,
};
use std::time::Duration;

// ── Frame extractor ──────────────────────────────────────────────────────────

#[tokio::test]
async fn frame_extract_at_missing_file() {
    let result = FrameExtractor::new()
        .extract_at(
            std::env::temp_dir().join("__oximedia_test_missing_frame__.mkv"),
            std::env::temp_dir().join("__oximedia_test_missing_frame_out__.png"),
            1.0,
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidInput(_))),
        "expected InvalidInput, got {result:?}"
    );
}

#[tokio::test]
async fn frame_extract_at_negative_timestamp() {
    let tmp = std::env::temp_dir().join("oximedia_ep_frame_neg.mkv");
    std::fs::write(&tmp, b"dummy").unwrap();
    let result = FrameExtractor::new()
        .extract_at(
            &tmp,
            std::env::temp_dir().join("ep_frame_neg_out.png"),
            -5.0,
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidTimestamp)),
        "expected InvalidTimestamp, got {result:?}"
    );
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn frame_extract_multiple_negative_timestamp() {
    let tmp = std::env::temp_dir().join("oximedia_ep_frame_multi_neg.mkv");
    std::fs::write(&tmp, b"dummy").unwrap();
    let result = FrameExtractor::new()
        .extract_multiple(&tmp, std::env::temp_dir(), &[1.0, -2.0, 3.0])
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidTimestamp)),
        "expected InvalidTimestamp, got {result:?}"
    );
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn frame_extract_multiple_empty_times_returns_empty() {
    let tmp = std::env::temp_dir().join("oximedia_ep_frame_empty.mkv");
    std::fs::write(&tmp, b"dummy").unwrap();
    let result = FrameExtractor::new()
        .extract_multiple(&tmp, std::env::temp_dir(), &[])
        .await;
    assert!(
        matches!(result, Ok(ref v) if v.is_empty()),
        "expected empty Vec, got {result:?}"
    );
    let _ = std::fs::remove_file(&tmp);
}

// ── Chapter splitter ─────────────────────────────────────────────────────────

#[tokio::test]
async fn chapter_split_missing_file() {
    let result = ChapterSplitter::new()
        .split(
            std::env::temp_dir().join("__oximedia_test_missing_chapter__.mkv"),
            std::env::temp_dir().join("__oximedia_test_missing_chapter_dir__"),
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::Io(_))),
        "expected Io, got {result:?}"
    );
}

#[test]
fn chapter_split_no_chapters_returns_empty_list() {
    // `list_chapters` on any file currently returns empty (demux not yet
    // integrated).
    let tmp = std::env::temp_dir().join("oximedia_ep_chapter_empty.mkv");
    std::fs::write(&tmp, b"dummy").unwrap();
    let splitter = ChapterSplitter::new();
    let chapters = splitter.list_chapters(&tmp).unwrap();
    assert!(chapters.is_empty(), "expected empty chapter list");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn chapter_from_durations_sum_correct() {
    let splitter = ChapterSplitter::new();
    let durations = [60.0_f64, 90.0, 30.0];
    let chapters = splitter.chapters_from_durations(&[], &durations);
    let total: f64 = chapters.iter().map(|c| c.duration()).sum();
    assert!((total - 180.0).abs() < 1e-9);
}

// ── Time splitter ────────────────────────────────────────────────────────────

#[tokio::test]
async fn time_split_missing_file() {
    let result = TimeSplitter::new(Duration::from_secs(60))
        .split(
            std::env::temp_dir().join("__oximedia_test_missing_time__.mkv"),
            std::env::temp_dir().join("__oximedia_test_missing_time_dir__"),
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::Io(_))),
        "expected Io, got {result:?}"
    );
}

#[test]
fn time_split_zero_duration_boundaries_error() {
    let splitter = TimeSplitter::new(Duration::ZERO);
    let result = splitter.calculate_segment_boundaries(120.0);
    assert!(
        matches!(result, Err(ConversionError::InvalidInput(_))),
        "expected InvalidInput, got {result:?}"
    );
}

#[test]
fn time_split_boundaries_contiguous() {
    let splitter = TimeSplitter::new(Duration::from_secs(30));
    let bounds = splitter.calculate_segment_boundaries(100.0).unwrap();
    for w in bounds.windows(2) {
        assert!(
            (w[0].1 - w[1].0).abs() < 1e-9,
            "boundaries must be contiguous"
        );
    }
}

// ── Audio extractor ──────────────────────────────────────────────────────────

#[tokio::test]
async fn audio_extract_missing_file() {
    let result = AudioExtractor::new()
        .extract(
            std::env::temp_dir().join("__oximedia_test_missing_audio__.mkv"),
            std::env::temp_dir().join("__oximedia_test_missing_audio_out__.wav"),
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidInput(_))),
        "expected InvalidInput, got {result:?}"
    );
}

#[tokio::test]
async fn audio_extract_segment_invalid_timestamp() {
    let tmp = std::env::temp_dir().join("oximedia_ep_audio_neg.wav");
    std::fs::write(&tmp, b"RIFF").unwrap();
    let result = AudioExtractor::new()
        .extract_segment(
            &tmp,
            std::env::temp_dir().join("oximedia_ep_audio_neg_out.wav"),
            -2.0,
            5.0,
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidTimestamp)),
        "expected InvalidTimestamp, got {result:?}"
    );
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn audio_format_lossless() {
    assert!(AudioFormat::Wav.is_lossless());
    assert!(AudioFormat::Flac.is_lossless());
    assert!(!AudioFormat::Opus.is_lossless());
}

// ── Video extractor ──────────────────────────────────────────────────────────

#[tokio::test]
async fn video_extract_missing_file() {
    let result = VideoExtractor::new()
        .extract(
            std::env::temp_dir().join("__oximedia_test_missing_video_ep__.mkv"),
            std::env::temp_dir().join("__oximedia_test_missing_video_ep_out__.mkv"),
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidInput(_))),
        "expected InvalidInput, got {result:?}"
    );
}

#[tokio::test]
async fn video_extract_segment_negative_timestamp() {
    let tmp = std::env::temp_dir().join("oximedia_ep_video_neg_ts.mkv");
    std::fs::write(&tmp, b"dummy").unwrap();
    let result = VideoExtractor::new()
        .extract_segment(
            &tmp,
            std::env::temp_dir().join("oximedia_ep_video_neg_ts_out.mkv"),
            -1.0,
            5.0,
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidTimestamp)),
        "expected InvalidTimestamp, got {result:?}"
    );
    let _ = std::fs::remove_file(&tmp);
}

// ── Video muter ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn video_muter_missing_file() {
    let result = VideoMuter::new()
        .mute(
            std::env::temp_dir().join("__oximedia_test_missing_mute_ep__.mkv"),
            std::env::temp_dir().join("__oximedia_test_missing_mute_ep_out__.mkv"),
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidInput(_))),
        "expected InvalidInput, got {result:?}"
    );
}

#[tokio::test]
async fn video_muter_empty_track_list() {
    let tmp = std::env::temp_dir().join("oximedia_ep_muter_empty_tracks.mkv");
    std::fs::write(&tmp, b"dummy").unwrap();
    let result = VideoMuter::new()
        .mute_tracks(
            &tmp,
            std::env::temp_dir().join("oximedia_ep_muter_empty_tracks_out.mkv"),
            &[],
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidInput(_))),
        "expected InvalidInput for empty track list, got {result:?}"
    );
    let _ = std::fs::remove_file(&tmp);
}

// ── Thumbnail generator ──────────────────────────────────────────────────────

#[tokio::test]
async fn thumbnail_generate_missing_file() {
    let result = ThumbnailGenerator::new()
        .generate(
            std::env::temp_dir().join("__oximedia_test_missing_thumb_ep__.mkv"),
            std::env::temp_dir().join("__oximedia_test_missing_thumb_ep_out__.jpg"),
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidInput(_))),
        "expected InvalidInput, got {result:?}"
    );
}

#[tokio::test]
async fn thumbnail_generate_zero_size() {
    let tmp = std::env::temp_dir().join("oximedia_ep_thumb_zero.mkv");
    std::fs::write(&tmp, b"dummy").unwrap();
    let result = ThumbnailGenerator::new()
        .with_size(0, 0)
        .generate(&tmp, std::env::temp_dir().join("ep_thumb_zero_out.jpg"))
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidInput(_))),
        "expected InvalidInput for zero size, got {result:?}"
    );
    let _ = std::fs::remove_file(&tmp);
}

#[tokio::test]
async fn thumbnail_generate_at_negative_time() {
    let tmp = std::env::temp_dir().join("oximedia_ep_thumb_neg.mkv");
    std::fs::write(&tmp, b"dummy").unwrap();
    let result = ThumbnailGenerator::new()
        .generate_at(
            &tmp,
            std::env::temp_dir().join("ep_thumb_neg_out.jpg"),
            -3.0,
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidTimestamp)),
        "expected InvalidTimestamp, got {result:?}"
    );
    let _ = std::fs::remove_file(&tmp);
}

// ── Subtitle extractor ───────────────────────────────────────────────────────

#[tokio::test]
async fn subtitle_extract_missing_file() {
    let result = SubtitleExtractor::new()
        .extract(
            std::env::temp_dir().join("__oximedia_test_missing_sub_ep__.srt"),
            std::env::temp_dir().join("__oximedia_test_missing_sub_ep_out__.srt"),
            0,
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::Io(_))),
        "expected Io, got {result:?}"
    );
}

#[tokio::test]
async fn subtitle_extract_container_format_unsupported() {
    let tmp = std::env::temp_dir().join("oximedia_ep_sub_container.mkv");
    std::fs::write(&tmp, b"dummy").unwrap();
    let result = SubtitleExtractor::new()
        .extract(
            &tmp,
            std::env::temp_dir().join("ep_sub_container_out.srt"),
            0,
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::UnsupportedFormat(_))),
        "expected UnsupportedFormat for container, got {result:?}"
    );
    let _ = std::fs::remove_file(&tmp);
}

// ── Subtitle converter ───────────────────────────────────────────────────────

#[tokio::test]
async fn subtitle_convert_missing_input() {
    let result = SubtitleConverter::new()
        .convert(
            std::env::temp_dir().join("__oximedia_test_missing_sub_conv__.srt"),
            std::env::temp_dir().join("__oximedia_test_missing_sub_conv_out__.vtt"),
            SubtitleFormat::WebVtt,
        )
        .await;
    assert!(
        matches!(result, Err(ConversionError::InvalidInput(_))),
        "expected InvalidInput, got {result:?}"
    );
}

#[tokio::test]
async fn subtitle_convert_srt_to_vtt_roundtrip() {
    use std::io::Write;
    let tmp_dir = std::env::temp_dir();
    let srt = tmp_dir.join("oximedia_ep_conv_rt.srt");
    let vtt = tmp_dir.join("oximedia_ep_conv_rt.vtt");

    {
        let mut f = std::fs::File::create(&srt).unwrap();
        f.write_all(
            b"1\n00:00:01,000 --> 00:00:04,000\nLine one.\n\n\
              2\n00:00:05,000 --> 00:00:08,000\nLine two.\n\n",
        )
        .unwrap();
    }

    let count = SubtitleConverter::new()
        .convert(&srt, &vtt, SubtitleFormat::WebVtt)
        .await
        .expect("conversion should succeed");

    assert_eq!(count, 2, "expected 2 events");
    let text = std::fs::read_to_string(&vtt).unwrap();
    assert!(text.starts_with("WEBVTT"));
    assert!(text.contains("Line one."));
    assert!(text.contains("Line two."));

    let _ = std::fs::remove_file(&srt);
    let _ = std::fs::remove_file(&vtt);
}
