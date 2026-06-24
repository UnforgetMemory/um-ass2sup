use crate::types::Rgba;

const BAYER_4X4: [[f64; 4]; 4] = [
    [0.0 / 16.0, 8.0 / 16.0, 2.0 / 16.0, 10.0 / 16.0],
    [12.0 / 16.0, 4.0 / 16.0, 14.0 / 16.0, 6.0 / 16.0],
    [3.0 / 16.0, 11.0 / 16.0, 1.0 / 16.0, 9.0 / 16.0],
    [15.0 / 16.0, 7.0 / 16.0, 13.0 / 16.0, 5.0 / 16.0],
];

/// Applies Floyd–Steinberg error-diffusion dithering to an RGBA image,
/// mapping each pixel to the nearest palette entry and distributing the
/// quantization error to neighbouring pixels (7/16, 3/16, 5/16, 1/16 weights).
///
/// `transparent_index` is the palette index used to seed the output buffer
/// and is written into transparent pixels. Returns a flat `Vec<u8>` of
/// palette indices in row-major order.
pub fn floyd_steinberg_dither(
    rgba: &[u8],
    width: u32,
    height: u32,
    palette: &[Rgba],
    transparent_index: u8,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let n = w * h;

    let mut err_r = vec![0.0f64; n];
    let mut err_g = vec![0.0f64; n];
    let mut err_b = vec![0.0f64; n];
    let mut err_a = vec![0.0f64; n];

    for i in 0..n {
        let base = i * 4;
        err_r[i] = f64::from(rgba[base]);
        err_g[i] = f64::from(rgba[base + 1]);
        err_b[i] = f64::from(rgba[base + 2]);
        err_a[i] = f64::from(rgba[base + 3]);
    }

    let mut indices = vec![transparent_index; n];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let old_r = err_r[idx].clamp(0.0, 255.0);
            let old_g = err_g[idx].clamp(0.0, 255.0);
            let old_b = err_b[idx].clamp(0.0, 255.0);
            let old_a = err_a[idx].clamp(0.0, 255.0);

            let target = Rgba::new(old_r as u8, old_g as u8, old_b as u8, old_a as u8);
            let nearest_idx = super::median_cut::find_nearest_index(&target, palette);
            indices[idx] = nearest_idx;

            let chosen = &palette[nearest_idx as usize];
            let qe_r = old_r - f64::from(chosen.r);
            let qe_g = old_g - f64::from(chosen.g);
            let qe_b = old_b - f64::from(chosen.b);
            let qe_a = old_a - f64::from(chosen.a);

            let distribute = |err_buf: &mut [f64], dx: usize, dy: usize, factor: f64, qe: f64| {
                let nx = x as isize + dx as isize;
                let ny = y as isize + dy as isize;
                if nx >= 0 && nx < w as isize && ny >= 0 && ny < h as isize {
                    let ni = ny as usize * w + nx as usize;
                    err_buf[ni] += qe * factor;
                }
            };

            distribute(&mut err_r, 1, 0, 7.0 / 16.0, qe_r);
            distribute(&mut err_g, 1, 0, 7.0 / 16.0, qe_g);
            distribute(&mut err_b, 1, 0, 7.0 / 16.0, qe_b);
            distribute(&mut err_a, 1, 0, 7.0 / 16.0, qe_a);

            distribute(&mut err_r, 0, 1, 5.0 / 16.0, qe_r);
            distribute(&mut err_g, 0, 1, 5.0 / 16.0, qe_g);
            distribute(&mut err_b, 0, 1, 5.0 / 16.0, qe_b);
            distribute(&mut err_a, 0, 1, 5.0 / 16.0, qe_a);

            distribute(&mut err_r, 1, 1, 1.0 / 16.0, qe_r);
            distribute(&mut err_g, 1, 1, 1.0 / 16.0, qe_g);
            distribute(&mut err_b, 1, 1, 1.0 / 16.0, qe_b);
            distribute(&mut err_a, 1, 1, 1.0 / 16.0, qe_a);

            if x > 0 {
                distribute(&mut err_r, 0, 1, 3.0 / 16.0, qe_r);
                distribute(&mut err_g, 0, 1, 3.0 / 16.0, qe_g);
                distribute(&mut err_b, 0, 1, 3.0 / 16.0, qe_b);
                distribute(&mut err_a, 0, 1, 3.0 / 16.0, qe_a);
            }
        }
    }

    indices
}

/// Applies 4×4 Bayer ordered dithering to an RGBA image by adding a fixed
/// threshold (from the Bayer matrix) to each channel before palette mapping.
///
/// Produces a visible cross-hatch pattern but is much faster than
/// Floyd–Steinberg. `transparent_index` seeds the output buffer and is
/// written into transparent pixels. Returns a flat `Vec<u8>` of palette
/// indices in row-major order.
pub fn ordered_dither(
    rgba: &[u8],
    width: u32,
    height: u32,
    palette: &[Rgba],
    transparent_index: u8,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let n = w * h;
    let mut indices = vec![transparent_index; n];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let base = idx * 4;
            let threshold = BAYER_4X4[y % 4][x % 4] * 255.0;

            let r = (f64::from(rgba[base]) + threshold).min(255.0) as u8;
            let g = (f64::from(rgba[base + 1]) + threshold).min(255.0) as u8;
            let b = (f64::from(rgba[base + 2]) + threshold).min(255.0) as u8;
            let a = (f64::from(rgba[base + 3]) + threshold).min(255.0) as u8;

            let target = Rgba::new(r, g, b, a);
            indices[idx] = super::median_cut::find_nearest_index(&target, palette);
        }
    }

    indices
}
