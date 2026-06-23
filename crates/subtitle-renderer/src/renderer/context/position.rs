//! Position tag handler: `\pos`, `\move`, `\org`.
use crate::context::{MoveAnim, RenderContext};
use ass_core::OverrideTag;

pub fn apply(tag: &OverrideTag, ctx: &mut RenderContext) {
    match tag {
        OverrideTag::Pos { x, y } => {
            ctx.has_pos = true;
            ctx.x = *x as f32;
            ctx.y = *y as f32;
        }
        OverrideTag::Move {
            x1,
            y1,
            x2,
            y2,
            t1,
            t2,
        } => {
            ctx.has_pos = true;
            ctx.move_animation = Some(MoveAnim {
                x1: *x1 as f32,
                y1: *y1 as f32,
                x2: *x2 as f32,
                y2: *y2 as f32,
                t1: *t1,
                t2: *t2,
            });
        }
        OverrideTag::Origin { x, y } => {
            ctx.origin_x = *x as f32;
            ctx.origin_y = *y as f32;
        }
        _ => {}
    }
}
