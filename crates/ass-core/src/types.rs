//! Core boxed types used by subtitle styles.
//!
//! These types are shared across style definitions and event data:
//! [`StyleRef`], [`BorderStyle`], [`Alignment`], [`Margins`], [`FontEncoding`].

use std::fmt;

/// Supported subtitle formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubtitleFormat {
    /// Advanced SubStation Alpha (.ass)
    Ass,
    /// SubStation Alpha (.ssa)
    Ssa,
    /// SubRip (.srt)
    Srt,
}

/// Named style reference (newtype).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StyleRef(pub String);

impl StyleRef {
    /// Create a new style reference.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
    /// Return the style name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StyleRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Into<String>> From<T> for StyleRef {
    fn from(s: T) -> Self {
        Self(s.into())
    }
}

/// Border style (1 = outline+shadow, 3 = opaque box).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    /// Standard outline + drop shadow.
    OutlineAndShadow = 1,
    /// Opaque background box behind text.
    OpaqueBox = 3,
}

impl BorderStyle {
    /// Convert a raw u8 to a [`BorderStyle`], returning `None` for invalid values.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::OutlineAndShadow),
            3 => Some(Self::OpaqueBox),
            _ => None,
        }
    }
    /// Return the u8 discriminant.
    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for BorderStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutlineAndShadow => write!(f, "outline+shadow"),
            Self::OpaqueBox => write!(f, "opaque box"),
        }
    }
}

/// Numpad alignment (1-9).
///
/// Layout: 7 8 9 / 4 5 6 / 1 2 3
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
    /// Convert a raw u8 to an [`Alignment`], returning `None` for invalid values.
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
    /// Return the u8 discriminant.
    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

/// Left/right/vertical margins.
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
    /// Create a new [`Margins`] value.
    pub const fn new(left: u32, right: u32, vertical: u32) -> Self {
        Self {
            left,
            right,
            vertical,
        }
    }
}

impl fmt::Display for Margins {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.left, self.right, self.vertical)
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

/// Font encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontEncoding(pub u8);

impl FontEncoding {
    /// Create a new [`FontEncoding`].
    pub fn new(v: u8) -> Self {
        Self(v)
    }
    /// Return the raw u8 encoding value.
    pub fn to_u8(self) -> u8 {
        self.0
    }
}

impl fmt::Display for FontEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Default for FontEncoding {
    fn default() -> Self {
        Self(1)
    }
}

impl fmt::Display for SubtitleFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ass => write!(f, "ASS"),
            Self::Ssa => write!(f, "SSA"),
            Self::Srt => write!(f, "SRT"),
        }
    }
}
