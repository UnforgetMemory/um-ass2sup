use ass_parser::AssFile;
mod common;
use subtitle_renderer::{RenderConfig, Renderer};

#[test]
fn test_perspective_frx_renders() {
    let ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\frx45}PerspectiveX
"#;
    let af = AssFile::parse(ass).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&af, 2000);
    assert!(frame.is_some(), "\\frx45 perspective should render");
    assert!(
        frame.unwrap().bitmap.iter().any(|&b| b != 0),
        "\\frx45 perspective text should have visible pixels"
    );
}

#[test]
fn test_perspective_with_org() {
    let ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\org(960,540)\frx45}PerspOrg
"#;
    let af = AssFile::parse(ass).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&af, 2000);
    assert!(frame.is_some(), "\\frx45 with \\org should render");
    assert!(
        frame.unwrap().bitmap.iter().any(|&b| b != 0),
        "\\frx45 with \\org text should have visible pixels"
    );
}

#[test]
fn test_perspective_fry_renders() {
    let ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\fry30}PerspectiveY
"#;
    let doc = common::parse_doc(ass);
    let renderer = Renderer::new(RenderConfig::default());
    let frame = common::render_doc(&renderer, &doc, 2000).unwrap();
    assert!(
        frame.bitmap.iter().any(|&b| b != 0),
        "\\fry30 perspective text should have visible pixels"
    );
}

#[test]
fn test_perspective_both_renders() {
    let ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\frx20\fry15}PerspectiveBoth
"#;
    let doc = common::parse_doc(ass);
    let renderer = Renderer::new(RenderConfig::default());
    let frame = common::render_doc(&renderer, &doc, 2000).unwrap();
    assert!(
        frame.bitmap.iter().any(|&b| b != 0),
        "\\frx20\\fry15 text should have visible pixels"
    );
}

#[test]
fn test_perspective_differs_from_plain() {
    let plain_ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,PlainText
"#;
    let persp_ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\frx45}PlainText
"#;
    let plain_doc = common::parse_doc(plain_ass);
    let persp_doc = common::parse_doc(persp_ass);
    let renderer = Renderer::new(RenderConfig::default());
    let plain_frame = common::render_doc(&renderer, &plain_doc, 2000).unwrap();
    let persp_frame = common::render_doc(&renderer, &persp_doc, 2000).unwrap();
    assert_ne!(
        plain_frame.bitmap, persp_frame.bitmap,
        "\\frx45 should produce different bitmap than plain text"
    );
}
