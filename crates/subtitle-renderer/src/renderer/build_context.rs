//! ASS override tag context builder — converts Event + Style → RenderContext.
//!
//! Handles all 50+ OverrideTag variants, time-aware animations (\move, \fad,
//! \fade, \t), and default alignment positioning.

use crate::context::{RenderConfig, RenderContext};
use crate::renderer::animation::{
    apply_transform_tag, compute_fad_alpha, compute_fade_complex, interpolate_move,
    parse_override_block,
};
use crate::renderer::text_layout::alignment_to_pos;
use ass_core::{Event, OverrideTag, Style, SubtitleDocument};

/// Maximum allowed blur radius to prevent DoS via large blur values.
const MAX_BLUR_RADIUS: f64 = 64.0;

/// Maximum allowed outline width to prevent DoS via large outline values.
const MAX_OUTLINE_WIDTH: f64 = 64.0;

/// Build a fully resolved RenderContext for an event at a given timestamp.
pub fn build_context(
    config: &RenderConfig,
    event: &Event,
    style: &Style,
    doc: &SubtitleDocument,
    timestamp_ms: u64,
    event_start_ms: u64,
    event_end_ms: u64,
) -> RenderContext {
    let mut ctx = RenderContext {
        font_name: if style.font_name.is_empty() {
            config.default_font.clone()
        } else {
            style.font_name.clone()
        },
        font_size: style.font_size as f32,
        primary_color: style.primary_color.to_rgba(),
        secondary_color: style.secondary_color.to_rgba(),
        outline_color: style.outline_color.to_rgba(),
        shadow_color: style.shadow_color.to_rgba(),
        bold: style.bold,
        italic: style.italic,
        outline_width: style.outline as f32,
        shadow_depth: style.shadow as f32,
        alignment: style.alignment.to_u8(),
        margin_l: event.margin_l.unwrap_or(style.margins.left) as f32,
        margin_r: event.margin_r.unwrap_or(style.margins.right) as f32,
        margin_v: event.margin_v.unwrap_or(style.margins.vertical) as f32,
        border_style: style.border_style.to_u8(),
        ..Default::default()
    };
    ctx.scale_x = style.scale_x as f32;
    ctx.scale_y = style.scale_y as f32;
    ctx.spacing = style.spacing as f32;
    ctx.underline = style.underline;
    ctx.strikeout = style.strikeout;
    ctx.rotation = style.angle as f32;

    // Guard against ASS files with no PlayRes (play_res_x/y = 0).
    // Division by zero would produce inf, causing invisible rendering.
    let sw = config.script_width.max(1);
    let sh = config.script_height.max(1);
    let scale_x = config.width as f32 / sw as f32;
    let scale_y = config.height as f32 / sh as f32;
    ctx.margin_l *= scale_x;
    ctx.margin_r *= scale_x;
    ctx.margin_v *= scale_y;
    ctx.font_size = ctx.font_size * config.height as f32 / sh as f32;
    if config.vsfilter_compat {
        // Experimental: scale font_size by ~0.764× to match GDI/VSFilter
        // advance widths (GetTextExtentPoint32W). Derived from comparing
        // swash vs easyavs2bdnxml output for MiSans/Microsoft YaHei at 1080p.
        // Exact factor depends on font; 0.764 is a per-CJK-font average.
        ctx.font_size *= 0.764;
    }

    let mut has_pos = false;
    let mut has_move = false;
    let mut move_x2 = 0.0;
    let mut move_y2 = 0.0;
    let mut move_t1 = 0u64;
    let mut move_t2 = 0u64;
    let mut has_fad = false;
    let mut fad_in = 0u64;
    let mut fad_out = 0u64;
    let mut has_fade_complex = false;
    let mut fade_params = (0u8, 0u8, 0u8, 0u64, 0u64, 0u64, 0u64);
    let mut last_clip_drawing: Option<(f32, String, bool)> = None;

    for tagged in &event.override_tags {
        match &tagged.tag {
            OverrideTag::FontSize(fs) => ctx.font_size = *fs as f32 * scale_y,
            OverrideTag::FontSizeRelative(delta) => {
                ctx.font_size = (ctx.font_size + *delta as f32).max(1.0);
            }
            OverrideTag::FontName(n) => ctx.font_name = n.clone(),
            OverrideTag::Bold(b) => ctx.bold = *b,
            OverrideTag::BoldWeight(w) => ctx.bold = *w >= 700,
            OverrideTag::Italic(i) => ctx.italic = *i,
            OverrideTag::Underline(u) => ctx.underline = *u,
            OverrideTag::Strikeout(s) => ctx.strikeout = *s,
            OverrideTag::PrimaryColor(c) => ctx.primary_color = c.to_rgba(),
            OverrideTag::SecondaryColor(c) => ctx.secondary_color = c.to_rgba(),
            OverrideTag::OutlineColor(c) => ctx.outline_color = c.to_rgba(),
            OverrideTag::ShadowColor(c) => ctx.shadow_color = c.to_rgba(),
            OverrideTag::Alpha { value } => {
                let a = 255 - *value;
                ctx.primary_color[3] = a;
                ctx.secondary_color[3] = a;
                ctx.outline_color[3] = a;
                ctx.shadow_color[3] = a;
            }
            OverrideTag::PrimaryAlpha { value } => ctx.primary_color[3] = 255 - *value,
            OverrideTag::SecondaryAlpha { value } => ctx.secondary_color[3] = 255 - *value,
            OverrideTag::OutlineAlpha { value } => ctx.outline_color[3] = 255 - *value,
            OverrideTag::ShadowAlpha { value } => ctx.shadow_color[3] = 255 - *value,
            OverrideTag::Border { x, y: _ } => {
                ctx.outline_width = (*x).clamp(0.0, MAX_OUTLINE_WIDTH) as f32;
                ctx.outline_x_width = 0.0;
                ctx.outline_y_width = 0.0;
            }
            OverrideTag::BorderX(w) => {
                ctx.outline_x_width = (*w).clamp(0.0, MAX_OUTLINE_WIDTH) as f32
            }
            OverrideTag::BorderY(w) => {
                ctx.outline_y_width = (*w).clamp(0.0, MAX_OUTLINE_WIDTH) as f32
            }
            OverrideTag::Shadow { x, y: _ } => {
                ctx.shadow_depth = *x as f32;
                ctx.shadow_x = 0.0;
                ctx.shadow_y = 0.0;
            }
            OverrideTag::ShadowX(d) => ctx.shadow_x = *d as f32,
            OverrideTag::ShadowY(d) => ctx.shadow_y = *d as f32,
            OverrideTag::Blur(r) | OverrideTag::GaussianBlur(r) => {
                ctx.blur = (*r).clamp(0.0, MAX_BLUR_RADIUS) as f32
            }
            OverrideTag::Spacing(s) => ctx.spacing = *s as f32,
            OverrideTag::Scale { x, y } => {
                ctx.scale_x = *x as f32;
                ctx.scale_y = *y as f32;
            }
            OverrideTag::ScaleReset => {
                ctx.scale_x = 100.0;
                ctx.scale_y = 100.0;
            }
            OverrideTag::Rotation { x, y, z } => {
                ctx.rotation = *z as f32;
                ctx.perspective_x = *x as f32;
                ctx.perspective_y = *y as f32;
            }
            OverrideTag::Origin { x, y } => {
                ctx.origin_x = *x as f32 * scale_x;
                ctx.origin_y = *y as f32 * scale_y;
            }
            OverrideTag::Shear { x, y } => {
                ctx.shear_x = *x as f32;
                ctx.shear_y = *y as f32;
            }
            OverrideTag::AlignmentVsfilter(a) => ctx.alignment = *a,
            OverrideTag::AlignmentNumpad(a) => ctx.alignment = *a,
            OverrideTag::WrapStyle(w) => ctx.wrap_style = *w,
            OverrideTag::Pos { x, y } => {
                ctx.x = *x as f32 * scale_x;
                ctx.y = *y as f32 * scale_y;
                has_pos = true;
                ctx.has_pos = true;
            }
            OverrideTag::Move {
                x1,
                y1,
                x2,
                y2,
                t1,
                t2,
            } => {
                ctx.x = *x1 as f32 * scale_x;
                ctx.y = *y1 as f32 * scale_y;
                move_x2 = *x2 as f32 * scale_x;
                move_y2 = *y2 as f32 * scale_y;
                move_t1 = *t1;
                move_t2 = *t2;
                has_move = true;
                has_pos = true;
                ctx.has_pos = true;
            }
            OverrideTag::Fade {
                duration_in,
                duration_out,
            } => {
                fad_in = *duration_in;
                fad_out = *duration_out;
                has_fad = true;
            }
            OverrideTag::FadeComplex {
                alpha_start,
                alpha_mid,
                alpha_end,
                t1,
                t2,
                t3,
                t4,
            } => {
                fade_params = (*alpha_start, *alpha_mid, *alpha_end, *t1, *t2, *t3, *t4);
                has_fade_complex = true;
            }
            OverrideTag::Clip { x1, y1, x2, y2 } => {
                ctx.clip_x1 = *x1 as f32 * scale_x;
                ctx.clip_y1 = *y1 as f32 * scale_y;
                ctx.clip_x2 = *x2 as f32 * scale_x;
                ctx.clip_y2 = *y2 as f32 * scale_y;
                ctx.clip_enabled = true;
                ctx.clip_inverse = false;
            }
            OverrideTag::ClipInverse { x1, y1, x2, y2 } => {
                ctx.clip_x1 = *x1 as f32 * scale_x;
                ctx.clip_y1 = *y1 as f32 * scale_y;
                ctx.clip_x2 = *x2 as f32 * scale_x;
                ctx.clip_y2 = *y2 as f32 * scale_y;
                ctx.clip_enabled = true;
                ctx.clip_inverse = true;
                ctx.clip_drawing_commands = None;
            }
            OverrideTag::ClipDrawing { scale, commands } => {
                last_clip_drawing = Some((*scale, commands.clone(), false));
                ctx.clip_drawing_commands = Some(commands.clone());
                ctx.clip_drawing_scale = *scale;
                ctx.clip_drawing_inverse = false;
                ctx.clip_enabled = true;
            }
            OverrideTag::ClipInverseDrawing { scale, commands } => {
                last_clip_drawing = Some((*scale, commands.clone(), true));
                ctx.clip_drawing_commands = Some(commands.clone());
                ctx.clip_drawing_scale = *scale;
                ctx.clip_drawing_inverse = true;
                ctx.clip_enabled = true;
            }
            OverrideTag::ClipDrawingCurrent => {
                // Reuse the most recent drawing clip commands (\clip(@)).
                if let Some((scale, ref cmds, inverse)) = last_clip_drawing {
                    ctx.clip_drawing_commands = Some(cmds.clone());
                    ctx.clip_drawing_scale = scale;
                    ctx.clip_drawing_inverse = inverse;
                    ctx.clip_enabled = true;
                }
            }
            OverrideTag::ClipInverseDrawingCurrent => {
                // Reuse the most recent drawing clip as inverted (\iclip(@)).
                if let Some((scale, ref cmds, _)) = last_clip_drawing {
                    ctx.clip_drawing_commands = Some(cmds.clone());
                    ctx.clip_drawing_scale = scale;
                    ctx.clip_drawing_inverse = true;
                    ctx.clip_enabled = true;
                }
            }
            OverrideTag::Transform { tag, t1, t2, accel } => {
                let parsed_inner = parse_override_block(tag);
                if parsed_inner
                    .iter()
                    .any(|t| matches!(t, OverrideTag::Pos { .. }))
                {
                    let (_ax, ay) = alignment_to_pos(ctx.alignment);
                    ctx.x = ctx.margin_l;
                    ctx.y = ctx.margin_v + ay * (config.height as f32 - ctx.margin_v * 2.0);
                    has_pos = true;
                }
                apply_transform_tag(
                    &mut ctx,
                    tag,
                    *t1,
                    *t2,
                    *accel,
                    timestamp_ms,
                    event_start_ms,
                    event_end_ms,
                    scale_x,
                    scale_y,
                );
            }
            OverrideTag::Reset(style_name) => {
                let reset_style = if style_name.is_empty() {
                    Some(style)
                } else {
                    doc.styles.iter().find(|s| s.name.as_str() == style_name)
                };
                if let Some(s) = reset_style {
                    ctx.font_name = s.font_name.clone();
                    ctx.font_size = s.font_size as f32 * scale_y;
                    ctx.bold = s.bold;
                    ctx.italic = s.italic;
                    ctx.primary_color = s.primary_color.to_rgba();
                    ctx.secondary_color = s.secondary_color.to_rgba();
                    ctx.outline_color = s.outline_color.to_rgba();
                    ctx.shadow_color = s.shadow_color.to_rgba();
                    ctx.outline_width = s.outline as f32;
                    ctx.shadow_depth = s.shadow as f32;
                    ctx.alignment = s.alignment.to_u8();
                    ctx.scale_x = s.scale_x as f32;
                    ctx.scale_y = s.scale_y as f32;
                    ctx.spacing = s.spacing as f32;
                    ctx.underline = s.underline;
                    ctx.strikeout = s.strikeout;
                    ctx.rotation = s.angle as f32;
                    ctx.border_style = s.border_style.to_u8();
                    ctx.perspective_x = 0.0;
                    ctx.perspective_y = 0.0;
                    ctx.animation_skip = false;
                }
            }
            OverrideTag::ResetAll => {
                ctx.font_name = style.font_name.clone();
                ctx.font_size = style.font_size as f32 * scale_y;
                ctx.bold = style.bold;
                ctx.italic = style.italic;
                ctx.primary_color = style.primary_color.to_rgba();
                ctx.secondary_color = style.secondary_color.to_rgba();
                ctx.outline_color = style.outline_color.to_rgba();
                ctx.shadow_color = style.shadow_color.to_rgba();
                ctx.outline_width = style.outline as f32;
                ctx.shadow_depth = style.shadow as f32;
                ctx.alignment = style.alignment.to_u8();
                ctx.writing_mode = 0;
                ctx.baseline_offset = 0.0;
                ctx.perspective_x = 0.0;
                ctx.perspective_y = 0.0;
                ctx.animation_skip = false;
            }
            OverrideTag::WritingMode(m) => ctx.writing_mode = *m,
            OverrideTag::BaselineOffset(o) => ctx.baseline_offset = *o,
            OverrideTag::DrawingMode(l) => ctx.drawing_mode = *l,
            OverrideTag::AnimationSkip => ctx.animation_skip = true,
            OverrideTag::Charset(c) => ctx.font_charset = *c,
            OverrideTag::Unknown(tag) => {
                tracing::warn!(tag = %tag, "unrecognized override tag ignored")
            }
            _ => {}
        }
    }

    if has_move {
        let elapsed = timestamp_ms.saturating_sub(event_start_ms);
        let (nx, ny) = interpolate_move(ctx.x, ctx.y, move_x2, move_y2, move_t1, move_t2, elapsed);
        ctx.x = nx;
        ctx.y = ny;
    }
    if has_fad {
        let elapsed = timestamp_ms.saturating_sub(event_start_ms);
        let duration = event_end_ms.saturating_sub(event_start_ms);
        ctx.alpha_multiplier = compute_fad_alpha(elapsed, duration, fad_in, fad_out);
    } else if has_fade_complex {
        let elapsed = timestamp_ms.saturating_sub(event_start_ms);
        let (a1, a2, a3, t1, t2, t3, t4) = fade_params;
        ctx.alpha_multiplier = compute_fade_complex(elapsed, a1, a2, a3, t1, t2, t3, t4);
    }
    if !has_pos {
        let (_ax, ay) = alignment_to_pos(ctx.alignment);
        ctx.x = ctx.margin_l;
        ctx.y = ctx.margin_v + ay * (config.height as f32 - ctx.margin_v * 2.0);
        if ay == 0.0 {
            ctx.y += ctx.font_size;
        }
    }
    if ctx.origin_x == 0.0
        && ctx.origin_y == 0.0
        && (ctx.perspective_x != 0.0 || ctx.perspective_y != 0.0)
    {
        ctx.origin_x = ctx.margin_l + (config.width as f32 - ctx.margin_l - ctx.margin_r) / 2.0;
        let (_ax, ay) = alignment_to_pos(ctx.alignment);
        ctx.origin_y = ctx.margin_v + ay * (config.height as f32 - ctx.margin_v * 2.0);
    }
    ctx
}
