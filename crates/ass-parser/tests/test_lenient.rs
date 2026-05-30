use ass_parser::{AssFile, ParseError};

const ERRORS_ASS: &str = include_str!("../../../tests/fixtures/errors.ass");

#[test]
fn lenient_parses_valid_events_despite_errors() {
    let (ass, errors) = AssFile::parse_lenient(ERRORS_ASS);
    assert!(!errors.is_empty(), "should collect parse errors");
    assert!(ass.events.len() >= 3, "should parse valid events, got {}", ass.events.len());
}

#[test]
fn lenient_collects_style_errors() {
    let (_ass, errors) = AssFile::parse_lenient(ERRORS_ASS);
    let style_errors: Vec<_> = errors.iter().filter(|e| matches!(e, ParseError::InvalidStyle(_))).collect();
    assert!(!style_errors.is_empty(), "should have style parse errors from BadStyle line");
}

#[test]
fn lenient_collects_event_errors() {
    let (_ass, errors) = AssFile::parse_lenient(ERRORS_ASS);
    let event_errors: Vec<_> = errors.iter().filter(|e| matches!(e, ParseError::InvalidEvent(_) | ParseError::InvalidTimestamp(_))).collect();
    assert!(!event_errors.is_empty(), "should have event parse errors");
}

#[test]
fn lenient_preserves_valid_style() {
    let (ass, _errors) = AssFile::parse_lenient(ERRORS_ASS);
    assert!(ass.styles.iter().any(|s| s.name == "Default"), "Default style should survive");
}

#[test]
fn lenient_skips_malformed_style_missing_fields() {
    let content = "\
[Script Info]
Title: Test

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Good,Arial,48,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
Style: Bad,Arial,48

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.00,Good,,0,0,0,,Valid text
";
    let (ass, errors) = AssFile::parse_lenient(content);
    assert_eq!(ass.styles.len(), 1, "only good style should be kept");
    assert_eq!(ass.styles[0].name, "Good");
    assert!(!errors.is_empty(), "bad style should produce an error");
}

#[test]
fn lenient_skips_malformed_event_bad_timestamp() {
    let content = "\
[Script Info]
Title: Test

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,First valid
Dialogue: 0,bad-time,0:00:06.00,Default,,0,0,0,,Should be skipped
Dialogue: 0,0:00:07.00,0:00:10.00,Default,,0,0,0,,Second valid
";
    let (ass, errors) = AssFile::parse_lenient(content);
    assert_eq!(ass.events.len(), 2, "valid events should survive, invalid skipped");
    assert_eq!(ass.events[0].text, "First valid");
    assert_eq!(ass.events[1].text, "Second valid");
    assert!(!errors.is_empty());
}

#[test]
fn lenient_skips_malformed_event_too_few_fields() {
    let content = "\
[Script Info]
Title: Test

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Good event
Dialogue: 0,0:00:04.00,0:00:06.00,Default
Dialogue: 0,0:00:07.00,0:00:10.00,Default,,0,0,0,,Another good
";
    let (ass, errors) = AssFile::parse_lenient(content);
    assert_eq!(ass.events.len(), 2, "events with too few fields should be skipped");
    assert!(!errors.is_empty());
}

#[test]
fn lenient_empty_input() {
    let (ass, errors) = AssFile::parse_lenient("");
    assert!(ass.events.is_empty());
    assert!(ass.styles.is_empty());
    assert!(errors.is_empty());
}

#[test]
fn lenient_no_script_info_uses_defaults() {
    let content = "\
[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Hello
";
    let (ass, errors) = AssFile::parse_lenient(content);
    assert!(errors.is_empty());
    assert_eq!(ass.script_info.play_res_x, 1920);
    assert_eq!(ass.script_info.play_res_y, 1080);
    assert_eq!(ass.script_info.script_type, "v4.00+");
    assert_eq!(ass.events.len(), 1);
}

#[test]
fn lenient_valid_file_produces_zero_errors() {
    let content = "\
[Script Info]
Title: Clean
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Hello
";
    let (ass, errors) = AssFile::parse_lenient(content);
    assert!(errors.is_empty(), "valid file should produce no errors, got {:?}", errors);
    assert_eq!(ass.events.len(), 1);
    assert_eq!(ass.styles.len(), 1);
}

#[test]
fn lenient_mixed_valid_and_invalid_events() {
    let content = "\
[Script Info]
Title: Mixed

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Valid 1
Dialogue: 0,0:00:04.00,0:00:06.00,Default,,0,0,0,,{\\bord(3}unmatched
Dialogue: 0,0:00:07.00,0:00:10.00,Default,,0,0,0,,Valid 2
";
    let (ass, errors) = AssFile::parse_lenient(content);
    // parse_lenient doesn't validate braces (that's the validator's job via V013)
    // all 3 events should parse successfully since they have correct field structure
    assert_eq!(ass.events.len(), 3, "all parseable events should be kept");
    assert!(errors.is_empty(), "lenient parse should not error on valid event structure: {:?}", errors);
}
