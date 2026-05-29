use ass_parser::color::AssColor;

#[test]
fn test_from_ass_hex_standard() {
    // ASS format: &HAABBGGRR
    let c = AssColor::from_ass_hex("&H80FF0000").unwrap();
    assert_eq!(c.alpha, 0x80);
    assert_eq!(c.blue, 0xFF);
    assert_eq!(c.green, 0x00);
    assert_eq!(c.red, 0x00);
}

#[test]
fn test_from_ass_hex_transparent() {
    let c = AssColor::from_ass_hex("&HFF000000").unwrap();
    assert_eq!(c.alpha, 0xFF);
    assert!(c.is_transparent());
}

#[test]
fn test_from_ass_hex_white() {
    // ASS white: alpha=0, B=FF, G=FF, R=FF
    let c = AssColor::from_ass_hex("&H00FFFFFF").unwrap();
    assert_eq!(c.to_rgba(), [0xFF, 0xFF, 0xFF, 0xFF]); // RGB = FFFFFF, alpha = 255-0=255
}

#[test]
fn test_from_ass_hex_with_extra_h() {
    // Some files use &HH format
    let c = AssColor::from_ass_hex("&HH80FF0000").unwrap();
    assert_eq!(c.alpha, 0x80);
}

#[test]
fn test_from_ass_hex_with_trailing_ampersand() {
    let c = AssColor::from_ass_hex("&H80FF0000&").unwrap();
    assert_eq!(c.alpha, 0x80);
}

#[test]
fn test_from_rgb() {
    let c = AssColor::from_rgb(255, 128, 0);
    assert_eq!(c.red, 255);
    assert_eq!(c.green, 128);
    assert_eq!(c.blue, 0);
    assert_eq!(c.alpha, 0);
}

#[test]
fn test_from_rgba() {
    let c = AssColor::from_rgba(10, 20, 30, 200);
    assert_eq!(c.red, 10);
    assert_eq!(c.green, 20);
    assert_eq!(c.blue, 30);
    assert_eq!(c.alpha, 200);
}

#[test]
fn test_to_rgba() {
    // ASS color is pre-multiplied alpha with inverted alpha value
    // alpha in ASS: 0 = opaque, 255 = transparent
    let c = AssColor::from_ass_hex("&H00FFFFFF").unwrap();
    let rgba = c.to_rgba();
    assert_eq!(rgba[0], 0xFF); // R
    assert_eq!(rgba[1], 0xFF); // G
    assert_eq!(rgba[2], 0xFF); // B
    assert_eq!(rgba[3], 255);  // Alpha = 255 - 0 = 255 (fully opaque)
}

#[test]
fn test_to_ass_hex_roundtrip() {
    let c = AssColor::from_ass_hex(&"&H80FF0080".to_uppercase()).unwrap();
    let hex = c.to_ass_hex();
    // Re-parse to verify
    let c2 = AssColor::from_ass_hex(&hex).unwrap();
    assert_eq!(c.alpha, c2.alpha);
    assert_eq!(c.blue, c2.blue);
    assert_eq!(c.green, c2.green);
    assert_eq!(c.red, c2.red);
}

#[test]
fn test_with_alpha() {
    let c = AssColor::from_rgb(255, 0, 0);
    let c2 = c.with_alpha(128);
    assert_eq!(c2.alpha, 128);
    assert_eq!(c2.red, 255);
}

#[test]
fn test_constants() {
    assert_eq!(AssColor::TRANSPARENT.alpha, 0);
    assert_eq!(AssColor::WHITE.blue, 255);
    assert_eq!(AssColor::WHITE.green, 255);
    assert_eq!(AssColor::WHITE.red, 255);
    assert_eq!(AssColor::BLACK.red, 0);
}

#[test]
fn test_display() {
    let c = AssColor::from_rgb(255, 0, 0);
    let display = format!("{}", c);
    assert!(display.contains("255") || display.contains("FF"));
}

#[test]
fn test_invalid_hex() {
    assert!(AssColor::from_ass_hex("not_a_color").is_err());
    assert!(AssColor::from_ass_hex("&HGG").is_err());
}
