//! Font-registry-based rendering pipeline.
//!
//! Provides `render_event_font_registry` — an alternative to `render_event_cosmic`
//! that uses the new FontRegistry + SimpleShaper + GlyphRasterizer stack.

use ass_core::{Effect, Event};
use parking_lot::Mutex;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform as SkiaTransform};

use crate::context::{RenderConfig, RenderContext};
use crate::effects;
use crate::effects::{
    apply_alpha_multiplier, apply_clip_mask, apply_drawing_clip_mask, composite_subregion,
};
use crate::font::rasterizer::GlyphRasterizer;
use crate::font::registry::FontRegistry;
use crate::font::types::RasterizedGlyph;
use crate::renderer::layout_font_registry::{shape_horizontal, shape_vertical};
use crate::renderer::text_layout::strip_override_blocks;
use crate::transform::AffineTransform;

use crate::renderer::PixmapPool;

pub struct FontRegistryRenderResources {
    pub registry: Mutex<FontRegistry>,
    pub pixmap_pool: Mutex<PixmapPool>,
}

impl FontRegistryRenderResources {
    pub fn new() -> Self {
        Self {
            registry: Mutex::new(FontRegistry::new()),
            pixmap_pool: Mutex::new(PixmapPool::new(8)),
        }
    }

    pub fn pool_get(&self, w: u32, h: u32) -> Option<Pixmap> {
        self.pixmap_pool.lock().get(w, h)
    }

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
    let mut ctx = ctx.clone();

    match &event.effect {
        Effect::Banner {
            delay,
            left_to_right,
            ..
        } if *delay > 0 => {
            let elapsed = (timestamp_ms.saturating_sub(event_start_ms)) as f32;
            ctx.x += elapsed / *delay as f32 * if *left_to_right { 1.0 } else { -1.0 };
        }
        Effect::ScrollUp { delay, top, bottom } if *delay > 0 => {
            let elapsed = (timestamp_ms.saturating_sub(event_start_ms)) as f32;
            ctx.y =
                (config.height as f32 - *bottom as f32 - elapsed / *delay as f32).max(*top as f32);
        }
        Effect::ScrollDown { delay, top, bottom } if *delay > 0 => {
            let elapsed = (timestamp_ms.saturating_sub(event_start_ms)) as f32;
            ctx.y =
                (*top as f32 + elapsed / *delay as f32).min(config.height as f32 - *bottom as f32);
        }
        _ => {}
    }

    let plain_text = strip_override_blocks(&event.text_raw);
    if plain_text.is_empty() {
        return;
    }

    if !event.karaoke.is_empty() {
        let registry = resources.registry.lock();
        super::font_registry_karaoke::render_karaoke_font_registry(
            pixmap,
            event,
            &ctx,
            config,
            &registry,
            timestamp_ms,
            event_start_ms,
        );
        return;
    }

    let drawing_level = crate::renderer::drawing::parse_drawing_level(&event.text_raw);
    if drawing_level > 0 {
        render_drawing(pixmap, &plain_text, &ctx);
        return;
    }

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
            &ctx,
            &registry,
            available_width,
            available_height,
            line_height,
        )
    } else {
        shape_horizontal(
            &plain_text,
            &ctx,
            config,
            &registry,
            available_width,
            line_height,
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
    for sl in &shaped_lines {
        let mut cx = sl.x_start - oxf;
        tracing::debug!(
            glyphs = sl.glyphs.len(),
            x_start = sl.x_start,
            line_y = sl.line_y,
            "rendering line"
        );
        for g in &sl.glyphs {
            let font_data = resolve_glyph_font_data(&registry, &ctx, g.glyph_id);
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
            match GlyphRasterizer::rasterize(&font_data, g.glyph_id, ctx.font_size) {
                Ok(rasterized) => {
                    tracing::debug!(
                        width = rasterized.width,
                        height = rasterized.height,
                        "rasterized glyph successfully"
                    );
                    composite_glyph(
                        &mut layer,
                        &rasterized,
                        cx + g.x_offset,
                        sl.line_y + g.y_offset - oyf,
                        ctx.primary_color,
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        glyph_id = g.glyph_id,
                        error = %e,
                        "failed to rasterize glyph"
                    );
                }
            }
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

    if ctx.border_style != 3 && ctx.blur > 0.0 {
        effects::apply_gaussian_blur(&mut layer, ctx.blur);
    }
    if ctx.border_style != 3 && ctx.shadow_depth > 0.0 {
        let ld = layer.data().to_vec();
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
        let sl = effects::apply_shadow(&ld, lw, lh, sdx, sdy, ctx.blur, ctx.shadow_color);
        let mut sp = match resources.pool_get(lw, lh) {
            Some(p) => p,
            None => return,
        };
        sp.data_mut().copy_from_slice(&sl);
        effects::composite_over(sp.data_mut(), layer.data(), lw, lh);
        layer.data_mut().copy_from_slice(sp.data());
        resources.pool_put(sp);
    }

    let simple = ctx.rotation == 0.0
        && ctx.shear_x == 0.0
        && ctx.shear_y == 0.0
        && ctx.perspective_x == 0.0
        && ctx.perspective_y == 0.0
        && !ctx.clip_enabled
        && ctx.clip_drawing_commands.is_none();

    // Check if layer has any visible pixels
    let non_zero = layer.data().iter().filter(|&&b| b > 0).count();
    tracing::debug!(
        layer_w = lw,
        layer_h = lh,
        non_zero_pixels = non_zero,
        "layer content before compositing"
    );

    if simple {
        if ctx.alpha_multiplier < 0.999 {
            apply_alpha_multiplier(layer.data_mut(), ctx.alpha_multiplier);
        }
        composite_subregion(pixmap.data_mut(), layer.data(), w, h, ox, oy, lw, lh);
    } else {
        let fd = transform_layer(layer.data(), lw, lh, w, h, &ctx);
        let mut result = if ctx.clip_enabled {
            apply_clip_to_data(fd, w, h, &ctx, config)
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

fn composite_glyph(
    layer: &mut Pixmap,
    rasterized: &RasterizedGlyph,
    x: f32,
    y: f32,
    color: [u8; 4],
) {
    let lw = layer.width();
    let lh = layer.height();
    let pix = layer.data_mut();

    tracing::debug!(
        x,
        y,
        rasterized_left = rasterized.left,
        rasterized_top = rasterized.top,
        rasterized_width = rasterized.width,
        rasterized_height = rasterized.height,
        layer_w = lw,
        layer_h = lh,
        "compositing glyph"
    );

    for py in 0..rasterized.height {
        for px in 0..rasterized.width {
            let alpha = rasterized.data[(py * rasterized.width + px) as usize];
            if alpha == 0 {
                continue;
            }
            let tx = x as i32 + rasterized.left + px as i32;
            let ty = y as i32 - rasterized.top + py as i32;
            if tx < 0 || ty < 0 || tx >= lw as i32 || ty >= lh as i32 {
                tracing::trace!(px, py, tx, ty, "pixel out of bounds");
                continue;
            }
            let pi = ((ty as u32 * lw + tx as u32) * 4) as usize;
            let f = alpha as f32 / 255.0;
            let da = pix[pi + 3] as f32 / 255.0;
            let ra = f + da * (1.0 - f);
            for c in 0..3 {
                pix[pi + c] = ((color[c] as f32 * f + pix[pi + c] as f32 * (1.0 - f)) / ra) as u8;
            }
            pix[pi + 3] = (ra * 255.0) as u8;
        }
    }
}

fn resolve_glyph_font_data(
    registry: &FontRegistry,
    ctx: &RenderContext,
    _glyph_id: u16,
) -> Vec<u8> {
    use crate::font::types::{FontQuery, FontStyle, FontWeight};

    let weight = if ctx.bold {
        FontWeight::Bold
    } else {
        FontWeight::Normal
    };

    // Try exact match first
    let q = FontQuery {
        family: ctx.font_name.clone(),
        weight,
        style: FontStyle::Normal,
    };
    let result = registry.query(&q);
    if let Some(id) = result.found {
        if let Some(data) = registry.get_font_data(id) {
            return data.to_vec();
        }
    }
    if let Some(sug) = result.suggestion {
        if let Some(data) = registry.get_font_data(sug.id) {
            return data.to_vec();
        }
    }

    // Parse family name to extract weight/style (e.g., "MiSans Demibold" -> family="MiSans", weight=Demibold)
    if let Some((parsed_family, parsed_weight)) = parse_font_name(&ctx.font_name) {
        let pq = FontQuery {
            family: parsed_family.to_string(),
            weight: parsed_weight,
            style: FontStyle::Normal,
        };
        let pr = registry.query(&pq);
        tracing::debug!(
            original = %ctx.font_name,
            parsed_family = %parsed_family,
            parsed_weight = ?parsed_weight,
            found = pr.found.is_some(),
            "parsed font query result for glyph"
        );

        if let Some(id) = pr.found {
            if let Some(data) = registry.get_font_data(id) {
                return data.to_vec();
            }
        }
        if let Some(sug) = pr.suggestion {
            if let Some(data) = registry.get_font_data(sug.id) {
                return data.to_vec();
            }
        }
    }

    Vec::new()
}

fn draw_decoration(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    thickness: f32,
    color: [u8; 4],
) {
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(x, y);
    pb.line_to(x + width, y);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut p = tiny_skia::Paint::default();
        p.set_color_rgba8(color[0], color[1], color[2], color[3]);
        let stroke = tiny_skia::Stroke {
            width: thickness,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &p, &stroke, tiny_skia::Transform::identity(), None);
    }
}

fn transform_layer(data: &[u8], lw: u32, lh: u32, w: u32, h: u32, ctx: &RenderContext) -> Vec<u8> {
    if ctx.perspective_x != 0.0 || ctx.perspective_y != 0.0 {
        AffineTransform::identity().apply_with_perspective(
            data,
            lw,
            lh,
            w,
            h,
            ctx.perspective_x,
            ctx.perspective_y,
            ctx.origin_x,
            ctx.origin_y,
        )
    } else if ctx.rotation != 0.0 || ctx.shear_x != 0.0 || ctx.shear_y != 0.0 {
        AffineTransform::identity().apply_to_pixmap(data, lw, lh, w, h)
    } else {
        data.to_vec()
    }
}

fn apply_clip_to_data(
    mut data: Vec<u8>,
    w: u32,
    h: u32,
    ctx: &RenderContext,
    config: &RenderConfig,
) -> Vec<u8> {
    if ctx.clip_drawing_commands.is_some() {
        let sx = config.width as f32 / config.script_width as f32;
        let sy = config.height as f32 / config.script_height as f32;
        apply_drawing_clip_mask(&mut data, w, h, ctx, sx, sy);
    } else {
        apply_clip_mask(&mut data, w, h, ctx);
    }
    data
}

fn render_drawing(pixmap: &mut Pixmap, text: &str, ctx: &RenderContext) {
    let cmds = crate::renderer::drawing::parse_drawing_commands(text);
    if cmds.is_empty() {
        return;
    }
    let mut b = PathBuilder::new();
    for cmd in &cmds {
        match cmd {
            crate::renderer::drawing::DrawingCommand::MoveTo(x, y) => b.move_to(*x, *y),
            crate::renderer::drawing::DrawingCommand::LineTo(x, y) => b.line_to(*x, *y),
            crate::renderer::drawing::DrawingCommand::BezierTo(x1, y1, x2, y2, x, y) => {
                b.cubic_to(*x1, *y1, *x2, *y2, *x, *y)
            }
            crate::renderer::drawing::DrawingCommand::Close => b.close(),
        }
    }
    if let Some(path) = b.finish() {
        let mut p = Paint::default();
        p.set_color_rgba8(
            ctx.primary_color[0],
            ctx.primary_color[1],
            ctx.primary_color[2],
            ctx.primary_color[3],
        );
        p.anti_alias = true;
        pixmap.fill_path(
            &path,
            &p,
            FillRule::Winding,
            SkiaTransform::identity(),
            None,
        );
    }
}

/// Parse font family name to extract weight/style information.
/// For example, "MiSans Demibold" -> ("MiSans", Demibold)
fn parse_font_name(family: &str) -> Option<(String, crate::font::types::FontWeight)> {
    use crate::font::types::FontWeight;

    let parts: Vec<&str> = family.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    // Try to find weight keyword in the last part(s)
    let weight_keywords = [
        ("Thin", FontWeight::Thin),
        ("ExtraLight", FontWeight::ExtraLight),
        ("Light", FontWeight::Light),
        ("Regular", FontWeight::Normal),
        ("Normal", FontWeight::Normal),
        ("Medium", FontWeight::Medium),
        ("Demibold", FontWeight::Semibold),
        ("SemiBold", FontWeight::Semibold),
        ("Bold", FontWeight::Bold),
        ("ExtraBold", FontWeight::ExtraBold),
        ("Black", FontWeight::Black),
        ("Heavy", FontWeight::Black),
    ];

    // Check if last part is a weight keyword
    let last = parts.last().unwrap();
    for (keyword, weight) in &weight_keywords {
        if last.eq_ignore_ascii_case(keyword) {
            let family_part = parts[..parts.len() - 1].join(" ");
            return Some((family_part, *weight));
        }
    }

    // Check if last two parts form a weight keyword (e.g., "Extra Bold")
    if parts.len() >= 3 {
        let last_two = format!("{} {}", parts[parts.len() - 2], parts[parts.len() - 1]);
        for (keyword, weight) in &weight_keywords {
            if last_two.eq_ignore_ascii_case(keyword) {
                let family_part = parts[..parts.len() - 2].join(" ");
                return Some((family_part, *weight));
            }
        }
    }

    None
}
