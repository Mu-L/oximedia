# oximedia-playout TODO

## Current Status
- 41 modules for professional broadcast playout server
- Key types: PlayoutServer, PlayoutConfig, VideoFormat, AudioFormat, PlayoutState
- Modules include: ad_insertion, api, asrun, automation, branding, bxf, catchup, cg, channel, channel_config, clip_store, compliance_ingest, content, device, event_log, failover, frame_buffer, gap_filler, graphics, highlight_automation, ingest, media_router_playout, monitoring, output, output_router, playback, playlist, playlist_ingest, playout_log, playout_schedule, preflight, rundown, schedule_block, schedule_slot, scheduler, secondary_events, signal_chain, subtitle_inserter, tally_system, transitions
- WASM-conditional compilation for many modules; feature gates for decklink/ndi

## Enhancements
- [x] Add graceful shutdown with in-flight frame draining in `PlayoutServer::stop()`
- [x] Implement configurable frame drop recovery strategy in `frame_buffer` (repeat last, black, slate)
- [x] Extend `failover` with cascading failover chains (primary -> secondary -> tertiary -> slate)
- [x] Add audio level monitoring with EBU R128 loudness gates in `monitoring`
- [x] Implement hot-swap of `PlayoutConfig` without stopping the server (runtime reconfiguration)
- [ ] Extend `graphics` engine with template-based lower-third generation (name/title fields)
- [ ] Add frame-accurate cue point triggering in `secondary_events` via timecode matching
- [ ] Implement `clip_store` integrity verification (checksum validation on ingest)

## New Features
- [ ] Add PTP (Precision Time Protocol) clock source support alongside internal/SDI
- [ ] Implement timecode burn-in overlay rendering in the `timecode_overlay` module stub
- [ ] Add IP multicast output support in `output` module (SMPTE ST 2110)
- [ ] Implement automated compliance recording in `compliance_ingest` with retention policies
- [ ] Add `catchup` module integration with VOD platform export (HLS manifest generation)
- [ ] Implement automated highlight clip extraction in `highlight_automation` using scene analysis
- [ ] Add BXF (Broadcast Exchange Format) import/export in `bxf` for schedule interchange
- [ ] Implement multi-format simulcast (HD + UHD output from single playout chain)

## Performance
- [ ] Use lock-free ring buffer in `frame_buffer` for zero-contention frame passing
- [ ] Implement GPU-accelerated graphics compositing in `graphics` engine
- [ ] Pre-decode upcoming playlist items in background threads for gapless transitions
- [ ] Profile and optimize `signal_chain` processing to stay within frame budget at 60fps

## Testing
- [ ] Add integration test for full playout lifecycle (start -> load playlist -> play -> stop)
- [ ] Test `failover` module under simulated source failure conditions
- [ ] Add frame timing accuracy tests verifying sub-millisecond jitter in `playback`
- [ ] Test `ad_insertion` SCTE-35 splice point accuracy with various content boundaries
- [ ] Add stress test for `scheduler` with rapid playlist swaps during live playout

## Documentation
- [ ] Document signal chain architecture (input -> decode -> process -> compose -> encode -> output)
- [ ] Add deployment guide for 24/7 broadcast playout with monitoring setup
- [ ] Document VideoFormat/AudioFormat selection guidelines for different broadcast standards
