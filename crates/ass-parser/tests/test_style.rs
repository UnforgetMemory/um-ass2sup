use ass_parser::style::Style;

#[test]
fn test_default_style() {
    let s = Style::default();
    assert_eq!(s.name, "Default");
    assert_eq!(s.font_name, "Arial");
    assert_eq!(s.font_size, 20.0);
    assert_eq!(s.bold, false);
    assert_eq!(s.alignment, 2);
    assert_eq!(s.margin_l, 10);
    assert_eq!(s.margin_r, 10);
    assert_eq!(s.margin_v, 10);
    assert_eq!(s.encoding, 1);
}

#[test]
fn test_parse_style_line() {
    // ASS Style format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
    let line = "Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1";
    let s = Style::parse_from_line(line).unwrap();
    assert_eq!(s.name, "Default");
    assert_eq!(s.font_name, "Arial");
    assert_eq!(s.font_size, 20.0);
    assert_eq!(s.bold, false);
    assert_eq!(s.alignment, 2);
}

#[test]
fn test_parse_style_bold() {
    let line = "Default,Arial,28,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1";
    let s = Style::parse_from_line(line).unwrap();
    assert_eq!(s.bold, true); // -1 means true in ASS
}

#[test]
fn test_parse_style_too_few_fields() {
    let line = "Default,Arial,20";
    assert!(Style::parse_from_line(line).is_err());
}

#[test]
fn test_parse_style_custom_alignment() {
    let line = "Sign,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,8,20,20,60,1";
    let s = Style::parse_from_line(line).unwrap();
    assert_eq!(s.name, "Sign");
    assert_eq!(s.font_size, 48.0);
    assert_eq!(s.alignment, 8);
    assert_eq!(s.margin_v, 60);
}

#[test]
fn test_parse_style_name_with_spaces() {
    let line = "My Style Name,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1";
    let s = Style::parse_from_line(line).unwrap();
    assert_eq!(s.name, "My Style Name");
}
