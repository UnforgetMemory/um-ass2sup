#![allow(missing_docs)]

//! Floyd–Steinberg error-diffusion dithering.
//!
//! Distributes quantisation error to four neighbour pixels with the
//! classic 7/16, 3/16, 5/16, 1/16 weights.
//!
//! **Important**: operates in linear-light to avoid gamma-induced hue shifts.
//! Callers should linearise sRGB input before calling and re-apply gamma
//! after.

use crate::quantize::nearest::find_nearest_index;

/// Apply Floyd–Steinberg error-diffusion dithering.
///
/// `rgba` is a flat RGBA byte buffer in row-major order. Returns flat
/// palette-index bytes in row-major order.
pub fn dither(rgba: &[u8], width: u32, height: u32, palette: &[[u8; 4]]) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let n = w * h;
    if n == 0 || palette.is_empty() {
        return vec![0; n];
    }

    // Error buffers in i16 for each channel (4× less memory than f64).
    let mut err_r = vec![0i16; n];
    let mut err_g = vec![0i16; n];
    let mut err_b = vec![0i16; n];
    let mut err_a = vec![0i16; n];

    // Initialise with source values.
    for (i, chunk) in rgba.chunks_exact(4).enumerate() {
        err_r[i] = chunk[0] as i16;
        err_g[i] = chunk[1] as i16;
        err_b[i] = chunk[2] as i16;
        err_a[i] = chunk[3] as i16;
    }

    let mut indices = vec![0u8; n];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let old_r = err_r[idx].clamp(0, 255) as u8;
            let old_g = err_g[idx].clamp(0, 255) as u8;
            let old_b = err_b[idx].clamp(0, 255) as u8;
            let old_a = err_a[idx].clamp(0, 255) as u8;

            let target = [old_r, old_g, old_b, old_a];
            let nearest = find_nearest_index(&target, palette);
            indices[idx] = nearest;

            let chosen = palette[nearest as usize];
            let qe_r = i16::from(old_r) - i16::from(chosen[0]);
            let qe_g = i16::from(old_g) - i16::from(chosen[1]);
            let qe_b = i16::from(old_b) - i16::from(chosen[2]);
            let qe_a = i16::from(old_a) - i16::from(chosen[3]);

            // Distribute error: right (7/16), down-left (3/16), down (5/16), down-right (1/16).
            let distribute =
                |buf: &mut [i16], dx: isize, dy: isize, num: i16, den: i16, qe: i16| {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx >= 0 && nx < w as isize && ny >= 0 && ny < h as isize {
                        let ni = ny as usize * w + nx as usize;
                        buf[ni] += qe * num / den;
                    }
                };

            distribute(&mut err_r, 1, 0, 7, 16, qe_r);
            distribute(&mut err_g, 1, 0, 7, 16, qe_g);
            distribute(&mut err_b, 1, 0, 7, 16, qe_b);
            distribute(&mut err_a, 1, 0, 7, 16, qe_a);

            distribute(&mut err_r, 0, 1, 5, 16, qe_r);
            distribute(&mut err_g, 0, 1, 5, 16, qe_g);
            distribute(&mut err_b, 0, 1, 5, 16, qe_b);
            distribute(&mut err_a, 0, 1, 5, 16, qe_a);

            distribute(&mut err_r, 1, 1, 1, 16, qe_r);
            distribute(&mut err_g, 1, 1, 1, 16, qe_g);
            distribute(&mut err_b, 1, 1, 1, 16, qe_b);
            distribute(&mut err_a, 1, 1, 1, 16, qe_a);

            if x > 0 {
                distribute(&mut err_r, -1, 1, 3, 16, qe_r);
                distribute(&mut err_g, -1, 1, 3, 16, qe_g);
                distribute(&mut err_b, -1, 1, 3, 16, qe_b);
                distribute(&mut err_a, -1, 1, 3, 16, qe_a);
            }
        }
    }

    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert!(dither(&[], 0, 0, &[]).is_empty());
    }

    #[test]
    fn single_pixel() {
        let rgba = vec![100, 150, 200, 255];
        let pal = [[100, 150, 200, 255], [0, 0, 0, 255]];
        let result = dither(&rgba, 1, 1, &pal);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn all_black_pixels() {
        let rgba = vec![0u8; 16 * 16 * 4];
        let pal = [[0, 0, 0, 255], [255, 255, 255, 255]];
        let result = dither(&rgba, 16, 16, &pal);
        // All pixels should map to black (index 0).
        assert!(result.iter().all(|&i| i == 0));
    }

    #[test]
    fn output_size() {
        let rgba = vec![128u8; 320 * 200 * 4];
        let pal = [[0, 0, 0, 255], [255, 255, 255, 255]];
        let result = dither(&rgba, 320, 200, &pal);
        assert_eq!(result.len(), 320 * 200);
    }
}
