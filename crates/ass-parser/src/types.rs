//! Strong-typed domain types for ASS/SSA subtitle files.
//!
//! This module provides newtypes and enums that replace raw integers/strings
//! in the AST, making style references, alignment values, border styles,
//! and margin groups type-safe at compile time.

use std::fmt;

/// A named style reference — used in both `Style::name` and `Event::style`.
///
/// ASS dialogues reference styles by name (e.g., `"Default"`, `"Sign"`).
/// Using `StyleName` instead of a bare `String` makes style resolution
/// explicit at the type level.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StyleName(pub String);

impl StyleName {
    /// Parse a style name from a trimmed string.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Return the inner name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for StyleName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StyleName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for StyleName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl<'a> From<&'a str> for StyleName {
    fn from(s: &'a str) -> Self {
        Self(s.to_string())
    }
}

impl PartialEq<str> for StyleName {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for StyleName {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// ASS border style.
///
/// V4+ spec values:
/// - `1` = Outline + drop shadow (default)
/// - `3` = Opaque box (background box behind text)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    /// Standard outline + drop shadow (value 1).
    OutlineAndShadow = 1,
    /// Opaque background box behind text (value 3).
    OpaqueBox = 3,
}

impl BorderStyle {
    /// Create a `BorderStyle` from an integer value.
    /// Returns `None` for unrecognised values.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::OutlineAndShadow),
            3 => Some(Self::OpaqueBox),
            _ => None,
        }
    }

    /// Return the raw ASS integer value.
    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for BorderStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_u8())
    }
}

/// ASS numpad alignment (1–9).
///
/// Layout (same as a numeric keypad):
/// ```text
/// 7 8 9
/// 4 5 6
/// 1 2 3
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    BottomLeft = 1,
    BottomCenter = 2,
    BottomRight = 3,
    MiddleLeft = 4,
    MiddleCenter = 5,
    MiddleRight = 6,
    TopLeft = 7,
    TopCenter = 8,
    TopRight = 9,
}

impl Alignment {
    /// Create an `Alignment` from a numpad value (1–9).
    /// Returns `None` for values outside the valid range.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::BottomLeft),
            2 => Some(Self::BottomCenter),
            3 => Some(Self::BottomRight),
            4 => Some(Self::MiddleLeft),
            5 => Some(Self::MiddleCenter),
            6 => Some(Self::MiddleRight),
            7 => Some(Self::TopLeft),
            8 => Some(Self::TopCenter),
            9 => Some(Self::TopRight),
            _ => None,
        }
    }

    /// Return the raw ASS numpad value.
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Return the horizontal component: -1 = left, 0 = center, 1 = right.
    pub fn horizontal_factor(self) -> i8 {
        match self {
            Self::BottomLeft | Self::MiddleLeft | Self::TopLeft => -1,
            Self::BottomCenter | Self::MiddleCenter | Self::TopCenter => 0,
            Self::BottomRight | Self::MiddleRight | Self::TopRight => 1,
        }
    }

    /// Return the vertical component: -1 = bottom, 0 = middle, 1 = top.
    pub fn vertical_factor(self) -> i8 {
        match self {
            Self::BottomLeft | Self::BottomCenter | Self::BottomRight => -1,
            Self::MiddleLeft | Self::MiddleCenter | Self::MiddleRight => 0,
            Self::TopLeft | Self::TopCenter | Self::TopRight => 1,
        }
    }
}

impl fmt::Display for Alignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_u8())
    }
}

/// Left, right, and vertical margins for a style or event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Margins {
    /// Left margin in pixels.
    pub left: u32,
    /// Right margin in pixels.
    pub right: u32,
    /// Vertical margin in pixels.
    pub vertical: u32,
}

impl Margins {
    /// Create a new `Margins` from left, right, vertical values.
    pub const fn new(left: u32, right: u32, vertical: u32) -> Self {
        Self {
            left,
            right,
            vertical,
        }
    }
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            left: 10,
            right: 10,
            vertical: 10,
        }
    }
}

impl fmt::Display for Margins {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{}", self.left, self.right, self.vertical)
    }
}

/// ASS font encoding identifier.
///
/// Common values:
/// - `0` = ANSI (Windows code page 1252)
/// - `1` = Default (system-dependent)
/// - `128` = SHIFTJIS (Japanese)
/// - `134` = GB2312 (Simplified Chinese)
/// - `136` = BIG5 (Traditional Chinese)
/// - `177` = HANGUL (Korean)
/// - `255` = OEM
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encoding(pub u8);

impl Encoding {
    /// Create a new `Encoding` value.
    pub fn new(v: u8) -> Self {
        Self(v)
    }

    /// Return the raw encoding byte.
    pub fn to_u8(self) -> u8 {
        self.0
    }
}

impl Default for Encoding {
    fn default() -> Self {
        Self(1)
    }
}

impl fmt::Display for Encoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_name_new() {
        let n = StyleName::new("Default");
        assert_eq!(n.as_str(), "Default");
    }

    #[test]
    fn style_name_display() {
        assert_eq!(StyleName::new("Sign").to_string(), "Sign");
    }

    #[test]
    fn style_name_eq_str() {
        let n = StyleName::new("Default");
        assert!(n == "Default");
        assert!(n != "Other");
    }

    #[test]
    fn border_style_roundtrip() {
        assert_eq!(BorderStyle::OutlineAndShadow.to_u8(), 1);
        assert_eq!(BorderStyle::OpaqueBox.to_u8(), 3);
        assert_eq!(BorderStyle::from_u8(1), Some(BorderStyle::OutlineAndShadow));
        assert_eq!(BorderStyle::from_u8(3), Some(BorderStyle::OpaqueBox));
        assert_eq!(BorderStyle::from_u8(2), None);
        assert_eq!(BorderStyle::from_u8(0), None);
    }

    #[test]
    fn alignment_roundtrip() {
        for v in 1..=9u8 {
            let a = Alignment::from_u8(v).unwrap();
            assert_eq!(a.to_u8(), v);
        }
        assert!(Alignment::from_u8(0).is_none());
        assert!(Alignment::from_u8(10).is_none());
    }

    #[test]
    fn alignment_factors() {
        assert_eq!(Alignment::BottomLeft.horizontal_factor(), -1);
        assert_eq!(Alignment::BottomCenter.horizontal_factor(), 0);
        assert_eq!(Alignment::BottomRight.horizontal_factor(), 1);
        assert_eq!(Alignment::TopLeft.vertical_factor(), 1);
        assert_eq!(Alignment::MiddleCenter.vertical_factor(), 0);
        assert_eq!(Alignment::BottomCenter.vertical_factor(), -1);
    }

    #[test]
    fn margins_default() {
        let m = Margins::default();
        assert_eq!(m.left, 10);
        assert_eq!(m.right, 10);
        assert_eq!(m.vertical, 10);
    }

    #[test]
    fn margins_display() {
        let m = Margins::new(20, 30, 40);
        assert_eq!(m.to_string(), "20,30,40");
    }

    #[test]
    fn encoding_default() {
        assert_eq!(Encoding::default().to_u8(), 1);
    }

    #[test]
    fn encoding_custom() {
        let e = Encoding::new(128);
        assert_eq!(e.to_u8(), 128);
        assert_eq!(e.to_string(), "128");
    }
}
