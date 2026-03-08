//! SubStation Alpha (SSA) and Advanced SubStation Alpha (ASS) parser.
//!
//! SSA/ASS is a complex subtitle format with extensive styling support.
//!
//! ```text
//! [Script Info]
//! Title: Movie Subtitles
//! ScriptType: v4.00+
//!
//! [V4+ Styles]
//! Format: Name, Fontname, Fontsize, PrimaryColour, ...
//! Style: Default,Arial,48,&H00FFFFFF,&H000000FF,...
//!
//! [Events]
//! Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
//! Dialogue: 0,0:00:01.00,0:00:04.00,Default,,0,0,0,,Hello world!
//! ```

use crate::style::{Alignment, Color, FontWeight, OutlineStyle, Position, ShadowStyle};
use crate::{Subtitle, SubtitleError, SubtitleResult, SubtitleStyle};
use std::collections::HashMap;

/// SSA/ASS subtitle file.
#[derive(Clone, Debug)]
pub struct AssFile {
    /// Script metadata.
    pub script_info: HashMap<String, String>,
    /// Named styles.
    pub styles: HashMap<String, SubtitleStyle>,
    /// Subtitle events.
    pub events: Vec<Subtitle>,
}

/// Parse SSA/ASS subtitle file.
///
/// # Errors
///
/// Returns error if the file is not valid SSA/ASS format.
pub fn parse(data: &[u8]) -> SubtitleResult<Vec<Subtitle>> {
    let text = String::from_utf8_lossy(data);
    let file = parse_ass(&text)?;
    Ok(file.events)
}

/// Parse SSA/ASS subtitle from string.
///
/// # Errors
///
/// Returns error if parsing fails.
pub fn parse_ass(input: &str) -> SubtitleResult<AssFile> {
    let normalized = input.replace("\r\n", "\n");

    let mut script_info = HashMap::new();
    let mut styles = HashMap::new();
    let mut events = Vec::new();

    let mut current_section = String::new();
    let mut style_format = Vec::new();
    let mut event_format = Vec::new();

    for line in normalized.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        // Section headers
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].to_string();
            continue;
        }

        match current_section.as_str() {
            "Script Info" => {
                if let Some((key, value)) = parse_key_value(line) {
                    script_info.insert(key, value);
                }
            }
            "V4+ Styles" | "V4 Styles" => {
                if line.starts_with("Format:") {
                    style_format = parse_format_line(line);
                } else if line.starts_with("Style:") {
                    if let Some(style) = parse_style_line(line, &style_format) {
                        styles.insert(style.0, style.1);
                    }
                }
            }
            "Events" => {
                if line.starts_with("Format:") {
                    event_format = parse_format_line(line);
                } else if line.starts_with("Dialogue:") || line.starts_with("Comment:") {
                    if let Some(event) = parse_event_line(line, &event_format, &styles) {
                        events.push(event);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(AssFile {
        script_info,
        styles,
        events,
    })
}

/// Parse key-value pair.
fn parse_key_value(line: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = line.splitn(2, ':').collect();
    if parts.len() == 2 {
        Some((parts[0].trim().to_string(), parts[1].trim().to_string()))
    } else {
        None
    }
}

/// Parse format line.
fn parse_format_line(line: &str) -> Vec<String> {
    let content = line.strip_prefix("Format:").unwrap_or(line);
    content.split(',').map(|s| s.trim().to_string()).collect()
}

/// Parse style line.
fn parse_style_line(line: &str, format: &[String]) -> Option<(String, SubtitleStyle)> {
    let content = line.strip_prefix("Style:")?;
    let values: Vec<&str> = content.split(',').map(str::trim).collect();

    let mut style_map = HashMap::new();
    for (i, field) in format.iter().enumerate() {
        if let Some(&value) = values.get(i) {
            style_map.insert(field.clone(), value.to_string());
        }
    }

    let name = style_map.get("Name")?.clone();
    let mut style = SubtitleStyle::default();

    // Font name (ignored - we use the provided font)
    // Fontsize
    if let Some(size) = style_map.get("Fontsize") {
        if let Ok(size) = size.parse::<f32>() {
            style.font_size = size;
        }
    }

    // Primary color
    if let Some(color) = style_map.get("PrimaryColour") {
        if let Ok(c) = parse_ass_color(color) {
            style.primary_color = c;
        }
    }

    // Secondary color
    if let Some(color) = style_map.get("SecondaryColour") {
        if let Ok(c) = parse_ass_color(color) {
            style.secondary_color = c;
        }
    }

    // Outline color
    if let Some(color) = style_map.get("OutlineColour") {
        if let Ok(c) = parse_ass_color(color) {
            style.outline = Some(OutlineStyle::new(c, 2.0));
        }
    }

    // Outline width
    if let Some(width) = style_map.get("Outline") {
        if let Ok(w) = width.parse::<f32>() {
            if let Some(outline) = &mut style.outline {
                outline.width = w;
            }
        }
    }

    // Shadow
    if let Some(shadow) = style_map.get("Shadow") {
        if let Ok(s) = shadow.parse::<f32>() {
            if s > 0.0 {
                style.shadow = Some(ShadowStyle::new(Color::black(), s, s, 0.0));
            }
        }
    }

    // Alignment (SSA uses numeric alignment)
    if let Some(align) = style_map.get("Alignment") {
        if let Ok(a) = align.parse::<u8>() {
            style.alignment = parse_ass_alignment(a);
        }
    }

    // Margins
    if let Some(margin) = style_map.get("MarginL") {
        if let Ok(m) = margin.parse::<u32>() {
            style.margin_left = m;
        }
    }
    if let Some(margin) = style_map.get("MarginR") {
        if let Ok(m) = margin.parse::<u32>() {
            style.margin_right = m;
        }
    }
    if let Some(margin) = style_map.get("MarginV") {
        if let Ok(m) = margin.parse::<u32>() {
            style.margin_bottom = m;
            style.margin_top = m;
        }
    }

    Some((name, style))
}

/// Parse event/dialogue line.
fn parse_event_line(
    line: &str,
    format: &[String],
    styles: &HashMap<String, SubtitleStyle>,
) -> Option<Subtitle> {
    let is_comment = line.starts_with("Comment:");
    let content = line
        .strip_prefix("Dialogue:")
        .or_else(|| line.strip_prefix("Comment:"))?;

    // Split carefully - text field may contain commas
    let parts: Vec<&str> = content.splitn(format.len(), ',').map(str::trim).collect();

    let mut event_map = HashMap::new();
    for (i, field) in format.iter().enumerate() {
        if let Some(&value) = parts.get(i) {
            event_map.insert(field.clone(), value.to_string());
        }
    }

    // Skip comments
    if is_comment {
        return None;
    }

    // Parse start and end times
    let start_str = event_map.get("Start")?;
    let end_str = event_map.get("End")?;

    let start_time = parse_ass_timestamp(start_str)?;
    let end_time = parse_ass_timestamp(end_str)?;

    // Get text
    let text = event_map.get("Text")?.clone();
    let text = strip_ass_tags(&text);

    // Get style
    let style_name = event_map
        .get("Style")
        .map(String::as_str)
        .unwrap_or("Default");
    let style = styles.get(style_name).cloned();

    let mut subtitle = Subtitle::new(start_time, end_time, text);
    subtitle.style = style;

    Some(subtitle)
}

/// Parse ASS timestamp (e.g., "0:00:01.00").
fn parse_ass_timestamp(ts: &str) -> Option<i64> {
    let parts: Vec<&str> = ts.split(':').collect();
    if parts.len() != 3 {
        return None;
    }

    let hours: i64 = parts[0].parse().ok()?;
    let minutes: i64 = parts[1].parse().ok()?;

    let sec_parts: Vec<&str> = parts[2].split('.').collect();
    if sec_parts.len() != 2 {
        return None;
    }

    let seconds: i64 = sec_parts[0].parse().ok()?;
    let centiseconds: i64 = sec_parts[1].parse().ok()?;

    Some(hours * 3600000 + minutes * 60000 + seconds * 1000 + centiseconds * 10)
}

/// Parse ASS color (format: &HAABBGGRR or &HAABBGGRR&).
fn parse_ass_color(color: &str) -> Result<Color, SubtitleError> {
    let color = color.trim_start_matches('&').trim_start_matches('H');
    let color = color.trim_end_matches('&');

    // ASS colors are in AABBGGRR format (reversed from typical RRGGBBAA)
    if color.len() < 6 {
        return Err(SubtitleError::InvalidColor(color.to_string()));
    }

    // Pad to 8 characters if needed (default alpha is FF)
    let padded = if color.len() == 6 {
        format!("FF{color}")
    } else {
        color.to_string()
    };

    let aa = u8::from_str_radix(&padded[0..2], 16)
        .map_err(|_| SubtitleError::InvalidColor(color.to_string()))?;
    let bb = u8::from_str_radix(&padded[2..4], 16)
        .map_err(|_| SubtitleError::InvalidColor(color.to_string()))?;
    let gg = u8::from_str_radix(&padded[4..6], 16)
        .map_err(|_| SubtitleError::InvalidColor(color.to_string()))?;
    let rr = u8::from_str_radix(&padded[6..8], 16)
        .map_err(|_| SubtitleError::InvalidColor(color.to_string()))?;

    // ASS alpha is inverted (0 = opaque, 255 = transparent)
    let alpha = 255 - aa;

    Ok(Color::new(rr, gg, bb, alpha))
}

/// Parse ASS alignment (numeric).
fn parse_ass_alignment(align: u8) -> Alignment {
    // ASS alignment: 1-3 = bottom, 4-6 = middle, 7-9 = top
    // Within each row: 1,4,7 = left, 2,5,8 = center, 3,6,9 = right
    match align % 3 {
        1 => Alignment::Left,
        2 => Alignment::Center,
        0 => Alignment::Right,
        _ => Alignment::Center,
    }
}

/// Strip ASS override tags from text.
fn strip_ass_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    let mut brace_depth = 0;

    for c in text.chars() {
        match c {
            '{' => {
                in_tag = true;
                brace_depth += 1;
            }
            '}' => {
                brace_depth -= 1;
                if brace_depth == 0 {
                    in_tag = false;
                }
            }
            _ => {
                if !in_tag {
                    result.push(c);
                }
            }
        }
    }

    // Handle line breaks
    result.replace("\\N", "\n").replace("\\n", "\n")
}

/// Format milliseconds as ASS timestamp.
#[must_use]
pub fn format_timestamp(ms: i64) -> String {
    let hours = ms / 3600000;
    let minutes = (ms % 3600000) / 60000;
    let seconds = (ms % 60000) / 1000;
    let centis = (ms % 1000) / 10;

    format!("{hours}:{minutes:02}:{seconds:02}.{centis:02}")
}

/// Write subtitles in ASS format.
///
/// # Errors
///
/// Returns error if writing fails.
pub fn write(subtitles: &[Subtitle]) -> SubtitleResult<String> {
    let mut output = String::new();

    // Script info
    output.push_str("[Script Info]\n");
    output.push_str("Title: Exported Subtitles\n");
    output.push_str("ScriptType: v4.00+\n");
    output.push_str("WrapStyle: 0\n");
    output.push_str("ScaledBorderAndShadow: yes\n");
    output.push_str("YCbCr Matrix: TV.709\n\n");

    // Default style
    output.push_str("[V4+ Styles]\n");
    output.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");
    output.push_str("Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,40,40,40,1\n\n");

    // Events
    output.push_str("[Events]\n");
    output.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    for subtitle in subtitles {
        let start = format_timestamp(subtitle.start_time);
        let end = format_timestamp(subtitle.end_time);
        let text = subtitle.text.replace('\n', "\\N");

        output.push_str(&format!(
            "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
            start, end, text
        ));
    }

    Ok(output)
}
