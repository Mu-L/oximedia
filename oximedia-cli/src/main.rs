//! OxiMedia CLI - Patent-free multimedia processing
//!
//! A command-line tool for working with media files using only
//! royalty-free codecs.
//!
//! # Usage
//!
//! ```bash
//! # Probe a media file
//! oximedia probe -i input.mkv
//!
//! # Transcode video
//! oximedia transcode -i input.mkv -o output.webm --codec vp9 --bitrate 2M
//!
//! # Extract frames
//! oximedia extract input.mkv frames_%04d.png
//!
//! # Batch process
//! oximedia batch input_dir/ output_dir/ config.toml
//! ```
//!
//! # Supported Formats
//!
//! OxiMedia only supports patent-free codecs:
//! - Video: AV1, VP9, VP8, Theora
//! - Audio: Opus, Vorbis, FLAC, PCM
//! - Containers: Matroska, WebM, Ogg, FLAC, WAV

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::unused_async)]
#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::similar_names)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::if_not_else)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::case_sensitive_file_extension_comparisons)]
#![allow(clippy::doc_link_with_quotes)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::needless_continue)]
#![allow(clippy::single_char_pattern)]

mod batch;
mod benchmark;
mod concat;
mod extract;
mod metadata;
mod presets;
mod progress;
mod sprite;
mod thumbnail;
mod transcode;
mod validate;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Patent-free multimedia framework CLI
#[derive(Parser)]
#[command(name = "oximedia")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output (can be used multiple times: -v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    /// Suppress all output except errors
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Output results in JSON format
    #[arg(long, global = true)]
    json: bool,
}

/// Available CLI commands.
#[derive(Subcommand)]
enum Commands {
    /// Probe media file and show information
    Probe {
        /// Input file path
        #[arg(short, long)]
        input: PathBuf,

        /// Show detailed information
        #[arg(short = 'V', long)]
        verbose: bool,

        /// Show stream information
        #[arg(short, long)]
        streams: bool,
    },

    /// Show supported formats and codecs
    Info,

    /// Transcode media file
    #[command(alias = "convert")]
    Transcode {
        /// Input file path (FFmpeg-compatible: -i)
        #[arg(short, long)]
        input: PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Use a preset (e.g., youtube-1080p, tv-4k)
        #[arg(long = "preset-name", conflicts_with_all = &["video_codec", "audio_codec", "video_bitrate", "audio_bitrate"])]
        preset_name: Option<String>,

        /// Video codec: av1, vp9, vp8 (FFmpeg-compatible: -c:v)
        #[arg(long = "codec", alias = "c:v")]
        video_codec: Option<String>,

        /// Audio codec: opus, vorbis, flac (FFmpeg-compatible: -c:a)
        #[arg(long, alias = "c:a")]
        audio_codec: Option<String>,

        /// Video bitrate (e.g., "2M", "500k") (FFmpeg-compatible: -b:v)
        #[arg(long = "bitrate", alias = "b:v")]
        video_bitrate: Option<String>,

        /// Audio bitrate (e.g., "128k") (FFmpeg-compatible: -b:a)
        #[arg(long, alias = "b:a")]
        audio_bitrate: Option<String>,

        /// Scale video (e.g., "1280:720", "1920:-1") (FFmpeg-compatible: -vf scale=)
        #[arg(long)]
        scale: Option<String>,

        /// Video filter chain (FFmpeg-compatible: -vf)
        #[arg(long, alias = "vf")]
        video_filter: Option<String>,

        /// Start time (seek) (FFmpeg-compatible: -ss)
        #[arg(long, alias = "ss")]
        start_time: Option<String>,

        /// Duration limit (FFmpeg-compatible: -t)
        #[arg(short = 't', long)]
        duration: Option<String>,

        /// Frame rate (e.g., "30", "23.976") (FFmpeg-compatible: -r)
        #[arg(short = 'r', long)]
        framerate: Option<String>,

        /// Encoder preset: ultrafast, superfast, veryfast, faster, fast, medium, slow, slower, veryslow
        #[arg(long, default_value = "medium")]
        preset: String,

        /// Enable two-pass encoding
        #[arg(long)]
        two_pass: bool,

        /// CRF quality (0-63 for VP9/VP8, 0-255 for AV1, lower is better)
        #[arg(long)]
        crf: Option<u32>,

        /// Number of threads (0 = auto)
        #[arg(long, default_value = "0")]
        threads: usize,

        /// Overwrite output file without asking
        #[arg(short = 'y', long)]
        overwrite: bool,

        /// Resume from previous incomplete encode
        #[arg(long)]
        resume: bool,
    },

    /// Extract frames from video
    Extract {
        /// Input video file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Output pattern (e.g., "frame_%04d.png")
        #[arg(value_name = "OUTPUT_PATTERN")]
        output_pattern: String,

        /// Output format: png, jpg, ppm
        #[arg(short, long)]
        format: Option<String>,

        /// Start time (seek)
        #[arg(long, alias = "ss")]
        start_time: Option<String>,

        /// Number of frames to extract
        #[arg(short = 'n', long)]
        frames: Option<usize>,

        /// Extract every Nth frame
        #[arg(long, default_value = "1")]
        every: usize,

        /// Quality for JPEG output (0-100)
        #[arg(long, default_value = "90")]
        quality: u8,
    },

    /// Batch process multiple files
    Batch {
        /// Input directory
        #[arg(value_name = "INPUT_DIR")]
        input_dir: PathBuf,

        /// Output directory
        #[arg(value_name = "OUTPUT_DIR")]
        output_dir: PathBuf,

        /// Configuration file (TOML)
        #[arg(value_name = "CONFIG")]
        config: PathBuf,

        /// Number of parallel jobs (0 = auto)
        #[arg(short, long, default_value = "0")]
        jobs: usize,

        /// Continue on errors
        #[arg(long)]
        continue_on_error: bool,

        /// Dry run (show what would be done)
        #[arg(long)]
        dry_run: bool,
    },

    /// Concatenate multiple videos
    Concat {
        /// Input files to concatenate
        #[arg(value_name = "INPUTS", required = true, num_args = 2..)]
        inputs: Vec<PathBuf>,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Concatenation method: simple, reencode, remux
        #[arg(long, default_value = "remux")]
        method: String,

        /// Validate input compatibility
        #[arg(long)]
        validate: bool,

        /// Overwrite output file without asking
        #[arg(short = 'y', long)]
        overwrite: bool,
    },

    /// Generate video thumbnails
    Thumbnail {
        /// Input video file
        #[arg(short, long)]
        input: PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Thumbnail mode: single, multiple, grid, auto
        #[arg(long, default_value = "auto")]
        mode: String,

        /// Timestamp for single mode (e.g., "30", "1:30", "1:01:30")
        #[arg(long)]
        timestamp: Option<String>,

        /// Number of thumbnails for multiple mode
        #[arg(long, default_value = "9")]
        count: usize,

        /// Grid rows for grid mode
        #[arg(long, default_value = "3")]
        rows: usize,

        /// Grid columns for grid mode
        #[arg(long, default_value = "3")]
        cols: usize,

        /// Thumbnail width (pixels)
        #[arg(long)]
        width: Option<u32>,

        /// Thumbnail height (pixels)
        #[arg(long)]
        height: Option<u32>,

        /// Output format: png, jpeg, webp
        #[arg(short, long, default_value = "png")]
        format: String,

        /// Quality for JPEG/WebP (0-100)
        #[arg(long, default_value = "90")]
        quality: u8,
    },

    /// Generate video thumbnail sprite sheet
    Sprite {
        /// Input video file
        #[arg(short, long)]
        input: PathBuf,

        /// Output sprite sheet file path
        #[arg(short, long)]
        output: PathBuf,

        /// Time interval between thumbnails in seconds (e.g., "10", "30")
        #[arg(long, conflicts_with = "count")]
        interval: Option<String>,

        /// Total number of thumbnails to generate
        #[arg(long, conflicts_with = "interval")]
        count: Option<usize>,

        /// Number of columns in grid
        #[arg(long, default_value = "5")]
        cols: usize,

        /// Number of rows in grid
        #[arg(long, default_value = "5")]
        rows: usize,

        /// Thumbnail width in pixels
        #[arg(long, default_value = "160")]
        width: u32,

        /// Thumbnail height in pixels
        #[arg(long, default_value = "90")]
        height: u32,

        /// Output format: png, jpeg, webp
        #[arg(short, long, default_value = "png")]
        format: String,

        /// Quality for JPEG/WebP (0-100)
        #[arg(long, default_value = "90")]
        quality: u8,

        /// Compression level (0-9)
        #[arg(long, default_value = "6")]
        compression: u8,

        /// Sampling strategy: uniform, scene-based, keyframe-only, smart
        #[arg(long, default_value = "uniform")]
        strategy: String,

        /// Layout mode: grid, vertical, horizontal, auto
        #[arg(long, default_value = "grid")]
        layout: String,

        /// Spacing between thumbnails in pixels
        #[arg(long, default_value = "2")]
        spacing: u32,

        /// Margin around sprite sheet in pixels
        #[arg(long, default_value = "0")]
        margin: u32,

        /// Generate WebVTT file for seeking
        #[arg(long)]
        vtt: bool,

        /// WebVTT output file path (default: `<output>`.vtt)
        #[arg(long, requires = "vtt")]
        vtt_output: Option<PathBuf>,

        /// Generate JSON manifest
        #[arg(long)]
        manifest: bool,

        /// JSON manifest output path (default: `<output>`.json)
        #[arg(long, requires = "manifest")]
        manifest_output: Option<PathBuf>,

        /// Show timestamps on thumbnails
        #[arg(long)]
        timestamps: bool,

        /// Maintain aspect ratio when scaling
        #[arg(long, default_value = "true")]
        aspect: bool,
    },

    /// Edit media metadata/tags
    Metadata {
        /// Input file
        #[arg(short, long)]
        input: PathBuf,

        /// Output file (defaults to input if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Show current metadata
        #[arg(long)]
        show: bool,

        /// Set metadata field (can be used multiple times: --set title="My Title")
        #[arg(long, value_parser = parse_key_val)]
        set: Vec<(String, String)>,

        /// Remove metadata field (can be used multiple times)
        #[arg(long)]
        remove: Vec<String>,

        /// Clear all metadata
        #[arg(long)]
        clear: bool,

        /// Copy metadata from another file
        #[arg(long)]
        copy_from: Option<PathBuf>,
    },

    /// Run encoding benchmarks
    Benchmark {
        /// Input file for benchmarking
        #[arg(short, long)]
        input: PathBuf,

        /// Codecs to test (e.g., av1, vp9, vp8)
        #[arg(long, default_values = &["av1", "vp9"])]
        codecs: Vec<String>,

        /// Presets to test (e.g., fast, medium, slow)
        #[arg(long, default_values = &["fast", "medium", "slow"])]
        presets: Vec<String>,

        /// Duration to encode in seconds (0 = full file)
        #[arg(long)]
        duration: Option<u64>,

        /// Number of iterations per configuration
        #[arg(long, default_value = "1")]
        iterations: usize,

        /// Output directory for benchmark files
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },

    /// Validate file integrity
    Validate {
        /// Input files to validate
        #[arg(value_name = "INPUTS", required = true)]
        inputs: Vec<PathBuf>,

        /// Validation checks: format, codec, stream, corruption, metadata, all
        #[arg(long, default_values = &["all"])]
        checks: Vec<String>,

        /// Strict mode (fail on warnings)
        #[arg(long)]
        strict: bool,

        /// Attempt to fix issues
        #[arg(long)]
        fix: bool,
    },

    /// Manage transcoding presets
    Preset {
        #[command(subcommand)]
        command: PresetCommand,
    },
}

/// Preset management subcommands.
#[derive(Subcommand)]
enum PresetCommand {
    /// List all available presets
    List {
        /// Filter by category (web, device, quality, archival, streaming, custom)
        #[arg(short, long)]
        category: Option<String>,

        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Show detailed information about a preset
    Show {
        /// Preset name
        #[arg(value_name = "NAME")]
        name: String,

        /// Output as TOML
        #[arg(long)]
        toml: bool,
    },

    /// Create a new custom preset interactively
    Create {
        /// Output directory for custom presets
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate a preset template file
    Template {
        /// Output file path
        #[arg(value_name = "OUTPUT")]
        output: PathBuf,
    },

    /// Import a preset from a TOML file
    Import {
        /// Input TOML file
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },

    /// Export a preset to a TOML file
    Export {
        /// Preset name
        #[arg(value_name = "NAME")]
        name: String,

        /// Output file path
        #[arg(value_name = "OUTPUT")]
        output: PathBuf,
    },

    /// Remove a custom preset
    Remove {
        /// Preset name
        #[arg(value_name = "NAME")]
        name: String,
    },
}

/// Parse key=value pairs for metadata setting.
fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("Invalid KEY=value: no `=` found in `{}`", s))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

/// Main entry point for the OxiMedia CLI.
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(cli.verbose, cli.quiet)?;

    // Disable colors if requested
    if cli.no_color {
        colored::control::set_override(false);
    }

    // Execute command
    let result = match cli.command {
        Commands::Probe {
            input,
            verbose,
            streams,
        } => probe_file(&input, verbose, streams).await,

        Commands::Info => {
            show_info();
            Ok(())
        }

        Commands::Transcode {
            input,
            output,
            preset_name,
            video_codec,
            audio_codec,
            video_bitrate,
            audio_bitrate,
            scale,
            video_filter,
            start_time,
            duration,
            framerate,
            preset,
            two_pass,
            crf,
            threads,
            overwrite,
            resume,
        } => {
            let options = transcode::TranscodeOptions {
                input,
                output,
                preset_name,
                video_codec,
                audio_codec,
                video_bitrate,
                audio_bitrate,
                scale,
                video_filter,
                start_time,
                duration,
                framerate,
                preset,
                two_pass,
                crf,
                threads,
                overwrite,
                resume,
            };
            transcode::transcode(options).await
        }

        Commands::Extract {
            input,
            output_pattern,
            format,
            start_time,
            frames,
            every,
            quality,
        } => {
            let options = extract::ExtractOptions {
                input,
                output_pattern,
                format,
                start_time,
                frames,
                every,
                quality,
            };
            extract::extract_frames(options).await
        }

        Commands::Batch {
            input_dir,
            output_dir,
            config,
            jobs,
            continue_on_error,
            dry_run,
        } => {
            let options = batch::BatchOptions {
                input_dir,
                output_dir,
                config,
                jobs,
                continue_on_error,
                dry_run,
            };
            batch::batch_process(options).await
        }

        Commands::Concat {
            inputs,
            output,
            method,
            validate,
            overwrite,
        } => {
            let concat_method = concat::ConcatMethod::from_str(&method)?;
            let options = concat::ConcatOptions {
                inputs,
                output,
                method: concat_method,
                validate,
                overwrite,
                json_output: cli.json,
                transition: concat::TransitionType::CleanCut,
                chapter_options: concat::ChapterOptions::default(),
                stream_selection: None,
                trim_ranges: Vec::new(),
                edl_file: None,
                force_format: None,
                keyframe_align: false,
                max_audio_desync_ms: 100.0,
            };
            concat::concat_videos(options).await
        }

        Commands::Thumbnail {
            input,
            output,
            mode,
            timestamp,
            count,
            rows,
            cols,
            width,
            height,
            format,
            quality,
        } => {
            let thumb_mode = match mode.to_lowercase().as_str() {
                "single" => {
                    let ts = if let Some(ref ts_str) = timestamp {
                        thumbnail::parse_timestamp(ts_str)?
                    } else {
                        return Err(anyhow::anyhow!(
                            "Timestamp is required for single mode (use --timestamp)"
                        ));
                    };
                    thumbnail::ThumbnailMode::Single { timestamp: ts }
                }
                "multiple" => thumbnail::ThumbnailMode::Multiple { count },
                "grid" => thumbnail::ThumbnailMode::Grid { rows, cols },
                "auto" => thumbnail::ThumbnailMode::Auto,
                _ => {
                    return Err(anyhow::anyhow!(
                        "Invalid mode. Use: single, multiple, grid, or auto"
                    ))
                }
            };

            let thumb_format = thumbnail::ThumbnailFormat::from_str(&format)?;

            let options = thumbnail::ThumbnailOptions {
                input,
                output,
                mode: thumb_mode,
                width,
                height,
                quality,
                format: thumb_format,
                json_output: cli.json,
            };
            thumbnail::generate_thumbnails(options).await
        }

        Commands::Sprite {
            input,
            output,
            interval,
            count,
            cols,
            rows,
            width,
            height,
            format,
            quality,
            compression,
            strategy,
            layout,
            spacing,
            margin,
            vtt,
            vtt_output,
            manifest,
            manifest_output,
            timestamps,
            aspect,
        } => {
            // Parse interval if provided
            let interval_secs = if let Some(ref interval_str) = interval {
                Some(sprite::parse_duration(interval_str)?)
            } else {
                None
            };

            // Parse format
            let img_format = sprite::ImageFormat::from_str(&format)?;

            // Parse sampling strategy
            let sampling_strategy = sprite::SamplingStrategy::from_str(&strategy)?;

            // Parse layout mode
            let layout_mode = sprite::LayoutMode::from_str(&layout)?;

            // Create sprite sheet configuration
            let mut config = sprite::SpriteSheetConfig {
                interval: interval_secs,
                count,
                thumbnail_width: width,
                thumbnail_height: height,
                columns: cols,
                rows,
                format: img_format,
                quality,
                strategy: sampling_strategy,
                layout: layout_mode,
                spacing,
                margin,
                maintain_aspect_ratio: aspect,
                compression,
            };

            // Validate and adjust configuration
            config = sprite::validate_and_adjust_config(config)?;

            // Create options
            let options = sprite::SpriteSheetOptions {
                input,
                output,
                config,
                generate_vtt: vtt,
                vtt_output,
                generate_manifest: manifest,
                manifest_output,
                show_timestamps: timestamps,
                json_output: cli.json,
            };

            sprite::generate_sprite_sheet(options).await
        }

        Commands::Metadata {
            input,
            output,
            show,
            set,
            remove,
            clear,
            copy_from,
        } => {
            use std::collections::HashMap;

            let operation = if show {
                metadata::MetadataOperation::Show
            } else if !set.is_empty() {
                let mut fields = HashMap::new();
                for (key, value) in set {
                    fields.insert(key, value);
                }
                metadata::MetadataOperation::Set { fields }
            } else if !remove.is_empty() {
                metadata::MetadataOperation::Remove { fields: remove }
            } else if clear {
                metadata::MetadataOperation::Clear
            } else if let Some(source) = copy_from {
                metadata::MetadataOperation::Copy { source }
            } else {
                // Default to show if no operation specified
                metadata::MetadataOperation::Show
            };

            let options = metadata::MetadataOptions {
                input,
                output,
                operation,
                json_output: cli.json,
            };
            metadata::manage_metadata(options).await
        }

        Commands::Benchmark {
            input,
            codecs,
            presets,
            duration,
            iterations,
            output_dir,
        } => {
            let options = benchmark::BenchmarkOptions {
                input,
                codecs,
                presets,
                duration,
                iterations,
                output_dir,
                json_output: cli.json,
            };
            benchmark::run_benchmark(options).await
        }

        Commands::Validate {
            inputs,
            checks,
            strict,
            fix,
        } => {
            let validation_checks: Result<Vec<validate::ValidationCheck>> = checks
                .iter()
                .map(|s| validate::ValidationCheck::from_str(s))
                .collect();

            let options = validate::ValidateOptions {
                inputs,
                checks: validation_checks?,
                strict,
                fix,
                json_output: cli.json,
            };
            validate::validate_files(options).await
        }

        Commands::Preset { command } => handle_preset_command(command, cli.json).await,
    };

    // Handle errors with colored output
    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        if let Some(source) = e.source() {
            eprintln!("{} {}", "Caused by:".yellow(), source);
        }
        std::process::exit(1);
    }

    Ok(())
}

/// Initialize logging based on verbosity level
fn init_logging(verbose: u8, quiet: bool) -> Result<()> {
    if quiet {
        // No logging in quiet mode
        return Ok(());
    }

    let level = match verbose {
        0 => Level::INFO,
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .compact()
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set tracing subscriber")?;

    Ok(())
}

/// Probe a media file and display format information.
async fn probe_file(path: &PathBuf, verbose: bool, show_streams: bool) -> Result<()> {
    use tokio::io::AsyncReadExt;

    info!("Probing file: {}", path.display());

    // Read first 4KB for probing
    let mut file = tokio::fs::File::open(path)
        .await
        .context("Failed to open input file")?;

    let mut buffer = vec![0u8; 4096];
    let bytes_read = file
        .read(&mut buffer)
        .await
        .context("Failed to read file")?;
    buffer.truncate(bytes_read);

    match oximedia_container::probe_format(&buffer) {
        Ok(result) => {
            println!("{}", "Format Information".green().bold());
            println!("{}", "=".repeat(50));
            println!("{:20} {:?}", "Container:", result.format);
            println!("{:20} {:.1}%", "Confidence:", result.confidence * 100.0);

            if verbose {
                println!("\n{}", "Technical Details".cyan().bold());
                println!("{}", "-".repeat(50));
                println!(
                    "{:20} {} bytes",
                    "File size:",
                    tokio::fs::metadata(path).await?.len()
                );
                println!(
                    "{:20} {:02X?}",
                    "Magic bytes:",
                    &buffer[..16.min(buffer.len())]
                );
            }

            if show_streams {
                println!("\n{}", "Stream Information".cyan().bold());
                println!("{}", "-".repeat(50));
                println!("(Stream parsing not yet implemented)");
            }

            Ok(())
        }
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            Err(anyhow::anyhow!("Could not detect format: {}", e))
        }
    }
}

/// Handle preset subcommands.
async fn handle_preset_command(command: PresetCommand, json_output: bool) -> Result<()> {
    use presets::{PresetCategory, PresetManager};

    let custom_dir = PresetManager::default_custom_dir()?;
    let manager = PresetManager::with_custom_dir(&custom_dir)?;

    match command {
        PresetCommand::List { category, verbose } => {
            let presets = if let Some(cat_str) = category {
                let cat = PresetCategory::from_str(&cat_str)?;
                manager.list_presets_by_category(cat)
            } else {
                manager.list_presets()
            };

            if json_output {
                let json = serde_json::to_string_pretty(&presets)?;
                println!("{}", json);
            } else {
                println!("{}", "Available Presets".green().bold());
                println!("{}", "=".repeat(80));
                println!();

                let mut current_category = None;
                for preset in presets {
                    if current_category != Some(preset.category) {
                        current_category = Some(preset.category);
                        println!("{}", preset.category.name().cyan().bold());
                        println!("{}", preset.category.description().dimmed());
                        println!();
                    }

                    let builtin_badge = if preset.builtin {
                        "[built-in]".dimmed()
                    } else {
                        "[custom]".yellow()
                    };

                    println!("  {} {}", preset.name.green(), builtin_badge);

                    if verbose {
                        println!("    {}", preset.description);
                        println!(
                            "    Video: {} @ {}",
                            preset.video.codec,
                            preset
                                .video
                                .bitrate
                                .as_ref()
                                .map(|s| s.as_str())
                                .unwrap_or("CRF")
                        );
                        println!(
                            "    Audio: {} @ {}",
                            preset.audio.codec,
                            preset
                                .audio
                                .bitrate
                                .as_ref()
                                .map(|s| s.as_str())
                                .unwrap_or("default")
                        );
                        println!("    Container: {}", preset.container);
                        if !preset.tags.is_empty() {
                            println!("    Tags: {}", preset.tags.join(", "));
                        }
                        println!();
                    }
                }

                println!();
                println!("Total: {} presets", manager.preset_names().len());
                println!();
                println!(
                    "Use {} to see detailed information",
                    "oximedia preset show <name>".yellow()
                );
            }

            Ok(())
        }

        PresetCommand::Show { name, toml } => {
            let preset = manager.get_preset(&name)?;

            if json_output {
                let json = serde_json::to_string_pretty(preset)?;
                println!("{}", json);
            } else if toml {
                // Save to temp and read back
                let temp_dir = std::env::temp_dir();
                presets::custom::save_preset_to_file(preset, &temp_dir)?;
                let toml_path = temp_dir.join(format!("{}.toml", preset.name));
                let toml_content = std::fs::read_to_string(&toml_path)?;
                println!("{}", toml_content);
                let _ignore = std::fs::remove_file(&toml_path);
            } else {
                println!("{}", format!("Preset: {}", preset.name).green().bold());
                println!("{}", "=".repeat(80));
                println!();

                println!("{}: {}", "Description".cyan().bold(), preset.description);
                println!("{}: {}", "Category".cyan().bold(), preset.category.name());
                println!("{}: {}", "Container".cyan().bold(), preset.container);
                println!(
                    "{}: {}",
                    "Type".cyan().bold(),
                    if preset.builtin { "Built-in" } else { "Custom" }
                );

                if !preset.tags.is_empty() {
                    println!("{}: {}", "Tags".cyan().bold(), preset.tags.join(", "));
                }

                println!();
                println!("{}", "Video Configuration".yellow().bold());
                println!("{}", "-".repeat(40));
                println!("  Codec: {}", preset.video.codec);
                if let Some(ref bitrate) = preset.video.bitrate {
                    println!("  Bitrate: {}", bitrate);
                }
                if let Some(crf) = preset.video.crf {
                    println!("  CRF: {}", crf);
                }
                if let Some(width) = preset.video.width {
                    println!(
                        "  Resolution: {}x{}",
                        width,
                        preset.video.height.unwrap_or(0)
                    );
                }
                if let Some(fps) = preset.video.fps {
                    println!("  Frame rate: {}", fps);
                }
                if let Some(ref preset_name) = preset.video.preset {
                    println!("  Encoder preset: {}", preset_name);
                }
                if let Some(ref pix_fmt) = preset.video.pixel_format {
                    println!("  Pixel format: {}", pix_fmt);
                }
                println!("  Two-pass: {}", preset.video.two_pass);

                println!();
                println!("{}", "Audio Configuration".yellow().bold());
                println!("{}", "-".repeat(40));
                println!("  Codec: {}", preset.audio.codec);
                if let Some(ref bitrate) = preset.audio.bitrate {
                    println!("  Bitrate: {}", bitrate);
                }
                if let Some(sample_rate) = preset.audio.sample_rate {
                    println!("  Sample rate: {} Hz", sample_rate);
                }
                if let Some(channels) = preset.audio.channels {
                    println!("  Channels: {}", channels);
                }

                println!();
                println!(
                    "{}",
                    format!(
                        "oximedia transcode -i input.mkv -o output.{} --preset-name {}",
                        preset.container, preset.name
                    )
                    .yellow()
                );
            }

            Ok(())
        }

        PresetCommand::Create { output } => {
            let preset = presets::custom::create_preset_interactive()?;

            let out_dir = output.unwrap_or(custom_dir);
            if !out_dir.exists() {
                std::fs::create_dir_all(&out_dir)?;
            }

            presets::custom::save_preset_to_file(&preset, &out_dir)?;

            println!(
                "{} Preset '{}' created successfully!",
                "✓".green(),
                preset.name
            );
            println!(
                "Saved to: {}",
                out_dir.join(format!("{}.toml", preset.name)).display()
            );

            Ok(())
        }

        PresetCommand::Template { output } => {
            presets::custom::generate_template(&output)?;
            println!("{} Template generated: {}", "✓".green(), output.display());
            println!(
                "Edit the template and import it with: oximedia preset import {}",
                output.display()
            );
            Ok(())
        }

        PresetCommand::Import { file } => {
            let preset = presets::custom::load_preset_from_file(&file)?;

            if !custom_dir.exists() {
                std::fs::create_dir_all(&custom_dir)?;
            }

            presets::custom::save_preset_to_file(&preset, &custom_dir)?;

            println!(
                "{} Preset '{}' imported successfully!",
                "✓".green(),
                preset.name
            );

            Ok(())
        }

        PresetCommand::Export { name, output } => {
            let preset = manager.get_preset(&name)?;

            if preset.builtin {
                println!(
                    "{} Cannot export built-in preset '{}'. Use 'oximedia preset show {} --toml' instead.",
                    "!".yellow(),
                    name,
                    name
                );
                return Ok(());
            }

            let output_dir = output.parent().unwrap_or_else(|| std::path::Path::new("."));
            presets::custom::save_preset_to_file(preset, output_dir)?;

            println!("{} Preset exported to: {}", "✓".green(), output.display());

            Ok(())
        }

        PresetCommand::Remove { name } => {
            let preset = manager.get_preset(&name)?;

            if preset.builtin {
                return Err(anyhow::anyhow!("Cannot remove built-in preset '{}'", name));
            }

            let preset_path = custom_dir.join(format!("{}.toml", name));
            if preset_path.exists() {
                std::fs::remove_file(&preset_path)?;
                println!("{} Preset '{}' removed successfully!", "✓".green(), name);
            } else {
                println!(
                    "{} Preset '{}' not found in custom directory",
                    "!".yellow(),
                    name
                );
            }

            Ok(())
        }
    }
}

/// Display information about supported formats and codecs.
fn show_info() {
    println!(
        "{}",
        "OxiMedia - Patent-Free Multimedia Framework".green().bold()
    );
    println!();

    println!("{}", "Supported Containers:".cyan().bold());
    println!("  {} Matroska (.mkv)", "✓".green());
    println!("  {} WebM (.webm)", "✓".green());
    println!("  {} Ogg (.ogg, .opus, .oga)", "✓".green());
    println!("  {} FLAC (.flac)", "✓".green());
    println!("  {} WAV (.wav)", "✓".green());
    println!();

    println!("{}", "Supported Video Codecs (Green List):".cyan().bold());
    println!("  {} AV1 (Primary codec, best compression)", "✓".green());
    println!("  {} VP9 (Excellent quality/size ratio)", "✓".green());
    println!("  {} VP8 (Legacy support)", "✓".green());
    println!("  {} Theora (Legacy support)", "✓".green());
    println!();

    println!("{}", "Supported Audio Codecs (Green List):".cyan().bold());
    println!("  {} Opus (Primary codec, versatile)", "✓".green());
    println!("  {} Vorbis (High quality)", "✓".green());
    println!("  {} FLAC (Lossless)", "✓".green());
    println!("  {} PCM (Uncompressed)", "✓".green());
    println!();

    println!("{}", "Rejected Codecs (Patent-Encumbered):".red().bold());
    println!("  {} H.264/AVC", "✗".red());
    println!("  {} H.265/HEVC", "✗".red());
    println!("  {} AAC", "✗".red());
    println!("  {} AC-3/E-AC-3", "✗".red());
    println!("  {} DTS", "✗".red());
    println!();

    println!("{}", "FFmpeg-Compatible Options:".yellow().bold());
    println!("  -i <file>          Input file");
    println!("  -c:v <codec>       Video codec (av1, vp9, vp8)");
    println!("  -c:a <codec>       Audio codec (opus, vorbis, flac)");
    println!("  -b:v <bitrate>     Video bitrate (e.g., 2M, 500k)");
    println!("  -vf <filter>       Video filter chain");
    println!("  -ss <time>         Seek to start time");
    println!("  -t <duration>      Duration limit");
    println!("  -r <fps>           Frame rate");
    println!();

    println!("{}", "Examples:".yellow().bold());
    println!("  oximedia transcode -i input.mp4 -o output.webm -c:v vp9 -b:v 2M");
    println!("  oximedia transcode -i input.mp4 -o output.webm --preset-name youtube-1080p");
    println!("  oximedia extract video.mkv frames_%04d.png --every 30");
    println!("  oximedia batch input/ output/ config.toml -j 4");
    println!("  oximedia concat video1.mkv video2.mkv -o output.mkv --method remux");
    println!("  oximedia thumbnail -i video.mkv -o thumb.png --mode auto");
    println!("  oximedia sprite -i video.mkv -o sprite.png --interval 10 --cols 5 --rows 5");
    println!("  oximedia sprite -i video.mkv -o sprite.png --count 100 --vtt --manifest");
    println!("  oximedia metadata -i video.mkv --show");
    println!("  oximedia benchmark -i test.mkv --codecs av1 vp9 --presets fast medium");
    println!("  oximedia validate video1.mkv video2.mkv --checks all --strict");
    println!("  oximedia preset list --category web");
    println!("  oximedia preset show youtube-1080p");
    println!();

    println!("{}", "Available Commands:".cyan().bold());
    println!(
        "  {} Probe media files and show format information",
        "probe".green()
    );
    println!("  {} Show supported formats and codecs", "info".green());
    println!("  {} Transcode media files", "transcode".green());
    println!("  {} Extract frames to images", "extract".green());
    println!("  {} Batch process multiple files", "batch".green());
    println!("  {} Concatenate multiple videos", "concat".green());
    println!("  {} Generate video thumbnails", "thumbnail".green());
    println!("  {} Generate video sprite sheets", "sprite".green());
    println!("  {} Edit media metadata/tags", "metadata".green());
    println!("  {} Run encoding benchmarks", "benchmark".green());
    println!("  {} Validate file integrity", "validate".green());
    println!("  {} Manage transcoding presets", "preset".green());
}
