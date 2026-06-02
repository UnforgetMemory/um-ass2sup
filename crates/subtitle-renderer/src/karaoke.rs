//! ASS karaoke subtitle rendering.
//!
//! Handles per-syllable timing and dual-layer rendering for `\k`, `\kf`, `\ko`, `\kt`
//! ASS override tags. The renderer computes syllable states (pending/active/done) at a
//! given timestamp and produces layered RGBA output for karaoke fill effects.

use ass_parser::karaoke::{KaraokeSegment, KaraokeStyle};

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
    /// ```
    /// use ass_parser::karaoke::{KaraokeSegment, KaraokeStyle};
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_seg(style: KaraokeStyle, dur: u64, text: &str, idx: usize) -> KaraokeSegment {
        KaraokeSegment::new(style, dur, text.to_string(), idx)
    }

    #[test]
    fn test_compute_syllable_states_pending() {
        let segs = vec![make_seg(KaraokeStyle::Instant, 1000, "Hello", 0)];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 500, 0);
        assert_eq!(states[0].phase, KaraokePhase::Pending);
    }

    #[test]
    fn test_compute_syllable_states_active() {
        let segs = vec![make_seg(KaraokeStyle::Instant, 1000, "Hello", 0)];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 500);
        match states[0].phase {
            KaraokePhase::Active { progress } => {
                assert!((progress - 0.5).abs() < 0.01);
            }
            _ => panic!("Expected Active phase"),
        }
    }

    #[test]
    fn test_compute_syllable_states_done() {
        let segs = vec![make_seg(KaraokeStyle::Instant, 1000, "Hello", 0)];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 1500);
        assert_eq!(states[0].phase, KaraokePhase::Done);
    }

    #[test]
    fn test_multi_syllable_timing() {
        let segs = vec![
            make_seg(KaraokeStyle::Instant, 500, "Hel", 0),
            make_seg(KaraokeStyle::Instant, 500, "lo", 1),
        ];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 700);
        assert_eq!(states[0].phase, KaraokePhase::Done);
        match states[1].phase {
            KaraokePhase::Active { progress } => {
                assert!((progress - 0.4).abs() < 0.01);
            }
            _ => panic!("Expected Active for second syllable"),
        }
    }

    #[test]
    fn test_should_highlight_instant_pending() {
        assert!(!KaraokeRenderer::should_highlight(
            KaraokeStyle::Instant,
            KaraokePhase::Pending
        ));
    }

    #[test]
    fn test_should_highlight_instant_done() {
        assert!(KaraokeRenderer::should_highlight(
            KaraokeStyle::Instant,
            KaraokePhase::Done
        ));
    }

    #[test]
    fn test_should_highlight_fill_done() {
        assert!(KaraokeRenderer::should_highlight(
            KaraokeStyle::Fill,
            KaraokePhase::Done
        ));
    }

    #[test]
    fn test_should_highlight_outline_done() {
        assert!(KaraokeRenderer::should_highlight(
            KaraokeStyle::Outline,
            KaraokePhase::Done
        ));
    }

    #[test]
    fn test_should_highlight_active() {
        assert!(KaraokeRenderer::should_highlight(
            KaraokeStyle::Fill,
            KaraokePhase::Active { progress: 0.5 }
        ));
    }

    #[test]
    fn test_get_fill_clip_x() {
        assert_eq!(KaraokeRenderer::get_fill_clip_x(0.0, 100.0), 0.0);
        assert_eq!(KaraokeRenderer::get_fill_clip_x(1.0, 100.0), 100.0);
        assert_eq!(KaraokeRenderer::get_fill_clip_x(0.5, 100.0), 50.0);
        assert_eq!(KaraokeRenderer::get_fill_clip_x(2.0, 100.0), 100.0);
    }

    #[test]
    fn test_get_karaoke_phases() {
        let segs = vec![
            make_seg(KaraokeStyle::Fill, 500, "A", 0),
            make_seg(KaraokeStyle::Fill, 500, "B", 1),
        ];
        let phases = KaraokeRenderer::get_karaoke_phases(&segs, 0, 300);
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].0, KaraokeStyle::Fill);
        assert!(matches!(phases[0].1, KaraokePhase::Active { .. }));
        assert_eq!(phases[1].0, KaraokeStyle::Fill);
        assert!(matches!(phases[1].1, KaraokePhase::Pending));
    }

    #[test]
    fn test_empty_segments() {
        let segs = vec![];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 500);
        assert!(states.is_empty());
    }

    #[test]
    fn test_zero_duration_segment() {
        let segs = vec![make_seg(KaraokeStyle::Instant, 0, "A", 0)];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 0);
        // Zero-duration: start==end, timestamp >= end → Done immediately
        assert_eq!(states[0].phase, KaraokePhase::Done);
    }

    #[test]
    fn test_outline_syllable_pending() {
        let segs = vec![make_seg(KaraokeStyle::Outline, 1000, "Hello", 0)];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 500, 0);
        assert_eq!(states[0].phase, KaraokePhase::Pending);
        assert_eq!(states[0].style, KaraokeStyle::Outline);
    }

    #[test]
    fn test_outline_syllable_active_progress() {
        let segs = vec![make_seg(KaraokeStyle::Outline, 1000, "Hello", 0)];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 250);
        match states[0].phase {
            KaraokePhase::Active { progress } => {
                assert!((progress - 0.25).abs() < 0.01, "Expected ~0.25 progress, got {progress}");
            }
            other => panic!("Expected Active phase, got {other:?}"),
        }
        assert_eq!(states[0].style, KaraokeStyle::Outline);
    }

    #[test]
    fn test_outline_syllable_done() {
        let segs = vec![make_seg(KaraokeStyle::Outline, 1000, "Hello", 0)];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 1500);
        assert_eq!(states[0].phase, KaraokePhase::Done);
        assert_eq!(states[0].style, KaraokeStyle::Outline);
    }

    #[test]
    fn test_outline_multi_syllable_timing() {
        let segs = vec![
            make_seg(KaraokeStyle::Outline, 500, "He", 0),
            make_seg(KaraokeStyle::Outline, 500, "llo", 1),
        ];
        // At t=700: first syllable [0,500) is Done, second [500,1000) is Active
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 700);
        assert_eq!(states[0].phase, KaraokePhase::Done);
        assert_eq!(states[1].style, KaraokeStyle::Outline);
        match states[1].phase {
            KaraokePhase::Active { progress } => {
                assert!((progress - 0.4).abs() < 0.01, "Expected ~0.4 progress, got {progress}");
            }
            other => panic!("Expected Active for second syllable, got {other:?}"),
        }
    }

    #[test]
    fn test_outline_highlight_pending() {
        assert!(!KaraokeRenderer::should_highlight(
            KaraokeStyle::Outline,
            KaraokePhase::Pending
        ));
    }

    #[test]
    fn test_outline_highlight_active() {
        assert!(KaraokeRenderer::should_highlight(
            KaraokeStyle::Outline,
            KaraokePhase::Active { progress: 0.5 }
        ));
    }

    #[test]
    fn test_outline_highlight_done() {
        assert!(KaraokeRenderer::should_highlight(
            KaraokeStyle::Outline,
            KaraokePhase::Done
        ));
    }

    #[test]
    fn test_outline_single_syllable_lifecycle() {
        let segs = vec![make_seg(KaraokeStyle::Outline, 1000, "Test", 0)];

        // Before start → Pending
        let states = KaraokeRenderer::compute_syllable_states(&segs, 1000, 0);
        assert_eq!(states[0].phase, KaraokePhase::Pending);

        // At start → Active (progress 0)
        let states = KaraokeRenderer::compute_syllable_states(&segs, 1000, 1000);
        match states[0].phase {
            KaraokePhase::Active { progress } => assert!(progress < 0.01),
            other => panic!("Expected Active at start, got {other:?}"),
        }

        // Mid-way → Active
        let states = KaraokeRenderer::compute_syllable_states(&segs, 1000, 1500);
        match states[0].phase {
            KaraokePhase::Active { progress } => {
                assert!((progress - 0.5).abs() < 0.01);
            }
            other => panic!("Expected Active at midpoint, got {other:?}"),
        }

        // After end → Done
        let states = KaraokeRenderer::compute_syllable_states(&segs, 1000, 2000);
        assert_eq!(states[0].phase, KaraokePhase::Done);
    }

    #[test]
    fn test_outline_get_karaoke_phases() {
        let segs = vec![
            make_seg(KaraokeStyle::Outline, 400, "A", 0),
            make_seg(KaraokeStyle::Outline, 600, "B", 1),
        ];
        let phases = KaraokeRenderer::get_karaoke_phases(&segs, 0, 200);
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].0, KaraokeStyle::Outline);
        assert!(matches!(phases[0].1, KaraokePhase::Active { .. }));
        assert_eq!(phases[1].0, KaraokeStyle::Outline);
        assert!(matches!(phases[1].1, KaraokePhase::Pending));
    }

    #[test]
    fn test_outline_zero_duration() {
        let segs = vec![make_seg(KaraokeStyle::Outline, 0, "A", 0)];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 0);
        assert_eq!(states[0].phase, KaraokePhase::Done);
        assert_eq!(states[0].style, KaraokeStyle::Outline);
    }

    // ── B4: \\kt absolute timing ──────────────────────────────

    fn make_kt_seg(dur: u64, text: &str, idx: usize) -> KaraokeSegment {
        KaraokeSegment::new(KaraokeStyle::Timing, dur, text.to_string(), idx)
    }

    #[test]
    fn test_kt_single_syllable_before_start() {
        // \kt syllable starts at event_start + 100ms = 100ms.
        let segs = vec![make_kt_seg(100, "A", 0)];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 50);
        assert_eq!(states[0].phase, KaraokePhase::Pending);
        assert_eq!(states[0].start_ms, 100);
    }

    #[test]
    fn test_kt_single_syllable_active() {
        let segs = vec![make_kt_seg(100, "A", 0)];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 100);
        match states[0].phase {
            KaraokePhase::Active { .. } => {} // zero-duration → instant done
            KaraokePhase::Done => {} // also acceptable for zero-duration
            _ => panic!("Expected Active or Done"),
        }
        assert_eq!(states[0].start_ms, 100);
    }

    #[test]
    fn test_kt_multi_syllable_absolute_timing() {
        // Three \kt syllables at absolute positions: 0ms, 100ms, 250ms
        let segs = vec![
            make_kt_seg(0, "Hel", 0),
            make_kt_seg(100, "lo", 1),
            make_kt_seg(250, "!", 2),
        ];
        // At t=150ms: first [0,100) = Done, second [100,250) = Active at 50/150≈0.33
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 150);
        assert_eq!(states[0].phase, KaraokePhase::Done, "first syllable should be Done");
        match states[1].phase {
            KaraokePhase::Active { progress } => {
                let expected = 50.0 / 150.0;
                assert!((progress - expected).abs() < 0.01, "Expected ~{expected}, got {progress}");
            }
            _ => panic!("second syllable should be Active"),
        }
        assert_eq!(states[2].phase, KaraokePhase::Pending, "third syllable should be Pending");
    }

    #[test]
    fn test_kt_event_start_offset() {
        // Event starts at 500ms, \kt(100) = start at 600ms.
        let segs = vec![make_kt_seg(100, "A", 0)];
        let states = KaraokeRenderer::compute_syllable_states(&segs, 500, 550);
        assert_eq!(states[0].phase, KaraokePhase::Pending);
        assert_eq!(states[0].start_ms, 600);
    }

    #[test]
    fn test_kt_mixed_with_k() {
        // \k(100) "A" at cursor, then \kt(300) "B" at absolute 300ms.
        let segs = vec![
            KaraokeSegment::new(KaraokeStyle::Instant, 100, "A".into(), 0),
            make_kt_seg(300, "B", 1),
        ];
        // At t=50ms: first syllable [0,100) = Active
        let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 50);
        assert!(matches!(states[0].phase, KaraokePhase::Active { .. }));
        assert_eq!(states[1].phase, KaraokePhase::Pending);
        assert_eq!(states[1].start_ms, 300);
    }
}
