# oximedia-image-transform TODO

## Current Status
- 8 modules implementing Cloudflare Images-compatible URL image transformation
- `transform.rs`: Strongly-typed params (resize, crop, quality, format, rotation, fit modes, border, padding, trim)
- `parser.rs`: Parse `/cdn-cgi/image/` paths, query strings, comma-separated transform strings
- `negotiation.rs`: Accept-header format negotiation (AVIF > WebP > JPEG/PNG) + client hints (DPR, Width, Viewport-Width)
- `processor.rs`: Image processing execution pipeline
- `quality.rs`: SSIM-guided quality auto-tuning with complexity analysis
- `security.rs`: Request validation, SSRF prevention, HMAC-SHA256 signed URL verification
- `watermark.rs`: Text and image watermark overlay with 9 positions, opacity, scaling
- `face_detect.rs`: Skin-tone + saliency-based face detection for smart gravity cropping
- Dependencies: thiserror only (lightweight crate)

## Enhancements
- [x] Add signed URL support to `security.rs` with HMAC-SHA256 verification to prevent abuse
- [x] Extend `negotiation.rs` with client-hints support (DPR, Viewport-Width, Width headers)
- [x] Add quality auto-tuning in `quality.rs` based on image complexity (SSIM-guided)
- [x] Implement progressive JPEG output option in `transform.rs` for faster perceived loading
- [x] Add cache-control header generation to `negotiation.rs` based on transform parameters
- [x] Extend `parser.rs` with named preset support (e.g., `/cdn-cgi/image/preset=thumbnail/photo.jpg`)
- [ ] Add aspect ratio preservation enforcement in `transform.rs` when both width and height are set

## New Features
- [x] Add a `watermark.rs` module for image watermark overlay (text and image) with configurable position and opacity
- [x] Implement a `face_detect.rs` module for smart cropping based on face detection
- [x] Add a `blur_region.rs` module for selective region blurring (privacy, NSFW)
- [ ] Implement a `responsive.rs` module for srcset/picture element generation with multiple breakpoints
- [ ] Add an `animation.rs` module for GIF/WebP animation resize and optimization
- [ ] Implement an `origin_fetch.rs` module for fetching source images from remote URLs with caching
- [ ] Add a `metrics.rs` module for tracking transform request statistics (hit rate, latency, format distribution)
- [ ] Implement a `batch_transform.rs` module for processing multiple transform variants in a single request
- [ ] Add an `image_analysis.rs` module for dominant color extraction and blur hash generation

## Performance
- [ ] Add response caching layer with content-addressable storage keyed on transform params hash
- [ ] Implement streaming transform pipeline to avoid loading entire source image into memory
- [ ] Add early termination in `processor.rs` when output dimensions are smaller than a threshold
- [ ] Implement parallel transform execution for batch requests

## Testing
- [ ] Add parser fuzz tests for malformed `/cdn-cgi/image/` URLs (empty params, invalid values, injection attempts)
- [ ] Test `negotiation.rs` with all common browser Accept header combinations
- [ ] Add round-trip tests: parse transform string -> serialize -> parse again -> compare
- [ ] Test `security.rs` with path traversal attempts, oversized dimensions, and resource exhaustion vectors
- [ ] Add integration tests with actual image data through the full parse -> process pipeline
- [ ] Test edge cases: zero width, negative quality, unknown format strings

## Documentation
- [ ] Document all supported transform parameters with examples and default values
- [ ] Add a compatibility matrix showing which Cloudflare Images features are supported
- [ ] Document the content negotiation priority rules and fallback behavior
