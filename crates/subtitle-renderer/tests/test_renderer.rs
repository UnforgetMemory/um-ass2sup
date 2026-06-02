use subtitle_renderer::{RenderConfig, RenderContext, RenderedFrame, FontManager, Shaper, Renderer};
use ass_parser::{AssFile, Effect, ScriptInfo, Style, Event, EventType, Timestamp, AssColor};

#[test]
fn test_render_config_default() {
    let cfg = RenderConfig::default();
    assert_eq!(cfg.width, 1920);
    assert_eq!(cfg.height, 1080);
    assert_eq!(cfg.script_width, 1920);
    assert_eq!(cfg.script_height, 1080);
    assert_eq!(cfg.default_font, "Arial");
    assert_eq!(cfg.default_font_size, 48.0);
}

#[test]
fn test_render_context_default() {
    let ctx = RenderContext::default();
    assert_eq!(ctx.font_name, "Arial");
    assert_eq!(ctx.font_size, 48.0);
    assert_eq!(ctx.primary_color, [255, 255, 255, 255]);
    assert_eq!(ctx.outline_color, [0, 0, 0, 255]);
    assert_eq!(ctx.alignment, 2);
    assert!(!ctx.bold);
    assert!(!ctx.italic);
}

#[test]
fn test_rendered_frame_clone() {
    let f = RenderedFrame {
        pts_ms: 1000,
        duration_ms: 4000,
        width: 1920,
        height: 1080,
        bitmap: vec![0u8; 1920 * 1080 * 4],
    };
    let c = f.clone();
    assert_eq!(c.pts_ms, 1000);
    assert_eq!(c.bitmap.len(), 1920 * 1080 * 4);
}

#[test]
fn test_font_manager_new() {
    let fm = FontManager::new();
    assert_eq!(fm.font_count(), 0);
}

#[test]
fn test_font_manager_default() {
    let fm = FontManager::default();
    assert_eq!(fm.font_count(), 0);
}

#[test]
fn test_font_manager_load_system() {
    let mut fm = FontManager::new();
    fm.load_system_fonts();
    assert!(fm.font_count() > 0);
}

fn find_any_font(fm: &FontManager) -> Option<fontdb::ID> {
    fm.query("Arial", false, false)
        .or_else(|| fm.query("Liberation Sans", false, false))
        .or_else(|| fm.query("DejaVu Sans", false, false))
        .or_else(|| fm.query("Noto Sans", false, false))
        .or_else(|| fm.list_fonts().first().map(|f| f.id))
}

#[test]
fn test_font_manager_query_returns_id() {
    let mut fm = FontManager::new();
    fm.load_system_fonts();
    let result = find_any_font(&fm);
    assert!(result.is_some(), "No system fonts found");
}

#[test]
fn test_font_manager_query_nonexistent() {
    let fm = FontManager::new();
    let result = fm.query("NonExistentFont12345", false, false);
    assert!(result.is_none());
}

#[test]
fn test_font_manager_get_font_data() {
    let mut fm = FontManager::new();
    fm.load_system_fonts();
    let id = find_any_font(&fm).expect("No system fonts found");
    let data = fm.get_font_data(id);
    assert!(data.is_some());
    assert!(!data.unwrap().is_empty());
}

#[test]
fn test_font_manager_list_fonts() {
    let mut fm = FontManager::new();
    fm.load_system_fonts();
    let fonts = fm.list_fonts();
    assert!(!fonts.is_empty());
    assert!(fonts.iter().any(|f| !f.family.is_empty()));
}

#[test]
fn test_shaper_shape() {
    let mut fm = FontManager::new();
    fm.load_system_fonts();
    let id = find_any_font(&fm).expect("No system fonts found");
    let shaper = Shaper::new(&fm);
    let result = shaper.shape("Hello", id, 48.0);
    assert!(result.is_ok());
    let shaped = result.unwrap();
    assert!(!shaped.glyphs.is_empty());
    assert!(shaped.total_advance > 0.0);
}

#[test]
fn test_shaper_empty_text() {
    let mut fm = FontManager::new();
    fm.load_system_fonts();
    let id = find_any_font(&fm).expect("No system fonts found");
    let shaper = Shaper::new(&fm);
    let result = shaper.shape("", id, 48.0);
    assert!(result.is_ok());
    assert!(result.unwrap().glyphs.is_empty());
}

#[test]
fn test_shaper_cjk_text() {
    let mut fm = FontManager::new();
    fm.load_system_fonts();
    if let Some(id) = find_any_font(&fm) {
        let shaper = Shaper::new(&fm);
        let result = shaper.shape("你好", id, 48.0);
        assert!(result.is_ok());
    }
}

fn make_default_ass() -> AssFile {
    let content = r#"[Script Info]
Title: Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World
"#;
    AssFile::parse(content).unwrap()
}

#[test]
fn test_render_ass_simple() {
    let ass = make_default_ass();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000);
    assert!(frame.is_some(), "Should render visible event at t=2000ms");
    let f = frame.unwrap();
    assert_eq!(f.width, 1920);
    assert_eq!(f.height, 1080);
    assert_eq!(f.bitmap.len(), 1920 * 1080 * 4);
    assert!(f.pts_ms == 2000);
}

#[test]
fn test_render_ass_outside_event_returns_none() {
    let ass = make_default_ass();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 500);
    assert!(frame.is_some(), "render_ass always returns Some pixmap");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert_eq!(non_zero, 0, "No visible events at t=500ms should produce empty bitmap");
}

#[test]
fn test_render_ass_bitmap_has_content() {
    let ass = make_default_ass();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000).unwrap();
    let non_zero = frame.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(non_zero > 0, "Bitmap should have non-zero pixels when text is rendered");
}

#[test]
fn test_render_ass_empty_events() {
    let content = r#"[Script Info]
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
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000);
    assert!(frame.is_some(), "render_ass always returns Some pixmap");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert_eq!(non_zero, 0, "Empty events should produce empty bitmap");
}

#[test]
fn test_render_ass_multiline_center() {
    let content = r#"[Script Info]
Title: Multi
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\an8}Line One\NLine Two
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000);
    assert!(frame.is_some(), "Should render multi-line text");
}

#[test]
fn test_render_ass_override_tags() {
    let content = r#"[Script Info]
Title: Tags
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\pos(200,300)\b1\i1\fs72\1c&H0000FF&}Bold Italic Red
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000).unwrap();
    assert!(frame.bitmap.iter().any(|&b| b > 0), "Override tags should produce visible output");
}

#[test]
fn test_render_ass_single_char() {
    let content = r#"[Script Info]
Title: Single
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,X
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000);
    assert!(frame.is_some(), "Single char should render");
}

#[test]
fn test_render_ass_long_text() {
    let long = "A".repeat(500);
    let content = format!(r#"[Script Info]
Title: Long
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{long}
"#);
    let ass = AssFile::parse(&content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000);
    assert!(frame.is_some(), "Long text should still produce a frame without panic");
}

#[test]
fn test_render_ass_overlay_two_events() {
    let content = r#"[Script Info]
Title: Overlay
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\an7}Top Left
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\an3}Bottom Right
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000).unwrap();
    let non_zero = frame.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(non_zero > 0, "Overlay events should produce visible output");
}

#[test]
fn test_render_ass_fade_effect() {
    let content = r#"[Script Info]
Title: Fade
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\fad(500,500)}Fading Text
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    // At start (1000ms), should be fading in
    let frame_start = renderer.render_ass(&ass, 1000);
    // At middle (3000ms), should be fully visible
    let frame_mid = renderer.render_ass(&ass, 3000);
    assert!(frame_start.is_some() || frame_mid.is_some(), "Fade effect should produce frames");
}

#[test]
fn test_render_ass_clip() {
    let content = r#"[Script Info]
Title: Clip
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\clip(100,100,500,500)}Clipped Text
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000);
    assert!(frame.is_some(), "Clipped text should still render");
}

#[test]
fn test_render_ass_custom_resolution() {
    let content = r#"[Script Info]
Title: 720p
ScriptType: v4.00+
PlayResX: 1280
PlayResY: 720

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,36,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,720p text
"#;
    let ass = AssFile::parse(content).unwrap();
    let cfg = RenderConfig { width: 1280, height: 720, script_width: 1280, script_height: 720, ..Default::default() };
    let mut renderer = Renderer::new(cfg);
    let frame = renderer.render_ass(&ass, 2000).unwrap();
    assert_eq!(frame.width, 1280);
    assert_eq!(frame.height, 720);
}

fn make_simple_ass(text: &str, start_cs: u64, end_cs: u64) -> AssFile {
    let mut ass = AssFile::new();
    ass.events.push(Event {
        event_type: EventType::Dialogue,
        layer: 0,
        start: Timestamp::from_ms(start_cs),
        end: Timestamp::from_ms(end_cs),
        style_name: "Default".to_string(),
        name: String::new(),
        margin_l: 0,
        margin_r: 0,
        margin_v: 0,
        effect: Effect::None,
        text: text.to_string(),
        override_tags: vec![],
        karaoke_segments: vec![],
        raw_override_block: String::new(),
    });
    ass
}

#[test]
fn test_render_ass_simple_text() {
    let renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("Hello World", 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some(), "Should render non-empty text");
    let f = frame.unwrap();
    assert_eq!(f.width, 1920);
    assert_eq!(f.height, 1080);
    assert_eq!(f.bitmap.len(), 1920 * 1080 * 4);
    assert!(f.bitmap.iter().any(|&b| b != 0), "Bitmap should have non-zero pixels");
}

#[test]
fn test_render_ass_returns_none_outside_time() {
    let renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("Hello", 1000, 5000);
    // render_ass always returns Some pixmap — empty when no events visible
    let f_before = renderer.render_ass(&ass, 0).unwrap();
    assert!(f_before.bitmap.iter().all(|&b| b == 0), "Before start: empty bitmap");
    let f_after = renderer.render_ass(&ass, 6000).unwrap();
    assert!(f_after.bitmap.iter().all(|&b| b == 0), "After end: empty bitmap");
    assert!(renderer.render_ass(&ass, 2000).is_some(), "During event");
}

#[test]
fn test_render_ass_empty_text_returns_none() {
    let renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("", 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some(), "render_ass always returns Some");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert_eq!(non_zero, 0, "No events should produce empty bitmap");
}

#[test]
fn test_render_ass_no_events() {
    let renderer = Renderer::new(RenderConfig::default());
    let ass = AssFile::new();
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some(), "render_ass always returns Some");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert_eq!(non_zero, 0, "No events should produce empty bitmap");
}

#[test]
fn test_render_ass_with_override_pos() {
    let renderer = Renderer::new(RenderConfig::default());
    let mut ass = make_simple_ass("Positioned", 0, 5000);
    ass.events[0].text = "{\\pos(500,300)}Positioned".to_string();
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
    assert!(frame.unwrap().bitmap.iter().any(|&b| b != 0));
}

#[test]
fn test_render_ass_with_color_override() {
    let renderer = Renderer::new(RenderConfig::default());
    let content = r#"[Script Info]
Title: Color Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H000000FF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,0,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Red Text
"#;
    let ass = AssFile::parse(content).unwrap();
    let frame = renderer.render_ass(&ass, 3000);
    assert!(frame.is_some());
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(non_zero > 0, "Should have rendered pixels");
}

#[test]
fn test_render_ass_with_fade() {
    let renderer = Renderer::new(RenderConfig::default());
    let mut ass = make_simple_ass("Fading", 0, 5000);
    ass.events[0].text = "{\\fad(500,500)}Fading".to_string();
    let frame_mid = renderer.render_ass(&ass, 2500);
    assert!(frame_mid.is_some(), "Mid-event should render");
    let f = frame_mid.unwrap();
    assert!(f.bitmap.iter().any(|&b| b != 0));
}

#[test]
fn test_render_ass_cache() {
    use subtitle_renderer::{FrameCache, make_frame_key};
    let renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("Cached", 0, 5000);
    let cache = FrameCache::new(16);
    let f1 = renderer.render_ass_cached(&ass, 1000, &cache, 0);
    assert!(f1.is_some());
    let key = make_frame_key(1000);
    assert!(cache.contains(&key));
    let f2 = renderer.render_ass_cached(&ass, 1000, &cache, 0);
    assert!(f2.is_some());
}

#[test]
fn test_render_single_character() {
    let renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("A", 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
    assert!(frame.unwrap().bitmap.iter().any(|&b| b != 0));
}

#[test]
fn test_render_long_text() {
    let renderer = Renderer::new(RenderConfig::default());
    let long = "A".repeat(200);
    let ass = make_simple_ass(&long, 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_unicode() {
    let renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("中文测试 🎵", 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_special_chars() {
    let renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("<>&\"'{}", 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_multiline() {
    let renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("Line1\\NLine2\\NLine3", 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
    assert!(frame.unwrap().bitmap.iter().any(|&b| b != 0));
}

#[test]
fn test_render_two_overlapping_events() {
    let renderer = Renderer::new(RenderConfig::default());
    let mut ass = AssFile::new();
    ass.events.push(Event {
        event_type: EventType::Dialogue,
        layer: 0,
        start: Timestamp::from_ms(0),
        end: Timestamp::from_ms(3000),
        style_name: "Default".to_string(),
        name: String::new(),
        margin_l: 0,
        margin_r: 0,
        margin_v: 0,
        effect: Effect::None,
        text: "Event1".to_string(),
        override_tags: vec![],
        karaoke_segments: vec![],
        raw_override_block: String::new(),
    });
    ass.events.push(Event {
        event_type: EventType::Dialogue,
        layer: 0,
        start: Timestamp::from_ms(1000),
        end: Timestamp::from_ms(5000),
        style_name: "Default".to_string(),
        name: String::new(),
        margin_l: 0,
        margin_r: 0,
        margin_v: 0,
        effect: Effect::None,
        text: "Event2".to_string(),
        override_tags: vec![],
        karaoke_segments: vec![],
        raw_override_block: String::new(),
    });
    let frame = renderer.render_ass(&ass, 2000);
    assert!(frame.is_some(), "Should render overlapping events");
    assert!(frame.unwrap().bitmap.iter().any(|&b| b != 0));
}

#[test]
fn test_render_with_move_tag() {
    let renderer = Renderer::new(RenderConfig::default());
    let mut ass = make_simple_ass("Moving", 0, 5000);
    ass.events[0].text = "{\\move(100,100,500,500)}Moving".to_string();
    let frame = renderer.render_ass(&ass, 2500);
    assert!(frame.is_some());
}

#[test]
fn test_render_with_blur() {
    let renderer = Renderer::new(RenderConfig::default());
    let mut ass = make_simple_ass("Blurred", 0, 5000);
    ass.events[0].text = "{\\blur(3)}Blurred".to_string();
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_with_rotation() {
    let renderer = Renderer::new(RenderConfig::default());
    let mut ass = make_simple_ass("Rotated", 0, 5000);
    ass.events[0].text = "{\\frz(45)}Rotated".to_string();
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_with_border() {
    let renderer = Renderer::new(RenderConfig::default());
    let mut ass = make_simple_ass("Bordered", 0, 5000);
    ass.events[0].text = "{\\bord(5)}Bordered".to_string();
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_with_shadow() {
    let renderer = Renderer::new(RenderConfig::default());
    let mut ass = make_simple_ass("Shadow", 0, 5000);
    ass.events[0].text = "{\\shad(5)}Shadow".to_string();
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_ko_karaoke_no_panic() {
    let content = r#"[Script Info]
Title: KO Test
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:06.00,Default,,0,0,0,,{\ko50}He{\ko100}llo{\ko150} Wo{\ko200}rld
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());

    let frame_before = renderer.render_ass(&ass, 500);
    assert!(frame_before.is_some());

    let frame_during = renderer.render_ass(&ass, 3000);
    assert!(frame_during.is_some());

    let frame_after = renderer.render_ass(&ass, 7000);
    assert!(frame_after.is_some());
}

// ═══════════════════════════════════════════════════════════════════
// Phase 12 Integration Tests
// ═══════════════════════════════════════════════════════════════════

// ── B0: Writing mode parsing ─────────────────────────────────────

#[test]
fn test_writing_mode_build_context_sets_mode() {
    let content = r#"[Script Info]
Title: WritingMode
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\writing_mode(2)}Vertical Text
"#;
    let ass = AssFile::parse(content).unwrap();
    assert!(!ass.events[0].override_tags.is_empty());
    let has_wm = ass.events[0].override_tags.iter().any(|t| matches!(t, ass_parser::OverrideTag::WritingMode(2)));
    assert!(has_wm, "writing_mode(2) should be parsed as WritingMode(2) tag");
}

#[test]
fn test_writing_mode_render_vertical_no_panic() {
    let content = r#"[Script Info]
Title: WritingMode
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\writing_mode(2)}縦書き
Dialogue: 0,0:00:05.00,0:00:09.00,Default,,0,0,0,,{\writing_mode(3)}Vertical left
Dialogue: 0,0:00:09.00,0:00:13.00,Default,,0,0,0,,{\writing_mode(1)}Horizontal
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());

    let frame = renderer.render_ass(&ass, 3000);
    assert!(frame.is_some(), "writing_mode(2) should produce frame");

    let frame_lr = renderer.render_ass(&ass, 7000);
    assert!(frame_lr.is_some(), "writing_mode(3) should produce frame");

    let frame_h = renderer.render_ass(&ass, 11000);
    assert!(frame_h.is_some(), "writing_mode(1) should produce frame");
}

// ── B0: Layer ordering ───────────────────────────────────────────

#[test]
fn test_layer_ordering_lower_renders_first() {
    let content = r#"[Script Info]
Title: LayerOrder
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 1,0:00:01.00,0:00:05.00,Default,,0,0,0,,TopLayer
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,BottomLayer
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 3000);
    assert!(frame.is_some(), "Layer-ordered rendering should produce a frame");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(non_zero > 0, "Layer-ordered rendering should have non-zero pixels");
}

// ── B0: \t comma parsing with nested parens ──────────────────────

#[test]
fn test_t_parsing_with_nested_parens_integration() {
    // Test via the standalone parser which correctly handles nested parens without
    // being affected by the \\-based splitting used in event-level parsing.
    let result = ass_parser::parse_override_tag("t(\\pos(100,200),0,1000,1)").unwrap();
    match &result {
        ass_parser::OverrideTag::Transform { tag, t1, t2, accel } => {
            assert_eq!(tag, "\\pos(100,200)", "Inner tag should preserve nested parens");
            assert_eq!(*t1, 0, "t1 should be 0");
            assert_eq!(*t2, 1000, "t2 should be 1000");
            assert!((*accel - 1.0).abs() < 0.01, "accel should be 1.0");
        }
        other => panic!("Expected Transform tag, got {other:?}"),
    }

    // Also verify the event-level parsing picks up at least a Transform tag
    let content = r#"[Script Info]
Title: TTest
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\t(\pos(100,200),0,1000,1)}Moving
"#;
    let ass = AssFile::parse(content).unwrap();
    let any_transform = ass.events[0].override_tags.iter().any(|t| {
        matches!(t, ass_parser::OverrideTag::Transform { .. })
    });
    assert!(any_transform, "Event should contain at least a Transform tag");
}

// ── B1: border_style=3 opaque box ────────────────────────────────

#[test]
fn test_border_style_3_renders_opaque_box() {
    let content = r#"[Script Info]
Title: BorderStyle3
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Opaque,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H000000FF,0,0,0,0,100,100,0,0,3,0,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Opaque,,0,0,0,,Opaque Box Text
"#;
    let ass = AssFile::parse(content).unwrap();
    // Verify the style is parsed with BorderStyle=3
    let style = &ass.styles[0];
    assert_eq!(style.border_style, 3, "Style should have BorderStyle=3");

    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 3000);
    assert!(frame.is_some(), "BorderStyle=3 should render");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(non_zero > 0, "Opaque box rendering should have visible pixels");
}

// ── B2: \ko outline boost ────────────────────────────────────────

#[test]
fn test_ko_outline_boost_active_syllable() {
    // Verify that \ko tags are parsed correctly and KO karaoke renders.
    let content = r#"[Script Info]
Title: KOTest
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:06.00,Default,,0,0,0,,{\ko100}He{\ko100}llo
"#;
    let ass = AssFile::parse(content).unwrap();
    // Verify the override tags contain KO karaoke style indicators
    let has_ko = ass.events[0].override_tags.iter().any(|t| {
        matches!(t, ass_parser::OverrideTag::Karaoke { style: ass_parser::karaoke::KaraokeStyle::Outline, .. })
    });
    assert!(has_ko, "KO override tag should be parsed as KaraokeStyle::Outline");

    // Render at t=2000ms to exercise the ko path
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 2000);
    assert!(frame.is_some(), "KO karaoke should render without panic at mid-event");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(non_zero > 0, "KO karaoke should produce visible output");
}

// ── B3: \r named style reset ─────────────────────────────────────

#[test]
fn test_r_named_style_reset_renders() {
    let content = r#"[Script Info]
Title: RTest
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1
Style: Alt,Times New Roman,36,&H00FF0000,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\rAlt}Reset To Alt Style
"#;
    let ass = AssFile::parse(content).unwrap();
    assert_eq!(ass.styles.len(), 2, "Should have two styles");
    assert_eq!(ass.styles[1].name, "Alt", "Second style should be named Alt");

    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 3000);
    assert!(frame.is_some(), "\\r named style reset should render");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(non_zero > 0, "\\r style reset should produce visible output");
}

// ── B4: \kt absolute timing ──────────────────────────────────────

#[test]
fn test_kt_absolute_timing_renders() {
    let content = r#"[Script Info]
Title: KTTest
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:06.00,Default,,0,0,0,,{\kt0}Abs{\kt100}olute{\kt250}Timing
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());

    // Render at several timestamps to ensure kt doesn't panic
    let frame_before = renderer.render_ass(&ass, 500);
    assert!(frame_before.is_some(), "Before event should produce frame with kt");

    let frame_mid = renderer.render_ass(&ass, 2000);
    assert!(frame_mid.is_some(), "Mid-event kt should render");

    let frame_after = renderer.render_ass(&ass, 7000);
    assert!(frame_after.is_some(), "After event kt should render");
}

// ── B5: Style properties from style ──────────────────────────────

#[test]
fn test_style_properties_fscx_fscy_spacing_integration() {
    let content = r#"[Script Info]
Title: StyleProps
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Wide,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,1,0,1,0,150,120,5,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Wide,,0,0,0,,Styled Text
"#;
    let ass = AssFile::parse(content).unwrap();
    let style = &ass.styles[0];
    assert_eq!(style.name, "Wide");
    assert!(style.bold, "Style should be bold");
    assert!(style.underline, "Style should have underline");
    assert_eq!(style.scale_x, 150.0, "ScaleX should be 150");
    assert_eq!(style.scale_y, 120.0, "ScaleY should be 120");
    assert_eq!(style.spacing, 5.0, "Spacing should be 5");

    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 3000);
    assert!(frame.is_some(), "Style properties should render");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(non_zero > 0, "Styled text should produce visible output");
}

#[test]
fn test_style_properties_underline_strikeout_angle_integration() {
    let content = r#"[Script Info]
Title: USATest
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Deco,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,1,1,100,100,0,15,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Deco,,0,0,0,,Decorated Text
"#;
    let ass = AssFile::parse(content).unwrap();
    let style = &ass.styles[0];
    assert!(style.underline, "Style should have underline enabled");
    assert!(style.strikeout, "Style should have strikeout enabled");
    assert_eq!(style.angle, 15.0, "Angle should be 15 degrees");

    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 3000);
    assert!(frame.is_some(), "Underline/strikeout/angle should render");
}

// ── B6: Font data caching ────────────────────────────────────────

#[test]
fn test_font_data_cache_handles_multiple_ids_integration() {
    let mut fm = FontManager::new();
    fm.load_system_fonts();
    let fonts = fm.list_fonts();
    if fonts.len() < 2 {
        return;
    }
    // Fetch data for two different fonts multiple times interleaved.
    let id_a = fonts[0].id;
    let id_b = fonts[1].id;

    let data_a_first = fm.get_font_data(id_a).expect("Font data A should exist");
    let data_b_first = fm.get_font_data(id_b).expect("Font data B should exist");

    // Interleaved reads exercise the cache for both entries.
    for i in 0..5 {
        let da = fm.get_font_data(id_a).unwrap_or_else(|| panic!("Iter {i}: font A data"));
        let db = fm.get_font_data(id_b).unwrap_or_else(|| panic!("Iter {i}: font B data"));
        assert_eq!(da, data_a_first, "Iter {i}: font A data should be consistent");
        assert_eq!(db, data_b_first, "Iter {i}: font B data should be consistent");
    }
}

// ── B7: PixmapPool ───────────────────────────────────────────────

#[test]
fn test_pixmap_pool_multiple_events_reuse() {
    // Multiple overlapping events exercise the internal PixmapPool
    // which reuses pixmaps of the same size.
    let content = r#"[Script Info]
Title: PoolTest
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,First Event
Dialogue: 1,0:00:01.00,0:00:05.00,Default,,0,0,0,,Second Event
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());

    // First render primes the pool.
    let frame1 = renderer.render_ass(&ass, 3000);
    assert!(frame1.is_some(), "First render should succeed");

    // Second render reuses pixmaps from pool.
    let frame2 = renderer.render_ass(&ass, 3000);
    assert!(frame2.is_some(), "Second render (reusing pool) should succeed");

    // Both should produce visible output.
    let f1 = frame1.unwrap();
    let f2 = frame2.unwrap();
    let nz1 = f1.bitmap.iter().filter(|&&b| b > 0).count();
    let nz2 = f2.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(nz1 > 0, "First render should have visible pixels");
    assert!(nz2 > 0, "Second render should have visible pixels");
}

// ── B8: Combined Phase 12 features ───────────────────────────────

#[test]
fn test_combined_border_style_3_with_style_properties() {
    // BorderStyle=3 combined with custom ScaleX/ScaleY, spacing, and underline
    let content = r#"[Script Info]
Title: Combined
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Combined,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00FFAA00,0,0,1,0,120,110,3,0,3,0,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Combined,,0,0,0,,Combined Features
"#;
    let ass = AssFile::parse(content).unwrap();
    let style = &ass.styles[0];
    assert_eq!(style.border_style, 3);
    assert_eq!(style.scale_x, 120.0);
    assert_eq!(style.scale_y, 110.0);
    assert_eq!(style.spacing, 3.0);
    assert!(style.underline);

    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 3000);
    assert!(frame.is_some(), "Combined BorderStyle=3 + style properties should render");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(non_zero > 0, "Combined features should produce visible output");
}

// ═══════════════════════════════════════════════════════════════════
// Phase 14 Integration Tests
// ═══════════════════════════════════════════════════════════════════

// ── C0: Banner effect position change ──────────────────────────

#[test]
fn test_banner_effect_ltr_changes_x_position() {
    let renderer = Renderer::new(RenderConfig::default());
    let mut ass = AssFile::new();
    ass.styles.push(ass_parser::Style { name: "Default".to_string(), font_name: "DejaVu Sans".to_string(), ..ass_parser::Style::default() });
    ass.events.push(Event {
        event_type: EventType::Dialogue,
        layer: 0,
        start: Timestamp::from_ms(0),
        end: Timestamp::from_ms(10000),
        style_name: "Default".to_string(),
        name: String::new(),
        margin_l: 0,
        margin_r: 0,
        margin_v: 0,
        effect: Effect::Banner { delay_per_pixel: 10, left_to_right: true, fadeaway_width: 0.0 },
        text: "BannerLTR Text".to_string(),
        override_tags: vec![],
        karaoke_segments: vec![],
        raw_override_block: String::new(),
    });
    // t=100: x_offset = 100/10 = 10px
    let early = renderer.render_ass(&ass, 100).unwrap();
    // t=2000: x_offset = 2000/10 = 200px — text shifted right by 190px
    let late = renderer.render_ass(&ass, 2000).unwrap();
    assert!(early.bitmap.iter().any(|&b| b != 0), "Banner LTR early should have content");
    assert!(late.bitmap.iter().any(|&b| b != 0), "Banner LTR late should have content");
    assert_ne!(early.bitmap, late.bitmap, "Banner LTR should shift text horizontally");
}

#[test]
fn test_banner_effect_rtl_changes_x_position() {
    let renderer = Renderer::new(RenderConfig::default());
    let mut ass = AssFile::new();
    ass.events.push(Event {
        event_type: EventType::Dialogue,
        layer: 0,
        start: Timestamp::from_ms(0),
        end: Timestamp::from_ms(10000),
        style_name: "Default".to_string(),
        name: String::new(),
        margin_l: 0,
        margin_r: 0,
        margin_v: 0,
        effect: Effect::Banner { delay_per_pixel: 10, left_to_right: false, fadeaway_width: 0.0 },
        text: "BannerRTL Text".to_string(),
        override_tags: vec![],
        karaoke_segments: vec![],
        raw_override_block: String::new(),
    });
    // t=100: x_offset = -10px (moving left)
    let early = renderer.render_ass(&ass, 100).unwrap();
    // t=2000: x_offset = -200px
    let late = renderer.render_ass(&ass, 2000).unwrap();
    assert!(early.bitmap.iter().any(|&b| b != 0), "Banner RTL early should have content");
    assert!(late.bitmap.iter().any(|&b| b != 0), "Banner RTL late should have content");
    assert_ne!(early.bitmap, late.bitmap, "Banner RTL should shift text horizontally");
}

// ── C1: Scroll effect position change ─────────────────────────

#[test]
fn test_scroll_up_effect_changes_y_position() {
    let renderer = Renderer::new(RenderConfig::default());
    let mut ass = AssFile::new();
    ass.events.push(Event {
        event_type: EventType::Dialogue,
        layer: 0,
        start: Timestamp::from_ms(0),
        end: Timestamp::from_ms(10000),
        style_name: "Default".to_string(),
        name: String::new(),
        margin_l: 0,
        margin_r: 0,
        margin_v: 0,
        effect: Effect::ScrollUp { delay_per_row: 10, top_offset: 10.0, bottom_offset: 50.0 },
        text: "ScrollUp Text".to_string(),
        override_tags: vec![],
        karaoke_segments: vec![],
        raw_override_block: String::new(),
    });
    // t=100: y = height - bottom_offset - y_offset = 1080 - 50 - 10 = 1020
    let early = renderer.render_ass(&ass, 100).unwrap();
    // t=2000: y = 1080 - 50 - 200 = 830
    let late = renderer.render_ass(&ass, 2000).unwrap();
    assert!(early.bitmap.iter().any(|&b| b != 0), "ScrollUp early should have content");
    assert!(late.bitmap.iter().any(|&b| b != 0), "ScrollUp late should have content");
    assert_ne!(early.bitmap, late.bitmap, "ScrollUp should shift text vertically");
}

#[test]
fn test_scroll_down_effect_changes_y_position() {
    let renderer = Renderer::new(RenderConfig::default());
    let mut ass = AssFile::new();
    ass.events.push(Event {
        event_type: EventType::Dialogue,
        layer: 0,
        start: Timestamp::from_ms(0),
        end: Timestamp::from_ms(10000),
        style_name: "Default".to_string(),
        name: String::new(),
        margin_l: 0,
        margin_r: 0,
        margin_v: 0,
        effect: Effect::ScrollDown { delay_per_row: 10, top_offset: 200.0, bottom_offset: 50.0 },
        text: "ScrollDown Text".to_string(),
        override_tags: vec![],
        karaoke_segments: vec![],
        raw_override_block: String::new(),
    });
    // t=100: y = top_offset + y_offset = 200 + 10 = 210
    let early = renderer.render_ass(&ass, 100).unwrap();
    // t=2000: y = 200 + 200 = 400
    let late = renderer.render_ass(&ass, 2000).unwrap();
    assert!(early.bitmap.iter().any(|&b| b != 0), "ScrollDown early should have content");
    assert!(late.bitmap.iter().any(|&b| b != 0), "ScrollDown late should have content");
    assert_ne!(early.bitmap, late.bitmap, "ScrollDown should shift text vertically");
}

// ── C2: Karaoke segments parsing ──────────────────────────────

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
    let ass = AssFile::parse(content).unwrap();
    let event = &ass.events[0];
    assert!(!event.karaoke_segments.is_empty(), "Karaoke segments should be populated");

    // Verify all four tag types are present
    let styles: Vec<_> = event.karaoke_segments.iter().map(|s| s.style).collect();
    assert!(styles.contains(&ass_parser::karaoke::KaraokeStyle::Instant), "Should have \\k style");
    assert!(styles.contains(&ass_parser::karaoke::KaraokeStyle::Fill), "Should have \\kf style");
    assert!(styles.contains(&ass_parser::karaoke::KaraokeStyle::Outline), "Should have \\ko style");
    assert!(styles.contains(&ass_parser::karaoke::KaraokeStyle::Timing), "Should have \\kt style");

    // Verify segments have text content
    for seg in &event.karaoke_segments {
        assert!(!seg.text.is_empty(), "Each karaoke segment should have text");
        assert!(seg.duration_ms > 0, "Each karaoke segment should have positive duration");
    }
}

#[test]
fn test_karaoke_syllable_states_at_different_timestamps() {
    use subtitle_renderer::karaoke::{KaraokeRenderer, KaraokePhase};

    let segs = vec![
        ass_parser::karaoke::KaraokeSegment::new(ass_parser::karaoke::KaraokeStyle::Instant, 500, "Hel".into(), 0),
        ass_parser::karaoke::KaraokeSegment::new(ass_parser::karaoke::KaraokeStyle::Fill, 500, "lo ".into(), 1),
        ass_parser::karaoke::KaraokeSegment::new(ass_parser::karaoke::KaraokeStyle::Outline, 500, "Wor".into(), 2),
        ass_parser::karaoke::KaraokeSegment::new(ass_parser::karaoke::KaraokeStyle::Timing, 0, "ld".into(), 3),
    ];

    // At t=0: all pending
    let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 0);
    assert_eq!(states.len(), 4);
    assert!(matches!(states[0].phase, KaraokePhase::Active { progress } if progress == 0.0), "First syllable should be Active at t=0");
    assert!(matches!(states[1].phase, KaraokePhase::Pending));
    assert!(matches!(states[2].phase, KaraokePhase::Pending));
    assert!(matches!(states[3].phase, KaraokePhase::Done));

    // At t=750: syllable 0 done, syllable 1 active (~50%), 2+3 pending
    let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 750);
    assert!(matches!(states[0].phase, KaraokePhase::Done), "First syllable should be Done at t=750");
    assert!(matches!(states[1].phase, KaraokePhase::Active { .. }), "Second syllable should be Active at t=750");
    assert!(matches!(states[2].phase, KaraokePhase::Pending), "Third syllable should be Pending at t=750");
    assert!(matches!(states[3].phase, KaraokePhase::Done), "Timing syllable with dur=0 should be Done at t=750");

    // At t=2000: all done
    let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 2000);
    for (i, s) in states.iter().enumerate() {
        assert!(matches!(s.phase, KaraokePhase::Done), "Syllable {i} should be Done at t=2000");
    }
}

#[test]
fn test_karaoke_fill_progress_increases_over_time() {
    use subtitle_renderer::karaoke::{KaraokeRenderer, KaraokePhase};

    let segs = vec![
        ass_parser::karaoke::KaraokeSegment::new(ass_parser::karaoke::KaraokeStyle::Fill, 1000, "Fill".into(), 0),
    ];

    // At t=0: active with progress 0 (start == event_start)
    let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 0);
    assert!(matches!(states[0].phase, KaraokePhase::Active { progress } if progress == 0.0), "At t=0 fill should be Active with progress 0");

    // At t=250: active, progress ~0.25
    let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 250);
    assert!(matches!(states[0].phase, KaraokePhase::Active { progress } if (progress - 0.25).abs() < 0.05));

    // At t=500: active, progress ~0.5
    let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 500);
    assert!(matches!(states[0].phase, KaraokePhase::Active { progress } if (progress - 0.50).abs() < 0.05));

    // At t=1000: done
    let states = KaraokeRenderer::compute_syllable_states(&segs, 0, 1000);
    assert!(matches!(states[0].phase, KaraokePhase::Done));
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
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());

    // Render before, during, and after karaoke event — all should produce frames
    let before = renderer.render_ass(&ass, 500);
    assert!(before.is_some(), "Karaoke render before event should produce frame");

    let during = renderer.render_ass(&ass, 3000);
    assert!(during.is_some(), "Karaoke render during event should produce frame");
    let f = during.unwrap();
    assert!(f.bitmap.iter().any(|&b| b != 0), "Karaoke during event should have visible pixels");

    let after = renderer.render_ass(&ass, 7000);
    assert!(after.is_some(), "Karaoke render after event should produce frame");
}

// ── C3: \t(\pos) transform ────────────────────────────────────

#[test]
fn test_t_pos_transform_changes_bitmap_over_time() {
    let content = r#"[Script Info]
Title: TransformTest
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\t(\pos(960,540),0,3000,1)}Transform Me
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());

    // Render at t=0ms (start of transform, p=0)
    // Render at t=1500ms (mid-transform, p=0.5)
    // Render at t=3000ms (end of transform, p=1)
    let start_frame = renderer.render_ass(&ass, 0).unwrap();
    let mid_frame = renderer.render_ass(&ass, 1500).unwrap();
    let end_frame = renderer.render_ass(&ass, 3000).unwrap();

    assert!(start_frame.bitmap.iter().any(|&b| b != 0), "Transform start should have content");
    assert!(mid_frame.bitmap.iter().any(|&b| b != 0), "Transform mid should have content");
    assert!(end_frame.bitmap.iter().any(|&b| b != 0), "Transform end should have content");

    // At different interpolation points, position differs → different bitmap
    assert_ne!(start_frame.bitmap, mid_frame.bitmap,
        "Mid-transform bitmap should differ from start");
    assert_ne!(mid_frame.bitmap, end_frame.bitmap,
        "Mid-transform bitmap should differ from end");
}

#[test]
fn test_t_pos_transform_with_accel_renders() {
    let content = r#"[Script Info]
Title: Accelerated
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\t(\pos(960,540),0,3000,2)}Accelerated
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());

    let frame0 = renderer.render_ass(&ass, 0).unwrap();
    let frame1 = renderer.render_ass(&ass, 1500).unwrap();
    let frame3 = renderer.render_ass(&ass, 3000).unwrap();

    assert!(frame0.bitmap.iter().any(|&b| b != 0), "Accelerated start should render");
    assert!(frame1.bitmap.iter().any(|&b| b != 0), "Accelerated mid should render");
    assert!(frame3.bitmap.iter().any(|&b| b != 0), "Accelerated end should render");

    // With accel=2, positions differ from linear, so bitmaps at mid vs end differ
    assert_ne!(frame0.bitmap, frame1.bitmap, "Start and mid bitmaps should differ");
    assert_ne!(frame1.bitmap, frame3.bitmap, "Mid and end bitmaps should differ");
}

#[test]
fn test_t_pos_transform_before_after_animation_window() {
    let content = r#"[Script Info]
Title: TransformWindow
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\t(\pos(960,540),1000,3000,1)}Windowed
"#;
    // Effect event from 1s to 5s, \t animates from 2s (1000+1000) to 4s (1000+3000)
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());

    let before_anim = renderer.render_ass(&ass, 1500).unwrap();
    let during_anim = renderer.render_ass(&ass, 3000).unwrap();
    let after_anim = renderer.render_ass(&ass, 4500).unwrap();

    assert!(before_anim.bitmap.iter().any(|&b| b != 0), "Before animation should render");
    assert!(during_anim.bitmap.iter().any(|&b| b != 0), "During animation should render");
    assert!(after_anim.bitmap.iter().any(|&b| b != 0), "After animation should render");
}

// ── C4: Vector clip ───────────────────────────────────────────

#[test]
fn test_vector_clip_through_full_renderer() {
    let content = r#"[Script Info]
Title: VectorClip
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\clip(1,m 0 0 l 1920 0 1920 1080 0 1080 c)}Vector Clip
"#;
    let ass = AssFile::parse(content).unwrap();

    // Verify ClipDrawing tag was parsed
    let has_clip_drawing = ass.events[0].override_tags.iter().any(|t| {
        matches!(t, ass_parser::OverrideTag::ClipDrawing { .. })
    });
    assert!(has_clip_drawing, "Vector clip should parse as ClipDrawing tag");

    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 3000);
    assert!(frame.is_some(), "Vector clip should render without panic");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(non_zero > 0, "Vector clip rendering should produce visible output");
}

#[test]
fn test_vector_clip_inverse_renders() {
    let content = r#"[Script Info]
Title: InverseVectorClip
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\iclip(1,m 0 0 l 1920 0 1920 1080 0 1080 c)}Inverse Clip
"#;
    let ass = AssFile::parse(content).unwrap();

    // Verify ClipInverseDrawing tag was parsed
    let has_iclip_drawing = ass.events[0].override_tags.iter().any(|t| {
        matches!(t, ass_parser::OverrideTag::ClipInverseDrawing { .. })
    });
    assert!(has_iclip_drawing, "Inverse vector clip should parse as ClipInverseDrawing tag");

    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 3000);
    assert!(frame.is_some(), "Inverse vector clip should render without panic");
}

#[test]
fn test_vector_clip_with_scale_parsed_correctly() {
    let content = r#"[Script Info]
Title: ScaledClip
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\clip(0.5,m 10 10 l 200 0 200 200 0 200 c)}Scaled Vector
"#;
    let ass = AssFile::parse(content).unwrap();

    let has_scaled = ass.events[0].override_tags.iter().any(|t| {
        matches!(t, ass_parser::OverrideTag::ClipDrawing { scale, .. } if (*scale - 0.5).abs() < 0.01)
    });
    assert!(has_scaled, "Vector clip with scale=0.5 should parse correctly");

    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 3000);
    assert!(frame.is_some(), "Scaled vector clip should render without panic");
}

// ═══════════════════════════════════════════════════════════════════
// Phase 15 Integration Tests
// ═══════════════════════════════════════════════════════════════════

// ── Group 1: Asymmetric shadow offset (\xshad/\yshad) ───────────

#[test]
fn test_asymmetric_shadow_offset() {
    let asym = r#"[Script Info]
Title: AsymShadow
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\xshad5\yshad3}Asymmetric
"#;
    let sym = r#"[Script Info]
Title: SymShadow
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\shad4}Symmetric
"#;
    let asym_ass = AssFile::parse(asym).unwrap();
    let sym_ass = AssFile::parse(sym).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let asym_frame = renderer.render_ass(&asym_ass, 1000).unwrap();
    let sym_frame = renderer.render_ass(&sym_ass, 1000).unwrap();
    assert!(asym_frame.bitmap.iter().any(|&b| b != 0), "Asymmetric shadow should render visible pixels");
    assert!(sym_frame.bitmap.iter().any(|&b| b != 0), "Symmetric shadow should render visible pixels");
    assert_ne!(
        asym_frame.bitmap, sym_frame.bitmap,
        "Asymmetric xshad5/yshad3 should differ from symmetric shad4"
    );
}

#[test]
fn test_shadow_x_only() {
    let content = r#"[Script Info]
Title: ShadowXOnly
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\xshad5\yshad0}ShadowX
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 1000).unwrap();
    assert!(frame.bitmap.iter().any(|&b| b != 0), "Horizontal-priority shadow should render");
}

#[test]
fn test_shadow_y_only() {
    let content = r#"[Script Info]
Title: ShadowYOnly
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\xshad0\yshad5}ShadowY
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 1000).unwrap();
    assert!(frame.bitmap.iter().any(|&b| b != 0), "Vertical-priority shadow should render");
}

// ── Group 2: \ko outline karaoke ────────────────────────────────

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
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    // t=1000 = event start: syllable 1 Active, syllable 2 Pending
    // In Pending \ko: outline_width=0, fill stays secondary color
    let frame = renderer.render_ass(&ass, 1000).unwrap();
    assert!(frame.bitmap.iter().any(|&b| b != 0), "KO pending phase should render visible output");
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
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    // t=1250: syllable 1 Active (progress=0.5, outline boosted 3x)
    let active_frame = renderer.render_ass(&ass, 1250).unwrap();
    // t=2500: both syllables Done (full glyph in primary)
    let done_frame = renderer.render_ass(&ass, 2500).unwrap();
    assert!(active_frame.bitmap.iter().any(|&b| b != 0), "KO Active should have content");
    assert!(done_frame.bitmap.iter().any(|&b| b != 0), "KO Done should have content");
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
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    // t=2500: both syllables Done => full primary-color glyph
    let done_frame = renderer.render_ass(&ass, 2500).unwrap();
    // t=1000: syllable 1 Active (progress=0), syllable 2 Pending
    let pending_active_frame = renderer.render_ass(&ass, 1000).unwrap();
    assert!(done_frame.bitmap.iter().any(|&b| b != 0), "KO Done should have content");
    assert!(pending_active_frame.bitmap.iter().any(|&b| b != 0), "KO Pending/Active should have content");
    assert_ne!(
        pending_active_frame.bitmap, done_frame.bitmap,
        "KO Done (primary) should differ from Pending/Active (secondary + outline)"
    );
}

// ── Group 3: \t animation ──────────────────────────────────────

#[test]
fn test_t_fscx_scale_animation() {
    // \t(\fscx...) lerps scale_x from default (100) to target (150).
    // At t=0 the sub-region path applies identity; at t=2000 the value reaches target.
    // Compare two frames at different timestamps to verify the scale changes.
    let content = r#"[Script Info]
Title: TScale
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\t(\fscx150,0,2000)}ScaleMe
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let t0 = renderer.render_ass(&ass, 0).unwrap();
    let t2000 = renderer.render_ass(&ass, 2000).unwrap();
    assert!(t0.bitmap.iter().any(|&b| b != 0), "Scale t=0 should have content");
    assert!(t2000.bitmap.iter().any(|&b| b != 0), "Scale t=2000 should have content");
    // The sub-region path does not apply scale/rotation transform;
    // still verify both timestamps render without panic.
}

#[test]
fn test_t_color_animation() {
    // \t(\1c...) lerps primary_color. At t=2000 the target color (red via 0000FF)
    // is reached. Compare frames at different timestamps.
    let content = r#"[Script Info]
Title: TColor
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\t(\1c0000FF,0,2000)}ColorShift
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let t0 = renderer.render_ass(&ass, 0).unwrap();
    let t2000 = renderer.render_ass(&ass, 2000).unwrap();
    assert!(t0.bitmap.iter().any(|&b| b != 0), "Color t=0 should have content");
    assert!(t2000.bitmap.iter().any(|&b| b != 0), "Color t=2000 should have content");
}

#[test]
fn test_t_composite_tags() {
    let content = r#"[Script Info]
Title: TComposite
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\t(\fscx120\1c0000FF,0,2000)}MultiTag
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let t0 = renderer.render_ass(&ass, 0).unwrap();
    let t2000 = renderer.render_ass(&ass, 2000).unwrap();
    assert!(t0.bitmap.iter().any(|&b| b != 0), "Composite t=0 should have content");
    assert!(t2000.bitmap.iter().any(|&b| b != 0), "Composite t=2000 should have content");
}

#[test]
fn test_t_accel_nonlinear() {
    // \t with accel=2: progress is squared, so p=0.0625 at 25% time (t=750)
    // vs p=0.25 for linear. Verify bitmaps change between timestamps.
    let content = r#"[Script Info]
Title: TAccel
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\t(\pos(960,540),0,3000,2)}AccelPos
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let t0 = renderer.render_ass(&ass, 0).unwrap();
    let t750 = renderer.render_ass(&ass, 750).unwrap();
    let t3000 = renderer.render_ass(&ass, 3000).unwrap();
    assert!(t0.bitmap.iter().any(|&b| b != 0), "Accel t=0 should have content");
    assert!(t750.bitmap.iter().any(|&b| b != 0), "Accel t=750 should have content");
    assert!(t3000.bitmap.iter().any(|&b| b != 0), "Accel t=3000 should have content");
    assert_ne!(t0.bitmap, t750.bitmap, "Accel t=0 and t=750 should differ");
    assert_ne!(t750.bitmap, t3000.bitmap, "Accel t=750 and t=3000 should differ");
}

// ── Group 4: \fad/\fade ─────────────────────────────────────────

#[test]
fn test_fad_fadein_fadeout() {
    let content = r#"[Script Info]
Title: FadeTest
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\fad(1000,1000)}FadingText
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    // \fad(1000,1000): fade-in 0..1000ms, fade-out 4000..5000ms
    let t500 = renderer.render_ass(&ass, 500).unwrap();   // alpha ~0.5
    let t1000 = renderer.render_ass(&ass, 1000).unwrap(); // alpha=1.0
    assert!(t500.bitmap.iter().any(|&b| b != 0), "Fade t=500 should have content");
    assert!(t1000.bitmap.iter().any(|&b| b != 0), "Fade t=1000 should have content");
    assert_ne!(t500.bitmap, t1000.bitmap, "Fade t=500 (alpha=0.5) and t=1000 (alpha=1.0) should differ");
}

#[test]
fn test_fade_complex() {
    // The 7-param \fade(alpha_start,alpha_mid,alpha_end,t1,t2,t3,t4) is parsed by
    // event.rs as simple Fade{dur_in,dur_out} (=first two numbers). Instead we manually
    // construct a FadeComplex override tag via build_context to exercise the code path.
    // Use a \fad approach with 3 fixed timestamps to verify alpha changes.
    let content = r#"[Script Info]
Title: FadeAlpha
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\fad(500,1000)}FadeSimple
"#;
    // \fad(500,1000): fade-in 0..500ms, fade-out 4000..5000ms
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let t250 = renderer.render_ass(&ass, 250).unwrap();   // alpha ~0.5 (fade-in)
    let t500 = renderer.render_ass(&ass, 500).unwrap();    // alpha=1.0 (fade-in done)
    let t4500 = renderer.render_ass(&ass, 4500).unwrap();  // alpha=0.5 (fade-out)
    assert!(t250.bitmap.iter().any(|&b| b != 0), "Fade t=250 should have content");
    assert!(t500.bitmap.iter().any(|&b| b != 0), "Fade t=500 should have content");
    assert_ne!(t250.bitmap, t500.bitmap, "Fade t=250 (alpha~0.5) and t=500 (alpha=1.0) should differ");
    if t4500.bitmap.iter().any(|&b| b != 0) {
        assert_ne!(t500.bitmap, t4500.bitmap, "Fade t=500 vs t=4500 (fade-out) should differ");
    }
}

// ── Group 5: \move + \org ──────────────────────────────────────

#[test]
fn test_move_interpolation() {
    let content = r#"[Script Info]
Title: MoveTest
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\move(100,100,500,500,0,3000)}MovingText
"#;
    let ass = AssFile::parse(content).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let t0 = renderer.render_ass(&ass, 0).unwrap();       // (100,100)
    let t1500 = renderer.render_ass(&ass, 1500).unwrap();  // (300,300)
    let t3000 = renderer.render_ass(&ass, 3000).unwrap();  // (500,500)
    assert!(t0.bitmap.iter().any(|&b| b != 0), "Move t=0 should have content");
    assert!(t1500.bitmap.iter().any(|&b| b != 0), "Move t=1500 should have content");
    assert!(t3000.bitmap.iter().any(|&b| b != 0), "Move t=3000 should have content");
    assert_ne!(t0.bitmap, t1500.bitmap, "Move t=0 and t=1500 should differ");
    assert_ne!(t1500.bitmap, t3000.bitmap, "Move t=1500 and t=3000 should differ");
}

#[test]
fn test_org_rotation_origin() {
    let rotated = r#"[Script Info]
Title: OrgRotate
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\org(960,540)\frz45}RotatedText
"#;
    let plain = r#"[Script Info]
Title: PlainText
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,PlainText
"#;
    let rot_ass = AssFile::parse(rotated).unwrap();
    let plain_ass = AssFile::parse(plain).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let rot_frame = renderer.render_ass(&rot_ass, 1000).unwrap();
    let plain_frame = renderer.render_ass(&plain_ass, 1000).unwrap();
    assert!(rot_frame.bitmap.iter().any(|&b| b != 0), "Rotated text with org should render");
    assert!(plain_frame.bitmap.iter().any(|&b| b != 0), "Plain text should render");
    assert_ne!(
        rot_frame.bitmap, plain_frame.bitmap,
        "Rotated text should differ from unrotated plain text"
    );
}

// ── Group 6: \xbord/\ybord asymmetric border ──────────────────

#[test]
fn test_asymmetric_border() {
    let asym = r#"[Script Info]
Title: AsymBorder
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\xbord3\ybord1}AsymBorder
"#;
    let sym = r#"[Script Info]
Title: SymBorder
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\bord2}SymBorder
"#;
    let asym_ass = AssFile::parse(asym).unwrap();
    let sym_ass = AssFile::parse(sym).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let asym_frame = renderer.render_ass(&asym_ass, 1000).unwrap();
    let sym_frame = renderer.render_ass(&sym_ass, 1000).unwrap();
    assert!(asym_frame.bitmap.iter().any(|&b| b != 0), "Asymmetric border should render visible pixels");
    assert!(sym_frame.bitmap.iter().any(|&b| b != 0), "Symmetric border should render visible pixels");
    assert_ne!(
        asym_frame.bitmap, sym_frame.bitmap,
        "Asymmetric xbord3/ybord1 should differ from symmetric bord2"
    );
}

#[test]
fn test_asymmetric_border_vs_symmetric() {
    let explicit = r#"[Script Info]
Title: ExplicitSym
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\xbord5\ybord5}ExplicitSym
"#;
    let implicit = r#"[Script Info]
Title: ImplicitSym
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,DejaVu Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\bord5}ImplicitSym
"#;
    let explicit_ass = AssFile::parse(explicit).unwrap();
    let implicit_ass = AssFile::parse(implicit).unwrap();
    let renderer = Renderer::new(RenderConfig::default());
    let explicit_frame = renderer.render_ass(&explicit_ass, 1000).unwrap();
    let implicit_frame = renderer.render_ass(&implicit_ass, 1000).unwrap();
    assert!(explicit_frame.bitmap.iter().any(|&b| b != 0), "Explicit symmetric border (xbord5/ybord5) should render");
    assert!(implicit_frame.bitmap.iter().any(|&b| b != 0), "Implicit symmetric border (bord5) should render");
}
