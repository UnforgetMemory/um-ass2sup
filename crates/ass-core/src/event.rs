//! Subtitle event types — [`Event`], [`EventType`], [`TaggedOverride`].

use std::fmt;

use crate::effect::Effect;
use crate::karaoke::KaraokeSegment;
use crate::span::Span;
use crate::types::StyleRef;
use crate::OverrideTag;

/// Event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    /// Dialogue event.
    Dialogue,
    /// Comment event (ignored by renderers).
    Comment,
    /// Picture event.
    Picture,
    /// Sound event.
    Sound,
    /// Movie event.
    Movie,
    /// Command event.
    Command,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dialogue => write!(f, "Dialogue"),
            Self::Comment => write!(f, "Comment"),
            Self::Picture => write!(f, "Picture"),
            Self::Sound => write!(f, "Sound"),
            Self::Movie => write!(f, "Movie"),
            Self::Command => write!(f, "Command"),
        }
    }
}

/// Override tag with source span information.
#[derive(Debug, Clone, PartialEq)]
pub struct TaggedOverride {
    /// The override tag.
    pub tag: OverrideTag,
    /// Optional source span for error reporting.
    pub span: Option<Span>,
}

/// A parsed subtitle event.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Source line number (1-based).
    pub source_line: u32,
    /// Event type.
    pub event_type: EventType,
    /// Z-order layer.
    pub layer: u32,
    /// Start timestamp in milliseconds.
    pub start_ms: u64,
    /// End timestamp in milliseconds.
    pub end_ms: u64,
    /// Style reference.
    pub style: StyleRef,
    /// Actor/speaker name.
    pub actor: String,
    /// Margin overrides (None = use style default).
    pub margin_l: Option<u32>,
    /// Margin overrides (None = use style default).
    pub margin_r: Option<u32>,
    /// Margin overrides (None = use style default).
    pub margin_v: Option<u32>,
    /// Effect.
    pub effect: Effect,
    /// Raw original text (no modifications).
    pub text_raw: String,
    /// Parsed override tags with source positions.
    pub override_tags: Vec<TaggedOverride>,
    /// Karaoke segments.
    pub karaoke: Vec<KaraokeSegment>,
}
