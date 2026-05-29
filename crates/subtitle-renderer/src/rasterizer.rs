use tiny_skia::{Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};
use crate::shaper::ShapedGlyph;
use crate::context::RenderContext;
use crate::font::FontManager;

pub struct Rasterizer;

impl Rasterizer {
    pub fn rasterize_glyph(
        pixmap: &mut Pixmap,
        font_manager: &FontManager,
        font_id: fontdb::ID,
        glyph: &ShapedGlyph,
        x: f32,
        y: f32,
        ctx: &RenderContext,
    ) {
        let data = match font_manager.get_font_data(font_id) {
            Some(d) => d,
            None => return,
        };

        let face = match ttf_parser::Face::parse(&data, 0) {
            Ok(f) => f,
            Err(_) => return,
        };

        let scale = ctx.font_size / face.units_per_em() as f32;
        let gx = x + glyph.x_offset;
        let gy = y + glyph.y_offset;

        let mut builder = PathBuilder::new();

        if let Some(bbox) = face.glyph_bounding_box(ttf_parser::GlyphId(glyph.glyph_id as u16)) {
            let rx = gx + bbox.x_min as f32 * scale;
            let ry = gy + bbox.y_min as f32 * scale;
            let rw = (bbox.x_max - bbox.x_min) as f32 * scale;
            let rh = (bbox.y_max - bbox.y_min) as f32 * scale;

            if let Some(rect) = Rect::from_xywh(rx, ry, rw, rh) {
                builder.push_rect(rect);
            }
        }

        if let Some(path) = builder.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(ctx.primary_color[0], ctx.primary_color[1], ctx.primary_color[2], ctx.primary_color[3]);
            paint.anti_alias = true;

            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );

            if ctx.outline_width > 0.0 {
                let stroke = Stroke {
                    width: ctx.outline_width * 2.0,
                    ..Default::default()
                };
                let mut outline_paint = Paint::default();
                outline_paint.set_color_rgba8(ctx.outline_color[0], ctx.outline_color[1], ctx.outline_color[2], ctx.outline_color[3]);
                outline_paint.anti_alias = true;

                pixmap.stroke_path(
                    &path,
                    &outline_paint,
                    &stroke,
                    Transform::identity(),
                    None,
                );
            }
        }
    }
}
