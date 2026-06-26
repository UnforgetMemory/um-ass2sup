use wide::u32x4;

/// Alpha-composite `src` over `dst` in-place using Porter-Duff "over".
pub fn composite_over(dst: &mut [u8], src: &[u8], width: u32, height: u32) {
    let len = (width * height * 4) as usize;
    if dst.len() < len || src.len() < len {
        return;
    }

    let chunks = len / 4;
    for i in 0..chunks {
        let off = i * 4;
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
        let inv = u32x4::splat(255) - sa;
        let r = s * sa / u32x4::splat(255) + d * inv / u32x4::splat(255);
        let mut r: [u32; 4] = r.into();
        // Porter-Duff "over": alpha_out = src_A + dst_A * (1 - src_A/255)
        // SIMD computes src_A²/255 for alpha lane; fix to correct formula.
        r[3] = src[off + 3] as u32 + (dst[off + 3] as u32 * (255 - src[off + 3] as u32)) / 255;
        dst[off] = r[0] as u8;
        dst[off + 1] = r[1] as u8;
        dst[off + 2] = r[2] as u8;
        dst[off + 3] = r[3] as u8;
    }
    // Remaining pixels
    for i in (chunks * 4..len).step_by(4) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_over_processes_all_pixels() {
        // Create a 4x1 image (4 pixels = 16 bytes)
        let width = 4u32;
        let height = 1u32;
        let len = (width * height * 4) as usize;

        // Source: all pixels have alpha=128, color=(255, 0, 0)
        let mut src = vec![0u8; len];
        for i in 0..4 {
            src[i * 4] = 255; // R
            src[i * 4 + 1] = 0; // G
            src[i * 4 + 2] = 0; // B
            src[i * 4 + 3] = 128; // A
        }

        // Destination: all pixels are (0, 0, 0, 0) (transparent black)
        let mut dst = vec![0u8; len];

        // Run composite_over
        composite_over(&mut dst, &src, width, height);

        // Verify ALL pixels are composited (not just every 4th)
        for i in 0..4 {
            let pixel = &dst[i * 4..(i + 1) * 4];
            assert_ne!(pixel, &[0, 0, 0, 0], "Pixel {i} was not composited");
            // Expected: src(255,0,0,128) over dst(0,0,0,0) = (128,0,0,128)
            assert_eq!(pixel[0], 128, "Pixel {i} R channel wrong");
            assert_eq!(pixel[1], 0, "Pixel {i} G channel wrong");
            assert_eq!(pixel[2], 0, "Pixel {i} B channel wrong");
            assert_eq!(pixel[3], 128, "Pixel {i} A channel wrong");
        }
    }

    #[test]
    fn composite_over_simd_vs_scalar_parity() {
        // Test with 8 pixels to exercise both SIMD and scalar paths
        let width = 8u32;
        let height = 1u32;
        let len = (width * height * 4) as usize;

        let mut src = vec![0u8; len];
        let mut dst_simd = vec![0u8; len];
        let mut dst_scalar = vec![0u8; len];

        // Fill source with varying alpha values
        for i in 0..8 {
            src[i * 4] = 200; // R
            src[i * 4 + 1] = 100; // G
            src[i * 4 + 2] = 50; // B
            src[i * 4 + 3] = (i * 30 + 10) as u8; // Varying alpha
        }

        // SIMD path (current implementation)
        composite_over(&mut dst_simd, &src, width, height);

        // Scalar reference (correct implementation)
        for i in (0..len).step_by(4) {
            let sa = src[i + 3] as u32;
            if sa == 0 {
                continue;
            }
            let inv = 255 - sa;
            dst_scalar[i] = ((src[i] as u32 * sa + dst_scalar[i] as u32 * inv) / 255) as u8;
            dst_scalar[i + 1] =
                ((src[i + 1] as u32 * sa + dst_scalar[i + 1] as u32 * inv) / 255) as u8;
            dst_scalar[i + 2] =
                ((src[i + 2] as u32 * sa + dst_scalar[i + 2] as u32 * inv) / 255) as u8;
            dst_scalar[i + 3] = (sa + (dst_scalar[i + 3] as u32 * inv) / 255) as u8;
        }

        assert_eq!(
            dst_simd, dst_scalar,
            "SIMD and scalar paths should produce identical results"
        );
    }
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
