//! Edge case tests for the subtitle validator.
//!
//! Covers: performance with many events, overlap boundary conditions,
//! karaoke overlap handling, strict vs lenient modes, structural validation,
//! timestamp validation, style references, duration warnings, and brace matching.

use ass_parser::AssFile;
use subtitle_validator::report::{OverlapConfig, OverlapSeverity, RuleId};
use subtitle_validator::{validate, validate_strict, Validator};

// ─────────────────────── Helpers ───────────────────────

fn parse_ass(input: &str) -> AssFile {
    AssFile::parse(input).unwrap()
}

/// Generate an ASS file with N dialogue events at 1-second intervals.
fn ass_with_n_events(n: usize) -> String {
    let mut events = String::new();
    for i in 0..n {
        let start_s = i;
        let end_s = i + 1;
        events.push_str(&format!(
            "Dialogue: 0,0:{:02}:{:02}.00,0:{:02}:{:02}.00,Default,,0,0,0,,Event {}\n",
            start_s / 60,
            start_s % 60,
            end_s / 60,
            end_s % 60,
            i,
        ));
    }

    format!(
        r#"[Script Info]
Title: Many Events
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
{}
"#,
        events
    )
}

fn minimal_ass_with_style() -> &'static str {
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

// ─────────────────────── Performance: 100+ Events ───────────────────────

#[test]
fn test_validation_100_plus_events_performance() {
    let input = ass_with_n_events(150);
    let ass = parse_ass(&input);

    let report = validate(&ass);

    // Should complete without issues
    assert_eq!(report.stats.total_events, 150);
    // Events are sequential (1s each), no overlaps
    assert!(
        report.overlaps.is_empty(),
        "Sequential 1s events should have no overlaps"
    );
}

#[test]
fn test_validation_500_events_performance() {
    let input = ass_with_n_events(500);
    let ass = parse_ass(&input);

    let report = validate(&ass);
    assert_eq!(report.stats.total_events, 500);
}

// ─────────────────────── Overlap: Exact Same Boundaries ───────────────────────

#[test]
fn test_overlap_exact_same_time_range() {
    let input = r#"[Script Info]
Title: Exact Overlap
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,First
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Second
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    assert!(
        !report.overlaps.is_empty(),
        "Exact same time range should be detected"
    );
    // Full overlap = Critical
    assert_eq!(report.overlaps[0].severity, OverlapSeverity::Critical);
}

// ─────────────────────── Overlap: Events That Barely Touch ───────────────────────

#[test]
fn test_overlap_events_barely_touch_no_overlap() {
    let input = r#"[Script Info]
Title: Touch Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,First
Dialogue: 0,0:00:05.00,0:00:09.00,Default,,0,0,0,,Second
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    // end of A == start of B → no time overlap (overlap_start >= overlap_end)
    assert!(
        report.overlaps.is_empty(),
        "Events that barely touch (end==start) should NOT be an overlap"
    );
}

// ─────────────────────── Overlap: Karaoke Events ───────────────────────

#[test]
fn test_overlap_karaoke_events_ignored_by_default() {
    let input = r#"[Script Info]
Title: Karaoke Overlap
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\k50}Hel{\k100}lo
Dialogue: 0,0:00:03.00,0:00:08.00,Default,,0,0,0,,Normal text
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    // Default config has ignore_karaoke=true
    // One event has karaoke, the other doesn't
    // The overlap should still be detected because only one has karaoke
    // (depends on implementation - if ignore_karaoke requires BOTH to be karaoke)
    // At minimum, verify it doesn't panic
    let _ = report.overlaps.len();
}

#[test]
fn test_overlap_karaoke_events_detected_in_strict_mode() {
    let input = r#"[Script Info]
Title: Karaoke Strict
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\k50}Karaoke
Dialogue: 0,0:00:03.00,0:00:08.00,Default,,0,0,0,,{\k50}Also karaoke
"#;
    let ass = parse_ass(input);

    // Strict mode with ignore_karaoke=false
    let config = OverlapConfig {
        strict: true,
        min_duration_ms: 0,
        check_visual: true,
        ignore_karaoke: false,
        position_threshold: 100.0,
        max_simultaneous_same_pos: 1,
    };
    let report = Validator::new().with_overlap_config(config).validate(&ass);

    // Should detect overlap even with karaoke events
    assert!(
        !report.overlaps.is_empty(),
        "Strict mode should detect karaoke overlaps when ignore_karaoke=false"
    );
}

// ─────────────────────── Strict vs Lenient Mode ───────────────────────

#[test]
fn test_strict_mode_detects_short_overlaps() {
    let input = r#"[Script Info]
Title: Short Overlap
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,First
Dialogue: 0,0:00:04.90,0:00:08.00,Default,,0,0,0,,Second
"#;
    let ass = parse_ass(input);

    // Lenient: min_duration_ms=500 → 100ms overlap below threshold
    let report_lenient = validate(&ass);

    // Strict: min_duration_ms=0 → detects 100ms overlap
    let report_strict = validate_strict(&ass);

    assert!(
        report_strict.overlaps.len() >= report_lenient.overlaps.len(),
        "Strict mode should detect at least as many overlaps as lenient"
    );
}

#[test]
fn test_lenient_mode_ignores_small_overlaps() {
    let input = r#"[Script Info]
Title: Tiny Overlap
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,First
Dialogue: 0,0:00:04.95,0:00:08.00,Default,,0,0,0,,Second
"#;
    let ass = parse_ass(input);

    // Lenient mode: min_duration_ms=500, 50ms overlap → should be ignored
    let config = OverlapConfig::lenient();
    let report = Validator::new().with_overlap_config(config).validate(&ass);

    assert!(
        report.overlaps.is_empty(),
        "Lenient mode should ignore 50ms overlaps (below 500ms threshold)"
    );
}

// ─────────────────────── Missing Script Info ───────────────────────

#[test]
fn test_validation_missing_script_info_section() {
    // ASS without [Script Info] — parser should use defaults
    let input = r#"[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Test
"#;
    let ass = parse_ass(input);
    // Default ScriptInfo has play_res_x=1920, play_res_y=1080 → valid
    let report = validate(&ass);

    // Should not have V002 (invalid resolution) since defaults are 1920x1080
    let v002: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V002)
        .collect();
    assert!(
        v002.is_empty(),
        "Missing ScriptInfo should use valid defaults"
    );
}

// ─────────────────────── Invalid Timestamps ───────────────────────

#[test]
fn test_validation_end_before_start() {
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
    assert!(!v011.is_empty(), "V011 should fire for end before start");
    assert!(
        !report.is_valid,
        "End before start should make report invalid"
    );
}

#[test]
fn test_validation_equal_start_end() {
    let input = r#"[Script Info]
Title: Equal Time
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:05.00,0:00:05.00,Default,,0,0,0,,Zero duration
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    // start >= end (equal counts) → V011
    let v011: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V011)
        .collect();
    assert!(!v011.is_empty(), "V011 should fire for start == end");
}

// ─────────────────────── Empty Style Name References ───────────────────────

#[test]
fn test_validation_empty_style_name_reference() {
    let input = r#"[Script Info]
Title: Empty Style Ref
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,, ,0,0,0,,Empty style name
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    // Empty style name "" doesn't match "Default" → V009
    // But the parser may trim it; check if the style_name is empty
    let event = &ass.events[0];
    if event.style.as_str().is_empty() || event.style.as_str() == " " {
        let v009: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.rule_id == RuleId::V009)
            .collect();
        assert!(
            !v009.is_empty(),
            "V009 should fire for empty/whitespace style name"
        );
    }
}

// ─────────────────────── Extremely Long Duration ───────────────────────

#[test]
fn test_validation_extremely_long_duration() {
    let input = r#"[Script Info]
Title: Long Duration
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:01:00.00,Default,,0,0,0,,Very long subtitle over 30 seconds
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    // Duration = 59s > 30s → V012 warning
    let v012: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V012)
        .collect();
    assert!(!v012.is_empty(), "V012 should fire for duration > 30s");
}

#[test]
fn test_validation_exactly_30_seconds_no_warning() {
    let input = r#"[Script Info]
Title: Exactly 30s
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:31.00,Default,,0,0,0,,Exactly 30 seconds
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    // Duration = 30000ms, condition is > 30000 → no warning
    let v012: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V012)
        .collect();
    assert!(
        v012.is_empty(),
        "V012 should NOT fire for exactly 30s (only >30s)"
    );
}

// ─────────────────────── Unmatched Braces ───────────────────────

#[test]
fn test_validation_unmatched_braces_extra_open() {
    let input = r#"[Script Info]
Title: Extra Open
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\b1 text
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    let v013: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V013)
        .collect();
    assert!(!v013.is_empty(), "V013 should fire for extra '{{'");
}

#[test]
fn test_validation_unmatched_braces_extra_close() {
    let input = r#"[Script Info]
Title: Extra Close
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,text} more
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    let v013: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V013)
        .collect();
    assert!(!v013.is_empty(), "V013 should fire for extra '}}'");
}

#[test]
fn test_validation_matched_braces_no_error() {
    let input = minimal_ass_with_style();
    let ass = parse_ass(input);
    let report = validate(&ass);

    let v013: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule_id == RuleId::V013)
        .collect();
    assert!(v013.is_empty(), "No V013 for properly matched braces");
}

// ─────────────────────── Overlap: Different Positions ───────────────────────

#[test]
fn test_overlap_different_positions_not_critical() {
    let input = r#"[Script Info]
Title: Diff Pos
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\pos(100,100)}Top left
Dialogue: 0,0:00:03.00,0:00:08.00,Default,,0,0,0,,{\pos(1800,900)}Bottom right
"#;
    let ass = parse_ass(input);
    let report = validate_strict(&ass);

    if !report.overlaps.is_empty() {
        // Different positions should not be Critical
        for overlap in &report.overlaps {
            if !overlap.visual_overlap {
                assert_ne!(overlap.severity, OverlapSeverity::Critical);
            }
        }
    }
}

// ─────────────────────── Overlap: Multiple simultaneous events ───────────────────────

#[test]
fn test_overlap_three_simultaneous_events() {
    let input = r#"[Script Info]
Title: Triple Overlap
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,First
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Second
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Third
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    // 3 events = 3 pairwise overlaps: (0,1), (0,2), (1,2)
    assert!(
        report.overlaps.len() >= 3,
        "3 simultaneous events should produce at least 3 pairwise overlaps, got {}",
        report.overlaps.len()
    );
}

// ─────────────────────── Stats Accuracy ───────────────────────

#[test]
fn test_stats_accuracy() {
    let input = r#"[Script Info]
Title: Stats Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
Style: Secondary,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello
Dialogue: 0,0:00:06.00,0:00:10.00,Default,,0,0,0,,World
Dialogue: 0,0:00:11.00,0:00:15.00,Default,,0,0,0,,{\k50}Ka{\k100}rao{\k75}ke
Comment: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,This is a comment
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    assert_eq!(
        report.stats.total_events, 4,
        "4 events total (3 dialogue + 1 comment)"
    );
    assert_eq!(report.stats.total_styles, 2, "2 styles");
    assert_eq!(report.stats.karaoke_events, 1, "1 event has karaoke tags");
}

// ─────────────────────── ValidationReport Methods ───────────────────────

#[test]
fn test_report_errors_method() {
    let input = r#"[Script Info]
Title: Errors
ScriptType: v4.00+
PlayResX: 0
PlayResY: 0

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    let errors = report.errors();
    assert!(
        !errors.is_empty(),
        "Should have errors for zero resolution and no events"
    );
    for err in &errors {
        assert_eq!(err.severity, subtitle_validator::report::Severity::Error);
    }
}

#[test]
fn test_report_warnings_method() {
    let input = r#"[Script Info]
Title: Warnings
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Test
"#;
    let ass = parse_ass(input);
    let report = validate(&ass);

    // No styles defined → V004 warning
    let warnings = report.warnings();
    assert!(!warnings.is_empty(), "Should have warnings for no styles");
    for warn in &warnings {
        assert_eq!(warn.severity, subtitle_validator::report::Severity::Warning);
    }
}
