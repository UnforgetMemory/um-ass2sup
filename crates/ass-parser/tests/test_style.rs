use ass_parser::style::Style;

#[test]
fn test_default_style() {
    let s = Style::default();
    assert_eq!(s.name, "Default");
    assert_eq!(s.font_name, "Arial");
    assert_eq!(s.font_size, 20.0);
    assert!(!s.bold);
    assert_eq!(s.alignment, 2);
    assert_eq!(s.margin_l, 10);
    assert_eq!(s.margin_r, 10);
    assert_eq!(s.margin_v, 10);
    assert_eq!(s.encoding, 1);
}

#[test]
fn test_parse_style_line() {
    let line = "Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1";
    let s = Style::parse_from_line(line).unwrap();
    assert_eq!(s.name, "Default");
    assert_eq!(s.font_name, "Arial");
    assert_eq!(s.font_size, 20.0);
    assert!(!s.bold);
    assert_eq!(s.alignment, 2);
}

#[test]
fn test_parse_style_bold() {
    let line = "Default,Arial,28,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1";
    let s = Style::parse_from_line(line).unwrap();
    assert!(s.bold);
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

#[test]
fn test_parse_v4_style_line() {
    let line = "Default,Arial,48,16777215,255,0,0,0,0,1,2,2,2,10,10,10,0,1";
    let s = Style::parse_from_line_v4(line).unwrap();
    assert_eq!(s.name, "Default");
    assert_eq!(s.font_name, "Arial");
    assert_eq!(s.font_size, 48.0);
    assert!(!s.bold);
    assert!(!s.italic);
    assert!(!s.underline);
    assert!(!s.strikeout);
    assert_eq!(s.border_style, 1);
    assert!((s.outline_width - 2.0).abs() < f64::EPSILON);
    assert!((s.shadow_depth - 2.0).abs() < f64::EPSILON);
    assert_eq!(s.alignment, 2);
    assert_eq!(s.margin_l, 10);
    assert_eq!(s.margin_r, 10);
    assert_eq!(s.margin_v, 10);
    assert_eq!(s.encoding, 1);
    assert_eq!(s.relative_to, 0);
}

#[test]
fn test_parse_v4_style_tertiary_colour_maps_to_outline() {
    let line = "Default,Arial,48,16777215,255,12345,0,0,0,1,2,2,2,10,10,10,0,1";
    let s = Style::parse_from_line_v4(line).unwrap();
    assert_eq!(s.outline_color.to_ass_hex(), "&H00003039");
}

#[test]
fn test_parse_v4_style_too_few_fields() {
    let line = "Default,Arial,48";
    assert!(Style::parse_from_line_v4(line).is_err());
}

#[test]
fn test_parse_v4_style_alpha_level_ignored() {
    let line_a0 = "Default,Arial,48,16777215,255,0,0,0,0,1,2,2,2,10,10,10,0,1";
    let line_a255 = "Default,Arial,48,16777215,255,0,0,0,0,1,2,2,2,10,10,10,255,1";
    let s0 = Style::parse_from_line_v4(line_a0).unwrap();
    let s255 = Style::parse_from_line_v4(line_a255).unwrap();
    assert_eq!(s0, s255);
}
