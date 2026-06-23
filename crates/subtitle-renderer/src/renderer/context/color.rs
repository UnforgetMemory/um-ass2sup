//! Color tag handler: `\1c`-`\4c`, `\1a`-`\4a`, `\alpha`.
use crate::context::RenderContext;
use ass_core::OverrideTag;

fn ass_alpha_to_rgba(a: u8) -> u8 {
    255 - a
}

fn apply_alpha(rgba: &mut [u8; 4], a: u8) {
    rgba[3] = ass_alpha_to_rgba(a);
}

pub fn apply(tag: &OverrideTag, ctx: &mut RenderContext) {
    match tag {
        OverrideTag::PrimaryColor(c) => ctx.primary_color = c.to_rgba(),
        OverrideTag::SecondaryColor(c) => ctx.secondary_color = c.to_rgba(),
        OverrideTag::OutlineColor(c) => ctx.outline_color = c.to_rgba(),
        OverrideTag::ShadowColor(c) => ctx.shadow_color = c.to_rgba(),
        OverrideTag::Alpha { value } => {
            let v = ass_alpha_to_rgba(*value);
            ctx.primary_color[3] = v;
            ctx.secondary_color[3] = v;
            ctx.outline_color[3] = v;
            ctx.shadow_color[3] = v;
        }
        OverrideTag::PrimaryAlpha { value } => apply_alpha(&mut ctx.primary_color, *value),
        OverrideTag::SecondaryAlpha { value } => apply_alpha(&mut ctx.secondary_color, *value),
        OverrideTag::OutlineAlpha { value } => apply_alpha(&mut ctx.outline_color, *value),
        OverrideTag::ShadowAlpha { value } => apply_alpha(&mut ctx.shadow_color, *value),
        _ => {}
    }
}
