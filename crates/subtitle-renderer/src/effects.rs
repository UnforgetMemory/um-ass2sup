//! Post-processing effects for subtitle rendering.
//!
//! Provides blur, shadow, and alpha compositing operations used by the
//! subtitle renderer to implement ASS override tags like `\be`, `\bord`,
//! `\shad`, and `\fad`.

use tiny_skia::Pixmap;
use wide::u32x4;

/// Apply an approximated box blur to a pixmap in-place.
///
/// Uses a separable 1D horizontal pass followed by a 1D vertical pass.
/// Larger radii produce stronger blur at higher computational cost (O(n * r)).
///
/// # Arguments
/// * `pixmap` — RGBA pixmap to blur in-place
/// * `radius` — Blur radius in pixels; values ≤ 0.0 are a no-op
///
/// This effect is used for the ASS `\be` (edge blur) and `\blur` tags.
pub fn apply_gaussian_blur(pixmap: &mut Pixmap, radius: f32) {
    if radius <= 0.0 {
        return;
    }
    let r = radius.ceil() as u32;
    let w = pixmap.width() as usize;
    let h = pixmap.height() as usize;
    let data = pixmap.data_mut();

    let mut temp = vec![0u8; data.len()];

    let r_u = r as usize;
    let count_val = 2 * r + 1;

    // ======== Horizontal pass ========
    let interior_x_end = w.saturating_sub(r_u);
    for y in 0..h {
        // Left edge — scalar (variable pixel count, includes bounds checks)
        for x in 0..r_u.min(w) {
            let (mut sr, mut sg, mut sb, mut sa, mut count) = (0u32, 0u32, 0u32, 0u32, 0u32);
            for dx in -(r as i32)..=(r as i32) {
                let nx = x as i32 + dx;
                if nx >= 0 && nx < w as i32 {
                    let idx = (y * w + nx as usize) * 4;
                    sr += data[idx] as u32;
                    sg += data[idx + 1] as u32;
                    sb += data[idx + 2] as u32;
                    sa += data[idx + 3] as u32;
                    count += 1;
                }
            }
            let idx = (y * w + x) * 4;
            temp[idx] = (sr / count) as u8;
            temp[idx + 1] = (sg / count) as u8;
            temp[idx + 2] = (sb / count) as u8;
            temp[idx + 3] = (sa / count) as u8;
        }

        // Interior — SIMD batch of 4, fixed count = 2*r+1
        let mut x = r_u;
        while x + 4 <= interior_x_end {
            let mut srv = u32x4::splat(0);
            let mut sgv = u32x4::splat(0);
            let mut sbv = u32x4::splat(0);
            let mut sav = u32x4::splat(0);

            for dx in -(r as i32)..=(r as i32) {
                let nx = (x as i32 + dx) as usize;
                let base = (y * w + nx) * 4;

                srv += u32x4::from([data[base] as u32, data[base + 4] as u32,
                                    data[base + 8] as u32, data[base + 12] as u32]);
                sgv += u32x4::from([data[base + 1] as u32, data[base + 5] as u32,
                                    data[base + 9] as u32, data[base + 13] as u32]);
                sbv += u32x4::from([data[base + 2] as u32, data[base + 6] as u32,
                                    data[base + 10] as u32, data[base + 14] as u32]);
                sav += u32x4::from([data[base + 3] as u32, data[base + 7] as u32,
                                    data[base + 11] as u32, data[base + 15] as u32]);
            }

            let r_arr = srv.to_array();
            let g_arr = sgv.to_array();
            let b_arr = sbv.to_array();
            let a_arr = sav.to_array();
            for i in 0..4 {
                let idx = (y * w + x + i) * 4;
                temp[idx] = (r_arr[i] / count_val) as u8;
                temp[idx + 1] = (g_arr[i] / count_val) as u8;
                temp[idx + 2] = (b_arr[i] / count_val) as u8;
                temp[idx + 3] = (a_arr[i] / count_val) as u8;
            }
            x += 4;
        }

        // Interior remainder — scalar
        while x < interior_x_end {
            let (mut sr, mut sg, mut sb, mut sa) = (0u32, 0u32, 0u32, 0u32);
            for dx in -(r as i32)..=(r as i32) {
                let nx = (x as i32 + dx) as usize;
                let idx = (y * w + nx) * 4;
                sr += data[idx] as u32;
                sg += data[idx + 1] as u32;
                sb += data[idx + 2] as u32;
                sa += data[idx + 3] as u32;
            }
            let idx = (y * w + x) * 4;
            temp[idx] = (sr / count_val) as u8;
            temp[idx + 1] = (sg / count_val) as u8;
            temp[idx + 2] = (sb / count_val) as u8;
            temp[idx + 3] = (sa / count_val) as u8;
            x += 1;
        }

        // Right edge — scalar (variable pixel count, includes bounds checks)
        for x in interior_x_end..w {
            let (mut sr, mut sg, mut sb, mut sa, mut count) = (0u32, 0u32, 0u32, 0u32, 0u32);
            for dx in -(r as i32)..=(r as i32) {
                let nx = x as i32 + dx;
                if nx >= 0 && nx < w as i32 {
                    let idx = (y * w + nx as usize) * 4;
                    sr += data[idx] as u32;
                    sg += data[idx + 1] as u32;
                    sb += data[idx + 2] as u32;
                    sa += data[idx + 3] as u32;
                    count += 1;
                }
            }
            let idx = (y * w + x) * 4;
            temp[idx] = (sr / count) as u8;
            temp[idx + 1] = (sg / count) as u8;
            temp[idx + 2] = (sb / count) as u8;
            temp[idx + 3] = (sa / count) as u8;
        }
    }

    data.copy_from_slice(&temp);

    // ======== Vertical pass ========
    let interior_y_end = h.saturating_sub(r_u);
    for y in 0..h {
        if y < r_u || y >= interior_y_end {
            // Top / bottom edge — scalar (variable pixel count)
            for x in 0..w {
                let (mut sr, mut sg, mut sb, mut sa, mut count) = (0u32, 0u32, 0u32, 0u32, 0u32);
                for dy in -(r as i32)..=(r as i32) {
                    let ny = y as i32 + dy;
                    if ny >= 0 && ny < h as i32 {
                        let idx = (ny as usize * w + x) * 4;
                        sr += data[idx] as u32;
                        sg += data[idx + 1] as u32;
                        sb += data[idx + 2] as u32;
                        sa += data[idx + 3] as u32;
                        count += 1;
                    }
                }
                let idx = (y * w + x) * 4;
                data[idx] = (sr / count) as u8;
                data[idx + 1] = (sg / count) as u8;
                data[idx + 2] = (sb / count) as u8;
                data[idx + 3] = (sa / count) as u8;
            }
        } else {
            // Interior — SIMD batch over x, fixed count
            let mut x = 0;
            while x + 4 <= w {
                let mut srv = u32x4::splat(0);
                let mut sgv = u32x4::splat(0);
                let mut sbv = u32x4::splat(0);
                let mut sav = u32x4::splat(0);

                for dy in -(r as i32)..=(r as i32) {
                    let ny = (y as i32 + dy) as usize;
                    let base = (ny * w + x) * 4;

                    srv += u32x4::from([data[base] as u32, data[base + 4] as u32,
                                        data[base + 8] as u32, data[base + 12] as u32]);
                    sgv += u32x4::from([data[base + 1] as u32, data[base + 5] as u32,
                                        data[base + 9] as u32, data[base + 13] as u32]);
                    sbv += u32x4::from([data[base + 2] as u32, data[base + 6] as u32,
                                        data[base + 10] as u32, data[base + 14] as u32]);
                    sav += u32x4::from([data[base + 3] as u32, data[base + 7] as u32,
                                        data[base + 11] as u32, data[base + 15] as u32]);
                }

                let r_arr = srv.to_array();
                let g_arr = sgv.to_array();
                let b_arr = sbv.to_array();
                let a_arr = sav.to_array();
                for i in 0..4 {
                    let idx = (y * w + x + i) * 4;
                    data[idx] = (r_arr[i] / count_val) as u8;
                    data[idx + 1] = (g_arr[i] / count_val) as u8;
                    data[idx + 2] = (b_arr[i] / count_val) as u8;
                    data[idx + 3] = (a_arr[i] / count_val) as u8;
                }
                x += 4;
            }

            // Remainder — scalar
            while x < w {
                let (mut sr, mut sg, mut sb, mut sa) = (0u32, 0u32, 0u32, 0u32);
                for dy in -(r as i32)..=(r as i32) {
                    let ny = (y as i32 + dy) as usize;
                    let idx = (ny * w + x) * 4;
                    sr += data[idx] as u32;
                    sg += data[idx + 1] as u32;
                    sb += data[idx + 2] as u32;
                    sa += data[idx + 3] as u32;
                }
                let idx = (y * w + x) * 4;
                data[idx] = (sr / count_val) as u8;
                data[idx + 1] = (sg / count_val) as u8;
                data[idx + 2] = (sb / count_val) as u8;
                data[idx + 3] = (sa / count_val) as u8;
                x += 1;
            }
        }
    }
}

/// Render a drop shadow behind subtitle text.
///
/// Creates a copy of the source data, offsets it by `(offset_x, offset_y)`,
/// applies blur, tints it with `shadow_color`, then composites it behind
/// the original. This implements the ASS `\shad` and `\xshad`/`\yshad` tags.
///
/// # Arguments
/// * `src` — Source RGBA pixel data
/// * `width` — Pixel width
/// * `height` — Pixel height
/// * `offset_x` — Horizontal shadow offset (positive = right)
/// * `offset_y` — Vertical shadow offset (positive = down)
/// * `blur_radius` — Blur radius; 0.0 = hard shadow
/// * `shadow_color` — Shadow tint as `[R, G, B, A]`
///
/// # Returns
/// New `Vec<u8>` containing the shadow layer (original + shadow composited).
pub fn apply_shadow(
    src: &[u8],
    width: u32,
    height: u32,
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    shadow_color: [u8; 4],
) -> Vec<u8> {
    let num_pixels = (width * height) as usize;

    // Step 1: Create shadow layer — replace non-transparent pixels with shadow_color
    let mut shadow_data = vec![0u8; num_pixels * 4];
    for i in 0..num_pixels {
        let idx = i * 4;
        let src_a = src[idx + 3] as u32;
        if src_a > 0 {
            shadow_data[idx] = shadow_color[0];
            shadow_data[idx + 1] = shadow_color[1];
            shadow_data[idx + 2] = shadow_color[2];
            shadow_data[idx + 3] = ((shadow_color[3] as u32 * src_a) / 255) as u8;
        }
    }

    // Step 2: Apply gaussian blur to the shadow layer
    if blur_radius > 0.0 {
        let mut shadow_pixmap = Pixmap::new(width, height).unwrap();
        shadow_pixmap
            .data_mut()
            .copy_from_slice(&shadow_data);
        apply_gaussian_blur(&mut shadow_pixmap, blur_radius);
        shadow_data = shadow_pixmap.data().to_vec();
    }

    // Step 3: Offset the shadow layer
    let mut result = vec![0u8; num_pixels * 4];
    let ox = offset_x.round() as i32;
    let oy = offset_y.round() as i32;

    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let sx = x - ox;
            let sy = y - oy;
            if sx >= 0 && sx < width as i32 && sy >= 0 && sy < height as i32 {
                let src_idx = (sy as u32 * width + sx as u32) as usize * 4;
                let dst_idx = (y as u32 * width + x as u32) as usize * 4;
                result[dst_idx] = shadow_data[src_idx];
                result[dst_idx + 1] = shadow_data[src_idx + 1];
                result[dst_idx + 2] = shadow_data[src_idx + 2];
                result[dst_idx + 3] = shadow_data[src_idx + 3];
            }
        }
    }

    result
}

/// Alpha-composite `src` over `dst` in-place using Porter-Duff "over".
///
/// Both buffers must be the same size (`width * height * 4` bytes, RGBA).
/// This is the fundamental blending operation for layering subtitle elements.
///
/// # Arguments
/// * `dst` — Destination RGBA buffer (modified in-place)
/// * `src` — Source RGBA buffer to composite on top
/// * `width` — Pixel width
/// * `height` — Pixel height
pub fn composite_over(dst: &mut [u8], src: &[u8], width: u32, height: u32) {
    assert_eq!(dst.len(), (width * height * 4) as usize);
    assert_eq!(src.len(), (width * height * 4) as usize);

    let n = (width * height) as usize;

    // SIMD: process 4 pixels per iteration (16 bytes).
    // Use u32x4 for the heavy multiplications, then extract to arrays for
    // integer division (u32x4 lacks Div) and per-pixel out_a == 0 handling.
    let simd_chunks = n / 4;
    let one = u32x4::splat(255);

    for chunk in 0..simd_chunks {
        let idx = chunk * 16;

        // Deinterleave RGBA components across 4 pixels into u32x4 lanes
        let sr = u32x4::from([src[idx] as u32, src[idx + 4] as u32,
            src[idx + 8] as u32, src[idx + 12] as u32]);
        let sg = u32x4::from([src[idx + 1] as u32, src[idx + 5] as u32,
            src[idx + 9] as u32, src[idx + 13] as u32]);
        let sb = u32x4::from([src[idx + 2] as u32, src[idx + 6] as u32,
            src[idx + 10] as u32, src[idx + 14] as u32]);
        let sa = u32x4::from([src[idx + 3] as u32, src[idx + 7] as u32,
            src[idx + 11] as u32, src[idx + 15] as u32]);
        let dr = u32x4::from([dst[idx] as u32, dst[idx + 4] as u32,
            dst[idx + 8] as u32, dst[idx + 12] as u32]);
        let dg = u32x4::from([dst[idx + 1] as u32, dst[idx + 5] as u32,
            dst[idx + 9] as u32, dst[idx + 13] as u32]);
        let db = u32x4::from([dst[idx + 2] as u32, dst[idx + 6] as u32,
            dst[idx + 10] as u32, dst[idx + 14] as u32]);
        let da = u32x4::from([dst[idx + 3] as u32, dst[idx + 7] as u32,
            dst[idx + 11] as u32, dst[idx + 15] as u32]);

        // SIMD: all heavy multiplies
        let inv_sa = one - sa;
        let num_alpha = da * inv_sa;
        let sr_sa = sr * sa;
        let sg_sa = sg * sa;
        let sb_sa = sb * sa;
        let dr_da_inv = dr * da * inv_sa;
        let dg_da_inv = dg * da * inv_sa;
        let db_da_inv = db * da * inv_sa;

        // Extract to arrays for scalar division + conditional
        let sa_a = sa.to_array();
        let na_a = num_alpha.to_array();
        let sr_a = sr_sa.to_array();
        let sg_a = sg_sa.to_array();
        let sb_a = sb_sa.to_array();
        let dr_a = dr_da_inv.to_array();
        let dg_a = dg_da_inv.to_array();
        let db_a = db_da_inv.to_array();

        for lane in 0..4 {
            let pi = idx + lane * 4;
            let sa_val = sa_a[lane];

            // out_a = sa + da * (255 - sa) / 255
            let out_a = sa_val + na_a[lane] / 255;

            if out_a == 0 {
                continue;
            }

            // out_c = (sc * sa + dc * da * (255 - sa) / 255) / out_a
            let out_r = ((sr_a[lane] + dr_a[lane] / 255) / out_a) as u8;
            let out_g = ((sg_a[lane] + dg_a[lane] / 255) / out_a) as u8;
            let out_b = ((sb_a[lane] + db_a[lane] / 255) / out_a) as u8;

            dst[pi] = out_r;
            dst[pi + 1] = out_g;
            dst[pi + 2] = out_b;
            dst[pi + 3] = out_a as u8;
        }
    }

    // Scalar fallback for remaining pixels (when n % 4 != 0)
    let remaining_start = simd_chunks * 4;
    for pix in remaining_start..n {
        let idx = pix * 4;
        let sa = src[idx + 3] as u32;
        if sa == 0 {
            continue;
        }
        let da = dst[idx + 3] as u32;
        let out_a = sa + da * (255 - sa) / 255;
        if out_a == 0 {
            continue;
        }
        for c in 0..3 {
            let sv = src[idx + c] as u32;
            let dv = dst[idx + c] as u32;
            dst[idx + c] = ((sv * sa + dv * da * (255 - sa) / 255) / out_a) as u8;
        }
        dst[idx + 3] = out_a as u8;
    }
}
