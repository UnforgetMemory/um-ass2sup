/// ASS karaoke timing style, corresponding to `\k`, `\kf`, `\ko`, `\kt` tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KaraokeStyle {
    /// `\k` — instant switch to primary color at syllable start
    Instant,
    /// `\kf` — left-to-right fill sweep over duration
    Fill,
    /// `\ko` — outline highlight at syllable start
    Outline,
    /// `\kt` — explicit per-syllable absolute timing
    Timing,
}

impl KaraokeStyle {
    /// Parses a karaoke tag name (`"k"`, `"kf"`, `"ko"`, `"kt"`) into a style.
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "k" => Some(Self::Instant),
            "kf" => Some(Self::Fill),
            "ko" => Some(Self::Outline),
            "kt" => Some(Self::Timing),
            _ => None,
        }
    }

    /// Returns the ASS tag name for this style (`"k"`, `"kf"`, `"ko"`, `"kt"`).
    pub fn tag_name(&self) -> &'static str {
        match self {
            Self::Instant => "k",
            Self::Fill => "kf",
            Self::Outline => "ko",
            Self::Timing => "kt",
        }
    }
}

/// A single karaoke syllable parsed from ASS override tags.
#[derive(Debug, Clone, PartialEq)]
pub struct KaraokeSegment {
    pub style: KaraokeStyle,
    /// Duration in milliseconds
    pub duration_ms: u64,
    pub text: String,
    /// Zero-based syllable index within the event
    pub index: usize,
}

impl KaraokeSegment {
    pub fn new(style: KaraokeStyle, duration_ms: u64, text: String, index: usize) -> Self {
        Self { style, duration_ms, text, index }
    }

    /// Returns the end timestamp (ms) given a start timestamp.
    pub fn end_time(&self, start_time: u64) -> u64 {
        start_time + self.duration_ms
    }
}
