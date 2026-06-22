//! Style types for ASS/SSA subtitle files.

use crate::{Alignment, AssColor, BorderStyle, FontEncoding, Margins, StyleRef};

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
