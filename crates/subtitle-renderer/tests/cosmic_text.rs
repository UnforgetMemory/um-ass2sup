//! Unit tests for the cosmic-text backend (FontCosmicResolver, CosmicShaper, rasterizer).

mod common;

use cosmic_text::{FontSystem, SwashCache};
use subtitle_renderer::{
    cosmic::rasterizer::rasterize_cosmic_glyph,
    cosmic::resolver::FontCosmicResolver,
    cosmic::shaper::{CosmicShapedGlyph, CosmicShaper},
    RenderContext,
};
use tiny_skia::Pixmap;

// ── shaper.rs ──────────────────────────────────────────────────

#[test]
fn shaper_basic_latin_returns_glyphs() {
    let mut fs = FontSystem::new();
    let glyphs = CosmicShaper::shape("Hello", &mut fs, 48.0, "", false, false);
    assert!(
        !glyphs.is_empty(),
        "expected at least one shaped glyph for latin text"
    );
    for g in &glyphs {
        validate_glyph(g);
    }
}

#[test]
fn shaper_cjk_does_not_panic() {
    let mut fs = FontSystem::new();
    // CJK shaping depends on available system fonts; we only require that
    // the call does not panic and returns a vector.
    let glyphs = CosmicShaper::shape("中文", &mut fs, 48.0, "", false, false);
    let _ = glyphs;
}

#[test]
fn shaper_empty_text_returns_empty() {
    let mut fs = FontSystem::new();
    let glyphs = CosmicShaper::shape("", &mut fs, 48.0, "", false, false);
    assert!(glyphs.is_empty(), "empty input must yield no glyphs");
}

#[test]
fn shaper_with_font_name_returns_glyphs() {
    let mut fs = FontSystem::new();
    let glyphs = CosmicShaper::shape("Test", &mut fs, 48.0, "Arial", false, false);
    assert!(
        !glyphs.is_empty(),
        "expected shaped glyphs when a font family is specified"
    );
}

#[test]
fn shaper_idempotent_same_input_same_output() {
    let mut fs = FontSystem::new();
    let first = CosmicShaper::shape("Idempotent", &mut fs, 48.0, "", false, false);
    let second = CosmicShaper::shape("Idempotent", &mut fs, 48.0, "", false, false);
    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.glyph_id, b.glyph_id, "glyph_id must be stable");
        assert_eq!(a.x_advance, b.x_advance, "x_advance must be stable");
        assert_eq!(a.y_advance, b.y_advance, "y_advance must be stable");
        assert_eq!(a.x_offset, b.x_offset, "x_offset must be stable");
        assert_eq!(a.y_offset, b.y_offset, "y_offset must be stable");
        assert_eq!(a.font_id, b.font_id, "font_id must be stable");
    }
}

// ── resolver.rs ────────────────────────────────────────────────

#[test]
fn resolver_new_loads_system_fonts() {
    let resolver = FontCosmicResolver::new();
    assert!(
        resolver.font_count() > 0,
        "FontCosmicResolver::new() should load system fonts"
    );
}

#[test]
fn resolver_load_font_data_does_not_panic() {
    let resolver = FontCosmicResolver::new();
    // Empty/invalid data should not panic.
    let id = resolver.load_font_data(vec![]);
    let _ = id;
}

#[test]
fn resolver_resolve_font_common_family() {
    let resolver = FontCosmicResolver::new();
    // Query a commonly available font family. The result is platform-dependent
    // so we accept either Some or None without asserting.
    let _ = resolver.resolve_font("Arial", false, false);
}

#[test]
fn resolver_font_count_nonzero_after_new() {
    let resolver = FontCosmicResolver::new();
    let count = resolver.font_count();
    assert!(
        count > 0,
        "expected at least one font face after loading system fonts, got {count}"
    );
}

// ── rasterizer.rs ──────────────────────────────────────────────

#[test]
fn rasterizer_smoke_no_panic() {
    let mut fs = FontSystem::new();
    let mut swash = SwashCache::new();
    let glyphs = CosmicShaper::shape("A", &mut fs, 48.0, "Arial", false, false);
    if glyphs.is_empty() {
        // No font available for "A" — skip rather than fail in minimal envs.
        return;
    }
    let glyph = &glyphs[0];
    let mut pixmap =
        Pixmap::new(64, 64).expect("failed to allocate 64x64 pixmap for rasterizer smoke test");
    let ctx = RenderContext::default();
    rasterize_cosmic_glyph(&mut pixmap, &mut fs, &mut swash, glyph, 0.0, 0.0, &ctx);
}

// ── helpers ────────────────────────────────────────────────────

fn validate_glyph(g: &CosmicShapedGlyph) {
    // Basic sanity checks; we do not assert exact metrics because they vary
    // across platforms and font versions.
    assert!(g.x_advance.is_finite(), "x_advance must be finite");
    assert!(g.y_advance.is_finite(), "y_advance must be finite");
    assert!(g.x_offset.is_finite(), "x_offset must be finite");
    assert!(g.y_offset.is_finite(), "y_offset must be finite");
}
