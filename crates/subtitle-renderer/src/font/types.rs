//! Pure domain types for the font subsystem.
//!
//! These types carry data but contain zero logic beyond trivial
//! conversions and accessors. All decision-making lives in the
//! registry, database, and discovery modules.

/// Opaque font identifier. Maps internally to a font database index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontId(pub u32);

impl From<u32> for FontId {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

/// Font weight scale, matching CSS/OpenType values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FontWeight {
    Thin = 100,
    ExtraLight = 200,
    Light = 300,
    Normal = 400,
    Medium = 500,
    Semibold = 600,
    Bold = 700,
    ExtraBold = 800,
    Black = 900,
}

impl FontWeight {
    /// Create a `FontWeight` from a raw `u16` value, rounding to the nearest
    /// standard OpenType weight.
    pub fn from_u16(w: u16) -> Self {
        match w {
            0..=149 => Self::Thin,
            150..=249 => Self::ExtraLight,
            250..=349 => Self::Light,
            350..=449 => Self::Normal,
            450..=549 => Self::Medium,
            550..=649 => Self::Semibold,
            650..=749 => Self::Bold,
            750..=849 => Self::ExtraBold,
            _ => Self::Black,
        }
    }

    /// Return the weight value as a `u16`.
    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

/// Font style (slant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontStyle {
    Normal,
    Italic,
}

/// Font stretch (width).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontStretch {
    Condensed,
    Normal,
    Expanded,
}

/// Metadata about a loaded font face.
#[derive(Debug, Clone)]
pub struct FontFace {
    pub id: FontId,
    pub family: String,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub stretch: FontStretch,
    pub path: Option<String>,
    pub is_system: bool,
    pub cjk: bool,
    pub corrupt: bool,
}

/// A font query — what the caller is looking for.
#[derive(Debug, Clone)]
pub struct FontQuery {
    pub family: String,
    pub weight: FontWeight,
    pub style: FontStyle,
}

/// Result of a font query.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub found: Option<FontId>,
    pub candidates: Vec<FontFace>,
    pub suggestion: Option<FontFace>,
}

/// Font availability check result.
#[derive(Debug, Clone)]
pub struct Availability {
    pub exact_match: bool,
    pub variants: Vec<FontFace>,
    pub suggestion: Option<FontFace>,
}

/// A shaped glyph from the shaper.
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub glyph_id: u16,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
}

/// A rasterized glyph image.
#[derive(Debug, Clone)]
pub struct RasterizedGlyph {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_id_from_u32() {
        let id: FontId = 42u32.into();
        assert_eq!(id, FontId(42));
        assert_ne!(id, FontId(0));
    }

    #[test]
    fn font_weight_ordering() {
        assert!(FontWeight::Thin < FontWeight::Normal);
        assert!(FontWeight::Normal < FontWeight::Bold);
        assert!(FontWeight::Bold < FontWeight::Black);
        assert_eq!(FontWeight::Normal, FontWeight::Normal);
    }

    #[test]
    fn font_weight_from_u16_rounding() {
        assert_eq!(FontWeight::from_u16(100), FontWeight::Thin);
        assert_eq!(FontWeight::from_u16(400), FontWeight::Normal);
        assert_eq!(FontWeight::from_u16(700), FontWeight::Bold);
        assert_eq!(FontWeight::from_u16(900), FontWeight::Black);
        // Boundary checks
        assert_eq!(FontWeight::from_u16(149), FontWeight::Thin);
        assert_eq!(FontWeight::from_u16(150), FontWeight::ExtraLight);
        assert_eq!(FontWeight::from_u16(349), FontWeight::Light);
        assert_eq!(FontWeight::from_u16(350), FontWeight::Normal);
    }

    #[test]
    fn font_weight_as_u16() {
        assert_eq!(FontWeight::Thin.as_u16(), 100);
        assert_eq!(FontWeight::Normal.as_u16(), 400);
        assert_eq!(FontWeight::Bold.as_u16(), 700);
        assert_eq!(FontWeight::Black.as_u16(), 900);
    }

    #[test]
    fn font_style_partial_eq() {
        assert_eq!(FontStyle::Normal, FontStyle::Normal);
        assert_eq!(FontStyle::Italic, FontStyle::Italic);
        assert_ne!(FontStyle::Normal, FontStyle::Italic);
    }

    #[test]
    fn font_stretch_variants_distinct() {
        assert_ne!(FontStretch::Condensed, FontStretch::Normal);
        assert_ne!(FontStretch::Normal, FontStretch::Expanded);
    }
}
