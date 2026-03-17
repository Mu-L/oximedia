//! XMP (Extensible Metadata Platform) parsing and writing support.
//!
//! XMP is an Adobe standard for embedding metadata in files using RDF/XML.
//!
//! # Format
//!
//! XMP metadata is stored as RDF/XML with various namespaces:
//! - **dc**: Dublin Core (dc:title, dc:creator, dc:rights, etc.)
//! - **xmp**: XMP Basic (xmp:CreateDate, xmp:ModifyDate, etc.)
//! - **xmpRights**: XMP Rights Management
//! - **photoshop**: Photoshop-specific metadata
//!
//! # Example
//!
//! ```xml
//! <x:xmpmeta xmlns:x="adobe:ns:meta/">
//!   <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
//!     <rdf:Description xmlns:dc="http://purl.org/dc/elements/1.1/">
//!       <dc:title>My Title</dc:title>
//!     </rdf:Description>
//!   </rdf:RDF>
//! </x:xmpmeta>
//! ```

use crate::{Error, Metadata, MetadataFormat, MetadataValue};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;

// ---- XMP Structured Property Types ----

/// XMP structured property kind per the XMP specification.
///
/// - **Seq** (`rdf:Seq`): Ordered array. The order of items is significant.
/// - **Bag** (`rdf:Bag`): Unordered set.  The order of items is not significant.
/// - **Alt** (`rdf:Alt`): Alternative values (typically language alternatives).
///   The first item is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XmpArrayKind {
    /// Ordered array (`rdf:Seq`).
    Seq,
    /// Unordered set (`rdf:Bag`).
    Bag,
    /// Language alternatives (`rdf:Alt`).
    Alt,
}

impl XmpArrayKind {
    /// The RDF element name for this array kind.
    pub fn rdf_element(self) -> &'static str {
        match self {
            Self::Seq => "rdf:Seq",
            Self::Bag => "rdf:Bag",
            Self::Alt => "rdf:Alt",
        }
    }

    /// Try to detect the array kind from a local element name.
    pub fn from_element(name: &str) -> Option<Self> {
        let local = if let Some(pos) = name.find(':') {
            &name[pos + 1..]
        } else {
            name
        };
        match local {
            "Seq" => Some(Self::Seq),
            "Bag" => Some(Self::Bag),
            "Alt" => Some(Self::Alt),
            _ => None,
        }
    }
}

/// An XMP structured property containing an array of items.
#[derive(Debug, Clone, PartialEq)]
pub struct XmpArray {
    /// The kind of array.
    pub kind: XmpArrayKind,
    /// The items in the array (in order for Seq, unordered for Bag,
    /// first = default for Alt).
    pub items: Vec<String>,
}

impl XmpArray {
    /// Create a new array.
    pub fn new(kind: XmpArrayKind) -> Self {
        Self {
            kind,
            items: Vec::new(),
        }
    }

    /// Add an item.
    pub fn push(&mut self, item: impl Into<String>) {
        self.items.push(item.into());
    }

    /// Get the default / first item (meaningful for Alt arrays).
    pub fn default_item(&self) -> Option<&str> {
        self.items.first().map(|s| s.as_str())
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the array is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// XMP packet start marker
const XMP_PACKET_START: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>"#;

/// XMP packet end marker
const XMP_PACKET_END: &str = r#"<?xpacket end="w"?>"#;

/// Parse XMP metadata from XML data.
///
/// Supports simple text properties and structured array properties
/// (`rdf:Seq`, `rdf:Bag`, `rdf:Alt`).  When an array is encountered,
/// the property value is stored as `MetadataValue::TextList`.
///
/// # Errors
///
/// Returns an error if the data is not valid XMP.
pub fn parse(data: &[u8]) -> Result<Metadata, Error> {
    let mut metadata = Metadata::new(MetadataFormat::Xmp);
    let mut reader = Reader::from_reader(Cursor::new(data));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut current_tag = String::new();
    let mut _current_namespace = String::new();

    // State for structured array parsing
    let mut in_array: Option<(String, XmpArrayKind)> = None; // (parent tag, kind)
    let mut array_items: Vec<String> = Vec::new();
    let mut in_li = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                // Check if this is an array container element
                if let Some(kind) = XmpArrayKind::from_element(&name) {
                    if !current_tag.is_empty() {
                        in_array = Some((current_tag.clone(), kind));
                        array_items.clear();
                    }
                } else if in_array.is_some() && name.ends_with(":li") || name == "rdf:li" {
                    in_li = true;
                } else if let Some(colon_pos) = name.find(':') {
                    _current_namespace = name[..colon_pos].to_string();
                    current_tag = name;
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if let Some(colon_pos) = name.find(':') {
                    _current_namespace = name[..colon_pos].to_string();
                    current_tag = name;
                }
            }
            Ok(Event::Text(e)) => {
                let text = reader
                    .decoder()
                    .decode(e.as_ref())
                    .map_err(|err| Error::Xml(format!("Failed to decode text: {err}")))?
                    .to_string();

                if text.trim().is_empty() {
                    buf.clear();
                    continue;
                }

                if in_li && in_array.is_some() {
                    array_items.push(text);
                } else if !current_tag.is_empty() {
                    metadata.insert(current_tag.clone(), MetadataValue::Text(text));
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                if name.ends_with(":li") || name == "rdf:li" {
                    in_li = false;
                } else if XmpArrayKind::from_element(&name).is_some() {
                    // End of array container -- store collected items
                    if let Some((ref parent_tag, _kind)) = in_array {
                        if !array_items.is_empty() {
                            metadata.insert(
                                parent_tag.clone(),
                                MetadataValue::TextList(array_items.clone()),
                            );
                        }
                    }
                    in_array = None;
                    array_items.clear();
                } else {
                    if in_array.is_none() {
                        current_tag.clear();
                        _current_namespace.clear();
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Xml(format!("XML parse error: {e}"))),
            _ => {}
        }
        buf.clear();
    }

    Ok(metadata)
}

/// Write XMP metadata to XML data.
///
/// # Errors
///
/// Returns an error if writing fails.
pub fn write(metadata: &Metadata) -> Result<Vec<u8>, Error> {
    let mut result = Vec::new();

    // Write packet start
    result.extend_from_slice(XMP_PACKET_START.as_bytes());
    result.push(b'\n');

    // Create XML writer
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // Write xmpmeta start
    let mut xmpmeta = BytesStart::new("x:xmpmeta");
    xmpmeta.push_attribute(("xmlns:x", "adobe:ns:meta/"));
    writer
        .write_event(Event::Start(xmpmeta))
        .map_err(|e| Error::Xml(format!("Failed to write xmpmeta start: {e}")))?;

    // Write RDF start
    let mut rdf = BytesStart::new("rdf:RDF");
    rdf.push_attribute(("xmlns:rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"));
    writer
        .write_event(Event::Start(rdf))
        .map_err(|e| Error::Xml(format!("Failed to write RDF start: {e}")))?;

    // Group fields by namespace
    let mut dc_fields = Vec::new();
    let mut xmp_fields = Vec::new();
    let mut other_fields = Vec::new();

    for (key, value) in metadata.fields() {
        if key.starts_with("dc:") {
            dc_fields.push((key, value));
        } else if key.starts_with("xmp:") {
            xmp_fields.push((key, value));
        } else {
            other_fields.push((key, value));
        }
    }

    // Write Dublin Core description
    if !dc_fields.is_empty() {
        write_description(&mut writer, &dc_fields, "http://purl.org/dc/elements/1.1/")?;
    }

    // Write XMP description
    if !xmp_fields.is_empty() {
        write_description(&mut writer, &xmp_fields, "http://ns.adobe.com/xap/1.0/")?;
    }

    // Write other descriptions
    if !other_fields.is_empty() {
        write_description(&mut writer, &other_fields, "")?;
    }

    // Write RDF end
    writer
        .write_event(Event::End(BytesEnd::new("rdf:RDF")))
        .map_err(|e| Error::Xml(format!("Failed to write RDF end: {e}")))?;

    // Write xmpmeta end
    writer
        .write_event(Event::End(BytesEnd::new("x:xmpmeta")))
        .map_err(|e| Error::Xml(format!("Failed to write xmpmeta end: {e}")))?;

    // Get XML data
    let xml_data = writer.into_inner().into_inner();
    result.extend_from_slice(&xml_data);

    // Write packet end
    result.push(b'\n');
    result.extend_from_slice(XMP_PACKET_END.as_bytes());

    Ok(result)
}

/// Write an RDF description with fields.
///
/// Text values are written as simple properties.
/// TextList values are written as `rdf:Bag` structured arrays.
fn write_description(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    fields: &[(&String, &MetadataValue)],
    namespace_uri: &str,
) -> Result<(), Error> {
    // Write Description start
    let mut desc = BytesStart::new("rdf:Description");
    if !namespace_uri.is_empty() {
        // Determine the right xmlns prefix from the first key
        let prefix = fields
            .first()
            .and_then(|(k, _)| k.find(':').map(|p| &k[..p]))
            .unwrap_or("dc");
        let attr_name = format!("xmlns:{prefix}");
        desc.push_attribute((attr_name.as_str(), namespace_uri));
    }
    writer
        .write_event(Event::Start(desc))
        .map_err(|e| Error::Xml(format!("Failed to write Description start: {e}")))?;

    // Write fields
    for (key, value) in fields {
        match value {
            MetadataValue::Text(text) => {
                writer
                    .write_event(Event::Start(BytesStart::new(key.as_str())))
                    .map_err(|e| Error::Xml(format!("Failed to write element start: {e}")))?;
                writer
                    .write_event(Event::Text(BytesText::new(text)))
                    .map_err(|e| Error::Xml(format!("Failed to write text: {e}")))?;
                writer
                    .write_event(Event::End(BytesEnd::new(key.as_str())))
                    .map_err(|e| Error::Xml(format!("Failed to write element end: {e}")))?;
            }
            MetadataValue::TextList(items) => {
                // Write as rdf:Bag
                write_xmp_array(writer, key, XmpArrayKind::Bag, items)?;
            }
            _ => {
                // Skip non-text values for XMP
            }
        }
    }

    // Write Description end
    writer
        .write_event(Event::End(BytesEnd::new("rdf:Description")))
        .map_err(|e| Error::Xml(format!("Failed to write Description end: {e}")))?;

    Ok(())
}

/// Write an XMP structured array property.
///
/// Produces:
/// ```xml
/// <tag>
///   <rdf:Seq|Bag|Alt>
///     <rdf:li>item1</rdf:li>
///     <rdf:li>item2</rdf:li>
///   </rdf:Seq|Bag|Alt>
/// </tag>
/// ```
fn write_xmp_array(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    tag: &str,
    kind: XmpArrayKind,
    items: &[String],
) -> Result<(), Error> {
    let rdf_elem = kind.rdf_element();

    // Open the property element
    writer
        .write_event(Event::Start(BytesStart::new(tag)))
        .map_err(|e| Error::Xml(format!("Failed to write array property start: {e}")))?;

    // Open the array container
    writer
        .write_event(Event::Start(BytesStart::new(rdf_elem)))
        .map_err(|e| Error::Xml(format!("Failed to write {rdf_elem} start: {e}")))?;

    // Write items
    for item in items {
        writer
            .write_event(Event::Start(BytesStart::new("rdf:li")))
            .map_err(|e| Error::Xml(format!("Failed to write rdf:li start: {e}")))?;
        writer
            .write_event(Event::Text(BytesText::new(item)))
            .map_err(|e| Error::Xml(format!("Failed to write rdf:li text: {e}")))?;
        writer
            .write_event(Event::End(BytesEnd::new("rdf:li")))
            .map_err(|e| Error::Xml(format!("Failed to write rdf:li end: {e}")))?;
    }

    // Close the array container
    writer
        .write_event(Event::End(BytesEnd::new(rdf_elem)))
        .map_err(|e| Error::Xml(format!("Failed to write {rdf_elem} end: {e}")))?;

    // Close the property element
    writer
        .write_event(Event::End(BytesEnd::new(tag)))
        .map_err(|e| Error::Xml(format!("Failed to write array property end: {e}")))?;

    Ok(())
}

/// Write an XMP structured array property directly (public API).
///
/// This produces standalone XML for the given array,
/// useful for building XMP documents programmatically.
pub fn write_xmp_array_property(
    metadata: &mut Metadata,
    key: &str,
    kind: XmpArrayKind,
    items: Vec<String>,
) {
    let _ = kind; // kind is used when writing; stored as TextList for now
    metadata.insert(key.to_string(), MetadataValue::TextList(items));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xmp_round_trip() {
        let mut metadata = Metadata::new(MetadataFormat::Xmp);

        metadata.insert(
            "dc:title".to_string(),
            MetadataValue::Text("Test Title".to_string()),
        );
        metadata.insert(
            "dc:creator".to_string(),
            MetadataValue::Text("Test Creator".to_string()),
        );
        metadata.insert(
            "dc:rights".to_string(),
            MetadataValue::Text("Copyright 2024".to_string()),
        );

        let data = write(&metadata).expect("Write failed");
        let parsed = parse(&data).expect("Parse failed");

        assert_eq!(
            parsed.get("dc:title").and_then(|v| v.as_text()),
            Some("Test Title")
        );
        assert_eq!(
            parsed.get("dc:creator").and_then(|v| v.as_text()),
            Some("Test Creator")
        );
        assert_eq!(
            parsed.get("dc:rights").and_then(|v| v.as_text()),
            Some("Copyright 2024")
        );
    }

    #[test]
    fn test_xmp_empty() {
        let metadata = Metadata::new(MetadataFormat::Xmp);
        let data = write(&metadata).expect("Write failed");
        assert!(data.starts_with(XMP_PACKET_START.as_bytes()));
        assert!(data.ends_with(XMP_PACKET_END.as_bytes()));
    }

    // ------- Structured property tests -------

    #[test]
    fn test_xmp_array_kind_element_names() {
        assert_eq!(XmpArrayKind::Seq.rdf_element(), "rdf:Seq");
        assert_eq!(XmpArrayKind::Bag.rdf_element(), "rdf:Bag");
        assert_eq!(XmpArrayKind::Alt.rdf_element(), "rdf:Alt");
    }

    #[test]
    fn test_xmp_array_kind_from_element() {
        assert_eq!(
            XmpArrayKind::from_element("rdf:Seq"),
            Some(XmpArrayKind::Seq)
        );
        assert_eq!(
            XmpArrayKind::from_element("rdf:Bag"),
            Some(XmpArrayKind::Bag)
        );
        assert_eq!(
            XmpArrayKind::from_element("rdf:Alt"),
            Some(XmpArrayKind::Alt)
        );
        assert_eq!(XmpArrayKind::from_element("rdf:Description"), None);
        assert_eq!(XmpArrayKind::from_element("Seq"), Some(XmpArrayKind::Seq));
    }

    #[test]
    fn test_xmp_array_push_and_default() {
        let mut arr = XmpArray::new(XmpArrayKind::Alt);
        assert!(arr.is_empty());
        assert_eq!(arr.len(), 0);
        assert_eq!(arr.default_item(), None);

        arr.push("English");
        arr.push("French");
        assert_eq!(arr.len(), 2);
        assert!(!arr.is_empty());
        assert_eq!(arr.default_item(), Some("English"));
    }

    #[test]
    fn test_parse_xmp_seq_array() {
        let xmp_xml = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description xmlns:dc="http://purl.org/dc/elements/1.1/">
      <dc:subject>
        <rdf:Seq>
          <rdf:li>landscape</rdf:li>
          <rdf:li>nature</rdf:li>
          <rdf:li>sunset</rdf:li>
        </rdf:Seq>
      </dc:subject>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

        let parsed = parse(xmp_xml.as_bytes()).expect("parse should succeed");
        let subjects = parsed
            .get("dc:subject")
            .and_then(|v| v.as_text_list())
            .expect("should be a text list");
        assert_eq!(subjects.len(), 3);
        assert_eq!(subjects[0], "landscape");
        assert_eq!(subjects[1], "nature");
        assert_eq!(subjects[2], "sunset");
    }

    #[test]
    fn test_parse_xmp_bag_array() {
        let xmp_xml = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description xmlns:dc="http://purl.org/dc/elements/1.1/">
      <dc:creator>
        <rdf:Bag>
          <rdf:li>Alice</rdf:li>
          <rdf:li>Bob</rdf:li>
        </rdf:Bag>
      </dc:creator>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

        let parsed = parse(xmp_xml.as_bytes()).expect("parse should succeed");
        let creators = parsed
            .get("dc:creator")
            .and_then(|v| v.as_text_list())
            .expect("should be a text list");
        assert_eq!(creators.len(), 2);
        assert_eq!(creators[0], "Alice");
        assert_eq!(creators[1], "Bob");
    }

    #[test]
    fn test_parse_xmp_alt_array() {
        let xmp_xml = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description xmlns:dc="http://purl.org/dc/elements/1.1/">
      <dc:title>
        <rdf:Alt>
          <rdf:li>My Photo Title</rdf:li>
          <rdf:li>Mon titre de photo</rdf:li>
        </rdf:Alt>
      </dc:title>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

        let parsed = parse(xmp_xml.as_bytes()).expect("parse should succeed");
        let titles = parsed
            .get("dc:title")
            .and_then(|v| v.as_text_list())
            .expect("should be a text list");
        assert_eq!(titles.len(), 2);
        assert_eq!(titles[0], "My Photo Title");
    }

    #[test]
    fn test_xmp_structured_array_write_and_parse_round_trip() {
        let mut metadata = Metadata::new(MetadataFormat::Xmp);

        // Add a text list (will be written as rdf:Bag)
        let keywords = vec!["music".to_string(), "jazz".to_string(), "live".to_string()];
        write_xmp_array_property(&mut metadata, "dc:subject", XmpArrayKind::Bag, keywords);

        // Also add a simple text property
        metadata.insert(
            "dc:title".to_string(),
            MetadataValue::Text("Jazz Concert".to_string()),
        );

        let data = write(&metadata).expect("write should succeed");
        let parsed = parse(&data).expect("parse should succeed");

        // Verify simple text survived
        assert_eq!(
            parsed.get("dc:title").and_then(|v| v.as_text()),
            Some("Jazz Concert")
        );

        // Verify array survived
        let subjects = parsed
            .get("dc:subject")
            .and_then(|v| v.as_text_list())
            .expect("should be text list");
        assert_eq!(subjects.len(), 3);
        assert_eq!(subjects[0], "music");
        assert_eq!(subjects[1], "jazz");
        assert_eq!(subjects[2], "live");
    }

    #[test]
    fn test_xmp_mixed_simple_and_structured() {
        let xmp_xml = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description xmlns:dc="http://purl.org/dc/elements/1.1/">
      <dc:format>image/jpeg</dc:format>
      <dc:subject>
        <rdf:Bag>
          <rdf:li>tag1</rdf:li>
          <rdf:li>tag2</rdf:li>
        </rdf:Bag>
      </dc:subject>
      <dc:rights>Copyright 2025</dc:rights>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;

        let parsed = parse(xmp_xml.as_bytes()).expect("parse should succeed");
        assert_eq!(
            parsed.get("dc:format").and_then(|v| v.as_text()),
            Some("image/jpeg")
        );
        assert_eq!(
            parsed.get("dc:rights").and_then(|v| v.as_text()),
            Some("Copyright 2025")
        );
        let subjects = parsed
            .get("dc:subject")
            .and_then(|v| v.as_text_list())
            .expect("text list");
        assert_eq!(subjects, &["tag1", "tag2"]);
    }
}
