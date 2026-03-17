# oximedia-profiler TODO

## Current Status
- 27 modules for performance profiling and optimization
- Core: Profiler, ProfilerConfig, ProfilingMode (Sampling, Instrumentation, EventBased, Continuous)
- Modules: allocation_tracker, benchmark, bottleneck, cache, call_graph, codec_profiler, cpu, event_trace, flame, flamegraph, frame, frame_profiler, gpu, hotspot, latency_profiler, mem_profile, memory, memory_profiler, network_profiler, optimize, pipeline_profiler, regression, report, report_format, resource, sampling_profiler, thread, throughput_profiler
- Dependencies: oximedia-core, sysinfo, serde, thiserror

## Enhancements
- [ ] Add hierarchical span tracking in `Profiler` for nested function timing (enter/exit scope)
- [ ] Implement adaptive sampling rate in `sampling_profiler` — increase rate during detected hotspots
- [ ] Extend `memory_profiler` with allocation site tracking (caller stack capture)
- [ ] Add real-time profiling data streaming via `report` module (live metrics push)
- [ ] Implement `bottleneck` detection with automated classification (CPU-bound, I/O-bound, memory-bound)
- [ ] Extend `codec_profiler` with per-codec frame encode/decode timing histograms
- [ ] Add `pipeline_profiler` integration with `oximedia-pipeline` for automatic node instrumentation
- [ ] Implement profile comparison in `regression` (diff two profiles to find regressions)

## New Features
- [ ] Add Chrome Tracing JSON export in `event_trace` for viewing in chrome://tracing
- [ ] Implement `perf`-compatible output format for integration with Linux perf tools
- [ ] Add power/energy profiling module using platform-specific APIs (RAPL on Linux, IOKit on macOS)
- [ ] Implement distributed profiling for multi-node encoding clusters (aggregate across machines)
- [ ] Add real-time frame budget visualization (per-frame waterfall chart data)
- [ ] Implement automated optimization suggestions in `optimize` based on profiling data
- [ ] Add lock contention profiling in `thread` module (track mutex wait times)

## Performance
- [ ] Reduce profiling overhead by using thread-local storage for sampling counters
- [ ] Implement zero-allocation event recording in `event_trace` with pre-allocated ring buffer
- [ ] Use lock-free data structures in `allocation_tracker` to minimize measurement perturbation
- [ ] Add configurable buffer sizes in `flamegraph` generation to handle deep call stacks efficiently

## Testing
- [ ] Add profiling overhead measurement tests (verify < 1% overhead in Sampling mode)
- [ ] Test `regression` detection with synthetic benchmark data containing known regressions
- [ ] Add tests for `flame` and `flamegraph` SVG output correctness with known call trees
- [ ] Test `hotspot` detection accuracy with synthetic workloads having known hot functions
- [ ] Verify `memory_profiler` tracks allocations correctly under concurrent allocation patterns

## Documentation
- [ ] Add profiling quickstart guide with common workflow patterns
- [ ] Document ProfilingMode selection criteria (when to use Sampling vs Instrumentation)
- [ ] Add flame graph interpretation guide with annotated example output
