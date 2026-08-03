//! Glyph compositing — Porter-Duff "over" blending of a rasterized glyph
//! into the per-event layer pixmap.

use crate::font::types::RasterizedGlyph;
use tiny_skia::Pixmap;

/// Composite a rasterized glyph onto `layer` at (x, y) with the given color.
///
/// `x`/`y` are the glyph's pen position; the glyph's own left/top offsets
/// position the bitmap relative to the pen. Per-pixel Porter-Duff "over"
/// with premultiplied-alpha blending.
pub fn composite_glyph(
    layer: &mut Pixmap,
    rasterized: &RasterizedGlyph,
    x: f32,
    y: f32,
    color: [u8; 4],
) {
    let lw = layer.width();
    let lh = layer.height();
    let pix = layer.data_mut();

    tracing::debug!(
        x,
        y,
        rasterized_left = rasterized.left,
        rasterized_top = rasterized.top,
        rasterized_width = rasterized.width,
        rasterized_height = rasterized.height,
        layer_w = lw,
        layer_h = lh,
        "compositing glyph"
    );

    for py in 0..rasterized.height {
        for px in 0..rasterized.width {
            let alpha = rasterized.data[(py * rasterized.width + px) as usize];
            if alpha == 0 {
                continue;
            }
            let tx = x as i32 + rasterized.left + px as i32;
            let ty = y as i32 - rasterized.top + py as i32;
            if tx < 0 || ty < 0 || tx >= lw as i32 || ty >= lh as i32 {
                tracing::trace!(px, py, tx, ty, "pixel out of bounds");
                continue;
            }
            let pi = ((ty as u32 * lw + tx as u32) * 4) as usize;
            let f = alpha as f32 / 255.0;
            let da = pix[pi + 3] as f32 / 255.0;
            let ra = f + da * (1.0 - f);
            for c in 0..3 {
                pix[pi + c] = ((color[c] as f32 * f + pix[pi + c] as f32 * (1.0 - f)) / ra) as u8;
            }
            pix[pi + 3] = (ra * 255.0) as u8;
        }
    }
}
