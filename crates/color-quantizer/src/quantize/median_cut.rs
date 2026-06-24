#![allow(missing_docs)]

//! Variance-optimised median-cut palette generation.
//!
//! Splits the colour-space box along its longest channel axis at the median,
//! recursively, until the target number of colour cubes is reached. Each
//! cube is then averaged to produce one palette entry.

/// Bounding box tracking min/max per channel for a set of RGBA pixels.
struct BoundingBox {
    r_min: u8,
    r_max: u8,
    g_min: u8,
    g_max: u8,
    b_min: u8,
    b_max: u8,
    a_min: u8,
    a_max: u8,
}

impl BoundingBox {
    fn from_pixels(pixels: &[[u8; 4]]) -> Option<Self> {
        if pixels.is_empty() {
            return None;
        }
        let mut bx = BoundingBox {
            r_min: 255,
            r_max: 0,
            g_min: 255,
            g_max: 0,
            b_min: 255,
            b_max: 0,
            a_min: 255,
            a_max: 0,
        };
        for p in pixels {
            bx.r_min = bx.r_min.min(p[0]);
            bx.r_max = bx.r_max.max(p[0]);
            bx.g_min = bx.g_min.min(p[1]);
            bx.g_max = bx.g_max.max(p[1]);
            bx.b_min = bx.b_min.min(p[2]);
            bx.b_max = bx.b_max.max(p[2]);
            bx.a_min = bx.a_min.min(p[3]);
            bx.a_max = bx.a_max.max(p[3]);
        }
        Some(bx)
    }

    fn longest_axis(&self) -> usize {
        let ranges = [
            u32::from(self.r_max - self.r_min),
            u32::from(self.g_max - self.g_min),
            u32::from(self.b_max - self.b_min),
            u32::from(self.a_max - self.a_min),
        ];
        ranges
            .iter()
            .enumerate()
            .max_by_key(|(_, v)| *v)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
}

fn average_color(pixels: &[[u8; 4]]) -> [u8; 4] {
    if pixels.is_empty() {
        return [0, 0, 0, 0];
    }
    let n = pixels.len() as u64;
    let (mut r, mut g, mut b, mut a) = (0u64, 0u64, 0u64, 0u64);
    for p in pixels {
        r += u64::from(p[0]);
        g += u64::from(p[1]);
        b += u64::from(p[2]);
        a += u64::from(p[3]);
    }
    [(r / n) as u8, (g / n) as u8, (b / n) as u8, (a / n) as u8]
}

/// Reduce a set of RGBA pixels to a palette of at most `max_colors` entries
/// using the median-cut algorithm.
///
/// Recursively splits the bounding box with the longest colour-channel range
/// at the median until the desired number of cubes is reached, then averages
/// each cube to produce the final palette.
pub fn quantize(pixels: &[[u8; 4]], max_colors: usize) -> Vec<[u8; 4]> {
    if pixels.is_empty() || max_colors == 0 {
        return Vec::new();
    }

    let mut cubes: Vec<Vec<[u8; 4]>> = vec![pixels.to_vec()];

    while cubes.len() < max_colors {
        // Find the largest cube (most pixels).
        let idx = match cubes
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| c.len())
            .map(|(i, _)| i)
        {
            Some(i) if cubes[i].len() > 1 => i,
            _ => break,
        };

        let mut cube = cubes.swap_remove(idx);
        let bb = match BoundingBox::from_pixels(&cube) {
            Some(bb) => bb,
            None => {
                cubes.push(cube);
                break;
            }
        };
        let axis = bb.longest_axis();

        cube.sort_by_key(|p| p[axis]);
        let mid = cube.len() / 2;
        let (left, right) = cube.split_at(mid);
        cubes.push(left.to_vec());
        cubes.push(right.to_vec());
    }

    cubes.iter().map(|c| average_color(c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        assert!(quantize(&[], 8).is_empty());
    }

    #[test]
    fn zero_colors() {
        let pixels = [[255, 0, 0, 255]];
        assert!(quantize(&pixels, 0).is_empty());
    }

    #[test]
    fn single_output() {
        let pixels = vec![[10, 20, 30, 255]; 100];
        let pal = quantize(&pixels, 1);
        assert_eq!(pal.len(), 1);
        assert_eq!(pal[0], [10, 20, 30, 255]);
    }

    #[test]
    fn max_colors_not_exceeded() {
        let pixels: Vec<[u8; 4]> = (0..100u8).map(|i| [i, i, i, 255]).collect();
        let pal = quantize(&pixels, 16);
        assert!(pal.len() <= 16);
        assert!(!pal.is_empty());
    }

    #[test]
    fn uniform_pixels_produce_few_colors() {
        let pixels = vec![[100, 150, 200, 255]; 1000];
        let pal = quantize(&pixels, 256);
        let mut unique = std::collections::HashSet::new();
        for entry in &pal {
            unique.insert(*entry);
        }
        assert!(
            unique.len() <= 2,
            "uniform data produced {} unique colours",
            unique.len()
        );
    }
}
