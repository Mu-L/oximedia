# oximedia-pipeline TODO

## Current Status
- 4 modules: `builder`, `execution_plan`, `graph`, `node`
- Provides declarative media processing pipeline DSL with typed filter graph
- Supports source/sink/filter nodes, pipeline validation, topological sort, execution planning
- Minimal dependencies (thiserror, uuid)

## Enhancements
- [x] Add parallel execution support in `ExecutionPlan` for independent pipeline branches
- [x] Implement pipeline serialization/deserialization (serde support for `PipelineGraph`)
- [x] Add pipeline merging — combine two `PipelineGraph` instances with shared nodes
- [x] Extend `FilterConfig` with parametric filter configuration (key-value properties)
- [x] Add `PipelineBuilder::branch()` for splitting a pipeline into multiple output paths
- [ ] Implement cycle detection with detailed path reporting in `PipelineError::CycleDetected`
- [ ] Add pipeline graph visualization export (DOT/Graphviz format from `PipelineGraph`)
- [ ] Support dynamic pipeline reconfiguration (add/remove nodes at runtime)

## New Features
- [ ] Add `PipelineProfiler` for timing each node's execution within a pipeline
- [ ] Implement pipeline templates — predefined pipeline patterns (transcode, ABR, thumbnail)
- [ ] Add a `PipelineValidator` that checks hardware resource availability before execution
- [x] Implement pipeline checkpointing — save/resume interrupted pipelines
- [ ] Add conditional nodes (`IfNode`) for branching based on stream properties
- [ ] Implement pipeline composition — nest sub-pipelines as single nodes
- [ ] Add `PipelineMetrics` for collecting throughput, latency, and buffer stats per node

## Performance
- [ ] Implement node fusion optimization in `PipelineOptimizer` (merge adjacent scale+crop)
- [ ] Add zero-copy frame passing between adjacent nodes in the same thread
- [ ] Implement memory pool allocation for `ResourceEstimate` to reduce allocation overhead
- [ ] Add SIMD-aware node scheduling in `ExecutionPlanner` for data-parallel filters

## Testing
- [ ] Add property-based tests for topological sort correctness with random graph shapes
- [ ] Test pipeline validation with malformed graphs (disconnected nodes, dangling pads)
- [ ] Add benchmark tests for `ExecutionPlanner` with large pipeline graphs (1000+ nodes)
- [ ] Test `PipelineBuilder` chain correctness for all filter types (scale, flip, crop, etc.)

## Documentation
- [ ] Add architecture diagram showing node/edge/pad relationship model
- [ ] Document thread safety guarantees for concurrent pipeline execution
- [ ] Add examples for multi-input pipelines (picture-in-picture, audio mixing)
