//! Font-registry-based rendering pipeline.
//!
//! Provides `render_event_font_registry` — an alternative to `render_event_cosmic`
//! that uses the new FontRegistry + SimpleShaper + GlyphRasterizer stack.

use ass_core::{Effect, Event};
use parking_lot::Mutex;
use tiny_skia::{FillRule, Pixmap};

use crate::context::{RenderConfig, RenderContext};
use crate::effects;
use crate::effects::{apply_alpha_multiplier, composite_over, composite_subregion};
use crate::font::glyph_cache::GlyphKey;
use crate::font::rasterizer::GlyphRasterizer;
use crate::font::registry::FontRegistry;
use crate::renderer::layout_font_registry::{shape_horizontal, shape_vertical};
use crate::renderer::text_layout::{process_ass_text_escapes, strip_override_blocks};

use crate::renderer::draw::{apply_clip_to_data, draw_decoration, render_drawing, transform_layer};
use crate::renderer::font_resolve::resolve_glyph_font_data;
use crate::renderer::glyph_composite::composite_glyph;

// Compatibility re-exports: existing callers reference these via
// `font_registry_renderer::*`.
pub use crate::renderer::font_resolve::parse_font_name;
pub(crate) use crate::renderer::font_resolve::resolve_font_data_inner;

use crate::renderer::PixmapPool;

/// Shared rendering resources: font registry, pixmap pool, font fallback map,
/// a persistent font-resolution cache, and a cross-frame glyph cache.
pub struct FontRegistryRenderResources {
    pub registry: Mutex<FontRegistry>,
    pub pixmap_pool: Mutex<PixmapPool>,
    pub font_map: std::collections::HashMap<String, Vec<String>>,
    /// Persistent (font, bold, style) → resolved font bytes. Replaces the
    /// per-event cache: resolution runs the expensive fallback chain once per
    /// distinct font instead of once per event per frame.
    pub font_data_cache: Mutex<std::collections::HashMap<String, std::sync::Arc<[u8]>>>,
    /// Cross-frame glyph rasterization cache (LRU, byte-budgeted).
    pub glyph_cache: Mutex<crate::font::glyph_cache::GlyphCache>,
}

impl FontRegistryRenderResources {
    /// Create a new FontRegistryRenderResources with an empty font registry and 8-slot pixmap pool.
    pub fn new() -> Self {
        Self {
            registry: Mutex::new(FontRegistry::new()),
            pixmap_pool: Mutex::new(PixmapPool::new(8)),
            font_map: std::collections::HashMap::new(),
            font_data_cache: Mutex::new(std::collections::HashMap::new()),
            // 64 MiB byte budget: a CJK glyph at 48 px is ~2–4 KB, so this
            // holds tens of thousands of glyphs — ample for a full movie's
            // unique glyph set while bounding memory.
            glyph_cache: Mutex::new(crate::font::glyph_cache::GlyphCache::new(64 << 20)),
        }
    }

    /// Borrow a cached Pixmap of at least w×h from the pool, or None.
    pub fn pool_get(&self, w: u32, h: u32) -> Option<Pixmap> {
        self.pixmap_pool.lock().get(w, h)
    }

    /// Return a Pixmap to the pool for reuse.
    pub fn pool_put(&self, p: Pixmap) {
        self.pixmap_pool.lock().put(p);
    }
}

impl Default for FontRegistryRenderResources {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::too_many_arguments)]
/// Render a single ASS event into a RGBA Pixmap bitmap using the font registry.
#[tracing::instrument(skip_all, fields(event_start = event.start_ms, event_end = event.end_ms))]
pub fn render_event_font_registry(
    pixmap: &mut Pixmap,
    event: &Event,
    ctx: &RenderContext,
    config: &RenderConfig,
    timestamp_ms: u64,
    event_start_ms: u64,
    resources: &mut FontRegistryRenderResources,
) {
    let w = pixmap.width();
    let h = pixmap.height();
    if w == 0 || h == 0 {
        return;
    }
    // Clone the render context only when a position-animation effect (Banner/
    // Scroll) actually mutates x/y — the common case (no such effect) borrows
    // the caller's context without copying its Strings.
    let ctx_owned = match &event.effect {
        Effect::Banner {
            delay,
            left_to_right,
            ..
        } if *delay > 0 => {
            let mut c = ctx.clone();
            let elapsed = (timestamp_ms.saturating_sub(event_start_ms)) as f32;
            c.x += elapsed / *delay as f32 * if *left_to_right { 1.0 } else { -1.0 };
            Some(c)
        }
        Effect::ScrollUp { delay, top, bottom } if *delay > 0 => {
            let mut c = ctx.clone();
            let elapsed = (timestamp_ms.saturating_sub(event_start_ms)) as f32;
            c.y =
                (config.height as f32 - *bottom as f32 - elapsed / *delay as f32).max(*top as f32);
            Some(c)
        }
        Effect::ScrollDown { delay, top, bottom } if *delay > 0 => {
            let mut c = ctx.clone();
            let elapsed = (timestamp_ms.saturating_sub(event_start_ms)) as f32;
            c.y =
                (*top as f32 + elapsed / *delay as f32).min(config.height as f32 - *bottom as f32);
            Some(c)
        }
        _ => None,
    };
    let ctx: &RenderContext = match &ctx_owned {
        Some(c) => c,
        None => ctx,
    };

    let plain_text = process_ass_text_escapes(&strip_override_blocks(&event.text_raw));
    if plain_text.is_empty() {
        return;
    }

    if !event.karaoke.is_empty() {
        let registry = resources.registry.lock();
        super::font_registry_karaoke::render_karaoke_font_registry(
            pixmap,
            event,
            ctx,
            config,
            &registry,
            timestamp_ms,
            event_start_ms,
        );
        return;
    }

    let drawing_level = crate::renderer::drawing::parse_drawing_level(&event.text_raw);
    if drawing_level > 0 {
        render_drawing(pixmap, &plain_text, ctx);
        return;
    }

    // Font-data resolution is served by the persistent cache in
    // `resources.font_data_cache` (keyed by font name + bold + style), so the
    // expensive fallback chain runs once per distinct font, not per event.

    let registry = resources.registry.lock();
    let available_width = config.width as f32 - ctx.margin_l - ctx.margin_r;
    let available_height = config.height as f32 - ctx.margin_v * 2.0;
    let line_height = ctx.font_size * 1.2;

    tracing::debug!(
        font = %ctx.font_name,
        font_size = ctx.font_size,
        bold = ctx.bold,
        "shaping text"
    );

    let shaped_lines = if ctx.writing_mode == 2 || ctx.writing_mode == 3 {
        shape_vertical(
            &plain_text,
            ctx,
            &registry,
            available_width,
            available_height,
            line_height,
            &resources.font_map,
            event.style.as_str(),
        )
    } else {
        shape_horizontal(
            &plain_text,
            ctx,
            config,
            &registry,
            available_width,
            line_height,
            &resources.font_map,
            event.style.as_str(),
        )
    };
    drop(registry);
    if shaped_lines.is_empty() {
        return;
    }

    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for sl in &shaped_lines {
        let mut cx = sl.x_start;
        for g in &sl.glyphs {
            let gx = cx + g.x_offset;
            let gy = sl.line_y + g.y_offset;
            min_x = min_x.min(gx);
            min_y = min_y.min(gy - g.y_advance);
            max_x = max_x.max(gx + g.x_advance);
            max_y = max_y.max(gy);
            cx += g.x_advance + ctx.spacing;
        }
    }
    if min_x == f32::MAX {
        return;
    }

    let pad = if ctx.border_style == 3 {
        0.0
    } else {
        (ctx.outline_width
            .max(ctx.outline_x_width)
            .max(ctx.outline_y_width)
            * 2.0
            + ctx.shadow_depth.max(ctx.shadow_x).max(ctx.shadow_y)
            + ctx.blur)
            .max(20.0)
    };
    let ox = (min_x - pad).floor() as i32;
    let oy = (min_y - pad).floor() as i32;
    let lw = ((max_x - min_x) + pad * 2.0).ceil().max(1.0) as u32;
    let lh = ((max_y - min_y) + pad * 2.0).ceil().max(1.0) as u32;
    let lw = lw.min(w.saturating_sub(ox.max(0) as u32)).max(1);
    let lh = lh.min(h.saturating_sub(oy.max(0) as u32)).max(1);

    let mut layer = match resources.pool_get(lw, lh) {
        Some(p) => p,
        None => return,
    };
    let oxf = ox as f32;
    let oyf = oy as f32;

    if ctx.border_style == 3 {
        let mut p = tiny_skia::Paint::default();
        p.set_color_rgba8(
            ctx.shadow_color[0],
            ctx.shadow_color[1],
            ctx.shadow_color[2],
            255,
        );
        if let Some(rect) = tiny_skia::Rect::from_xywh(0.0, 0.0, lw as f32, lh as f32) {
            let mut pb = tiny_skia::PathBuilder::new();
            pb.push_rect(rect);
            if let Some(path) = pb.finish() {
                layer.fill_path(
                    &path,
                    &p,
                    FillRule::Winding,
                    tiny_skia::Transform::identity(),
                    None,
                );
            }
        }
    }

    let registry = resources.registry.lock();
    tracing::debug!(shaped_lines = shaped_lines.len(), "rendering shaped lines");
    // Resolve the event's font data once: the (font, bold, style) key is
    // constant across every glyph in this event, so computing the key and
    // running the (expensive) fallback chain inside the glyph loop would
    // format + lowercase + lock + clone per glyph. Hoisted out of the loop.
    let font_data = {
        let cache_key = format!(
            "{}:{}:{}",
            ctx.font_name.to_lowercase(),
            ctx.bold,
            event.style
        );
        resources
            .font_data_cache
            .lock()
            .entry(cache_key)
            .or_insert_with(|| {
                resolve_glyph_font_data(
                    &registry,
                    ctx,
                    shaped_lines
                        .first()
                        .and_then(|sl| sl.glyphs.first())
                        .map(|g| g.glyph_id)
                        .unwrap_or(0),
                    &resources.font_map,
                    event.style.as_str(),
                )
            })
            .clone()
    };
    for sl in &shaped_lines {
        let mut cx = sl.x_start - oxf;
        tracing::debug!(
            glyphs = sl.glyphs.len(),
            x_start = sl.x_start,
            line_y = sl.line_y,
            "rendering line"
        );
        for g in &sl.glyphs {
            if font_data.is_empty() {
                tracing::warn!(
                    glyph_id = g.glyph_id,
                    font = %ctx.font_name,
                    "no font data found for glyph"
                );
                continue;
            }
            tracing::debug!(
                glyph_id = g.glyph_id,
                font_data_len = font_data.len(),
                x = cx + g.x_offset,
                y = sl.line_y + g.y_offset - oyf,
                "rasterizing glyph"
            );
            // Cross-frame glyph cache: the same glyph at the same size is
            // rasterized identically across frames (fade/alpha changes happen
            // at composite time, not rasterization time), so cache by font
            // allocation identity + glyph id + exact size bits.
            let gkey = GlyphKey {
                font: font_data.as_ptr() as usize,
                glyph: g.glyph_id,
                size: ctx.font_size.to_bits(),
            };
            let rasterized = {
                let mut gc = resources.glyph_cache.lock();
                if let Some(hit) = gc.get(&gkey) {
                    hit
                } else {
                    match GlyphRasterizer::rasterize(&font_data, g.glyph_id, ctx.font_size) {
                        Ok(r) => gc.insert(gkey, r),
                        Err(e) => {
                            tracing::warn!(
                                glyph_id = g.glyph_id,
                                error = %e,
                                "failed to rasterize glyph"
                            );
                            continue;
                        }
                    }
                }
            };
            composite_glyph(
                &mut layer,
                &rasterized,
                cx + g.x_offset,
                sl.line_y + g.y_offset - oyf,
                ctx.primary_color,
            );
            cx += g.x_advance + ctx.spacing;
        }
        let total_w = sl.glyphs.iter().map(|g| g.x_advance).sum::<f32>();
        if ctx.underline {
            draw_decoration(
                &mut layer,
                sl.x_start - oxf,
                sl.line_y + ctx.font_size * 0.1 - oyf,
                total_w,
                ctx.font_size * 0.05,
                ctx.primary_color,
            );
        }
        if ctx.strikeout {
            draw_decoration(
                &mut layer,
                sl.x_start - oxf,
                sl.line_y - ctx.font_size * 0.35 - oyf,
                total_w,
                ctx.font_size * 0.05,
                ctx.primary_color,
            );
        }
    }
    drop(registry);

    // ── Outline / border pass ──
    // Tint fill with outline_color, blur to expand, then place under fill.
    // Only for border_style != 3 (OpaqueBox) and outline_width > 0.
    let has_outline = ctx.border_style != 3 && ctx.outline_width > 0.1;
    // Save pre-outline fill data for shadow creation (shadow must not include
    // outline). Only needed when an outline pass will tint a copy of it.
    let fill_data = if has_outline {
        layer.data().to_vec()
    } else {
        Vec::new()
    };
    if has_outline {
        let mut o_px = match resources.pool_get(lw, lh) {
            Some(p) => p,
            None => return,
        };
        o_px.data_mut().copy_from_slice(&fill_data);
        // Tint any visible pixel with outline_color, preserving alpha coverage
        for px in o_px.data_mut().chunks_exact_mut(4) {
            if px[3] > 0 {
                px[0] = ctx.outline_color[0];
                px[1] = ctx.outline_color[1];
                px[2] = ctx.outline_color[2];
            }
        }
        // Blur expands the tinted mask, creating the border thickness
        if ctx.outline_width > 0.5 {
            effects::apply_gaussian_blur(&mut o_px, ctx.outline_width);
        }
        // Place fill over outline: outline below, fill above
        composite_over(o_px.data_mut(), layer.data(), lw, lh);
        layer.data_mut().copy_from_slice(o_px.data());
        resources.pool_put(o_px);
    }

    if ctx.border_style != 3 && ctx.blur > 0.0 {
        effects::apply_gaussian_blur(&mut layer, ctx.blur);
    }
    if ctx.border_style != 3 && ctx.shadow_depth > 0.0 {
        // Use pre-outline fill data for shadow, so outline isn't shadowed twice
        let shadow_src: &[u8] = if has_outline {
            &fill_data
        } else {
            layer.data()
        };
        let sdx = if ctx.shadow_x != 0.0 {
            ctx.shadow_x
        } else {
            ctx.shadow_depth
        };
        let sdy = if ctx.shadow_y != 0.0 {
            ctx.shadow_y
        } else {
            ctx.shadow_depth
        };
        let sl = effects::apply_shadow(shadow_src, lw, lh, sdx, sdy, ctx.blur, ctx.shadow_color);
        let mut sp = match resources.pool_get(lw, lh) {
            Some(p) => p,
            None => return,
        };
        sp.data_mut().copy_from_slice(&sl);
        // Place shadow under the full layer (outline+fill or fill-only)
        composite_over(sp.data_mut(), layer.data(), lw, lh);
        layer.data_mut().copy_from_slice(sp.data());
        resources.pool_put(sp);
    }

    let simple = ctx.rotation == 0.0
        && ctx.shear_x == 0.0
        && ctx.shear_y == 0.0
        && (ctx.scale_x - 100.0).abs() < 0.01
        && (ctx.scale_y - 100.0).abs() < 0.01
        && ctx.perspective_x == 0.0
        && ctx.perspective_y == 0.0
        && !ctx.clip_enabled
        && ctx.clip_drawing_commands.is_none();

    // Layer content stats are only used by a debug! log — skip the full-layer
    // scan when debug tracing is disabled (the default).
    if tracing::enabled!(tracing::Level::DEBUG) {
        let non_zero = layer.data().iter().filter(|&&b| b > 0).count();
        tracing::debug!(
            layer_w = lw,
            layer_h = lh,
            non_zero_pixels = non_zero,
            "layer content before compositing"
        );
    }

    if simple {
        if ctx.alpha_multiplier < 0.999 {
            apply_alpha_multiplier(layer.data_mut(), ctx.alpha_multiplier);
        }
        composite_subregion(pixmap.data_mut(), layer.data(), w, h, ox, oy, lw, lh);
    } else {
        let fd = transform_layer(layer.data(), lw, lh, w, h, ctx);
        let mut result = if ctx.clip_enabled {
            apply_clip_to_data(fd, w, h, ctx, config)
        } else {
            fd
        };
        if ctx.alpha_multiplier < 0.999 {
            apply_alpha_multiplier(&mut result, ctx.alpha_multiplier);
        }
        effects::composite_over(pixmap.data_mut(), &result, w, h);
    }
    resources.pool_put(layer);
}

#[cfg(test)]
mod tests;
