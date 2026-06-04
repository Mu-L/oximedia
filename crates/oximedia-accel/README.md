# oximedia-accel

![Status: Stable](https://img.shields.io/badge/status-stable-green)

Hardware acceleration layer for OxiMedia using Vulkan compute shaders, with automatic CPU fallback for systems without GPU support.

Part of the [oximedia](https://github.com/cool-japan/oximedia) workspace — a comprehensive pure-Rust media processing framework.

Version: 0.1.8 — 2026-05-29 — 409 tests

## Features

- Automatic GPU device enumeration and selection via Vulkan
- Efficient GPU memory allocation and buffer transfer
- Compute kernels: image scaling, color conversion, motion estimation
- Automatic CPU fallback when GPU is unavailable
- Safe Vulkan API access via vulkano
- Task graph scheduling for concurrent GPU operations
- Memory arena and pool management
- Fence timeline for GPU synchronization
- Pipeline acceleration abstractions
- Profiling and performance statistics
- Prefetch and cache management

## Build prerequisites

`oximedia-accel` depends on [`vulkano-shaders`](https://crates.io/crates/vulkano-shaders),
which uses the `vulkano_shaders::shader!` proc-macro to compile GLSL compute
shaders to SPIR-V **at build time**. That macro pulls in `shaderc-sys`, and
when no pre-built `shaderc` library is found on the host (the typical case
on a fresh developer machine), `shaderc-sys` builds `shaderc` from source.
Building `shaderc` from source requires the following native tools to be on
`PATH`:

| Tool | Purpose | Install |
|---|---|---|
| **cmake** (>= 3.17) | Drives the `shaderc` C++ build | `brew install cmake` / `apt install cmake` / `winget install Kitware.CMake` |
| **Python 3** | Used by `glslang`'s build scripts | `brew install python` / `apt install python3` / `winget install Python.Python.3` |
| **C++ compiler** | Compiles `shaderc` / `glslang` / `SPIRV-Tools` | Xcode CLT / `apt install build-essential` / MSVC Build Tools |
| **Git** | `shaderc-sys` clones `shaderc` sources | `brew install git` / `apt install git` / `winget install Git.Git` |

These prerequisites apply to **every** build of `oximedia-accel` — the
dependency on `vulkano-shaders` is not feature-gated.

If you do not need Vulkan compute acceleration, you can exclude this crate
from a workspace build:

```bash
cargo build --workspace --exclude oximedia-accel
```

The runtime itself still falls back to the pure-Rust `CpuFallback` backend
when no Vulkan driver is present, but the GLSL → SPIR-V step at compile
time is unconditional.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
oximedia-accel = "0.1.8"
```

```rust
use oximedia_accel::{AccelContext, HardwareAccel, ScaleFilter};
use oximedia_core::types::PixelFormat;

fn example() -> Result<(), Box<dyn std::error::Error>> {
    // Create acceleration context (automatically selects GPU or CPU)
    let accel = AccelContext::new()?;

    // Perform image scaling
    let input = vec![0u8; 1920 * 1080 * 3];
    let output = accel.scale_image(
        &input,
        1920, 1080,
        1280, 720,
        PixelFormat::Rgb24,
        ScaleFilter::Bilinear,
    )?;
    Ok(())
}
```

## API Overview

**Core types:**
- `AccelContext` — Main entry point; selects GPU or CPU backend automatically
- `HardwareAccel` (trait) — Unified interface for GPU and CPU implementations
- `ScaleFilter` — Scaling filter variants (nearest, bilinear, bicubic)
- `AccelError`, `AccelResult` — Error types

**Backends:**
- `VulkanAccel` — Vulkan compute backend
- `CpuFallback` — Pure-CPU fallback implementation

**Modules (37 source files, 401 public items):**
- `device`, `device_caps` — GPU device management and capability detection
- `buffer`, `pool`, `memory_arena`, `memory_bandwidth` — Memory management
- `kernels`, `shaders` — Compute kernels and SPIR-V shaders
- `task_graph`, `task_scheduler`, `dispatch` — Parallel task scheduling
- `pipeline_accel` — Pipeline-level acceleration
- `fence_timeline` — GPU synchronization primitives
- `ops` — High-level compute operations
- `cache`, `prefetch` — Caching and prefetch strategies
- `accel_profile`, `accel_stats` — Profiling and statistics
- `traits` — Core trait definitions
- `error` — Error types

## License

Apache-2.0 — Copyright 2024-2026 COOLJAPAN OU (Team Kitasan)
