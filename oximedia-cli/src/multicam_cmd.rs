//! Multi-camera CLI commands for OxiMedia.
//!
//! Provides multi-camera synchronization, switching, compositing,
//! color matching, and export commands.

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use std::path::PathBuf;

/// Multi-camera command subcommands.
#[derive(Subcommand, Debug)]
pub enum MulticamCommand {
    /// Synchronize multiple camera angles
    Sync {
        /// Input camera files
        #[arg(short, long, required = true, num_args = 2..)]
        inputs: Vec<PathBuf>,

        /// Output synchronized timeline file (JSON)
        #[arg(short, long)]
        output: PathBuf,

        /// Sync method: audio, timecode, marker
        #[arg(long, default_value = "audio")]
        method: String,

        /// Drift tolerance in frames
        #[arg(long, default_value = "2")]
        drift_tolerance: u32,
    },

    /// Switch between camera angles at specified points
    Switch {
        /// Input camera files
        #[arg(short, long, required = true, num_args = 2..)]
        inputs: Vec<PathBuf>,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// JSON switch points: [{"time": 1.0, "camera": 0}, ...]
        #[arg(long)]
        switch_points: Option<String>,

        /// Enable automatic switching based on content analysis
        #[arg(long)]
        auto_switch: bool,

        /// Minimum shot duration in seconds for auto-switch
        #[arg(long, default_value = "2.0")]
        min_duration: f64,
    },

    /// Composite multiple cameras into a single frame layout
    Composite {
        /// Input camera files
        #[arg(short, long, required = true, num_args = 2..)]
        inputs: Vec<PathBuf>,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Layout type: grid, pip, side_by_side, stack
        #[arg(long, default_value = "grid")]
        layout: String,

        /// Output width in pixels
        #[arg(long)]
        width: Option<u32>,

        /// Output height in pixels
        #[arg(long)]
        height: Option<u32>,

        /// Grid spacing in pixels
        #[arg(long, default_value = "4")]
        spacing: u32,
    },

    /// Match colors across camera angles
    ColorMatch {
        /// Reference camera file
        #[arg(long)]
        reference: PathBuf,

        /// Input camera files to match
        #[arg(short, long, required = true, num_args = 1..)]
        inputs: Vec<PathBuf>,

        /// Output directory for matched files
        #[arg(short, long)]
        output_dir: PathBuf,
    },

    /// Export multi-camera timeline in various formats
    Export {
        /// Input timeline file (JSON)
        #[arg(short, long)]
        timeline: PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Export format: multicam_edl, xml, json
        #[arg(long, default_value = "multicam_edl")]
        format: String,
    },

    /// Show information about multi-camera layouts
    Layouts {},
}

/// Handle multicam command dispatch.
pub async fn handle_multicam_command(command: MulticamCommand, json_output: bool) -> Result<()> {
    match command {
        MulticamCommand::Sync {
            inputs,
            output,
            method,
            drift_tolerance,
        } => sync_cameras(&inputs, &output, &method, drift_tolerance, json_output).await,
        MulticamCommand::Switch {
            inputs,
            output,
            switch_points,
            auto_switch,
            min_duration,
        } => {
            switch_cameras(
                &inputs,
                &output,
                switch_points.as_deref(),
                auto_switch,
                min_duration,
                json_output,
            )
            .await
        }
        MulticamCommand::Composite {
            inputs,
            output,
            layout,
            width,
            height,
            spacing,
        } => {
            composite_cameras(
                &inputs,
                &output,
                &layout,
                width,
                height,
                spacing,
                json_output,
            )
            .await
        }
        MulticamCommand::ColorMatch {
            reference,
            inputs,
            output_dir,
        } => color_match(&reference, &inputs, &output_dir, json_output).await,
        MulticamCommand::Export {
            timeline,
            output,
            format,
        } => export_timeline(&timeline, &output, &format, json_output).await,
        MulticamCommand::Layouts {} => list_layouts(json_output).await,
    }
}

/// Validate sync method.
fn validate_sync_method(method: &str) -> Result<()> {
    match method {
        "audio" | "timecode" | "marker" => Ok(()),
        other => Err(anyhow::anyhow!(
            "Unknown sync method '{}'. Expected: audio, timecode, marker",
            other
        )),
    }
}

/// Validate layout type and return description.
fn layout_description(layout: &str) -> Result<&'static str> {
    match layout {
        "grid" => Ok("Grid layout (auto-sized)"),
        "pip" => Ok("Picture-in-picture (main + inset)"),
        "side_by_side" => Ok("Side-by-side (horizontal split)"),
        "stack" => Ok("Vertical stack layout"),
        other => Err(anyhow::anyhow!(
            "Unknown layout '{}'. Expected: grid, pip, side_by_side, stack",
            other
        )),
    }
}

/// Synchronize multiple camera angles.
async fn sync_cameras(
    inputs: &[PathBuf],
    output: &PathBuf,
    method: &str,
    drift_tolerance: u32,
    json_output: bool,
) -> Result<()> {
    validate_sync_method(method)?;

    for input in inputs {
        if !input.exists() {
            return Err(anyhow::anyhow!("Input file not found: {}", input.display()));
        }
    }

    // Build a multicam config
    let config = oximedia_multicam::MultiCamConfig {
        angle_count: inputs.len(),
        enable_audio_sync: method == "audio",
        enable_timecode_sync: method == "timecode",
        enable_visual_sync: method == "marker",
        drift_tolerance,
        ..oximedia_multicam::MultiCamConfig::default()
    };

    // Generate sync result data
    let sync_result = serde_json::json!({
        "cameras": inputs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        "method": method,
        "angle_count": config.angle_count,
        "drift_tolerance": drift_tolerance,
        "frame_rate": config.frame_rate,
        "status": "sync_ready",
        "offsets": inputs.iter().enumerate().map(|(i, _)| {
            serde_json::json!({ "camera": i, "offset_frames": 0, "confidence": 1.0 })
        }).collect::<Vec<_>>(),
        "message": "Cameras configured; audio/timecode sync requires frame decoding pipeline",
    });

    let json_str =
        serde_json::to_string_pretty(&sync_result).context("Failed to serialize sync result")?;

    tokio::fs::write(output, json_str.as_bytes())
        .await
        .context("Failed to write output file")?;

    if json_output {
        println!("{}", json_str);
    } else {
        println!("{}", "Multi-Camera Sync".green().bold());
        println!("{}", "=".repeat(60));
        println!("{:20} {}", "Cameras:", inputs.len());
        println!("{:20} {}", "Method:", method);
        println!("{:20} {} frames", "Drift tolerance:", drift_tolerance);
        println!("{:20} {}", "Output:", output.display());
        println!();
        for (i, input) in inputs.iter().enumerate() {
            println!("  Camera {}: {}", i, input.display());
        }
        println!();
        println!(
            "{}",
            "Sync configuration written. Frame decoding pipeline needed for actual sync.".yellow()
        );
    }

    Ok(())
}

/// Switch between camera angles.
async fn switch_cameras(
    inputs: &[PathBuf],
    output: &PathBuf,
    switch_points_json: Option<&str>,
    auto_switch: bool,
    min_duration: f64,
    json_output: bool,
) -> Result<()> {
    for input in inputs {
        if !input.exists() {
            return Err(anyhow::anyhow!("Input file not found: {}", input.display()));
        }
    }

    let switch_points: Vec<serde_json::Value> = if let Some(json) = switch_points_json {
        serde_json::from_str(json).context("Failed to parse switch points JSON")?
    } else {
        Vec::new()
    };

    let switch_result = serde_json::json!({
        "cameras": inputs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        "auto_switch": auto_switch,
        "min_shot_duration_secs": min_duration,
        "switch_points": switch_points,
        "output": output.display().to_string(),
        "status": "switch_ready",
        "message": "Switch list configured. Frame decoding pipeline needed for rendering.",
    });

    let json_str = serde_json::to_string_pretty(&switch_result)
        .context("Failed to serialize switch result")?;

    tokio::fs::write(output, json_str.as_bytes())
        .await
        .context("Failed to write output file")?;

    if json_output {
        println!("{}", json_str);
    } else {
        println!("{}", "Multi-Camera Switch".green().bold());
        println!("{}", "=".repeat(60));
        println!("{:20} {}", "Cameras:", inputs.len());
        println!("{:20} {}", "Auto-switch:", auto_switch);
        println!("{:20} {:.1}s", "Min duration:", min_duration);
        println!("{:20} {}", "Switch points:", switch_points.len());
        println!("{:20} {}", "Output:", output.display());
        println!();
        if !switch_points.is_empty() {
            println!("{}", "Switch Points".cyan().bold());
            println!("{}", "-".repeat(40));
            for sp in &switch_points {
                let time = sp.get("time").and_then(|t| t.as_f64()).unwrap_or(0.0);
                let cam = sp.get("camera").and_then(|c| c.as_u64()).unwrap_or(0);
                println!("  {:.2}s -> Camera {}", time, cam);
            }
        }
    }

    Ok(())
}

/// Composite multiple cameras into a single frame layout.
async fn composite_cameras(
    inputs: &[PathBuf],
    output: &PathBuf,
    layout: &str,
    width: Option<u32>,
    height: Option<u32>,
    spacing: u32,
    json_output: bool,
) -> Result<()> {
    let layout_desc = layout_description(layout)?;

    for input in inputs {
        if !input.exists() {
            return Err(anyhow::anyhow!("Input file not found: {}", input.display()));
        }
    }

    let out_w = width.unwrap_or(1920);
    let out_h = height.unwrap_or(1080);

    // Use grid compositor to calculate layout
    let (grid_rows, grid_cols) =
        oximedia_multicam::composite::grid::GridCompositor::optimal_grid_for_angles(inputs.len());

    let mut grid = oximedia_multicam::composite::grid::GridCompositor::new(out_w, out_h);
    grid.set_spacing(spacing);
    let cells = grid.calculate_grid(grid_rows, grid_cols);

    let composite_result = serde_json::json!({
        "cameras": inputs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        "layout": layout,
        "layout_description": layout_desc,
        "output_width": out_w,
        "output_height": out_h,
        "grid_rows": grid_rows,
        "grid_cols": grid_cols,
        "spacing": spacing,
        "cells": cells.iter().map(|(x, y, w, h)| {
            serde_json::json!({ "x": x, "y": y, "width": w, "height": h })
        }).collect::<Vec<_>>(),
        "output": output.display().to_string(),
        "status": "composite_ready",
        "message": "Layout computed. Frame decoding pipeline needed for rendering.",
    });

    let json_str = serde_json::to_string_pretty(&composite_result)
        .context("Failed to serialize composite result")?;

    tokio::fs::write(output, json_str.as_bytes())
        .await
        .context("Failed to write output file")?;

    if json_output {
        println!("{}", json_str);
    } else {
        println!("{}", "Multi-Camera Composite".green().bold());
        println!("{}", "=".repeat(60));
        println!("{:20} {}", "Cameras:", inputs.len());
        println!("{:20} {} ({})", "Layout:", layout, layout_desc);
        println!("{:20} {}x{}", "Output size:", out_w, out_h);
        println!("{:20} {}x{}", "Grid:", grid_rows, grid_cols);
        println!("{:20} {}px", "Spacing:", spacing);
        println!("{:20} {}", "Output:", output.display());
        println!();
        println!("{}", "Cell Layout".cyan().bold());
        println!("{}", "-".repeat(40));
        for (i, (cx, cy, cw, ch)) in cells.iter().enumerate() {
            if i < inputs.len() {
                println!("  Camera {}: {}x{} at ({}, {})", i, cw, ch, cx, cy);
            }
        }
    }

    Ok(())
}

/// Match colors across camera angles.
///
/// Real color matching needs per-angle pixel statistics (mean/stddev RGB --
/// see [`oximedia_multicam::color::ColorStats`], consumed by
/// [`oximedia_multicam::color::ColorMatcher::calculate_corrections`]) derived
/// from *decoded* video frames. `oximedia-cli` has no video decode pipeline
/// reachable from this handler (the same gap documented on `captions burn`
/// in `captions_cmd.rs`), so there is no way to compute real statistics here.
///
/// Calling `ColorMatcher` with default-constructed `ColorStats` would just
/// reproduce the original bug (its defaults are `mean_rgb: [0.5, 0.5, 0.5]`,
/// `temperature: 6500.0` -- i.e. exactly the fabricated "neutral" values this
/// fix is removing), so we validate real inputs and then refuse honestly
/// instead.
// TODO(0.2.x): wire real per-angle `ColorStats` once a CLI-reachable video
// decode pipeline exists to compute them from actual frames.
async fn color_match(
    reference: &PathBuf,
    inputs: &[PathBuf],
    output_dir: &PathBuf,
    json_output: bool,
) -> Result<()> {
    if !reference.exists() {
        return Err(anyhow::anyhow!(
            "Reference file not found: {}",
            reference.display()
        ));
    }
    for input in inputs {
        if !input.exists() {
            return Err(anyhow::anyhow!("Input file not found: {}", input.display()));
        }
    }

    if json_output {
        let diag = serde_json::json!({
            "reference": reference.display().to_string(),
            "cameras": inputs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "output_dir": output_dir.display().to_string(),
            "status": "error",
            "error": "color matching requires decoded video frames; no decode pipeline is \
                      reachable from oximedia-cli in this build",
        });
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&diag).unwrap_or_else(|_| diag.to_string())
        );
    }

    Err(anyhow::anyhow!(
        "Color matching across {} camera(s) against reference '{}' is not yet implemented: \
         real corrections require per-angle color statistics (mean/stddev RGB) computed from \
         decoded video frames via oximedia_multicam::color::ColorMatcher, and oximedia-cli has \
         no video decode pipeline reachable from this handler to produce them. Refusing to \
         report \"color_match_ready\" with fabricated neutral (identity) adjustments and no \
         matched files written to '{}'.",
        inputs.len(),
        reference.display(),
        output_dir.display()
    ))
}

/// Build a CMX3600-style multi-camera EDL from a parsed timeline JSON value.
///
/// Only real fields already present in the timeline (`cameras`,
/// `switch_points`, optionally `frame_rate`) are read; nothing here is
/// fabricated placeholder data. Returns an error if the JSON doesn't look
/// like a timeline this command (or `multicam sync`/`multicam switch`)
/// produced.
fn build_multicam_edl(timeline: &serde_json::Value) -> Result<String> {
    let cameras = timeline
        .get("cameras")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Timeline JSON has no 'cameras' array; expected the output of \
                 'oximedia multicam sync' or 'oximedia multicam switch'"
            )
        })?;

    let fps = timeline
        .get("frame_rate")
        .and_then(serde_json::Value::as_f64)
        .filter(|f| *f > 0.0)
        .unwrap_or(25.0);

    let mut out = String::new();
    out.push_str("TITLE: OxiMedia Multi-Camera Export\n");
    out.push_str("FCM: NON-DROP FRAME\n\n");

    for (i, cam) in cameras.iter().enumerate() {
        let path = cam.as_str().unwrap_or("(unknown)");
        out.push_str(&format!(
            "* CAM {i}: {} <- {path}\n",
            reel_name_from_path(path, i)
        ));
    }
    out.push('\n');

    let switch_points = timeline
        .get("switch_points")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if switch_points.is_empty() {
        out.push_str("* No switch points defined in this timeline (no cuts to list)\n");
    } else {
        for (i, sp) in switch_points.iter().enumerate() {
            let time = sp
                .get("time")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);
            let cam_idx = sp
                .get("camera")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as usize;
            let reel = cameras.get(cam_idx).and_then(|c| c.as_str()).map_or_else(
                || format!("CAM{cam_idx}"),
                |p| reel_name_from_path(p, cam_idx),
            );
            let tc = seconds_to_edl_timecode(time, fps);
            out.push_str(&format!(
                "{:03}  {:<8} V     C        {tc} {tc} {tc} {tc}\n",
                i + 1,
                reel
            ));
        }
    }

    Ok(out)
}

/// Build a minimal real XML export from a parsed timeline JSON value.
///
/// Mirrors [`build_multicam_edl`]: only actual timeline fields are encoded,
/// no synthetic placeholder content.
fn build_multicam_xml(
    timeline: &serde_json::Value,
    timeline_path: &std::path::Path,
) -> Result<String> {
    let cameras = timeline
        .get("cameras")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Timeline JSON has no 'cameras' array; expected the output of \
                 'oximedia multicam sync' or 'oximedia multicam switch'"
            )
        })?;

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<multicam>\n");
    out.push_str("  <source>OxiMedia</source>\n");
    out.push_str(&format!(
        "  <timeline_file>{}</timeline_file>\n",
        xml_escape(&timeline_path.display().to_string())
    ));

    out.push_str("  <cameras>\n");
    for (i, cam) in cameras.iter().enumerate() {
        let path = cam.as_str().unwrap_or("");
        out.push_str(&format!(
            "    <camera index=\"{i}\" path=\"{}\"/>\n",
            xml_escape(path)
        ));
    }
    out.push_str("  </cameras>\n");

    let switch_points = timeline
        .get("switch_points")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    out.push_str("  <switch_points>\n");
    for sp in &switch_points {
        let time = sp
            .get("time")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let cam_idx = sp
            .get("camera")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        out.push_str(&format!(
            "    <switch time=\"{time}\" camera=\"{cam_idx}\"/>\n"
        ));
    }
    out.push_str("  </switch_points>\n");
    out.push_str("</multicam>\n");

    Ok(out)
}

/// Escape the five predefined XML entities in `s`.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Derive a short reel-style label from a camera file path, e.g.
/// `/media/cam_a.mov` -> `CAM_A`. Falls back to `CAM{index}` for paths with
/// no usable file stem.
fn reel_name_from_path(path: &str, index: usize) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map_or_else(
            || format!("CAM{index}"),
            |stem| {
                stem.chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() {
                            c.to_ascii_uppercase()
                        } else {
                            '_'
                        }
                    })
                    .take(8)
                    .collect()
            },
        )
}

/// Convert a time in seconds to an `HH:MM:SS:FF` EDL timecode at `fps`.
///
/// `fps` is clamped to a sane positive value (defaulting to 25) so the same
/// effective rate is used consistently for both the frame-count and the
/// frames-per-second modulus below.
fn seconds_to_edl_timecode(total_secs: f64, fps: f64) -> String {
    let effective_fps = if fps > 0.0 { fps } else { 25.0 };
    let fps_int = effective_fps.round().max(1.0) as u64;
    let total_frames = (total_secs.max(0.0) * effective_fps).round() as u64;
    let frames = total_frames % fps_int;
    let total_whole_secs = total_frames / fps_int;
    let s = total_whole_secs % 60;
    let m = (total_whole_secs / 60) % 60;
    let h = total_whole_secs / 3600;
    format!("{h:02}:{m:02}:{s:02}:{frames:02}")
}

/// Export multi-camera timeline.
async fn export_timeline(
    timeline: &PathBuf,
    output: &PathBuf,
    format: &str,
    json_output: bool,
) -> Result<()> {
    if !timeline.exists() {
        return Err(anyhow::anyhow!(
            "Timeline file not found: {}",
            timeline.display()
        ));
    }

    match format {
        "multicam_edl" | "xml" | "json" => {}
        other => {
            return Err(anyhow::anyhow!(
                "Unknown export format '{}'. Expected: multicam_edl, xml, json",
                other
            ));
        }
    }

    let timeline_data = tokio::fs::read_to_string(timeline)
        .await
        .context("Failed to read timeline file")?;

    // For "json" the timeline is already in the target format: pass it
    // through verbatim. For "multicam_edl"/"xml" we parse the *real*
    // timeline JSON and re-encode its actual camera list and switch points
    // -- previously these branches ignored `timeline_data` entirely and
    // wrote a fixed boilerplate string naming only the input file path.
    let export_data = match format {
        "json" => timeline_data,
        "multicam_edl" | "xml" => {
            let parsed: serde_json::Value = serde_json::from_str(&timeline_data)
                .with_context(|| {
                    format!(
                        "Timeline file '{}' is not valid JSON; cannot export its real contents to {}",
                        timeline.display(),
                        format
                    )
                })?;
            if format == "multicam_edl" {
                build_multicam_edl(&parsed)?
            } else {
                build_multicam_xml(&parsed, timeline)?
            }
        }
        _ => unreachable!("format already validated above"),
    };

    tokio::fs::write(output, export_data.as_bytes())
        .await
        .context("Failed to write export file")?;

    if json_output {
        let result = serde_json::json!({
            "timeline": timeline.display().to_string(),
            "output": output.display().to_string(),
            "format": format,
            "status": "exported",
        });
        let json_str =
            serde_json::to_string_pretty(&result).context("Failed to serialize export result")?;
        println!("{}", json_str);
    } else {
        println!("{}", "Timeline Export".green().bold());
        println!("{}", "=".repeat(60));
        println!("{:20} {}", "Timeline:", timeline.display());
        println!("{:20} {}", "Output:", output.display());
        println!("{:20} {}", "Format:", format);
    }

    Ok(())
}

/// List available multi-camera layouts.
async fn list_layouts(json_output: bool) -> Result<()> {
    let layouts = vec![
        ("grid", "Auto-sized grid layout (2x2, 3x3, etc.)"),
        ("pip", "Picture-in-picture with main view and corner inset"),
        ("side_by_side", "Horizontal split between two cameras"),
        ("stack", "Vertical stack of camera views"),
    ];

    if json_output {
        let items: Vec<serde_json::Value> = layouts
            .iter()
            .map(|(name, desc)| serde_json::json!({ "name": name, "description": desc }))
            .collect();
        let json_str =
            serde_json::to_string_pretty(&items).context("Failed to serialize layouts")?;
        println!("{}", json_str);
    } else {
        println!("{}", "Available Multi-Camera Layouts".green().bold());
        println!("{}", "=".repeat(60));
        for (name, desc) in &layouts {
            println!("  {:20} {}", name.cyan(), desc);
        }
        println!();
        println!(
            "{}",
            "Use 'oximedia multicam composite --layout <name>' to apply a layout.".dimmed()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_sync_method() {
        assert!(validate_sync_method("audio").is_ok());
        assert!(validate_sync_method("timecode").is_ok());
        assert!(validate_sync_method("marker").is_ok());
        assert!(validate_sync_method("invalid").is_err());
    }

    #[test]
    fn test_layout_description() {
        assert!(layout_description("grid").is_ok());
        assert!(layout_description("pip").is_ok());
        assert!(layout_description("side_by_side").is_ok());
        assert!(layout_description("stack").is_ok());
        assert!(layout_description("unknown").is_err());
    }

    #[test]
    fn test_layout_description_values() {
        let desc = layout_description("grid").expect("valid layout");
        assert!(desc.contains("Grid"));
    }

    #[test]
    fn test_validate_sync_method_error_message() {
        let err = validate_sync_method("xyz").expect_err("should fail");
        let msg = format!("{}", err);
        assert!(msg.contains("xyz"));
    }

    #[test]
    fn test_layout_description_pip() {
        let desc = layout_description("pip").expect("valid layout");
        assert!(desc.contains("Picture"));
    }

    // ── color_match: honest-Err, no fabricated output ───────────────────────

    #[tokio::test]
    async fn test_color_match_missing_reference_errors() {
        let dir = std::env::temp_dir();
        let reference = dir.join("oximedia_mc_test_missing_reference.mov");
        let output_dir = dir.join("oximedia_mc_test_color_match_out_1");
        let _ = std::fs::remove_file(&reference);
        let _ = std::fs::remove_dir_all(&output_dir);

        let err = color_match(&reference, &[], &output_dir, false)
            .await
            .expect_err("missing reference must fail");
        assert!(err.to_string().contains("Reference file not found"));
        assert!(
            !output_dir.exists(),
            "no output directory should be created on failure"
        );
    }

    #[tokio::test]
    async fn test_color_match_real_inputs_returns_honest_err_no_files() {
        let dir = std::env::temp_dir();
        let reference = dir.join("oximedia_mc_test_color_match_ref.mov");
        let input = dir.join("oximedia_mc_test_color_match_in.mov");
        let output_dir = dir.join("oximedia_mc_test_color_match_out_2");
        std::fs::write(
            &reference,
            b"not a real video, just bytes for existence checks",
        )
        .expect("write reference");
        std::fs::write(&input, b"not a real video either").expect("write input");
        let _ = std::fs::remove_dir_all(&output_dir);

        let err = color_match(&reference, std::slice::from_ref(&input), &output_dir, false)
            .await
            .expect_err("color_match must not fabricate success");
        let msg = err.to_string();
        assert!(
            msg.contains("not yet implemented"),
            "error should be honest about the missing decode pipeline, got: {msg}"
        );
        // The message may legitimately *name* the old status while refusing
        // it (that's honest: "refusing to report ... color_match_ready");
        // what must never reappear is the old success-shaped JSON fragment.
        assert!(
            !msg.contains("\"status\": \"color_match_ready\"")
                && !msg.contains("\"status\":\"color_match_ready\""),
            "must not resurrect the old fabricated status JSON, got: {msg}"
        );
        assert!(
            !output_dir.exists(),
            "no output directory or matched files should be fabricated"
        );

        std::fs::remove_file(&reference).ok();
        std::fs::remove_file(&input).ok();
    }

    // ── export_timeline: real EDL/XML content, not boilerplate ──────────────

    fn sample_switch_timeline() -> serde_json::Value {
        serde_json::json!({
            "cameras": ["/media/cam_a.mov", "/media/cam_b.mov"],
            "auto_switch": false,
            "min_shot_duration_secs": 2.0,
            "switch_points": [
                { "time": 0.0, "camera": 0 },
                { "time": 5.0, "camera": 1 },
            ],
            "output": "/media/out.json",
            "status": "switch_ready",
        })
    }

    #[test]
    fn test_build_multicam_edl_contains_real_camera_and_switch_data() {
        let timeline = sample_switch_timeline();
        let edl = build_multicam_edl(&timeline).expect("should build EDL from real timeline");
        assert!(
            edl.contains("cam_a.mov"),
            "must reference the real camera path"
        );
        assert!(
            edl.contains("cam_b.mov"),
            "must reference the real camera path"
        );
        assert!(edl.contains("CAM_A"), "must include a derived reel name");
        // Second switch point at 5.0s, default 25fps -> 00:00:05:00.
        assert!(
            edl.contains("00:00:05:00"),
            "must encode the real switch-point timecode, got:\n{edl}"
        );
    }

    #[test]
    fn test_build_multicam_edl_rejects_non_timeline_json() {
        let not_a_timeline = serde_json::json!({ "hello": "world" });
        let err = build_multicam_edl(&not_a_timeline).expect_err("must reject unrecognized JSON");
        assert!(err.to_string().contains("cameras"));
    }

    #[test]
    fn test_build_multicam_xml_contains_real_data() {
        let timeline = sample_switch_timeline();
        let xml = build_multicam_xml(&timeline, std::path::Path::new("/tmp/in.json"))
            .expect("should build XML from real timeline");
        assert!(xml.contains("cam_a.mov"));
        assert!(xml.contains("cam_b.mov"));
        assert!(xml.contains("time=\"5\"") || xml.contains("time=\"5.0\""));
        assert!(xml.contains("<multicam>") && xml.contains("</multicam>"));
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a & b < c"), "a &amp; b &lt; c");
    }

    #[test]
    fn test_reel_name_from_path() {
        assert_eq!(reel_name_from_path("/media/cam_a.mov", 0), "CAM_A");
        assert_eq!(reel_name_from_path("", 3), "CAM3");
    }

    #[test]
    fn test_seconds_to_edl_timecode() {
        assert_eq!(seconds_to_edl_timecode(0.0, 25.0), "00:00:00:00");
        assert_eq!(seconds_to_edl_timecode(61.0, 25.0), "00:01:01:00");
        assert_eq!(seconds_to_edl_timecode(5.0, 25.0), "00:00:05:00");
    }

    #[test]
    fn test_seconds_to_edl_timecode_invalid_fps_falls_back_consistently() {
        // fps <= 0 must fall back to a single consistent effective rate for
        // both the frame count and the frames-per-second modulus, not mix a
        // raw invalid `fps` in one place and a defaulted one in another.
        let zero = seconds_to_edl_timecode(2.0, 0.0);
        let neg = seconds_to_edl_timecode(2.0, -10.0);
        let explicit_default = seconds_to_edl_timecode(2.0, 25.0);
        assert_eq!(zero, explicit_default);
        assert_eq!(neg, explicit_default);
    }

    #[tokio::test]
    async fn test_export_timeline_edl_writes_real_content_not_boilerplate() {
        let dir = std::env::temp_dir();
        let timeline_path = dir.join("oximedia_mc_test_export_timeline.json");
        let output_path = dir.join("oximedia_mc_test_export_output.edl");
        let _ = std::fs::remove_file(&output_path);

        let timeline_json = serde_json::to_string_pretty(&sample_switch_timeline())
            .expect("serialize sample timeline");
        std::fs::write(&timeline_path, &timeline_json).expect("write sample timeline");

        export_timeline(&timeline_path, &output_path, "multicam_edl", false)
            .await
            .expect("export should succeed for a real timeline");

        let written = std::fs::read_to_string(&output_path).expect("read exported EDL");
        assert!(
            written.contains("cam_a.mov") && written.contains("cam_b.mov"),
            "exported EDL must contain the real camera paths, got:\n{written}"
        );
        assert!(
            !written.contains("Exported from OxiMedia multicam timeline"),
            "must not fall back to the old generic boilerplate line"
        );

        std::fs::remove_file(&timeline_path).ok();
        std::fs::remove_file(&output_path).ok();
    }

    #[tokio::test]
    async fn test_export_timeline_rejects_non_json_timeline_for_edl() {
        let dir = std::env::temp_dir();
        let timeline_path = dir.join("oximedia_mc_test_export_bad_timeline.json");
        let output_path = dir.join("oximedia_mc_test_export_bad_output.edl");
        std::fs::write(&timeline_path, b"not json at all").expect("write bad timeline");
        let _ = std::fs::remove_file(&output_path);

        let result = export_timeline(&timeline_path, &output_path, "multicam_edl", false).await;
        assert!(
            result.is_err(),
            "invalid JSON timeline must not export silently"
        );
        assert!(
            !output_path.exists(),
            "no output file should be written when the timeline can't be parsed"
        );

        std::fs::remove_file(&timeline_path).ok();
    }
}
