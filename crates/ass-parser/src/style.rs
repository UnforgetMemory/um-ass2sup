use super::color::AssColor;

/// ASS/SSA style definition from the `[V4+ Styles]` or `[V4 Styles]` section.
///
/// Default: Arial 20pt, white primary, black outline/shadow, alignment 2 (bottom-center).
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub name: String,
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
    /// Horizontal scale percentage (100 = normal)
    pub scale_x: f64,
    /// Vertical scale percentage (100 = normal)
    pub scale_y: f64,
    /// Extra letter spacing in pixels
    pub spacing: f64,
    /// Rotation angle in degrees
    pub angle: f64,
    /// 1 = outline + drop shadow, 3 = opaque box
    pub border_style: u8,
    pub outline_width: f64,
    pub shadow_depth: f64,
    /// ASS numpad alignment (1-9)
    pub alignment: u8,
    pub margin_l: u32,
    pub margin_r: u32,
    pub margin_v: u32,
    /// Font encoding (0 = ANSI, 1 = default, etc.)
    pub encoding: u8,
    /// 0 = window, 1 = video
    pub relative_to: u8,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            font_name: "Arial".to_string(),
            font_size: 20.0,
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
}

impl Style {
    /// Parses a comma-separated `Style:` line into a `Style` struct.
    ///
    /// Expects 23-24 fields: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour,
    /// OutlineColour, ShadowColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY,
    /// Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV,
    /// Encoding [, RelativeTo].
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::InvalidStyle`](crate::ParseError::InvalidStyle) if fewer than 23 fields are present.
    pub fn parse_from_line(line: &str) -> Result<Self, super::error::ParseError> {
        let fields: Vec<&str> = line.splitn(24, ',').collect();
        if fields.len() < 23 {
            return Err(super::error::ParseError::InvalidStyle(format!(
                "expected 23 fields, got {}", fields.len()
            )));
        }
        Ok(Self {
            name: fields[0].trim().to_string(),
            font_name: fields[1].trim().to_string(),
            font_size: fields[2].trim().parse().unwrap_or(20.0),
            primary_color: AssColor::from_ass_hex(fields[3].trim()).unwrap_or(AssColor::WHITE),
            secondary_color: AssColor::from_ass_hex(fields[4].trim()).unwrap_or(AssColor::WHITE),
            outline_color: AssColor::from_ass_hex(fields[5].trim()).unwrap_or(AssColor::BLACK),
            shadow_color: AssColor::from_ass_hex(fields[6].trim()).unwrap_or(AssColor::BLACK),
            bold: fields[7].trim() == "-1" || fields[7].trim() == "1",
            italic: fields[8].trim() == "-1" || fields[8].trim() == "1",
            underline: fields[9].trim() == "-1" || fields[9].trim() == "1",
            strikeout: fields[10].trim() == "-1" || fields[10].trim() == "1",
            scale_x: fields[11].trim().parse().unwrap_or(100.0),
            scale_y: fields[12].trim().parse().unwrap_or(100.0),
            spacing: fields[13].trim().parse().unwrap_or(0.0),
            angle: fields[14].trim().parse().unwrap_or(0.0),
            border_style: fields[15].trim().parse().unwrap_or(1),
            outline_width: fields[16].trim().parse().unwrap_or(2.0),
            shadow_depth: fields[17].trim().parse().unwrap_or(2.0),
            alignment: fields[18].trim().parse().unwrap_or(2),
            margin_l: fields[19].trim().parse().unwrap_or(10),
            margin_r: fields[20].trim().parse().unwrap_or(10),
            margin_v: fields[21].trim().parse().unwrap_or(10),
            encoding: fields[22].trim().parse().unwrap_or(1),
            relative_to: if fields.len() > 23 { fields[23].trim().parse().unwrap_or(0) } else { 0 },
        })
    }

    /// Parses a comma-separated `Style:` line from an SSA v4 (`[V4 Styles]`) section.
    ///
    /// SSA v4 has 18 fields (no Underline/StrikeOut/ScaleX/ScaleY/Spacing/Angle, has AlphaLevel):
    /// Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, TertiaryColour, BackColour,
    /// Bold, Italic, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV,
    /// AlphaLevel, Encoding
    ///
    /// `TertiaryColour` is mapped to `outline_colour` (its functional equivalent in SSA v4).
    /// Missing fields (underline, strikeout, scale_x, scale_y, spacing, angle) default to off/100%/0.
    /// `AlphaLevel` is ignored (no equivalent in v4+).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::InvalidStyle`](crate::ParseError::InvalidStyle) if fewer than 18 fields are present.
    pub fn parse_from_line_v4(line: &str) -> Result<Self, super::error::ParseError> {
        let fields: Vec<&str> = line.splitn(19, ',').collect();
        if fields.len() < 18 {
            return Err(super::error::ParseError::InvalidStyle(format!(
                "expected 18 fields for SSA v4 style, got {}", fields.len()
            )));
        }
        // Helper: parse a color value that may be decimal (SSA v4) or &H hex
        let parse_v4_color = |s: &str| -> AssColor {
            let s = s.trim();
            if s.starts_with("&H") || s.starts_with("&h") {
                return AssColor::from_ass_hex(s).unwrap_or(AssColor::WHITE);
            }
            let val: i64 = s.parse().unwrap_or(0);
            AssColor::from_raw_abgr(val as u32)
        };
        Ok(Self {
            name: fields[0].trim().to_string(),
            font_name: fields[1].trim().to_string(),
            font_size: fields[2].trim().parse().unwrap_or(20.0),
            primary_color: parse_v4_color(fields[3]),
            secondary_color: parse_v4_color(fields[4]),
            outline_color: parse_v4_color(fields[5]),
            shadow_color: parse_v4_color(fields[6]),
            bold: fields[7].trim() == "-1" || fields[7].trim() == "1",
            italic: fields[8].trim() == "-1" || fields[8].trim() == "1",
            underline: false,
            strikeout: false,
            scale_x: 100.0,
            scale_y: 100.0,
            spacing: 0.0,
            angle: 0.0,
            border_style: fields[9].trim().parse().unwrap_or(1),
            outline_width: fields[10].trim().parse().unwrap_or(2.0),
            shadow_depth: fields[11].trim().parse().unwrap_or(2.0),
            alignment: fields[12].trim().parse().unwrap_or(2),
            margin_l: fields[13].trim().parse().unwrap_or(10),
            margin_r: fields[14].trim().parse().unwrap_or(10),
            margin_v: fields[15].trim().parse().unwrap_or(10),
            encoding: if fields.len() > 17 { fields[17].trim().parse().unwrap_or(1) } else { 1 },
            relative_to: 0,
        })
    }
}
