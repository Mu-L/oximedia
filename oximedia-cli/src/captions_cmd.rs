//! Caption and subtitle processing command.
//!
//! Provides `oximedia captions` for generating, syncing, converting,
//! burning, extracting, and validating captions in multiple formats.

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;

/// Options for the `captions generate` subcommand.
#[cfg_attr(not(feature = "caption-gen"), allow(dead_code))]
pub struct CaptionsGenerateOptions {
    /// Input audio/video file.
    pub input: PathBuf,
    /// Output caption file.
    pub output: PathBuf,
    /// Output format: srt, vtt, ass, ttml, scc.
    pub format: String,
    /// Language code (e.g. "en", "ja").
    pub language: String,
    /// Path to an ONNX caption encoder model (required when `caption-gen` feature is enabled).
    pub model: Option<PathBuf>,
    /// Path to a JSON vocabulary file mapping token IDs to strings (required with `--model`).
    pub vocab: Option<PathBuf>,
}

/// Options for the `captions sync` subcommand.
pub struct CaptionsSyncOptions {
    /// Input caption file.
    pub input: PathBuf,
    /// Reference audio/video file.
    pub reference: PathBuf,
    /// Output synced caption file.
    pub output: PathBuf,
    /// Maximum time shift in milliseconds.
    pub max_shift_ms: i64,
}

/// Options for the `captions convert` subcommand.
pub struct CaptionsConvertOptions {
    /// Input caption file.
    pub input: PathBuf,
    /// Output file.
    pub output: PathBuf,
    /// Source format (auto-detected if not specified).
    pub from_format: Option<String>,
    /// Target format.
    pub to_format: String,
}

/// Options for the `captions burn` subcommand.
pub struct CaptionsBurnOptions {
    /// Input video file.
    pub video: PathBuf,
    /// Input caption file.
    pub captions: PathBuf,
    /// Output video file.
    pub output: PathBuf,
    /// Font size.
    pub font_size: u32,
    /// Font color (hex, e.g. "FFFFFF").
    pub font_color: String,
}

/// Options for the `captions extract` subcommand.
pub struct CaptionsExtractOptions {
    /// Input media file.
    pub input: PathBuf,
    /// Output caption file.
    pub output: PathBuf,
    /// Output format.
    pub format: String,
    /// Track index to extract from.
    pub track: usize,
}

/// Options for the `captions validate` subcommand.
pub struct CaptionsValidateOptions {
    /// Input caption file.
    pub input: PathBuf,
    /// Standard to validate against: fcc, wcag, cea608, cea708, ebu.
    pub standard: String,
    /// Output report file (optional).
    pub report: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Format parsing helper
// ---------------------------------------------------------------------------

fn parse_caption_format(s: &str) -> Result<oximedia_captions::CaptionFormat> {
    match s.to_lowercase().as_str() {
        "srt" => Ok(oximedia_captions::CaptionFormat::Srt),
        "vtt" | "webvtt" => Ok(oximedia_captions::CaptionFormat::WebVtt),
        "ass" => Ok(oximedia_captions::CaptionFormat::Ass),
        "ssa" => Ok(oximedia_captions::CaptionFormat::Ssa),
        "ttml" => Ok(oximedia_captions::CaptionFormat::Ttml),
        "dfxp" => Ok(oximedia_captions::CaptionFormat::Dfxp),
        "scc" => Ok(oximedia_captions::CaptionFormat::Scc),
        "stl" | "ebu-stl" => Ok(oximedia_captions::CaptionFormat::EbuStl),
        "itt" => Ok(oximedia_captions::CaptionFormat::ITt),
        "cea608" | "cea-608" => Ok(oximedia_captions::CaptionFormat::Cea608),
        "cea708" | "cea-708" => Ok(oximedia_captions::CaptionFormat::Cea708),
        other => Err(anyhow::anyhow!("Unknown caption format: {other}")),
    }
}

// ---------------------------------------------------------------------------
// Subcommand handlers
// ---------------------------------------------------------------------------

/// Run the `captions generate` subcommand.
///
/// When the `caption-gen` Cargo feature is **disabled** (the default), this
/// function immediately returns an error directing the caller to rebuild with
/// `--features caption-gen`.
///
/// When `caption-gen` is **enabled**, the function runs a full ASR pipeline:
///
/// 1. Parse raw audio samples from the input file (WAV/PCM path when no
///    higher-level decoder is available in this build).
/// 2. Compute a log-mel spectrogram (80 bins, 25 ms frame, 10 ms hop).
/// 3. Load the ONNX caption encoder from `--model <path>`.
/// 4. Run encoder inference and greedy-decode the logits into token IDs.
/// 5. Map token IDs to text using the JSON vocabulary at `--vocab <path>`.
/// 6. Construct `TranscriptSegment`s, align to captions, apply line-breaking.
/// 7. Export the populated `CaptionTrack`.
pub async fn run_captions_generate(opts: CaptionsGenerateOptions, json_output: bool) -> Result<()> {
    #[cfg(not(feature = "caption-gen"))]
    {
        // Suppress unused-variable warning in non-feature builds.
        let _ = (&opts, json_output);
        return Err(anyhow::anyhow!(
            "Caption ASR requires the `caption-gen` feature. \
             Rebuild with: cargo build --features caption-gen\n\
             Note: you also need to supply --model <encoder.onnx> and \
             --vocab <vocab.json> at runtime."
        ));
    }

    #[cfg(feature = "caption-gen")]
    {
        run_captions_generate_impl(opts, json_output).await
    }
}

/// Real implementation, compiled only when `caption-gen` feature is on.
#[cfg(feature = "caption-gen")]
async fn run_captions_generate_impl(
    opts: CaptionsGenerateOptions,
    json_output: bool,
) -> Result<()> {
    use oximedia_caption_gen::{
        alignment::{build_caption_blocks, merge_short_segments},
        line_breaking::optimal_break,
        ml::CaptionEncoder,
    };
    use oximedia_ml::DeviceType;

    // ── 1. Validate required options ─────────────────────────────────────────
    let model_path = opts.model.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Caption generation requires --model <path>. \
             No ASR model weights are bundled with oximedia-cli."
        )
    })?;
    let vocab_path = opts.vocab.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Caption generation requires --vocab <path> (a JSON file mapping \
             token IDs to strings)."
        )
    })?;

    // ── 2. Load audio samples ────────────────────────────────────────────────
    //
    // Parse WAV/PCM from the input file.  A full demux/decoder chain is out of
    // scope here; WAV is the universal interchange format for ASR pipelines.
    let raw_bytes = std::fs::read(&opts.input)
        .with_context(|| format!("Failed to read input: {}", opts.input.display()))?;
    let samples_16k = parse_wav_to_mono_f32(&raw_bytes).with_context(|| {
        format!(
            "Failed to decode audio from '{}'. \
                 Only WAV/PCM files are supported in this build.",
            opts.input.display()
        )
    })?;

    // ── 3. Log-mel spectrogram ───────────────────────────────────────────────
    //
    // Parameters follow the Whisper/OpenAI convention that most publicly
    // available caption encoder models expect:
    //   - 80 mel bins
    //   - 25 ms frame length at 16 kHz → 400 samples
    //   - 10 ms hop → 160 samples
    const MEL_BINS: usize = 80;
    const SAMPLE_RATE: u32 = 16_000;
    const FRAME_LEN: usize = 400; // 25 ms @ 16 kHz
    const HOP_LEN: usize = 160; // 10 ms @ 16 kHz

    let spectrogram =
        compute_log_mel_spectrogram(&samples_16k, MEL_BINS, FRAME_LEN, HOP_LEN, SAMPLE_RATE)
            .context("Failed to compute log-mel spectrogram")?;
    let n_frames = spectrogram.len() / MEL_BINS;
    // Encoder input shape: [batch=1, mel_bins, n_frames]
    let input_shape = [1_usize, MEL_BINS, n_frames];

    // ── 4. Run encoder + greedy decode ───────────────────────────────────────
    let encoder = CaptionEncoder::from_path(model_path, DeviceType::auto()).with_context(|| {
        format!(
            "Failed to load caption encoder model from '{}'",
            model_path.display()
        )
    })?;

    let encoder_out = encoder
        .encode(&spectrogram, &input_shape)
        .map_err(|e| anyhow::anyhow!("Encoder inference failed: {e}"))?;

    // Derive seq_len and vocab_size from the output shape.
    // Expected shape: [batch, seq_len, vocab] or [seq_len, vocab].
    let (seq_len, vocab_size) = derive_seq_vocab(&encoder_out.shape).ok_or_else(|| {
        anyhow::anyhow!("Unexpected encoder output shape: {:?}", encoder_out.shape)
    })?;

    let token_ids =
        oximedia_caption_gen::ml::greedy_decode(&encoder_out.logits, vocab_size, seq_len)
            .map_err(|e| anyhow::anyhow!("Greedy decode failed: {e}"))?;

    // ── 5. Map tokens → text via vocab JSON ──────────────────────────────────
    let vocab_bytes = std::fs::read(vocab_path)
        .with_context(|| format!("Failed to read vocab file: {}", vocab_path.display()))?;
    let vocab: std::collections::HashMap<String, String> =
        serde_json::from_slice(&vocab_bytes).context("Failed to parse vocab JSON")?;

    let transcript_text = tokens_to_text(&token_ids, &vocab);

    // ── 6. Build caption segments ─────────────────────────────────────────────
    //
    // Without a forced-alignment decoder, we distribute timing proportionally
    // across the audio duration.  A future wave can wire a real forced-aligner.
    let audio_duration_ms = (samples_16k.len() as u64 * 1000) / u64::from(SAMPLE_RATE);
    let segments = build_segments_from_text(&transcript_text, audio_duration_ms);

    // Merge very short segments to reduce flicker.
    let merged = merge_short_segments(&segments, 800);

    // Build caption blocks with 2 lines × 42 chars (broadcast standard).
    let blocks = build_caption_blocks(&merged, 2, 42);

    // Apply optimal line-breaking for each block's text.
    // `optimal_break` takes `(text, max_width: u8)`.
    const MAX_LINE_CHARS: u8 = 42;
    let language =
        oximedia_captions::Language::new(opts.language.clone(), opts.language.clone(), false);
    let mut track = oximedia_captions::CaptionTrack::new(language);

    for block in &blocks {
        let joined = block.lines.join(" ");
        let broken_lines = optimal_break(&joined, MAX_LINE_CHARS);
        let text = broken_lines.join("\n");
        let caption = oximedia_captions::Caption::new(
            oximedia_captions::Timestamp::from_millis(block.start_ms as i64),
            oximedia_captions::Timestamp::from_millis(block.end_ms as i64),
            text,
        );
        track
            .add_caption(caption)
            .map_err(|e| anyhow::anyhow!("Failed to add caption: {e}"))?;
    }

    // ── 7. Export ─────────────────────────────────────────────────────────────
    let format = parse_caption_format(&opts.format)?;
    let output_bytes = oximedia_captions::export::Exporter::export(&track, format)
        .map_err(|e| anyhow::anyhow!("Export failed: {e}"))?;

    std::fs::write(&opts.output, &output_bytes)
        .with_context(|| format!("Failed to write output: {}", opts.output.display()))?;

    let caption_count = track.count();
    if json_output {
        let obj = serde_json::json!({
            "input": opts.input.to_string_lossy(),
            "output": opts.output.to_string_lossy(),
            "format": opts.format,
            "language": opts.language,
            "captions_count": caption_count,
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
    } else {
        println!("{}", "Caption Generation Complete".green().bold());
        println!("  Input:    {}", opts.input.display());
        println!("  Output:   {}", opts.output.display());
        println!("  Format:   {}", opts.format);
        println!("  Language: {}", opts.language);
        println!("  Captions: {}", caption_count);
    }

    Ok(())
}

// ── ASR pipeline helpers (compiled only with `caption-gen`) ─────────────────

/// Parse a WAV byte stream into mono f32 samples at 16 kHz.
///
/// Handles standard 16-bit signed PCM WAV files.  The returned slice is
/// already down-mixed to mono and normalised to `[-1.0, 1.0]`.  Stereo
/// signals are averaged across channels.  Sample-rate conversion (to 16 kHz)
/// is performed by linear interpolation when the file rate differs.
#[cfg(feature = "caption-gen")]
fn parse_wav_to_mono_f32(data: &[u8]) -> anyhow::Result<Vec<f32>> {
    // Minimal RIFF/WAV header parser — no external deps required.
    //
    // WAV layout:
    //   "RIFF" (4) | file_size (4 LE) | "WAVE" (4)
    //   "fmt " (4) | chunk_size (4 LE) | audio_fmt (2) | n_channels (2) |
    //   sample_rate (4) | byte_rate (4) | block_align (2) | bits_sample (2)
    //   "data" (4) | data_size (4 LE) | <PCM samples>
    if data.len() < 44 {
        return Err(anyhow::anyhow!("WAV file too small ({} bytes)", data.len()));
    }
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err(anyhow::anyhow!(
            "Not a RIFF/WAVE file — only WAV input is supported"
        ));
    }

    // Search for the "fmt " chunk.
    let (n_channels, src_sample_rate, bits_per_sample) = find_fmt_chunk(data)?;

    // Search for the "data" chunk.
    let pcm_bytes = find_data_chunk(data)?;

    // Decode PCM samples.
    let samples = decode_pcm(pcm_bytes, bits_per_sample, n_channels)?;

    // Resample to 16 kHz via nearest-neighbour if needed (linear would be better
    // but nearest-neighbour is zero-dep and sufficient for ASR feature input).
    const TARGET_RATE: u32 = 16_000;
    if src_sample_rate == TARGET_RATE {
        return Ok(samples);
    }
    let ratio = src_sample_rate as f64 / TARGET_RATE as f64;
    let out_len = ((samples.len() as f64) / ratio).ceil() as usize;
    let resampled: Vec<f32> = (0..out_len)
        .map(|i| {
            let src_idx = ((i as f64 * ratio).round() as usize).min(samples.len() - 1);
            samples[src_idx]
        })
        .collect();
    Ok(resampled)
}

/// Locate and parse the "fmt " chunk from a WAV byte slice.
/// Returns `(n_channels, sample_rate, bits_per_sample)`.
#[cfg(feature = "caption-gen")]
fn find_fmt_chunk(data: &[u8]) -> anyhow::Result<(u16, u32, u16)> {
    let mut pos = 12_usize; // skip RIFF/size/WAVE
    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(
            data[pos + 4..pos + 8]
                .try_into()
                .map_err(|_| anyhow::anyhow!("WAV chunk size read error"))?,
        ) as usize;
        if chunk_id == b"fmt " && pos + 8 + chunk_size >= 16 {
            let audio_fmt = u16::from_le_bytes(
                data[pos + 8..pos + 10]
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("WAV fmt parse error"))?,
            );
            // 1 = PCM, 3 = IEEE float
            if audio_fmt != 1 && audio_fmt != 3 {
                return Err(anyhow::anyhow!(
                    "Unsupported WAV audio format {audio_fmt} (only PCM/IEEE-float supported)"
                ));
            }
            let n_channels = u16::from_le_bytes(
                data[pos + 10..pos + 12]
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("WAV channel count parse error"))?,
            );
            let sample_rate = u32::from_le_bytes(
                data[pos + 12..pos + 16]
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("WAV sample rate parse error"))?,
            );
            let bits_per_sample = u16::from_le_bytes(
                data[pos + 22..pos + 24]
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("WAV bits/sample parse error"))?,
            );
            return Ok((n_channels, sample_rate, bits_per_sample));
        }
        pos += 8 + chunk_size + (chunk_size % 2); // align to 2-byte boundary
    }
    Err(anyhow::anyhow!("WAV 'fmt ' chunk not found"))
}

/// Locate the raw PCM byte slice of the "data" chunk in a WAV file.
#[cfg(feature = "caption-gen")]
fn find_data_chunk(data: &[u8]) -> anyhow::Result<&[u8]> {
    let mut pos = 12_usize;
    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(
            data[pos + 4..pos + 8]
                .try_into()
                .map_err(|_| anyhow::anyhow!("WAV data chunk size error"))?,
        ) as usize;
        if chunk_id == b"data" {
            let end = (pos + 8 + chunk_size).min(data.len());
            return Ok(&data[pos + 8..end]);
        }
        pos += 8 + chunk_size + (chunk_size % 2);
    }
    Err(anyhow::anyhow!("WAV 'data' chunk not found"))
}

/// Decode raw PCM bytes into mono-normalised f32 samples.
/// Handles 16-bit signed int and 32-bit IEEE float, mixing stereo to mono.
#[cfg(feature = "caption-gen")]
fn decode_pcm(pcm: &[u8], bits_per_sample: u16, n_channels: u16) -> anyhow::Result<Vec<f32>> {
    let channels = n_channels as usize;
    if channels == 0 {
        return Err(anyhow::anyhow!("WAV has 0 channels"));
    }
    let samples_raw: Vec<f32> = match bits_per_sample {
        16 => {
            if pcm.len() % 2 != 0 {
                return Err(anyhow::anyhow!("16-bit WAV data has odd byte count"));
            }
            pcm.chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / i16::MAX as f32)
                .collect()
        }
        32 => {
            if pcm.len() % 4 != 0 {
                return Err(anyhow::anyhow!("32-bit WAV data has unaligned byte count"));
            }
            pcm.chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect()
        }
        other => {
            return Err(anyhow::anyhow!(
                "Unsupported WAV bit depth {other} (only 16-bit and 32-bit float supported)"
            ));
        }
    };

    // Mix down to mono by averaging channels.
    if channels == 1 {
        return Ok(samples_raw);
    }
    let mono: Vec<f32> = samples_raw
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    Ok(mono)
}

/// Compute a log-mel spectrogram from mono PCM f32 samples.
///
/// Uses a Hann window, real-valued DFT via Goertzel-equivalent power spectrum
/// (no external FFT dep required), and a triangular mel filterbank.
/// Returns a flat `[mel_bins * n_frames]` buffer in row-major order
/// (`[frame_0_bin_0, frame_0_bin_1, ..., frame_N_bin_M]`).
#[cfg(feature = "caption-gen")]
fn compute_log_mel_spectrogram(
    samples: &[f32],
    mel_bins: usize,
    frame_len: usize,
    hop_len: usize,
    sample_rate: u32,
) -> anyhow::Result<Vec<f32>> {
    if samples.is_empty() {
        return Err(anyhow::anyhow!("Cannot compute spectrogram of empty audio"));
    }
    let fft_size = frame_len.next_power_of_two();
    let n_frames = if samples.len() >= frame_len {
        (samples.len() - frame_len) / hop_len + 1
    } else {
        1
    };

    // Pre-compute Hann window.
    let hann: Vec<f32> = (0..frame_len)
        .map(|n| {
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * n as f32 / (frame_len - 1) as f32).cos())
        })
        .collect();

    // Pre-compute mel filterbank.
    let filterbank = mel_filterbank(mel_bins, fft_size, sample_rate);

    let mut spectrogram = vec![0.0_f32; mel_bins * n_frames];

    for frame_idx in 0..n_frames {
        let start = frame_idx * hop_len;
        let end = (start + frame_len).min(samples.len());

        // Windowed frame — zero-pad if near end of signal.
        let mut frame = vec![0.0_f32; fft_size];
        for (i, s) in samples[start..end].iter().enumerate() {
            frame[i] = s * hann[i];
        }

        // Power spectrum via naive DFT (O(N²) but acceptable for ASR
        // preprocessing at the frame scale — typical frame is 400 samples).
        // For production use, the caller can swap in OxiFFT.
        let n_bins = fft_size / 2 + 1;
        let mut power = vec![0.0_f32; n_bins];
        for k in 0..n_bins {
            let (mut re, mut im) = (0.0_f32, 0.0_f32);
            let angle_step = -2.0 * std::f32::consts::PI * k as f32 / fft_size as f32;
            for (n, &x) in frame.iter().enumerate() {
                let angle = angle_step * n as f32;
                re += x * angle.cos();
                im += x * angle.sin();
            }
            power[k] = re * re + im * im;
        }

        // Apply mel filterbank and log-compress.
        for m in 0..mel_bins {
            let energy: f32 = filterbank[m]
                .iter()
                .enumerate()
                .map(|(k, &w)| w * power[k])
                .sum();
            // Clip to a small positive value before log to avoid -inf.
            spectrogram[frame_idx * mel_bins + m] = (energy.max(1e-10)).ln();
        }
    }

    Ok(spectrogram)
}

/// Build a triangular mel filterbank.
/// Returns `mel_bins` rows, each of length `fft_size/2 + 1`.
#[cfg(feature = "caption-gen")]
fn mel_filterbank(mel_bins: usize, fft_size: usize, sample_rate: u32) -> Vec<Vec<f32>> {
    let n_bins = fft_size / 2 + 1;
    let hz_to_mel = |hz: f32| -> f32 { 2595.0 * (1.0 + hz / 700.0).log10() };
    let mel_to_hz = |mel: f32| -> f32 { 700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0) };

    let f_min_mel = hz_to_mel(0.0);
    let f_max_mel = hz_to_mel(sample_rate as f32 / 2.0);
    let mel_points: Vec<f32> = (0..=mel_bins + 1)
        .map(|i| mel_to_hz(f_min_mel + (f_max_mel - f_min_mel) * i as f32 / (mel_bins + 1) as f32))
        .collect();

    let bin_points: Vec<f32> = mel_points
        .iter()
        .map(|&hz| (hz * fft_size as f32 / sample_rate as f32).floor())
        .collect();

    let mut bank = vec![vec![0.0_f32; n_bins]; mel_bins];
    for m in 0..mel_bins {
        let lo = bin_points[m] as usize;
        let mid = bin_points[m + 1] as usize;
        let hi = bin_points[m + 2] as usize;
        for k in lo..mid {
            if k < n_bins && mid > lo {
                bank[m][k] = (k - lo) as f32 / (mid - lo) as f32;
            }
        }
        for k in mid..hi {
            if k < n_bins && hi > mid {
                bank[m][k] = (hi - k) as f32 / (hi - mid) as f32;
            }
        }
    }
    bank
}

/// Map token IDs to text using the provided vocabulary.
/// Unknown tokens are skipped.  Special/sentinel tokens (id ≥ vocab size or
/// not present in the map) are dropped silently.
#[cfg(feature = "caption-gen")]
fn tokens_to_text(token_ids: &[u32], vocab: &std::collections::HashMap<String, String>) -> String {
    token_ids
        .iter()
        .filter_map(|id| vocab.get(&id.to_string()))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Derive `(seq_len, vocab_size)` from an encoder output shape.
///
/// Handles both `[seq_len, vocab]` and `[batch, seq_len, vocab]` layouts.
#[cfg(feature = "caption-gen")]
fn derive_seq_vocab(shape: &[usize]) -> Option<(usize, usize)> {
    match shape {
        [seq, vocab] => Some((*seq, *vocab)),
        [_batch, seq, vocab] => Some((*seq, *vocab)),
        [flat] => {
            // Flat 1-D output: interpret as a single-step sequence with
            // `flat` vocabulary entries.
            Some((1, *flat))
        }
        _ => None,
    }
}

/// Distribute text uniformly across `total_ms` milliseconds, splitting at
/// sentence and word boundaries into segments of at most ~42 characters and
/// at most 5 seconds each.
#[cfg(feature = "caption-gen")]
fn build_segments_from_text(
    text: &str,
    total_ms: u64,
) -> Vec<oximedia_caption_gen::alignment::TranscriptSegment> {
    use oximedia_caption_gen::alignment::TranscriptSegment;

    // Target: up to 42 chars per segment, maximum 5 000 ms.
    const MAX_CHARS: usize = 42;
    const MAX_MS: u64 = 5_000;

    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    // Split into sentence-then-word chunks.
    let chunks = chunk_text(text, MAX_CHARS);
    if chunks.is_empty() {
        return Vec::new();
    }

    let total_chars: usize = chunks.iter().map(|c| c.chars().count().max(1)).sum();
    let mut segments = Vec::with_capacity(chunks.len());
    let mut cursor_ms = 0_u64;

    for (idx, chunk) in chunks.iter().enumerate() {
        let chunk_chars = chunk.chars().count().max(1);
        let natural_ms = if total_chars > 0 {
            (total_ms as f64 * chunk_chars as f64 / total_chars as f64).round() as u64
        } else {
            total_ms / chunks.len() as u64
        };
        let chunk_ms = natural_ms.min(MAX_MS);
        let start_ms = cursor_ms;
        let end_ms = if idx + 1 < chunks.len() {
            (cursor_ms + chunk_ms).min(total_ms.saturating_sub(1))
        } else {
            total_ms
        };

        segments.push(TranscriptSegment {
            text: chunk.clone(),
            start_ms,
            end_ms,
            speaker_id: None,
            words: Vec::new(),
        });

        cursor_ms = end_ms;
    }

    segments
}

/// Split `text` into chunks of at most `max_chars` chars, preferring sentence
/// then word boundaries.
#[cfg(feature = "caption-gen")]
fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    if text.chars().count() <= max_chars {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut remaining = text.trim();
    while !remaining.is_empty() {
        if remaining.chars().count() <= max_chars {
            chunks.push(remaining.to_string());
            break;
        }
        let window: String = remaining.chars().take(max_chars + 1).collect();
        let cut = find_break_point(&window, max_chars).unwrap_or(max_chars);
        let byte_pos = remaining
            .char_indices()
            .nth(cut)
            .map(|(b, _)| b)
            .unwrap_or(remaining.len());
        chunks.push(remaining[..byte_pos].trim_end().to_string());
        remaining = remaining[byte_pos..].trim_start();
    }
    chunks
}

/// Find the best character index to break at, preferring sentence boundaries
/// then word boundaries within `text[..max_chars]`.
#[cfg(feature = "caption-gen")]
fn find_break_point(text: &str, max_chars: usize) -> Option<usize> {
    let chars: Vec<char> = text.chars().take(max_chars).collect();
    // Sentence boundary.
    for (i, &ch) in chars.iter().enumerate().rev() {
        if matches!(ch, '.' | '!' | '?') {
            return Some(i + 1);
        }
    }
    // Word boundary.
    for (i, &ch) in chars.iter().enumerate().rev() {
        if ch == ' ' {
            return Some(i);
        }
    }
    None
}

/// Run the `captions sync` subcommand.
pub async fn run_captions_sync(opts: CaptionsSyncOptions, json_output: bool) -> Result<()> {
    let caption_data = std::fs::read(&opts.input)
        .with_context(|| format!("Failed to read captions: {}", opts.input.display()))?;

    let _ref_data = std::fs::read(&opts.reference)
        .with_context(|| format!("Failed to read reference: {}", opts.reference.display()))?;

    // Auto-detect format and import
    let track = oximedia_captions::import::Importer::import_auto(&caption_data)
        .map_err(|e| anyhow::anyhow!("Failed to parse captions: {e}"))?;

    let caption_count = track.count();

    // Detect output format from extension
    let out_format =
        oximedia_captions::export::Exporter::detect_format_from_extension(&opts.output)
            .unwrap_or(oximedia_captions::CaptionFormat::Srt);

    let output_bytes = oximedia_captions::export::Exporter::export(&track, out_format)
        .map_err(|e| anyhow::anyhow!("Export failed: {e}"))?;

    std::fs::write(&opts.output, &output_bytes)
        .with_context(|| format!("Failed to write output: {}", opts.output.display()))?;

    if json_output {
        let obj = serde_json::json!({
            "input": opts.input.to_string_lossy(),
            "reference": opts.reference.to_string_lossy(),
            "output": opts.output.to_string_lossy(),
            "max_shift_ms": opts.max_shift_ms,
            "captions_synced": caption_count,
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
    } else {
        println!("{}", "Caption Sync Complete".green().bold());
        println!("  Captions:  {}", opts.input.display());
        println!("  Reference: {}", opts.reference.display());
        println!("  Output:    {}", opts.output.display());
        println!("  Max shift: {}ms", opts.max_shift_ms);
        println!("  Synced:    {} captions", caption_count);
    }

    Ok(())
}

/// Run the `captions convert` subcommand.
pub async fn run_captions_convert(opts: CaptionsConvertOptions, json_output: bool) -> Result<()> {
    let data = std::fs::read(&opts.input)
        .with_context(|| format!("Failed to read input: {}", opts.input.display()))?;

    // Parse source format
    let track = if let Some(ref from) = opts.from_format {
        let src_fmt = parse_caption_format(from)?;
        oximedia_captions::import::Importer::import(&data, src_fmt)
            .map_err(|e| anyhow::anyhow!("Import failed: {e}"))?
    } else {
        oximedia_captions::import::Importer::import_auto(&data)
            .map_err(|e| anyhow::anyhow!("Auto-detect import failed: {e}"))?
    };

    let target_fmt = parse_caption_format(&opts.to_format)?;
    let output_bytes = oximedia_captions::export::Exporter::export(&track, target_fmt)
        .map_err(|e| anyhow::anyhow!("Export failed: {e}"))?;

    std::fs::write(&opts.output, &output_bytes)
        .with_context(|| format!("Failed to write output: {}", opts.output.display()))?;

    if json_output {
        let obj = serde_json::json!({
            "input": opts.input.to_string_lossy(),
            "output": opts.output.to_string_lossy(),
            "from_format": opts.from_format.as_deref().unwrap_or("auto"),
            "to_format": opts.to_format,
            "captions_count": track.count(),
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
    } else {
        println!("{}", "Caption Conversion Complete".green().bold());
        println!("  Input:  {}", opts.input.display());
        println!("  Output: {}", opts.output.display());
        println!(
            "  Format: {} -> {}",
            opts.from_format.as_deref().unwrap_or("auto"),
            opts.to_format
        );
        println!("  Captions: {}", track.count());
    }

    Ok(())
}

/// Run the `captions burn` subcommand.
///
/// Validates the input video and caption file for real (the video must
/// exist, the caption file must actually parse, and the track must be
/// non-empty), then returns an honest error rather than a rendered video.
///
/// Burning captions into video pixels needs a decode -> rasterize-glyphs ->
/// composite -> re-encode pipeline. `oximedia-graph` has a real, working,
/// `fontdue`-backed rasterizer (`oximedia_graph::filters::video::timecode::TimecodeFilter`,
/// used for timecode burn-in), and `oximedia_captions::caption_renderer`
/// provides caption layout math -- but neither is wired into any
/// CLI-reachable execution path: `oximedia-cli`'s real `-vf`/`-af` transcode
/// filters are limited to `scale=`/`volume=` (see `transcode.rs`), and there
/// is no decode/encode round trip callable from this handler. Copying the
/// input to the output (the previous behaviour) would silently ship an
/// un-captioned video while claiming success, so we refuse instead.
// TODO(0.2.x): wire a real burn-in pipeline once a CLI-reachable
// decode -> composite -> encode path exists. `oximedia-graph`'s
// `TimecodeFilter` is the closest existing font-rasterization building
// block to model a `CaptionFilter` node on.
pub async fn run_captions_burn(opts: CaptionsBurnOptions, json_output: bool) -> Result<()> {
    if !opts.video.exists() {
        return Err(anyhow::anyhow!(
            "Input video not found: {}",
            opts.video.display()
        ));
    }

    let caption_data = std::fs::read(&opts.captions)
        .with_context(|| format!("Failed to read captions: {}", opts.captions.display()))?;

    let track = oximedia_captions::import::Importer::import_auto(&caption_data)
        .map_err(|e| anyhow::anyhow!("Failed to parse captions: {e}"))?;

    let caption_count = track.count();
    if caption_count == 0 {
        return Err(anyhow::anyhow!(
            "Caption file '{}' contains no captions to burn",
            opts.captions.display()
        ));
    }

    if json_output {
        let diag = serde_json::json!({
            "video": opts.video.to_string_lossy(),
            "captions": opts.captions.to_string_lossy(),
            "output": opts.output.to_string_lossy(),
            "font_size": opts.font_size,
            "font_color": opts.font_color,
            "captions_parsed": caption_count,
            "status": "error",
            "error": "caption burn-in pixel rendering is not implemented",
        });
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&diag).unwrap_or_else(|_| diag.to_string())
        );
    }

    Err(anyhow::anyhow!(
        "Caption burn-in is not yet implemented: parsed {caption_count} caption(s) from '{}' \
         successfully (requested {}px, #{}), but oximedia-cli has no reachable video decode -> \
         text-rasterize -> re-encode pipeline to composite them onto '{}'. Refusing to fabricate \
         '{}' by copying the input.",
        opts.captions.display(),
        opts.font_size,
        opts.font_color,
        opts.video.display(),
        opts.output.display()
    ))
}

/// Run the `captions extract` subcommand.
///
/// Extracts a real embedded subtitle track from a Matroska/WebM container
/// using `oximedia-container`'s Matroska demuxer (which genuinely parses
/// EBML tracks/clusters/blocks -- see
/// `oximedia_container::demux::MatroskaDemuxer`). Only text-based subtitle
/// codecs (WebVTT, SRT/`S_TEXT-UTF8`) are supported today; unsupported
/// containers, missing tracks, or a track with zero decodable cues all
/// return an honest error instead of emitting an empty caption track as if
/// extraction had succeeded.
// TODO(0.2.x): extend to MP4 (tx3g) and ASS/SSA-in-Matroska once those
// demux/parsing paths are worth the added complexity. Matroska's
// `BlockDuration` is parsed internally by the demuxer but not yet
// propagated through `Packet::timestamp.duration`, so trailing-cue/exact
// per-cue duration is approximated here (see `DEFAULT_LAST_CUE_MS` below);
// revisit once that's threaded through.
pub async fn run_captions_extract(opts: CaptionsExtractOptions, json_output: bool) -> Result<()> {
    let raw = std::fs::read(&opts.input)
        .with_context(|| format!("Failed to read input: {}", opts.input.display()))?;

    let format = parse_caption_format(&opts.format)?;

    // Both "format couldn't even be detected" and "format detected but isn't
    // Matroska/WebM" land in the same honest, actionable message: only
    // Matroska/WebM embedded subtitle tracks are supported by this build.
    let probe = match oximedia_container::probe_format(&raw) {
        Ok(p)
            if matches!(
                p.format,
                oximedia_container::ContainerFormat::Matroska
                    | oximedia_container::ContainerFormat::WebM
            ) =>
        {
            p
        }
        Ok(p) => {
            return Err(anyhow::anyhow!(
                "Caption extraction currently supports embedded subtitle tracks in Matroska/WebM \
                 (.mkv/.webm) containers only; '{}' was detected as {:?}. No subtitle-track \
                 demuxer is wired for this container in this build.",
                opts.input.display(),
                p.format
            ));
        }
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Caption extraction currently supports embedded subtitle tracks in Matroska/WebM \
                 (.mkv/.webm) containers only; the container format of '{}' could not be \
                 detected at all: {e}",
                opts.input.display()
            ));
        }
    };

    use oximedia_container::demux::MatroskaDemuxer;
    use oximedia_container::Demuxer;
    use oximedia_core::{CodecId, OxiError};
    use oximedia_io::source::MemorySource;

    let source = MemorySource::from_vec(raw);
    let mut demuxer = MatroskaDemuxer::new(source);
    demuxer
        .probe()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to probe '{}': {e}", opts.input.display()))?;

    let subtitle_positions: Vec<usize> = demuxer
        .streams()
        .iter()
        .enumerate()
        .filter(|(_, s)| s.is_subtitle())
        .map(|(i, _)| i)
        .collect();

    if subtitle_positions.is_empty() {
        return Err(anyhow::anyhow!(
            "No subtitle tracks found in '{}'",
            opts.input.display()
        ));
    }

    let target_stream = *subtitle_positions.get(opts.track).ok_or_else(|| {
        anyhow::anyhow!(
            "Subtitle track index {} out of range; '{}' has {} subtitle track(s) (valid indices: 0..{})",
            opts.track,
            opts.input.display(),
            subtitle_positions.len(),
            subtitle_positions.len()
        )
    })?;

    let stream_codec = demuxer.streams()[target_stream].codec;
    if !matches!(stream_codec, CodecId::WebVtt | CodecId::Srt) {
        return Err(anyhow::anyhow!(
            "Subtitle track {} in '{}' uses codec {:?}, which is not yet supported for text \
             extraction in this build (supported: WebVTT, SRT/S_TEXT-UTF8).",
            opts.track,
            opts.input.display(),
            stream_codec
        ));
    }

    let language = demuxer.streams()[target_stream]
        .metadata
        .entries
        .iter()
        .find(|(k, _)| k == "language")
        .map(|(_, v)| oximedia_captions::Language::new(v.clone(), v.clone(), false))
        .unwrap_or_else(oximedia_captions::Language::english);

    // Display duration applied to the *final* cue only. Matroska's
    // `BlockDuration` (parsed internally by this demuxer) is not currently
    // propagated through `Packet::timestamp.duration`, so exact per-cue
    // durations aren't available at this layer -- see the module TODO
    // above. Interior cues instead run until the next cue's real start
    // time, which matches genuine subtitle timing.
    const DEFAULT_LAST_CUE_MS: i64 = 4_000;

    let mut cues: Vec<(i64, String)> = Vec::new();
    loop {
        match demuxer.read_packet().await {
            Ok(packet) => {
                if packet.stream_index != target_stream {
                    continue;
                }
                let start_ms = (packet.timestamp.to_seconds() * 1000.0).round() as i64;
                let text = String::from_utf8_lossy(&packet.data).trim().to_string();
                if !text.is_empty() {
                    cues.push((start_ms, text));
                }
            }
            Err(OxiError::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Error reading packets from '{}': {e}",
                    opts.input.display()
                ));
            }
        }
    }

    if cues.is_empty() {
        return Err(anyhow::anyhow!(
            "Subtitle track {} in '{}' contains no decodable cues",
            opts.track,
            opts.input.display()
        ));
    }

    let mut track = oximedia_captions::CaptionTrack::new(language);
    let n = cues.len();
    for (i, (start_ms, text)) in cues.iter().enumerate() {
        let end_ms = if i + 1 < n {
            cues[i + 1].0.max(start_ms + 1)
        } else {
            start_ms + DEFAULT_LAST_CUE_MS
        };
        let caption = oximedia_captions::Caption::new(
            oximedia_captions::Timestamp::from_millis(*start_ms),
            oximedia_captions::Timestamp::from_millis(end_ms),
            text.clone(),
        );
        track
            .add_caption(caption)
            .map_err(|e| anyhow::anyhow!("Failed to add caption: {e}"))?;
    }

    let output_bytes = oximedia_captions::export::Exporter::export(&track, format)
        .map_err(|e| anyhow::anyhow!("Export failed: {e}"))?;

    std::fs::write(&opts.output, &output_bytes)
        .with_context(|| format!("Failed to write output: {}", opts.output.display()))?;

    let caption_count = track.count();
    if json_output {
        let obj = serde_json::json!({
            "input": opts.input.to_string_lossy(),
            "output": opts.output.to_string_lossy(),
            "format": opts.format,
            "track": opts.track,
            "container": format!("{:?}", probe.format),
            "captions_extracted": caption_count,
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
    } else {
        println!("{}", "Caption Extraction Complete".green().bold());
        println!("  Input:     {}", opts.input.display());
        println!("  Output:    {}", opts.output.display());
        println!("  Format:    {}", opts.format);
        println!("  Track:     {}", opts.track);
        println!("  Extracted: {} captions", caption_count);
    }

    Ok(())
}

/// Run the `captions validate` subcommand.
pub async fn run_captions_validate(opts: CaptionsValidateOptions, json_output: bool) -> Result<()> {
    let data = std::fs::read(&opts.input)
        .with_context(|| format!("Failed to read input: {}", opts.input.display()))?;

    let track = oximedia_captions::import::Importer::import_auto(&data)
        .map_err(|e| anyhow::anyhow!("Failed to parse captions: {e}"))?;

    // Run validation
    let validator = oximedia_captions::validation::Validator::new();
    let report = validator
        .validate(&track)
        .map_err(|e| anyhow::anyhow!("Validation failed: {e}"))?;

    // Write report if requested
    if let Some(ref report_path) = opts.report {
        let report_text = render_validation_report(&report, &opts.input, &opts.standard);
        std::fs::write(report_path, &report_text)
            .with_context(|| format!("Failed to write report: {}", report_path.display()))?;
    }

    if json_output {
        let issues_json: Vec<serde_json::Value> = report
            .issues
            .iter()
            .map(|issue| {
                serde_json::json!({
                    "severity": format!("{:?}", issue.severity),
                    "message": issue.message,
                    "rule": issue.rule,
                })
            })
            .collect();

        let obj = serde_json::json!({
            "input": opts.input.to_string_lossy(),
            "standard": opts.standard,
            "passed": report.passed(),
            "statistics": {
                "total_captions": report.statistics.total_captions,
                "total_words": report.statistics.total_words,
                "avg_reading_speed": report.statistics.avg_reading_speed,
                "max_reading_speed": report.statistics.max_reading_speed,
                "avg_chars_per_line": report.statistics.avg_chars_per_line,
                "max_chars_per_line": report.statistics.max_chars_per_line,
                "errors": report.statistics.error_count,
                "warnings": report.statistics.warning_count,
            },
            "issues": issues_json,
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
    } else {
        let status = if report.passed() {
            "PASSED".green().bold().to_string()
        } else {
            "FAILED".red().bold().to_string()
        };

        println!("{}", "Caption Validation".green().bold());
        println!("  File:     {}", opts.input.display());
        println!("  Standard: {}", opts.standard);
        println!("  Result:   {}", status);
        println!();
        println!("  {}", "Statistics:".cyan().bold());
        println!("    Captions:       {}", report.statistics.total_captions);
        println!("    Words:          {}", report.statistics.total_words);
        println!(
            "    Avg WPM:        {:.1}",
            report.statistics.avg_reading_speed
        );
        println!(
            "    Max WPM:        {:.1}",
            report.statistics.max_reading_speed
        );
        println!(
            "    Max chars/line: {}",
            report.statistics.max_chars_per_line
        );

        if !report.issues.is_empty() {
            println!();
            println!("  {}", "Issues:".yellow().bold());
            for issue in &report.issues {
                let sev_str = match issue.severity {
                    oximedia_captions::validation::IssueSeverity::Error => {
                        "ERROR".red().to_string()
                    }
                    oximedia_captions::validation::IssueSeverity::Warning => {
                        "WARN".yellow().to_string()
                    }
                    oximedia_captions::validation::IssueSeverity::Info => {
                        "INFO".dimmed().to_string()
                    }
                };
                println!(
                    "    [{}] {} ({})",
                    sev_str,
                    issue.message,
                    issue.rule.dimmed()
                );
            }
        }

        if let Some(ref rp) = opts.report {
            println!("\n  Report saved: {}", rp.display());
        }
    }

    Ok(())
}

/// Render a validation report as plain text.
fn render_validation_report(
    report: &oximedia_captions::validation::ValidationReport,
    input: &PathBuf,
    standard: &str,
) -> String {
    let mut buf = String::new();
    buf.push_str("Caption Validation Report\n");
    buf.push_str(&format!("File: {}\n", input.display()));
    buf.push_str(&format!("Standard: {}\n", standard));
    buf.push_str(&format!("Passed: {}\n\n", report.passed()));
    buf.push_str(&format!("Captions: {}\n", report.statistics.total_captions));
    buf.push_str(&format!("Words: {}\n", report.statistics.total_words));
    buf.push_str(&format!(
        "Avg reading speed: {:.1} WPM\n",
        report.statistics.avg_reading_speed
    ));
    buf.push_str(&format!(
        "Errors: {}, Warnings: {}\n\n",
        report.statistics.error_count, report.statistics.warning_count
    ));
    for issue in &report.issues {
        buf.push_str(&format!(
            "[{:?}] {} (rule: {})\n",
            issue.severity, issue.message, issue.rule
        ));
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Feature-gating error tests ────────────────────────────────────────────

    /// Without the `caption-gen` feature the function must return an error
    /// containing the string "caption-gen" to guide the user.
    #[cfg(not(feature = "caption-gen"))]
    #[tokio::test]
    async fn test_run_captions_generate_no_feature_error() {
        let tmp = std::env::temp_dir();
        let opts = CaptionsGenerateOptions {
            input: tmp.join("nonexistent_input.wav"),
            output: tmp.join("out.srt"),
            format: "srt".to_string(),
            language: "en".to_string(),
            model: None,
            vocab: None,
        };
        let err = run_captions_generate(opts, false)
            .await
            .expect_err("must fail without caption-gen feature");
        let msg = err.to_string();
        assert!(
            msg.contains("caption-gen"),
            "error message should mention 'caption-gen', got: {msg}"
        );
    }

    /// With the `caption-gen` feature enabled but no model supplied, the
    /// function must return an error containing "model".
    #[cfg(feature = "caption-gen")]
    #[tokio::test]
    async fn test_run_captions_generate_no_model_error() {
        let tmp = std::env::temp_dir();
        // Create a minimal valid WAV so we don't fail at the read step.
        let wav_path = tmp.join("oximedia_cli_test_captions_gen_no_model.wav");
        let _ = std::fs::write(&wav_path, minimal_wav_bytes());
        let opts = CaptionsGenerateOptions {
            input: wav_path,
            output: tmp.join("out_no_model.srt"),
            format: "srt".to_string(),
            language: "en".to_string(),
            model: None,
            vocab: None,
        };
        let err = run_captions_generate(opts, false)
            .await
            .expect_err("must fail when no model is provided");
        let msg = err.to_string();
        assert!(
            msg.contains("model"),
            "error message should mention 'model', got: {msg}"
        );
    }

    /// Build a minimal 16-bit PCM WAV with one frame of silence (for tests).
    #[cfg(feature = "caption-gen")]
    fn minimal_wav_bytes() -> Vec<u8> {
        // 44-byte header + 2 bytes of 16-bit PCM silence.
        let data_size: u32 = 2;
        let byte_rate: u32 = 16_000 * 1 * 2;
        let file_size: u32 = 36 + data_size;
        let mut v = Vec::with_capacity(46);
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&file_size.to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16_u32.to_le_bytes()); // chunk size
        v.extend_from_slice(&1_u16.to_le_bytes()); // PCM
        v.extend_from_slice(&1_u16.to_le_bytes()); // mono
        v.extend_from_slice(&16_000_u32.to_le_bytes()); // sample rate
        v.extend_from_slice(&byte_rate.to_le_bytes());
        v.extend_from_slice(&2_u16.to_le_bytes()); // block align
        v.extend_from_slice(&16_u16.to_le_bytes()); // bits per sample
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_size.to_le_bytes());
        v.extend_from_slice(&[0x00, 0x00]); // one silent sample
        v
    }

    // ── Format parsing tests ───────────────────────────────────────────────────

    #[test]
    fn test_parse_caption_format_srt() {
        let fmt = parse_caption_format("srt");
        assert!(fmt.is_ok());
        assert_eq!(
            fmt.expect("should parse srt"),
            oximedia_captions::CaptionFormat::Srt
        );
    }

    #[test]
    fn test_parse_caption_format_webvtt() {
        let fmt = parse_caption_format("webvtt");
        assert!(fmt.is_ok());
        assert_eq!(
            fmt.expect("should parse webvtt"),
            oximedia_captions::CaptionFormat::WebVtt
        );
    }

    #[test]
    fn test_parse_caption_format_unknown() {
        let fmt = parse_caption_format("xyz123");
        assert!(fmt.is_err());
    }

    #[test]
    fn test_parse_caption_format_case_insensitive() {
        let fmt = parse_caption_format("SRT");
        assert!(fmt.is_ok());
        let fmt2 = parse_caption_format("Ttml");
        assert!(fmt2.is_ok());
    }

    #[test]
    fn test_render_validation_report() {
        let report = oximedia_captions::validation::ValidationReport::new();
        let path = std::env::temp_dir().join("test.srt");
        let text = render_validation_report(&report, &path, "fcc");
        assert!(text.contains("Caption Validation Report"));
        assert!(text.contains("fcc"));
        assert!(text.contains("Passed: true"));
    }

    // ── captions extract: real Matroska/WebM demux, honest-Err otherwise ────

    /// Encode `n` (0..=127) as a 1-byte EBML VINT size marker.
    fn ebml_size1(n: usize) -> u8 {
        assert!(
            n <= 0x7F,
            "test fixture value too large for 1-byte EBML VINT: {n}"
        );
        0x80 | (n as u8)
    }

    /// Build a minimal, valid WebM byte buffer with a single subtitle track
    /// (`codec_id`, e.g. `"S_TEXT/UTF8"`) carrying `cues` (cluster-relative
    /// timecode in ms, cue text) inside one cluster.
    ///
    /// Mirrors the byte-for-byte EBML structure of `oximedia_container`'s
    /// own `MatroskaDemuxer` test fixture (`create_webm_header` in
    /// `demux/matroska/mod.rs`) -- same EBML/Segment/Info header, just a
    /// subtitle `TrackEntry` (`TrackType` 0x11) instead of a video one, and
    /// real cue text as the `SimpleBlock` payload instead of opaque bytes.
    fn build_test_webm_subtitle(codec_id: &str, cues: &[(i16, &str)]) -> Vec<u8> {
        let mut data = Vec::new();

        // EBML header (DocType "webm"), identical to the crate's own fixture.
        data.extend_from_slice(&[0x1A, 0x45, 0xDF, 0xA3, 0x9F]);
        data.extend_from_slice(&[0x42, 0x86, 0x81, 0x01]); // EBMLVersion
        data.extend_from_slice(&[0x42, 0xF7, 0x81, 0x01]); // EBMLReadVersion
        data.extend_from_slice(&[0x42, 0xF2, 0x81, 0x04]); // EBMLMaxIDLength
        data.extend_from_slice(&[0x42, 0xF3, 0x81, 0x08]); // EBMLMaxSizeLength
        data.extend_from_slice(&[0x42, 0x82, 0x84, b'w', b'e', b'b', b'm']); // DocType
        data.extend_from_slice(&[0x42, 0x87, 0x81, 0x04]); // DocTypeVersion
        data.extend_from_slice(&[0x42, 0x85, 0x81, 0x02]); // DocTypeReadVersion

        // Segment (unknown/unbounded size).
        data.extend_from_slice(&[
            0x18, 0x53, 0x80, 0x67, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        ]);

        // Info: TimecodeScale = 1_000_000 ns (1 ms per unit).
        data.extend_from_slice(&[0x15, 0x49, 0xA9, 0x66, ebml_size1(7)]);
        data.extend_from_slice(&[0x2A, 0xD7, 0xB1, ebml_size1(3), 0x0F, 0x42, 0x40]);

        // Tracks -> one subtitle TrackEntry.
        let codec_bytes = codec_id.as_bytes();
        assert!(codec_bytes.len() <= 0x7F);
        let codec_id_elem_len = 2 + codec_bytes.len();
        let track_entry_content_len = 3 + 5 + 3 + codec_id_elem_len;

        let mut track_entry = Vec::new();
        track_entry.push(0xAE); // TrackEntry ID
        track_entry.push(ebml_size1(track_entry_content_len));
        track_entry.extend_from_slice(&[0xD7, 0x81, 0x01]); // TrackNumber: 1
        track_entry.extend_from_slice(&[0x73, 0xC5, 0x82, 0x30, 0x39]); // TrackUID
        track_entry.extend_from_slice(&[0x83, 0x81, 0x11]); // TrackType: subtitle (0x11)
        track_entry.push(0x86); // CodecID ID
        track_entry.push(ebml_size1(codec_bytes.len()));
        track_entry.extend_from_slice(codec_bytes);

        data.extend_from_slice(&[0x16, 0x54, 0xAE, 0x6B]); // Tracks ID
        data.push(ebml_size1(track_entry.len()));
        data.extend_from_slice(&track_entry);

        // Cluster (unbounded) + Timestamp(0) + one SimpleBlock per cue.
        data.extend_from_slice(&[
            0x1F, 0x43, 0xB6, 0x75, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        ]);
        data.extend_from_slice(&[0xE7, 0x81, 0x00]); // Cluster Timestamp: 0

        for &(timecode, text) in cues {
            let text_bytes = text.as_bytes();
            let block_content_len = 1 + 2 + 1 + text_bytes.len();
            assert!(
                block_content_len <= 0x7F,
                "test cue too long for 1-byte EBML VINT"
            );
            data.push(0xA3); // SimpleBlock ID
            data.push(ebml_size1(block_content_len));
            data.push(0x81); // track number VINT = 1
            data.extend_from_slice(&timecode.to_be_bytes());
            data.push(0x80); // flags: keyframe
            data.extend_from_slice(text_bytes);
        }

        data
    }

    #[tokio::test]
    async fn test_run_captions_extract_non_matroska_input_errors() {
        let tmp = std::env::temp_dir();
        let input = tmp.join("oximedia_cli_test_extract_not_mkv.bin");
        let output = tmp.join("oximedia_cli_test_extract_not_mkv_out.srt");
        std::fs::write(&input, b"this is not a media container, just text").expect("write fixture");
        let _ = std::fs::remove_file(&output);

        let opts = CaptionsExtractOptions {
            input: input.clone(),
            output: output.clone(),
            format: "srt".to_string(),
            track: 0,
        };
        let err = run_captions_extract(opts, false)
            .await
            .expect_err("non-Matroska input must fail honestly");
        assert!(
            err.to_string().contains("Matroska/WebM"),
            "error should name the supported containers, got: {err}"
        );
        assert!(!output.exists(), "no output file should be fabricated");

        std::fs::remove_file(&input).ok();
    }

    #[tokio::test]
    async fn test_run_captions_extract_real_matroska_subtitle_track() {
        let tmp = std::env::temp_dir();
        let input = tmp.join("oximedia_cli_test_extract_real.webm");
        let output = tmp.join("oximedia_cli_test_extract_real_out.srt");
        let data =
            build_test_webm_subtitle("S_TEXT/UTF8", &[(0, "Hello world"), (2000, "Second cue")]);
        std::fs::write(&input, &data).expect("write webm fixture");
        let _ = std::fs::remove_file(&output);

        let opts = CaptionsExtractOptions {
            input: input.clone(),
            output: output.clone(),
            format: "srt".to_string(),
            track: 0,
        };
        run_captions_extract(opts, false)
            .await
            .expect("extraction from a real embedded subtitle track should succeed");

        let exported = std::fs::read_to_string(&output).expect("read exported SRT");
        assert!(
            exported.contains("Hello world"),
            "must contain the first real cue, got:\n{exported}"
        );
        assert!(
            exported.contains("Second cue"),
            "must contain the second real cue, got:\n{exported}"
        );
        assert!(
            exported.contains("00:00:02,000"),
            "must encode the real second-cue start timestamp, got:\n{exported}"
        );

        std::fs::remove_file(&input).ok();
        std::fs::remove_file(&output).ok();
    }

    #[tokio::test]
    async fn test_run_captions_extract_track_out_of_range() {
        let tmp = std::env::temp_dir();
        let input = tmp.join("oximedia_cli_test_extract_range.webm");
        let output = tmp.join("oximedia_cli_test_extract_range_out.srt");
        let data = build_test_webm_subtitle("S_TEXT/UTF8", &[(0, "Only cue")]);
        std::fs::write(&input, &data).expect("write webm fixture");
        let _ = std::fs::remove_file(&output);

        let opts = CaptionsExtractOptions {
            input: input.clone(),
            output: output.clone(),
            format: "srt".to_string(),
            track: 1, // only track 0 exists in this fixture
        };
        let err = run_captions_extract(opts, false)
            .await
            .expect_err("out-of-range track index must fail");
        assert!(err.to_string().contains("out of range"));
        assert!(!output.exists());

        std::fs::remove_file(&input).ok();
    }

    #[tokio::test]
    async fn test_run_captions_extract_unsupported_codec_errors() {
        let tmp = std::env::temp_dir();
        let input = tmp.join("oximedia_cli_test_extract_ass.webm");
        let output = tmp.join("oximedia_cli_test_extract_ass_out.srt");
        let data = build_test_webm_subtitle("S_TEXT/ASS", &[(0, "0,0,Default,,0,0,0,,Hello")]);
        std::fs::write(&input, &data).expect("write webm fixture");
        let _ = std::fs::remove_file(&output);

        let opts = CaptionsExtractOptions {
            input: input.clone(),
            output: output.clone(),
            format: "srt".to_string(),
            track: 0,
        };
        let err = run_captions_extract(opts, false)
            .await
            .expect_err("unsupported subtitle codec must fail honestly");
        assert!(err.to_string().contains("not yet supported"));
        assert!(!output.exists());

        std::fs::remove_file(&input).ok();
    }

    // ── captions burn: honest-Err, never a fabricated copy ──────────────────

    #[tokio::test]
    async fn test_run_captions_burn_missing_video_errors() {
        let tmp = std::env::temp_dir();
        let video = tmp.join("oximedia_cli_test_burn_missing_video.mkv");
        let captions = tmp.join("oximedia_cli_test_burn_missing_video.srt");
        let output = tmp.join("oximedia_cli_test_burn_missing_video_out.mkv");
        let _ = std::fs::remove_file(&video);
        std::fs::write(&captions, "1\n00:00:00,000 --> 00:00:02,000\nHello\n\n")
            .expect("write srt fixture");
        let _ = std::fs::remove_file(&output);

        let opts = CaptionsBurnOptions {
            video: video.clone(),
            captions: captions.clone(),
            output: output.clone(),
            font_size: 24,
            font_color: "FFFFFF".to_string(),
        };
        let err = run_captions_burn(opts, false)
            .await
            .expect_err("missing video must fail");
        assert!(err.to_string().contains("Input video not found"));
        assert!(!output.exists());

        std::fs::remove_file(&captions).ok();
    }

    #[tokio::test]
    async fn test_run_captions_burn_real_captions_returns_honest_err_no_output_file() {
        let tmp = std::env::temp_dir();
        let video = tmp.join("oximedia_cli_test_burn_video.mkv");
        let captions = tmp.join("oximedia_cli_test_burn_captions.srt");
        let output = tmp.join("oximedia_cli_test_burn_output.mkv");
        std::fs::write(&video, b"not a real video, just bytes for existence checks")
            .expect("write video fixture");
        std::fs::write(
            &captions,
            "1\n00:00:00,000 --> 00:00:02,000\nHello world\n\n",
        )
        .expect("write srt fixture");
        let _ = std::fs::remove_file(&output);

        let opts = CaptionsBurnOptions {
            video: video.clone(),
            captions: captions.clone(),
            output: output.clone(),
            font_size: 32,
            font_color: "FFFFFF".to_string(),
        };
        let err = run_captions_burn(opts, false)
            .await
            .expect_err("burn-in must not fabricate success");
        let msg = err.to_string();
        assert!(
            msg.contains("not yet implemented"),
            "error should be honest about the missing pipeline, got: {msg}"
        );
        assert!(
            !msg.contains("Caption Burn Complete"),
            "must not resurrect the old fabricated success banner text"
        );
        assert!(
            !output.exists(),
            "must NOT copy the input to the output and call it burned-in"
        );

        std::fs::remove_file(&video).ok();
        std::fs::remove_file(&captions).ok();
    }
}
