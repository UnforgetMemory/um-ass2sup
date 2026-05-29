use std::fmt;
use crate::error::ParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssColor {
    pub alpha: u8,
    pub blue: u8,
    pub green: u8,
    pub red: u8,
}

impl AssColor {
    pub const TRANSPARENT: Self = Self { alpha: 0, blue: 0, green: 0, red: 0 };
    pub const WHITE: Self = Self { alpha: 0, blue: 255, green: 255, red: 255 };
    pub const BLACK: Self = Self { alpha: 0, blue: 0, green: 0, red: 0 };

    pub fn new(alpha: u8, blue: u8, green: u8, red: u8) -> Self {
        Self { alpha, blue, green, red }
    }

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

    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { alpha: 0, blue: b, green: g, red: r }
    }

    pub fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { alpha: a, blue: b, green: g, red: r }
    }

    pub fn to_rgba(&self) -> [u8; 4] {
        [self.red, self.green, self.blue, 255 - self.alpha]
    }

    pub fn to_ass_hex(&self) -> String {
        format!("&H{:02X}{:02X}{:02X}{:02X}", self.alpha, self.blue, self.green, self.red)
    }

    pub fn with_alpha(&self, alpha: u8) -> Self {
        Self { alpha, ..*self }
    }

    pub fn is_transparent(&self) -> bool {
        self.alpha == 255
    }
}

impl fmt::Display for AssColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_ass_hex())
    }
}
