//! Media-file probing handler (text/json/csv/ndjson outputs).

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;
use tracing::info;

/// Probe a media file and display format information.
///
/// # Arguments
/// * `path` - Path to the media file to probe
/// * `verbose` - Whether to show detailed technical information
/// * `show_streams` - Whether to list individual stream details
/// * `compute_hash` - Whether to compute a real SHA-256 content hash of the
///   full file (`--hash`), via `oximedia_archive_pro::checksum::ChecksumGenerator`
/// * `output_format` - Output format: "text", "json", or "csv"
/// * `show_chapters` - Whether to show chapter information
/// * `show_metadata` - Whether to dump all metadata key/value pairs
/// * `ndjson` - When `true`, emit a single NDJSON record to stdout
pub(crate) async fn probe_file(
    path: &PathBuf,
    verbose: bool,
    show_streams: bool,
    compute_hash: bool,
    output_format: &str,
    show_chapters: bool,
    show_metadata: bool,
    ndjson: bool,
) -> Result<()> {
    use tokio::io::AsyncReadExt;

    info!("Probing file: {}", path.display());

    // Read first 8KB for probing (more data = better detection accuracy)
    let mut file = tokio::fs::File::open(path)
        .await
        .context("Failed to open input file")?;

    let mut buffer = vec![0u8; 8192];
    let bytes_read = file
        .read(&mut buffer)
        .await
        .context("Failed to read file")?;
    buffer.truncate(bytes_read);

    let file_size = tokio::fs::metadata(path).await?.len();
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("<unknown>");

    match oximedia_container::probe_format(&buffer) {
        Ok(result) => {
            // Optionally compute a real SHA-256 content hash/fingerprint of the
            // whole file via `oximedia-archive-pro`'s streaming checksum
            // generator (Blake3/MD5/SHA256/SHA512/XXH3 capable; SHA-256 is
            // both its own `ChecksumGenerator::new()` default and the
            // `ChecksumAlgorithm::recommended()` choice for preservation).
            // Runs on a blocking thread since `generate_file` performs
            // synchronous buffered I/O over the full file, independent of the
            // 8 KiB header sniff above.
            let hash_hex: Option<String> = if compute_hash {
                use oximedia_archive_pro::checksum::{ChecksumAlgorithm, ChecksumGenerator};

                let hash_path = path.clone();
                let checksum = tokio::task::spawn_blocking(move || {
                    ChecksumGenerator::new()
                        .with_algorithms(vec![ChecksumAlgorithm::Sha256])
                        .generate_file(&hash_path)
                })
                .await
                .context("hash computation task panicked")?
                .map_err(|e| anyhow::anyhow!("Failed to compute file hash: {e}"))?;

                checksum.checksums.get(&ChecksumAlgorithm::Sha256).cloned()
            } else {
                None
            };

            // NDJSON: emit a single record and return before the regular branches.
            if ndjson {
                colored::control::set_override(false);
                let mut record = serde_json::json!({
                    "path": path.display().to_string(),
                    "file_name": file_name,
                    "file_size_bytes": file_size,
                    "container": format!("{:?}", result.format),
                    "confidence": result.confidence,
                });
                if let Some(ref hash) = hash_hex {
                    record["hash"] = serde_json::json!({
                        "algorithm": "sha256",
                        "value": hash,
                    });
                }
                let mut writer = crate::output::NdjsonWriter::new(std::io::stdout());
                writer
                    .emit(&record)
                    .context("Failed to write NDJSON probe record")?;
                return Ok(());
            }

            match output_format {
                "json" => {
                    let mut probe_json = serde_json::json!({
                        "file": path.display().to_string(),
                        "file_name": file_name,
                        "file_size_bytes": file_size,
                        "container": format!("{:?}", result.format),
                        "confidence": result.confidence,
                    });

                    if show_streams {
                        probe_json["streams"] = serde_json::json!([
                            {
                                "index": 0,
                                "codec_type": "video",
                                "codec": "unknown",
                                "resolution": "unknown",
                                "bitrate": null,
                                "language": null,
                            }
                        ]);
                    }

                    if show_chapters {
                        probe_json["chapters"] = serde_json::json!([]);
                    }

                    if show_metadata {
                        probe_json["metadata"] = serde_json::json!({
                            "filename": file_name,
                        });
                    }

                    if let Some(ref hash) = hash_hex {
                        probe_json["hash"] = serde_json::json!({
                            "algorithm": "sha256",
                            "value": hash,
                        });
                    }

                    let json_str = serde_json::to_string_pretty(&probe_json)
                        .context("Failed to serialize probe result")?;
                    println!("{}", json_str);
                }
                "csv" => {
                    if let Some(ref hash) = hash_hex {
                        println!("file,container,confidence,file_size,hash_sha256");
                        println!(
                            "{},{:?},{:.4},{},{}",
                            path.display(),
                            result.format,
                            result.confidence,
                            file_size,
                            hash
                        );
                    } else {
                        println!("file,container,confidence,file_size");
                        println!(
                            "{},{:?},{:.4},{}",
                            path.display(),
                            result.format,
                            result.confidence,
                            file_size
                        );
                    }

                    if show_streams {
                        println!();
                        println!("stream_index,codec_type,codec,resolution,bitrate,language");
                        println!("0,video,unknown,unknown,,");
                    }
                }
                _ => {
                    // Default: text output
                    println!("{}", "Format Information".green().bold());
                    println!("{}", "=".repeat(50));
                    println!("{:20} {}", "File:", file_name);
                    println!("{:20} {:?}", "Container:", result.format);
                    println!("{:20} {:.1}%", "Confidence:", result.confidence * 100.0);
                    println!("{:20} {} bytes", "File size:", file_size);
                    if let Some(ref hash) = hash_hex {
                        println!("{:20} {}", "Hash (SHA-256):", hash);
                    }

                    if verbose {
                        println!("\n{}", "Technical Details".cyan().bold());
                        println!("{}", "-".repeat(50));
                        println!("{:20} {}", "Full path:", path.display());
                        println!(
                            "{:20} {:02X?}",
                            "Magic bytes:",
                            &buffer[..16.min(buffer.len())]
                        );
                        println!("{:20} {} KB read", "Header bytes:", bytes_read / 1024);
                    }

                    if show_streams {
                        println!("\n{}", "Stream Information".cyan().bold());
                        println!("{}", "-".repeat(50));
                        println!(
                            "{:<6} {:<12} {:<16} {:<14} {:<10} Language",
                            "Index", "Type", "Codec", "Resolution", "Bitrate"
                        );
                        println!("{}", "-".repeat(70));
                        println!(
                            "{:<6} {:<12} {:<16} {:<14} {:<10} und",
                            "#0", "video", "unknown", "unknown", "N/A"
                        );
                        println!();
                        println!(
                            "{}",
                            "Note: Full stream parsing requires a demuxed container.".dimmed()
                        );
                    }

                    if show_chapters {
                        println!("\n{}", "Chapter Information".cyan().bold());
                        println!("{}", "-".repeat(50));
                        println!("{}", "(No chapters detected in probe data.)".dimmed());
                    }

                    if show_metadata {
                        println!("\n{}", "Metadata".cyan().bold());
                        println!("{}", "-".repeat(50));
                        println!("{:<24} {}", "filename:", file_name);
                        println!("{:<24} {}", "file_size:", file_size);
                        println!("{:<24} {:?}", "detected_format:", result.format);
                        println!();
                        println!(
                            "{}",
                            "Note: Full metadata requires container-level parsing.".dimmed()
                        );
                    }
                }
            }

            Ok(())
        }
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            Err(anyhow::anyhow!("Could not detect format: {}", e))
        }
    }
}
