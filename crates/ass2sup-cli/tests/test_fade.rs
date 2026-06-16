//! Tests for `compute_render_pts` — fade-aware render timestamp selection.
//!
//! These tests verify that events with `\fad` or `\fade` override tags
//! produce render timestamps that skip past the transparent fade-in phase,
//! so the rendered SUP frame contains visible text.

use ass_parser::{AssFile, Event, EventType, OverrideTag};

// compute_render_pts is pub(crate) so tests access it via ass2sup_cli::
use ass2sup_cli::compute_render_pts;

use color_quantizer::Quantizer;
use pgs_encoder::PgsEncoder;
use subtitle_renderer::{RenderConfig, Renderer};

/// Parse a single dialogue event from an ASS line.
fn event_from_line(line: &str) -> Event {
    Event::parse_from_line(EventType::Dialogue, line).expect("failed to parse event line")
}

/// Load a fixture ASS file from the workspace tests/fixtures directory.
fn load_fixture(name: &str) -> AssFile {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let path = manifest.join("../../tests/fixtures").join(name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e));
    AssFile::parse(&content).expect("failed to parse fixture ASS")
}

// ---------------------------------------------------------------------------
// Unit tests for compute_render_pts
// ---------------------------------------------------------------------------

#[test]
fn test_no_fade_returns_start_time() {
    let e = event_from_line("0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello");
    assert_eq!(
        compute_render_pts(&e),
        1000,
        "no fade tags -> start time unchanged"
    );
}

#[test]
fn test_fad_in_returns_start_plus_fadein() {
    let e = event_from_line("0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\fad(500,800)}Faded");
    assert_eq!(
        compute_render_pts(&e),
        1500,
        "\\fad(500,800) at start=1000 -> render at 1500"
    );
}

#[test]
fn test_fad_in_zero_returns_start() {
    let e = event_from_line("0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\fad(0,800)}FadeOutOnly");
    assert_eq!(compute_render_pts(&e), 1000, "fade_in=0 -> render at start");
}

#[test]
fn test_fad_clamped_to_event_end() {
    // fade_in (1000ms) exceeds event duration (500ms) — clamp to end
    let e = event_from_line("0,0:00:01.00,0:00:01.50,Default,,0,0,0,,{\\fad(1000,0)}TooLong");
    assert_eq!(
        compute_render_pts(&e),
        1500,
        "fade_in that exceeds duration -> clamped to end"
    );
}

#[test]
fn test_parsed_fad_events_from_file() {
    // Load a real .ass file and verify compute_render_pts works on parsed events
    let ass = load_fixture("fade_effects.ass");
    let events: Vec<_> = ass.dialogue_events().collect();

    // First event: {\fad(500,500)}Simple fade at 0:00:01.00
    assert_eq!(
        compute_render_pts(events[0]),
        1500,
        "first fade event -> start+500"
    );

    // Second event: {\fad(1000,0)}Fade in only at 0:00:04.00
    assert_eq!(
        compute_render_pts(events[1]),
        5000,
        "fade_in only -> start+1000"
    );

    // Third event: {\fad(0,1000)}Fade out only at 0:00:07.00
    assert_eq!(
        compute_render_pts(events[2]),
        7000,
        "fade_out only -> start unchanged"
    );
}

#[test]
fn test_non_fade_events_unaffected() {
    // Events in position_move.ass have no fade tags
    let ass = load_fixture("position_move.ass");
    let events: Vec<_> = ass.dialogue_events().collect();
    for event in &events {
        let has_fade = event.override_tags.iter().any(|t| {
            matches!(
                t,
                OverrideTag::Fade { .. } | OverrideTag::FadeComplex { .. }
            )
        });
        if !has_fade {
            let pts = compute_render_pts(event);
            assert_eq!(
                pts,
                event.start.as_ms(),
                "non-fade event {} pts unchanged",
                event.start
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Integration tests
// ---------------------------------------------------------------------------

fn find_any_font() -> String {
    let candidates = [
        "Arial",
        "Liberation Sans",
        "DejaVu Sans",
        "Noto Sans",
        "Helvetica",
    ];
    let mut fm = subtitle_renderer::FontManager::default();
    fm.load_system_fonts();
    for name in &candidates {
        if fm.query_with_fallback(name, false, false).is_some() {
            return name.to_string();
        }
    }
    "Arial".to_string()
}

fn test_config() -> RenderConfig {
    RenderConfig {
        width: 1920,
        height: 1080,
        script_width: 1920,
        script_height: 1080,
        default_font: find_any_font(),
        default_font_size: 48.0,
    }
}

#[test]
fn test_fade_demonstrates_bug_at_start() {
    // Demonstrate the bug: rendering \fad events at event.start produces blank frames
    let ass = load_fixture("fade_effects.ass");
    let renderer = Renderer::new(test_config());
    let events: Vec<_> = ass.dialogue_events().collect();

    // First event: {\fad(500,500)}Simple fade, start=1000
    let event = &events[0];
    let frame_at_start = renderer
        .render_ass(&ass, event.start.as_ms())
        .expect("render_ass should return Some");
    let non_zero = frame_at_start.bitmap.iter().filter(|&&b| b > 0).count();
    assert_eq!(
        non_zero, 0,
        "at t=start, \\fad(500,500) event should be blank (alpha=0)"
    );

    // But at the adjusted timestamp, it should be visible
    let adjusted = compute_render_pts(event);
    assert!(
        adjusted > event.start.as_ms(),
        "render pt should be after start for fade events"
    );
    let frame_adjusted = renderer
        .render_ass(&ass, adjusted)
        .expect("render_ass should return Some");
    let non_zero_adjusted = frame_adjusted.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(
        non_zero_adjusted > 0,
        "at adjusted timestamp t={}, frame should be visible",
        adjusted
    );
}

#[test]
fn test_fade_effects_full_pipeline() {
    // Full pipeline: all fade events should produce non-zero PGS segments
    let ass = load_fixture("fade_effects.ass");
    let renderer = Renderer::new(test_config());
    let quantizer = Quantizer::new(255);
    let mut encoder = PgsEncoder::new(1920, 1080, 24.0);

    let events: Vec<_> = ass.dialogue_events().cloned().collect();
    assert_eq!(events.len(), 14, "fade_effects.ass has 14 events");

    let mut visible_count = 0;
    for event in &events {
        let render_pts = compute_render_pts(event);
        let display_pts = event.start.as_ms();
        let duration_ms = event.duration_ms();

        if let Some(frame) = renderer.render_ass(&ass, render_pts) {
            let non_zero = frame.bitmap.iter().filter(|&&b| b > 0).count();
            if non_zero > 0 {
                let q = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
                let segments = encoder.encode_frame(&q, display_pts, duration_ms);
                assert!(
                    !segments.is_empty(),
                    "visible frame should produce PGS segments"
                );
                visible_count += 1;
            }
        }
    }
    assert!(
        visible_count >= 10,
        "expected 10+ visible events from 14 fade events, got {}",
        visible_count
    );
}
