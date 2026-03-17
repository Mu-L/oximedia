# oximedia-mixer TODO

## Current Status
- 33 source files implementing a professional DAW-style audio mixer
- Core: AudioMixer with channels, buses (master/group/aux/matrix), effects, automation, session management
- Modules: automation, bus, channel, effects/effects_chain, dynamics, eq_band, limiter, metering, routing, session, snapshot, crossfade, delay_line, sidechain, vca, pan_matrix, matrix_mixer
- Additional: fader_group, solo_bus, talkback, scene_recall, monitor_mix, send_return, insert_chain, processing
- Full DSP pipeline: input gain -> effects -> fader (× VCA) -> pan -> PDC -> sends -> bus -> master with soft clipping
- ProcessingEngine: bus routing, RuntimeEffectsChain, aux send/return, VCA groups, PDC delay compensation

## Enhancements
- [ ] Replace `unwrap_or` in `package_both` path conversion with proper error propagation
- [x] Make `process()` use bus routing -- channels route to group/aux buses via ProcessingEngine
- [x] Implement actual effect processing in the channel strip -- RuntimeEffectsChain with AudioEffect trait integration
- [x] Add send/return routing in process() -- aux sends (pre/post-fader) wired into DSP path with bus accumulation
- [x] Implement VCA (Voltage Controlled Amplifier) group control in process() -- VCA fader multiplied onto linked channels
- [ ] Add solo-in-place and AFL/PFL solo modes to the processing pipeline
- [ ] Implement automation playback in process() -- read automation data and apply parameter changes per buffer
- [ ] Replace soft_clip() tanh approximation with a proper oversampled limiter on the master bus

## New Features
- [ ] Add surround panning (5.1/7.1) support in pan_matrix beyond current stereo L/R panning
- [ ] Implement Ambisonics encoding/decoding for spatial audio mixing
- [ ] Add plugin hosting API for external audio effects (VST3-style interface definition)
- [ ] Implement channel folding/unfolding for stereo-to-mono and mono-to-stereo conversion
- [x] Add latency compensation (PDC - Plugin Delay Compensation) across effect chains -- PdcDelayLine with recompute_pdc()
- [ ] Implement offline bounce/render with higher-than-realtime processing
- [ ] Add MIDI control surface mapping (MCU/HUI protocol support)

## Performance
- [ ] Process channels in parallel using rayon when channel count > 8
- [ ] Use SIMD for the master bus summing loop (accumulate L/R across channels)
- [ ] Pre-allocate channel output buffers in AudioMixer instead of allocating per-process() call
- [ ] Implement lock-free parameter updates using atomic f32 for gain/pan to avoid blocking the audio thread
- [ ] Add buffer pooling to avoid repeated Vec allocation in extract_f32_samples()

## Testing
- [x] Add test that verifies gain=0.5 produces -6 dB output (linear gain verification)
- [x] Test pan law: center pan should produce equal L/R, hard left should produce silence on R
- [ ] Add test for soft_clip: verify output never exceeds 1.0 for any input value
- [ ] Test channel add/remove during processing (dynamic channel count changes)
- [x] Add integration test that processes a known signal through the full mixer pipeline and verifies output

## Documentation
- [ ] Document the DSP signal flow with a detailed diagram (input -> gain -> phase -> inserts -> EQ -> dynamics -> fader -> pan -> sends)
- [ ] Add examples for common workflows: basic stereo mix, submix with group bus, reverb send/return
- [ ] Document automation modes (Read/Write/Touch/Latch/Trim) with usage examples
