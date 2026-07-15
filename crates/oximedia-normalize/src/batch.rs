//! Batch file processing for loudness normalization.
//!
//! Provides utilities for processing multiple audio files with normalization.
//!
//! # Supported input/output format
//!
//! This module decodes and encodes **WAV** (PCM / IEEE float, via
//! [`oximedia_audio::wav`]) — the format `oximedia-audio`'s pure-Rust codec
//! already fully round-trips. Any file that is not a valid WAV stream produces an
//! honest per-file [`NormalizeError`] (surfaced as `Err` from [`BatchProcessor::process_file`],
//! or as a failed [`BatchResult`] entry from [`BatchProcessor::process_directory`] /
//! [`BatchProcessor::process_files`]) rather than a fabricated success.
//!
//! `// TODO(0.2.x):` additional codecs (MP3/FLAC/Opus/etc.) can be wired in here
//! once this module needs to support them — the two-pass analyze → gain → process
//! pipeline in [`process_decoded`] is already format-agnostic (it only needs
//! decoded `f32` samples plus a sample rate / channel count).

use crate::{
    AnalysisResult, LoudnessAnalyzer, NormalizationProcessor, NormalizeError, NormalizeResult,
    ProcessorConfig, ReplayGainCalculator, ReplayGainValues,
};
use oximedia_audio::wav::{WavReader, WavSpec, WavWriter};
use oximedia_metering::Standard;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Batch processing configuration.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct BatchConfig {
    /// Target loudness standard.
    pub standard: Standard,

    /// Enable true peak limiting.
    pub enable_limiter: bool,

    /// Enable dynamic range compression.
    pub enable_drc: bool,

    /// Write loudness metadata tags.
    ///
    /// `// TODO(0.2.x):` not yet honored by [`BatchProcessor::process_file`] — see
    /// the module docs on [`process_decoded`] for what a real implementation needs.
    pub write_metadata: bool,

    /// Calculate and write ReplayGain tags.
    pub write_replaygain: bool,

    /// Output format (None = same as input).
    pub output_format: Option<String>,

    /// Maximum gain adjustment in dB.
    pub max_gain_db: f64,

    /// Overwrite existing files.
    pub overwrite: bool,

    /// Process files in parallel.
    pub parallel: bool,
}

impl BatchConfig {
    /// Create a new batch configuration.
    pub fn new(standard: Standard) -> Self {
        Self {
            standard,
            enable_limiter: true,
            enable_drc: false,
            write_metadata: true,
            write_replaygain: true,
            output_format: None,
            max_gain_db: 20.0,
            overwrite: false,
            parallel: true,
        }
    }

    /// Create a minimal configuration (gain only).
    pub fn minimal(standard: Standard) -> Self {
        Self {
            standard,
            enable_limiter: false,
            enable_drc: false,
            write_metadata: false,
            write_replaygain: false,
            output_format: None,
            max_gain_db: 20.0,
            overwrite: false,
            parallel: false,
        }
    }
}

/// Batch processing result for a single file.
#[derive(Clone, Debug)]
pub struct BatchResult {
    /// Input file path.
    pub input_path: PathBuf,

    /// Output file path.
    pub output_path: PathBuf,

    /// Analysis result.
    pub analysis: AnalysisResult,

    /// Applied gain in dB.
    pub applied_gain_db: f64,

    /// ReplayGain values (if calculated).
    pub replay_gain: Option<ReplayGainValues>,

    /// Processing time in seconds.
    pub processing_time_s: f64,

    /// Success flag.
    pub success: bool,

    /// Error message (if failed).
    pub error: Option<String>,
}

impl BatchResult {
    /// Create a successful result.
    pub fn success(
        input_path: PathBuf,
        output_path: PathBuf,
        analysis: AnalysisResult,
        applied_gain_db: f64,
        processing_time_s: f64,
    ) -> Self {
        Self {
            input_path,
            output_path,
            analysis,
            applied_gain_db,
            replay_gain: None,
            processing_time_s,
            success: true,
            error: None,
        }
    }

    /// Create a failed result.
    pub fn failure(input_path: PathBuf, error: String) -> Self {
        Self {
            input_path,
            output_path: PathBuf::new(),
            analysis: create_empty_analysis(),
            applied_gain_db: 0.0,
            replay_gain: None,
            processing_time_s: 0.0,
            success: false,
            error: Some(error),
        }
    }

    /// Add ReplayGain values.
    pub fn with_replay_gain(mut self, rg: ReplayGainValues) -> Self {
        self.replay_gain = Some(rg);
        self
    }
}

/// Batch processor for normalizing multiple files.
pub struct BatchProcessor {
    config: BatchConfig,
}

impl BatchProcessor {
    /// Create a new batch processor.
    pub fn new(config: BatchConfig) -> Self {
        Self { config }
    }

    /// Process a single WAV file: decode, run the real two-pass loudness
    /// normalization pipeline, and write the normalized output.
    ///
    /// `sample_rate` / `channels` are validated against the file's own decoded
    /// format (a WAV file is self-describing) — a mismatch is a caller error and
    /// is reported as [`NormalizeError::InvalidConfig`] rather than silently
    /// measuring/processing the audio under the wrong assumptions.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be decoded as WAV, if `sample_rate` /
    /// `channels` do not match the file's actual format, or if analysis/processing/
    /// encoding fails.
    pub fn process_file(
        &self,
        input_path: &Path,
        output_path: &Path,
        sample_rate: f64,
        channels: usize,
    ) -> NormalizeResult<BatchResult> {
        let start_time = Instant::now();

        let (samples, spec) = decode_wav(input_path)?;

        if spec.channels as usize != channels {
            return Err(NormalizeError::InvalidConfig(format!(
                "{}: file has {} channel(s), but {} were requested",
                input_path.display(),
                spec.channels,
                channels
            )));
        }
        if (f64::from(spec.sample_rate) - sample_rate).abs() > 0.5 {
            return Err(NormalizeError::InvalidConfig(format!(
                "{}: file is {} Hz, but {} Hz was requested",
                input_path.display(),
                spec.sample_rate,
                sample_rate
            )));
        }

        self.process_decoded(input_path, output_path, samples, spec, start_time)
    }

    /// Process all files in a directory: every regular file in `input_dir` (no
    /// recursion into subdirectories) is decoded and normalized if possible; each
    /// gets one [`BatchResult`] entry — successful or failed — so the returned
    /// `Vec` always reflects the directory's real contents instead of an
    /// unconditional empty result.
    ///
    /// Sample rate and channel count are taken from each file's own decoded WAV
    /// header (there is no single external hint to validate against, unlike
    /// [`Self::process_file`]).
    ///
    /// # Errors
    ///
    /// Returns an error if `input_dir` is not a directory, or if `output_dir`
    /// cannot be created. Per-file decode/processing failures are reported as
    /// failed [`BatchResult`] entries, not as a top-level `Err`.
    pub fn process_directory(
        &self,
        input_dir: &Path,
        output_dir: &Path,
    ) -> NormalizeResult<Vec<BatchResult>> {
        if !input_dir.is_dir() {
            return Err(NormalizeError::InvalidConfig(
                "Input path is not a directory".to_string(),
            ));
        }

        // Create output directory if it doesn't exist
        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)?;
        }

        let mut entries: Vec<PathBuf> = std::fs::read_dir(input_dir)?
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect();
        entries.sort();

        let mut results = Vec::with_capacity(entries.len());
        for input_path in entries {
            let start_time = Instant::now();

            let Some(file_name) = input_path.file_name() else {
                results.push(BatchResult::failure(
                    input_path,
                    "input path has no file name".to_string(),
                ));
                continue;
            };
            let output_path = output_dir.join(file_name);

            if output_path.exists() && !self.config.overwrite {
                results.push(BatchResult::failure(
                    input_path,
                    "Output file exists and overwrite is disabled".to_string(),
                ));
                continue;
            }

            let outcome = decode_wav(&input_path).and_then(|(samples, spec)| {
                self.process_decoded(&input_path, &output_path, samples, spec, start_time)
            });
            match outcome {
                Ok(result) => results.push(result),
                Err(e) => results.push(BatchResult::failure(input_path, e.to_string())),
            }
        }

        Ok(results)
    }

    /// Process a list of files.
    pub fn process_files(
        &self,
        files: &[(PathBuf, PathBuf)],
        sample_rate: f64,
        channels: usize,
    ) -> NormalizeResult<Vec<BatchResult>> {
        let mut results = Vec::new();

        for (input_path, output_path) in files {
            // Check if output exists and overwrite is disabled
            if output_path.exists() && !self.config.overwrite {
                results.push(BatchResult::failure(
                    input_path.clone(),
                    "Output file exists and overwrite is disabled".to_string(),
                ));
                continue;
            }

            match self.process_file(input_path, output_path, sample_rate, channels) {
                Ok(result) => results.push(result),
                Err(e) => results.push(BatchResult::failure(input_path.clone(), e.to_string())),
            }
        }

        Ok(results)
    }

    /// Get the batch configuration.
    pub fn config(&self) -> &BatchConfig {
        &self.config
    }

    /// Generate a summary report for batch results.
    pub fn generate_report(results: &[BatchResult]) -> BatchReport {
        let total = results.len();
        let successful = results.iter().filter(|r| r.success).count();
        let failed = total - successful;

        let avg_gain = if successful > 0 {
            results
                .iter()
                .filter(|r| r.success)
                .map(|r| r.applied_gain_db)
                .sum::<f64>()
                / successful as f64
        } else {
            0.0
        };

        let total_time = results.iter().map(|r| r.processing_time_s).sum::<f64>();

        BatchReport {
            total_files: total,
            successful_files: successful,
            failed_files: failed,
            average_gain_db: avg_gain,
            total_processing_time_s: total_time,
        }
    }

    /// Core two-pass pipeline shared by [`Self::process_file`] and
    /// [`Self::process_directory`]: analyze the real decoded `samples`, compute
    /// and apply gain (with optional limiter/DRC), write the normalized output,
    /// and — when configured — compute real ReplayGain values from the same
    /// samples.
    fn process_decoded(
        &self,
        input_path: &Path,
        output_path: &Path,
        samples: Vec<f32>,
        spec: WavSpec,
        start_time: Instant,
    ) -> NormalizeResult<BatchResult> {
        if samples.is_empty() {
            return Err(NormalizeError::InsufficientData(format!(
                "{} contains no audio samples",
                input_path.display()
            )));
        }

        let sample_rate = f64::from(spec.sample_rate);
        let channels = spec.channels as usize;

        // Pass 1: analyze the real decoded samples (not an empty meter).
        let mut analyzer = LoudnessAnalyzer::new(self.config.standard, sample_rate, channels)?;
        analyzer.process_f32(&samples);
        let analysis = analyzer.result();

        // Calculate gain, respecting the configured safety cap.
        let mut gain_db = analysis.recommended_gain_db;
        if gain_db.abs() > self.config.max_gain_db {
            gain_db = gain_db.signum() * self.config.max_gain_db;
        }

        // Pass 2: apply the gain (+ optional limiter / DRC) to the real samples.
        let processor_config = ProcessorConfig {
            sample_rate,
            channels,
            enable_limiter: self.config.enable_limiter,
            enable_drc: self.config.enable_drc,
            lookahead_ms: 5.0,
        };
        let mut processor = NormalizationProcessor::new(processor_config)?;
        let mut output_samples = vec![0.0_f32; samples.len()];
        processor.process_f32(&samples, &mut output_samples, gain_db)?;

        // Write the real, normalized audio to disk (not a placeholder / no-op).
        encode_wav(output_path, &output_samples, spec)?;

        let processing_time = start_time.elapsed().as_secs_f64();
        let mut result = BatchResult::success(
            input_path.to_path_buf(),
            output_path.to_path_buf(),
            analysis,
            gain_db,
            processing_time,
        );

        // Calculate ReplayGain from the same real samples (not an empty meter).
        if self.config.write_replaygain {
            let mut rg_calc = ReplayGainCalculator::new(sample_rate, channels)?;
            rg_calc.process_f32(&samples);
            if let Ok(rg) = rg_calc.calculate() {
                result = result.with_replay_gain(rg);
            }
        }

        // TODO(0.2.x): honor `self.config.write_metadata` by embedding loudness /
        // ReplayGain tags into the output container — e.g. append an `"id3 "` RIFF
        // sub-chunk built from `oximedia_metadata::{Metadata, MetadataFormat::Id3v2}`
        // and repatch the RIFF size field. Not implemented this pass;
        // `BatchConfig::write_metadata` is currently inert for WAV output.

        Ok(result)
    }
}

/// Batch processing summary report.
#[derive(Clone, Debug)]
pub struct BatchReport {
    /// Total number of files processed.
    pub total_files: usize,

    /// Number of successfully processed files.
    pub successful_files: usize,

    /// Number of failed files.
    pub failed_files: usize,

    /// Average gain applied in dB.
    pub average_gain_db: f64,

    /// Total processing time in seconds.
    pub total_processing_time_s: f64,
}

impl BatchReport {
    /// Format as human-readable string.
    pub fn format(&self) -> String {
        format!(
            "Batch Processing Report\n\
             =======================\n\
             Total files: {}\n\
             Successful: {}\n\
             Failed: {}\n\
             Average gain: {:+.2} dB\n\
             Total time: {:.2}s",
            self.total_files,
            self.successful_files,
            self.failed_files,
            self.average_gain_db,
            self.total_processing_time_s
        )
    }
}

/// Create an empty analysis result for error cases.
fn create_empty_analysis() -> AnalysisResult {
    use oximedia_metering::{ComplianceResult, LoudnessMetrics};

    AnalysisResult {
        integrated_lufs: f64::NEG_INFINITY,
        loudness_range: 0.0,
        true_peak_dbtp: f64::NEG_INFINITY,
        target_lufs: -23.0,
        max_peak_dbtp: -1.0,
        recommended_gain_db: 0.0,
        safe_gain_db: 0.0,
        is_compliant: false,
        compliance: ComplianceResult {
            standard: Standard::EbuR128,
            loudness_compliant: false,
            peak_compliant: false,
            lra_acceptable: false,
            integrated_lufs: f64::NEG_INFINITY,
            true_peak_dbtp: f64::NEG_INFINITY,
            loudness_range: 0.0,
            target_lufs: -23.0,
            max_peak_dbtp: -1.0,
            deviation_lu: 0.0,
        },
        metrics: LoudnessMetrics::default(),
        standard: Standard::EbuR128,
    }
}

/// Decode a WAV file's samples (interleaved `f32` in `[-1.0, 1.0]`) and its format spec.
fn decode_wav(path: &Path) -> NormalizeResult<(Vec<f32>, WavSpec)> {
    let file = File::open(path)?;
    let mut reader = WavReader::new(BufReader::new(file))?;
    let spec = reader.spec();
    let samples = reader.read_samples_f32()?;
    Ok((samples, spec))
}

/// Encode interleaved `f32` samples to a WAV file at `path`, using `spec`'s bit
/// depth / channel / sample-rate layout.
fn encode_wav(path: &Path, samples: &[f32], spec: WavSpec) -> NormalizeResult<()> {
    let file = File::create(path)?;
    let mut writer = WavWriter::new(BufWriter::new(file), spec);
    writer.write_samples_f32(samples)?;
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a real mono WAV file (16-bit PCM) containing a sine wave of the given
    /// linear amplitude, and return the samples that were written.
    fn write_sine_wav(path: &Path, sample_rate: u32, amplitude: f32, secs: f64) -> Vec<f32> {
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            float: false,
        };
        let n = (f64::from(sample_rate) * secs) as usize;
        let samples: Vec<f32> = (0..n)
            .map(|i| {
                amplitude * (std::f32::consts::TAU * 1000.0 * i as f32 / sample_rate as f32).sin()
            })
            .collect();
        let file = File::create(path).expect("create test wav");
        let mut writer = WavWriter::new(BufWriter::new(file), spec);
        writer
            .write_samples_f32(&samples)
            .expect("write test samples");
        writer.finalize().expect("finalize test wav");
        samples
    }

    fn rms(samples: &[f32]) -> f64 {
        (samples
            .iter()
            .map(|&s| f64::from(s) * f64::from(s))
            .sum::<f64>()
            / samples.len() as f64)
            .sqrt()
    }

    #[test]
    fn test_batch_config_creation() {
        let config = BatchConfig::new(Standard::EbuR128);
        assert!(config.enable_limiter);
        assert!(config.write_metadata);

        let minimal = BatchConfig::minimal(Standard::Spotify);
        assert!(!minimal.enable_limiter);
        assert!(!minimal.write_metadata);
    }

    #[test]
    fn test_batch_processor_creation() {
        let config = BatchConfig::new(Standard::EbuR128);
        let processor = BatchProcessor::new(config);
        assert!(processor.config().enable_limiter);
    }

    #[test]
    fn test_batch_result() {
        let result = BatchResult::failure(PathBuf::from("test.wav"), "Test error".to_string());
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_batch_report_format() {
        let report = BatchReport {
            total_files: 10,
            successful_files: 8,
            failed_files: 2,
            average_gain_db: -3.5,
            total_processing_time_s: 125.5,
        };

        let formatted = report.format();
        assert!(formatted.contains("Total files: 10"));
        assert!(formatted.contains("Successful: 8"));
    }

    /// `process_file` must actually decode the input, measure its real (finite)
    /// loudness, compute a non-zero gain, and write a normalized WAV whose content
    /// differs from a silent / unmodified copy — the exact behaviors the former
    /// placeholder implementation faked.
    #[test]
    fn test_process_file_normalizes_real_quiet_wav_toward_target() {
        let dir = std::env::temp_dir().join("oximedia_batch_process_file_test");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let input_path = dir.join("quiet.wav");
        let output_path = dir.join("normalized.wav");

        let sample_rate = 48_000u32;
        let input_samples = write_sine_wav(&input_path, sample_rate, 0.05, 3.0);

        let config = BatchConfig {
            standard: Standard::Spotify,
            enable_limiter: false,
            enable_drc: false,
            write_metadata: false,
            write_replaygain: true,
            output_format: None,
            max_gain_db: 40.0,
            overwrite: true,
            parallel: false,
        };
        let processor = BatchProcessor::new(config);

        let result = processor
            .process_file(&input_path, &output_path, f64::from(sample_rate), 1)
            .expect("process_file should decode, analyze, and write real output");

        assert!(result.success);
        assert!(
            result.analysis.integrated_lufs.is_finite(),
            "loudness must be measured from real decoded audio, got {}",
            result.analysis.integrated_lufs
        );
        assert!(
            result.applied_gain_db > 1.0,
            "a quiet input targeting -14 LUFS should need a substantial positive gain, got {}",
            result.applied_gain_db
        );
        let rg = result
            .replay_gain
            .expect("write_replaygain=true should populate replay_gain from real samples");
        assert!(rg.track_gain.is_finite());

        // Decode the output back and prove it is real, gained audio -- not a silent
        // or unmodified copy of the input.
        let (output_samples, out_spec) = decode_wav(&output_path).expect("decode output wav");
        assert_eq!(output_samples.len(), input_samples.len());
        assert_eq!(out_spec.sample_rate, sample_rate);

        let rms_in = rms(&input_samples);
        let rms_out = rms(&output_samples);
        assert!(
            rms_out > rms_in * 1.5,
            "output should be measurably louder than the (quiet) input: rms_in={rms_in}, \
             rms_out={rms_out}"
        );
        let expected_linear = 10.0_f64.powf(result.applied_gain_db / 20.0);
        let ratio = rms_out / rms_in;
        assert!(
            (ratio - expected_linear).abs() / expected_linear < 0.05,
            "output/input RMS ratio ({ratio}) should match the applied linear gain \
             ({expected_linear}) within 5%"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_process_file_rejects_channel_mismatch() {
        let dir = std::env::temp_dir().join("oximedia_batch_process_file_mismatch_test");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let input_path = dir.join("mono.wav");
        let output_path = dir.join("out.wav");
        write_sine_wav(&input_path, 48_000, 0.1, 0.5);

        let processor = BatchProcessor::new(BatchConfig::minimal(Standard::EbuR128));
        // File is mono (1 channel); request stereo (2) -- must error, not silently
        // misinterpret the sample layout.
        let result = processor.process_file(&input_path, &output_path, 48_000.0, 2);
        assert!(matches!(result, Err(NormalizeError::InvalidConfig(_))));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_process_file_errors_on_non_wav_input() {
        let dir = std::env::temp_dir().join("oximedia_batch_process_file_nonwav_test");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let input_path = dir.join("not_audio.txt");
        std::fs::write(&input_path, b"this is definitely not a wav file").expect("write text file");
        let output_path = dir.join("out.wav");

        let processor = BatchProcessor::new(BatchConfig::minimal(Standard::EbuR128));
        let result = processor.process_file(&input_path, &output_path, 48_000.0, 1);
        assert!(
            result.is_err(),
            "non-WAV input must be an honest error, not a fabricated success"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `process_directory` must enumerate the directory's real contents and return
    /// one result per file, never an unconditional empty `Vec`.
    #[test]
    fn test_process_directory_processes_real_files() {
        let dir = std::env::temp_dir().join("oximedia_batch_process_directory_test");
        let out_dir = dir.join("out");
        std::fs::create_dir_all(&dir).expect("create temp dir");

        write_sine_wav(&dir.join("a.wav"), 44_100, 0.05, 1.0);
        write_sine_wav(&dir.join("b.wav"), 44_100, 0.2, 1.0);
        std::fs::write(dir.join("not_audio.txt"), b"nope").expect("write stray file");

        let config = BatchConfig {
            standard: Standard::EbuR128,
            enable_limiter: false,
            enable_drc: false,
            write_metadata: false,
            write_replaygain: false,
            output_format: None,
            max_gain_db: 40.0,
            overwrite: true,
            parallel: false,
        };
        let processor = BatchProcessor::new(config);

        let results = processor
            .process_directory(&dir, &out_dir)
            .expect("process_directory should succeed");

        assert_eq!(
            results.len(),
            3,
            "STUB REGRESSION: expected one result per directory entry (2 wav + 1 non-wav)"
        );
        let successes = results.iter().filter(|r| r.success).count();
        assert_eq!(successes, 2, "both real WAV files should succeed");
        let failures = results.iter().filter(|r| !r.success).count();
        assert_eq!(
            failures, 1,
            "the non-WAV file should fail honestly, not be silently skipped"
        );

        // Both successful outputs must actually exist on disk with real audio content.
        for r in results.iter().filter(|r| r.success) {
            assert!(r.output_path.exists(), "{:?} should exist", r.output_path);
            let (samples, _) = decode_wav(&r.output_path).expect("decode real output");
            assert!(!samples.is_empty());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_process_directory_rejects_non_directory_input() {
        let dir = std::env::temp_dir().join("oximedia_batch_process_directory_not_a_dir_test");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let file_path = dir.join("just_a_file.wav");
        write_sine_wav(&file_path, 48_000, 0.1, 0.2);

        let processor = BatchProcessor::new(BatchConfig::minimal(Standard::EbuR128));
        let result = processor.process_directory(&file_path, &dir.join("out"));
        assert!(matches!(result, Err(NormalizeError::InvalidConfig(_))));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
