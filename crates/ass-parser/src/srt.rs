use super::color::AssColor;
use super::effect::Effect;
use super::event::{Event, EventType};
use super::style::Style;
use super::timestamp::Timestamp;
use super::ParseError;
use super::{AssFile, ScriptInfo, SubtitleFormat};

/// Parses an SRT (SubRip) subtitle file into an [`AssFile`].
///
/// SRT files use a simpler format than ASS — each block has an index, timecodes, and text.
/// HTML-style tags (`<b>`, `<i>`, `<u>`, `<s>`) are converted to ASS override blocks.
/// A default style (Arial 48pt, white with black outline) is applied.
///
/// # SRT Block Format
///
/// ```text
/// 1
/// 00:00:01,000 --> 00:00:05,000
/// Hello World
/// ```
pub fn parse_srt(content: &str) -> Result<AssFile, ParseError> {
    let mut events = Vec::new();
    let blocks = content.split("\n\n").peekable();

    for block in blocks {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() < 2 {
            continue;
        }
        let timecode_line = if lines[0].contains("-->") {
            lines[0]
        } else if lines.len() > 1 && lines[1].contains("-->") {
            lines[1]
        } else {
            continue;
        };
        let text_start = if lines[0].contains("-->") { 1 } else { 2 };
        let text: String = lines[text_start..].join("\n");
        let (start, end) = parse_srt_timecodes(timecode_line)?;
        events.push(Event {
            event_type: EventType::Dialogue,
            layer: 0,
            start,
            end,
            style_name: "Default".to_string(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: convert_srt_tags(&text),
            override_tags: Vec::new(),
            karaoke_segments: Vec::new(),
            raw_override_block: String::new(),
        });
    }

    Ok(AssFile {
        format: SubtitleFormat::Srt,
        script_info: ScriptInfo::default(),
        styles: vec![srt_default_style()],
        events,
        embedded_fonts: Vec::new(),
    })
}

fn parse_srt_timecodes(line: &str) -> Result<(Timestamp, Timestamp), ParseError> {
    let parts: Vec<&str> = line.split("-->").collect();
    if parts.len() != 2 {
        return Err(ParseError::InvalidTimestamp(line.to_string()));
    }
    let start = parse_srt_timecode(parts[0].trim())?;
    let end = parse_srt_timecode(parts[1].trim())?;
    Ok((start, end))
}

fn parse_srt_timecode(s: &str) -> Result<Timestamp, ParseError> {
    let s = s
        .split_once(',')
        .or_else(|| s.split_once('.'))
        .unwrap_or((s, "0"));
    let time = s.0;
    let ms: u64 = s.1.parse().unwrap_or(0);
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() != 3 {
        return Err(ParseError::InvalidTimestamp(s.0.to_string()));
    }
    let h: u64 = parts[0]
        .parse()
        .map_err(|_| ParseError::InvalidTimestamp(s.0.to_string()))?;
    let m: u64 = parts[1]
        .parse()
        .map_err(|_| ParseError::InvalidTimestamp(s.0.to_string()))?;
    let sec: u64 = parts[2]
        .parse()
        .map_err(|_| ParseError::InvalidTimestamp(s.0.to_string()))?;
    let ms_total = h
        .saturating_mul(3_600_000)
        .saturating_add(m.saturating_mul(60_000))
        .saturating_add(sec.saturating_mul(1000))
        .saturating_add(ms);
    Ok(Timestamp::from_ms(ms_total))
}

fn convert_srt_tags(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::new();
            while let Some(&next) = chars.peek() {
                if next == '>' {
                    chars.next();
                    break;
                }
                tag.push(next);
                chars.next();
            }
            match tag.to_lowercase().as_str() {
                "b" => result.push_str("{\\b1}"),
                "/b" => result.push_str("{\\b0}"),
                "i" => result.push_str("{\\i1}"),
                "/i" => result.push_str("{\\i0}"),
                "u" => result.push_str("{\\u1}"),
                "/u" => result.push_str("{\\u0}"),
                "s" => result.push_str("{\\s1}"),
                "/s" => result.push_str("{\\s0}"),
                _ => {}
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn strip_override_tags(text: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '{' => in_tag = true,
            '}' => in_tag = false,
            _ => {
                if !in_tag {
                    result.push(c);
                }
            }
        }
    }
    result
}

fn ass_to_srt_text(text: &str) -> String {
    let stripped = strip_override_tags(text);
    stripped.replace("\\N", "\n").replace("\\n", "\n")
}

fn format_srt_time(ts: Timestamp) -> String {
    let ms = ts.as_ms();
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let ms_remainder = ms % 1_000;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms_remainder)
}

impl AssFile {
    pub fn to_srt(&self) -> String {
        let mut events: Vec<&Event> = self.events.iter().filter(|e| e.is_dialogue()).collect();

        events.sort_by_key(|e| (e.start, e.layer));

        let mut output = String::new();
        for (i, event) in events.iter().enumerate() {
            let start = format_srt_time(event.start);
            let end = format_srt_time(event.end);
            let text = ass_to_srt_text(&event.text);

            output.push_str(&format!(
                "{}\n{} --> {}\n{}\n",
                i + 1,
                start,
                end,
                text.trim(),
            ));

            if i + 1 < events.len() {
                output.push('\n');
            }
        }

        output
    }
}

fn srt_default_style() -> Style {
    Style {
        name: "Default".to_string(),
        font_name: "Arial".to_string(),
        font_size: 48.0,
        primary_color: AssColor::WHITE,
        secondary_color: AssColor::WHITE,
        outline_color: AssColor::BLACK,
        shadow_color: AssColor::BLACK,
        bold: false,
        italic: false,
        underline: false,
        strikeout: false,
        scale_x: 100.0,
        scale_y: 100.0,
        spacing: 0.0,
        angle: 0.0,
        border_style: 1,
        outline_width: 2.0,
        shadow_depth: 2.0,
        alignment: 2,
        margin_l: 10,
        margin_r: 10,
        margin_v: 10,
        encoding: 1,
        relative_to: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Effect, Event, EventType};

    #[test]
    fn to_srt_basic() {
        let mut ass = AssFile::new();
        ass.events.push(Event {
            event_type: EventType::Dialogue,
            layer: 0,
            start: Timestamp::from_ms(1000),
            end: Timestamp::from_ms(5000),
            style_name: "Default".to_string(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: "Hello World".to_string(),
            override_tags: Vec::new(),
            karaoke_segments: Vec::new(),
            raw_override_block: String::new(),
        });

        let srt = ass.to_srt();
        let expected = "1\n00:00:01,000 --> 00:00:05,000\nHello World\n";
        assert_eq!(srt, expected);
    }

    #[test]
    fn to_srt_multiple_events() {
        let mut ass = AssFile::new();
        ass.events.push(Event {
            event_type: EventType::Dialogue,
            layer: 0,
            start: Timestamp::from_ms(5000),
            end: Timestamp::from_ms(10000),
            style_name: "Default".to_string(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: "Second".to_string(),
            override_tags: Vec::new(),
            karaoke_segments: Vec::new(),
            raw_override_block: String::new(),
        });
        ass.events.push(Event {
            event_type: EventType::Dialogue,
            layer: 0,
            start: Timestamp::from_ms(1000),
            end: Timestamp::from_ms(5000),
            style_name: "Default".to_string(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: "First".to_string(),
            override_tags: Vec::new(),
            karaoke_segments: Vec::new(),
            raw_override_block: String::new(),
        });

        let srt = ass.to_srt();
        assert!(srt.starts_with("1\n00:00:01,000 --> 00:00:05,000\nFirst\n"));
        assert!(srt.contains("2\n00:00:05,000 --> 00:00:10,000\nSecond\n"));
    }

    #[test]
    fn to_srt_strips_override_tags() {
        let mut ass = AssFile::new();
        ass.events.push(Event {
            event_type: EventType::Dialogue,
            layer: 0,
            start: Timestamp::ZERO,
            end: Timestamp::from_ms(3000),
            style_name: "Default".to_string(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: "{\\b1}Bold{\\b0} and {\\i1}Italic{\\i0}".to_string(),
            override_tags: Vec::new(),
            karaoke_segments: Vec::new(),
            raw_override_block: String::new(),
        });

        let srt = ass.to_srt();
        assert!(srt.contains("Bold and Italic"));
        assert!(!srt.contains("{\\"));
    }

    #[test]
    fn to_srt_converts_newline_escapes() {
        let mut ass = AssFile::new();
        ass.events.push(Event {
            event_type: EventType::Dialogue,
            layer: 0,
            start: Timestamp::ZERO,
            end: Timestamp::from_ms(3000),
            style_name: "Default".to_string(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: "Line one\\NLine two".to_string(),
            override_tags: Vec::new(),
            karaoke_segments: Vec::new(),
            raw_override_block: String::new(),
        });

        let srt = ass.to_srt();
        assert!(srt.contains("Line one\nLine two"));
    }

    #[test]
    fn to_srt_skips_comments() {
        let mut ass = AssFile::new();
        ass.events.push(Event {
            event_type: EventType::Comment,
            layer: 0,
            start: Timestamp::ZERO,
            end: Timestamp::from_ms(3000),
            style_name: "Default".to_string(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: "Comment".to_string(),
            override_tags: Vec::new(),
            karaoke_segments: Vec::new(),
            raw_override_block: String::new(),
        });
        ass.events.push(Event {
            event_type: EventType::Dialogue,
            layer: 0,
            start: Timestamp::from_ms(1000),
            end: Timestamp::from_ms(2000),
            style_name: "Default".to_string(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: "Visible".to_string(),
            override_tags: Vec::new(),
            karaoke_segments: Vec::new(),
            raw_override_block: String::new(),
        });

        let srt = ass.to_srt();
        assert!(!srt.contains("Comment"));
        assert!(srt.contains("Visible"));
    }

    #[test]
    fn to_srt_empty_events() {
        let ass = AssFile::new();
        let srt = ass.to_srt();
        assert_eq!(srt, "");
    }

    #[test]
    fn to_srt_timestamp_formatting() {
        let mut ass = AssFile::new();
        ass.events.push(Event {
            event_type: EventType::Dialogue,
            layer: 0,
            start: Timestamp::from_hms(1, 23, 45, 678),
            end: Timestamp::from_hms(2, 0, 0, 0),
            style_name: "Default".to_string(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: "Timestamps".to_string(),
            override_tags: Vec::new(),
            karaoke_segments: Vec::new(),
            raw_override_block: String::new(),
        });

        let srt = ass.to_srt();
        assert!(srt.contains("01:23:45,678 --> 02:00:00,000"));
    }

    #[test]
    fn to_srt_roundtrip() {
        let input = "1\n00:00:01,000 --> 00:00:05,000\nHello World\n\n2\n00:00:06,000 --> 00:00:10,000\nLine two\n";
        let parsed = parse_srt(input).unwrap();
        let output = parsed.to_srt();
        assert_eq!(input, output);
    }

    #[test]
    fn fuzz_regression_srt_malformed_timestamp() {
        // Fuzz crasher: `3:2223:00006817148741241740400-->`
        // Tests whether parse_srt panics on malformed timestamps with overflow-causing seconds.
        let input = std::fs::read_to_string("tests/data/fuzz_srt_crash.txt")
            .expect("fuzz_srt_crash.txt test data file missing");
        // Must not panic — should return Err gracefully
        let result = parse_srt(&input);
        // If this passed without panic, the crasher is stale (no actual bug remains).
        // If it panicked, the fix made it return Err instead.
        assert!(
            result.is_err(),
            "expected Err for malformed SRT timestamp, got Ok"
        );
    }

    #[test]
    fn fuzz_regression_srt_malformed_timestamp_inline() {
        // Edge case: same pattern inline (no test data dependency)
        let input = "3:2223:00006817148741241740400-->\n+";
        let result = parse_srt(input);
        assert!(result.is_err(), "expected Err for malformed SRT timestamp");
    }

    #[test]
    fn srt_edge_empty_string() {
        let result = parse_srt("");
        assert!(result.is_ok());
    }

    #[test]
    fn srt_edge_single_char() {
        let result = parse_srt("x");
        assert!(result.is_ok());
    }

    #[test]
    fn srt_edge_just_arrow() {
        // Just the arrow with no valid timestamps
        let result = parse_srt("-->");
        assert!(result.is_ok());
    }

    #[test]
    fn srt_edge_negative_timestamp() {
        let result = parse_srt("0\n-1:00:00,000 --> 00:00:05,000\nHello");
        assert!(result.is_err());
    }

    #[test]
    fn to_srt_strips_complex_tags() {
        let mut ass = AssFile::new();
        ass.events.push(Event {
            event_type: EventType::Dialogue,
            layer: 0,
            start: Timestamp::ZERO,
            end: Timestamp::from_ms(3000),
            style_name: "Default".to_string(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: "{\\pos(100,200)}Positioned{\\move(0,0,100,100)} text".to_string(),
            override_tags: Vec::new(),
            karaoke_segments: Vec::new(),
            raw_override_block: String::new(),
        });

        let srt = ass.to_srt();
        assert!(srt.contains("Positioned text"));
        assert!(!srt.contains("\\pos"));
        assert!(!srt.contains("\\move"));
    }
}
