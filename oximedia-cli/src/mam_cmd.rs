//! Media Asset Management (MAM) CLI commands.
//!
//! Provides commands for ingesting, searching, cataloging, exporting, and tagging
//! media assets in a local catalog database (JSON-based).

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Command definitions
// ---------------------------------------------------------------------------

/// MAM command subcommands.
#[derive(Subcommand, Debug)]
pub enum MamCommand {
    /// Ingest media files into the asset catalog
    Ingest {
        /// Input file(s) to ingest
        #[arg(short, long, required = true)]
        input: Vec<PathBuf>,

        /// Catalog database path (JSON file)
        #[arg(long)]
        catalog: PathBuf,

        /// Comma-separated tags to apply
        #[arg(long)]
        tags: Option<String>,

        /// Collection name to assign
        #[arg(long)]
        collection: Option<String>,

        /// Recursively scan directories
        #[arg(long)]
        recursive: bool,

        /// Generate proxy files for previews (not implemented yet: warns and
        /// proceeds; use `oximedia proxy generate` instead)
        #[arg(long)]
        generate_proxy: bool,

        /// Probe each file and store technical metadata (container format,
        /// codec, dimensions, duration) on the asset record
        #[arg(long)]
        extract_metadata: bool,
    },

    /// Search the asset catalog
    Search {
        /// Catalog database path (JSON file)
        #[arg(short, long)]
        catalog: PathBuf,

        /// Search query string
        #[arg(short = 'Q', long)]
        query: String,

        /// Filter by comma-separated tags
        #[arg(long)]
        tags: Option<String>,

        /// Filter by format (e.g., mkv, webm, flac)
        #[arg(long)]
        format: Option<String>,

        /// Filter by date (from), ISO 8601
        #[arg(long)]
        date_from: Option<String>,

        /// Filter by date (to), ISO 8601
        #[arg(long)]
        date_to: Option<String>,

        /// Maximum number of results
        #[arg(long)]
        limit: Option<u32>,

        /// Sort by: relevance, name, date, size
        #[arg(long, default_value = "relevance")]
        sort: String,
    },

    /// Show catalog summary and statistics
    Catalog {
        /// Catalog database path (JSON file)
        #[arg(short, long)]
        catalog: PathBuf,

        /// Show detailed statistics
        #[arg(long)]
        stats: bool,

        /// Detect and report duplicate assets
        #[arg(long)]
        duplicates: bool,
    },

    /// Export assets from catalog
    Export {
        /// Catalog database path (JSON file)
        #[arg(short, long)]
        catalog: PathBuf,

        /// Output directory for exported assets
        #[arg(short, long)]
        output: PathBuf,

        /// Filter assets by query
        #[arg(long)]
        query: Option<String>,

        /// Filter assets by collection
        #[arg(long)]
        collection: Option<String>,

        /// Export mode: copy, move, link
        #[arg(long, default_value = "copy")]
        mode: String,

        /// Manifest format: json, csv
        #[arg(long, default_value = "json")]
        manifest_format: String,
    },

    /// Add or modify tags on assets
    Tag {
        /// Catalog database path (JSON file)
        #[arg(short, long)]
        catalog: PathBuf,

        /// Target asset ID
        #[arg(long)]
        asset_id: Option<String>,

        /// Target assets by query
        #[arg(long)]
        query: Option<String>,

        /// Comma-separated tags to add
        #[arg(long)]
        add_tags: Option<String>,

        /// Comma-separated tags to remove
        #[arg(long)]
        remove_tags: Option<String>,

        /// Set or change collection assignment
        #[arg(long)]
        set_collection: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Catalog data model
// ---------------------------------------------------------------------------

/// A single media asset record.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AssetRecord {
    id: String,
    path: String,
    filename: String,
    format: String,
    size_bytes: u64,
    duration_secs: Option<f64>,
    width: Option<u32>,
    height: Option<u32>,
    codec: Option<String>,
    tags: Vec<String>,
    collection: Option<String>,
    ingested_at: String,
    checksum: String,
    metadata: HashMap<String, String>,
}

/// A named collection of assets.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CollectionRecord {
    name: String,
    description: String,
    created_at: String,
}

/// The full catalog database persisted as JSON.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CatalogDb {
    version: u32,
    assets: Vec<AssetRecord>,
    collections: Vec<CollectionRecord>,
}

// ---------------------------------------------------------------------------
// Catalog persistence helpers
// ---------------------------------------------------------------------------

fn load_catalog(path: &PathBuf) -> Result<CatalogDb> {
    if !path.exists() {
        return Ok(CatalogDb {
            version: 1,
            ..CatalogDb::default()
        });
    }
    let data = std::fs::read_to_string(path).context("Failed to read catalog file")?;
    let db: CatalogDb = serde_json::from_str(&data).context("Failed to parse catalog JSON")?;
    Ok(db)
}

fn save_catalog(path: &PathBuf, db: &CatalogDb) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).context("Failed to create catalog directory")?;
        }
    }
    let data = serde_json::to_string_pretty(db).context("Failed to serialize catalog")?;
    std::fs::write(path, data).context("Failed to write catalog file")?;
    Ok(())
}

fn generate_asset_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("asset-{:016x}", now.as_nanos())
}

fn compute_checksum(path: &std::path::Path) -> Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).context("Failed to open file for checksum")?;
    let mut hasher_state: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
    let mut buf = [0u8; 8192];
    loop {
        let n = file
            .read(&mut buf)
            .context("Failed to read file for checksum")?;
        if n == 0 {
            break;
        }
        for &byte in &buf[..n] {
            hasher_state ^= u64::from(byte);
            hasher_state = hasher_state.wrapping_mul(0x100000001b3);
        }
    }
    Ok(format!("{:016x}", hasher_state))
}

fn detect_format(path: &std::path::Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown")
        .to_lowercase()
}

fn now_iso8601() -> String {
    // Real RFC 3339 / ISO 8601 UTC timestamp. Catalogs written by older
    // builds stored plain epoch-seconds strings here; parse_asset_timestamp
    // accepts both, so old and new records stay comparable.
    chrono::Utc::now().to_rfc3339()
}

/// Parse a stored `ingested_at` value into Unix epoch seconds.
///
/// Accepts the current RFC 3339 form and the legacy plain epoch-seconds
/// string written by pre-0.2.0 catalogs. Returns `None` for unparseable
/// values (such records are excluded when a date filter is active — they
/// cannot be compared honestly).
fn parse_asset_timestamp(s: &str) -> Option<i64> {
    if let Ok(epoch) = s.parse::<i64>() {
        return Some(epoch);
    }
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.timestamp())
}

/// Parse a `--date-from` / `--date-to` bound into Unix epoch seconds.
///
/// Accepts a full RFC 3339 timestamp (`2026-07-15T12:00:00Z`), a plain date
/// (`2026-07-15` — interpreted as the start of that UTC day for `--date-from`
/// and the end of it for `--date-to`), or raw epoch seconds.
fn parse_date_bound(s: &str, is_end: bool) -> Result<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok(dt.timestamp());
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let time = if is_end {
            chrono::NaiveTime::from_hms_opt(23, 59, 59)
        } else {
            chrono::NaiveTime::from_hms_opt(0, 0, 0)
        }
        .ok_or_else(|| anyhow::anyhow!("Internal error building time bound"))?;
        return Ok(date.and_time(time).and_utc().timestamp());
    }
    if let Ok(epoch) = s.parse::<i64>() {
        return Ok(epoch);
    }
    Err(anyhow::anyhow!(
        "Invalid date '{s}'. Expected RFC 3339 (2026-07-15T12:00:00Z), a date (2026-07-15), \
         or Unix epoch seconds"
    ))
}

fn parse_tags(tags: &Option<String>) -> Vec<String> {
    tags.as_ref()
        .map(|t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Command handler
// ---------------------------------------------------------------------------

/// Handle MAM command dispatch.
pub async fn handle_mam_command(command: MamCommand, json_output: bool) -> Result<()> {
    match command {
        MamCommand::Ingest {
            input,
            catalog,
            tags,
            collection,
            recursive,
            generate_proxy,
            extract_metadata,
        } => {
            // No proxy-output policy (directory, codec, naming) exists on the
            // ingest surface yet, so proxy generation cannot take real effect
            // here; warn instead of silently dropping the request.
            // TODO(0.2.x): wire `oximedia_proxy::ProxyGenerator` into ingest
            // once a --proxy-dir/--proxy-codec surface is designed (the
            // standalone `oximedia proxy generate` path is already real).
            if generate_proxy {
                eprintln!(
                    "warning: --generate-proxy is not implemented for `mam ingest` yet and is \
                     ignored; use `oximedia proxy generate` instead"
                );
            }
            run_ingest(
                &input,
                &catalog,
                &tags,
                &collection,
                recursive,
                extract_metadata,
                json_output,
            )
            .await
        }
        MamCommand::Search {
            catalog,
            query,
            tags,
            format,
            date_from,
            date_to,
            limit,
            sort,
        } => {
            run_search(
                &catalog,
                &query,
                &tags,
                &format,
                date_from.as_deref(),
                date_to.as_deref(),
                limit,
                &sort,
                json_output,
            )
            .await
        }
        MamCommand::Catalog {
            catalog,
            stats,
            duplicates,
        } => run_catalog(&catalog, stats, duplicates, json_output).await,
        MamCommand::Export {
            catalog,
            output,
            query,
            collection,
            mode,
            manifest_format,
        } => {
            run_export(
                &catalog,
                &output,
                &query,
                &collection,
                &mode,
                &manifest_format,
                json_output,
            )
            .await
        }
        MamCommand::Tag {
            catalog,
            asset_id,
            query,
            add_tags,
            remove_tags,
            set_collection,
        } => {
            run_tag(
                &catalog,
                &asset_id,
                &query,
                &add_tags,
                &remove_tags,
                &set_collection,
                json_output,
            )
            .await
        }
    }
}

// ---------------------------------------------------------------------------
// Ingest
// ---------------------------------------------------------------------------

/// Probe a media file's container and fill real technical metadata into the
/// asset record (`--extract-metadata`): codec, dimensions, duration, plus a
/// `container_format` entry and any container-level key/value metadata.
///
/// Uses `oximedia_container::MultiFormatProber` on the file head — the same
/// real prober behind the TUI mini-probe. Unrecognized files simply gain no
/// metadata (the prober reports `"unknown"`), which is recorded honestly.
fn extract_asset_metadata(path: &std::path::Path, record: &mut AssetRecord) -> Result<()> {
    use std::io::Read;

    const PROBE_BYTES: usize = 64 * 1024;
    let mut buf = vec![0u8; PROBE_BYTES];
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open {} for probing", path.display()))?;
    let n = file
        .read(&mut buf)
        .with_context(|| format!("Failed to read {} for probing", path.display()))?;
    buf.truncate(n);

    let info = oximedia_container::MultiFormatProber::probe(&buf);

    record
        .metadata
        .insert("container_format".to_string(), info.format.clone());
    if let Some(duration_ms) = info.duration_ms {
        record.duration_secs = Some(duration_ms as f64 / 1000.0);
    }
    for (key, value) in &info.metadata {
        record.metadata.insert(key.clone(), value.clone());
    }

    if let Some(video) = info.streams.iter().find(|s| s.stream_type == "video") {
        record.codec = Some(video.codec.clone());
        record.width = video.width;
        record.height = video.height;
    } else if let Some(audio) = info.streams.iter().find(|s| s.stream_type == "audio") {
        record.codec = Some(audio.codec.clone());
        if let Some(sr) = audio.sample_rate {
            record
                .metadata
                .insert("sample_rate_hz".to_string(), sr.to_string());
        }
        if let Some(ch) = audio.channels {
            record
                .metadata
                .insert("channels".to_string(), ch.to_string());
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_ingest(
    inputs: &[PathBuf],
    catalog: &PathBuf,
    tags: &Option<String>,
    collection: &Option<String>,
    recursive: bool,
    extract_metadata: bool,
    json_output: bool,
) -> Result<()> {
    let mut db = load_catalog(catalog)?;
    let tag_list = parse_tags(tags);
    let mut ingested: Vec<AssetRecord> = Vec::new();

    // Collect all files
    let mut files: Vec<PathBuf> = Vec::new();
    for p in inputs {
        if p.is_dir() {
            collect_files(p, recursive, &mut files)?;
        } else if p.is_file() {
            files.push(p.clone());
        } else {
            return Err(anyhow::anyhow!("Path not found: {}", p.display()));
        }
    }

    if files.is_empty() {
        return Err(anyhow::anyhow!("No files found to ingest"));
    }

    for file_path in &files {
        let meta = std::fs::metadata(file_path)
            .with_context(|| format!("Failed to read metadata for {}", file_path.display()))?;
        let checksum = compute_checksum(file_path)?;

        // Skip if already in catalog (by checksum)
        if db.assets.iter().any(|a| a.checksum == checksum) {
            if !json_output {
                println!(
                    "  {} {} (already in catalog)",
                    "Skip:".yellow(),
                    file_path.display()
                );
            }
            continue;
        }

        let mut record = AssetRecord {
            id: generate_asset_id(),
            path: file_path.to_string_lossy().to_string(),
            filename: file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            format: detect_format(file_path),
            size_bytes: meta.len(),
            duration_secs: None,
            width: None,
            height: None,
            codec: None,
            tags: tag_list.clone(),
            collection: collection.clone(),
            ingested_at: now_iso8601(),
            checksum,
            metadata: HashMap::new(),
        };

        // --extract-metadata: probe the container for real and store what it
        // reports (codec, dimensions, duration, container format).
        if extract_metadata {
            extract_asset_metadata(file_path, &mut record)?;
        }

        ingested.push(record.clone());
        db.assets.push(record);
    }

    // Ensure collection record exists
    if let Some(ref coll_name) = collection {
        if !db.collections.iter().any(|c| &c.name == coll_name) {
            db.collections.push(CollectionRecord {
                name: coll_name.clone(),
                description: String::new(),
                created_at: now_iso8601(),
            });
        }
    }

    save_catalog(catalog, &db)?;

    if json_output {
        let result = serde_json::json!({
            "command": "ingest",
            "catalog": catalog.display().to_string(),
            "ingested_count": ingested.len(),
            "total_assets": db.assets.len(),
            "assets": ingested.iter().map(|a| serde_json::json!({
                "id": a.id,
                "filename": a.filename,
                "size_bytes": a.size_bytes,
                "format": a.format,
            })).collect::<Vec<_>>(),
        });
        let s = serde_json::to_string_pretty(&result).context("Failed to serialize result")?;
        println!("{s}");
    } else {
        println!("{}", "MAM Ingest".green().bold());
        println!("{}", "=".repeat(60));
        println!("{:20} {}", "Catalog:", catalog.display());
        println!("{:20} {}", "Files ingested:", ingested.len());
        println!("{:20} {}", "Total assets:", db.assets.len());
        println!();
        for a in &ingested {
            println!(
                "  {} {} ({} bytes, {})",
                "+".green(),
                a.filename,
                a.size_bytes,
                a.format
            );
        }
    }

    Ok(())
}

fn collect_files(dir: &PathBuf, recursive: bool, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries =
        std::fs::read_dir(dir).with_context(|| format!("Failed to read dir {}", dir.display()))?;
    for entry in entries {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();
        if path.is_file() {
            out.push(path);
        } else if path.is_dir() && recursive {
            collect_files(&path, recursive, out)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn run_search(
    catalog: &PathBuf,
    query: &str,
    tags: &Option<String>,
    format: &Option<String>,
    date_from: Option<&str>,
    date_to: Option<&str>,
    limit: Option<u32>,
    sort: &str,
    json_output: bool,
) -> Result<()> {
    let db = load_catalog(catalog)?;
    let tag_filter = parse_tags(tags);
    let query_lower = query.to_lowercase();
    let max_results = limit.unwrap_or(50) as usize;

    // Parse date bounds up front so invalid values fail before any output.
    let from_epoch = date_from.map(|s| parse_date_bound(s, false)).transpose()?;
    let to_epoch = date_to.map(|s| parse_date_bound(s, true)).transpose()?;
    if let (Some(from), Some(to)) = (from_epoch, to_epoch) {
        if from > to {
            return Err(anyhow::anyhow!(
                "--date-from is after --date-to; no asset can match"
            ));
        }
    }

    let mut results: Vec<&AssetRecord> = db
        .assets
        .iter()
        .filter(|a| {
            // Text match on filename, path, tags, collection, format
            let text_match = a.filename.to_lowercase().contains(&query_lower)
                || a.path.to_lowercase().contains(&query_lower)
                || a.tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&query_lower))
                || a.collection
                    .as_ref()
                    .map_or(false, |c| c.to_lowercase().contains(&query_lower))
                || a.format.to_lowercase().contains(&query_lower);

            // Tag filter
            let tag_ok =
                tag_filter.is_empty() || tag_filter.iter().all(|tf| a.tags.iter().any(|t| t == tf));

            // Format filter
            let fmt_ok = format
                .as_ref()
                .map_or(true, |f| a.format.eq_ignore_ascii_case(f));

            // Ingest-date filter: compares real parsed timestamps. Records
            // whose timestamp cannot be parsed are excluded while a date
            // filter is active — they cannot be compared honestly.
            let date_ok = if from_epoch.is_none() && to_epoch.is_none() {
                true
            } else {
                match parse_asset_timestamp(&a.ingested_at) {
                    Some(epoch) => {
                        from_epoch.map_or(true, |from| epoch >= from)
                            && to_epoch.map_or(true, |to| epoch <= to)
                    }
                    None => false,
                }
            };

            text_match && tag_ok && fmt_ok && date_ok
        })
        .collect();

    // Sort
    match sort {
        "name" => results.sort_by(|a, b| a.filename.cmp(&b.filename)),
        "size" => results.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes)),
        // Numeric timestamp sort — string comparison would order legacy
        // epoch-second records against RFC 3339 records incorrectly.
        "date" => results.sort_by_key(|a| {
            std::cmp::Reverse(parse_asset_timestamp(&a.ingested_at).unwrap_or(i64::MIN))
        }),
        _ => {} // relevance = insertion order
    }

    let total = results.len();
    results.truncate(max_results);

    if json_output {
        let result = serde_json::json!({
            "command": "search",
            "query": query,
            "total": total,
            "returned": results.len(),
            "assets": results.iter().map(|a| serde_json::json!({
                "id": a.id,
                "filename": a.filename,
                "format": a.format,
                "size_bytes": a.size_bytes,
                "tags": a.tags,
                "collection": a.collection,
            })).collect::<Vec<_>>(),
        });
        let s = serde_json::to_string_pretty(&result).context("Failed to serialize result")?;
        println!("{s}");
    } else {
        println!("{}", "MAM Search".green().bold());
        println!("{}", "=".repeat(60));
        println!("{:20} {}", "Query:", query);
        println!("{:20} {} (showing {})", "Results:", total, results.len());
        println!();
        for (i, a) in results.iter().enumerate() {
            println!(
                "  {}. {} [{}] {} bytes{}",
                i + 1,
                a.filename.cyan(),
                a.format,
                a.size_bytes,
                a.collection
                    .as_ref()
                    .map(|c| format!(" ({})", c))
                    .unwrap_or_default()
            );
            if !a.tags.is_empty() {
                println!("     tags: {}", a.tags.join(", ").dimmed());
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Catalog
// ---------------------------------------------------------------------------

async fn run_catalog(
    catalog: &PathBuf,
    stats: bool,
    duplicates: bool,
    json_output: bool,
) -> Result<()> {
    let db = load_catalog(catalog)?;

    let total_size: u64 = db.assets.iter().map(|a| a.size_bytes).sum();
    let formats: HashMap<String, usize> = db.assets.iter().fold(HashMap::new(), |mut acc, a| {
        *acc.entry(a.format.clone()).or_insert(0) += 1;
        acc
    });

    // Detect duplicates by checksum
    let dup_groups: Vec<Vec<&AssetRecord>> = if duplicates {
        let mut checksum_map: HashMap<&str, Vec<&AssetRecord>> = HashMap::new();
        for a in &db.assets {
            checksum_map.entry(&a.checksum).or_default().push(a);
        }
        checksum_map.into_values().filter(|v| v.len() > 1).collect()
    } else {
        Vec::new()
    };

    if json_output {
        let mut result = serde_json::json!({
            "command": "catalog",
            "catalog": catalog.display().to_string(),
            "total_assets": db.assets.len(),
            "total_collections": db.collections.len(),
            "total_size_bytes": total_size,
            "formats": formats,
        });
        if duplicates {
            result["duplicate_groups"] = serde_json::json!(dup_groups.len());
        }
        let s = serde_json::to_string_pretty(&result).context("Failed to serialize result")?;
        println!("{s}");
    } else {
        println!("{}", "MAM Catalog".green().bold());
        println!("{}", "=".repeat(60));
        println!("{:20} {}", "Catalog:", catalog.display());
        println!("{:20} {}", "Total assets:", db.assets.len());
        println!("{:20} {}", "Collections:", db.collections.len());
        println!(
            "{:20} {:.2} MB",
            "Total size:",
            total_size as f64 / (1024.0 * 1024.0)
        );

        if stats {
            println!();
            println!("{}", "Format Distribution".cyan().bold());
            println!("{}", "-".repeat(60));
            let mut fmt_vec: Vec<_> = formats.into_iter().collect();
            fmt_vec.sort_by(|a, b| b.1.cmp(&a.1));
            for (fmt, count) in &fmt_vec {
                println!("  {:12} {}", fmt, count);
            }

            if !db.collections.is_empty() {
                println!();
                println!("{}", "Collections".cyan().bold());
                println!("{}", "-".repeat(60));
                for c in &db.collections {
                    let count = db
                        .assets
                        .iter()
                        .filter(|a| a.collection.as_ref() == Some(&c.name))
                        .count();
                    println!("  {:20} {} assets", c.name, count);
                }
            }
        }

        if duplicates && !dup_groups.is_empty() {
            println!();
            println!("{}", "Duplicate Groups".yellow().bold());
            println!("{}", "-".repeat(60));
            for (i, group) in dup_groups.iter().enumerate() {
                println!("  Group {} ({} files):", i + 1, group.len());
                for a in group {
                    println!("    - {} ({})", a.filename, a.id);
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

async fn run_export(
    catalog: &PathBuf,
    output: &PathBuf,
    query: &Option<String>,
    collection: &Option<String>,
    mode: &str,
    manifest_format: &str,
    json_output: bool,
) -> Result<()> {
    let db = load_catalog(catalog)?;

    let selected: Vec<&AssetRecord> = db
        .assets
        .iter()
        .filter(|a| {
            let query_ok = query.as_ref().map_or(true, |q| {
                let ql = q.to_lowercase();
                a.filename.to_lowercase().contains(&ql)
                    || a.tags.iter().any(|t| t.to_lowercase().contains(&ql))
            });
            let coll_ok = collection
                .as_ref()
                .map_or(true, |c| a.collection.as_ref() == Some(c));
            query_ok && coll_ok
        })
        .collect();

    if selected.is_empty() {
        return Err(anyhow::anyhow!("No assets match the export criteria"));
    }

    // Ensure output directory exists
    if !output.exists() {
        std::fs::create_dir_all(output).context("Failed to create output directory")?;
    }

    let mut exported = Vec::new();
    for asset in &selected {
        let src = std::path::Path::new(&asset.path);
        if !src.exists() {
            if !json_output {
                println!("  {} {} (source missing)", "Skip:".yellow(), asset.filename);
            }
            continue;
        }
        let dest = output.join(&asset.filename);
        match mode {
            "copy" => {
                std::fs::copy(src, &dest)
                    .with_context(|| format!("Failed to copy {}", asset.filename))?;
            }
            "link" => {
                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(src, &dest)
                        .with_context(|| format!("Failed to symlink {}", asset.filename))?;
                }
                #[cfg(not(unix))]
                {
                    std::fs::copy(src, &dest)
                        .with_context(|| format!("Failed to copy {}", asset.filename))?;
                }
            }
            _ => {
                std::fs::copy(src, &dest)
                    .with_context(|| format!("Failed to copy {}", asset.filename))?;
            }
        }
        exported.push(asset);
    }

    // Write manifest
    let manifest_path = output.join(format!("manifest.{manifest_format}"));
    let manifest_data: Vec<serde_json::Value> = exported
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "filename": a.filename,
                "format": a.format,
                "size_bytes": a.size_bytes,
                "checksum": a.checksum,
                "tags": a.tags,
                "collection": a.collection,
            })
        })
        .collect();
    let manifest_str =
        serde_json::to_string_pretty(&manifest_data).context("Failed to serialize manifest")?;
    std::fs::write(&manifest_path, &manifest_str).context("Failed to write manifest")?;

    if json_output {
        let result = serde_json::json!({
            "command": "export",
            "output": output.display().to_string(),
            "exported_count": exported.len(),
            "mode": mode,
            "manifest": manifest_path.display().to_string(),
        });
        let s = serde_json::to_string_pretty(&result).context("Failed to serialize")?;
        println!("{s}");
    } else {
        println!("{}", "MAM Export".green().bold());
        println!("{}", "=".repeat(60));
        println!("{:20} {}", "Output:", output.display());
        println!("{:20} {}", "Exported:", exported.len());
        println!("{:20} {}", "Mode:", mode);
        println!("{:20} {}", "Manifest:", manifest_path.display());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tag
// ---------------------------------------------------------------------------

async fn run_tag(
    catalog: &PathBuf,
    asset_id: &Option<String>,
    query: &Option<String>,
    add_tags: &Option<String>,
    remove_tags: &Option<String>,
    set_collection: &Option<String>,
    json_output: bool,
) -> Result<()> {
    let mut db = load_catalog(catalog)?;
    let tags_to_add = parse_tags(add_tags);
    let tags_to_remove = parse_tags(remove_tags);

    if asset_id.is_none() && query.is_none() {
        return Err(anyhow::anyhow!(
            "Must specify either --asset-id or --query to select assets"
        ));
    }

    let mut modified_count = 0usize;
    for asset in &mut db.assets {
        let matches = if let Some(ref id) = asset_id {
            asset.id == *id
        } else if let Some(ref q) = query {
            let ql = q.to_lowercase();
            asset.filename.to_lowercase().contains(&ql)
                || asset.tags.iter().any(|t| t.to_lowercase().contains(&ql))
        } else {
            false
        };

        if !matches {
            continue;
        }

        // Add tags
        for tag in &tags_to_add {
            if !asset.tags.contains(tag) {
                asset.tags.push(tag.clone());
            }
        }
        // Remove tags
        asset.tags.retain(|t| !tags_to_remove.contains(t));
        // Set collection
        if let Some(ref coll) = set_collection {
            asset.collection = Some(coll.clone());
        }

        modified_count += 1;
    }

    save_catalog(catalog, &db)?;

    if json_output {
        let result = serde_json::json!({
            "command": "tag",
            "modified_count": modified_count,
            "tags_added": tags_to_add,
            "tags_removed": tags_to_remove,
            "collection_set": set_collection,
        });
        let s = serde_json::to_string_pretty(&result).context("Failed to serialize")?;
        println!("{s}");
    } else {
        println!("{}", "MAM Tag".green().bold());
        println!("{}", "=".repeat(60));
        println!("{:20} {}", "Modified assets:", modified_count);
        if !tags_to_add.is_empty() {
            println!("{:20} {}", "Tags added:", tags_to_add.join(", "));
        }
        if !tags_to_remove.is_empty() {
            println!("{:20} {}", "Tags removed:", tags_to_remove.join(", "));
        }
        if let Some(ref coll) = set_collection {
            println!("{:20} {}", "Collection:", coll);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tags_some() {
        let tags = Some("foo, bar ,baz".to_string());
        let result = parse_tags(&tags);
        assert_eq!(result, vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn test_parse_tags_none() {
        let result = parse_tags(&None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_detect_format() {
        assert_eq!(detect_format(std::path::Path::new("video.mkv")), "mkv");
        assert_eq!(detect_format(std::path::Path::new("audio.FLAC")), "flac");
        assert_eq!(detect_format(std::path::Path::new("noext")), "unknown");
    }

    #[test]
    fn test_catalog_roundtrip() {
        let db = CatalogDb {
            version: 1,
            assets: vec![AssetRecord {
                id: "test-001".to_string(),
                path: std::env::temp_dir()
                    .join("test.mkv")
                    .to_string_lossy()
                    .to_string(),
                filename: "test.mkv".to_string(),
                format: "mkv".to_string(),
                size_bytes: 1024,
                duration_secs: Some(60.0),
                width: Some(1920),
                height: Some(1080),
                codec: Some("av1".to_string()),
                tags: vec!["raw".to_string()],
                collection: Some("dailies".to_string()),
                ingested_at: "1234567890".to_string(),
                checksum: "abcdef0123456789".to_string(),
                metadata: HashMap::new(),
            }],
            collections: vec![CollectionRecord {
                name: "dailies".to_string(),
                description: "Daily rushes".to_string(),
                created_at: "1234567890".to_string(),
            }],
        };
        let json = serde_json::to_string(&db);
        assert!(json.is_ok());
        let parsed: Result<CatalogDb, _> =
            serde_json::from_str(&json.expect("serialization should succeed"));
        assert!(parsed.is_ok());
        let parsed = parsed.expect("deserialization should succeed");
        assert_eq!(parsed.assets.len(), 1);
        assert_eq!(parsed.collections.len(), 1);
    }

    #[test]
    fn test_generate_asset_id_uniqueness() {
        let id1 = generate_asset_id();
        let id2 = generate_asset_id();
        // IDs should start with "asset-"
        assert!(id1.starts_with("asset-"));
        assert!(id2.starts_with("asset-"));
    }

    #[test]
    fn test_parse_asset_timestamp_forms() {
        // Legacy epoch-seconds string form.
        assert_eq!(parse_asset_timestamp("1234567890"), Some(1_234_567_890));
        // Current RFC 3339 form.
        let epoch = parse_asset_timestamp("2026-07-15T00:00:00+00:00").expect("rfc3339");
        assert_eq!(epoch, 1_784_073_600);
        // Garbage is None, never a bogus comparison value.
        assert_eq!(parse_asset_timestamp("not-a-date"), None);
        // now_iso8601 output must round-trip through the parser.
        assert!(parse_asset_timestamp(&now_iso8601()).is_some());
    }

    #[test]
    fn test_parse_date_bound_forms() {
        // Plain date: from = start of day, to = end of day (inclusive).
        let from = parse_date_bound("2026-07-15", false).expect("date from");
        let to = parse_date_bound("2026-07-15", true).expect("date to");
        assert_eq!(from, 1_784_073_600);
        assert_eq!(to - from, 86_399, "end-of-day bound must span the day");
        // RFC 3339 and epoch forms.
        assert_eq!(
            parse_date_bound("2026-07-15T12:00:00Z", false).expect("rfc3339"),
            1_784_116_800
        );
        assert_eq!(parse_date_bound("1234", false).expect("epoch"), 1234);
        // Garbage errors with the accepted grammar.
        let msg = parse_date_bound("yesterday", false)
            .expect_err("must reject")
            .to_string();
        assert!(
            msg.contains("RFC 3339"),
            "must explain accepted forms: {msg}"
        );
    }

    /// End-to-end proof that `--date-from` / `--date-to` genuinely filter:
    /// two assets with known ingest dates, a window matching only one.
    #[tokio::test]
    async fn test_search_date_filter_is_real() {
        let dir = std::env::temp_dir();
        let catalog = dir.join("oximedia_mam_date_filter_test.json");
        std::fs::remove_file(&catalog).ok();

        let mk_asset = |id: &str, name: &str, ingested_at: &str| AssetRecord {
            id: id.to_string(),
            path: dir.join(name).to_string_lossy().to_string(),
            filename: name.to_string(),
            format: "wav".to_string(),
            size_bytes: 10,
            duration_secs: None,
            width: None,
            height: None,
            codec: None,
            tags: vec![],
            collection: None,
            ingested_at: ingested_at.to_string(),
            checksum: id.to_string(),
            metadata: HashMap::new(),
        };

        let db = CatalogDb {
            version: 1,
            assets: vec![
                mk_asset("a-old", "old.wav", "2026-01-05T10:00:00+00:00"),
                // Legacy epoch form: 2026-07-10T00:00:00Z = 1783641600.
                mk_asset("a-new", "new.wav", "1783641600"),
            ],
            collections: vec![],
        };
        save_catalog(&catalog, &db).expect("save catalog");

        // Window covering only July 2026 must match only the new asset —
        // and must do so across BOTH stored timestamp formats.
        run_search(
            &catalog,
            "wav",
            &None,
            &None,
            Some("2026-07-01"),
            Some("2026-07-31"),
            None,
            "relevance",
            true,
        )
        .await
        .expect("search must succeed");

        // Assert by re-running the same filter logic the search uses.
        let from = parse_date_bound("2026-07-01", false).expect("from");
        let to = parse_date_bound("2026-07-31", true).expect("to");
        let loaded = load_catalog(&catalog).expect("load");
        let matched: Vec<&AssetRecord> = loaded
            .assets
            .iter()
            .filter(|a| {
                parse_asset_timestamp(&a.ingested_at)
                    .map(|e| e >= from && e <= to)
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(matched.len(), 1, "exactly one asset in the July window");
        assert_eq!(matched[0].id, "a-new");

        std::fs::remove_file(&catalog).ok();
    }

    /// `--extract-metadata` must store real probed values for a real WAV file.
    #[test]
    fn test_extract_asset_metadata_wav() {
        let dir = std::env::temp_dir();
        let wav_path = dir.join("oximedia_mam_probe_test.wav");

        // Minimal valid WAV: RIFF header + fmt + tiny data chunk.
        let sample_rate: u32 = 44_100;
        let data: Vec<u8> = vec![0u8; 32];
        let mut wav: Vec<u8> = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        std::fs::write(&wav_path, &wav).expect("write wav");

        let mut record = AssetRecord {
            id: "probe-test".to_string(),
            path: wav_path.to_string_lossy().to_string(),
            filename: "oximedia_mam_probe_test.wav".to_string(),
            format: "wav".to_string(),
            size_bytes: wav.len() as u64,
            duration_secs: None,
            width: None,
            height: None,
            codec: None,
            tags: vec![],
            collection: None,
            ingested_at: now_iso8601(),
            checksum: "x".to_string(),
            metadata: HashMap::new(),
        };

        extract_asset_metadata(&wav_path, &mut record).expect("probe must succeed");
        assert_eq!(
            record.metadata.get("container_format").map(String::as_str),
            Some("wav"),
            "real prober must identify the WAV container, got {:?}",
            record.metadata
        );

        std::fs::remove_file(&wav_path).ok();
    }
}
