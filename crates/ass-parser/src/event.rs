use super::effect::{parse_effect, Effect};
use super::timestamp::Timestamp;
use super::override_tag::OverrideTag;
use super::karaoke::{KaraokeSegment, KaraokeStyle};

/// ASS/SSA event type (first field of an event line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    /// Visible subtitle dialogue
    Dialogue,
    /// Non-rendered comment
    Comment,
    /// Picture overlay (rare)
    Picture,
    /// Sound effect (rare)
    Sound,
    /// Movie overlay (rare)
    Movie,
    /// Command (rare)
    Command,
}

impl EventType {
    /// Parses an event type string (`"Dialogue"`, `"Comment"`, etc.).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "Dialogue" => Some(Self::Dialogue),
            "Comment" => Some(Self::Comment),
            "Picture" => Some(Self::Picture),
            "Sound" => Some(Self::Sound),
            "Movie" => Some(Self::Movie),
            "Command" => Some(Self::Command),
            _ => None,
        }
    }
}

/// A parsed ASS/SSA subtitle event (dialogue or comment line).
///
/// Each event represents a single subtitle display with timing, styling, and text content.
/// The [`text`] field contains the raw subtitle text with embedded override tags (e.g., `{\b1}Bold{\b0}`),
/// while [`override_tags`] and [`karaoke_segments`] hold the parsed representations.
///
/// # ASS Event Line Format
///
/// An ASS event line has 10 comma-separated fields:
/// ```text
/// Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World
///           ^layer ^start     ^end      ^style  ^name ^mL ^mR ^mV ^effect ^text
/// ```
///
/// [`text`]: Event::text
/// [`override_tags`]: Event::override_tags
/// [`karaoke_segments`]: Event::karaoke_segments
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Event type — `Dialogue` for visible subtitles, `Comment` for non-rendered notes.
    pub event_type: EventType,
    /// Rendering layer (higher layers render on top).
    pub layer: u32,
    /// Display start time.
    pub start: Timestamp,
    /// Display end time.
    pub end: Timestamp,
    /// Name of the style from `[V4+ Styles]` section (e.g., `"Default"`).
    pub style_name: String,
    /// Optional actor/speaker name (rarely used).
    pub name: String,
    /// Left margin override in pixels (0 = use style default).
    pub margin_l: u32,
    /// Right margin override in pixels (0 = use style default).
    pub margin_r: u32,
    /// Vertical margin override in pixels (0 = use style default).
    pub margin_v: u32,
    /// Effect applied to the subtitle text (banner, scroll, karaoke, or none).
    pub effect: Effect,
    /// Raw subtitle text including override tag blocks (e.g., `"{\b1}Bold{\b0} normal"`).
    pub text: String,
    /// Parsed override tags extracted from `{\\tag}` blocks in [`text`](Event::text).
    pub override_tags: Vec<OverrideTag>,
    /// Parsed karaoke segments (populated when karaoke tags are present).
    pub karaoke_segments: Vec<KaraokeSegment>,
    /// Concatenated raw content of all override blocks (for round-trip fidelity).
    pub raw_override_block: String,
}

impl Event {
    /// Parses an event from its comma-separated data fields.
    ///
    /// The `data` parameter should contain the 9 fields after the event type prefix:
    /// `Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text`
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::InvalidEvent`] if fewer than 10 fields are present,
    /// or [`ParseError::InvalidTimestamp`] if start/end timecodes are malformed.
    pub fn parse_from_line(event_type: EventType, data: &str) -> Result<Self, super::error::ParseError> {
        let parts: Vec<&str> = data.splitn(10, ',').collect();
        if parts.len() < 10 {
            return Err(super::error::ParseError::InvalidEvent(format!(
                "expected 10 fields, got {}", parts.len()
            )));
        }
        let layer: u32 = parts[0].trim().parse().unwrap_or(0);
        let start = Timestamp::from_ass_time(parts[1].trim())?;
        let end = Timestamp::from_ass_time(parts[2].trim())?;
        let style_name = parts[3].trim().to_string();
        let name = parts[4].trim().to_string();
        let margin_l: u32 = parts[5].trim().parse().unwrap_or(0);
        let margin_r: u32 = parts[6].trim().parse().unwrap_or(0);
        let margin_v: u32 = parts[7].trim().parse().unwrap_or(0);
        let effect = parse_effect(parts[8]);
        let text = parts[9].trim().to_string();
        let (override_tags, karaoke_segments, raw_override_block) = parse_text_with_tags(&text);
        Ok(Self {
            event_type,
            layer,
            start,
            end,
            style_name,
            name,
            margin_l,
            margin_r,
            margin_v,
            effect,
            text,
            override_tags,
            karaoke_segments,
            raw_override_block,
        })
    }

    /// Returns the display duration in milliseconds (`end - start`).
    pub fn duration_ms(&self) -> u64 {
        self.start.duration_ms(self.end)
    }

    /// Returns `true` if this event is a visible subtitle (not a comment).
    pub fn is_dialogue(&self) -> bool {
        self.event_type == EventType::Dialogue
    }

    /// Returns `true` if the text contains any override tag blocks (`{\\tag}`).
    pub fn has_override_tags(&self) -> bool {
        !self.override_tags.is_empty()
    }

    /// Returns `true` if karaoke timing tags (`\\k`, `\\kf`, `\\ko`, `\\kt`) were found.
    pub fn has_karaoke(&self) -> bool {
        !self.karaoke_segments.is_empty()
    }
}

/// Split a tag string by `\` while respecting parenthesis nesting.
/// `\t(\pos(960,540),0,3000,1)` should NOT be split at the inner `\pos`.
fn split_tags_respecting_parens(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut paren_depth: usize = 0;

    for ch in s.chars() {
        match ch {
            '(' => {
                current.push(ch);
                paren_depth += 1;
            }
            ')' if paren_depth > 0 => {
                current.push(ch);
                paren_depth -= 1;
            }
            '\\' if paren_depth == 0 => {
                if !current.is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

fn parse_text_with_tags(text: &str) -> (Vec<OverrideTag>, Vec<KaraokeSegment>, String) {
    let mut tags = Vec::new();
    let mut karaoke = Vec::new();
    let mut raw_block = String::new();
    let chars = text.chars().peekable();
    let mut in_override = false;
    let mut current_tag = String::new();

    let mut pending_karaoke: Option<(KaraokeStyle, u64)> = None;
    let mut syllable_text = String::new();
    let mut segment_index = 0usize;

    for c in chars {
        if c == '{' {
            in_override = true;
            current_tag.clear();
            continue;
        }
        if c == '}' {
            in_override = false;
            raw_block.push_str(&current_tag);
            for part in split_tags_respecting_parens(&current_tag) {
                if let Some(tag) = parse_single_tag(&part) {
                    if let OverrideTag::Karaoke { style, duration } = &tag {
                        if let Some((prev_style, prev_dur)) = pending_karaoke.take() {
                            karaoke.push(KaraokeSegment::new(
                                prev_style,
                                prev_dur,
                                std::mem::take(&mut syllable_text),
                                segment_index,
                            ));
                            segment_index += 1;
                        }
                        pending_karaoke = Some((*style, *duration));
                    }
                    tags.push(tag);
                }
            }
            current_tag.clear();
            continue;
        }
        if in_override {
            current_tag.push(c);
        } else if pending_karaoke.is_some() {
            syllable_text.push(c);
        }
    }

    if let Some((style, duration)) = pending_karaoke.take() {
        karaoke.push(KaraokeSegment::new(
            style,
            duration,
            std::mem::take(&mut syllable_text),
            segment_index,
        ));
    }

    (tags, karaoke, raw_block)
}

fn parse_hex_u8(s: &str) -> Result<u8, std::num::ParseIntError> {
    let s = s.trim().trim_start_matches("H").trim_start_matches("h").trim_end_matches('&');
    u8::from_str_radix(s, 16)
}

fn parse_ass_color(s: &str) -> Result<super::color::AssColor, ()> {
    let s = s.trim().trim_start_matches("H").trim_start_matches("h").trim_end_matches('&');
    if s.len() < 6 { return Err(()); }
    let hex = if s.len() >= 8 { &s[s.len()-8..] } else { s };
    let parse = |range: &str| u8::from_str_radix(range, 16).map_err(|_| ());
    if hex.len() == 8 {
        let alpha = parse(&hex[0..2])?;
        let blue = parse(&hex[2..4])?;
        let green = parse(&hex[4..6])?;
        let red = parse(&hex[6..8])?;
        Ok(super::color::AssColor { alpha, blue, green, red })
    } else if hex.len() == 6 {
        let blue = parse(&hex[0..2])?;
        let green = parse(&hex[2..4])?;
        let red = parse(&hex[4..6])?;
        Ok(super::color::AssColor { alpha: 0, blue, green, red })
    } else {
        Err(())
    }
}

/// Splits a string by commas that are NOT inside parentheses.
/// Needed for `\t(\pos(100,200),0,1000,1)` where the inner tag may contain commas.
fn split_commas_paren_aware(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

fn parse_single_tag(s: &str) -> Option<OverrideTag> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.starts_with("pos(") {
        let inner = s.trim_start_matches("pos(").trim_end_matches(')');
        let nums: Vec<f64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 2 {
            return Some(OverrideTag::Pos { x: nums[0], y: nums[1] });
        }
    }
    if s.starts_with("move(") {
        let inner = s.trim_start_matches("move(").trim_end_matches(')');
        let nums: Vec<f64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 4 {
            let (t1, t2) = if nums.len() >= 6 { (nums[4] as u64, nums[5] as u64) } else { (0, 0) };
            return Some(OverrideTag::Move { x1: nums[0], y1: nums[1], x2: nums[2], y2: nums[3], t1, t2 });
        }
    }
    if s.starts_with("fad(") || s.starts_with("fade(") {
        let inner = s.trim_start_matches("fad(").trim_start_matches("fade(").trim_end_matches(')');
        let nums: Vec<u64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 2 {
            return Some(OverrideTag::Fade { duration_in: nums[0], duration_out: nums[1] });
        }
    }
    if let Some(stripped) = s.strip_prefix("fn") {
        return Some(OverrideTag::FontName(stripped.to_string()));
    }
    if let Some(stripped) = s.strip_prefix("fs") {
        if let Ok(size) = stripped.parse() {
            return Some(OverrideTag::FontSize(size));
        }
    }
    if s == "b1" || s == "b-1" {
        return Some(OverrideTag::Bold(s == "b1"));
    }
    if let Some(stripped) = s.strip_prefix("b") {
        if let Ok(weight) = stripped.parse() {
            return Some(OverrideTag::BoldWeight(weight));
        }
    }
    if s == "i1" || s == "i-1" {
        return Some(OverrideTag::Italic(s == "i1"));
    }
    if s == "u1" || s == "u-1" {
        return Some(OverrideTag::Underline(s == "u1"));
    }
    if s == "s1" || s == "s-1" {
        return Some(OverrideTag::Strikeout(s == "s1"));
    }
    if let Some(stripped) = s.strip_prefix("an") {
        if let Ok(align) = stripped.parse::<u8>() {
            if (1..=9).contains(&align) {
                return Some(OverrideTag::AlignmentNumpad(align));
            }
        }
    }
    if s.starts_with("a") && !s.starts_with("an") {
        if let Ok(align) = s[1..].parse::<u8>() {
            return Some(OverrideTag::Alignment(align));
        }
    }
    if let Some(stripped) = s.strip_prefix("k") {
        let (tag_str, dur_str) = if let Some(stripped) = stripped.strip_prefix("f") { ("kf", stripped) }
            else if let Some(stripped) = stripped.strip_prefix('o') { ("ko", stripped) }
            else if let Some(stripped) = stripped.strip_prefix('t') { ("kt", stripped) }
            else { ("k", stripped) };
        if let Some(style) = super::karaoke::KaraokeStyle::from_tag(tag_str) {
            if let Ok(dur) = dur_str.parse::<u64>() {
                return Some(OverrideTag::Karaoke { style, duration: dur * 10 });
            }
        }
    }
    // \t(tag,duration,accel) or \t(tag,duration) or \t(tag) — transform animation
    if s.starts_with("t(") {
        let inner = s.trim_start_matches("t(").trim_end_matches(')');
        let parts: Vec<&str> = split_commas_paren_aware(inner);
        if !parts.is_empty() {
            let tag = parts[0].trim().to_string();
            let t1 = parts.get(1).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
            let t2 = parts.get(2).and_then(|v| v.trim().parse().ok()).unwrap_or(t1);
            let accel = parts.get(3).and_then(|v| v.trim().parse().ok()).unwrap_or(1.0);
            return Some(OverrideTag::Transform { tag, t1, t2, accel });
        }
    }
    // \clip(x1,y1,x2,y2) — rectangular clip, or \clip(scale,commands) — vector clip
    if s.starts_with("clip(") {
        let inner = s.trim_start_matches("clip(").trim_end_matches(')');
        let nums: Vec<f64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 4 {
            return Some(OverrideTag::Clip { x1: nums[0], y1: nums[1], x2: nums[2], y2: nums[3] });
        }
        // Vector drawing form: \clip(scale, drawing_commands)
        if let Some(comma_pos) = inner.find(',') {
            let scale_str = inner[..comma_pos].trim();
            let commands = inner[comma_pos + 1..].trim();
            if let Ok(scale) = scale_str.parse::<f32>() {
                return Some(OverrideTag::ClipDrawing { scale, commands: commands.to_string() });
            }
        }
    }
    // \iclip(x1,y1,x2,y2) — inverse rectangular clip, or \iclip(scale,commands) — inverse vector clip
    if s.starts_with("iclip(") {
        let inner = s.trim_start_matches("iclip(").trim_end_matches(')');
        let nums: Vec<f64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 4 {
            return Some(OverrideTag::ClipInverse { x1: nums[0], y1: nums[1], x2: nums[2], y2: nums[3] });
        }
        // Vector drawing form: \iclip(scale, drawing_commands)
        if let Some(comma_pos) = inner.find(',') {
            let scale_str = inner[..comma_pos].trim();
            let commands = inner[comma_pos + 1..].trim();
            if let Ok(scale) = scale_str.parse::<f32>() {
                return Some(OverrideTag::ClipInverseDrawing { scale, commands: commands.to_string() });
            }
        }
    }
    // \fade(a1,a2,a3,t1,t2,t3,t4) — 7-parameter complex fade
    if s.starts_with("fade(") && s.matches(',').count() >= 6 {
        let inner = s.trim_start_matches("fade(").trim_end_matches(')');
        let nums: Vec<u64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 7 {
            return Some(OverrideTag::FadeComplex {
                alpha_start: nums[0] as u8, alpha_mid: nums[1] as u8, alpha_end: nums[2] as u8,
                t1: nums[3], t2: nums[4], t3: nums[5], t4: nums[6],
            });
        }
    }
    // \org(x,y) — rotation origin
    if s.starts_with("org(") {
        let inner = s.trim_start_matches("org(").trim_end_matches(')');
        let nums: Vec<f64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 2 {
            return Some(OverrideTag::Origin { x: nums[0], y: nums[1] });
        }
    }
    // \frz(angle), \fr(angle), \frx(angle), \fry(angle) — rotation
    if let Some(stripped) = s.strip_prefix("frz") {
        if let Ok(z) = stripped.parse::<f64>() {
            return Some(OverrideTag::Rotation { x: 0.0, y: 0.0, z });
        }
    }
    if let Some(stripped) = s.strip_prefix("frx") {
        if let Ok(x) = stripped.parse::<f64>() {
            return Some(OverrideTag::Rotation { x, y: 0.0, z: 0.0 });
        }
    }
    if let Some(stripped) = s.strip_prefix("fry") {
        if let Ok(y) = stripped.parse::<f64>() {
            return Some(OverrideTag::Rotation { x: 0.0, y, z: 0.0 });
        }
    }
    if let Some(stripped) = s.strip_prefix("fr") {
        if let Ok(z) = stripped.parse::<f64>() {
            return Some(OverrideTag::Rotation { x: 0.0, y: 0.0, z });
        }
    }
    // \fscx(pct), \fscy(pct) — scale
    if let Some(stripped) = s.strip_prefix("fscx") {
        if let Ok(x) = stripped.parse::<f64>() {
            return Some(OverrideTag::Scale { x, y: 100.0 });
        }
    }
    if let Some(stripped) = s.strip_prefix("fscy") {
        if let Ok(y) = stripped.parse::<f64>() {
            return Some(OverrideTag::Scale { x: 100.0, y });
        }
    }
    // \fax(shear), \fay(shear) — shear
    if let Some(stripped) = s.strip_prefix("fax") {
        if let Ok(x) = stripped.parse::<f64>() {
            return Some(OverrideTag::Shear { x, y: 0.0 });
        }
    }
    if let Some(stripped) = s.strip_prefix("fay") {
        if let Ok(y) = stripped.parse::<f64>() {
            return Some(OverrideTag::Shear { x: 0.0, y });
        }
    }
    // \xbord(w), \ybord(w) — border X/Y
    if let Some(stripped) = s.strip_prefix("xbord") {
        if let Ok(w) = stripped.parse::<f64>() {
            return Some(OverrideTag::BorderX(w));
        }
    }
    if let Some(stripped) = s.strip_prefix("ybord") {
        if let Ok(w) = stripped.parse::<f64>() {
            return Some(OverrideTag::BorderY(w));
        }
    }
    // \xshad(d), \yshad(d) — shadow X/Y
    if let Some(stripped) = s.strip_prefix("xshad") {
        if let Ok(d) = stripped.parse::<f64>() {
            return Some(OverrideTag::ShadowX(d));
        }
    }
    if let Some(stripped) = s.strip_prefix("yshad") {
        if let Ok(d) = stripped.parse::<f64>() {
            return Some(OverrideTag::ShadowY(d));
        }
    }
    // \be(strength) — blur edge
    if let Some(stripped) = s.strip_prefix("be") {
        if let Ok(v) = stripped.parse::<f64>() {
            return Some(OverrideTag::Blur(v));
        }
    }
    // \q(style) — wrap style 0-3
    if let Some(stripped) = s.strip_prefix("q") {
        if let Ok(v) = stripped.parse::<u8>() {
            if v <= 3 {
                return Some(OverrideTag::WrapStyle(v));
            }
        }
    }
    // \p(level) — drawing mode (0=off, 1+=on)
    if s.starts_with("p") && !s.starts_with("pos") && !s.starts_with("pbo") {
        if let Ok(v) = s[1..].parse::<u8>() {
            return Some(OverrideTag::DrawingMode(v));
        }
    }
    // \pbo(offset) — baseline offset
    if let Some(stripped) = s.strip_prefix("pbo") {
        if let Ok(v) = stripped.parse::<f64>() {
            return Some(OverrideTag::BaselineOffset(v));
        }
    }
    // \writing_mode(N) — text direction (1=horizontal, 2=vertical-right, 3=vertical-left)
    if s.starts_with("writing_mode(") {
        let inner = s.trim_start_matches("writing_mode(").trim_end_matches(')');
        if let Ok(v) = inner.parse::<u8>() {
            return Some(OverrideTag::WritingMode(v));
        }
    }
    // \1c, \2c, \3c, \4c — color aliases
    for (prefix, variant) in [("1c", "primary"), ("2c", "secondary"), ("3c", "outline"), ("4c", "shadow")] {
        if let Some(color_str) = s.strip_prefix(prefix) {
            if let Ok(color) = parse_ass_color(color_str) {
                return Some(match variant {
                    "primary" => OverrideTag::PrimaryColor(color),
                    "secondary" => OverrideTag::SecondaryColor(color),
                    "outline" => OverrideTag::OutlineColor(color),
                    "shadow" => OverrideTag::ShadowColor(color),
                    _ => unreachable!(),
                });
            }
        }
    }
    // \alpha(value) — global alpha
    if let Some(val_str) = s.strip_prefix("alpha") {
        if let Ok(v) = parse_hex_u8(val_str) {
            return Some(OverrideTag::Alpha { value: v });
        }
    }
    // \1a, \2a, \3a, \4a — alpha aliases
    for (prefix, variant) in [("1a", "primary"), ("2a", "secondary"), ("3a", "outline"), ("4a", "shadow")] {
        if let Some(val_str) = s.strip_prefix(prefix) {
            if let Ok(v) = parse_hex_u8(val_str) {
                return Some(match variant {
                    "primary" => OverrideTag::PrimaryAlpha { value: v },
                    "secondary" => OverrideTag::SecondaryAlpha { value: v },
                    "outline" => OverrideTag::OutlineAlpha { value: v },
                    "shadow" => OverrideTag::ShadowAlpha { value: v },
                    _ => unreachable!(),
                });
            }
        }
    }
    // \r(style_name) — reset to style
    if s.starts_with("r") && !s.starts_with("reset") {
        return Some(OverrideTag::Reset(s[1..].to_string()));
    }
    // \fe(encoding) — font charset encoding
    if let Some(stripped) = s.strip_prefix("fe") {
        if let Ok(v) = stripped.parse::<u8>() {
            return Some(OverrideTag::Charset(v));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::karaoke::KaraokeStyle;

    #[test]
    fn event_writing_mode() {
        let result = parse_single_tag("writing_mode(2)").unwrap();
        assert_eq!(result, OverrideTag::WritingMode(2));
    }

    #[test]
    fn event_writing_mode_one() {
        let result = parse_single_tag("writing_mode(1)").unwrap();
        assert_eq!(result, OverrideTag::WritingMode(1));
    }

    #[test]
    fn event_transform_with_paren_inner_tag() {
        let result = parse_single_tag("t(\\pos(100,200),0,1000,1)").unwrap();
        assert_eq!(
            result,
            OverrideTag::Transform {
                tag: "\\pos(100,200)".to_string(),
                t1: 0,
                t2: 1000,
                accel: 1.0,
            }
        );
    }

    #[test]
    fn event_transform_with_simple_tag() {
        let result = parse_single_tag("t(\\b1,0,500,1)").unwrap();
        assert_eq!(
            result,
            OverrideTag::Transform {
                tag: "\\b1".to_string(),
                t1: 0,
                t2: 500,
                accel: 1.0,
            }
        );
    }

    #[test]
    fn event_transform_three_parts() {
        let result = parse_single_tag("t(\\i1,0,300)").unwrap();
        assert_eq!(
            result,
            OverrideTag::Transform {
                tag: "\\i1".to_string(),
                t1: 0,
                t2: 300,
                accel: 1.0,
            }
        );
    }

    #[test]
    fn event_transform_two_parts() {
        let result = parse_single_tag("t(\\fs20)").unwrap();
        assert_eq!(
            result,
            OverrideTag::Transform {
                tag: "\\fs20".to_string(),
                t1: 0,
                t2: 0,
                accel: 1.0,
            }
        );
    }

    #[test]
    fn event_split_commas_paren_aware_nested() {
        let parts = split_commas_paren_aware("\\clip(10,20,30,40),100,200");
        assert_eq!(parts, vec!["\\clip(10,20,30,40)", "100", "200"]);
    }

    // ── Karaoke segment tests ─────────────────────────────────────────

    #[test]
    fn karaoke_single_k_tag() {
        let (_tags, segments, _raw) = parse_text_with_tags("{\\k50}Hello");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].style, KaraokeStyle::Instant);
        assert_eq!(segments[0].duration_ms, 500); // 50 cs * 10 = 500 ms
        assert_eq!(segments[0].text, "Hello");
        assert_eq!(segments[0].index, 0);
    }

    #[test]
    fn karaoke_multiple_k_tags() {
        let (_tags, segments, _raw) = parse_text_with_tags("{\\k50}Hel{\\k100}lo World");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].style, KaraokeStyle::Instant);
        assert_eq!(segments[0].duration_ms, 500);
        assert_eq!(segments[0].text, "Hel");
        assert_eq!(segments[1].style, KaraokeStyle::Instant);
        assert_eq!(segments[1].duration_ms, 1000);
        assert_eq!(segments[1].text, "lo World");
    }

    #[test]
    fn karaoke_text_before_first_tag_not_included() {
        let (_tags, segments, _raw) = parse_text_with_tags("Plain{\\k50}Hello");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello");
    }

    #[test]
    fn karaoke_kf_variant() {
        let (_tags, segments, _raw) = parse_text_with_tags("{\\kf30}Fill!");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].style, KaraokeStyle::Fill);
        assert_eq!(segments[0].duration_ms, 300);
        assert_eq!(segments[0].text, "Fill!");
    }

    #[test]
    fn karaoke_ko_variant() {
        let (_tags, segments, _raw) = parse_text_with_tags("{\\ko20}Outline");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].style, KaraokeStyle::Outline);
        assert_eq!(segments[0].duration_ms, 200);
        assert_eq!(segments[0].text, "Outline");
    }

    #[test]
    fn karaoke_kt_variant() {
        let (_tags, segments, _raw) = parse_text_with_tags("{\\kt100}Timing");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].style, KaraokeStyle::Timing);
        assert_eq!(segments[0].duration_ms, 1000);
        assert_eq!(segments[0].text, "Timing");
    }

    #[test]
    fn karaoke_non_karaoke_override_blocks_transparent() {
        let (_tags, segments, _raw) = parse_text_with_tags("{\\k50\\b1}Hel{\\b0}lo{\\kf100} World");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].style, KaraokeStyle::Instant);
        assert_eq!(segments[0].duration_ms, 500);
        assert_eq!(segments[0].text, "Hello");
        assert_eq!(segments[1].style, KaraokeStyle::Fill);
        assert_eq!(segments[1].duration_ms, 1000);
        assert_eq!(segments[1].text, " World");
    }

    #[test]
    fn karaoke_no_karaoke_tags_empty() {
        let (_tags, segments, _raw) = parse_text_with_tags("{\\b1}Bold text{\\b0}");
        assert!(segments.is_empty());
    }

    #[test]
    fn karaoke_empty_text() {
        let (_tags, segments, _raw) = parse_text_with_tags("{\\k50}{\\k100}Text");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "");
        assert_eq!(segments[0].duration_ms, 500);
        assert_eq!(segments[1].text, "Text");
        assert_eq!(segments[1].duration_ms, 1000);
    }

    #[test]
    fn karaoke_override_tags_populated() {
        let (tags, _segments, _raw) = parse_text_with_tags("{\\k50\\b1}Hello{\\i1} World");
        assert!(tags.iter().any(|t| matches!(t, OverrideTag::Bold(true))));
        assert!(tags.iter().any(|t| matches!(t, OverrideTag::Karaoke { .. })));
        assert!(tags.iter().any(|t| matches!(t, OverrideTag::Italic(true))));
    }

    #[test]
    fn karaoke_via_event_parse() {
        let data = "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\k50}Hel{\\k100}lo World";
        let event = Event::parse_from_line(EventType::Dialogue, data).unwrap();
        assert_eq!(event.karaoke_segments.len(), 2);
        assert_eq!(event.karaoke_segments[0].text, "Hel");
        assert_eq!(event.karaoke_segments[0].duration_ms, 500);
        assert_eq!(event.karaoke_segments[1].text, "lo World");
        assert_eq!(event.karaoke_segments[1].duration_ms, 1000);
        assert!(event.has_karaoke());
    }

    // ── Vector clip tests ─────────────────────────────────────────────

    #[test]
    fn clip_vector_drawing() {
        let result = parse_single_tag("clip(1,m 0 0 l 100 0 100 100 0 100)").unwrap();
        assert_eq!(
            result,
            OverrideTag::ClipDrawing {
                scale: 1.0,
                commands: "m 0 0 l 100 0 100 100 0 100".to_string()
            }
        );
    }

    #[test]
    fn iclip_vector_drawing() {
        let result = parse_single_tag("iclip(2,m 0 0 l 50 0 50 50 0 50)").unwrap();
        assert_eq!(
            result,
            OverrideTag::ClipInverseDrawing {
                scale: 2.0,
                commands: "m 0 0 l 50 0 50 50 0 50".to_string()
            }
        );
    }

    #[test]
    fn clip_rectangular_unchanged() {
        let result = parse_single_tag("clip(10,20,30,40)").unwrap();
        assert_eq!(result, OverrideTag::Clip { x1: 10.0, y1: 20.0, x2: 30.0, y2: 40.0 });
    }

    #[test]
    fn iclip_rectangular_unchanged() {
        let result = parse_single_tag("iclip(10,20,30,40)").unwrap();
        assert_eq!(result, OverrideTag::ClipInverse { x1: 10.0, y1: 20.0, x2: 30.0, y2: 40.0 });
    }

    #[test]
    fn clip_vector_minimal_commands() {
        let result = parse_single_tag("clip(1,m 0 0)").unwrap();
        assert_eq!(result, OverrideTag::ClipDrawing { scale: 1.0, commands: "m 0 0".to_string() });
    }

    #[test]
    fn clip_vector_fractional_scale() {
        let result = parse_single_tag("clip(0.5,m 10 10 l 20 20)").unwrap();
        assert_eq!(result, OverrideTag::ClipDrawing { scale: 0.5, commands: "m 10 10 l 20 20".to_string() });
    }

    #[test]
    fn clip_vector_through_parse_text_with_tags() {
        let (_tags, _segments, _raw) =
            parse_text_with_tags("{\\clip(1,m 0 0 l 100 0 100 100 0 100)}Vector{\\clip(10,20,30,40)}Clip");
        assert!(_tags.iter().any(|t| matches!(t, OverrideTag::ClipDrawing { scale: 1.0, .. })));
        assert!(_tags.iter().any(|t| matches!(t, OverrideTag::Clip { x1: 10.0, .. })));
    }
}
