#![allow(dead_code)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::return_self_not_must_use)]
//! SVG document construction and rendering.
//!
//! Provides a simple in-memory SVG element tree with serialisation to
//! an SVG string — useful for generating broadcast graphics assets.

/// A single SVG element that can be placed in a document.
#[derive(Debug, Clone, PartialEq)]
pub enum SvgElement {
    /// A filled/stroked rectangle.
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        fill: String,
        stroke: Option<String>,
    },
    /// A text label.
    Text {
        x: f32,
        y: f32,
        content: String,
        font_size: f32,
        fill: String,
    },
    /// A line segment.
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stroke: String,
        stroke_width: f32,
    },
    /// A circle.
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        fill: String,
    },
    /// A raw SVG path.
    Path {
        d: String,
        fill: String,
        stroke: Option<String>,
    },
}

impl SvgElement {
    /// Returns the SVG tag name for this element.
    pub fn tag_name(&self) -> &'static str {
        match self {
            Self::Rect { .. } => "rect",
            Self::Text { .. } => "text",
            Self::Line { .. } => "line",
            Self::Circle { .. } => "circle",
            Self::Path { .. } => "path",
        }
    }

    /// Serialises this element to an SVG attribute string.
    #[allow(clippy::cast_precision_loss)]
    pub fn to_svg_string(&self) -> String {
        match self {
            Self::Rect {
                x,
                y,
                width,
                height,
                fill,
                stroke,
            } => {
                let stroke_attr = stroke
                    .as_deref()
                    .map(|s| format!(" stroke=\"{s}\""))
                    .unwrap_or_default();
                format!(
                    "<rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" fill=\"{fill}\"{stroke_attr}/>"
                )
            }
            Self::Text {
                x,
                y,
                content,
                font_size,
                fill,
            } => {
                format!(
                    "<text x=\"{x}\" y=\"{y}\" font-size=\"{font_size}\" fill=\"{fill}\">{content}</text>"
                )
            }
            Self::Line {
                x1,
                y1,
                x2,
                y2,
                stroke,
                stroke_width,
            } => {
                format!(
                    "<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"{stroke}\" stroke-width=\"{stroke_width}\"/>"
                )
            }
            Self::Circle { cx, cy, r, fill } => {
                format!("<circle cx=\"{cx}\" cy=\"{cy}\" r=\"{r}\" fill=\"{fill}\"/>")
            }
            Self::Path { d, fill, stroke } => {
                let stroke_attr = stroke
                    .as_deref()
                    .map(|s| format!(" stroke=\"{s}\""))
                    .unwrap_or_default();
                format!("<path d=\"{d}\" fill=\"{fill}\"{stroke_attr}/>")
            }
        }
    }
}

// ── SvgDoc ────────────────────────────────────────────────────────────────

/// An in-memory SVG document with a fixed viewport.
#[derive(Debug, Clone)]
pub struct SvgDoc {
    width: f32,
    height: f32,
    elements: Vec<SvgElement>,
}

impl SvgDoc {
    /// Creates a new SVG document with the given viewport dimensions.
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            elements: Vec::new(),
        }
    }

    /// Appends an element to the document.
    pub fn add_element(&mut self, elem: SvgElement) {
        self.elements.push(elem);
    }

    /// Returns the number of elements in the document.
    pub fn element_count(&self) -> usize {
        self.elements.len()
    }

    /// Returns the document viewport dimensions `(width, height)`.
    pub fn dimensions(&self) -> (f32, f32) {
        (self.width, self.height)
    }

    /// Iterates over the elements in document order.
    pub fn elements(&self) -> impl Iterator<Item = &SvgElement> {
        self.elements.iter()
    }
}

// ── SvgRenderer ───────────────────────────────────────────────────────────

/// Builds and serialises `SvgDoc` instances to SVG markup.
#[derive(Debug, Default)]
pub struct SvgRenderer;

impl SvgRenderer {
    /// Creates a new renderer.
    pub fn new() -> Self {
        Self
    }

    /// Appends a rectangle to `doc` and returns a mutable reference for chaining.
    pub fn render_rect(
        &self,
        doc: &mut SvgDoc,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        fill: impl Into<String>,
    ) {
        doc.add_element(SvgElement::Rect {
            x,
            y,
            width,
            height,
            fill: fill.into(),
            stroke: None,
        });
    }

    /// Appends a text element to `doc`.
    pub fn render_text(
        &self,
        doc: &mut SvgDoc,
        x: f32,
        y: f32,
        content: impl Into<String>,
        font_size: f32,
        fill: impl Into<String>,
    ) {
        doc.add_element(SvgElement::Text {
            x,
            y,
            content: content.into(),
            font_size,
            fill: fill.into(),
        });
    }

    /// Serialises the document to an SVG string.
    pub fn to_string(&self, doc: &SvgDoc) -> String {
        let mut out = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\">\n",
            doc.width, doc.height
        );
        for elem in doc.elements() {
            out.push_str("  ");
            out.push_str(&elem.to_svg_string());
            out.push('\n');
        }
        out.push_str("</svg>");
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_tag_names() {
        let rect = SvgElement::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            fill: "red".into(),
            stroke: None,
        };
        assert_eq!(rect.tag_name(), "rect");

        let text = SvgElement::Text {
            x: 0.0,
            y: 0.0,
            content: "hi".into(),
            font_size: 12.0,
            fill: "black".into(),
        };
        assert_eq!(text.tag_name(), "text");

        let line = SvgElement::Line {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
            stroke: "blue".into(),
            stroke_width: 1.0,
        };
        assert_eq!(line.tag_name(), "line");

        let circle = SvgElement::Circle {
            cx: 5.0,
            cy: 5.0,
            r: 3.0,
            fill: "green".into(),
        };
        assert_eq!(circle.tag_name(), "circle");

        let path = SvgElement::Path {
            d: "M0 0 L10 10".into(),
            fill: "none".into(),
            stroke: None,
        };
        assert_eq!(path.tag_name(), "path");
    }

    #[test]
    fn test_rect_svg_string_contains_coords() {
        let elem = SvgElement::Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
            fill: "blue".into(),
            stroke: None,
        };
        let s = elem.to_svg_string();
        assert!(s.contains("x=\"10\""));
        assert!(s.contains("fill=\"blue\""));
    }

    #[test]
    fn test_rect_svg_string_with_stroke() {
        let elem = SvgElement::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            fill: "none".into(),
            stroke: Some("red".into()),
        };
        let s = elem.to_svg_string();
        assert!(s.contains("stroke=\"red\""));
    }

    #[test]
    fn test_text_svg_string() {
        let elem = SvgElement::Text {
            x: 5.0,
            y: 15.0,
            content: "Hello".into(),
            font_size: 14.0,
            fill: "white".into(),
        };
        let s = elem.to_svg_string();
        assert!(s.contains(">Hello</text>"));
        assert!(s.contains("font-size=\"14\""));
    }

    #[test]
    fn test_circle_svg_string() {
        let elem = SvgElement::Circle {
            cx: 50.0,
            cy: 50.0,
            r: 25.0,
            fill: "gold".into(),
        };
        let s = elem.to_svg_string();
        assert!(s.contains("cx=\"50\""));
        assert!(s.contains("r=\"25\""));
    }

    #[test]
    fn test_doc_element_count() {
        let mut doc = SvgDoc::new(1920.0, 1080.0);
        assert_eq!(doc.element_count(), 0);
        doc.add_element(SvgElement::Circle {
            cx: 0.0,
            cy: 0.0,
            r: 1.0,
            fill: "red".into(),
        });
        assert_eq!(doc.element_count(), 1);
    }

    #[test]
    fn test_doc_dimensions() {
        let doc = SvgDoc::new(1280.0, 720.0);
        assert_eq!(doc.dimensions(), (1280.0, 720.0));
    }

    #[test]
    fn test_renderer_render_rect_adds_element() {
        let renderer = SvgRenderer::new();
        let mut doc = SvgDoc::new(800.0, 600.0);
        renderer.render_rect(&mut doc, 0.0, 0.0, 100.0, 50.0, "cyan");
        assert_eq!(doc.element_count(), 1);
    }

    #[test]
    fn test_renderer_render_text_adds_element() {
        let renderer = SvgRenderer::new();
        let mut doc = SvgDoc::new(800.0, 600.0);
        renderer.render_text(&mut doc, 10.0, 20.0, "OxiMedia", 16.0, "white");
        assert_eq!(doc.element_count(), 1);
    }

    #[test]
    fn test_renderer_to_string_opens_and_closes_svg() {
        let renderer = SvgRenderer::new();
        let doc = SvgDoc::new(100.0, 100.0);
        let svg = renderer.to_string(&doc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn test_renderer_to_string_contains_element() {
        let renderer = SvgRenderer::new();
        let mut doc = SvgDoc::new(200.0, 200.0);
        renderer.render_rect(&mut doc, 5.0, 5.0, 50.0, 50.0, "orange");
        let svg = renderer.to_string(&doc);
        assert!(svg.contains("<rect"));
        assert!(svg.contains("fill=\"orange\""));
    }

    #[test]
    fn test_path_without_stroke() {
        let elem = SvgElement::Path {
            d: "M0 0 L5 5".into(),
            fill: "black".into(),
            stroke: None,
        };
        let s = elem.to_svg_string();
        assert!(!s.contains("stroke="));
    }

    #[test]
    fn test_renderer_multiple_elements_order() {
        let renderer = SvgRenderer::new();
        let mut doc = SvgDoc::new(200.0, 200.0);
        renderer.render_rect(&mut doc, 0.0, 0.0, 10.0, 10.0, "red");
        renderer.render_text(&mut doc, 5.0, 5.0, "Label", 10.0, "white");
        let elems: Vec<_> = doc.elements().collect();
        assert_eq!(elems[0].tag_name(), "rect");
        assert_eq!(elems[1].tag_name(), "text");
    }
}
