//! Cosmic-text karaoke rendering: syllable-level fill clip sweep, outline highlight.

use ass_parser::karaoke::KaraokeStyle;
use ass_parser::Event;
use tiny_skia::Pixmap;

use crate::context::{RenderConfig, RenderContext};
use crate::cosmic::effects::composite_subregion;
use crate::cosmic::rasterizer::rasterize_cosmic_glyph;
use crate::cosmic::shaper::{CosmicShapedGlyph, CosmicShaper};
use crate::effects;
use crate::karaoke::{KaraokePhase, KaraokeRenderer};

/// Render karaoke using cosmic-text shaping and rasterization.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_karaoke_cosmic(
    pixmap: &mut Pixmap,
    event: &Event,
    ctx: &RenderContext,
    _config: &RenderConfig,
    cosmic_fs: &mut cosmic_text::FontSystem,
    cosmic_cache: &mut cosmic_text::SwashCache,
    ts: u64,
    es: u64,
) {
    let w = pixmap.width();
    let h = pixmap.height();
    let segs = &event.karaoke_segments;
    let states = KaraokeRenderer::compute_syllable_states(segs, es, ts);

    // Phase 1: Shape all syllables, build glyph info list
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
        let shaped = CosmicShaper::shape(
            &syllable.text,
            cosmic_fs,
            ctx.font_size,
            &ctx.font_name,
            ctx.bold,
            ctx.italic,
        );
        let sx = syllable_infos
            .last()
            .map(|last: &SyllableInfo| last.syllable_x + last.syllable_width + ctx.spacing)
            .unwrap_or(0.0);
        let sw: f32 = shaped.iter().map(|g| g.x_advance).sum();
        for g in &shaped {
            min_x = min_x.min(sx + g.x_offset);
            min_y = min_y.min(ctx.y + g.y_offset);
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

    // Phase 2: Allocate layers
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

    // Phase 3: Render background layer (secondary color)
    for info in &syllable_infos {
        let mut cx = info.syllable_x - oxf;
        for glyph in &info.glyphs {
            let bc = RenderContext {
                primary_color: ctx.secondary_color,
                ..ctx.clone()
            };
            rasterize_cosmic_glyph(
                &mut bg,
                cosmic_fs,
                cosmic_cache,
                glyph,
                cx + glyph.x_offset,
                ctx.y + glyph.y_offset - oyf,
                &bc,
            );
            cx += glyph.x_advance;
        }
    }

    // Phase 4: Render foreground layer (primary color for active/done syllables)
    for info in &syllable_infos {
        if !info.is_active {
            continue;
        }
        let mut cx = info.syllable_x - oxf;
        for glyph in &info.glyphs {
            let fc = RenderContext {
                primary_color: ctx.primary_color,
                ..ctx.clone()
            };
            rasterize_cosmic_glyph(
                &mut fg,
                cosmic_fs,
                cosmic_cache,
                glyph,
                cx + glyph.x_offset,
                ctx.y + glyph.y_offset - oyf,
                &fc,
            );
            cx += glyph.x_advance;
        }
    }

    // Phase 5: Apply effects
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
        for (layer, _) in [(&mut bg, false), (&mut fg, false)] {
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

    // Phase 6: Composite foreground over background
    effects::composite_over(bg.data_mut(), fg.data(), lw, lh);
    composite_subregion(pixmap.data_mut(), bg.data(), w, h, ox, oy, lw, lh);
}

/// Internal structure for syllable layout info.
#[allow(dead_code)]
struct SyllableInfo {
    glyphs: Vec<CosmicShapedGlyph>,
    syllable_x: f32,
    syllable_width: f32,
    is_active: bool,
    progress: f32,
    style: KaraokeStyle,
}
