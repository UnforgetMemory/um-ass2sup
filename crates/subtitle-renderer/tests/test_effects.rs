//! Integration tests for ASS rendering effect tags.
//!
//! Each test creates a simple ASS document with a single effect tag,
//! renders at the event midpoint, and asserts that a frame is produced.

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

/// ASS template with an `EFFECT_TAG` placeholder in the dialogue text.
const ASS_TEMPLATE: &str = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{EFFECT_TAG}Hello World
"#;

fn doc_with_effect(effect_tag: &str) -> SubtitleDocument {
    let content = ASS_TEMPLATE.replace("{EFFECT_TAG}", effect_tag);
    make_doc(&content)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_fade_effect() {
    let doc = doc_with_effect(r"\fad(200,300)");
    let renderer = default_renderer();

    // The event runs from 1000 ms to 5000 ms.
    // At 3000 ms we are well past the 200 ms fade-in, so rendering should succeed.
    let frame = renderer.render_ass(&doc, 3_000);
    assert!(
        frame.is_some(),
        "{{\\fad(200,300)}} should produce a frame at 3000 ms (past fade-in)"
    );
}

#[test]
fn test_clip_rect() {
    let doc = doc_with_effect(r"\clip(100,100,300,300)");
    let renderer = default_renderer();

    let frame = renderer.render_ass(&doc, 3_000);
    assert!(
        frame.is_some(),
        "{{\\clip(100,100,300,300)}} should produce a frame at 3000 ms"
    );
}

#[test]
fn test_move_tag() {
    let doc = doc_with_effect(r"\move(100,100,200,200)");
    let renderer = default_renderer();

    let frame = renderer.render_ass(&doc, 3_000);
    assert!(
        frame.is_some(),
        "{{\\move(100,100,200,200)}} should produce a frame at 3000 ms"
    );
}

#[test]
fn test_blur_effect() {
    let doc = doc_with_effect(r"\blur(5)");
    let renderer = default_renderer();

    let frame = renderer.render_ass(&doc, 3_000);
    assert!(
        frame.is_some(),
        "{{\\blur(5)}} should produce a frame at 3000 ms"
    );
}

#[test]
fn test_rotation() {
    let doc = doc_with_effect(r"\frz(45)");
    let renderer = default_renderer();

    let frame = renderer.render_ass(&doc, 3_000);
    assert!(
        frame.is_some(),
        "{{\\frz(45)}} should produce a frame at 3000 ms"
    );
}
