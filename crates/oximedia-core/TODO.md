# oximedia-core TODO

## Current Status
- 42 source files; foundational types and traits for the entire OxiMedia framework
- Types: Rational, Timestamp, PixelFormat, SampleFormat, CodecId, MediaType, FourCC, ChannelLayout
- Traits: Decoder, Demuxer interfaces
- Error handling: unified OxiError with patent violation detection (blocks H.264, H.265, AAC, etc.)
- Memory: buffer pools (alloc/buffer_pool), ring buffer, work queue, event queue
- HDR: metadata, transfer functions, color primaries, conversions, parser
- Additional: codec_info, codec_negotiation, type_registry, error_context, resource_handle, sync, frame_info, media_time, version
- WASM support via feature gate

## Enhancements
- [ ] Add Timestamp arithmetic operations (add, sub, multiply by Rational) with overflow protection
- [ ] Extend `PixelFormat` in `types/pixel_format.rs` with NV12, NV21, P010, P016 for hardware interop
- [ ] Add `SampleFormat` support for 24-bit and 64-bit float in `types/sample_format.rs`
- [ ] Extend `codec_negotiation.rs` with automatic format negotiation between encoder and decoder
- [ ] Improve `error_context.rs` with structured error context chain (file, function, line info)
- [ ] Add `ChannelLayout` presets for Atmos and surround configurations in `channel_layout.rs`
- [ ] Extend `buffer_pool.rs` with memory pressure callbacks and automatic pool shrinking
- [ ] Add `CodecId` variants for all supported codecs (currently missing some like WebP, GIF, JPEG-XL)

## New Features
- [ ] Implement zero-copy frame sharing between crates using `resource_handle.rs` with ref-counted buffers
- [ ] Add media duration/bitrate estimation utilities in `media_time.rs`
- [ ] Implement typed FourCC constants for all supported codecs in `fourcc.rs`
- [ ] Add `sync.rs` inter-thread synchronization primitives optimized for media pipelines (bounded channel with backpressure)
- [ ] Implement frame pool with configurable pre-allocation for low-latency pipelines in `alloc/`
- [ ] Add color primaries and matrix coefficients to `PixelFormat` metadata
- [ ] Implement WASM-compatible async runtime abstraction in `wasm.rs` for cross-platform pipelines

## Performance
- [ ] Optimize `Rational` arithmetic in `types/rational.rs` with GCD reduction on construction
- [ ] Add SIMD-accelerated pixel format conversion helpers in `convert/pixel.rs`
- [ ] Implement lock-free ring buffer variant in `ring_buffer.rs` for single-producer/single-consumer
- [ ] Optimize `work_queue.rs` with work-stealing scheduler for multi-threaded pipelines
- [ ] Add cache-line-aligned buffer allocation in `alloc/mod.rs` for SIMD-friendly access
- [ ] Profile and optimize `event_queue.rs` for high-throughput event processing (>1M events/sec)

## Testing
- [ ] Add property-based tests for `Rational` arithmetic (commutativity, associativity, overflow)
- [ ] Test all `PixelFormat` variants for correct plane count, bit depth, and chroma subsampling
- [ ] Test patent violation detection in `error.rs` for all known patent-encumbered codec names
- [ ] Add buffer pool stress test: allocate/deallocate across multiple threads
- [ ] Test `Timestamp` conversion accuracy between different time bases (90kHz, 48kHz, 1/fps)
- [ ] Test `type_registry.rs` registration and lookup with concurrent access
- [ ] Add WASM compilation test to verify `wasm` feature compiles cleanly

## Documentation
- [ ] Document the patent-free codec philosophy and green list in crate-level docs
- [ ] Add type conversion guide (Timestamp <-> seconds, Rational <-> f64)
- [ ] Document buffer pool usage patterns for zero-copy media pipelines
