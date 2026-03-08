//! Matroska tag parsing and writing support.
//!
//! Matroska (MKV) and WebM use XML-based tags.
//!
//! # Format
//!
//! Matroska tags are stored as XML with the following structure:
//! ```xml
//! <Tags>
//!   <Tag>
//!     <Simple>
//!       <Name>TITLE</Name>
//!       <String>My Title</String>
//!     </Simple>
//!   </Tag>
//! </Tags>
//! ```
//!
//! # Common Tags
//!
//! - **TITLE**: Title
//! - **ARTIST**: Artist
//! - **ALBUM**: Album
//! - **DATE_RELEASED**: Release date
//! - **COMMENT**: Comment
//! - **ENCODER**: Encoder software

use crate::{Error, Metadata, MetadataFormat, MetadataValue};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;

/// Parse Matroska tags from XML data.
///
/// # Errors
///
/// Returns an error if the data is not valid Matroska tags.
pub fn parse(data: &[u8]) -> Result<Metadata, Error> {
    let mut metadata = Metadata::new(MetadataFormat::Matroska);
    let mut reader = Reader::from_reader(Cursor::new(data));
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut current_name = String::new();
    let mut in_simple = false;
    let mut in_name = false;
    let mut in_string = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "Simple" => in_simple = true,
                    "Name" if in_simple => in_name = true,
                    "String" if in_simple => in_string = true,
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                let text = reader
                    .decoder()
                    .decode(e.as_ref())
                    .map_err(|e| Error::Xml(format!("Failed to decode text: {e}")))?
                    .to_string();

                if in_name {
                    current_name = text;
                } else if in_string && !current_name.is_empty() {
                    metadata.insert(current_name.clone(), MetadataValue::Text(text));
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "Simple" => {
                        in_simple = false;
                        current_name.clear();
                    }
                    "Name" => in_name = false,
                    "String" => in_string = false,
                    _ => {}
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

/// Write Matroska tags to XML data.
///
/// # Errors
///
/// Returns an error if writing fails.
pub fn write(metadata: &Metadata) -> Result<Vec<u8>, Error> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // Write Tags start
    writer
        .write_event(Event::Start(BytesStart::new("Tags")))
        .map_err(|e| Error::Xml(format!("Failed to write Tags start: {e}")))?;

    // Write Tag start
    writer
        .write_event(Event::Start(BytesStart::new("Tag")))
        .map_err(|e| Error::Xml(format!("Failed to write Tag start: {e}")))?;

    // Write each field as a Simple element
    for (key, value) in metadata.fields() {
        if let Some(text) = value.as_text() {
            // Write Simple start
            writer
                .write_event(Event::Start(BytesStart::new("Simple")))
                .map_err(|e| Error::Xml(format!("Failed to write Simple start: {e}")))?;

            // Write Name
            writer
                .write_event(Event::Start(BytesStart::new("Name")))
                .map_err(|e| Error::Xml(format!("Failed to write Name start: {e}")))?;
            writer
                .write_event(Event::Text(BytesText::new(key)))
                .map_err(|e| Error::Xml(format!("Failed to write Name text: {e}")))?;
            writer
                .write_event(Event::End(BytesEnd::new("Name")))
                .map_err(|e| Error::Xml(format!("Failed to write Name end: {e}")))?;

            // Write String
            writer
                .write_event(Event::Start(BytesStart::new("String")))
                .map_err(|e| Error::Xml(format!("Failed to write String start: {e}")))?;
            writer
                .write_event(Event::Text(BytesText::new(text)))
                .map_err(|e| Error::Xml(format!("Failed to write String text: {e}")))?;
            writer
                .write_event(Event::End(BytesEnd::new("String")))
                .map_err(|e| Error::Xml(format!("Failed to write String end: {e}")))?;

            // Write Simple end
            writer
                .write_event(Event::End(BytesEnd::new("Simple")))
                .map_err(|e| Error::Xml(format!("Failed to write Simple end: {e}")))?;
        }
    }

    // Write Tag end
    writer
        .write_event(Event::End(BytesEnd::new("Tag")))
        .map_err(|e| Error::Xml(format!("Failed to write Tag end: {e}")))?;

    // Write Tags end
    writer
        .write_event(Event::End(BytesEnd::new("Tags")))
        .map_err(|e| Error::Xml(format!("Failed to write Tags end: {e}")))?;

    Ok(writer.into_inner().into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matroska_round_trip() {
        let mut metadata = Metadata::new(MetadataFormat::Matroska);

        metadata.insert(
            "TITLE".to_string(),
            MetadataValue::Text("Test Title".to_string()),
        );
        metadata.insert(
            "ARTIST".to_string(),
            MetadataValue::Text("Test Artist".to_string()),
        );
        metadata.insert(
            "ALBUM".to_string(),
            MetadataValue::Text("Test Album".to_string()),
        );

        // Write
        let data = write(&metadata).expect("Write failed");

        // Parse
        let parsed = parse(&data).expect("Parse failed");

        assert_eq!(
            parsed.get("TITLE").and_then(|v| v.as_text()),
            Some("Test Title")
        );
        assert_eq!(
            parsed.get("ARTIST").and_then(|v| v.as_text()),
            Some("Test Artist")
        );
        assert_eq!(
            parsed.get("ALBUM").and_then(|v| v.as_text()),
            Some("Test Album")
        );
    }

    #[test]
    fn test_matroska_empty() {
        let metadata = Metadata::new(MetadataFormat::Matroska);

        // Write
        let data = write(&metadata).expect("Write failed");

        // Should contain Tags element
        let data_str = String::from_utf8_lossy(&data);
        assert!(data_str.contains("<Tags>"));
        assert!(data_str.contains("</Tags>"));
    }
}
