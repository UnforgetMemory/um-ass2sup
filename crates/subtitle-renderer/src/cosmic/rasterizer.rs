//! Cosmic-text glyph rasterizer — SwashImage → tiny-skia Pixmap bridge.
//!
//! Converts swash rasterized glyph images into RGBA pixel data, applying
//! fill color and anisotropic outline via morphological dilation.

use crate::context::RenderContext;
use crate::cosmic::shaper::CosmicShapedGlyph;
use cosmic_text::{CacheKey, CacheKeyFlags, SwashContent, Weight};
use tiny_skia::Pixmap;

/// Rasterize a cosmic-text glyph onto a pixmap at the given position.
///
/// Resolves the glyph image via `swash_cache`, composites the fill using
/// `ctx.primary_color`, and applies an anisotropic outline by dilating the
/// glyph alpha mask when `ctx.outline_width > 0`.
pub fn rasterize_cosmic_glyph(
    pixmap: &mut Pixmap,
    font_system: &mut cosmic_text::FontSystem,
    swash_cache: &mut cosmic_text::SwashCache,
    glyph: &CosmicShapedGlyph,
    x: f32,
    y: f32,
    ctx: &RenderContext,
) {
    if pixmap.width() == 0 || pixmap.height() == 0 {
        return;
    }
    let (cache_key, int_x, int_y) = CacheKey::new(
        glyph.font_id,
        glyph.glyph_id,
        ctx.font_size,
        (x + glyph.x_offset, y + glyph.y_offset),
        Weight::NORMAL,
        CacheKeyFlags::empty(),
    );
    let image = match swash_cache.get_image(font_system, cache_key) {
        Some(img) => img,
        None => return,
    };
    let p = &image.placement;
    let w = p.width as usize;
    let h = p.height as usize;
    if w == 0 || h == 0 || image.data.is_empty() {
        return;
    }
    let dx = int_x + p.left;
    let dy = int_y - p.top;

    type OutlineBuf = (Vec<u8>, Vec<u8>, usize, usize, usize, usize);
    let mut outline: Option<OutlineBuf> = None;

    if ctx.outline_width > 0.0 {
        let ox = ctx.outline_x_width.max(ctx.outline_width).ceil() as usize;
        let oy = ctx.outline_y_width.max(ctx.outline_width).ceil() as usize;
        if ox > 0 || oy > 0 {
            let tw = w + 2 * ox;
            let th = h + 2 * oy;
            let mut mask = vec![0u8; tw * th];
            match image.content {
                SwashContent::Mask | SwashContent::SubpixelMask => {
                    for (i, &a) in image.data.iter().enumerate() {
                        mask[(oy + i / w) * tw + (ox + i % w)] = a;
                    }
                }
                SwashContent::Color => {
                    for (i, px) in image.data.chunks(4).enumerate() {
                        mask[(oy + i / w) * tw + (ox + i % w)] = px[3];
                    }
                }
            }
            let orig = mask.clone();
            if ox > 0 {
                for y in 0..th {
                    for x in 0..tw {
                        let mut mx = 0u8;
                        for kx in x.saturating_sub(ox)..=(x + ox).min(tw - 1) {
                            mx = mx.max(orig[y * tw + kx]);
                        }
                        mask[y * tw + x] = mx;
                    }
                }
            }
            if oy > 0 {
                let col = mask.clone();
                for y in 0..th {
                    for x in 0..tw {
                        let mut mx = 0u8;
                        for ky in y.saturating_sub(oy)..=(y + oy).min(th - 1) {
                            mx = mx.max(col[ky * tw + x]);
                        }
                        mask[y * tw + x] = mx;
                    }
                }
            }
            outline = Some((orig, mask, ox, oy, tw, th));
        }
    }

    match image.content {
        SwashContent::Mask | SwashContent::SubpixelMask => {
            for (i, &a) in image.data.iter().enumerate() {
                if a == 0 {
                    continue;
                }
                let tx = dx + (i % w) as i32;
                let ty = dy + (i / w) as i32;
                if tx < 0 || ty < 0 || tx >= pixmap.width() as i32 || ty >= pixmap.height() as i32 {
                    continue;
                }
                let pi = ((ty as u32 * pixmap.width() + tx as u32) * 4) as usize;
                let pix = pixmap.data_mut();
                let f = f32::from(a) / 255.0;
                let da = f32::from(pix[pi + 3]) / 255.0;
                let ra = f + da * (1.0 - f);
                for c in 0..3 {
                    pix[pi + c] = ((f32::from(ctx.primary_color[c]) * f
                        + f32::from(pix[pi + c]) * (1.0 - f))
                        / ra) as u8;
                }
                pix[pi + 3] = (ra * 255.0) as u8;
            }
        }
        SwashContent::Color => {
            for (i, px) in image.data.chunks(4).enumerate() {
                let tx = dx + (i % w) as i32;
                let ty = dy + (i / w) as i32;
                if tx < 0 || ty < 0 || tx >= pixmap.width() as i32 || ty >= pixmap.height() as i32 {
                    continue;
                }
                let pi = ((ty as u32 * pixmap.width() + tx as u32) * 4) as usize;
                let pix = pixmap.data_mut();
                let sa = f32::from(px[3]) / 255.0;
                let da = f32::from(pix[pi + 3]) / 255.0;
                let ra = sa + da * (1.0 - sa);
                for c in 0..3 {
                    pix[pi + c] =
                        ((f32::from(px[c]) * sa + f32::from(pix[pi + c]) * (1.0 - sa)) / ra) as u8;
                }
                pix[pi + 3] = (ra * 255.0) as u8;
            }
        }
    }

    if let Some((orig, dilated, ox, oy, tw, th)) = outline {
        let pw = pixmap.width() as usize;
        let ph = pixmap.height() as usize;
        let pix = pixmap.data_mut();
        for py in 0..th {
            for px in 0..tw {
                let i = py * tw + px;
                let fa = orig[i];
                let da = dilated[i];
                if da > fa {
                    let of = (f32::from(da) - f32::from(fa)) / 255.0;
                    if of > 1.0 / 256.0 {
                        let tx = dx - ox as i32 + px as i32;
                        let ty = dy - oy as i32 + py as i32;
                        if tx < 0 || ty < 0 || tx >= pw as i32 || ty >= ph as i32 {
                            continue;
                        }
                        let pi = ((ty as u32 * pw as u32 + tx as u32) * 4) as usize;
                        let da = f32::from(pix[pi + 3]) / 255.0;
                        let ra = of + da * (1.0 - of);
                        for c in 0..3 {
                            pix[pi + c] = ((f32::from(ctx.outline_color[c]) * of
                                + f32::from(pix[pi + c]) * (1.0 - of))
                                / ra) as u8;
                        }
                        pix[pi + 3] = (ra * 255.0) as u8;
                    }
                }
            }
        }
    }
}
