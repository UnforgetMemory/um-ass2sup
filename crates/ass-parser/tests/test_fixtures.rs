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
        assert!(
            event.override_tags.is_empty(),
            "simple.ass should have no override tags"
        );
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
    assert!(ass.events[2].override_tags.iter().any(|t| matches!(
        t,
        OverrideTag::Fade {
            duration_in: 500,
            duration_out: 500
        }
    )));
    assert!(ass.events[6]
        .override_tags
        .iter()
        .any(|t| matches!(t, OverrideTag::Clip { .. })));
    assert!(ass.events[7]
        .override_tags
        .iter()
        .any(|t| matches!(t, OverrideTag::ClipInverse { .. })));
}

#[test]
fn effects_ass_has_alignment() {
    let ass = AssFile::parse(EFFECTS_ASS).expect("effects.ass should parse");
    let last = &ass.events[8];
    assert!(last
        .override_tags
        .iter()
        .any(|t| matches!(t, OverrideTag::AlignmentNumpad(8))));
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
        assert!(
            event
                .override_tags
                .iter()
                .any(|t| matches!(t, OverrideTag::Karaoke { .. })),
            "each karaoke event should have karaoke tags"
        );
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
    let dialogue_texts: Vec<&str> = ass
        .events
        .iter()
        .filter(|e| e.is_dialogue())
        .map(|e| e.text.as_str())
        .collect();
    assert!(dialogue_texts.contains(&"Valid subtitle before errors"));
    assert!(dialogue_texts.contains(&"Valid subtitle after errors"));
}

#[test]
fn errors_ass_skips_invalid_timestamp_event() {
    let (ass, errors) = AssFile::parse_lenient(ERRORS_ASS);
    let timestamp_errors: Vec<_> = errors
        .iter()
        .filter(|e| matches!(e, ass_parser::ParseError::InvalidTimestamp(_)))
        .collect();
    assert!(
        !timestamp_errors.is_empty(),
        "should have timestamp error from invalid time"
    );
    assert!(
        ass.events
            .iter()
            .all(|e| e.text != "Invalid start timestamp"),
        "invalid event should be skipped"
    );
}

const VECTOR_CLIP_ASS: &str =
    include_str!("../../subtitle-renderer/tests/fixtures/vector_clip.ass");
const WRITING_MODE_ASS: &str =
    include_str!("../../subtitle-renderer/tests/fixtures/writing_mode.ass");
const KARAOKE_KO_DETAILED_ASS: &str =
    include_str!("../../subtitle-renderer/tests/fixtures/karaoke_ko_detailed.ass");

#[test]
fn vector_clip_ass_event_count() {
    let ass = AssFile::parse(VECTOR_CLIP_ASS).expect("vector_clip.ass should parse");
    assert_eq!(ass.events.len(), 5);
}

#[test]
fn vector_clip_ass_rectangular_clip() {
    let ass = AssFile::parse(VECTOR_CLIP_ASS).expect("vector_clip.ass should parse");
    let event3 = &ass.events[3];
    assert!(
        event3
            .override_tags
            .iter()
            .any(|t| matches!(t, OverrideTag::Clip { x1, y2, .. }
        if (*x1 - 960.0).abs() < f64::EPSILON && (*y2 - 1080.0).abs() < f64::EPSILON)),
        "event 3 should have rectangular Clip tag"
    );
}

#[test]
fn vector_clip_ass_pos_tag() {
    let ass = AssFile::parse(VECTOR_CLIP_ASS).expect("vector_clip.ass should parse");
    let event4 = &ass.events[4];
    assert!(
        event4
            .override_tags
            .iter()
            .any(|t| matches!(t, OverrideTag::Pos { x, y }
        if (*x - 960.0).abs() < f64::EPSILON && (*y - 540.0).abs() < f64::EPSILON)),
        "event 4 should have Pos tag"
    );
}

#[test]
fn vector_clip_ass_vector_drawing_parse_override_tag() {
    let tag = ass_parser::parse_override_tag("clip(1,m 0 0 l 100 0 l 100 100 l 0 100)").unwrap();
    assert!(
        matches!(tag, OverrideTag::ClipDrawing { scale, .. } if (scale - 1.0).abs() < f32::EPSILON)
    );
}

#[test]
fn vector_clip_ass_vector_inverse_drawing_parse_override_tag() {
    let tag = ass_parser::parse_override_tag("iclip(1,m 0 0 l 100 0 l 100 100 l 0 100)").unwrap();
    assert!(
        matches!(tag, OverrideTag::ClipInverseDrawing { scale, .. } if (scale - 1.0).abs() < f32::EPSILON)
    );
}

#[test]
fn vector_clip_ass_vector_drawing_scale_two() {
    let tag = ass_parser::parse_override_tag("clip(2,m 0 0 l 50 0 l 50 50 l 0 50)").unwrap();
    assert!(
        matches!(tag, OverrideTag::ClipDrawing { scale, commands, .. }
        if (scale - 2.0).abs() < f32::EPSILON && commands.contains("m 0 0"))
    );
}

#[test]
fn writing_mode_ass_event_count() {
    let ass = AssFile::parse(WRITING_MODE_ASS).expect("writing_mode.ass should parse");
    assert_eq!(ass.events.len(), 3);
}

#[test]
fn writing_mode_ass_events_have_correct_text() {
    let ass = AssFile::parse(WRITING_MODE_ASS).expect("writing_mode.ass should parse");
    assert!(ass.events[0].text.contains("writing_mode(2)"));
    assert!(ass.events[0].text.contains("縦書きテスト"));
    assert!(ass.events[1].text.contains("writing_mode(3)"));
    assert!(ass.events[1].text.contains("Vertical Mode 3"));
    assert!(ass.events[2].text.contains("writing_mode(1)"));
    assert!(ass.events[2].text.contains("Horizontal"));
}

#[test]
fn writing_mode_ass_events_are_dialogue() {
    let ass = AssFile::parse(WRITING_MODE_ASS).expect("writing_mode.ass should parse");
    assert!(ass.events.iter().all(|e| e.is_dialogue()));
}

#[test]
fn writing_mode_ass_style() {
    let ass = AssFile::parse(WRITING_MODE_ASS).expect("writing_mode.ass should parse");
    assert!(ass.events.iter().all(|e| e.style_name == "Default"));
}

#[test]
fn karaoke_ko_detailed_ass_event_count() {
    let ass =
        AssFile::parse(KARAOKE_KO_DETAILED_ASS).expect("karaoke_ko_detailed.ass should parse");
    assert_eq!(ass.events.len(), 3);
}

#[test]
fn karaoke_ko_detailed_ass_ko_tags() {
    let ass =
        AssFile::parse(KARAOKE_KO_DETAILED_ASS).expect("karaoke_ko_detailed.ass should parse");
    let event0 = &ass.events[0];
    let ko_tags: Vec<_> = event0
        .override_tags
        .iter()
        .filter(|t| {
            matches!(
                t,
                OverrideTag::Karaoke {
                    style: ass_parser::KaraokeStyle::Outline,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(ko_tags.len(), 4, "event 0 should have 4 \\ko tags");
}

#[test]
fn karaoke_ko_detailed_ass_k_tags() {
    let ass =
        AssFile::parse(KARAOKE_KO_DETAILED_ASS).expect("karaoke_ko_detailed.ass should parse");
    let event1 = &ass.events[1];
    let k_tags: Vec<_> = event1
        .override_tags
        .iter()
        .filter(|t| {
            matches!(
                t,
                OverrideTag::Karaoke {
                    style: ass_parser::KaraokeStyle::Instant,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(k_tags.len(), 2, "event 1 should have 2 \\k tags");
}

#[test]
fn karaoke_ko_detailed_ass_kf_tag() {
    let ass =
        AssFile::parse(KARAOKE_KO_DETAILED_ASS).expect("karaoke_ko_detailed.ass should parse");
    let event2 = &ass.events[2];
    assert!(
        event2.override_tags.iter().any(|t| matches!(
            t,
            OverrideTag::Karaoke {
                style: ass_parser::KaraokeStyle::Fill,
                duration: 2000
            }
        )),
        "event 2 should have \\kf200 tag (duration 2000ms)"
    );
}

#[test]
fn karaoke_ko_detailed_ass_ko_durations() {
    let ass =
        AssFile::parse(KARAOKE_KO_DETAILED_ASS).expect("karaoke_ko_detailed.ass should parse");
    let event0 = &ass.events[0];
    let durations: Vec<u64> = event0
        .override_tags
        .iter()
        .filter_map(|t| {
            if let OverrideTag::Karaoke { duration, .. } = t {
                Some(*duration)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        durations,
        vec![500, 1000, 1500, 2000],
        "\\ko durations should be 50*10, 100*10, 150*10, 200*10"
    );
}
