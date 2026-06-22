/// ASS subtitle color in `&HAABBGGRR` format.
///
/// ASS uses **inverted alpha** in **ABGR** byte order:
/// - `alpha`: 0 = fully opaque, 255 = fully transparent
/// - Color channels: blue, green, red (opposite of standard RGB)
///
/// # Examples
/// ```
/// use ass_core::AssColor;
///
/// let c = AssColor::from_ass_hex("&H00FFFFFF").unwrap();
/// assert_eq!(c.to_rgba(), [255, 255, 255, 255]); // white, opaque
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssColor {
    /// Alpha (0 = opaque, 255 = transparent)
    pub alpha: u8,
    /// Blue channel
    pub blue: u8,
    /// Green channel
    pub green: u8,
    /// Red channel
    pub red: u8,
}

impl AssColor {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self {
        alpha: 255,
        blue: 0,
        green: 0,
        red: 0,
    };
    /// Opaque white.
    pub const WHITE: Self = Self {
        alpha: 0,
        blue: 255,
        green: 255,
        red: 255,
    };
    /// Opaque black.
    pub const BLACK: Self = Self {
        alpha: 0,
        blue: 0,
        green: 0,
        red: 0,
    };

    /// Create from individual ABGR channels.
    #[inline]
    pub const fn new(alpha: u8, blue: u8, green: u8, red: u8) -> Self {
        Self {
            alpha,
            blue,
            green,
            red,
        }
    }

    /// Parse ASS hex color `&HAABBGGRR` or `&HBBGGRR`.
    ///
    /// Accepts optional `&H` prefix and trailing `&`.
    /// Expects 8 hex chars (ABGR) or 6 hex chars (BBGGRR, alpha=0).
    pub fn from_ass_hex(s: &str) -> Result<Self, crate::ParseError> {
        let s = s
            .strip_prefix("&H")
            .or_else(|| s.strip_prefix("&h"))
            .ok_or_else(|| crate::ParseError::invalid_color("color", s))?;
        let s = s.strip_prefix("H").unwrap_or(s);
        let s = s.trim_end_matches('&');
        if s.len() != 8 && s.len() != 6 {
            return Err(crate::ParseError::invalid_color("color", s));
        }
        if s.len() == 8 {
            let alpha = u8::from_str_radix(&s[0..2], 16)
                .map_err(|_| crate::ParseError::invalid_color("color", s))?;
            let blue = u8::from_str_radix(&s[2..4], 16)
                .map_err(|_| crate::ParseError::invalid_color("color", s))?;
            let green = u8::from_str_radix(&s[4..6], 16)
                .map_err(|_| crate::ParseError::invalid_color("color", s))?;
            let red = u8::from_str_radix(&s[6..8], 16)
                .map_err(|_| crate::ParseError::invalid_color("color", s))?;
            Ok(Self {
                alpha,
                blue,
                green,
                red,
            })
        } else {
            // 6 chars: BBGGRR, alpha defaults to 0 (opaque)
            let blue = u8::from_str_radix(&s[0..2], 16)
                .map_err(|_| crate::ParseError::invalid_color("color", s))?;
            let green = u8::from_str_radix(&s[2..4], 16)
                .map_err(|_| crate::ParseError::invalid_color("color", s))?;
            let red = u8::from_str_radix(&s[4..6], 16)
                .map_err(|_| crate::ParseError::invalid_color("color", s))?;
            Ok(Self {
                alpha: 0,
                blue,
                green,
                red,
            })
        }
    }

    /// Create from ABGR u32 (SSA v4 decimal format).
    pub fn from_raw_abgr(val: u32) -> Self {
        Self {
            alpha: ((val >> 24) & 0xFF) as u8,
            blue: ((val >> 16) & 0xFF) as u8,
            green: ((val >> 8) & 0xFF) as u8,
            red: (val & 0xFF) as u8,
        }
    }

    /// Create from standard RGB (opaque).
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            alpha: 0,
            blue: b,
            green: g,
            red: r,
        }
    }

    /// Create from standard RGBA (alpha uses ASS convention).
    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            alpha: a,
            blue: b,
            green: g,
            red: r,
        }
    }

    /// Convert to standard `[R, G, B, A]` (A=255=opaque).
    pub fn to_rgba(&self) -> [u8; 4] {
        [self.red, self.green, self.blue, 255 - self.alpha]
    }

    /// Serialize to ASS hex `&HAABBGGRR`.
    pub fn to_ass_hex(&self) -> String {
        format!(
            "&H{:02X}{:02X}{:02X}{:02X}",
            self.alpha, self.blue, self.green, self.red
        )
    }

    /// Replace alpha channel.
    #[inline]
    pub fn with_alpha(&self, alpha: u8) -> Self {
        Self { alpha, ..*self }
    }

    /// True if fully transparent (alpha == 255).
    #[inline]
    pub fn is_transparent(&self) -> bool {
        self.alpha == 255
    }
}

impl std::fmt::Display for AssColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_ass_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_ass_hex_white() {
        let c = AssColor::from_ass_hex("&H00FFFFFF").unwrap();
        assert_eq!(c.alpha, 0);
        assert_eq!(c.red, 255);
        assert_eq!(c.green, 255);
        assert_eq!(c.blue, 255);
    }

    #[test]
    fn from_ass_hex_transparent() {
        let c = AssColor::from_ass_hex("&HFF000000").unwrap();
        assert_eq!(c.alpha, 255);
        assert_eq!(c.to_rgba(), [0, 0, 0, 0]);
    }

    #[test]
    fn from_ass_hex_no_alpha() {
        let c = AssColor::from_ass_hex("&HFFFFFF").unwrap();
        assert_eq!(c.alpha, 0); // defaults to opaque
        assert_eq!(c.to_rgba(), [255, 255, 255, 255]);
    }

    #[test]
    fn from_ass_hex_with_trailing_ampersand() {
        let c = AssColor::from_ass_hex("&H00FF0000&").unwrap();
        assert_eq!(c.red, 0);
        assert_eq!(c.green, 0);
        assert_eq!(c.blue, 255);
    }

    #[test]
    fn from_ass_hex_double_h() {
        let c = AssColor::from_ass_hex("&H00FFFFFF").unwrap();
        assert_eq!(c.to_ass_hex(), "&H00FFFFFF");
    }

    #[test]
    fn from_rgb_conversion() {
        let c = AssColor::from_rgb(255, 0, 0);
        assert_eq!(c.to_ass_hex(), "&H000000FF");
    }

    #[test]
    fn from_rgba_roundtrip() {
        let c = AssColor::from_rgba(128, 64, 32, 16);
        let rgba = c.to_rgba();
        assert_eq!(rgba, [128, 64, 32, 239]); // 255 - 16 = 239
    }

    #[test]
    fn to_rgba_alpha_inversion() {
        let c = AssColor {
            alpha: 0,
            blue: 0,
            green: 0,
            red: 255,
        };
        assert_eq!(c.to_rgba(), [255, 0, 0, 255]);

        let c = AssColor {
            alpha: 255,
            blue: 0,
            green: 0,
            red: 255,
        };
        assert_eq!(c.to_rgba(), [255, 0, 0, 0]);
    }

    #[test]
    fn invalid_hex_returns_error() {
        assert!(AssColor::from_ass_hex("not_hex").is_err());
        assert!(AssColor::from_ass_hex("&HXYZ").is_err());
        assert!(AssColor::from_ass_hex("").is_err());
    }

    #[test]
    fn with_alpha_replaces() {
        let c = AssColor::WHITE.with_alpha(128);
        assert_eq!(c.alpha, 128);
        assert_eq!(c.red, 255);
    }

    #[test]
    fn is_transparent_true() {
        assert!(AssColor::TRANSPARENT.is_transparent());
    }

    #[test]
    fn is_transparent_false() {
        assert!(!AssColor::WHITE.is_transparent());
    }

    #[test]
    fn from_raw_abgr() {
        let c = AssColor::from_raw_abgr(0xFF_00_00_FF); // A=255, B=0, G=0, R=255
        assert_eq!(c.alpha, 255);
        assert_eq!(c.red, 255);
        assert_eq!(c.to_rgba(), [255, 0, 0, 0]);
    }
}
