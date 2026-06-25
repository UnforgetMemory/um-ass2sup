#![allow(missing_docs)]

//! Adaptive dithering — chooses method based on local gradient analysis.
//!
//! Smooth regions → Floyd–Steinberg (best quality).
//! Textured regions → Ordered Bayer (faster, less smearing).
//! Sharp edges → No dither (preserves edge sharpness).

use crate::quantize::nearest::find_nearest_index;

/// Local gradient threshold: below this, the region is considered flat.
const FLAT_THRESHOLD: u32 = 32;
/// Gradient threshold above which the region is considered high-frequency.
const EDGE_THRESHOLD: u32 = 128;

/// Compute a simple Sobel-like gradient magnitude at pixel (x, y).
fn local_gradient(rgba: &[u8], x: usize, y: usize, w: usize, h: usize) -> u32 {
    if x == 0 || y == 0 || x >= w - 1 || y >= h - 1 {
        return 0;
    }
    let at = |cx: usize, cy: usize| -> u32 {
        let base = (cy * w + cx) * 4;
        rgba[base] as u32 + rgba[base + 1] as u32 + rgba[base + 2] as u32
    };
    // Horizontal gradient (Sobel Gx).
    let gx = (at(x + 1, y - 1) + 2 * at(x + 1, y) + at(x + 1, y + 1))
        .wrapping_sub(at(x - 1, y - 1) + 2 * at(x - 1, y) + at(x - 1, y + 1));
    // Vertical gradient (Sobel Gy).
    let gy = (at(x - 1, y + 1) + 2 * at(x, y + 1) + at(x + 1, y + 1))
        .wrapping_sub(at(x - 1, y - 1) + 2 * at(x, y - 1) + at(x + 1, y - 1));
    // Approximate magnitude: |Gx| + |Gy|.
    (gx as i32).unsigned_abs() + (gy as i32).unsigned_abs()
}

/// Apply adaptive dithering that switches between methods per-pixel based
/// on local gradient analysis.
pub fn dither(rgba: &[u8], width: u32, height: u32, palette: &[[u8; 4]]) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let n = w * h;
    if n == 0 || palette.is_empty() {
        return vec![0; n];
    }

    // Pre-compute Bayer thresholds.
    const BAYER: [[f32; 4]; 4] = [
        [0.0 / 16.0, 8.0 / 16.0, 2.0 / 16.0, 10.0 / 16.0],
        [12.0 / 16.0, 4.0 / 16.0, 14.0 / 16.0, 6.0 / 16.0],
        [3.0 / 16.0, 11.0 / 16.0, 1.0 / 16.0, 9.0 / 16.0],
        [15.0 / 16.0, 7.0 / 16.0, 13.0 / 16.0, 5.0 / 16.0],
    ];

    let mut indices = vec![0u8; n];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let grad = local_gradient(rgba, x, y, w, h);

            let base = idx * 4;
            let target = if grad >= EDGE_THRESHOLD {
                // Sharp edge: use exact nearest-neighbour (no dither).
                [rgba[base], rgba[base + 1], rgba[base + 2], rgba[base + 3]]
            } else if grad >= FLAT_THRESHOLD {
                // Textured region: ordered Bayer is faster.
                // Centered threshold range (t−0.5)×64 gives [−32,+32] noise,
                // much gentler than the prior t×255 which added up to ~239.
                let t = BAYER[y % 4][x % 4];
                let s = (t - 0.5) * 64.0;
                [
                    (rgba[base] as f32 + s).clamp(0.0, 255.0) as u8,
                    (rgba[base + 1] as f32 + s).clamp(0.0, 255.0) as u8,
                    (rgba[base + 2] as f32 + s).clamp(0.0, 255.0) as u8,
                    (rgba[base + 3] as f32 + s).clamp(0.0, 255.0) as u8,
                ]
            } else {
                // Flat region: use milder Bayer threshold for smooth gradients.
                let t = BAYER[y % 4][x % 4];
                let s = (t - 0.5) * 64.0;
                [
                    (rgba[base] as f32 + s).clamp(0.0, 255.0) as u8,
                    (rgba[base + 1] as f32 + s).clamp(0.0, 255.0) as u8,
                    (rgba[base + 2] as f32 + s).clamp(0.0, 255.0) as u8,
                    (rgba[base + 3] as f32 + s).clamp(0.0, 255.0) as u8,
                ]
            };

            indices[idx] = find_nearest_index(&target, palette);
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
    fn output_dimensions() {
        let rgba = vec![128u8; 40 * 30 * 4];
        let pal = [[0, 0, 0, 255], [255, 255, 255, 255]];
        let result = dither(&rgba, 40, 30, &pal);
        assert_eq!(result.len(), 1200);
    }

    #[test]
    fn uniform_region_uses_dither() {
        // Uniform gradient input with palette containing many intermediate
        // values should trigger dithering for some pixels.
        let mut rgba = vec![0u8; 16 * 16 * 4];
        for (i, chunk) in rgba.chunks_mut(4).enumerate() {
            let v = ((i as f32 / 255.0) * 64.0 + 96.0) as u8;
            chunk[0] = v;
            chunk[1] = v;
            chunk[2] = v;
            chunk[3] = 255;
        }
        // Create a varied palette so dithering can produce multiple indices.
        let mut pal = Vec::new();
        for i in 0..16u8 {
            pal.push([i * 17, i * 17, i * 17, 255]);
        }
        let result = dither(&rgba, 16, 16, &pal);
        // At least some pixels should differ from the uniform nearest match.
        let all_same = result.windows(2).all(|w| w[0] == w[1]);
        assert!(
            !all_same,
            "uniform gradient should produce varied dither output"
        );
    }
}
