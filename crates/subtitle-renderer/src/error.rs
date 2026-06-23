/// Errors that can occur during rendering of a single event.
///
/// Each error is caught and logged individually — one event's failure
/// does not prevent other events from rendering.
#[derive(Debug, Clone)]
pub enum EventError {
    /// HarfBuzz shaping failed for the event text.
    ShapeFailed(String),
    /// Required font was not found after all fallback attempts.
    FontMissing(String),
    /// Rendering overflowed (bitmap too large, too many glyphs, etc.).
    Overflow(String),
}

impl std::fmt::Display for EventError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ShapeFailed(s) => write!(f, "shape failed: {s}"),
            Self::FontMissing(s) => write!(f, "font missing: {s}"),
            Self::Overflow(s) => write!(f, "overflow: {s}"),
        }
    }
}

impl std::error::Error for EventError {}
