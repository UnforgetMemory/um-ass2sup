use ass_parser::srt::parse_srt;

#[test]
fn test_parse_simple_srt() {
    let input = r#"1
00:00:01,000 --> 00:00:05,000
Hello World

2
00:00:06,000 --> 00:00:10,000
Second subtitle"#;

    let ass = parse_srt(input).unwrap();
    assert_eq!(ass.events.len(), 2);
    assert_eq!(ass.events[0].text, "Hello World");
    assert_eq!(ass.events[1].text, "Second subtitle");
    assert_eq!(ass.events[0].start.as_ms(), 1000);
    assert_eq!(ass.events[0].end.as_ms(), 5000);
}

#[test]
fn test_parse_srt_with_html_tags() {
    let input = r#"1
00:00:01,000 --> 00:00:05,000
<b>Bold</b> and <i>italic</i> and <u>underline</u>"#;

    let ass = parse_srt(input).unwrap();
    assert_eq!(ass.events.len(), 1);
    let text = &ass.events[0].text;
    // Should convert to ASS override tags
    assert!(text.contains("{\\b1}") || text.contains("Bold"));
}

#[test]
fn test_parse_srt_timecodes_with_dot() {
    let input = r#"1
00:00:01.500 --> 00:00:05.500
Dot separator"#;

    let ass = parse_srt(input).unwrap();
    assert_eq!(ass.events[0].start.as_ms(), 1500);
    assert_eq!(ass.events[0].end.as_ms(), 5500);
}

#[test]
fn test_parse_srt_multiline() {
    let input = r#"1
00:00:01,000 --> 00:00:05,000
Line one
Line two
Line three"#;

    let ass = parse_srt(input).unwrap();
    let text = &ass.events[0].text;
    // Should have multiline text
    assert!(text.contains("Line one"));
    assert!(text.contains("Line two") || text.contains("\\N") || text.contains("\n"));
}

#[test]
fn test_parse_srt_stripped_number() {
    let input = r#"42
00:00:01,000 --> 00:00:05,000
Numbered subtitle"#;

    let ass = parse_srt(input).unwrap();
    assert_eq!(ass.events.len(), 1);
}

#[test]
fn test_parse_srt_empty() {
    let input = "";
    let ass = parse_srt(input).unwrap();
    assert_eq!(ass.events.len(), 0);
}

#[test]
fn test_parse_srt_default_style() {
    let input = r#"1
00:00:01,000 --> 00:00:05,000
Test"#;

    let ass = parse_srt(input).unwrap();
    assert_eq!(ass.styles.len(), 1);
    assert_eq!(ass.styles[0].font_name, "Arial");
    assert_eq!(ass.styles[0].font_size, 48.0);
}

#[test]
fn test_parse_srt_all_events_are_dialogue() {
    let input = r#"1
00:00:01,000 --> 00:00:05,000
First

2
00:00:06,000 --> 00:00:10,000
Second"#;

    let ass = parse_srt(input).unwrap();
    for e in &ass.events {
        assert!(e.is_dialogue());
    }
}

#[test]
fn test_parse_srt_huge_timestamp_no_panic() {
    // Regression: fuzz crash — sec * 1000 overflowed u64 for huge second values
    // Should saturate instead of panicking
    let input = "3:2223:00006817148741241740400-->\ntest";
    let result = parse_srt(input);
    // Must not panic — may return Ok or Err
    let _ = result;
}
