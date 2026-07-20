//! Frame composition: blend libass layers onto an RGBA canvas.

use wide::u32x4;

use crate::domain::frame::{AssImageData, RgbaFrame};

/// Composite a list of libass images into a single RGBA frame.
///
/// Each image is a 1-byte-per-pixel alpha mask with a separate RGBA color
/// stored as 0xAABBGGRR. The AA byte is the per-image alpha (0=opaque,
/// 255=transparent in ASS convention), which is combined with the per-pixel
/// bitmap alpha for the effective opacity. Images are composited in list
/// order using Porter-Duff "over". Handles stride > w for padded bitmaps.
pub fn compose_frame(images: &[AssImageData], width: u32, height: u32) -> RgbaFrame {
    let stride_bytes = width as usize * 4;
    let mut frame = vec![0u8; stride_bytes * height as usize];

    for img in images {
        if img.w == 0 || img.h == 0 || img.bitmap.is_empty() {
            continue;
        }

        let iw = img.w as usize;
        let ih = img.h as usize;
        let istride = img.stride as usize;
        let dx = img.dst_x as usize;
        let dy = img.dst_y as usize;

        // libass stores color as 0xRRGGBBAA (MSB = R, LSB = alpha in ASS convention).
        // ASS alpha: 0=opaque, 255=transparent. RGBA alpha: 0=transparent, 255=opaque.
        let cr = ((img.color >> 24) & 0xFF) as u32;
        let cg = ((img.color >> 16) & 0xFF) as u32;
        let cb = ((img.color >> 8) & 0xFF) as u32;
        let color_alpha = 255 - (img.color & 0xFF) as u32;

        // SIMD constants for Porter-Duff blending
        let src_r = u32x4::splat(cr);
        let src_g = u32x4::splat(cg);
        let src_b = u32x4::splat(cb);
        let div_255 = u32x4::splat(255u32);

        for sy in 0..ih {
            let fy = dy + sy;
            if fy >= height as usize {
                break;
            }
            let alpha_row_start = sy * istride;
            let frame_row_start = fy * stride_bytes;

            let mut sx = 0usize;
            // SIMD: process 4 pixels at a time
            while sx + 4 <= iw {
                let fx = dx + sx;
                if fx + 4 > width as usize {
                    break;
                }

                let fi = frame_row_start + fx * 4;

                // Load 4 bitmap alpha bytes
                let ba0 = img.bitmap[alpha_row_start + sx] as u32;
                let ba1 = img.bitmap[alpha_row_start + sx + 1] as u32;
                let ba2 = img.bitmap[alpha_row_start + sx + 2] as u32;
                let ba3 = img.bitmap[alpha_row_start + sx + 3] as u32;

                let a0 = ba0 * color_alpha / 255;
                let a1 = ba1 * color_alpha / 255;
                let a2 = ba2 * color_alpha / 255;
                let a3 = ba3 * color_alpha / 255;

                // Skip if all transparent
                if a0 == 0 && a1 == 0 && a2 == 0 && a3 == 0 {
                    sx += 4;
                    continue;
                }

                // Load 4 destination pixels — one u32x4 per RGBA channel
                let dst_r = u32x4::from([
                    frame[fi] as u32,
                    frame[fi + 4] as u32,
                    frame[fi + 8] as u32,
                    frame[fi + 12] as u32,
                ]);
                let dst_g = u32x4::from([
                    frame[fi + 1] as u32,
                    frame[fi + 5] as u32,
                    frame[fi + 9] as u32,
                    frame[fi + 13] as u32,
                ]);
                let dst_b = u32x4::from([
                    frame[fi + 2] as u32,
                    frame[fi + 6] as u32,
                    frame[fi + 10] as u32,
                    frame[fi + 14] as u32,
                ]);
                let dst_a = u32x4::from([
                    frame[fi + 3] as u32,
                    frame[fi + 7] as u32,
                    frame[fi + 11] as u32,
                    frame[fi + 15] as u32,
                ]);

                let alpha = u32x4::from([a0, a1, a2, a3]);
                let inv = u32x4::splat(255) - alpha;

                // Porter-Duff "over" per channel
                let result_r = src_r * alpha / div_255 + dst_r * inv / div_255;
                let result_g = src_g * alpha / div_255 + dst_g * inv / div_255;
                let result_b = src_b * alpha / div_255 + dst_b * inv / div_255;
                // Alpha: src_A + dst_A * (1 - src_A/255)
                let result_a = alpha + dst_a * inv / div_255;

                let rr: [u32; 4] = result_r.into();
                let rg: [u32; 4] = result_g.into();
                let rb: [u32; 4] = result_b.into();
                let ra: [u32; 4] = result_a.into();

                frame[fi] = rr[0] as u8;
                frame[fi + 1] = rg[0] as u8;
                frame[fi + 2] = rb[0] as u8;
                frame[fi + 3] = ra[0] as u8;

                frame[fi + 4] = rr[1] as u8;
                frame[fi + 5] = rg[1] as u8;
                frame[fi + 6] = rb[1] as u8;
                frame[fi + 7] = ra[1] as u8;

                frame[fi + 8] = rr[2] as u8;
                frame[fi + 9] = rg[2] as u8;
                frame[fi + 10] = rb[2] as u8;
                frame[fi + 11] = ra[2] as u8;

                frame[fi + 12] = rr[3] as u8;
                frame[fi + 13] = rg[3] as u8;
                frame[fi + 14] = rb[3] as u8;
                frame[fi + 15] = ra[3] as u8;

                sx += 4;
            }

            // Scalar fallback for remaining pixels
            while sx < iw {
                let fx = dx + sx;
                if fx >= width as usize {
                    break;
                }
                let bitmap_alpha = img.bitmap[alpha_row_start + sx] as u32;
                let alpha = bitmap_alpha * color_alpha / 255;
                if alpha == 0 {
                    sx += 1;
                    continue;
                }
                let fi = frame_row_start + fx * 4;

                if alpha == 255 {
                    frame[fi] = cr as u8;
                    frame[fi + 1] = cg as u8;
                    frame[fi + 2] = cb as u8;
                    frame[fi + 3] = 255;
                } else {
                    let inv = 255 - alpha;
                    let da = frame[fi + 3] as u32;
                    frame[fi] = ((cr * alpha + frame[fi] as u32 * inv) / 255) as u8;
                    frame[fi + 1] = ((cg * alpha + frame[fi + 1] as u32 * inv) / 255) as u8;
                    frame[fi + 2] = ((cb * alpha + frame[fi + 2] as u32 * inv) / 255) as u8;
                    frame[fi + 3] = (alpha + da * inv / 255) as u8;
                }
                sx += 1;
            }
        }
    }

    RgbaFrame {
        data: frame,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::frame::ImageType;

    #[test]
    fn single_opaque_green() {
        let img = AssImageData {
            w: 2,
            h: 2,
            stride: 2,
            bitmap: vec![255; 4],
            color: 0x00FF0000,
            dst_x: 0,
            dst_y: 0,
            image_type: ImageType::Character,
        };
        let out = compose_frame(&[img], 2, 2);
        assert_eq!(out.data[1], 255);
        assert_eq!(out.data[3], 255);
    }

    #[test]
    fn transparent_alpha_skipped() {
        let img = AssImageData {
            w: 1,
            h: 1,
            stride: 1,
            bitmap: vec![0],
            color: 0xFF000000,
            dst_x: 0,
            dst_y: 0,
            image_type: ImageType::Character,
        };
        let out = compose_frame(&[img], 1, 1);
        assert_eq!(&out.data[..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn stride_wider_than_width() {
        let img = AssImageData {
            w: 2,
            h: 1,
            stride: 4,
            bitmap: vec![255; 4],
            color: 0x00FF0000,
            dst_x: 0,
            dst_y: 0,
            image_type: ImageType::Character,
        };
        let out = compose_frame(&[img], 2, 1);
        assert_eq!(out.data[1], 255);
        assert_eq!(out.data[5], 255);
    }

    #[test]
    fn blend_bg_red_fg_blue() {
        let bg = AssImageData {
            w: 1,
            h: 1,
            stride: 1,
            bitmap: vec![128],
            color: 0xFF000000,
            dst_x: 0,
            dst_y: 0,
            image_type: ImageType::Outline,
        };
        let fg = AssImageData {
            w: 1,
            h: 1,
            stride: 1,
            bitmap: vec![128],
            color: 0x0000FF00,
            dst_x: 0,
            dst_y: 0,
            image_type: ImageType::Character,
        };
        let out = compose_frame(&[bg, fg], 1, 1);
        assert_eq!(out.data[0], 63);
        assert_eq!(out.data[2], 128);
    }

    #[test]
    fn simd_scalar_parity_4px() {
        let img = AssImageData {
            w: 4,
            h: 1,
            stride: 4,
            bitmap: vec![64, 128, 192, 255],
            color: 0x00FF0000,
            dst_x: 0,
            dst_y: 0,
            image_type: ImageType::Character,
        };
        let out = compose_frame(&[img], 4, 1);
        assert_eq!(out.data[1], 64);
        assert_eq!(out.data[3], 64);
        assert_eq!(out.data[5], 128);
        assert_eq!(out.data[7], 128);
        assert_eq!(out.data[9], 192);
        assert_eq!(out.data[11], 192);
        assert_eq!(out.data[13], 255);
        assert_eq!(out.data[15], 255);
    }

    #[test]
    fn simd_scalar_parity_6px() {
        let mut bitmap = vec![0u8; 6];
        for i in 0..6 {
            bitmap[i] = (i * 51) as u8;
        }
        let img = AssImageData {
            w: 6,
            h: 1,
            stride: 6,
            bitmap,
            color: 0x0000FF00,
            dst_x: 0,
            dst_y: 0,
            image_type: ImageType::Character,
        };
        let out = compose_frame(&[img], 6, 1);
        for i in 0..6 {
            let expected_alpha = (i * 51) as u8;
            assert_eq!(out.data[i * 4 + 2], expected_alpha, "B at pixel {i}");
            assert_eq!(out.data[i * 4 + 3], expected_alpha, "A at pixel {i}");
        }
    }

    #[test]
    fn mixed_transparency_4px() {
        let img = AssImageData {
            w: 4,
            h: 1,
            stride: 4,
            bitmap: vec![255, 0, 255, 0],
            color: 0x00FF0000,
            dst_x: 0,
            dst_y: 0,
            image_type: ImageType::Character,
        };
        let out = compose_frame(&[img], 4, 1);
        assert_eq!(out.data[1], 255);
        assert_eq!(out.data[3], 255);
        assert_eq!(out.data[9], 255);
        assert_eq!(out.data[11], 255);
        assert_eq!(out.data[4], 0);
        assert_eq!(out.data[5], 0);
        assert_eq!(out.data[6], 0);
        assert_eq!(out.data[7], 0);
        assert_eq!(out.data[12], 0);
        assert_eq!(out.data[13], 0);
        assert_eq!(out.data[14], 0);
        assert_eq!(out.data[15], 0);
    }
}
