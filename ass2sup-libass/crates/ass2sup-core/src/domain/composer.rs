//! Frame composition: blend libass layers onto an RGBA canvas.

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
        let cr = (img.color >> 24) & 0xFF;
        let cg = (img.color >> 16) & 0xFF;
        let cb = (img.color >> 8) & 0xFF;
        let color_alpha = 255 - (img.color & 0xFF);

        for sy in 0..ih {
            let fy = dy + sy;
            if fy >= height as usize {
                break;
            }
            let alpha_row_start = sy * istride;
            let frame_row_start = fy * stride_bytes;

            for sx in 0..iw {
                let fx = dx + sx;
                if fx >= width as usize {
                    break;
                }
                let bitmap_alpha = img.bitmap[alpha_row_start + sx] as u32;
                // Combine per-pixel alpha with per-image color alpha (for fades)
                let alpha = bitmap_alpha * color_alpha / 255;
                if alpha == 0 {
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

    // libass color format: 0xRRGGBBAA (MSB → LSB)
    // RR = Red, GG = Green, BB = Blue, AA = alpha (ASS: 0=opaque, 255=transparent)
    // 0x00FF0000 = R=0, G=255, B=0, A=0 (opaque) = GREEN
    // 0xFF000000 = R=255, G=0, B=0, A=0 (opaque) = RED
    // 0x0000FF00 = R=0, G=0, B=255, A=0 (opaque) = BLUE

    #[test]
    fn single_opaque_green() {
        let img = AssImageData {
            w: 2,
            h: 2,
            stride: 2,
            bitmap: vec![255; 4],
            color: 0x00FF0000, // RR=0x00, GG=0xFF, BB=0x00, AA=0x00 → GREEN
            dst_x: 0,
            dst_y: 0,
            image_type: ImageType::Character,
        };
        let out = compose_frame(&[img], 2, 2);
        assert_eq!(out.data[1], 255); // G
        assert_eq!(out.data[3], 255); // A
    }

    #[test]
    fn transparent_alpha_skipped() {
        let img = AssImageData {
            w: 1,
            h: 1,
            stride: 1,
            bitmap: vec![0],
            color: 0xFF000000, // opaque RED but bitmap is 0
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
            color: 0x00FF0000, // GREEN
            dst_x: 0,
            dst_y: 0,
            image_type: ImageType::Character,
        };
        let out = compose_frame(&[img], 2, 1);
        assert_eq!(out.data[1], 255); // G at pixel (0,0)
        assert_eq!(out.data[5], 255); // G at pixel (1,0)
    }

    #[test]
    fn blend_bg_red_fg_blue() {
        // bg: RED at 50% bitmap alpha
        let bg = AssImageData {
            w: 1,
            h: 1,
            stride: 1,
            bitmap: vec![128],
            color: 0xFF000000, // RR=0xFF, GG=0x00, BB=0x00, AA=0x00 → RED
            dst_x: 0,
            dst_y: 0,
            image_type: ImageType::Outline,
        };
        // fg: BLUE at 50% bitmap alpha, composited over bg
        let fg = AssImageData {
            w: 1,
            h: 1,
            stride: 1,
            bitmap: vec![128],
            color: 0x0000FF00, // RR=0x00, GG=0x00, BB=0xFF, AA=0x00 → BLUE
            dst_x: 0,
            dst_y: 0,
            image_type: ImageType::Character,
        };
        let out = compose_frame(&[bg, fg], 1, 1);
        // bg at bitmap_alpha=128, color_alpha=255 → effective=128: R=(255*128+0*127)/255=128
        // fg at bitmap_alpha=128, color_alpha=255 → effective=128
        //   blended over bg: B=(255*128+0*127)/255=128, R=(0*128+128*127)/255=63
        assert_eq!(out.data[0], 63); // R
        assert_eq!(out.data[2], 128); // B
    }
}
