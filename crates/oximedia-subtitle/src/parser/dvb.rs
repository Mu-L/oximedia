//! DVB (Digital Video Broadcasting) subtitle decoder.
//!
//! DVB subtitles are bitmap-based subtitles used in European digital television.
//! They are defined in ETSI EN 300 743.

use crate::{SubtitleError, SubtitleResult};
use std::collections::HashMap;

/// DVB subtitle decoder.
pub struct DvbDecoder {
    /// Page compositions.
    pages: HashMap<u16, PageComposition>,
    /// Region compositions.
    regions: HashMap<u8, RegionComposition>,
    /// CLUT definitions (Color Look-Up Tables).
    cluts: HashMap<u8, Clut>,
    /// Object data.
    objects: HashMap<u16, ObjectData>,
    /// Display definition.
    display_definition: Option<DisplayDefinition>,
    /// Decoded subtitles (as bitmaps).
    subtitles: Vec<DvbSubtitle>,
}

/// A DVB subtitle (bitmap-based).
#[derive(Clone, Debug)]
pub struct DvbSubtitle {
    /// Start time in milliseconds.
    pub start_time: i64,
    /// End time in milliseconds.
    pub end_time: i64,
    /// Page ID.
    pub page_id: u16,
    /// Regions to display.
    pub regions: Vec<RegionDisplay>,
}

/// Region display information.
#[derive(Clone, Debug)]
pub struct RegionDisplay {
    /// Region ID.
    pub region_id: u8,
    /// Horizontal position.
    pub x: u16,
    /// Vertical position.
    pub y: u16,
    /// Region width.
    pub width: u16,
    /// Region height.
    pub height: u16,
    /// Bitmap data (RGBA).
    pub bitmap: Vec<u8>,
}

/// Page composition.
#[derive(Clone, Debug)]
struct PageComposition {
    page_id: u16,
    page_timeout: u8,
    page_version: u8,
    page_state: u8,
    regions: Vec<RegionReference>,
}

/// Reference to a region in a page.
#[derive(Clone, Debug)]
struct RegionReference {
    region_id: u8,
    horizontal_address: u16,
    vertical_address: u16,
}

/// Region composition.
#[derive(Clone, Debug)]
struct RegionComposition {
    region_id: u8,
    region_version: u8,
    region_width: u16,
    region_height: u16,
    region_depth: u8,
    clut_id: u8,
    objects: Vec<ObjectReference>,
}

/// Reference to an object in a region.
#[derive(Clone, Debug)]
struct ObjectReference {
    object_id: u16,
    object_type: u8,
    horizontal_position: u16,
    vertical_position: u16,
    foreground_pixel_code: u8,
    background_pixel_code: u8,
}

/// Color Look-Up Table.
#[derive(Clone, Debug)]
struct Clut {
    clut_id: u8,
    clut_version: u8,
    entries: HashMap<u8, ClutEntry>,
}

/// CLUT entry (color).
#[derive(Clone, Copy, Debug)]
struct ClutEntry {
    r: u8,
    g: u8,
    b: u8,
    t: u8, // Transparency
}

/// Object data (bitmap).
#[derive(Clone, Debug)]
struct ObjectData {
    object_id: u16,
    object_version: u8,
    coding_method: u8,
    non_modifying_color_flag: bool,
    top_field_data: Vec<u8>,
    bottom_field_data: Vec<u8>,
}

/// Display definition.
#[derive(Clone, Debug)]
struct DisplayDefinition {
    width: u16,
    height: u16,
}

impl DvbDecoder {
    /// Create a new DVB subtitle decoder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pages: HashMap::new(),
            regions: HashMap::new(),
            cluts: HashMap::new(),
            objects: HashMap::new(),
            display_definition: None,
            subtitles: Vec::new(),
        }
    }

    /// Decode DVB subtitle segment.
    ///
    /// # Errors
    ///
    /// Returns error if decoding fails.
    pub fn decode_segment(&mut self, data: &[u8], timestamp_ms: i64) -> SubtitleResult<()> {
        if data.len() < 6 {
            return Err(SubtitleError::ParseError(
                "DVB segment too short".to_string(),
            ));
        }

        // Parse segment header
        let sync_byte = data[0];
        if sync_byte != 0x0F {
            return Err(SubtitleError::ParseError(
                "Invalid DVB sync byte".to_string(),
            ));
        }

        let segment_type = data[1];
        let page_id = u16::from_be_bytes([data[2], data[3]]);
        let segment_length = u16::from_be_bytes([data[4], data[5]]) as usize;

        if data.len() < 6 + segment_length {
            return Err(SubtitleError::ParseError(
                "DVB segment data too short".to_string(),
            ));
        }

        let segment_data = &data[6..6 + segment_length];

        match segment_type {
            0x10 => self.decode_page_composition(page_id, segment_data)?,
            0x11 => self.decode_region_composition(segment_data)?,
            0x12 => self.decode_clut_definition(segment_data)?,
            0x13 => self.decode_object_data(segment_data)?,
            0x14 => self.decode_display_definition(segment_data)?,
            0x80 => {
                // End of display set
                self.render_page(page_id, timestamp_ms)?;
            }
            _ => {
                // Unknown segment type - skip
            }
        }

        Ok(())
    }

    /// Decode page composition segment.
    fn decode_page_composition(&mut self, page_id: u16, data: &[u8]) -> SubtitleResult<()> {
        if data.len() < 2 {
            return Ok(());
        }

        let page_timeout = data[0];
        let page_version_state = data[1];
        let page_version = (page_version_state >> 4) & 0x0F;
        let page_state = page_version_state & 0x03;

        let mut regions = Vec::new();
        let mut pos = 2;

        while pos + 6 <= data.len() {
            let region_id = data[pos];
            let horizontal_address = u16::from_be_bytes([data[pos + 2], data[pos + 3]]);
            let vertical_address = u16::from_be_bytes([data[pos + 4], data[pos + 5]]);

            regions.push(RegionReference {
                region_id,
                horizontal_address,
                vertical_address,
            });

            pos += 6;
        }

        let page = PageComposition {
            page_id,
            page_timeout,
            page_version,
            page_state,
            regions,
        };

        self.pages.insert(page_id, page);
        Ok(())
    }

    /// Decode region composition segment.
    fn decode_region_composition(&mut self, data: &[u8]) -> SubtitleResult<()> {
        if data.len() < 10 {
            return Ok(());
        }

        let region_id = data[0];
        let region_version = (data[1] >> 4) & 0x0F;
        let _fill_flag = (data[1] & 0x08) != 0;
        let region_width = u16::from_be_bytes([data[2], data[3]]);
        let region_height = u16::from_be_bytes([data[4], data[5]]);
        let region_level_of_compatibility = (data[6] >> 5) & 0x07;
        let region_depth = (data[6] >> 2) & 0x07;
        let clut_id = data[7];

        let mut objects = Vec::new();
        let mut pos = 10;

        while pos + 6 <= data.len() {
            let object_id = u16::from_be_bytes([data[pos], data[pos + 1]]);
            let object_type = (data[pos + 2] >> 6) & 0x03;
            let object_provider_flag = (data[pos + 2] >> 4) & 0x03;
            let horizontal_position = u16::from_be_bytes([data[pos + 2] & 0x0F, data[pos + 3]]);
            let vertical_position = u16::from_be_bytes([data[pos + 4] & 0x0F, data[pos + 5]]);

            let (foreground_pixel_code, background_pixel_code) =
                if object_type == 0x01 || object_type == 0x02 {
                    if pos + 8 <= data.len() {
                        (data[pos + 6], data[pos + 7])
                    } else {
                        (0, 0)
                    }
                } else {
                    (0, 0)
                };

            objects.push(ObjectReference {
                object_id,
                object_type,
                horizontal_position,
                vertical_position,
                foreground_pixel_code,
                background_pixel_code,
            });

            pos += if object_type == 0x01 || object_type == 0x02 {
                8
            } else {
                6
            };
        }

        let region = RegionComposition {
            region_id,
            region_version,
            region_width,
            region_height,
            region_depth,
            clut_id,
            objects,
        };

        self.regions.insert(region_id, region);
        Ok(())
    }

    /// Decode CLUT definition segment.
    fn decode_clut_definition(&mut self, data: &[u8]) -> SubtitleResult<()> {
        if data.len() < 2 {
            return Ok(());
        }

        let clut_id = data[0];
        let clut_version = (data[1] >> 4) & 0x0F;

        let mut entries = HashMap::new();
        let mut pos = 2;

        while pos + 2 <= data.len() {
            let clut_entry_id = data[pos];
            let clut_entry_flags = data[pos + 1];

            let entry_2bit = (clut_entry_flags & 0x80) != 0;
            let entry_4bit = (clut_entry_flags & 0x40) != 0;
            let entry_8bit = (clut_entry_flags & 0x20) != 0;
            let full_range_flag = (clut_entry_flags & 0x01) != 0;

            if pos + 6 > data.len() {
                break;
            }

            let (y, cr, cb, t) = (data[pos + 2], data[pos + 3], data[pos + 4], data[pos + 5]);

            // Convert YCrCb to RGB
            let (r, g, b) = Self::ycrcb_to_rgb(y, cr, cb);

            entries.insert(
                clut_entry_id,
                ClutEntry {
                    r,
                    g,
                    b,
                    t: 255 - t,
                },
            );

            pos += 6;
        }

        let clut = Clut {
            clut_id,
            clut_version,
            entries,
        };

        self.cluts.insert(clut_id, clut);
        Ok(())
    }

    /// Decode object data segment.
    fn decode_object_data(&mut self, data: &[u8]) -> SubtitleResult<()> {
        if data.len() < 3 {
            return Ok(());
        }

        let object_id = u16::from_be_bytes([data[0], data[1]]);
        let object_version = (data[2] >> 4) & 0x0F;
        let coding_method = (data[2] >> 2) & 0x03;
        let non_modifying_color_flag = (data[2] & 0x02) != 0;

        // For simplicity, store raw data (decoding pixel data would be complex)
        let object = ObjectData {
            object_id,
            object_version,
            coding_method,
            non_modifying_color_flag,
            top_field_data: data[3..].to_vec(),
            bottom_field_data: Vec::new(),
        };

        self.objects.insert(object_id, object);
        Ok(())
    }

    /// Decode display definition segment.
    fn decode_display_definition(&mut self, data: &[u8]) -> SubtitleResult<()> {
        if data.len() < 5 {
            return Ok(());
        }

        let width = u16::from_be_bytes([data[1], data[2]]);
        let height = u16::from_be_bytes([data[3], data[4]]);

        self.display_definition = Some(DisplayDefinition { width, height });
        Ok(())
    }

    /// Render a page to subtitle.
    fn render_page(&mut self, page_id: u16, timestamp_ms: i64) -> SubtitleResult<()> {
        if let Some(page) = self.pages.get(&page_id).cloned() {
            let mut regions_display = Vec::new();

            for region_ref in &page.regions {
                if let Some(region) = self.regions.get(&region_ref.region_id).cloned() {
                    // Create a simple placeholder bitmap (actual rendering would be complex)
                    let width = region.region_width;
                    let height = region.region_height;
                    let bitmap = vec![0u8; (width * height * 4) as usize]; // RGBA

                    regions_display.push(RegionDisplay {
                        region_id: region.region_id,
                        x: region_ref.horizontal_address,
                        y: region_ref.vertical_address,
                        width,
                        height,
                        bitmap,
                    });
                }
            }

            let timeout_ms = i64::from(page.page_timeout) * 1000;
            let end_time = if timeout_ms > 0 {
                timestamp_ms + timeout_ms
            } else {
                timestamp_ms + 5000 // Default 5 seconds
            };

            self.subtitles.push(DvbSubtitle {
                start_time: timestamp_ms,
                end_time,
                page_id,
                regions: regions_display,
            });
        }

        Ok(())
    }

    /// Convert YCrCb to RGB.
    fn ycrcb_to_rgb(y: u8, cr: u8, cb: u8) -> (u8, u8, u8) {
        let y_f = f32::from(y);
        let cr_f = f32::from(cr) - 128.0;
        let cb_f = f32::from(cb) - 128.0;

        let r = (y_f + 1.402 * cr_f).clamp(0.0, 255.0) as u8;
        let g = (y_f - 0.344136 * cb_f - 0.714136 * cr_f).clamp(0.0, 255.0) as u8;
        let b = (y_f + 1.772 * cb_f).clamp(0.0, 255.0) as u8;

        (r, g, b)
    }

    /// Get all decoded subtitles.
    #[must_use]
    pub fn take_subtitles(&mut self) -> Vec<DvbSubtitle> {
        std::mem::take(&mut self.subtitles)
    }

    /// Finalize decoding and return all subtitles.
    #[must_use]
    pub fn finalize(self) -> Vec<DvbSubtitle> {
        self.subtitles
    }
}

impl Default for DvbDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ycrcb_conversion() {
        let (r, g, b) = DvbDecoder::ycrcb_to_rgb(255, 128, 128);
        assert_eq!(r, 255);
        assert_eq!(g, 255);
        assert_eq!(b, 255);
    }

    #[test]
    fn test_decoder_creation() {
        let decoder = DvbDecoder::new();
        assert_eq!(decoder.pages.len(), 0);
        assert_eq!(decoder.regions.len(), 0);
    }
}
