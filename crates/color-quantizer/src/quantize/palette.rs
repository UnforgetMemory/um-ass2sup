#![allow(missing_docs)]

//! Palette data structures and optimisation helpers.
//!
//! The palette is stored as `Vec<[u8; 4]>` (flat RGBA tuples) for direct
//! SIMD vector loads — no `Rgba` struct indirection.

use std::collections::HashSet;

/// Build a palette from a set of RGBA pixels.
///
/// First deduplicates pixels using a hash set. If the unique colours fit
/// within `max_colors`, returns them directly. Otherwise, delegates to the
/// median-cut or octree quantizer (selected via `use_octree`).
pub fn build_palette(pixels: &[[u8; 4]], max_colors: usize, use_octree: bool) -> Vec<[u8; 4]> {
    if pixels.is_empty() || max_colors == 0 {
        return Vec::new();
    }

    // Fast path: count unique colours.
    let mut seen = HashSet::new();
    let mut uniques = Vec::new();
    for p in pixels {
        if seen.insert(*p) {
            uniques.push(*p);
            if uniques.len() > max_colors {
                // Too many unique colours → delegate to quantizer.
                return if use_octree {
                    crate::quantize::naarahara::quantize(pixels, max_colors)
                } else {
                    crate::quantize::median_cut::quantize(pixels, max_colors)
                };
            }
        }
    }

    uniques
}

/// Sort palette entries by BT.709 relative luminance for stable PGS encoding.
pub fn sort_by_luminance(palette: &mut [[u8; 4]]) {
    palette.sort_by_key(|c| (c[0] as u32 * 2126 + c[1] as u32 * 7152 + c[2] as u32 * 722) >> 10);
}

/// Merge two palettes, keeping at most `max_colors` entries.
///
/// Deduplicates entries and returns the first `max_colors` from the merged
/// result. Entries from `a` are preferred (they appear first).
pub fn merge_palettes(a: &[[u8; 4]], b: &[[u8; 4]], max_colors: usize) -> Vec<[u8; 4]> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();
    for entry in a.iter().chain(b) {
        if seen.insert(*entry) && merged.len() < max_colors {
            merged.push(*entry);
        }
    }
    merged
}

/// Compress a palette to fit within `max_colors` by merging the closest
/// colour pairs iteratively.
///
/// This is used when the palette slightly exceeds the limit and we want to
/// avoid a full requantisation. Each iteration merges the two closest
/// entries (by Euclidean distance) into their average.
pub fn compress(palette: &[[u8; 4]], max_colors: usize) -> Vec<[u8; 4]> {
    if palette.len() <= max_colors {
        return palette.to_vec();
    }

    let mut p = palette.to_vec();
    while p.len() > max_colors {
        // Find the closest pair.
        let mut best_dist = u32::MAX;
        let mut best_pair = (0, 1);
        for i in 0..p.len() {
            for j in i + 1..p.len() {
                let dr = i32::from(p[i][0]) - i32::from(p[j][0]);
                let dg = i32::from(p[i][1]) - i32::from(p[j][1]);
                let db = i32::from(p[i][2]) - i32::from(p[j][2]);
                let d = (dr * dr + dg * dg + db * db) as u32;
                if d < best_dist {
                    best_dist = d;
                    best_pair = (i, j);
                }
            }
        }

        // Merge the pair into their average.
        let (i, j) = best_pair;
        let avg = [
            ((p[i][0] as u16 + p[j][0] as u16) / 2) as u8,
            ((p[i][1] as u16 + p[j][1] as u16) / 2) as u8,
            ((p[i][2] as u16 + p[j][2] as u16) / 2) as u8,
            ((p[i][3] as u16 + p[j][3] as u16) / 2) as u8,
        ];
        // Remove j first (higher index) to avoid shifting.
        p.swap_remove(j);
        p.swap_remove(i);
        p.push(avg);
    }

    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_palette_dedup() {
        let pixels = vec![[255, 0, 0, 255], [0, 255, 0, 255], [255, 0, 0, 255]];
        let pal = build_palette(&pixels, 8, false);
        assert_eq!(pal.len(), 2);
    }

    #[test]
    fn build_palette_empty() {
        assert!(build_palette(&[], 8, false).is_empty());
    }

    #[test]
    fn sort_luminance_stable() {
        let mut pal = vec![[0, 0, 0, 255], [255, 255, 255, 255], [128, 128, 128, 255]];
        sort_by_luminance(&mut pal);
        assert_eq!(pal[0], [0, 0, 0, 255]);
        assert_eq!(pal[2], [255, 255, 255, 255]);
    }

    #[test]
    fn merge_dedup() {
        let a = vec![[1, 2, 3, 255], [4, 5, 6, 255]];
        let b = vec![[4, 5, 6, 255], [7, 8, 9, 255]];
        let merged = merge_palettes(&a, &b, 8);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn compress_within_limit() {
        let pal = vec![[1, 2, 3, 255], [4, 5, 6, 255]];
        assert_eq!(compress(&pal, 4).len(), 2);
    }

    #[test]
    fn compress_exceeding_limit() {
        let pal: Vec<[u8; 4]> = (0..10u8).map(|i| [i * 25, i * 25, i * 25, 255]).collect();
        let compressed = compress(&pal, 5);
        assert!(compressed.len() <= 5);
    }
}
