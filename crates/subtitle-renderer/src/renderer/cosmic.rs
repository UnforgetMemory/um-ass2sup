//! Cosmic-text rendering pipeline — replaces fontdb+rustybuzz+ttf-parser.
//!
//! Provides `render_event_cosmic` — the main entry point for rendering a single
//! ASS/SSA dialogue event using cosmic-text shaping and SwashCache rasterization,
//! sharing all post-processing (effects, transforms, clip masks) with the
//! existing pipeline.

use ass_core::{Effect, Event};
use parking_lot::Mutex;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform as SkiaTransform};

use crate::context::{RenderConfig, RenderContext};
use crate::cosmic::effects::{
    apply_alpha_multiplier, apply_clip_mask, apply_drawing_clip_mask, composite_subregion,
};
use crate::cosmic::rasterizer::rasterize_cosmic_glyph;
use crate::cosmic::FontCosmicResolver;
use crate::effects;
use crate::renderer::text_layout::strip_override_blocks;
use crate::transform::AffineTransform;

use super::cosmic_karaoke::render_karaoke_cosmic;
use super::layout::{shape_horizontal, shape_vertical};

/// Reusable pixmap pool to reduce allocations across events.
pub struct CosmicPixmapPool {
    pool: Vec<Pixmap>,
    max_cached: usize,
}

impl CosmicPixmapPool {
    pub(crate) fn new(max_cached: usize) -> Self {
        Self {
            pool: Vec::new(),
            max_cached,
        }
    }
    pub(crate) fn get(&mut self, w: u32, h: u32) -> Option<Pixmap> {
        if let Some(pos) = self
            .pool
            .iter()
            .position(|p| p.width() == w && p.height() == h)
        {
            let mut p = self.pool.remove(pos);
            p.data_mut().fill(0);
            return Some(p);
        }
        Pixmap::new(w, h)
    }
    pub(crate) fn put(&mut self, p: Pixmap) {
        if self.pool.len() < self.max_cached {
            self.pool.push(p);
        }
    }
}

/// Cosmic-text rendering resources: resolver, font system, swash cache, pixmap pool.
pub struct CosmicRenderResources {
    pub resolver: FontCosmicResolver,
    pub pixmap_pool: Mutex<CosmicPixmapPool>,
}

impl CosmicRenderResources {
    pub fn new() -> Self {
        Self {
            resolver: FontCosmicResolver::new(),
            pixmap_pool: Mutex::new(CosmicPixmapPool::new(8)),
        }
    }
    pub(crate) fn pool_get(&self, w: u32, h: u32) -> Option<Pixmap> {
        self.pixmap_pool.lock().get(w, h)
    }
    pub(crate) fn pool_put(&self, p: Pixmap) {
        self.pixmap_pool.lock().put(p);
    }
}

/// Render a single ASS/SSA event using cosmic-text shaping and rasterization.
#[allow(clippy::too_many_arguments)]
pub fn render_event_cosmic(
    pixmap: &mut Pixmap,
    event: &Event,
    ctx: &RenderContext,
    config: &RenderConfig,
    timestamp_ms: u64,
    event_start_ms: u64,
    cosmic: &mut CosmicRenderResources,
) {
    let w = pixmap.width();
    let h = pixmap.height();
    if w == 0 || h == 0 {
        return;
    }
    let mut ctx = ctx.clone();

    // Banner/Scroll effect offset
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

    // Karaoke
    if !event.karaoke.is_empty() {
        render_karaoke_cosmic(
            pixmap,
            event,
            &ctx,
            config,
            &mut cosmic.resolver.font_system(),
            &mut cosmic.resolver.swash_cache(),
            timestamp_ms,
            event_start_ms,
        );
        return;
    }

    // Drawing mode
    let drawing_level = crate::renderer::drawing::parse_drawing_level(&event.text_raw);
    if drawing_level > 0 {
        render_drawing(pixmap, &plain_text, &ctx, drawing_level);
        return;
    }

    // Text layout
    let available_width = config.width as f32 - ctx.margin_l - ctx.margin_r;
    let available_height = config.height as f32 - ctx.margin_v * 2.0;
    let line_height = ctx.font_size * 1.2;

    let shaped_lines = if ctx.writing_mode == 2 || ctx.writing_mode == 3 {
        shape_vertical(
            &plain_text,
            &ctx,
            &mut cosmic.resolver.font_system(),
            available_width,
            available_height,
            line_height,
        )
    } else {
        shape_horizontal(
            &plain_text,
            &ctx,
            config,
            &mut cosmic.resolver.font_system(),
            available_width,
            line_height,
        )
    };
    if shaped_lines.is_empty() {
        return;
    }

    // Sub-region bounds
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for sl in &shaped_lines {
        let mut cx = sl.x_start;
        for g in &sl.glyphs {
            let gx = cx + g.x_offset;
            let gy = sl.line_y + g.y_offset;
            min_x = min_x.min(gx);
            min_y = min_y.min(gy);
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

    let mut layer = match cosmic.pool_get(lw, lh) {
        Some(p) => p,
        None => return,
    };
    let oxf = ox as f32;
    let oyf = oy as f32;
    let fs = &mut cosmic.resolver.font_system();
    let cache = &mut cosmic.resolver.swash_cache();

    // border_style=3 opaque box
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

    // Render glyphs into layer
    for sl in &shaped_lines {
        let mut cx = sl.x_start - oxf;
        for g in &sl.glyphs {
            rasterize_cosmic_glyph(
                &mut layer,
                fs,
                cache,
                g,
                cx + g.x_offset,
                sl.line_y + g.y_offset - oyf,
                &ctx,
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

    // Effects: blur, shadow
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
        let mut sp = match cosmic.pool_get(lw, lh) {
            Some(p) => p,
            None => return,
        };
        sp.data_mut().copy_from_slice(&sl);
        effects::composite_over(sp.data_mut(), layer.data(), lw, lh);
        layer.data_mut().copy_from_slice(sp.data());
        cosmic.pool_put(sp);
    }

    // \p4 clip mask: render text normally, then clip to the drawing path
    if drawing_level == 4 {
        let cmds = crate::renderer::drawing::parse_drawing_commands(&plain_text);
        if !cmds.is_empty() {
            ctx.clip_drawing_commands = Some(plain_text.clone());
            ctx.clip_drawing_scale = 1.0;
            ctx.clip_drawing_inverse = false;
            ctx.clip_enabled = true;
        }
    }

    // Composite onto output
    let simple = ctx.rotation == 0.0
        && ctx.shear_x == 0.0
        && ctx.shear_y == 0.0
        && ctx.perspective_x == 0.0
        && ctx.perspective_y == 0.0
        && !ctx.clip_enabled
        && ctx.clip_drawing_commands.is_none();
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
    cosmic.pool_put(layer);
}

/// Draw a horizontal line (underline/strikeout).
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

/// Apply affine/perspective transform to layer data.
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

/// Apply clip mask to pixel data.
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

/// Render ASS drawing commands into the pixmap.
fn render_drawing(pixmap: &mut Pixmap, text: &str, ctx: &RenderContext, _level: u8) {
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
