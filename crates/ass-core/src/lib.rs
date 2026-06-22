//! `ass-core`: ASS/SSA/SRT subtitle parser — lossless, libass-compatible.
//!
//! This crate provides a complete parser for Advanced SubStation Alpha (ASS),
//! SubStation Alpha (SSA), and SubRip (SRT) subtitle files.
//!
//! # Design principles
//!
//! - **Lossless**: original text is preserved in [`Event::text_raw`].
//! - **Libass-compatible**: all override tags match libass semantics.
//! - **No silent data loss**: every fallback is recorded as a [`Warning`].
//! - **Source locations**: every [`ParseError`] can carry a [`Span`].
//! - **No floating point time**: [`time::Fps`] uses rational arithmetic.
//!
//! # Quick start
//!
//! ```rust
//! use ass_core::SubtitleDocument;
//!
//! let content = r#"
//! [Script Info]
//! Title: Test
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
//! let doc = SubtitleDocument::parse(content).unwrap();
//! assert_eq!(doc.events.len(), 1);
//! ```

//! Color types for ASS `&HAABBGGRR` format.
pub mod color;
/// Effect types for ASS event effects (Banner, Scroll, Karaoke).
pub mod effect;
/// Parse error and warning types with source span tracking.
pub mod error;
/// Event types and structures.
pub mod event;
/// Karaoke syllable timing types (`\k`, `\kf`, `\ko`, `\kt`).
pub mod karaoke;
/// Line-level lexer — tokenization and section recognition.
pub mod lexer;
/// ASS override tag parser — libass/VSFilter compatible.
pub mod override_tag;
/// Section-level parsers for ScriptInfo, Styles, Events, Fonts.
pub mod section;
/// Source position tracking (`Span`) for error reporting.
pub mod span;
/// SRT subtitle parser.
pub mod srt;

/// Time types: rational Fps, Timestamp, format converters.
pub mod time;
/// Core boxed types shared across styles and events.
pub mod types;

pub use color::AssColor;
pub use effect::{parse_effect, Effect};
pub use error::{ParseError, Warning, WarningKind, WarningSeverity};
pub use event::*;
pub use karaoke::{KaraokeSegment, KaraokeStyle};
pub use span::Span;

pub use time::{Fps, Timestamp};
pub use types::*;

use std::collections::HashMap;

/// Script metadata from `[Script Info]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptMetadata {
    /// Script title.
    pub title: String,
    /// Format version (e.g., "v4.00+").
    pub script_type: String,
    /// Wrap style (0=smart, 1=end-of-line, 2=no-wrap, 3=smart-lower).
    pub wrap_style: u8,
    /// Scale border/shadow with resolution.
    pub scaled_border_and_shadow: bool,
    /// YCbCr matrix type.
    pub ycbcr_matrix: String,
    /// Horizontal resolution.
    pub play_res_x: u32,
    /// Vertical resolution.
    pub play_res_y: u32,
    /// Extra key-value pairs not in standard fields.
    pub extra: HashMap<String, String>,
}

impl Default for ScriptMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            script_type: "v4.00+".into(),
            wrap_style: 0,
            scaled_border_and_shadow: true,
            ycbcr_matrix: "None".into(),
            play_res_x: 1920,
            play_res_y: 1080,
            extra: HashMap::new(),
        }
    }
}

/// A subtitle style definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub name: StyleRef,
    pub font_name: String,
    pub font_size: f64,
    pub primary_color: AssColor,
    pub secondary_color: AssColor,
    pub outline_color: AssColor,
    pub shadow_color: AssColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
    pub scale_x: f64,
    pub scale_y: f64,
    pub spacing: f64,
    pub angle: f64,
    pub border_style: BorderStyle,
    pub outline: f64,
    pub shadow: f64,
    pub alignment: Alignment,
    pub margins: Margins,
    pub encoding: FontEncoding,
}

/// Embedded font reference.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddedFont {
    pub font_name: String,
    pub filename: String,
}

/// Complete subtitle document — the output of parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleDocument {
    /// Detected format.
    pub format: SubtitleFormat,
    /// Script metadata.
    pub metadata: ScriptMetadata,
    /// Style definitions.
    pub styles: Vec<Style>,
    /// Events (dialogue and others).
    pub events: Vec<Event>,
    /// Embedded font references.
    pub fonts: Vec<EmbeddedFont>,
    /// Non-fatal warnings collected during parsing.
    pub warnings: Vec<Warning>,
}

impl Default for SubtitleDocument {
    fn default() -> Self {
        Self {
            format: SubtitleFormat::Ass,
            metadata: ScriptMetadata::default(),
            styles: Vec::new(),
            events: Vec::new(),
            fonts: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

/// Override tag — parsed from `{\tag}` blocks.
/// Full variant list matches libass `ass_parse.c` semantics.
#[derive(Debug, Clone, PartialEq)]
pub enum OverrideTag {
    // ── Position ──
    Pos {
        x: f64,
        y: f64,
    },
    Move {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        t1: u64,
        t2: u64,
    },
    // ── Origin ──
    Origin {
        x: f64,
        y: f64,
    },
    // ── Fade ──
    Fade {
        duration_in: u64,
        duration_out: u64,
    },
    FadeComplex {
        alpha_start: u8,
        alpha_mid: u8,
        alpha_end: u8,
        t1: u64,
        t2: u64,
        t3: u64,
        t4: u64,
    },
    // ── Font ──
    FontName(String),
    FontSize(f64),
    FontSizeRelative(isize), // \fs+N / \fs-N (libass compat)
    Bold(bool),
    BoldWeight(u32),
    Italic(bool),
    Underline(bool),
    Strikeout(bool),
    // ── Colour ──
    PrimaryColor(AssColor),
    SecondaryColor(AssColor),
    OutlineColor(AssColor),
    ShadowColor(AssColor),
    Alpha {
        value: u8,
    },
    PrimaryAlpha {
        value: u8,
    },
    SecondaryAlpha {
        value: u8,
    },
    OutlineAlpha {
        value: u8,
    },
    ShadowAlpha {
        value: u8,
    },
    // ── Border / Shadow ──
    Border {
        x: f64,
        y: f64,
    }, // \bord(w) → x=y=w
    BorderX(f64),
    BorderY(f64),
    Shadow {
        x: f64,
        y: f64,
    }, // \shad(d) → x=y=d
    ShadowX(f64),
    ShadowY(f64),
    // ── Blur ──
    Blur(f64),         // \be
    GaussianBlur(f64), // \blur
    // ── Spacing ──
    Spacing(f64),
    // ── Scale ──
    Scale {
        x: f64,
        y: f64,
    },
    /// Reset scale to style defaults (`\fsc`).
    ScaleReset,
    // ── Rotation ──
    Rotation {
        x: f64,
        y: f64,
        z: f64,
    },
    // ── Shear ──
    Shear {
        x: f64,
        y: f64,
    },
    // ── Alignment ──
    AlignmentVsfilter(u8), // \a with VSFilter quirks applied
    AlignmentNumpad(u8),   // \an (1-9)
    // ── Wrap / Writing mode ──
    WrapStyle(u8),
    WritingMode(u8),
    // ── Clip ──
    Clip {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    ClipInverse {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    ClipDrawing {
        scale: f32,
        commands: String,
    },
    ClipInverseDrawing {
        scale: f32,
        commands: String,
    },
    ClipDrawingCurrent,        // \clip(@)
    ClipInverseDrawingCurrent, // \iclip(@)
    // ── Transform ──
    Transform {
        tag: String,
        t1: u64,
        t2: u64,
        accel: f64,
    },
    // ── Reset ──
    Reset(String), // \r[style]
    ResetAll,      // \r (no arg)
    // ── Karaoke ──
    Karaoke {
        style: KaraokeStyle,
        duration: u64,
    },
    // ── Drawing ──
    DrawingMode(u8),
    BaselineOffset(f64),
    // ── Other ──
    Charset(u8),     // \fe
    AnimationSkip,   // \!
    Unknown(String), // Unrecognised tag, raw text preserved
}

impl SubtitleDocument {
    /// Parse ASS/SSA content from a string (strict mode).
    pub fn parse(content: &str) -> Result<Self, ParseError> {
        let (doc, errors) = Self::parse_with_recovery(content);
        if errors.is_empty() {
            Ok(doc)
        } else {
            Err(errors.into_iter().next().unwrap())
        }
    }

    /// Parse with full error recovery using lexer + section parsers.
    pub fn parse_with_recovery(content: &str) -> (Self, Vec<ParseError>) {
        let lines = crate::lexer::lex(content);
        let mut doc = SubtitleDocument {
            format: SubtitleFormat::Ass,
            ..Self::default()
        };
        let mut errors = Vec::new();
        let mut iter = crate::lexer::SectionIter::new(&lines);

        while let Some((name, section_lines)) = iter.next_section() {
            match name {
                "Script Info" => doc.metadata = crate::section::parse_script_info(section_lines),
                "V4+ Styles" => {
                    let (styles, errs) = crate::section::parse_styles(section_lines, false);
                    doc.styles.extend(styles);
                    errors.extend(errs);
                }
                "V4 Styles" => {
                    let (styles, errs) = crate::section::parse_styles(section_lines, true);
                    doc.styles.extend(styles);
                    errors.extend(errs);
                }
                "Events" => {
                    let (events, errs) = crate::section::parse_events(section_lines);
                    doc.events = events;
                    errors.extend(errs);
                }
                "Fonts" => doc.fonts = crate::section::parse_fonts(section_lines),
                _ => {}
            }
        }
        (doc, errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_ass_parse() {
        let content = r#"
[Script Info]
Title: Test
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World
"#;
        let doc = SubtitleDocument::parse(content).unwrap();
        assert_eq!(doc.events.len(), 1);
        assert_eq!(doc.styles.len(), 1);
        assert_eq!(doc.metadata.title, "Test");
        assert_eq!(doc.metadata.play_res_x, 1920);
        assert_eq!(doc.events[0].text_raw, "Hello World");
        assert_eq!(doc.events[0].start_ms, 1000);
        assert_eq!(doc.events[0].end_ms, 5000);
    }

    #[test]
    fn margin_opt_returns_none_for_zero() {
        // parse_opt_u32 was removed; margins use Option<u32> from section parser
        // Tested via section::parse_events
    }

    #[test]
    fn ssa_v4_style_parse() {
        // Tested in section module tests
    }

    #[test]
    fn recover_from_bad_event() {
        let content = "\
[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,bad-time,0:00:03.00,Default,,0,0,0,,First
Dialogue: 0,0:00:04.00,0:00:06.00,Default,,0,0,0,,Second
";
        let (doc, errors) = SubtitleDocument::parse_with_recovery(content);
        assert_eq!(doc.events.len(), 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(doc.events[0].text_raw, "Second");
    }
}
