use super::timestamp::Timestamp;
use super::override_tag::OverrideTag;
use super::karaoke::KaraokeSegment;

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
    pub fn from_str(s: &str) -> Option<Self> {
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
    /// Effect name (e.g., `"Banner;5;0;scroll"` for scrolling text).
    pub effect: String,
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
        let effect = parts[8].trim().to_string();
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

fn parse_text_with_tags(text: &str) -> (Vec<OverrideTag>, Vec<KaraokeSegment>, String) {
    let mut tags = Vec::new();
    let karaoke = Vec::new();
    let mut raw_block = String::new();
    let mut chars = text.chars().peekable();
    let mut in_override = false;
    let mut current_tag = String::new();
    let mut current_text = String::new();

    while let Some(c) = chars.next() {
        if c == '{' {
            in_override = true;
            current_tag.clear();
            continue;
        }
        if c == '}' {
            in_override = false;
            raw_block.push_str(&current_tag);
            for part in current_tag.split('\\').filter(|s| !s.is_empty()) {
                if let Some(tag) = parse_single_tag(part) {
                    tags.push(tag);
                }
            }
            current_tag.clear();
            continue;
        }
        if in_override {
            current_tag.push(c);
        } else {
            current_text.push(c);
        }
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
    if s.starts_with("fn") {
        return Some(OverrideTag::FontName(s[2..].to_string()));
    }
    if s.starts_with("fs") {
        if let Ok(size) = s[2..].parse() {
            return Some(OverrideTag::FontSize(size));
        }
    }
    if s == "b1" || s == "b-1" {
        return Some(OverrideTag::Bold(s == "b1"));
    }
    if s.starts_with("b") {
        if let Ok(weight) = s[1..].parse() {
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
    if s.starts_with("an") {
        if let Ok(align) = s[2..].parse::<u8>() {
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
    if s.starts_with("k") || s.starts_with("kf") || s.starts_with("ko") || s.starts_with("kt") {
        let (tag_str, dur_str) = if s.starts_with("kf") { ("kf", &s[2..]) }
            else if s.starts_with("ko") { ("ko", &s[2..]) }
            else if s.starts_with("kt") { ("kt", &s[2..]) }
            else { ("k", &s[1..]) };
        if let Some(style) = super::karaoke::KaraokeStyle::from_tag(tag_str) {
            if let Ok(dur) = dur_str.parse::<u64>() {
                return Some(OverrideTag::Karaoke { style, duration: dur * 10 });
            }
        }
    }
    // \t(tag,duration,accel) or \t(tag,duration) or \t(tag) — transform animation
    if s.starts_with("t(") {
        let inner = s.trim_start_matches("t(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split(',').collect();
        if !parts.is_empty() {
            let tag = parts[0].trim().to_string();
            let t1 = parts.get(1).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
            let t2 = parts.get(2).and_then(|v| v.trim().parse().ok()).unwrap_or(t1);
            let accel = parts.get(3).and_then(|v| v.trim().parse().ok()).unwrap_or(1.0);
            return Some(OverrideTag::Transform { tag, t1, t2, accel });
        }
    }
    // \clip(x1,y1,x2,y2) — rectangular clip
    if s.starts_with("clip(") {
        let inner = s.trim_start_matches("clip(").trim_end_matches(')');
        let nums: Vec<f64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 4 {
            return Some(OverrideTag::Clip { x1: nums[0], y1: nums[1], x2: nums[2], y2: nums[3] });
        }
    }
    // \iclip(x1,y1,x2,y2) — inverse rectangular clip
    if s.starts_with("iclip(") {
        let inner = s.trim_start_matches("iclip(").trim_end_matches(')');
        let nums: Vec<f64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 4 {
            return Some(OverrideTag::ClipInverse { x1: nums[0], y1: nums[1], x2: nums[2], y2: nums[3] });
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
    if s.starts_with("frz") {
        if let Ok(z) = s[3..].parse::<f64>() {
            return Some(OverrideTag::Rotation { x: 0.0, y: 0.0, z });
        }
    }
    if s.starts_with("frx") {
        if let Ok(x) = s[3..].parse::<f64>() {
            return Some(OverrideTag::Rotation { x, y: 0.0, z: 0.0 });
        }
    }
    if s.starts_with("fry") {
        if let Ok(y) = s[3..].parse::<f64>() {
            return Some(OverrideTag::Rotation { x: 0.0, y, z: 0.0 });
        }
    }
    if s.starts_with("fr") {
        if let Ok(z) = s[2..].parse::<f64>() {
            return Some(OverrideTag::Rotation { x: 0.0, y: 0.0, z });
        }
    }
    // \fscx(pct), \fscy(pct) — scale
    if s.starts_with("fscx") {
        if let Ok(x) = s[4..].parse::<f64>() {
            return Some(OverrideTag::Scale { x, y: 100.0 });
        }
    }
    if s.starts_with("fscy") {
        if let Ok(y) = s[4..].parse::<f64>() {
            return Some(OverrideTag::Scale { x: 100.0, y });
        }
    }
    // \fax(shear), \fay(shear) — shear
    if s.starts_with("fax") {
        if let Ok(x) = s[3..].parse::<f64>() {
            return Some(OverrideTag::Shear { x, y: 0.0 });
        }
    }
    if s.starts_with("fay") {
        if let Ok(y) = s[3..].parse::<f64>() {
            return Some(OverrideTag::Shear { x: 0.0, y });
        }
    }
    // \xbord(w), \ybord(w) — border X/Y
    if s.starts_with("xbord") {
        if let Ok(w) = s[5..].parse::<f64>() {
            return Some(OverrideTag::BorderX(w));
        }
    }
    if s.starts_with("ybord") {
        if let Ok(w) = s[5..].parse::<f64>() {
            return Some(OverrideTag::BorderY(w));
        }
    }
    // \xshad(d), \yshad(d) — shadow X/Y
    if s.starts_with("xshad") {
        if let Ok(d) = s[5..].parse::<f64>() {
            return Some(OverrideTag::ShadowX(d));
        }
    }
    if s.starts_with("yshad") {
        if let Ok(d) = s[5..].parse::<f64>() {
            return Some(OverrideTag::ShadowY(d));
        }
    }
    // \be(strength) — blur edge
    if s.starts_with("be") {
        if let Ok(v) = s[2..].parse::<f64>() {
            return Some(OverrideTag::Blur(v));
        }
    }
    // \q(style) — wrap style 0-3
    if s.starts_with("q") {
        if let Ok(v) = s[1..].parse::<u8>() {
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
    if s.starts_with("pbo") {
        if let Ok(v) = s[3..].parse::<f64>() {
            return Some(OverrideTag::BaselineOffset(v));
        }
    }
    // \1c, \2c, \3c, \4c — color aliases
    for (prefix, variant) in [("1c", "primary"), ("2c", "secondary"), ("3c", "outline"), ("4c", "shadow")] {
        if s.starts_with(prefix) {
            let color_str = &s[prefix.len()..];
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
    if s.starts_with("alpha") {
        let val_str = &s[5..];
        if let Ok(v) = parse_hex_u8(val_str) {
            return Some(OverrideTag::Alpha { value: v });
        }
    }
    // \1a, \2a, \3a, \4a — alpha aliases
    for (prefix, variant) in [("1a", "primary"), ("2a", "secondary"), ("3a", "outline"), ("4a", "shadow")] {
        if s.starts_with(prefix) {
            let val_str = &s[prefix.len()..];
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
    if s.starts_with("fe") {
        if let Ok(v) = s[2..].parse::<u8>() {
            return Some(OverrideTag::Charset(v));
        }
    }
    None
}
