use ass_parser::AssFile;

const MINIMAL_ASS: &str = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World
Dialogue: 0,0:00:06.00,0:00:10.00,Default,,0,0,0,,Second line
"#;

#[test]
fn test_parse_minimal_ass() {
    let ass = AssFile::parse(MINIMAL_ASS).unwrap();
    assert_eq!(ass.events.len(), 2);
    assert_eq!(ass.styles.len(), 1);
    assert_eq!(ass.script_info.title, "Test");
    assert_eq!(ass.script_info.play_res_x, 1920);
    assert_eq!(ass.script_info.play_res_y, 1080);
}

#[test]
fn test_parse_dialogue_events() {
    let ass = AssFile::parse(MINIMAL_ASS).unwrap();
    let dialogues: Vec<_> = ass.dialogue_events().collect();
    assert_eq!(dialogues.len(), 2);
    assert_eq!(dialogues[0].text, "Hello World");
    assert_eq!(dialogues[1].text, "Second line");
}

#[test]
fn test_find_style() {
    let ass = AssFile::parse(MINIMAL_ASS).unwrap();
    let style = ass.find_style("Default").unwrap();
    assert_eq!(style.font_name, "Arial");
    assert_eq!(style.font_size, 20.0);
}

#[test]
fn test_find_style_not_found() {
    let ass = AssFile::parse(MINIMAL_ASS).unwrap();
    assert!(ass.find_style("Nonexistent").is_none());
}

#[test]
fn test_resolution() {
    let ass = AssFile::parse(MINIMAL_ASS).unwrap();
    let (w, h) = ass.resolution();
    assert_eq!(w, 1920);
    assert_eq!(h, 1080);
}

#[test]
fn test_parse_script_info_defaults() {
    let input = r#"[Script Info]
Title: My Subtitle

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Test
"#;
    let ass = AssFile::parse(input).unwrap();
    assert_eq!(ass.script_info.title, "My Subtitle");
    // Should have default resolution
    let (w, h) = ass.resolution();
    assert!(w > 0 && h > 0);
}

#[test]
fn test_parse_v4_styles() {
    let input = r#"[Script Info]
Title: V4 Test

[V4 Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,24,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,V4 style
"#;
    let ass = AssFile::parse(input).unwrap();
    assert_eq!(ass.styles.len(), 1);
    assert_eq!(ass.styles[0].font_size, 24.0);
}

#[test]
fn test_parse_no_events() {
    let input = r#"[Script Info]
Title: Empty

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"#;
    let ass = AssFile::parse(input).unwrap();
    assert_eq!(ass.events.len(), 0);
}

#[test]
fn test_parse_multiple_styles() {
    let input = r#"[Script Info]
Title: Multi Style

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
Style: Sign,Impact,48,&H0000FFFF,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,3,2,8,20,20,80,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Default text
Dialogue: 0,0:00:06.00,0:00:10.00,Sign,,0,0,0,,Sign text
"#;
    let ass = AssFile::parse(input).unwrap();
    assert_eq!(ass.styles.len(), 2);
    assert_eq!(ass.events.len(), 2);
    assert_eq!(ass.events[0].style_name, "Default");
    assert_eq!(ass.events[1].style_name, "Sign");
}

#[test]
fn test_parse_events_with_override_tags() {
    let input = r#"[Script Info]
Title: Override Test

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\pos(960,540)}Centered text
"#;
    let ass = AssFile::parse(input).unwrap();
    assert_eq!(ass.events.len(), 1);
    assert!(ass.events[0].has_override_tags());
}

#[test]
fn test_parse_format_detection() {
    use ass_parser::SubtitleFormat;
    use std::path::Path;
    assert!(matches!(
        SubtitleFormat::detect(Path::new("test.ass")),
        Some(SubtitleFormat::Ass)
    ));
    assert!(matches!(
        SubtitleFormat::detect(Path::new("test.srt")),
        Some(SubtitleFormat::Srt)
    ));
}
