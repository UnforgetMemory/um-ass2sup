//! ASS/SSA/SRT subtitle file parser.
//!
//! This crate provides parsing and representation of Advanced SubStation Alpha (ASS),
//! SubStation Alpha (SSA), and SubRip (SRT) subtitle files.
//!
//! # Quick Start
//!
//! ```rust
//! use ass_parser::AssFile;
//!
//! let ass_content = r#"
//! [Script Info]
//! Title: Example
//! PlayResX: 1920
//! PlayResY: 1080
//!
//! [V4+ Styles]
//! Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
//! Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
//!
//! [Events]
//! Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
//! Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World
//! "#;
//!
//! let ass = AssFile::parse(ass_content).unwrap();
//! assert_eq!(ass.events.len(), 1);
//! ```
pub mod color;
pub mod effect;
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
pub use effect::Effect;
pub use error::ParseError;
pub use event::{Event, EventType};
pub use karaoke::{KaraokeSegment, KaraokeStyle};
pub use override_tag::parse_override_tag;
pub use override_tag::OverrideTag;
pub use style::Style;
pub use timestamp::Timestamp;

/// Script-level metadata from the `[Script Info]` section.
///
/// Contains resolution, script type, and other global settings that affect
/// how the subtitle file should be rendered.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptInfo {
    /// Title of the subtitle script.
    pub title: String,
    /// Script format version (e.g., "v4.00+").
    pub script_type: String,
    /// Word wrap mode: 0=smart, 1=end-of-line, 2=no word wrap, 3=simple.
    pub wrap_style: u8,
    /// Whether border and shadow widths are scaled with resolution.
    pub scaled_border_and_shadow: bool,
    /// YCbCr color matrix specification (e.g., "None", "TV.601").
    pub ycbcr_matrix: String,
    /// Horizontal script resolution in pixels.
    pub play_res_x: u32,
    /// Vertical script resolution in pixels.
    pub play_res_y: u32,
    /// Additional key-value pairs not covered by standard fields.
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

/// Supported subtitle file formats.
#[derive(Debug, Clone, PartialEq)]
pub enum SubtitleFormat {
    /// Advanced SubStation Alpha (.ass)
    Ass,
    /// SubStation Alpha (.ssa)
    Ssa,
    /// SubRip (.srt)
    Srt,
}

impl SubtitleFormat {
    /// Detect subtitle format from file extension.
    ///
    /// Returns `None` for unrecognized extensions.
    pub fn detect(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "ass" => Some(Self::Ass),
            "ssa" => Some(Self::Ssa),
            "srt" => Some(Self::Srt),
            _ => None,
        }
    }
}

/// An embedded font entry from the ASS `[Fonts]` section.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddedFont {
    /// Font family name (e.g. "Arial").
    pub font_name: String,
    /// Filename of the font file on disk.
    pub filename: String,
}

/// A parsed ASS/SSA/SRT subtitle file.
///
/// Contains script metadata, styles, dialogue events, and any embedded fonts.
/// Use [`AssFile::parse`] to create from a string, or [`AssFile::parse_file`]
/// to read from disk.
#[derive(Debug, Clone, PartialEq)]
pub struct AssFile {
    /// Detected subtitle format (ASS, SSA, or SRT).
    pub format: SubtitleFormat,
    /// Script-level metadata from `[Script Info]`.
    pub script_info: ScriptInfo,
    /// Named styles defined in `[V4+ Styles]` or `[V4 Styles]`.
    pub styles: Vec<Style>,
    /// Dialogue and other events from `[Events]`.
    pub events: Vec<Event>,
    /// Embedded font references from `[Fonts]`.
    pub embedded_fonts: Vec<EmbeddedFont>,
}

impl Default for AssFile {
    fn default() -> Self {
        Self::new()
    }
}

impl AssFile {
    /// Creates an empty ASS file with default script info (1920x1080, v4.00+).
    pub fn new() -> Self {
        Self {
            format: SubtitleFormat::Ass,
            script_info: ScriptInfo::default(),
            styles: Vec::new(),
            events: Vec::new(),
            embedded_fonts: Vec::new(),
        }
    }

    /// Parse ASS/SSA/SRT content from a string.
    ///
    /// Supports `[Script Info]`, `[V4+ Styles]`, `[V4 Styles]`, `[Events]`,
    /// and `[Fonts]` sections. Lines starting with `;` or `!` are treated as
    /// comments and skipped.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if a required field is malformed (e.g. invalid
    /// style format line, unparseable event timestamp).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ass_parser::AssFile;
    ///
    /// let content = r#"
    /// [Script Info]
    /// Title: Test
    /// PlayResX: 1920
    /// PlayResY: 1080
    ///
    /// [V4+ Styles]
    /// Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
    /// Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
    ///
    /// [Events]
    /// Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
    /// Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World
    /// "#;
    ///
    /// let ass = AssFile::parse(content).unwrap();
    /// assert_eq!(ass.events.len(), 1);
    /// ```
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
                "V4+ Styles" => ass.parse_style_line(line, false)?,
                "V4 Styles" => ass.parse_style_line(line, true)?,
                "Events" => ass.parse_event_line(line)?,
                "Fonts" => ass.parse_font_line(line),
                _ => {}
            }
        }
        Ok(ass)
    }

    /// Parse ASS content leniently, recovering from errors instead of aborting.
    ///
    /// Returns a tuple of (partial AssFile, list of errors encountered).
    /// Invalid events and styles are skipped; valid portions are still parsed correctly.
    /// Missing [Script Info] section uses defaults (1920x1080, v4.00+).
    pub fn parse_lenient(content: &str) -> (Self, Vec<ParseError>) {
        let mut ass = Self::new();
        let mut errors = Vec::new();
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
                "Script Info" => {
                    let _ = ass.parse_script_info(line);
                }
                "V4+ Styles" => {
                    if line.starts_with("Format:") {
                        continue;
                    }
                    if let Err(e) = ass.parse_style_line(line, false) {
                        errors.push(e);
                    }
                }
                "V4 Styles" => {
                    if line.starts_with("Format:") {
                        continue;
                    }
                    if let Err(e) = ass.parse_style_line(line, true) {
                        errors.push(e);
                    }
                }
                "Events" => {
                    if line.starts_with("Format:") {
                        continue;
                    }
                    if let Err(e) = ass.parse_event_line(line) {
                        errors.push(e);
                    }
                }
                "Fonts" => ass.parse_font_line(line),
                _ => {}
            }
        }
        (ass, errors)
    }

    /// Parse an ASS/SSA/SRT file from disk.
    ///
    /// Reads the file content, detects the format from the file extension,
    /// and parses it.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the file cannot be read or contains invalid
    /// subtitle data.
    pub fn parse_file(path: &Path) -> Result<Self, ParseError> {
        let content = std::fs::read_to_string(path)?;
        let format = SubtitleFormat::detect(path).unwrap_or(SubtitleFormat::Ass);
        let mut ass = match format {
            SubtitleFormat::Srt => crate::srt::parse_srt(&content)?,
            _ => Self::parse(&content)?,
        };
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
                    self.script_info
                        .extra
                        .insert(key.to_string(), value.to_string());
                }
            }
        }
        Ok(())
    }

    fn parse_style_line(&mut self, line: &str, is_v4: bool) -> Result<(), ParseError> {
        if line.starts_with("Style:") {
            let style_data = line.trim_start_matches("Style:").trim();
            let style = if is_v4 {
                Style::parse_from_line_v4(style_data)?
            } else {
                Style::parse_from_line(style_data)?
            };
            self.styles.push(style);
        }
        Ok(())
    }

    fn parse_event_line(&mut self, line: &str) -> Result<(), ParseError> {
        if let Some(colon_pos) = line.find(':') {
            let type_str = &line[..colon_pos];
            if let Some(event_type) = EventType::parse(type_str) {
                let event_data = line[colon_pos + 1..].trim();
                let event = Event::parse_from_line(event_type, event_data)?;
                self.events.push(event);
            }
        }
        Ok(())
    }

    /// Returns an iterator over dialogue events only (skips comments, pictures, etc.).
    pub fn dialogue_events(&self) -> impl Iterator<Item = &Event> {
        self.events.iter().filter(|e| e.is_dialogue())
    }

    /// Parse a line from the [Fonts] section.
    /// Format: `fontname: FontName, filename: file.ttf` or `fontname: FontName`
    fn parse_font_line(&mut self, line: &str) {
        let mut font_name = String::new();
        let mut filename = String::new();
        for part in line.split(',') {
            let part = part.trim();
            if let Some(name) = part.strip_prefix("fontname:") {
                font_name = name.trim().to_string();
            } else if let Some(name) = part.strip_prefix("Fontname:") {
                font_name = name.trim().to_string();
            } else if let Some(f) = part.strip_prefix("filename:") {
                filename = f.trim().to_string();
            } else if let Some(f) = part.strip_prefix("Filename:") {
                filename = f.trim().to_string();
            }
        }
        if !font_name.is_empty() {
            self.embedded_fonts.push(EmbeddedFont {
                font_name,
                filename,
            });
        }
    }

    /// Load embedded font files from disk based on parsed filenames.
    /// `base_dir` is the directory containing the .ass file.
    pub fn load_embedded_fonts(&mut self, base_dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
        let mut loaded = Vec::new();
        for ef in &self.embedded_fonts {
            if ef.filename.is_empty() {
                continue;
            }
            let path = base_dir.join(&ef.filename);
            if let Ok(data) = std::fs::read(&path) {
                loaded.push((ef.font_name.clone(), data));
            }
        }
        loaded
    }

    /// Find a style by name.
    pub fn find_style(&self, name: &str) -> Option<&Style> {
        self.styles.iter().find(|s| s.name == name)
    }

    /// Returns the script resolution as (width, height) from `[Script Info]`.
    pub fn resolution(&self) -> (u32, u32) {
        (self.script_info.play_res_x, self.script_info.play_res_y)
    }
}
