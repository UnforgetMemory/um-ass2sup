pub mod color;
pub mod error;
pub mod event;
pub mod karaoke;
pub mod override_tag;
pub mod srt;
pub mod style;
pub mod timestamp;

use std::collections::HashMap;
use std::path::Path;

pub use color::AssColor;
pub use error::ParseError;
pub use event::{Event, EventType};
pub use karaoke::{KaraokeSegment, KaraokeStyle};
pub use override_tag::OverrideTag;
pub use style::Style;
pub use timestamp::Timestamp;

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptInfo {
    pub title: String,
    pub script_type: String,
    pub wrap_style: u8,
    pub scaled_border_and_shadow: bool,
    pub ycbcr_matrix: String,
    pub play_res_x: u32,
    pub play_res_y: u32,
    pub extra: HashMap<String, String>,
}

impl Default for ScriptInfo {
    fn default() -> Self {
        Self {
            title: String::new(),
            script_type: "v4.00+".to_string(),
            wrap_style: 0,
            scaled_border_and_shadow: true,
            ycbcr_matrix: "None".to_string(),
            play_res_x: 1920,
            play_res_y: 1080,
            extra: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SubtitleFormat {
    Ass,
    Ssa,
    Srt,
}

impl SubtitleFormat {
    pub fn detect(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "ass" => Some(Self::Ass),
            "ssa" => Some(Self::Ssa),
            "srt" => Some(Self::Srt),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssFile {
    pub format: SubtitleFormat,
    pub script_info: ScriptInfo,
    pub styles: Vec<Style>,
    pub events: Vec<Event>,
}

impl AssFile {
    pub fn new() -> Self {
        Self {
            format: SubtitleFormat::Ass,
            script_info: ScriptInfo::default(),
            styles: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn parse(content: &str) -> Result<Self, ParseError> {
        let mut ass = Self::new();
        let mut current_section = String::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('!') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
                continue;
            }
            match current_section.as_str() {
                "Script Info" => ass.parse_script_info(line)?,
                "V4+ Styles" | "V4 Styles" => ass.parse_style_line(line)?,
                "Events" => ass.parse_event_line(line)?,
                _ => {}
            }
        }
        Ok(ass)
    }

    pub fn parse_file(path: &Path) -> Result<Self, ParseError> {
        let content = std::fs::read_to_string(path)?;
        let format = SubtitleFormat::detect(path).unwrap_or(SubtitleFormat::Ass);
        let mut ass = Self::parse(&content)?;
        ass.format = format;
        Ok(ass)
    }

    fn parse_script_info(&mut self, line: &str) -> Result<(), ParseError> {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "Title" => self.script_info.title = value.to_string(),
                "ScriptType" => self.script_info.script_type = value.to_string(),
                "WrapStyle" => self.script_info.wrap_style = value.parse().unwrap_or(0),
                "ScaledBorderAndShadow" => {
                    self.script_info.scaled_border_and_shadow = value.to_lowercase() == "yes"
                }
                "YCbCr Matrix" => self.script_info.ycbcr_matrix = value.to_string(),
                "PlayResX" => self.script_info.play_res_x = value.parse().unwrap_or(1920),
                "PlayResY" => self.script_info.play_res_y = value.parse().unwrap_or(1080),
                _ => {
                    self.script_info.extra.insert(key.to_string(), value.to_string());
                }
            }
        }
        Ok(())
    }

    fn parse_style_line(&mut self, line: &str) -> Result<(), ParseError> {
        if line.starts_with("Style:") {
            let style_data = line.trim_start_matches("Style:").trim();
            let style = Style::parse_from_line(style_data)?;
            self.styles.push(style);
        }
        Ok(())
    }

    fn parse_event_line(&mut self, line: &str) -> Result<(), ParseError> {
        if let Some(colon_pos) = line.find(':') {
            let type_str = &line[..colon_pos];
            if let Some(event_type) = EventType::from_str(type_str) {
                let event_data = line[colon_pos + 1..].trim();
                let event = Event::parse_from_line(event_type, event_data)?;
                self.events.push(event);
            }
        }
        Ok(())
    }

    pub fn dialogue_events(&self) -> impl Iterator<Item = &Event> {
        self.events.iter().filter(|e| e.is_dialogue())
    }

    pub fn find_style(&self, name: &str) -> Option<&Style> {
        self.styles.iter().find(|s| s.name == name)
    }

    pub fn resolution(&self) -> (u32, u32) {
        (self.script_info.play_res_x, self.script_info.play_res_y)
    }
}
