//! Glyph rasterization using tiny-skia and ttf-parser.
//!
//! Converts shaped glyph outlines into RGBA pixel data, applying fill color,
//! outline, and shadow as specified by the ASS style.

use tiny_skia::{Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};
use crate::shaper::ShapedGlyph;
use crate::context::RenderContext;
use crate::font::FontManager;

/// Adapter that converts ttf-parser glyph outline commands into tiny-skia path
/// commands, applying font-unit-to-pixel scaling and screen-space translation.
struct OutlineAdapter<'a> {
    builder: &'a mut PathBuilder,
    scale: f32,
    offset_x: f32,
    offset_y: f32,
}

impl ttf_parser::OutlineBuilder for OutlineAdapter<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(self.offset_x + x * self.scale, self.offset_y + y * self.scale);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(self.offset_x + x * self.scale, self.offset_y + y * self.scale);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.builder.quad_to(
            self.offset_x + x1 * self.scale,
            self.offset_y + y1 * self.scale,
            self.offset_x + x * self.scale,
            self.offset_y + y * self.scale,
        );
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.builder.cubic_to(
            self.offset_x + x1 * self.scale,
            self.offset_y + y1 * self.scale,
            self.offset_x + x2 * self.scale,
            self.offset_y + y2 * self.scale,
            self.offset_x + x * self.scale,
            self.offset_y + y * self.scale,
        );
    }

    fn close(&mut self) {
        self.builder.close();
    }
}

/// Rasterizer for converting glyph outlines to RGBA bitmaps.
pub struct Rasterizer;

impl Rasterizer {
    /// Rasterize a single glyph onto the target pixmap.
    ///
    /// Extracts the actual glyph outline from the font via `outline_glyph()`
    /// and fills it with the primary color, then strokes the outline. Falls
    /// back to a filled bounding-box rectangle when the glyph has no outline
    /// data (e.g. bitmap-only glyphs). Applies the scaling and position offset
    /// from the render context.
    ///
    /// # Arguments
    /// * `pixmap` — Target RGBA pixmap
    /// * `font_manager` — Font database for outline extraction
    /// * `font_id` — Font identifier
    /// * `glyph` — Shaped glyph with position/offset info
    /// * `x` — Base X position
    /// * `y` — Base Y position
    /// * `ctx` — Render context with colors, outline width, scale
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
        let glyph_id = ttf_parser::GlyphId(glyph.glyph_id as u16);

        let has_outline = face.outline_glyph(glyph_id, &mut OutlineAdapter {
            builder: &mut builder,
            scale,
            offset_x: gx,
            offset_y: gy,
        }).is_some();

        if !has_outline {
            if let Some(bbox) = face.glyph_bounding_box(glyph_id) {
                let rx = gx + bbox.x_min as f32 * scale;
                let ry = gy + bbox.y_min as f32 * scale;
                let rw = (bbox.x_max - bbox.x_min) as f32 * scale;
                let rh = (bbox.y_max - bbox.y_min) as f32 * scale;

                if let Some(rect) = Rect::from_xywh(rx, ry, rw, rh) {
                    builder.push_rect(rect);
                }
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
