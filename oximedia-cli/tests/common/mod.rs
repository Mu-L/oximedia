//! Shared test helpers for oximedia-cli integration tests.

use std::path::PathBuf;
use tempfile::TempDir;

/// Build a minimal valid WAV file in memory (44 bytes header + PCM data).
///
/// Generates a pure sine wave at `freq_hz` with the given sample rate,
/// channel count, and duration.
pub fn make_sine_wav(freq_hz: f32, sample_rate: u32, channels: u16, duration_secs: f32) -> Vec<u8> {
    let num_samples = (sample_rate as f32 * duration_secs) as u32;
    let num_channels = u32::from(channels);
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels * u32::from(bits_per_sample / 8);
    let block_align = channels * (bits_per_sample / 8);
    let data_size = num_samples * num_channels * u32::from(bits_per_sample / 8);
    let file_size = 36 + data_size;

    let mut buf = Vec::with_capacity(44 + data_size as usize);
    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());
    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    // PCM samples — sine wave
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (2.0 * std::f32::consts::PI * freq_hz * t).sin();
        let pcm = (sample * 32767.0) as i16;
        for _ch in 0..channels {
            buf.extend_from_slice(&pcm.to_le_bytes());
        }
    }
    buf
}

/// Write a WAV file to a temp dir and return `(TempDir, path)`.
///
/// The `TempDir` must be kept alive for the duration of the test.
pub fn write_wav_fixture(
    freq_hz: f32,
    sample_rate: u32,
    channels: u16,
    duration_secs: f32,
) -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("failed to create TempDir");
    let path = dir.path().join("test.wav");
    let data = make_sine_wav(freq_hz, sample_rate, channels, duration_secs);
    std::fs::write(&path, data).expect("failed to write WAV fixture");
    (dir, path)
}
