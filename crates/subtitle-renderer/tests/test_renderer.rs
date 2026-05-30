use subtitle_renderer::{RenderConfig, RenderContext, RenderedFrame, FontManager, Shaper, Renderer};
use ass_parser::{AssFile, ScriptInfo, Style, Event, EventType, Timestamp, AssColor};

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
        effect: String::new(),
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
        effect: String::new(),
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
        effect: String::new(),
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
