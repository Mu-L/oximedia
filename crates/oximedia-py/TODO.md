# oximedia-py TODO

## Current Status
- 105 source files providing comprehensive Python bindings via PyO3
- 94 public modules covering nearly all OxiMedia crates
- Bindings include: codecs (AV1, VP9, VP8, Opus, Vorbis, FLAC), containers (Matroska, Ogg, MP4), quality assessment, audio analysis, scene detection, filter graph, effects, LUT, EDL, streaming, async pipeline, Jupyter integration, DataFrame export, broadcast, transcoding, color management, timecode, cv2 compatibility, and many more
- Registers classes and functions into single `oximedia` Python module with `cv2` submodule
- Dependencies: pyo3, ndarray, numpy, image, tokio, plus 40+ oximedia workspace crates

## Enhancements
- [ ] Add `__repr__` and `__str__` implementations to all pyclass types for better REPL experience
- [ ] Implement Python context manager protocol (`__enter__`/`__exit__`) for resource-holding types (demuxers, encoders)
- [ ] Add type stub generation (.pyi files) for IDE autocomplete and static type checking
- [ ] Extend `cv2_compat` with additional OpenCV function coverage (morphological ops, contour detection)
- [ ] Add numpy array zero-copy frame access in `VideoFrame` and `AudioFrame` via buffer protocol
- [ ] Implement async/await support for Python 3.10+ coroutines in `async_pipeline`
- [ ] Add pickle support for serializable types (EncoderConfig, QualityScore, etc.)
- [ ] Extend `dataframe` module with Apache Arrow export for zero-copy pandas interop

## New Features
- [ ] Add `oximedia.io` submodule for file read/write operations (open, probe, transcode)
- [ ] Implement Python logging integration — route Rust tracing events to Python logging module
- [ ] Add `oximedia.utils` submodule with common media helper functions (duration_to_timecode, fps_to_rational)
- [ ] Implement streaming iterator protocol for frame-by-frame decoding (`for frame in decoder`)
- [ ] Add `oximedia.benchmark` module exposing profiler bindings for Python performance analysis
- [ ] Implement callback-based progress reporting for long-running operations (encode, transcode)
- [ ] Add CLI wrapper — `python -m oximedia transcode input.mkv output.webm`
- [ ] Implement `oximedia.test` submodule with test media generators (synthetic video/audio frames)

## Performance
- [ ] Minimize GIL holding time in CPU-intensive operations (encode, decode, quality assessment)
- [ ] Use `PyBuffer` protocol for zero-copy data exchange with numpy arrays
- [ ] Implement batch processing APIs that release GIL for parallel Rust execution
- [ ] Add optional memory pool for frame allocation to reduce Python GC pressure
- [ ] Profile and optimize hot paths in `filter_graph` Python-Rust boundary crossings

## Testing
- [ ] Add Python-level integration tests using pytest (test import, basic encode/decode roundtrip)
- [ ] Test cv2_compat API parity with actual OpenCV Python — verify compatible function signatures
- [ ] Add memory leak tests for long-running Python scripts using tracemalloc
- [ ] Test Jupyter integration with nbconvert-based notebook execution
- [ ] Verify all pyclass types are importable and constructible from Python

## Documentation
- [ ] Add Python API reference generated from type stubs
- [ ] Create Jupyter notebook tutorials for common workflows (transcode, quality check, scene detect)
- [ ] Document cv2 compatibility layer with migration guide from OpenCV Python
