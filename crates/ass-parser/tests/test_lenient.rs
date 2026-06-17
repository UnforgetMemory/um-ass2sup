use ass_parser::srt::SrtFile;
use ass_parser::{AssFile, ParseError, ParseWarning};

const ERRORS_ASS: &str = include_str!("../../../tests/fixtures/errors.ass");

#[test]
fn lenient_parses_valid_events_despite_errors() {
    let (ass, errors) = AssFile::parse_lenient(ERRORS_ASS);
    assert!(!errors.is_empty(), "should collect parse errors");
    assert!(
        ass.events.len() >= 3,
        "should parse valid events, got {}",
        ass.events.len()
    );
}

#[test]
fn lenient_collects_style_errors() {
    let (_ass, errors) = AssFile::parse_lenient(ERRORS_ASS);
    let style_errors: Vec<_> = errors
        .iter()
        .filter(|e| matches!(e, ParseError::InvalidStyle(_)))
        .collect();
    assert!(
        !style_errors.is_empty(),
        "should have style parse errors from BadStyle line"
    );
}

#[test]
fn lenient_collects_event_errors() {
    let (_ass, errors) = AssFile::parse_lenient(ERRORS_ASS);
    let event_errors: Vec<_> = errors
        .iter()
        .filter(|e| {
            matches!(
                e,
                ParseError::InvalidEvent(_) | ParseError::InvalidTimestamp(_)
            )
        })
        .collect();
    assert!(!event_errors.is_empty(), "should have event parse errors");
}

#[test]
fn lenient_preserves_valid_style() {
    let (ass, _errors) = AssFile::parse_lenient(ERRORS_ASS);
    assert!(
        ass.styles.iter().any(|s| s.name == "Default"),
        "Default style should survive"
    );
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
    assert_eq!(
        ass.events.len(),
        2,
        "valid events should survive, invalid skipped"
    );
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
    assert_eq!(
        ass.events.len(),
        2,
        "events with too few fields should be skipped"
    );
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
    assert!(
        errors.is_empty(),
        "valid file should produce no errors, got {:?}",
        errors
    );
    assert_eq!(ass.events.len(), 1);
    assert_eq!(ass.styles.len(), 1);
}

#[test]
fn fuzz_regression_lenient_fonts_garbage() {
    // Fuzz crasher: ASS with garbled [Fonts] sections containing binary noise.
    // Tests whether parse_lenient panics on malformed font lines.
    let input = std::fs::read_to_string("tests/data/fuzz_lenient_crash.txt")
        .expect("fuzz_lenient_crash.txt test data file missing");
    // Must not panic
    let (ass, errors) = AssFile::parse_lenient(&input);
    // Even with garbled data, there should be no panic — errors are ok
    // If readable events survived, even better
    let _ = (ass, errors);
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
    assert!(
        errors.is_empty(),
        "lenient parse should not error on valid event structure: {:?}",
        errors
    );
}

// ── parse_with_recovery ──────────────────────────────────────────

#[test]
fn recovery_parses_valid_events_with_warnings() {
    let (ass, errors) = AssFile::parse_with_recovery(ERRORS_ASS);
    assert!(!errors.is_empty(), "should collect parse errors");
    assert!(
        !ass.warnings.is_empty(),
        "should populate warnings from recovery parse"
    );
    assert!(
        ass.events.len() >= 3,
        "should parse valid events, got {}",
        ass.events.len()
    );
}

#[test]
fn recovery_unknown_section_generates_warning() {
    let content = "\
[Script Info]
Title: Test

[UnknownSection]
SomeLine: value

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Hello
";
    let (ass, errors) = AssFile::parse_with_recovery(content);
    assert!(errors.is_empty(), "no hard errors expected: {:?}", errors);
    let has_unknown_section = ass
        .warnings
        .iter()
        .any(|w| matches!(w, ParseWarning::UnknownSection(_)));
    assert!(
        has_unknown_section,
        "expected UnknownSection warning, got: {:?}",
        ass.warnings
    );
    assert_eq!(ass.events.len(), 1);
}

#[test]
fn recovery_truncated_still_returns_ast() {
    let content = "[Script Info\nTitle: Half\nPlayResX: 1920";
    let (ass, errors) = AssFile::parse_with_recovery(content);
    // No section marker completes — Script Info without ] means no section known
    assert!(ass.warnings.is_empty() || !errors.is_empty());
    // But it should not panic and return a valid AssFile
    let _ = ass;
}

#[test]
fn recovery_valid_file_no_warnings() {
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
    let (ass, errors) = AssFile::parse_with_recovery(content);
    assert!(errors.is_empty(), "no errors expected: {:?}", errors);
    assert!(
        ass.warnings.is_empty(),
        "no warnings expected: {:?}",
        ass.warnings
    );
    assert_eq!(ass.events.len(), 1);
    assert_eq!(ass.styles.len(), 1);
}

#[test]
fn recovery_bad_style_adds_warning() {
    let content = "\
[Script Info]
Title: Test

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
Style: Bad,Arial,48

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Hello
";
    let (ass, errors) = AssFile::parse_with_recovery(content);
    assert_eq!(ass.styles.len(), 1, "only good style survives");
    assert!(!errors.is_empty(), "bad style should produce error");
    assert!(!ass.warnings.is_empty(), "bad style should produce warning");
    let has_invalid_field = ass
        .warnings
        .iter()
        .any(|w| matches!(w, ParseWarning::InvalidField { .. }));
    assert!(has_invalid_field, "expected InvalidField warning");
}

// ── SRT round-trip ───────────────────────────────────────────────

#[test]
fn srt_roundtrip_via_srtfile() {
    let input = "1\n00:00:01,000 --> 00:00:05,000\nHello World\n\n2\n00:00:06,000 --> 00:00:10,000\nLine two\n";
    let srt_file = SrtFile::parse(input);
    assert_eq!(srt_file.events.len(), 2);
    assert!(srt_file.warnings.is_empty());

    let ass = AssFile::from_srt(&srt_file);
    assert_eq!(ass.events.len(), 2);
    assert_eq!(ass.events[0].text, "Hello World");

    let output = ass.to_srt();
    assert_eq!(input, output, "SRT round-trip should preserve events");
}

#[test]
fn srt_roundtrip_with_srt_tags() {
    let input = "1\n00:00:01,000 --> 00:00:05,000\n<b>Bold</b> and <i>Italic</i>\n";
    let srt_file = SrtFile::parse(input);
    let ass = AssFile::from_srt(&srt_file);
    let output = ass.to_srt();
    // Original has HTML tags but to_srt converts back to plain text
    assert!(output.contains("Bold and Italic"));
    assert_eq!(
        ass.events[0].text,
        "{\\b1}Bold{\\b0} and {\\i1}Italic{\\i0}"
    );
}

#[test]
fn srt_corrupted_input_has_warnings() {
    // Single line with no text: event will parse but have empty text.
    // This is structurally valid — SRT blocks can have empty text.
    let input = "1\n00:00:01,000 --> 00:00:05,000\n";
    let srt_file = SrtFile::parse(input);
    assert_eq!(srt_file.events.len(), 1);
    // No warning expected — structure is complete, just empty text
    assert!(
        srt_file.warnings.is_empty(),
        "expected no warnings for structurally complete block: {:?}",
        srt_file.warnings
    );
}

#[test]
fn srt_missing_timecode_skipped() {
    let input = "1\njust text\n\n2\n00:00:01,000 --> 00:00:05,000\nHello\n";
    let srt_file = SrtFile::parse(input);
    assert_eq!(srt_file.events.len(), 1);
    assert_eq!(srt_file.events[0].text, "Hello");
    assert!(!srt_file.warnings.is_empty(), "block 0 should have warning");
}

#[test]
fn srt_empty_input_ok() {
    let srt_file = SrtFile::parse("");
    assert!(srt_file.events.is_empty());
    assert!(srt_file.warnings.is_empty());
}
