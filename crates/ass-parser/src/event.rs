use super::timestamp::Timestamp;
use super::override_tag::OverrideTag;
use super::karaoke::KaraokeSegment;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    Dialogue,
    Comment,
    Picture,
    Sound,
    Movie,
    Command,
}

impl EventType {
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

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub event_type: EventType,
    pub layer: u32,
    pub start: Timestamp,
    pub end: Timestamp,
    pub style_name: String,
    pub name: String,
    pub margin_l: u32,
    pub margin_r: u32,
    pub margin_v: u32,
    pub effect: String,
    pub text: String,
    pub override_tags: Vec<OverrideTag>,
    pub karaoke_segments: Vec<KaraokeSegment>,
    pub raw_override_block: String,
}

impl Event {
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

    pub fn duration_ms(&self) -> u64 {
        self.start.duration_ms(self.end)
    }

    pub fn is_dialogue(&self) -> bool {
        self.event_type == EventType::Dialogue
    }

    pub fn has_override_tags(&self) -> bool {
        !self.override_tags.is_empty()
    }

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
    None
}
