# oximedia-net TODO

## Current Status
- 26 source files/directories covering major streaming protocols
- Protocols: HLS, DASH, RTMP, SRT, WebRTC, SMPTE ST 2110, QUIC
- Infrastructure: CDN (multi-CDN failover, circuit breaker), ABR (adaptive bitrate), FEC (forward error correction)
- Network: connection_pool, bandwidth_estimator, bandwidth_throttle, flow_control, packet_buffer, qos_monitor
- Utilities: protocol_detect, retry_policy, session_tracker, stream_mux, multicast, network_path, rtp_session
- Re-exports: SRT stats, encryption session, ABR controller/variant/bandwidth types

## Enhancements
- [x] Implement CMAF low-latency HLS (LL-HLS) with partial segment support in hls module (verified 2026-05-16; src/hls/ll_hls.rs:727 lines, struct LlHlsConfig:18, MediaPart:65)
- [x] Add DASH low-latency (LL-DASH) with chunked transfer encoding in dash module (verified 2026-05-16; src/dash/ll_dash.rs, src/dash/ll_dash_chunker.rs)
- [x] Implement SRT caller/listener/rendezvous connection modes fully in srt module (verified 2026-05-16; src/srt/connection_mode.rs:24-28 CallerState:59, ListenerState:171, RendezvousState)
- [x] Add RTMP enhanced mode (RTMP+) with AV1/VP9 codec support in rtmp module (verified 2026-05-16; src/rtmp/enhanced.rs:833 lines)
- [x] Implement WebRTC WHIP/WHEP signaling for browser-based ingest/playback in webrtc module (verified 2026-05-16; src/webrtc/whip_whep.rs:1040 lines)
- [x] Add QUIC datagram mode for ultra-low-latency media transport in quic module
- [x] Improve ABR algorithm with buffer-based (BBA) strategy alongside bandwidth-based in abr module
- [x] Add SRT encryption with AES-256-GCM in addition to current AES-128-CTR

## New Features
- [x] Implement RIST (Reliable Internet Stream Transport) protocol as alternative to SRT (verified 2026-05-16; src/rist.rs:1493 lines)
- [x] Add Zixi-compatible protocol support for broadcast contribution links
- [x] Implement SMPTE ST 2022-7 seamless protection switching (dual-path redundancy) in smpte2110 (verified 2026-05-16; src/smpte2022_7.rs:1018 lines)
- [x] Add media relay/restreaming server -- receive from one protocol, retransmit on another (verified 2026-05-16; src/relay.rs)
- [ ] Implement bandwidth-aware transcoding trigger -- signal codec crate to reduce quality when bandwidth drops (verified-open 2026-05-16: bandwidth_trigger.rs exists but trigger→codec signaling not wired)
- [x] Add ICE (Interactive Connectivity Establishment) for WebRTC NAT traversal (verified 2026-05-16; src/webrtc/ice.rs:1082 lines, src/webrtc/ice_agent.rs)
- [x] Implement multipath streaming -- send redundant streams over multiple network interfaces (verified 2026-05-16; src/multipath.rs:1241 lines)

## Performance
- [x] Use connection pooling in CDN module for keep-alive HTTP connections to edge servers (verified 2026-05-16; src/cdn/keepalive_pool.rs:1 HTTP keep-alive connection pool per-host)
- [ ] Implement zero-copy segment serving using sendfile/splice syscalls in HLS/DASH server (verified-open 2026-05-16: no sendfile/splice in HLS/DASH server paths)
- [ ] Add io_uring support for high-throughput RTP packet handling in smpte2110 (verified-open 2026-05-16: no io_uring in smpte2110)
- [ ] Implement packet pacing in SRT to smooth out burst traffic and reduce jitter (verified-open 2026-05-16: not yet implemented)
- [ ] Profile and optimize FEC encoding/decoding -- consider SIMD-accelerated XOR operations (verified-open 2026-05-16: not yet implemented)
- [ ] Cache parsed manifest/playlist structures to avoid re-parsing on each client request (verified-open 2026-05-16: not yet implemented)

## Testing
- [ ] Add HLS playlist generation test verifying M3U8 format compliance (EXT-X-VERSION, segment tags)
- [ ] Test DASH MPD generation against schema validation
- [ ] Add SRT connection test with simulated packet loss verifying ARQ retransmission
- [ ] Test CDN failover: simulate primary CDN timeout, verify automatic switch to secondary
- [ ] Add ABR test: simulate fluctuating bandwidth, verify smooth quality transitions without rebuffering
- [ ] Test RTMP handshake and chunk stream parsing against reference implementation output

## Documentation
- [ ] Document supported protocol matrix with capabilities (latency, reliability, encryption, ABR)
- [ ] Add protocol selection guide: when to use HLS vs DASH vs SRT vs WebRTC vs ST 2110
- [ ] Document CDN configuration with examples for Cloudflare, Fastly, CloudFront integration
