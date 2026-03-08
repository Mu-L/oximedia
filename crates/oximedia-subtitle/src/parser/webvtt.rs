//! WebVTT (Web Video Text Tracks) subtitle parser.
//!
//! WebVTT is a W3C standard format for web video captions.
//!
//! ```text
//! WEBVTT
//!
//! 00:00:01.000 --> 00:00:04.000
//! This is the first subtitle.
//!
//! 00:00:05.000 --> 00:00:08.000 position:50% align:middle
//! This is a positioned subtitle.
//! ```

use crate::style::{Alignment, Position};
use crate::{Subtitle, SubtitleError, SubtitleResult};
use nom::{
    bytes::complete::{tag, take_until, take_while, take_while1},
    character::complete::{char, digit1, line_ending, not_line_ending, space0},
    combinator::{map, map_res, opt},
    multi::{many0, many1},
    sequence::{preceded, separated_pair, terminated},
    IResult,
};

/// Parse WebVTT subtitle file.
///
/// # Errors
///
/// Returns error if the file is not valid WebVTT format.
pub fn parse(data: &[u8]) -> SubtitleResult<Vec<Subtitle>> {
    let text = String::from_utf8_lossy(data);
    parse_webvtt(&text)
}

/// Parse WebVTT subtitle from string.
///
/// # Errors
///
/// Returns error if parsing fails.
pub fn parse_webvtt(input: &str) -> SubtitleResult<Vec<Subtitle>> {
    let normalized = input.replace("\r\n", "\n");

    match parse_vtt_file(&normalized) {
        Ok((_, subtitles)) => Ok(subtitles),
        Err(e) => Err(SubtitleError::ParseError(format!(
            "WebVTT parse error: {e}"
        ))),
    }
}

/// Parse complete WebVTT file.
fn parse_vtt_file(input: &str) -> IResult<&str, Vec<Subtitle>> {
    // Skip BOM if present
    let mut input = input.strip_prefix('\u{feff}').unwrap_or(input);

    // Parse header
    let (rest, _) = parse_header(input)?;
    input = rest;

    // Skip optional metadata
    let (rest, _) = skip_metadata(input)?;
    input = rest;

    // Parse cues
    let mut cues = Vec::new();
    loop {
        // Skip whitespace
        let (rest, _) = take_while(|c: char| c.is_whitespace())(input)?;
        input = rest;

        // Check if we've reached end of input
        if input.is_empty() {
            break;
        }

        // Try to parse a cue block
        match parse_cue_block(input) {
            Ok((rest, cue)) => {
                cues.push(cue);
                input = rest;
            }
            Err(_) => break,
        }
    }

    Ok((input, cues))
}

/// Parse WebVTT header.
fn parse_header(input: &str) -> IResult<&str, ()> {
    let (input, _) = tag("WEBVTT")(input)?;
    // Try to parse optional trailing content
    let (input, _) = if let Ok((rest, _)) = space0::<_, nom::error::Error<_>>(input) {
        if let Ok((rest2, _)) = not_line_ending::<_, nom::error::Error<_>>(rest) {
            (rest2, ())
        } else {
            (rest, ())
        }
    } else {
        (input, ())
    };
    let (input, _) = line_ending(input)?;
    Ok((input, ()))
}

/// Skip metadata and empty lines.
fn skip_metadata(input: &str) -> IResult<&str, ()> {
    let (input, _) = take_while(|c: char| c.is_whitespace())(input)?;
    Ok((input, ()))
}

/// Parse a cue block.
fn parse_cue_block(input: &str) -> IResult<&str, Subtitle> {
    let (input, _) = skip_empty_lines(input)?;

    // Optional cue identifier - try to parse it
    let (input, _) = if let Ok((rest, _)) = cue_identifier(input) {
        if let Ok((rest2, _)) = line_ending::<_, nom::error::Error<_>>(rest) {
            (rest2, ())
        } else {
            (input, ())
        }
    } else {
        (input, ())
    };

    // Cue timings and settings
    let (input, (start, end, settings)) = cue_timings(input)?;
    let (input, _) = line_ending(input)?;

    // Cue payload (text)
    let (input, text) = cue_payload(input)?;

    // Parse settings
    let (position, _alignment) = parse_cue_settings(&settings);

    let mut subtitle = Subtitle::new(start, end, text);
    if let Some(pos) = position {
        subtitle.position = Some(pos);
    }

    Ok((input, subtitle))
}

/// Alternative name for consistency.
fn cue_block(input: &str) -> IResult<&str, Subtitle> {
    parse_cue_block(input)
}

/// Skip empty lines.
fn skip_empty_lines(input: &str) -> IResult<&str, ()> {
    let (input, _) = take_while(|c: char| c.is_whitespace())(input)?;
    Ok((input, ()))
}

/// Parse cue identifier.
fn cue_identifier(input: &str) -> IResult<&str, &str> {
    // Identifier is any line that doesn't contain "-->"
    let (input, id) = take_while1(|c: char| c != '\n' && c != '\r')(input)?;
    if id.contains("-->") {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }
    Ok((input, id))
}

/// Parse cue timings line.
fn cue_timings(input: &str) -> IResult<&str, (i64, i64, String)> {
    let (input, start) = vtt_timestamp(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = tag("-->")(input)?;
    let (input, _) = space0(input)?;
    let (input, end) = vtt_timestamp(input)?;
    // Try to parse optional settings
    let (input, settings) = if let Ok((rest, _)) = space0::<_, nom::error::Error<_>>(input) {
        if let Ok((rest2, s)) = not_line_ending::<_, nom::error::Error<_>>(rest) {
            (rest2, s.to_string())
        } else {
            (rest, String::new())
        }
    } else {
        (input, String::new())
    };

    Ok((input, (start, end, settings)))
}

/// Parse WebVTT timestamp (e.g., "00:00:01.000" or "01.000").
fn vtt_timestamp(input: &str) -> IResult<&str, i64> {
    // Try long format first (HH:MM:SS.mmm)
    if let Ok((rest, ts)) = timestamp_long(input) {
        return Ok((rest, ts));
    }

    // Try short format (MM:SS.mmm)
    timestamp_short(input)
}

/// Parse long timestamp format (HH:MM:SS.mmm).
fn timestamp_long(input: &str) -> IResult<&str, i64> {
    let (input, hours) = digit1(input)?;
    let (input, _) = char(':')(input)?;
    let (input, minutes) = digit1(input)?;
    let (input, _) = char(':')(input)?;
    let (input, seconds) = digit1(input)?;
    let (input, _) = char('.')(input)?;
    let (input, millis) = digit1(input)?;

    let result = parse_timestamp_parts(hours, minutes, seconds, millis)
        .map_err(|_| nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Fail)))?;

    Ok((input, result))
}

/// Parse short timestamp format (MM:SS.mmm).
fn timestamp_short(input: &str) -> IResult<&str, i64> {
    let (input, minutes) = digit1(input)?;
    let (input, _) = char(':')(input)?;
    let (input, seconds) = digit1(input)?;
    let (input, _) = char('.')(input)?;
    let (input, millis) = digit1(input)?;

    let result = parse_timestamp_short(minutes, seconds, millis)
        .map_err(|_| nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Fail)))?;

    Ok((input, result))
}

/// Parse timestamp parts (with hours).
fn parse_timestamp_parts(
    hours: &str,
    minutes: &str,
    seconds: &str,
    millis: &str,
) -> Result<i64, std::num::ParseIntError> {
    let h: i64 = hours.parse()?;
    let m: i64 = minutes.parse()?;
    let s: i64 = seconds.parse()?;
    let ms: i64 = millis.parse()?;

    Ok(h * 3600000 + m * 60000 + s * 1000 + ms)
}

/// Parse timestamp parts (without hours).
fn parse_timestamp_short(
    minutes: &str,
    seconds: &str,
    millis: &str,
) -> Result<i64, std::num::ParseIntError> {
    let m: i64 = minutes.parse()?;
    let s: i64 = seconds.parse()?;
    let ms: i64 = millis.parse()?;

    Ok(m * 60000 + s * 1000 + ms)
}

/// Parse cue payload (text).
fn cue_payload(input: &str) -> IResult<&str, String> {
    let mut text = String::new();
    let mut remaining = input;

    #[allow(clippy::while_let_loop)]
    loop {
        // Try to parse a line
        match not_line_ending::<_, nom::error::Error<_>>(remaining) {
            Ok((rest, line)) => {
                if line.trim().is_empty() {
                    // Empty line marks end of payload
                    let (rest, _) =
                        line_ending::<_, nom::error::Error<_>>(rest).unwrap_or((rest, ""));
                    return Ok((rest, text));
                }
                // Add line to text
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(line);
                // Try to consume line ending
                if let Ok((rest2, _)) = line_ending::<_, nom::error::Error<_>>(rest) {
                    remaining = rest2;
                } else {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    // Strip WebVTT tags (basic implementation)
    let cleaned = strip_vtt_tags(&text);

    Ok((input, cleaned))
}

/// Parse cue settings.
fn parse_cue_settings(settings: &str) -> (Option<Position>, Option<Alignment>) {
    let mut position = None;
    let mut alignment = None;

    for setting in settings.split_whitespace() {
        if let Some(value) = setting.strip_prefix("position:") {
            if let Ok(percent) = value.trim_end_matches('%').parse::<f32>() {
                position = Some(Position::new(percent / 100.0, 0.9));
            }
        } else if let Some(value) = setting.strip_prefix("align:") {
            alignment = match value {
                "start" | "left" => Some(Alignment::Left),
                "center" | "middle" => Some(Alignment::Center),
                "end" | "right" => Some(Alignment::Right),
                _ => None,
            };
        }
    }

    (position, alignment)
}

/// Strip WebVTT formatting tags.
fn strip_vtt_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;

    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ => {
                if !in_tag {
                    result.push(c);
                }
            }
        }
    }

    crate::text::decode_html_entities(&result)
}

/// Format milliseconds as WebVTT timestamp.
#[must_use]
pub fn format_timestamp(ms: i64) -> String {
    let hours = ms / 3600000;
    let minutes = (ms % 3600000) / 60000;
    let seconds = (ms % 60000) / 1000;
    let millis = ms % 1000;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
    } else {
        format!("{minutes:02}:{seconds:02}.{millis:03}")
    }
}

/// Write subtitles in WebVTT format.
///
/// # Errors
///
/// Returns error if writing fails.
pub fn write(subtitles: &[Subtitle]) -> SubtitleResult<String> {
    let mut output = String::from("WEBVTT\n\n");

    for subtitle in subtitles {
        // Timestamps
        output.push_str(&format!(
            "{} --> {}\n",
            format_timestamp(subtitle.start_time),
            format_timestamp(subtitle.end_time)
        ));

        // Text
        output.push_str(&subtitle.text);
        output.push_str("\n\n");
    }

    Ok(output)
}
