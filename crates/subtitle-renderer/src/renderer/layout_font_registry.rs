//! Font-registry-based layout: shaped lines using SimpleShaper.

use crate::context::{RenderConfig, RenderContext};
use crate::font::registry::FontRegistry;
use crate::font::shaper::SimpleShaper;
use crate::font::types::ShapedGlyph;
use crate::renderer::text_layout::{remap_alignment_vertical, wrap_text_vertical};

pub(crate) struct ShapedLine {
    pub(crate) glyphs: Vec<ShapedGlyph>,
    pub(crate) line_y: f32,
    pub(crate) x_start: f32,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn shape_horizontal(
    text: &str,
    ctx: &RenderContext,
    config: &RenderConfig,
    registry: &FontRegistry,
    aw: f32,
    lh: f32,
    font_map: &std::collections::HashMap<String, Vec<String>>,
    style_name: &str,
) -> Vec<ShapedLine> {
    let font_data = resolve_font_data(registry, &ctx.font_name, ctx.bold, font_map, style_name);
    tracing::debug!(
        font = %ctx.font_name,
        font_data_len = font_data.len(),
        "resolved font data for layout"
    );
    let lines = wrap_text_lines_simple(text, &font_data, ctx.font_size, ctx.spacing, aw);
    tracing::debug!(lines = lines.len(), "wrapped text lines");
    if lines.is_empty() {
        return vec![];
    }
    let total_h = lines.len() as f32 * lh;
    let ar = ((ctx.alignment - 1) / 3) as usize;
    let yb = if ctx.has_pos {
        // \pos(x,y): anchor the text block according to alignment.
        // With line_y = yb + i*lh (lines go down):
        //   top    → first line at anchor  → yb = ctx.y
        //   center → block centre at anchor → yb = ctx.y - total_h/2 + lh/2
        //   bottom → last  line at anchor  → yb = ctx.y - total_h + lh
        match ar {
            0 => ctx.y - total_h + lh,
            1 => ctx.y - total_h / 2.0 + lh / 2.0,
            _ => ctx.y,
        }
    } else {
        // Compute y base from alignment and margins only.
        // In !has_pos mode, ctx.y has already been set by build_context alignment; do NOT
        // double-apply it.  Instead compute yb from scratch using the safe area.
        let safe_top = ctx.margin_v;
        let safe_h = config.height as f32 - ctx.margin_v * 2.0;
        match ar {
            // Bottom: last line's baseline at safe area bottom
            0 => safe_top + safe_h - total_h + lh,
            // Center: text block centre at safe area centre
            1 => safe_top + (safe_h - total_h) / 2.0 + lh / 2.0,
            // Top: first line's baseline just below safe area top
            _ => safe_top + ctx.font_size,
        }
    };
    let mut r = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let gs = SimpleShaper::shape(line, &font_data, ctx.font_size).unwrap_or_default();
        let glyph_count = gs.len() as f32;
        let ta: f32 = gs.iter().map(|g| g.x_advance).sum::<f32>() + glyph_count * ctx.spacing;
        let ac = (ctx.alignment - 1) % 3;
        let xs = if ctx.has_pos {
            match ac {
                2 => ctx.x - ta,
                1 => ctx.x - ta / 2.0,
                _ => ctx.x,
            }
        } else {
            match ac {
                2 => ctx.x + aw - ta,
                1 => ctx.x + (aw - ta) / 2.0,
                _ => ctx.x,
            }
        };
        r.push(ShapedLine {
            glyphs: gs,
            line_y: yb + i as f32 * lh,
            x_start: xs,
        });
    }
    r
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn shape_vertical(
    text: &str,
    ctx: &RenderContext,
    registry: &FontRegistry,
    aw: f32,
    ah: f32,
    lh: f32,
    font_map: &std::collections::HashMap<String, Vec<String>>,
    style_name: &str,
) -> Vec<ShapedLine> {
    let cols = wrap_text_vertical(text, ah, lh);
    if cols.is_empty() {
        return vec![];
    }
    let font_data = resolve_font_data(registry, &ctx.font_name, ctx.bold, font_map, style_name);
    let rm = remap_alignment_vertical(ctx.alignment, ctx.writing_mode);
    let ac = rm % 3;
    let tw = cols.len() as f32 * lh;
    let xb = match ac {
        2 => ctx.x + (aw - tw) / 2.0,
        0 => ctx.x + aw - tw,
        _ => ctx.x,
    };
    let mut r = Vec::new();
    for (ci, col) in cols.iter().enumerate() {
        let cx = if ctx.writing_mode == 2 {
            xb + (cols.len() - 1 - ci) as f32 * lh
        } else {
            xb + ci as f32 * lh
        };
        for (j, ch) in col.chars().enumerate() {
            let gs =
                SimpleShaper::shape(&ch.to_string(), &font_data, ctx.font_size).unwrap_or_default();
            for g in gs {
                r.push(ShapedLine {
                    glyphs: vec![g],
                    line_y: ctx.y + j as f32 * lh,
                    x_start: cx,
                });
            }
        }
    }
    r
}

fn wrap_text_lines_simple(text: &str, font_data: &[u8], fz: f32, sp: f32, mw: f32) -> Vec<String> {
    if text.is_empty() || mw <= 0.0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut cl = String::new();
        let mut cw = 0.0;
        for word in para.split_whitespace() {
            let wt = if cl.is_empty() {
                word.to_string()
            } else {
                format!(" {word}")
            };
            let shaped = SimpleShaper::shape(&wt, font_data, fz).unwrap_or_default();
            let glyph_count = shaped.len() as f32;
            let ww: f32 = shaped.iter().map(|g| g.x_advance).sum::<f32>() + glyph_count * sp;
            if cw + ww > mw && !cl.is_empty() {
                lines.push(cl);
                cl = word.to_string();
                let shaped_cl = SimpleShaper::shape(&cl, font_data, fz).unwrap_or_default();
                let glyph_count_cl = shaped_cl.len() as f32;
                cw = shaped_cl.iter().map(|g| g.x_advance).sum::<f32>() + glyph_count_cl * sp;
            } else {
                cl = if cl.is_empty() {
                    word.to_string()
                } else {
                    format!("{cl} {word}")
                };
                cw += ww;
            }
        }
        if !cl.is_empty() {
            lines.push(cl);
        }
    }
    lines
}

fn resolve_font_data(
    registry: &FontRegistry,
    family: &str,
    bold: bool,
    font_map: &std::collections::HashMap<String, Vec<String>>,
    style_name: &str,
) -> Vec<u8> {
    super::font_registry_renderer::resolve_font_data_inner(
        registry,
        family,
        bold,
        Some(font_map),
        style_name,
        true,
    )
}
