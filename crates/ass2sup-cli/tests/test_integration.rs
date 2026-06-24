use ass_core::SubtitleDocument;
use std::path::Path;

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
";

#[test]
fn test_parse_simple_ass() {
    let doc = SubtitleDocument::parse(SIMPLE_ASS).unwrap();
    assert_eq!(doc.events.len(), 1);
    assert_eq!(doc.styles.len(), 1);
    assert_eq!(doc.metadata.play_res_x, 1920);
}

#[test]
fn test_parse_srt() {
    let srt = "1\n00:00:01,000 --> 00:00:05,000\nHello World\n";
    let doc = ass_core::srt::parse_srt(srt).unwrap();
    assert_eq!(doc.events.len(), 1);
}

#[test]
fn test_format_from_extension() {
    let ext = Path::new("test.srt").extension().and_then(|e| e.to_str());
    assert_eq!(ext, Some("srt"));

    let ass_ext = Path::new("test.ass").extension().and_then(|e| e.to_str());
    assert_eq!(ass_ext, Some("ass"));

    let ssa_ext = Path::new("test.ssa").extension().and_then(|e| e.to_str());
    assert_eq!(ssa_ext, Some("ssa"));
}

#[test]
fn test_parse_empty_content_produces_empty_doc() {
    let result = SubtitleDocument::parse("");
    assert!(result.is_ok());
    let doc = result.unwrap();
    assert!(doc.events.is_empty());
}

#[test]
fn test_parse_srt_empty_content() {
    let doc = ass_core::srt::parse_srt("").unwrap();
    assert_eq!(doc.events.len(), 0);
}
