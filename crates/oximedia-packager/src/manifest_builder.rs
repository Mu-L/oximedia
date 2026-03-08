#![allow(dead_code)]
//! Manifest building for HLS and DASH adaptive streams.
//!
//! Provides a builder that accumulates track descriptors and can emit
//! HLS master playlist or DASH MPD text.

use std::fmt::Write as FmtWrite;

/// The manifest/playlist format to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManifestFormat {
    /// HTTP Live Streaming master playlist (M3U8).
    Hls,
    /// MPEG-DASH Media Presentation Description (MPD XML).
    Dash,
    /// Microsoft Smooth Streaming manifest (XML).
    Smooth,
}

impl ManifestFormat {
    /// Returns the MIME type for the manifest document itself.
    #[must_use]
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Hls => "application/vnd.apple.mpegurl",
            Self::Dash => "application/dash+xml",
            Self::Smooth => "application/vnd.ms-sstr+xml",
        }
    }

    /// Returns the conventional file extension (without leading dot).
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::Hls => "m3u8",
            Self::Dash => "mpd",
            Self::Smooth => "ism",
        }
    }
}

impl std::fmt::Display for ManifestFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hls => write!(f, "HLS"),
            Self::Dash => write!(f, "DASH"),
            Self::Smooth => write!(f, "Smooth"),
        }
    }
}

/// Describes a single rendition (video or audio track) to include in the manifest.
#[derive(Debug, Clone)]
pub struct ManifestTrack {
    /// URI of the media playlist or segment.
    uri: String,
    /// Bandwidth in bits per second.
    bandwidth_bps: u64,
    /// Video width in pixels (0 for audio-only).
    width: u32,
    /// Video height in pixels (0 for audio-only).
    height: u32,
    /// Codec string (e.g. "av01.0.05M.08").
    codecs: String,
    /// Frame rate (0.0 for audio-only).
    frame_rate: f64,
    /// Whether this track carries audio.
    is_audio: bool,
}

impl ManifestTrack {
    /// Create a new video track descriptor.
    pub fn video(
        uri: impl Into<String>,
        bandwidth_bps: u64,
        width: u32,
        height: u32,
        codecs: impl Into<String>,
        frame_rate: f64,
    ) -> Self {
        Self {
            uri: uri.into(),
            bandwidth_bps,
            width,
            height,
            codecs: codecs.into(),
            frame_rate,
            is_audio: false,
        }
    }

    /// Create a new audio-only track descriptor.
    pub fn audio(uri: impl Into<String>, bandwidth_bps: u64, codecs: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            bandwidth_bps,
            width: 0,
            height: 0,
            codecs: codecs.into(),
            frame_rate: 0.0,
            is_audio: true,
        }
    }

    /// Returns bandwidth in kbps.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn bandwidth_kbps(&self) -> f64 {
        self.bandwidth_bps as f64 / 1000.0
    }

    /// Returns the track URI.
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the codec string.
    #[must_use]
    pub fn codecs(&self) -> &str {
        &self.codecs
    }

    /// Returns `true` if this is an audio-only track.
    #[must_use]
    pub fn is_audio(&self) -> bool {
        self.is_audio
    }
}

/// Builds streaming manifests from a collection of tracks.
pub struct ManifestBuilder {
    tracks: Vec<ManifestTrack>,
    target_duration_s: u32,
    media_sequence: u64,
    base_url: String,
}

impl ManifestBuilder {
    /// Create a new manifest builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            target_duration_s: 6,
            media_sequence: 0,
            base_url: String::new(),
        }
    }

    /// Set the HLS target segment duration in seconds.
    #[must_use]
    pub fn with_target_duration(mut self, seconds: u32) -> Self {
        self.target_duration_s = seconds;
        self
    }

    /// Set the media sequence number for live playlists.
    #[must_use]
    pub fn with_media_sequence(mut self, seq: u64) -> Self {
        self.media_sequence = seq;
        self
    }

    /// Set a base URL to prepend to relative URIs.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Add a track to the manifest.
    pub fn add_track(&mut self, track: ManifestTrack) {
        self.tracks.push(track);
    }

    /// Returns the number of tracks registered.
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    /// Generate an HLS master playlist (M3U8) string.
    #[must_use]
    pub fn build_hls(&self) -> String {
        let mut out = String::from("#EXTM3U\n#EXT-X-VERSION:6\n");
        for track in &self.tracks {
            if track.is_audio {
                let _ = write!(
                    out,
                    "#EXT-X-STREAM-INF:BANDWIDTH={},CODECS=\"{}\"\n{}{}\n",
                    track.bandwidth_bps, track.codecs, self.base_url, track.uri
                );
            } else {
                let _ = write!(
                    out,
                    "#EXT-X-STREAM-INF:BANDWIDTH={},RESOLUTION={}x{},CODECS=\"{}\",FRAME-RATE={:.3}\n{}{}\n",
                    track.bandwidth_bps,
                    track.width,
                    track.height,
                    track.codecs,
                    track.frame_rate,
                    self.base_url,
                    track.uri
                );
            }
        }
        out
    }

    /// Generate a minimal DASH MPD XML string.
    #[must_use]
    pub fn build_dash(&self) -> String {
        let mut out = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <MPD xmlns=\"urn:mpeg:dash:schema:mpd:2011\" profiles=\"urn:mpeg:dash:profile:isoff-live:2011\" type=\"static\">\n\
             <Period>\n",
        );

        for (i, track) in self.tracks.iter().enumerate() {
            if track.is_audio {
                let _ = write!(
                    out,
                    "  <AdaptationSet id=\"{}\" mimeType=\"audio/mp4\" codecs=\"{}\">\n\
                         <Representation id=\"r{}\" bandwidth=\"{}\">\n\
                           <BaseURL>{}{}</BaseURL>\n\
                         </Representation>\n\
                   </AdaptationSet>\n",
                    i, track.codecs, i, track.bandwidth_bps, self.base_url, track.uri
                );
            } else {
                let _ = write!(
                    out,
                    "  <AdaptationSet id=\"{}\" mimeType=\"video/mp4\" codecs=\"{}\">\n\
                         <Representation id=\"r{}\" bandwidth=\"{}\" width=\"{}\" height=\"{}\">\n\
                           <BaseURL>{}{}</BaseURL>\n\
                         </Representation>\n\
                   </AdaptationSet>\n",
                    i,
                    track.codecs,
                    i,
                    track.bandwidth_bps,
                    track.width,
                    track.height,
                    self.base_url,
                    track.uri
                );
            }
        }

        out.push_str("</Period>\n</MPD>");
        out
    }
}

impl Default for ManifestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_format_mime_hls() {
        assert_eq!(
            ManifestFormat::Hls.mime_type(),
            "application/vnd.apple.mpegurl"
        );
    }

    #[test]
    fn test_manifest_format_mime_dash() {
        assert_eq!(ManifestFormat::Dash.mime_type(), "application/dash+xml");
    }

    #[test]
    fn test_manifest_format_extension() {
        assert_eq!(ManifestFormat::Hls.extension(), "m3u8");
        assert_eq!(ManifestFormat::Dash.extension(), "mpd");
        assert_eq!(ManifestFormat::Smooth.extension(), "ism");
    }

    #[test]
    fn test_manifest_format_display() {
        assert_eq!(ManifestFormat::Hls.to_string(), "HLS");
        assert_eq!(ManifestFormat::Dash.to_string(), "DASH");
    }

    #[test]
    fn test_track_bandwidth_kbps() {
        let track = ManifestTrack::video("v1.m3u8", 3_000_000, 1280, 720, "av01.0.05M.08", 30.0);
        let kbps = track.bandwidth_kbps();
        assert!((kbps - 3000.0).abs() < 0.1);
    }

    #[test]
    fn test_track_audio_is_audio() {
        let track = ManifestTrack::audio("audio.m3u8", 128_000, "opus");
        assert!(track.is_audio());
    }

    #[test]
    fn test_track_video_not_audio() {
        let track = ManifestTrack::video("v.m3u8", 2_000_000, 1280, 720, "av01", 25.0);
        assert!(!track.is_audio());
    }

    #[test]
    fn test_track_uri_and_codecs() {
        let track = ManifestTrack::video("stream.m3u8", 1_000_000, 854, 480, "vp09", 24.0);
        assert_eq!(track.uri(), "stream.m3u8");
        assert_eq!(track.codecs(), "vp09");
    }

    #[test]
    fn test_builder_track_count() {
        let mut builder = ManifestBuilder::new();
        builder.add_track(ManifestTrack::video(
            "v.m3u8", 1_000_000, 1280, 720, "av01", 30.0,
        ));
        builder.add_track(ManifestTrack::audio("a.m3u8", 128_000, "opus"));
        assert_eq!(builder.track_count(), 2);
    }

    #[test]
    fn test_build_hls_contains_extm3u() {
        let mut builder = ManifestBuilder::new();
        builder.add_track(ManifestTrack::video(
            "v.m3u8", 2_000_000, 1280, 720, "av01", 30.0,
        ));
        let hls = builder.build_hls();
        assert!(hls.starts_with("#EXTM3U"));
    }

    #[test]
    fn test_build_hls_contains_bandwidth() {
        let mut builder = ManifestBuilder::new();
        builder.add_track(ManifestTrack::video(
            "v.m3u8", 2_000_000, 1280, 720, "av01", 30.0,
        ));
        let hls = builder.build_hls();
        assert!(hls.contains("2000000"));
    }

    #[test]
    fn test_build_dash_contains_mpd() {
        let mut builder = ManifestBuilder::new();
        builder.add_track(ManifestTrack::video(
            "v.mp4", 3_000_000, 1920, 1080, "av01", 25.0,
        ));
        let dash = builder.build_dash();
        assert!(dash.contains("<MPD"));
        assert!(dash.contains("</MPD>"));
    }

    #[test]
    fn test_build_dash_contains_adaptation_set() {
        let mut builder = ManifestBuilder::new();
        builder.add_track(ManifestTrack::audio("a.mp4", 128_000, "opus"));
        let dash = builder.build_dash();
        assert!(dash.contains("AdaptationSet"));
    }

    #[test]
    fn test_builder_with_base_url() {
        let mut builder = ManifestBuilder::new().with_base_url("https://cdn.example.com/");
        builder.add_track(ManifestTrack::video(
            "720p.m3u8",
            2_000_000,
            1280,
            720,
            "av01",
            30.0,
        ));
        let hls = builder.build_hls();
        assert!(hls.contains("https://cdn.example.com/720p.m3u8"));
    }
}
