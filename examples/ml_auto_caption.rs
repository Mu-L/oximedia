//! Stub example for the upcoming AutoCaption pipeline.
//!
//! The `AutoCaption` typed pipeline is scheduled to land in **Wave 2 Slice C**
//! of the 0.1.5 ML roadmap. Until that ships, this example demonstrates
//! the pieces of `oximedia::ml` that are already available — in
//! particular, the device-probe API that any real AutoCaption pipeline
//! will build on.
//!
//! When Wave 2 Slice C ships, this example will be expanded to:
//!   - Load an audio waveform from `argv[1]`.
//!   - Run speech recognition → caption alignment via
//!     `AutoCaption::from_path(model, DeviceType::auto())`.
//!   - Emit SRT / WebVTT output.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p oximedia --example ml_auto_caption --features ml
//! ```

// TODO: wire AutoCaption::from_path once Wave 2 Slice C ships.

use oximedia::prelude::*;

fn format_bytes(bytes: Option<u64>) -> String {
    bytes.map_or_else(|| "—".to_string(), |b| format!("{b} B"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("AutoCaption pipeline is scheduled for Wave 2 Slice C.");
    eprintln!("This example currently demonstrates only the device-probe API.\n");

    println!("OxiMedia — ML Auto-Caption (stub)");
    println!("=================================\n");

    // Step 1: auto-select the best available device.
    let device = DeviceType::auto();
    println!(
        "Auto-selected inference device: {device} ({})",
        device.display_name()
    );

    // Step 2: full capability probe across every compiled-in backend.
    let capabilities = DeviceCapabilities::probe_all();
    println!("\nDevice capability probe:");
    println!(
        "  {:<10} {:<12} {:<28} {:<10} {:<6} {:<6} {:<6}",
        "Backend", "Status", "Device Name", "Memory", "fp16", "bf16", "int8"
    );
    println!("  {}", "-".repeat(80));
    for caps in &capabilities {
        let status = if caps.is_available {
            "available"
        } else {
            "unavailable"
        };
        println!(
            "  {:<10} {:<12} {:<28} {:<10} {:<6} {:<6} {:<6}",
            caps.device_type.name(),
            status,
            caps.device_name,
            format_bytes(caps.memory_total_bytes),
            caps.supports_fp16,
            caps.supports_bf16,
            caps.supports_int8,
        );
    }

    // Step 3: the capability record for the best device (handy for UIs).
    let best = DeviceCapabilities::best_available();
    println!(
        "\nBest available: {} — int8:{} fp16:{} bf16:{}",
        best, best.supports_int8, best.supports_fp16, best.supports_bf16,
    );

    // Step 4: roadmap pointer so callers know what to expect next.
    println!("\nAutoCaption pipeline (Wave 2 Slice C) will expose:");
    println!("  * AutoCaption::from_path(model, device) -> MlResult<AutoCaption>");
    println!("  * AutoCaption::run(AudioWaveform) -> MlResult<Vec<CaptionLine>>");
    println!("  * Speech alignment, WCAG 2.1 line breaking, speaker diarization");

    Ok(())
}
