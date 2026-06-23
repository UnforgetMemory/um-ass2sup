//! Cosmic-text layout: shaped lines, word wrapping, horizontal/vertical shaping.

use cosmic_text::FontSystem;

use crate::context::{RenderConfig, RenderContext};
use crate::cosmic::shaper::{CosmicShapedGlyph, CosmicShaper};
use crate::renderer::text_layout::{remap_alignment_vertical, wrap_text_vertical};

/// A shaped line produced by cosmic-text, used internally for layout.
#[allow(dead_code)]
pub(crate) struct CosmicShapedLine {
    pub(crate) glyphs: Vec<CosmicShapedGlyph>,
    pub(crate) total_advance: f32,
    pub(crate) line_y: f32,
    pub(crate) x_start: f32,
}

/// Shape horizontal text into lines using cosmic-text.
pub(crate) fn shape_horizontal(
    text: &str,
    ctx: &RenderContext,
    config: &RenderConfig,
    cosmic_fs: &mut FontSystem,
    aw: f32,
    lh: f32,
) -> Vec<CosmicShapedLine> {
    let lines = wrap_text_lines(
        text,
        cosmic_fs,
        ctx.font_size,
        ctx.spacing,
        aw,
        &ctx.font_name,
        ctx.bold,
        ctx.italic,
    );
    if lines.is_empty() {
        return vec![];
    }
    let total_h = lines.len() as f32 * lh;
    let ar = ((ctx.alignment - 1) / 3) as usize;
    let yb = match ar {
        0 => ctx.y,
        1 => ctx.y + (config.height as f32 - ctx.margin_v * 2.0 - total_h) / 2.0,
        _ => ctx.y + config.height as f32 - ctx.margin_v * 2.0 - total_h,
    };
    let mut r = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let gs = CosmicShaper::shape(
            line,
            cosmic_fs,
            ctx.font_size,
            &ctx.font_name,
            ctx.bold,
            ctx.italic,
        );
        let ta: f32 = gs.iter().map(|g| g.x_advance).sum();
        let ac = (ctx.alignment - 1) % 3;
        let xs = match ac {
            2 => ctx.x + aw - ta,
            1 => ctx.x + (aw - ta) / 2.0,
            _ => ctx.x,
        };
        r.push(CosmicShapedLine {
            glyphs: gs,
            total_advance: ta,
            line_y: yb + i as f32 * lh,
            x_start: xs,
        });
    }
    r
}

/// Shape vertical text using cosmic-text.
pub(crate) fn shape_vertical(
    text: &str,
    ctx: &RenderContext,
    cosmic_fs: &mut FontSystem,
    aw: f32,
    ah: f32,
    lh: f32,
) -> Vec<CosmicShapedLine> {
    let cols = wrap_text_vertical(text, ah, lh);
    if cols.is_empty() {
        return vec![];
    }
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
            let gs = CosmicShaper::shape(
                &ch.to_string(),
                cosmic_fs,
                ctx.font_size,
                &ctx.font_name,
                ctx.bold,
                ctx.italic,
            );
            for g in gs {
                r.push(CosmicShapedLine {
                    glyphs: vec![g],
                    total_advance: 0.0,
                    line_y: ctx.y + j as f32 * lh,
                    x_start: cx,
                });
            }
        }
    }
    r
}

/// Word-by-word line wrapping using cosmic-text for width measurement.
#[allow(clippy::too_many_arguments)]
pub(crate) fn wrap_text_lines(
    text: &str,
    fs: &mut FontSystem,
    fz: f32,
    _sp: f32,
    mw: f32,
    fn_name: &str,
    bold: bool,
    italic: bool,
) -> Vec<String> {
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
            let ww: f32 = CosmicShaper::shape(&wt, fs, fz, fn_name, bold, italic)
                .iter()
                .map(|g| g.x_advance)
                .sum();
            if cw + ww > mw && !cl.is_empty() {
                lines.push(cl);
                cl = word.to_string();
                cw = CosmicShaper::shape(&cl, fs, fz, fn_name, bold, italic)
                    .iter()
                    .map(|g| g.x_advance)
                    .sum();
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
