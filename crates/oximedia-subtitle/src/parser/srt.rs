//! SubRip (SRT) subtitle parser.
//!
//! SRT is a simple subtitle format with the following structure:
//!
//! ```text
//! 1
//! 00:00:01,000 --> 00:00:04,000
//! This is the first subtitle.
//!
//! 2
//! 00:00:05,000 --> 00:00:08,000
//! This is the second subtitle.
//! It can span multiple lines.
//! ```

use crate::{Subtitle, SubtitleError, SubtitleResult};
use nom::{
    bytes::complete::{tag, take_until, take_while, take_while1},
    character::complete::{char, digit1, line_ending, not_line_ending},
    combinator::{map, map_res, opt},
    multi::many1,
    sequence::{preceded, separated_pair, terminated},
    IResult,
};

/// Parse SRT subtitle file.
///
/// # Errors
///
/// Returns error if the file is not valid SRT format.
pub fn parse(data: &[u8]) -> SubtitleResult<Vec<Subtitle>> {
    let text = String::from_utf8_lossy(data);
    parse_srt(&text)
}

/// Parse SRT subtitle from string.
///
/// # Errors
///
/// Returns error if parsing fails.
pub fn parse_srt(input: &str) -> SubtitleResult<Vec<Subtitle>> {
    // Normalize line endings
    let normalized = input.replace("\r\n", "\n");

    match parse_subtitle_file(&normalized) {
        Ok((_, subtitles)) => Ok(subtitles),
        Err(e) => Err(SubtitleError::ParseError(format!("SRT parse error: {e}"))),
    }
}

/// Check if text looks like SRT format.
#[must_use]
pub fn is_srt_format(text: &str) -> bool {
    // Look for typical SRT patterns: number, timestamp arrow, text
    let lines: Vec<&str> = text.lines().take(10).collect();

    for window in lines.windows(3) {
        if window[0].trim().chars().all(|c| c.is_ascii_digit())
            && window[1].contains("-->")
            && window[1].contains(':')
        {
            return true;
        }
    }

    false
}

/// Parse complete subtitle file.
fn parse_subtitle_file(input: &str) -> IResult<&str, Vec<Subtitle>> {
    // Skip BOM if present
    let mut input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut subtitles = Vec::new();

    loop {
        // Skip whitespace
        let (rest, _) = take_while(|c: char| c.is_whitespace())(input)?;
        input = rest;

        // Check if we've reached end of input
        if input.is_empty() {
            break;
        }

        // Try to parse an entry
        match parse_subtitle_entry(input) {
            Ok((rest, subtitle)) => {
                subtitles.push(subtitle);
                input = rest;
            }
            Err(_) => break,
        }
    }

    if subtitles.is_empty() {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Many1,
        )))
    } else {
        Ok((input, subtitles))
    }
}

/// Parse a single subtitle entry.
fn parse_subtitle_entry(input: &str) -> IResult<&str, Subtitle> {
    let (input, _) = skip_empty_lines(input)?;

    let (input, _sequence) = digit1(input)?;
    let (input, _) = line_ending(input)?;
    let (input, (start, end)) = parse_timestamp_line(input)?;
    let (input, _) = line_ending(input)?;
    let (input, text) = subtitle_text(input)?;

    Ok((input, Subtitle::new(start, end, text)))
}

/// Skip empty lines and whitespace.
fn skip_empty_lines(input: &str) -> IResult<&str, ()> {
    let (input, _) = take_while(|c: char| c.is_whitespace())(input)?;
    Ok((input, ()))
}

/// Parse timestamp line (e.g., "00:00:01,000 --> 00:00:04,000").
fn parse_timestamp_line(input: &str) -> IResult<&str, (i64, i64)> {
    let (input, start) = timestamp(input)?;
    let (input, _) = tag(" --> ")(input)?;
    let (input, end) = timestamp(input)?;
    Ok((input, (start, end)))
}

/// Parse timestamp line with optional trailing content.
fn timestamp_line(input: &str) -> IResult<&str, (i64, i64)> {
    let (input, times) = parse_timestamp_line(input)?;
    // Try to parse optional trailing content
    let (input, _) = if let Ok((rest, _)) = tag::<_, _, nom::error::Error<_>>(" ")(input) {
        let (rest, _) = not_line_ending(rest)?;
        (rest, ())
    } else {
        (input, ())
    };
    Ok((input, times))
}

/// Parse a timestamp (e.g., "00:00:01,000").
fn timestamp(input: &str) -> IResult<&str, i64> {
    let (input, hours) = digit1(input)?;
    let (input, _) = char(':')(input)?;
    let (input, minutes) = digit1(input)?;
    let (input, _) = char(':')(input)?;
    let (input, seconds) = digit1(input)?;
    let (input, _) = char(',')(input)?;
    let (input, millis) = digit1(input)?;

    let result = parse_timestamp_parts(hours, minutes, seconds, millis)
        .map_err(|_| nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Fail)))?;

    Ok((input, result))
}

/// Parse timestamp components into milliseconds.
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

/// Parse subtitle text (until next entry or end of file).
fn subtitle_text(input: &str) -> IResult<&str, String> {
    let mut text = String::new();
    let mut remaining = input;

    #[allow(clippy::while_let_loop)]
    loop {
        // Try to parse a line
        match not_line_ending::<_, nom::error::Error<_>>(remaining) {
            Ok((rest, line)) => {
                if line.trim().is_empty() {
                    // Empty line marks end of subtitle
                    let (rest, _) =
                        line_ending::<_, nom::error::Error<_>>(rest).unwrap_or((rest, ""));
                    return Ok((rest, crate::text::decode_html_entities(&text)));
                }
                // Add line to text
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(line);
                // Try to consume line ending
                if let Ok((rest, _)) = line_ending::<_, nom::error::Error<_>>(rest) {
                    remaining = rest;
                } else {
                    remaining = rest;
                    break;
                }
            }
            Err(_) => break,
        }
    }

    if text.is_empty() {
        Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Many1,
        )))
    } else {
        Ok((remaining, crate::text::decode_html_entities(&text)))
    }
}

/// Format milliseconds as SRT timestamp.
#[must_use]
pub fn format_timestamp(ms: i64) -> String {
    let hours = ms / 3600000;
    let minutes = (ms % 3600000) / 60000;
    let seconds = (ms % 60000) / 1000;
    let millis = ms % 1000;

    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

/// Write subtitles in SRT format.
///
/// # Errors
///
/// Returns error if writing fails.
pub fn write(subtitles: &[Subtitle]) -> SubtitleResult<String> {
    let mut output = String::new();

    for (i, subtitle) in subtitles.iter().enumerate() {
        // Sequence number
        output.push_str(&format!("{}\n", i + 1));

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
