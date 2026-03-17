# oximedia-spatial TODO

## Current Status
- 8 source files (lib.rs + 7 modules): ambisonics, binaural, head_tracking, object_audio, room_simulation, vbap, wave_field
- Higher-Order Ambisonics (HOA) up to 3rd order encoding/decoding
- HRTF-based binaural rendering, image-source room acoustics, VBAP for 2D/3D loudspeaker arrays
- Quaternion-based head tracking, wave field synthesis, ADM object audio (Dolby Atmos / Auro-3D / DTS:X beds)
- Minimal dependencies: only thiserror

## Enhancements
- [x] Extend `AmbisonicsEncoder` to support 4th and 5th order HOA for large-venue immersive applications
- [x] Add near-field compensation filters to `ambisonics` for close-source rendering accuracy
- [x] Implement HRTF interpolation in `binaural` using spherical harmonics for smoother head rotation transitions
- [x] Add frequency-dependent wall absorption coefficients to `room_simulation` (currently likely uses single absorption value)
- [x] Extend `vbap` to support irregular speaker layouts with automatic triangulation (Delaunay-based)
- [ ] Add IMU sensor fusion (accelerometer + gyroscope + magnetometer) to `head_tracking` complementary filter
- [ ] Implement distance attenuation models (inverse square, logarithmic, custom curve) in `object_audio`
- [ ] Add Doppler effect simulation for moving sound sources

## New Features
- [ ] Implement `dbap` (Distance-Based Amplitude Panning) module as alternative to VBAP for non-regular layouts
- [ ] Add `reverb` module with algorithmic late reverberation (Schroeder/Moorer style) separate from room simulation
- [ ] Implement `hoa_decoder` module with AllRAD (All-Round Ambisonic Decoding) for arbitrary speaker arrays
- [ ] Add `spatial_audio_format` module for ADM BWF (Broadcast Wave Format) and MPEG-H 3D Audio metadata I/O
- [ ] Implement `acoustic_raytracer` module for geometric acoustics beyond image-source method
- [ ] Add `binauralizer` module that converts any multi-channel format to binaural using virtual speakers
- [ ] Implement `spatial_capture` module for A-format to B-format microphone array conversion (Soundfield, Eigenmike)
- [ ] Add `zone_control` module for multi-zone spatial audio rendering in installations

## Performance
- [ ] Use SIMD-accelerated spherical harmonic coefficient computation in `ambisonics` encoding
- [ ] Implement partitioned convolution in `binaural` HRTF rendering for lower latency (overlap-save with FFT)
- [ ] Cache VBAP gain matrices for static speaker layouts to avoid per-frame triangulation lookups
- [ ] Use parallel ray casting in `room_simulation` for independent reflection path computation
- [ ] Pre-compute and cache head-tracking rotation matrices for common angle quantization steps

## Testing
- [ ] Add energy preservation tests for `AmbisonicsEncoder` (total energy in equals total energy out)
- [ ] Test `binaural` rendering with known HRTF datasets (MIT KEMAR, CIPIC) and verify ITD/ILD values
- [ ] Add convergence tests for `head_tracking` complementary filter with synthetic IMU data
- [ ] Test `vbap` panning law correctness for source positions at exact speaker locations (gain should be 1.0 at that speaker)
- [ ] Add round-trip test: encode mono source to Ambisonics then decode back, verify spectral similarity

## Documentation
- [ ] Add spatial coordinate convention diagram (azimuth, elevation, distance) used throughout the crate
- [ ] Document supported loudspeaker layout formats and how to define custom arrays for VBAP/WFS
- [ ] Add usage examples for integrating head_tracking with binaural rendering in real-time playback
