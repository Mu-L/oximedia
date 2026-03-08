//! Color histogram for indexing.

/// Color histogram
pub struct ColorHistogram {
    bins: Vec<f32>,
}

impl ColorHistogram {
    /// Create a new histogram with specified bins
    #[must_use]
    pub fn new(num_bins: usize) -> Self {
        Self {
            bins: vec![0.0; num_bins],
        }
    }

    /// Add a color to the histogram
    pub fn add_color(&mut self, _r: u8, _g: u8, _b: u8) {
        // Placeholder
    }

    /// Normalize the histogram
    pub fn normalize(&mut self) {
        let sum: f32 = self.bins.iter().sum();
        if sum > 0.0 {
            for bin in &mut self.bins {
                *bin /= sum;
            }
        }
    }

    /// Get bins
    #[must_use]
    pub fn bins(&self) -> &[f32] {
        &self.bins
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_histogram() {
        let histogram = ColorHistogram::new(64);
        assert_eq!(histogram.bins().len(), 64);
    }
}
