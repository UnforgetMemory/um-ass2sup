//! Drawing helpers — decoration lines, layer transforms, clipping, and
//! ASS vector drawings applied to a rendered layer pixmap.

use crate::context::{RenderConfig, RenderContext};
use crate::effects::{apply_clip_mask, apply_drawing_clip_mask};
use crate::transform::AffineTransform;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform as SkiaTransform};

/// Stroke a straight decoration line (e.g. underline/strikeout) onto a pixmap.
pub(crate) fn draw_decoration(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    thickness: f32,
    color: [u8; 4],
) {
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    pb.line_to(x + width, y);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut p = Paint::default();
        p.set_color_rgba8(color[0], color[1], color[2], color[3]);
        let stroke = Stroke {
            width: thickness,
            ..Default::default()
        };
        pixmap.stroke_path(&path, &p, &stroke, SkiaTransform::identity(), None);
    }
}

/// Build a 2D affine transform from RenderContext values (scale, shear, rotation)
/// for transforming a pixmap around its centre.
fn build_layer_transform(lw: u32, lh: u32, ctx: &RenderContext) -> AffineTransform {
    let cx = lw as f32 / 2.0;
    let cy = lh as f32 / 2.0;
    let sx = ctx.scale_x / 100.0;
    let sy = ctx.scale_y / 100.0;

    // Order: translate to origin → scale → shear → rotate → translate back
    // This matches ASS convention where \fscx/\fscy scale the rendered bitmap,
    // \fax/\fay shear it, and \frz/\fr rotates it, all around the bitmap centre.
    AffineTransform::translate(cx, cy)
        .then(&AffineTransform::scale(sx, sy))
        .then(&AffineTransform::shear(ctx.shear_x, ctx.shear_y))
        .then(&AffineTransform::rotate(ctx.rotation))
        .then(&AffineTransform::translate(-cx, -cy))
}

/// Apply the context's transform (scale/shear/rotation/perspective) to a
/// layer buffer, producing the output buffer.
pub(crate) fn transform_layer(
    data: &[u8],
    lw: u32,
    lh: u32,
    w: u32,
    h: u32,
    ctx: &RenderContext,
) -> Vec<u8> {
    let needs_transform = ctx.rotation != 0.0
        || ctx.shear_x != 0.0
        || ctx.shear_y != 0.0
        || (ctx.scale_x - 100.0).abs() > 0.01
        || (ctx.scale_y - 100.0).abs() > 0.01;

    if ctx.perspective_x != 0.0 || ctx.perspective_y != 0.0 {
        let t = if needs_transform {
            build_layer_transform(lw, lh, ctx)
        } else {
            AffineTransform::identity()
        };
        t.apply_with_perspective(
            data,
            lw,
            lh,
            w,
            h,
            ctx.perspective_x,
            ctx.perspective_y,
            ctx.origin_x,
            ctx.origin_y,
        )
    } else if needs_transform {
        let t = build_layer_transform(lw, lh, ctx);
        t.apply_to_pixmap(data, lw, lh, w, h)
    } else {
        data.to_vec()
    }
}

/// Apply the context's clip (vector drawing clip or rectangular clip) to a
/// layer buffer.
pub(crate) fn apply_clip_to_data(
    mut data: Vec<u8>,
    w: u32,
    h: u32,
    ctx: &RenderContext,
    config: &RenderConfig,
) -> Vec<u8> {
    if ctx.clip_drawing_commands.is_some() {
        let sx = config.width as f32 / config.script_width as f32;
        let sy = config.height as f32 / config.script_height as f32;
        apply_drawing_clip_mask(&mut data, w, h, ctx, sx, sy);
    } else {
        apply_clip_mask(&mut data, w, h, ctx);
    }
    data
}

/// Render ASS vector drawing commands (`\p1`) into a pixmap with the
/// context's primary color.
pub(crate) fn render_drawing(pixmap: &mut Pixmap, text: &str, ctx: &RenderContext) {
    let cmds = crate::renderer::drawing::parse_drawing_commands(text);
    if cmds.is_empty() {
        return;
    }
    let mut b = PathBuilder::new();
    for cmd in &cmds {
        match cmd {
            crate::renderer::drawing::DrawingCommand::MoveTo(x, y) => b.move_to(*x, *y),
            crate::renderer::drawing::DrawingCommand::LineTo(x, y) => b.line_to(*x, *y),
            crate::renderer::drawing::DrawingCommand::BezierTo(x1, y1, x2, y2, x, y) => {
                b.cubic_to(*x1, *y1, *x2, *y2, *x, *y)
            }
            crate::renderer::drawing::DrawingCommand::Close => b.close(),
        }
    }
    if let Some(path) = b.finish() {
        let mut p = Paint::default();
        p.set_color_rgba8(
            ctx.primary_color[0],
            ctx.primary_color[1],
            ctx.primary_color[2],
            ctx.primary_color[3],
        );
        p.anti_alias = true;
        pixmap.fill_path(
            &path,
            &p,
            FillRule::Winding,
            SkiaTransform::identity(),
            None,
        );
    }
}
