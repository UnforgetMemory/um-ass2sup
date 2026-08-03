//! Unit tests for `render_event_font_registry` and font resolution,
//! kept in a separate file so the production renderer stays focused.

use crate::font::registry::FontRegistry;
use crate::font::types::{FontQuery, FontStyle, FontWeight};

fn dejavu_path() -> &'static std::path::Path {
    std::path::Path::new("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
}

fn has_dejavu() -> bool {
    dejavu_path().exists()
}

#[test]
fn font_data_cache_roundtrip() {
    // Verify that font data loaded into the registry is retrievable
    // via get_font_data — the cache that resolve_glyph_font_data depends on.
    if !has_dejavu() {
        eprintln!("SKIP: no DejaVu Sans font found");
        return;
    }
    let data = std::fs::read(dejavu_path()).expect("read DejaVuSans.ttf");
    let mut registry = FontRegistry::new();
    let id = registry
        .load_user_font_data(data.clone())
        .expect("load font");

    let cached = registry.get_font_data(id);
    assert!(
        cached.is_some(),
        "get_font_data should return Some for loaded font"
    );
    assert!(
        !cached.unwrap().is_empty(),
        "cached font data should not be empty"
    );
}

#[test]
fn font_data_cache_nonexistent_id() {
    // get_font_data should return None for an invalid font ID.
    let registry = FontRegistry::new();
    let invalid_id = crate::font::types::FontId(9999);
    let cached = registry.get_font_data(invalid_id);
    assert!(
        cached.is_none(),
        "get_font_data should return None for non-existent ID"
    );
}

#[test]
fn font_data_cache_exact_match_then_fallback() {
    // Simulate the resolve_glyph_font_data cache path: load a font,
    // query by name, then retrieve cached data via the found ID.
    if !has_dejavu() {
        eprintln!("SKIP: no DejaVu Sans font found");
        return;
    }
    let raw_data = std::fs::read(dejavu_path()).expect("read DejaVuSans.ttf");
    let mut registry = FontRegistry::new();
    registry.load_user_font_data(raw_data).expect("load font");

    let q = FontQuery {
        family: "DejaVu Sans".into(),
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
    };
    let result = registry.query(&q);
    assert!(result.found.is_some(), "DejaVu Sans Normal should be found");

    let cached = registry.get_font_data(result.found.unwrap());
    assert!(cached.is_some(), "cached data for DejaVu Sans should exist");
}
