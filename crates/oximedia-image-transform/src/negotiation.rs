// SPDX-License-Identifier: Apache-2.0
// Copyright (c) COOLJAPAN OU (Team Kitasan)

//! Accept header parsing and format negotiation for Cloudflare Images-compatible
//! content negotiation.
//!
//! Implements RFC 7231 Accept header parsing with quality factors, and provides
//! format auto-negotiation that prefers modern formats (AVIF > WebP > JPEG/PNG).

use crate::transform::OutputFormat;

/// Parsed Accept header entry with quality factor.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptEntry {
    /// MIME type (e.g., "image/avif", "image/*", "*/*").
    pub media_type: String,
    /// Quality factor (0.0-1.0, default 1.0).
    pub quality: f32,
}

/// Parse an HTTP Accept header into an ordered list of [`AcceptEntry`].
///
/// Entries are sorted by quality factor descending. Entries with equal quality
/// preserve their original order (stable sort).
///
/// # Examples
///
/// ```
/// # use oximedia_image_transform::negotiation::parse_accept_header;
/// let entries = parse_accept_header("image/avif,image/webp;q=0.9,image/jpeg;q=0.8,*/*;q=0.1");
/// assert_eq!(entries[0].media_type, "image/avif");
/// assert_eq!(entries[0].quality, 1.0);
/// assert_eq!(entries[1].media_type, "image/webp");
/// ```
pub fn parse_accept_header(accept: &str) -> Vec<AcceptEntry> {
    let mut entries = Vec::new();

    for segment in accept.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }

        let mut parts = segment.splitn(2, ';');
        let media_type = match parts.next() {
            Some(mt) => mt.trim().to_string(),
            None => continue,
        };

        if media_type.is_empty() {
            continue;
        }

        let quality = match parts.next() {
            Some(params) => parse_quality_from_params(params),
            None => 1.0,
        };

        entries.push(AcceptEntry {
            media_type,
            quality,
        });
    }

    // Stable sort by quality descending
    entries.sort_by(|a, b| {
        b.quality
            .partial_cmp(&a.quality)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    entries
}

/// Extract quality value from the parameters portion of an Accept entry.
///
/// Looks for `q=<value>` among semicolon-separated parameters.
fn parse_quality_from_params(params: &str) -> f32 {
    for param in params.split(';') {
        let param = param.trim();
        if let Some(q_str) = param.strip_prefix("q=") {
            if let Ok(q) = q_str.trim().parse::<f32>() {
                return q.clamp(0.0, 1.0);
            }
        }
    }
    1.0
}

/// Determine the best output format based on Accept header and requested format.
///
/// - If `requested_format` is not [`OutputFormat::Auto`], returns it directly.
/// - If `requested_format` is [`OutputFormat::Auto`], negotiates based on the
///   Accept header with the following priority:
///   1. AVIF if accepted
///   2. WebP if accepted
///   3. PNG if accepted (useful when source has transparency)
///   4. JPEG as final fallback
pub fn negotiate_format(accept: &str, requested_format: OutputFormat) -> OutputFormat {
    if requested_format != OutputFormat::Auto {
        return requested_format;
    }

    let entries = parse_accept_header(accept);

    // Check for specific format support in priority order
    if accepts_mime(&entries, "image/avif") {
        return OutputFormat::Avif;
    }
    if accepts_mime(&entries, "image/webp") {
        return OutputFormat::WebP;
    }
    if accepts_mime(&entries, "image/png") {
        return OutputFormat::Png;
    }

    OutputFormat::Jpeg
}

/// Get the MIME type string for an output format.
pub fn format_to_mime(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Avif => "image/avif",
        OutputFormat::WebP => "image/webp",
        OutputFormat::Jpeg | OutputFormat::Baseline => "image/jpeg",
        OutputFormat::Png => "image/png",
        OutputFormat::Gif => "image/gif",
        OutputFormat::Json => "application/json",
        OutputFormat::Auto => "image/jpeg", // fallback
    }
}

/// HTTP response headers for a transformed image.
///
/// Follows Cloudflare Images conventions for caching, content negotiation,
/// and debug information.
#[derive(Debug, Clone)]
pub struct ResponseHeaders {
    /// Content-Type header value.
    pub content_type: String,
    /// Cache-Control header value.
    pub cache_control: String,
    /// ETag header value (quoted).
    pub etag: String,
    /// Vary header value.
    pub vary: String,
    /// Cloudflare-style debug header showing resize status and size info.
    pub cf_resized: String,
}

impl ResponseHeaders {
    /// Build response headers for a transformed image.
    ///
    /// - `content_type` is derived from the output format.
    /// - `cache_control` is set to aggressive immutable caching (1 year).
    /// - `etag` is derived from the cache key.
    /// - `vary` is set to "Accept" because `format=auto` depends on the Accept header.
    /// - `cf_resized` includes debug info about the transformation.
    pub fn new(
        format: OutputFormat,
        cache_key: &str,
        original_size: u64,
        transformed_size: u64,
    ) -> Self {
        let content_type = format_to_mime(format).to_string();
        let cache_control = "public, max-age=31536000, immutable".to_string();
        let etag = format!("\"{cache_key}\"");
        let vary = "Accept".to_string();

        let savings = if original_size > 0 && transformed_size < original_size {
            let pct = ((original_size - transformed_size) as f64 / original_size as f64) * 100.0;
            format!(" saved={pct:.1}%")
        } else {
            String::new()
        };
        let cf_resized = format!(
            "internal=ok/f={} orig={original_size} out={transformed_size}{savings}",
            format.as_str()
        );

        Self {
            content_type,
            cache_control,
            etag,
            vary,
            cf_resized,
        }
    }

    /// Return all headers as key-value pairs suitable for HTTP response construction.
    pub fn to_pairs(&self) -> Vec<(&str, &str)> {
        vec![
            ("Content-Type", &self.content_type),
            ("Cache-Control", &self.cache_control),
            ("ETag", &self.etag),
            ("Vary", &self.vary),
            ("Cf-Resized", &self.cf_resized),
        ]
    }
}

/// Generate an appropriate `Cache-Control` header value for a transformed image.
///
/// The policy is designed to balance CDN caching efficiency against freshness:
///
/// | Condition                                  | Value                                       |
/// |--------------------------------------------|---------------------------------------------|
/// | `params.quality < 50`                      | `"public, max-age=86400"` (1 day)           |
/// | `format` is AVIF **or** WebP               | `"public, max-age=604800, immutable"` (7 d) |
/// | Default                                    | `"public, max-age=3600"` (1 hour)           |
///
/// The quality check comes first: heavily-compressed assets age quickly because
/// they are typically regenerated with different settings as quality tuning evolves.
/// Modern formats (AVIF/WebP) benefit from long immutable caching because their
/// URLs already embed the format as part of the cache key.
///
/// # Example
///
/// ```
/// use oximedia_image_transform::negotiation::generate_cache_control;
/// use oximedia_image_transform::transform::{OutputFormat, TransformParams};
///
/// let mut params = TransformParams::default();
/// params.format = OutputFormat::Avif;
/// assert_eq!(generate_cache_control(&params), "public, max-age=604800, immutable");
///
/// params.format = OutputFormat::Jpeg;
/// params.quality = 40;
/// assert_eq!(generate_cache_control(&params), "public, max-age=86400");
///
/// params.quality = 85;
/// assert_eq!(generate_cache_control(&params), "public, max-age=3600");
/// ```
pub fn generate_cache_control(params: &crate::transform::TransformParams) -> String {
    // Quality < 50 → short cache; these assets are often experimental or degraded.
    if params.quality < 50 {
        return "public, max-age=86400".to_string();
    }
    // Modern lossless-friendly formats get long immutable caching.
    if matches!(params.format, OutputFormat::Avif | OutputFormat::WebP) {
        return "public, max-age=604800, immutable".to_string();
    }
    // Default: 1-hour cache suits most JPEG/PNG transforms.
    "public, max-age=3600".to_string()
}

/// Check if a client supports a specific image format based on an Accept header string.
///
/// Parses the Accept header and checks whether the MIME type corresponding to
/// the given format is accepted (either directly or via a wildcard).
pub fn supports_format(accept: &str, format: OutputFormat) -> bool {
    let entries = parse_accept_header(accept);
    let mime = format_to_mime(format);
    accepts_mime(&entries, mime)
}

/// Check if a list of accept entries accepts a given MIME type.
///
/// Supports exact match, type wildcard (e.g., `image/*`), and full wildcard (`*/*`).
/// Entries with quality 0 are treated as "not accepted".
fn accepts_mime(entries: &[AcceptEntry], mime: &str) -> bool {
    for entry in entries {
        if entry.quality <= 0.0 {
            continue;
        }
        // Exact match
        if entry.media_type == mime {
            return true;
        }
        // Full wildcard
        if entry.media_type == "*/*" {
            return true;
        }
        // Type wildcard (e.g., image/*)
        if let Some(prefix) = entry.media_type.strip_suffix("/*") {
            if let Some(type_part) = mime.split('/').next() {
                if prefix == type_part {
                    return true;
                }
            }
        }
    }
    false
}

// ============================================================================
// Client hints support (DPR, Viewport-Width, Width)
// ============================================================================

/// Parsed HTTP client hints.
///
/// Supports the following headers defined in the HTTP Client Hints specification:
/// - `DPR` — device pixel ratio (e.g., `2.0`)
/// - `Viewport-Width` — CSS viewport width in pixels (e.g., `1920`)
/// - `Width` — intended display width of the resource in CSS pixels (e.g., `800`)
///
/// When present, these override or supplement the URL-based transform parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientHints {
    /// Device pixel ratio from the `DPR` header.
    pub dpr: Option<f64>,
    /// Viewport width from the `Viewport-Width` header.
    pub viewport_width: Option<u32>,
    /// Intended display width from the `Width` header.
    pub width: Option<u32>,
}

impl Default for ClientHints {
    fn default() -> Self {
        Self {
            dpr: None,
            viewport_width: None,
            width: None,
        }
    }
}

impl ClientHints {
    /// Parse client hints from HTTP header values.
    ///
    /// Each parameter is `Option<&str>` representing the raw header value.
    /// Invalid or negative values are silently ignored (treated as absent).
    ///
    /// # Example
    ///
    /// ```
    /// # use oximedia_image_transform::negotiation::ClientHints;
    /// let hints = ClientHints::from_headers(Some("2.0"), Some("1920"), Some("800"));
    /// assert_eq!(hints.dpr, Some(2.0));
    /// assert_eq!(hints.viewport_width, Some(1920));
    /// assert_eq!(hints.width, Some(800));
    /// ```
    pub fn from_headers(
        dpr_header: Option<&str>,
        viewport_width_header: Option<&str>,
        width_header: Option<&str>,
    ) -> Self {
        let dpr = dpr_header.and_then(|v| {
            let parsed = v.trim().parse::<f64>().ok()?;
            if parsed > 0.0 && parsed <= 10.0 {
                Some(parsed)
            } else {
                None
            }
        });

        let viewport_width = viewport_width_header.and_then(|v| {
            let parsed = v.trim().parse::<u32>().ok()?;
            if parsed > 0 {
                Some(parsed)
            } else {
                None
            }
        });

        let width = width_header.and_then(|v| {
            let parsed = v.trim().parse::<u32>().ok()?;
            if parsed > 0 {
                Some(parsed)
            } else {
                None
            }
        });

        Self {
            dpr,
            viewport_width,
            width,
        }
    }

    /// Apply client hints to transform parameters.
    ///
    /// Rules:
    /// 1. If `Width` header is present and no explicit width is set in params,
    ///    use the `Width` hint as the output width.
    /// 2. If `DPR` header is present and no explicit DPR is set (i.e., DPR == 1.0),
    ///    use the client's DPR.
    /// 3. `Viewport-Width` is stored for logging/analytics but does not
    ///    directly alter the output dimensions (it informs responsive decisions).
    pub fn apply_to_params(&self, params: &mut crate::transform::TransformParams) {
        // Width hint: only override if no explicit width was requested
        if let Some(hint_w) = self.width {
            if params.width.is_none() {
                params.width = Some(hint_w.min(crate::transform::MAX_DIMENSION));
            }
        }

        // DPR hint: only override if params DPR is default (1.0)
        if let Some(hint_dpr) = self.dpr {
            if (params.dpr - 1.0).abs() < f64::EPSILON {
                params.dpr = hint_dpr.clamp(crate::transform::MIN_DPR, crate::transform::MAX_DPR);
            }
        }
    }

    /// Generate response headers for client hints.
    ///
    /// Returns `Content-DPR` and `Vary` headers that should be included
    /// in the HTTP response when client hints were used.
    pub fn response_headers(&self) -> Vec<(&'static str, String)> {
        let mut headers = Vec::new();

        if let Some(dpr) = self.dpr {
            headers.push(("Content-DPR", format!("{dpr:.1}")));
        }

        // Vary header should include hint headers that affect output
        let mut vary_parts = vec!["Accept"];
        if self.dpr.is_some() {
            vary_parts.push("DPR");
        }
        if self.viewport_width.is_some() {
            vary_parts.push("Viewport-Width");
        }
        if self.width.is_some() {
            vary_parts.push("Width");
        }
        headers.push(("Vary", vary_parts.join(", ")));

        // Accept-CH to advertise supported hints
        headers.push(("Accept-CH", "DPR, Viewport-Width, Width".to_string()));

        headers
    }

    /// Returns `true` if any client hint was provided.
    pub fn has_hints(&self) -> bool {
        self.dpr.is_some() || self.viewport_width.is_some() || self.width.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Accept header parsing ──

    #[test]
    fn test_parse_single_entry() {
        let entries = parse_accept_header("image/webp");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].media_type, "image/webp");
        assert_eq!(entries[0].quality, 1.0);
    }

    #[test]
    fn test_parse_multiple_entries_with_quality() {
        let entries = parse_accept_header("image/avif,image/webp;q=0.9,image/jpeg;q=0.8,*/*;q=0.1");
        assert_eq!(entries.len(), 4);
        // Should be sorted by quality descending
        assert_eq!(entries[0].media_type, "image/avif");
        assert_eq!(entries[0].quality, 1.0);
        assert_eq!(entries[1].media_type, "image/webp");
        assert!((entries[1].quality - 0.9).abs() < 0.001);
        assert_eq!(entries[2].media_type, "image/jpeg");
        assert!((entries[2].quality - 0.8).abs() < 0.001);
        assert_eq!(entries[3].media_type, "*/*");
        assert!((entries[3].quality - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_parse_empty_accept() {
        let entries = parse_accept_header("");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_whitespace_only() {
        let entries = parse_accept_header("   ,  ,  ");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_quality_clamped_to_one() {
        let entries = parse_accept_header("image/avif;q=5.0");
        assert_eq!(entries[0].quality, 1.0);
    }

    #[test]
    fn test_parse_quality_clamped_to_zero() {
        let entries = parse_accept_header("image/avif;q=-1.0");
        assert_eq!(entries[0].quality, 0.0);
    }

    #[test]
    fn test_parse_no_quality_defaults_to_one() {
        let entries = parse_accept_header("image/png");
        assert_eq!(entries[0].quality, 1.0);
    }

    #[test]
    fn test_parse_quality_with_extra_params() {
        // Some clients send additional parameters alongside q
        let entries = parse_accept_header("image/webp;q=0.8;level=1");
        assert_eq!(entries[0].media_type, "image/webp");
        assert!((entries[0].quality - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_parse_chrome_accept_header() {
        let entries =
            parse_accept_header("image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8");
        assert_eq!(entries.len(), 6);
        // All non-wildcard entries have q=1.0, */* has q=0.8
        // After sorting: 5 entries with q=1.0 first, then */* with 0.8
        assert!((entries[5].quality - 0.8).abs() < 0.001);
        assert_eq!(entries[5].media_type, "*/*");
    }

    #[test]
    fn test_parse_safari_accept_header() {
        let entries =
            parse_accept_header("image/webp,image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5");
        // No AVIF in Safari's typical header
        let has_avif = entries.iter().any(|e| e.media_type == "image/avif");
        assert!(!has_avif);
    }

    #[test]
    fn test_parse_sorts_by_quality_descending() {
        let entries = parse_accept_header("image/jpeg;q=0.5,image/avif;q=1.0,image/webp;q=0.8");
        assert_eq!(entries[0].media_type, "image/avif");
        assert_eq!(entries[1].media_type, "image/webp");
        assert_eq!(entries[2].media_type, "image/jpeg");
    }

    #[test]
    fn test_parse_equal_quality_preserves_order() {
        let entries = parse_accept_header("image/webp,image/avif");
        // Both have q=1.0, stable sort preserves original order
        assert_eq!(entries[0].media_type, "image/webp");
        assert_eq!(entries[1].media_type, "image/avif");
    }

    // ── Format negotiation ──

    #[test]
    fn test_negotiate_explicit_format_returns_directly() {
        // When format != Auto, accept header is ignored
        let result = negotiate_format("image/avif,image/webp", OutputFormat::Png);
        assert_eq!(result, OutputFormat::Png);
    }

    #[test]
    fn test_negotiate_explicit_jpeg() {
        let result = negotiate_format("image/avif", OutputFormat::Jpeg);
        assert_eq!(result, OutputFormat::Jpeg);
    }

    #[test]
    fn test_negotiate_auto_prefers_avif() {
        let result = negotiate_format(
            "image/avif,image/webp;q=0.9,image/jpeg;q=0.8",
            OutputFormat::Auto,
        );
        assert_eq!(result, OutputFormat::Avif);
    }

    #[test]
    fn test_negotiate_auto_webp_when_no_avif() {
        let result = negotiate_format("image/webp,image/jpeg", OutputFormat::Auto);
        assert_eq!(result, OutputFormat::WebP);
    }

    #[test]
    fn test_negotiate_auto_png_when_no_avif_no_webp() {
        let result = negotiate_format("image/png,image/jpeg", OutputFormat::Auto);
        assert_eq!(result, OutputFormat::Png);
    }

    #[test]
    fn test_negotiate_auto_jpeg_fallback() {
        let result = negotiate_format("image/jpeg", OutputFormat::Auto);
        assert_eq!(result, OutputFormat::Jpeg);
    }

    #[test]
    fn test_negotiate_auto_empty_accept_falls_to_jpeg() {
        let result = negotiate_format("", OutputFormat::Auto);
        assert_eq!(result, OutputFormat::Jpeg);
    }

    #[test]
    fn test_negotiate_auto_wildcard_matches_avif() {
        // */* should match image/avif
        let result = negotiate_format("*/*", OutputFormat::Auto);
        assert_eq!(result, OutputFormat::Avif);
    }

    #[test]
    fn test_negotiate_auto_image_wildcard_matches_avif() {
        let result = negotiate_format("image/*", OutputFormat::Auto);
        assert_eq!(result, OutputFormat::Avif);
    }

    #[test]
    fn test_negotiate_auto_no_image_types() {
        let result = negotiate_format("text/html,application/json", OutputFormat::Auto);
        assert_eq!(result, OutputFormat::Jpeg);
    }

    #[test]
    fn test_negotiate_auto_zero_quality_avif_skipped() {
        // AVIF with q=0 should be treated as not accepted
        let result = negotiate_format("image/avif;q=0,image/webp", OutputFormat::Auto);
        assert_eq!(result, OutputFormat::WebP);
    }

    // ── MIME type mapping ──

    #[test]
    fn test_format_to_mime_avif() {
        assert_eq!(format_to_mime(OutputFormat::Avif), "image/avif");
    }

    #[test]
    fn test_format_to_mime_webp() {
        assert_eq!(format_to_mime(OutputFormat::WebP), "image/webp");
    }

    #[test]
    fn test_format_to_mime_jpeg() {
        assert_eq!(format_to_mime(OutputFormat::Jpeg), "image/jpeg");
    }

    #[test]
    fn test_format_to_mime_png() {
        assert_eq!(format_to_mime(OutputFormat::Png), "image/png");
    }

    #[test]
    fn test_format_to_mime_gif() {
        assert_eq!(format_to_mime(OutputFormat::Gif), "image/gif");
    }

    #[test]
    fn test_format_to_mime_baseline() {
        assert_eq!(format_to_mime(OutputFormat::Baseline), "image/jpeg");
    }

    #[test]
    fn test_format_to_mime_auto_fallback() {
        assert_eq!(format_to_mime(OutputFormat::Auto), "image/jpeg");
    }

    // ── Response headers ──

    #[test]
    fn test_response_headers_content_type() {
        let headers = ResponseHeaders::new(OutputFormat::WebP, "key123", 10000, 5000);
        assert_eq!(headers.content_type, "image/webp");
    }

    #[test]
    fn test_response_headers_cache_control() {
        let headers = ResponseHeaders::new(OutputFormat::Avif, "key", 1000, 800);
        assert_eq!(headers.cache_control, "public, max-age=31536000, immutable");
    }

    #[test]
    fn test_response_headers_etag() {
        let headers = ResponseHeaders::new(OutputFormat::Jpeg, "abc_def_123", 500, 400);
        assert_eq!(headers.etag, "\"abc_def_123\"");
    }

    #[test]
    fn test_response_headers_vary() {
        let headers = ResponseHeaders::new(OutputFormat::Png, "k", 100, 100);
        assert_eq!(headers.vary, "Accept");
    }

    #[test]
    fn test_response_headers_cf_resized_with_savings() {
        let headers = ResponseHeaders::new(OutputFormat::WebP, "k", 10000, 5000);
        assert!(headers.cf_resized.contains("internal=ok"));
        assert!(headers.cf_resized.contains("orig=10000"));
        assert!(headers.cf_resized.contains("out=5000"));
        assert!(headers.cf_resized.contains("saved=50.0%"));
    }

    #[test]
    fn test_response_headers_cf_resized_no_savings() {
        let headers = ResponseHeaders::new(OutputFormat::Png, "k", 1000, 1200);
        // No savings when output is larger than original
        assert!(!headers.cf_resized.contains("saved="));
    }

    #[test]
    fn test_response_headers_cf_resized_zero_original() {
        let headers = ResponseHeaders::new(OutputFormat::Jpeg, "k", 0, 500);
        assert!(!headers.cf_resized.contains("saved="));
    }

    #[test]
    fn test_response_headers_to_pairs() {
        let headers = ResponseHeaders::new(OutputFormat::Avif, "test_key", 2000, 1000);
        let pairs = headers.to_pairs();
        assert_eq!(pairs.len(), 5);
        assert_eq!(pairs[0].0, "Content-Type");
        assert_eq!(pairs[0].1, "image/avif");
        assert_eq!(pairs[1].0, "Cache-Control");
        assert_eq!(pairs[2].0, "ETag");
        assert_eq!(pairs[3].0, "Vary");
        assert_eq!(pairs[4].0, "Cf-Resized");
    }

    #[test]
    fn test_response_headers_different_formats() {
        let avif_h = ResponseHeaders::new(OutputFormat::Avif, "k", 100, 80);
        let webp_h = ResponseHeaders::new(OutputFormat::WebP, "k", 100, 80);
        let jpeg_h = ResponseHeaders::new(OutputFormat::Jpeg, "k", 100, 80);
        assert_eq!(avif_h.content_type, "image/avif");
        assert_eq!(webp_h.content_type, "image/webp");
        assert_eq!(jpeg_h.content_type, "image/jpeg");
    }

    // ── supports_format ──

    #[test]
    fn test_supports_format_avif() {
        assert!(supports_format("image/avif,image/webp", OutputFormat::Avif));
    }

    #[test]
    fn test_supports_format_webp() {
        assert!(supports_format("image/webp,image/jpeg", OutputFormat::WebP));
    }

    #[test]
    fn test_supports_format_not_supported() {
        assert!(!supports_format("image/jpeg", OutputFormat::Avif));
    }

    #[test]
    fn test_supports_format_via_wildcard() {
        assert!(supports_format("image/*", OutputFormat::Avif));
        assert!(supports_format("*/*", OutputFormat::WebP));
    }

    #[test]
    fn test_supports_format_zero_quality_not_supported() {
        assert!(!supports_format("image/avif;q=0", OutputFormat::Avif));
    }

    #[test]
    fn test_supports_format_empty_accept() {
        assert!(!supports_format("", OutputFormat::Avif));
        assert!(!supports_format("", OutputFormat::WebP));
        assert!(!supports_format("", OutputFormat::Jpeg));
    }

    #[test]
    fn test_supports_format_non_image_wildcard_no_match() {
        // text/* should NOT match image/avif
        assert!(!supports_format("text/*", OutputFormat::Avif));
    }

    // ── Edge cases ──

    #[test]
    fn test_accepts_mime_exact_zero_quality() {
        let entries = vec![AcceptEntry {
            media_type: "image/avif".to_string(),
            quality: 0.0,
        }];
        assert!(!accepts_mime(&entries, "image/avif"));
    }

    #[test]
    fn test_accepts_mime_wildcard_zero_quality() {
        let entries = vec![AcceptEntry {
            media_type: "*/*".to_string(),
            quality: 0.0,
        }];
        assert!(!accepts_mime(&entries, "image/avif"));
    }

    #[test]
    fn test_negotiate_all_formats_explicit() {
        // Ensure every explicit format is returned as-is
        for fmt in &[
            OutputFormat::Avif,
            OutputFormat::WebP,
            OutputFormat::Jpeg,
            OutputFormat::Png,
            OutputFormat::Gif,
            OutputFormat::Baseline,
        ] {
            let result = negotiate_format("", *fmt);
            assert_eq!(result, *fmt);
        }
    }

    #[test]
    fn test_parse_malformed_quality() {
        // Invalid q value should default to 1.0
        let entries = parse_accept_header("image/webp;q=notanumber");
        assert_eq!(entries[0].quality, 1.0);
    }

    #[test]
    fn test_parse_missing_q_value() {
        // Parameter without q= should default to 1.0
        let entries = parse_accept_header("image/webp;level=1");
        assert_eq!(entries[0].quality, 1.0);
    }

    // ── Client hints ──

    #[test]
    fn test_client_hints_parse_all() {
        let hints = ClientHints::from_headers(Some("2.0"), Some("1920"), Some("800"));
        assert_eq!(hints.dpr, Some(2.0));
        assert_eq!(hints.viewport_width, Some(1920));
        assert_eq!(hints.width, Some(800));
    }

    #[test]
    fn test_client_hints_parse_none() {
        let hints = ClientHints::from_headers(None, None, None);
        assert_eq!(hints.dpr, None);
        assert_eq!(hints.viewport_width, None);
        assert_eq!(hints.width, None);
    }

    #[test]
    fn test_client_hints_parse_partial() {
        let hints = ClientHints::from_headers(Some("3.0"), None, Some("400"));
        assert_eq!(hints.dpr, Some(3.0));
        assert_eq!(hints.viewport_width, None);
        assert_eq!(hints.width, Some(400));
    }

    #[test]
    fn test_client_hints_invalid_dpr() {
        // Negative DPR
        let hints = ClientHints::from_headers(Some("-1.0"), None, None);
        assert_eq!(hints.dpr, None);

        // Zero DPR
        let hints = ClientHints::from_headers(Some("0"), None, None);
        assert_eq!(hints.dpr, None);

        // Too large DPR
        let hints = ClientHints::from_headers(Some("100"), None, None);
        assert_eq!(hints.dpr, None);

        // Non-numeric
        let hints = ClientHints::from_headers(Some("abc"), None, None);
        assert_eq!(hints.dpr, None);
    }

    #[test]
    fn test_client_hints_invalid_width() {
        let hints = ClientHints::from_headers(None, None, Some("0"));
        assert_eq!(hints.width, None);

        let hints = ClientHints::from_headers(None, None, Some("abc"));
        assert_eq!(hints.width, None);
    }

    #[test]
    fn test_client_hints_invalid_viewport() {
        let hints = ClientHints::from_headers(None, Some("0"), None);
        assert_eq!(hints.viewport_width, None);
    }

    #[test]
    fn test_client_hints_whitespace_trimming() {
        let hints = ClientHints::from_headers(Some("  2.5  "), Some("  1024  "), Some("  640  "));
        assert_eq!(hints.dpr, Some(2.5));
        assert_eq!(hints.viewport_width, Some(1024));
        assert_eq!(hints.width, Some(640));
    }

    #[test]
    fn test_client_hints_apply_to_params_width() {
        use crate::transform::TransformParams;

        let hints = ClientHints::from_headers(None, None, Some("600"));
        let mut params = TransformParams::default();
        hints.apply_to_params(&mut params);
        assert_eq!(params.width, Some(600));
    }

    #[test]
    fn test_client_hints_apply_does_not_override_explicit_width() {
        use crate::transform::TransformParams;

        let hints = ClientHints::from_headers(None, None, Some("600"));
        let mut params = TransformParams::default();
        params.width = Some(800);
        hints.apply_to_params(&mut params);
        assert_eq!(params.width, Some(800));
    }

    #[test]
    fn test_client_hints_apply_dpr() {
        use crate::transform::TransformParams;

        let hints = ClientHints::from_headers(Some("2.0"), None, None);
        let mut params = TransformParams::default();
        hints.apply_to_params(&mut params);
        assert!((params.dpr - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_client_hints_apply_does_not_override_explicit_dpr() {
        use crate::transform::TransformParams;

        let hints = ClientHints::from_headers(Some("3.0"), None, None);
        let mut params = TransformParams::default();
        params.dpr = 2.0;
        hints.apply_to_params(&mut params);
        assert!((params.dpr - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_client_hints_apply_dpr_clamped() {
        use crate::transform::TransformParams;

        let hints = ClientHints::from_headers(Some("8.0"), None, None);
        let mut params = TransformParams::default();
        hints.apply_to_params(&mut params);
        assert!((params.dpr - 4.0).abs() < f64::EPSILON); // clamped to MAX_DPR
    }

    #[test]
    fn test_client_hints_has_hints() {
        assert!(!ClientHints::default().has_hints());
        assert!(ClientHints::from_headers(Some("2.0"), None, None).has_hints());
        assert!(ClientHints::from_headers(None, Some("1024"), None).has_hints());
        assert!(ClientHints::from_headers(None, None, Some("800")).has_hints());
    }

    #[test]
    fn test_client_hints_response_headers_basic() {
        let hints = ClientHints::from_headers(Some("2.0"), Some("1920"), Some("800"));
        let headers = hints.response_headers();
        // Should include Content-DPR
        assert!(headers
            .iter()
            .any(|(k, v)| *k == "Content-DPR" && v == "2.0"));
        // Vary should include all hint headers
        let vary = headers
            .iter()
            .find(|(k, _)| *k == "Vary")
            .map(|(_, v)| v.as_str());
        assert!(vary.is_some());
        let vary_str = vary.expect("vary");
        assert!(vary_str.contains("DPR"));
        assert!(vary_str.contains("Width"));
        assert!(vary_str.contains("Viewport-Width"));
        // Accept-CH advertises supported hints
        assert!(headers.iter().any(|(k, _)| *k == "Accept-CH"));
    }

    #[test]
    fn test_client_hints_response_headers_no_hints() {
        let hints = ClientHints::default();
        let headers = hints.response_headers();
        // No Content-DPR when no DPR hint
        assert!(!headers.iter().any(|(k, _)| *k == "Content-DPR"));
        // Vary should only include Accept
        let vary = headers
            .iter()
            .find(|(k, _)| *k == "Vary")
            .map(|(_, v)| v.clone());
        assert_eq!(vary, Some("Accept".to_string()));
    }

    #[test]
    fn test_client_hints_default() {
        let hints = ClientHints::default();
        assert_eq!(hints.dpr, None);
        assert_eq!(hints.viewport_width, None);
        assert_eq!(hints.width, None);
    }

    // ── generate_cache_control tests ──

    use crate::transform::TransformParams;

    /// AVIF format must produce the long immutable cache directive.
    #[test]
    fn test_cache_control_avif() {
        let mut params = TransformParams::default();
        params.format = OutputFormat::Avif;
        let cc = generate_cache_control(&params);
        assert_eq!(cc, "public, max-age=604800, immutable");
    }

    /// WebP format must also produce the long immutable cache directive.
    #[test]
    fn test_cache_control_webp() {
        let mut params = TransformParams::default();
        params.format = OutputFormat::WebP;
        let cc = generate_cache_control(&params);
        assert_eq!(cc, "public, max-age=604800, immutable");
    }

    /// Low quality (< 50) should produce the 1-day cache regardless of format.
    #[test]
    fn test_cache_control_low_quality() {
        let mut params = TransformParams::default();
        params.quality = 40;
        let cc = generate_cache_control(&params);
        assert_eq!(cc, "public, max-age=86400");
    }

    /// Low quality takes priority over AVIF/WebP format check.
    #[test]
    fn test_cache_control_low_quality_avif_priority() {
        let mut params = TransformParams::default();
        params.format = OutputFormat::Avif;
        params.quality = 30;
        let cc = generate_cache_control(&params);
        // Quality < 50 wins the priority check.
        assert_eq!(cc, "public, max-age=86400");
    }

    /// Default params (JPEG-like, quality 85) should produce the short 1-hour cache.
    #[test]
    fn test_cache_control_default() {
        let params = TransformParams::default();
        let cc = generate_cache_control(&params);
        assert_eq!(cc, "public, max-age=3600");
    }

    #[test]
    fn test_cache_control_jpeg_normal_quality() {
        let mut params = TransformParams::default();
        params.format = OutputFormat::Jpeg;
        params.quality = 85;
        let cc = generate_cache_control(&params);
        assert_eq!(cc, "public, max-age=3600");
    }

    #[test]
    fn test_cache_control_png() {
        let mut params = TransformParams::default();
        params.format = OutputFormat::Png;
        let cc = generate_cache_control(&params);
        assert_eq!(cc, "public, max-age=3600");
    }

    /// Boundary: quality == 50 is NOT low quality (the condition is strictly < 50).
    #[test]
    fn test_cache_control_boundary_quality_50() {
        let mut params = TransformParams::default();
        params.quality = 50;
        params.format = OutputFormat::Jpeg;
        let cc = generate_cache_control(&params);
        assert_eq!(cc, "public, max-age=3600");
    }

    /// Boundary: quality == 49 IS low quality.
    #[test]
    fn test_cache_control_boundary_quality_49() {
        let mut params = TransformParams::default();
        params.quality = 49;
        let cc = generate_cache_control(&params);
        assert_eq!(cc, "public, max-age=86400");
    }
}
