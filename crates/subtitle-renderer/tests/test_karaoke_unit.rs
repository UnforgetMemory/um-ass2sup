//! Unit tests for the karaoke syllable-state machine (extracted from
//! `src/karaoke.rs` to keep the production file focused).

use ass_core::{KaraokeSegment, KaraokeStyle};
use subtitle_renderer::{KaraokePhase, KaraokeRenderer};

fn make_seg(style: KaraokeStyle, dur: u64, text: &str, idx: usize) -> KaraokeSegment {
    KaraokeSegment::new(style, dur, text.to_string(), idx)
}

#[test]
fn test_compute_syllable_states_pending() {
    let segs = vec![make_seg(KaraokeStyle::Instant, 1000, "Hello", 0)];
    // event starts at 1000ms; at t=0 the syllable is still Pending.
    let states = KaraokeRenderer::compute_syllable_states(&segs, 1000, 0);
    assert_eq!(states[0].phase, KaraokePhase::Pending);
    assert_eq!(states[0].start_ms, 1000);
}

#[test]
fn test_compute_syllable_states_active() {
    let segs = vec![make_seg(KaraokeStyle::Instant, 1000, "Hello", 0)];
    // event starts at 1000ms; at t=1500 we are 500ms into the syllable.
    let states = KaraokeRenderer::compute_syllable_states(&segs, 1000, 1500);
    assert!(matches!(states[0].phase, KaraokePhase::Active { .. }));
    assert_eq!(states[0].start_ms, 1000);
}

#[test]
fn test_compute_syllable_states_done() {
    let segs = vec![make_seg(KaraokeStyle::Instant, 1000, "Hello", 0)];
    let states = KaraokeRenderer::compute_syllable_states(&segs, 1000, 2500);
    assert!(matches!(states[0].phase, KaraokePhase::Done));
    assert_eq!(states[0].start_ms, 1000);
}

#[test]
fn test_multi_syllable_timing() {
    let segs = vec![
        make_seg(KaraokeStyle::Instant, 1000, "Hello", 0),
        make_seg(KaraokeStyle::Instant, 500, "World", 1),
    ];
    // event starts at 1000ms; at t=1200 first syllable is Active, second Pending.
    let states = KaraokeRenderer::compute_syllable_states(&segs, 1000, 1200);
    assert_eq!(states.len(), 2);
    assert!(matches!(states[0].phase, KaraokePhase::Active { .. }));
    assert!(matches!(states[1].phase, KaraokePhase::Pending));
    assert_eq!(states[1].start_ms, 2000);
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
        KaraokeStyle::Instant,
        KaraokePhase::Active { progress: 0.5 }
    ));
}

#[test]
fn test_get_fill_clip_x() {
    assert_eq!(KaraokeRenderer::get_fill_clip_x(0.5, 100.0), 50.0);
    assert_eq!(KaraokeRenderer::get_fill_clip_x(0.0, 100.0), 0.0);
    assert_eq!(KaraokeRenderer::get_fill_clip_x(1.0, 100.0), 100.0);
    // Clamped outside [0, 1]
    assert_eq!(KaraokeRenderer::get_fill_clip_x(2.0, 100.0), 100.0);
    assert_eq!(KaraokeRenderer::get_fill_clip_x(-1.0, 100.0), 0.0);
}

#[test]
fn test_get_karaoke_phases() {
    let segs = vec![
        make_seg(KaraokeStyle::Instant, 1000, "Hello", 0),
        make_seg(KaraokeStyle::Instant, 500, "World", 1),
    ];
    let phases = KaraokeRenderer::get_karaoke_phases(&segs, 0, 1500);
    assert_eq!(phases.len(), 2);
    assert!(matches!(phases[0].1, KaraokePhase::Done));
    assert!(matches!(phases[1].1, KaraokePhase::Done));
}

#[test]
fn test_empty_segments() {
    let states = KaraokeRenderer::compute_syllable_states(&[], 0, 0);
    assert!(states.is_empty());
}

#[test]
fn test_zero_duration_segment() {
    let segs = vec![make_seg(KaraokeStyle::Instant, 0, "Hi", 0)];
    let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 0);
    assert_eq!(states.len(), 1);
    assert!(matches!(states[0].phase, KaraokePhase::Done));
}

#[test]
fn test_outline_syllable_pending() {
    let segs = vec![make_seg(KaraokeStyle::Outline, 1000, "Hello", 0)];
    let states = KaraokeRenderer::compute_syllable_states(&segs, 1000, 0);
    assert!(matches!(states[0].phase, KaraokePhase::Pending));
}

#[test]
fn test_outline_syllable_active_progress() {
    let segs = vec![make_seg(KaraokeStyle::Outline, 1000, "Hello", 0)];
    let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 250);
    match states[0].phase {
        KaraokePhase::Active { progress } => {
            assert!(
                (progress - 0.25).abs() < 0.01,
                "Expected ~0.25 progress, got {progress}"
            );
        }
        ref other => panic!("Expected Active, got {other:?}"),
    }
}

#[test]
fn test_outline_syllable_done() {
    let segs = vec![make_seg(KaraokeStyle::Outline, 1000, "Hello", 0)];
    let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 1500);
    assert!(matches!(states[0].phase, KaraokePhase::Done));
}

#[test]
fn test_outline_multi_syllable_timing() {
    let segs = vec![
        make_seg(KaraokeStyle::Outline, 1000, "Hello", 0),
        make_seg(KaraokeStyle::Outline, 500, "World", 1),
    ];
    // event starts at 1000ms; at t=2600 both syllables are Done.
    let states = KaraokeRenderer::compute_syllable_states(&segs, 1000, 2600);
    assert_eq!(states.len(), 2);
    assert!(matches!(states[0].phase, KaraokePhase::Done));
    assert_eq!(states[1].start_ms, 2000);
    assert!(matches!(states[1].phase, KaraokePhase::Done));
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
fn test_instant_multi_syllable_active_transition() {
    let segs = vec![
        make_seg(KaraokeStyle::Instant, 1000, "Hello", 0),
        make_seg(KaraokeStyle::Instant, 500, "World", 1),
    ];
    // event starts at 1000ms; at t=2500 both syllables are Done.
    let states = KaraokeRenderer::compute_syllable_states(&segs, 1000, 2500);
    assert_eq!(states.len(), 2);
    assert!(matches!(states[0].phase, KaraokePhase::Done));
    assert!(matches!(states[1].phase, KaraokePhase::Done));
}

#[test]
fn test_karaoke_phase_debug_and_copy() {
    let phase = KaraokePhase::Active { progress: 0.5 };
    let copied = phase; // KaraokePhase: Copy
    assert_eq!(format!("{phase:?}"), format!("{copied:?}"));
    let _ = KaraokePhase::Pending;
    let _ = KaraokePhase::Done;
}

/// Build a `\kt` (Timing-style) segment: duration doubles as the absolute
/// start offset from the event start.
fn make_kt_seg(dur: u64, text: &str, idx: usize) -> KaraokeSegment {
    KaraokeSegment::new(KaraokeStyle::Timing, dur, text.to_string(), idx)
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
        KaraokePhase::Done => {}          // also acceptable for zero-duration
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
    assert_eq!(
        states[0].phase,
        KaraokePhase::Done,
        "first syllable should be Done"
    );
    match states[1].phase {
        KaraokePhase::Active { progress } => {
            let expected = 50.0 / 150.0;
            assert!(
                (progress - expected).abs() < 0.01,
                "Expected ~{expected}, got {progress}"
            );
        }
        _ => panic!("second syllable should be Active"),
    }
    assert_eq!(
        states[2].phase,
        KaraokePhase::Pending,
        "third syllable should be Pending"
    );
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
fn test_outline_zero_duration() {
    let segs = vec![make_seg(KaraokeStyle::Outline, 0, "A", 0)];
    let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 0);
    assert_eq!(states[0].phase, KaraokePhase::Done);
    assert_eq!(states[0].style, KaraokeStyle::Outline);
}
