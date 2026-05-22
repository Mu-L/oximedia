# oximedia-ndi TODO

## Current Status
- 38 source files implementing clean-room NDI protocol (no official SDK dependency)
- Core: NdiSender, NdiReceiver, DiscoveryService (mDNS-based), TallyServer
- Codec: SpeedHQ codec for low-latency video compression, YUV format support
- Features: PTZ control, tally lights, frame sync, genlock, bandwidth adaptation, failover
- Modules: frame_buffer, clock_sync, connection_state, statistics, source_filter, routing, group, metadata
- Additional: recording, latency_monitor, color_space_ndi, av_buffer, channel_map, tally_bus/tally_manager

## Enhancements
- [x] Implement actual mDNS advertisement in DiscoveryService sender-side (verified 2026-05-16; src/mdns_advertiser.rs:160 MdnsAdvertiser::start, MdnsAdvertisement:30, 381 lines)
- [ ] Add NDI|HX2 compressed mode support using AV1 instead of SpeedHQ for lower bandwidth (verified-open 2026-05-16: hx2.rs:172 Hx2Encoder exists but is a stub; doc says "full AV1 integration would call rav1e/dav1d" at line 167)
- [x] Implement connection_state reconnection logic with exponential backoff on connection loss
- [x] Add frame dropping strategy in frame_buffer when receiver falls behind sender rate
- [x] Improve clock_sync with PTP-like (IEEE 1588) precision timing instead of NTP-style sync (verified 2026-05-16; src/ptp_clock.rs:178 PtpClock, PtpServoConfig:117 PI servo, process_sample:216, 523 lines)
- [x] Add bandwidth probing in bandwidth module to auto-detect available network capacity (verified 2026-05-16; src/bandwidth_probe.rs:111 BandwidthProber, estimate_bps:201, QualityLevel:67, 600 lines)
- [x] Implement source_filter with regex pattern matching on source names and group filtering

## New Features
- [x] Add NDI recording to disk with segmented file output (timed segments, like NDI Tools Record) (verified 2026-05-16; src/recording.rs:170 RecordingSession, RecordingConfig:64, SegmentInfo:117, 525 lines)
- [x] Implement NDI routing/switching -- route any discovered source to any output (NDI Router equivalent) (verified 2026-05-16; src/routing.rs:48 NdiRouter, NdiRoute:10, add_route:61, 371 lines)
- [ ] Add multi-GPU encoding support for high-resolution NDI sending (4K60 requires significant compute) (verified-open 2026-05-16: no multi-GPU module in ndi sources; GPU encoding not present)
- [x] Implement NDI alpha channel support for keying/compositing workflows (verified 2026-05-16; src/alpha_channel.rs:59 AlphaFrame, premultiply:124, unpremultiply:144, 433 lines)
- [x] Add KVM (keyboard/video/mouse) metadata transport over NDI metadata channel (verified 2026-05-16; src/kvm.rs:61 KeyEvent, MouseMoveEvent:129, KeyModifiers:27, 609 lines)
- [x] Implement NDI bridge mode for cross-subnet discovery and streaming (verified 2026-05-16; src/bridge.rs:135 BridgeRouteTable, RelayEndpoint:92, SubnetId:29, 537 lines)
- [x] Add embedded web preview server -- serve low-res MJPEG preview of any NDI source via HTTP (verified 2026-05-16; src/web_preview.rs:152 MjpegBroadcaster, JpegFrame:100, PreviewConfig:33, 606 lines)

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
