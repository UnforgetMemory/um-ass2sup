//! Clip tag handler: `\clip`, `\iclip`, `\clip(scale,commands)`, `\iclip(scale,commands)`, `\clip(@)`, `\iclip(@)`.
use crate::context::RenderContext;
use ass_core::OverrideTag;

pub fn apply(tag: &OverrideTag, ctx: &mut RenderContext) {
    match tag {
        OverrideTag::Clip { x1, y1, x2, y2 } => {
            ctx.clip_x1 = *x1 as f32;
            ctx.clip_y1 = *y1 as f32;
            ctx.clip_x2 = *x2 as f32;
            ctx.clip_y2 = *y2 as f32;
            ctx.clip_enabled = true;
            ctx.clip_inverse = false;
        }
        OverrideTag::ClipInverse { x1, y1, x2, y2 } => {
            ctx.clip_x1 = *x1 as f32;
            ctx.clip_y1 = *y1 as f32;
            ctx.clip_x2 = *x2 as f32;
            ctx.clip_y2 = *y2 as f32;
            ctx.clip_enabled = true;
            ctx.clip_inverse = true;
        }
        OverrideTag::ClipDrawing { scale, commands } => {
            ctx.clip_drawing_commands = Some(commands.clone());
            ctx.clip_drawing_scale = *scale;
            ctx.clip_enabled = true;
            ctx.clip_drawing_inverse = false;
        }
        OverrideTag::ClipInverseDrawing { scale, commands } => {
            ctx.clip_drawing_commands = Some(commands.clone());
            ctx.clip_drawing_scale = *scale;
            ctx.clip_enabled = true;
            ctx.clip_drawing_inverse = true;
        }
        OverrideTag::ClipDrawingCurrent => {
            ctx.clip_drawing_current = true;
            ctx.clip_enabled = true;
        }
        OverrideTag::ClipInverseDrawingCurrent => {
            ctx.clip_drawing_current = true;
            ctx.clip_enabled = true;
            ctx.clip_drawing_inverse_current = true;
        }
        _ => {}
    }
}
