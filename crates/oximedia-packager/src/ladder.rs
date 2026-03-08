//! Bitrate ladder generation for adaptive streaming.

use crate::config::{BitrateEntry, BitrateLadder};
use crate::error::{PackagerError, PackagerResult};
use tracing::{debug, info};

/// Video quality preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityPreset {
    /// Low quality (mobile).
    Low,
    /// Medium quality (SD).
    Medium,
    /// High quality (HD).
    High,
    /// Very high quality (Full HD).
    VeryHigh,
    /// Ultra quality (4K).
    Ultra,
}

/// Source video information.
#[derive(Debug, Clone)]
pub struct SourceInfo {
    /// Source width.
    pub width: u32,
    /// Source height.
    pub height: u32,
    /// Source bitrate (if known).
    pub bitrate: Option<u32>,
    /// Source frame rate.
    pub framerate: f64,
    /// Source codec.
    pub codec: String,
}

impl SourceInfo {
    /// Create new source info.
    #[must_use]
    pub fn new(width: u32, height: u32, framerate: f64, codec: String) -> Self {
        Self {
            width,
            height,
            bitrate: None,
            framerate,
            codec,
        }
    }

    /// Set the source bitrate.
    #[must_use]
    pub fn with_bitrate(mut self, bitrate: u32) -> Self {
        self.bitrate = Some(bitrate);
        self
    }

    /// Get the aspect ratio.
    #[must_use]
    pub fn aspect_ratio(&self) -> f64 {
        f64::from(self.width) / f64::from(self.height)
    }

    /// Check if source is 4K or higher.
    #[must_use]
    pub fn is_4k_or_higher(&self) -> bool {
        self.width >= 3840 || self.height >= 2160
    }

    /// Check if source is 1080p or higher.
    #[must_use]
    pub fn is_1080p_or_higher(&self) -> bool {
        self.width >= 1920 || self.height >= 1080
    }

    /// Check if source is 720p or higher.
    #[must_use]
    pub fn is_720p_or_higher(&self) -> bool {
        self.width >= 1280 || self.height >= 720
    }
}

/// Bitrate ladder generator.
pub struct LadderGenerator {
    source: SourceInfo,
    codec: String,
    min_bitrate: u32,
    max_bitrate: Option<u32>,
}

impl LadderGenerator {
    /// Create a new ladder generator.
    #[must_use]
    pub fn new(source: SourceInfo) -> Self {
        Self {
            source,
            codec: "av1".to_string(),
            min_bitrate: 250_000, // 250 kbps
            max_bitrate: None,
        }
    }

    /// Set the target codec.
    #[must_use]
    pub fn with_codec(mut self, codec: &str) -> Self {
        self.codec = codec.to_string();
        self
    }

    /// Set the minimum bitrate.
    #[must_use]
    pub fn with_min_bitrate(mut self, bitrate: u32) -> Self {
        self.min_bitrate = bitrate;
        self
    }

    /// Set the maximum bitrate.
    #[must_use]
    pub fn with_max_bitrate(mut self, bitrate: u32) -> Self {
        self.max_bitrate = Some(bitrate);
        self
    }

    /// Generate a bitrate ladder.
    pub fn generate(&self) -> PackagerResult<BitrateLadder> {
        info!(
            "Generating bitrate ladder for {}x{} source",
            self.source.width, self.source.height
        );

        let mut ladder = BitrateLadder::new();
        let entries = self.generate_entries()?;

        for entry in entries {
            debug!(
                "Adding ladder entry: {}x{} @ {} bps",
                entry.width, entry.height, entry.bitrate
            );
            ladder.add_entry(entry);
        }

        ladder.auto_generate = false;
        Ok(ladder)
    }

    /// Generate ladder entries based on source.
    fn generate_entries(&self) -> PackagerResult<Vec<BitrateEntry>> {
        let mut entries = Vec::new();

        // Determine which resolutions to include based on source
        if self.source.is_4k_or_higher() {
            // 4K source: generate 4K, 1080p, 720p, 480p, 360p
            entries.extend(self.create_4k_ladder()?);
        } else if self.source.is_1080p_or_higher() {
            // 1080p source: generate 1080p, 720p, 480p, 360p
            entries.extend(self.create_1080p_ladder()?);
        } else if self.source.is_720p_or_higher() {
            // 720p source: generate 720p, 480p, 360p
            entries.extend(self.create_720p_ladder()?);
        } else {
            // SD source: generate source resolution and lower
            entries.extend(self.create_sd_ladder()?);
        }

        // Filter out entries that exceed max bitrate or are below min bitrate
        entries.retain(|e| {
            e.bitrate >= self.min_bitrate
                && (self.max_bitrate.is_none()
                    || e.bitrate
                        <= self
                            .max_bitrate
                            .expect("invariant: max_bitrate is Some (checked above)"))
        });

        if entries.is_empty() {
            return Err(PackagerError::InvalidLadder(
                "No valid bitrate entries generated".to_string(),
            ));
        }

        Ok(entries)
    }

    /// Create 4K bitrate ladder.
    fn create_4k_ladder(&self) -> PackagerResult<Vec<BitrateEntry>> {
        let ar = self.source.aspect_ratio();
        let mut entries = Vec::new();

        // 4K (3840x2160 or adjusted for aspect ratio)
        let (width_4k, height_4k) = self.adjust_resolution(3840, 2160, ar);
        entries.push(
            BitrateEntry::new(
                self.calculate_bitrate(width_4k, height_4k),
                width_4k,
                height_4k,
                &self.codec,
            )
            .with_framerate(self.source.framerate),
        );

        // 1080p
        let (width_1080, height_1080) = self.adjust_resolution(1920, 1080, ar);
        entries.push(
            BitrateEntry::new(
                self.calculate_bitrate(width_1080, height_1080),
                width_1080,
                height_1080,
                &self.codec,
            )
            .with_framerate(self.source.framerate),
        );

        // 720p
        let (width_720, height_720) = self.adjust_resolution(1280, 720, ar);
        entries.push(
            BitrateEntry::new(
                self.calculate_bitrate(width_720, height_720),
                width_720,
                height_720,
                &self.codec,
            )
            .with_framerate(self.source.framerate),
        );

        // 480p
        entries.push(self.create_480p_entry(ar)?);

        // 360p
        entries.push(self.create_360p_entry(ar)?);

        Ok(entries)
    }

    /// Create 1080p bitrate ladder.
    fn create_1080p_ladder(&self) -> PackagerResult<Vec<BitrateEntry>> {
        let ar = self.source.aspect_ratio();
        let mut entries = Vec::new();

        // 1080p
        let (width_1080, height_1080) = self.adjust_resolution(1920, 1080, ar);
        entries.push(
            BitrateEntry::new(
                self.calculate_bitrate(width_1080, height_1080),
                width_1080,
                height_1080,
                &self.codec,
            )
            .with_framerate(self.source.framerate),
        );

        // 720p
        let (width_720, height_720) = self.adjust_resolution(1280, 720, ar);
        entries.push(
            BitrateEntry::new(
                self.calculate_bitrate(width_720, height_720),
                width_720,
                height_720,
                &self.codec,
            )
            .with_framerate(self.source.framerate),
        );

        // 480p
        entries.push(self.create_480p_entry(ar)?);

        // 360p
        entries.push(self.create_360p_entry(ar)?);

        Ok(entries)
    }

    /// Create 720p bitrate ladder.
    fn create_720p_ladder(&self) -> PackagerResult<Vec<BitrateEntry>> {
        let ar = self.source.aspect_ratio();
        let mut entries = Vec::new();

        // 720p
        let (width_720, height_720) = self.adjust_resolution(1280, 720, ar);
        entries.push(
            BitrateEntry::new(
                self.calculate_bitrate(width_720, height_720),
                width_720,
                height_720,
                &self.codec,
            )
            .with_framerate(self.source.framerate),
        );

        // 480p
        entries.push(self.create_480p_entry(ar)?);

        // 360p
        entries.push(self.create_360p_entry(ar)?);

        Ok(entries)
    }

    /// Create SD bitrate ladder.
    fn create_sd_ladder(&self) -> PackagerResult<Vec<BitrateEntry>> {
        let ar = self.source.aspect_ratio();
        let mut entries = Vec::new();

        // Source resolution
        entries.push(
            BitrateEntry::new(
                self.calculate_bitrate(self.source.width, self.source.height),
                self.source.width,
                self.source.height,
                &self.codec,
            )
            .with_framerate(self.source.framerate),
        );

        // 360p if source is larger
        if self.source.height > 360 {
            entries.push(self.create_360p_entry(ar)?);
        }

        // 240p for mobile
        if self.source.height > 240 {
            let (width_240, height_240) = self.adjust_resolution(426, 240, ar);
            entries.push(
                BitrateEntry::new(
                    self.calculate_bitrate(width_240, height_240),
                    width_240,
                    height_240,
                    &self.codec,
                )
                .with_framerate(self.source.framerate),
            );
        }

        Ok(entries)
    }

    /// Create 480p entry.
    fn create_480p_entry(&self, aspect_ratio: f64) -> PackagerResult<BitrateEntry> {
        let (width, height) = self.adjust_resolution(854, 480, aspect_ratio);
        Ok(BitrateEntry::new(
            self.calculate_bitrate(width, height),
            width,
            height,
            &self.codec,
        )
        .with_framerate(self.source.framerate))
    }

    /// Create 360p entry.
    fn create_360p_entry(&self, aspect_ratio: f64) -> PackagerResult<BitrateEntry> {
        let (width, height) = self.adjust_resolution(640, 360, aspect_ratio);
        Ok(BitrateEntry::new(
            self.calculate_bitrate(width, height),
            width,
            height,
            &self.codec,
        )
        .with_framerate(self.source.framerate))
    }

    /// Adjust resolution to match aspect ratio.
    fn adjust_resolution(&self, width: u32, height: u32, target_ar: f64) -> (u32, u32) {
        let current_ar = f64::from(width) / f64::from(height);

        if (current_ar - target_ar).abs() < 0.01 {
            return (width, height);
        }

        // Adjust width to match target aspect ratio
        let adjusted_width = (f64::from(height) * target_ar).round() as u32;
        // Ensure even dimensions for video encoding
        let adjusted_width = (adjusted_width / 2) * 2;

        (adjusted_width, height)
    }

    /// Calculate bitrate based on resolution and codec.
    fn calculate_bitrate(&self, width: u32, height: u32) -> u32 {
        let pixels = u64::from(width) * u64::from(height);
        let fps = self.source.framerate;

        // Base bitrate calculation (bits per pixel)
        let base_bpp = match self.codec.as_str() {
            "av1" => 0.05, // AV1 is most efficient
            "vp9" => 0.08, // VP9 is efficient
            "vp8" => 0.12, // VP8 is less efficient
            _ => 0.08,     // Default to VP9 efficiency
        };

        // Adjust for frame rate
        let fps_factor = if fps > 30.0 {
            1.0 + ((fps - 30.0) / 30.0) * 0.3
        } else {
            fps / 30.0
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let bitrate = (pixels as f64 * base_bpp * fps_factor) as u32;

        // Clamp to reasonable range
        bitrate.max(250_000).min(50_000_000)
    }
}

/// Pre-defined bitrate ladder presets.
pub struct LadderPresets;

impl LadderPresets {
    /// Get a standard HLS ladder for 1080p content.
    #[must_use]
    pub fn hls_1080p() -> BitrateLadder {
        let mut ladder = BitrateLadder::new();

        ladder.add_entry(BitrateEntry::new(5_000_000, 1920, 1080, "av1"));
        ladder.add_entry(BitrateEntry::new(3_000_000, 1280, 720, "av1"));
        ladder.add_entry(BitrateEntry::new(1_500_000, 854, 480, "av1"));
        ladder.add_entry(BitrateEntry::new(800_000, 640, 360, "av1"));

        ladder.auto_generate = false;
        ladder
    }

    /// Get a standard DASH ladder for 4K content.
    #[must_use]
    pub fn dash_4k() -> BitrateLadder {
        let mut ladder = BitrateLadder::new();

        ladder.add_entry(BitrateEntry::new(15_000_000, 3840, 2160, "av1"));
        ladder.add_entry(BitrateEntry::new(8_000_000, 2560, 1440, "av1"));
        ladder.add_entry(BitrateEntry::new(5_000_000, 1920, 1080, "av1"));
        ladder.add_entry(BitrateEntry::new(3_000_000, 1280, 720, "av1"));
        ladder.add_entry(BitrateEntry::new(1_500_000, 854, 480, "av1"));

        ladder.auto_generate = false;
        ladder
    }

    /// Get a mobile-optimized ladder.
    #[must_use]
    pub fn mobile_optimized() -> BitrateLadder {
        let mut ladder = BitrateLadder::new();

        ladder.add_entry(BitrateEntry::new(1_500_000, 854, 480, "av1"));
        ladder.add_entry(BitrateEntry::new(800_000, 640, 360, "av1"));
        ladder.add_entry(BitrateEntry::new(400_000, 426, 240, "av1"));

        ladder.auto_generate = false;
        ladder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_info_aspect_ratio() {
        let source = SourceInfo::new(1920, 1080, 30.0, "av1".to_string());
        assert!((source.aspect_ratio() - 16.0 / 9.0).abs() < 0.01);
    }

    #[test]
    fn test_ladder_generation_1080p() {
        let source = SourceInfo::new(1920, 1080, 30.0, "av1".to_string());
        let generator = LadderGenerator::new(source);
        let ladder = generator.generate().expect("should succeed in test");

        assert!(!ladder.entries.is_empty());
        assert!(ladder.entries.iter().any(|e| e.height == 1080));
        assert!(ladder.entries.iter().any(|e| e.height == 720));
        assert!(ladder.entries.iter().any(|e| e.height == 360));
    }

    #[test]
    fn test_bitrate_calculation() {
        let source = SourceInfo::new(1920, 1080, 30.0, "av1".to_string());
        let generator = LadderGenerator::new(source);

        let bitrate_1080 = generator.calculate_bitrate(3840, 2160);
        let bitrate_720 = generator.calculate_bitrate(1920, 1080);

        assert!(bitrate_1080 > bitrate_720);
    }
}
