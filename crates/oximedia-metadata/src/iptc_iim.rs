//! IPTC IIM (Information Interchange Model) structured metadata types.
//!
//! Provides typed structs for working with IPTC datasets, records, and builder patterns.

/// An individual IPTC dataset entry consisting of record number, dataset number, and raw data.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct IptcDataset {
    /// IPTC record number (1 = Envelope, 2 = Application, 3 = NewsPhoto).
    pub record: u8,
    /// IPTC dataset number within the record.
    pub dataset: u8,
    /// Raw dataset data bytes.
    pub data: Vec<u8>,
}

impl IptcDataset {
    /// Create a new IPTC dataset.
    #[allow(dead_code)]
    pub fn new(record: u8, dataset: u8, data: Vec<u8>) -> Self {
        Self { record, dataset, data }
    }

    /// Return the (record, dataset) tag pair.
    #[allow(dead_code)]
    pub fn tag(&self) -> (u8, u8) {
        (self.record, self.dataset)
    }

    /// Attempt to decode the dataset data as a UTF-8 string.
    #[allow(dead_code)]
    pub fn as_string(&self) -> Option<String> {
        String::from_utf8(self.data.clone()).ok()
    }
}

/// IPTC record type identifiers.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IptcRecord {
    /// Record 1: Envelope record (routing/service information).
    EnvelopeRecord,
    /// Record 2: Application record (editorial/content metadata).
    ApplicationRecord,
    /// Record 3: News photo record.
    NewsPhotoRecord,
}

impl IptcRecord {
    /// Return the numeric record number for this record type.
    #[allow(dead_code)]
    pub fn record_number(self) -> u8 {
        match self {
            Self::EnvelopeRecord => 1,
            Self::ApplicationRecord => 2,
            Self::NewsPhotoRecord => 3,
        }
    }
}

/// Structured IPTC Application Record (Record 2) metadata.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct IptcApplicationRecord {
    /// Dataset 2:105 - Headline.
    pub headline: Option<String>,
    /// Dataset 2:120 - Caption/Abstract.
    pub caption: Option<String>,
    /// Dataset 2:25 - Keywords (repeatable).
    pub keywords: Vec<String>,
    /// Dataset 2:80 - By-line (photographer/creator).
    pub byline: Option<String>,
    /// Dataset 2:90 - City.
    pub city: Option<String>,
    /// Dataset 2:101 - Country.
    pub country: Option<String>,
    /// Dataset 2:116 - Copyright notice.
    pub copyright: Option<String>,
    /// Dataset 2:10 - Urgency (0-9, where 1 = most urgent).
    pub urgency: u8,
}

impl IptcApplicationRecord {
    /// Serialize this record into a flat list of `IptcDataset` entries.
    #[allow(dead_code)]
    pub fn to_datasets(&self) -> Vec<IptcDataset> {
        let mut datasets = Vec::new();
        let rec = IptcRecord::ApplicationRecord.record_number();

        if let Some(ref h) = self.headline {
            datasets.push(IptcDataset::new(rec, 105, h.as_bytes().to_vec()));
        }
        if let Some(ref c) = self.caption {
            datasets.push(IptcDataset::new(rec, 120, c.as_bytes().to_vec()));
        }
        for kw in &self.keywords {
            datasets.push(IptcDataset::new(rec, 25, kw.as_bytes().to_vec()));
        }
        if let Some(ref b) = self.byline {
            datasets.push(IptcDataset::new(rec, 80, b.as_bytes().to_vec()));
        }
        if let Some(ref c) = self.city {
            datasets.push(IptcDataset::new(rec, 90, c.as_bytes().to_vec()));
        }
        if let Some(ref c) = self.country {
            datasets.push(IptcDataset::new(rec, 101, c.as_bytes().to_vec()));
        }
        if let Some(ref c) = self.copyright {
            datasets.push(IptcDataset::new(rec, 116, c.as_bytes().to_vec()));
        }
        if self.urgency > 0 {
            datasets.push(IptcDataset::new(rec, 10, vec![b'0' + self.urgency.min(9)]));
        }

        datasets
    }

    /// Count the total number of words across headline, caption, and keywords.
    #[allow(dead_code)]
    pub fn word_count(&self) -> usize {
        let mut count = 0;
        if let Some(ref h) = self.headline {
            count += h.split_whitespace().count();
        }
        if let Some(ref c) = self.caption {
            count += c.split_whitespace().count();
        }
        for kw in &self.keywords {
            count += kw.split_whitespace().count();
        }
        count
    }
}

/// Parser for raw IPTC binary data (IIM format with 0x1C tag marker).
#[allow(dead_code)]
pub struct IptcParser;

impl IptcParser {
    /// Parse a byte slice containing IPTC IIM data into a list of datasets.
    ///
    /// Each dataset is expected to be preceded by the 0x1C marker byte, followed
    /// by record number, dataset number, and a 2-byte big-endian length field.
    #[allow(dead_code)]
    pub fn parse_iptc(data: &[u8]) -> Vec<IptcDataset> {
        const MARKER: u8 = 0x1C;
        let mut datasets = Vec::new();
        let mut i = 0;

        while i + 4 < data.len() {
            if data[i] != MARKER {
                i += 1;
                continue;
            }
            let record = data[i + 1];
            let dataset = data[i + 2];
            let length = u16::from_be_bytes([data[i + 3], data[i + 4]]) as usize;
            i += 5;

            if i + length > data.len() {
                break;
            }
            let payload = data[i..i + length].to_vec();
            datasets.push(IptcDataset::new(record, dataset, payload));
            i += length;
        }

        datasets
    }
}

/// Builder for constructing an `IptcApplicationRecord` using a fluent API.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct IptcBuilder {
    record: IptcApplicationRecord,
}

impl IptcBuilder {
    /// Create a new builder with all fields empty.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the headline (dataset 2:105).
    #[allow(dead_code)]
    pub fn headline(mut self, value: impl Into<String>) -> Self {
        self.record.headline = Some(value.into());
        self
    }

    /// Set the caption (dataset 2:120).
    #[allow(dead_code)]
    pub fn caption(mut self, value: impl Into<String>) -> Self {
        self.record.caption = Some(value.into());
        self
    }

    /// Add a keyword (dataset 2:25, repeatable).
    #[allow(dead_code)]
    pub fn add_keyword(mut self, kw: impl Into<String>) -> Self {
        self.record.keywords.push(kw.into());
        self
    }

    /// Set the by-line/creator (dataset 2:80).
    #[allow(dead_code)]
    pub fn byline(mut self, value: impl Into<String>) -> Self {
        self.record.byline = Some(value.into());
        self
    }

    /// Set the copyright notice (dataset 2:116).
    #[allow(dead_code)]
    pub fn copyright(mut self, value: impl Into<String>) -> Self {
        self.record.copyright = Some(value.into());
        self
    }

    /// Set the urgency (dataset 2:10, clamped 1-9).
    #[allow(dead_code)]
    pub fn urgency(mut self, value: u8) -> Self {
        self.record.urgency = value.min(9);
        self
    }

    /// Consume the builder and return the completed `IptcApplicationRecord`.
    #[allow(dead_code)]
    pub fn build(self) -> IptcApplicationRecord {
        self.record
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iptc_dataset_new_and_tag() {
        let ds = IptcDataset::new(2, 80, b"Photographer".to_vec());
        assert_eq!(ds.tag(), (2, 80));
        assert_eq!(ds.record, 2);
        assert_eq!(ds.dataset, 80);
    }

    #[test]
    fn test_iptc_dataset_as_string_valid_utf8() {
        let ds = IptcDataset::new(2, 120, b"Hello World".to_vec());
        assert_eq!(ds.as_string(), Some("Hello World".to_string()));
    }

    #[test]
    fn test_iptc_dataset_as_string_invalid_utf8() {
        let ds = IptcDataset::new(2, 120, vec![0xFF, 0xFE]);
        assert_eq!(ds.as_string(), None);
    }

    #[test]
    fn test_iptc_record_numbers() {
        assert_eq!(IptcRecord::EnvelopeRecord.record_number(), 1);
        assert_eq!(IptcRecord::ApplicationRecord.record_number(), 2);
        assert_eq!(IptcRecord::NewsPhotoRecord.record_number(), 3);
    }

    #[test]
    fn test_iptc_application_record_to_datasets_headline() {
        let rec = IptcApplicationRecord {
            headline: Some("Breaking News".to_string()),
            ..Default::default()
        };
        let datasets = rec.to_datasets();
        assert_eq!(datasets.len(), 1);
        assert_eq!(datasets[0].dataset, 105);
        assert_eq!(datasets[0].as_string().expect("should succeed in test"), "Breaking News");
    }

    #[test]
    fn test_iptc_application_record_keywords() {
        let rec = IptcApplicationRecord {
            keywords: vec!["rust".to_string(), "media".to_string()],
            ..Default::default()
        };
        let datasets = rec.to_datasets();
        let kw_datasets: Vec<_> = datasets.iter().filter(|d| d.dataset == 25).collect();
        assert_eq!(kw_datasets.len(), 2);
    }

    #[test]
    fn test_iptc_application_record_word_count() {
        let rec = IptcApplicationRecord {
            headline: Some("Two Words".to_string()),
            caption: Some("One two three".to_string()),
            keywords: vec!["single".to_string()],
            ..Default::default()
        };
        assert_eq!(rec.word_count(), 6);
    }

    #[test]
    fn test_iptc_application_record_word_count_empty() {
        let rec = IptcApplicationRecord::default();
        assert_eq!(rec.word_count(), 0);
    }

    #[test]
    fn test_iptc_parser_parse_valid() {
        // Build a minimal IPTC IIM block manually
        let mut data = Vec::new();
        let payload = b"TestHeadline";
        data.push(0x1C);
        data.push(2); // record
        data.push(105); // dataset (headline)
        let len = payload.len() as u16;
        data.extend_from_slice(&len.to_be_bytes());
        data.extend_from_slice(payload);

        let datasets = IptcParser::parse_iptc(&data);
        assert_eq!(datasets.len(), 1);
        assert_eq!(datasets[0].tag(), (2, 105));
        assert_eq!(datasets[0].as_string().expect("should succeed in test"), "TestHeadline");
    }

    #[test]
    fn test_iptc_parser_parse_multiple() {
        let mut data = Vec::new();
        for (ds_num, text) in &[(80u8, b"Alice" as &[u8]), (90, b"Paris")] {
            data.push(0x1C);
            data.push(2);
            data.push(*ds_num);
            let len = text.len() as u16;
            data.extend_from_slice(&len.to_be_bytes());
            data.extend_from_slice(text);
        }
        let datasets = IptcParser::parse_iptc(&data);
        assert_eq!(datasets.len(), 2);
        assert_eq!(datasets[0].dataset, 80);
        assert_eq!(datasets[1].dataset, 90);
    }

    #[test]
    fn test_iptc_parser_skips_non_marker_bytes() {
        let mut data = vec![0x00, 0xFF, 0xAB]; // garbage
        let payload = b"skip test";
        data.push(0x1C);
        data.push(2);
        data.push(101);
        let len = payload.len() as u16;
        data.extend_from_slice(&len.to_be_bytes());
        data.extend_from_slice(payload);

        let datasets = IptcParser::parse_iptc(&data);
        assert_eq!(datasets.len(), 1);
        assert_eq!(datasets[0].dataset, 101);
    }

    #[test]
    fn test_iptc_builder_basic() {
        let rec = IptcBuilder::new()
            .headline("Big Story")
            .caption("Details here")
            .add_keyword("news")
            .byline("Jane Doe")
            .copyright("2026 NewsOrg")
            .build();

        assert_eq!(rec.headline.as_deref(), Some("Big Story"));
        assert_eq!(rec.caption.as_deref(), Some("Details here"));
        assert_eq!(rec.keywords, vec!["news"]);
        assert_eq!(rec.byline.as_deref(), Some("Jane Doe"));
        assert_eq!(rec.copyright.as_deref(), Some("2026 NewsOrg"));
    }

    #[test]
    fn test_iptc_builder_multiple_keywords() {
        let rec = IptcBuilder::new()
            .add_keyword("alpha")
            .add_keyword("beta")
            .add_keyword("gamma")
            .build();
        assert_eq!(rec.keywords.len(), 3);
    }

    #[test]
    fn test_iptc_builder_urgency_clamped() {
        let rec = IptcBuilder::new().urgency(15).build();
        assert_eq!(rec.urgency, 9);

        let rec2 = IptcBuilder::new().urgency(5).build();
        assert_eq!(rec2.urgency, 5);
    }

    #[test]
    fn test_iptc_application_record_urgency_dataset() {
        let rec = IptcApplicationRecord {
            urgency: 3,
            ..Default::default()
        };
        let datasets = rec.to_datasets();
        let urg: Vec<_> = datasets.iter().filter(|d| d.dataset == 10).collect();
        assert_eq!(urg.len(), 1);
        assert_eq!(urg[0].data, vec![b'0' + 3]);
    }
}
