use ass_core::SubtitleDocument;

const SIMPLE_ASS: &str = "[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World
Dialogue: 0,0:00:06.00,0:00:10.00,Default,,0,0,0,,Second Line
";

#[test]
fn test_parse_two_events() {
    let doc = SubtitleDocument::parse(SIMPLE_ASS).unwrap();
    assert_eq!(doc.events.len(), 2);
}

#[test]
fn test_event_access() {
    let doc = SubtitleDocument::parse(SIMPLE_ASS).unwrap();
    let first = &doc.events[0];
    assert_eq!(first.start_ms, 1000);
    assert_eq!(first.end_ms, 5000);
    assert_eq!(first.style.as_str(), "Default");
    assert_eq!(first.text_raw, "Hello World");

    let second = &doc.events[1];
    assert_eq!(second.start_ms, 6000);
    assert_eq!(second.end_ms, 10000);
    assert_eq!(second.text_raw, "Second Line");
}

#[test]
fn test_roundtrip_srt() {
    let srt = "1\n00:00:01,000 --> 00:00:05,000\nHello World\n\n2\n00:00:06,000 --> 00:00:10,000\nSecond Line\n";
    let doc = ass_core::srt::parse_srt(srt).unwrap();
    assert_eq!(doc.events.len(), 2);

    // Convert back to SRT
    let output = ass_core::srt::to_srt(&doc);
    assert!(output.contains("Hello World"));
    assert!(output.contains("Second Line"));
    assert!(output.contains("00:00:01,000"));
}
