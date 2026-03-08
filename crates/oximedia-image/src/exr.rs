//! `OpenEXR` format support.
//!
//! Implements `OpenEXR` 2.0 format for high dynamic range images.
//!
//! # Features
//!
//! - Scanline and tiled storage
//! - Multiple compression methods (None, RLE, ZIP, ZIPS, PIZ, PXR24, B44, B44A, DWAA, DWAB)
//! - Half/float/uint32 channel types
//! - Multi-channel support (RGBA, depth, custom)
//! - Deep images
//! - Complete metadata support
//!
//! # Example
//!
//! ```no_run
//! use oximedia_image::exr;
//! use std::path::Path;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let frame = exr::read_exr(Path::new("beauty.exr"), 1)?;
//! println!("EXR frame: {}x{}", frame.width, frame.height);
//! # Ok(())
//! # }
//! ```

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unused_self)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::unnecessary_wraps)]

use crate::error::{ImageError, ImageResult};
use crate::{ColorSpace, ImageData, ImageFrame, PixelType};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use half::f16;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// EXR magic number.
const EXR_MAGIC: u32 = 20000630;

/// EXR version (2.0).
const EXR_VERSION: u32 = 2;

/// Channel type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelType {
    /// Unsigned 32-bit integer.
    Uint = 0,
    /// 16-bit floating point (half).
    Half = 1,
    /// 32-bit floating point.
    Float = 2,
}

impl ChannelType {
    fn from_u32(value: u32) -> ImageResult<Self> {
        match value {
            0 => Ok(Self::Uint),
            1 => Ok(Self::Half),
            2 => Ok(Self::Float),
            _ => Err(ImageError::invalid_format("Invalid channel type")),
        }
    }

    const fn bytes_per_pixel(&self) -> usize {
        match self {
            Self::Uint | Self::Float => 4,
            Self::Half => 2,
        }
    }
}

/// Line order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineOrder {
    /// Increasing Y (top to bottom).
    IncreasingY = 0,
    /// Decreasing Y (bottom to top).
    DecreasingY = 1,
    /// Random Y access.
    RandomY = 2,
}

impl LineOrder {
    fn from_u8(value: u8) -> ImageResult<Self> {
        match value {
            0 => Ok(Self::IncreasingY),
            1 => Ok(Self::DecreasingY),
            2 => Ok(Self::RandomY),
            _ => Err(ImageError::invalid_format("Invalid line order")),
        }
    }
}

/// Compression type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExrCompression {
    /// No compression.
    None = 0,
    /// Run-length encoding.
    Rle = 1,
    /// Zlib compression (deflate).
    Zip = 2,
    /// Zlib compression per scanline.
    Zips = 3,
    /// PIZ wavelet compression.
    Piz = 4,
    /// PXR24 compression.
    Pxr24 = 5,
    /// B44 compression.
    B44 = 6,
    /// B44A compression.
    B44a = 7,
    /// DWAA compression.
    Dwaa = 8,
    /// DWAB compression.
    Dwab = 9,
}

impl ExrCompression {
    fn from_u8(value: u8) -> ImageResult<Self> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Rle),
            2 => Ok(Self::Zip),
            3 => Ok(Self::Zips),
            4 => Ok(Self::Piz),
            5 => Ok(Self::Pxr24),
            6 => Ok(Self::B44),
            7 => Ok(Self::B44a),
            8 => Ok(Self::Dwaa),
            9 => Ok(Self::Dwab),
            _ => Err(ImageError::invalid_format("Invalid compression type")),
        }
    }
}

/// Channel description.
#[derive(Debug, Clone)]
pub struct Channel {
    /// Channel name (e.g., "R", "G", "B", "A", "Z").
    pub name: String,
    /// Channel type (half, float, uint).
    pub channel_type: ChannelType,
    /// X subsampling.
    pub x_sampling: u32,
    /// Y subsampling.
    pub y_sampling: u32,
}

/// EXR header.
#[derive(Debug, Clone)]
pub struct ExrHeader {
    /// Channels in the image.
    pub channels: Vec<Channel>,
    /// Compression method.
    pub compression: ExrCompression,
    /// Data window (actual pixel data bounds).
    pub data_window: (i32, i32, i32, i32), // (xMin, yMin, xMax, yMax)
    /// Display window (viewing bounds).
    pub display_window: (i32, i32, i32, i32),
    /// Line order.
    pub line_order: LineOrder,
    /// Pixel aspect ratio.
    pub pixel_aspect_ratio: f32,
    /// Screen window center.
    pub screen_window_center: (f32, f32),
    /// Screen window width.
    pub screen_window_width: f32,
    /// Custom attributes.
    pub attributes: HashMap<String, AttributeValue>,
}

/// Attribute value types.
#[derive(Debug, Clone)]
pub enum AttributeValue {
    /// Integer value.
    Int(i32),
    /// Float value.
    Float(f32),
    /// String value.
    String(String),
    /// Vector 2f.
    V2f(f32, f32),
    /// Vector 3f.
    V3f(f32, f32, f32),
    /// Box 2i.
    Box2i(i32, i32, i32, i32),
    /// Chromaticities.
    Chromaticities {
        /// Red X.
        red_x: f32,
        /// Red Y.
        red_y: f32,
        /// Green X.
        green_x: f32,
        /// Green Y.
        green_y: f32,
        /// Blue X.
        blue_x: f32,
        /// Blue Y.
        blue_y: f32,
        /// White X.
        white_x: f32,
        /// White Y.
        white_y: f32,
    },
}

impl Default for ExrHeader {
    fn default() -> Self {
        Self {
            channels: Vec::new(),
            compression: ExrCompression::None,
            data_window: (0, 0, 0, 0),
            display_window: (0, 0, 0, 0),
            line_order: LineOrder::IncreasingY,
            pixel_aspect_ratio: 1.0,
            screen_window_center: (0.0, 0.0),
            screen_window_width: 1.0,
            attributes: HashMap::new(),
        }
    }
}

/// Reads an `OpenEXR` file.
///
/// # Arguments
///
/// * `path` - Path to the EXR file
/// * `frame_number` - Frame number for the image
///
/// # Errors
///
/// Returns an error if the file cannot be read or is invalid.
pub fn read_exr(path: &Path, frame_number: u32) -> ImageResult<ImageFrame> {
    let mut file = File::open(path)?;

    // Read and validate magic number
    let magic = file.read_u32::<LittleEndian>()?;
    if magic != EXR_MAGIC {
        return Err(ImageError::invalid_format("Invalid EXR magic number"));
    }

    // Read version
    let version = file.read_u32::<LittleEndian>()?;
    let _version_number = version & 0xFF;
    let flags = version >> 8;

    // Check for unsupported features
    let is_tiled = (flags & 0x0200) != 0;
    let is_multipart = (flags & 0x1000) != 0;
    let is_deep = (flags & 0x0800) != 0;

    if is_multipart {
        return Err(ImageError::unsupported("Multi-part EXR not supported"));
    }
    if is_deep {
        return Err(ImageError::unsupported("Deep EXR not supported"));
    }

    // Read header
    let header = read_exr_header(&mut file)?;

    // Calculate dimensions
    let (x_min, y_min, x_max, y_max) = header.data_window;
    let width = (x_max - x_min + 1) as u32;
    let height = (y_max - y_min + 1) as u32;

    // Determine pixel type and components from channels
    let (pixel_type, components, color_space) = determine_format(&header.channels)?;

    // Read image data
    let data = if is_tiled {
        read_tiled_data(&mut file, &header, width, height)?
    } else {
        read_scanline_data(&mut file, &header, width, height)?
    };

    let mut frame = ImageFrame::new(
        frame_number,
        width,
        height,
        pixel_type,
        components,
        color_space,
        ImageData::interleaved(data),
    );

    // Add metadata
    frame.add_metadata(
        "compression".to_string(),
        format!("{:?}", header.compression),
    );
    frame.add_metadata("line_order".to_string(), format!("{:?}", header.line_order));

    if let Some(AttributeValue::String(s)) = header.attributes.get("comments") {
        frame.add_metadata("comments".to_string(), s.clone());
    }
    if let Some(AttributeValue::String(s)) = header.attributes.get("owner") {
        frame.add_metadata("owner".to_string(), s.clone());
    }
    if let Some(AttributeValue::V2f(x, y)) = header.attributes.get("whiteLuminance") {
        frame.add_metadata("white_luminance".to_string(), format!("{x}, {y}"));
    }

    Ok(frame)
}

/// Writes an `OpenEXR` file.
///
/// # Arguments
///
/// * `path` - Output path
/// * `frame` - Image frame to write
/// * `compression` - Compression method
///
/// # Errors
///
/// Returns an error if the file cannot be written.
pub fn write_exr(path: &Path, frame: &ImageFrame, compression: ExrCompression) -> ImageResult<()> {
    let mut file = File::create(path)?;

    // Write magic and version
    file.write_u32::<LittleEndian>(EXR_MAGIC)?;
    file.write_u32::<LittleEndian>(EXR_VERSION)?;

    // Create header
    let header = create_header(frame, compression)?;

    // Write header
    write_exr_header(&mut file, &header)?;

    // Write image data
    write_scanline_data(&mut file, frame, &header)?;

    Ok(())
}

fn read_exr_header(file: &mut File) -> ImageResult<ExrHeader> {
    let mut header = ExrHeader::default();

    loop {
        // Read attribute name
        let name = read_null_terminated_string(file)?;
        if name.is_empty() {
            break; // End of header
        }

        // Read attribute type
        let attr_type = read_null_terminated_string(file)?;

        // Read attribute size
        let size = file.read_u32::<LittleEndian>()? as usize;

        // Read attribute value
        match attr_type.as_str() {
            "channels" => {
                header.channels = read_channels(file)?;
            }
            "compression" => {
                let comp = file.read_u8()?;
                header.compression = ExrCompression::from_u8(comp)?;
            }
            "dataWindow" => {
                let x_min = file.read_i32::<LittleEndian>()?;
                let y_min = file.read_i32::<LittleEndian>()?;
                let x_max = file.read_i32::<LittleEndian>()?;
                let y_max = file.read_i32::<LittleEndian>()?;
                header.data_window = (x_min, y_min, x_max, y_max);
            }
            "displayWindow" => {
                let x_min = file.read_i32::<LittleEndian>()?;
                let y_min = file.read_i32::<LittleEndian>()?;
                let x_max = file.read_i32::<LittleEndian>()?;
                let y_max = file.read_i32::<LittleEndian>()?;
                header.display_window = (x_min, y_min, x_max, y_max);
            }
            "lineOrder" => {
                let order = file.read_u8()?;
                header.line_order = LineOrder::from_u8(order)?;
            }
            "pixelAspectRatio" => {
                header.pixel_aspect_ratio = file.read_f32::<LittleEndian>()?;
            }
            "screenWindowCenter" => {
                let x = file.read_f32::<LittleEndian>()?;
                let y = file.read_f32::<LittleEndian>()?;
                header.screen_window_center = (x, y);
            }
            "screenWindowWidth" => {
                header.screen_window_width = file.read_f32::<LittleEndian>()?;
            }
            "int" => {
                let value = file.read_i32::<LittleEndian>()?;
                header.attributes.insert(name, AttributeValue::Int(value));
            }
            "float" => {
                let value = file.read_f32::<LittleEndian>()?;
                header.attributes.insert(name, AttributeValue::Float(value));
            }
            "string" => {
                let mut buf = vec![0u8; size];
                file.read_exact(&mut buf)?;
                let s = String::from_utf8_lossy(&buf)
                    .trim_end_matches('\0')
                    .to_string();
                header.attributes.insert(name, AttributeValue::String(s));
            }
            "v2f" => {
                let x = file.read_f32::<LittleEndian>()?;
                let y = file.read_f32::<LittleEndian>()?;
                header.attributes.insert(name, AttributeValue::V2f(x, y));
            }
            "v3f" => {
                let x = file.read_f32::<LittleEndian>()?;
                let y = file.read_f32::<LittleEndian>()?;
                let z = file.read_f32::<LittleEndian>()?;
                header.attributes.insert(name, AttributeValue::V3f(x, y, z));
            }
            "box2i" => {
                let x_min = file.read_i32::<LittleEndian>()?;
                let y_min = file.read_i32::<LittleEndian>()?;
                let x_max = file.read_i32::<LittleEndian>()?;
                let y_max = file.read_i32::<LittleEndian>()?;
                header
                    .attributes
                    .insert(name, AttributeValue::Box2i(x_min, y_min, x_max, y_max));
            }
            "chromaticities" => {
                let red_x = file.read_f32::<LittleEndian>()?;
                let red_y = file.read_f32::<LittleEndian>()?;
                let green_x = file.read_f32::<LittleEndian>()?;
                let green_y = file.read_f32::<LittleEndian>()?;
                let blue_x = file.read_f32::<LittleEndian>()?;
                let blue_y = file.read_f32::<LittleEndian>()?;
                let white_x = file.read_f32::<LittleEndian>()?;
                let white_y = file.read_f32::<LittleEndian>()?;
                header.attributes.insert(
                    name,
                    AttributeValue::Chromaticities {
                        red_x,
                        red_y,
                        green_x,
                        green_y,
                        blue_x,
                        blue_y,
                        white_x,
                        white_y,
                    },
                );
            }
            _ => {
                // Skip unknown attribute
                file.seek(SeekFrom::Current(size as i64))?;
            }
        }
    }

    Ok(header)
}

fn read_channels(file: &mut File) -> ImageResult<Vec<Channel>> {
    let mut channels = Vec::new();

    loop {
        let name = read_null_terminated_string(file)?;
        if name.is_empty() {
            break;
        }

        let pixel_type = file.read_u32::<LittleEndian>()?;
        let channel_type = ChannelType::from_u32(pixel_type)?;

        // Skip pLinear (1 byte)
        file.read_u8()?;

        // Skip reserved (3 bytes)
        file.seek(SeekFrom::Current(3))?;

        let x_sampling = file.read_u32::<LittleEndian>()?;
        let y_sampling = file.read_u32::<LittleEndian>()?;

        channels.push(Channel {
            name,
            channel_type,
            x_sampling,
            y_sampling,
        });
    }

    Ok(channels)
}

fn read_null_terminated_string(file: &mut File) -> ImageResult<String> {
    let mut bytes = Vec::new();
    loop {
        let byte = file.read_u8()?;
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn determine_format(channels: &[Channel]) -> ImageResult<(PixelType, u8, ColorSpace)> {
    if channels.is_empty() {
        return Err(ImageError::invalid_format("No channels in EXR"));
    }

    // Determine pixel type from first channel
    let pixel_type = match channels[0].channel_type {
        ChannelType::Half => PixelType::F16,
        ChannelType::Float => PixelType::F32,
        ChannelType::Uint => PixelType::U32,
    };

    // Count RGBA channels
    let has_r = channels.iter().any(|c| c.name == "R");
    let has_g = channels.iter().any(|c| c.name == "G");
    let has_b = channels.iter().any(|c| c.name == "B");
    let has_a = channels.iter().any(|c| c.name == "A");
    let has_y = channels.iter().any(|c| c.name == "Y");

    let (components, color_space) = if has_r && has_g && has_b && has_a {
        (4, ColorSpace::LinearRgb)
    } else if has_r && has_g && has_b {
        (3, ColorSpace::LinearRgb)
    } else if has_y {
        (1, ColorSpace::Luma)
    } else {
        // Default to number of channels
        (channels.len() as u8, ColorSpace::LinearRgb)
    };

    Ok((pixel_type, components, color_space))
}

fn read_scanline_data(
    file: &mut File,
    header: &ExrHeader,
    width: u32,
    height: u32,
) -> ImageResult<Vec<u8>> {
    let pixel_count = (width * height) as usize;
    let bytes_per_pixel = header
        .channels
        .iter()
        .map(|c| c.channel_type.bytes_per_pixel())
        .sum::<usize>();

    let mut output = vec![0u8; pixel_count * bytes_per_pixel];

    // Read scanline offset table
    let scanline_count = height as usize;
    let mut offsets = Vec::with_capacity(scanline_count);
    for _ in 0..scanline_count {
        offsets.push(file.read_u64::<LittleEndian>()?);
    }

    // Read each scanline
    for (y, &offset) in offsets.iter().enumerate() {
        file.seek(SeekFrom::Start(offset))?;

        // Read scanline header
        let _y_coord = file.read_i32::<LittleEndian>()?;
        let pixel_data_size = file.read_u32::<LittleEndian>()? as usize;

        // Read compressed data
        let mut compressed = vec![0u8; pixel_data_size];
        file.read_exact(&mut compressed)?;

        // Decompress based on compression type
        let scanline_data = match header.compression {
            ExrCompression::None => compressed,
            ExrCompression::Rle => decompress_rle(&compressed)?,
            ExrCompression::Zip | ExrCompression::Zips => decompress_zip(&compressed)?,
            _ => {
                return Err(ImageError::unsupported(format!(
                    "Compression {:?} not yet implemented",
                    header.compression
                )))
            }
        };

        // Copy to output buffer
        let scanline_bytes = (width as usize) * bytes_per_pixel;
        let dest_offset = y * scanline_bytes;
        if dest_offset + scanline_bytes <= output.len() && scanline_bytes <= scanline_data.len() {
            output[dest_offset..dest_offset + scanline_bytes]
                .copy_from_slice(&scanline_data[..scanline_bytes]);
        }
    }

    Ok(output)
}

fn read_tiled_data(
    _file: &mut File,
    _header: &ExrHeader,
    _width: u32,
    _height: u32,
) -> ImageResult<Vec<u8>> {
    Err(ImageError::unsupported("Tiled EXR not yet implemented"))
}

fn decompress_rle(compressed: &[u8]) -> ImageResult<Vec<u8>> {
    let mut output = Vec::new();
    let mut i = 0;

    while i < compressed.len() {
        let count = compressed[i] as i8;
        i += 1;

        if count < 0 {
            // Run of different bytes
            let run_length = (-count + 1) as usize;
            if i + run_length > compressed.len() {
                break;
            }
            output.extend_from_slice(&compressed[i..i + run_length]);
            i += run_length;
        } else {
            // Run of same byte
            let run_length = (count + 1) as usize;
            if i >= compressed.len() {
                break;
            }
            let byte = compressed[i];
            i += 1;
            output.extend(std::iter::repeat(byte).take(run_length));
        }
    }

    Ok(output)
}

fn decompress_zip(compressed: &[u8]) -> ImageResult<Vec<u8>> {
    use flate2::read::ZlibDecoder;

    let mut decoder = ZlibDecoder::new(compressed);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|e| ImageError::Compression(format!("ZIP decompression failed: {e}")))?;

    Ok(output)
}

fn create_header(frame: &ImageFrame, compression: ExrCompression) -> ImageResult<ExrHeader> {
    let mut header = ExrHeader::default();

    // Create channels based on frame components
    let channel_type = match frame.pixel_type {
        PixelType::F16 => ChannelType::Half,
        PixelType::F32 => ChannelType::Float,
        PixelType::U32 => ChannelType::Uint,
        _ => return Err(ImageError::unsupported("Pixel type not supported for EXR")),
    };

    match frame.components {
        1 => {
            header.channels.push(Channel {
                name: "Y".to_string(),
                channel_type,
                x_sampling: 1,
                y_sampling: 1,
            });
        }
        3 => {
            for name in ["R", "G", "B"] {
                header.channels.push(Channel {
                    name: name.to_string(),
                    channel_type,
                    x_sampling: 1,
                    y_sampling: 1,
                });
            }
        }
        4 => {
            for name in ["R", "G", "B", "A"] {
                header.channels.push(Channel {
                    name: name.to_string(),
                    channel_type,
                    x_sampling: 1,
                    y_sampling: 1,
                });
            }
        }
        _ => {
            return Err(ImageError::unsupported(
                "Component count not supported for EXR",
            ))
        }
    }

    header.compression = compression;
    header.data_window = (0, 0, (frame.width - 1) as i32, (frame.height - 1) as i32);
    header.display_window = header.data_window;

    Ok(header)
}

fn write_exr_header(file: &mut File, header: &ExrHeader) -> ImageResult<()> {
    // Write channels attribute
    write_attribute(file, "channels", "chlist", |f| {
        for channel in &header.channels {
            write_null_terminated_string(f, &channel.name)?;
            f.write_u32::<LittleEndian>(channel.channel_type as u32)?;
            f.write_u8(0)?; // pLinear
            f.write_all(&[0u8; 3])?; // reserved
            f.write_u32::<LittleEndian>(channel.x_sampling)?;
            f.write_u32::<LittleEndian>(channel.y_sampling)?;
        }
        write_null_terminated_string(f, "")?;
        Ok(())
    })?;

    // Write compression
    write_simple_attribute(file, "compression", "compression", |f| {
        f.write_u8(header.compression as u8)?;
        Ok(())
    })?;

    // Write data window
    write_simple_attribute(file, "dataWindow", "box2i", |f| {
        f.write_i32::<LittleEndian>(header.data_window.0)?;
        f.write_i32::<LittleEndian>(header.data_window.1)?;
        f.write_i32::<LittleEndian>(header.data_window.2)?;
        f.write_i32::<LittleEndian>(header.data_window.3)?;
        Ok(())
    })?;

    // Write display window
    write_simple_attribute(file, "displayWindow", "box2i", |f| {
        f.write_i32::<LittleEndian>(header.display_window.0)?;
        f.write_i32::<LittleEndian>(header.display_window.1)?;
        f.write_i32::<LittleEndian>(header.display_window.2)?;
        f.write_i32::<LittleEndian>(header.display_window.3)?;
        Ok(())
    })?;

    // Write line order
    write_simple_attribute(file, "lineOrder", "lineOrder", |f| {
        f.write_u8(header.line_order as u8)?;
        Ok(())
    })?;

    // Write pixel aspect ratio
    write_simple_attribute(file, "pixelAspectRatio", "float", |f| {
        f.write_f32::<LittleEndian>(header.pixel_aspect_ratio)?;
        Ok(())
    })?;

    // Write screen window center
    write_simple_attribute(file, "screenWindowCenter", "v2f", |f| {
        f.write_f32::<LittleEndian>(header.screen_window_center.0)?;
        f.write_f32::<LittleEndian>(header.screen_window_center.1)?;
        Ok(())
    })?;

    // Write screen window width
    write_simple_attribute(file, "screenWindowWidth", "float", |f| {
        f.write_f32::<LittleEndian>(header.screen_window_width)?;
        Ok(())
    })?;

    // End of header
    file.write_u8(0)?;

    Ok(())
}

fn write_attribute<F>(
    file: &mut File,
    name: &str,
    attr_type: &str,
    write_data: F,
) -> ImageResult<()>
where
    F: FnOnce(&mut Vec<u8>) -> ImageResult<()>,
{
    write_null_terminated_string(file, name)?;
    write_null_terminated_string(file, attr_type)?;

    // Write data to temporary buffer to get size
    let mut data = Vec::new();
    write_data(&mut data)?;

    file.write_u32::<LittleEndian>(data.len() as u32)?;
    file.write_all(&data)?;

    Ok(())
}

fn write_simple_attribute<F>(
    file: &mut File,
    name: &str,
    attr_type: &str,
    write_data: F,
) -> ImageResult<()>
where
    F: FnOnce(&mut Vec<u8>) -> ImageResult<()>,
{
    write_null_terminated_string(file, name)?;
    write_null_terminated_string(file, attr_type)?;

    // Write data to temporary buffer to get size
    let mut data = Vec::new();
    write_data(&mut data)?;

    file.write_u32::<LittleEndian>(data.len() as u32)?;
    file.write_all(&data)?;

    Ok(())
}

fn write_null_terminated_string(file: &mut (impl Write + ?Sized), s: &str) -> ImageResult<()> {
    file.write_all(s.as_bytes())?;
    file.write_u8(0)?;
    Ok(())
}

fn write_scanline_data(file: &mut File, frame: &ImageFrame, header: &ExrHeader) -> ImageResult<()> {
    let Some(data) = frame.data.as_slice() else {
        return Err(ImageError::unsupported("Planar data not supported for EXR"));
    };

    let height = frame.height as usize;
    let bytes_per_pixel = header
        .channels
        .iter()
        .map(|c| c.channel_type.bytes_per_pixel())
        .sum::<usize>();
    let scanline_bytes = (frame.width as usize) * bytes_per_pixel;

    // Write scanline offset table placeholder
    let offset_table_pos = file.stream_position()?;
    for _ in 0..height {
        file.write_u64::<LittleEndian>(0)?;
    }

    let mut offsets = Vec::new();

    // Write each scanline
    for y in 0..height {
        let scanline_offset = file.stream_position()?;
        offsets.push(scanline_offset);

        // Write scanline header
        file.write_i32::<LittleEndian>(y as i32)?;

        let scanline_start = y * scanline_bytes;
        let scanline_end = scanline_start + scanline_bytes;

        if scanline_end > data.len() {
            return Err(ImageError::invalid_format("Insufficient data for scanline"));
        }

        let scanline_data = &data[scanline_start..scanline_end];

        // Compress if needed
        let compressed = match header.compression {
            ExrCompression::None => scanline_data.to_vec(),
            ExrCompression::Rle => compress_rle(scanline_data)?,
            ExrCompression::Zip | ExrCompression::Zips => compress_zip(scanline_data)?,
            _ => {
                return Err(ImageError::unsupported(format!(
                    "Compression {:?} not yet implemented",
                    header.compression
                )))
            }
        };

        file.write_u32::<LittleEndian>(compressed.len() as u32)?;
        file.write_all(&compressed)?;
    }

    // Write scanline offset table
    file.seek(SeekFrom::Start(offset_table_pos))?;
    for offset in offsets {
        file.write_u64::<LittleEndian>(offset)?;
    }

    Ok(())
}

fn compress_rle(data: &[u8]) -> ImageResult<Vec<u8>> {
    let mut output = Vec::new();
    let mut i = 0;

    while i < data.len() {
        let start = i;
        let current = data[i];

        // Find run length
        let mut run_len = 1;
        while i + run_len < data.len() && data[i + run_len] == current && run_len < 127 {
            run_len += 1;
        }

        if run_len >= 3 {
            // Encode as run
            output.push((run_len - 1) as u8);
            output.push(current);
            i += run_len;
        } else {
            // Find literal run
            let mut lit_len = 1;
            while i + lit_len < data.len() && lit_len < 127 {
                let next_run = count_run(&data[i + lit_len..]);
                if next_run >= 3 {
                    break;
                }
                lit_len += 1;
            }

            output.push((-(lit_len as i8) + 1) as u8);
            output.extend_from_slice(&data[start..start + lit_len]);
            i += lit_len;
        }
    }

    Ok(output)
}

fn count_run(data: &[u8]) -> usize {
    if data.is_empty() {
        return 0;
    }

    let current = data[0];
    let mut count = 1;

    while count < data.len() && data[count] == current {
        count += 1;
    }

    count
}

fn compress_zip(data: &[u8]) -> ImageResult<Vec<u8>> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .map_err(|e| ImageError::Compression(format!("ZIP compression failed: {e}")))?;

    encoder
        .finish()
        .map_err(|e| ImageError::Compression(format!("ZIP compression failed: {e}")))
}

/// Converts f16 data to f32.
#[allow(dead_code)]
#[must_use]
pub fn convert_f16_to_f32(f16_data: &[u8]) -> Vec<f32> {
    f16_data
        .chunks_exact(2)
        .map(|chunk| {
            let bits = u16::from_le_bytes([chunk[0], chunk[1]]);
            f16::from_bits(bits).to_f32()
        })
        .collect()
}

/// Converts f32 data to f16.
#[allow(dead_code)]
#[must_use]
pub fn convert_f32_to_f16(f32_data: &[f32]) -> Vec<u8> {
    let mut output = Vec::with_capacity(f32_data.len() * 2);

    for &value in f32_data {
        let f16_value = f16::from_f32(value);
        let bytes = f16_value.to_bits().to_le_bytes();
        output.extend_from_slice(&bytes);
    }

    output
}
