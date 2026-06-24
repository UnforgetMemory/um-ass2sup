//! Basic integration tests for subtitle-renderer.
//!
//! Tests cover default config values, frame cloning, and render-lifecycle
//! behavior for ASS documents with and without visible events.

use ass_core::SubtitleDocument;
use subtitle_renderer::{RenderConfig, RenderedFrame, Renderer};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_doc(content: &str) -> SubtitleDocument {
    SubtitleDocument::parse(content).unwrap()
}

fn default_ass() -> SubtitleDocument {
    make_doc(
        r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World
"#,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_render_config_default() {
    let config = RenderConfig::default();
    assert_eq!(config.width, 1920, "default width should be 1920");
    assert_eq!(config.height, 1080, "default height should be 1080");
    assert_eq!(
        config.script_width, 1920,
        "default script_width should be 1920"
    );
    assert_eq!(
        config.script_height, 1080,
        "default script_height should be 1080"
    );
    assert_eq!(config.default_font, "Arial", "default font should be Arial");
    assert_eq!(
        config.default_font_size, 48.0,
        "default font size should be 48.0"
    );
}

#[test]
fn test_rendered_frame_clone() {
    let frame = RenderedFrame {
        pts_ms: 1000,
        duration_ms: 5000,
        width: 1920,
        height: 1080,
        bitmap: vec![42u8; 1920 * 1080 * 4],
    };
    let cloned = frame.clone();

    assert_eq!(cloned.pts_ms, frame.pts_ms, "pts_ms should match");
    assert_eq!(
        cloned.duration_ms, frame.duration_ms,
        "duration_ms should match"
    );
    assert_eq!(cloned.width, frame.width, "width should match");
    assert_eq!(cloned.height, frame.height, "height should match");
    assert_eq!(cloned.bitmap, frame.bitmap, "bitmap data should match");
}

#[test]
fn test_render_ass_simple() {
    let renderer = Renderer::new(RenderConfig::default());
    let doc = default_ass();

    // The dialogue event is visible from 1 000 ms to 5 000 ms.
    let frame = renderer.render_ass(&doc, 3_000);
    assert!(
        frame.is_some(),
        "Expected a rendered frame at timestamp 3000 ms"
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

#[test]
fn test_render_ass_empty_events() {
    let renderer = Renderer::new(RenderConfig::default());
    // A default document has no events / styles.
    // The renderer may return a frame even with no events,
    // but it should have the correct dimensions.
    let doc = SubtitleDocument::default();

    let frame = renderer.render_ass(&doc, 3_000);
    if let Some(f) = frame {
        assert_eq!(f.width, 1920, "Frame width should match config");
        assert_eq!(f.height, 1080, "Frame height should match config");
    }
}

#[test]
fn test_render_ass_outside_time() {
    let renderer = Renderer::new(RenderConfig::default());
    let doc = default_ass();

    // The dialogue event runs from 0:00:01.00 (1000 ms) to 0:00:05.00 (5000 ms).
    // Timestamp 500 ms is before the event start.
    // The renderer might still produce a frame, but with no visible content.
    let frame = renderer.render_ass(&doc, 500);
    if let Some(f) = frame {
        assert_eq!(f.width, 1920, "Frame width should match config");
        assert_eq!(f.height, 1080, "Frame height should match config");
    }
}
