//! Benchmarks for the font subsystem: FontRegistry queries, glyph shaping,
//! rasterization, and full fallback-chain resolution.
//!
//! All benches use system DejaVu fonts (fonts-dejavu-core). When no fonts are
//! available the individual bench skips with an `eprintln!` instead of
//! panicking.

use std::collections::HashMap;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use subtitle_renderer::font::rasterizer::GlyphRasterizer;
use subtitle_renderer::font::registry::FontRegistry;
use subtitle_renderer::font::shaper::SimpleShaper;
use subtitle_renderer::font::{FontQuery, FontStyle, FontWeight};
use subtitle_renderer::parse_font_name;
use swash::FontRef;

/// Build a registry loaded with system fonts, or `None` to skip benches.
fn setup_registry() -> Option<FontRegistry> {
    let mut registry = FontRegistry::new();
    if registry.load_system_fonts() == 0 {
        eprintln!("SKIP: no system fonts found (install fonts-dejavu-core)");
        return None;
    }
    Some(registry)
}

/// Resolve `family` to font data (as `Arc<[u8]>` when hit) walking the full
/// fallback chain — exact match → suggestion → `parse_font_name` decomposition
/// → font_map → first available family. Mirrors the pub(crate)
/// `renderer::font_resolve::resolve_font_data_inner` through the public API.
fn resolve_full_chain(
    registry: &FontRegistry,
    family: &str,
    font_map: &HashMap<String, Vec<String>>,
    style_name: &str,
) -> Option<std::sync::Arc<[u8]>> {
    let normal = FontWeight::Normal;
    // Step 1: exact match + suggestion
    let q = FontQuery {
        family: family.to_string(),
        weight: normal,
        style: FontStyle::Normal,
    };
    let result = registry.query(&q);
    if let Some(id) = result.found {
        return registry.get_font_data_arc(id);
    }
    if let Some(sug) = result.suggestion {
        return registry.get_font_data_arc(sug.id);
    }

    // Step 2: parse_font_name decomposition (e.g. "MiSans Demibold")
    if let Some((parsed_family, parsed_weight)) = parse_font_name(family) {
        let pq = FontQuery {
            family: parsed_family,
            weight: parsed_weight,
            style: FontStyle::Normal,
        };
        let pr = registry.query(&pq);
        if let Some(id) = pr.found {
            return registry.get_font_data_arc(id);
        }
        if let Some(sug) = pr.suggestion {
            return registry.get_font_data_arc(sug.id);
        }
    }

    // Step 3: font_map fallback (per-style, then Default)
    if let Some(fallbacks) = font_map.get(style_name).or_else(|| font_map.get("Default")) {
        for fb in fallbacks {
            if fb == family {
                continue;
            }
            let fq = FontQuery {
                family: fb.clone(),
                weight: normal,
                style: FontStyle::Normal,
            };
            let fr = registry.query(&fq);
            if let Some(id) = fr.found {
                return registry.get_font_data_arc(id);
            }
            if let Some(sug) = fr.suggestion {
                return registry.get_font_data_arc(sug.id);
            }
        }
    }

    // Step 4: last resort — first available family
    for candidate in registry.list_families() {
        let fq = FontQuery {
            family: candidate,
            weight: normal,
            style: FontStyle::Normal,
        };
        if let Some(id) = registry.query(&fq).found {
            return registry.get_font_data_arc(id);
        }
    }
    None
}

fn bench_query_hit(c: &mut Criterion) {
    let Some(registry) = setup_registry() else {
        return;
    };
    let q = FontQuery {
        family: "DejaVu Sans".into(),
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
    };
    if registry.query(&q).found.is_none() {
        eprintln!("SKIP: DejaVu Sans not found in system fonts");
        return;
    }
    c.bench_function("font_query_hit_dejavu_sans", |b| {
        b.iter(|| black_box(registry.query(black_box(&q))))
    });
}

fn bench_query_miss(c: &mut Criterion) {
    let Some(registry) = setup_registry() else {
        return;
    };
    let q = FontQuery {
        family: "CompletelyFakeFont12345".into(),
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
    };
    c.bench_function("font_query_miss_unknown_family", |b| {
        b.iter(|| black_box(registry.query(black_box(&q))))
    });
}

fn bench_shape_latin(c: &mut Criterion) {
    let Some(registry) = setup_registry() else {
        return;
    };
    let q = FontQuery {
        family: "DejaVu Sans".into(),
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
    };
    let Some(font_id) = registry.query(&q).found else {
        eprintln!("SKIP: DejaVu Sans not found in system fonts");
        return;
    };
    let Some(font_data) = registry.get_font_data_arc(font_id) else {
        return;
    };
    let text = "The quick brown fox jumps over the lazy dog. Pack my box with \
        five dozen liquor jugs. How vexingly quick daft zebras jump. \
        Sphinx of black quartz, judge my vow.";
    let font_bytes: &[u8] = &font_data;
    if SimpleShaper::shape(text, font_bytes, 48.0).is_err() {
        eprintln!("SKIP: shaping failed");
        return;
    }
    c.bench_function("shape_latin_40_words_48px", |b| {
        b.iter(|| {
            let _ = black_box(SimpleShaper::shape(
                black_box(text),
                black_box(font_bytes),
                48.0,
            ));
        })
    });
}

fn bench_shape_cjk(c: &mut Criterion) {
    let Some(registry) = setup_registry() else {
        return;
    };
    let q = FontQuery {
        family: "DejaVu Sans".into(),
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
    };
    let Some(font_id) = registry.query(&q).found else {
        eprintln!("SKIP: DejaVu Sans not found in system fonts");
        return;
    };
    let Some(font_data) = registry.get_font_data_arc(font_id) else {
        return;
    };
    // Real subtitle line from .localref/ movie fixtures. DejaVu Sans has no CJK
    // coverage, so glyphs are unmapped and skipped — the measured work is the
    // per-character charmap lookup over the CJK run.
    let text = "第50届锡切斯国际奇幻电影节 轨道奖获得者 \
        每年在西班牙锡切斯举行 专门展映奇幻 恐怖和科幻类电影";
    let font_bytes: &[u8] = &font_data;
    if SimpleShaper::shape(text, font_bytes, 48.0).is_err() {
        eprintln!("SKIP: shaping failed");
        return;
    }
    c.bench_function("shape_cjk_40_chars_48px", |b| {
        b.iter(|| {
            let _ = black_box(SimpleShaper::shape(
                black_box(text),
                black_box(font_bytes),
                48.0,
            ));
        })
    });
}

fn bench_rasterize(c: &mut Criterion) {
    let Some(registry) = setup_registry() else {
        return;
    };
    let q = FontQuery {
        family: "DejaVu Sans".into(),
        weight: FontWeight::Normal,
        style: FontStyle::Normal,
    };
    let Some(font_id) = registry.query(&q).found else {
        eprintln!("SKIP: DejaVu Sans not found in system fonts");
        return;
    };
    let Some(font_data) = registry.get_font_data_arc(font_id) else {
        return;
    };
    let Some(font_ref) = FontRef::from_index(&font_data, 0) else {
        eprintln!("SKIP: could not parse DejaVu Sans data");
        return;
    };
    let glyph_id = font_ref.charmap().map('A');
    let font_bytes: &[u8] = &font_data;
    if GlyphRasterizer::rasterize(font_bytes, glyph_id, 48.0).is_err() {
        eprintln!("SKIP: rasterization failed");
        return;
    }
    c.bench_function("glyph_rasterize_A_48px", |b| {
        b.iter(|| {
            let _ = black_box(GlyphRasterizer::rasterize(
                black_box(font_bytes),
                black_box(glyph_id),
                48.0,
            ));
        })
    });
}

fn bench_full_fallback_chain(c: &mut Criterion) {
    let Some(registry) = setup_registry() else {
        return;
    };
    // A family that misses every step until the last-resort family scan, so the
    // whole chain (exact → suggestion → parse_font_name → font_map → last
    // resort) is walked on every iteration.
    let font_map: HashMap<String, Vec<String>> =
        [("Default".to_string(), vec!["AlsoMissingFont".to_string()])]
            .into_iter()
            .collect();
    if resolve_full_chain(&registry, "CompletelyMissingFont", &font_map, "Default").is_none() {
        eprintln!("SKIP: fallback chain resolved to nothing");
        return;
    }
    c.bench_function("font_resolve_full_fallback_chain_miss", |b| {
        b.iter(|| {
            black_box(resolve_full_chain(
                black_box(&registry),
                black_box("CompletelyMissingFont"),
                black_box(&font_map),
                black_box("Default"),
            ));
        })
    });
}

criterion_group!(
    benches,
    bench_query_hit,
    bench_query_miss,
    bench_shape_latin,
    bench_shape_cjk,
    bench_rasterize,
    bench_full_fallback_chain
);
criterion_main!(benches);
