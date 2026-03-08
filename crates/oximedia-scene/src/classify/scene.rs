//! Scene classification (indoor/outdoor, day/night, etc.).

use crate::common::Confidence;
use crate::error::{SceneError, SceneResult};
use serde::{Deserialize, Serialize};

/// Type of scene detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneType {
    /// Indoor scene.
    Indoor,
    /// Outdoor scene.
    Outdoor,
    /// Day scene (bright, well-lit).
    Day,
    /// Night scene (dark, low-light).
    Night,
    /// Landscape orientation and composition.
    Landscape,
    /// Portrait orientation and composition.
    Portrait,
    /// Urban environment.
    Urban,
    /// Natural environment.
    Natural,
    /// Water scene (ocean, lake, river).
    Water,
    /// Sky-dominant scene.
    Sky,
    /// Unknown or mixed scene.
    Unknown,
}

impl SceneType {
    /// Get all possible scene types.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::Indoor,
            Self::Outdoor,
            Self::Day,
            Self::Night,
            Self::Landscape,
            Self::Portrait,
            Self::Urban,
            Self::Natural,
            Self::Water,
            Self::Sky,
            Self::Unknown,
        ]
    }

    /// Get human-readable name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Indoor => "Indoor",
            Self::Outdoor => "Outdoor",
            Self::Day => "Day",
            Self::Night => "Night",
            Self::Landscape => "Landscape",
            Self::Portrait => "Portrait",
            Self::Urban => "Urban",
            Self::Natural => "Natural",
            Self::Water => "Water",
            Self::Sky => "Sky",
            Self::Unknown => "Unknown",
        }
    }
}

/// Scene classification result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneClassification {
    /// Primary scene type.
    pub scene_type: SceneType,
    /// Confidence score.
    pub confidence: Confidence,
    /// Scores for all scene types.
    pub scores: Vec<(SceneType, f32)>,
    /// Additional features used for classification.
    pub features: SceneFeatures,
}

/// Features extracted for scene classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneFeatures {
    /// Average brightness (0.0-1.0).
    pub brightness: f32,
    /// Color temperature (warm/cool).
    pub color_temperature: f32,
    /// Saturation level (0.0-1.0).
    pub saturation: f32,
    /// Sky region ratio (0.0-1.0).
    pub sky_ratio: f32,
    /// Vegetation ratio (0.0-1.0).
    pub vegetation_ratio: f32,
    /// Artificial structure ratio (0.0-1.0).
    pub structure_ratio: f32,
    /// Horizon line position (0.0-1.0, from top).
    pub horizon_position: Option<f32>,
}

impl Default for SceneFeatures {
    fn default() -> Self {
        Self {
            brightness: 0.5,
            color_temperature: 0.5,
            saturation: 0.5,
            sky_ratio: 0.0,
            vegetation_ratio: 0.0,
            structure_ratio: 0.0,
            horizon_position: None,
        }
    }
}

/// Configuration for scene classification.
#[derive(Debug, Clone)]
pub struct SceneConfig {
    /// Minimum confidence threshold.
    pub confidence_threshold: f32,
    /// Enable color histogram analysis.
    pub use_color_histogram: bool,
    /// Enable edge detection.
    pub use_edge_detection: bool,
    /// Enable texture analysis.
    pub use_texture_analysis: bool,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.5,
            use_color_histogram: true,
            use_edge_detection: true,
            use_texture_analysis: true,
        }
    }
}

/// Scene classifier using color histograms and heuristics.
pub struct SceneClassifier {
    config: SceneConfig,
}

impl SceneClassifier {
    /// Create a new scene classifier.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SceneConfig::default(),
        }
    }

    /// Create a scene classifier with custom configuration.
    #[must_use]
    pub fn with_config(config: SceneConfig) -> Self {
        Self { config }
    }

    /// Classify a scene from RGB image data.
    ///
    /// # Arguments
    ///
    /// * `rgb_data` - RGB image data (height x width x 3)
    /// * `width` - Image width
    /// * `height` - Image height
    ///
    /// # Errors
    ///
    /// Returns error if classification fails or invalid dimensions.
    pub fn classify(
        &self,
        rgb_data: &[u8],
        width: usize,
        height: usize,
    ) -> SceneResult<SceneClassification> {
        if rgb_data.len() != width * height * 3 {
            return Err(SceneError::InvalidDimensions(format!(
                "Expected {} bytes, got {}",
                width * height * 3,
                rgb_data.len()
            )));
        }

        // Extract features
        let features = self.extract_features(rgb_data, width, height)?;

        // Compute scores for each scene type
        let mut scores = Vec::new();
        scores.push((SceneType::Indoor, self.score_indoor(&features)));
        scores.push((SceneType::Outdoor, self.score_outdoor(&features)));
        scores.push((SceneType::Day, self.score_day(&features)));
        scores.push((SceneType::Night, self.score_night(&features)));
        scores.push((SceneType::Landscape, self.score_landscape(&features)));
        scores.push((SceneType::Portrait, self.score_portrait(&features)));
        scores.push((SceneType::Urban, self.score_urban(&features)));
        scores.push((SceneType::Natural, self.score_natural(&features)));
        scores.push((SceneType::Water, self.score_water(&features)));
        scores.push((SceneType::Sky, self.score_sky(&features)));

        // Find highest score
        let (scene_type, confidence) = scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map_or((SceneType::Unknown, 0.0), |(t, s)| (*t, *s));

        Ok(SceneClassification {
            scene_type,
            confidence: Confidence::new(confidence),
            scores,
            features,
        })
    }

    /// Extract scene features from RGB data.
    fn extract_features(
        &self,
        rgb_data: &[u8],
        width: usize,
        height: usize,
    ) -> SceneResult<SceneFeatures> {
        let mut brightness_sum = 0.0;
        let mut saturation_sum = 0.0;
        let mut color_temp_sum = 0.0;
        let pixel_count = width * height;

        // Analyze pixels
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 3;
                let r = f32::from(rgb_data[idx]);
                let g = f32::from(rgb_data[idx + 1]);
                let b = f32::from(rgb_data[idx + 2]);

                // Brightness (perceived luminance)
                brightness_sum += 0.299 * r + 0.587 * g + 0.114 * b;

                // Saturation
                let max = r.max(g).max(b);
                let min = r.min(g).min(b);
                if max > 0.0 {
                    saturation_sum += (max - min) / max;
                }

                // Color temperature (blue vs red)
                color_temp_sum += (b - r) / 255.0;
            }
        }

        let brightness = (brightness_sum / (pixel_count as f32 * 255.0)).clamp(0.0, 1.0);
        let saturation = (saturation_sum / pixel_count as f32).clamp(0.0, 1.0);
        let color_temperature = ((color_temp_sum / pixel_count as f32) + 1.0) / 2.0;

        // Detect sky, vegetation, and structures using color heuristics
        let (sky_ratio, vegetation_ratio, structure_ratio) =
            self.detect_regions(rgb_data, width, height);

        // Detect horizon
        let horizon_position = self.detect_horizon(rgb_data, width, height);

        Ok(SceneFeatures {
            brightness,
            color_temperature,
            saturation,
            sky_ratio,
            vegetation_ratio,
            structure_ratio,
            horizon_position,
        })
    }

    /// Detect sky, vegetation, and structure regions.
    fn detect_regions(&self, rgb_data: &[u8], width: usize, height: usize) -> (f32, f32, f32) {
        let mut sky_pixels = 0;
        let mut vegetation_pixels = 0;
        let mut structure_pixels = 0;
        let pixel_count = width * height;

        for i in (0..rgb_data.len()).step_by(3) {
            let r = rgb_data[i];
            let g = rgb_data[i + 1];
            let b = rgb_data[i + 2];

            // Sky: blue dominant, high brightness
            if b > r && b > g && b > 128 {
                sky_pixels += 1;
            }
            // Vegetation: green dominant
            else if g > r && g > b && g > 64 {
                vegetation_pixels += 1;
            }
            // Structure: low saturation (gray tones)
            else {
                let max = r.max(g).max(b);
                let min = r.min(g).min(b);
                if max > 0 && (max - min) < 30 {
                    structure_pixels += 1;
                }
            }
        }

        (
            sky_pixels as f32 / pixel_count as f32,
            vegetation_pixels as f32 / pixel_count as f32,
            structure_pixels as f32 / pixel_count as f32,
        )
    }

    /// Detect horizon line position.
    fn detect_horizon(&self, rgb_data: &[u8], width: usize, height: usize) -> Option<f32> {
        // Simple horizon detection: find strongest horizontal edge in middle third
        let start_y = height / 3;
        let end_y = (height * 2) / 3;
        let mut max_edge = 0.0;
        let mut horizon_y = None;

        for y in start_y..end_y {
            let mut edge_strength = 0.0;
            for x in 1..width - 1 {
                let _idx = (y * width + x) * 3;
                let idx_above = ((y - 1) * width + x) * 3;
                let idx_below = ((y + 1) * width + x) * 3;

                // Vertical gradient
                for c in 0..3 {
                    let diff = (rgb_data[idx_below + c] as i32 - rgb_data[idx_above + c] as i32)
                        .unsigned_abs() as f32;
                    edge_strength += diff;
                }
            }

            if edge_strength > max_edge {
                max_edge = edge_strength;
                horizon_y = Some(y);
            }
        }

        horizon_y.map(|y| y as f32 / height as f32)
    }

    // Scoring functions for each scene type
    fn score_indoor(&self, features: &SceneFeatures) -> f32 {
        let mut score = 0.0;
        // Indoor scenes typically have lower brightness
        score += (1.0 - features.brightness) * 0.3;
        // Less sky
        score += (1.0 - features.sky_ratio) * 0.4;
        // More structures
        score += features.structure_ratio * 0.3;
        score.clamp(0.0, 1.0)
    }

    fn score_outdoor(&self, features: &SceneFeatures) -> f32 {
        let mut score = 0.0;
        // Higher brightness
        score += features.brightness * 0.3;
        // More sky
        score += features.sky_ratio * 0.4;
        // Natural elements
        score += features.vegetation_ratio * 0.3;
        score.clamp(0.0, 1.0)
    }

    fn score_day(&self, features: &SceneFeatures) -> f32 {
        // High brightness, high saturation
        (features.brightness * 0.7 + features.saturation * 0.3).clamp(0.0, 1.0)
    }

    fn score_night(&self, features: &SceneFeatures) -> f32 {
        // Low brightness
        (1.0 - features.brightness).clamp(0.0, 1.0)
    }

    fn score_landscape(&self, features: &SceneFeatures) -> f32 {
        let mut score = 0.0;
        // Horizon present
        if features.horizon_position.is_some() {
            score += 0.5;
        }
        // Sky and vegetation
        score += (features.sky_ratio + features.vegetation_ratio) * 0.5;
        score.clamp(0.0, 1.0)
    }

    fn score_portrait(&self, features: &SceneFeatures) -> f32 {
        // Less sky, more centered composition
        let mut score = 1.0 - features.sky_ratio;
        if let Some(horizon) = features.horizon_position {
            // Horizon in middle third is less common in portraits
            if (0.33..=0.67).contains(&horizon) {
                score *= 0.5;
            }
        }
        score.clamp(0.0, 1.0)
    }

    fn score_urban(&self, features: &SceneFeatures) -> f32 {
        // More structures, less vegetation
        let mut score = features.structure_ratio * 0.6;
        score += (1.0 - features.vegetation_ratio) * 0.4;
        score.clamp(0.0, 1.0)
    }

    fn score_natural(&self, features: &SceneFeatures) -> f32 {
        // More vegetation, high saturation
        (features.vegetation_ratio * 0.7 + features.saturation * 0.3).clamp(0.0, 1.0)
    }

    fn score_water(&self, features: &SceneFeatures) -> f32 {
        // Cool color temperature, specific horizon position
        let mut score = 0.0;
        if features.color_temperature > 0.5 {
            score += (features.color_temperature - 0.5) * 2.0 * 0.5;
        }
        if let Some(horizon) = features.horizon_position {
            // Water typically has horizon in middle third
            if (0.33..=0.67).contains(&horizon) {
                score += 0.5;
            }
        }
        score.clamp(0.0, 1.0)
    }

    fn score_sky(&self, features: &SceneFeatures) -> f32 {
        // High sky ratio
        features.sky_ratio
    }
}

impl Default for SceneClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_type_name() {
        assert_eq!(SceneType::Indoor.name(), "Indoor");
        assert_eq!(SceneType::Outdoor.name(), "Outdoor");
    }

    #[test]
    fn test_scene_classifier() {
        let classifier = SceneClassifier::new();

        // Create a bright blue image (sky scene)
        let width = 100;
        let height = 100;
        let mut rgb_data = vec![0u8; width * height * 3];
        for i in (0..rgb_data.len()).step_by(3) {
            rgb_data[i] = 100; // R
            rgb_data[i + 1] = 150; // G
            rgb_data[i + 2] = 255; // B (bright blue)
        }

        let result = classifier.classify(&rgb_data, width, height);
        assert!(result.is_ok());

        let classification = result.expect("should succeed in test");
        assert!(classification.confidence.value() > 0.0);
    }

    #[test]
    fn test_invalid_dimensions() {
        let classifier = SceneClassifier::new();
        let rgb_data = vec![0u8; 100];
        let result = classifier.classify(&rgb_data, 10, 10);
        assert!(result.is_err());
    }

    #[test]
    fn test_scene_features_default() {
        let features = SceneFeatures::default();
        assert!((features.brightness - 0.5).abs() < f32::EPSILON);
        assert!((features.saturation - 0.5).abs() < f32::EPSILON);
    }
}
