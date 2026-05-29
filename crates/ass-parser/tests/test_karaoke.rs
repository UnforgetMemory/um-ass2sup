use ass_parser::karaoke::{KaraokeSegment, KaraokeStyle};

#[test]
fn test_karaoke_style_from_tag() {
    assert!(matches!(KaraokeStyle::from_tag("k"), Some(KaraokeStyle::Instant)));
    assert!(matches!(KaraokeStyle::from_tag("kf"), Some(KaraokeStyle::Fill)));
    assert!(matches!(KaraokeStyle::from_tag("ko"), Some(KaraokeStyle::Outline)));
    assert!(matches!(KaraokeStyle::from_tag("kt"), Some(KaraokeStyle::Timing)));
}

#[test]
fn test_karaoke_style_tag_name() {
    assert_eq!(KaraokeStyle::Instant.tag_name(), "k");
    assert_eq!(KaraokeStyle::Fill.tag_name(), "kf");
    assert_eq!(KaraokeStyle::Outline.tag_name(), "ko");
    assert_eq!(KaraokeStyle::Timing.tag_name(), "kt");
}

#[test]
fn test_karaoke_style_invalid_tag() {
    assert!(KaraokeStyle::from_tag("invalid").is_none());
    assert!(KaraokeStyle::from_tag("K").is_none());
    assert!(KaraokeStyle::from_tag("").is_none());
}

#[test]
fn test_karaoke_segment() {
    let seg = KaraokeSegment::new(KaraokeStyle::Fill, 500, "Hello".to_string(), 0);
    assert_eq!(seg.duration_ms, 500);
    assert_eq!(seg.text, "Hello");
    assert_eq!(seg.index, 0);
    assert!(matches!(seg.style, KaraokeStyle::Fill));
}

#[test]
fn test_karaoke_end_time() {
    let seg = KaraokeSegment::new(KaraokeStyle::Instant, 1000, "世界".to_string(), 1);
    assert_eq!(seg.end_time(5000), 6000); // start=5000 + duration=1000
}

#[test]
fn test_karaoke_end_time_zero_duration() {
    let seg = KaraokeSegment::new(KaraokeStyle::Instant, 0, "".to_string(), 0);
    assert_eq!(seg.end_time(1000), 1000);
}
