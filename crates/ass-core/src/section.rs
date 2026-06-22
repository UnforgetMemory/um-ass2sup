//! Section-level parsers for ASS/SSA file structure.
//!
//! Parses the structured content of each ASS section:
//! - `[Script Info]` → key-value metadata
//! - `[V4+ Styles]` / `[V4 Styles]` → style definitions
//! - `[Events]` → event/dialogue lines
//! - `[Fonts]` → embedded font references

use crate::{
    error::ParseError,
    lexer::{Line, Token},
    override_tag::parse_tags,
    time::Timestamp,
    Alignment, AssColor, BorderStyle, EmbeddedFont, Event, EventType, FontEncoding, Margins,
    ScriptMetadata, Style, StyleRef,
};

/// Parse lines from a `[Script Info]` section.
pub fn parse_script_info(lines: &[Line]) -> ScriptMetadata {
    let mut meta = ScriptMetadata::default();
    for line in lines {
        if let Token::KeyValue { ref key, ref value } = line.token {
            match key.as_str() {
                "Title" => meta.title = value.clone(),
                "ScriptType" => meta.script_type = value.clone(),
                "WrapStyle" => meta.wrap_style = value.parse().unwrap_or(0),
                "ScaledBorderAndShadow" => {
                    meta.scaled_border_and_shadow = value.eq_ignore_ascii_case("yes")
                }
                "YCbCr Matrix" => meta.ycbcr_matrix = value.clone(),
                "PlayResX" => meta.play_res_x = value.parse().unwrap_or(1920),
                "PlayResY" => meta.play_res_y = value.parse().unwrap_or(1080),
                _ => {
                    meta.extra.insert(key.clone(), value.clone());
                }
            }
        }
    }
    meta
}

/// Parse styles from `[V4+ Styles]` or `[V4 Styles]` section lines.
pub fn parse_styles(lines: &[Line], is_v4: bool) -> (Vec<Style>, Vec<ParseError>) {
    let mut styles = Vec::new();
    let mut errors = Vec::new();
    for line in lines {
        if let Token::StyleData(ref data) = line.token {
            match parse_style_line(data, is_v4) {
                Ok(s) => styles.push(s),
                Err(e) => errors.push(e),
            }
        }
    }
    (styles, errors)
}

fn parse_style_line(data: &str, is_v4: bool) -> Result<Style, ParseError> {
    let limit = if is_v4 { 19 } else { 24 };
    let fields: Vec<&str> = data.splitn(limit, ',').collect();
    let min_fields = if is_v4 { 18 } else { 23 };

    if fields.len() < min_fields {
        return Err(ParseError::InvalidStyle {
            detail: format!("expected {min_fields} fields, got {}", fields.len()),
            span: None,
        });
    }

    let flag = |s: &str| matches!(s.trim(), "-1" | "1");

    if is_v4 {
        let parse_v4_color = |s: &str| -> AssColor {
            let s = s.trim();
            if s.starts_with("&H") || s.starts_with("&h") {
                return AssColor::from_ass_hex(s).unwrap_or(AssColor::WHITE);
            }
            AssColor::from_raw_abgr(s.parse::<i64>().unwrap_or(0) as u32)
        };
        Ok(Style {
            name: StyleRef::new(fields[0].trim()),
            font_name: fields[1].trim().to_string(),
            font_size: fields[2].trim().parse().unwrap_or(20.0),
            primary_color: parse_v4_color(fields[3]),
            secondary_color: parse_v4_color(fields[4]),
            outline_color: parse_v4_color(fields[5]),
            shadow_color: parse_v4_color(fields[6]),
            bold: flag(fields[7]),
            italic: flag(fields[8]),
            underline: false,
            strikeout: false,
            scale_x: 100.0,
            scale_y: 100.0,
            spacing: 0.0,
            angle: 0.0,
            border_style: BorderStyle::from_u8(fields[9].trim().parse().unwrap_or(1))
                .unwrap_or(BorderStyle::OutlineAndShadow),
            outline: fields[10].trim().parse().unwrap_or(2.0),
            shadow: fields[11].trim().parse().unwrap_or(2.0),
            alignment: Alignment::from_u8(fields[12].trim().parse().unwrap_or(2))
                .unwrap_or(Alignment::BottomCenter),
            margins: Margins::new(
                fields[13].trim().parse().unwrap_or(10),
                fields[14].trim().parse().unwrap_or(10),
                fields[15].trim().parse().unwrap_or(10),
            ),
            encoding: FontEncoding::new(
                fields
                    .get(17)
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(1),
            ),
        })
    } else {
        Ok(Style {
            name: StyleRef::new(fields[0].trim()),
            font_name: fields[1].trim().to_string(),
            font_size: fields[2].trim().parse().unwrap_or(20.0),
            primary_color: AssColor::from_ass_hex(fields[3].trim()).unwrap_or(AssColor::WHITE),
            secondary_color: AssColor::from_ass_hex(fields[4].trim()).unwrap_or(AssColor::WHITE),
            outline_color: AssColor::from_ass_hex(fields[5].trim()).unwrap_or(AssColor::BLACK),
            shadow_color: AssColor::from_ass_hex(fields[6].trim()).unwrap_or(AssColor::BLACK),
            bold: flag(fields[7]),
            italic: flag(fields[8]),
            underline: flag(fields[9]),
            strikeout: flag(fields[10]),
            scale_x: fields[11].trim().parse().unwrap_or(100.0),
            scale_y: fields[12].trim().parse().unwrap_or(100.0),
            spacing: fields[13].trim().parse().unwrap_or(0.0),
            angle: fields[14].trim().parse().unwrap_or(0.0),
            border_style: BorderStyle::from_u8(fields[15].trim().parse().unwrap_or(1))
                .unwrap_or(BorderStyle::OutlineAndShadow),
            outline: fields[16].trim().parse().unwrap_or(2.0),
            shadow: fields[17].trim().parse().unwrap_or(2.0),
            alignment: Alignment::from_u8(fields[18].trim().parse().unwrap_or(2))
                .unwrap_or(Alignment::BottomCenter),
            margins: Margins::new(
                fields[19].trim().parse().unwrap_or(10),
                fields[20].trim().parse().unwrap_or(10),
                fields[21].trim().parse().unwrap_or(10),
            ),
            encoding: FontEncoding::new(fields[22].trim().parse().unwrap_or(1)),
        })
    }
}

/// Parse events from `[Events]` section lines.
pub fn parse_events(lines: &[Line]) -> (Vec<Event>, Vec<ParseError>) {
    let mut events = Vec::new();
    let mut errors = Vec::new();
    for line in lines {
        if let Token::EventData {
            ref type_name,
            ref data,
        } = line.token
        {
            let et = match type_name.trim() {
                "Dialogue" => EventType::Dialogue,
                "Comment" => EventType::Comment,
                "Picture" => EventType::Picture,
                "Sound" => EventType::Sound,
                "Movie" => EventType::Movie,
                "Command" => EventType::Command,
                _ => {
                    errors.push(ParseError::InvalidEvent {
                        detail: format!("unknown event type '{type_name}'"),
                        span: None,
                    });
                    continue;
                }
            };
            match parse_event_data(et, data) {
                Ok(ev) => events.push(ev),
                Err(e) => errors.push(e),
            }
        }
    }
    (events, errors)
}

fn parse_event_data(event_type: EventType, data: &str) -> Result<Event, ParseError> {
    let parts: Vec<&str> = data.splitn(10, ',').collect();
    if parts.len() < 10 {
        return Err(ParseError::InvalidEvent {
            detail: format!("expected 10 fields, got {}", parts.len()),
            span: None,
        });
    }

    let layer: u32 = parts[0].trim().parse().unwrap_or(0);
    let start =
        Timestamp::from_ass_time(parts[1].trim()).map_err(|_| ParseError::InvalidEvent {
            detail: format!("bad start time '{}'", parts[1].trim()),
            span: None,
        })?;
    let end = Timestamp::from_ass_time(parts[2].trim()).map_err(|_| ParseError::InvalidEvent {
        detail: format!("bad end time '{}'", parts[2].trim()),
        span: None,
    })?;

    let margin = |i: usize| {
        let v: u32 = parts[i].trim().parse().ok()?;
        if v == 0 {
            None
        } else {
            Some(v)
        }
    };

    Ok(Event {
        source_line: 0,
        event_type,
        layer,
        start_ms: start.as_ms(),
        end_ms: end.as_ms(),
        style: StyleRef::new(parts[3].trim()),
        actor: parts[4].trim().to_string(),
        margin_l: margin(5),
        margin_r: margin(6),
        margin_v: margin(7),
        effect: crate::effect::parse_effect(parts[8]),
        text_raw: parts[9].to_string(),
        override_tags: parse_tags(parts[9]).0,
        karaoke: parse_tags(parts[9]).1,
    })
}

/// Parse embedded font references from `[Fonts]` section lines.
pub fn parse_fonts(lines: &[Line]) -> Vec<EmbeddedFont> {
    let mut fonts = Vec::new();
    for line in lines {
        if let Token::FontLine(ref data) = line.token {
            if let Some(font) = parse_font_line(data) {
                fonts.push(font);
            }
        }
    }
    fonts
}

fn parse_font_line(data: &str) -> Option<EmbeddedFont> {
    let mut font_name = String::new();
    let mut filename = String::new();
    for part in data.split(',') {
        let p = part.trim();
        if let Some(name) = p
            .strip_prefix("fontname:")
            .or_else(|| p.strip_prefix("Fontname:"))
        {
            font_name = name.trim().to_string();
        } else if let Some(f) = p
            .strip_prefix("filename:")
            .or_else(|| p.strip_prefix("Filename:"))
        {
            filename = f.trim().to_string();
        }
    }
    if font_name.is_empty() {
        None
    } else {
        Some(EmbeddedFont {
            font_name,
            filename,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(tok: Token) -> Line {
        Line {
            token: tok,
            span: crate::Span::new(1, 1, 0),
        }
    }

    #[test]
    fn script_info_play_res() {
        let lines = vec![
            make_line(Token::KeyValue {
                key: "PlayResX".into(),
                value: "1920".into(),
            }),
            make_line(Token::KeyValue {
                key: "PlayResY".into(),
                value: "1080".into(),
            }),
        ];
        let meta = parse_script_info(&lines);
        assert_eq!(meta.play_res_x, 1920);
        assert_eq!(meta.play_res_y, 1080);
    }

    #[test]
    fn v4plus_style() {
        let lines = vec![make_line(Token::StyleData(
            "Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1".into()
        ))];
        let (styles, errors) = parse_styles(&lines, false);
        assert_eq!(errors.len(), 0);
        assert_eq!(styles[0].font_size, 48.0);
    }

    #[test]
    fn dialogue_event() {
        let lines = vec![make_line(Token::EventData {
            type_name: "Dialogue".into(),
            data: "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello".into(),
        })];
        let (events, errors) = parse_events(&lines);
        assert_eq!(errors.len(), 0);
        assert_eq!(events[0].start_ms, 1000);
        assert_eq!(events[0].text_raw, "Hello");
    }

    #[test]
    fn comment_event_skipped() {
        let lines = vec![make_line(Token::EventData {
            type_name: "Comment".into(),
            data: "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,note".into(),
        })];
        let (events, _) = parse_events(&lines);
        assert_eq!(events[0].event_type, EventType::Comment);
    }

    #[test]
    fn unknown_event_type() {
        let lines = vec![make_line(Token::EventData {
            type_name: "Unknown".into(),
            data: "".into(),
        })];
        let (_, errors) = parse_events(&lines);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn font_line_parsed() {
        let lines = vec![make_line(Token::FontLine(
            "fontname: Arial, filename: arial.ttf".into(),
        ))];
        let fonts = parse_fonts(&lines);
        assert_eq!(fonts.len(), 1);
        assert_eq!(fonts[0].font_name, "Arial");
    }

    #[test]
    fn margin_opt() {
        let lines = vec![make_line(Token::EventData {
            type_name: "Dialogue".into(),
            data: "0,0:00:01.00,0:00:05.00,Default,,10,0,20,,Hi".into(),
        })];
        let (events, _) = parse_events(&lines);
        assert_eq!(events[0].margin_l, Some(10));
        assert_eq!(events[0].margin_r, None); // 0 → None
        assert_eq!(events[0].margin_v, Some(20));
    }
}
