# oximedia-graph TODO

## Current Status
- 30+ modules implementing a filter graph pipeline for media processing
- Core: graph builder, nodes, ports, connections, frame pool
- Filters: video (passthrough, null sink) and audio filters
- Advanced: topological sort, cycle detection, graph merge, partitioning, subgraphs
- Scheduling, profiling, serialization, visualization, optimization
- Dependencies: oximedia-core, oximedia-codec, oximedia-audio, fontdue

## Enhancements
- [ ] Add parallel execution support to `scheduler.rs` using rayon for independent node branches
- [ ] Extend `optimization.rs` with automatic filter fusion (merge adjacent compatible nodes)
- [ ] Add backpressure mechanism to `data_flow.rs` to prevent memory exhaustion on slow sinks
- [ ] Implement frame format negotiation between connected ports in `port.rs`
- [ ] Add dynamic graph reconfiguration (hot-swap nodes) without rebuilding the entire graph
- [ ] Extend `graph_stats.rs` with latency histograms per node and per edge
- [ ] Add error recovery / retry semantics to `processing_graph.rs` for transient failures
- [ ] Implement `node_cache.rs` with LRU eviction policy and configurable cache size limits
- [ ] Add graph snapshot/restore for checkpoint-based processing in `serialize.rs`

## New Features
- [x] Add a `SplitFilter` node that duplicates frames to multiple output ports (tee/fanout)
- [x] Add a `MergeFilter` node that combines multiple input streams (e.g., picture-in-picture)
- [x] Implement a `RateLimitFilter` to throttle frame throughput for real-time playback
- [x] Add a `TimecodeFilter` node that stamps/modifies frame timestamps
- [x] Implement a graph DSL parser (text-based graph description) complementing `serialize.rs`
- [x] Add `filters/video/scale.rs` for resolution scaling within the graph pipeline
- [x] Add `filters/video/crop.rs` for frame cropping within the graph pipeline
- [ ] Add `filters/audio/resample.rs` for sample rate conversion within the graph pipeline
- [ ] Implement async graph execution mode using tokio tasks for I/O-bound source/sink nodes

## Performance
- [ ] Add SIMD-accelerated frame copy in `frame.rs` for large video buffers
- [ ] Implement zero-copy frame passing between compatible adjacent nodes
- [ ] Add memory pool pre-allocation in `FramePool` based on graph topology analysis
- [ ] Profile and optimize `topological.rs` sort for large graphs (>1000 nodes)
- [ ] Add lock-free ring buffers for inter-node frame passing in `port_buffer.rs`

## Testing
- [ ] Add integration tests for complex multi-branch graph topologies (diamond, fan-out/fan-in)
- [ ] Add stress tests for `graph_merge.rs` with overlapping node IDs
- [ ] Test `cycle_detect.rs` with self-loops and multi-edge cycles
- [ ] Add benchmarks for graph execution throughput at various node counts

## Documentation
- [ ] Add architecture diagram in module-level docs showing node/port/connection relationships
- [ ] Document thread-safety guarantees for concurrent graph execution
- [ ] Add examples for building common filter chains (transcode, overlay, split/merge)
