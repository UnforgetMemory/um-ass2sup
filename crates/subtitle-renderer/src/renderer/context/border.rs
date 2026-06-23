//! Border tag handler: `\bord`, `\xbord`, `\ybord`, `\shad`, `\xshad`, `\yshad`.
use crate::context::RenderContext;
use ass_core::OverrideTag;

pub fn apply(tag: &OverrideTag, ctx: &mut RenderContext) {
    match tag {
        OverrideTag::Border { x, .. } => {
            ctx.outline_width = *x as f32;
            ctx.outline_x_width = 0.0;
            ctx.outline_y_width = 0.0;
        }
        OverrideTag::BorderX(v) => ctx.outline_x_width = *v as f32,
        OverrideTag::BorderY(v) => ctx.outline_y_width = *v as f32,
        OverrideTag::Shadow { x, .. } => {
            ctx.shadow_depth = *x as f32;
            ctx.shadow_x = 0.0;
            ctx.shadow_y = 0.0;
        }
        OverrideTag::ShadowX(v) => ctx.shadow_x = *v as f32,
        OverrideTag::ShadowY(v) => ctx.shadow_y = *v as f32,
        _ => {}
    }
}
