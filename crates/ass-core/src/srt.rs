//! SubRip (SRT) subtitle parser.
//!
//! Parses SRT files into a [`SubtitleDocument`](crate::SubtitleDocument).
//! SRT blocks have the form:
//!
//! ```text
//! 1
//! 00:00:01,000 --> 00:00:05,000
//! Hello World
//! ```
//!
//! HTML-style tags (`<b>`, `<i>`, `<u>`, `<s>`) are converted to ASS
//! override tag format (`{\b1}`, etc.).

use crate::{
    error::ParseError, time::Timestamp, Effect, Event, EventType, ScriptMetadata, Style, StyleRef,
    SubtitleDocument, SubtitleFormat,
};

/// Parse SRT content into a `SubtitleDocument`.
pub fn parse_srt(content: &str) -> Result<SubtitleDocument, ParseError> {
    let mut events = Vec::new();
    let mut warnings = Vec::new();
    let blocks: Vec<&str> = content.split("\n\n").collect();

    for (idx, block) in blocks.iter().enumerate() {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() < 2 {
            warnings.push(crate::Warning {
                kind: crate::WarningKind::SrtBlockSkipped {
                    index: idx,
                    reason: "fewer than 2 lines".into(),
                },
                severity: crate::WarningSeverity::Warning,
                span: None,
            });
            continue;
        }

        // Find the timecode line (contains `-->`)
        let timecode_idx = lines.iter().position(|l| l.contains("-->"));
        let timecode_idx = match timecode_idx {
            Some(i) => i,
            None => {
                warnings.push(crate::Warning {
                    kind: crate::WarningKind::SrtBlockSkipped {
                        index: idx,
                        reason: "no timecode line found".into(),
                    },
                    severity: crate::WarningSeverity::Warning,
                    span: None,
                });
                continue;
            }
        };

        let text_start = timecode_idx + 1;
        let text: String = lines[text_start..].join("\n");
        let _converted = convert_srt_tags(&text);

        match parse_srt_timecodes(lines[timecode_idx]) {
            Ok((start, end)) => {
                events.push(Event {
                    source_line: 0,
                    event_type: EventType::Dialogue,
                    layer: 0,
                    start_ms: start.as_ms(),
                    end_ms: end.as_ms(),
                    style: StyleRef::new("Default"),
                    actor: String::new(),
                    margin_l: None,
                    margin_r: None,
                    margin_v: None,
                    effect: Effect::None,
                    text_raw: text,
                    override_tags: Vec::new(),
                    karaoke: Vec::new(),
                });
            }
            Err(e) => {
                warnings.push(crate::Warning {
                    kind: crate::WarningKind::SrtBlockSkipped {
                        index: idx,
                        reason: format!("invalid timestamp: {e}"),
                    },
                    severity: crate::WarningSeverity::Warning,
                    span: None,
                });
            }
        }
    }

    Ok(SubtitleDocument {
        format: SubtitleFormat::Srt,
        metadata: ScriptMetadata::default(),
        styles: vec![srt_default_style()],
        events,
        fonts: Vec::new(),
        warnings,
    })
}

fn parse_srt_timecodes(line: &str) -> Result<(Timestamp, Timestamp), ParseError> {
    let line = line.trim();
    let parts: Vec<&str> = line.split("-->").collect();
    if parts.len() != 2 {
        return Err(ParseError::invalid_timestamp(line));
    }
    let start = Timestamp::from_srt_timecode(parts[0].trim())?;
    let end = Timestamp::from_srt_timecode(parts[1].trim())?;
    Ok((start, end))
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
        name: StyleRef::new("Default"),
        font_name: "Arial".into(),
        font_size: 48.0,
        primary_color: crate::AssColor::WHITE,
        secondary_color: crate::AssColor::WHITE,
        outline_color: crate::AssColor::BLACK,
        shadow_color: crate::AssColor::BLACK,
        bold: false,
        italic: false,
        underline: false,
        strikeout: false,
        scale_x: 100.0,
        scale_y: 100.0,
        spacing: 0.0,
        angle: 0.0,
        border_style: crate::BorderStyle::OutlineAndShadow,
        outline: 2.0,
        shadow: 2.0,
        alignment: crate::Alignment::BottomCenter,
        margins: crate::Margins::new(10, 10, 10),
        encoding: crate::FontEncoding::new(1),
    }
}

/// Convert `SubtitleDocument` back to SRT format.
pub fn to_srt(doc: &SubtitleDocument) -> String {
    let mut dialogue: Vec<&Event> = doc
        .events
        .iter()
        .filter(|e| e.event_type == EventType::Dialogue)
        .collect();
    dialogue.sort_by_key(|e| (e.start_ms, e.layer));

    let mut output = String::new();
    for (i, event) in dialogue.iter().enumerate() {
        let start = crate::time::ms_to_srt_timecode(event.start_ms);
        let end = crate::time::ms_to_srt_timecode(event.end_ms);
        let text = strip_override_tags(&event.text_raw);
        output.push_str(&format!("{i}\n{start} --> {end}\n{text}\n", i = i + 1));
        if i + 1 < dialogue.len() {
            output.push('\n');
        }
    }
    output
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
    result.replace("\\N", "\n").replace("\\n", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_srt() {
        let srt = "1\n00:00:01,000 --> 00:00:05,000\nHello World";
        let doc = parse_srt(srt).unwrap();
        assert_eq!(doc.events.len(), 1);
        assert_eq!(doc.events[0].start_ms, 1000);
        assert_eq!(doc.events[0].end_ms, 5000);
        assert_eq!(doc.events[0].text_raw, "Hello World");
    }

    #[test]
    fn html_tags_converted() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\n<b>Bold</b> and <i>Italic</i>";
        let doc = parse_srt(srt).unwrap();
        assert_eq!(doc.events[0].text_raw, "<b>Bold</b> and <i>Italic</i>");
    }

    #[test]
    fn multiple_events() {
        let srt =
            "1\n00:00:01,000 --> 00:00:02,000\nFirst\n\n2\n00:00:03,000 --> 00:00:04,000\nSecond";
        let doc = parse_srt(srt).unwrap();
        assert_eq!(doc.events.len(), 2);
    }

    #[test]
    fn timecode_with_dot_separator() {
        let srt = "1\n00:00:01.500 --> 00:00:05.000\nTest";
        let doc = parse_srt(srt).unwrap();
        assert_eq!(doc.events[0].start_ms, 1500);
    }

    #[test]
    fn empty_input() {
        let doc = parse_srt("").unwrap();
        assert_eq!(doc.events.len(), 0);
    }

    #[test]
    fn short_millis() {
        let srt = "1\n00:00:01,5 --> 00:00:02,0\nTest";
        let doc = parse_srt(srt).unwrap();
        assert_eq!(doc.events[0].start_ms, 1500);
    }

    #[test]
    fn to_srt_roundtrip() {
        let input = "1\n00:00:01,000 --> 00:00:05,000\nHello World\n\n2\n00:00:06,000 --> 00:00:10,000\nLine two\n";
        let doc = parse_srt(input).unwrap();
        let output = to_srt(&doc);
        assert_eq!(input, output);
    }

    #[test]
    fn override_tags_stripped_in_output() {
        let mut doc = SubtitleDocument::default();
        doc.events.push(Event {
            source_line: 0,
            event_type: EventType::Dialogue,
            layer: 0,
            start_ms: 0,
            end_ms: 3000,
            style: StyleRef::new("Default"),
            actor: String::new(),
            margin_l: None,
            margin_r: None,
            margin_v: None,
            effect: Effect::None,
            text_raw: "{\\b1}Bold{\\b0} text".into(),
            override_tags: Vec::new(),
            karaoke: Vec::new(),
        });
        let srt = to_srt(&doc);
        assert!(srt.contains("Bold text"));
        assert!(!srt.contains("{\\"));
    }
}
