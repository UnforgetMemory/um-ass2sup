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

pub(crate) fn shape_horizontal(
    text: &str,
    ctx: &RenderContext,
    config: &RenderConfig,
    registry: &FontRegistry,
    aw: f32,
    lh: f32,
) -> Vec<ShapedLine> {
    let font_data = resolve_font_data(registry, &ctx.font_name, ctx.bold);
    tracing::debug!(
        font = %ctx.font_name,
        font_data_len = font_data.len(),
        "resolved font data for layout"
    );
    let lines = wrap_text_lines_simple(text, &font_data, ctx.font_size, ctx.spacing, aw);
    tracing::debug!(
        lines = lines.len(),
        "wrapped text lines"
    );
    if lines.is_empty() {
        return vec![];
    }
    let total_h = lines.len() as f32 * lh;
    let ar = ((ctx.alignment - 1) / 3) as usize;
    let yb = if ctx.has_pos {
        match ar {
            0 => ctx.y,
            1 => ctx.y - total_h / 2.0,
            _ => ctx.y - total_h,
        }
    } else {
        match ar {
            0 => ctx.y,
            1 => ctx.y + (config.height as f32 - ctx.margin_v * 2.0 - total_h) / 2.0,
            _ => ctx.y + config.height as f32 - ctx.margin_v * 2.0 - total_h,
        }
    };
    let mut r = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let gs = SimpleShaper::shape(line, &font_data, ctx.font_size).unwrap_or_default();
        let ta: f32 = gs.iter().map(|g| g.x_advance).sum();
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

pub(crate) fn shape_vertical(
    text: &str,
    ctx: &RenderContext,
    registry: &FontRegistry,
    aw: f32,
    ah: f32,
    lh: f32,
) -> Vec<ShapedLine> {
    let cols = wrap_text_vertical(text, ah, lh);
    if cols.is_empty() {
        return vec![];
    }
    let font_data = resolve_font_data(registry, &ctx.font_name, ctx.bold);
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

fn wrap_text_lines_simple(text: &str, font_data: &[u8], fz: f32, _sp: f32, mw: f32) -> Vec<String> {
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
            let ww: f32 = SimpleShaper::shape(&wt, font_data, fz)
                .unwrap_or_default()
                .iter()
                .map(|g| g.x_advance)
                .sum();
            if cw + ww > mw && !cl.is_empty() {
                lines.push(cl);
                cl = word.to_string();
                cw = SimpleShaper::shape(&cl, font_data, fz)
                    .unwrap_or_default()
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

fn resolve_font_data(registry: &FontRegistry, family: &str, bold: bool) -> Vec<u8> {
    use crate::font::types::{FontQuery, FontStyle, FontWeight};

    let weight = if bold {
        FontWeight::Bold
    } else {
        FontWeight::Normal
    };

    // Try exact match first
    let q = FontQuery {
        family: family.to_string(),
        weight,
        style: FontStyle::Normal,
    };
    let result = registry.query(&q);
    tracing::debug!(
        family = %family,
        weight = ?weight,
        found = result.found.is_some(),
        candidates = result.candidates.len(),
        suggestion = result.suggestion.is_some(),
        "font query result"
    );

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
    if let Some((parsed_family, parsed_weight)) = parse_font_name(family) {
        let pq = FontQuery {
            family: parsed_family.to_string(),
            weight: parsed_weight,
            style: FontStyle::Normal,
        };
        let pr = registry.query(&pq);
        tracing::debug!(
            original = %family,
            parsed_family = %parsed_family,
            parsed_weight = ?parsed_weight,
            found = pr.found.is_some(),
            "parsed font query result"
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
