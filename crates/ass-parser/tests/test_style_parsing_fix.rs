use ass_parser::AssFile;

#[test]
fn test_v4_plus_styles_with_trailing_space_recognized() {
    // 带尾随空格的节头也应该能正确识别
    let input = "[Script Info]
Title: Test
PlayResX: 1920
PlayResY: 1080

[V4+ Styles] 
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World";

    let ass = AssFile::parse(input).unwrap();
    assert_eq!(ass.styles.len(), 1, "Should parse 1 style");
    assert_eq!(ass.styles[0].name, "Default");
    assert_eq!(ass.styles[0].font_name, "Arial");
}

#[test]
fn test_v4_plus_styles_with_inline_comment_recognized() {
    // 带内联注释的节头也应该能正确识别
    let input = "[Script Info]
Title: Test
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]; This is a comment
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World";

    let ass = AssFile::parse(input).unwrap();
    assert_eq!(ass.styles.len(), 1, "Should parse 1 style");
    assert_eq!(ass.styles[0].name, "Default");
}

#[test]
fn test_v4_styles_legacy_section_works() {
    // 验证 [V4 Styles] 节头（SSA v4 旧格式）仍然能正确工作
    let input = "[Script Info]
Title: Test

[V4 Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, TertiaryColour, BackColour, Bold, Italic, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, AlphaLevel, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,1,2,2,2,10,10,10,0,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World";

    let ass = AssFile::parse(input).unwrap();
    assert_eq!(ass.styles.len(), 1, "Should parse 1 SSA v4 style");
    assert_eq!(ass.styles[0].name, "Default");
}
