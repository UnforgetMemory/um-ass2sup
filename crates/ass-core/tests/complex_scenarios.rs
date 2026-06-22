//! Complex multi-subtitle multi-effect integration tests for ass-core.
//!
//! These tests exercise the parser with realistic ASS content:
//! - Multiple simultaneous events across layers
//! - Overlapping time regions with various tag combinations
//! - Combined move + clip + fad effects
//! - Transform \t() animations
//! - Karaoke events (\k, \kf) running concurrently
//! - Banner effect mixed with override tags
//! - Mixed SSA v4 / V4+ style parsing
//! - Large 100+ event stress test

use ass_core::{Effect, EventType, KaraokeStyle, OverrideTag, SubtitleDocument};

// ---------------------------------------------------------------------------
// Helper: minimal ASS preamble with V4+ styles
// ---------------------------------------------------------------------------

fn default_styles() -> &'static str {
    "[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
Style: Alt,DejaVu Sans,36,&H00FFFF00,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
Style: KaraokeStyle,Arial,40,&H00FFFFFF,&H0000FFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
Style: BannerStyle,Arial,32,&H00FF8800,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
"
}

fn make_ass(extra_sections: &str) -> String {
    format!(
        "[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080
{}
[Events]
Format: Layer, Start, End, Style, Actor, MarginL, MarginR, MarginV, Effect, Text
{}",
        default_styles(),
        extra_sections
    )
}

fn parse_events(events_text: &str) -> SubtitleDocument {
    let content = make_ass(events_text);
    let (doc, errors) = SubtitleDocument::parse_with_recovery(&content);
    assert!(errors.is_empty(), "Parse errors: {errors:?}");
    doc
}

fn parse_doc(content: &str) -> SubtitleDocument {
    let (doc, errors) = SubtitleDocument::parse_with_recovery(content);
    assert!(errors.is_empty(), "Parse errors: {errors:?}");
    doc
}

// ---------------------------------------------------------------------------
// 1) Multiple simultaneous events on different layers with \pos and \fad
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_simultaneous_layers_with_pos_fad() {
    // Three events all active at the same time (0.5s — 3.5s) on layers 0, 1, 2
    // Each has \pos and \fad
    let events = "\
Dialogue: 0,0:00:00.50,0:00:03.50,Default,,0,0,0,,{\\pos(100,200)}{\\fad(100,200)}Hello from layer 0
Dialogue: 1,0:00:00.50,0:00:03.50,Alt,,0,0,0,,{\\pos(300,400)}{\\fad(150,250)}Layer 1 subtitle
Dialogue: 2,0:00:00.50,0:00:03.50,Default,,0,0,0,,{\\pos(500,600)}{\\fad(200,300)}Layer 2 here
";
    let doc = parse_events(events);

    assert_eq!(doc.events.len(), 3, "expected 3 events");

    // Layer values
    assert_eq!(doc.events[0].layer, 0);
    assert_eq!(doc.events[1].layer, 1);
    assert_eq!(doc.events[2].layer, 2);

    // All have same timing
    for ev in &doc.events {
        assert_eq!(ev.start_ms, 500, "start_ms should be 500");
        assert_eq!(ev.end_ms, 3500, "end_ms should be 3500");
    }

    // Verify \pos tags: each event should have a Pos override tag
    let check_pos = |ev: &ass_core::Event, expected_x: f64, expected_y: f64| {
        let has_pos = ev.override_tags.iter().any(|to| {
            matches!(to.tag, OverrideTag::Pos { x, y } if (x - expected_x).abs() < 0.001 && (y - expected_y).abs() < 0.001)
        });
        assert!(
            has_pos,
            "Event missing expected Pos({expected_x},{expected_y}) in {ev:?}"
        );
    };
    check_pos(&doc.events[0], 100.0, 200.0);
    check_pos(&doc.events[1], 300.0, 400.0);
    check_pos(&doc.events[2], 500.0, 600.0);

    // Verify \fad tags
    let check_fad = |ev: &ass_core::Event, expected_in: u64, expected_out: u64| {
        let has_fad = ev.override_tags.iter().any(|to| {
            matches!(to.tag, OverrideTag::Fade { duration_in, duration_out }
                if duration_in == expected_in && duration_out == expected_out)
        });
        assert!(
            has_fad,
            "Event missing expected Fade({expected_in},{expected_out})"
        );
    };
    check_fad(&doc.events[0], 100, 200);
    check_fad(&doc.events[1], 150, 250);
    check_fad(&doc.events[2], 200, 300);
}

// ---------------------------------------------------------------------------
// 2) Overlapping time regions
// ---------------------------------------------------------------------------

#[test]
fn test_overlapping_time_regions() {
    // Events with various overlap patterns:
    //   A: 0.0s → 5.0s (long, contains others)
    //   B: 1.0s → 3.0s (fully inside A)
    //   C: 2.0s → 6.0s (overlaps A partially)
    //   D: 7.0s → 8.0s (no overlap with others)
    //   E: 4.0s → 5.0s (exact end matches A's end)
    let events = "\
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,Long event
Dialogue: 0,0:00:01.00,0:00:03.00,Alt,,0,0,0,,Contained
Dialogue: 1,0:00:02.00,0:00:06.00,Default,,0,0,0,,Overlap tail
Dialogue: 0,0:00:07.00,0:00:08.00,Default,,0,0,0,,No overlap
Dialogue: 2,0:00:04.00,0:00:05.00,Default,,0,0,0,,Exact end match
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 5);

    // A: 0–5000
    assert_eq!(doc.events[0].start_ms, 0);
    assert_eq!(doc.events[0].end_ms, 5000);

    // B: 1000–3000 (fully contained in A)
    assert_eq!(doc.events[1].start_ms, 1000);
    assert_eq!(doc.events[1].end_ms, 3000);
    assert!(doc.events[1].start_ms >= doc.events[0].start_ms);
    assert!(doc.events[1].end_ms <= doc.events[0].end_ms);

    // C: 2000–6000 (overlaps A partially)
    assert_eq!(doc.events[2].start_ms, 2000);
    assert_eq!(doc.events[2].end_ms, 6000);
    assert!(doc.events[2].start_ms < doc.events[0].end_ms);
    assert!(doc.events[2].end_ms > doc.events[0].end_ms);

    // D: 7000–8000 (no overlap)
    assert_eq!(doc.events[3].start_ms, 7000);
    assert_eq!(doc.events[3].end_ms, 8000);
    assert!(doc.events[3].start_ms > doc.events[0].end_ms);

    // E: 4000–5000 (exact end match with A)
    assert_eq!(doc.events[4].end_ms, doc.events[0].end_ms);
    assert_eq!(doc.events[4].start_ms, 4000);

    // Verify event types are all Dialogue
    for ev in &doc.events {
        assert_eq!(ev.event_type, EventType::Dialogue);
    }

    // Check styles are preserved
    assert_eq!(doc.events[0].style.as_str(), "Default");
    assert_eq!(doc.events[1].style.as_str(), "Alt");
    assert_eq!(doc.events[2].style.as_str(), "Default");
}

// ---------------------------------------------------------------------------
// 3) Combined \move + \clip + \fad in same event
// ---------------------------------------------------------------------------

#[test]
fn test_combined_move_clip_fad() {
    // One event with \move, \clip, and \fad working together
    let events = "\
Dialogue: 0,0:00:01.00,0:00:04.00,Default,,0,0,0,,{\\move(100,200,500,600,0,3000)}{\\clip(50,60,700,800)}{\\fad(200,300)}Moving and clipped text
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 1);

    let ev = &doc.events[0];
    assert_eq!(ev.start_ms, 1000);
    assert_eq!(ev.end_ms, 4000);
    assert_eq!(ev.layer, 0);

    // Verify \move tag
    let has_move = ev.override_tags.iter().any(|to| {
        matches!(to.tag, OverrideTag::Move { x1, y1, x2, y2, t1, t2 }
            if (x1 - 100.0).abs() < 0.001
            && (y1 - 200.0).abs() < 0.001
            && (x2 - 500.0).abs() < 0.001
            && (y2 - 600.0).abs() < 0.001
            && t1 == 0
            && t2 == 3000)
    });
    assert!(has_move, "Event missing expected Move tag");

    // Verify \clip tag
    let has_clip = ev.override_tags.iter().any(|to| {
        matches!(to.tag, OverrideTag::Clip { x1, y1, x2, y2 }
            if (x1 - 50.0).abs() < 0.001
            && (y1 - 60.0).abs() < 0.001
            && (x2 - 700.0).abs() < 0.001
            && (y2 - 800.0).abs() < 0.001)
    });
    assert!(has_clip, "Event missing expected Clip tag");

    // Verify \fad tag
    let has_fad = ev.override_tags.iter().any(|to| {
        matches!(to.tag, OverrideTag::Fade { duration_in, duration_out }
            if duration_in == 200 && duration_out == 300)
    });
    assert!(has_fad, "Event missing expected Fade tag");

    // Verify raw text preserved exactly
    assert!(ev.text_raw.contains("Moving and clipped text"));

    // Verify there are exactly 3 override tags (move, clip, fad)
    // plus possibly text from non-override content being parsed as tags
    assert_eq!(
        ev.override_tags.len(),
        3,
        "Expected exactly 3 override tags"
    );
}

// ---------------------------------------------------------------------------
// 4) Transform \t() with different properties
// ---------------------------------------------------------------------------

#[test]
fn test_transform_different_properties() {
    // Tests \t() with various animated properties.
    // Libass-compatible behavior:
    // - \t(t1,t2) — 2 args: cnt=1, t1=0, t2=0, accel=parts[0], tag=parts[1]
    // - \t(\tag) — backslash-first: cnt=0, all defaults, tag=whole inner content
    let events = "\
Dialogue: 0,0:00:00.50,0:00:05.00,Default,,0,0,0,,{\\t(1000,2000)}Simple timing
Dialogue: 1,0:00:01.00,0:00:06.00,Alt,,0,0,0,,{\\t(500,1500)}Timing only
Dialogue: 2,0:00:00.00,0:00:04.00,Default,,0,0,0,,{\\t(\\fs72)}Font size animate
Dialogue: 3,0:00:02.00,0:00:07.00,Default,,0,0,0,,{\\t(\\bord5)}Border animate
Dialogue: 4,0:00:03.00,0:00:08.00,Default,,0,0,0,,{\\t(1000,2000,0.5)}Accel no inner tag
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 5);

    // Event 0: \t(1000,2000) — no tag (neither arg starts with \): t1=1000, t2=2000
    {
        let ev = &doc.events[0];
        let t = ev.override_tags.iter().find_map(|to| {
            if let OverrideTag::Transform { tag, t1, t2, .. } = &to.tag {
                Some((tag.as_str(), *t1, *t2))
            } else {
                None
            }
        });
        assert_eq!(t, Some(("", 1000, 2000)));
    }

    // Event 2: \t(\fs72) — backslash-first: tag="\\fs72", t1=0, t2=0
    {
        let ev = &doc.events[2];
        let has_fs = ev.override_tags.iter().any(|to| {
            matches!(&to.tag, OverrideTag::Transform { tag, t1, t2, .. }
                if tag == "\\fs72" && *t1 == 0 && *t2 == 0)
        });
        assert!(has_fs, "Event 2 should have transform with \\fs72");
    }

    // Event 3: \t(\bord5) — backslash-first: tag="\\bord5", t1=0, t2=0
    {
        let ev = &doc.events[3];
        let has_bord = ev.override_tags.iter().any(|to| {
            matches!(&to.tag, OverrideTag::Transform { tag, .. }
                if tag == "\\bord5")
        });
        assert!(has_bord, "Event 3 should have transform with \\bord5");
    }

    // Event 4: \t(1000,2000,0.5) — no tag: t1=1000, t2=2000, accel=0.5
    {
        let ev = &doc.events[4];
        let has_t = ev.override_tags.iter().any(|to| {
            matches!(&to.tag, OverrideTag::Transform { t1, t2, .. }
                if *t1 == 1000 && *t2 == 2000)
        });
        assert!(
            has_t,
            "Event 4 should have a Transform tag with t1=1000 t2=2000"
        );
    }
}

// ---------------------------------------------------------------------------
// 5) Multiple karaoke events with \k and \kf simultaneously
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_karaoke_events_simultaneous() {
    // Two events with karaoke running at the same time on different layers
    let events = "\
Dialogue: 0,0:00:00.00,0:00:04.00,KaraokeStyle,,0,0,0,Karaoke,{\\k50}Wel-{\\k40}come{\\k30} to{\\k60} karaoke!
Dialogue: 1,0:00:00.00,0:00:04.00,KaraokeStyle,,0,0,0,Karaoke,{\\kf30}Si-{\\kf40}mul-{\\kf25}ta-{\\kf35}neous
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 2);

    // Both events should have karaoke segments
    // Event 0: \k tags (Instant style)
    assert_eq!(doc.events[0].effect, Effect::Karaoke);
    let kseg0 = &doc.events[0].karaoke;
    assert!(!kseg0.is_empty(), "Event 0 should have karaoke segments");
    for seg in kseg0 {
        assert_eq!(
            seg.style,
            KaraokeStyle::Instant,
            "Event 0 should have Instant karaoke"
        );
    }
    // First syllable: \k50 → 500ms "Wel-"
    assert_eq!(kseg0[0].duration_ms, 500, "\\k50 should be 500ms");
    assert_eq!(kseg0[0].text, "Wel-");

    // Event 1: \kf tags (Fill style)
    assert_eq!(doc.events[1].effect, Effect::Karaoke);
    let kseg1 = &doc.events[1].karaoke;
    assert!(!kseg1.is_empty(), "Event 1 should have karaoke segments");
    for seg in kseg1 {
        assert_eq!(
            seg.style,
            KaraokeStyle::Fill,
            "Event 1 should have Fill karaoke"
        );
    }
    // First syllable: \kf30 → 300ms "Si-"
    assert_eq!(kseg1[0].duration_ms, 300, "\\kf30 should be 300ms");
    assert_eq!(kseg1[0].text, "Si-");
}

#[test]
fn test_karaoke_ko_kt_styles() {
    // Test \ko and \kt karaoke styles
    let events = "\
Dialogue: 0,0:00:00.50,0:00:03.00,KaraokeStyle,,0,0,0,Karaoke,{\\ko45}Out-{\\ko35}line
Dialogue: 1,0:00:01.00,0:00:04.00,KaraokeStyle,,0,0,0,Karaoke,{\\kt60}Tim-{\\kt20}ing
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 2);

    // Event 0: \ko (Outline style)
    let kseg0 = &doc.events[0].karaoke;
    assert!(!kseg0.is_empty());
    assert_eq!(kseg0[0].style, KaraokeStyle::Outline);
    assert_eq!(kseg0[0].duration_ms, 450);
    assert_eq!(kseg0[0].text, "Out-");

    // Event 1: \kt (Timing style)
    let kseg1 = &doc.events[1].karaoke;
    assert!(!kseg1.is_empty());
    assert_eq!(kseg1[0].style, KaraokeStyle::Timing);
    assert_eq!(kseg1[0].duration_ms, 600);
    assert_eq!(kseg1[0].text, "Tim-");
}

// ---------------------------------------------------------------------------
// 6) Banner effect combined with override tags
// ---------------------------------------------------------------------------

#[test]
fn test_banner_effect_with_override_tags() {
    // Banner effect with various override tags
    let events = "\
Dialogue: 0,0:00:00.00,0:00:05.00,BannerStyle,,0,0,0,Banner;8;1;40,Hello banner world!
Dialogue: 0,0:00:01.00,0:00:06.00,BannerStyle,,0,0,0,Banner;12;0;60,{\\b1}{\\i1}{\\fs40}Styled banner text
Dialogue: 0,0:00:02.00,0:00:07.00,BannerStyle,,0,0,0,Banner;15;0;30,{\\c&H00FF00&}{\\bord3}{\\shad2}Coloured banner
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 3);

    // Event 0: Bare text, no override tags
    assert_eq!(
        doc.events[0].effect,
        Effect::Banner {
            delay: 8,
            left_to_right: true,
            fadeaway: 40,
        }
    );
    assert!(doc.events[0].override_tags.is_empty());
    assert_eq!(doc.events[0].text_raw, "Hello banner world!");

    // Event 1: Banner with bold, italic, and font size overrides
    assert_eq!(
        doc.events[1].effect,
        Effect::Banner {
            delay: 12,
            left_to_right: false,
            fadeaway: 60,
        }
    );
    let has_bold = doc.events[1]
        .override_tags
        .iter()
        .any(|to| matches!(to.tag, OverrideTag::Bold(true)));
    assert!(has_bold, "Event 1 should have Bold(true)");

    let has_italic = doc.events[1]
        .override_tags
        .iter()
        .any(|to| matches!(to.tag, OverrideTag::Italic(true)));
    assert!(has_italic, "Event 1 should have Italic(true)");

    let has_fontsize = doc.events[1]
        .override_tags
        .iter()
        .any(|to| matches!(to.tag, OverrideTag::FontSize(v) if (v - 40.0).abs() < 0.001));
    assert!(has_fontsize, "Event 1 should have FontSize(40)");

    // Event 2: Banner with colour, border, shadow
    assert_eq!(
        doc.events[2].effect,
        Effect::Banner {
            delay: 15,
            left_to_right: false,
            fadeaway: 30,
        }
    );
    let has_green = doc.events[2].override_tags.iter().any(
        |to| matches!(&to.tag, OverrideTag::PrimaryColor(c) if c.to_ass_hex().contains("00FF00")),
    );
    assert!(has_green, "Event 2 should have green primary colour");
}

// ---------------------------------------------------------------------------
// 7) SSA v4 and V4+ styles mixed parse
// ---------------------------------------------------------------------------

#[test]
fn test_mixed_ssa_v4_and_v4plus_styles() {
    // A document with both [V4 Styles] and [V4+ Styles] sections
    // Real-world SSA files may use [V4 Styles] format
    let content = "\
[Script Info]
ScriptType: v4.00
Title: Mixed format test

[V4 Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, TertiaryColour, BackColour, Bold, Italic, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, AlphaLevel, Encoding
Style: SSAOne,Arial,48,65535,255,0,12632256,-1,0,1,2,2,2,10,10,10,0,0
Style: SSATwo,Times New Roman,36,16776960,255,0,12632256,-1,0,1,2,2,5,10,10,10,0,0

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: V4PlusOne,Arial,40,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,-1,0,0,0,100,100,0,0,1,2,3,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Actor, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.00,SSAOne,,0,0,0,,First SSA style
Dialogue: 1,0:00:02.00,0:00:04.00,SSATwo,,0,0,0,,Second SSA style
Dialogue: 0,0:00:00.00,0:00:05.00,V4PlusOne,,0,0,0,,V4+ style event
";
    let doc = parse_doc(content);

    // Should have styles from BOTH sections
    assert_eq!(
        doc.styles.len(),
        3,
        "Should have 3 styles from both sections"
    );

    // Check first V4 style
    let ssa1 = doc.styles.iter().find(|s| s.name.as_str() == "SSAOne");
    assert!(ssa1.is_some(), "SSAOne style should exist");
    assert_eq!(ssa1.unwrap().font_name, "Arial");
    assert_eq!(ssa1.unwrap().font_size, 48.0);

    // Check second V4 style
    let ssa2 = doc.styles.iter().find(|s| s.name.as_str() == "SSATwo");
    assert!(ssa2.is_some(), "SSATwo style should exist");
    assert_eq!(ssa2.unwrap().font_name, "Times New Roman");
    assert_eq!(ssa2.unwrap().font_size, 36.0);

    // Check V4+ style
    let v4p = doc.styles.iter().find(|s| s.name.as_str() == "V4PlusOne");
    assert!(v4p.is_some(), "V4PlusOne style should exist");
    assert_eq!(v4p.unwrap().font_name, "Arial");

    // Events should reference correct styles
    assert_eq!(doc.events.len(), 3);
    assert_eq!(doc.events[0].style.as_str(), "SSAOne");
    assert_eq!(doc.events[1].style.as_str(), "SSATwo");
    assert_eq!(doc.events[2].style.as_str(), "V4PlusOne");
}

#[test]
fn test_ssa_v4_borderstyle_convert() {
    // SSA v4 uses BorderStyle 1=outline+shadow. Verify it maps correctly.
    let content = "\
[Script Info]
ScriptType: v4.00

[V4 Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, TertiaryColour, BackColour, Bold, Italic, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, AlphaLevel, Encoding
Style: SSAOne,Arial,48,65535,255,0,12632256,-1,0,1,2,2,2,10,10,10,0,0

[Events]
Format: Layer, Start, End, Style, Actor, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,SSAOne,,0,0,0,,Testing
";
    let doc = parse_doc(content);
    assert_eq!(doc.styles.len(), 1);
    let style = &doc.styles[0];
    assert_eq!(style.name.as_str(), "SSAOne");
    assert_eq!(style.outline, 2.0);
    assert_eq!(style.shadow, 2.0);
}

// ---------------------------------------------------------------------------
// 8) 100+ events stress test
// ---------------------------------------------------------------------------

#[test]
fn test_100_plus_events_stress() {
    // Generate 100+ events with varying tags, times, and styles
    let mut events_buf = String::new();

    for i in 0usize..120 {
        let layer = i % 4;
        let start_sec = (i * 2) % 60;
        let end_sec = start_sec + 3;
        let style = if i % 2 == 0 { "Default" } else { "Alt" };

        // Vary the tags with each event type
        let tags = match i % 5 {
            0 => format!("{{\\pos({x},{y})}}", x = 100 + i * 10, y = 200 + i * 5),
            1 => format!("{{\\fad({f},{f})}}", f = 100 + i % 10 * 50),
            2 => format!("{{\\move(0,0,{x},{y})}}", x = 100 + i * 8, y = 100 + i * 6),
            3 => format!("{{\\b1}}{{\\i1}}{{\\fs{fs}}}", fs = 20 + (i % 20)),
            4 => {
                let col = format!("{:06X}", (i * 0x12345) & 0xFFFFFF);
                let bd = (i % 5) + 1;
                format!("{{\\c&H{col}&}}{{\\bord{bd}}}")
            }
            _ => unreachable!(),
        };

        let text = format!("Stress test event #{i} with various tags");
        events_buf.push_str(&format!(
            "Dialogue: {layer},{start:02}:00:00.00,{end:02}:00:00.00,{style},,0,0,0,,{tags}{text}\n",
            layer = layer,
            start = start_sec / 60,
            end = end_sec / 60,
            style = style,
            tags = tags,
            text = text,
        ));
    }

    let doc = parse_events(&events_buf);
    assert_eq!(doc.events.len(), 120, "Should have 120 events");

    // Verify all events have valid timing and structure
    for (i, ev) in doc.events.iter().enumerate() {
        // End must be after start (or equal if both same)
        assert!(
            ev.end_ms >= ev.start_ms,
            "Event {i}: end ({}) < start ({})",
            ev.end_ms,
            ev.start_ms
        );

        // Style name should be valid
        let style_name = ev.style.as_str();
        assert!(
            style_name == "Default" || style_name == "Alt",
            "Event {i}: unexpected style '{style_name}'"
        );

        // Raw text preserved (no corruption)
        assert!(
            ev.text_raw.contains("Stress test event"),
            "Event {i}: text corrupted"
        );
    }

    // Verify all 5 tag patterns generated override tags correctly
    let event_types: Vec<usize> = (0..120).map(|i| i % 5).collect();

    // Check pos events (type 0)
    for i in (0..120).filter(|&i| event_types[i] == 0) {
        let ev = &doc.events[i];
        let has_pos = ev
            .override_tags
            .iter()
            .any(|to| matches!(to.tag, OverrideTag::Pos { .. }));
        assert!(has_pos, "Event {i} (pos type) missing Pos tag");
    }

    // Check fad events (type 1)
    for i in (0..120).filter(|&i| event_types[i] == 1) {
        let ev = &doc.events[i];
        let has_fad = ev
            .override_tags
            .iter()
            .any(|to| matches!(to.tag, OverrideTag::Fade { .. }));
        assert!(has_fad, "Event {i} (fad type) missing Fade tag");
    }

    // Check bold events (type 3) should have Bold tag
    for i in (0..120).filter(|&i| event_types[i] == 3) {
        let ev = &doc.events[i];
        let has_bold = ev
            .override_tags
            .iter()
            .any(|to| matches!(to.tag, OverrideTag::Bold(true)));
        assert!(has_bold, "Event {i} (bold type) missing Bold tag");
    }

    // Check colour events (type 4) should have PrimaryColor tag
    for i in (0..120).filter(|&i| event_types[i] == 4) {
        let ev = &doc.events[i];
        let has_color = ev
            .override_tags
            .iter()
            .any(|to| matches!(to.tag, OverrideTag::PrimaryColor(_)));
        assert!(has_color, "Event {i} (color type) missing PrimaryColor tag");
    }

    // Verify layer distribution
    let layers: Vec<u32> = doc.events.iter().map(|e| e.layer).collect();
    assert!(layers.contains(&0), "Should have layer 0");
    assert!(layers.contains(&1), "Should have layer 1");
    assert!(layers.contains(&2), "Should have layer 2");
    assert!(layers.contains(&3), "Should have layer 3");
}

// ---------------------------------------------------------------------------
// Edge cases within the complex scenarios
// ---------------------------------------------------------------------------

#[test]
fn test_move_6arg_swap_handling() {
    // \move with explicit t1,t2 where t1 > t2 (should be swapped)
    let events = "\
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\\move(0,0,100,100,3000,1000)}Swapped timing
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 1);
    let ev = &doc.events[0];

    let has_swap = ev
        .override_tags
        .iter()
        .any(|to| matches!(to.tag, OverrideTag::Move { t1, t2, .. } if t1 == 1000 && t2 == 3000));
    assert!(
        has_swap,
        "Move should have swapped t1/t2 (3000,1000 → 1000,3000)"
    );
}

#[test]
fn test_fade_complex_7arg() {
    // 7-argument \fade with alpha ramp
    let events = "\
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\\fade(255,0,255,500,2000,3000,4500)}Complex fade
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 1);
    let ev = &doc.events[0];

    let has_fade = ev.override_tags.iter().any(|to| {
        matches!(to.tag, OverrideTag::FadeComplex {
            alpha_start, alpha_mid, alpha_end,
            t1, t2, t3, t4,
        } if alpha_start == 255 && alpha_mid == 0 && alpha_end == 255
            && t1 == 500 && t2 == 2000 && t3 == 3000 && t4 == 4500)
    });
    assert!(has_fade, "Complex 7-arg fade not parsed correctly");
}

#[test]
fn test_multiple_overlapping_hms_timestamps() {
    // Events with timestamps crossing the hour boundary
    let events = "\
Dialogue: 0,0:59:50.00,1:00:05.00,Default,,0,0,0,,Cross-hour start
Dialogue: 0,1:00:00.00,1:00:30.00,Alt,,0,0,0,,Cross-hour end
Dialogue: 0,1:30:00.00,1:30:15.00,Default,,0,0,0,,Well into second hour
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 3);

    // 0:59:50.00 = 59*60*1000 + 50*1000 = 3,590,000 ms
    assert_eq!(doc.events[0].start_ms, 59 * 60_000 + 50_000);

    // 1:00:00.00 = 3,600,000 ms
    assert_eq!(doc.events[1].start_ms, 3_600_000);
    assert_eq!(doc.events[1].end_ms, 3_600_000 + 30_000);

    // 1:30:00.00 = 5,400,000 ms
    assert_eq!(doc.events[2].start_ms, 5_400_000);
    assert_eq!(doc.events[2].end_ms, 5_400_000 + 15_000);
}

#[test]
fn test_uppercase_k_equals_kf() {
    // libass compat: uppercase \K should be parsed as \kf (Fill) style
    let events = "\
Dialogue: 0,0:00:00.00,0:00:03.00,KaraokeStyle,,0,0,0,Karaoke,{\\K40}Kap-{\\K30}ping
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 1);

    let kseg = &doc.events[0].karaoke;
    assert!(!kseg.is_empty(), "Should have karaoke segments");
    assert_eq!(
        kseg[0].style,
        KaraokeStyle::Fill,
        "\\K should map to Fill style"
    );
    assert_eq!(kseg[0].duration_ms, 400, "\\K40 should be 400ms");
    assert_eq!(kseg[0].text, "Kap-");
}

#[test]
fn test_scroll_effects_with_override_tags() {
    // Scroll up and scroll down effects combined with override tags
    let events = "\
Dialogue: 0,0:00:00.00,0:00:08.00,Default,,0,0,0,Scroll up;100;50;200,{\\c&HFF8800&}{\\fs36}Scrolling up text
Dialogue: 0,0:00:02.00,0:00:10.00,Default,,0,0,0,Scroll down;150;30;180,{\\b1}{\\i1}Scrolling down text
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 2);

    // Scroll up
    assert_eq!(
        doc.events[0].effect,
        Effect::ScrollUp {
            delay: 100,
            top: 50,
            bottom: 200,
        }
    );
    let has_color = doc.events[0].override_tags.iter().any(
        |to| matches!(&to.tag, OverrideTag::PrimaryColor(c) if c.to_ass_hex().contains("FF8800")),
    );
    assert!(has_color, "Scroll up should have colour override");

    // Scroll down
    assert_eq!(
        doc.events[1].effect,
        Effect::ScrollDown {
            delay: 150,
            top: 30,
            bottom: 180,
        }
    );
    let has_bold = doc.events[1]
        .override_tags
        .iter()
        .any(|to| matches!(to.tag, OverrideTag::Bold(true)));
    assert!(has_bold, "Scroll down should have bold override");
}

#[test]
fn test_comment_events_parsed() {
    // Comment events should be parsed, not skipped
    let events = "\
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,Regular dialogue
Comment: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,This is a comment
Comment: 0,0:00:02.00,0:00:04.00,Default,,0,0,0,,{\\pos(100,200)}Another comment with tags
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 3);
    assert_eq!(doc.events[0].event_type, EventType::Dialogue);
    assert_eq!(doc.events[1].event_type, EventType::Comment);
    assert_eq!(doc.events[2].event_type, EventType::Comment);

    // Comment events should still have tag parsing
    assert!(
        !doc.events[2].override_tags.is_empty(),
        "Comment events should have override tags parsed"
    );
    let has_pos = doc.events[2]
        .override_tags
        .iter()
        .any(|to| matches!(to.tag, OverrideTag::Pos { .. }));
    assert!(has_pos, "Comment event should have Pos tag");
}

#[test]
fn test_v4plus_alignment_values() {
    // Verify alignment values in V4+ styles are correctly parsed
    let events = "\
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\\an7}Top-left
Dialogue: 0,0:00:01.00,0:00:06.00,Default,,0,0,0,,{\\an8}Top-centre
Dialogue: 0,0:00:02.00,0:00:07.00,Default,,0,0,0,,{\\an9}Top-right
Dialogue: 0,0:00:03.00,0:00:08.00,Default,,0,0,0,,{\\an4}Centre-left
Dialogue: 0,0:00:04.00,0:00:09.00,Default,,0,0,0,,{\\an5}Centre
Dialogue: 0,0:00:05.00,0:00:10.00,Default,,0,0,0,,{\\an2}Bottom-centre
";
    let doc = parse_events(events);
    assert_eq!(doc.events.len(), 6);

    let alignment_at = |i: usize| -> Option<u8> {
        doc.events[i].override_tags.iter().find_map(|to| {
            if let OverrideTag::AlignmentNumpad(v) = to.tag {
                Some(v)
            } else {
                None
            }
        })
    };

    assert_eq!(alignment_at(0), Some(7));
    assert_eq!(alignment_at(1), Some(8));
    assert_eq!(alignment_at(2), Some(9));
    assert_eq!(alignment_at(3), Some(4));
    assert_eq!(alignment_at(4), Some(5));
    assert_eq!(alignment_at(5), Some(2));
}
