# oximedia-ndi TODO

## Current Status
- 38 source files implementing clean-room NDI protocol (no official SDK dependency)
- Core: NdiSender, NdiReceiver, DiscoveryService (mDNS-based), TallyServer
- Codec: SpeedHQ codec for low-latency video compression, YUV format support
- Features: PTZ control, tally lights, frame sync, genlock, bandwidth adaptation, failover
- Modules: frame_buffer, clock_sync, connection_state, statistics, source_filter, routing, group, metadata
- Additional: recording, latency_monitor, color_space_ndi, av_buffer, channel_map, tally_bus/tally_manager

## Enhancements
- [ ] Implement actual mDNS advertisement in DiscoveryService sender-side (currently may only discover)
- [ ] Add NDI|HX2 compressed mode support using AV1 instead of SpeedHQ for lower bandwidth
- [x] Implement connection_state reconnection logic with exponential backoff on connection loss
- [x] Add frame dropping strategy in frame_buffer when receiver falls behind sender rate
- [ ] Improve clock_sync with PTP-like (IEEE 1588) precision timing instead of NTP-style sync
- [ ] Add bandwidth probing in bandwidth module to auto-detect available network capacity
- [x] Implement source_filter with regex pattern matching on source names and group filtering

## New Features
- [ ] Add NDI recording to disk with segmented file output (timed segments, like NDI Tools Record)
- [ ] Implement NDI routing/switching -- route any discovered source to any output (NDI Router equivalent)
- [ ] Add multi-GPU encoding support for high-resolution NDI sending (4K60 requires significant compute)
- [ ] Implement NDI alpha channel support for keying/compositing workflows
- [ ] Add KVM (keyboard/video/mouse) metadata transport over NDI metadata channel
- [ ] Implement NDI bridge mode for cross-subnet discovery and streaming
- [ ] Add embedded web preview server -- serve low-res MJPEG preview of any NDI source via HTTP

## Performance
- [ ] Implement zero-copy frame passing between sender/receiver using shared memory for local connections
- [x] Add SIMD-accelerated YUV<->RGB conversion in color_space_ndi
- [ ] Use io_uring (Linux) / kqueue (macOS) for high-throughput network I/O in transport
- [ ] Implement frame buffer pool to avoid allocation per video frame in frame_buffer
- [ ] Add parallel SpeedHQ encoding of frame slices for multi-core utilization in codec module
- [ ] Profile and reduce per-frame metadata serialization overhead in metadata_frame

## Testing
- [ ] Add loopback test: NdiSender -> NdiReceiver on localhost verifying frame data integrity
- [ ] Test discovery with multiple sources on same machine verifying unique naming
- [ ] Add latency measurement test: timestamp frames at send, measure at receive
- [ ] Test failover behavior when primary source goes offline and backup takes over
- [ ] Verify PTZ command serialization/deserialization roundtrip for all PtzCommand variants
- [ ] Test tally state propagation: set program tally on sender, verify receiver sees it

## Documentation
- [ ] Document the NDI protocol wire format as implemented (packet structure, handshake, frame headers)
- [ ] Add network setup guide (firewall ports, multicast requirements, VLAN recommendations)
- [ ] Document the SpeedHQ codec implementation and its performance characteristics vs. original NDI codec
