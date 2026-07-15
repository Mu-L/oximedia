//! Video denoising command.
//!
//! Provides the `oximedia denoise` subcommand for noise reduction using the
//! `oximedia-denoise` crate's `Denoiser` and `DenoiseConfig`.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Options for the `denoise` command.
pub struct DenoiseOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub mode: String,
    pub strength: f32,
    pub spatial: bool,
    pub temporal: bool,
    pub preserve_grain: bool,
}

/// Entry point called from `main.rs`.
///
/// Validates the denoise configuration against the real `oximedia-denoise`
/// engine, then returns an honest error: the [`oximedia_denoise::Denoiser`]
/// operates on decoded [`oximedia_codec::VideoFrame`]s, so producing an output
/// file requires a full decode → denoise → encode video pipeline that is not
/// yet wired into the CLI. No output file is produced or claimed.
pub async fn run_denoise(opts: DenoiseOptions, _json_output: bool) -> Result<()> {
    use oximedia_denoise::Denoiser;

    // Validate arguments and configuration up front so bad flags still surface
    // their specific error (mode/strength) rather than the pipeline message.
    let mode = parse_mode(&opts.mode)?;
    let config = build_config(mode, opts.strength, opts.preserve_grain)?;
    let _denoiser =
        Denoiser::new(config).with_context(|| "Failed to initialise Denoiser".to_string())?;

    // TODO(0.2.x): wire decode(input) → Denoiser::process(frame) per frame → encode(output).
    Err(anyhow::anyhow!(
        "denoise: the video frame pipeline is not yet wired to the CLI. The denoise engine \
         processes decoded frames, so reading '{}' and writing '{}' requires a decode -> \
         denoise -> encode pipeline (planned for 0.2.x). Validated denoise configuration \
         (mode '{}', strength {:.2}, spatial={}, temporal={}); no output written.",
        opts.input.display(),
        opts.output.display(),
        opts.mode,
        opts.strength,
        opts.spatial,
        opts.temporal
    ))
}

/// Map CLI mode string to `DenoiseMode`.
fn parse_mode(mode: &str) -> Result<oximedia_denoise::DenoiseMode> {
    use oximedia_denoise::DenoiseMode;
    match mode.to_lowercase().replace('-', "_").as_str() {
        "fast" => Ok(DenoiseMode::Fast),
        "balanced" => Ok(DenoiseMode::Balanced),
        "quality" => Ok(DenoiseMode::Quality),
        "grain_aware" | "grain-aware" => Ok(DenoiseMode::GrainAware),
        other => anyhow::bail!(
            "Unknown denoise mode '{}'. Use: fast, balanced, quality, grain-aware",
            other
        ),
    }
}

/// Build a `DenoiseConfig` from the provided options.
fn build_config(
    mode: oximedia_denoise::DenoiseMode,
    strength: f32,
    preserve_grain: bool,
) -> Result<oximedia_denoise::DenoiseConfig> {
    use oximedia_denoise::DenoiseConfig;

    let base = match mode {
        oximedia_denoise::DenoiseMode::Fast => DenoiseConfig::light(),
        oximedia_denoise::DenoiseMode::Quality => DenoiseConfig::strong(),
        _ => DenoiseConfig::medium(),
    };

    let config = DenoiseConfig {
        mode,
        strength: strength.clamp(0.0, 1.0),
        preserve_grain,
        ..base
    };

    config
        .validate()
        .with_context(|| "Invalid denoise configuration")?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(mode: &str, output: PathBuf) -> DenoiseOptions {
        DenoiseOptions {
            input: std::env::temp_dir().join("oximedia_denoise_in.mp4"),
            output,
            mode: mode.to_string(),
            strength: 0.5,
            spatial: true,
            temporal: true,
            preserve_grain: false,
        }
    }

    #[tokio::test]
    async fn denoise_is_honest_error_and_writes_nothing() {
        let output = std::env::temp_dir().join("oximedia_denoise_out.mp4");
        std::fs::remove_file(&output).ok();

        let result = run_denoise(opts("balanced", output.clone()), false).await;
        assert!(result.is_err(), "denoise must return an honest error");
        assert!(!output.exists(), "no output file may be produced");
    }

    #[tokio::test]
    async fn denoise_invalid_mode_errors() {
        let output = std::env::temp_dir().join("oximedia_denoise_badmode_out.mp4");
        let result = run_denoise(opts("bogus-mode", output), false).await;
        assert!(result.is_err(), "invalid mode must error");
    }
}
