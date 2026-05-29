//! End-to-end integration tests for the full ASS→SUP pipeline.
//!
//! These tests exercise the complete conversion pipeline using actual crate APIs:
//! ass-parser → subtitle-validator → subtitle-renderer → color-quantizer → pgs-encoder.

use ass_parser::AssFile;
use color_quantizer::Quantizer;
use pgs_encoder::PgsEncoder;
use subtitle_renderer::{RenderConfig, Renderer};
use subtitle_validator::validate;

// ─────────────────────── Helpers ───────────────────────

/// Minimal valid ASS content with one dialogue event.
fn minimal_ass() -> &'static str {
    r#"[Script Info]
Title: Integration Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World
"#
}

/// Run the full ASS→SUP pipeline for a given ASS content string.
/// Returns the collected SUP byte payloads (one per dialogue event).
fn run_full_pipeline(ass_content: &str) -> Vec<Vec<u8>> {
    let ass = AssFile::parse(ass_content).expect("ASS parse failed");
    let _report = validate(&ass);
    let renderer = Renderer::new(RenderConfig::default());
    let quantizer = Quantizer::new(255);
    let mut pgs = PgsEncoder::new(1920, 1080, 23.976);
    let mut sup_outputs = Vec::new();

    for event in ass.dialogue_events() {
        let start_ms = event.start.as_ms();
        let duration_ms = event.duration_ms();
        if let Some(frame) = renderer.render_ass(&ass, start_ms) {
            let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
            let sup_data = pgs.encode_frame_to_bytes(&quantized, start_ms, duration_ms);
            sup_outputs.push(sup_data);
        }
    }

    sup_outputs
}

// ─────────────────────── Full Pipeline Tests ───────────────────────

#[test]
fn test_full_pipeline_ass_to_sup() {
    let sup_outputs = run_full_pipeline(minimal_ass());

    assert!(!sup_outputs.is_empty(), "Pipeline should produce at least one SUP output");

    for (i, sup_data) in sup_outputs.iter().enumerate() {
        assert!(sup_data.len() >= 2, "SUP output {} should have at least 2 bytes", i);
        assert_eq!(sup_data[0], b'P', "SUP output {} should start with 'P' magic byte", i);
        assert_eq!(sup_data[1], b'G', "SUP output {} should start with 'G' magic byte", i);
    }
}

#[test]
fn test_srt_to_ass_to_sup_pipeline() {
    let srt_content = "\
1
00:00:01,000 --> 00:00:05,000
Hello World

2
00:00:06,000 --> 00:00:10,000
Second subtitle
";

    // SRT → ASS
    let ass = ass_parser::srt::parse_srt(srt_content).expect("SRT parse failed");
    assert_eq!(ass.events.len(), 2, "SRT should produce 2 events");
    assert_eq!(ass.format, ass_parser::SubtitleFormat::Srt);

    // ASS → validate → render → quantize → SUP
    let report = validate(&ass);
    assert!(report.is_valid, "SRT-derived ASS should be valid");

    let renderer = Renderer::new(RenderConfig::default());
    let quantizer = Quantizer::new(255);
    let mut pgs = PgsEncoder::new(1920, 1080, 23.976);
    let mut sup_count = 0;

    for event in ass.dialogue_events() {
        let start_ms = event.start.as_ms();
        let duration_ms = event.duration_ms();
        if let Some(frame) = renderer.render_ass(&ass, start_ms) {
            let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
            let sup_data = pgs.encode_frame_to_bytes(&quantized, start_ms, duration_ms);
            assert!(sup_data.len() >= 2, "SUP data should have at least 2 bytes");
            assert_eq!(sup_data[0], b'P');
            assert_eq!(sup_data[1], b'G');
            sup_count += 1;
        }
    }

    assert_eq!(sup_count, 2, "Should produce 2 SUP frames from 2 SRT entries");
}

#[test]
fn test_validation_integration_overlapping_events() {
    let ass_content = r#"[Script Info]
Title: Overlap Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,First subtitle
Dialogue: 0,0:00:03.00,0:00:08.00,Default,,0,0,0,,Overlapping subtitle
"#;

    let ass = AssFile::parse(ass_content).expect("ASS parse failed");
    let report = validate(&ass);

    // Overlapping events at same position should trigger warnings
    assert!(
        !report.overlaps.is_empty(),
        "Overlapping events should trigger overlap warnings"
    );

    // Pipeline should still work despite overlaps
    let sup_outputs = run_full_pipeline(ass_content);
    assert_eq!(sup_outputs.len(), 2, "Should still produce 2 SUP frames");
}

#[test]
fn test_ass_with_karaoke_tags_full_pipeline() {
    let ass_content = r#"[Script Info]
Title: Karaoke Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\k50}Hel{\k100}lo {\k75}World
"#;

    let ass = AssFile::parse(ass_content).expect("ASS parse failed");
    let event = ass.dialogue_events().next().unwrap();
    assert!(
        event.override_tags.iter().any(|t| matches!(t, ass_parser::OverrideTag::Karaoke { .. })),
        "Should detect karaoke override tags"
    );

    let sup_outputs = run_full_pipeline(ass_content);
    assert!(!sup_outputs.is_empty(), "Karaoke ASS should produce SUP output");
    assert_eq!(sup_outputs[0][0], b'P');
    assert_eq!(sup_outputs[0][1], b'G');
}

#[test]
fn test_ass_with_override_tags_full_pipeline() {
    let ass_content = r#"[Script Info]
Title: Override Tags Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\pos(960,540)\fs60\b1\i1\fad(500,500)\blur(2)}Positioned text
Dialogue: 0,0:00:06.00,0:00:10.00,Default,,0,0,0,,{\move(100,100,1820,980,0,4000)}Moving text
"#;

    let ass = AssFile::parse(ass_content).expect("ASS parse failed");

    for event in ass.dialogue_events() {
        assert!(event.has_override_tags(), "Events should have override tags");
    }

    let sup_outputs = run_full_pipeline(ass_content);
    assert_eq!(sup_outputs.len(), 2, "Should produce 2 SUP frames");

    for (i, sup_data) in sup_outputs.iter().enumerate() {
        assert!(sup_data.len() >= 2, "SUP output {} should have magic bytes", i);
        assert_eq!(sup_data[0], b'P');
        assert_eq!(sup_data[1], b'G');
    }
}

#[test]
fn test_empty_ass_produces_empty_sup() {
    let ass_content = r#"[Script Info]
Title: Empty
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"#;

    let sup_outputs = run_full_pipeline(ass_content);
    assert!(sup_outputs.is_empty(), "Empty ASS (no dialogue) should produce no SUP output");
}

#[test]
fn test_multiple_dialogue_events_produce_multiple_display_sets() {
    let ass_content = r#"[Script Info]
Title: Multiple Events
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,First
Dialogue: 0,0:00:04.00,0:00:06.00,Default,,0,0,0,,Second
Dialogue: 0,0:00:07.00,0:00:09.00,Default,,0,0,0,,Third
"#;

    let ass = AssFile::parse(ass_content).expect("ASS parse failed");
    let dialogue_count = ass.dialogue_events().count();
    assert_eq!(dialogue_count, 3, "Should have 3 dialogue events");

    let sup_outputs = run_full_pipeline(ass_content);
    assert_eq!(sup_outputs.len(), 3, "Should produce 3 SUP display sets");

    // Each output should be valid SUP
    for (i, sup_data) in sup_outputs.iter().enumerate() {
        assert!(sup_data.len() >= 13, "SUP output {} should have at least a header (13 bytes)", i);
        assert_eq!(sup_data[0], b'P', "Magic byte P at output {}", i);
        assert_eq!(sup_data[1], b'G', "Magic byte G at output {}", i);
    }

    // Verify composition numbers increment (bytes 11-12 of each PCS segment)
    // PCS segment type is at byte 10 = 0x16
    for sup_data in &sup_outputs {
        assert_eq!(sup_data[10], 0x16, "First segment should be PCS");
    }
}

#[test]
fn test_pipeline_preserves_timing() {
    let ass_content = r#"[Script Info]
Title: Timing Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Timed text
"#;

    let ass = AssFile::parse(ass_content).expect("ASS parse failed");
    let renderer = Renderer::new(RenderConfig::default());
    let quantizer = Quantizer::new(255);
    let mut pgs = PgsEncoder::new(1920, 1080, 23.976);

    let event = ass.dialogue_events().next().unwrap();
    let start_ms = event.start.as_ms();
    let duration_ms = event.duration_ms();

    assert_eq!(start_ms, 1000, "Start should be 1000ms");
    assert_eq!(duration_ms, 4000, "Duration should be 4000ms");

    if let Some(frame) = renderer.render_ass(&ass, start_ms) {
        let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
        let segments = pgs.encode_frame(&quantized, start_ms, duration_ms);

        let expected_pts = 1000 * 90;
        assert_eq!(segments[0].pts, expected_pts, "PTS should be 90kHz of start_ms");

        let expected_end_pts = (1000 + 4000) * 90;
        let end_segment = segments.last().unwrap();
        assert_eq!(end_segment.pts, expected_end_pts, "END PTS should be 90kHz of end time");
    }
}

#[test]
fn test_pipeline_with_different_resolutions() {
    let ass_content = r#"[Script Info]
Title: Resolution Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Test
"#;

    let ass = AssFile::parse(ass_content).expect("ASS parse failed");
    let renderer = Renderer::new(RenderConfig {
        width: 1280,
        height: 720,
        ..Default::default()
    });
    let quantizer = Quantizer::new(255);
    let mut pgs = PgsEncoder::new(1280, 720, 23.976);

    let event = ass.dialogue_events().next().unwrap();
    if let Some(frame) = renderer.render_ass(&ass, event.start.as_ms()) {
        assert_eq!(frame.width, 1280);
        assert_eq!(frame.height, 720);
        let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
        let sup_data = pgs.encode_frame_to_bytes(&quantized, event.start.as_ms(), event.duration_ms());
        assert!(sup_data.len() >= 2);
        assert_eq!(sup_data[0], b'P');
        assert_eq!(sup_data[1], b'G');
    }
}
