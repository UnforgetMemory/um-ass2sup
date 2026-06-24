#![allow(missing_docs)]

//! Nearest-neighbor colour search using a k-d tree with branch-and-bound pruning.
//!
//! For palettes below [`LINEAR_THRESHOLD`] (32 entries) a linear scan is used
//! instead — it is faster and guarantees tie-breaking parity with the original
//! implementation. For larger palettes a k-d tree is built once per palette
//! slice and cached via thread-local storage.
//!
//! Tie-breaking: when two palette entries are equidistant, the *lower index*
//! is preferred (matching the stable behaviour of `Iterator::min_by_key`).

use std::cell::RefCell;

/// Minimum palette size before the k-d tree is used instead of linear scan.
const LINEAR_THRESHOLD: usize = 32;

/// Maximum palette indices in a k-d tree leaf before forcing another split.
const LEAF_SIZE: usize = 8;

// ---------------------------------------------------------------------------
// k-d tree
// ---------------------------------------------------------------------------

enum KdNode {
    Leaf(Vec<usize>),
    Split {
        axis: usize,
        threshold: u8,
        left: Box<KdNode>,
        right: Box<KdNode>,
    },
}

struct KdCacheEntry {
    palette_ptr: usize,
    palette_len: usize,
    tree: KdNode,
}

thread_local! {
    static KD_TREE_CACHE: RefCell<Option<KdCacheEntry>> = const { RefCell::new(None) };
}

/// Extract a colour channel by axis index (0=R, 1=G, 2=B, 3=A).
#[inline(always)]
fn channel(c: &[u8; 4], axis: usize) -> u8 {
    c[axis]
}

/// Squared Euclidean distance between two RGBA colours, using `i32` to
/// avoid overflow.
#[inline(always)]
fn distance_sq(a: &[u8; 4], b: &[u8; 4]) -> u32 {
    let dr = i32::from(a[0]) - i32::from(b[0]);
    let dg = i32::from(a[1]) - i32::from(b[1]);
    let db = i32::from(a[2]) - i32::from(b[2]);
    let da = i32::from(a[3]) - i32::from(b[3]);
    (dr * dr + dg * dg + db * db + da * da) as u32
}

/// Weighted Euclidean distance with channel-specific weights
/// (3×R, 4×G, 2×B — approximating perceived luminance difference).
///
/// Uses `u64` to avoid overflow on large channel differences.
#[inline(always)]
fn distance_sq_weighted(a: &[u8; 4], b: &[u8; 4]) -> u64 {
    let dr = i64::from(a[0]) - i64::from(b[0]);
    let dg = i64::from(a[1]) - i64::from(b[1]);
    let db = i64::from(a[2]) - i64::from(b[2]);
    (dr * dr * 3 + dg * dg * 4 + db * db * 2) as u64
}

fn longest_axis_of_indices(indices: &[usize], palette: &[[u8; 4]]) -> usize {
    let (mut min_r, mut max_r) = (255u8, 0u8);
    let (mut min_g, mut max_g) = (255u8, 0u8);
    let (mut min_b, mut max_b) = (255u8, 0u8);
    let (mut min_a, mut max_a) = (255u8, 0u8);

    for &i in indices {
        let p = &palette[i];
        min_r = min_r.min(p[0]);
        max_r = max_r.max(p[0]);
        min_g = min_g.min(p[1]);
        max_g = max_g.max(p[1]);
        min_b = min_b.min(p[2]);
        max_b = max_b.max(p[2]);
        min_a = min_a.min(p[3]);
        max_a = max_a.max(p[3]);
    }

    let ranges = [
        u32::from(max_r - min_r),
        u32::from(max_g - min_g),
        u32::from(max_b - min_b),
        u32::from(max_a - min_a),
    ];

    ranges
        .iter()
        .enumerate()
        .max_by_key(|(_, v)| *v)
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn build_kdtree(indices: &[usize], palette: &[[u8; 4]]) -> KdNode {
    if indices.len() <= LEAF_SIZE {
        return KdNode::Leaf(indices.to_vec());
    }

    let axis = longest_axis_of_indices(indices, palette);

    let mut sorted = indices.to_vec();
    sorted.sort_by(|&a, &b| channel(&palette[a], axis).cmp(&channel(&palette[b], axis)));

    let mid = sorted.len() / 2;
    let threshold = channel(&palette[sorted[mid]], axis);

    // Partition point: first index with value >= threshold.
    let split = sorted.partition_point(|&i| channel(&palette[i], axis) < threshold);

    if split == 0 || split >= sorted.len() {
        return KdNode::Leaf(sorted);
    }

    KdNode::Split {
        axis,
        threshold,
        left: Box::new(build_kdtree(&sorted[..split], palette)),
        right: Box::new(build_kdtree(&sorted[split..], palette)),
    }
}

impl KdNode {
    fn nearest(&self, color: &[u8; 4], palette: &[[u8; 4]], best: &mut (usize, u32)) {
        match self {
            KdNode::Leaf(indices) => {
                for &i in indices {
                    let d = distance_sq(color, &palette[i]);
                    if d < best.1 || (d == best.1 && i < best.0) {
                        *best = (i, d);
                    }
                }
            }
            KdNode::Split {
                axis,
                threshold,
                left,
                right,
            } => {
                let val = color[*axis];
                let (near, far) = if val < *threshold {
                    (left.as_ref(), right.as_ref())
                } else {
                    (right.as_ref(), left.as_ref())
                };
                near.nearest(color, palette, best);

                // Branch-and-bound: only search far side if plane distance
                // might beat the current best.
                let diff = i32::from(val) - i32::from(*threshold);
                let plane_dist = (diff * diff) as u32;
                if plane_dist < best.1 {
                    far.nearest(color, palette, best);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Linear scan fallback
// ---------------------------------------------------------------------------

fn find_nearest_linear(color: &[u8; 4], palette: &[[u8; 4]]) -> u8 {
    palette
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| distance_sq(color, p))
        .map(|(i, _)| i as u8)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Find the palette index closest to `color` (unweighted Euclidean).
///
/// Uses linear scan for small palettes and a cached k-d tree for larger ones.
#[inline]
pub fn find_nearest_index(color: &[u8; 4], palette: &[[u8; 4]]) -> u8 {
    if palette.len() < LINEAR_THRESHOLD {
        return find_nearest_linear(color, palette);
    }

    KD_TREE_CACHE.with_borrow_mut(|cache| {
        let ptr = palette.as_ptr() as usize;
        let len = palette.len();

        let needs_rebuild = match cache {
            Some(entry) => entry.palette_ptr != ptr || entry.palette_len != len,
            None => true,
        };

        if needs_rebuild {
            let indices: Vec<usize> = (0..len).collect();
            let tree = build_kdtree(&indices, palette);
            *cache = Some(KdCacheEntry {
                palette_ptr: ptr,
                palette_len: len,
                tree,
            });
        }

        let entry = cache.as_ref().unwrap();
        let first_dist = distance_sq(color, &palette[0]);
        let mut best = (0usize, first_dist);
        entry.tree.nearest(color, palette, &mut best);
        best.0 as u8
    })
}

/// Find the nearest palette index using a *weighted* Euclidean distance
/// (3×R, 4×G, 2×B). Always uses linear scan (no k-d tree caching).
///
/// This is slightly slower but produces perceptually better results.
/// Use it when palette building, not for per-pixel mapping in dithering.
#[inline]
pub fn find_nearest_weighted(color: &[u8; 4], palette: &[[u8; 4]]) -> u8 {
    palette
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| distance_sq_weighted(color, p))
        .map(|(i, _)| i as u8)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_scan_small_palette() {
        let pal = [[0u8, 0, 0, 255], [255, 255, 255, 255]];
        assert_eq!(find_nearest_index(&[10, 10, 10, 255], &pal), 0);
        assert_eq!(find_nearest_index(&[200, 200, 200, 255], &pal), 1);
    }

    #[test]
    fn kdtree_large_palette() {
        // Build a 64-entry palette
        let mut pal = Vec::new();
        for i in 0..64u8 {
            pal.push([
                i.wrapping_mul(4),
                i.wrapping_mul(7),
                i.wrapping_mul(11),
                255,
            ]);
        }
        // Query should find exact match
        for (idx, entry) in pal.iter().enumerate() {
            let result = find_nearest_index(entry, &pal);
            assert_eq!(result as usize, idx, "k-d tree mismatch at index {idx}");
        }
    }

    #[test]
    fn parity_with_linear() {
        let mut pal = Vec::new();
        for i in 0u16..64 {
            let v = i as u8;
            pal.push([
                v.wrapping_mul(4),
                v.wrapping_mul(7),
                v.wrapping_mul(11),
                255,
            ]);
        }
        // Test against linear scan for a batch of random-ish colours
        for r in 0..10u8 {
            for g in 0..10u8 {
                for b in 0..10u8 {
                    let c = [r * 25, g * 25, b * 25, 255];
                    let kd = find_nearest_index(&c, &pal);
                    let lin = find_nearest_linear(&c, &pal);
                    assert_eq!(kd, lin, "parity fail for {:?}: kd={kd}, lin={lin}", c);
                }
            }
        }
    }

    #[test]
    fn empty_palette_returns_zero() {
        assert_eq!(find_nearest_index(&[0, 0, 0, 255], &[]), 0);
    }

    #[test]
    fn single_palette() {
        let pal = [[128, 128, 128, 255]];
        assert_eq!(find_nearest_index(&[0, 0, 0, 255], &pal), 0);
        assert_eq!(find_nearest_index(&[255, 255, 255, 255], &pal), 0);
    }

    #[test]
    fn weighted_distance_prefers_green() {
        let pal = [[100, 0, 0, 255], [0, 100, 0, 255], [0, 0, 100, 255]];
        // With 3×R, 4×G, 2×B weights, a moderately green query maps to green.
        let query = [0, 60, 0, 255];
        let idx = find_nearest_weighted(&query, &pal);
        assert_eq!(
            idx, 1,
            "green query should map to green entry, got index {idx}"
        );
    }

    #[test]
    fn tie_breaking_lower_index() {
        // Two equidistant colours: both should resolve to the lower index.
        let pal = [[0, 0, 0, 255], [100, 0, 0, 255], [100, 0, 0, 255]];
        let query = [100, 0, 0, 255];
        // Entries 1 and 2 are identical; tie-breaking favours index 1.
        assert_eq!(find_nearest_index(&query, &pal), 1);
    }

    #[test]
    fn transparent_handling() {
        let pal = [[0, 0, 0, 0], [255, 0, 0, 255]];
        // Full-alpha query should prefer the opaque colour
        let query = [200, 0, 0, 255];
        assert_eq!(find_nearest_index(&query, &pal), 1);
    }
}
