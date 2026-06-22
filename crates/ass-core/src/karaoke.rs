/// ASS karaoke timing style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KaraokeStyle {
    /// `\k` — instant switch to primary colour.
    Instant,
    /// `\kf` — left-to-right fill sweep.
    Fill,
    /// `\ko` — outline highlight.
    Outline,
    /// `\kt` — explicit per-syllable timing.
    Timing,
}

impl KaraokeStyle {
    /// Parse from tag name (`k`, `kf`, `ko`, `kt`).
    /// Also accepts `K` (uppercase) as alias for `kf` (libass compat).
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "k" => Some(Self::Instant),
            "kf" | "K" => Some(Self::Fill),
            "ko" => Some(Self::Outline),
            "kt" => Some(Self::Timing),
            _ => None,
        }
    }

    /// Return the canonical tag name.
    pub fn tag_name(&self) -> &'static str {
        match self {
            Self::Instant => "k",
            Self::Fill => "kf",
            Self::Outline => "ko",
            Self::Timing => "kt",
        }
    }
}

/// A single karaoke syllable parsed from override tags.
#[derive(Debug, Clone, PartialEq)]
pub struct KaraokeSegment {
    /// Karaoke style (timing method).
    pub style: KaraokeStyle,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Text content of this syllable.
    pub text: String,
    /// Zero-based segment index within the event.
    pub index: usize,
}

impl KaraokeSegment {
    /// Create a new karaoke segment.
    pub fn new(style: KaraokeStyle, duration_ms: u64, text: String, index: usize) -> Self {
        Self {
            style,
            duration_ms,
            text,
            index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_tag_k() {
        assert_eq!(KaraokeStyle::from_tag("k"), Some(KaraokeStyle::Instant));
    }
    #[test]
    fn from_tag_kf() {
        assert_eq!(KaraokeStyle::from_tag("kf"), Some(KaraokeStyle::Fill));
    }
    #[test]
    fn from_tag_k_upper() {
        assert_eq!(KaraokeStyle::from_tag("K"), Some(KaraokeStyle::Fill));
    }
    #[test]
    fn from_tag_ko() {
        assert_eq!(KaraokeStyle::from_tag("ko"), Some(KaraokeStyle::Outline));
    }
    #[test]
    fn from_tag_kt() {
        assert_eq!(KaraokeStyle::from_tag("kt"), Some(KaraokeStyle::Timing));
    }
    #[test]
    fn invalid_tag() {
        assert_eq!(KaraokeStyle::from_tag("invalid"), None);
    }
    #[test]
    fn segment_new() {
        let seg = KaraokeSegment::new(KaraokeStyle::Fill, 1000, "Hello".into(), 0);
        assert_eq!(seg.duration_ms, 1000);
        assert_eq!(seg.text, "Hello");
        assert_eq!(seg.index, 0);
    }
}
