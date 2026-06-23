mod common;
use subtitle_renderer::{RenderConfig, Renderer};

// ── Embedded font data loadable ─────────────────────────────────

#[test]
fn test_embedded_font_data_loadable() {
    let font_path = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
    let font_data = std::fs::read(font_path).expect("DejaVu Sans TTF should exist");
    let mut renderer = Renderer::new(RenderConfig::default());
    let id = renderer.cosmic_render().load_font_data(font_data);
    assert_ne!(
        id,
        fontdb::ID::dummy(),
        "load_font_data should return a valid font ID"
    );
}

#[test]
fn test_embedded_font_override_renders() {
    let font_path = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
    let font_data = std::fs::read(font_path).expect("DejaVu Sans TTF should exist");
    let mut renderer = Renderer::new(RenderConfig::default());
    renderer.cosmic_render().load_font_data(font_data);

    let ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,EmbeddedFontOverride
"#;
    let doc = common::parse_doc(ass);
    let frame = common::render_doc(&renderer, &doc, 2000);
    assert!(
        frame.is_some(),
        "Render with loaded font data should not panic"
    );
    let f = frame.unwrap();
    assert!(
        f.bitmap.iter().any(|&b| b != 0),
        "Text with loaded font data should have visible pixels"
    );
}

// ── Embedded font references parsed from ASS ─────────────────────

#[test]
fn test_embedded_font_parse_has_references() {
    let ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Fonts]
fontname: TestFont, filename: /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,EmbeddedTest
"#;
    let doc = common::parse_doc(ass);
    assert!(
        !doc.fonts.is_empty(),
        "Parsed document should contain embedded font references"
    );
    assert_eq!(
        doc.fonts[0].font_name, "TestFont",
        "First embedded font should be TestFont"
    );
    assert_eq!(
        doc.fonts[0].filename, "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "Embedded font filename should match"
    );
}

#[test]
fn test_embedded_font_missing_file_reference() {
    let ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Fonts]
fontname: MissingFont, filename: nonexistent_font_12345.ttf

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,MissingTest
"#;
    let doc = common::parse_doc(ass);
    assert!(
        !doc.fonts.is_empty(),
        "Missing font reference should still be parsed into fonts list"
    );
    assert_eq!(
        doc.fonts[0].filename, "nonexistent_font_12345.ttf",
        "Missing font filename should be preserved in parsed document"
    );
}

#[test]
fn test_embedded_font_empty_filename() {
    let ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Fonts]
fontname: EmptyFont

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,EmptyTest
"#;
    let doc = common::parse_doc(ass);
    assert!(
        !doc.fonts.is_empty(),
        "Embedded font entry with empty filename should still be parsed"
    );
    assert_eq!(
        doc.fonts[0].font_name, "EmptyFont",
        "Fontname should be EmptyFont"
    );
    assert!(
        doc.fonts[0].filename.is_empty(),
        "Filename should be empty when not specified"
    );
}
