use std::fmt;
use crate::error::ParseError;

/// ASS subtitle color in `&HAABBGGRR` format.
///
/// ASS uses **pre-multiplied alpha** in **ABGR** byte order, where:
/// - `alpha`: 0 = fully opaque, 255 = fully transparent (inverted from RGBA convention)
/// - Color channels: blue, green, red (opposite of standard RGB)
///
/// # Conversion
///
/// Use [`to_rgba()`](Self::to_rgba) to convert to standard RGBA for rendering,
/// or [`from_rgb()`](Self::from_rgb)/[`from_rgba()`](Self::from_rgba) to construct from standard channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssColor {
    /// Alpha channel (0 = opaque, 255 = transparent)
    pub alpha: u8,
    /// Blue channel
    pub blue: u8,
    /// Green channel
    pub green: u8,
    /// Red channel
    pub red: u8,
}

impl AssColor {
    /// Fully transparent black (alpha=0, all channels 0).
    pub const TRANSPARENT: Self = Self { alpha: 0, blue: 0, green: 0, red: 0 };
    /// Opaque white (alpha=0, B/G/R = 255).
    pub const WHITE: Self = Self { alpha: 0, blue: 255, green: 255, red: 255 };
    /// Opaque black (alpha=0, all channels 0). Same bytes as `TRANSPARENT` but semantically different.
    pub const BLACK: Self = Self { alpha: 0, blue: 0, green: 0, red: 0 };

    pub fn new(alpha: u8, blue: u8, green: u8, red: u8) -> Self {
        Self { alpha, blue, green, red }
    }

    /// Parses an ASS hex color string like `&HAABBGGRR` or `&HBBGGRR`.
    ///
    /// Accepts optional `&H` prefix, optional extra `H`, and optional trailing `&`.
    /// Expects exactly 8 hex characters after prefix stripping (4-byte ABGR).
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::InvalidColor`] if the format is wrong or hex parsing fails.
    pub fn from_ass_hex(s: &str) -> Result<Self, ParseError> {
        let s = s.strip_prefix("&H").ok_or_else(|| ParseError::InvalidColor(s.to_string()))?;
        let s = s.strip_prefix("H").unwrap_or(s);
        let s = s.trim_end_matches('&');
        if s.len() != 8 {
            return Err(ParseError::InvalidColor(s.to_string()));
        }
        let alpha = u8::from_str_radix(&s[0..2], 16).map_err(|_| ParseError::InvalidColor(s.to_string()))?;
        let blue = u8::from_str_radix(&s[2..4], 16).map_err(|_| ParseError::InvalidColor(s.to_string()))?;
        let green = u8::from_str_radix(&s[4..6], 16).map_err(|_| ParseError::InvalidColor(s.to_string()))?;
        let red = u8::from_str_radix(&s[6..8], 16).map_err(|_| ParseError::InvalidColor(s.to_string()))?;
        Ok(Self { alpha, blue, green, red })
    }

    /// Creates an opaque color from an ABGR u32 value (SSA v4 decimal color format).
    /// Byte order: AA BB GG RR (alpha, blue, green, red).
    pub fn from_raw_abgr(val: u32) -> Self {
        Self {
            alpha: ((val >> 24) & 0xFF) as u8,
            blue: ((val >> 16) & 0xFF) as u8,
            green: ((val >> 8) & 0xFF) as u8,
            red: (val & 0xFF) as u8,
        }
    }

    /// Creates an opaque color from RGB channels (alpha=0 in ASS convention = opaque).
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { alpha: 0, blue: b, green: g, red: r }
    }

    /// Creates a color from RGBA channels (alpha uses ASS convention: 0=opaque, 255=transparent).
    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { alpha: a, blue: b, green: g, red: r }
    }

    /// Converts to standard `[R, G, B, A]` format where A=255 means fully opaque.
    pub fn to_rgba(&self) -> [u8; 4] {
        [self.red, self.green, self.blue, 255 - self.alpha]
    }

    /// Serializes to ASS hex format `&HAABBGGRR`.
    pub fn to_ass_hex(&self) -> String {
        format!("&H{:02X}{:02X}{:02X}{:02X}", self.alpha, self.blue, self.green, self.red)
    }

    /// Returns a copy with the alpha channel replaced.
    pub fn with_alpha(&self, alpha: u8) -> Self {
        Self { alpha, ..*self }
    }

    /// Returns `true` if fully transparent (alpha == 255 in ASS convention).
    pub fn is_transparent(&self) -> bool {
        self.alpha == 255
    }
}

impl fmt::Display for AssColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_ass_hex())
    }
}
