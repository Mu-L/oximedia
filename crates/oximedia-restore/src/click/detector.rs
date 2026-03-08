//! Click and pop detection.

use crate::error::RestoreResult;

/// Click detection configuration.
#[derive(Debug, Clone)]
pub struct ClickDetectorConfig {
    /// Detection sensitivity (0.0 = low, 1.0 = high).
    pub sensitivity: f32,
    /// Minimum click duration in samples.
    pub min_duration: usize,
    /// Maximum click duration in samples.
    pub max_duration: usize,
    /// Threshold multiplier for detection.
    pub threshold_multiplier: f32,
}

impl Default for ClickDetectorConfig {
    fn default() -> Self {
        Self {
            sensitivity: 0.5,
            min_duration: 1,
            max_duration: 100,
            threshold_multiplier: 3.0,
        }
    }
}

/// Detected click or pop.
#[derive(Debug, Clone)]
pub struct Click {
    /// Start sample index.
    pub start: usize,
    /// End sample index (exclusive).
    pub end: usize,
    /// Peak magnitude.
    pub magnitude: f32,
}

/// Click detector.
#[derive(Debug, Clone)]
pub struct ClickDetector {
    config: ClickDetectorConfig,
}

impl ClickDetector {
    /// Create a new click detector.
    #[must_use]
    pub fn new(config: ClickDetectorConfig) -> Self {
        Self { config }
    }

    /// Detect clicks in samples.
    ///
    /// # Arguments
    ///
    /// * `samples` - Input samples
    ///
    /// # Returns
    ///
    /// List of detected clicks.
    pub fn detect(&self, samples: &[f32]) -> RestoreResult<Vec<Click>> {
        if samples.len() < 3 {
            return Ok(Vec::new());
        }

        // Compute first-order difference to highlight transients
        let mut diff: Vec<f32> = Vec::with_capacity(samples.len() - 1);
        for i in 1..samples.len() {
            diff.push((samples[i] - samples[i - 1]).abs());
        }

        // Compute adaptive threshold based on median
        let threshold = self.compute_threshold(&diff);

        // Find regions above threshold
        let mut clicks = Vec::new();
        let mut in_click = false;
        let mut click_start = 0;
        let mut peak_magnitude = 0.0;

        for (i, &d) in diff.iter().enumerate() {
            if !in_click && d > threshold {
                // Start of click
                in_click = true;
                click_start = i;
                peak_magnitude = d;
            } else if in_click {
                if d > threshold {
                    // Continue click
                    peak_magnitude = peak_magnitude.max(d);
                } else {
                    // End of click
                    let duration = i - click_start;
                    if duration >= self.config.min_duration && duration <= self.config.max_duration
                    {
                        clicks.push(Click {
                            start: click_start,
                            end: i,
                            magnitude: peak_magnitude,
                        });
                    }
                    in_click = false;
                }
            }
        }

        // Handle click at end of buffer
        if in_click {
            let duration = diff.len() - click_start;
            if duration >= self.config.min_duration && duration <= self.config.max_duration {
                clicks.push(Click {
                    start: click_start,
                    end: diff.len(),
                    magnitude: peak_magnitude,
                });
            }
        }

        Ok(clicks)
    }

    /// Compute adaptive threshold using median-based method.
    fn compute_threshold(&self, diff: &[f32]) -> f32 {
        if diff.is_empty() {
            return 0.0;
        }

        // Compute median
        let mut sorted = diff.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let median = if sorted.len() % 2 == 0 {
            (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
        } else {
            sorted[sorted.len() / 2]
        };

        // Compute MAD (Median Absolute Deviation)
        let mut deviations: Vec<f32> = diff.iter().map(|&d| (d - median).abs()).collect();
        deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mad = if deviations.len() % 2 == 0 {
            (deviations[deviations.len() / 2 - 1] + deviations[deviations.len() / 2]) / 2.0
        } else {
            deviations[deviations.len() / 2]
        };

        // Threshold = median + multiplier * MAD
        let base_threshold = median + self.config.threshold_multiplier * mad;

        // Adjust for sensitivity
        base_threshold * (1.0 - self.config.sensitivity * 0.5)
    }
}

/// Detect clicks using a simple energy-based method.
///
/// # Arguments
///
/// * `samples` - Input samples
/// * `threshold` - Energy threshold
/// * `window_size` - Window size for energy computation
///
/// # Returns
///
/// List of detected clicks.
#[must_use]
pub fn detect_clicks_simple(samples: &[f32], threshold: f32, window_size: usize) -> Vec<Click> {
    if samples.len() < window_size {
        return Vec::new();
    }

    let mut clicks: Vec<Click> = Vec::new();
    let half_window = window_size / 2;

    for i in half_window..samples.len() - half_window {
        // Compute local energy
        let start = i.saturating_sub(half_window);
        let end = (i + half_window).min(samples.len());

        let energy: f32 = samples[start..end].iter().map(|&s| s * s).sum();
        let avg_energy = energy / (end - start) as f32;

        if avg_energy > threshold {
            // Check if this is a new click or part of existing one
            if let Some(last_click) = clicks.last_mut() {
                if i <= last_click.end + window_size {
                    // Extend existing click
                    last_click.end = i + 1;
                    last_click.magnitude = last_click.magnitude.max(avg_energy);
                    continue;
                }
            }

            // New click
            clicks.push(Click {
                start: i.saturating_sub(window_size),
                end: i + window_size,
                magnitude: avg_energy,
            });
        }
    }

    clicks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_click_detector() {
        let mut samples = vec![0.0; 1000];

        // Add some clicks
        samples[100] = 1.0;
        samples[101] = -0.8;
        samples[500] = 0.9;
        samples[501] = -0.7;

        let detector = ClickDetector::new(ClickDetectorConfig::default());
        let clicks = detector.detect(&samples).expect("should succeed in test");

        assert!(!clicks.is_empty());
    }

    #[test]
    fn test_detect_clicks_simple() {
        let mut samples = vec![0.0; 1000];

        // Add impulse
        samples[500] = 1.0;

        let clicks = detect_clicks_simple(&samples, 0.1, 5);
        assert!(!clicks.is_empty());
    }

    #[test]
    fn test_config_default() {
        let config = ClickDetectorConfig::default();
        assert_eq!(config.sensitivity, 0.5);
        assert!(config.min_duration > 0);
        assert!(config.max_duration > config.min_duration);
    }
}
