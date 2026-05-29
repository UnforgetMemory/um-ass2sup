use super::timestamp::Timestamp;
use super::event::{Event, EventType};
use super::color::AssColor;
use super::style::Style;
use super::{AssFile, ScriptInfo, SubtitleFormat};
use super::ParseError;

pub fn parse_srt(content: &str) -> Result<AssFile, ParseError> {
    let mut events = Vec::new();
    let mut blocks = content.split("\n\n").peekable();

    while let Some(block) = blocks.next() {
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
            effect: String::new(),
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
    let s = s.split_once(',').or_else(|| s.split_once('.')).unwrap_or((s, "0"));
    let time = s.0;
    let ms: u64 = s.1.parse().unwrap_or(0);
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() != 3 {
        return Err(ParseError::InvalidTimestamp(s.0.to_string()));
    }
    let h: u64 = parts[0].parse().map_err(|_| ParseError::InvalidTimestamp(s.0.to_string()))?;
    let m: u64 = parts[1].parse().map_err(|_| ParseError::InvalidTimestamp(s.0.to_string()))?;
    let sec: u64 = parts[2].parse().map_err(|_| ParseError::InvalidTimestamp(s.0.to_string()))?;
    Ok(Timestamp::from_ms(h * 3600000 + m * 60000 + sec * 1000 + ms))
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
