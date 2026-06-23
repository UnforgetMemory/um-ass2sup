//! Reset tag handler: `\r`, `\rStyleName`.
use crate::context::RenderContext;
use ass_core::{OverrideTag, Style, SubtitleDocument};

pub fn apply(tag: &OverrideTag, ctx: &mut RenderContext, doc: &SubtitleDocument, style: &Style) {
    match tag {
        OverrideTag::ResetAll => apply_style(ctx, style),
        OverrideTag::Reset(name) if name.is_empty() => apply_style(ctx, style),
        OverrideTag::Reset(name) => {
            if let Some(s) = doc.styles.iter().find(|s| s.name.as_str() == name.as_str()) {
                apply_style(ctx, s);
            }
        }
        _ => {}
    }
}

fn apply_style(ctx: &mut RenderContext, s: &Style) {
    ctx.font_name = s.font_name.clone();
    ctx.font_size = s.font_size as f32;
    ctx.bold = s.bold;
    ctx.italic = s.italic;
    ctx.underline = s.underline;
    ctx.strikeout = s.strikeout;
    ctx.primary_color = s.primary_color.to_rgba();
    ctx.secondary_color = s.secondary_color.to_rgba();
    ctx.outline_color = s.outline_color.to_rgba();
    ctx.shadow_color = s.shadow_color.to_rgba();
    ctx.scale_x = s.scale_x as f32;
    ctx.scale_y = s.scale_y as f32;
    ctx.spacing = s.spacing as f32;
    ctx.rotation = s.angle as f32;
    ctx.outline_width = s.outline as f32;
    ctx.shadow_depth = s.shadow as f32;
    ctx.alignment = s.alignment as u8;
}
