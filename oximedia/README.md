# oximedia

Facade crate for the OxiMedia multimedia framework.

## Overview

`oximedia` is the main entry point for using the OxiMedia framework. It re-exports types from all component crates for convenient access:

- `oximedia-core` - Core types and traits
- `oximedia-io` - I/O and bit-level reading
- `oximedia-container` - Container demuxing/muxing

## Usage

### Basic Import

```rust
use oximedia::prelude::*;
```

### Format Probing

```rust
use oximedia::prelude::*;

fn probe_file(path: &str) -> OxiResult<()> {
    let data = std::fs::read(path)?;
    let result = probe_format(&data)?;

    println!("Format: {:?}", result.format);
    println!("Confidence: {:.1}%", result.confidence * 100.0);

    Ok(())
}
```

### Available Types

From `oximedia-core`:

| Type | Description |
|------|-------------|
| `Rational` | Exact rational numbers |
| `Timestamp` | Media timestamps with timebase |
| `PixelFormat` | Video pixel formats |
| `SampleFormat` | Audio sample formats |
| `CodecId` | Codec identifiers |
| `MediaType` | Media type (Video, Audio, Subtitle) |
| `OxiError` | Unified error type |

From `oximedia-io`:

| Type | Description |
|------|-------------|
| `BitReader` | Bit-level reading |
| `MediaSource` | Async media source trait |
| `FileSource` | File-based source |
| `MemorySource` | Memory-based source |

From `oximedia-container`:

| Type | Description |
|------|-------------|
| `ContainerFormat` | Container format enum |
| `Packet` | Compressed media packet |
| `PacketFlags` | Packet properties |
| `StreamInfo` | Stream metadata |
| `Demuxer` | Demuxer trait |
| `probe_format` | Format detection function |

## Prelude

The prelude module exports the most commonly used types:

```rust
pub use crate::{
    CodecId, MediaType, OxiError, OxiResult,
    PixelFormat, Rational, SampleFormat, Timestamp,
    ContainerFormat, Packet, PacketFlags,
    probe_format,
};

pub use oximedia_io::MediaSource;
pub use oximedia_container::Demuxer;
```

## Example

```rust
use oximedia::prelude::*;

#[tokio::main]
async fn main() -> OxiResult<()> {
    // Create a timestamp
    let ts = Timestamp::new(1000, Rational::new(1, 1000));
    println!("Timestamp: {} seconds", ts.to_seconds());

    // Check codec type
    let codec = CodecId::Av1;
    println!("AV1 is video: {}", codec.is_video());

    // Probe a format
    let data = vec![0x1A, 0x45, 0xDF, 0xA3]; // EBML header
    if let Ok(result) = probe_format(&data) {
        println!("Detected: {:?}", result.format);
    }

    Ok(())
}
```

## Policy

- No unsafe code (`#![forbid(unsafe_code)]`)
- No warnings
- Apache 2.0 license

## License

Apache-2.0
