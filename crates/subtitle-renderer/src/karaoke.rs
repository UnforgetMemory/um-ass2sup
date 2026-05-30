use ass_parser::karaoke::{KaraokeSegment, KaraokeStyle};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KaraokePhase {
    Pending,
    Active {
        progress: f32,
    },
    Done,
}

#[derive(Debug, Clone)]
pub struct SyllableState {
    pub index: usize,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub phase: KaraokePhase,
}

pub struct KaraokeRenderer;

impl KaraokeRenderer {
    pub fn compute_syllable_states(
        segments: &[KaraokeSegment],
        event_start_ms: u64,
        timestamp_ms: u64,
    ) -> Vec<SyllableState> {
        let mut states = Vec::with_capacity(segments.len());
        let mut cursor = event_start_ms;
        for seg in segments {
            let start = cursor;
            let end = cursor + seg.duration_ms;
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
            states.push(SyllableState {
                index: seg.index,
                start_ms: start,
                end_ms: end,
                text: seg.text.clone(),
                phase,
            });
            cursor = end;
        }
        states
    }

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

    pub fn should_highlight(style: KaraokeStyle, phase: KaraokePhase) -> bool {
        match phase {
            KaraokePhase::Pending => false,
            KaraokePhase::Active { .. } => true,
            KaraokePhase::Done => matches!(style, KaraokeStyle::Instant | KaraokeStyle::Fill),
        }
    }

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
        assert!(!KaraokeRenderer::should_highlight(
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
}
