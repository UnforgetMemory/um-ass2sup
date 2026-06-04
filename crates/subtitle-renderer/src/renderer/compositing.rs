use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform as SkiaTransform};
use crate::context::RenderContext;
use crate::shaper::Shaper;
use super::ShapedLine;
use super::drawing::{DrawingCommand, parse_drawing_commands};

pub(super) fn apply_alpha_multiplier(data: &mut [u8], alpha: f32) {
    let factor = alpha.clamp(0.0, 1.0);
    if factor >= 1.0 {
        return; // no-op
    }
    if factor <= 0.0 {
        // Zero all alpha channels
        for i in (3..data.len()).step_by(4) {
            data[i] = 0;
        }
        return;
    }

    let factor_256 = (factor * 256.0) as u32;

    // Process 4 pixels at a time (16 bytes)
    let chunks = data.len() / 16;
    for chunk in 0..chunks {
        let base = chunk * 16;
        for &offset in &[3usize, 7, 11, 15] {
            let idx = base + offset;
            data[idx] = ((u32::from(data[idx]) * factor_256) >> 8) as u8;
        }
    }

    // Handle remaining pixels
    let remaining_start = chunks * 16;
    for i in (remaining_start + 3..data.len()).step_by(4) {
        data[i] = ((u32::from(data[i]) * factor_256) >> 8) as u8;
    }
}

pub(super) fn apply_clip_mask(data: &mut [u8], w: u32, h: u32, ctx: &RenderContext) {
    let x1 = ctx.clip_x1.max(0.0) as u32;
    let y1 = ctx.clip_y1.max(0.0) as u32;
    let x2 = ctx.clip_x2.max(0.0).min(w as f32) as u32;
    let y2 = ctx.clip_y2.max(0.0).min(h as f32) as u32;

    for py in 0..h {
        for px in 0..w {
            let inside = px >= x1 && px < x2 && py >= y1 && py < y2;
            let clear = if ctx.clip_inverse { inside } else { !inside };
            if clear {
                let idx = ((py * w + px) * 4) as usize;
                data[idx] = 0;
                data[idx + 1] = 0;
                data[idx + 2] = 0;
                data[idx + 3] = 0;
            }
        }
    }
}

/// Apply a vector drawing clip mask to pixel data.
///
/// Parses `.clip_drawing_commands` and builds a tiny_skia path, then clears
/// pixels outside (or inside, for inverse clips) the filled path.
pub(super) fn apply_drawing_clip_mask(data: &mut [u8], w: u32, h: u32, ctx: &RenderContext, sx: f32, sy: f32) {
    let commands_text = match ctx.clip_drawing_commands {
        Some(ref c) => c,
        None => return,
    };

    let commands = parse_drawing_commands(commands_text);
    let scale = 1.0 / ctx.clip_drawing_scale;

    let mut pb = PathBuilder::new();
    for cmd in &commands {
        match cmd {
            DrawingCommand::MoveTo(x, y) => pb.move_to(x * scale * sx, y * scale * sy),
            DrawingCommand::LineTo(x, y) => pb.line_to(x * scale * sx, y * scale * sy),
            DrawingCommand::BezierTo(x1, y1, x2, y2, x3, y3) => {
                pb.cubic_to(
                    x1 * scale * sx, y1 * scale * sy,
                    x2 * scale * sx, y2 * scale * sy,
                    x3 * scale * sx, y3 * scale * sy,
                );
            }
            DrawingCommand::Close => pb.close(),
        }
    }

    if let Some(path) = pb.finish() {
        let mut mask = if let Some(p) = Pixmap::new(w, h) { p } else { return; };
        let mut paint = Paint::default();
        paint.set_color_rgba8(255, 255, 255, 255);
        paint.anti_alias = true;
        mask.fill_path(&path, &paint, FillRule::EvenOdd, SkiaTransform::identity(), None);

        for py in 0..h {
            for px in 0..w {
                let idx = ((py * w + px) * 4 + 3) as usize;
                let inside = mask.data()[idx] > 0;
                let clear = if ctx.clip_drawing_inverse { inside } else { !inside };
                if clear {
                    let base = idx - 3;
                    data[base] = 0;
                    data[base + 1] = 0;
                    data[base + 2] = 0;
                    data[base + 3] = 0;
                }
            }
        }
    }
}

/// Composite a sub-region source buffer into a larger destination buffer.
///
/// Performs Porter-Duff "over" compositing of a (`sw` × `sh`) source image
/// positioned at (`sx`, `sy`) in the (`dw` × `dh`) destination image.
/// The source and destination pixel data are in RGBA byte order.
#[allow(clippy::too_many_arguments)]
pub(super) fn composite_subregion(
    dst: &mut [u8],
    src: &[u8],
    dst_w: u32,
    dst_h: u32,
    src_x: u32,
    src_y: u32,
    src_w: u32,
    src_h: u32,
) {
    for ry in 0..src_h {
        let dy = src_y + ry;
        if dy >= dst_h {
            continue;
        }
        for rx in 0..src_w {
            let dx = src_x + rx;
            if dx >= dst_w {
                continue;
            }

            let si = (ry * src_w + rx) as usize * 4;
            let di = (dy * dst_w + dx) as usize * 4;

            let sa = u32::from(src[si + 3]);
            if sa == 0 {
                continue;
            }

            if sa == 255 {
                // Fully opaque source — direct copy, no blending needed
                dst[di] = src[si];
                dst[di + 1] = src[si + 1];
                dst[di + 2] = src[si + 2];
                dst[di + 3] = 255;
                continue;
            }

            let da = u32::from(dst[di + 3]);
            let inv_sa = 255 - sa;
            let out_a = sa + da * inv_sa / 255;
            if out_a == 0 {
                continue;
            }

            dst[di]     = ((u32::from(src[si]) * sa + u32::from(dst[di]) * da * inv_sa / 255) / out_a) as u8;
            dst[di + 1] = ((u32::from(src[si + 1]) * sa + u32::from(dst[di + 1]) * da * inv_sa / 255) / out_a) as u8;
            dst[di + 2] = ((u32::from(src[si + 2]) * sa + u32::from(dst[di + 2]) * da * inv_sa / 255) / out_a) as u8;
            dst[di + 3] = out_a as u8;
        }
    }
}

/// Compute the tight ink bounding box of all glyphs in `shaped_lines`.
///
/// Returns `(min_x, min_y, max_x, max_y)` in the layer's pixel coordinate space
/// (before any shadow/blur/border padding). Returns `None` if no glyph bbox can
/// be determined (triggers fallback to full-frame allocation).
pub(super) fn compute_tight_bbox(
    shaped_lines: &[ShapedLine],
    shaper: &Shaper,
    font_id: fontdb::ID,
    font_size: f32,
    ctx: &RenderContext,
) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut any_glyph = false;

    for sl in shaped_lines {
        let mut x = sl.x_start;
        for glyph in &sl.shaped.glyphs {
            if let Some(bbox) = shaper.get_glyph_bbox(font_id, glyph.glyph_id, font_size) {
                any_glyph = true;
                let gx = x + glyph.x_offset;
                let gy = sl.line_y + glyph.y_offset;
                min_x = min_x.min(gx + bbox.x_min);
                min_y = min_y.min(gy + bbox.y_min);
                max_x = max_x.max(gx + bbox.x_max);
                max_y = max_y.max(gy + bbox.y_max);
            }
            x += glyph.x_advance;
        }

        // Account for underline / strikeout lines.
        if ctx.underline {
            let uy = sl.line_y + ctx.font_size * 0.1;
            min_y = min_y.min(uy - 2.0);
            max_y = max_y.max(uy + 2.0);
            min_x = min_x.min(sl.x_start);
            max_x = max_x.max(sl.x_start + sl.shaped.total_advance);
            any_glyph = true;
        }
        if ctx.strikeout {
            let sy = sl.line_y - ctx.font_size * 0.35;
            min_y = min_y.min(sy - 2.0);
            max_y = max_y.max(sy + 2.0);
            min_x = min_x.min(sl.x_start);
            max_x = max_x.max(sl.x_start + sl.shaped.total_advance);
            any_glyph = true;
        }
    }

    if !any_glyph {
        return None;
    }
    Some((min_x, min_y, max_x, max_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alpha_multiplier_full() {
        let mut data = vec![255, 255, 255, 200, 128, 128, 128, 100];
        apply_alpha_multiplier(&mut data, 1.0);
        assert_eq!(data[3], 200); // unchanged
        assert_eq!(data[7], 100); // unchanged
    }

    #[test]
    fn test_alpha_multiplier_half() {
        let mut data = vec![255, 255, 255, 200, 128, 128, 128, 100];
        apply_alpha_multiplier(&mut data, 0.5);
        assert_eq!(data[3], 100); // 200 * 0.5
        assert_eq!(data[7], 50);  // 100 * 0.5
    }

    #[test]
    fn test_alpha_multiplier_zero() {
        let mut data = vec![255, 255, 255, 200];
        apply_alpha_multiplier(&mut data, 0.0);
        assert_eq!(data[3], 0);
    }

    #[test]
    fn test_alpha_multiplier_only_alpha() {
        let mut data = vec![100, 150, 200, 160];
        apply_alpha_multiplier(&mut data, 0.5);
        assert_eq!(data[0], 100); // R unchanged
        assert_eq!(data[1], 150); // G unchanged
        assert_eq!(data[2], 200); // B unchanged
        assert_eq!(data[3], 80);  // A halved
    }

    // ── apply_clip_mask ───────────────────────────────────────

    #[test]
    fn test_clip_mask_normal_inside_preserved() {
        let mut data = vec![0u8; 4 * 4 * 4]; // 4x4 RGBA image
        // Fill all pixels with white
        for i in 0..16 {
            data[i * 4] = 255;
            data[i * 4 + 1] = 255;
            data[i * 4 + 2] = 255;
            data[i * 4 + 3] = 255;
        }
        let ctx = RenderContext {
            clip_enabled: true,
            clip_x1: 1.0,
            clip_y1: 1.0,
            clip_x2: 3.0,
            clip_y2: 3.0,
            clip_inverse: false,
            ..Default::default()
        };
        apply_clip_mask(&mut data, 4, 4, &ctx);
        // Inside clip: pixel (1,1) should be preserved
        let inside_idx = ((4 + 1) * 4) as usize;
        assert_eq!(data[inside_idx + 3], 255);
        // Outside clip: pixel (0,0) should be cleared
        assert_eq!(data[3], 0);
    }

    #[test]
    fn test_clip_mask_inverse_inside_cleared() {
        let mut data = vec![0u8; 4 * 4 * 4]; // 4x4 image
        for i in 0..16 {
            data[i * 4] = 255;
            data[i * 4 + 1] = 255;
            data[i * 4 + 2] = 255;
            data[i * 4 + 3] = 255;
        }
        let ctx = RenderContext {
            clip_enabled: true,
            clip_x1: 1.0,
            clip_y1: 1.0,
            clip_x2: 3.0,
            clip_y2: 3.0,
            clip_inverse: true,
            ..Default::default()
        };
        apply_clip_mask(&mut data, 4, 4, &ctx);
        // Inside clip: pixel (1,1) should be CLEARED
        let inside_idx = ((4 + 1) * 4) as usize;
        assert_eq!(data[inside_idx + 3], 0);
        // Outside clip: pixel (0,0) should be PRESERVED
        assert_eq!(data[3], 255);
    }

    // ── apply_drawing_clip_mask ──────────────────────────────────

    #[test]
    fn test_drawing_clip_normal_triangle() {
        let w = 10u32;
        let h = 10u32;
        let mut data = vec![255u8; (w * h * 4) as usize];

        let ctx = RenderContext {
            clip_drawing_commands: Some("m 5 0 l 10 10 l 0 10".to_string()),
            clip_drawing_scale: 1.0,
            clip_drawing_inverse: false,
            ..Default::default()
        };

        apply_drawing_clip_mask(&mut data, w, h, &ctx, 1.0, 1.0);

        // Center of triangle (roughly) should be preserved
        let inside_alpha = data[((5 * w + 5) * 4 + 3) as usize];
        assert_eq!(inside_alpha, 255, "pixel inside triangle should be preserved");

        // Top-left corner should be cleared (outside triangle)
        let outside_alpha = data[3];
        assert_eq!(outside_alpha, 0, "pixel outside triangle should be cleared");
    }

    #[test]
    fn test_drawing_clip_inverse_triangle() {
        let w = 10u32;
        let h = 10u32;
        let mut data = vec![255u8; (w * h * 4) as usize];

        let ctx = RenderContext {
            clip_drawing_commands: Some("m 5 0 l 10 10 l 0 10".to_string()),
            clip_drawing_scale: 1.0,
            clip_drawing_inverse: true,
            ..Default::default()
        };

        apply_drawing_clip_mask(&mut data, w, h, &ctx, 1.0, 1.0);

        // Center of triangle should be cleared (inverse)
        let inside_alpha = data[((5 * w + 5) * 4 + 3) as usize];
        assert_eq!(inside_alpha, 0, "pixel inside triangle should be cleared for inverse");

        // Top-left corner should be preserved (outside triangle)
        let outside_alpha = data[3];
        assert_eq!(outside_alpha, 255, "pixel outside triangle should be preserved for inverse");
    }

    #[test]
    fn test_drawing_clip_scaled_coordinates() {
        let w = 20u32;
        let h = 20u32;
        let mut data = vec![255u8; (w * h * 4) as usize];

        let ctx = RenderContext {
            // Scale=2 means coordinates are halved: m 5 0 l 10 10 l 0 10 becomes m 2.5 0 l 5 5 l 0 5
            clip_drawing_commands: Some("m 5 0 l 10 10 l 0 10".to_string()),
            clip_drawing_scale: 2.0,
            clip_drawing_inverse: false,
            ..Default::default()
        };

        apply_drawing_clip_mask(&mut data, w, h, &ctx, 1.0, 1.0);

        // Point (3,3) should be inside the scaled triangle
        let inside_alpha = data[((3 * w + 3) * 4 + 3) as usize];
        assert_eq!(inside_alpha, 255, "pixel inside scaled triangle should be preserved");

        // Point (9,9) should be outside the scaled triangle
        let outside_alpha = data[((9 * w + 9) * 4 + 3) as usize];
        assert_eq!(outside_alpha, 0, "pixel outside scaled triangle should be cleared");
    }

    // ── edge cases ────────────────────────────────────────────

    #[test]
    fn test_alpha_multiplier_empty_data() {
        let mut data: Vec<u8> = vec![];
        apply_alpha_multiplier(&mut data, 0.5);
        assert!(data.is_empty());
    }

    #[test]
    fn test_alpha_multiplier_clamp_over_1() {
        let mut data = vec![100, 150, 200, 160];
        apply_alpha_multiplier(&mut data, 2.0);
        assert_eq!(data[3], 160);
    }

    #[test]
    fn test_alpha_multiplier_clamp_under_0() {
        let mut data = vec![100, 150, 200, 160];
        apply_alpha_multiplier(&mut data, -0.5);
        assert_eq!(data[3], 0);
    }

    #[test]
    fn test_clip_mask_out_of_bounds_clip() {
        let mut data = vec![255u8; 4 * 4 * 4];
        let ctx = RenderContext {
            clip_enabled: true,
            clip_x1: 10.0,
            clip_y1: 10.0,
            clip_x2: 20.0,
            clip_y2: 20.0,
            clip_inverse: false,
            ..Default::default()
        };
        apply_clip_mask(&mut data, 4, 4, &ctx);
        assert_eq!(data[3], 0);
    }

    #[test]
    fn test_clip_mask_full_image_clip() {
        let mut data = vec![255u8; 4 * 4 * 4];
        let ctx = RenderContext {
            clip_enabled: true,
            clip_x1: 0.0,
            clip_y1: 0.0,
            clip_x2: 4.0,
            clip_y2: 4.0,
            clip_inverse: false,
            ..Default::default()
        };
        apply_clip_mask(&mut data, 4, 4, &ctx);
        assert_eq!(data[3], 255);
    }

    #[test]
    fn test_clip_mask_negative_coords_clamped() {
        let mut data = vec![255u8; 4 * 4 * 4];
        let ctx = RenderContext {
            clip_enabled: true,
            clip_x1: -5.0,
            clip_y1: -5.0,
            clip_x2: 2.0,
            clip_y2: 2.0,
            clip_inverse: false,
            ..Default::default()
        };
        apply_clip_mask(&mut data, 4, 4, &ctx);
        assert_eq!(data[3], 255);
    }

    #[test]
    fn test_composite_subregion_empty_source() {
        let mut dst = vec![255u8; 4 * 4 * 4];
        let src: Vec<u8> = vec![];
        composite_subregion(&mut dst, &src, 4, 4, 0, 0, 0, 0);
        assert_eq!(dst[3], 255);
    }

    #[test]
    fn test_composite_subregion_transparent_source() {
        let mut dst = vec![255u8; 4 * 4 * 4];
        let src = vec![0u8; 4 * 2 * 2];
        composite_subregion(&mut dst, &src, 4, 4, 0, 0, 2, 2);
        assert_eq!(dst[3], 255);
    }

    #[test]
    fn test_drawing_clip_no_commands() {
        let mut data = vec![255u8; 4 * 4 * 4];
        let ctx = RenderContext::default();
        apply_drawing_clip_mask(&mut data, 4, 4, &ctx, 1.0, 1.0);
        assert_eq!(data[3], 255);
    }

    #[test]
    fn test_drawing_clip_empty_path() {
        let mut data = vec![255u8; 4 * 4 * 4];
        let ctx = RenderContext {
            clip_drawing_commands: Some("".to_string()),
            clip_drawing_scale: 1.0,
            clip_drawing_inverse: false,
            ..Default::default()
        };
        apply_drawing_clip_mask(&mut data, 4, 4, &ctx, 1.0, 1.0);
        assert_eq!(data[3], 255);
    }
}
