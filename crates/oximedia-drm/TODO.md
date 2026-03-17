# oximedia-drm TODO

## Current Status
- 35 source files covering CENC, key management, licensing, device auth, policy, and watermarking
- 4 DRM systems: Widevine, PlayReady, FairPlay, ClearKey (feature-gated)
- AES-CTR and AES-GCM encryption via aes-gcm crate
- PSSH box parsing/generation, license server framework, key rotation
- Additional: geo-fencing, audit trails, compliance, device registry, session tokens
- Dependencies: aes-gcm, base64, uuid, serde_json, quick-xml

## Enhancements
- [x] Replace `expect("hardcoded ... UUID is valid")` calls in `DrmSystem::system_id()` and `from_uuid()` with `const` UUIDs or lazy_static (no unwrap policy)
- [x] Add AES-CBC encryption mode support in `aes_cbc.rs` for CENC `cbcs` scheme (required by FairPlay)
- [ ] Implement actual Widevine license request/response protocol in `widevine.rs`
- [ ] Implement actual PlayReady license acquisition in `playready.rs`
- [ ] Implement actual FairPlay Streaming key delivery in `fairplay.rs`
- [ ] Enhance `key_rotation.rs` with configurable rotation intervals and overlap windows
- [x] Add PSSH box version 1 support with KID list in `pssh.rs`
- [ ] Implement `license_chain.rs` hierarchical license inheritance
- [x] Add rate limiting to `license_server.rs` to prevent abuse (implemented in `rate_limit.rs`)

## New Features
- [x] Implement CPIX (Content Protection Information Exchange) document support
- [ ] Add DASH CENC signaling helpers for MPD manifest generation
- [ ] Implement hardware-backed key storage interface (TPM/Secure Enclave abstraction)
- [x] Add ClearKey EME (Encrypted Media Extensions) JSON license format support
- [x] Implement multi-DRM packaging (single content encrypted, multiple PSSH boxes)
- [x] Add forensic watermark detection in `watermark_detect` complementing `watermark_embed.rs`
- [ ] Implement license offline persistence and renewal in `offline.rs`
- [ ] Add HDCP output protection level enforcement in `output_control.rs`

## Performance
- [ ] Use hardware AES-NI instructions via aes crate's `aes` feature for CTR mode encryption
- [x] Add parallel encryption of CENC subsample ranges using rayon
- [ ] Cache UUID parsing results as constants to avoid repeated string parsing
- [ ] Optimize `content_key.rs` key derivation with pre-computed key schedules
- [ ] Add buffer pooling for encryption/decryption operations to reduce allocations

## Testing
- [x] Add CENC encryption/decryption round-trip tests with known test vectors
- [ ] Test PSSH parsing against real Widevine and PlayReady PSSH boxes
- [ ] Add `clearkey.rs` integration test with W3C ClearKey test vectors
- [ ] Test `key_rotation_schedule.rs` with time-based rotation triggers
- [ ] Add `geo_fence.rs` tests with various IP geolocation scenarios
- [ ] Test `device_registry.rs` device registration and revocation flows
- [ ] Add `compliance.rs` tests verifying CPSA (Content Protection Security Architecture) rules

## Documentation
- [ ] Document the CENC encryption scheme variants (cenc, cbcs, cens, cbc1)
- [ ] Add DRM system comparison table (Widevine vs PlayReady vs FairPlay vs ClearKey)
- [ ] Document the key management lifecycle from `key_lifecycle.rs`
