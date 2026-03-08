//! Color grading edit nodes for the timeline editor.
//!
//! Provides a node-based color grading system with lift/gamma/gain,
//! curves, HSL adjustments, and LUT application.

#![allow(dead_code)]

/// A single color grading node type.
#[derive(Debug, Clone, PartialEq)]
pub enum GradeNode {
    /// Lift, Gamma, Gain correction.
    LiftGammaGain {
        /// Lift values (shadows) per channel [R, G, B].
        lift: [f32; 3],
        /// Gamma values (midtones) per channel [R, G, B].
        gamma: [f32; 3],
        /// Gain values (highlights) per channel [R, G, B].
        gain: [f32; 3],
    },
    /// Custom curves adjustment.
    Curves {
        /// Control points for the master curve.
        points: Vec<(f32, f32)>,
    },
    /// Hue/Saturation/Luminance adjustment.
    Hsl {
        /// Hue rotation in degrees.
        hue: f32,
        /// Saturation multiplier.
        saturation: f32,
        /// Luminance offset.
        luminance: f32,
    },
    /// LUT file application.
    Lut {
        /// Path to the LUT file.
        path: String,
        /// Blend opacity (0.0–1.0).
        opacity: f32,
    },
}

impl GradeNode {
    /// Return a human-readable name for this node type.
    #[must_use]
    pub fn node_name(&self) -> &'static str {
        match self {
            Self::LiftGammaGain { .. } => "LiftGammaGain",
            Self::Curves { .. } => "Curves",
            Self::Hsl { .. } => "HSL",
            Self::Lut { .. } => "LUT",
        }
    }
}

/// A color grade edit attached to a clip.
#[derive(Debug, Clone)]
pub struct ColorGradeEdit {
    /// The grading node used.
    pub node: GradeNode,
    /// Whether bypassed (not applied).
    pub bypassed: bool,
    /// Whether this edit permanently alters pixel data.
    pub destructive: bool,
}

impl ColorGradeEdit {
    /// Create a new `ColorGradeEdit`.
    #[must_use]
    pub fn new(node: GradeNode, destructive: bool) -> Self {
        Self {
            node,
            bypassed: false,
            destructive,
        }
    }

    /// Returns `true` if the edit permanently alters pixel data.
    #[must_use]
    pub fn is_destructive(&self) -> bool {
        self.destructive
    }

    /// Returns `true` if the edit is currently bypassed.
    #[must_use]
    pub fn is_bypassed(&self) -> bool {
        self.bypassed
    }

    /// Toggle the bypass state.
    pub fn toggle_bypass(&mut self) {
        self.bypassed = !self.bypassed;
    }
}

/// A stack of color grade edits applied in order.
#[derive(Debug, Clone, Default)]
pub struct ColorGradeStack {
    nodes: Vec<ColorGradeEdit>,
}

impl ColorGradeStack {
    /// Create an empty `ColorGradeStack`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a `ColorGradeEdit` to the stack.
    pub fn push(&mut self, edit: ColorGradeEdit) {
        self.nodes.push(edit);
    }

    /// Return the number of nodes in the stack.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Apply the stack (simulation — returns a pass-through pixel value).
    ///
    /// Each active, non-bypassed node is visited in order.
    /// Returns the input value unchanged in this stub implementation.
    #[must_use]
    pub fn apply(&self, pixel: [f32; 3]) -> [f32; 3] {
        let mut out = pixel;
        for edit in &self.nodes {
            if edit.bypassed {
                continue;
            }
            match &edit.node {
                GradeNode::LiftGammaGain { lift, gamma, gain } => {
                    for i in 0..3 {
                        #[allow(clippy::cast_precision_loss)]
                        let v = out[i] + lift[i];
                        let v = v.powf(1.0 / gamma[i].max(0.001));
                        out[i] = (v * gain[i]).clamp(0.0, 1.0);
                    }
                }
                GradeNode::Hsl {
                    saturation,
                    luminance,
                    ..
                } => {
                    let lum = 0.2126 * out[0] + 0.7152 * out[1] + 0.0722 * out[2];
                    for v in &mut out {
                        *v = ((*v - lum) * saturation + lum + luminance).clamp(0.0, 1.0);
                    }
                }
                GradeNode::Lut { opacity, .. } => {
                    // Identity stub: blend towards identity by opacity factor
                    let blend = opacity.clamp(0.0, 1.0);
                    for v in &mut out {
                        *v = (*v * blend + *v * (1.0 - blend)).clamp(0.0, 1.0);
                    }
                }
                GradeNode::Curves { .. } => {
                    // Curves stub: identity pass
                }
            }
        }
        out
    }

    /// Returns a reference to the node edits in the stack.
    #[must_use]
    pub fn edits(&self) -> &[ColorGradeEdit] {
        &self.nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lggg_node() -> GradeNode {
        GradeNode::LiftGammaGain {
            lift: [0.0; 3],
            gamma: [1.0; 3],
            gain: [1.0; 3],
        }
    }

    #[test]
    fn test_grade_node_name_lgg() {
        assert_eq!(lggg_node().node_name(), "LiftGammaGain");
    }

    #[test]
    fn test_grade_node_name_curves() {
        let n = GradeNode::Curves { points: vec![] };
        assert_eq!(n.node_name(), "Curves");
    }

    #[test]
    fn test_grade_node_name_hsl() {
        let n = GradeNode::Hsl {
            hue: 0.0,
            saturation: 1.0,
            luminance: 0.0,
        };
        assert_eq!(n.node_name(), "HSL");
    }

    #[test]
    fn test_grade_node_name_lut() {
        let n = GradeNode::Lut {
            path: "film.cube".to_string(),
            opacity: 1.0,
        };
        assert_eq!(n.node_name(), "LUT");
    }

    #[test]
    fn test_color_grade_edit_is_destructive() {
        let edit = ColorGradeEdit::new(lggg_node(), true);
        assert!(edit.is_destructive());
    }

    #[test]
    fn test_color_grade_edit_not_destructive() {
        let edit = ColorGradeEdit::new(lggg_node(), false);
        assert!(!edit.is_destructive());
    }

    #[test]
    fn test_toggle_bypass() {
        let mut edit = ColorGradeEdit::new(lggg_node(), false);
        assert!(!edit.is_bypassed());
        edit.toggle_bypass();
        assert!(edit.is_bypassed());
        edit.toggle_bypass();
        assert!(!edit.is_bypassed());
    }

    #[test]
    fn test_stack_node_count() {
        let mut stack = ColorGradeStack::new();
        assert_eq!(stack.node_count(), 0);
        stack.push(ColorGradeEdit::new(lggg_node(), false));
        assert_eq!(stack.node_count(), 1);
        stack.push(ColorGradeEdit::new(
            GradeNode::Curves { points: vec![] },
            false,
        ));
        assert_eq!(stack.node_count(), 2);
    }

    #[test]
    fn test_stack_apply_identity_lgg() {
        let mut stack = ColorGradeStack::new();
        stack.push(ColorGradeEdit::new(lggg_node(), false));
        let px = [0.5, 0.4, 0.3];
        let out = stack.apply(px);
        // gain=1, gamma=1, lift=0 → identity
        assert!((out[0] - 0.5).abs() < 1e-5);
        assert!((out[1] - 0.4).abs() < 1e-5);
        assert!((out[2] - 0.3).abs() < 1e-5);
    }

    #[test]
    fn test_stack_apply_bypass_skips_node() {
        let mut stack = ColorGradeStack::new();
        let mut edit = ColorGradeEdit::new(
            GradeNode::Hsl {
                hue: 0.0,
                saturation: 0.0, // would desaturate
                luminance: 0.0,
            },
            false,
        );
        edit.bypassed = true;
        stack.push(edit);
        let px = [0.8, 0.2, 0.5];
        let out = stack.apply(px);
        // bypassed → unchanged
        assert!((out[0] - 0.8).abs() < 1e-5);
    }

    #[test]
    fn test_stack_apply_empty() {
        let stack = ColorGradeStack::new();
        let px = [0.1, 0.2, 0.9];
        let out = stack.apply(px);
        assert_eq!(out, px);
    }

    #[test]
    fn test_stack_edits_len() {
        let mut stack = ColorGradeStack::new();
        stack.push(ColorGradeEdit::new(lggg_node(), false));
        assert_eq!(stack.edits().len(), 1);
    }

    #[test]
    fn test_stack_lut_identity_opacity() {
        let mut stack = ColorGradeStack::new();
        stack.push(ColorGradeEdit::new(
            GradeNode::Lut {
                path: "test.cube".into(),
                opacity: 1.0,
            },
            false,
        ));
        let px = [0.3, 0.6, 0.9];
        let out = stack.apply(px);
        assert!((out[0] - 0.3).abs() < 1e-5);
    }

    #[test]
    fn test_stack_hsl_full_saturation() {
        let mut stack = ColorGradeStack::new();
        stack.push(ColorGradeEdit::new(
            GradeNode::Hsl {
                hue: 0.0,
                saturation: 1.0,
                luminance: 0.0,
            },
            false,
        ));
        let px = [0.5, 0.5, 0.5];
        let out = stack.apply(px);
        // saturation=1, no luminance offset → should equal original for grey
        assert!((out[0] - 0.5).abs() < 1e-5);
    }
}
