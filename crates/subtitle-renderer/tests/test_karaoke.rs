//! Karaoke rendering integration tests.
//!
//! Verifies that ASS karaoke override tags (`\k`, `\kf`, `\ko`, `\kt`) produce
//! rendered frames at the midpoint of visible events.

use ass_core::SubtitleDocument;
use subtitle_renderer::{RenderConfig, Renderer};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_doc(content: &str) -> SubtitleDocument {
    SubtitleDocument::parse(content).unwrap()
}

fn default_renderer() -> Renderer {
    Renderer::new(RenderConfig::default())
}

const BASE: &str = "[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
";

// ---------------------------------------------------------------------------
// \k (instant) — switches colour instantly per syllable
// ---------------------------------------------------------------------------

#[test]
fn test_karaoke_instant() {
    let ass = format!(
        "{}Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{{\\k50}}lyrics",
        BASE
    );
    let doc = make_doc(&ass);
    let renderer = default_renderer();

    // Event visible from 1000 ms to 5000 ms → midpoint = 3000 ms.
    let frame = renderer.render_ass(&doc, 3_000);
    assert!(
        frame.is_some(),
        "Expected a rendered frame with \\k tag at midpoint"
    );

    let frame = frame.unwrap();
    assert_eq!(frame.width, 1920);
    assert_eq!(frame.height, 1080);
    assert_eq!(frame.pts_ms, 3_000);
    assert!(
        frame.duration_ms > 0,
        "duration_ms should be positive for a visible event"
    );
}

// ---------------------------------------------------------------------------
// \kf (fill) — left-to-right clip sweep
// ---------------------------------------------------------------------------

#[test]
fn test_karaoke_fill() {
    let ass = format!(
        "{}Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{{\\kf50}}lyrics",
        BASE
    );
    let doc = make_doc(&ass);
    let renderer = default_renderer();

    let frame = renderer.render_ass(&doc, 3_000);
    assert!(
        frame.is_some(),
        "Expected a rendered frame with \\kf tag at midpoint"
    );

    let frame = frame.unwrap();
    assert_eq!(frame.width, 1920);
    assert_eq!(frame.height, 1080);
    assert_eq!(frame.pts_ms, 3_000);
    assert!(frame.duration_ms > 0);
}

// ---------------------------------------------------------------------------
// \ko (outline) — outline highlight
// ---------------------------------------------------------------------------

#[test]
fn test_karaoke_outline() {
    let ass = format!(
        "{}Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{{\\ko50}}lyrics",
        BASE
    );
    let doc = make_doc(&ass);
    let renderer = default_renderer();

    let frame = renderer.render_ass(&doc, 3_000);
    assert!(
        frame.is_some(),
        "Expected a rendered frame with \\ko tag at midpoint"
    );

    let frame = frame.unwrap();
    assert_eq!(frame.width, 1920);
    assert_eq!(frame.height, 1080);
    assert_eq!(frame.pts_ms, 3_000);
    assert!(frame.duration_ms > 0);
}

// ---------------------------------------------------------------------------
// \kt (timing) — absolute per-syllable timing
// ---------------------------------------------------------------------------

#[test]
fn test_karaoke_timing() {
    let ass = format!(
        "{}Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{{\\kt50}}lyrics",
        BASE
    );
    let doc = make_doc(&ass);
    let renderer = default_renderer();

    let frame = renderer.render_ass(&doc, 3_000);
    assert!(
        frame.is_some(),
        "Expected a rendered frame with \\kt tag at midpoint"
    );

    let frame = frame.unwrap();
    assert_eq!(frame.width, 1920);
    assert_eq!(frame.height, 1080);
    assert_eq!(frame.pts_ms, 3_000);
    assert!(frame.duration_ms > 0);
}
