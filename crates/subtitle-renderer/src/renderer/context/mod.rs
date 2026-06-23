#![allow(dead_code)]
//! DEPRECATED: 53-tag build_context for ass_core SubtitleDocument.
//! No longer used — the cosmic path uses Renderer::build_context (ass_parser).
//! Kept temporarily for reference; will be removed in a future cleanup.

pub mod border;
pub mod clip;
pub mod color;
pub mod effect;
pub mod font;
pub mod geometry;
pub mod karaoke;
pub mod misc;
pub mod position;
pub mod reset;
pub mod transform;

#[cfg(test)]
mod tests;

use crate::context::{RenderConfig, RenderContext};
use ass_core::{Event, OverrideTag, Style, SubtitleDocument};

/// Build the render context for a single event at a given timestamp.
///
/// 1. Initialise from style defaults
/// 2. Apply per-tag overrides
/// 3. Apply deferred animations
pub fn build_context(
    event: &Event,
    style: &Style,
    doc: &SubtitleDocument,
    config: &RenderConfig,
    timestamp_ms: u64,
    event_start_ms: u64,
) -> RenderContext {
    let mut ctx = context_from_style(style);

    // Apply scaling
    apply_scaling(&mut ctx, config);

    // Apply override tags
    for to in &event.override_tags {
        apply_tag(&to.tag, &mut ctx, doc, style);
    }

    // Deferred: move interpolation
    if let Some(mv) = ctx.move_animation {
        let width = (mv.t2.saturating_sub(mv.t1)) as f32;
        let elapsed = (timestamp_ms.saturating_sub(mv.t1)).min(mv.t2.saturating_sub(mv.t1)) as f32;
        let t = if width > 0.0 {
            (elapsed / width).clamp(0.0, 1.0)
        } else {
            1.0
        };
        ctx.x = mv.x1 + (mv.x2 - mv.x1) * t;
        ctx.y = mv.y1 + (mv.y2 - mv.y1) * t;
    }

    // Deferred: fade effects (\fad, \fade)
    effect::apply_fade(event, &mut ctx, timestamp_ms, event_start_ms);

    ctx
}

fn context_from_style(s: &Style) -> RenderContext {
    RenderContext {
        font_name: s.font_name.clone(),
        font_size: s.font_size as f32,
        bold: s.bold,
        italic: s.italic,
        underline: s.underline,
        strikeout: s.strikeout,
        primary_color: s.primary_color.to_rgba(),
        secondary_color: s.secondary_color.to_rgba(),
        outline_color: s.outline_color.to_rgba(),
        shadow_color: s.shadow_color.to_rgba(),
        scale_x: s.scale_x as f32,
        scale_y: s.scale_y as f32,
        spacing: s.spacing as f32,
        rotation: s.angle as f32,
        outline_width: s.outline as f32,
        shadow_depth: s.shadow as f32,
        alignment: s.alignment as u8,
        border_style: s.border_style as u8,
        charset: s.encoding.0,
        ..Default::default()
    }
}

fn apply_scaling(ctx: &mut RenderContext, config: &RenderConfig) {
    let sx = config.width as f32 / config.script_width as f32;
    let sy = config.height as f32 / config.script_height as f32;
    ctx.x *= sx;
    ctx.y *= sy;
    ctx.margin_l *= sx;
    ctx.margin_r *= sx;
    ctx.margin_v *= sy;
    ctx.clip_x1 *= sx;
    ctx.clip_x2 *= sx;
    ctx.clip_y1 *= sy;
    ctx.clip_y2 *= sy;
}

fn apply_tag(tag: &OverrideTag, ctx: &mut RenderContext, doc: &SubtitleDocument, style: &Style) {
    match tag {
        OverrideTag::Pos { .. } | OverrideTag::Move { .. } | OverrideTag::Origin { .. } => {
            position::apply(tag, ctx)
        }
        OverrideTag::FontName(_)
        | OverrideTag::FontSize(_)
        | OverrideTag::FontSizeRelative(_)
        | OverrideTag::Bold(_)
        | OverrideTag::BoldWeight(_)
        | OverrideTag::Italic(_)
        | OverrideTag::Underline(_)
        | OverrideTag::Strikeout(_) => font::apply(tag, ctx),
        OverrideTag::PrimaryColor(_)
        | OverrideTag::SecondaryColor(_)
        | OverrideTag::OutlineColor(_)
        | OverrideTag::ShadowColor(_) => color::apply(tag, ctx),
        OverrideTag::Alpha { .. }
        | OverrideTag::PrimaryAlpha { .. }
        | OverrideTag::SecondaryAlpha { .. }
        | OverrideTag::OutlineAlpha { .. }
        | OverrideTag::ShadowAlpha { .. } => color::apply(tag, ctx),
        OverrideTag::Border { .. }
        | OverrideTag::BorderX(_)
        | OverrideTag::BorderY(_)
        | OverrideTag::Shadow { .. }
        | OverrideTag::ShadowX(_)
        | OverrideTag::ShadowY(_) => border::apply(tag, ctx),
        OverrideTag::Scale { .. }
        | OverrideTag::ScaleReset
        | OverrideTag::Rotation { .. }
        | OverrideTag::Shear { .. }
        | OverrideTag::Spacing(_)
        | OverrideTag::Blur(_)
        | OverrideTag::GaussianBlur(_) => geometry::apply(tag, ctx, style),
        OverrideTag::Clip { .. }
        | OverrideTag::ClipInverse { .. }
        | OverrideTag::ClipDrawing { .. }
        | OverrideTag::ClipInverseDrawing { .. }
        | OverrideTag::ClipDrawingCurrent
        | OverrideTag::ClipInverseDrawingCurrent => clip::apply(tag, ctx),
        OverrideTag::Karaoke { .. } => karaoke::apply(tag, ctx),
        OverrideTag::ResetAll | OverrideTag::Reset(_) => reset::apply(tag, ctx, doc, style),
        OverrideTag::Transform { .. } => transform::apply(tag, ctx),
        _ => misc::apply(tag, ctx),
    }
}
