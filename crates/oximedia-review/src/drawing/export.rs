//! Export drawings and annotations.

use crate::{
    drawing::{Annotation, Shape},
    error::ReviewResult,
    SessionId,
};
use serde::{Deserialize, Serialize};

/// Export format for drawings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    /// SVG (Scalable Vector Graphics).
    Svg,
    /// PNG (Portable Network Graphics).
    Png,
    /// PDF (Portable Document Format).
    Pdf,
    /// JSON (JavaScript Object Notation).
    Json,
}

/// Export options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    /// Export format.
    pub format: ExportFormat,
    /// Include hidden annotations.
    pub include_hidden: bool,
    /// Include locked annotations.
    pub include_locked: bool,
    /// Background color (if any).
    pub background_color: Option<crate::drawing::Color>,
    /// Export resolution (for raster formats).
    pub resolution: (u32, u32),
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Svg,
            include_hidden: false,
            include_locked: true,
            background_color: None,
            resolution: (1920, 1080),
        }
    }
}

impl ExportOptions {
    /// Create new export options.
    #[must_use]
    pub fn new(format: ExportFormat) -> Self {
        Self {
            format,
            ..Default::default()
        }
    }

    /// Set whether to include hidden annotations.
    #[must_use]
    pub fn include_hidden(mut self, include: bool) -> Self {
        self.include_hidden = include;
        self
    }

    /// Set resolution.
    #[must_use]
    pub fn with_resolution(mut self, width: u32, height: u32) -> Self {
        self.resolution = (width, height);
        self
    }
}

/// Export annotations to a file.
///
/// # Arguments
///
/// * `session_id` - ID of the session
/// * `annotations` - Annotations to export
/// * `options` - Export options
/// * `output_path` - Output file path
///
/// # Errors
///
/// Returns error if export fails.
pub async fn export_annotations(
    session_id: SessionId,
    annotations: &[Annotation],
    options: &ExportOptions,
    output_path: &str,
) -> ReviewResult<()> {
    // Filter annotations based on options
    let filtered: Vec<&Annotation> = annotations
        .iter()
        .filter(|a| (options.include_hidden || a.visible) && (options.include_locked || !a.locked))
        .collect();

    match options.format {
        ExportFormat::Svg => export_to_svg(session_id, &filtered, output_path).await?,
        ExportFormat::Png => export_to_png(session_id, &filtered, options, output_path).await?,
        ExportFormat::Pdf => export_to_pdf(session_id, &filtered, output_path).await?,
        ExportFormat::Json => export_to_json(session_id, &filtered, output_path).await?,
    }

    Ok(())
}

async fn export_to_svg(
    _session_id: SessionId,
    _annotations: &[&Annotation],
    _output_path: &str,
) -> ReviewResult<()> {
    // In a real implementation, this would:
    // 1. Create SVG document
    // 2. Add each annotation as SVG element
    // 3. Write to file

    Ok(())
}

async fn export_to_png(
    _session_id: SessionId,
    _annotations: &[&Annotation],
    _options: &ExportOptions,
    _output_path: &str,
) -> ReviewResult<()> {
    // In a real implementation, this would:
    // 1. Create image buffer
    // 2. Render each annotation
    // 3. Write to PNG file

    Ok(())
}

async fn export_to_pdf(
    _session_id: SessionId,
    _annotations: &[&Annotation],
    _output_path: &str,
) -> ReviewResult<()> {
    // In a real implementation, this would:
    // 1. Create PDF document
    // 2. Add each annotation as PDF element
    // 3. Write to file

    Ok(())
}

async fn export_to_json(
    _session_id: SessionId,
    annotations: &[&Annotation],
    output_path: &str,
) -> ReviewResult<()> {
    // Serialize annotations to JSON
    let json = serde_json::to_string_pretty(annotations)?;

    // Write to file
    std::fs::write(output_path, json)?;

    Ok(())
}

/// Convert shape to SVG path string.
#[must_use]
pub fn shape_to_svg_path(shape: &Shape) -> String {
    match shape {
        Shape::Arrow(arrow) => {
            format!(
                "M {} {} L {} {}",
                arrow.start.x, arrow.start.y, arrow.end.x, arrow.end.y
            )
        }
        Shape::Circle(circle) => {
            format!(
                "M {} {} m -{}, 0 a {},{} 0 1,0 {},0 a {},{} 0 1,0 -{},0",
                circle.center.x,
                circle.center.y,
                circle.radius,
                circle.radius,
                circle.radius,
                circle.radius * 2.0,
                circle.radius,
                circle.radius,
                circle.radius * 2.0
            )
        }
        Shape::Rectangle(rect) => {
            format!(
                "M {} {} L {} {} L {} {} L {} {} Z",
                rect.top_left.x,
                rect.top_left.y,
                rect.bottom_right.x,
                rect.top_left.y,
                rect.bottom_right.x,
                rect.bottom_right.y,
                rect.top_left.x,
                rect.bottom_right.y
            )
        }
        Shape::Freehand(path) => {
            if path.points.is_empty() {
                return String::new();
            }

            let mut svg = format!("M {} {}", path.points[0].x, path.points[0].y);
            for point in &path.points[1..] {
                svg.push_str(&format!(" L {} {}", point.x, point.y));
            }
            svg
        }
        Shape::Text(_) => String::new(), // Text handled separately
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::{Arrow, Circle, Color, Point, Rectangle, StrokeStyle};

    #[test]
    fn test_export_options_default() {
        let options = ExportOptions::default();
        assert_eq!(options.format, ExportFormat::Svg);
        assert!(!options.include_hidden);
        assert!(options.include_locked);
    }

    #[test]
    fn test_export_options_builder() {
        let options = ExportOptions::new(ExportFormat::Png)
            .include_hidden(true)
            .with_resolution(3840, 2160);

        assert_eq!(options.format, ExportFormat::Png);
        assert!(options.include_hidden);
        assert_eq!(options.resolution, (3840, 2160));
    }

    #[test]
    fn test_shape_to_svg_path_arrow() {
        let arrow = Arrow::new(Point::new(0.0, 0.0), Point::new(1.0, 1.0), 0.1);
        let svg = shape_to_svg_path(&Shape::Arrow(arrow));
        assert!(svg.starts_with("M 0 0 L 1 1"));
    }

    #[test]
    fn test_shape_to_svg_path_circle() {
        let circle = Circle::new(Point::new(0.5, 0.5), 0.2);
        let svg = shape_to_svg_path(&Shape::Circle(circle));
        assert!(svg.contains("M 0.5 0.5"));
    }

    #[test]
    fn test_shape_to_svg_path_rectangle() {
        let rect = Rectangle::new(Point::new(0.0, 0.0), Point::new(1.0, 1.0));
        let svg = shape_to_svg_path(&Shape::Rectangle(rect));
        assert!(svg.contains("M 0 0"));
        assert!(svg.contains("Z")); // Path closes
    }

    #[tokio::test]
    async fn test_export_to_json() {
        let session_id = SessionId::new();
        let drawing = crate::drawing::Drawing {
            id: crate::DrawingId::new(),
            session_id,
            frame: 100,
            tool: crate::drawing::tools::DrawingTool::Circle,
            shape: Shape::Circle(Circle::new(Point::new(0.5, 0.5), 0.2)),
            style: StrokeStyle::solid(Color::red(), 2.0),
            author: "test".to_string(),
        };

        let annotation = Annotation::new(drawing);
        let annotations = vec![&annotation];

        let temp_file = "/tmp/test_export.json";
        let result = export_to_json(session_id, &annotations, temp_file).await;
        assert!(result.is_ok());

        // Clean up
        let _ = std::fs::remove_file(temp_file);
    }
}
