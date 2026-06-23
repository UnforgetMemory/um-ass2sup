use wide::u32x4;

/// Alpha-composite `src` over `dst` in-place using Porter-Duff "over".
pub fn composite_over(dst: &mut [u8], src: &[u8], width: u32, height: u32) {
    let len = (width * height * 4) as usize;
    debug_assert!(dst.len() >= len && src.len() >= len);

    let one = u32x4::splat(255);
    let chunks = len / 16;
    for i in 0..chunks {
        let off = i * 16;
        let s = u32x4::from([
            src[off] as u32,
            src[off + 1] as u32,
            src[off + 2] as u32,
            src[off + 3] as u32,
        ]);
        let d = u32x4::from([
            dst[off] as u32,
            dst[off + 1] as u32,
            dst[off + 2] as u32,
            dst[off + 3] as u32,
        ]);
        let sa = u32x4::splat(src[off + 3] as u32);
        let inv_da = one - sa;
        let r = s + d * inv_da / u32x4::splat(255);
        let r: [u32; 4] = r.into();
        dst[off] = r[0] as u8;
        dst[off + 1] = r[1] as u8;
        dst[off + 2] = r[2] as u8;
        dst[off + 3] = r[3] as u8;
    }
    // Remaining pixels
    for i in (chunks * 16..len).step_by(4) {
        let sa = src[i + 3] as u32;
        if sa == 0 {
            continue;
        }
        let inv = 255 - sa;
        dst[i] = ((src[i] as u32 * sa + dst[i] as u32 * inv) / 255) as u8;
        dst[i + 1] = ((src[i + 1] as u32 * sa + dst[i + 1] as u32 * inv) / 255) as u8;
        dst[i + 2] = ((src[i + 2] as u32 * sa + dst[i + 2] as u32 * inv) / 255) as u8;
        dst[i + 3] = (sa + (dst[i + 3] as u32 * inv) / 255) as u8;
    }
}

/// Apply a uniform alpha multiplier to all pixels in an RGBA buffer.
pub fn apply_alpha_multiplier(data: &mut [u8], alpha: f32) {
    let factor = alpha.clamp(0.0, 1.0);
    if factor >= 1.0 {
        return;
    }
    if factor <= 0.0 {
        data.iter_mut().skip(3).step_by(4).for_each(|a| *a = 0);
        return;
    }
    data.iter_mut()
        .skip(3)
        .step_by(4)
        .for_each(|a| *a = (*a as f32 * factor) as u8);
}

/// Composite a sub-region source buffer into a larger destination buffer.
#[allow(clippy::too_many_arguments)]
pub fn composite_subregion(
    dst: &mut [u8],
    src: &[u8],
    dw: u32,
    dh: u32,
    sx: i32,
    sy: i32,
    sw: u32,
    sh: u32,
) {
    for dy in sy.max(0)..(sy + sh as i32).min(dh as i32) {
        if dy < 0 {
            continue;
        }
        let src_y = (dy - sy) as u32;
        if src_y >= sh {
            continue;
        }
        for dx in sx.max(0)..(sx + sw as i32).min(dw as i32) {
            if dx < 0 {
                continue;
            }
            let src_x = (dx - sx) as u32;
            if src_x >= sw {
                continue;
            }
            let di = (dy as u32 * dw + dx as u32) * 4;
            let si = (src_y * sw + src_x) * 4;
            let sa = src[si as usize + 3] as u32;
            if sa == 0 {
                continue;
            }
            let inv = 255 - sa;
            let d = 255;
            let di = di as usize;
            let si = si as usize;
            dst[di] = ((src[si] as u32 * sa + dst[di] as u32 * inv) / d) as u8;
            dst[di + 1] = ((src[si + 1] as u32 * sa + dst[di + 1] as u32 * inv) / d) as u8;
            dst[di + 2] = ((src[si + 2] as u32 * sa + dst[di + 2] as u32 * inv) / d) as u8;
            dst[di + 3] = (sa + (dst[di + 3] as u32 * inv) / d) as u8;
        }
    }
}
