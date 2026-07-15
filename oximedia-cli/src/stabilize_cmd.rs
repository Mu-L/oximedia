//! Video stabilisation command.
//!
//! Provides `oximedia stabilize` using `oximedia-stabilize` to remove unwanted
//! camera shake via configurable motion models and quality presets.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Options for the `stabilize` command.
pub struct StabilizeOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub mode: String,
    pub quality: String,
    pub smoothing: u32,
    pub zoom: bool,
}

/// Entry point called from `main.rs`.
///
/// Validates the stabilisation configuration against the real
/// `oximedia-stabilize` engine, then returns an honest error: the
/// [`oximedia_stabilize::Stabilizer`] consumes a decoded frame sequence, so
/// producing an output file requires a full decode → stabilize → encode video
/// pipeline that is not yet wired into the CLI. No output file is produced.
pub async fn run_stabilize(opts: StabilizeOptions, _json_output: bool) -> Result<()> {
    use oximedia_stabilize::{StabilizeConfig, Stabilizer};

    let stab_mode = parse_mode(&opts.mode)?;
    let quality = parse_quality(&opts.quality)?;

    // Smoothing strength as normalised 0.0-1.0 from the frame-count window
    let smoothing_strength = (opts.smoothing as f64 / 100.0).clamp(0.01, 1.0);

    let config = StabilizeConfig::new()
        .with_mode(stab_mode)
        .with_quality(quality)
        .with_smoothing_strength(smoothing_strength)
        .with_zoom_optimization(opts.zoom);

    config
        .validate()
        .with_context(|| "Invalid stabilisation configuration")?;

    let _stabilizer = Stabilizer::new(config).with_context(|| "Failed to initialise Stabilizer")?;

    // TODO(0.2.x): wire decode(input) → Stabilizer::stabilize(&frames) → encode(output).
    Err(anyhow::anyhow!(
        "stabilize: the video frame pipeline is not yet wired to the CLI. The stabilizer \
         consumes a decoded frame sequence, so reading '{}' and writing '{}' requires a \
         decode -> stabilize -> encode pipeline (planned for 0.2.x). Validated stabilisation \
         configuration (mode '{}', quality '{}'); no output written.",
        opts.input.display(),
        opts.output.display(),
        opts.mode,
        opts.quality
    ))
}

/// Map CLI mode string to `StabilizationMode`.
fn parse_mode(mode: &str) -> Result<oximedia_stabilize::StabilizationMode> {
    use oximedia_stabilize::StabilizationMode;
    match mode.to_lowercase().as_str() {
        "translation" => Ok(StabilizationMode::Translation),
        "affine" => Ok(StabilizationMode::Affine),
        "perspective" => Ok(StabilizationMode::Perspective),
        "3d" | "threed" => Ok(StabilizationMode::ThreeD),
        other => anyhow::bail!(
            "Unknown stabilisation mode '{}'. Use: translation, affine, perspective, 3d",
            other
        ),
    }
}

/// Map CLI quality string to `QualityPreset`.
fn parse_quality(quality: &str) -> Result<oximedia_stabilize::QualityPreset> {
    use oximedia_stabilize::QualityPreset;
    match quality.to_lowercase().as_str() {
        "fast" => Ok(QualityPreset::Fast),
        "balanced" => Ok(QualityPreset::Balanced),
        "maximum" | "max" => Ok(QualityPreset::Maximum),
        other => anyhow::bail!(
            "Unknown quality preset '{}'. Use: fast, balanced, maximum",
            other
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(mode: &str, quality: &str, output: PathBuf) -> StabilizeOptions {
        StabilizeOptions {
            input: std::env::temp_dir().join("oximedia_stabilize_in.mp4"),
            output,
            mode: mode.to_string(),
            quality: quality.to_string(),
            smoothing: 30,
            zoom: true,
        }
    }

    #[tokio::test]
    async fn stabilize_is_honest_error_and_writes_nothing() {
        let output = std::env::temp_dir().join("oximedia_stabilize_out.mp4");
        std::fs::remove_file(&output).ok();

        let result = run_stabilize(opts("affine", "balanced", output.clone()), false).await;
        assert!(result.is_err(), "stabilize must return an honest error");
        assert!(!output.exists(), "no output file may be produced");
    }

    #[tokio::test]
    async fn stabilize_invalid_mode_errors() {
        let output = std::env::temp_dir().join("oximedia_stabilize_badmode_out.mp4");
        let result = run_stabilize(opts("bogus", "balanced", output), false).await;
        assert!(result.is_err(), "invalid mode must error");
    }
}
