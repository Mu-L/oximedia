//! Drawing tools and annotations for visual feedback.

use crate::{DrawingId, SessionId};
use serde::{Deserialize, Serialize};

pub mod annotation;
pub mod color;
pub mod export;
pub mod tools;

pub use annotation::{Annotation, AnnotationLayer};
pub use color::{Color, StrokeStyle};
pub use tools::{DrawingTool, Shape};

/// Point in 2D space (normalized coordinates 0.0-1.0).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// X coordinate (0.0 = left, 1.0 = right).
    pub x: f32,
    /// Y coordinate (0.0 = top, 1.0 = bottom).
    pub y: f32,
}

impl Point {
    /// Create a new point.
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Calculate distance to another point.
    #[must_use]
    pub fn distance_to(&self, other: &Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Rectangle defined by top-left and bottom-right corners.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rectangle {
    /// Top-left corner.
    pub top_left: Point,
    /// Bottom-right corner.
    pub bottom_right: Point,
}

impl Rectangle {
    /// Create a new rectangle.
    #[must_use]
    pub fn new(top_left: Point, bottom_right: Point) -> Self {
        Self {
            top_left,
            bottom_right,
        }
    }

    /// Get the width of the rectangle.
    #[must_use]
    pub fn width(&self) -> f32 {
        (self.bottom_right.x - self.top_left.x).abs()
    }

    /// Get the height of the rectangle.
    #[must_use]
    pub fn height(&self) -> f32 {
        (self.bottom_right.y - self.top_left.y).abs()
    }

    /// Get the center point.
    #[must_use]
    pub fn center(&self) -> Point {
        Point::new(
            (self.top_left.x + self.bottom_right.x) / 2.0,
            (self.top_left.y + self.bottom_right.y) / 2.0,
        )
    }

    /// Check if a point is inside the rectangle.
    #[must_use]
    pub fn contains(&self, point: &Point) -> bool {
        point.x >= self.top_left.x
            && point.x <= self.bottom_right.x
            && point.y >= self.top_left.y
            && point.y <= self.bottom_right.y
    }
}

/// Circle defined by center and radius.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Circle {
    /// Center point.
    pub center: Point,
    /// Radius (in normalized coordinates).
    pub radius: f32,
}

impl Circle {
    /// Create a new circle.
    #[must_use]
    pub fn new(center: Point, radius: f32) -> Self {
        Self { center, radius }
    }

    /// Check if a point is inside the circle.
    #[must_use]
    pub fn contains(&self, point: &Point) -> bool {
        self.center.distance_to(point) <= self.radius
    }
}

/// Arrow from one point to another.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Arrow {
    /// Start point.
    pub start: Point,
    /// End point.
    pub end: Point,
    /// Arrow head size.
    pub head_size: f32,
}

impl Arrow {
    /// Create a new arrow.
    #[must_use]
    pub fn new(start: Point, end: Point, head_size: f32) -> Self {
        Self {
            start,
            end,
            head_size,
        }
    }

    /// Get the length of the arrow.
    #[must_use]
    pub fn length(&self) -> f32 {
        self.start.distance_to(&self.end)
    }
}

/// Text annotation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextAnnotation {
    /// Position of the text.
    pub position: Point,
    /// Text content.
    pub text: String,
    /// Font size.
    pub font_size: f32,
}

impl TextAnnotation {
    /// Create a new text annotation.
    #[must_use]
    pub fn new(position: Point, text: String, font_size: f32) -> Self {
        Self {
            position,
            text,
            font_size,
        }
    }
}

/// Freehand path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreehandPath {
    /// Points in the path.
    pub points: Vec<Point>,
    /// Whether the path is smooth.
    pub smooth: bool,
}

impl FreehandPath {
    /// Create a new freehand path.
    #[must_use]
    pub fn new(points: Vec<Point>, smooth: bool) -> Self {
        Self { points, smooth }
    }

    /// Add a point to the path.
    pub fn add_point(&mut self, point: Point) {
        self.points.push(point);
    }

    /// Get the total length of the path.
    #[must_use]
    pub fn length(&self) -> f32 {
        self.points
            .windows(2)
            .map(|w| w[0].distance_to(&w[1]))
            .sum()
    }
}

/// Drawing on a frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drawing {
    /// Drawing ID.
    pub id: DrawingId,
    /// Session ID.
    pub session_id: SessionId,
    /// Frame number.
    pub frame: i64,
    /// Drawing tool used.
    pub tool: DrawingTool,
    /// Shape data.
    pub shape: Shape,
    /// Color and style.
    pub style: StrokeStyle,
    /// Author.
    pub author: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_distance() {
        let p1 = Point::new(0.0, 0.0);
        let p2 = Point::new(3.0, 4.0);
        assert!((p1.distance_to(&p2) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_rectangle_dimensions() {
        let rect = Rectangle::new(Point::new(0.0, 0.0), Point::new(1.0, 1.0));
        assert!((rect.width() - 1.0).abs() < 0.001);
        assert!((rect.height() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_rectangle_center() {
        let rect = Rectangle::new(Point::new(0.0, 0.0), Point::new(2.0, 2.0));
        let center = rect.center();
        assert!((center.x - 1.0).abs() < 0.001);
        assert!((center.y - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_rectangle_contains() {
        let rect = Rectangle::new(Point::new(0.0, 0.0), Point::new(1.0, 1.0));
        assert!(rect.contains(&Point::new(0.5, 0.5)));
        assert!(!rect.contains(&Point::new(1.5, 0.5)));
    }

    #[test]
    fn test_circle_contains() {
        let circle = Circle::new(Point::new(0.0, 0.0), 1.0);
        assert!(circle.contains(&Point::new(0.5, 0.0)));
        assert!(!circle.contains(&Point::new(2.0, 0.0)));
    }

    #[test]
    fn test_arrow_length() {
        let arrow = Arrow::new(Point::new(0.0, 0.0), Point::new(3.0, 4.0), 0.1);
        assert!((arrow.length() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_freehand_path() {
        let mut path = FreehandPath::new(Vec::new(), true);
        path.add_point(Point::new(0.0, 0.0));
        path.add_point(Point::new(1.0, 0.0));
        assert_eq!(path.points.len(), 2);
        assert!((path.length() - 1.0).abs() < 0.001);
    }
}
