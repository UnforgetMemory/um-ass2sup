//! Structured font event telemetry.
//!
//! Records font-load, font-query, corruption, and glyph-cache events
//! so that callers can monitor font subsystem health and performance.

use super::types::{FontId, FontQuery, FontWeight};

/// A single font-related telemetry event.
#[derive(Debug, Clone)]
pub enum FontEvent {
    /// A font was loaded (or attempted to be loaded).
    Loaded {
        id: FontId,
        family: String,
        weight: FontWeight,
        path: Option<String>,
        corrupt: bool,
        took_us: u64,
    },
    /// A font query completed.
    Queried {
        query: FontQuery,
        result: Option<FontId>,
        candidates_count: usize,
        took_us: u64,
    },
    /// A corrupted font was detected.
    Corrupted {
        path: String,
        reason: String,
        recoverable: bool,
    },
    /// Glyph cache statistics snapshot.
    GlyphCache {
        hit: usize,
        miss: usize,
        size: usize,
    },
}

/// Accumulates font-telemetry events during a conversion run.
#[derive(Debug, Default)]
pub struct FontTelemetry {
    events: Vec<FontEvent>,
}

impl FontTelemetry {
    /// Create a new empty telemetry buffer.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Record a single event into the buffer.
    pub fn record(&mut self, event: FontEvent) {
        self.events.push(event);
    }

    /// Read-only access to all recorded events so far.
    pub fn events(&self) -> &[FontEvent] {
        &self.events
    }

    /// Empty the event buffer.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_event_loaded_debug() {
        let event = FontEvent::Loaded {
            id: FontId(7),
            family: "DejaVu Sans".into(),
            weight: FontWeight::Bold,
            path: Some("/usr/share/fonts/DejaVuSans-Bold.ttf".into()),
            corrupt: false,
            took_us: 1234,
        };
        let debug = format!("{event:?}");
        assert!(debug.contains("Loaded"));
        assert!(debug.contains("DejaVu Sans"));
        assert!(debug.contains("Bold"));
        assert!(debug.contains("DejaVuSans-Bold.ttf"));
    }

    #[test]
    fn font_event_queried_clone() {
        let event = FontEvent::Queried {
            query: FontQuery {
                family: "Arial".into(),
                weight: FontWeight::Normal,
                style: super::super::types::FontStyle::Normal,
            },
            result: Some(FontId(3)),
            candidates_count: 5,
            took_us: 42,
        };
        let cloned = event.clone();
        match (&event, &cloned) {
            (
                FontEvent::Queried {
                    query: q1,
                    result: r1,
                    candidates_count: cc1,
                    took_us: t1,
                },
                FontEvent::Queried {
                    query: q2,
                    result: r2,
                    candidates_count: cc2,
                    took_us: t2,
                },
            ) => {
                assert_eq!(q1.family, q2.family);
                assert_eq!(q1.weight, q2.weight);
                assert_eq!(q1.style, q2.style);
                assert_eq!(r1, r2);
                assert_eq!(cc1, cc2);
                assert_eq!(t1, t2);
            }
            _ => panic!("clone changed variant kind"),
        }
    }

    #[test]
    fn telemetry_record_and_read() {
        let mut tel = FontTelemetry::new();
        assert!(tel.events().is_empty());

        tel.record(FontEvent::GlyphCache {
            hit: 10,
            miss: 2,
            size: 64,
        });
        tel.record(FontEvent::Corrupted {
            path: "broken.ttf".into(),
            reason: "invalid magic".into(),
            recoverable: false,
        });

        assert_eq!(tel.events().len(), 2);
        assert!(matches!(tel.events()[0], FontEvent::GlyphCache { .. }));
        assert!(matches!(tel.events()[1], FontEvent::Corrupted { .. }));
    }

    #[test]
    fn telemetry_clear_empties_buffer() {
        let mut tel = FontTelemetry::new();
        tel.record(FontEvent::GlyphCache {
            hit: 0,
            miss: 0,
            size: 0,
        });
        assert_eq!(tel.events().len(), 1);

        tel.clear();
        assert!(tel.events().is_empty());
    }
}
