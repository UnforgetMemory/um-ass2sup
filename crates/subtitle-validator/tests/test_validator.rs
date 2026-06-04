use ass_parser::AssFile;
use subtitle_validator::{
    report::{OverlapConfig, RuleId},
    validate, validate_strict,
};

fn parse_ass(input: &str) -> AssFile {
    AssFile::parse(input).unwrap()
}

fn minimal_ass() -> &'static str {
    r#"[Script Info]
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
"#
}

// ===== Validation Tests =====

#[test]
fn test_valid_ass() {
    let ass = parse_ass(minimal_ass());
    let report = validate(&ass);
    assert!(
        report.is_valid,
        "Valid ASS should pass. Errors: {:?}",
        report.errors()
    );
}

#[test]
fn test_v001_script_type_warning() {
    let input = r#"[Script Info]
Title: Old Format
ScriptType: v3.00
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Test
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);
    // Should have V001 warning about script type
    let v001: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V001)
        .collect();
    assert!(
        !v001.is_empty(),
        "Should have V001 warning for v3.00 script type"
    );
}

#[test]
fn test_v002_zero_resolution() {
    let input = r#"[Script Info]
Title: Zero Res
ScriptType: v4.00+
PlayResX: 0
PlayResY: 0

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Test
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);
    let v002: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V002)
        .collect();
    assert!(
        !v002.is_empty(),
        "Should have V002 error for zero resolution"
    );
}

#[test]
fn test_v003_no_events() {
    let input = r#"[Script Info]
Title: No Events
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);
    let v003: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V003)
        .collect();
    assert!(!v003.is_empty(), "Should have V003 error for no events");
}

#[test]
fn test_v004_no_styles() {
    let input = r#"[Script Info]
Title: No Styles
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Test
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);
    let v004: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V004)
        .collect();
    assert!(!v004.is_empty(), "Should have V004 warning for no styles");
}

#[test]
fn test_v006_negative_font_size() {
    let input = r#"[Script Info]
Title: Bad Font
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,-10,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Test
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);
    let v006: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V006)
        .collect();
    assert!(
        !v006.is_empty(),
        "Should have V006 warning for negative font size"
    );
}

#[test]
fn test_v008_invalid_alignment() {
    let input = r#"[Script Info]
Title: Bad Align
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,15,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Test
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);
    let v008: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V008)
        .collect();
    assert!(
        !v008.is_empty(),
        "Should have V008 error for invalid alignment 15"
    );
}

#[test]
fn test_v009_undefined_style_ref() {
    let input = r#"[Script Info]
Title: Bad Style Ref
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Nonexistent,,0,0,0,,Test
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);
    let v009: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V009)
        .collect();
    assert!(
        !v009.is_empty(),
        "Should have V009 warning for undefined style"
    );
}

#[test]
fn test_v011_start_after_end() {
    let input = r#"[Script Info]
Title: Bad Time
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:10.00,0:00:01.00,Default,,0,0,0,,Backwards
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);
    let v011: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V011)
        .collect();
    assert!(!v011.is_empty(), "Should have V011 error for start >= end");
}

#[test]
fn test_v013_unmatched_braces() {
    let input = r#"[Script Info]
Title: Bad Braces
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\b1 unmatched
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);
    let v013: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V013)
        .collect();
    assert!(
        !v013.is_empty(),
        "Should have V013 error for unmatched braces"
    );
}

// ===== Overlap Detection Tests =====

#[test]
fn test_no_overlap() {
    let input = r#"[Script Info]
Title: No Overlap
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,First
Dialogue: 0,0:00:06.00,0:00:10.00,Default,,0,0,0,,Second
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);
    assert!(
        report.overlaps.is_empty(),
        "Non-overlapping events should have no overlaps"
    );
}

#[test]
fn test_time_overlap_same_position() {
    let input = r#"[Script Info]
Title: Overlap
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,First
Dialogue: 0,0:00:03.00,0:00:08.00,Default,,0,0,0,,Second
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);
    assert!(
        !report.overlaps.is_empty(),
        "Overlapping events should be detected"
    );
}

#[test]
fn test_overlap_different_positions() {
    let input = r#"[Script Info]
Title: Different Pos
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\pos(100,100)}Top left
Dialogue: 0,0:00:03.00,0:00:08.00,Default,,0,0,0,,{\\pos(1000,900)}Bottom right
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);
    // Overlap exists but visual overlap should be false (different positions)
    if !report.overlaps.is_empty() {
        assert!(
            !report.overlaps[0].visual_overlap
                || report.overlaps[0].severity
                    != subtitle_validator::report::OverlapSeverity::Critical,
            "Different positions should not be Critical severity"
        );
    }
}

#[test]
fn test_strict_mode_more_sensitive() {
    let input = r#"[Script Info]
Title: Strict Mode
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,First
Dialogue: 0,0:00:01.05,0:00:05.00,Default,,0,0,0,,Almost same time
"#;
    let ass = parse_ass(input);

    // Lenient mode (default) - 50ms overlap below 100ms threshold
    let report_lenient = validate(&ass);

    // Strict mode - 0ms threshold
    let report_strict = validate_strict(&ass);

    // Strict mode should detect more overlaps
    assert!(
        report_strict.overlaps.len() >= report_lenient.overlaps.len(),
        "Strict mode should detect at least as many overlaps as lenient mode"
    );
}

#[test]
fn test_report_summary() {
    let ass = parse_ass(minimal_ass());
    let report = validate(&ass);
    let summary = report.summary();
    assert!(!summary.is_empty());
    assert!(summary.contains("Validation") || summary.contains("errors"));
}

#[test]
fn test_report_display() {
    let ass = parse_ass(minimal_ass());
    let report = validate(&ass);
    let display = format!("{}", report);
    assert!(!display.is_empty());
}

#[test]
fn test_stats_counting() {
    let ass = parse_ass(minimal_ass());
    let report = validate(&ass);
    assert_eq!(report.stats.total_events, 1);
    assert_eq!(report.stats.total_styles, 1);
}

#[test]
fn test_overlap_config_custom() {
    let config = OverlapConfig {
        strict: true,
        min_duration_ms: 0,
        check_visual: true,
        ignore_karaoke: false,
        position_threshold: 100.0,
        max_simultaneous_same_pos: 1,
    };

    let input = r#"[Script Info]
Title: Custom Config
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,First
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Simultaneous
"#;
    let ass = parse_ass(input);
    let validator = subtitle_validator::Validator::new().with_overlap_config(config);
    let report = validator.validate(&ass);
    // Exact same time range = critical overlap
    assert!(
        !report.overlaps.is_empty(),
        "Exact same time range should be detected as overlap"
    );
}
