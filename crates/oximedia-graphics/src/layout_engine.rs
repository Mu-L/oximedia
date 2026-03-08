#![allow(dead_code)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::return_self_not_must_use)]
//! Constraint-based layout engine for broadcast graphics.
//!
//! Provides a simple box-model layout system that resolves fixed and
//! flexible size constraints, enabling responsive broadcast graphics
//! without hard-coded pixel coordinates.

/// A sizing constraint for one dimension of a layout box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LayoutConstraint {
    /// Exact pixel size.
    Fixed(f32),
    /// Proportional share of the available space (weight).
    Flex(f32),
    /// Minimum and maximum pixel bounds.
    Bounded { min: f32, max: f32 },
    /// Expands to fill all remaining space after fixed boxes are placed.
    #[default]
    Fill,
}

impl LayoutConstraint {
    /// Returns `true` if this constraint can stretch to fill available space.
    pub fn is_flexible(&self) -> bool {
        matches!(self, Self::Flex(_) | Self::Fill)
    }

    /// Returns the fixed size if this is a `Fixed` constraint, otherwise `None`.
    pub fn fixed_size(&self) -> Option<f32> {
        match self {
            Self::Fixed(s) => Some(*s),
            _ => None,
        }
    }

    /// Clamps `value` to be within this constraint's bounds where applicable.
    pub fn clamp(&self, value: f32) -> f32 {
        match self {
            Self::Fixed(s) => *s,
            Self::Bounded { min, max } => value.clamp(*min, *max),
            Self::Flex(_) | Self::Fill => value.max(0.0),
        }
    }
}

// ── LayoutBox ─────────────────────────────────────────────────────────────

/// A named rectangular region with width/height constraints and a margin.
#[derive(Debug, Clone)]
pub struct LayoutBox {
    /// Unique identifier for this box.
    pub id: String,
    /// Constraint governing the box width.
    pub width_constraint: LayoutConstraint,
    /// Constraint governing the box height.
    pub height_constraint: LayoutConstraint,
    /// Uniform margin applied to all four sides (pixels).
    pub margin: f32,
}

impl LayoutBox {
    /// Creates a new layout box with the given constraints.
    pub fn new(
        id: impl Into<String>,
        width_constraint: LayoutConstraint,
        height_constraint: LayoutConstraint,
    ) -> Self {
        Self {
            id: id.into(),
            width_constraint,
            height_constraint,
            margin: 0.0,
        }
    }

    /// Sets the margin and returns `self` for chaining.
    pub fn with_margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }

    /// Computes the resolved `(width, height)` given the available space.
    ///
    /// Flexible constraints are resolved to the full available dimension.
    #[allow(clippy::cast_precision_loss)]
    pub fn compute_size(&self, available_width: f32, available_height: f32) -> (f32, f32) {
        let w = match self.width_constraint {
            LayoutConstraint::Fixed(s) => s,
            LayoutConstraint::Flex(weight) => available_width * weight,
            LayoutConstraint::Fill => available_width,
            LayoutConstraint::Bounded { min, max } => available_width.clamp(min, max),
        };
        let h = match self.height_constraint {
            LayoutConstraint::Fixed(s) => s,
            LayoutConstraint::Flex(weight) => available_height * weight,
            LayoutConstraint::Fill => available_height,
            LayoutConstraint::Bounded { min, max } => available_height.clamp(min, max),
        };
        (w - self.margin * 2.0, h - self.margin * 2.0)
    }
}

// ── LayoutResult ──────────────────────────────────────────────────────────

/// The resolved geometry for a single box after layout.
#[derive(Debug, Clone)]
pub struct LayoutResult {
    /// Identifier matching the source `LayoutBox`.
    pub id: String,
    /// Left edge of the resolved box in container coordinates.
    pub x: f32,
    /// Top edge of the resolved box in container coordinates.
    pub y: f32,
    /// Resolved width of the box after margin subtraction.
    pub width: f32,
    /// Resolved height of the box after margin subtraction.
    pub height: f32,
}

impl LayoutResult {
    /// Returns the area of this layout result.
    pub fn total_area(&self) -> f32 {
        self.width * self.height
    }

    /// Returns `true` if the point `(px, py)` falls within this box.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }
}

// ── LayoutEngine ──────────────────────────────────────────────────────────

/// A simple horizontal-flow layout engine.
///
/// Boxes are placed left-to-right within the container. Flexible boxes
/// share the remaining width equally after fixed boxes are placed.
#[derive(Debug)]
pub struct LayoutEngine {
    boxes: Vec<LayoutBox>,
    container_width: f32,
    container_height: f32,
}

impl LayoutEngine {
    /// Creates a layout engine with the given container dimensions.
    pub fn new(container_width: f32, container_height: f32) -> Self {
        Self {
            boxes: Vec::new(),
            container_width,
            container_height,
        }
    }

    /// Appends a box to the layout.
    pub fn add_box(&mut self, b: LayoutBox) {
        self.boxes.push(b);
    }

    /// Returns the number of boxes registered.
    pub fn box_count(&self) -> usize {
        self.boxes.len()
    }

    /// Resolves all box geometries using a horizontal flow algorithm.
    ///
    /// Fixed-width boxes are placed first; remaining width is divided
    /// among flexible boxes proportionally to their flex weight.
    #[allow(clippy::cast_precision_loss)]
    pub fn layout_all(&self) -> Vec<LayoutResult> {
        // Phase 1: total fixed width used
        let fixed_width: f32 = self
            .boxes
            .iter()
            .filter_map(|b| b.width_constraint.fixed_size())
            .sum();

        let remaining_width = (self.container_width - fixed_width).max(0.0);

        // Phase 2: total flex weight
        let total_flex_weight: f32 = self
            .boxes
            .iter()
            .map(|b| match b.width_constraint {
                LayoutConstraint::Flex(w) => w,
                LayoutConstraint::Fill => 1.0,
                _ => 0.0,
            })
            .sum();

        let flex_unit = if total_flex_weight > 0.0 {
            remaining_width / total_flex_weight
        } else {
            0.0
        };

        // Phase 3: assign positions
        let mut x = 0.0_f32;
        let mut results = Vec::with_capacity(self.boxes.len());

        for b in &self.boxes {
            let w = match b.width_constraint {
                LayoutConstraint::Fixed(s) => s,
                LayoutConstraint::Flex(weight) => flex_unit * weight,
                LayoutConstraint::Fill => flex_unit,
                LayoutConstraint::Bounded { min, max } => remaining_width.clamp(min, max),
            };
            let h = match b.height_constraint {
                LayoutConstraint::Fixed(s) => s,
                LayoutConstraint::Flex(weight) => self.container_height * weight,
                LayoutConstraint::Fill => self.container_height,
                LayoutConstraint::Bounded { min, max } => self.container_height.clamp(min, max),
            };

            let inner_w = (w - b.margin * 2.0).max(0.0);
            let inner_h = (h - b.margin * 2.0).max(0.0);

            results.push(LayoutResult {
                id: b.id.clone(),
                x: x + b.margin,
                y: b.margin,
                width: inner_w,
                height: inner_h,
            });

            x += w;
        }

        results
    }
}

// ─────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_is_flexible() {
        assert!(LayoutConstraint::Flex(1.0).is_flexible());
        assert!(LayoutConstraint::Fill.is_flexible());
        assert!(!LayoutConstraint::Fixed(100.0).is_flexible());
        assert!(!LayoutConstraint::Bounded {
            min: 10.0,
            max: 200.0
        }
        .is_flexible());
    }

    #[test]
    fn test_constraint_fixed_size() {
        assert_eq!(LayoutConstraint::Fixed(50.0).fixed_size(), Some(50.0));
        assert_eq!(LayoutConstraint::Fill.fixed_size(), None);
    }

    #[test]
    fn test_constraint_clamp_fixed() {
        let c = LayoutConstraint::Fixed(120.0);
        assert_eq!(c.clamp(999.0), 120.0);
    }

    #[test]
    fn test_constraint_clamp_bounded() {
        let c = LayoutConstraint::Bounded {
            min: 50.0,
            max: 200.0,
        };
        assert_eq!(c.clamp(300.0), 200.0);
        assert_eq!(c.clamp(10.0), 50.0);
        assert_eq!(c.clamp(100.0), 100.0);
    }

    #[test]
    fn test_default_constraint_is_fill() {
        assert_eq!(LayoutConstraint::default(), LayoutConstraint::Fill);
    }

    #[test]
    fn test_layout_box_compute_size_fixed() {
        let b = LayoutBox::new(
            "box",
            LayoutConstraint::Fixed(100.0),
            LayoutConstraint::Fixed(50.0),
        );
        assert_eq!(b.compute_size(800.0, 600.0), (100.0, 50.0));
    }

    #[test]
    fn test_layout_box_compute_size_fill() {
        let b = LayoutBox::new("box", LayoutConstraint::Fill, LayoutConstraint::Fill);
        assert_eq!(b.compute_size(800.0, 600.0), (800.0, 600.0));
    }

    #[test]
    fn test_layout_box_margin_reduces_size() {
        let b = LayoutBox::new(
            "box",
            LayoutConstraint::Fixed(100.0),
            LayoutConstraint::Fixed(50.0),
        )
        .with_margin(5.0);
        let (w, h) = b.compute_size(800.0, 600.0);
        assert_eq!(w, 90.0); // 100 - 2*5
        assert_eq!(h, 40.0); // 50  - 2*5
    }

    #[test]
    fn test_layout_result_total_area() {
        let r = LayoutResult {
            id: "x".into(),
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        };
        assert_eq!(r.total_area(), 5000.0);
    }

    #[test]
    fn test_layout_result_contains() {
        let r = LayoutResult {
            id: "x".into(),
            x: 10.0,
            y: 10.0,
            width: 100.0,
            height: 50.0,
        };
        assert!(r.contains(50.0, 30.0));
        assert!(!r.contains(5.0, 30.0));
        assert!(!r.contains(50.0, 5.0));
    }

    #[test]
    fn test_engine_single_fixed_box() {
        let mut engine = LayoutEngine::new(1920.0, 1080.0);
        engine.add_box(LayoutBox::new(
            "logo",
            LayoutConstraint::Fixed(200.0),
            LayoutConstraint::Fixed(100.0),
        ));
        let results = engine.layout_all();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].width, 200.0);
        assert_eq!(results[0].x, 0.0);
    }

    #[test]
    fn test_engine_two_flex_boxes_split_equally() {
        let mut engine = LayoutEngine::new(1000.0, 100.0);
        engine.add_box(LayoutBox::new(
            "a",
            LayoutConstraint::Flex(1.0),
            LayoutConstraint::Fill,
        ));
        engine.add_box(LayoutBox::new(
            "b",
            LayoutConstraint::Flex(1.0),
            LayoutConstraint::Fill,
        ));
        let results = engine.layout_all();
        assert_eq!(results.len(), 2);
        assert!((results[0].width - 500.0).abs() < 0.01);
        assert!((results[1].width - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_engine_box_count() {
        let mut engine = LayoutEngine::new(500.0, 500.0);
        engine.add_box(LayoutBox::new(
            "a",
            LayoutConstraint::Fill,
            LayoutConstraint::Fill,
        ));
        engine.add_box(LayoutBox::new(
            "b",
            LayoutConstraint::Fill,
            LayoutConstraint::Fill,
        ));
        assert_eq!(engine.box_count(), 2);
    }

    #[test]
    fn test_engine_fixed_plus_flex() {
        let mut engine = LayoutEngine::new(1000.0, 100.0);
        engine.add_box(LayoutBox::new(
            "sidebar",
            LayoutConstraint::Fixed(200.0),
            LayoutConstraint::Fill,
        ));
        engine.add_box(LayoutBox::new(
            "main",
            LayoutConstraint::Flex(1.0),
            LayoutConstraint::Fill,
        ));
        let results = engine.layout_all();
        assert_eq!(results[0].width, 200.0);
        assert!((results[1].width - 800.0).abs() < 0.01);
    }

    #[test]
    fn test_engine_empty_produces_empty_results() {
        let engine = LayoutEngine::new(1920.0, 1080.0);
        assert!(engine.layout_all().is_empty());
    }
}
