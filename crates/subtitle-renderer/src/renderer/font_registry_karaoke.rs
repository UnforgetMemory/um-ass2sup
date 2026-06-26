//! Font-registry karaoke rendering: syllable-level fill clip sweep, outline highlight.

use ass_core::Event;
use ass_core::KaraokeStyle;
use tiny_skia::Pixmap;

use crate::context::{RenderConfig, RenderContext};
use crate::effects;
use crate::effects::composite_subregion;
use crate::font::rasterizer::GlyphRasterizer;
use crate::font::registry::FontRegistry;
use crate::font::types::ShapedGlyph;
use crate::font::shaper::SimpleShaper;
use crate::karaoke::{KaraokePhase, KaraokeRenderer};

#[allow(dead_code)]
struct SyllableInfo {
    glyphs: Vec<ShapedGlyph>,
    syllable_x: f32,
    syllable_width: f32,
    is_active: bool,
    progress: f32,
    style: KaraokeStyle,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_karaoke_font_registry(
    pixmap: &mut Pixmap,
    event: &Event,
    ctx: &RenderContext,
    _config: &RenderConfig,
    registry: &FontRegistry,
    ts: u64,
    es: u64,
) {
    let w = pixmap.width();
    let h = pixmap.height();
    let segs = &event.karaoke;
    let states = KaraokeRenderer::compute_syllable_states(segs, es, ts);

    let font_data = resolve_font_data(registry, &ctx.font_name, ctx.bold);

    let mut syllable_infos: Vec<SyllableInfo> = Vec::new();
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut any_glyph = false;

    for syllable in &states {
        if syllable.text.is_empty() {
            continue;
        }
        let shaped = SimpleShaper::shape(&syllable.text, &font_data, ctx.font_size)
            .unwrap_or_default();
        let sx = syllable_infos
            .last()
            .map(|last: &SyllableInfo| last.syllable_x + last.syllable_width + ctx.spacing)
            .unwrap_or(0.0);
        let sw: f32 = shaped.iter().map(|g| g.x_advance).sum();
        for g in &shaped {
            min_x = min_x.min(sx + g.x_offset);
            min_y = min_y.min(ctx.y + g.y_offset - g.y_advance);
            max_x = max_x.max(sx + g.x_offset + g.x_advance);
            max_y = max_y.max(ctx.y + g.y_offset);
            any_glyph = true;
        }
        let is_active = matches!(
            syllable.phase,
            KaraokePhase::Active { .. } | KaraokePhase::Done
        );
        let progress = match syllable.phase {
            KaraokePhase::Active { progress } => progress,
            _ => 0.0,
        };
        syllable_infos.push(SyllableInfo {
            glyphs: shaped,
            syllable_x: sx,
            syllable_width: sw,
            is_active,
            progress,
            style: syllable.style,
        });
    }
    if !any_glyph {
        return;
    }

    let border = ctx
        .outline_width
        .max(ctx.outline_x_width)
        .max(ctx.outline_y_width);
    let pad = (border * 2.0 + ctx.shadow_depth + ctx.blur).max(20.0);
    let ox = (min_x - pad).floor() as i32;
    let oy = (min_y - pad).floor() as i32;
    let lw = ((max_x - min_x) + pad * 2.0).ceil().max(1.0) as u32;
    let lh = ((max_y - min_y) + pad * 2.0).ceil().max(1.0) as u32;
    let lw = lw.min(w.saturating_sub(ox.max(0) as u32)).max(1);
    let lh = lh.min(h.saturating_sub(oy.max(0) as u32)).max(1);

    let mut bg = match Pixmap::new(lw, lh) {
        Some(p) => p,
        None => return,
    };
    let mut fg = match Pixmap::new(lw, lh) {
        Some(p) => p,
        None => return,
    };
    let oxf = ox as f32;
    let oyf = oy as f32;

    for info in &syllable_infos {
        let mut cx = info.syllable_x - oxf;
        for glyph in &info.glyphs {
            if let Ok(rasterized) =
                GlyphRasterizer::rasterize(&font_data, glyph.glyph_id, ctx.font_size)
            {
                composite_glyph(
                    &mut bg,
                    &rasterized,
                    cx + glyph.x_offset,
                    ctx.y + glyph.y_offset - oyf,
                    ctx.secondary_color,
                );
            }
            cx += glyph.x_advance;
        }
    }

    for info in &syllable_infos {
        if !info.is_active {
            continue;
        }
        let mut cx = info.syllable_x - oxf;
        for glyph in &info.glyphs {
            if let Ok(rasterized) =
                GlyphRasterizer::rasterize(&font_data, glyph.glyph_id, ctx.font_size)
            {
                composite_glyph(
                    &mut fg,
                    &rasterized,
                    cx + glyph.x_offset,
                    ctx.y + glyph.y_offset - oyf,
                    ctx.primary_color,
                );
            }
            cx += glyph.x_advance;
        }
    }

    if ctx.blur > 0.0 {
        effects::apply_gaussian_blur(&mut bg, ctx.blur);
        effects::apply_gaussian_blur(&mut fg, ctx.blur);
    }
    if ctx.shadow_depth > 0.0 {
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
        for layer in [&mut bg, &mut fg] {
            let ld = layer.data().to_vec();
            let sl = effects::apply_shadow(&ld, lw, lh, sdx, sdy, ctx.blur, ctx.shadow_color);
            let mut sp = match Pixmap::new(lw, lh) {
                Some(p) => p,
                None => return,
            };
            sp.data_mut().copy_from_slice(&sl);
            effects::composite_over(sp.data_mut(), layer.data(), lw, lh);
            layer.data_mut().copy_from_slice(sp.data());
        }
    }

    effects::composite_over(bg.data_mut(), fg.data(), lw, lh);
    composite_subregion(pixmap.data_mut(), bg.data(), w, h, ox, oy, lw, lh);
}

fn composite_glyph(
    layer: &mut Pixmap,
    rasterized: &crate::font::types::RasterizedGlyph,
    x: f32,
    y: f32,
    color: [u8; 4],
) {
    let lw = layer.width();
    let lh = layer.height();
    let pix = layer.data_mut();

    for py in 0..rasterized.height {
        for px in 0..rasterized.width {
            let alpha = rasterized.data[(py * rasterized.width + px) as usize];
            if alpha == 0 {
                continue;
            }
            let tx = x as i32 + rasterized.left + px as i32;
            let ty = y as i32 - rasterized.top + py as i32;
            if tx < 0 || ty < 0 || tx >= lw as i32 || ty >= lh as i32 {
                continue;
            }
            let pi = ((ty as u32 * lw + tx as u32) * 4) as usize;
            let f = alpha as f32 / 255.0;
            let da = pix[pi + 3] as f32 / 255.0;
            let ra = f + da * (1.0 - f);
            for c in 0..3 {
                pix[pi + c] =
                    ((color[c] as f32 * f + pix[pi + c] as f32 * (1.0 - f)) / ra) as u8;
            }
            pix[pi + 3] = (ra * 255.0) as u8;
        }
    }
}

fn resolve_font_data(registry: &FontRegistry, family: &str, bold: bool) -> Vec<u8> {
    use crate::font::types::{FontQuery, FontStyle, FontWeight};

    let weight = if bold {
        FontWeight::Bold
    } else {
        FontWeight::Normal
    };
    let q = FontQuery {
        family: family.to_string(),
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
    Vec::new()
}
