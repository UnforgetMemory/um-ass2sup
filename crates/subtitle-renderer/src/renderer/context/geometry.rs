//! Geometry tag handler: `\fscx`, `\fscy`, `\fsc`, `\frx`, `\fry`, `\frz`, `\fr`, `\fax`, `\fay`, `\fsp`, `\be`, `\blur`.
use crate::context::RenderContext;
use ass_core::{OverrideTag, Style};

pub fn apply(tag: &OverrideTag, ctx: &mut RenderContext, style: &Style) {
    match tag {
        OverrideTag::Scale { x, y } => {
            ctx.scale_x = *x as f32;
            ctx.scale_y = *y as f32;
        }
        OverrideTag::ScaleReset => {
            ctx.scale_x = style.scale_x as f32;
            ctx.scale_y = style.scale_y as f32;
        }
        OverrideTag::Rotation { x, y, z } => {
            ctx.perspective_x = *x as f32;
            ctx.perspective_y = *y as f32;
            ctx.rotation = *z as f32;
        }
        OverrideTag::Shear { x, y } => {
            ctx.shear_x = *x as f32;
            ctx.shear_y = *y as f32;
        }
        OverrideTag::Spacing(v) => ctx.spacing = *v as f32,
        OverrideTag::Blur(v) | OverrideTag::GaussianBlur(v) => ctx.blur = *v as f32,
        _ => {}
    }
}
