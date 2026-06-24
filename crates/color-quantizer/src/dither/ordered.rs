#![allow(missing_docs)]

//! Bayer ordered dithering.
//!
//! Uses a 4×4 Bayer matrix to produce a deterministic halftone pattern.
//! Faster than error-diffusion but produces a characteristic cross-hatch
//! texture in smooth gradients.

use crate::quantize::nearest::find_nearest_index;

/// 4×4 Bayer matrix normalised to [0/16, 15/16].
const BAYER_4X4: [[f32; 4]; 4] = [
    [0.0 / 16.0, 8.0 / 16.0, 2.0 / 16.0, 10.0 / 16.0],
    [12.0 / 16.0, 4.0 / 16.0, 14.0 / 16.0, 6.0 / 16.0],
    [3.0 / 16.0, 11.0 / 16.0, 1.0 / 16.0, 9.0 / 16.0],
    [15.0 / 16.0, 7.0 / 16.0, 13.0 / 16.0, 5.0 / 16.0],
];

/// Apply 4×4 Bayer ordered dithering.
///
/// `rgba` is a flat RGBA byte buffer. Returns flat palette-index bytes.
pub fn dither(rgba: &[u8], width: u32, height: u32, palette: &[[u8; 4]]) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let n = w * h;
    if n == 0 || palette.is_empty() {
        return vec![0; n];
    }

    let mut indices = vec![0u8; n];
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let base = idx * 4;
            let threshold = BAYER_4X4[y % 4][x % 4];

            let r = (rgba[base] as f32 + threshold * 255.0).min(255.0) as u8;
            let g = (rgba[base + 1] as f32 + threshold * 255.0).min(255.0) as u8;
            let b = (rgba[base + 2] as f32 + threshold * 255.0).min(255.0) as u8;
            let a = (rgba[base + 3] as f32 + threshold * 255.0).min(255.0) as u8;

            indices[idx] = find_nearest_index(&[r, g, b, a], palette);
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
        let pal = [[100, 150, 200, 255]];
        let result = dither(&rgba, 1, 1, &pal);
        assert_eq!(result[0], 0);
    }

    #[test]
    fn output_dimensions() {
        let rgba = vec![128u8; 40 * 30 * 4];
        let pal = [[0, 0, 0, 255], [255, 255, 255, 255]];
        let result = dither(&rgba, 40, 30, &pal);
        assert_eq!(result.len(), 1200);
    }

    #[test]
    fn bayer_pattern_deterministic() {
        let rgba = vec![128u8; 8 * 8 * 4];
        let pal = [[0, 0, 0, 255], [255, 255, 255, 255]];
        let r1 = dither(&rgba, 8, 8, &pal);
        let r2 = dither(&rgba, 8, 8, &pal);
        assert_eq!(r1, r2, "Bayer dither must be deterministic");
    }
}
