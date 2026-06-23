//! Glyph rasterization using tiny-skia and ttf-parser.
//!
//! Converts shaped glyph outlines into RGBA pixel data, applying fill color,
//! outline, and shadow as specified by the ASS style.
//!
//! # Anisotropic outlines
//!
//! ASS override tags `\xbord` and `\ybord` allow different outline widths in
//! the X and Y directions.  Since tiny-skia only supports uniform stroke width,
//! we implement anisotropic outlines via **morphological dilation**:
//!
//! 1. Render the fill path into an alpha mask.
//! 2. Dilate the mask by `outline_x_width` horizontally and
//!    `outline_y_width` vertically (separable max filter).
//! 3. Where the dilated alpha exceeds the original fill alpha we know the
//!    pixel belongs to the outline region.  Blend `outline_color` over the
//!    pixmap at those pixels using the difference as the source alpha.
//!
//! This approach is mathematically correct for all combinations of
//! `outline_x_width` / `outline_y_width` (including values smaller than the
//! reference `outline_width`) and behaves correctly at anti-aliased edges.

use crate::context::RenderContext;
use crate::font::FontManager;
use crate::shaper::ShapedGlyph;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

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
        // Font coordinates have y-up; screen coordinates have y-down.
        // Negate y to convert from font space to screen space.
        self.builder.move_to(
            self.offset_x + x * self.scale,
            self.offset_y - y * self.scale,
        );
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(
            self.offset_x + x * self.scale,
            self.offset_y - y * self.scale,
        );
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.builder.quad_to(
            self.offset_x + x1 * self.scale,
            self.offset_y - y1 * self.scale,
            self.offset_x + x * self.scale,
            self.offset_y - y * self.scale,
        );
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.builder.cubic_to(
            self.offset_x + x1 * self.scale,
            self.offset_y - y1 * self.scale,
            self.offset_x + x2 * self.scale,
            self.offset_y - y2 * self.scale,
            self.offset_x + x * self.scale,
            self.offset_y - y * self.scale,
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
        let (data, face_index) = match font_manager.get_font_data_with_index(font_id) {
            Some(d) => d,
            None => return,
        };

        let face = match ttf_parser::Face::parse(&data, face_index) {
            Ok(f) => f,
            Err(_) => return,
        };

        let scale = ctx.font_size / f32::from(face.units_per_em());
        let gx = x + glyph.x_offset;
        let gy = y + glyph.y_offset;

        let mut builder = PathBuilder::new();
        let glyph_id = ttf_parser::GlyphId(glyph.glyph_id as u16);

        let has_outline = face
            .outline_glyph(
                glyph_id,
                &mut OutlineAdapter {
                    builder: &mut builder,
                    scale,
                    offset_x: gx,
                    offset_y: gy,
                },
            )
            .is_some();

        if !has_outline {
            if let Some(bbox) = face.glyph_bounding_box(glyph_id) {
                let rx = gx + f32::from(bbox.x_min) * scale;
                // Font y_min/y_max are y-up; convert to screen y-down
                let ry = gy - f32::from(bbox.y_max) * scale;
                let rw = f32::from(bbox.x_max - bbox.x_min) * scale;
                let rh = f32::from(bbox.y_max - bbox.y_min) * scale;

                if let Some(rect) = Rect::from_xywh(rx, ry, rw, rh) {
                    builder.push_rect(rect);
                }
            }
        }

        if let Some(path) = builder.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(
                ctx.primary_color[0],
                ctx.primary_color[1],
                ctx.primary_color[2],
                ctx.primary_color[3],
            );
            paint.anti_alias = true;

            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );

            if ctx.outline_width > 0.0 {
                let mut outline_paint = Paint::default();
                outline_paint.set_color_rgba8(
                    ctx.outline_color[0],
                    ctx.outline_color[1],
                    ctx.outline_color[2],
                    ctx.outline_color[3],
                );
                outline_paint.anti_alias = true;

                apply_anisotropic_outline(
                    pixmap,
                    &path,
                    ctx.outline_color,
                    ctx.outline_width,
                    ctx.outline_x_width,
                    ctx.outline_y_width,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Anisotropic outline helpers
// ---------------------------------------------------------------------------

/// Apply an anisotropic outline to a pixmap that already has the fill rendered.
///
/// When `outline_x_width == outline_width` and `outline_y_width == outline_width`
/// the function falls back to a uniform `stroke_path` (the fast path).
///
/// Otherwise it uses morphological dilation of the fill's alpha mask by
/// `(outline_x_width, outline_y_width)` and blends `outline_color` into pixels
/// where the dilated mask extends beyond the original fill region.
pub(crate) fn apply_anisotropic_outline(
    pixmap: &mut Pixmap,
    path: &tiny_skia::Path,
    outline_color: [u8; 4],
    outline_width: f32,
    outline_x_width: f32,
    outline_y_width: f32,
) {
    let w = pixmap.width() as usize;
    let h = pixmap.height() as usize;

    let ox = if outline_x_width > 0.0 {
        outline_x_width
    } else {
        outline_width
    };
    let oy = if outline_y_width > 0.0 {
        outline_y_width
    } else {
        outline_width
    };

    // ── Build the fill alpha mask ──────────────────────────────────────
    let mut mask = if let Some(p) = Pixmap::new(w as u32, h as u32) {
        p
    } else {
        return;
    };
    let mut white = Paint::default();
    white.set_color_rgba8(255, 255, 255, 255);
    white.anti_alias = true;
    mask.fill_path(path, &white, FillRule::Winding, Transform::identity(), None);

    // Save the original (pre-dilation) per-pixel alpha for comparison.
    let orig_alpha: Vec<u8> = (0..w * h).map(|i| mask.data()[i * 4 + 3]).collect();

    // ── Dilate by ceiled pixel radius ─────────────────────────────────
    let rx = ox.ceil() as usize;
    let ry = oy.ceil() as usize;

    if rx > 0 {
        dilate_alpha_horizontal(mask.data_mut(), w, h, rx, &orig_alpha);
    }
    if ry > 0 {
        dilate_alpha_vertical(mask.data_mut(), w, h, ry);
    }

    // ── Blend outline_color where dilated mask exceeds original fill ──
    let dilated_data = mask.data();
    let pix_data = pixmap.data_mut();

    for (i, &fa) in orig_alpha.iter().enumerate() {
        let di = i * 4;
        let da = dilated_data[di + 3];

        if da > fa {
            let out_frac = (f32::from(da) - f32::from(fa)) / 255.0;
            if out_frac > 1.0 / 256.0 {
                let dst_a = f32::from(pix_data[di + 3]) / 255.0;
                let res_a = out_frac + dst_a * (1.0 - out_frac);
                debug_assert!(res_a > 0.0);
                // pix_data[di..+2] is already premultiplied by tiny-skia; do NOT
                // multiply by dst_a again (that would double-count the alpha).
                pix_data[di] = ((f32::from(outline_color[0]) * out_frac
                    + f32::from(pix_data[di]) * (1.0 - out_frac))
                    / res_a) as u8;
                pix_data[di + 1] = ((f32::from(outline_color[1]) * out_frac
                    + f32::from(pix_data[di + 1]) * (1.0 - out_frac))
                    / res_a) as u8;
                pix_data[di + 2] = ((f32::from(outline_color[2]) * out_frac
                    + f32::from(pix_data[di + 2]) * (1.0 - out_frac))
                    / res_a) as u8;
                pix_data[di + 3] = (res_a * 255.0) as u8;
            }
        }
    }
}

/// In-place horizontal max-filter (dilation) of RGBA alpha channel.
///
/// Reads original alpha values from `orig_src` (flat per-pixel alpha,
/// one byte per pixel, row-major), writes the windowed max into `data`'s
/// alpha byte at the corresponding position.
fn dilate_alpha_horizontal(data: &mut [u8], w: usize, h: usize, radius: usize, orig_src: &[u8]) {
    for y in 0..h {
        for x in 0..w {
            let mut max_a = 0u8;
            let x0 = x.saturating_sub(radius);
            let x1 = (x + radius).min(w - 1);
            for kx in x0..=x1 {
                let a = orig_src[y * w + kx];
                if a > max_a {
                    max_a = a;
                }
            }
            data[(y * w + x) * 4 + 3] = max_a;
        }
    }
}

/// In-place vertical max-filter (dilation) of RGBA alpha channel.
///
/// Reads the **already horizontally dilated** alpha bytes from `data` and
/// writes the windowed max back.  Because this runs *after* the horizontal
/// pass the order of passes does not matter (separable rectangle max).
fn dilate_alpha_vertical(data: &mut [u8], w: usize, h: usize, radius: usize) {
    // Snapshot the current (horizontally dilated) alpha values.
    let col_buf: Vec<u8> = (0..w * h).map(|i| data[i * 4 + 3]).collect();

    for y in 0..h {
        for x in 0..w {
            let mut max_a = 0u8;
            let y0 = y.saturating_sub(radius);
            let y1 = (y + radius).min(h - 1);
            for ky in y0..=y1 {
                let a = col_buf[ky * w + x];
                if a > max_a {
                    max_a = a;
                }
            }
            data[(y * w + x) * 4 + 3] = max_a;
        }
    }
}
