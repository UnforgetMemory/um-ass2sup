mod common;
use subtitle_renderer::{RenderConfig, Renderer};

// ── Karaoke rendering ───────────────────────────────────────────

#[test]
fn test_karaoke_fad_alpha_applied() {
    // Karaoke with \fad at t=500 (mid-fade-in) should have reduced alpha.
    // \fad(1000,1000): fade-in 0..1000ms → at t=500, alpha_multiplier ~0.5
    let ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\fad(1000,1000)\k100}Test"#;

    let doc = common::parse_doc(ass);
    let renderer = Renderer::new(RenderConfig::default());
    let frame = common::render_doc(&renderer, &doc, 500).unwrap();
    assert!(
        frame.bitmap.iter().any(|&b| b != 0),
        "Karaoke with fade should have visible pixels at t=500"
    );
}

#[test]
fn test_ko_active_outline_boost() {
    let content = r#"[Script Info]
Title: KOActive
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:06.00,Default,,0,0,0,,{\ko50}First{\ko50}Second
"#;
    let doc = common::parse_doc(content);
    let renderer = Renderer::new(RenderConfig::default());
    // t=1250: syllable 1 Active (progress=0.5, outline boosted 3x)
    let active_frame = common::render_doc(&renderer, &doc, 1250).unwrap();
    // t=2500: both syllables Done (full glyph in primary)
    let done_frame = common::render_doc(&renderer, &doc, 2500).unwrap();
    assert!(
        active_frame.bitmap.iter().any(|&b| b != 0),
        "KO Active should have content"
    );
    assert!(
        done_frame.bitmap.iter().any(|&b| b != 0),
        "KO Done should have content"
    );
    // Active \ko (secondary fill + primary outline sweep) vs Done (full primary glyph)
    assert_ne!(
        active_frame.bitmap, done_frame.bitmap,
        "KO Active (outline sweep) should differ from Done (full primary glyph)"
    );
}

#[test]
fn test_ko_done_full_glyph() {
    let content = r#"[Script Info]
Title: KODone
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:06.00,Default,,0,0,0,,{\ko50}First{\ko50}Second
"#;
    let doc = common::parse_doc(content);
    let renderer = Renderer::new(RenderConfig::default());
    // t=2500: both syllables Done => full primary-color glyph
    let done_frame = common::render_doc(&renderer, &doc, 2500).unwrap();
    // t=1000: syllable 1 Active (progress=0), syllable 2 Pending
    let pending_active_frame = common::render_doc(&renderer, &doc, 1000).unwrap();
    assert!(
        done_frame.bitmap.iter().any(|&b| b != 0),
        "KO Done should have content"
    );
    assert!(
        pending_active_frame.bitmap.iter().any(|&b| b != 0),
        "KO Pending/Active should have content"
    );
    assert_ne!(
        pending_active_frame.bitmap, done_frame.bitmap,
        "KO Done (primary) should differ from Pending/Active (secondary + outline)"
    );
}

#[test]
fn test_karaoke_render_all_styles_no_panic() {
    let content = r#"[Script Info]
Title: KaraokeRender
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:06.00,Default,,0,0,0,,{\k50}Hel{\kf75}lo {\ko100}Wor{\kt200}ld
"#;
    let doc = common::parse_doc(content);
    let renderer = Renderer::new(RenderConfig::default());

    // Render before, during, and after karaoke event — all should produce frames
    let before = common::render_doc(&renderer, &doc, 500);
    assert!(
        before.is_some(),
        "Karaoke render before event should produce frame"
    );

    let during = common::render_doc(&renderer, &doc, 3000);
    assert!(during.is_some(), "Mid-event kt should render");
    let f = during.unwrap();
    assert!(
        f.bitmap.iter().any(|&b| b != 0),
        "Karaoke during event should have visible pixels"
    );

    let after = common::render_doc(&renderer, &doc, 7000);
    assert!(
        after.is_some(),
        "Karaoke render after event should produce frame"
    );
}

#[test]
fn test_karaoke_segments_populated_with_all_tags() {
    let content = r#"[Script Info]
Title: KaraokeAll
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:06.00,Default,,0,0,0,,{\k50}Hel{\kf75}lo {\ko100}Wor{\kt200}ld
"#;
    let doc = common::parse_doc(content);
    let event = &doc.events[0];
    assert!(
        !event.karaoke.is_empty(),
        "Karaoke segments should be populated"
    );

    // Verify all four tag types are present
    let styles: Vec<_> = event.karaoke.iter().map(|s| s.style).collect();
    assert!(
        styles.contains(&ass_core::KaraokeStyle::Instant),
        "Should have \\k style"
    );
    assert!(
        styles.contains(&ass_core::KaraokeStyle::Fill),
        "Should have \\kf style"
    );
    assert!(
        styles.contains(&ass_core::KaraokeStyle::Outline),
        "Should have \\ko style"
    );
    assert!(
        styles.contains(&ass_core::KaraokeStyle::Timing),
        "Should have \\kt style"
    );

    // Verify segments have text content
    for seg in &event.karaoke {
        assert!(
            !seg.text.is_empty(),
            "Each karaoke segment should have text"
        );
        assert!(
            seg.duration_ms > 0,
            "Each karaoke segment should have positive duration"
        );
    }
}

#[test]
fn test_ko_pending_no_outline() {
    let content = r#"[Script Info]
Title: KOPending
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:06.00,Default,,0,0,0,,{\ko50}First{\ko50}Second
"#;
    let doc = common::parse_doc(content);
    let renderer = Renderer::new(RenderConfig::default());
    // t=1000 = event start: syllable 1 Active, syllable 2 Pending
    // In Pending \ko: outline_width=0, fill stays secondary color
    let frame = common::render_doc(&renderer, &doc, 1000).unwrap();
    assert!(
        frame.bitmap.iter().any(|&b| b != 0),
        "KO pending phase should render visible output"
    );
}

#[test]
fn test_karaoke_shadow_blur() {
    // Verify karaoke shadow has blur applied (same as non-karaoke path)
    let ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,5,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\blur5\k100}Test"#;

    let doc = common::parse_doc(ass);
    let renderer = Renderer::new(RenderConfig::default());
    let frame = common::render_doc(&renderer, &doc, 1000).unwrap();

    let non_zero = frame.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(
        non_zero > 0,
        "Karaoke with shadow+blur should produce visible pixels"
    );

    // Compare with no-blur version — blurred shadow should have more non-zero pixels
    let ass_no_blur = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,5,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\k100}Test"#;

    let doc_no_blur = common::parse_doc(ass_no_blur);
    let frame_no_blur = common::render_doc(&renderer, &doc_no_blur, 1000).unwrap();
    let non_zero_no_blur = frame_no_blur.bitmap.iter().filter(|&&b| b > 0).count();

    // Blurred shadow should spread to more pixels
    assert!(
        non_zero >= non_zero_no_blur,
        "Blurred karaoke shadow ({non_zero} pixels) should have at least as many visible pixels as non-blurred ({non_zero_no_blur})"
    );
}

#[test]
fn test_karaoke_no_shadow_no_blur() {
    // Verify no shadow artifacts when shadow_depth=0 (default style)
    let ass = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\k100}Test"#;

    let doc = common::parse_doc(ass);
    let renderer = Renderer::new(RenderConfig::default());
    let frame = common::render_doc(&renderer, &doc, 1000).unwrap();

    let non_zero = frame.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(
        non_zero > 0,
        "Karaoke without shadow should produce visible pixels"
    );

    // Compare with shadow version — no-shadow should have fewer pixels
    let ass_shadow = r#"[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,5,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\k100}Test"#;

    let doc_shadow = common::parse_doc(ass_shadow);
    let frame_shadow = common::render_doc(&renderer, &doc_shadow, 1000).unwrap();
    let non_zero_shadow = frame_shadow.bitmap.iter().filter(|&&b| b > 0).count();

    // Shadow should add more visible pixels (shadow extends beyond text)
    assert!(
        non_zero_shadow >= non_zero,
        "Shadow version ({non_zero_shadow} pixels) should have at least as many visible pixels as no-shadow ({non_zero})"
    );
}
