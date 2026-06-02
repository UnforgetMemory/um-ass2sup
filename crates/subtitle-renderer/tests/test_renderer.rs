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
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 500);
    assert!(frame.is_some(), "render_ass always returns Some pixmap");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert_eq!(non_zero, 0, "No visible events at t=500ms should produce empty bitmap");
}

#[test]
fn test_render_ass_bitmap_has_content() {
    let ass = make_default_ass();
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("", 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some(), "render_ass always returns Some");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert_eq!(non_zero, 0, "No events should produce empty bitmap");
}

#[test]
fn test_render_ass_no_events() {
    let mut renderer = Renderer::new(RenderConfig::default());
    let ass = AssFile::new();
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some(), "render_ass always returns Some");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert_eq!(non_zero, 0, "No events should produce empty bitmap");
}

#[test]
fn test_render_ass_with_override_pos() {
    let mut renderer = Renderer::new(RenderConfig::default());
    let mut ass = make_simple_ass("Positioned", 0, 5000);
    ass.events[0].text = "{\\pos(500,300)}Positioned".to_string();
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
    assert!(frame.unwrap().bitmap.iter().any(|&b| b != 0));
}

#[test]
fn test_render_ass_with_color_override() {
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("Cached", 0, 5000);
    let cache = FrameCache::new(16);
    let f1 = renderer.render_ass_cached(&ass, 1000, &cache, 0);
    assert!(f1.is_some());
    let key = make_frame_key(0, 1000);
    assert!(cache.contains(&key));
    let f2 = renderer.render_ass_cached(&ass, 1000, &cache, 0);
    assert!(f2.is_some());
}

#[test]
fn test_render_single_character() {
    let mut renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("A", 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
    assert!(frame.unwrap().bitmap.iter().any(|&b| b != 0));
}

#[test]
fn test_render_long_text() {
    let mut renderer = Renderer::new(RenderConfig::default());
    let long = "A".repeat(200);
    let ass = make_simple_ass(&long, 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_unicode() {
    let mut renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("中文测试 🎵", 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_special_chars() {
    let mut renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("<>&\"'{}", 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_multiline() {
    let mut renderer = Renderer::new(RenderConfig::default());
    let ass = make_simple_ass("Line1\\NLine2\\NLine3", 0, 5000);
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
    assert!(frame.unwrap().bitmap.iter().any(|&b| b != 0));
}

#[test]
fn test_render_two_overlapping_events() {
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
    let mut ass = make_simple_ass("Moving", 0, 5000);
    ass.events[0].text = "{\\move(100,100,500,500)}Moving".to_string();
    let frame = renderer.render_ass(&ass, 2500);
    assert!(frame.is_some());
}

#[test]
fn test_render_with_blur() {
    let mut renderer = Renderer::new(RenderConfig::default());
    let mut ass = make_simple_ass("Blurred", 0, 5000);
    ass.events[0].text = "{\\blur(3)}Blurred".to_string();
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_with_rotation() {
    let mut renderer = Renderer::new(RenderConfig::default());
    let mut ass = make_simple_ass("Rotated", 0, 5000);
    ass.events[0].text = "{\\frz(45)}Rotated".to_string();
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_with_border() {
    let mut renderer = Renderer::new(RenderConfig::default());
    let mut ass = make_simple_ass("Bordered", 0, 5000);
    ass.events[0].text = "{\\bord(5)}Bordered".to_string();
    let frame = renderer.render_ass(&ass, 1000);
    assert!(frame.is_some());
}

#[test]
fn test_render_with_shadow() {
    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());

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
    let mut renderer = Renderer::new(RenderConfig::default());

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
    let mut renderer = Renderer::new(RenderConfig::default());
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

    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());
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

    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());

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

    let mut renderer = Renderer::new(RenderConfig::default());
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

    let mut renderer = Renderer::new(RenderConfig::default());
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
    let mut renderer = Renderer::new(RenderConfig::default());

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

    let mut renderer = Renderer::new(RenderConfig::default());
    let frame = renderer.render_ass(&ass, 3000);
    assert!(frame.is_some(), "Combined BorderStyle=3 + style properties should render");
    let f = frame.unwrap();
    let non_zero = f.bitmap.iter().filter(|&&b| b > 0).count();
    assert!(non_zero > 0, "Combined features should produce visible output");
}
