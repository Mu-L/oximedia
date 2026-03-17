# oximedia-routing TODO

## Current Status
- 33 modules covering crosspoint matrix routing, virtual patch bay, channel mapping/remapping, signal flow graphs, audio embedding/de-embedding, format conversion, gain staging, AFL/PFL/Solo monitoring, preset management, MADI (64-channel), Dante metadata, NMOS IS-04/IS-05, automation, IP routing (ST 2110), failover, bandwidth budgeting, route optimization, link aggregation, latency calculation, topology mapping, traffic shaping
- Prelude module re-exports key types for ergonomic access
- Feature gates: `nmos-http`, `nmos-discovery`
- Dependencies: oximedia-core, oximedia-audio, petgraph, serde, optional hyper/mdns-sd

## Enhancements
- [ ] Add mix-minus routing in `matrix` for broadcast IFB feeds (output = sum of inputs minus one)
- [ ] Extend `channel::ChannelRemapper` with 7.1.4 Atmos bed downmix and upmix matrices
- [ ] Add `signal_monitor` threshold-based alerts (signal present, signal lost, overmodulation)
- [ ] Implement `failover_route` automatic switchover with configurable detection timeout and glitch-free transition
- [ ] Extend `automation::AutomationTimeline` with curve-based gain automation (linear, S-curve, exponential fades)
- [ ] Add `route_audit` diff comparison between two routing snapshots for change tracking
- [ ] Implement `latency_calc` end-to-end latency measurement across multi-hop signal paths
- [ ] Add `bandwidth_budget` warnings when aggregate throughput approaches network capacity limits

## New Features
- [ ] Add a `virtual_soundcard` module for OS-level audio routing (WASAPI/CoreAudio/ALSA loopback)
- [ ] Implement `aes67` module for AES67 audio-over-IP interoperability alongside Dante
- [ ] Add `routing_macro` module for defining complex routing configurations declaratively
- [ ] Implement `signal_generator` for test tones (sine, pink noise, sweep) routable through the matrix
- [ ] Add `metering_bridge` module for inserting level meters at arbitrary points in the signal path
- [ ] Implement `routing_snapshot` for saving/restoring complete routing state with atomic rollback
- [ ] Add `gpio_trigger` module for hardware GPI/O-triggered routing changes
- [ ] Implement `tally_system` for signaling active routing paths to external tally light controllers
- [ ] Add `intercom` module for point-to-point and party-line communication routing

## Performance
- [ ] Optimize `CrosspointMatrix::connect` for large matrices (256x256+) using sparse representation
- [ ] Add SIMD-optimized summing in `matrix` for mix bus computation with many active crosspoints
- [ ] Implement lock-free routing updates in `matrix_router` for glitch-free real-time changes
- [ ] Cache `flow::SignalFlowGraph::validate` results and invalidate only on topology changes
- [ ] Add zero-latency path optimization in `route_optimizer` for live monitoring chains

## Testing
- [ ] Add tests for `ChannelRemapper` with all standard layout conversions (mono/stereo/5.1/7.1)
- [ ] Test `failover_route` switchover timing under simulated signal loss conditions
- [ ] Add stress tests for `CrosspointMatrix` with rapid connect/disconnect cycles (1000 ops/sec)
- [ ] Test `automation` timeline execution with sub-frame accuracy at various frame rates
- [ ] Add integration tests for `nmos` discovery and registration with mock registry

## Documentation
- [ ] Add signal flow diagrams for common routing scenarios (live production, post-production, broadcast)
- [ ] Document the MADI channel numbering and frame mode relationship in `madi`
- [ ] Add NMOS IS-04/IS-05 integration guide with network configuration requirements
- [ ] Document the gain staging chain: input gain -> matrix gain -> bus gain -> output gain
