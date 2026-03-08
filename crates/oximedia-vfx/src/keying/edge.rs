//! Edge refinement for keyed footage.

use crate::{EffectParams, Frame, VfxResult, VideoEffect};
use serde::{Deserialize, Serialize};

/// Edge refinement method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeRefinementMethod {
    /// Erode edges.
    Erode,
    /// Dilate edges.
    Dilate,
    /// Smooth edges.
    Smooth,
    /// Sharpen edges.
    Sharpen,
}

/// Edge refinement effect.
pub struct EdgeRefine {
    method: EdgeRefinementMethod,
    amount: f32,
    radius: u32,
}

impl EdgeRefine {
    /// Create a new edge refine effect.
    #[must_use]
    pub const fn new(method: EdgeRefinementMethod) -> Self {
        Self {
            method,
            amount: 1.0,
            radius: 1,
        }
    }

    /// Set refinement amount (0.0 - 1.0).
    #[must_use]
    pub fn with_amount(mut self, amount: f32) -> Self {
        self.amount = amount.clamp(0.0, 1.0);
        self
    }

    /// Set processing radius.
    #[must_use]
    pub fn with_radius(mut self, radius: u32) -> Self {
        self.radius = radius.max(1);
        self
    }

    fn get_neighbor_alphas(&self, input: &Frame, x: u32, y: u32) -> Vec<u8> {
        let mut alphas = Vec::new();
        let r = self.radius as i32;

        for dy in -r..=r {
            for dx in -r..=r {
                let nx = (x as i32 + dx).max(0).min(input.width as i32 - 1) as u32;
                let ny = (y as i32 + dy).max(0).min(input.height as i32 - 1) as u32;

                if let Some(pixel) = input.get_pixel(nx, ny) {
                    alphas.push(pixel[3]);
                }
            }
        }

        alphas
    }
}

impl VideoEffect for EdgeRefine {
    fn name(&self) -> &'static str {
        "Edge Refine"
    }

    fn description(&self) -> &'static str {
        "Refine edges of keyed footage"
    }

    fn apply(
        &mut self,
        input: &Frame,
        output: &mut Frame,
        _params: &EffectParams,
    ) -> VfxResult<()> {
        for y in 0..output.height {
            for x in 0..output.width {
                let pixel = input.get_pixel(x, y).unwrap_or([0, 0, 0, 0]);
                let alphas = self.get_neighbor_alphas(input, x, y);

                let new_alpha = match self.method {
                    EdgeRefinementMethod::Erode => {
                        let min_alpha = *alphas.iter().min().unwrap_or(&255);
                        let current = pixel[3];
                        ((1.0 - self.amount) * f32::from(current)
                            + self.amount * f32::from(min_alpha)) as u8
                    }
                    EdgeRefinementMethod::Dilate => {
                        let max_alpha = *alphas.iter().max().unwrap_or(&0);
                        let current = pixel[3];
                        ((1.0 - self.amount) * f32::from(current)
                            + self.amount * f32::from(max_alpha)) as u8
                    }
                    EdgeRefinementMethod::Smooth => {
                        let avg_alpha =
                            alphas.iter().map(|&a| u32::from(a)).sum::<u32>() / alphas.len() as u32;
                        let current = pixel[3];
                        ((1.0 - self.amount) * f32::from(current) + self.amount * avg_alpha as f32)
                            as u8
                    }
                    EdgeRefinementMethod::Sharpen => {
                        let avg_alpha =
                            alphas.iter().map(|&a| u32::from(a)).sum::<u32>() / alphas.len() as u32;
                        let current = i32::from(pixel[3]);
                        let sharpened =
                            current + ((current - avg_alpha as i32) as f32 * self.amount) as i32;
                        sharpened.clamp(0, 255) as u8
                    }
                };

                output.set_pixel(x, y, [pixel[0], pixel[1], pixel[2], new_alpha]);
            }
        }

        Ok(())
    }

    fn supports_gpu(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_refine() {
        let methods = [
            EdgeRefinementMethod::Erode,
            EdgeRefinementMethod::Dilate,
            EdgeRefinementMethod::Smooth,
        ];

        for method in methods {
            let mut refine = EdgeRefine::new(method);
            let input = Frame::new(100, 100).expect("should succeed in test");
            let mut output = Frame::new(100, 100).expect("should succeed in test");
            let params = EffectParams::new();
            refine
                .apply(&input, &mut output, &params)
                .expect("should succeed in test");
        }
    }
}
