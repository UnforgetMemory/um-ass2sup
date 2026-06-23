use crate::context::RenderContext;
use crate::renderer::drawing::{parse_drawing_commands, DrawingCommand};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform as SkiaTransform};

/// Apply a rectangular clip mask to RGBA pixel data.
/// Pixels outside the clip rectangle are zeroed (or inside for inverse).
pub fn apply_clip_mask(data: &mut [u8], w: u32, h: u32, ctx: &RenderContext) {
    let x1 = ctx.clip_x1.max(0.0) as usize;
    let y1 = ctx.clip_y1.max(0.0) as usize;
    let x2 = ctx.clip_x2.min(w as f32) as usize;
    let y2 = ctx.clip_y2.min(h as f32) as usize;
    let wu = w as usize;
    if ctx.clip_inverse {
        for y in y1..y2 {
            data[y * wu * 4 + x1 * 4..y * wu * 4 + x2 * 4].fill(0);
        }
    } else {
        for y in 0..h as usize {
            if y < y1 || y >= y2 {
                data[y * wu * 4..(y + 1) * wu * 4].fill(0);
            } else {
                if x1 > 0 {
                    data[y * wu * 4..y * wu * 4 + x1 * 4].fill(0);
                }
                if x2 < wu {
                    data[y * wu * 4 + x2 * 4..(y + 1) * wu * 4].fill(0);
                }
            }
        }
    }
}

/// Apply a vector drawing clip mask to RGBA pixel data.
/// Renders drawing commands as a path and clips to it.
pub fn apply_drawing_clip_mask(
    data: &mut [u8],
    w: u32,
    h: u32,
    ctx: &RenderContext,
    scale_x: f32,
    scale_y: f32,
) {
    let commands = match &ctx.clip_drawing_commands {
        Some(c) => c,
        None => return,
    };
    let scale = ctx.clip_drawing_scale;
    let sx = w as f32 / scale_x;
    let sy = h as f32 / scale_y;
    let mut pb = PathBuilder::new();
    for cmd in parse_drawing_commands(commands) {
        match cmd {
            DrawingCommand::MoveTo(x, y) => pb.move_to(x * scale * sx, y * scale * sy),
            DrawingCommand::LineTo(x, y) => pb.line_to(x * scale * sx, y * scale * sy),
            DrawingCommand::BezierTo(x1, y1, x2, y2, x3, y3) => pb.cubic_to(
                x1 * scale * sx,
                y1 * scale * sy,
                x2 * scale * sx,
                y2 * scale * sy,
                x3 * scale * sx,
                y3 * scale * sy,
            ),
            DrawingCommand::Close => pb.close(),
        }
    }
    if ctx.clip_inverse {
        // Inverse clip: clear outside the drawing path
        if let Some(path) = pb.finish() {
            let mut mask = match Pixmap::new(w, h) {
                Some(p) => p,
                None => return,
            };
            let mut p = Paint::default();
            p.set_color_rgba8(255, 255, 255, 255);
            mask.fill_path(
                &path,
                &p,
                FillRule::Winding,
                SkiaTransform::identity(),
                None,
            );
            for (i, &ma) in mask.data().iter().skip(3).step_by(4).enumerate() {
                if ma == 0 {
                    data[i * 4 + 3] = 0;
                }
            }
        }
    } else {
        if let Some(path) = pb.finish() {
            let mut mask = match Pixmap::new(w, h) {
                Some(p) => p,
                None => return,
            };
            let mut p = Paint::default();
            p.set_color_rgba8(255, 255, 255, 255);
            mask.fill_path(
                &path,
                &p,
                FillRule::Winding,
                SkiaTransform::identity(),
                None,
            );
            for (i, &ma) in mask.data().iter().skip(3).step_by(4).enumerate() {
                if ma == 0 {
                    data[i * 4 + 3] = 0;
                }
            }
        }
    }
}
