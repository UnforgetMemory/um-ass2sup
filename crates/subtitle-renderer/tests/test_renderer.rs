use subtitle_renderer::{RenderConfig, RenderContext, RenderedFrame, FontManager, Shaper};

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
