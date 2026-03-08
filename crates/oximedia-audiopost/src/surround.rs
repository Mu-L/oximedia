//! Surround sound processing for audio post-production.
//!
//! Provides surround format descriptions, channel mapping, VBAP-style panning,
//! and simple LFE management.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

/// Surround sound format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurroundLayout {
    /// Single channel (mono).
    Mono,
    /// Two channels (L, R).
    Stereo,
    /// Three channels (L, R, C).
    Lrc,
    /// Four channels (L, R, Ls, Rs).
    Quad,
    /// Six channels (L, R, C, LFE, Ls, Rs).
    FiveOne,
    /// Eight channels (L, R, C, LFE, Lss, Rss, Lrs, Rrs).
    SevenOne,
}

impl SurroundLayout {
    /// Number of audio channels in this layout.
    #[must_use]
    pub const fn channel_count(self) -> u8 {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::Lrc => 3,
            Self::Quad => 4,
            Self::FiveOne => 6,
            Self::SevenOne => 8,
        }
    }

    /// Whether this layout contains a dedicated LFE channel.
    #[must_use]
    pub const fn has_lfe(self) -> bool {
        matches!(self, Self::FiveOne | Self::SevenOne)
    }
}

/// Assignment of named channels to physical track indices.
#[derive(Debug, Clone, Default)]
pub struct ChannelMap {
    /// List of (channel_name, track_index) pairs.
    pub assignments: Vec<(String, u8)>,
}

impl ChannelMap {
    /// Create an empty channel map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create the standard 5.1 channel map (L, R, C, LFE, Ls, Rs).
    #[must_use]
    pub fn new_51() -> Self {
        Self {
            assignments: vec![
                ("L".to_string(), 0),
                ("R".to_string(), 1),
                ("C".to_string(), 2),
                ("LFE".to_string(), 3),
                ("Ls".to_string(), 4),
                ("Rs".to_string(), 5),
            ],
        }
    }

    /// Create the standard 7.1 channel map (L, R, C, LFE, Lss, Rss, Lrs, Rrs).
    #[must_use]
    pub fn new_71() -> Self {
        Self {
            assignments: vec![
                ("L".to_string(), 0),
                ("R".to_string(), 1),
                ("C".to_string(), 2),
                ("LFE".to_string(), 3),
                ("Lss".to_string(), 4),
                ("Rss".to_string(), 5),
                ("Lrs".to_string(), 6),
                ("Rrs".to_string(), 7),
            ],
        }
    }

    /// Find the track index assigned to a named channel.
    ///
    /// Returns `None` if the channel is not in the map.
    #[must_use]
    pub fn find_channel(&self, name: &str) -> Option<u8> {
        self.assignments
            .iter()
            .find(|(ch, _)| ch == name)
            .map(|(_, idx)| *idx)
    }

    /// Add or update a channel assignment.
    pub fn assign(&mut self, name: impl Into<String>, track_index: u8) {
        let name = name.into();
        if let Some(entry) = self.assignments.iter_mut().find(|(ch, _)| *ch == name) {
            entry.1 = track_index;
        } else {
            self.assignments.push((name, track_index));
        }
    }
}

/// VBAP-style surround panner for a given layout.
///
/// Maps a 2D position (x in –1..=1 left-right, y in –1..=1 back-front) to
/// per-channel gains. The gains are normalized so that the sum of squares equals 1.
#[derive(Debug, Clone)]
pub struct SurroundPannerNew {
    /// The surround layout used for panning.
    pub format: SurroundLayout,
}

impl SurroundPannerNew {
    /// Create a new panner for the given layout.
    #[must_use]
    pub fn new(format: SurroundLayout) -> Self {
        Self { format }
    }

    /// Compute per-channel gains for a position (x, y).
    ///
    /// - `x`: –1.0 = hard left, 0.0 = center, +1.0 = hard right.
    /// - `y`: –1.0 = rear, 0.0 = side, +1.0 = front.
    ///
    /// Returns a `Vec` of linear gains, one per channel, normalized so
    /// the sum of squares equals 1.0 (or all zeros for mono).
    #[must_use]
    pub fn pan(&self, x: f32, y: f32) -> Vec<f32> {
        let ch = self.format.channel_count() as usize;
        let mut gains = vec![0.0f32; ch];

        // Clamp inputs
        let x = x.clamp(-1.0, 1.0);
        let y = y.clamp(-1.0, 1.0);

        match self.format {
            SurroundLayout::Mono => {
                gains[0] = 1.0;
                return gains;
            }
            SurroundLayout::Stereo => {
                // Constant-power pan
                let angle = (x * 0.5 + 0.5) * std::f32::consts::FRAC_PI_2;
                gains[0] = angle.cos(); // L
                gains[1] = angle.sin(); // R
            }
            SurroundLayout::Lrc => {
                let right_frac = (x + 1.0) / 2.0;
                let left_frac = 1.0 - right_frac;
                let center_frac = 1.0 - right_frac.abs() - (left_frac - 0.5).abs();
                gains[0] = (left_frac * (1.0 - center_frac.max(0.0))).sqrt();
                gains[1] = (right_frac * (1.0 - center_frac.max(0.0))).sqrt();
                gains[2] = center_frac.max(0.0).sqrt();
            }
            SurroundLayout::Quad => {
                // L, R, Ls, Rs
                let front = ((y + 1.0) / 2.0).clamp(0.0, 1.0);
                let rear = 1.0 - front;
                let right = ((x + 1.0) / 2.0).clamp(0.0, 1.0);
                let left = 1.0 - right;
                gains[0] = (front * left).sqrt();
                gains[1] = (front * right).sqrt();
                gains[2] = (rear * left).sqrt();
                gains[3] = (rear * right).sqrt();
            }
            SurroundLayout::FiveOne => {
                // L, R, C, LFE, Ls, Rs
                let front = ((y + 1.0) / 2.0).clamp(0.0, 1.0);
                let rear = 1.0 - front;
                let right = ((x + 1.0) / 2.0).clamp(0.0, 1.0);
                let left = 1.0 - right;
                // Front: split between C and L/R
                let center_amt = 1.0 - (x.abs());
                let lr_front = 1.0 - center_amt * 0.5;
                gains[0] = (front * left * lr_front).sqrt();
                gains[1] = (front * right * lr_front).sqrt();
                gains[2] = (front * center_amt * 0.5).sqrt();
                gains[3] = 0.0; // LFE not driven by panning
                gains[4] = (rear * left).sqrt();
                gains[5] = (rear * right).sqrt();
            }
            SurroundLayout::SevenOne => {
                // L, R, C, LFE, Lss, Rss, Lrs, Rrs
                let front = ((y + 1.0) / 2.0).clamp(0.0, 1.0);
                let rear_total = 1.0 - front;
                let side = (rear_total * 0.5).sqrt();
                let rear = (rear_total * 0.5).sqrt();
                let right = ((x + 1.0) / 2.0).clamp(0.0, 1.0);
                let left = 1.0 - right;
                let center_amt = 1.0 - x.abs();
                let lr_front = 1.0 - center_amt * 0.5;
                gains[0] = (front * left * lr_front).sqrt();
                gains[1] = (front * right * lr_front).sqrt();
                gains[2] = (front * center_amt * 0.5).sqrt();
                gains[3] = 0.0;
                gains[4] = side * left; // Lss
                gains[5] = side * right; // Rss
                gains[6] = rear * left; // Lrs
                gains[7] = rear * right; // Rrs
            }
        }

        // Normalize so sum of squares = 1 (skip for mono which is already 1)
        if self.format != SurroundLayout::Mono {
            let sum_sq: f32 = gains.iter().map(|&g| g * g).sum();
            if sum_sq > 1e-9 {
                let norm = sum_sq.sqrt();
                for g in &mut gains {
                    *g /= norm;
                }
            }
        }

        gains
    }
}

/// Low-frequency effects manager with simple low-pass filtering.
#[derive(Debug, Clone)]
pub struct LfeManager {
    /// Low-pass crossover frequency in Hz.
    pub crossover_hz: f32,
    /// LFE channel gain in dB (applied after filtering).
    pub gain_db: f32,
}

impl LfeManager {
    /// Create a new LFE manager.
    #[must_use]
    pub fn new(crossover_hz: f32, gain_db: f32) -> Self {
        Self {
            crossover_hz,
            gain_db,
        }
    }

    /// Extract LFE content from a mono sample slice using a running-average low-pass filter.
    ///
    /// The window size is derived from the crossover frequency and sample rate.
    /// A linear gain is applied based on `gain_db`.
    ///
    /// Returns a `Vec<f32>` of the same length as `samples`.
    #[must_use]
    pub fn extract_lfe(&self, samples: &[f32], sample_rate: u32) -> Vec<f32> {
        if samples.is_empty() || sample_rate == 0 {
            return vec![];
        }

        // Window size = samples per half-cycle at crossover frequency
        // Clamp to at least 1 to avoid division by zero
        let window = ((sample_rate as f32 / (2.0 * self.crossover_hz.max(1.0))) as usize).max(1);
        let linear_gain = 10_f32.powf(self.gain_db / 20.0);

        let n = samples.len();
        let mut output = vec![0.0f32; n];

        // Running-average low-pass
        let mut running_sum = 0.0f32;
        // Pre-fill with zeros (causal filter)
        for (i, &s) in samples.iter().enumerate() {
            running_sum += s;
            if i >= window {
                running_sum -= samples[i - window];
            }
            let effective_window = (i + 1).min(window);
            output[i] = (running_sum / effective_window as f32) * linear_gain;
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surround_layout_channel_count_mono() {
        assert_eq!(SurroundLayout::Mono.channel_count(), 1);
    }

    #[test]
    fn test_surround_layout_channel_count_stereo() {
        assert_eq!(SurroundLayout::Stereo.channel_count(), 2);
    }

    #[test]
    fn test_surround_layout_channel_count_51() {
        assert_eq!(SurroundLayout::FiveOne.channel_count(), 6);
    }

    #[test]
    fn test_surround_layout_channel_count_71() {
        assert_eq!(SurroundLayout::SevenOne.channel_count(), 8);
    }

    #[test]
    fn test_has_lfe_51() {
        assert!(SurroundLayout::FiveOne.has_lfe());
    }

    #[test]
    fn test_has_lfe_stereo() {
        assert!(!SurroundLayout::Stereo.has_lfe());
    }

    #[test]
    fn test_channel_map_51_find_lfe() {
        let map = ChannelMap::new_51();
        assert_eq!(map.find_channel("LFE"), Some(3));
    }

    #[test]
    fn test_channel_map_71_find_lrs() {
        let map = ChannelMap::new_71();
        assert_eq!(map.find_channel("Lrs"), Some(6));
    }

    #[test]
    fn test_channel_map_find_missing() {
        let map = ChannelMap::new_51();
        assert_eq!(map.find_channel("Unknown"), None);
    }

    #[test]
    fn test_channel_map_assign_new() {
        let mut map = ChannelMap::new();
        map.assign("X", 5);
        assert_eq!(map.find_channel("X"), Some(5));
    }

    #[test]
    fn test_channel_map_assign_update() {
        let mut map = ChannelMap::new_51();
        map.assign("L", 10);
        assert_eq!(map.find_channel("L"), Some(10));
    }

    #[test]
    fn test_panner_mono_returns_one() {
        let panner = SurroundPannerNew::new(SurroundLayout::Mono);
        let gains = panner.pan(0.5, 0.5);
        assert_eq!(gains.len(), 1);
        assert!((gains[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_panner_stereo_center_equal() {
        let panner = SurroundPannerNew::new(SurroundLayout::Stereo);
        let gains = panner.pan(0.0, 0.0);
        assert_eq!(gains.len(), 2);
        // At center (x=0), both channels should be approximately equal
        let diff = (gains[0] - gains[1]).abs();
        assert!(diff < 0.1, "L and R differ by {diff} at center");
    }

    #[test]
    fn test_panner_51_returns_six_gains() {
        let panner = SurroundPannerNew::new(SurroundLayout::FiveOne);
        let gains = panner.pan(0.0, 1.0);
        assert_eq!(gains.len(), 6);
    }

    #[test]
    fn test_panner_71_returns_eight_gains() {
        let panner = SurroundPannerNew::new(SurroundLayout::SevenOne);
        let gains = panner.pan(0.0, 0.0);
        assert_eq!(gains.len(), 8);
    }

    #[test]
    fn test_panner_gains_normalized() {
        let panner = SurroundPannerNew::new(SurroundLayout::Stereo);
        let gains = panner.pan(0.3, 0.0);
        let sum_sq: f32 = gains.iter().map(|&g| g * g).sum();
        assert!((sum_sq - 1.0).abs() < 1e-5, "sum_sq={sum_sq}");
    }

    #[test]
    fn test_lfe_manager_output_length() {
        let lfe = LfeManager::new(120.0, 0.0);
        let samples = vec![0.5f32; 100];
        let out = lfe.extract_lfe(&samples, 48000);
        assert_eq!(out.len(), 100);
    }

    #[test]
    fn test_lfe_manager_empty_input() {
        let lfe = LfeManager::new(120.0, 0.0);
        let out = lfe.extract_lfe(&[], 48000);
        assert!(out.is_empty());
    }

    #[test]
    fn test_lfe_manager_gain_applied() {
        let lfe_unity = LfeManager::new(120.0, 0.0);
        let lfe_boost = LfeManager::new(120.0, 6.0); // ~2x linear
        let samples = vec![1.0f32; 100];
        let out_unity = lfe_unity.extract_lfe(&samples, 48000);
        let out_boost = lfe_boost.extract_lfe(&samples, 48000);
        // Last sample should reflect the gain ratio
        assert!(
            out_boost[99] > out_unity[99],
            "Boost should increase output"
        );
    }
}
