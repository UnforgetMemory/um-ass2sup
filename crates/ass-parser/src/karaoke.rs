#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KaraokeStyle {
    Instant,
    Fill,
    Outline,
    Timing,
}

impl KaraokeStyle {
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "k" => Some(Self::Instant),
            "kf" => Some(Self::Fill),
            "ko" => Some(Self::Outline),
            "kt" => Some(Self::Timing),
            _ => None,
        }
    }

    pub fn tag_name(&self) -> &'static str {
        match self {
            Self::Instant => "k",
            Self::Fill => "kf",
            Self::Outline => "ko",
            Self::Timing => "kt",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KaraokeSegment {
    pub style: KaraokeStyle,
    pub duration_ms: u64,
    pub text: String,
    pub index: usize,
}

impl KaraokeSegment {
    pub fn new(style: KaraokeStyle, duration_ms: u64, text: String, index: usize) -> Self {
        Self { style, duration_ms, text, index }
    }

    pub fn end_time(&self, start_time: u64) -> u64 {
        start_time + self.duration_ms
    }
}
