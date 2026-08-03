//! ASS karaoke subtitle rendering.
//!
//! Handles per-syllable timing and dual-layer rendering for `\k`, `\kf`, `\ko`, `\kt`
//! ASS override tags. The renderer computes syllable states (pending/active/done) at a
//! given timestamp and produces layered RGBA output for karaoke fill effects.

use ass_core::{KaraokeSegment, KaraokeStyle};

/// Current animation phase of a karaoke syllable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KaraokePhase {
    /// Syllable has not yet started (timestamp < start_ms).
    Pending,
    /// Syllable is currently being highlighted (start_ms ≤ timestamp < end_ms).
    Active {
        /// Progress through the syllable, 0.0–1.0.
        progress: f32,
    },
    /// Syllable animation is complete (timestamp ≥ end_ms).
    Done,
}

/// Computed state of a single karaoke syllable at a specific timestamp.
#[derive(Debug, Clone)]
pub struct SyllableState {
    /// Index of the syllable within the event's karaoke segments.
    pub index: usize,
    /// Start time in milliseconds (absolute, from event start).
    pub start_ms: u64,
    /// End time in milliseconds (absolute).
    pub end_ms: u64,
    /// The syllable text content.
    pub text: String,
    /// Current animation phase at the queried timestamp.
    pub phase: KaraokePhase,
    /// Karaoke style from the original ASS tag.
    pub style: KaraokeStyle,
}

/// Renderer for ASS karaoke subtitle effects.
///
/// Produces per-syllable timing states and determines highlight visibility
/// for dual-layer rendering (background layer + foreground fill layer).
///
/// # ASS karaoke tags
///
/// | Tag  | Style    | Behavior                          |
/// |------|----------|-----------------------------------|
/// | `\k` | Instant  | Switches color instantly per syllable |
/// | `\kf` | Fill     | Left-to-right clip sweep           |
/// | `\ko` | Outline  | Outline highlight                  |
/// | `\kt` | Timing   | Absolute per-syllable timing       |
pub struct KaraokeRenderer;

impl KaraokeRenderer {
    /// Computes the animation phase for each karaoke syllable at a given timestamp.
    ///
    /// Segments are laid out sequentially starting at `event_start_ms`. Each segment's
    /// phase is determined by comparing `timestamp_ms` against its computed time range.
    ///
    /// # Arguments
    ///
    /// * `segments` — karaoke segments from the event's parsed ASS data.
    /// * `event_start_ms` — the event's start time in milliseconds.
    /// * `timestamp_ms` — the current frame's timestamp in milliseconds.
    ///
    /// # Returns
    ///
    /// A `Vec<SyllableState>` with one entry per segment, in order.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Karaoke types live in ass_core crate
    /// use ass_core::karaoke::{KaraokeSegment, KaraokeStyle};
    /// use subtitle_renderer::karaoke::{KaraokeRenderer, KaraokePhase};
    ///
    /// let segs = vec![KaraokeSegment::new(KaraokeStyle::Instant, 1000, "Hi".into(), 0)];
    /// let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 500);
    /// assert!(matches!(states[0].phase, KaraokePhase::Active { progress } if (progress - 0.5).abs() < 0.01));
    /// ```
    pub fn compute_syllable_states(
        segments: &[KaraokeSegment],
        event_start_ms: u64,
        timestamp_ms: u64,
    ) -> Vec<SyllableState> {
        // Phase 1: Compute absolute start times.
        // \kt segments use duration_ms as absolute start offset from event_start_ms.
        // \k/\kf/\ko segments use sequential timing from cursor.
        let mut starts = Vec::with_capacity(segments.len());
        let mut cursor = event_start_ms;
        for seg in segments {
            let start = if seg.style == KaraokeStyle::Timing {
                event_start_ms + seg.duration_ms
            } else {
                cursor
            };
            starts.push(start);
            if seg.style == KaraokeStyle::Timing {
                cursor = start;
            } else {
                cursor = start + seg.duration_ms;
            }
        }

        // Phase 2: Compute durations and syllable states.
        let mut states = Vec::with_capacity(segments.len());
        for (i, seg) in segments.iter().enumerate() {
            let start = starts[i];
            let end = if seg.style == KaraokeStyle::Timing {
                // For \kt, the syllable lasts until the next syllable starts.
                starts.get(i + 1).copied().unwrap_or(start)
            } else {
                start + seg.duration_ms
            };

            let phase = if timestamp_ms < start {
                KaraokePhase::Pending
            } else if timestamp_ms >= end {
                KaraokePhase::Done
            } else {
                let elapsed = timestamp_ms - start;
                let duration = end.saturating_sub(start);
                let progress = if duration > 0 {
                    elapsed as f32 / duration as f32
                } else {
                    1.0
                };
                KaraokePhase::Active { progress }
            };
            states.push(SyllableState {
                index: seg.index,
                start_ms: start,
                end_ms: end,
                text: seg.text.clone(),
                phase,
                style: seg.style,
            });
        }
        states
    }

    /// Returns `(style, phase, start_x)` tuples for each syllable at the given timestamp.
    ///
    /// Alternative to `compute_syllable_states` that returns a lightweight tuple
    /// instead of full `SyllableState` objects.
    pub fn get_karaoke_phases(
        segments: &[KaraokeSegment],
        event_start_ms: u64,
        timestamp_ms: u64,
    ) -> Vec<(KaraokeStyle, KaraokePhase, f32)> {
        segments
            .iter()
            .scan(event_start_ms, |cursor, seg| {
                let start = *cursor;
                let end = start + seg.duration_ms;
                *cursor = end;
                let phase = if timestamp_ms < start {
                    KaraokePhase::Pending
                } else if timestamp_ms >= end {
                    KaraokePhase::Done
                } else {
                    let elapsed = timestamp_ms - start;
                    let progress = if seg.duration_ms > 0 {
                        elapsed as f32 / seg.duration_ms as f32
                    } else {
                        1.0
                    };
                    KaraokePhase::Active { progress }
                };
                Some((seg.style, phase, start as f32))
            })
            .collect()
    }

    /// Returns `true` if the syllable should be highlighted in the foreground (primary) color.
    ///
    /// Active syllables are always highlighted. Done syllables are highlighted for
    /// Instant, Fill, and Outline styles (Outline Done shows full primary glyph).
    pub fn should_highlight(style: KaraokeStyle, phase: KaraokePhase) -> bool {
        match phase {
            KaraokePhase::Pending => false,
            KaraokePhase::Active { .. } => true,
            KaraokePhase::Done => matches!(
                style,
                KaraokeStyle::Instant | KaraokeStyle::Fill | KaraokeStyle::Outline
            ),
        }
    }

    /// Returns the x-coordinate for a `\kf` fill clip mask at the given progress.
    ///
    /// `progress` is clamped to 0.0–1.0. The returned value is `progress * total_width`,
    /// representing the left-to-right reveal position.
    pub fn get_fill_clip_x(progress: f32, total_width: f32) -> f32 {
        progress.clamp(0.0, 1.0) * total_width
    }
}
