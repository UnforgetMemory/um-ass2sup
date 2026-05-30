use ass_parser::{AssFile, OverrideTag};

const SIMPLE_ASS: &str = include_str!("../../../tests/fixtures/simple.ass");
const EFFECTS_ASS: &str = include_str!("../../../tests/fixtures/effects.ass");
const KARAOKE_ASS: &str = include_str!("../../../tests/fixtures/karaoke.ass");
const OVERLAPPING_ASS: &str = include_str!("../../../tests/fixtures/overlapping.ass");
const ERRORS_ASS: &str = include_str!("../../../tests/fixtures/errors.ass");

#[test]
fn simple_ass_event_count() {
    let ass = AssFile::parse(SIMPLE_ASS).expect("simple.ass should parse");
    assert_eq!(ass.events.len(), 3);
    assert!(ass.events.iter().all(|e| e.is_dialogue()));
}

#[test]
fn simple_ass_style_count() {
    let ass = AssFile::parse(SIMPLE_ASS).expect("simple.ass should parse");
    assert_eq!(ass.styles.len(), 1);
    assert_eq!(ass.styles[0].name, "Default");
    assert_eq!(ass.styles[0].font_name, "Arial");
    assert!((ass.styles[0].font_size - 48.0).abs() < f64::EPSILON);
}

#[test]
fn simple_ass_no_override_tags() {
    let ass = AssFile::parse(SIMPLE_ASS).expect("simple.ass should parse");
    for event in &ass.events {
        assert!(event.override_tags.is_empty(), "simple.ass should have no override tags");
    }
}

#[test]
fn simple_ass_resolution() {
    let ass = AssFile::parse(SIMPLE_ASS).expect("simple.ass should parse");
    assert_eq!(ass.resolution(), (1920, 1080));
}

#[test]
fn effects_ass_event_count() {
    let ass = AssFile::parse(EFFECTS_ASS).expect("effects.ass should parse");
    assert_eq!(ass.events.len(), 9);
}

#[test]
fn effects_ass_has_pos_tag() {
    let ass = AssFile::parse(EFFECTS_ASS).expect("effects.ass should parse");
    let first = &ass.events[0];
    assert!(first.override_tags.iter().any(|t| matches!(t, OverrideTag::Pos { x, y } if (*x - 960.0).abs() < f64::EPSILON && (*y - 540.0).abs() < f64::EPSILON)));
}

#[test]
fn effects_ass_has_move_tag() {
    let ass = AssFile::parse(EFFECTS_ASS).expect("effects.ass should parse");
    let second = &ass.events[1];
    assert!(second.override_tags.iter().any(|t| matches!(t, OverrideTag::Move { x1, y1, x2, y2, .. } if (*x1 - 100.0).abs() < f64::EPSILON && (*y1 - 100.0).abs() < f64::EPSILON && (*x2 - 1820.0).abs() < f64::EPSILON && (*y2 - 1000.0).abs() < f64::EPSILON)));
}

#[test]
fn effects_ass_has_fade_and_clip() {
    let ass = AssFile::parse(EFFECTS_ASS).expect("effects.ass should parse");
    assert!(ass.events[2].override_tags.iter().any(|t| matches!(t, OverrideTag::Fade { duration_in: 500, duration_out: 500 })));
    assert!(ass.events[6].override_tags.iter().any(|t| matches!(t, OverrideTag::Clip { .. })));
    assert!(ass.events[7].override_tags.iter().any(|t| matches!(t, OverrideTag::ClipInverse { .. })));
}

#[test]
fn effects_ass_has_alignment() {
    let ass = AssFile::parse(EFFECTS_ASS).expect("effects.ass should parse");
    let last = &ass.events[8];
    assert!(last.override_tags.iter().any(|t| matches!(t, OverrideTag::AlignmentNumpad(8))));
}

#[test]
fn karaoke_ass_event_count() {
    let ass = AssFile::parse(KARAOKE_ASS).expect("karaoke.ass should parse");
    assert_eq!(ass.events.len(), 4);
}

#[test]
fn karaoke_ass_has_karaoke_tags() {
    let ass = AssFile::parse(KARAOKE_ASS).expect("karaoke.ass should parse");
    for event in &ass.events {
        assert!(event.override_tags.iter().any(|t| matches!(t, OverrideTag::Karaoke { .. })),
            "each karaoke event should have karaoke tags");
    }
}

#[test]
fn overlapping_ass_event_count() {
    let ass = AssFile::parse(OVERLAPPING_ASS).expect("overlapping.ass should parse");
    assert_eq!(ass.events.len(), 5);
    assert_eq!(ass.styles.len(), 3);
}

#[test]
fn overlapping_ass_has_different_styles() {
    let ass = AssFile::parse(OVERLAPPING_ASS).expect("overlapping.ass should parse");
    let style_names: Vec<&str> = ass.events.iter().map(|e| e.style_name.as_str()).collect();
    assert!(style_names.contains(&"Default"));
    assert!(style_names.contains(&"Top"));
    assert!(style_names.contains(&"Karaoke"));
}

#[test]
fn errors_ass_lenient_parses_valid_events() {
    let (ass, errors) = AssFile::parse_lenient(ERRORS_ASS);
    assert!(!errors.is_empty(), "errors.ass should produce parse errors");
    let dialogue_texts: Vec<&str> = ass.events.iter().filter(|e| e.is_dialogue()).map(|e| e.text.as_str()).collect();
    assert!(dialogue_texts.contains(&"Valid subtitle before errors"));
    assert!(dialogue_texts.contains(&"Valid subtitle after errors"));
}

#[test]
fn errors_ass_skips_invalid_timestamp_event() {
    let (ass, errors) = AssFile::parse_lenient(ERRORS_ASS);
    let timestamp_errors: Vec<_> = errors.iter().filter(|e| matches!(e, ass_parser::ParseError::InvalidTimestamp(_))).collect();
    assert!(!timestamp_errors.is_empty(), "should have timestamp error from invalid time");
    assert!(ass.events.iter().all(|e| e.text != "Invalid start timestamp"), "invalid event should be skipped");
}
