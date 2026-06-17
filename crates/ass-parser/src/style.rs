//! ASS/SSA style definitions from `[V4+ Styles]` and `[V4 Styles]` sections.
//!
//! V4+ styles have 22 comma-separated fields:
//! `Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour,
//!  Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle,
//!  Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding`

use super::color::AssColor;
use super::types::{Alignment, BorderStyle, Encoding, Margins, StyleName};

/// ASS/SSA style definition from the `[V4+ Styles]` or `[V4 Styles]` section.
///
/// Default: Arial 20pt, white primary, black outline/shadow, alignment 2 (bottom-center).
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// Style name (referenced by `Event::style`).
    pub name: StyleName,
    /// Font family name.
    pub font_name: String,
    /// Font size in points.
    pub font_size: f64,
    /// Primary fill colour.
    pub primary_color: AssColor,
    /// Secondary colour (used in karaoke).
    pub secondary_color: AssColor,
    /// Outline / border colour.
    pub outline_color: AssColor,
    /// Shadow / background colour.
    pub shadow_color: AssColor,
    /// Bold flag (`-1` or `1` = bold).
    pub bold: bool,
    /// Italic flag.
    pub italic: bool,
    /// Underline flag.
    pub underline: bool,
    /// Strikeout / strikethrough flag.
    pub strikeout: bool,
    /// Horizontal scale as percentage (100 = normal).
    pub scale_x: f64,
    /// Vertical scale as percentage (100 = normal).
    pub scale_y: f64,
    /// Extra letter spacing in pixels.
    pub spacing: f64,
    /// Rotation angle in degrees.
    pub angle: f64,
    /// Border style (outline+shadow, or opaque box).
    pub border_style: BorderStyle,
    /// Outline / border width in pixels.
    pub outline: f64,
    /// Shadow depth in pixels.
    pub shadow: f64,
    /// Numpad alignment (1–9).
    pub alignment: Alignment,
    /// Raw alignment value as it appeared in the source file (1–255).
    /// Preserved so the validator can flag out-of-range values
    /// (V008) that the typed `alignment` field silently coerces
    /// back to `BottomCenter` (the libass convention).
    pub raw_alignment: u8,
    /// Left, right, and vertical margins.
    pub margins: Margins,
    /// Font encoding identifier.
    pub encoding: Encoding,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            name: StyleName::new("Default"),
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
            border_style: BorderStyle::OutlineAndShadow,
            outline: 2.0,
            shadow: 2.0,
            alignment: Alignment::BottomCenter,
            raw_alignment: 2,
            margins: Margins::default(),
            encoding: Encoding::default(),
        }
    }
}

impl Style {
    /// Parse a boolean ASS flag (`"-1"`, `"1"` = true, anything else = false).
    fn parse_flag(s: &str) -> bool {
        let t = s.trim();
        t == "-1" || t == "1"
    }

    /// Parses a comma-separated `Style:` line into a `Style` struct.
    ///
    /// Expects 23 fields: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour,
    /// OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY,
    /// Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV,
    /// Encoding.
    ///
    /// An optional 24th field (`RelativeTo`) is accepted for SSA compatibility but ignored.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::InvalidStyle`](crate::ParseError::InvalidStyle) if fewer than 23 fields are present.
    pub fn parse_from_line(line: &str) -> Result<Self, super::error::ParseError> {
        let fields: Vec<&str> = line.splitn(24, ',').collect();
        if fields.len() < 23 {
            return Err(super::error::ParseError::InvalidStyle(format!(
                "expected 23 fields, got {}",
                fields.len()
            )));
        }
        Ok(Self {
            name: StyleName::new(fields[0].trim()),
            font_name: fields[1].trim().to_string(),
            font_size: fields[2].trim().parse().unwrap_or(20.0),
            primary_color: AssColor::from_ass_hex(fields[3].trim()).unwrap_or(AssColor::WHITE),
            secondary_color: AssColor::from_ass_hex(fields[4].trim()).unwrap_or(AssColor::WHITE),
            outline_color: AssColor::from_ass_hex(fields[5].trim()).unwrap_or(AssColor::BLACK),
            shadow_color: AssColor::from_ass_hex(fields[6].trim()).unwrap_or(AssColor::BLACK),
            bold: Self::parse_flag(fields[7]),
            italic: Self::parse_flag(fields[8]),
            underline: Self::parse_flag(fields[9]),
            strikeout: Self::parse_flag(fields[10]),
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
            raw_alignment: fields[18].trim().parse().unwrap_or(2),
            margins: Margins::new(
                fields[19].trim().parse().unwrap_or(10),
                fields[20].trim().parse().unwrap_or(10),
                fields[21].trim().parse().unwrap_or(10),
            ),
            encoding: Encoding::new(fields[22].trim().parse().unwrap_or(1)),
        })
    }

    /// Parses a comma-separated `Style:` line from an SSA v4 (`[V4 Styles]`) section.
    ///
    /// SSA v4 has 18 fields (no Underline/StrikeOut/ScaleX/ScaleY/Spacing/Angle, has AlphaLevel):
    /// Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, TertiaryColour, BackColour,
    /// Bold, Italic, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV,
    /// AlphaLevel, Encoding
    ///
    /// `TertiaryColour` is mapped to `outline_color` (its functional equivalent in SSA v4).
    /// Missing fields (underline, strikeout, scale_x, scale_y, spacing, angle) default to off/100%/0.
    /// `AlphaLevel` is ignored (no equivalent in V4+).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::InvalidStyle`](crate::ParseError::InvalidStyle) if fewer than 18 fields are present.
    pub fn parse_from_line_v4(line: &str) -> Result<Self, super::error::ParseError> {
        let fields: Vec<&str> = line.splitn(19, ',').collect();
        if fields.len() < 18 {
            return Err(super::error::ParseError::InvalidStyle(format!(
                "expected 18 fields for SSA v4 style, got {}",
                fields.len()
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
            name: StyleName::new(fields[0].trim()),
            font_name: fields[1].trim().to_string(),
            font_size: fields[2].trim().parse().unwrap_or(20.0),
            primary_color: parse_v4_color(fields[3]),
            secondary_color: parse_v4_color(fields[4]),
            outline_color: parse_v4_color(fields[5]),
            shadow_color: parse_v4_color(fields[6]),
            bold: Self::parse_flag(fields[7]),
            italic: Self::parse_flag(fields[8]),
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
            raw_alignment: fields[12].trim().parse().unwrap_or(2),
            margins: Margins::new(
                fields[13].trim().parse().unwrap_or(10),
                fields[14].trim().parse().unwrap_or(10),
                fields[15].trim().parse().unwrap_or(10),
            ),
            encoding: Encoding::new(if fields.len() > 17 {
                fields[17].trim().parse().unwrap_or(1)
            } else {
                1
            }),
        })
    }

    /// Serialize this style back into the ASS `Style:` line format (without the `Style:` prefix).
    ///
    /// The output has 23 comma-separated fields matching [`parse_from_line`](Self::parse_from_line).
    pub fn to_ass_string(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            self.name,
            self.font_name,
            self.font_size,
            self.primary_color.to_ass_hex(),
            self.secondary_color.to_ass_hex(),
            self.outline_color.to_ass_hex(),
            self.shadow_color.to_ass_hex(),
            flag_to_ass(self.bold),
            flag_to_ass(self.italic),
            flag_to_ass(self.underline),
            flag_to_ass(self.strikeout),
            self.scale_x,
            self.scale_y,
            self.spacing,
            self.angle,
            self.border_style,
            self.outline,
            self.shadow,
            self.alignment,
            self.margins.left,
            self.margins.right,
            self.margins.vertical,
            self.encoding,
        )
    }
}

/// Convert a boolean to an ASS flag (`"-1"` for true, `"0"` for false).
fn flag_to_ass(v: bool) -> &'static str {
    if v {
        "-1"
    } else {
        "0"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_style() {
        let s = Style::default();
        assert_eq!(s.name, "Default");
        assert_eq!(s.font_name, "Arial");
        assert_eq!(s.font_size, 20.0);
        assert!(!s.bold);
        assert_eq!(s.alignment.to_u8(), 2);
        assert_eq!(s.margins.left, 10);
        assert_eq!(s.margins.right, 10);
        assert_eq!(s.margins.vertical, 10);
        assert_eq!(s.encoding.to_u8(), 1);
    }

    #[test]
    fn parse_style_line() {
        let line = "Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1";
        let s = Style::parse_from_line(line).unwrap();
        assert_eq!(s.name, "Default");
        assert_eq!(s.font_name, "Arial");
        assert_eq!(s.font_size, 20.0);
        assert!(!s.bold);
        assert_eq!(s.alignment.to_u8(), 2);
    }

    #[test]
    fn parse_style_bold() {
        let line = "Default,Arial,28,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1";
        let s = Style::parse_from_line(line).unwrap();
        assert!(s.bold);
    }

    #[test]
    fn parse_style_too_few_fields() {
        let line = "Default,Arial,20";
        assert!(Style::parse_from_line(line).is_err());
    }

    #[test]
    fn parse_style_custom_alignment() {
        let line = "Sign,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,8,20,20,60,1";
        let s = Style::parse_from_line(line).unwrap();
        assert_eq!(s.name, "Sign");
        assert_eq!(s.font_size, 48.0);
        assert_eq!(s.alignment.to_u8(), 8);
        assert_eq!(s.margins.vertical, 60);
    }

    #[test]
    fn parse_style_name_with_spaces() {
        let line = "My Style Name,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1";
        let s = Style::parse_from_line(line).unwrap();
        assert_eq!(s.name, "My Style Name");
    }

    #[test]
    fn parse_v4_style_line() {
        let line = "Default,Arial,48,16777215,255,0,0,0,0,1,2,2,2,10,10,10,0,1";
        let s = Style::parse_from_line_v4(line).unwrap();
        assert_eq!(s.name, "Default");
        assert_eq!(s.font_name, "Arial");
        assert_eq!(s.font_size, 48.0);
        assert!(!s.bold);
        assert!(!s.italic);
        assert!(!s.underline);
        assert!(!s.strikeout);
        assert_eq!(s.border_style.to_u8(), 1);
        assert!((s.outline - 2.0).abs() < f64::EPSILON);
        assert!((s.shadow - 2.0).abs() < f64::EPSILON);
        assert_eq!(s.alignment.to_u8(), 2);
        assert_eq!(s.margins.left, 10);
        assert_eq!(s.margins.right, 10);
        assert_eq!(s.margins.vertical, 10);
        assert_eq!(s.encoding.to_u8(), 1);
    }

    #[test]
    fn parse_v4_style_tertiary_colour_maps_to_outline() {
        let line = "Default,Arial,48,16777215,255,12345,0,0,0,1,2,2,2,10,10,10,0,1";
        let s = Style::parse_from_line_v4(line).unwrap();
        assert_eq!(s.outline_color.to_ass_hex(), "&H00003039");
    }

    #[test]
    fn parse_v4_style_too_few_fields() {
        let line = "Default,Arial,48";
        assert!(Style::parse_from_line_v4(line).is_err());
    }

    #[test]
    fn parse_v4_style_alpha_level_ignored() {
        let line_a0 = "Default,Arial,48,16777215,255,0,0,0,0,1,2,2,2,10,10,10,0,1";
        let line_a255 = "Default,Arial,48,16777215,255,0,0,0,0,1,2,2,2,10,10,10,255,1";
        let s0 = Style::parse_from_line_v4(line_a0).unwrap();
        let s255 = Style::parse_from_line_v4(line_a255).unwrap();
        assert_eq!(s0, s255);
    }

    // ── Round-trip tests ──────────────────────────────────────────────

    /// Helper: parse a string, serialize it, parse the result, expect identical styles.
    fn assert_roundtrip(line: &str) {
        let s1 = Style::parse_from_line(line).unwrap();
        let serialized = s1.to_ass_string();
        let s2 = Style::parse_from_line(&serialized).unwrap();
        assert_eq!(
            s1, s2,
            "round-trip failed\n  input: {line}\n  output: {serialized}"
        );
    }

    #[test]
    fn roundtrip_default_style() {
        assert_roundtrip("Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1");
    }

    #[test]
    fn roundtrip_bold_style() {
        assert_roundtrip("BoldStyle,Impact,48,&H00FFFF00,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,3,3,2,10,10,10,1");
    }

    #[test]
    fn roundtrip_italic_underline_strikeout() {
        assert_roundtrip("Deco,Times New Roman,36,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,-1,-1,-1,100,100,0,0,1,2,2,2,10,10,10,1");
    }

    #[test]
    fn roundtrip_top_right_alignment() {
        assert_roundtrip("TopRight,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,9,10,10,10,1");
    }

    #[test]
    fn roundtrip_scaled_style() {
        assert_roundtrip("Wide,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,150,75,5,0,1,2,2,2,10,10,10,1");
    }

    #[test]
    fn roundtrip_custom_margins() {
        assert_roundtrip("Margins,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,50,80,120,1");
    }

    #[test]
    fn roundtrip_opaque_box() {
        assert_roundtrip("BoxStyle,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&HFF000000,0,0,0,0,100,100,0,0,3,0,0,2,10,10,10,1");
    }

    #[test]
    fn roundtrip_encoding_japanese() {
        assert_roundtrip("Japanese,MS Gothic,24,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,1,1,2,10,10,10,128");
    }

    #[test]
    fn roundtrip_colour_alpha() {
        assert_roundtrip("AlphaStyle,Arial,20,&H80FF0000,&H8000FF00,&H800000FF,&HFF000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1");
    }

    #[test]
    fn roundtrip_angle_spacing() {
        assert_roundtrip("Rotated,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,3,45.5,1,2,2,2,10,10,10,1");
    }

    #[test]
    fn parse_flag_true_variants() {
        assert!(Style::parse_flag("-1"));
        assert!(Style::parse_flag("1"));
        assert!(!Style::parse_flag("0"));
        assert!(!Style::parse_flag("2"));
        assert!(!Style::parse_flag("true"));
        assert!(!Style::parse_flag(""));
    }

    // ── Alignment-specific tests ───────────────────────────────────────

    #[test]
    fn alignment_bottom_center_default() {
        let s = Style::default();
        assert_eq!(s.alignment, Alignment::BottomCenter);
        assert_eq!(s.alignment.to_u8(), 2);
    }

    #[test]
    fn alignment_roundtrip_all() {
        for align in 1..=9u8 {
            let line = format!(
                "Test,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,{align},10,10,10,1"
            );
            let s = Style::parse_from_line(&line).unwrap();
            assert_eq!(s.alignment.to_u8(), align);
        }
    }

    // ── BorderStyle-specific tests ─────────────────────────────────────

    #[test]
    fn border_style_outline_and_shadow_default() {
        let s = Style::default();
        assert_eq!(s.border_style, BorderStyle::OutlineAndShadow);
    }

    #[test]
    fn border_style_opaque_box() {
        let line = "Box,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&HFF000000,0,0,0,0,100,100,0,0,3,0,0,2,10,10,10,1";
        let s = Style::parse_from_line(line).unwrap();
        assert_eq!(s.border_style, BorderStyle::OpaqueBox);
    }

    // ── Field-level round-trip (individual fields in isolation) ────────

    #[test]
    fn field_font_name() {
        let line = "CustomFont,Comic Sans MS,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1";
        let s = Style::parse_from_line(line).unwrap();
        assert_eq!(s.font_name, "Comic Sans MS");
    }

    #[test]
    fn field_font_size() {
        let line = "Big,Arial,72,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1";
        let s = Style::parse_from_line(line).unwrap();
        assert!((s.font_size - 72.0).abs() < f64::EPSILON);
    }

    #[test]
    fn field_scale_x() {
        let line = "Wide,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,200,100,0,0,1,2,2,2,10,10,10,1";
        let s = Style::parse_from_line(line).unwrap();
        assert!((s.scale_x - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn field_scale_y() {
        let line = "Tall,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,200,0,0,1,2,2,2,10,10,10,1";
        let s = Style::parse_from_line(line).unwrap();
        assert!((s.scale_y - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn field_spacing() {
        let line = "Spread,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,10,0,1,2,2,2,10,10,10,1";
        let s = Style::parse_from_line(line).unwrap();
        assert!((s.spacing - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn field_angle() {
        let line = "Rotated,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,90,1,2,2,2,10,10,10,1";
        let s = Style::parse_from_line(line).unwrap();
        assert!((s.angle - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn field_outline_width() {
        let line = "Thick,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,5,2,2,10,10,10,1";
        let s = Style::parse_from_line(line).unwrap();
        assert!((s.outline - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn field_shadow_depth() {
        let line = "Shadowed,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,8,2,10,10,10,1";
        let s = Style::parse_from_line(line).unwrap();
        assert!((s.shadow - 8.0).abs() < f64::EPSILON);
    }
}
