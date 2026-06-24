//! Integration tests for subtitle-renderer.
//!
//! Tests cover PixmapPool-backed Renderer creation, multi-event rendering,
//! and edge cases with empty subtitle documents.

use ass_core::SubtitleDocument;
use subtitle_renderer::{RenderConfig, Renderer};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_ass_doc() -> SubtitleDocument {
    SubtitleDocument::parse(
        r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello
Dialogue: 0,0:00:06.00,0:00:10.00,Default,,0,0,0,,World
"#,
    )
    .unwrap()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_renderer_creation() {
    // Just verify construction doesn't panic.
    let _renderer = Renderer::new(RenderConfig::default());
}

#[test]
fn test_renderer_multiple_events() {
    let renderer = Renderer::new(RenderConfig::default());
    let doc = make_ass_doc();

    // First dialogue: 0:00:01.00 → 0:00:05.00 → visible at 3000 ms.
    let f1 = renderer.render_ass(&doc, 3000);
    // Second dialogue: 0:00:06.00 → 0:00:10.00 → visible at 8000 ms.
    let f2 = renderer.render_ass(&doc, 8000);

    assert!(f1.is_some(), "First event should render at t=3000 ms");
    assert!(f2.is_some(), "Second event should render at t=8000 ms");
}

#[test]
fn test_renderer_subtitle_document_default() {
    let renderer = Renderer::new(RenderConfig::default());
    let doc = SubtitleDocument::default();

    let result = renderer.render_ass(&doc, 1000);
    // Renderer may return a frame even for empty doc (blank canvas).
    // Verify it doesn't panic.
    if let Some(f) = result {
        assert_eq!(f.width, 1920, "Frame width should match config");
    }
}
