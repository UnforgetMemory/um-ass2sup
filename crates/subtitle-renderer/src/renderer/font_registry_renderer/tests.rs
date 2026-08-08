//! Unit tests for `render_event_font_registry` and font resolution,
//! kept in a separate file so the production renderer stays focused.

use std::sync::Arc;

use super::*;
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

/// Resolve via the shared cache for a fixed query, counting cache misses.
fn cached_resolve(
    resources: &FontRegistryRenderResources,
    reg: &FontRegistry,
    flag: bool,
    miss_count: &mut usize,
) -> Arc<[u8]> {
    resolve_font_data_cached(resources, "DejaVu Sans", false, "Default", flag, || {
        *miss_count += 1;
        resolve_font_data_inner(reg, "DejaVu Sans", false, None, "Default", flag)
    })
}

#[test]
fn resolve_font_data_cached_parity_and_flag_key_separation() {
    // Parity: the shared cache must return byte-identical data to a direct
    // (uncached) fallback-chain resolution — for BOTH bold-upgrade flags.
    // Key separation: layout (bold_upgrade=true) and render (false) resolve
    // differently, so they must not share a cache entry; this is what the
    // bold-upgrade dimension in the cache key guarantees.
    if !has_dejavu() {
        eprintln!("SKIP: no DejaVu Sans font found");
        return;
    }
    let resources = FontRegistryRenderResources::new();
    {
        let mut reg = resources.registry.lock();
        reg.load_system_fonts();
    }
    let reg = resources.registry.lock();
    let direct_true = resolve_font_data_inner(&reg, "DejaVu Sans", false, None, "Default", true);
    let direct_false = resolve_font_data_inner(&reg, "DejaVu Sans", false, None, "Default", false);

    let mut miss_count = 0usize;
    let cached_true_1 = cached_resolve(&resources, &reg, true, &mut miss_count);
    assert_eq!(miss_count, 1, "first bold_upgrade=true resolve must miss");
    let cached_false = cached_resolve(&resources, &reg, false, &mut miss_count);
    assert_eq!(
        miss_count, 2,
        "bold_upgrade=false must use a distinct cache key"
    );
    let cached_true_2 = cached_resolve(&resources, &reg, true, &mut miss_count);
    assert_eq!(miss_count, 2, "bold_upgrade=true key must now hit");
    let _ = cached_resolve(&resources, &reg, false, &mut miss_count);
    assert_eq!(miss_count, 2, "bold_upgrade=false key must now hit");

    // Cached results are byte-identical to the direct fallback-chain result.
    assert_eq!(cached_true_1.as_ref(), direct_true.as_ref());
    assert_eq!(cached_false.as_ref(), direct_false.as_ref());
    // A hit returns the same stored Arc (cheap clone, no re-resolution).
    assert!(Arc::ptr_eq(&cached_true_1, &cached_true_2));
}
