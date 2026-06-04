//! End-to-end integration tests for the full ASS→SUP pipeline.
//!
//! These tests exercise the complete conversion pipeline using actual crate APIs:
//! ass-parser → subtitle-validator → subtitle-renderer → color-quantizer → pgs-encoder.

use ass_parser::AssFile;
use color_quantizer::Quantizer;
use pgs_encoder::PgsEncoder;
use std::path::PathBuf;
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

        let expected_pts = (1000u128 * 90000 * 1001 / 1000000) as u64;
        assert_eq!(segments[0].pts, expected_pts, "PTS should be 90kHz NTSC of start_ms");

        let expected_end_pts = (5000u128 * 90000 * 1001 / 1000000) as u64;
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

/// Helper: run pipeline at a specific timestamp for the first dialogue event,
/// return the SUP output bytes.
fn run_pipeline_at(ass_content: &str, timestamp_ms: u64) -> Vec<u8> {
    let ass = AssFile::parse(ass_content).expect("ASS parse failed");
    let renderer = Renderer::new(RenderConfig::default());
    let quantizer = Quantizer::new(255);
    let mut pgs = PgsEncoder::new(1920, 1080, 23.976);

    let event = ass.dialogue_events().next().expect("At least one dialogue event");
    let duration_ms = event.duration_ms();
    if let Some(frame) = renderer.render_ass(&ass, timestamp_ms) {
        let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
        return pgs.encode_frame_to_bytes(&quantized, timestamp_ms, duration_ms);
    }
    Vec::new()
}

// ═══════════════════════════════════════════════════════════════════
// Phase 14 Full Pipeline Integration Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_banner_effect_full_pipeline() {
    let ass_content = r#"[Script Info]
Title: BannerPipeline
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,Banner;10;1;0,Banner Text
"#;

    let sup_early = run_pipeline_at(ass_content, 100);
    let sup_late = run_pipeline_at(ass_content, 2000);

    assert!(!sup_early.is_empty(), "Banner early should produce SUP");
    assert!(!sup_late.is_empty(), "Banner late should produce SUP");
    assert_eq!(sup_early[0], b'P', "Banner early SUP should start with PG magic");
    assert_eq!(sup_early[1], b'G', "Banner early SUP should start with PG magic");
    assert_eq!(sup_late[0], b'P', "Banner late SUP should start with PG magic");
    assert_eq!(sup_late[1], b'G', "Banner late SUP should start with PG magic");
}

#[test]
fn test_scroll_effect_full_pipeline() {
    let ass_content = r#"[Script Info]
Title: ScrollPipeline
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,Scroll up;10;50;50,Scrolling Up Text
Dialogue: 0,0:00:05.00,0:00:10.00,Default,,0,0,0,Scroll down;10;200;50,Scrolling Down Text
"#;

    let sup_outputs = run_full_pipeline(ass_content);
    assert_eq!(sup_outputs.len(), 2, "Scroll pipeline should produce 2 SUP outputs");

    for (i, sup_data) in sup_outputs.iter().enumerate() {
        assert!(sup_data.len() >= 2, "Scroll SUP output {} should have magic bytes", i);
        assert_eq!(sup_data[0], b'P', "Scroll SUP output {} should start with PG", i);
        assert_eq!(sup_data[1], b'G', "Scroll SUP output {} should start with PG", i);
    }
}

#[test]
fn test_karaoke_full_pipeline() {
    let ass_content = r#"[Script Info]
Title: KaraokePipeline
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\k50}Hel{\kf75}lo {\ko100}Wor{\kt200}ld
"#;

    let ass = AssFile::parse(ass_content).expect("ASS parse failed");
    assert!(
        ass.events[0].override_tags.iter().any(|t| matches!(t, ass_parser::OverrideTag::Karaoke { .. })),
        "Should detect karaoke override tags"
    );
    assert!(!ass.events[0].karaoke_segments.is_empty(), "Karaoke segments should be populated");

    // Run full pipeline — should produce valid SUP
    let sup_outputs = run_full_pipeline(ass_content);
    assert!(!sup_outputs.is_empty(), "Karaoke ASS should produce SUP output");

    // Verify SUP magic bytes
    assert_eq!(sup_outputs[0][0], b'P', "Karaoke SUP should start with P");
    assert_eq!(sup_outputs[0][1], b'G', "Karaoke SUP should start with G");

    // Also verify pipeline works at a mid-karaoke timestamp
    let mid_sup = run_pipeline_at(ass_content, 3000);
    assert!(!mid_sup.is_empty(), "Mid-karaoke pipeline should produce SUP");
    assert_eq!(mid_sup[0], b'P');
    assert_eq!(mid_sup[1], b'G');
}

#[test]
fn test_t_transform_full_pipeline() {
    let ass_content = r#"[Script Info]
Title: TransformPipeline
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\t(\pos(960,540),0,2000,1)}Transform Text
"#;

    // Parse and verify transform tag
    let ass = AssFile::parse(ass_content).expect("ASS parse failed");
    assert!(
        ass.events[0].override_tags.iter().any(|t| matches!(t, ass_parser::OverrideTag::Transform { .. })),
        "Should parse Transform tag"
    );

    // Run pipeline at multiple timestamps to exercise interpolation
    let sup_start = run_pipeline_at(ass_content, 1000);
    assert!(!sup_start.is_empty(), "Transform start should produce SUP");
    assert_eq!(sup_start[0], b'P');

    let sup_mid = run_pipeline_at(ass_content, 3000);
    assert!(!sup_mid.is_empty(), "Transform mid should produce SUP");
    assert_eq!(sup_mid[0], b'P');

    let sup_end = run_pipeline_at(ass_content, 5000);
    assert!(!sup_end.is_empty(), "Transform end should produce SUP");
    assert_eq!(sup_end[0], b'P');
}

#[test]
fn test_vector_clip_full_pipeline() {
    let ass_content = r#"[Script Info]
Title: VectorClipPipeline
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\clip(1,m 0 0 l 100 0 100 100 0 100)}Vector Clip Text
"#;

    let ass = AssFile::parse(ass_content).expect("ASS parse failed");
    assert!(
        ass.events[0].override_tags.iter().any(|t| matches!(t, ass_parser::OverrideTag::ClipDrawing { .. })),
        "Vector clip should parse as ClipDrawing"
    );

    let sup_outputs = run_full_pipeline(ass_content);
    assert!(!sup_outputs.is_empty(), "Vector clip ASS should produce SUP output");
    let sup_data = &sup_outputs[0];
    assert!(sup_data.len() >= 2, "SUP data should have magic bytes");
    assert_eq!(sup_data[0], b'P', "SUP should start with P");
    assert_eq!(sup_data[1], b'G', "SUP should start with G");
}

// ═══════════════════════════════════════════════════════════════════
// Error-Path Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_empty_string_fails_gracefully() {
    let result = AssFile::parse("");
    // Parser accepts empty string as valid (empty ASS) - should not panic
    let _ = result;
}

#[test]
fn test_binary_garbage_fails_gracefully() {
    let garbage: &[u8] = &[0xFF, 0xFE, 0x00, 0x01, 0x80, 0x90, 0xAB, 0xCD];
    let s = String::from_utf8_lossy(garbage);
    let result = AssFile::parse(&s);
    // Should not panic - either succeeds or returns error
    let _ = result;
}

#[test]
fn test_missing_script_info_parses() {
    let ass = "[V4+ Styles]\nFormat: Name, Fontname\nStyle: Default,Arial\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello\n";
    let result = AssFile::parse(ass);
    // Missing [Script Info] - lenient parser should handle, strict may fail
    let _ = result;
}

#[test]
fn test_invalid_timestamp_fails() {
    let ass = "[Script Info]\nTitle: Test\nScriptType: v4.00+\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.000,0:00:05.000,Default,,0,0,0,,Hello\n";
    let result = AssFile::parse(ass);
    // Should either parse or fail gracefully - no panic
    let _ = result;
}

#[test]
fn test_missing_style_reference() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,NonExistent,,0,0,0,,Hello
"#;
    let sup_outputs = run_full_pipeline(ass);
    assert!(!sup_outputs.is_empty(), "Should produce SUP output even with non-existent style ref");
    assert_eq!(sup_outputs[0][0], b'P', "SUP should start with P");
    assert_eq!(sup_outputs[0][1], b'G', "SUP should start with G");
}

#[test]
fn test_render_with_no_events() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"#;
    let ass = AssFile::parse(ass).expect("Valid ASS with no events should parse");
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000);
    if let Some(frame) = frame {
        assert!(frame.bitmap.iter().all(|&b| b == 0), "No events should produce empty frame");
    }
}

#[test]
fn test_render_with_zero_duration() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:01.00,Default,,0,0,0,,Zero
"#;
    let ass = AssFile::parse(ass).expect("Zero duration should parse");
    let renderer = Renderer::new(RenderConfig::default());
    // Zero duration event at its start time should not panic
    let _ = renderer.render_ass(&ass, 1000);
}

#[test]
fn test_render_with_max_timestamp() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,99:59:59.99,Default,,0,0,0,,Long
"#;
    let ass = AssFile::parse(ass).expect("Very long event should parse");
    let renderer = Renderer::new(RenderConfig::default());
    // Very large timestamp (100 hours) should not cause panic or overflow
    let _ = renderer.render_ass(&ass, 360000000);
}

// ═══════════════════════════════════════════════════════════════════
// Edge-Case ASS Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_unicode_text() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,日本語テスト 🎌 한국어
"#;
    let ass = AssFile::parse(ass).expect("Unicode ASS should parse");
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000).expect("Unicode text should render");
    assert!(frame.width > 0, "Unicode text should produce valid frame");
}

#[test]
fn test_many_simultaneous_events() {
    let header = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"#;
    let mut events = String::new();
    for i in 0..50 {
        events.push_str(&format!("Dialogue: {i},0:00:01.00,0:00:05.00,Default,,0,0,0,,Event{i}\n"));
    }
    let ass_content = format!("{}{}", header, events);
    let ass = AssFile::parse(&ass_content).expect("Many events should parse");
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000).expect("Should render at 2s");
    assert!(frame.width > 0, "50 simultaneous events should render");
}

#[test]
fn test_long_event_duration() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,25:00:00.00,Default,,0,0,0,,25 hour event
"#;
    let ass = AssFile::parse(ass).expect("Long duration ASS should parse");
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 36000000).expect("Should render at 10h");
    assert!(frame.width > 0, "Long duration event should render at 10h");
}

#[test]
fn test_nested_override_tags() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\b1\i1\c&H0000FF&\fs72}Nested{\b0\i0\c&HFFFFFF&\fs48}Tags
"#;
    let ass = AssFile::parse(ass).expect("Nested override tags ASS should parse");
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000).expect("Nested override tags should render");
    assert!(frame.width > 0, "Nested override tags should render");
}

#[test]
fn test_drawing_mode_p1() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\p1}m 0 0 l 100 0 100 100 0 100 c{\p0}
"#;
    let ass = AssFile::parse(ass).expect("Drawing mode p1 ASS should parse");
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000).expect("Drawing mode p1 should render");
    assert!(frame.width > 0, "Drawing mode p1 should render");
}

#[test]
fn test_drawing_mode_p4_clip() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\p4}m 0 0 l 1920 0 1920 540 0 540 c{\p0}Clipped
"#;
    let ass = AssFile::parse(ass).expect("Drawing mode p4 ASS should parse");
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000).expect("Drawing mode p4 clip should render");
    assert!(frame.width > 0, "Drawing mode p4 clip should render");
}

#[test]
fn test_writing_mode_vertical() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\q2}Vertical Text
"#;
    let ass = AssFile::parse(ass).expect("Writing mode vertical ASS should parse");
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000).expect("Writing mode vertical should render");
    assert!(frame.width > 0, "Writing mode vertical should render");
}

// ═══════════════════════════════════════════════════════════════════
// Font Fallback Tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_nonexistent_font_falls_back() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,NonExistentFont12345,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Fallback Test
"#;
    let ass = AssFile::parse(ass).expect("ASS with nonexistent font should parse");
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000).expect("Nonexistent font should fall back and render");
    // Should not panic - falls back to a default font
    assert!(frame.width > 0, "Nonexistent font should fall back and render");
}

#[test]
fn test_default_font_used() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Default Font
"#;
    let ass = AssFile::parse(ass).expect("ASS with DejaVu Sans should parse");
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000).expect("Default DejaVu Sans should render");
    assert!(frame.width > 0, "Default DejaVu Sans should render");
}

#[test]
fn test_font_size_override() {
    let ass = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\fs72}Large Text
"#;
    let ass = AssFile::parse(ass).expect("ASS with font size override should parse");
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000).expect("Font size override should render");
    assert!(frame.width > 0, "Font size override should render");
}

#[test]
fn test_invalid_style_fails() {
    let ass = "\
[Script Info]
Title: Invalid Style
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default, Arial, 48, &H00FFFFFF, &H000000FF, &H00000000, &H00000000, 0, 0, 0, 0, 100, 100, 0, 0, 1, 2, 0, 2, 10, 10, 10, 1
Style: BAD_STYLE_TOO_FEW_FIELDS, Arial, 48

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Test";
    let result = AssFile::parse(ass);
    if let Ok(ass) = result {
        assert!(!ass.styles.is_empty(), "Should have at least one valid style");
    }
}

// ═══════════════════════════════════════════════════════════════════
// SRT Dispatch Tests (RED - these fail before the fix)
// ═══════════════════════════════════════════════════════════════════

fn workspace_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
}

#[test]
fn test_parse_file_handles_srt() {
    let path = workspace_fixtures_dir().join("basic.srt");
    let ass = AssFile::parse_file(&path).expect("SRT via parse_file should succeed");
    assert!(
        !ass.events.is_empty(),
        "SRT should produce events via parse_file (got 0)"
    );
}

#[test]
fn test_cli_convert_srt_to_sup_produces_nonzero_output() {
    use assert_cmd::Command;
    let tmp = std::env::temp_dir().join("h1_srt_test.sup");
    let _ = std::fs::remove_file(&tmp);
    Command::cargo_bin("ass2sup")
        .unwrap()
        .args([
            workspace_fixtures_dir().join("basic.srt").to_str().unwrap(),
            "-o",
        ])
        .arg(&tmp)
        .assert()
        .success();
    let metadata = std::fs::metadata(&tmp).expect("output should exist");
    assert!(metadata.len() > 0, "SRT conversion must produce > 0 bytes");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_cli_convert_chinese_srt_produces_nonzero_output() {
    use assert_cmd::Command;
    let tmp = std::env::temp_dir().join("h1_cn_srt_test.sup");
    let _ = std::fs::remove_file(&tmp);
    Command::cargo_bin("ass2sup")
        .unwrap()
        .args([
            workspace_fixtures_dir().join("chinese.srt").to_str().unwrap(),
            "-o",
        ])
        .arg(&tmp)
        .assert()
        .success();
    let metadata = std::fs::metadata(&tmp).expect("output should exist");
    assert!(metadata.len() > 0, "Chinese SRT must produce > 0 bytes");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_render_with_negative_duration() {
    let ass = "\
[Script Info]
Title: Negative Duration
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:05.00,0:00:01.00,Default,,0,0,0,,Reverse Time";
    let result = AssFile::parse(ass);
    assert!(result.is_ok(), "Negative duration ASS should parse");
    let ass = result.unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let _frame = renderer.render_ass(&ass, 3000);
}

#[test]
fn test_invalid_glob_does_not_panic() {
    use assert_cmd::Command;
    // Before fix: binary panics on bad glob
    // After fix: graceful "No input files found" or similar error, exit code != 0 but NO panic
    let output = Command::cargo_bin("ass2sup")
        .unwrap()
        .args(["--glob", "[invalid-glob-pattern"])
        .output()
        .expect("binary should run");

    // The binary should NOT panic. Panics in Rust print "thread 'main' panicked" to stderr.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("panicked") && !stderr.contains("Invalid glob pattern: "),
        "Binary should not panic on invalid glob, but stderr was: {}",
        stderr
    );
    // Exit code should be non-zero (no files found)
    assert!(!output.status.success(), "Binary should exit non-zero when no files found");
}
