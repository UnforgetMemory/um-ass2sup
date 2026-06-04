use ass_parser::event::{Event, EventType};
use ass_parser::override_tag::OverrideTag;

#[test]
fn test_parse_dialogue_line() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World",
    )
    .unwrap();
    assert!(e.is_dialogue());
    assert_eq!(e.layer, 0);
    assert_eq!(e.start.as_ms(), 1000);
    assert_eq!(e.end.as_ms(), 5000);
    assert_eq!(e.style_name, "Default");
    assert_eq!(e.text, "Hello World");
    assert_eq!(e.duration_ms(), 4000);
}

#[test]
fn test_parse_comment_line() {
    let e = Event::parse_from_line(
        EventType::Comment,
        "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Comment text",
    )
    .unwrap();
    assert!(!e.is_dialogue());
    assert!(matches!(e.event_type, EventType::Comment));
}

#[test]
fn test_event_type_from_str() {
    assert!(matches!(
        EventType::parse("Dialogue"),
        Some(EventType::Dialogue)
    ));
    assert!(matches!(
        EventType::parse("Comment"),
        Some(EventType::Comment)
    ));
    assert!(matches!(
        EventType::parse("Picture"),
        Some(EventType::Picture)
    ));
    assert!(matches!(EventType::parse("Sound"), Some(EventType::Sound)));
    assert!(matches!(EventType::parse("Movie"), Some(EventType::Movie)));
    assert!(matches!(
        EventType::parse("Command"),
        Some(EventType::Command)
    ));
    assert!(EventType::parse("Other").is_none());
}

#[test]
fn test_event_with_override_tags() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\pos(100,200)}Positioned text",
    )
    .unwrap();
    assert!(e.has_override_tags());
    assert_eq!(e.override_tags.len(), 1);
    match &e.override_tags[0] {
        OverrideTag::Pos { x, y } => {
            assert_eq!(*x, 100.0);
            assert_eq!(*y, 200.0);
        }
        _ => panic!("Expected Pos tag"),
    }
}

#[test]
fn test_event_with_multiple_tags() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\b1\\i1\\pos(100,200)}Styled",
    )
    .unwrap();
    assert!(e.has_override_tags());
    assert!(e.override_tags.len() >= 3);
}

#[test]
fn test_event_karaoke_detection() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\kf50}Ka{\\kf100}ra",
    )
    .unwrap();
    assert!(
        e.has_karaoke()
            || e.override_tags
                .iter()
                .any(|t| matches!(t, OverrideTag::Karaoke { .. }))
    );
}

#[test]
fn test_event_no_override_tags() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Plain text",
    )
    .unwrap();
    assert!(!e.has_override_tags());
}

#[test]
fn test_event_empty_text() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,",
    )
    .unwrap();
    assert_eq!(e.text, "");
}

#[test]
fn test_event_with_style() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Sign,,0,0,0,,Text with sign style",
    )
    .unwrap();
    assert_eq!(e.style_name, "Sign");
}

#[test]
fn test_event_invalid_format() {
    assert!(Event::parse_from_line(EventType::Dialogue, "only,few,fields").is_err());
}

#[test]
fn test_event_move_tag() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\move(100,200,300,400)}Moved",
    )
    .unwrap();
    assert!(e.has_override_tags());
    match &e.override_tags[0] {
        OverrideTag::Move { x1, y1, x2, y2, .. } => {
            assert_eq!(*x1, 100.0);
            assert_eq!(*y1, 200.0);
            assert_eq!(*x2, 300.0);
            assert_eq!(*y2, 400.0);
        }
        _ => panic!("Expected Move tag"),
    }
}

#[test]
fn test_event_fade_tag() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\fad(500,500)}Faded",
    )
    .unwrap();
    assert!(e.has_override_tags());
    match &e.override_tags[0] {
        OverrideTag::Fade {
            duration_in,
            duration_out,
        } => {
            assert_eq!(*duration_in, 500);
            assert_eq!(*duration_out, 500);
        }
        _ => panic!("Expected Fade tag"),
    }
}

#[test]
fn test_event_font_tag() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\fnImpact}Changed font",
    )
    .unwrap();
    match &e.override_tags[0] {
        OverrideTag::FontName(name) => assert_eq!(name, "Impact"),
        _ => panic!("Expected FontName tag"),
    }
}

#[test]
fn test_event_fontsize_tag() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\fs36}Bigger",
    )
    .unwrap();
    match &e.override_tags[0] {
        OverrideTag::FontSize(size) => assert_eq!(*size, 36.0),
        _ => panic!("Expected FontSize tag"),
    }
}

#[test]
fn test_event_alignment_tag() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\an8}Top center",
    )
    .unwrap();
    match &e.override_tags[0] {
        OverrideTag::AlignmentNumpad(align) => assert_eq!(*align, 8),
        _ => panic!("Expected AlignmentNumpad tag"),
    }
}

#[test]
fn test_event_margins() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:01.00,0:00:05.00,Default,,50,100,25,0,,Custom margins",
    )
    .unwrap();
    assert_eq!(e.margin_l, 50);
    assert_eq!(e.margin_r, 100);
    assert_eq!(e.margin_v, 25);
}

#[test]
fn test_duration_saturating() {
    let e = Event::parse_from_line(
        EventType::Dialogue,
        "0,0:00:05.00,0:00:01.00,Default,,0,0,0,,Invalid duration",
    )
    .unwrap();
    assert_eq!(e.duration_ms(), 0);
}
