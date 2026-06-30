//! Vendored RGBA bitmap helpers.
//!
//! These functions are adapted from the parent `ass2sup` project:
//! - `crates/subtitle-renderer/src/effects/composite.rs`
//! - `crates/ass2sup-cli/src/util.rs`

/// Porter-Duff "over" compositing: overlay `src` onto `dst`.
///
/// Both buffers are RGBA (4 bytes per pixel, row-major, no padding).
/// `src` must be exactly `width * height * 4` bytes.
pub fn composite_over(dst: &mut [u8], src: &[u8], width: u32, height: u32) {
    let total = width as usize * height as usize;
    for i in 0..total {
        let base = i * 4;
        let sa = src[base + 3] as u32;
        if sa == 0 {
            continue;
        }
        let da = dst[base + 3] as u32;
        if sa == 255 {
            dst[base..base + 4].copy_from_slice(&src[base..base + 4]);
            continue;
        }
        let inv_sa = 255 - sa;
        for c in 0..3 {
            let s = src[base + c] as u32 * sa;
            let d = dst[base + c] as u32 * da;
            dst[base + c] = ((s + d * inv_sa / 255) / 255) as u8;
        }
        dst[base + 3] = (sa + da * inv_sa / 255) as u8;
    }
}

/// Apply a uniform alpha multiplier to an RGBA buffer.
pub fn apply_alpha_multiplier(data: &mut [u8], alpha: f32) {
    if alpha >= 1.0 {
        return;
    }
    let clamped = alpha.clamp(0.0, 1.0);
    for a in data[3..].iter_mut().step_by(4) {
        *a = (*a as f32 * clamped) as u8;
    }
}

/// Composite a source sub-region into a larger destination buffer.
/// The source region `(sx, sy, sw, sh)` is composited into the destination
/// at position `(dx, dy)`.
#[allow(clippy::too_many_arguments)]
pub fn composite_subregion(
    dst: &mut [u8],
    src: &[u8],
    dst_width: u32,
    dst_height: u32,
    src_width: u32,
    src_height: u32,
    dx: i32,
    dy: i32,
    sx: i32,
    sy: i32,
    sw: u32,
    sh: u32,
) {
    let dw = dst_width as i32;
    let dh = dst_height as i32;
    let sw_i = sw as i32;
    let sh_i = sh as i32;

    for row in 0..sh_i {
        let src_row = sy + row;
        if src_row < 0 || src_row >= src_height as i32 {
            continue;
        }
        let dst_row = dy + row;
        if dst_row < 0 || dst_row >= dh {
            continue;
        }
        for col in 0..sw_i {
            let src_col = sx + col;
            if src_col < 0 || src_col >= src_width as i32 {
                continue;
            }
            let dst_col = dx + col;
            if dst_col < 0 || dst_col >= dw {
                continue;
            }

            let src_idx = (src_row as usize * src_width as usize + src_col as usize) * 4;
            let dst_idx = (dst_row as usize * dw as usize + dst_col as usize) * 4;

            let sa = src[src_idx + 3] as u32;
            if sa == 0 {
                continue;
            }
            let da = dst[dst_idx + 3] as u32;
            if sa == 255 {
                dst[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
                continue;
            }
            let inv_sa = 255 - sa;
            for c in 0..3 {
                let s = src[src_idx + c] as u32 * sa;
                let d = dst[dst_idx + c] as u32 * da;
                dst[dst_idx + c] = ((s + d * inv_sa / 255) / 255) as u8;
            }
            dst[dst_idx + 3] = (sa + da * inv_sa / 255) as u8;
        }
    }
}

/// Crop an RGBA buffer to the tight bounding box of non-transparent pixels.
///
/// Returns `None` if the image is completely transparent.
pub fn crop_to_tight_bbox(
    bitmap: &[u8],
    width: u32,
    height: u32,
) -> Option<(Vec<u8>, u32, u32, u32, u32)> {
    if bitmap.len() < width as usize * height as usize * 4 {
        return None;
    }

    let w = width as usize;
    let h = height as usize;

    // Find top and bottom
    let mut top = None;
    let mut bottom = 0usize;
    for y in 0..h {
        for x in 0..w {
            if bitmap[(y * w + x) * 4 + 3] != 0 {
                if top.is_none() {
                    top = Some(y);
                }
                bottom = y;
                break;
            }
        }
    }

    let top = top?; // None means fully transparent
    let bottom = bottom;

    // Find left and right
    let mut left = w;
    let mut right = 0usize;
    for y in top..=bottom {
        for x in 0..w {
            if bitmap[(y * w + x) * 4 + 3] != 0 {
                if x < left {
                    left = x;
                }
                if x > right {
                    right = x;
                }
            }
        }
    }

    let crop_w = right - left + 1;
    let crop_h = bottom - top + 1;

    let mut cropped = Vec::with_capacity(crop_w * crop_h * 4);
    for y in top..=bottom {
        let src_start = (y * w + left) * 4;
        cropped.extend_from_slice(&bitmap[src_start..src_start + crop_w * 4]);
    }

    Some((
        cropped,
        left as u32,
        top as u32,
        crop_w as u32,
        crop_h as u32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crop_simple() {
        let mut rgba = vec![0u8; 16 * 16 * 4];
        // Set pixel at (5,5) to white, opaque
        let base = (5 * 16 + 5) * 4;
        rgba[base] = 255;
        rgba[base + 1] = 255;
        rgba[base + 2] = 255;
        rgba[base + 3] = 255;

        let cropped = crop_to_tight_bbox(&rgba, 16, 16);
        assert!(cropped.is_some());
        let (data, x, y, w, h) = cropped.unwrap();
        assert_eq!(w, 1);
        assert_eq!(h, 1);
        assert_eq!(x, 5);
        assert_eq!(y, 5);
        assert_eq!(&data[..4], &[255, 255, 255, 255]);
    }

    #[test]
    fn test_crop_transparent() {
        let rgba = vec![0u8; 16 * 16 * 4];
        assert!(crop_to_tight_bbox(&rgba, 16, 16).is_none());
    }

    #[test]
    fn test_composite_over_simple() {
        let mut dst = vec![0u8; 4 * 4]; // 1x1 RGBA
        let src = vec![255, 0, 0, 255]; // 1 red pixel
        composite_over(&mut dst, &src, 1, 1);
        assert_eq!(dst[0], 255); // R
        assert_eq!(dst[1], 0); // G
        assert_eq!(dst[2], 0); // B
        assert_eq!(dst[3], 255); // A
    }
}
