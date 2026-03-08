//! XML construction utilities for IMF document generation.
//!
//! IMF relies heavily on XML for CPL, PKL, ASSETMAP, and OPL documents.
//! This module provides lightweight builder types for programmatically
//! constructing XML trees and serialising them to strings, without
//! depending on a full XML DOM library.

#![allow(dead_code)]

use std::fmt;

// ---------------------------------------------------------------------------
// XmlAttribute
// ---------------------------------------------------------------------------

/// A single key-value attribute on an XML element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlAttribute {
    /// Attribute name.
    pub name: String,
    /// Attribute value (will be XML-escaped on serialisation).
    pub value: String,
}

impl XmlAttribute {
    /// Create a new attribute.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Format as `name="escaped_value"`.
    #[must_use]
    pub fn to_xml_string(&self) -> String {
        format!("{}=\"{}\"", self.name, escape_xml_attr(&self.value))
    }
}

impl fmt::Display for XmlAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_xml_string())
    }
}

// ---------------------------------------------------------------------------
// XmlElement
// ---------------------------------------------------------------------------

/// An XML element node with optional attributes, text content, and children.
#[derive(Debug, Clone)]
pub struct XmlElement {
    /// Element tag name (may include namespace prefix, e.g., "cpl:Id").
    pub tag: String,
    /// Attributes on this element.
    pub attributes: Vec<XmlAttribute>,
    /// Direct text content (if any). Mutually exclusive with children in
    /// well-formed documents, but both are allowed for flexibility.
    pub text: Option<String>,
    /// Child elements.
    pub children: Vec<XmlElement>,
}

impl XmlElement {
    /// Create a new element with the given tag name.
    #[must_use]
    pub fn new(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            attributes: Vec::new(),
            text: None,
            children: Vec::new(),
        }
    }

    /// Add an attribute.
    #[must_use]
    pub fn with_attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.push(XmlAttribute::new(name, value));
        self
    }

    /// Set the text content.
    #[must_use]
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Add a child element.
    #[must_use]
    pub fn with_child(mut self, child: XmlElement) -> Self {
        self.children.push(child);
        self
    }

    /// Returns `true` if element has no children and no text.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.children.is_empty() && self.text.is_none()
    }

    /// Count of direct children.
    #[must_use]
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Count of attributes.
    #[must_use]
    pub fn attr_count(&self) -> usize {
        self.attributes.len()
    }

    /// Look up an attribute value by name.
    #[must_use]
    pub fn get_attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }

    /// Find first child with the given tag name.
    #[must_use]
    pub fn find_child(&self, tag: &str) -> Option<&XmlElement> {
        self.children.iter().find(|c| c.tag == tag)
    }

    /// Recursively count all descendant elements (not including self).
    #[must_use]
    pub fn descendant_count(&self) -> usize {
        self.children.iter().map(|c| 1 + c.descendant_count()).sum()
    }

    /// Serialize the element to an XML string (no XML declaration).
    #[must_use]
    pub fn to_xml_string(&self) -> String {
        self.write_xml(0)
    }

    /// Internal recursive writer with indentation level.
    fn write_xml(&self, indent: usize) -> String {
        let prefix = "  ".repeat(indent);
        let mut buf = String::new();

        // Opening tag
        buf.push_str(&prefix);
        buf.push('<');
        buf.push_str(&self.tag);
        for attr in &self.attributes {
            buf.push(' ');
            buf.push_str(&attr.to_xml_string());
        }

        if self.is_empty() {
            buf.push_str("/>\n");
            return buf;
        }

        buf.push('>');

        if let Some(ref text) = self.text {
            if self.children.is_empty() {
                // Inline text: <Tag>text</Tag>
                buf.push_str(&escape_xml_text(text));
                buf.push_str("</");
                buf.push_str(&self.tag);
                buf.push_str(">\n");
                return buf;
            }
        }

        buf.push('\n');

        // Text content on its own line (when children also exist)
        if let Some(ref text) = self.text {
            buf.push_str(&"  ".repeat(indent + 1));
            buf.push_str(&escape_xml_text(text));
            buf.push('\n');
        }

        for child in &self.children {
            buf.push_str(&child.write_xml(indent + 1));
        }

        buf.push_str(&prefix);
        buf.push_str("</");
        buf.push_str(&self.tag);
        buf.push_str(">\n");
        buf
    }
}

impl fmt::Display for XmlElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_xml_string())
    }
}

// ---------------------------------------------------------------------------
// XmlBuilder
// ---------------------------------------------------------------------------

/// Convenience builder for creating IMF XML documents from a root element.
#[derive(Debug, Clone)]
pub struct XmlBuilder {
    /// Optional XML declaration version string (e.g. "1.0").
    pub xml_version: Option<String>,
    /// Optional encoding declaration (e.g. "UTF-8").
    pub encoding: Option<String>,
    /// Root element of the document.
    pub root: XmlElement,
}

impl XmlBuilder {
    /// Create a new XML builder with a root element tag.
    #[must_use]
    pub fn new(root_tag: impl Into<String>) -> Self {
        Self {
            xml_version: Some("1.0".to_string()),
            encoding: Some("UTF-8".to_string()),
            root: XmlElement::new(root_tag),
        }
    }

    /// Set the XML version.
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.xml_version = Some(version.into());
        self
    }

    /// Disable the XML declaration.
    #[must_use]
    pub fn without_declaration(mut self) -> Self {
        self.xml_version = None;
        self.encoding = None;
        self
    }

    /// Add an attribute to the root element.
    #[must_use]
    pub fn with_root_attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.root.attributes.push(XmlAttribute::new(name, value));
        self
    }

    /// Add a child element to the root.
    #[must_use]
    pub fn with_child(mut self, child: XmlElement) -> Self {
        self.root.children.push(child);
        self
    }

    /// Serialize the full document to a string (including XML declaration
    /// if configured).
    #[must_use]
    pub fn to_string(&self) -> String {
        let mut buf = String::new();

        if let Some(ref ver) = self.xml_version {
            buf.push_str("<?xml version=\"");
            buf.push_str(ver);
            buf.push('"');
            if let Some(ref enc) = self.encoding {
                buf.push_str(" encoding=\"");
                buf.push_str(enc);
                buf.push('"');
            }
            buf.push_str("?>\n");
        }

        buf.push_str(&self.root.to_xml_string());
        buf
    }
}

impl fmt::Display for XmlBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}

// ---------------------------------------------------------------------------
// XML escaping helpers
// ---------------------------------------------------------------------------

/// Escape special characters for XML attribute values.
fn escape_xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Escape special characters for XML text content.
fn escape_xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribute_new() {
        let a = XmlAttribute::new("id", "abc");
        assert_eq!(a.name, "id");
        assert_eq!(a.value, "abc");
    }

    #[test]
    fn test_attribute_to_xml_string() {
        let a = XmlAttribute::new("name", "hello");
        assert_eq!(a.to_xml_string(), "name=\"hello\"");
    }

    #[test]
    fn test_attribute_escaping() {
        let a = XmlAttribute::new("val", "a<b&c\"d");
        let s = a.to_xml_string();
        assert!(s.contains("&lt;"));
        assert!(s.contains("&amp;"));
        assert!(s.contains("&quot;"));
    }

    #[test]
    fn test_element_empty() {
        let e = XmlElement::new("EmptyTag");
        assert!(e.is_empty());
        let xml = e.to_xml_string();
        assert!(xml.contains("<EmptyTag/>"));
    }

    #[test]
    fn test_element_with_text() {
        let e = XmlElement::new("Title").with_text("My Movie");
        let xml = e.to_xml_string();
        assert!(xml.contains("<Title>My Movie</Title>"));
    }

    #[test]
    fn test_element_text_escaping() {
        let e = XmlElement::new("Data").with_text("a < b & c");
        let xml = e.to_xml_string();
        assert!(xml.contains("a &lt; b &amp; c"));
    }

    #[test]
    fn test_element_with_attr() {
        let e = XmlElement::new("Resource").with_attr("id", "r1");
        let xml = e.to_xml_string();
        assert!(xml.contains("id=\"r1\""));
    }

    #[test]
    fn test_element_with_children() {
        let e = XmlElement::new("Root")
            .with_child(XmlElement::new("A").with_text("1"))
            .with_child(XmlElement::new("B").with_text("2"));
        assert_eq!(e.child_count(), 2);
        let xml = e.to_xml_string();
        assert!(xml.contains("<A>1</A>"));
        assert!(xml.contains("<B>2</B>"));
    }

    #[test]
    fn test_element_get_attr() {
        let e = XmlElement::new("X")
            .with_attr("foo", "bar")
            .with_attr("baz", "qux");
        assert_eq!(e.get_attr("foo"), Some("bar"));
        assert_eq!(e.get_attr("missing"), None);
    }

    #[test]
    fn test_element_find_child() {
        let e = XmlElement::new("Root")
            .with_child(XmlElement::new("Alpha"))
            .with_child(XmlElement::new("Beta"));
        assert!(e.find_child("Alpha").is_some());
        assert!(e.find_child("Gamma").is_none());
    }

    #[test]
    fn test_element_descendant_count() {
        let e = XmlElement::new("Root")
            .with_child(
                XmlElement::new("A")
                    .with_child(XmlElement::new("A1"))
                    .with_child(XmlElement::new("A2")),
            )
            .with_child(XmlElement::new("B"));
        // A + A1 + A2 + B = 4
        assert_eq!(e.descendant_count(), 4);
    }

    #[test]
    fn test_builder_with_declaration() {
        let doc = XmlBuilder::new("Root")
            .with_root_attr("xmlns", "http://example.com")
            .with_child(XmlElement::new("Title").with_text("Test"))
            .to_string();
        assert!(doc.starts_with("<?xml version=\"1.0\""));
        assert!(doc.contains("encoding=\"UTF-8\""));
        assert!(doc.contains("<Root"));
        assert!(doc.contains("<Title>Test</Title>"));
    }

    #[test]
    fn test_builder_without_declaration() {
        let doc = XmlBuilder::new("Root").without_declaration().to_string();
        assert!(!doc.contains("<?xml"));
        assert!(doc.contains("<Root/>"));
    }

    #[test]
    fn test_builder_display() {
        let doc = XmlBuilder::new("Root").with_child(XmlElement::new("A"));
        let s = format!("{doc}");
        assert!(s.contains("<Root>"));
        assert!(s.contains("</Root>"));
    }

    #[test]
    fn test_attribute_display() {
        let a = XmlAttribute::new("x", "y");
        assert_eq!(format!("{a}"), "x=\"y\"");
    }

    #[test]
    fn test_element_display() {
        let e = XmlElement::new("Tag").with_text("val");
        let s = format!("{e}");
        assert!(s.contains("<Tag>val</Tag>"));
    }
}
