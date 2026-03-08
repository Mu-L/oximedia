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

/// XMP packet start marker
const XMP_PACKET_START: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>"#;

/// XMP packet end marker
const XMP_PACKET_END: &str = r#"<?xpacket end="w"?>"#;

/// Parse XMP metadata from XML data.
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
    let mut current_namespace = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                // Extract namespace prefix
                if let Some(colon_pos) = name.find(':') {
                    current_namespace = name[..colon_pos].to_string();
                    current_tag = name.clone();
                }
            }
            Ok(Event::Text(e)) => {
                if !current_tag.is_empty() {
                    let text = reader
                        .decoder()
                        .decode(e.as_ref())
                        .map_err(|e| Error::Xml(format!("Failed to decode text: {e}")))?
                        .to_string();

                    if !text.trim().is_empty() {
                        metadata.insert(current_tag.clone(), MetadataValue::Text(text));
                    }
                }
            }
            Ok(Event::End(_)) => {
                current_tag.clear();
                current_namespace.clear();
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
fn write_description(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    fields: &[(&String, &MetadataValue)],
    namespace_uri: &str,
) -> Result<(), Error> {
    // Write Description start
    let mut desc = BytesStart::new("rdf:Description");
    if !namespace_uri.is_empty() {
        desc.push_attribute(("xmlns:dc", namespace_uri));
    }
    writer
        .write_event(Event::Start(desc))
        .map_err(|e| Error::Xml(format!("Failed to write Description start: {e}")))?;

    // Write fields
    for (key, value) in fields {
        if let Some(text) = value.as_text() {
            // Write element start
            writer
                .write_event(Event::Start(BytesStart::new(key.as_str())))
                .map_err(|e| Error::Xml(format!("Failed to write element start: {e}")))?;

            // Write text
            writer
                .write_event(Event::Text(BytesText::new(text)))
                .map_err(|e| Error::Xml(format!("Failed to write text: {e}")))?;

            // Write element end
            writer
                .write_event(Event::End(BytesEnd::new(key.as_str())))
                .map_err(|e| Error::Xml(format!("Failed to write element end: {e}")))?;
        }
    }

    // Write Description end
    writer
        .write_event(Event::End(BytesEnd::new("rdf:Description")))
        .map_err(|e| Error::Xml(format!("Failed to write Description end: {e}")))?;

    Ok(())
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

        // Write
        let data = write(&metadata).expect("Write failed");

        // Parse
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

        // Write
        let data = write(&metadata).expect("Write failed");

        // Should contain packet markers
        assert!(data.starts_with(XMP_PACKET_START.as_bytes()));
        assert!(data.ends_with(XMP_PACKET_END.as_bytes()));
    }
}
