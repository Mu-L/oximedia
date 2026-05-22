# oximedia-io TODO

## Current Status
- 27+ modules providing the I/O foundation for OxiMedia
- Sources: FileSource (tokio async), MemorySource, MediaSource trait
- Bit-level: BitReader with Exp-Golomb coding support for H.264-style parsing
- Buffering: buffered_io, buffered_reader, ring_buffer, buffer_pool
- Advanced I/O: aligned_io, async_io, scatter_gather, seekable, mmap, splice_pipe
- Writing: chunked_writer, write_journal (journaled writes for crash safety)
- Utilities: checksum, compression, copy_engine, format_detector, io_stats, rate_limiter
- File ops: file_metadata, file_watch, temp_files, progress_reader, verify_io
- Pipeline: io_pipeline for chaining I/O operations
- Dependencies: oximedia-core, bytes, async-trait, tokio (non-wasm)

## Enhancements
- [x] Add configurable read-ahead buffering to `buffered_reader.rs` with adaptive window sizing
- [x] Extend `ring_buffer.rs` with wait-free SPSC (single producer single consumer) mode for async pipelines
- [x] Add CRC-32C (Castagnoli) alongside existing checksums in `checksum.rs` for modern formats (verified 2026-05-16; src/checksum.rs:219 Crc32c variant, compute fn:252)
- [x] Implement write coalescing in `chunked_writer.rs` to batch small writes into larger I/O operations
- [x] Extend `format_detector.rs` with detection for all OxiMedia-supported container formats (MXF, DPX, EXR)
- [x] Add `rate_limiter.rs` support for both read and write direction with separate bandwidth limits (verified 2026-05-16; src/rate_limiter.rs:371 IoDirection::Read/Write, read_bucket/write_bucket)
- [x] Extend `mmap.rs` with huge page support on Linux for large file mappings (verified 2026-05-16; src/mmap.rs:317 HugePageSize enum, MAP_HUGETLB:320)
- [x] Add `file_watch.rs` support for recursive directory watching with debounce (verified 2026-05-16; src/file_watch.rs:57 debounce Duration, recursive:59)

## New Features
- [x] Add an `http_source.rs` module for streaming media from HTTP/HTTPS URLs with range request support (verified 2026-05-16; src/http_source.rs:559 lines)
- [x] Implement an `s3_source.rs` module for reading from S3-compatible object storage (verified 2026-05-16; src/s3_source.rs:566 lines)
- [x] Add a `pipe_source.rs` module for reading from Unix pipes and stdin (verified 2026-05-16; src/pipe_source.rs:371 lines)
- [x] Implement a `multipart_writer.rs` module for writing large files in parallel segments (verified 2026-05-16; src/multipart_writer.rs:501 lines)
- [x] Add a `prefetch.rs` module for predictive I/O prefetching based on access patterns (verified 2026-05-16; src/prefetch.rs:455 lines)
- [x] Implement a `dedup_writer.rs` module for content-deduplicating writes (hash-based block dedup) (verified 2026-05-16; src/dedup_writer.rs:282 lines)
- [x] Add an `io_metrics.rs` module with Prometheus-compatible I/O throughput and latency metrics (verified 2026-05-16; src/io_metrics.rs:264 lines)
- [x] Implement a `retrying_source.rs` wrapper that retries failed reads with exponential backoff

## Performance
- [x] Add vectored I/O (readv/writev) support to `scatter_gather.rs` for reduced syscall overhead (verified 2026-05-16; src/scatter_gather.rs:1 vectored I/O primitives, ReadVec/test_readvec:237)
- [x] Implement direct I/O (O_DIRECT) option in `aligned_io.rs` for bypassing OS page cache (verified 2026-05-16; src/aligned_io.rs:7 O_DIRECT modeling, alignment-aware:100)
- [ ] Add zero-copy sendfile/splice support in `copy_engine.rs` on Linux (verified-open 2026-05-16: no sendfile/splice in copy_engine.rs)
- [ ] Optimize `BitReader` for batch bit extraction (read 32/64 bits at a time from buffer) (verified-open 2026-05-16: not yet implemented)
- [x] Add io_uring support as an optional backend for `async_io.rs` on Linux (verified 2026-05-16; src/async_io.rs:149 io_uring/IOCP/kqueue data structures modeled)
- [ ] Implement double-buffered reading in `buffered_io.rs` for overlapped I/O and processing (verified-open 2026-05-16: not yet implemented)

## Testing
- [ ] Add tests for `write_journal.rs` crash recovery by simulating interrupted writes
- [ ] Test `ring_buffer.rs` under concurrent producer/consumer with varying rates
- [ ] Add `mmap.rs` tests with files larger than available RAM to verify windowed mapping
- [ ] Test `format_detector.rs` with truncated files and zero-length inputs
- [ ] Add throughput benchmarks for `copy_engine.rs` comparing buffered vs. mmap vs. splice paths
- [ ] Test `progress_reader.rs` callback accuracy at various read granularities

## Documentation
- [ ] Add an I/O architecture diagram showing the source -> buffer -> pipeline -> writer flow
- [ ] Document the wasm32 limitations (no FileSource, no mmap, no file_watch)
- [ ] Add performance tuning guide for buffer sizes and I/O strategies per use case
